use crate::types::{AdminError, AdminResult, InlineRowInfo, InlineStyle};
use async_trait::async_trait;
use reinhardt_core::model_form::{
	AllEditableModelFields, ModelFormFieldKind, ModelFormPayload, ModelFormPrimaryKey,
	ModelFormPrimaryKeyFields, ModelFormSchema, normalize_native_model_form_value,
};
use reinhardt_db::orm::transaction::AtomicTransaction;
use reinhardt_db::orm::{
	DatabaseConnection, Filter, FilterOperator, FilterValue, Manager, Model, OrmExecutor,
};
use reinhardt_forms::{FormModel, InlineFormSet, ModelForm, ModelFormError};
use serde::Serialize;
use serde_json::{Map, Value};
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::marker::PhantomData;
use std::sync::Arc;
use thiserror::Error;

pub(crate) const MAX_INLINE_ROWS: usize = 100;

/// One parsed inline row mutation with its stable submitted index.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct InlineRowMutation {
	pub(crate) submitted_index: usize,
	pub(crate) id: Option<String>,
	pub(crate) values: HashMap<String, Value>,
	pub(crate) delete: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InlineSaveOperation {
	Create,
	Update,
	Delete,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct InlineSaveOutcome {
	pub(crate) operation: InlineSaveOperation,
	pub(crate) model_identity: String,
	pub(crate) table_name: String,
	pub(crate) object_id: String,
	pub(crate) changed_fields: Vec<String>,
}

/// A typed inline validation or persistence failure.
#[derive(Debug, Error)]
pub(crate) enum InlineMutationError {
	#[error("invalid inline submission: {0}")]
	Validation(String),
	#[error("inline row validation failed")]
	RowValidation {
		errors: HashMap<String, Vec<String>>,
	},
	#[error("inline persistence failed: {0}")]
	Persistence(String),
}

impl From<reinhardt_core::exception::Error> for InlineMutationError {
	fn from(error: reinhardt_core::exception::Error) -> Self {
		Self::Persistence(error.to_string())
	}
}

#[async_trait(?Send)]
pub(crate) trait InlineAdapter: Send + Sync {
	async fn load_rows(
		&self,
		parent_id: &str,
		connection: &mut DatabaseConnection,
	) -> Result<Vec<InlineRowInfo>, InlineMutationError>;

	async fn save_rows(
		&self,
		inline_key: &str,
		parent_id: &str,
		rows: Vec<InlineRowMutation>,
		transaction: &mut AtomicTransaction,
	) -> Result<Vec<InlineSaveOutcome>, InlineMutationError>;
}

/// Cloneable configuration for editing a related child model inline.
#[derive(Clone)]
pub struct InlineModelAdmin {
	key: String,
	child_model: String,
	foreign_key: String,
	fields: Vec<String>,
	style: InlineStyle,
	extra: usize,
	can_delete: bool,
	adapter: Arc<dyn InlineAdapter>,
}

impl fmt::Debug for InlineModelAdmin {
	fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
		formatter
			.debug_struct("InlineModelAdmin")
			.field("key", &self.key)
			.field("child_model", &self.child_model)
			.field("foreign_key", &self.foreign_key)
			.field("fields", &self.fields)
			.field("style", &self.style)
			.field("extra", &self.extra)
			.field("can_delete", &self.can_delete)
			.finish_non_exhaustive()
	}
}

impl InlineModelAdmin {
	/// Create a typed parent-child inline configuration.
	pub fn new<P, C>(
		child_model: impl Into<String>,
		foreign_key: impl Into<String>,
		fields: &[&str],
	) -> AdminResult<Self>
	where
		P: FormModel + ModelFormPrimaryKey + ModelFormPrimaryKeyFields + 'static,
		P::PrimaryKey: Serialize,
		C: FormModel + ModelFormPrimaryKeyFields + 'static,
		C::Data<AllEditableModelFields>: Default,
	{
		let child_model = child_model.into();
		let foreign_key = foreign_key.into();
		validate_typed_configuration::<P, C>(&foreign_key, fields)?;
		let key = format!(
			"{}-{}",
			identifier_part(C::table_name()),
			identifier_part(&foreign_key)
		);
		if key == "-" {
			return Err(AdminError::ValidationError(
				"inline key cannot be empty".to_owned(),
			));
		}
		let fields = fields
			.iter()
			.map(|field| (*field).to_owned())
			.collect::<Vec<String>>();
		Ok(Self {
			key,
			child_model: child_model.clone(),
			foreign_key: foreign_key.clone(),
			fields: fields.clone(),
			style: InlineStyle::Tabular,
			extra: 0,
			can_delete: false,
			adapter: Arc::new(TypedInlineAdapter::<P, C> {
				model_identity: child_model,
				foreign_key,
				fields,
				_marker: PhantomData,
			}),
		})
	}

	/// Set the inline presentation style.
	pub fn style(mut self, style: InlineStyle) -> Self {
		self.style = style;
		self
	}

	/// Set the number of blank child rows, capped by the submission limit.
	pub fn extra(mut self, extra: usize) -> Self {
		self.extra = extra.min(MAX_INLINE_ROWS);
		self
	}

	/// Enable or disable explicit deletion of existing child rows.
	pub fn can_delete(mut self, can_delete: bool) -> Self {
		self.can_delete = can_delete;
		self
	}

	/// Stable key used by flat inline control names.
	pub fn key(&self) -> &str {
		&self.key
	}

	/// Child model display name.
	pub fn child_model(&self) -> &str {
		&self.child_model
	}

	/// Generated relationship identifier on the child model.
	pub fn foreign_key(&self) -> &str {
		&self.foreign_key
	}

	/// Editable child fields.
	pub fn fields(&self) -> &[String] {
		&self.fields
	}

	/// Configured presentation style.
	pub fn style_value(&self) -> InlineStyle {
		self.style
	}

	/// Number of blank rows appended to loaded children.
	pub fn extra_rows(&self) -> usize {
		self.extra
	}

	/// Whether explicit child deletion is enabled.
	pub fn delete_enabled(&self) -> bool {
		self.can_delete
	}

	pub(crate) fn adapter(&self) -> &Arc<dyn InlineAdapter> {
		&self.adapter
	}

	pub(crate) fn validate_resolved(inlines: &[Self]) -> AdminResult<()> {
		let mut keys = HashSet::new();
		for inline in inlines {
			if !keys.insert(inline.key()) {
				return Err(AdminError::ValidationError(format!(
					"inline key '{}' is configured more than once",
					inline.key()
				)));
			}
		}
		Ok(())
	}
}

fn identifier_part(value: &str) -> String {
	value
		.chars()
		.map(|character| {
			if character.is_ascii_alphanumeric() || matches!(character, '_' | '-') {
				character.to_ascii_lowercase()
			} else {
				'_'
			}
		})
		.collect::<String>()
		.trim_matches('_')
		.to_owned()
}

fn validate_typed_configuration<P, C>(foreign_key: &str, fields: &[&str]) -> AdminResult<()>
where
	P: FormModel + ModelFormPrimaryKey + ModelFormPrimaryKeyFields + 'static,
	C: FormModel + ModelFormPrimaryKeyFields + 'static,
{
	let schema = C::Schema::fields();
	let relationship = schema
		.iter()
		.find(|descriptor| descriptor.name == foreign_key)
		.ok_or_else(|| {
			AdminError::ValidationError(format!("inline foreign key '{foreign_key}' is unknown"))
		})?;
	if !relationship.generated_relation_id {
		return Err(AdminError::ValidationError(format!(
			"inline foreign key '{foreign_key}' is not a generated relationship identifier"
		)));
	}
	if !C::Schema::relation_target_matches::<P>(foreign_key) {
		return Err(AdminError::ValidationError(format!(
			"inline foreign key '{foreign_key}' does not target the configured parent"
		)));
	}

	let mut configured = HashSet::new();
	for field in fields {
		if !configured.insert(*field) {
			return Err(AdminError::ValidationError(format!(
				"inline field '{field}' is configured more than once"
			)));
		}
		let descriptor = schema
			.iter()
			.find(|descriptor| descriptor.name == *field)
			.ok_or_else(|| {
				AdminError::ValidationError(format!("inline field '{field}' is unknown"))
			})?;
		if !descriptor.editable
			|| descriptor.generated_relation_id
			|| C::primary_key_fields().contains(field)
		{
			return Err(AdminError::ValidationError(format!(
				"inline field '{field}' is not editable"
			)));
		}
	}
	Ok(())
}

struct TypedInlineAdapter<P, C> {
	model_identity: String,
	foreign_key: String,
	fields: Vec<String>,
	_marker: PhantomData<fn() -> (P, C)>,
}

#[async_trait(?Send)]
impl<P, C> InlineAdapter for TypedInlineAdapter<P, C>
where
	P: FormModel + ModelFormPrimaryKey + ModelFormPrimaryKeyFields + 'static,
	P::PrimaryKey: Serialize,
	C: FormModel + ModelFormPrimaryKeyFields + 'static,
	C::Data<AllEditableModelFields>: Default,
{
	async fn load_rows(
		&self,
		parent_id: &str,
		connection: &mut DatabaseConnection,
	) -> Result<Vec<InlineRowInfo>, InlineMutationError> {
		let rows = Manager::<C>::new()
			.filter(Filter::new(
				self.foreign_key.clone(),
				FilterOperator::Eq,
				filter_value(C::Schema::fields(), &self.foreign_key, parent_id)?,
			))
			.all_with_db(connection)
			.await
			.map_err(|error| InlineMutationError::Persistence(error.to_string()))?;
		rows.into_iter()
			.map(|child| self.project_child(child))
			.collect()
	}

	async fn save_rows(
		&self,
		inline_key: &str,
		parent_id: &str,
		rows: Vec<InlineRowMutation>,
		transaction: &mut AtomicTransaction,
	) -> Result<Vec<InlineSaveOutcome>, InlineMutationError> {
		let parent = load_one::<P>(
			P::primary_key_field(),
			filter_value(P::Schema::fields(), P::primary_key_field(), parent_id)?,
			None,
			transaction,
		)
		.await?
		.ok_or_else(|| InlineMutationError::Validation("parent row does not exist".to_owned()))?;

		let mut formset = InlineFormSet::<P, C>::for_update(parent, self.foreign_key.clone());
		let mut submitted_indices = Vec::new();
		let mut deletes = Vec::new();
		let mut pending_outcomes = Vec::new();
		let mut ids = HashSet::new();
		for row in rows {
			let mut changed_fields = row.values.keys().cloned().collect::<Vec<_>>();
			changed_fields.sort_unstable();
			let existing = match row.id.as_deref() {
				Some(id) => {
					if !ids.insert(id.to_owned()) {
						return Err(InlineMutationError::Validation(format!(
							"inline child ID '{id}' is submitted more than once"
						)));
					}
					load_one::<C>(
						C::primary_key_field(),
						filter_value(C::Schema::fields(), C::primary_key_field(), id)?,
						Some((
							self.foreign_key.as_str(),
							filter_value(C::Schema::fields(), &self.foreign_key, parent_id)?,
						)),
						transaction,
					)
					.await?
					.ok_or_else(|| {
						InlineMutationError::Validation(format!(
							"inline child ID '{id}' does not belong to the parent"
						))
					})?
				}
				None if row.delete => {
					return Err(InlineMutationError::Validation(
						"a new inline row cannot be deleted".to_owned(),
					));
				}
				None => {
					let payload = self.payload(inline_key, row.submitted_index, row.values)?;
					formset.add_child_form(ModelForm::from_payload(payload));
					submitted_indices.push(row.submitted_index);
					pending_outcomes.push((
						row.submitted_index,
						InlineSaveOperation::Create,
						None,
						changed_fields,
					));
					continue;
				}
			};
			let object_id = existing
				.primary_key()
				.ok_or_else(|| {
					InlineMutationError::Validation("inline child has no primary key".to_owned())
				})?
				.to_string();

			if row.delete {
				deletes.push((row.submitted_index, existing, object_id));
			} else {
				let payload = self.payload(inline_key, row.submitted_index, row.values)?;
				formset.add_child_form(ModelForm::from_payload_and_instance(payload, existing));
				submitted_indices.push(row.submitted_index);
				pending_outcomes.push((
					row.submitted_index,
					InlineSaveOperation::Update,
					Some(object_id),
					changed_fields,
				));
			}
		}

		if let Err(error) = formset
			.save_children(transaction as &mut dyn OrmExecutor)
			.await
		{
			return Err(formset_error(
				inline_key,
				&submitted_indices,
				&formset,
				error,
			));
		}
		let mut outcomes = pending_outcomes
			.into_iter()
			.zip(formset.child_forms())
			.map(
				|((submitted_index, operation, object_id, changed_fields), form)| {
					let object_id = object_id
						.or_else(|| {
							form.instance()
								.and_then(|instance| instance.primary_key())
								.map(|primary_key| primary_key.to_string())
						})
						.ok_or_else(|| {
							InlineMutationError::Persistence(
								"saved inline child has no primary key".to_owned(),
							)
						})?;
					Ok((
						submitted_index,
						self.outcome(operation, object_id, changed_fields),
					))
				},
			)
			.collect::<Result<Vec<_>, InlineMutationError>>()?;
		for (submitted_index, child, object_id) in deletes {
			let primary_key = child.primary_key().ok_or_else(|| {
				InlineMutationError::Validation("inline child has no primary key".to_owned())
			})?;
			Manager::<C>::new()
				.delete_with_executor(transaction, primary_key)
				.await
				.map_err(|error| InlineMutationError::Persistence(error.to_string()))?;
			outcomes.push((
				submitted_index,
				self.outcome(InlineSaveOperation::Delete, object_id, Vec::new()),
			));
		}
		outcomes.sort_unstable_by_key(|(submitted_index, _)| *submitted_index);
		Ok(outcomes.into_iter().map(|(_, outcome)| outcome).collect())
	}
}

impl<P, C> TypedInlineAdapter<P, C>
where
	C: FormModel,
	C::Data<AllEditableModelFields>: Default,
{
	fn project_child(&self, child: C) -> Result<InlineRowInfo, InlineMutationError> {
		let id = child
			.primary_key()
			.map(|primary_key| primary_key.to_string());
		let object = serde_json::to_value(child)
			.map_err(|error| InlineMutationError::Persistence(error.to_string()))?;
		let object = object.as_object().ok_or_else(|| {
			InlineMutationError::Persistence("child model must serialize as an object".to_owned())
		})?;
		let values = self
			.fields
			.iter()
			.map(|field| {
				object
					.get(field)
					.cloned()
					.map(|value| (field.clone(), value))
					.ok_or_else(|| {
						InlineMutationError::Persistence(format!(
							"child model did not serialize field '{field}'"
						))
					})
			})
			.collect::<Result<_, _>>()?;
		Ok(InlineRowInfo { id, values })
	}

	fn payload(
		&self,
		inline_key: &str,
		submitted_index: usize,
		values: HashMap<String, Value>,
	) -> Result<C::Data<AllEditableModelFields>, InlineMutationError> {
		let normalized = normalize_native_model_form_value::<C::Schema, AllEditableModelFields>(
			Value::Object(values.into_iter().collect::<Map<_, _>>()),
		)
		.map_err(|error| row_error(inline_key, submitted_index, "__all__", error.to_string()))?;
		let object = normalized.as_object().ok_or_else(|| {
			row_error(
				inline_key,
				submitted_index,
				"__all__",
				"inline row must be an object".to_owned(),
			)
		})?;
		let mut payload = C::Data::<AllEditableModelFields>::default();
		for (field, value) in object {
			payload.set_json(field, value.clone()).map_err(|error| {
				row_error(inline_key, submitted_index, field, error.to_string())
			})?;
		}
		Ok(payload)
	}

	fn outcome(
		&self,
		operation: InlineSaveOperation,
		object_id: String,
		changed_fields: Vec<String>,
	) -> InlineSaveOutcome {
		InlineSaveOutcome {
			operation,
			model_identity: self.model_identity.clone(),
			table_name: C::table_name().to_owned(),
			object_id,
			changed_fields,
		}
	}
}

async fn load_one<M>(
	field: &str,
	value: FilterValue,
	owner: Option<(&str, FilterValue)>,
	transaction: &mut AtomicTransaction,
) -> Result<Option<M>, InlineMutationError>
where
	M: Model,
{
	let mut query = Manager::<M>::new().filter(Filter::new(field, FilterOperator::Eq, value));
	if let Some((owner_field, owner_value)) = owner {
		query = query.filter(Filter::new(owner_field, FilterOperator::Eq, owner_value));
	}
	let mut rows = query
		.all_with_executor(transaction)
		.await
		.map_err(|error| InlineMutationError::Persistence(error.to_string()))?;
	if rows.len() > 1 {
		return Err(InlineMutationError::Validation(
			"inline lookup returned multiple rows".to_owned(),
		));
	}
	Ok(rows.pop())
}

fn filter_value(
	schema: &[reinhardt_core::model_form::ModelFormFieldDescriptor],
	field: &str,
	value: &str,
) -> Result<FilterValue, InlineMutationError> {
	let kind = schema
		.iter()
		.find(|descriptor| descriptor.name == field)
		.map(|descriptor| descriptor.kind)
		.ok_or_else(|| InlineMutationError::Validation(format!("unknown model field '{field}'")))?;
	match kind {
		ModelFormFieldKind::Integer { .. } => value
			.parse::<i64>()
			.map(FilterValue::Integer)
			.map_err(|_| InlineMutationError::Validation(format!("invalid integer ID '{value}'"))),
		ModelFormFieldKind::Float { .. } | ModelFormFieldKind::Decimal { .. } => value
			.parse::<f64>()
			.map(FilterValue::Float)
			.map_err(|_| InlineMutationError::Validation(format!("invalid numeric ID '{value}'"))),
		ModelFormFieldKind::Boolean => value
			.parse::<bool>()
			.map(FilterValue::Boolean)
			.map_err(|_| InlineMutationError::Validation(format!("invalid boolean ID '{value}'"))),
		_ => Ok(FilterValue::String(value.to_owned())),
	}
}

fn formset_error<P, C>(
	inline_key: &str,
	indices: &[usize],
	formset: &InlineFormSet<P, C>,
	error: ModelFormError,
) -> InlineMutationError
where
	P: FormModel,
	C: FormModel,
{
	let errors = indices
		.iter()
		.zip(formset.child_forms())
		.flat_map(|(index, form)| {
			form.form().errors().iter().map(move |(field, messages)| {
				(format!("{inline_key}.{index}.{field}"), messages.clone())
			})
		})
		.collect::<HashMap<_, _>>();
	if errors.is_empty() {
		InlineMutationError::Persistence(error.to_string())
	} else {
		InlineMutationError::RowValidation { errors }
	}
}

fn row_error(inline_key: &str, index: usize, field: &str, message: String) -> InlineMutationError {
	InlineMutationError::RowValidation {
		errors: HashMap::from([(format!("{inline_key}.{index}.{field}"), vec![message])]),
	}
}

#[cfg(all(test, server))]
mod tests {
	use super::*;
	use crate::core::{ModelAdmin, ModelAdminConfig};
	use reinhardt_db::associations::ForeignKeyField;
	use reinhardt_db::backends::DatabaseConnection as BackendsConnection;
	use reinhardt_db::orm::{DatabaseConnectionLease, QueryValue};
	use reinhardt_macros::model;
	use rstest::rstest;
	use serde::{Deserialize, Serialize};
	use serde_json::json;

	#[model(
		app_label = "admin",
		table_name = "inline_parents",
		form = true,
		info = false
	)]
	#[derive(Clone, Deserialize, Serialize)]
	struct Parent {
		#[field(primary_key = true)]
		id: Option<i64>,
		#[field(max_length = 100)]
		name: String,
	}

	#[model(
		app_label = "admin",
		table_name = "inline_other_parents",
		form = true,
		info = false
	)]
	#[derive(Clone, Deserialize, Serialize)]
	struct OtherParent {
		#[field(primary_key = true)]
		id: Option<i64>,
	}

	#[model(
		app_label = "admin",
		table_name = "inline_children",
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
		position: i64,
	}

	#[rstest]
	fn inline_configuration_uses_a_stable_identifier_safe_key() {
		let inline =
			InlineModelAdmin::new::<Parent, Child>("Line Item", "parent_id", &["name", "position"])
				.unwrap();

		assert_eq!(inline.key(), "inline_children-parent_id");
		assert_eq!(inline.child_model(), "Line Item");
		assert_eq!(inline.foreign_key(), "parent_id");
		assert_eq!(inline.fields(), &["name", "position"]);
		assert_eq!(inline.style_value(), InlineStyle::Tabular);
		assert_eq!(inline.extra_rows(), 0);
		assert_eq!(inline.delete_enabled(), false);
	}

	#[rstest]
	fn inline_configuration_rejects_invalid_relationships_and_fields() {
		assert!(InlineModelAdmin::new::<Parent, Child>("Child", "position", &["name"]).is_err());
		assert!(
			InlineModelAdmin::new::<OtherParent, Child>("Child", "parent_id", &["name"]).is_err()
		);
		assert!(
			InlineModelAdmin::new::<Parent, Child>("Child", "parent_id", &["missing"]).is_err()
		);
		assert!(InlineModelAdmin::new::<Parent, Child>("Child", "parent_id", &["id"]).is_err());
		assert!(
			InlineModelAdmin::new::<Parent, Child>("Child", "parent_id", &["parent_id"]).is_err()
		);
		assert!(
			InlineModelAdmin::new::<Parent, Child>("Child", "parent_id", &["name", "name"])
				.is_err()
		);
	}

	#[rstest]
	fn inline_builder_caps_extra_rows_and_resolution_rejects_duplicate_keys() {
		let inline = InlineModelAdmin::new::<Parent, Child>("Child", "parent_id", &["name"])
			.unwrap()
			.style(InlineStyle::Stacked)
			.extra(usize::MAX)
			.can_delete(true);
		assert_eq!(inline.extra_rows(), 100);
		assert_eq!(inline.style_value(), InlineStyle::Stacked);
		assert_eq!(inline.delete_enabled(), true);

		let admin = ModelAdminConfig::builder()
			.model_name("Parent")
			.inlines(vec![inline.clone(), inline])
			.build();
		assert!(admin.is_err());
	}

	#[rstest]
	fn model_admin_defaults_to_no_inline_configuration() {
		let admin = ModelAdminConfig::new("Parent");
		assert_eq!(admin.inlines().len(), 0);
	}

	async fn sqlite_connection() -> (DatabaseConnectionLease, DatabaseConnection) {
		let owner = BackendsConnection::connect_sqlite("sqlite::memory:")
			.await
			.unwrap();
		let lease = DatabaseConnectionLease::register(owner).unwrap();
		let connection = lease.handle();
		connection
			.execute(
				"CREATE TABLE inline_parents (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL)",
				vec![],
			)
			.await
			.unwrap();
		connection
			.execute(
				"CREATE TABLE inline_children (id INTEGER PRIMARY KEY AUTOINCREMENT, parent_id INTEGER NOT NULL, name TEXT NOT NULL, position BIGINT NOT NULL)",
				vec![],
			)
			.await
			.unwrap();
		(lease, connection)
	}

	async fn seed_parent(connection: &DatabaseConnection, id: i64, name: &str) {
		connection
			.execute(
				"INSERT INTO inline_parents (id, name) VALUES (?, ?)",
				vec![QueryValue::Int(id), QueryValue::String(name.to_owned())],
			)
			.await
			.unwrap();
	}

	async fn seed_child(
		connection: &DatabaseConnection,
		id: i64,
		parent_id: i64,
		name: &str,
		position: i64,
	) {
		connection
			.execute(
				"INSERT INTO inline_children (id, parent_id, name, position) VALUES (?, ?, ?, ?)",
				vec![
					QueryValue::Int(id),
					QueryValue::Int(parent_id),
					QueryValue::String(name.to_owned()),
					QueryValue::Int(position),
				],
			)
			.await
			.unwrap();
	}

	fn mutation(
		submitted_index: usize,
		id: Option<&str>,
		name: &str,
		position: i64,
		delete: bool,
	) -> InlineRowMutation {
		InlineRowMutation {
			submitted_index,
			id: id.map(str::to_owned),
			values: HashMap::from([
				("name".to_owned(), json!(name)),
				("position".to_owned(), json!(position)),
			]),
			delete,
		}
	}

	#[rstest]
	#[tokio::test]
	async fn typed_adapter_loads_creates_updates_and_deletes_generated_models() {
		let (_lease, mut connection) = sqlite_connection().await;
		seed_parent(&connection, 1, "parent").await;
		seed_child(&connection, 10, 1, "first", 1).await;
		seed_child(&connection, 11, 1, "second", 2).await;
		let inline =
			InlineModelAdmin::new::<Parent, Child>("Child", "parent_id", &["name", "position"])
				.unwrap();

		let loaded = inline
			.adapter()
			.load_rows("1", &mut connection)
			.await
			.unwrap();
		assert_eq!(loaded.len(), 2);
		assert_eq!(loaded[0].values.len(), 2);
		assert_eq!(loaded[0].values.get("name"), Some(&json!("first")));

		let outcomes = connection
			.atomic(async |transaction| {
				inline
					.adapter()
					.save_rows(
						inline.key(),
						"1",
						vec![
							mutation(2, Some("10"), "updated", 3, false),
							mutation(4, Some("11"), "ignored", 0, true),
							mutation(7, None, "created", 4, false),
						],
						transaction,
					)
					.await
			})
			.await
			.unwrap();
		assert_eq!(
			outcomes,
			vec![
				InlineSaveOutcome {
					operation: InlineSaveOperation::Update,
					model_identity: "Child".to_owned(),
					table_name: "inline_children".to_owned(),
					object_id: "10".to_owned(),
					changed_fields: vec!["name".to_owned(), "position".to_owned()],
				},
				InlineSaveOutcome {
					operation: InlineSaveOperation::Delete,
					model_identity: "Child".to_owned(),
					table_name: "inline_children".to_owned(),
					object_id: "11".to_owned(),
					changed_fields: Vec::new(),
				},
				InlineSaveOutcome {
					operation: InlineSaveOperation::Create,
					model_identity: "Child".to_owned(),
					table_name: "inline_children".to_owned(),
					object_id: "12".to_owned(),
					changed_fields: vec!["name".to_owned(), "position".to_owned()],
				},
			]
		);

		let loaded = inline
			.adapter()
			.load_rows("1", &mut connection)
			.await
			.unwrap();
		assert_eq!(loaded.len(), 2);
		assert!(loaded.iter().any(|row| {
			row.id.as_deref() == Some("10")
				&& row.values.get("name") == Some(&json!("updated"))
				&& row.values.get("position") == Some(&json!(3))
		}));
		assert!(loaded.iter().any(|row| {
			row.id.as_deref() != Some("10")
				&& row.values.get("name") == Some(&json!("created"))
				&& row.values.get("position") == Some(&json!(4))
		}));
		assert!(!loaded.iter().any(|row| row.id.as_deref() == Some("11")));
	}

	#[rstest]
	#[tokio::test]
	async fn typed_adapter_rejects_cross_parent_child_ids() {
		let (_lease, connection) = sqlite_connection().await;
		seed_parent(&connection, 1, "first parent").await;
		seed_parent(&connection, 2, "second parent").await;
		seed_child(&connection, 20, 2, "owned elsewhere", 1).await;
		let inline =
			InlineModelAdmin::new::<Parent, Child>("Child", "parent_id", &["name", "position"])
				.unwrap();

		let error = connection
			.atomic(async |transaction| {
				inline
					.adapter()
					.save_rows(
						inline.key(),
						"1",
						vec![mutation(9, Some("20"), "stolen", 2, false)],
						transaction,
					)
					.await
			})
			.await
			.unwrap_err();

		assert!(matches!(error, InlineMutationError::Validation(_)));
	}

	#[rstest]
	#[tokio::test]
	async fn typed_adapter_maps_child_validation_to_submitted_row_index() {
		let (_lease, connection) = sqlite_connection().await;
		seed_parent(&connection, 1, "parent").await;
		let inline =
			InlineModelAdmin::new::<Parent, Child>("Child", "parent_id", &["name", "position"])
				.unwrap();
		let invalid_name = "x".repeat(101);

		let error = connection
			.atomic(async |transaction| {
				inline
					.adapter()
					.save_rows(
						inline.key(),
						"1",
						vec![mutation(9, None, &invalid_name, 1, false)],
						transaction,
					)
					.await
			})
			.await
			.unwrap_err();

		assert!(matches!(
			error,
			InlineMutationError::RowValidation { errors }
				if errors.contains_key("inline_children-parent_id.9.name")
		));
	}
}
