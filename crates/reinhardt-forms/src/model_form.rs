//! ModelForm implementation for ORM integration
//!
//! ModelForms automatically generate forms from ORM models, handling field
//! inference, validation, and saving.

mod error;
mod field_factory;

pub use error::ModelFormError;

use crate::Form;
use crate::form::ALL_FIELDS_KEY;
use reinhardt_core::model_form::{
	AllEditableModelFields, ModelFormPayload, ModelFormPayloadError, ModelFormPolicy,
	ModelFormSchema,
};
use reinhardt_db::orm::{Model, OrmExecutor};
use serde_json::Value;
use std::collections::HashMap;
use std::marker::PhantomData;

/// Explicit persistence operation used for an already validated model candidate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelFormPersistenceMode {
	/// Insert a candidate created from a form payload.
	Create,
	/// Update a candidate built from an existing model instance.
	Update,
}

/// Native bridge generated for models that opt in to model-backed forms.
// The native model form contract intentionally exposes an async persistence method.
#[allow(async_fn_in_trait)]
pub trait FormModel: Model + Clone + Send + Sync {
	/// Generated descriptor schema for this model.
	type Schema: ModelFormSchema<Model = Self>;
	/// Generated typed payload under the active field policy.
	type Data<P: ModelFormPolicy>: ModelFormPayload<P>;

	/// Builds a create candidate from supplied values and declared model defaults.
	fn build_from_payload<P: ModelFormPolicy>(data: &Self::Data<P>)
	-> Result<Self, ModelFormError>;

	/// Builds a validation-only candidate while allowing one trusted deferred field.
	///
	/// Inline formsets use this before a newly created parent has a generated
	/// primary key. Implementations must use the deferred field only to construct
	/// the candidate for validation; persistence must still require its real value.
	fn build_from_payload_with_deferred_required_field<P: ModelFormPolicy>(
		data: &Self::Data<P>,
		deferred_field: &str,
	) -> Result<Self, ModelFormError> {
		let _ = deferred_field;
		Self::build_from_payload(data)
	}

	/// Applies supplied payload values to an existing candidate.
	fn apply_payload<P: ModelFormPolicy>(
		&mut self,
		data: &Self::Data<P>,
	) -> Result<(), ModelFormError>;

	/// Persists this candidate using an explicit create or update operation.
	async fn save_with_mode(
		&mut self,
		executor: &mut dyn OrmExecutor,
		mode: ModelFormPersistenceMode,
	) -> Result<(), ModelFormError>;

	/// Inserts this candidate using the caller-owned ORM executor.
	///
	/// Call [`Self::save_with_mode`] with [`ModelFormPersistenceMode::Update`]
	/// when persisting a known existing model.
	async fn save(&mut self, executor: &mut dyn OrmExecutor) -> Result<(), ModelFormError> {
		self.save_with_mode(executor, ModelFormPersistenceMode::Create)
			.await
	}

	/// Convert model instance to a choice label for display in forms
	///
	/// Default implementation returns the string representation of the primary key.
	///
	/// Derive-generated implementations use this default. Configure a
	/// [`crate::ModelChoiceField`] or [`crate::ModelMultipleChoiceField`] with
	/// its `choice_label` callback when an application needs a custom label.
	///
	/// # Examples
	///
	/// ```ignore
	/// # struct Example { id: i32, name: String }
	/// # impl Example {
	/// fn to_choice_label(&self) -> String {
	///     format!("{} - {}", self.id, self.name)
	/// }
	/// # }
	/// ```
	fn to_choice_label(&self) -> String {
		self.primary_key()
			.map(|primary_key| primary_key.to_string())
			.unwrap_or_default()
	}

	/// Get the primary key value as a string for form field validation
	///
	/// Default implementation uses the "id" field.
	///
	/// # Examples
	///
	/// ```ignore
	/// # struct Example { id: i32 }
	/// # impl Example {
	/// fn to_choice_value(&self) -> String {
	///     self.id.to_string()
	/// }
	/// # }
	/// ```
	fn to_choice_value(&self) -> String {
		self.primary_key()
			.map(|primary_key| primary_key.to_string())
			.unwrap_or_default()
	}
}

type ModelValidator<T> = dyn Fn(&T) -> Result<(), Vec<String>> + Send + Sync;

/// A native form that validates a generated payload and persists model candidates.
pub struct ModelForm<T, P = AllEditableModelFields>
where
	T: FormModel,
	P: ModelFormPolicy,
{
	form: Form,
	data: T::Data<P>,
	supplied_fields: Vec<&'static str>,
	instance: Option<T>,
	validated_candidate: Option<T>,
	persistence_mode: ModelFormPersistenceMode,
	model_validator: Option<Box<ModelValidator<T>>>,
	_policy: PhantomData<P>,
}

impl<T, P> ModelForm<T, P>
where
	T: FormModel,
	P: ModelFormPolicy,
{
	fn initialize(
		data: T::Data<P>,
		instance: Option<T>,
		persistence_mode: ModelFormPersistenceMode,
	) -> Self {
		let supplied_fields = data.supplied_fields();
		let mut form = Form::new();
		let mut form_data = HashMap::new();

		for descriptor in T::Schema::fields() {
			if descriptor.editable
				&& P::allows(descriptor.name)
				&& supplied_fields.contains(&descriptor.name)
			{
				let explicit_null = descriptor.nullable
					&& data
						.get_json(descriptor.name)
						.is_some_and(|value| value.is_null());
				if explicit_null {
					continue;
				}
				form.add_field(field_factory::create_form_field(descriptor));
				if let Some(value) = data.get_json(descriptor.name) {
					form_data.insert(descriptor.name.to_owned(), value);
				}
			}
		}
		form.bind(form_data);

		Self {
			form,
			data,
			supplied_fields,
			instance,
			validated_candidate: None,
			persistence_mode,
			model_validator: None,
			_policy: PhantomData,
		}
	}

	/// Creates a model form for a new instance.
	pub fn from_payload(data: T::Data<P>) -> Self {
		Self::initialize(data, None, ModelFormPersistenceMode::Create)
	}

	/// Creates a model form that applies a payload to an existing instance.
	pub fn from_payload_and_instance(data: T::Data<P>, instance: T) -> Self {
		Self::initialize(data, Some(instance), ModelFormPersistenceMode::Update)
	}

	/// Installs a model-level validator that runs after cleaned values are applied.
	pub fn with_model_validator(
		mut self,
		validator: impl Fn(&T) -> Result<(), Vec<String>> + Send + Sync + 'static,
	) -> Self {
		self.model_validator = Some(Box::new(validator));
		self.validated_candidate = None;
		self
	}
	fn clean_payload(&mut self) -> Result<(), ModelFormError> {
		if let Some(field) = self.data.forbidden_fields().first() {
			return Err(ModelFormError::ForbiddenInput { field });
		}
		if self.persistence_mode == ModelFormPersistenceMode::Update
			&& self.supplied_fields.contains(&T::primary_key_field())
		{
			return Err(ModelFormError::FieldValidation {
				errors: HashMap::from([(
					T::primary_key_field().to_owned(),
					vec!["model form primary keys cannot be updated".to_owned()],
				)]),
			});
		}

		if !self.form.is_valid() {
			return Err(ModelFormError::FieldValidation {
				errors: self.form.errors().clone(),
			});
		}

		for field in &self.supplied_fields {
			let Some(value) = self.form.cleaned_data().get(*field).cloned() else {
				continue;
			};
			self.data.set_json(field, value).map_err(|error| {
				let message = error.to_string();
				match error {
					ModelFormPayloadError::ForbiddenField { .. } => {
						ModelFormError::ForbiddenInput { field }
					}
					ModelFormPayloadError::UnknownField { .. }
					| ModelFormPayloadError::InvalidValue { .. } => ModelFormError::FieldValidation {
						errors: HashMap::from([((*field).to_owned(), vec![message])]),
					},
				}
			})?;
		}

		Ok(())
	}

	/// Validates the payload and builds a model candidate without database access.
	pub fn build_instance(&mut self) -> Result<T, ModelFormError> {
		if let Some(candidate) = &self.validated_candidate {
			return Ok(candidate.clone());
		}

		self.clean_payload()?;
		let mut candidate = match &self.instance {
			Some(instance) => instance.clone(),
			None => T::build_from_payload(&self.data)?,
		};
		candidate.apply_payload(&self.data)?;

		if let Some(validator) = &self.model_validator {
			validator(&candidate).map_err(|errors| ModelFormError::ModelValidation { errors })?;
		}

		self.validated_candidate = Some(candidate.clone());
		Ok(candidate)
	}

	/// Returns whether the current payload can produce a valid model candidate.
	pub fn is_valid(&mut self) -> bool {
		match self.build_instance() {
			Ok(_) => true,
			Err(error) => {
				self.record_validation_error(&error);
				false
			}
		}
	}

	fn record_validation_error(&mut self, error: &ModelFormError) {
		match error {
			ModelFormError::ForbiddenInput { field }
			| ModelFormError::MissingModelField { field } => {
				self.form.add_error(*field, error.to_string());
			}
			ModelFormError::FieldValidation { errors } => {
				for (field, messages) in errors {
					for message in messages {
						let already_recorded = self
							.form
							.errors()
							.get(field)
							.is_some_and(|existing| existing.contains(message));
						if !already_recorded {
							self.form.add_error(field, message);
						}
					}
				}
			}
			ModelFormError::ModelValidation { errors } => {
				for message in errors {
					self.form.add_error(ALL_FIELDS_KEY, message);
				}
			}
			ModelFormError::Persistence { .. } | ModelFormError::PersistenceAfterCreate { .. } => {}
		}
	}

	/// Persists a validated candidate through the caller-owned executor.
	pub async fn save(&mut self, executor: &mut dyn OrmExecutor) -> Result<T, ModelFormError> {
		if self.validated_candidate.is_none() {
			self.build_instance()?;
		}

		let candidate = self
			.validated_candidate
			.as_mut()
			.expect("build_instance caches a validated candidate");
		if let Err(error) =
			FormModel::save_with_mode(candidate, executor, self.persistence_mode).await
		{
			if matches!(error, ModelFormError::PersistenceAfterCreate { .. }) {
				self.persistence_mode = ModelFormPersistenceMode::Update;
			}
			return Err(error);
		}
		let saved = candidate.clone();
		self.instance = Some(saved.clone());
		self.persistence_mode = ModelFormPersistenceMode::Update;
		Ok(saved)
	}

	/// Replaces one payload field, primarily for inline foreign-key assignment.
	pub fn set_field_value(
		&mut self,
		field_name: &str,
		value: Value,
	) -> Result<(), ModelFormError> {
		let Some(descriptor) = T::Schema::fields()
			.iter()
			.find(|descriptor| descriptor.name == field_name)
		else {
			return Err(ModelFormError::FieldValidation {
				errors: HashMap::from([(
					field_name.to_owned(),
					vec!["unknown model form field".to_owned()],
				)]),
			});
		};
		let field_name = descriptor.name;
		let form_value = value.clone();
		self.data.set_json(field_name, value).map_err(|error| {
			let message = error.to_string();
			match error {
				ModelFormPayloadError::ForbiddenField { .. } => {
					ModelFormError::ForbiddenInput { field: field_name }
				}
				ModelFormPayloadError::UnknownField { .. }
				| ModelFormPayloadError::InvalidValue { .. } => ModelFormError::FieldValidation {
					errors: HashMap::from([(field_name.to_owned(), vec![message])]),
				},
			}
		})?;
		let mut bound_values = self.form.cleaned_data().clone();
		if self
			.form
			.fields()
			.iter()
			.all(|field| field.name() != field_name)
		{
			self.form
				.add_field(field_factory::create_form_field(descriptor));
		}
		bound_values.insert(field_name.to_owned(), form_value);
		self.form.bind(bound_values);
		self.validated_candidate = None;
		if !self.supplied_fields.contains(&field_name) {
			self.supplied_fields.push(field_name);
		}
		Ok(())
	}

	/// Returns a reference to the underlying form.
	pub fn form(&self) -> &Form {
		&self.form
	}
	/// Returns a mutable reference to the underlying form.
	pub fn form_mut(&mut self) -> &mut Form {
		self.validated_candidate = None;
		&mut self.form
	}
	/// Returns a reference to the model instance, if one exists.
	pub fn instance(&self) -> Option<&T> {
		self.instance.as_ref()
	}

	pub(crate) fn is_submission_candidate(&self) -> bool {
		self.instance.is_some()
			|| !self.supplied_fields.is_empty()
			|| !self.data.forbidden_fields().is_empty()
	}

	pub(crate) fn is_valid_with_deferred_required_field(&mut self, deferred_field: &str) -> bool {
		let mut valid = self.form.is_valid();
		for descriptor in T::Schema::fields() {
			if descriptor.name == deferred_field
				|| !descriptor.editable
				|| !descriptor.required
				|| self.supplied_fields.contains(&descriptor.name)
			{
				continue;
			}
			self.form
				.add_error(descriptor.name, "This field is required.");
			valid = false;
		}
		if !valid {
			return false;
		}
		if let Err(error) = self.clean_payload() {
			self.record_validation_error(&error);
			return false;
		}
		let mut candidate = match &self.instance {
			Some(instance) => instance.clone(),
			None => {
				match T::build_from_payload_with_deferred_required_field(&self.data, deferred_field)
				{
					Ok(candidate) => candidate,
					Err(error) => {
						self.record_validation_error(&error);
						return false;
					}
				}
			}
		};
		if let Err(error) = candidate.apply_payload(&self.data) {
			self.record_validation_error(&error);
			return false;
		}
		if let Some(validator) = &self.model_validator
			&& let Err(errors) = validator(&candidate)
		{
			self.record_validation_error(&ModelFormError::ModelValidation { errors });
			return false;
		}
		valid
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use chrono::{DateTime, NaiveDate, Utc};
	use std::collections::VecDeque;
	use std::sync::Arc;
	use std::sync::atomic::{AtomicUsize, Ordering};

	use reinhardt_core::exception::{DatabaseError, DatabaseErrorKind, Error};
	use reinhardt_core::model_form::{
		ModelFormFieldDescriptor, ModelFormFieldKind, ModelFormPolicy,
	};
	use reinhardt_db::orm::connection::{
		DatabaseBackend, OrmExecutor, QueryResult, QueryValue, Row,
	};
	use reinhardt_macros::model;
	use serde::{Deserialize, Serialize};
	use serde_json::json;

	#[model(
		app_label = "forms",
		table_name = "model_form_questions",
		form = true,
		info = false
	)]
	#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
	struct Question {
		#[field(primary_key = true)]
		id: Option<i64>,
		#[field(max_length = 200)]
		title: String,
		owner_id: i64,
		#[field(default = true)]
		published: bool,
	}

	#[model(
		app_label = "forms",
		table_name = "model_form_uuid_records",
		form = true,
		info = false
	)]
	#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
	struct UuidRecord {
		#[field(primary_key = true, include_in_new = false)]
		id: uuid::Uuid,
		#[field(max_length = 200)]
		title: String,
	}

	#[model(
		app_label = "forms",
		table_name = "model_form_optional_uuid_records",
		form = true,
		info = false
	)]
	#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
	struct OptionalUuidRecord {
		#[field(primary_key = true)]
		id: Option<uuid::Uuid>,
		#[field(max_length = 200)]
		title: String,
	}

	#[model(
		app_label = "forms",
		table_name = "model_form_zero_sentinel_records",
		form = true,
		info = false
	)]
	#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
	struct ZeroSentinelRecord {
		#[field(primary_key = true)]
		id: i32,
		#[field(max_length = 200)]
		title: String,
	}

	#[model(
		app_label = "forms",
		table_name = "model_form_temporal_records",
		form = true,
		info = false
	)]
	#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
	struct TemporalRecord {
		#[field(primary_key = true)]
		id: Option<i64>,
		aware_at: DateTime<Utc>,
		naive_at: chrono::NaiveDateTime,
		nullable_naive_at: Option<chrono::NaiveDateTime>,
	}

	#[model(
		app_label = "forms",
		table_name = "model_form_hidden_required_records",
		form = true,
		info = false
	)]
	#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
	struct HiddenRequiredRecord {
		#[field(primary_key = true)]
		id: Option<i64>,
		#[field(max_length = 200)]
		title: String,
		#[field(max_length = 200, editable = false)]
		audit_actor: String,
	}

	#[model(
		app_label = "forms",
		table_name = "model_form_skipped_default_records",
		form = true,
		info = false
	)]
	#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
	struct SkippedDefaultRecord {
		#[field(primary_key = true)]
		id: Option<i64>,
		#[field(max_length = 200)]
		title: String,
		#[field(max_length = 200, skip = true)]
		system_value: String,
	}

	#[model(
		app_label = "forms",
		table_name = "model_form_excluded_from_new_records",
		form = true,
		info = false
	)]
	#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
	struct ExcludedFromNewRecord {
		#[field(primary_key = true)]
		id: Option<i64>,
		#[field(max_length = 200)]
		title: String,
		#[field(max_length = 200, include_in_new = false)]
		system_value: String,
	}

	struct QuestionPolicy;

	impl ModelFormPolicy for QuestionPolicy {
		fn allows(field: &str) -> bool {
			matches!(field, "title" | "owner_id" | "published")
		}
	}

	struct TitleOnly;

	impl ModelFormPolicy for TitleOnly {
		fn allows(field: &str) -> bool {
			field == "title"
		}
	}

	#[derive(Debug)]
	struct RetryExecutor {
		rows: VecDeque<Result<Row, Error>>,
		fetch_one_calls: usize,
		queries: Vec<String>,
	}

	impl RetryExecutor {
		fn new(rows: impl IntoIterator<Item = Result<Row, Error>>) -> Self {
			Self {
				rows: rows.into_iter().collect(),
				fetch_one_calls: 0,
				queries: Vec::new(),
			}
		}
	}

	#[reinhardt_core::async_trait]
	impl OrmExecutor for RetryExecutor {
		fn backend(&self) -> DatabaseBackend {
			DatabaseBackend::Postgres
		}

		async fn execute(
			&mut self,
			_sql: &str,
			_params: Vec<QueryValue>,
		) -> Result<QueryResult, Error> {
			Err(DatabaseError::new(DatabaseErrorKind::Query, "unexpected execute call").into())
		}

		async fn fetch_one(&mut self, sql: &str, _params: Vec<QueryValue>) -> Result<Row, Error> {
			self.fetch_one_calls += 1;
			self.queries.push(sql.to_owned());
			self.rows.pop_front().unwrap_or_else(|| {
				Err(DatabaseError::new(DatabaseErrorKind::Query, "missing queued row").into())
			})
		}

		async fn fetch_all(
			&mut self,
			_sql: &str,
			_params: Vec<QueryValue>,
		) -> Result<Vec<Row>, Error> {
			Err(DatabaseError::new(DatabaseErrorKind::Query, "unexpected fetch_all call").into())
		}

		async fn fetch_optional(
			&mut self,
			_sql: &str,
			_params: Vec<QueryValue>,
		) -> Result<Option<Row>, Error> {
			Err(
				DatabaseError::new(DatabaseErrorKind::Query, "unexpected fetch_optional call")
					.into(),
			)
		}
	}

	#[derive(Debug)]
	struct MySqlHydrationRetryExecutor {
		fetch_rows: VecDeque<Result<Row, Error>>,
		queries: Vec<String>,
	}

	impl MySqlHydrationRetryExecutor {
		fn new(fetch_rows: impl IntoIterator<Item = Result<Row, Error>>) -> Self {
			Self {
				fetch_rows: fetch_rows.into_iter().collect(),
				queries: Vec::new(),
			}
		}
	}

	#[reinhardt_core::async_trait]
	impl OrmExecutor for MySqlHydrationRetryExecutor {
		fn backend(&self) -> DatabaseBackend {
			DatabaseBackend::MySql
		}

		async fn execute(
			&mut self,
			sql: &str,
			_params: Vec<QueryValue>,
		) -> Result<QueryResult, Error> {
			self.queries.push(sql.to_owned());
			Ok(QueryResult {
				rows_affected: 1,
				last_insert_id: Some(23),
			})
		}

		async fn fetch_one(&mut self, sql: &str, _params: Vec<QueryValue>) -> Result<Row, Error> {
			self.queries.push(sql.to_owned());
			self.fetch_rows.pop_front().unwrap_or_else(|| {
				Err(DatabaseError::new(DatabaseErrorKind::Query, "missing queued row").into())
			})
		}

		async fn fetch_all(
			&mut self,
			_sql: &str,
			_params: Vec<QueryValue>,
		) -> Result<Vec<Row>, Error> {
			Err(DatabaseError::new(DatabaseErrorKind::Query, "unexpected fetch_all call").into())
		}

		async fn fetch_optional(
			&mut self,
			_sql: &str,
			_params: Vec<QueryValue>,
		) -> Result<Option<Row>, Error> {
			Err(
				DatabaseError::new(DatabaseErrorKind::Query, "unexpected fetch_optional call")
					.into(),
			)
		}
	}

	fn question_row(id: i64, title: &str, owner_id: i64, published: bool) -> Row {
		let mut row = Row::new();
		row.insert("id".to_owned(), QueryValue::Int(id));
		row.insert("title".to_owned(), QueryValue::String(title.to_owned()));
		row.insert("owner_id".to_owned(), QueryValue::Int(owner_id));
		row.insert("published".to_owned(), QueryValue::Bool(published));
		row
	}

	fn question_payload(title: &str, owner_id: i64) -> QuestionModelFormData<QuestionPolicy> {
		let mut data = QuestionModelFormData::<QuestionPolicy>::empty();
		data.set_title(title.to_owned());
		data.set_owner_id(owner_id);
		data
	}

	fn uuid_record_row(id: uuid::Uuid, title: &str) -> Row {
		let mut row = Row::new();
		row.insert("id".to_owned(), QueryValue::Uuid(id));
		row.insert("title".to_owned(), QueryValue::String(title.to_owned()));
		row
	}

	fn optional_uuid_record_row(id: uuid::Uuid, title: &str) -> Row {
		uuid_record_row(id, title)
	}

	fn zero_sentinel_record_row(title: &str) -> Row {
		let mut row = Row::new();
		row.insert("id".to_owned(), QueryValue::Int(0));
		row.insert("title".to_owned(), QueryValue::String(title.to_owned()));
		row
	}

	#[test]
	fn generated_model_form_builds_create_candidate_from_typed_payload() {
		let data = question_payload("Created", 7);

		let mut form = ModelForm::<Question, QuestionPolicy>::from_payload(data);
		let built = form.build_instance().unwrap();

		assert_eq!(built.title, "Created");
		assert_eq!(built.owner_id, 7);
		assert_eq!(built.id, None);
	}

	#[test]
	fn generated_model_form_preserves_omitted_update_fields() {
		let mut data = QuestionModelFormData::<QuestionPolicy>::empty();
		data.set_title("Updated".to_owned());
		let instance = Question {
			id: Some(19),
			title: "Original".to_owned(),
			owner_id: 41,
			published: false,
		};

		let mut form =
			ModelForm::<Question, QuestionPolicy>::from_payload_and_instance(data, instance);
		let built = form.build_instance().unwrap();

		assert_eq!(built.id, Some(19));
		assert_eq!(built.title, "Updated");
		assert_eq!(built.owner_id, 41);
		assert!(!built.published);
	}

	#[test]
	fn generated_model_form_uses_declared_model_defaults_on_create() {
		let data = question_payload("Defaulted", 3);

		let mut form = ModelForm::<Question, QuestionPolicy>::from_payload(data);
		let built = form.build_instance().unwrap();

		assert!(built.published);
	}

	#[test]
	fn generated_model_form_round_trips_aware_and_naive_datetimes_through_native_fields() {
		let mut data = TemporalRecordModelFormData::<AllEditableModelFields>::empty();
		data.set_json("aware_at", json!("2026-07-25T14:30:00Z"))
			.expect("aware datetime should deserialize");
		data.set_json("naive_at", json!("2026-07-25T14:30:00"))
			.expect("naive datetime should deserialize");
		let mut form = ModelForm::<TemporalRecord>::from_payload(data);

		let built = form
			.build_instance()
			.expect("native field cleaning should preserve both datetime types");

		let expected = NaiveDate::from_ymd_opt(2026, 7, 25)
			.expect("valid date")
			.and_hms_opt(14, 30, 0)
			.expect("valid time");
		assert_eq!(
			built.aware_at,
			DateTime::<Utc>::from_naive_utc_and_offset(expected, Utc)
		);
		assert_eq!(built.naive_at, expected);
		assert_eq!(built.nullable_naive_at, None);
	}

	#[test]
	fn generated_model_form_accepts_explicit_null_for_nullable_non_text_field() {
		let mut data = TemporalRecordModelFormData::<AllEditableModelFields>::empty();
		data.set_json("aware_at", json!("2026-07-25T14:30:00Z"))
			.expect("aware datetime should deserialize");
		data.set_json("naive_at", json!("2026-07-25T14:30:00"))
			.expect("naive datetime should deserialize");
		data.set_json("nullable_naive_at", Value::Null)
			.expect("nullable datetime should accept an explicit clear");
		let mut form = ModelForm::<TemporalRecord>::from_payload(data);

		let built = form
			.build_instance()
			.expect("explicit null should bypass non-null field conversion");

		assert_eq!(built.nullable_naive_at, None);
	}

	#[test]
	fn generated_model_form_reports_unresolved_required_model_field() {
		let mut data = QuestionModelFormData::<QuestionPolicy>::empty();
		data.set_title("Missing owner".to_owned());

		let mut form = ModelForm::<Question, QuestionPolicy>::from_payload(data);
		let error = form.build_instance().unwrap_err();

		assert!(matches!(
			error,
			ModelFormError::MissingModelField { field: "owner_id" }
		));
	}

	#[test]
	fn generated_model_form_reports_unresolved_required_non_editable_field() {
		let mut data = HiddenRequiredRecordModelFormData::<AllEditableModelFields>::empty();
		data.set_title("Missing audit actor".to_owned());

		let mut form = ModelForm::<HiddenRequiredRecord>::from_payload(data);
		let error = form.build_instance().unwrap_err();

		assert!(matches!(
			error,
			ModelFormError::MissingModelField {
				field: "audit_actor"
			}
		));
	}

	#[test]
	fn generated_model_form_default_initializes_skipped_field() {
		let mut data = SkippedDefaultRecordModelFormData::<AllEditableModelFields>::empty();
		data.set_title("Skipped default".to_owned());

		let mut form = ModelForm::<SkippedDefaultRecord>::from_payload(data);
		let built = form.build_instance().unwrap();

		assert_eq!(built.title, "Skipped default");
		assert_eq!(built.system_value, "");
	}

	#[test]
	fn generated_model_form_default_initializes_field_excluded_from_new() {
		let mut data = ExcludedFromNewRecordModelFormData::<AllEditableModelFields>::empty();
		data.set_title("Excluded default".to_owned());

		let mut form = ModelForm::<ExcludedFromNewRecord>::from_payload(data);
		let built = form.build_instance().unwrap();

		assert_eq!(built.title, "Excluded default");
		assert_eq!(built.system_value, "");
	}

	#[test]
	fn generated_model_form_applies_cleaned_values_before_model_validation() {
		let data = question_payload("  cleaned title  ", 5);
		let mut form = ModelForm::<Question, QuestionPolicy>::from_payload(data)
			.with_model_validator(|candidate| {
				if candidate.title == "CLEANED TITLE" {
					Ok(())
				} else {
					Err(vec!["validator observed uncleaned data".to_owned()])
				}
			});
		form.form_mut().add_field_clean_function("title", |value| {
			Ok(json!(
				value
					.as_str()
					.expect("title cleaner receives text")
					.trim()
					.to_uppercase()
			))
		});

		let built = form.build_instance().unwrap();

		assert_eq!(built.title, "CLEANED TITLE");
	}

	#[test]
	fn replacement_value_overrides_the_bound_form_value() {
		let data = question_payload("Replacement", 7);
		let mut form = ModelForm::<Question, QuestionPolicy>::from_payload(data);

		form.set_field_value("owner_id", json!(9)).unwrap();
		let built = form.build_instance().unwrap();

		assert_eq!(built.owner_id, 9);
	}

	#[test]
	fn recorded_forbidden_wire_input_precedes_field_cleaning() {
		let data: QuestionModelFormData<TitleOnly> = serde_json::from_value(json!({
			"title": "",
			"owner_id": 7,
		}))
		.unwrap();

		let mut form = ModelForm::<Question, TitleOnly>::from_payload(data);
		let error = form.build_instance().unwrap_err();

		assert!(matches!(
			error,
			ModelFormError::ForbiddenInput { field: "owner_id" }
		));
	}

	#[test]
	fn is_valid_records_structured_model_errors_on_the_form() {
		let data: QuestionModelFormData<TitleOnly> = serde_json::from_value(json!({
			"title": "Question",
			"owner_id": 7,
		}))
		.unwrap();

		let mut form = ModelForm::<Question, TitleOnly>::from_payload(data);

		assert!(!form.is_valid());
		assert_eq!(
			form.form().errors().get("owner_id"),
			Some(&vec!["model form field 'owner_id' is forbidden".to_owned()])
		);
	}

	#[test]
	fn generated_model_form_reuses_non_idempotently_cleaned_candidate_across_retry() {
		let data = question_payload("Retryable", 17);
		let cleaner_calls = Arc::new(AtomicUsize::new(0));
		let mut executor = RetryExecutor::new([
			Err(Error::database_with_source(
				DatabaseErrorKind::Timeout,
				"temporary database timeout",
				std::io::Error::new(std::io::ErrorKind::TimedOut, "driver timeout"),
			)),
			Ok(question_row(23, "Retryable-1", 17, true)),
		]);
		let mut form = ModelForm::<Question, QuestionPolicy>::from_payload(data);
		let cleaner_calls_for_field = Arc::clone(&cleaner_calls);
		form.form_mut()
			.add_field_clean_function("title", move |value| {
				let call = cleaner_calls_for_field.fetch_add(1, Ordering::SeqCst) + 1;
				Ok(json!(format!(
					"{}-{call}",
					value.as_str().expect("title cleaner receives text")
				)))
			});

		let built = form.build_instance().unwrap();
		assert_eq!(built.title, "Retryable-1");
		assert_eq!(cleaner_calls.load(Ordering::SeqCst), 1);

		let first_error = tokio_test::block_on(form.save(&mut executor)).unwrap_err();
		assert_eq!(
			first_error.database_error().map(DatabaseError::kind),
			Some(DatabaseErrorKind::Timeout)
		);
		assert!(form.instance().is_none());
		assert_eq!(form.build_instance().unwrap(), built);
		assert_eq!(cleaner_calls.load(Ordering::SeqCst), 1);

		let saved = tokio_test::block_on(form.save(&mut executor)).unwrap();
		assert_eq!(saved.id, Some(23));
		assert_eq!(saved.title, "Retryable-1");
		assert_eq!(form.instance(), Some(&saved));
		assert_eq!(executor.fetch_one_calls, 2);
		assert_eq!(cleaner_calls.load(Ordering::SeqCst), 1);
	}

	#[test]
	fn mysql_hydration_failure_never_retries_the_insert() {
		let data = question_payload("Persisted before hydration", 17);
		let mut executor = MySqlHydrationRetryExecutor::new([
			Err(DatabaseError::new(DatabaseErrorKind::Query, "MySQL reload failed").into()),
			Ok(question_row(23, "Persisted before hydration", 17, true)),
		]);
		let mut form = ModelForm::<Question, QuestionPolicy>::from_payload(data);

		let error = tokio_test::block_on(form.save(&mut executor)).unwrap_err();
		assert!(matches!(
			error,
			ModelFormError::PersistenceAfterCreate { .. }
		));

		let retry_error = tokio_test::block_on(form.save(&mut executor)).unwrap_err();
		assert!(matches!(retry_error, ModelFormError::Persistence { .. }));
		assert_eq!(
			executor
				.queries
				.iter()
				.filter(|query| query.trim_start().starts_with("INSERT"))
				.count(),
			1
		);
	}

	#[test]
	fn generated_uuid_model_form_include_in_new_false_retries_create_with_same_dynamic_default() {
		let mut data = UuidRecordModelFormData::<AllEditableModelFields>::empty();
		data.set_title("UUID create".to_owned());
		let mut form = ModelForm::<UuidRecord>::from_payload(data);
		let built = form.build_instance().unwrap();
		let generated_id = built.id;
		let mut executor = RetryExecutor::new([
			Err(DatabaseError::new(DatabaseErrorKind::Timeout, "retry UUID create").into()),
			Ok(uuid_record_row(generated_id, "UUID create")),
		]);

		let first_error = tokio_test::block_on(form.save(&mut executor)).unwrap_err();
		assert_eq!(
			first_error.database_error().map(DatabaseError::kind),
			Some(DatabaseErrorKind::Timeout)
		);
		assert_eq!(form.build_instance().unwrap().id, generated_id);

		let saved = tokio_test::block_on(form.save(&mut executor)).unwrap();

		assert_eq!(saved.id, generated_id);
		assert_eq!(executor.fetch_one_calls, 2);
		assert!(
			executor
				.queries
				.iter()
				.all(|query| query.trim_start().starts_with("INSERT"))
		);
	}

	#[test]
	fn generated_optional_uuid_model_form_uses_create_path() {
		let mut data = OptionalUuidRecordModelFormData::<AllEditableModelFields>::empty();
		data.set_title("Optional UUID create".to_owned());
		let mut form = ModelForm::<OptionalUuidRecord>::from_payload(data);
		let built = form.build_instance().unwrap();
		let generated_id = built.id.expect("optional UUID primary key is generated");
		let mut executor = RetryExecutor::new([Ok(optional_uuid_record_row(
			generated_id,
			"Optional UUID create",
		))]);

		let saved = tokio_test::block_on(form.save(&mut executor)).unwrap();

		assert_eq!(saved.id, Some(generated_id));
		assert_eq!(executor.fetch_one_calls, 1);
		assert!(executor.queries[0].trim_start().starts_with("INSERT"));
	}

	#[test]
	fn direct_form_model_save_inserts_assigned_primary_keys() {
		let id = uuid::Uuid::from_u128(0x019c_1234_5678_7abc_8def_0123_4567_89ab);
		let mut record = UuidRecord {
			id,
			title: "Assigned primary key".to_owned(),
		};
		let mut executor = RetryExecutor::new([Ok(uuid_record_row(id, "Assigned primary key"))]);

		tokio_test::block_on(FormModel::save(&mut record, &mut executor)).unwrap();

		assert!(executor.queries[0].trim_start().starts_with("INSERT"));
	}

	#[test]
	fn generated_existing_zero_sentinel_model_form_uses_update_path() {
		let mut data = ZeroSentinelRecordModelFormData::<AllEditableModelFields>::empty();
		data.set_title("Existing zero sentinel".to_owned());
		let instance = ZeroSentinelRecord {
			id: 0,
			title: "Original".to_owned(),
		};
		let mut form = ModelForm::<ZeroSentinelRecord>::from_payload_and_instance(data, instance);
		let mut executor =
			RetryExecutor::new([Ok(zero_sentinel_record_row("Existing zero sentinel"))]);

		let saved = tokio_test::block_on(form.save(&mut executor)).unwrap();

		assert_eq!(saved.id, 0);
		assert_eq!(saved.title, "Existing zero sentinel");
		assert_eq!(executor.fetch_one_calls, 1);
		assert!(executor.queries[0].trim_start().starts_with("UPDATE"));
	}

	#[test]
	fn descriptor_factory_maps_all_supported_field_kinds() {
		let cases = [
			(
				ModelFormFieldKind::Text {
					min_length: None,
					max_length: Some(20),
					multiline: false,
				},
				json!("text"),
			),
			(
				ModelFormFieldKind::Email {
					min_length: Some(3),
					max_length: Some(50),
				},
				json!("person@example.com"),
			),
			(
				ModelFormFieldKind::Url {
					min_length: Some(8),
					max_length: Some(80),
				},
				json!("https://example.com"),
			),
			(
				ModelFormFieldKind::Integer {
					min: Some(1),
					max: Some(10),
				},
				json!(5),
			),
			(
				ModelFormFieldKind::Float {
					min: None,
					max: None,
				},
				json!(1.5),
			),
			(
				ModelFormFieldKind::Decimal {
					min: None,
					max: None,
				},
				json!("1.25"),
			),
			(ModelFormFieldKind::Boolean, json!(true)),
			(ModelFormFieldKind::Date, json!("2026-07-25")),
			(ModelFormFieldKind::Time, json!("14:30:00")),
			(ModelFormFieldKind::DateTime, json!("2026-07-25 14:30:00")),
			(
				ModelFormFieldKind::Uuid,
				json!("01983c74-08c2-7ad2-a596-6bdbba00be40"),
			),
			(ModelFormFieldKind::Json, json!("{\"valid\":true}")),
		];

		for (kind, value) in cases {
			let descriptor = ModelFormFieldDescriptor {
				name: "value",
				kind,
				required: true,
				has_default: false,
				nullable: false,
				editable: true,
				generated_relation_id: false,
			};
			let field = field_factory::create_form_field(&descriptor);

			assert_eq!(field.name(), "value");
			assert!(field.required());
			assert!(
				field.clean(Some(&value)).is_ok(),
				"descriptor kind {kind:?} must accept its native value"
			);
		}
	}

	#[test]
	fn descriptor_factory_applies_text_length_and_integer_range() {
		let text = field_factory::create_form_field(&ModelFormFieldDescriptor {
			name: "short",
			kind: ModelFormFieldKind::Text {
				min_length: Some(2),
				max_length: Some(3),
				multiline: true,
			},
			required: false,
			has_default: false,
			nullable: false,
			editable: true,
			generated_relation_id: false,
		});
		let integer = field_factory::create_form_field(&ModelFormFieldDescriptor {
			name: "bounded",
			kind: ModelFormFieldKind::Integer {
				min: Some(2),
				max: Some(4),
			},
			required: true,
			has_default: false,
			nullable: false,
			editable: true,
			generated_relation_id: false,
		});

		assert!(!text.required());
		assert!(text.clean(Some(&json!("a"))).is_err());
		assert!(text.clean(Some(&json!("four"))).is_err());
		assert!(integer.clean(Some(&json!(1))).is_err());
		assert!(integer.clean(Some(&json!(5))).is_err());
	}

	#[test]
	fn descriptor_factory_preserves_unsigned_integer_values() {
		let field = field_factory::create_form_field(&ModelFormFieldDescriptor {
			name: "identifier",
			kind: ModelFormFieldKind::Integer {
				min: None,
				max: None,
			},
			required: true,
			has_default: false,
			nullable: false,
			editable: true,
			generated_relation_id: false,
		});
		let value = json!(u64::MAX);

		assert_eq!(field.clean(Some(&value)).unwrap(), value);
	}

	#[test]
	fn descriptor_factory_accepts_structured_json_values() {
		let field = field_factory::create_form_field(&ModelFormFieldDescriptor {
			name: "metadata",
			kind: ModelFormFieldKind::Json,
			required: true,
			has_default: false,
			nullable: false,
			editable: true,
			generated_relation_id: false,
		});
		let value = json!({"nested": [true, {"count": 2}]});

		assert_eq!(field.clean(Some(&value)).unwrap(), value);
	}

	#[test]
	fn descriptor_factory_preserves_exact_decimal_text() {
		let field = field_factory::create_form_field(&ModelFormFieldDescriptor {
			name: "amount",
			kind: ModelFormFieldKind::Decimal {
				min: None,
				max: None,
			},
			required: true,
			has_default: false,
			nullable: false,
			editable: true,
			generated_relation_id: false,
		});
		let value = json!("12345678901234567890.12345678");

		assert_eq!(field.clean(Some(&value)).unwrap(), value);
	}
}
