use crate::core::database::AdminCreateResult;
use crate::core::history::insert_history_event;
use crate::core::inline::{
	InlineMutationError, InlineRowMutation, InlineSaveOperation, InlineSaveOutcome, MAX_INLINE_ROWS,
};
use crate::core::{AdminSite, AdminUser, InlineModelAdmin};
use crate::server::audit;
use crate::server::error::{AdminAuth, IntoServerFnError, MapServerFnError, ModelPermission};
use crate::server::security::sanitize_mutation_values;
use crate::types::AdminError;
use reinhardt_db::orm::{DatabaseConnection, transaction::AtomicTransaction};
use reinhardt_pages::server_fn::ServerFnError;
use serde_json::Value;
use std::collections::{BTreeMap, HashMap, HashSet};
use thiserror::Error;

const INLINE_PREFIX: &str = "__reinhardt_inlines";
const INLINE_CONTROL_PREFIX: &str = "__reinhardt_inlines.";
const MAX_INLINE_PAYLOAD_BYTES: usize = 10 * 1024 * 1024;

#[derive(Debug)]
pub(crate) struct ParsedInlineMutations {
	pub(crate) key: String,
	pub(crate) rows: Vec<InlineRowMutation>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum InlinePermission {
	Add,
	Change,
	Delete,
}

impl From<InlinePermission> for ModelPermission {
	fn from(permission: InlinePermission) -> Self {
		match permission {
			InlinePermission::Add => Self::Add,
			InlinePermission::Change => Self::Change,
			InlinePermission::Delete => Self::Delete,
		}
	}
}

/// Error type shared by parent and inline writes inside one atomic callback.
#[derive(Debug, Error)]
pub(crate) enum InlineTransactionError {
	#[error(transparent)]
	Admin(#[from] AdminError),
	#[error(transparent)]
	Inline(#[from] InlineMutationError),
	#[error(transparent)]
	Core(#[from] reinhardt_core::exception::Error),
}

#[derive(Default)]
struct PartialInlineRow {
	id: Option<String>,
	values: HashMap<String, Value>,
	delete: bool,
	present: bool,
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
			"__present" => row.present = parse_delete(value)?,
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
		if row.id.is_none() && !row.delete && !row.present && row.values.values().all(blank_value) {
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

/// Apply the existing mutation sanitizer to every submitted child value.
pub(crate) fn sanitize_inline_mutations(mutations: &mut [ParsedInlineMutations]) {
	for mutation in mutations {
		for row in &mut mutation.rows {
			sanitize_mutation_values(&mut row.values);
		}
	}
}

/// Remove existing inline rows whose submitted values match the stored row.
pub(crate) async fn remove_unchanged_inline_mutations(
	inlines: &[InlineModelAdmin],
	parent_id: &str,
	mutations: &mut [ParsedInlineMutations],
	connection: &mut DatabaseConnection,
) -> Result<(), InlineMutationError> {
	for inline in inlines {
		let Some(mutation_index) = mutations
			.iter()
			.position(|mutation| mutation.key == inline.key())
		else {
			continue;
		};
		let original_values = inline
			.adapter()
			.load_rows(parent_id, MAX_INLINE_ROWS + 1, connection)
			.await?
			.into_iter()
			.filter_map(|row| row.id.map(|id| (id, row.values)))
			.collect::<HashMap<_, _>>();
		let mutation = &mut mutations[mutation_index];
		let mut unchanged_indices = HashSet::new();
		for row in &mutation.rows {
			let Some(id) = row.id.as_deref() else {
				continue;
			};
			if row.delete {
				continue;
			}
			let Some(original) = original_values.get(id) else {
				continue;
			};
			let normalized = inline.adapter().normalize_row_values(&row.values)?;
			if normalized
				.iter()
				.all(|(field, value)| original.get(field) == Some(value))
			{
				unchanged_indices.insert(row.submitted_index);
			}
		}
		mutation
			.rows
			.retain(|row| !unchanged_indices.contains(&row.submitted_index));
	}
	Ok(())
}

fn classify_inline_permissions(
	inlines: &[InlineModelAdmin],
	mutations: &[ParsedInlineMutations],
) -> Result<Vec<(String, InlinePermission)>, InlineMutationError> {
	let rows_by_key = mutations
		.iter()
		.map(|mutation| (mutation.key.as_str(), mutation.rows.as_slice()))
		.collect::<HashMap<_, _>>();
	let mut seen = HashSet::new();
	let mut permissions = Vec::new();

	for inline in inlines {
		for row in rows_by_key.get(inline.key()).copied().unwrap_or_default() {
			let permission = match (&row.id, row.delete) {
				(None, false) => InlinePermission::Add,
				(Some(_), false) => InlinePermission::Change,
				(Some(_), true) if inline.delete_enabled() => InlinePermission::Delete,
				(Some(_), true) => {
					return Err(InlineMutationError::Validation(format!(
						"inline deletion is disabled for '{}'",
						inline.key()
					)));
				}
				(None, true) => {
					return Err(InlineMutationError::Validation(
						"a new inline row cannot be deleted".to_owned(),
					));
				}
			};
			let identity = inline.adapter().table_name().to_ascii_lowercase();
			if seen.insert((identity, permission)) {
				permissions.push((inline.adapter().table_name().to_owned(), permission));
			}
		}
	}

	Ok(permissions)
}

/// Resolve every configured child admin and authorize each requested operation class once.
pub(crate) async fn preflight_inline_permissions(
	auth: &AdminAuth,
	site: &AdminSite,
	user: &dyn AdminUser,
	inlines: &[InlineModelAdmin],
	mutations: &[ParsedInlineMutations],
) -> Result<(), ServerFnError> {
	let mut child_admins = HashMap::new();
	for inline in inlines {
		let configured_identity = inline.adapter().table_name().to_owned();
		if let std::collections::hash_map::Entry::Vacant(e) =
			child_admins.entry(configured_identity)
		{
			let child_admin = site
				.get_model_admin_by_table_name(inline.adapter().table_name())
				.map_server_fn_error()?;
			e.insert(child_admin);
		}
	}
	let rows_by_key = mutations
		.iter()
		.map(|mutation| (mutation.key.as_str(), mutation.rows.as_slice()))
		.collect::<HashMap<_, _>>();
	let mut readonly_errors = HashMap::new();
	for inline in inlines {
		let child_admin = child_admins
			.get(inline.adapter().table_name())
			.ok_or_else(|| ServerFnError::server(500, "Inline configuration resolution failed"))?;
		inline
			.validate_child_table(child_admin.table_name())
			.map_server_fn_error()?;
		let readonly_fields = child_admin.readonly_fields();
		for row in rows_by_key.get(inline.key()).copied().unwrap_or_default() {
			for field in inline.fields() {
				if readonly_fields.contains(&field.as_str()) && row.values.contains_key(field) {
					readonly_errors.insert(
						format!("{}.{}.{}", inline.key(), row.submitted_index, field),
						vec![format!(
							"Field '{field}' is read-only and cannot be modified"
						)],
					);
				}
			}
		}
	}
	if !readonly_errors.is_empty() {
		return Err(map_inline_mutation_error(
			InlineMutationError::RowValidation {
				errors: readonly_errors,
			},
		));
	}

	let permissions =
		classify_inline_permissions(inlines, mutations).map_err(map_inline_mutation_error)?;
	for (table_name, permission) in permissions {
		let child_admin = child_admins
			.get(&table_name)
			.ok_or_else(|| ServerFnError::server(500, "Inline configuration resolution failed"))?;
		auth.require_model_permission(child_admin.as_ref(), user, permission.into())
			.await?;
	}

	Ok(())
}

/// Save configured child groups sequentially on the caller-owned transaction.
pub(crate) async fn save_inline_mutations(
	inlines: &[InlineModelAdmin],
	parent_id: &str,
	mutations: Vec<ParsedInlineMutations>,
	transaction: &mut AtomicTransaction,
) -> Result<Vec<InlineSaveOutcome>, InlineTransactionError> {
	let mut rows_by_key = mutations
		.into_iter()
		.map(|mutation| (mutation.key, mutation.rows))
		.collect::<HashMap<_, _>>();
	let mut outcomes = Vec::new();
	for inline in inlines {
		let Some(rows) = rows_by_key.remove(inline.key()) else {
			continue;
		};
		outcomes.extend(
			inline
				.adapter()
				.save_rows(inline.key(), parent_id, rows, transaction)
				.await?,
		);
	}
	Ok(outcomes)
}

/// Persist committed child outcomes using each registered model's canonical identity.
pub(crate) async fn insert_inline_history_events(
	site: &AdminSite,
	actor: &str,
	outcomes: &[InlineSaveOutcome],
	transaction: &mut AtomicTransaction,
) -> Result<(), InlineTransactionError> {
	for outcome in outcomes {
		let child_admin = site.get_model_admin_by_table_name(&outcome.table_name)?;
		if child_admin.table_name() != outcome.table_name {
			return Err(AdminError::ValidationError(
				"inline outcome does not match the registered model".to_owned(),
			)
			.into());
		}
		let action_name = match outcome.operation {
			InlineSaveOperation::Create => "CREATE",
			InlineSaveOperation::Update => "UPDATE",
			InlineSaveOperation::Delete => "DELETE",
		};
		let event = audit::new_history_event(
			actor,
			action_name,
			child_admin.model_name(),
			child_admin.table_name(),
			&outcome.object_id,
			outcome.changed_fields.clone(),
			1,
		);
		insert_history_event(transaction, &event).await?;
	}

	Ok(())
}

/// Convert the returned parent key while preserving the legacy affected count.
pub(crate) fn created_parent_identity(
	created: &AdminCreateResult,
) -> Result<(String, u64), InlineTransactionError> {
	match &created.primary_key {
		Value::Number(number) => number
			.as_u64()
			.map(|id| (id.to_string(), id))
			.ok_or_else(|| {
				AdminError::DatabaseError("created parent primary key is not unsigned".to_owned())
					.into()
			}),
		Value::String(id) => Ok((id.clone(), created.affected)),
		_ => Err(AdminError::DatabaseError(
			"created parent primary key is not a string or unsigned integer".to_owned(),
		)
		.into()),
	}
}

/// Map inline failures without exposing persistence diagnostics.
pub(crate) fn map_inline_mutation_error(error: InlineMutationError) -> ServerFnError {
	match error {
		InlineMutationError::Validation(message) => ServerFnError::application(message),
		InlineMutationError::RowValidation { errors } => {
			let mut errors = errors.into_iter().collect::<Vec<_>>();
			errors.sort_unstable_by(|left, right| left.0.cmp(&right.0));
			ServerFnError::validation_with_message(
				"Inline row validation failed",
				errors.into_iter().flat_map(|(field, messages)| {
					messages
						.into_iter()
						.map(move |message| (field.clone(), message))
				}),
			)
		}
		InlineMutationError::Persistence(_) => {
			ServerFnError::server(500, "Inline persistence failed")
		}
	}
}

/// Map atomic orchestration failures to client-safe server-function errors.
pub(crate) fn map_inline_transaction_error(error: InlineTransactionError) -> ServerFnError {
	match error {
		InlineTransactionError::Admin(error) => error.into_server_fn_error(),
		InlineTransactionError::Inline(error) => map_inline_mutation_error(error),
		InlineTransactionError::Core(_) => {
			ServerFnError::server(500, "Database transaction failed")
		}
	}
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
		Value::Bool(value) => !*value,
		Value::Array(values) => values.is_empty(),
		_ => false,
	}
}

#[cfg(all(test, server))]
mod tests {
	use super::*;
	use crate::core::database::AdminCreateResult;
	use crate::types::AdminError;
	use async_trait::async_trait;
	use reinhardt_db::associations::ForeignKeyField;
	use reinhardt_http::AuthState;
	use reinhardt_macros::model;
	use reinhardt_pages::server_fn::ServerFnErrorKind;
	use rstest::rstest;
	use serde::{Deserialize, Serialize};
	use serde_json::json;
	use std::collections::HashMap;
	use std::future::Future;
	use std::sync::Arc;

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

	struct TestUser;

	impl AdminUser for TestUser {
		fn is_active(&self) -> bool {
			true
		}

		fn is_staff(&self) -> bool {
			true
		}

		fn is_superuser(&self) -> bool {
			false
		}

		fn get_username(&self) -> &str {
			"test-user"
		}
	}

	struct OperationPermissionAdmin {
		table_name: &'static str,
		denied: Option<InlinePermission>,
		readonly: bool,
	}

	#[async_trait]
	impl crate::core::ModelAdmin for OperationPermissionAdmin {
		fn model_name(&self) -> &str {
			"Shared child name"
		}

		fn table_name(&self) -> &str {
			self.table_name
		}

		async fn has_add_permission(&self, _: &dyn AdminUser) -> bool {
			self.denied != Some(InlinePermission::Add)
		}

		async fn has_change_permission(&self, _: &dyn AdminUser) -> bool {
			self.denied != Some(InlinePermission::Change)
		}

		async fn has_delete_permission(&self, _: &dyn AdminUser) -> bool {
			self.denied != Some(InlinePermission::Delete)
		}

		fn readonly_fields(&self) -> Vec<&str> {
			if self.readonly {
				vec!["name"]
			} else {
				Vec::new()
			}
		}
	}

	fn authenticated_admin_auth() -> AdminAuth {
		let request = reinhardt_http::Request::builder()
			.uri("/admin/test")
			.build()
			.unwrap();
		request
			.extensions
			.insert(AuthState::authenticated("user-1", true, true));
		AdminAuth::from_arc_request(&Arc::new(request))
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
		assert!(parsed[0].rows[0].delete);
	}

	#[rstest]
	#[case(json!(false))]
	#[case(json!([]))]
	fn parser_ignores_untouched_checkbox_and_multi_select_extra_rows(#[case] value: Value) {
		let inline = inline();
		let mut data = HashMap::from([(format!("{INLINE_PREFIX}.{}.0.name", inline.key()), value)]);

		let parsed = parse_inline_mutations(&mut data, &[inline]).unwrap();

		assert!(parsed.is_empty());
		assert!(data.is_empty());
	}

	#[rstest]
	fn parser_preserves_an_explicitly_present_false_value() {
		let inline = inline();
		let mut data = HashMap::from([
			(
				format!("{INLINE_PREFIX}.{}.0.__present", inline.key()),
				json!(true),
			),
			(
				format!("{INLINE_PREFIX}.{}.0.name", inline.key()),
				json!(false),
			),
		]);

		let parsed = parse_inline_mutations(&mut data, &[inline]).unwrap();

		assert_eq!(parsed.len(), 1);
		assert_eq!(parsed[0].rows.len(), 1);
		assert_eq!(parsed[0].rows[0].values.get("name"), Some(&json!(false)));
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

	#[rstest]
	fn permission_classifier_deduplicates_each_child_operation() {
		let inline = inline().can_delete(true);
		let mutations = vec![ParsedInlineMutations {
			key: inline.key().to_owned(),
			rows: vec![
				InlineRowMutation {
					submitted_index: 0,
					id: None,
					values: HashMap::from([("name".to_owned(), json!("first"))]),
					delete: false,
				},
				InlineRowMutation {
					submitted_index: 1,
					id: None,
					values: HashMap::from([("name".to_owned(), json!("second"))]),
					delete: false,
				},
				InlineRowMutation {
					submitted_index: 2,
					id: Some("7".to_owned()),
					values: HashMap::from([("name".to_owned(), json!("changed"))]),
					delete: false,
				},
				InlineRowMutation {
					submitted_index: 3,
					id: Some("8".to_owned()),
					values: HashMap::new(),
					delete: true,
				},
			],
		}];

		let permissions = classify_inline_permissions(&[inline], &mutations).unwrap();

		assert_eq!(
			permissions,
			vec![
				("Child".to_owned(), InlinePermission::Add),
				("Child".to_owned(), InlinePermission::Change),
				("Child".to_owned(), InlinePermission::Delete),
			]
		);
	}

	#[rstest]
	#[tokio::test]
	async fn preflight_checks_each_configured_child_admin_when_display_names_match() {
		let site = AdminSite::new("Test admin");
		site.register(
			"Child",
			OperationPermissionAdmin {
				table_name: "parser_children",
				denied: None,
				readonly: false,
			},
		)
		.unwrap();
		site.register(
			"Other child",
			OperationPermissionAdmin {
				table_name: "parser_other_children",
				denied: Some(InlinePermission::Add),
				readonly: false,
			},
		)
		.unwrap();
		let inlines = vec![inline(), other_inline()];
		let mutations = inlines
			.iter()
			.map(|inline| ParsedInlineMutations {
				key: inline.key().to_owned(),
				rows: vec![InlineRowMutation {
					submitted_index: 0,
					id: None,
					values: HashMap::from([("name".to_owned(), json!("new child"))]),
					delete: false,
				}],
			})
			.collect::<Vec<_>>();

		let error = preflight_inline_permissions(
			&authenticated_admin_auth(),
			&site,
			&TestUser,
			&inlines,
			&mutations,
		)
		.await
		.unwrap_err();

		assert_eq!(error.kind(), ServerFnErrorKind::Server);
		assert_eq!(error.status(), Some(403));
		assert_eq!(error.user_message(), "Permission denied");
	}

	#[rstest]
	#[tokio::test]
	async fn preflight_rejects_submitted_readonly_child_fields_as_row_errors() {
		let site = AdminSite::new("Test admin");
		site.register(
			"Child",
			OperationPermissionAdmin {
				table_name: "parser_children",
				denied: None,
				readonly: true,
			},
		)
		.unwrap();
		let inline = inline();
		let mutations = vec![ParsedInlineMutations {
			key: inline.key().to_owned(),
			rows: vec![InlineRowMutation {
				submitted_index: 4,
				id: Some("7".to_owned()),
				values: HashMap::from([("name".to_owned(), json!("tampered"))]),
				delete: false,
			}],
		}];

		let error = preflight_inline_permissions(
			&authenticated_admin_auth(),
			&site,
			&TestUser,
			std::slice::from_ref(&inline),
			&mutations,
		)
		.await
		.unwrap_err();

		assert_eq!(error.kind(), ServerFnErrorKind::Validation);
		assert_eq!(error.status(), Some(422));
		assert_eq!(error.field_errors().len(), 1);
		assert_eq!(
			error.field_errors()[0].field(),
			format!("{}.4.name", inline.key())
		);
		assert_eq!(
			error.field_errors()[0].message(),
			"Field 'name' is read-only and cannot be modified"
		);
	}

	#[rstest]
	#[case::add(InlinePermission::Add, None, false)]
	#[case::change(InlinePermission::Change, Some("7"), false)]
	#[case::delete(InlinePermission::Delete, Some("7"), true)]
	#[tokio::test]
	async fn preflight_rejects_each_denied_child_operation(
		#[case] denied: InlinePermission,
		#[case] id: Option<&str>,
		#[case] delete: bool,
	) {
		let site = AdminSite::new("Test admin");
		site.register(
			"Child",
			OperationPermissionAdmin {
				table_name: "parser_children",
				denied: Some(denied),
				readonly: false,
			},
		)
		.unwrap();
		let inline = inline().can_delete(true);
		let mutations = vec![ParsedInlineMutations {
			key: inline.key().to_owned(),
			rows: vec![InlineRowMutation {
				submitted_index: 0,
				id: id.map(str::to_owned),
				values: HashMap::from([("name".to_owned(), json!("child"))]),
				delete,
			}],
		}];

		let error = preflight_inline_permissions(
			&authenticated_admin_auth(),
			&site,
			&TestUser,
			&[inline],
			&mutations,
		)
		.await
		.unwrap_err();

		assert_eq!(error.kind(), ServerFnErrorKind::Server);
		assert_eq!(error.status(), Some(403));
		assert_eq!(error.user_message(), "Permission denied");
	}

	#[rstest]
	#[tokio::test]
	async fn preflight_rejects_admin_registered_for_a_different_typed_table() {
		let site = AdminSite::new("Test admin");
		site.register(
			"Child",
			OperationPermissionAdmin {
				table_name: "parser_other_children",
				denied: None,
				readonly: false,
			},
		)
		.unwrap();
		let inline = inline();
		let mutations = vec![ParsedInlineMutations {
			key: inline.key().to_owned(),
			rows: vec![InlineRowMutation {
				submitted_index: 0,
				id: None,
				values: HashMap::from([("name".to_owned(), json!("child"))]),
				delete: false,
			}],
		}];

		let error = preflight_inline_permissions(
			&authenticated_admin_auth(),
			&site,
			&TestUser,
			&[inline],
			&mutations,
		)
		.await
		.unwrap_err();

		assert_eq!(error.kind(), ServerFnErrorKind::Application);
		assert_eq!(
			error.user_message(),
			"inline child 'Child' resolves to table 'parser_other_children', expected 'parser_children'"
		);
	}

	#[rstest]
	fn permission_classifier_rejects_disabled_deletion() {
		let inline = inline();
		let key = inline.key().to_owned();
		let mutations = vec![ParsedInlineMutations {
			key: key.clone(),
			rows: vec![InlineRowMutation {
				submitted_index: 0,
				id: Some("7".to_owned()),
				values: HashMap::new(),
				delete: true,
			}],
		}];

		let error = classify_inline_permissions(&[inline], &mutations).unwrap_err();

		assert_validation(error, &format!("inline deletion is disabled for '{key}'"));
	}

	#[rstest]
	fn sanitizer_applies_existing_xss_rules_to_child_values() {
		let mut mutations = vec![ParsedInlineMutations {
			key: inline().key().to_owned(),
			rows: vec![InlineRowMutation {
				submitted_index: 0,
				id: None,
				values: HashMap::from([(
					"name".to_owned(),
					json!("<script>alert('xss')</script>"),
				)]),
				delete: false,
			}],
		}];

		sanitize_inline_mutations(&mut mutations);

		assert_eq!(
			mutations[0].rows[0].values["name"],
			json!("&lt;script&gt;alert(&#x27;xss&#x27;)&lt;/script&gt;")
		);
	}

	#[rstest]
	fn inline_row_errors_map_to_structured_server_validation() {
		let error = map_inline_mutation_error(InlineMutationError::RowValidation {
			errors: HashMap::from([(
				"parser_children-parent_id.3.name".to_owned(),
				vec!["Name is required".to_owned(), "Name is invalid".to_owned()],
			)]),
		});

		assert_eq!(error.kind(), ServerFnErrorKind::Validation);
		assert_eq!(error.status(), Some(422));
		assert_eq!(error.user_message(), "Inline row validation failed");
		assert_eq!(error.field_errors().len(), 2);
		assert_eq!(
			error.field_errors()[0].field(),
			"parser_children-parent_id.3.name"
		);
		assert_eq!(error.field_errors()[0].message(), "Name is required");
		assert_eq!(error.field_errors()[1].message(), "Name is invalid");
	}

	#[rstest]
	fn inline_persistence_and_core_errors_hide_internal_details() {
		let persistence = map_inline_mutation_error(InlineMutationError::Persistence(
			"postgres password=secret".to_owned(),
		));
		let transaction = map_inline_transaction_error(InlineTransactionError::from(
			reinhardt_core::exception::Error::Internal("driver secret".to_owned()),
		));

		assert_eq!(persistence.kind(), ServerFnErrorKind::Server);
		assert_eq!(persistence.status(), Some(500));
		assert_eq!(persistence.user_message(), "Inline persistence failed");
		assert_eq!(transaction.kind(), ServerFnErrorKind::Server);
		assert_eq!(transaction.status(), Some(500));
		assert_eq!(transaction.user_message(), "Database transaction failed");
	}

	#[rstest]
	#[case(json!(42), 1, Ok(("42".to_owned(), 42)))]
	#[case(json!("parent-uuid"), 1, Ok(("parent-uuid".to_owned(), 1)))]
	#[case(json!(-1), 1, Err(()))]
	#[case(Value::Bool(true), 1, Err(()))]
	fn created_parent_identity_preserves_legacy_affected_count(
		#[case] primary_key: Value,
		#[case] affected: u64,
		#[case] expected: Result<(String, u64), ()>,
	) {
		let created = AdminCreateResult {
			affected,
			primary_key,
		};

		let actual = created_parent_identity(&created).map_err(|_| ());

		assert_eq!(actual, expected);
	}

	#[rstest]
	fn admin_database_errors_remain_sanitized_through_transaction_mapping() {
		let error = map_inline_transaction_error(InlineTransactionError::from(
			AdminError::DatabaseError("postgres password=secret".to_owned()),
		));

		assert_eq!(error.kind(), ServerFnErrorKind::Server);
		assert_eq!(error.status(), Some(500));
		assert_eq!(error.user_message(), "Database operation failed");
	}

	fn assert_send_future(future: impl Future + Send) {
		drop(future);
	}

	fn assert_create_record_future_is_send(
		model_name: String,
		request: crate::types::MutationRequest,
		site: reinhardt_di::KeyedDepends<crate::core::AdminSiteKey, crate::core::AdminSite>,
		db: reinhardt_di::KeyedDepends<crate::core::AdminDatabaseKey, crate::core::AdminDatabase>,
		http_request: reinhardt_pages::server_fn::ServerFnRequest,
		user: crate::server::AdminAuthenticatedUser,
	) {
		assert_send_future(crate::server::create_record(
			model_name,
			request,
			site,
			db,
			http_request,
			user,
		));
	}

	fn assert_update_record_future_is_send(
		model_name: String,
		id: String,
		request: crate::types::MutationRequest,
		site: reinhardt_di::KeyedDepends<crate::core::AdminSiteKey, crate::core::AdminSite>,
		db: reinhardt_di::KeyedDepends<crate::core::AdminDatabaseKey, crate::core::AdminDatabase>,
		http_request: reinhardt_pages::server_fn::ServerFnRequest,
		user: crate::server::AdminAuthenticatedUser,
	) {
		assert_send_future(crate::server::update_record(
			model_name,
			id,
			request,
			site,
			db,
			http_request,
			user,
		));
	}

	#[rstest]
	fn inline_server_function_futures_are_send() {
		let _create_type_check = assert_create_record_future_is_send;
		let _update_type_check = assert_update_record_future_is_send;
	}
}
