use crate::core::InlineModelAdmin;
use crate::core::inline::{InlineMutationError, InlineRowMutation, MAX_INLINE_ROWS};
use serde_json::Value;
use std::collections::{BTreeMap, HashMap, HashSet};

const INLINE_PREFIX: &str = "__reinhardt_inlines";
const MAX_INLINE_PAYLOAD_BYTES: usize = 10 * 1024 * 1024;

#[derive(Debug)]
pub(crate) struct ParsedInlineMutations {
	pub(crate) key: String,
	pub(crate) rows: Vec<InlineRowMutation>,
}

#[derive(Default)]
struct PartialInlineRow {
	id: Option<String>,
	values: HashMap<String, Value>,
	delete: bool,
}

/// Remove and parse reserved flat inline controls before parent validation.
pub(crate) fn parse_inline_mutations(
	data: &mut HashMap<String, Value>,
	inlines: &[InlineModelAdmin],
) -> Result<Vec<ParsedInlineMutations>, InlineMutationError> {
	InlineModelAdmin::validate_resolved(inlines)
		.map_err(|error| InlineMutationError::Validation(error.to_string()))?;
	let configured = inlines
		.iter()
		.map(|inline| (inline.key(), inline))
		.collect::<HashMap<_, _>>();
	let reserved = data
		.iter()
		.filter(|(name, _)| name.starts_with(&format!("{INLINE_PREFIX}.")))
		.map(|(name, value)| (name.clone(), value.clone()))
		.collect::<Vec<_>>();
	let payload_bytes = reserved.iter().try_fold(0usize, |size, (name, value)| {
		let value_size = serde_json::to_vec(value)
			.map_err(|error| InlineMutationError::Validation(error.to_string()))?
			.len();
		size.checked_add(name.len() + value_size).ok_or_else(|| {
			InlineMutationError::Validation("inline payload size overflow".to_owned())
		})
	})?;
	if payload_bytes > MAX_INLINE_PAYLOAD_BYTES {
		return Err(InlineMutationError::Validation(
			"inline payload exceeds 10 MiB".to_owned(),
		));
	}

	let mut rows = BTreeMap::<(String, usize), PartialInlineRow>::new();
	for (path, value) in &reserved {
		let parts = path.split('.').collect::<Vec<_>>();
		if parts.len() != 4 || parts[0] != INLINE_PREFIX {
			return Err(InlineMutationError::Validation(format!(
				"malformed inline path '{path}'"
			)));
		}
		let inline = configured.get(parts[1]).ok_or_else(|| {
			InlineMutationError::Validation(format!("unknown inline key '{}'", parts[1]))
		})?;
		let index = parts[2].parse::<usize>().map_err(|_| {
			InlineMutationError::Validation(format!("invalid inline row '{}'", parts[2]))
		})?;
		if index.to_string() != parts[2] || index >= MAX_INLINE_ROWS {
			return Err(InlineMutationError::Validation(format!(
				"inline row index '{}' exceeds the limit",
				parts[2]
			)));
		}
		let row = rows.entry((parts[1].to_owned(), index)).or_default();
		match parts[3] {
			"__id" => row.id = parse_id(value)?,
			"__delete" => row.delete = parse_delete(value)?,
			field if field == inline.foreign_key() => {
				return Err(InlineMutationError::Validation(format!(
					"inline foreign key '{field}' cannot be submitted"
				)));
			}
			field if inline.fields().iter().any(|configured| configured == field) => {
				row.values.insert(field.to_owned(), value.clone());
			}
			field => {
				return Err(InlineMutationError::Validation(format!(
					"unknown inline field '{field}'"
				)));
			}
		}
	}

	let mut ids = HashMap::<String, HashSet<String>>::new();
	let mut parsed = BTreeMap::<String, Vec<InlineRowMutation>>::new();
	for ((key, submitted_index), row) in rows {
		if row.id.is_none() && !row.delete && row.values.values().all(blank_value) {
			continue;
		}
		if row.delete && row.id.is_none() {
			return Err(InlineMutationError::Validation(
				"a new inline row cannot be deleted".to_owned(),
			));
		}
		if let Some(id) = &row.id
			&& !ids.entry(key.clone()).or_default().insert(id.clone())
		{
			return Err(InlineMutationError::Validation(format!(
				"inline child ID '{id}' is submitted more than once"
			)));
		}
		parsed.entry(key).or_default().push(InlineRowMutation {
			submitted_index,
			id: row.id,
			values: row.values,
			delete: row.delete,
		});
	}
	for name in reserved.iter().map(|(name, _)| name) {
		data.remove(name);
	}
	Ok(parsed
		.into_iter()
		.map(|(key, rows)| ParsedInlineMutations { key, rows })
		.collect())
}

fn parse_id(value: &Value) -> Result<Option<String>, InlineMutationError> {
	match value {
		Value::String(value) if value.is_empty() => Ok(None),
		Value::String(value) => Ok(Some(value.clone())),
		Value::Number(value) => Ok(Some(value.to_string())),
		_ => Err(InlineMutationError::Validation(
			"inline child ID must be a string or number".to_owned(),
		)),
	}
}

fn parse_delete(value: &Value) -> Result<bool, InlineMutationError> {
	match value {
		Value::Bool(value) => Ok(*value),
		Value::String(value) if matches!(value.as_str(), "true" | "on" | "1") => Ok(true),
		Value::String(value) if matches!(value.as_str(), "false" | "0" | "") => Ok(false),
		_ => Err(InlineMutationError::Validation(
			"inline delete control must be boolean".to_owned(),
		)),
	}
}

fn blank_value(value: &Value) -> bool {
	match value {
		Value::Null => true,
		Value::String(value) => value.trim().is_empty(),
		_ => false,
	}
}

#[cfg(all(test, server))]
mod tests {
	use super::*;
	use reinhardt_db::associations::ForeignKeyField;
	use reinhardt_macros::model;
	use rstest::rstest;
	use serde::{Deserialize, Serialize};
	use serde_json::json;
	use std::collections::HashMap;

	#[model(
		app_label = "admin",
		table_name = "parser_parents",
		form = true,
		info = false
	)]
	#[derive(Clone, Deserialize, Serialize)]
	struct Parent {
		#[field(primary_key = true)]
		id: Option<i64>,
	}

	#[model(
		app_label = "admin",
		table_name = "parser_children",
		form = true,
		info = false
	)]
	#[derive(Clone, Deserialize, Serialize)]
	struct Child {
		#[field(primary_key = true)]
		id: Option<i64>,
		#[rel(foreign_key, related_name = "children")]
		parent: ForeignKeyField<Parent>,
		#[field(max_length = 100)]
		name: String,
	}

	fn inline() -> InlineModelAdmin {
		InlineModelAdmin::new::<Parent, Child>("Child", "parent_id", &["name"]).unwrap()
	}

	#[rstest]
	fn parser_preserves_indices_delete_intent_and_ignores_blank_extra_rows() {
		let inline = inline();
		let key = inline.key().to_owned();
		let mut data = HashMap::from([
			(format!("__reinhardt_inlines.{key}.2.__id"), json!("7")),
			(
				format!("__reinhardt_inlines.{key}.2.__delete"),
				json!("true"),
			),
			(format!("__reinhardt_inlines.{key}.4.name"), json!("")),
			("title".to_owned(), json!("parent")),
		]);

		let parsed = parse_inline_mutations(&mut data, &[inline]).unwrap();

		assert_eq!(data, HashMap::from([("title".to_owned(), json!("parent"))]));
		assert_eq!(parsed.len(), 1);
		assert_eq!(parsed[0].rows.len(), 1);
		assert_eq!(parsed[0].rows[0].submitted_index, 2);
		assert_eq!(parsed[0].rows[0].id.as_deref(), Some("7"));
		assert_eq!(parsed[0].rows[0].delete, true);
	}

	#[rstest]
	#[case("__reinhardt_inlines.parser_children-parent_id.0", json!("x"))]
	#[case("__reinhardt_inlines.parser_children-parent_id.nope.name", json!("x"))]
	#[case("__reinhardt_inlines.unknown.0.name", json!("x"))]
	#[case("__reinhardt_inlines.parser_children-parent_id.0.unknown", json!("x"))]
	#[case("__reinhardt_inlines.parser_children-parent_id.0.parent_id", json!("1"))]
	fn parser_rejects_malformed_or_untrusted_paths(
		#[case] path: &str,
		#[case] value: serde_json::Value,
	) {
		let mut data = HashMap::from([(path.to_owned(), value)]);
		assert!(parse_inline_mutations(&mut data, &[inline()]).is_err());
	}

	#[rstest]
	fn parser_rejects_duplicate_ids() {
		let inline = inline();
		let key = inline.key();
		let mut data = HashMap::from([
			(format!("__reinhardt_inlines.{key}.0.__id"), json!("7")),
			(format!("__reinhardt_inlines.{key}.1.__id"), json!("7")),
		]);
		assert!(parse_inline_mutations(&mut data, &[inline]).is_err());
	}

	#[rstest]
	fn parser_rejects_rows_at_or_above_the_limit() {
		let inline = inline();
		let key = inline.key();
		let mut data = HashMap::from([(
			format!("__reinhardt_inlines.{key}.{MAX_INLINE_ROWS}.name"),
			json!("overflow"),
		)]);

		assert!(parse_inline_mutations(&mut data, &[inline]).is_err());
	}

	#[rstest]
	fn parser_rejects_oversized_payloads_without_removing_parent_data() {
		let inline = inline();
		let key = inline.key();
		let inline_path = format!("__reinhardt_inlines.{key}.0.name");
		let mut data = HashMap::from([
			(
				inline_path.clone(),
				json!("x".repeat(MAX_INLINE_PAYLOAD_BYTES)),
			),
			("title".to_owned(), json!("parent")),
		]);

		assert!(parse_inline_mutations(&mut data, &[inline]).is_err());
		assert_eq!(data.get("title"), Some(&json!("parent")));
		assert!(data.contains_key(&inline_path));
	}
}
