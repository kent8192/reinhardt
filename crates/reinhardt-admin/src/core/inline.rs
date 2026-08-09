use crate::types::{AdminError, AdminResult, InlineRowInfo, InlineStyle};
use async_trait::async_trait;
use reinhardt_core::model_form::{
	AllEditableModelFields, ModelFormFieldKind, ModelFormPayload, ModelFormPrimaryKey,
	ModelFormPrimaryKeyFields, ModelFormSchema, normalize_native_model_form_value,
};
use reinhardt_db::orm::transaction::AtomicTransaction;
use reinhardt_db::orm::{
	CustomManager, DatabaseConnection, FieldAssignment, Filter, FilterOperator, FilterValue,
	Manager, Model, QuerySet, UpdateValue,
};
use reinhardt_forms::form::ALL_FIELDS_KEY;
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

#[async_trait]
pub(crate) trait InlineAdapter: Send + Sync {
	fn normalize_child_id(&self, id: &str) -> Result<String, InlineMutationError>;

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
		C::Data<AllEditableModelFields>: Default + Send,
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
		let mut total_extra = 0usize;
		for inline in inlines {
			if !keys.insert(inline.key()) {
				return Err(AdminError::ValidationError(format!(
					"inline key '{}' is configured more than once",
					inline.key()
				)));
			}
			total_extra = total_extra
				.checked_add(inline.extra_rows())
				.ok_or_else(|| {
					AdminError::ValidationError(
						"inline configurations exceed 100 total extra rows".to_owned(),
					)
				})?;
			if total_extra > MAX_INLINE_ROWS {
				return Err(AdminError::ValidationError(
					"inline configurations exceed 100 total extra rows".to_owned(),
				));
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
		if matches!(*field, "__id" | "__delete") {
			return Err(AdminError::ValidationError(format!(
				"inline field '{field}' is reserved"
			)));
		}
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

#[async_trait]
impl<P, C> InlineAdapter for TypedInlineAdapter<P, C>
where
	P: FormModel + ModelFormPrimaryKey + ModelFormPrimaryKeyFields + 'static,
	P::PrimaryKey: Serialize,
	C: FormModel + ModelFormPrimaryKeyFields + 'static,
	C::Data<AllEditableModelFields>: Default + Send,
{
	fn normalize_child_id(&self, id: &str) -> Result<String, InlineMutationError> {
		normalize_filter_value(filter_value(
			C::Schema::fields(),
			C::primary_key_field(),
			id,
		)?)
	}

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
				Some(id) => load_one::<C>(
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
				})?,
				None if row.delete => {
					return Err(InlineMutationError::Validation(
						"a new inline row cannot be deleted".to_owned(),
					));
				}
				None => {
					let payload = self.payload(inline_key, row.submitted_index, row.values)?;
					formset.add_child_form(ModelForm::from_payload(payload));
					submitted_indices.push(row.submitted_index);
					pending_outcomes.push((row.submitted_index, None, changed_fields));
					continue;
				}
			};
			let object_id = existing
				.primary_key()
				.ok_or_else(|| {
					InlineMutationError::Validation("inline child has no primary key".to_owned())
				})?
				.to_string();
			if !ids.insert(object_id.clone()) {
				return Err(InlineMutationError::Validation(format!(
					"inline child ID '{object_id}' is submitted more than once"
				)));
			}

			if row.delete {
				deletes.push((row.submitted_index, existing, object_id));
			} else {
				let payload = self.payload(inline_key, row.submitted_index, row.values)?;
				formset.add_child_form(ModelForm::from_payload_and_instance(payload, existing));
				submitted_indices.push(row.submitted_index);
				pending_outcomes.push((row.submitted_index, Some(object_id), changed_fields));
			}
		}

		let candidates = match formset.prepare_child_instances() {
			Ok(candidates) => candidates,
			Err(error) => {
				return Err(formset_error(
					inline_key,
					&submitted_indices,
					&formset,
					error,
				));
			}
		};
		drop(formset);

		let manager = C::objects();
		let mut outcomes = Vec::with_capacity(pending_outcomes.len() + deletes.len());
		for ((submitted_index, object_id, changed_fields), mut candidate) in
			pending_outcomes.into_iter().zip(candidates)
		{
			let (operation, object_id) = match object_id {
				None => (
					InlineSaveOperation::Create,
					manager
						.create_with_conn(transaction, &candidate)
						.await
						.map_err(|error| InlineMutationError::Persistence(error.to_string()))?
						.primary_key()
						.map(|primary_key| primary_key.to_string())
						.ok_or_else(|| {
							InlineMutationError::Persistence(
								"saved inline child has no primary key".to_owned(),
							)
						})?,
				),
				Some(object_id) => {
					self.update_owned_child(
						&manager,
						&mut candidate,
						&object_id,
						parent_id,
						transaction,
					)
					.await?;
					(InlineSaveOperation::Update, object_id)
				}
			};
			outcomes.push((
				submitted_index,
				self.outcome(operation, object_id, changed_fields),
			));
		}
		for (submitted_index, child, object_id) in deletes {
			self.delete_owned_child(&manager, &child, &object_id, parent_id, transaction)
				.await?;
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
	async fn update_owned_child(
		&self,
		manager: &C::Objects,
		candidate: &mut C,
		object_id: &str,
		parent_id: &str,
		transaction: &mut AtomicTransaction,
	) -> Result<(), InlineMutationError> {
		manager
			.before_save(candidate)
			.map_err(|error| InlineMutationError::Persistence(error.to_string()))?;
		let generated = C::generated_field_names();
		let assignments = candidate
			.encode_database_fields()
			.map_err(|error| InlineMutationError::Persistence(error.to_string()))?
			.into_iter()
			.filter(|(field, _)| {
				field != C::primary_key_field()
					&& field != &self.foreign_key
					&& !generated.contains(&field.as_str())
			})
			.map(|(field, value)| FieldAssignment::new(field, UpdateValue::Typed(Ok(value))))
			.collect::<Vec<_>>();
		if assignments.is_empty() {
			return Err(InlineMutationError::Persistence(
				"inline child has no writable fields".to_owned(),
			));
		}
		let affected = owned_child_query::<C>(&self.foreign_key, object_id, parent_id)?
			.update_fields_with_conn(transaction, assignments)
			.await
			.map_err(|error| InlineMutationError::Persistence(error.to_string()))?;
		require_single_owned_row("update", object_id, affected)
	}

	async fn delete_owned_child(
		&self,
		manager: &C::Objects,
		child: &C,
		object_id: &str,
		parent_id: &str,
		transaction: &mut AtomicTransaction,
	) -> Result<(), InlineMutationError> {
		manager
			.before_delete(child)
			.map_err(|error| InlineMutationError::Persistence(error.to_string()))?;
		let affected = owned_child_query::<C>(&self.foreign_key, object_id, parent_id)?
			.delete_with_conn(transaction)
			.await
			.map_err(|error| InlineMutationError::Persistence(error.to_string()))?;
		require_single_owned_row("delete", object_id, affected)
	}

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
		.map_err(|error| {
			row_error(
				inline_key,
				submitted_index,
				ALL_FIELDS_KEY,
				error.to_string(),
			)
		})?;
		let object = normalized.as_object().ok_or_else(|| {
			row_error(
				inline_key,
				submitted_index,
				ALL_FIELDS_KEY,
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

fn owned_child_query<C>(
	foreign_key: &str,
	object_id: &str,
	parent_id: &str,
) -> Result<QuerySet<C>, InlineMutationError>
where
	C: FormModel,
{
	Ok(Manager::<C>::new()
		.filter(Filter::new(
			C::primary_key_field(),
			FilterOperator::Eq,
			filter_value(C::Schema::fields(), C::primary_key_field(), object_id)?,
		))
		.filter(Filter::new(
			foreign_key,
			FilterOperator::Eq,
			filter_value(C::Schema::fields(), foreign_key, parent_id)?,
		)))
}

fn require_single_owned_row(
	operation: &str,
	object_id: &str,
	affected: u64,
) -> Result<(), InlineMutationError> {
	if affected == 1 {
		Ok(())
	} else {
		Err(InlineMutationError::Persistence(format!(
			"inline child {operation} for ID '{object_id}' affected {affected} rows"
		)))
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

fn normalize_filter_value(value: FilterValue) -> Result<String, InlineMutationError> {
	match value {
		FilterValue::String(value) => Ok(value),
		FilterValue::Integer(value) => Ok(value.to_string()),
		FilterValue::Float(value) => Ok(value.to_string()),
		FilterValue::Boolean(value) => Ok(value.to_string()),
		_ => Err(InlineMutationError::Validation(
			"inline primary key cannot be normalized".to_owned(),
		)),
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
	use crate::core::{InlineStyle as PublicInlineStyle, ModelAdmin, ModelAdminConfig};
	use reinhardt_db::associations::ForeignKeyField;
	use reinhardt_db::backends::DatabaseConnection as BackendsConnection;
	use reinhardt_db::orm::{DatabaseConnectionLease, QueryValue};
	use reinhardt_forms::form::ALL_FIELDS_KEY;
	use reinhardt_macros::model;
	use rstest::rstest;
	use serde::{Deserialize, Serialize};
	use serde_json::json;
	use std::future::Future;

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
		let public_style: PublicInlineStyle = InlineStyle::Tabular;

		assert_eq!(inline.key(), "inline_children-parent_id");
		assert_eq!(inline.child_model(), "Line Item");
		assert_eq!(inline.foreign_key(), "parent_id");
		assert_eq!(inline.fields(), &["name", "position"]);
		assert_eq!(inline.style_value(), public_style);
		assert_eq!(inline.extra_rows(), 0);
		assert_eq!(inline.delete_enabled(), false);
	}

	#[rstest]
	fn inline_configuration_rejects_invalid_relationships_and_fields() {
		let wrong_relationship =
			InlineModelAdmin::new::<Parent, Child>("Child", "position", &["name"]).unwrap_err();
		assert_eq!(
			wrong_relationship.to_string(),
			"Validation error: inline foreign key 'position' is not a generated relationship identifier"
		);

		let wrong_parent =
			InlineModelAdmin::new::<OtherParent, Child>("Child", "parent_id", &["name"])
				.unwrap_err();
		assert_eq!(
			wrong_parent.to_string(),
			"Validation error: inline foreign key 'parent_id' does not target the configured parent"
		);

		for (field, expected) in [
			(
				"missing",
				"Validation error: inline field 'missing' is unknown",
			),
			("id", "Validation error: inline field 'id' is not editable"),
			(
				"parent_id",
				"Validation error: inline field 'parent_id' is not editable",
			),
			("__id", "Validation error: inline field '__id' is reserved"),
			(
				"__delete",
				"Validation error: inline field '__delete' is reserved",
			),
		] {
			let error =
				InlineModelAdmin::new::<Parent, Child>("Child", "parent_id", &[field]).unwrap_err();
			assert_eq!(error.to_string(), expected);
		}

		let duplicate =
			InlineModelAdmin::new::<Parent, Child>("Child", "parent_id", &["name", "name"])
				.unwrap_err();
		assert_eq!(
			duplicate.to_string(),
			"Validation error: inline field 'name' is configured more than once"
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
		assert!(inline.delete_enabled());

		let admin = ModelAdminConfig::builder()
			.model_name("Parent")
			.inlines(vec![inline.clone(), inline])
			.build();
		assert_eq!(
			admin.unwrap_err().to_string(),
			"Validation error: inline key 'inline_children-parent_id' is configured more than once"
		);
	}

	#[rstest]
	fn inline_resolution_rejects_more_than_one_hundred_total_extra_rows() {
		let first = InlineModelAdmin::new::<Parent, Child>("Child", "parent_id", &["name"])
			.unwrap()
			.extra(60);
		let mut second = first.clone().extra(41);
		second.key = "other-inline".to_owned();

		let error = ModelAdminConfig::builder()
			.model_name("Parent")
			.inlines(vec![first, second])
			.build()
			.unwrap_err();

		assert_eq!(
			error.to_string(),
			"Validation error: inline configurations exceed 100 total extra rows"
		);
	}

	#[rstest]
	fn model_admin_defaults_to_no_inline_configuration() {
		let admin = ModelAdminConfig::new("Parent");
		assert!(admin.inlines().is_empty());
	}

	fn assert_send_future<F: Future + Send>(_future: F) {}

	fn assert_save_rows_future_is_send<'a>(
		adapter: &'a dyn InlineAdapter,
		transaction: &'a mut AtomicTransaction,
	) {
		assert_send_future(adapter.save_rows("inline", "1", Vec::new(), transaction));
	}

	#[rstest]
	fn inline_adapter_save_future_is_send() {
		let _type_check = assert_save_rows_future_is_send;
	}

	#[rstest]
	fn owned_child_writes_constrain_both_primary_key_and_trusted_foreign_key() {
		let query = owned_child_query::<Child>("parent_id", "7", "1").unwrap();

		let (update_sql, update_params) = query.update_fields_sql([("name", "updated")]).unwrap();
		let (delete_sql, delete_params) = query.delete_sql().unwrap();

		assert_eq!(
			update_sql,
			"UPDATE \"inline_children\" SET \"name\" = $1 WHERE (\"id\" = $2 AND \"parent_id\" = $3)"
		);
		assert_eq!(update_params, vec!["updated", "7", "1"]);
		assert_eq!(
			delete_sql,
			"DELETE FROM \"inline_children\" WHERE (\"id\" = $1 AND \"parent_id\" = $2)"
		);
		assert_eq!(delete_params, vec!["7", "1"]);
	}

	#[rstest]
	#[case(0)]
	#[case(2)]
	fn owned_child_writes_require_exactly_one_affected_row(#[case] affected: u64) {
		let error = require_single_owned_row("update", "7", affected).unwrap_err();

		let InlineMutationError::Persistence(message) = error else {
			panic!("expected persistence error");
		};
		assert_eq!(
			message,
			format!("inline child update for ID '7' affected {affected} rows")
		);
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

		let mut loaded = inline
			.adapter()
			.load_rows("1", &mut connection)
			.await
			.unwrap();
		loaded.sort_by(|left, right| left.id.cmp(&right.id));
		assert_eq!(
			loaded,
			vec![
				InlineRowInfo {
					id: Some("10".to_owned()),
					values: HashMap::from([
						("name".to_owned(), json!("first")),
						("position".to_owned(), json!(1)),
					]),
				},
				InlineRowInfo {
					id: Some("11".to_owned()),
					values: HashMap::from([
						("name".to_owned(), json!("second")),
						("position".to_owned(), json!(2)),
					]),
				},
			]
		);

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

		let mut loaded = inline
			.adapter()
			.load_rows("1", &mut connection)
			.await
			.unwrap();
		loaded.sort_by(|left, right| left.id.cmp(&right.id));
		assert_eq!(
			loaded,
			vec![
				InlineRowInfo {
					id: Some("10".to_owned()),
					values: HashMap::from([
						("name".to_owned(), json!("updated")),
						("position".to_owned(), json!(3)),
					]),
				},
				InlineRowInfo {
					id: Some("12".to_owned()),
					values: HashMap::from([
						("name".to_owned(), json!("created")),
						("position".to_owned(), json!(4)),
					]),
				},
			]
		);
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

		assert_eq!(
			error.to_string(),
			"invalid inline submission: inline child ID '20' does not belong to the parent"
		);
	}

	#[rstest]
	#[tokio::test]
	async fn typed_adapter_rejects_duplicate_ids_after_primary_key_normalization() {
		let (_lease, connection) = sqlite_connection().await;
		seed_parent(&connection, 1, "parent").await;
		seed_child(&connection, 7, 1, "existing", 1).await;
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
						vec![
							mutation(0, Some("07"), "first", 2, false),
							mutation(1, Some("7"), "second", 3, false),
						],
						transaction,
					)
					.await
			})
			.await
			.unwrap_err();

		assert_eq!(
			error.to_string(),
			"invalid inline submission: inline child ID '7' is submitted more than once"
		);
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

		let InlineMutationError::RowValidation { errors } = error else {
			panic!("expected row validation error");
		};
		assert_eq!(
			errors,
			HashMap::from([(
				"inline_children-parent_id.9.name".to_owned(),
				vec!["Ensure this value has at most 100 characters (it has 101)".to_owned()],
			)])
		);
	}

	#[rstest]
	fn typed_adapter_maps_model_form_non_field_errors_to_the_submitted_row() {
		let inline =
			InlineModelAdmin::new::<Parent, Child>("Child", "parent_id", &["name", "position"])
				.unwrap();
		let mut payload = <Child as FormModel>::Data::<AllEditableModelFields>::default();
		payload.set_json("name", json!("valid")).unwrap();
		payload.set_json("position", json!(1)).unwrap();
		let form = ModelForm::from_payload(payload)
			.with_model_validator(|_| Err(vec!["row values conflict".to_owned()]));
		let parent = Parent {
			id: Some(1),
			name: "parent".to_owned(),
		};
		let mut formset =
			InlineFormSet::<Parent, Child>::for_update(parent, "parent_id".to_owned());
		formset.add_child_form(form);

		let error = formset.prepare_child_instances().unwrap_err();
		let mapped = formset_error(inline.key(), &[9], &formset, error);

		assert_eq!(ALL_FIELDS_KEY, "_all");
		let InlineMutationError::RowValidation { errors } = mapped else {
			panic!("expected row validation error");
		};
		assert_eq!(
			errors,
			HashMap::from([(
				"inline_children-parent_id.9._all".to_owned(),
				vec!["row values conflict".to_owned()],
			)])
		);
	}
}
