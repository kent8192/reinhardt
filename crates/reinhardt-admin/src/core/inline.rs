use super::AdminQuery;
use crate::types::{AdminError, AdminResult, InlineRowInfo, InlineStyle};
use async_trait::async_trait;
use reinhardt_core::model_form::{
	AllEditableModelFields, ModelFormFieldKind, ModelFormPayload, ModelFormPrimaryKey,
	ModelFormPrimaryKeyFields, ModelFormSchema, normalize_native_model_form_value,
};
use reinhardt_db::orm::transaction::AtomicTransaction;
use reinhardt_db::orm::{
	CustomManager, DatabaseConnection, DatabaseStorageKind, DatabaseValue, FieldAssignment, Filter,
	FilterOperator, FilterValue, Model, QuerySet, UpdateValue,
};
use reinhardt_forms::form::ALL_FIELDS_KEY;
use reinhardt_forms::{FormModel, InlineFormSet, ModelForm, ModelFormError};
use serde::Serialize;
use serde_json::{Map, Value};
use std::collections::{BTreeMap, HashMap, HashSet};
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
	pub(crate) inline_key: String,
	pub(crate) submitted_index: usize,
	pub(crate) operation: InlineSaveOperation,
	pub(crate) model_identity: String,
	pub(crate) table_name: String,
	pub(crate) object_id: String,
	pub(crate) changed_fields: Vec<String>,
	pub(crate) previous_values: HashMap<String, Value>,
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
	fn table_name(&self) -> &'static str;
	fn parent_table_name(&self) -> &'static str;
	fn parent_primary_key_column(&self) -> &'static str;

	fn normalize_child_id(&self, id: &str) -> Result<String, InlineMutationError>;

	fn normalize_row_values(
		&self,
		values: &HashMap<String, Value>,
	) -> Result<HashMap<String, Value>, InlineMutationError>;

	async fn load_rows(
		&self,
		parent_id: &str,
		limit: usize,
		query: Option<&AdminQuery>,
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
		C::CleanedData<AllEditableModelFields>: Send,
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

	/// Table name of the typed parent model.
	pub(crate) fn parent_table_name(&self) -> &'static str {
		self.adapter.parent_table_name()
	}

	/// Physical primary-key column of the typed parent model.
	pub(crate) fn parent_primary_key_column(&self) -> &'static str {
		self.adapter.parent_primary_key_column()
	}

	pub(crate) fn validate_child_table(&self, table_name: &str) -> AdminResult<()> {
		if table_name != self.adapter.table_name() {
			return Err(AdminError::ValidationError(format!(
				"inline child '{}' resolves to table '{}', expected '{}'",
				self.child_model,
				table_name,
				self.adapter.table_name()
			)));
		}
		Ok(())
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

	pub(crate) fn validate_for_parent(
		inlines: &[Self],
		parent_table: &str,
		parent_pk_column: &str,
	) -> AdminResult<()> {
		Self::validate_resolved(inlines)?;
		for inline in inlines {
			if inline.parent_table_name() != parent_table
				|| inline.parent_primary_key_column() != parent_pk_column
			{
				return Err(AdminError::ValidationError(format!(
					"inline '{}' targets parent '{}:{}', but the admin is '{}:{}'",
					inline.key(),
					inline.parent_table_name(),
					inline.parent_primary_key_column(),
					parent_table,
					parent_pk_column
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
	if C::primary_key_fields().len() != 1 {
		return Err(AdminError::ValidationError(
			"inline child model must have exactly one primary key field".to_owned(),
		));
	}
	validate_identifier_kind("parent primary key", P::FIELD_KIND)?;
	let schema = C::Schema::fields();
	let child_primary_key_kind = C::primary_key_field_kind().ok_or_else(|| {
		AdminError::ValidationError("inline child primary key is unknown".to_owned())
	})?;
	validate_identifier_kind("child primary key", child_primary_key_kind)?;
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
	validate_identifier_kind("foreign key", relationship.kind)?;

	let mut configured = HashSet::new();
	for field in fields {
		if matches!(*field, "__id" | "__delete" | "__present") {
			return Err(AdminError::ValidationError(format!(
				"inline field '{field}' is reserved"
			)));
		}
		if !configured.insert(*field) {
			return Err(AdminError::ValidationError(format!(
				"inline field '{field}' is configured more than once"
			)));
		}
		if C::primary_key_fields().contains(field) {
			return Err(AdminError::ValidationError(format!(
				"inline field '{field}' is not editable"
			)));
		}
		let descriptor = schema
			.iter()
			.find(|descriptor| descriptor.name == *field)
			.ok_or_else(|| {
				AdminError::ValidationError(format!("inline field '{field}' is unknown"))
			})?;
		if !descriptor.editable || descriptor.generated_relation_id {
			return Err(AdminError::ValidationError(format!(
				"inline field '{field}' is not editable"
			)));
		}
	}
	Ok(())
}

fn validate_identifier_kind(role: &str, kind: ModelFormFieldKind) -> AdminResult<()> {
	if matches!(
		kind,
		ModelFormFieldKind::Integer { .. }
			| ModelFormFieldKind::Text { .. }
			| ModelFormFieldKind::Email { .. }
			| ModelFormFieldKind::Url { .. }
			| ModelFormFieldKind::Uuid
	) {
		Ok(())
	} else {
		Err(AdminError::ValidationError(format!(
			"inline {role} uses an unsupported identifier type"
		)))
	}
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
	C::CleanedData<AllEditableModelFields>: Send,
{
	fn table_name(&self) -> &'static str {
		C::table_name()
	}

	fn parent_table_name(&self) -> &'static str {
		P::table_name()
	}

	fn parent_primary_key_column(&self) -> &'static str {
		P::primary_key_column()
	}

	fn normalize_child_id(&self, id: &str) -> Result<String, InlineMutationError> {
		let kind = C::primary_key_field_kind().ok_or_else(|| {
			InlineMutationError::Validation("inline child primary key is unknown".to_owned())
		})?;
		normalize_filter_value(filter_value_kind(kind, C::primary_key_field(), id)?)
	}

	fn normalize_row_values(
		&self,
		values: &HashMap<String, Value>,
	) -> Result<HashMap<String, Value>, InlineMutationError> {
		let normalized = normalize_native_model_form_value::<C::Schema, AllEditableModelFields>(
			Value::Object(values.clone().into_iter().collect()),
		)
		.map_err(|error| InlineMutationError::Validation(error.to_string()))?;
		normalized
			.as_object()
			.cloned()
			.map(|values| values.into_iter().collect())
			.ok_or_else(|| {
				InlineMutationError::Validation("inline row must be an object".to_owned())
			})
	}

	async fn load_rows(
		&self,
		parent_id: &str,
		limit: usize,
		query: Option<&AdminQuery>,
		connection: &mut DatabaseConnection,
	) -> Result<Vec<InlineRowInfo>, InlineMutationError> {
		let manager = C::objects();
		let mut queryset = manager.all().filter(Filter::new(
			self.foreign_key.clone(),
			FilterOperator::Eq,
			filter_value(C::Schema::fields(), &self.foreign_key, parent_id)?,
		));
		if let Some(query) = query {
			if query.table_name() != C::table_name() {
				return Err(InlineMutationError::Validation(
					"inline query targets the wrong table".to_owned(),
				));
			}
			for condition in query.conditions() {
				queryset = queryset.filter(condition.clone());
			}
		}
		let rows = queryset
			.limit(limit)
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
		let parent_manager = P::objects();
		let parent = load_one_with_manager(
			&parent_manager,
			P::primary_key_field(),
			filter_value_kind(P::FIELD_KIND, P::primary_key_field(), parent_id)?,
			None,
			transaction,
		)
		.await?
		.ok_or_else(|| InlineMutationError::Validation("parent row does not exist".to_owned()))?;
		let parent_primary_key = parent.primary_key().ok_or_else(|| {
			InlineMutationError::Validation("parent row has no primary key".to_owned())
		})?;
		let parent_database_value = P::primary_key_database_value(&parent_primary_key)
			.map_err(|error| InlineMutationError::Persistence(error.to_string()))?;

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
					let child_manager = C::objects();
					load_one_with_manager(
						&child_manager,
						C::primary_key_field(),
						filter_value_kind(
							C::primary_key_field_kind().ok_or_else(|| {
								InlineMutationError::Validation(
									"inline child primary key is unknown".to_owned(),
								)
							})?,
							C::primary_key_field(),
							id,
						)?,
						Some((
							self.foreign_key.as_str(),
							FilterValue::Typed(Ok(parent_database_value.clone())),
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
						None,
						changed_fields,
						HashMap::new(),
					));
					continue;
				}
			};
			let child_primary_key = existing.primary_key().ok_or_else(|| {
				InlineMutationError::Validation("inline child has no primary key".to_owned())
			})?;
			let object_id = child_primary_key.to_string();
			let child_database_value = C::primary_key_database_value(&child_primary_key)
				.map_err(|error| InlineMutationError::Persistence(error.to_string()))?;
			let previous_values = self.project_values(&existing)?;
			if !ids.insert(object_id.clone()) {
				return Err(InlineMutationError::Validation(format!(
					"inline child ID '{object_id}' is submitted more than once"
				)));
			}

			if row.delete {
				deletes.push((
					row.submitted_index,
					existing,
					object_id,
					child_database_value,
					previous_values,
				));
			} else {
				let payload = self.payload(inline_key, row.submitted_index, row.values)?;
				formset.add_child_form(ModelForm::from_payload_and_instance(payload, existing));
				submitted_indices.push(row.submitted_index);
				pending_outcomes.push((
					row.submitted_index,
					Some((object_id, child_database_value)),
					changed_fields,
					previous_values,
				));
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
		for (submitted_index, child, object_id, child_primary_key, previous_values) in deletes {
			self.delete_owned_child(
				&manager,
				&child,
				&object_id,
				&child_primary_key,
				&parent_database_value,
				transaction,
			)
			.await?;
			outcomes.push((
				submitted_index,
				self.outcome(
					inline_key,
					submitted_index,
					InlineSaveOperation::Delete,
					object_id,
					Vec::new(),
					previous_values,
				),
			));
		}
		for ((submitted_index, object_id, mut changed_fields, previous_values), mut candidate) in
			pending_outcomes.into_iter().zip(candidates)
		{
			let (operation, object_id) = match object_id {
				None => {
					let before_save = candidate
						.encode_database_fields()
						.map_err(|error| InlineMutationError::Persistence(error.to_string()))?;
					let saved = manager
						.create_with_conn(transaction, &candidate)
						.await
						.map_err(|error| InlineMutationError::Persistence(error.to_string()))?;
					changed_fields.extend(changed_database_fields(
						&before_save,
						&saved
							.encode_database_fields()
							.map_err(|error| InlineMutationError::Persistence(error.to_string()))?,
					));
					let object_id = saved
						.primary_key()
						.map(|primary_key| primary_key.to_string())
						.ok_or_else(|| {
							InlineMutationError::Persistence(
								"saved inline child has no primary key".to_owned(),
							)
						})?;
					(InlineSaveOperation::Create, object_id)
				}
				Some((object_id, child_primary_key)) => {
					changed_fields.extend(
						self.update_owned_child(
							&manager,
							&mut candidate,
							&object_id,
							&child_primary_key,
							&parent_database_value,
							transaction,
						)
						.await?,
					);
					(InlineSaveOperation::Update, object_id)
				}
			};
			changed_fields.retain(|field| field != C::primary_key_field());
			changed_fields.sort_unstable();
			changed_fields.dedup();
			outcomes.push((
				submitted_index,
				self.outcome(
					inline_key,
					submitted_index,
					operation,
					object_id,
					changed_fields,
					previous_values,
				),
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
		child_primary_key: &DatabaseValue,
		parent_primary_key: &DatabaseValue,
		transaction: &mut AtomicTransaction,
	) -> Result<Vec<String>, InlineMutationError> {
		let before_save = candidate
			.encode_database_fields()
			.map_err(|error| InlineMutationError::Persistence(error.to_string()))?;
		manager
			.before_save(candidate)
			.map_err(|error| InlineMutationError::Persistence(error.to_string()))?;
		let generated = C::generated_field_names();
		let mut encoded = candidate
			.encode_database_fields()
			.map_err(|error| InlineMutationError::Persistence(error.to_string()))?;
		let auto_now_values = auto_now_database_values::<C>();
		let auto_now_fields = auto_now_values.keys().cloned().collect::<HashSet<_>>();
		let mut changed_fields = changed_database_fields(&before_save, &encoded);
		for (field, value) in auto_now_values {
			encoded.insert(field, value);
		}
		changed_fields.extend(auto_now_fields.iter().cloned());
		let assignments = encoded
			.into_iter()
			.filter(|(field, _)| {
				field != C::primary_key_field()
					&& field != &self.foreign_key
					&& (!generated.contains(&field.as_str()) || auto_now_fields.contains(field))
			})
			.map(|(field, value)| FieldAssignment::new(field, UpdateValue::Typed(Ok(value))))
			.collect::<Vec<_>>();
		if assignments.is_empty() {
			return Err(InlineMutationError::Persistence(
				"inline child has no writable fields".to_owned(),
			));
		}
		let affected = owned_child_query::<C>(
			manager,
			&self.foreign_key,
			child_primary_key,
			parent_primary_key,
		)
		.update_fields_with_conn(transaction, assignments)
		.await
		.map_err(|error| InlineMutationError::Persistence(error.to_string()))?;
		require_single_owned_row("update", object_id, affected)?;
		Ok(changed_fields)
	}

	async fn delete_owned_child(
		&self,
		manager: &C::Objects,
		child: &C,
		object_id: &str,
		child_primary_key: &DatabaseValue,
		parent_primary_key: &DatabaseValue,
		transaction: &mut AtomicTransaction,
	) -> Result<(), InlineMutationError> {
		manager
			.before_delete(child)
			.map_err(|error| InlineMutationError::Persistence(error.to_string()))?;
		let affected = owned_child_query::<C>(
			manager,
			&self.foreign_key,
			child_primary_key,
			parent_primary_key,
		)
		.delete_with_conn(transaction)
		.await
		.map_err(|error| InlineMutationError::Persistence(error.to_string()))?;
		require_single_owned_row("delete", object_id, affected)
	}

	fn project_child(&self, child: C) -> Result<InlineRowInfo, InlineMutationError> {
		let id = child
			.primary_key()
			.map(|primary_key| primary_key.to_string());
		let values = self.project_values(&child)?;
		Ok(InlineRowInfo { id, values })
	}

	fn project_values(&self, child: &C) -> Result<HashMap<String, Value>, InlineMutationError> {
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
		Ok(values)
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
		inline_key: &str,
		submitted_index: usize,
		operation: InlineSaveOperation,
		object_id: String,
		changed_fields: Vec<String>,
		previous_values: HashMap<String, Value>,
	) -> InlineSaveOutcome {
		InlineSaveOutcome {
			inline_key: inline_key.to_owned(),
			submitted_index,
			operation,
			model_identity: self.model_identity.clone(),
			table_name: C::table_name().to_owned(),
			object_id,
			changed_fields,
			previous_values,
		}
	}
}

fn changed_database_fields(
	before: &BTreeMap<String, DatabaseValue>,
	after: &BTreeMap<String, DatabaseValue>,
) -> Vec<String> {
	after
		.iter()
		.filter(|(field, value)| before.get(*field) != Some(*value))
		.map(|(field, _)| field.clone())
		.collect()
}

fn auto_now_database_values<C: FormModel>() -> HashMap<String, DatabaseValue> {
	let now = chrono::Utc::now();
	C::field_metadata()
		.into_iter()
		.filter_map(|field| {
			if !matches!(
				field.attributes.get("auto_now"),
				Some(reinhardt_db::orm::fields::FieldKwarg::Bool(true))
			) {
				return None;
			}
			let value = match field.storage_kind? {
				DatabaseStorageKind::Date => DatabaseValue::Date(now.date_naive()),
				DatabaseStorageKind::Time => DatabaseValue::Time(now.time()),
				DatabaseStorageKind::DateTime => DatabaseValue::DateTime(now),
				DatabaseStorageKind::NaiveDateTime => DatabaseValue::NaiveDateTime(now.naive_utc()),
				_ => return None,
			};
			Some((field.name, value))
		})
		.collect()
}

fn owned_child_query<C>(
	manager: &C::Objects,
	foreign_key: &str,
	child_primary_key: &DatabaseValue,
	parent_primary_key: &DatabaseValue,
) -> QuerySet<C>
where
	C: FormModel,
{
	manager
		.all()
		.filter(Filter::new(
			C::primary_key_field(),
			FilterOperator::Eq,
			FilterValue::Typed(Ok(child_primary_key.clone())),
		))
		.filter(Filter::new(
			foreign_key,
			FilterOperator::Eq,
			FilterValue::Typed(Ok(parent_primary_key.clone())),
		))
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

async fn load_one_with_manager<M, O>(
	manager: &O,
	field: &str,
	value: FilterValue,
	owner: Option<(&str, FilterValue)>,
	transaction: &mut AtomicTransaction,
) -> Result<Option<M>, InlineMutationError>
where
	M: Model,
	O: CustomManager<Model = M>,
{
	let mut query = manager
		.all()
		.filter(Filter::new(field, FilterOperator::Eq, value));
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
	filter_value_kind(kind, field, value)
}

fn filter_value_kind(
	kind: ModelFormFieldKind,
	field: &str,
	value: &str,
) -> Result<FilterValue, InlineMutationError> {
	match kind {
		ModelFormFieldKind::Integer { .. } => value
			.parse::<i64>()
			.map(FilterValue::Integer)
			.or_else(|_| {
				value
					.parse::<u64>()
					.map(|_| FilterValue::String(value.to_owned()))
			})
			.map_err(|_| InlineMutationError::Validation(format!("invalid integer ID '{value}'"))),
		ModelFormFieldKind::Uuid => value
			.parse()
			.map(DatabaseValue::Uuid)
			.map(|value| FilterValue::Typed(Ok(value)))
			.map_err(|_| InlineMutationError::Validation(format!("invalid UUID ID '{value}'"))),
		ModelFormFieldKind::Text { .. }
		| ModelFormFieldKind::Email { .. }
		| ModelFormFieldKind::Url { .. } => Ok(FilterValue::String(value.to_owned())),
		_ => Err(InlineMutationError::Validation(format!(
			"unsupported inline identifier type for field '{field}'"
		))),
	}
}

fn normalize_filter_value(value: FilterValue) -> Result<String, InlineMutationError> {
	match value {
		FilterValue::Typed(Ok(DatabaseValue::Uuid(value))) => Ok(value.to_string()),
		FilterValue::String(value) => Ok(value),
		FilterValue::Integer(value) => Ok(value.to_string()),
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
				(
					format!("{}.{}.{}", inline_key, index, field),
					messages.clone(),
				)
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
	use crate::core::{AdminSite, InlineStyle as PublicInlineStyle, ModelAdmin, ModelAdminConfig};
	use crate::server::inline::{ParsedInlineMutations, remove_unchanged_inline_mutations};
	use reinhardt_db::associations::ForeignKeyField;
	use reinhardt_db::backends::DatabaseConnection as BackendsConnection;
	use reinhardt_db::orm::{DatabaseConnectionLease, DatabaseValue, Manager, QueryValue};
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
		#[field(auto_now = true, null = true)]
		updated_at: Option<chrono::NaiveDateTime>,
	}

	#[model(
		app_label = "admin",
		table_name = "inline_typed_parents",
		form = true,
		info = false
	)]
	#[derive(Clone, Deserialize, Serialize)]
	struct TypedParent {
		#[field(primary_key = true)]
		id: Option<i32>,
	}

	#[model(
		app_label = "admin",
		table_name = "inline_typed_children",
		form = true,
		info = false
	)]
	#[derive(Clone, Deserialize, Serialize)]
	struct TypedChild {
		#[field(primary_key = true)]
		id: Option<i32>,
		#[rel(foreign_key, related_name = "typed_children")]
		parent: ForeignKeyField<TypedParent>,
		#[field(max_length = 100)]
		name: String,
	}

	#[model(
		app_label = "admin",
		table_name = "inline_renamed_parents",
		form = true,
		info = false
	)]
	#[derive(Clone, Deserialize, Serialize)]
	struct RenamedParent {
		#[field(primary_key = true, db_column = "parent_pk")]
		id: Option<i64>,
	}

	#[model(
		app_label = "admin",
		table_name = "inline_renamed_children",
		form = true,
		info = false
	)]
	#[derive(Clone, Deserialize, Serialize)]
	struct RenamedChild {
		#[field(primary_key = true)]
		id: Option<i64>,
		#[rel(foreign_key, related_name = "renamed_children")]
		parent: ForeignKeyField<RenamedParent>,
		#[field(max_length = 100)]
		name: String,
	}

	#[model(
		app_label = "admin",
		table_name = "inline_composite_children",
		form = true,
		info = false
	)]
	#[derive(Clone, Deserialize, Serialize)]
	struct CompositeChild {
		#[field(primary_key = true)]
		tenant_id: i64,
		#[field(primary_key = true)]
		sequence: i64,
		#[rel(foreign_key, related_name = "composite_children")]
		parent: ForeignKeyField<Parent>,
		#[field(max_length = 100)]
		name: String,
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
		assert!(!inline.delete_enabled());
	}

	#[rstest]
	fn inline_configuration_rejects_invalid_relationships_and_fields() {
		let composite = InlineModelAdmin::new::<Parent, CompositeChild>(
			"Composite child",
			"parent_id",
			&["name"],
		)
		.unwrap_err();
		assert_eq!(
			composite.to_string(),
			"Validation error: inline child model must have exactly one primary key field"
		);

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
			(
				"__present",
				"Validation error: inline field '__present' is reserved",
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
	fn inline_identifier_kinds_reject_inexact_and_temporal_values() {
		for kind in [
			ModelFormFieldKind::Float {
				min: None,
				max: None,
			},
			ModelFormFieldKind::Decimal {
				min: None,
				max: None,
			},
			ModelFormFieldKind::Boolean,
			ModelFormFieldKind::Date,
			ModelFormFieldKind::Time,
			ModelFormFieldKind::DateTime,
			ModelFormFieldKind::NaiveDateTime,
			ModelFormFieldKind::Json,
		] {
			let error = validate_identifier_kind("child primary key", kind).unwrap_err();
			assert_eq!(
				error.to_string(),
				"Validation error: inline child primary key uses an unsupported identifier type"
			);
		}
	}

	#[rstest]
	fn uuid_inline_identifiers_use_typed_filters_and_canonical_strings() {
		let schema = [reinhardt_core::model_form::ModelFormFieldDescriptor {
			name: "id",
			kind: ModelFormFieldKind::Uuid,
			required: true,
			has_default: false,
			nullable: false,
			editable: false,
			generated_relation_id: false,
			trim: false,
		}];
		let id = "01983c74-08c2-7ad2-a596-6bdbba00be40";

		let normalized = normalize_filter_value(filter_value(&schema, "id", id).unwrap()).unwrap();

		assert_eq!(normalized, id);
	}

	#[rstest]
	fn wide_integer_inline_ids_are_not_narrowed_to_i64() {
		let schema = [reinhardt_core::model_form::ModelFormFieldDescriptor {
			name: "id",
			kind: ModelFormFieldKind::Integer {
				min: None,
				max: None,
			},
			required: true,
			has_default: false,
			nullable: false,
			editable: false,
			generated_relation_id: false,
			trim: false,
		}];
		let id = "9223372036854775808";

		let filter = filter_value(&schema, "id", id).unwrap();

		match &filter {
			FilterValue::String(value) => assert_eq!(value, id),
			value => panic!("expected a string filter value, got {value:?}"),
		}
		assert_eq!(normalize_filter_value(filter).unwrap(), id);
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
	fn inline_builder_rejects_a_mismatched_typed_parent() {
		let inline =
			InlineModelAdmin::new::<Parent, Child>("Child", "parent_id", &["name"]).unwrap();

		let error = ModelAdminConfig::builder()
			.model_name("OtherParent")
			.inlines(vec![inline])
			.build()
			.unwrap_err();

		assert_eq!(
			error.to_string(),
			"Validation error: inline 'inline_children-parent_id' targets parent 'inline_parents:id', but the admin is 'OtherParent:id'"
		);
	}

	struct CustomInlineAdmin {
		inline: InlineModelAdmin,
	}

	#[async_trait::async_trait]
	impl ModelAdmin for CustomInlineAdmin {
		fn model_name(&self) -> &str {
			"OtherParent"
		}

		fn table_name(&self) -> &str {
			"inline_other_parents"
		}

		fn inlines(&self) -> Vec<InlineModelAdmin> {
			vec![self.inline.clone()]
		}
	}

	#[rstest]
	fn admin_site_rejects_a_custom_admin_with_a_mismatched_typed_parent() {
		let site = AdminSite::new("Admin");
		let inline =
			InlineModelAdmin::new::<Parent, Child>("Child", "parent_id", &["name"]).unwrap();

		let error = site
			.register("OtherParent", CustomInlineAdmin { inline })
			.unwrap_err();

		assert_eq!(
			error.to_string(),
			"Validation error: inline 'inline_children-parent_id' targets parent 'inline_parents:id', but the admin is 'inline_other_parents:id'"
		);
	}

	#[rstest]
	fn inline_builder_matches_a_renamed_parent_primary_key_column() {
		let inline = InlineModelAdmin::new::<RenamedParent, RenamedChild>(
			"Line Item",
			"parent_id",
			&["name"],
		)
		.unwrap();

		let admin = ModelAdminConfig::builder()
			.model_name("RenamedParent")
			.table_name("inline_renamed_parents")
			.pk_field("parent_pk")
			.inlines(vec![inline])
			.build()
			.expect("physical primary-key column should match typed parent");

		assert_eq!(admin.pk_field(), "parent_pk");
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
		let manager = Manager::<Child>::new();
		let query = owned_child_query::<Child>(
			&manager,
			"parent_id",
			&DatabaseValue::I64(7),
			&DatabaseValue::I64(1),
		);

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
	fn owned_child_writes_preserve_typed_primary_key_codecs() {
		let child_primary_key = TypedChild::primary_key_database_value(&7).unwrap();
		let parent_primary_key = TypedParent::primary_key_database_value(&1).unwrap();
		let manager = Manager::<TypedChild>::new();
		let query = owned_child_query::<TypedChild>(
			&manager,
			"parent_id",
			&child_primary_key,
			&parent_primary_key,
		);

		assert_eq!(child_primary_key, DatabaseValue::I32(7));
		assert_eq!(parent_primary_key, DatabaseValue::I32(1));
		assert_eq!(query.filters().len(), 2);
		assert_eq!(query.filters()[0].field, "id");
		match &query.filters()[0].value {
			FilterValue::Typed(Ok(value)) => assert_eq!(value, &DatabaseValue::I32(7)),
			value => panic!("expected typed child primary key, got {value:?}"),
		}
		assert_eq!(query.filters()[1].field, "parent_id");
		match &query.filters()[1].value {
			FilterValue::Typed(Ok(value)) => assert_eq!(value, &DatabaseValue::I32(1)),
			value => panic!("expected typed parent primary key, got {value:?}"),
		}
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
				"CREATE TABLE inline_children (id INTEGER PRIMARY KEY AUTOINCREMENT, parent_id INTEGER NOT NULL, name TEXT NOT NULL, position BIGINT NOT NULL, updated_at TEXT)",
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
	async fn inline_update_history_keeps_only_changed_fields() {
		let (_lease, mut connection) = sqlite_connection().await;
		seed_parent(&connection, 1, "parent").await;
		seed_child(&connection, 10, 1, "first", 1).await;
		let inline =
			InlineModelAdmin::new::<Parent, Child>("Child", "parent_id", &["name", "position"])
				.unwrap();
		let mut mutations = vec![ParsedInlineMutations {
			key: inline.key().to_owned(),
			rows: vec![mutation(0, Some("10"), "updated", 1, false)],
		}];

		remove_unchanged_inline_mutations(
			std::slice::from_ref(&inline),
			"1",
			&mut mutations,
			&HashMap::from([(
				inline.key().to_owned(),
				crate::core::AdminQuery::new(inline.adapter().table_name()),
			)]),
			&mut connection,
		)
		.await
		.expect("unchanged inline values should be removed");
		assert_eq!(
			mutations[0].rows[0].values,
			HashMap::from([("name".to_owned(), json!("updated"))])
		);

		let outcomes = connection
			.atomic(async |transaction| {
				inline
					.adapter()
					.save_rows(inline.key(), "1", mutations.remove(0).rows, transaction)
					.await
			})
			.await
			.expect("inline update should commit");
		assert_eq!(outcomes[0].changed_fields, ["name", "updated_at"]);
	}

	#[rstest]
	#[tokio::test]
	async fn unchanged_detection_cannot_read_rows_outside_the_object_scope() {
		let (_lease, mut connection) = sqlite_connection().await;
		seed_parent(&connection, 1, "parent").await;
		seed_child(&connection, 10, 1, "secret", 1).await;
		let inline =
			InlineModelAdmin::new::<Parent, Child>("Child", "parent_id", &["name", "position"])
				.unwrap();
		let mut mutations = vec![ParsedInlineMutations {
			key: inline.key().to_owned(),
			rows: vec![mutation(0, Some("10"), "secret", 1, false)],
		}];
		let denied_scope = AdminQuery::new(Child::table_name()).filter(Filter::new(
			"position",
			FilterOperator::Eq,
			FilterValue::Integer(2),
		));

		remove_unchanged_inline_mutations(
			std::slice::from_ref(&inline),
			"1",
			&mut mutations,
			&HashMap::from([(inline.key().to_owned(), denied_scope)]),
			&mut connection,
		)
		.await
		.expect("out-of-scope rows must remain unread");

		assert_eq!(mutations[0].rows.len(), 1);
		assert_eq!(mutations[0].rows[0].values["name"], json!("secret"));
	}

	#[rstest]
	fn inline_history_detects_manager_induced_field_changes() {
		let before = BTreeMap::from([
			("name".to_owned(), DatabaseValue::String("draft".to_owned())),
			("status".to_owned(), DatabaseValue::String("new".to_owned())),
		]);
		let after = BTreeMap::from([
			("name".to_owned(), DatabaseValue::String("draft".to_owned())),
			("slug".to_owned(), DatabaseValue::String("draft".to_owned())),
			(
				"status".to_owned(),
				DatabaseValue::String("ready".to_owned()),
			),
		]);

		assert_eq!(changed_database_fields(&before, &after), ["slug", "status"]);
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
			.load_rows("1", MAX_INLINE_ROWS + 1, None, &mut connection)
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
		assert_eq!(
			inline
				.adapter()
				.load_rows("1", 1, None, &mut connection)
				.await
				.unwrap()
				.len(),
			1
		);
		let scoped_query = AdminQuery::new(Child::table_name()).filter(Filter::new(
			"position",
			FilterOperator::Eq,
			FilterValue::Integer(2),
		));
		let scoped = inline
			.adapter()
			.load_rows(
				"1",
				MAX_INLINE_ROWS + 1,
				Some(&scoped_query),
				&mut connection,
			)
			.await
			.unwrap();
		assert_eq!(scoped.len(), 1);
		assert_eq!(scoped[0].id.as_deref(), Some("11"));

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
		let inline_key = inline.key().to_owned();
		assert_eq!(
			outcomes,
			vec![
				InlineSaveOutcome {
					inline_key: inline_key.clone(),
					submitted_index: 2,
					operation: InlineSaveOperation::Update,
					model_identity: "Child".to_owned(),
					table_name: "inline_children".to_owned(),
					object_id: "10".to_owned(),
					changed_fields: vec![
						"name".to_owned(),
						"position".to_owned(),
						"updated_at".to_owned(),
					],
					previous_values: HashMap::from([
						("name".to_owned(), json!("first")),
						("position".to_owned(), json!(1)),
					]),
				},
				InlineSaveOutcome {
					inline_key: inline_key.clone(),
					submitted_index: 4,
					operation: InlineSaveOperation::Delete,
					model_identity: "Child".to_owned(),
					table_name: "inline_children".to_owned(),
					object_id: "11".to_owned(),
					changed_fields: Vec::new(),
					previous_values: HashMap::from([
						("name".to_owned(), json!("second")),
						("position".to_owned(), json!(2)),
					]),
				},
				InlineSaveOutcome {
					inline_key,
					submitted_index: 7,
					operation: InlineSaveOperation::Create,
					model_identity: "Child".to_owned(),
					table_name: "inline_children".to_owned(),
					object_id: "12".to_owned(),
					changed_fields: vec!["name".to_owned(), "position".to_owned()],
					previous_values: HashMap::new(),
				},
			]
		);

		let mut loaded = inline
			.adapter()
			.load_rows("1", MAX_INLINE_ROWS + 1, None, &mut connection)
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
