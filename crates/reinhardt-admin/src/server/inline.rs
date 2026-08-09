use crate::core::InlineModelAdmin;
use crate::core::inline::{InlineMutationError, InlineRowMutation, MAX_INLINE_ROWS};
use serde_json::Value;
use std::collections::{BTreeMap, HashMap, HashSet};

const INLINE_PREFIX: &str = "__reinhardt_inlines";
const INLINE_CONTROL_PREFIX: &str = "__reinhardt_inlines.";
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
		.filter(|(name, _)| {
			name.as_str() == INLINE_PREFIX || name.starts_with(INLINE_CONTROL_PREFIX)
		})
		.map(|(name, value)| (name.clone(), value.clone()))
		.collect::<serde_json::Map<_, _>>();
	let payload_bytes = serde_json::to_vec(&reserved)
		.map_err(|error| InlineMutationError::Validation(error.to_string()))?
		.len();
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
			"__id" => {
				row.id = parse_id(value)?
					.map(|id| inline.adapter().normalize_child_id(&id))
					.transpose()?;
			}
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
	if rows.len() > MAX_INLINE_ROWS {
		return Err(InlineMutationError::Validation(
			"inline submission exceeds 100 rows".to_owned(),
		));
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
	for name in reserved.keys() {
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

	#[model(
		app_label = "admin",
		table_name = "parser_other_children",
		form = true,
		info = false
	)]
	#[derive(Clone, Deserialize, Serialize)]
	struct OtherChild {
		#[field(primary_key = true)]
		id: Option<i64>,
		#[rel(foreign_key, related_name = "other_children")]
		parent: ForeignKeyField<Parent>,
		#[field(max_length = 100)]
		name: String,
	}

	fn inline() -> InlineModelAdmin {
		InlineModelAdmin::new::<Parent, Child>("Child", "parent_id", &["name"]).unwrap()
	}

	fn other_inline() -> InlineModelAdmin {
		InlineModelAdmin::new::<Parent, OtherChild>("Other child", "parent_id", &["name"]).unwrap()
	}

	fn assert_validation(error: InlineMutationError, expected: &str) {
		let InlineMutationError::Validation(message) = error else {
			panic!("expected inline validation error");
		};
		assert_eq!(message, expected);
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
	#[case(
		"__reinhardt_inlines.parser_children-parent_id.0",
		json!("x"),
		"malformed inline path '__reinhardt_inlines.parser_children-parent_id.0'"
	)]
	#[case(
		"__reinhardt_inlines.parser_children-parent_id.nope.name",
		json!("x"),
		"invalid inline row 'nope'"
	)]
	#[case(
		"__reinhardt_inlines.unknown.0.name",
		json!("x"),
		"unknown inline key 'unknown'"
	)]
	#[case(
		"__reinhardt_inlines.parser_children-parent_id.0.unknown",
		json!("x"),
		"unknown inline field 'unknown'"
	)]
	#[case(
		"__reinhardt_inlines.parser_children-parent_id.0.parent_id",
		json!("1"),
		"inline foreign key 'parent_id' cannot be submitted"
	)]
	fn parser_rejects_malformed_or_untrusted_paths(
		#[case] path: &str,
		#[case] value: serde_json::Value,
		#[case] expected: &str,
	) {
		let mut data = HashMap::from([(path.to_owned(), value)]);

		let error = parse_inline_mutations(&mut data, &[inline()]).unwrap_err();

		assert_validation(error, expected);
		assert!(data.contains_key(path));
	}

	#[rstest]
	fn parser_rejects_duplicate_ids_after_primary_key_normalization() {
		let inline = inline();
		let key = inline.key();
		let mut data = HashMap::from([
			(format!("__reinhardt_inlines.{key}.0.__id"), json!("07")),
			(format!("__reinhardt_inlines.{key}.1.__id"), json!("7")),
		]);

		let error = parse_inline_mutations(&mut data, &[inline]).unwrap_err();

		assert_validation(error, "inline child ID '7' is submitted more than once");
	}

	#[rstest]
	fn parser_rejects_rows_at_or_above_the_limit() {
		let inline = inline();
		let key = inline.key();
		let mut data = HashMap::from([(
			format!("__reinhardt_inlines.{key}.{MAX_INLINE_ROWS}.name"),
			json!("overflow"),
		)]);

		let error = parse_inline_mutations(&mut data, &[inline]).unwrap_err();

		assert_validation(error, "inline row index '100' exceeds the limit");
	}

	#[rstest]
	fn parser_rejects_the_exact_reserved_prefix_without_a_control_path() {
		let mut data = HashMap::from([(INLINE_PREFIX.to_owned(), json!({"unexpected": true}))]);

		let error = parse_inline_mutations(&mut data, &[inline()]).unwrap_err();

		assert_validation(error, "malformed inline path '__reinhardt_inlines'");
		assert!(data.contains_key(INLINE_PREFIX));
	}

	#[rstest]
	fn parser_leaves_reserved_prefix_lookalikes_as_parent_data() {
		let path = "__reinhardt_inlines_extra.parser_children-parent_id.0.name";
		let mut data = HashMap::from([(path.to_owned(), json!("parent value"))]);

		let parsed = parse_inline_mutations(&mut data, &[inline()]).unwrap();

		assert!(parsed.is_empty());
		assert_eq!(data.get(path), Some(&json!("parent value")));
	}

	#[rstest]
	fn parser_accepts_one_hundred_rows_across_two_inlines() {
		let first = inline();
		let second = other_inline();
		let first_key = first.key().to_owned();
		let second_key = second.key().to_owned();
		let mut data = HashMap::new();
		for index in 0..50 {
			data.insert(
				format!("{INLINE_PREFIX}.{}.{}.name", first.key(), index),
				json!("first"),
			);
			data.insert(
				format!("{INLINE_PREFIX}.{}.{}.name", second.key(), index),
				json!("second"),
			);
		}

		let parsed = parse_inline_mutations(&mut data, &[first, second]).unwrap();

		assert_eq!(parsed.len(), 2);
		assert_eq!(parsed[0].key, first_key);
		assert_eq!(parsed[0].rows.len(), 50);
		assert_eq!(parsed[1].key, second_key);
		assert_eq!(parsed[1].rows.len(), 50);
		assert!(data.is_empty());
	}

	#[rstest]
	fn parser_rejects_more_than_one_hundred_rows_across_two_inlines() {
		let first = inline();
		let second = other_inline();
		let mut data = HashMap::new();
		for index in 0..51 {
			data.insert(
				format!("{INLINE_PREFIX}.{}.{}.name", first.key(), index),
				json!("first"),
			);
		}
		for index in 0..50 {
			data.insert(
				format!("{INLINE_PREFIX}.{}.{}.name", second.key(), index),
				json!("second"),
			);
		}

		let error = parse_inline_mutations(&mut data, &[first, second]).unwrap_err();

		assert_validation(error, "inline submission exceeds 100 rows");
		assert_eq!(data.len(), 101);
	}

	fn payload_at_serialized_size(
		inline: &InlineModelAdmin,
		size: usize,
	) -> HashMap<String, Value> {
		let path = format!("{INLINE_PREFIX}.{}.0.name", inline.key());
		let escaped_prefix = "\"\\\n";
		let mut data = HashMap::from([(path.clone(), json!(escaped_prefix))]);
		let serialized_prefix_size = serde_json::to_vec(&data).unwrap().len();
		let filler = "x".repeat(size - serialized_prefix_size);
		data.insert(path, json!(format!("{escaped_prefix}{filler}")));
		assert_eq!(serde_json::to_vec(&data).unwrap().len(), size);
		data
	}

	#[rstest]
	fn parser_accepts_the_exact_serialized_payload_limit() {
		let inline = inline();
		let mut data = payload_at_serialized_size(&inline, MAX_INLINE_PAYLOAD_BYTES);

		let parsed = parse_inline_mutations(&mut data, &[inline]).unwrap();

		assert_eq!(parsed.len(), 1);
		assert_eq!(parsed[0].rows.len(), 1);
		assert!(data.is_empty());
	}

	#[rstest]
	fn parser_rejects_one_byte_over_the_serialized_payload_limit_without_mutation() {
		let inline = inline();
		let mut data = payload_at_serialized_size(&inline, MAX_INLINE_PAYLOAD_BYTES + 1);
		let original = data.clone();

		let error = parse_inline_mutations(&mut data, &[inline]).unwrap_err();

		assert_validation(error, "inline payload exceeds 10 MiB");
		assert_eq!(data, original);
	}
}
