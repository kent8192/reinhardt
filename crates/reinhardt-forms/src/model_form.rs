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
	AllEditableModelFields, ModelFormCleanedPayload, ModelFormFieldKind, ModelFormPayload,
	ModelFormPayloadError, ModelFormPolicy, ModelFormPrimaryKeyFields, ModelFormSchema,
	ModelFormValidatingPayload,
};
use reinhardt_core::validators::{ValidationError, ValidationErrors};
use reinhardt_db::orm::transaction::AtomicTransactionOutcome;
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
pub trait FormModel: Model + ModelFormPrimaryKeyFields + Clone + Send + Sync {
	/// Generated descriptor schema for this model.
	type Schema: ModelFormSchema<Model = Self>;
	/// Generated typed payload under the active field policy.
	type Data<P: ModelFormPolicy>: ModelFormPayload<P>
		+ ModelFormValidatingPayload<Cleaned = Self::CleanedData<P>>
		+ Clone;
	/// Generated cleaned payload under the active field policy.
	///
	/// **Parity: P0.** This associated type belongs to the native ORM bridge;
	/// generated cleaned payload types remain available on all targets.
	type CleanedData<P: ModelFormPolicy>: ModelFormCleanedPayload<Raw = Self::Data<P>>;

	/// Cleans a payload before applying it to an existing model.
	///
	/// Generated implementations override this compatibility hook to merge
	/// omitted values for synchronous validation. Hand-written implementations
	/// retain strict validation unless they opt in to the update extension.
	///
	/// **Parity: P0.** This update bridge is available only with native ORM
	/// integration.
	#[doc(hidden)]
	fn clean_for_update<P: ModelFormPolicy>(
		data: Self::Data<P>,
		_existing: &Self,
	) -> Result<Self::CleanedData<P>, ValidationErrors> {
		data.clean_and_validate()
	}

	/// Builds a create candidate from cleaned data and explicit server values.
	///
	/// **Parity: P0.** Model construction is available only with native ORM
	/// integration.
	#[doc(hidden)]
	fn build_from_cleaned_compat<P: ModelFormPolicy>(
		data: &Self::CleanedData<P>,
		server_values: &HashMap<String, Value>,
	) -> Result<Self, ModelFormError>;

	/// Applies cleaned payload values to an existing candidate.
	///
	/// **Parity: P0.** Model mutation is available only with native ORM
	/// integration.
	fn apply_cleaned<P: ModelFormPolicy>(
		&mut self,
		data: &Self::CleanedData<P>,
	) -> Result<(), ModelFormError>;

	/// Applies a server-trusted relationship value excluded from public payloads.
	fn set_trusted_field_json(&mut self, field: &str, value: Value) -> Result<(), ModelFormError> {
		let _ = value;
		Err(ModelFormError::FieldValidation {
			errors: HashMap::from([(
				field.to_owned(),
				vec!["unknown trusted model field".to_owned()],
			)]),
		})
	}

	/// Returns the input kind accepted by a server-trusted relationship field.
	fn trusted_relation_field_kind(_field: &str) -> Option<ModelFormFieldKind> {
		None
	}

	/// Returns whether a server-trusted relationship field requires a parent key.
	///
	/// **Parity: P0.** Native inline formsets use this metadata for relationships
	/// excluded from the public payload schema. Unknown and optional fields return false.
	#[doc(hidden)]
	fn trusted_relation_field_is_required(_field: &str) -> bool {
		false
	}

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

/// Cleans a generated model-form payload using its schema and field policy.
///
/// This is the native counterpart of generated payload validation. It preserves
/// omitted and explicit-null values, normalizes only submitted editable fields,
/// and reports policy and field errors in schema order.
///
/// **Parity: P0.** This helper depends on the native forms implementation;
/// generated payload cleaning exposes a separate P2 implementation on WASM.
pub fn clean_generated_payload<S, P, D>(data: &mut D) -> Result<(), ValidationErrors>
where
	S: ModelFormSchema,
	P: ModelFormPolicy,
	D: ModelFormPayload<P>,
{
	clean_generated_payload_with_trusted_values::<S, P, D>(data, None, true, &[])
}

/// Cleans only supplied fields for native generated update validation.
///
/// **Parity: P0.** This helper depends on the native forms implementation;
/// generated payload cleaning exposes a separate P2 implementation on WASM.
#[doc(hidden)]
pub fn clean_generated_partial_payload<S, P, D>(data: &mut D) -> Result<(), ValidationErrors>
where
	S: ModelFormSchema,
	P: ModelFormPolicy,
	D: ModelFormPayload<P>,
{
	clean_generated_payload_with_trusted_values::<S, P, D>(data, None, false, &[])
}

/// Cleans only supplied fields for generated update validation using existing
/// model values as trusted storage references.
#[doc(hidden)]
pub fn clean_generated_partial_payload_with_trusted_values<S, P, D>(
	data: &mut D,
	trusted_values: Option<&Value>,
) -> Result<(), ValidationErrors>
where
	S: ModelFormSchema,
	P: ModelFormPolicy,
	D: ModelFormPayload<P>,
{
	clean_generated_payload_with_trusted_values::<S, P, D>(data, trusted_values, false, &[])
}

/// Cleans a generated model-form snapshot while deferring required file fields
/// to the multipart server-function boundary.
///
/// Rejects deferred names that do not describe required file or image fields.
#[doc(hidden)]
pub fn clean_generated_payload_with_deferred_required_fields<S, P, D>(
	data: &mut D,
	deferred_fields: &[&str],
) -> Result<(), ValidationErrors>
where
	S: ModelFormSchema,
	P: ModelFormPolicy,
	D: ModelFormPayload<P>,
{
	let mut errors = ValidationErrors::new();
	for &field in deferred_fields {
		if !S::fields().iter().any(|descriptor| {
			descriptor.name == field
				&& descriptor.required
				&& matches!(
					descriptor.kind,
					ModelFormFieldKind::File | ModelFormFieldKind::Image
				)
		}) {
			errors.add(
				field.to_owned(),
				ValidationError::Custom(
					"only required file or image fields may be deferred".to_owned(),
				),
			);
		}
	}
	if !errors.is_empty() {
		return Err(errors);
	}
	clean_generated_payload_with_trusted_values::<S, P, D>(data, None, true, deferred_fields)
}

/// Cleans a native generated payload while deferring one required relationship identifier.
///
/// **Parity: P0.** Inline formsets use this helper before a generated parent key
/// is available; ordinary create validation remains strict on every target.
/// Required relationships excluded from the public schema use native model metadata.
/// Unknown fields, scalar fields, and optional relationship identifiers cannot be deferred.
#[doc(hidden)]
pub fn clean_generated_payload_with_deferred_required_field<S, P, D>(
	data: &mut D,
	deferred_field: &str,
) -> Result<(), ValidationErrors>
where
	S: ModelFormSchema,
	S::Model: FormModel,
	P: ModelFormPolicy,
	D: ModelFormPayload<P>,
{
	let descriptor = S::fields()
		.iter()
		.find(|descriptor| descriptor.name == deferred_field);
	let required_relation = match descriptor {
		Some(descriptor) => descriptor.generated_relation_id && descriptor.required,
		None => {
			S::Model::trusted_relation_field_kind(deferred_field).is_some()
				&& S::Model::trusted_relation_field_is_required(deferred_field)
		}
	};
	if !required_relation {
		let mut errors = ValidationErrors::new();
		errors.add(
			deferred_field.to_owned(),
			ValidationError::Custom(
				"only generated relationship identifiers may be deferred".to_owned(),
			),
		);
		return Err(errors);
	}
	clean_generated_payload_with_trusted_values::<S, P, D>(data, None, true, &[deferred_field])
}

fn clean_generated_payload_with_trusted_values<S, P, D>(
	data: &mut D,
	trusted_values: Option<&Value>,
	require_all: bool,
	deferred_required_fields: &[&str],
) -> Result<(), ValidationErrors>
where
	S: ModelFormSchema,
	P: ModelFormPolicy,
	D: ModelFormPayload<P>,
{
	let supplied = data.supplied_fields();
	let mut form = Form::new();
	let mut bound = HashMap::new();
	for descriptor in S::fields() {
		if descriptor.editable
			&& P::allows(descriptor.name)
			&& supplied.contains(&descriptor.name)
			&& !data
				.get_json(descriptor.name)
				.is_some_and(|value| descriptor.nullable && value.is_null())
		{
			form.add_field(field_factory::create_form_field_with_trusted_value(
				descriptor,
				trusted_values.and_then(|values| values.get(descriptor.name)),
			));
			if let Some(value) = data.get_json(descriptor.name) {
				bound.insert(descriptor.name.to_owned(), value);
			}
		}
	}
	form.bind(bound);

	let mut errors = ValidationErrors::new();
	let forbidden_fields = data.forbidden_fields();
	for descriptor in S::fields() {
		if forbidden_fields.contains(&descriptor.name) {
			errors.add(
				descriptor.name,
				ValidationError::Custom("This field is not allowed.".to_owned()),
			);
		}
	}
	if !errors.is_empty() {
		return Err(errors);
	}
	let form_is_valid = form.is_valid();
	for descriptor in S::fields() {
		if require_all
			&& descriptor.editable
			&& P::allows(descriptor.name)
			&& descriptor.required
			&& !supplied.contains(&descriptor.name)
			&& !deferred_required_fields.contains(&descriptor.name)
		{
			errors.add(
				descriptor.name,
				ValidationError::Custom("This field is required.".to_owned()),
			);
		}
		if !form_is_valid && let Some(messages) = form.errors().get(descriptor.name) {
			for message in messages {
				errors.add(descriptor.name, ValidationError::Custom(message.clone()));
			}
		}
	}
	if !errors.is_empty() {
		return Err(errors);
	}
	for field in supplied {
		if let Some(value) = form.cleaned_data().get(field).cloned() {
			data.set_json(field, value).map_err(|error| {
				let mut errors = ValidationErrors::new();
				errors.add(field, ValidationError::Custom(error.to_string()));
				errors
			})?;
		}
	}
	Ok(())
}

fn model_form_error_from_validation_errors(errors: ValidationErrors) -> ModelFormError {
	let errors = errors
		.ordered_field_errors()
		.map(|(field, validation_errors)| {
			let messages = validation_errors
				.iter()
				.map(|error| match error {
					ValidationError::Custom(message) => message.clone(),
					_ => error.to_string(),
				})
				.collect();
			(field.to_owned(), messages)
		})
		.collect();
	ModelFormError::FieldValidation { errors }
}

struct PendingTransactionSave<T> {
	outcome: AtomicTransactionOutcome,
	candidate_before_save: T,
	instance_before_save: Option<T>,
	persistence_mode_before_save: ModelFormPersistenceMode,
}

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
	cleaned_data: Option<T::CleanedData<P>>,
	deferred_required_field: Option<String>,
	validated_candidate: Option<T>,
	trusted_field_values: HashMap<String, Value>,
	persistence_mode: ModelFormPersistenceMode,
	pending_transaction_save: Option<PendingTransactionSave<T>>,
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
		let instance_values = instance
			.as_ref()
			.and_then(|instance| serde_json::to_value(instance).ok());

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
				let trusted_value = instance_values
					.as_ref()
					.and_then(|values| values.get(descriptor.name));
				form.add_field(field_factory::create_form_field_with_trusted_value(
					descriptor,
					trusted_value,
				));
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
			cleaned_data: None,
			deferred_required_field: None,
			validated_candidate: None,
			trusted_field_values: HashMap::new(),
			persistence_mode,
			pending_transaction_save: None,
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
		if self.cleaned_data.is_some() {
			return Ok(());
		}
		if let Some(field) = self.data.forbidden_fields().first() {
			return Err(ModelFormError::ForbiddenInput { field });
		}
		if self.persistence_mode == ModelFormPersistenceMode::Update
			&& let Some(field) = T::primary_key_fields()
				.iter()
				.copied()
				.find(|field| self.supplied_fields.contains(field))
		{
			return Err(ModelFormError::FieldValidation {
				errors: HashMap::from([(
					field.to_owned(),
					vec!["model form primary keys cannot be updated".to_owned()],
				)]),
			});
		}
		let instance_values = self
			.instance
			.as_ref()
			.and_then(|instance| serde_json::to_value(instance).ok());
		clean_generated_payload_with_trusted_values::<T::Schema, P, _>(
			&mut self.data,
			instance_values.as_ref(),
			self.persistence_mode == ModelFormPersistenceMode::Create,
			&[],
		)
		.map_err(model_form_error_from_validation_errors)?;

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

		let cleaned = match self.instance.as_ref() {
			Some(existing) => T::clean_for_update(self.data.clone(), existing),
			None => self.data.clone().clean_and_validate(),
		}
		.map_err(model_form_error_from_validation_errors)?;
		self.cleaned_data = Some(cleaned);
		self.deferred_required_field = None;
		Ok(())
	}

	/// Validates the payload and builds a model candidate without database access.
	pub fn build_instance(&mut self) -> Result<T, ModelFormError> {
		self.form.clear_errors();
		if let Some(candidate) = &self.validated_candidate {
			return Ok(candidate.clone());
		}

		self.clean_payload()?;
		let cleaned = self
			.cleaned_data
			.as_ref()
			.expect("clean_payload caches cleaned data");
		let mut candidate = match &self.instance {
			Some(instance) => instance.clone(),
			None => T::build_from_cleaned_compat(cleaned, &self.trusted_field_values)?,
		};
		candidate.apply_cleaned(cleaned)?;
		for (field, value) in &self.trusted_field_values {
			T::set_trusted_field_json(&mut candidate, field, value.clone())?;
		}

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
			ModelFormError::Persistence { .. }
			| ModelFormError::PersistenceAfterCreate { .. }
			| ModelFormError::TransactionOutcomePending => {}
		}
	}

	/// Persists a validated candidate through the caller-owned executor.
	pub async fn save(&mut self, executor: &mut dyn OrmExecutor) -> Result<T, ModelFormError> {
		self.finalize_transaction_save()?;
		if self.validated_candidate.is_none() {
			self.build_instance()?;
		}

		let candidate = self
			.validated_candidate
			.as_mut()
			.expect("build_instance caches a validated candidate");
		let candidate_before_save = candidate.clone();
		let instance_before_save = self.instance.clone();
		let persistence_mode_before_save = self.persistence_mode;
		let transaction_outcome = executor.transaction_outcome();
		if let Err(error) =
			FormModel::save_with_mode(candidate, executor, self.persistence_mode).await
		{
			if matches!(error, ModelFormError::PersistenceAfterCreate { .. }) {
				if let Some(outcome) = transaction_outcome {
					self.pending_transaction_save = Some(PendingTransactionSave {
						outcome,
						candidate_before_save,
						instance_before_save,
						persistence_mode_before_save,
					});
				} else {
					self.persistence_mode = ModelFormPersistenceMode::Update;
				}
			}
			return Err(error);
		}
		let saved = candidate.clone();
		self.instance = Some(saved.clone());
		if let Some(outcome) = transaction_outcome {
			self.pending_transaction_save = Some(PendingTransactionSave {
				outcome,
				candidate_before_save,
				instance_before_save,
				persistence_mode_before_save,
			});
		} else {
			self.persistence_mode = ModelFormPersistenceMode::Update;
		}
		Ok(saved)
	}

	fn finalize_transaction_save(&mut self) -> Result<(), ModelFormError> {
		let Some(pending) = self.pending_transaction_save.take() else {
			return Ok(());
		};
		if pending.outcome.is_committed() {
			self.persistence_mode = ModelFormPersistenceMode::Update;
			return Ok(());
		}
		if pending.outcome.is_rolled_back() {
			self.instance = pending.instance_before_save;
			self.validated_candidate = Some(pending.candidate_before_save);
			self.persistence_mode = pending.persistence_mode_before_save;
			return Ok(());
		}
		self.pending_transaction_save = Some(pending);
		Err(ModelFormError::TransactionOutcomePending)
	}

	pub(crate) fn finalize_transaction(&mut self) -> Result<(), ModelFormError> {
		self.finalize_transaction_save()
	}

	/// Replaces one payload field, primarily for inline foreign-key assignment.
	pub fn set_field_value(
		&mut self,
		field_name: &str,
		value: Value,
	) -> Result<(), ModelFormError> {
		self.finalize_transaction_save()?;
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
		let mut bound_values = self.form.bound_data().clone();
		if self
			.form
			.fields()
			.iter()
			.all(|field| field.name() != field_name)
		{
			let trusted_value = self
				.instance
				.as_ref()
				.and_then(|instance| serde_json::to_value(instance).ok())
				.and_then(|values| values.get(field_name).cloned());
			self.form
				.add_field(field_factory::create_form_field_with_trusted_value(
					descriptor,
					trusted_value.as_ref(),
				));
		}
		bound_values.insert(field_name.to_owned(), form_value);
		self.form.bind(bound_values);
		self.cleaned_data = None;
		self.deferred_required_field = None;
		self.validated_candidate = None;
		if !self.supplied_fields.contains(&field_name) {
			self.supplied_fields.push(field_name);
		}
		Ok(())
	}

	pub(crate) fn set_trusted_field_value(
		&mut self,
		field_name: &str,
		value: Value,
	) -> Result<(), ModelFormError> {
		self.finalize_transaction_save()?;
		if T::Schema::fields()
			.iter()
			.find(|descriptor| descriptor.name == field_name)
			.is_some_and(|descriptor| descriptor.editable)
		{
			if self.validated_candidate.is_some()
				&& self.data.get_json(field_name).as_ref() == Some(&value)
			{
				return Ok(());
			}
			return self.set_field_value(field_name, value);
		}
		if self.validated_candidate.is_some()
			&& self.trusted_field_values.get(field_name) == Some(&value)
		{
			return Ok(());
		}
		self.trusted_field_values
			.insert(field_name.to_owned(), value);
		self.cleaned_data = None;
		self.deferred_required_field = None;
		self.validated_candidate = None;
		Ok(())
	}

	pub(crate) fn set_deferred_trusted_field_value(
		&mut self,
		field_name: &str,
		value: Value,
	) -> Result<(), ModelFormError> {
		self.finalize_transaction_save()?;
		let descriptor = T::Schema::fields()
			.iter()
			.find(|descriptor| descriptor.name == field_name)
			.copied();
		let trusted_relation = descriptor
			.is_some_and(|descriptor| descriptor.generated_relation_id && descriptor.required)
			|| (descriptor.is_none()
				&& T::trusted_relation_field_kind(field_name).is_some()
				&& T::trusted_relation_field_is_required(field_name));
		if !trusted_relation {
			return Err(ModelFormError::FieldValidation {
				errors: HashMap::from([(
					field_name.to_owned(),
					vec!["only generated relationship identifiers may be deferred".to_owned()],
				)]),
			});
		}
		if self.cleaned_data.is_none()
			|| self.deferred_required_field.as_deref() != Some(field_name)
		{
			return Err(ModelFormError::FieldValidation {
				errors: HashMap::from([(
					field_name.to_owned(),
					vec!["deferred relationship field was not validated".to_owned()],
				)]),
			});
		}
		self.trusted_field_values
			.insert(field_name.to_owned(), value);
		self.deferred_required_field = None;
		self.validated_candidate = None;
		Ok(())
	}

	pub(crate) fn has_deferred_required_field(&self, field_name: &str) -> bool {
		self.cleaned_data.is_some() && self.deferred_required_field.as_deref() == Some(field_name)
	}

	/// Returns a reference to the underlying form.
	pub fn form(&self) -> &Form {
		&self.form
	}
	/// Returns a mutable reference to the underlying form.
	pub fn form_mut(&mut self) -> &mut Form {
		self.cleaned_data = None;
		self.deferred_required_field = None;
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

	/// Performs structural validation before an inline formset assigns a generated parent key.
	///
	/// Model-level validation intentionally runs only after the real key is installed, so
	/// validators may safely depend on that relationship.
	pub(crate) fn is_valid_with_deferred_required_field(&mut self, deferred_field: &str) -> bool {
		if self.cleaned_data.is_some()
			&& self.deferred_required_field.as_deref() == Some(deferred_field)
		{
			return true;
		}
		self.cleaned_data = None;
		self.deferred_required_field = None;
		self.validated_candidate = None;
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
		let mut data = self.data.clone();
		for field in &self.supplied_fields {
			let Some(value) = self.form.cleaned_data().get(*field).cloned() else {
				continue;
			};
			if let Err(error) = data.set_json(field, value) {
				let message = error.to_string();
				let error = match error {
					ModelFormPayloadError::ForbiddenField { .. } => {
						ModelFormError::ForbiddenInput { field }
					}
					ModelFormPayloadError::UnknownField { .. }
					| ModelFormPayloadError::InvalidValue { .. } => ModelFormError::FieldValidation {
						errors: HashMap::from([((*field).to_owned(), vec![message])]),
					},
				};
				self.record_validation_error(&error);
				return false;
			}
		}
		let cleaned = match data.clean_and_validate_with_deferred_required_field(deferred_field) {
			Ok(cleaned) => cleaned,
			Err(errors) => {
				let error = model_form_error_from_validation_errors(errors);
				self.record_validation_error(&error);
				return false;
			}
		};
		self.cleaned_data = Some(cleaned);
		self.deferred_required_field = T::Schema::fields()
			.iter()
			.find(|descriptor| descriptor.name == deferred_field)
			.filter(|descriptor| descriptor.generated_relation_id && descriptor.required)
			.map(|descriptor| descriptor.name.to_owned())
			.or_else(|| {
				(T::trusted_relation_field_kind(deferred_field).is_some()
					&& T::trusted_relation_field_is_required(deferred_field))
				.then(|| deferred_field.to_owned())
			});
		self.validated_candidate = None;
		true
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
		ModelFormFieldDescriptor, ModelFormFieldKind, ModelFormPolicy, ModelFormUpdatingPayload,
		ModelFormValidatingPayload,
	};
	use reinhardt_db::orm::connection::{
		DatabaseBackend, OrmExecutor, QueryResult, QueryValue, Row,
	};
	use reinhardt_macros::model;
	use rstest::rstest;
	use serde::{Deserialize, Serialize};
	use serde_json::json;
	use serial_test::serial;

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
		#[form(trim)]
		title: String,
		owner_id: i64,
		#[field(default = true)]
		published: bool,
	}

	#[model(
		app_label = "forms",
		table_name = "model_form_assigned_key_documents",
		form = true,
		info = false
	)]
	#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
	struct AssignedKeyDocument {
		#[field(primary_key = true, editable = true, max_length = 64)]
		id: String,
		#[field(max_length = 200)]
		title: String,
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
		table_name = "model_form_composite_primary_key_records",
		form = true,
		info = false
	)]
	#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
	struct CompositePrimaryKeyRecord {
		#[field(primary_key = true, auto_increment = false)]
		account_id: i64,
		#[field(primary_key = true, auto_increment = false)]
		sequence: i64,
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
		table_name = "model_form_multiple_hidden_required_records",
		form = true,
		info = false
	)]
	#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
	struct MultipleHiddenRequiredRecord {
		#[field(primary_key = true)]
		id: Option<i64>,
		#[field(max_length = 200)]
		title: String,
		#[field(editable = false)]
		organization_id: i64,
		#[field(max_length = 200, editable = false)]
		audit_actor: String,
	}

	#[model(
		app_label = "forms",
		table_name = "model_form_hidden_relation_owners",
		form = true,
		info = false
	)]
	#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
	struct HiddenRelationOwner {
		#[field(primary_key = true)]
		id: i64,
		#[field(max_length = 200)]
		name: String,
	}

	#[model(
		app_label = "forms",
		table_name = "model_form_hidden_relation_records",
		form = true,
		info = false
	)]
	#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
	struct HiddenRequiredRelationRecord {
		#[field(primary_key = true)]
		id: Option<i64>,
		#[field(max_length = 200)]
		title: String,
		#[field(editable = false)]
		#[rel(foreign_key)]
		owner: reinhardt_db::associations::ForeignKeyField<HiddenRelationOwner>,
	}

	#[model(
		app_label = "forms",
		table_name = "model_form_excluded_relation_records",
		form = true,
		info = false
	)]
	#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
	struct ExcludedRequiredRelationRecord {
		#[field(primary_key = true)]
		id: Option<i64>,
		#[field(max_length = 200)]
		title: String,
		#[field(include_in_new = false)]
		#[rel(foreign_key)]
		owner: reinhardt_db::associations::ForeignKeyField<HiddenRelationOwner>,
	}

	#[model(
		app_label = "forms",
		table_name = "model_form_optional_relation_records",
		form = true,
		info = false
	)]
	#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
	struct OptionalRelationRecord {
		#[field(primary_key = true)]
		id: Option<i64>,
		#[field(max_length = 200)]
		title: String,
		#[rel(foreign_key, null = true)]
		public_owner: reinhardt_db::associations::ForeignKeyField<HiddenRelationOwner>,
		#[field(editable = false)]
		#[rel(foreign_key, null = true)]
		hidden_owner: reinhardt_db::associations::ForeignKeyField<HiddenRelationOwner>,
	}

	#[model(
		app_label = "forms",
		table_name = "model_form_snapshot_upload_records",
		form = true,
		info = false
	)]
	#[derive(Debug, Clone, Deserialize, Serialize)]
	struct SnapshotUploadRecord {
		#[field(primary_key = true)]
		id: Option<i64>,
		#[field(max_length = 200)]
		#[form(trim)]
		title: String,
		#[field(upload_to = "documents", max_length = 255)]
		document: reinhardt_db::orm::FileField,
		#[field(upload_to = "images", max_length = 255)]
		avatar: reinhardt_db::orm::ImageField,
		#[field(upload_to = "documents", max_length = 255)]
		optional_document: Option<reinhardt_db::orm::FileField>,
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

	#[model(
		app_label = "forms",
		table_name = "model_form_cleaning_records",
		form = true,
		info = false
	)]
	#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
	struct CleaningRecord {
		#[field(primary_key = true)]
		id: Option<i64>,
		#[field(min_length = 3, max_length = 5)]
		#[form(trim)]
		name: String,
		#[field(url = true, max_length = 200)]
		#[form(trim)]
		website: String,
	}

	#[model(
		app_label = "forms",
		table_name = "model_form_validation_matrix_records",
		form = true,
		info = false
	)]
	#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
	#[form(validate = validate_validation_matrix_record)]
	struct ValidationMatrixRecord {
		#[field(primary_key = true)]
		id: Option<i64>,
		#[field(min_length = 3, max_length = 20)]
		#[form(trim)]
		title: String,
		#[field(email = true, max_length = 200)]
		#[form(trim)]
		email: String,
		#[field(url = true, max_length = 200)]
		#[form(trim)]
		api_url: String,
		#[field(min_value = 1, max_value = 10)]
		quantity: i64,
		#[field(min_value = 1, max_value = 10)]
		ratio: f64,
		#[field(min_value = 1, max_value = 10)]
		amount: rust_decimal::Decimal,
		#[field(max_length = 40, blank = true)]
		nullable_note: Option<Option<String>>,
		nullable_flag: Option<bool>,
		config: serde_json::Value,
		published: bool,
		event_date: chrono::NaiveDate,
		event_time: chrono::NaiveTime,
		aware_at: DateTime<Utc>,
		naive_at: chrono::NaiveDateTime,
		token: uuid::Uuid,
		#[field(upload_to = "documents", max_length = 255)]
		document: Option<reinhardt_db::orm::FileField>,
		#[field(upload_to = "images", max_length = 255)]
		avatar: Option<reinhardt_db::orm::ImageField>,
	}

	static VALIDATION_MATRIX_CALLS: AtomicUsize = AtomicUsize::new(0);
	const PARITY_NUMERIC_ERRORS: &[(&str, &str)] = &[
		(
			"quantity",
			"Ensure this value is greater than or equal to 1",
		),
		("ratio", "Ensure this value is less than or equal to 10"),
		("amount", "Ensure this value is greater than or equal to 1"),
	];
	const PARITY_EMAIL_ERRORS: &[(&str, &str)] = &[("email", "Enter a valid email address")];
	const PARITY_URL_ERRORS: &[(&str, &str)] = &[("api_url", "Enter a valid URL")];
	const PARITY_JSON_DEPTH_ERRORS: &[(&str, &str)] =
		&[("config", "JSON structure is too deeply nested.")];
	const PARITY_DATE_ERRORS: &[(&str, &str)] =
		&[("event_date", "Enter a valid date with a 4-digit year")];
	const PARITY_DATETIME_ERRORS: &[(&str, &str)] =
		&[("aware_at", "Enter a year between 1000 and 9999")];
	const PARITY_FILE_ERRORS: &[(&str, &str)] = &[(
		"document",
		"Stored file references must come from the existing instance",
	)];
	const PARITY_IMAGE_ERRORS: &[(&str, &str)] = &[(
		"avatar",
		"Stored file references must come from the existing instance",
	)];
	const PARITY_FORBIDDEN_ERRORS: &[(&str, &str)] = &[("email", "This field is not allowed.")];
	const PARITY_CROSS_FIELD_ERRORS: &[(&str, &str)] =
		&[("title", "Blocked title"), ("_all", "Blocked project")];

	fn validate_validation_matrix_record<P: ModelFormPolicy>(
		payload: &CleanedValidationMatrixRecordModelFormData<P>,
	) -> Result<(), ValidationErrors> {
		VALIDATION_MATRIX_CALLS.fetch_add(1, Ordering::SeqCst);
		let mut errors = ValidationErrors::new();
		if payload.title().is_some_and(|title| title == "blocked") {
			errors.add(
				"_all",
				ValidationError::Custom("Blocked project".to_owned()),
			);
			errors.add("title", ValidationError::Custom("Blocked title".to_owned()));
		}
		if errors.is_empty() {
			Ok(())
		} else {
			Err(errors)
		}
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

	struct ExplicitNullTitlePayload;

	impl ModelFormPayload<QuestionPolicy> for ExplicitNullTitlePayload {
		fn supplied_fields(&self) -> Vec<&'static str> {
			vec!["title"]
		}

		fn forbidden_fields(&self) -> &[&'static str] {
			&[]
		}

		fn get_json(&self, field: &str) -> Option<Value> {
			(field == "title").then_some(Value::Null)
		}

		fn set_json(&mut self, field: &str, _value: Value) -> Result<(), ModelFormPayloadError> {
			Err(ModelFormPayloadError::UnknownField {
				field: field.to_owned(),
			})
		}
	}

	struct ReverseForbiddenPayload;

	impl ModelFormPayload<TitleOnly> for ReverseForbiddenPayload {
		fn supplied_fields(&self) -> Vec<&'static str> {
			vec![]
		}

		fn forbidden_fields(&self) -> &[&'static str] {
			&["published", "owner_id"]
		}

		fn get_json(&self, _field: &str) -> Option<Value> {
			None
		}

		fn set_json(&mut self, field: &str, _value: Value) -> Result<(), ModelFormPayloadError> {
			Err(ModelFormPayloadError::UnknownField {
				field: field.to_owned(),
			})
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

	fn ordered_validation_errors(errors: &ValidationErrors) -> Vec<(String, Vec<ValidationError>)> {
		errors
			.ordered_field_errors()
			.map(|(field, errors)| (field.to_owned(), errors.to_vec()))
			.collect()
	}

	#[rstest]
	fn generated_payload_cleaner_normalizes_and_reports_field_errors() {
		let mut data = question_payload("  cleaned title  ", 7);
		clean_generated_payload::<QuestionFormSchema, QuestionPolicy, _>(&mut data)
			.expect("trimmed payload should be valid");
		assert_eq!(data.title(), Some(&"cleaned title".to_owned()));

		let mut required_after_trim = question_payload("   ", 7);
		let required_errors = clean_generated_payload::<QuestionFormSchema, QuestionPolicy, _>(
			&mut required_after_trim,
		)
		.expect_err("required text must be checked after trimming");
		assert_eq!(
			ordered_validation_errors(&required_errors),
			vec![(
				"title".to_owned(),
				vec![ValidationError::Custom(
					"This field is required.".to_owned()
				)],
			)]
		);

		let mut too_short = CleaningRecordModelFormData::<AllEditableModelFields>::empty();
		too_short
			.set_name("  ab  ".to_owned())
			.expect("permitted name should be accepted");
		too_short
			.set_website("https://example.com".to_owned())
			.expect("permitted website should be accepted");
		let minimum_errors =
			clean_generated_payload::<CleaningRecordFormSchema, AllEditableModelFields, _>(
				&mut too_short,
			)
			.expect_err("minimum length must use the normalized text");
		assert_eq!(
			ordered_validation_errors(&minimum_errors),
			vec![(
				"name".to_owned(),
				vec![ValidationError::Custom(
					"Ensure this value has at least 3 characters (it has 2)".to_owned(),
				)],
			)]
		);

		let mut too_long = CleaningRecordModelFormData::<AllEditableModelFields>::empty();
		too_long
			.set_name("  too-long  ".to_owned())
			.expect("permitted name should be accepted");
		too_long
			.set_website("https://example.com".to_owned())
			.expect("permitted website should be accepted");
		let maximum_errors =
			clean_generated_payload::<CleaningRecordFormSchema, AllEditableModelFields, _>(
				&mut too_long,
			)
			.expect_err("maximum length must use the normalized text");
		assert_eq!(
			ordered_validation_errors(&maximum_errors),
			vec![(
				"name".to_owned(),
				vec![ValidationError::Custom(
					"Ensure this value has at most 5 characters (it has 8)".to_owned(),
				)],
			)]
		);

		let mut invalid_url = CleaningRecordModelFormData::<AllEditableModelFields>::empty();
		invalid_url
			.set_website("  not a URL  ".to_owned())
			.expect("permitted URL should be accepted");
		invalid_url
			.set_name("valid".to_owned())
			.expect("permitted name should be accepted");
		let url_errors =
			clean_generated_payload::<CleaningRecordFormSchema, AllEditableModelFields, _>(
				&mut invalid_url,
			)
			.expect_err("URL validation must run after trimming");
		assert_eq!(
			ordered_validation_errors(&url_errors),
			vec![(
				"website".to_owned(),
				vec![ValidationError::Custom("Enter a valid URL".to_owned())],
			)]
		);

		let mut forbidden: QuestionModelFormData<TitleOnly> = serde_json::from_value(json!({
			"title": "Question",
			"owner_id": 7,
		}))
		.expect("known forbidden fields are recorded on the generated payload");
		let forbidden_errors =
			clean_generated_payload::<QuestionFormSchema, TitleOnly, _>(&mut forbidden)
				.expect_err("forbidden fields must take precedence over field cleaning");
		assert_eq!(
			forbidden_errors.field_errors().get("owner_id").unwrap(),
			&vec![ValidationError::Custom(
				"This field is not allowed.".to_owned()
			)]
		);
	}

	#[rstest]
	fn generated_payload_cleaner_rejects_nonnullable_null_and_preserves_nullable_clear() {
		let mut nonnullable = ExplicitNullTitlePayload;
		let nonnullable_errors =
			clean_generated_payload::<QuestionFormSchema, QuestionPolicy, _>(&mut nonnullable)
				.expect_err("explicit null must not bypass required non-null field validation");
		assert_eq!(
			ordered_validation_errors(&nonnullable_errors),
			vec![
				(
					"title".to_owned(),
					vec![ValidationError::Custom(
						"This field is required.".to_owned()
					)],
				),
				(
					"owner_id".to_owned(),
					vec![ValidationError::Custom(
						"This field is required.".to_owned()
					)],
				)
			]
		);

		let mut nullable = TemporalRecordModelFormData::<AllEditableModelFields>::empty();
		let timestamp = NaiveDate::from_ymd_opt(2026, 9, 1)
			.unwrap()
			.and_hms_opt(12, 30, 0)
			.unwrap();
		nullable
			.set_aware_at(timestamp.and_utc())
			.expect("required aware datetime should be accepted");
		nullable
			.set_naive_at(timestamp)
			.expect("required naive datetime should be accepted");
		nullable
			.set_json("nullable_naive_at", Value::Null)
			.expect("nullable payload should accept an explicit clear");
		clean_generated_payload::<TemporalRecordFormSchema, AllEditableModelFields, _>(
			&mut nullable,
		)
		.expect("nullable explicit null should remain a clear operation");
		assert_eq!(nullable.get_json("nullable_naive_at"), Some(Value::Null));
	}

	#[rstest]
	#[serial(model_form_validation_matrix)]
	fn generated_validating_payload_matches_the_target_neutral_validation_matrix() {
		fn valid_payload() -> ValidationMatrixRecordModelFormData<AllEditableModelFields> {
			let mut payload =
				ValidationMatrixRecordModelFormData::<AllEditableModelFields>::empty();
			payload.set_title("  trimmed  ".to_owned()).unwrap();
			payload
				.set_email("  person@example.com  ".to_owned())
				.unwrap();
			payload
				.set_api_url("  https://example.com/path?query=value  ".to_owned())
				.unwrap();
			payload.set_quantity(5).unwrap();
			payload.set_ratio(5.5).unwrap();
			payload
				.set_amount(rust_decimal::Decimal::new(55, 1))
				.unwrap();
			payload.set_config(json!({"nested": [true]})).unwrap();
			payload.set_published(false).unwrap();
			payload
				.set_event_date(NaiveDate::from_ymd_opt(2026, 9, 1).unwrap())
				.unwrap();
			payload
				.set_event_time(chrono::NaiveTime::from_hms_opt(12, 30, 0).unwrap())
				.unwrap();
			payload
				.set_aware_at(
					NaiveDate::from_ymd_opt(2026, 9, 1)
						.unwrap()
						.and_hms_opt(12, 30, 0)
						.unwrap()
						.and_utc(),
				)
				.unwrap();
			payload
				.set_naive_at(
					NaiveDate::from_ymd_opt(2026, 9, 1)
						.unwrap()
						.and_hms_opt(12, 30, 0)
						.unwrap(),
				)
				.unwrap();
			payload.set_token(uuid::Uuid::nil()).unwrap();
			payload
		}

		fn expected_errors(expected: &[(&str, &str)]) -> Vec<(String, String)> {
			expected
				.iter()
				.map(|(field, message)| ((*field).to_owned(), (*message).to_owned()))
				.collect()
		}

		fn error_tuples<P: ModelFormPolicy>(
			payload: ValidationMatrixRecordModelFormData<P>,
			existing: &ValidationMatrixRecord,
		) -> Vec<(String, String)> {
			match payload.clean_and_validate_for_update(existing) {
				Ok(_) => panic!("payload should fail validation"),
				Err(errors) => errors
					.ordered_field_errors()
					.flat_map(|(field, errors)| {
						errors.iter().map(move |error| {
							let message = match error {
								ValidationError::Custom(message) => message.clone(),
								_ => error.to_string(),
							};
							(field.to_owned(), message)
						})
					})
					.collect(),
			}
		}

		let existing = valid_payload()
			.clean_and_validate()
			.unwrap()
			.into_model()
			.unwrap();
		VALIDATION_MATRIX_CALLS.store(0, Ordering::SeqCst);

		let cleaned = valid_payload().clean_and_validate().unwrap();
		assert_eq!(cleaned.title(), Some(&"trimmed".to_owned()));
		assert_eq!(cleaned.email(), Some(&"person@example.com".to_owned()));
		assert_eq!(
			cleaned.api_url(),
			Some(&"https://example.com/path?query=value".to_owned())
		);
		assert_eq!(cleaned.quantity(), Some(&5));
		assert_eq!(cleaned.ratio(), Some(&5.5));
		assert_eq!(cleaned.amount(), Some(&rust_decimal::Decimal::new(55, 1)));
		assert_eq!(cleaned.nullable_note(), None);
		assert_eq!(cleaned.nullable_flag(), None);
		assert_eq!(cleaned.config(), Some(&json!({"nested": [true]})));
		assert_eq!(cleaned.published(), Some(&false));
		assert_eq!(
			cleaned.event_date(),
			Some(&NaiveDate::from_ymd_opt(2026, 9, 1).unwrap())
		);
		assert_eq!(
			cleaned.event_time(),
			Some(&chrono::NaiveTime::from_hms_opt(12, 30, 0).unwrap())
		);
		assert_eq!(
			cleaned.aware_at().map(|value| value.to_rfc3339()),
			Some("2026-09-01T12:30:00+00:00".to_owned())
		);
		assert_eq!(
			cleaned.naive_at().map(|value| value.to_string()),
			Some("2026-09-01 12:30:00".to_owned())
		);
		assert_eq!(cleaned.token(), Some(&uuid::Uuid::nil()));
		assert_eq!(VALIDATION_MATRIX_CALLS.load(Ordering::SeqCst), 1);
		assert_eq!(
			ValidationMatrixRecordFormSchema::fields()
				.iter()
				.find(|descriptor| descriptor.name == "amount")
				.map(|descriptor| descriptor.kind),
			Some(ModelFormFieldKind::Decimal {
				min: Some("1"),
				max: Some("10"),
			})
		);

		let mut explicit_null =
			ValidationMatrixRecordModelFormData::<AllEditableModelFields>::empty();
		explicit_null
			.set_json("nullable_note", Value::Null)
			.unwrap();
		let cleaned = explicit_null
			.clean_and_validate_for_update(&existing)
			.unwrap();
		assert_eq!(cleaned.nullable_note(), Some(&None));

		let mut nullable_bool =
			ValidationMatrixRecordModelFormData::<AllEditableModelFields>::empty();
		nullable_bool
			.set_json("nullable_flag", Value::Null)
			.unwrap();
		let cleaned = nullable_bool
			.clean_and_validate_for_update(&existing)
			.unwrap();
		assert_eq!(cleaned.nullable_flag(), Some(&None));

		let mut json_null = ValidationMatrixRecordModelFormData::<AllEditableModelFields>::empty();
		json_null.set_config(Value::Null).unwrap();
		assert_eq!(
			json_null
				.clean_and_validate_for_update(&existing)
				.unwrap()
				.config(),
			Some(&Value::Null)
		);

		let mut numeric = valid_payload();
		numeric.set_quantity(0).unwrap();
		numeric.set_ratio(11.0).unwrap();
		numeric.set_amount(rust_decimal::Decimal::ZERO).unwrap();
		assert_eq!(
			error_tuples(numeric, &existing),
			expected_errors(PARITY_NUMERIC_ERRORS)
		);
		assert_eq!(VALIDATION_MATRIX_CALLS.load(Ordering::SeqCst), 4);

		let mut email = ValidationMatrixRecordModelFormData::<AllEditableModelFields>::empty();
		email.set_email("person@localhost".to_owned()).unwrap();
		assert_eq!(
			error_tuples(email, &existing),
			expected_errors(PARITY_EMAIL_ERRORS)
		);

		let mut url = ValidationMatrixRecordModelFormData::<AllEditableModelFields>::empty();
		url.set_api_url("https://example.com:123456/".to_owned())
			.unwrap();
		assert_eq!(
			error_tuples(url, &existing),
			expected_errors(PARITY_URL_ERRORS)
		);

		let mut deep = Value::Null;
		for _ in 0..66 {
			deep = Value::Array(vec![deep]);
		}
		let mut json_depth = ValidationMatrixRecordModelFormData::<AllEditableModelFields>::empty();
		json_depth.set_config(deep).unwrap();
		assert_eq!(
			error_tuples(json_depth, &existing),
			expected_errors(PARITY_JSON_DEPTH_ERRORS)
		);

		let mut date = ValidationMatrixRecordModelFormData::<AllEditableModelFields>::empty();
		date.set_event_date(NaiveDate::from_ymd_opt(25, 1, 15).unwrap())
			.unwrap();
		assert_eq!(
			error_tuples(date, &existing),
			expected_errors(PARITY_DATE_ERRORS)
		);

		let mut year = ValidationMatrixRecordModelFormData::<AllEditableModelFields>::empty();
		year.set_aware_at(
			NaiveDate::from_ymd_opt(25, 1, 15)
				.unwrap()
				.and_hms_opt(14, 30, 0)
				.unwrap()
				.and_utc(),
		)
		.unwrap();
		assert_eq!(
			error_tuples(year, &existing),
			expected_errors(PARITY_DATETIME_ERRORS)
		);

		let mut document = ValidationMatrixRecordModelFormData::<AllEditableModelFields>::empty();
		document
			.set_document(Some(
				reinhardt_db::orm::FileField::from_existing("documents/report.pdf", "default")
					.unwrap(),
			))
			.unwrap();
		assert_eq!(
			error_tuples(document, &existing),
			expected_errors(PARITY_FILE_ERRORS)
		);

		let mut avatar = ValidationMatrixRecordModelFormData::<AllEditableModelFields>::empty();
		avatar
			.set_avatar(Some(
				reinhardt_db::orm::ImageField::from_existing("images/avatar.png", "default")
					.unwrap(),
			))
			.unwrap();
		assert_eq!(
			error_tuples(avatar, &existing),
			expected_errors(PARITY_IMAGE_ERRORS)
		);

		let calls_before_forbidden = VALIDATION_MATRIX_CALLS.load(Ordering::SeqCst);
		let forbidden: ValidationMatrixRecordModelFormData<TitleOnly> =
			serde_json::from_value(json!({
				"title": "blocked",
				"email": "person@example.com",
			}))
			.unwrap();
		assert_eq!(
			error_tuples(forbidden, &existing),
			expected_errors(PARITY_FORBIDDEN_ERRORS)
		);
		assert_eq!(
			VALIDATION_MATRIX_CALLS.load(Ordering::SeqCst),
			calls_before_forbidden
		);

		let mut blocked = ValidationMatrixRecordModelFormData::<AllEditableModelFields>::empty();
		blocked.set_title("  blocked  ".to_owned()).unwrap();
		assert_eq!(
			error_tuples(blocked, &existing),
			expected_errors(PARITY_CROSS_FIELD_ERRORS)
		);

		let calls_before_field_error = VALIDATION_MATRIX_CALLS.load(Ordering::SeqCst);
		let mut field_error =
			ValidationMatrixRecordModelFormData::<AllEditableModelFields>::empty();
		field_error.set_title("blocked".to_owned()).unwrap();
		field_error.set_quantity(0).unwrap();
		assert_eq!(
			error_tuples(field_error, &existing),
			expected_errors(&[(
				"quantity",
				"Ensure this value is greater than or equal to 1",
			)])
		);
		assert_eq!(
			VALIDATION_MATRIX_CALLS.load(Ordering::SeqCst),
			calls_before_field_error
		);
	}

	#[rstest]
	fn generated_payload_cleaner_reports_forbidden_fields_in_schema_order() {
		let mut payload = ReverseForbiddenPayload;
		let errors = clean_generated_payload::<QuestionFormSchema, TitleOnly, _>(&mut payload)
			.expect_err("forbidden payload fields must be rejected");

		assert_eq!(
			errors
				.ordered_field_errors()
				.map(|(field, _)| field)
				.collect::<Vec<_>>(),
			["owner_id", "published"]
		);
	}

	#[rstest]
	fn deferred_cleaning_rejects_non_relation_required_fields() {
		let mut data = QuestionModelFormData::<QuestionPolicy>::empty();
		data.set_title("Question".to_owned())
			.expect("question title should be accepted");
		let mut form = ModelForm::<Question, QuestionPolicy>::from_payload(data);

		let valid = form.is_valid_with_deferred_required_field("owner_id");

		assert_eq!(valid, false);
		assert_eq!(
			form.form().errors(),
			&HashMap::from([(
				"owner_id".to_owned(),
				vec!["only generated relationship identifiers may be deferred".to_owned()],
			)])
		);
	}

	#[rstest]
	#[case::scalar(&["document", "avatar", "title"], &["title"])]
	#[case::optional_file(&["document", "avatar", "optional_document"], &["optional_document"])]
	#[case::unknown(&["document", "avatar", "unknown"], &["unknown"])]
	#[case::multiple_invalid(&["title", "unknown"], &["title", "unknown"])]
	fn generated_snapshot_deferral_rejects_non_required_uploads(
		#[case] deferred_fields: &[&str],
		#[case] invalid_fields: &[&str],
	) {
		// Arrange
		let data = SnapshotUploadRecordModelFormData::<AllEditableModelFields>::empty();

		// Act
		let errors = data
			.clean_and_validate_with_deferred_required_fields(deferred_fields)
			.err()
			.expect("only required upload fields may be deferred");

		// Assert
		assert_eq!(
			ordered_validation_errors(&errors),
			invalid_fields
				.iter()
				.map(|field| (
					(*field).to_owned(),
					vec![ValidationError::Custom(
						"only required file or image fields may be deferred".to_owned(),
					)],
				))
				.collect::<Vec<_>>()
		);
	}

	#[rstest]
	fn generated_snapshot_deferral_accepts_required_uploads_only() {
		// Arrange
		let mut data = SnapshotUploadRecordModelFormData::<AllEditableModelFields>::empty();
		data.set_title("  Upload  ".to_owned()).unwrap();

		// Act
		let strict_errors = data.clone().clean_and_validate().err().unwrap();
		let cleaned = data
			.clean_and_validate_with_deferred_required_fields(&["document", "avatar"])
			.expect("required uploads may be deferred to the multipart boundary");

		// Assert
		assert_eq!(
			ordered_validation_errors(&strict_errors),
			["document", "avatar"]
				.into_iter()
				.map(|field| (
					field.to_owned(),
					vec![ValidationError::Custom(
						"This field is required.".to_owned()
					)],
				))
				.collect::<Vec<_>>()
		);
		assert_eq!(cleaned.title().map(String::as_str), Some("Upload"));
		assert_eq!(cleaned.document(), None);
		assert_eq!(cleaned.avatar(), None);
	}

	#[rstest]
	#[case::public_optional("public_owner_id")]
	#[case::hidden_optional("hidden_owner_id")]
	#[case::unknown("unknown")]
	fn deferred_cleaning_rejects_optional_and_unknown_relations(#[case] field: &str) {
		// Arrange
		let mut data = OptionalRelationRecordModelFormData::<AllEditableModelFields>::empty();
		data.set_title("Optional relation".to_owned()).unwrap();

		// Act
		let errors = data
			.clean_and_validate_with_deferred_required_field(field)
			.err()
			.expect("optional and unknown relationships cannot be deferred");

		// Assert
		assert_eq!(
			ordered_validation_errors(&errors),
			vec![(
				field.to_owned(),
				vec![ValidationError::Custom(
					"only generated relationship identifiers may be deferred".to_owned(),
				)],
			)]
		);
	}

	#[rstest]
	fn deferred_cleaning_accepts_required_relation_excluded_from_new() {
		// Arrange
		let mut data =
			ExcludedRequiredRelationRecordModelFormData::<AllEditableModelFields>::empty();
		data.set_title("Excluded relation".to_owned()).unwrap();
		let mut form = ModelForm::<ExcludedRequiredRelationRecord>::from_payload(data);

		// Act
		let valid = form.is_valid_with_deferred_required_field("owner_id");
		form.set_deferred_trusted_field_value("owner_id", json!(42))
			.unwrap();
		let built = form.build_instance().unwrap();

		// Assert
		assert_eq!(valid, true);
		assert_eq!(built.owner_id, 42);
		assert_eq!(built.title, "Excluded relation");
	}

	#[rstest]
	fn inline_formset_saves_hidden_required_relation_after_parent_auto_id() {
		// Arrange
		let mut data = HiddenRequiredRelationRecordModelFormData::<AllEditableModelFields>::empty();
		data.set_title("Hidden relation".to_owned()).unwrap();
		let mut formset = crate::formsets::InlineFormSet::<
			HiddenRelationOwner,
			HiddenRequiredRelationRecord,
		>::for_create(
			HiddenRelationOwner {
				id: 0,
				name: "Parent".to_owned(),
			},
			"owner_id".to_owned(),
		);
		formset.add_child_form(ModelForm::from_payload(data));
		let mut owner_row = Row::new();
		owner_row.insert("id".to_owned(), QueryValue::Int(42));
		owner_row.insert("name".to_owned(), QueryValue::String("Parent".to_owned()));
		let mut child_row = Row::new();
		child_row.insert("id".to_owned(), QueryValue::Int(7));
		child_row.insert("owner_id".to_owned(), QueryValue::Int(42));
		child_row.insert(
			"title".to_owned(),
			QueryValue::String("Hidden relation".to_owned()),
		);
		let mut executor = RetryExecutor::new([Ok(owner_row), Ok(child_row)]);

		// Act
		tokio_test::block_on(formset.save(&mut executor))
			.expect("a hidden required relation must accept the generated parent key");

		// Assert
		assert_eq!(formset.parent().id, 42);
		assert_eq!(formset.parent().name, "Parent");
		let child = formset.child_forms()[0].instance().unwrap();
		assert_eq!(child.id, Some(7));
		assert_eq!(child.owner_id, 42);
		assert_eq!(child.title, "Hidden relation");
		assert_eq!(executor.fetch_one_calls, 2);
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

	#[rstest]
	fn cleaned_payload_requires_complete_server_context_for_direct_create() {
		let mut data = HiddenRequiredRecordModelFormData::<AllEditableModelFields>::empty();
		data.set_title("Created directly".to_owned())
			.expect("editable title should be accepted");
		let cleaned = data
			.clean_and_validate()
			.expect("payload should clean before construction");
		let context =
			HiddenRequiredRecordModelFormServerContext::new().audit_actor("system".to_owned());

		let built = cleaned
			.into_model(context)
			.expect("complete server context should construct the model");

		assert_eq!(built.title, "Created directly");
		assert_eq!(built.audit_actor, "system");
	}

	#[rstest]
	fn cleaned_payload_server_context_accepts_required_hidden_relation_key() {
		let mut data = HiddenRequiredRelationRecordModelFormData::<AllEditableModelFields>::empty();
		data.set_title("Related directly".to_owned())
			.expect("editable title should be accepted");
		let cleaned = data
			.clean_and_validate()
			.expect("payload should clean before construction");
		let context = HiddenRequiredRelationRecordModelFormServerContext::new().owner_id(42);

		let built = cleaned
			.into_model(context)
			.expect("complete relationship context should construct the model");

		assert_eq!(built.owner_id, 42);
	}

	#[rstest]
	fn cleaned_payload_server_context_tracks_multiple_fields_in_any_setter_order() {
		let mut data = MultipleHiddenRequiredRecordModelFormData::<AllEditableModelFields>::empty();
		data.set_title("Multiple server values".to_owned())
			.expect("editable title should be accepted");
		let cleaned = data
			.clean_and_validate()
			.expect("payload should clean before construction");
		let context = MultipleHiddenRequiredRecordModelFormServerContext::new()
			.audit_actor("system".to_owned())
			.organization_id(42);

		let built = cleaned
			.into_model(context)
			.expect("all server context fields should construct the model");

		assert_eq!(built.organization_id, 42);
		assert_eq!(built.audit_actor, "system");
	}

	#[rstest]
	fn cleaned_payload_update_preserves_server_owned_fields() {
		let mut data = HiddenRequiredRecordModelFormData::<AllEditableModelFields>::empty();
		data.set_title("Updated directly".to_owned())
			.expect("editable title should be accepted");
		let cleaned = data
			.clean_and_validate()
			.expect("payload should clean before update");
		let existing = HiddenRequiredRecord {
			id: Some(9),
			title: "Original".to_owned(),
			audit_actor: "original actor".to_owned(),
		};

		let updated = cleaned
			.apply_to(existing)
			.expect("cleaned payload should apply to an existing model");

		assert_eq!(updated.id, Some(9));
		assert_eq!(updated.title, "Updated directly");
		assert_eq!(updated.audit_actor, "original actor");
	}

	#[rstest]
	fn cleaned_payload_without_server_fields_constructs_directly() {
		let cleaned = question_payload("Direct", 17)
			.clean_and_validate()
			.expect("payload should clean before construction");

		let built = cleaned
			.into_model()
			.expect("models without server context should construct directly");

		assert_eq!(built.title, "Direct");
		assert_eq!(built.owner_id, 17);
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
	fn generated_payload_rejects_supplied_assigned_primary_keys_on_update() {
		let mut data = AssignedKeyDocumentModelFormData::<AllEditableModelFields>::empty();
		data.set_id("attacker-key".to_owned())
			.expect("assigned primary key should be editable");
		let existing = AssignedKeyDocument {
			id: "existing-key".to_owned(),
			title: "existing".to_owned(),
		};

		let errors = match data.clean_and_validate_for_update(&existing) {
			Ok(_) => panic!("direct generated updates must reject supplied primary keys"),
			Err(errors) => errors,
		};

		assert!(matches!(
			errors.ordered_field_errors().next(),
			Some(("id", [ValidationError::Custom(message)]))
				if message == "model form primary keys cannot be updated"
		));
	}

	#[test]
	fn generated_model_form_rejects_every_composite_primary_key_field_on_update() {
		let mut data = CompositePrimaryKeyRecordModelFormData::<AllEditableModelFields>::empty();
		data.set_sequence(2)
			.expect("composite primary-key field should be represented in the payload");
		let instance = CompositePrimaryKeyRecord {
			account_id: 1,
			sequence: 1,
			title: "Original".to_owned(),
		};
		let mut form =
			ModelForm::<CompositePrimaryKeyRecord>::from_payload_and_instance(data, instance);

		let error = form
			.build_instance()
			.expect_err("updates must reject later composite primary-key fields");

		assert!(matches!(
			error,
			ModelFormError::FieldValidation { errors }
				if errors.get("sequence")
					== Some(&vec!["model form primary keys cannot be updated".to_owned()])
		));
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
		data.set_title("Missing owner".to_owned()).unwrap();

		let mut form = ModelForm::<Question, QuestionPolicy>::from_payload(data);
		let error = form.build_instance().unwrap_err();

		assert_eq!(
			error,
			ModelFormError::FieldValidation {
				errors: HashMap::from([(
					"owner_id".to_owned(),
					vec!["This field is required.".to_owned()],
				)]),
			}
		);
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

	#[rstest]
	fn trusted_non_editable_field_rebuilds_a_cleaned_candidate() {
		let validator_calls = Arc::new(AtomicUsize::new(0));
		let validator_calls_for_candidate = Arc::clone(&validator_calls);
		let mut data = HiddenRequiredRecordModelFormData::<AllEditableModelFields>::empty();
		data.set_title("Trusted relation".to_owned());
		let mut form =
			ModelForm::<HiddenRequiredRecord>::from_payload(data).with_model_validator(move |_| {
				validator_calls_for_candidate.fetch_add(1, Ordering::SeqCst);
				Ok(())
			});

		form.set_trusted_field_value("audit_actor", json!("system"))
			.expect("a trusted non-editable field should satisfy model construction");
		let built = form
			.build_instance()
			.expect("the trusted value should be retained in the candidate");

		assert_eq!(built.audit_actor, "system");

		form.set_trusted_field_value("audit_actor", json!("system"))
			.expect("an unchanged trusted value should retain the candidate");
		assert_eq!(form.build_instance().unwrap().audit_actor, "system");
		assert_eq!(validator_calls.load(Ordering::SeqCst), 1);

		form.set_trusted_field_value("audit_actor", json!("replacement"))
			.expect("trusted mutation should invalidate cached construction");
		assert_eq!(
			form.build_instance()
				.expect("replacement trusted value should rebuild the candidate")
				.audit_actor,
			"replacement"
		);
		assert_eq!(validator_calls.load(Ordering::SeqCst), 2);
	}

	#[test]
	fn generated_model_form_handles_required_non_editable_foreign_key() {
		let mut data = HiddenRequiredRelationRecordModelFormData::<AllEditableModelFields>::empty();
		data.set_title("Hidden relation".to_owned())
			.expect("editable title should be accepted");

		let mut form = ModelForm::<HiddenRequiredRelationRecord>::from_payload(data);
		let error = form
			.build_instance()
			.expect_err("a missing hidden foreign key must not build a normal candidate");
		assert!(matches!(
			error,
			ModelFormError::MissingModelField { field: "owner_id" }
		));

		let mut data = HiddenRequiredRelationRecordModelFormData::<AllEditableModelFields>::empty();
		data.set_title("Trusted relation".to_owned())
			.expect("editable title should be accepted");
		let mut form = ModelForm::<HiddenRequiredRelationRecord>::from_payload(data);
		form.set_trusted_field_value("owner_id", json!(42))
			.expect("a trusted hidden foreign key should be accepted");

		let built = form
			.build_instance()
			.expect("the trusted deferred path should build a candidate");
		assert_eq!(built.owner_id, 42);
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
	fn generated_model_form_save_runs_model_validation_before_persistence() {
		let data = question_payload("Rejected", 5);
		let mut form = ModelForm::<Question, QuestionPolicy>::from_payload(data)
			.with_model_validator(|_| Err(vec!["model validation failed".to_owned()]));
		let mut executor = RetryExecutor::new(Vec::<Result<Row, Error>>::new());

		let error = tokio_test::block_on(form.save(&mut executor))
			.expect_err("save must not persist a candidate rejected by model validation");

		assert!(matches!(error, ModelFormError::ModelValidation { .. }));
		assert_eq!(executor.fetch_one_calls, 0);
	}

	#[rstest]
	#[case::is_valid(false)]
	#[case::build_instance(true)]
	fn revalidation_replaces_errors_without_recleaning_payload(#[case] build_directly: bool) {
		// Arrange
		let model_calls = Arc::new(AtomicUsize::new(0));
		let model_calls_for_validator = Arc::clone(&model_calls);
		let cleaner_calls = Arc::new(AtomicUsize::new(0));
		let cleaner_calls_for_field = Arc::clone(&cleaner_calls);
		let mut form =
			ModelForm::<Question, QuestionPolicy>::from_payload(question_payload("Retryable", 7))
				.with_model_validator(move |_| {
					let call = model_calls_for_validator.fetch_add(1, Ordering::SeqCst);
					if call < 2 {
						Err(vec![format!("temporary rejection {}", call + 1)])
					} else {
						Ok(())
					}
				});
		form.form_mut()
			.add_field_clean_function("title", move |value| {
				cleaner_calls_for_field.fetch_add(1, Ordering::SeqCst);
				Ok(json!(format!("{}!", value.as_str().unwrap())))
			});

		// Act and Assert
		for message in ["temporary rejection 1", "temporary rejection 2"] {
			assert_eq!(form.is_valid(), false);
			assert_eq!(
				form.form().errors(),
				&HashMap::from([(ALL_FIELDS_KEY.to_owned(), vec![message.to_owned()])])
			);
		}
		let valid = if build_directly {
			form.build_instance().is_ok()
		} else {
			form.is_valid()
		};
		assert_eq!(valid, true);
		assert_eq!(form.form().errors(), &HashMap::new());
		assert_eq!(form.build_instance().unwrap().title, "Retryable!");
		assert_eq!(model_calls.load(Ordering::SeqCst), 3);
		assert_eq!(cleaner_calls.load(Ordering::SeqCst), 1);
	}

	#[test]
	fn replacement_value_overrides_the_bound_form_value() {
		let data = question_payload("Replacement", 7);
		let mut form = ModelForm::<Question, QuestionPolicy>::from_payload(data);
		assert_eq!(form.build_instance().unwrap().owner_id, 7);

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
	fn generated_model_form_keeps_non_idempotently_cleaned_candidate_after_uncertain_create() {
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
		assert!(matches!(
			first_error,
			ModelFormError::PersistenceAfterCreate { .. }
		));
		assert_eq!(
			first_error.database_error().map(DatabaseError::kind),
			Some(DatabaseErrorKind::Timeout)
		);
		assert!(form.instance().is_none());
		assert_eq!(form.build_instance().unwrap(), built);
		assert_eq!(cleaner_calls.load(Ordering::SeqCst), 1);
		assert_eq!(executor.fetch_one_calls, 1);
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
	fn generated_uuid_model_form_reuses_dynamic_default_for_update_after_uncertain_insert() {
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
		assert!(matches!(
			first_error,
			ModelFormError::PersistenceAfterCreate { .. }
		));
		assert_eq!(
			first_error.database_error().map(DatabaseError::kind),
			Some(DatabaseErrorKind::Timeout)
		);
		assert_eq!(form.build_instance().unwrap().id, generated_id);

		let saved = tokio_test::block_on(form.save(&mut executor)).unwrap();

		assert_eq!(saved.id, generated_id);
		assert_eq!(executor.fetch_one_calls, 2);
		assert!(executor.queries[0].trim_start().starts_with("INSERT"));
		assert!(executor.queries[1].trim_start().starts_with("UPDATE"));
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
				trim: false,
			};
			let field = field_factory::create_form_field(&descriptor);

			assert_eq!(field.name(), "value");
			if matches!(kind, ModelFormFieldKind::Boolean) {
				assert!(!field.required());
			} else {
				assert!(field.required());
			}
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
			trim: false,
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
			trim: false,
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
			trim: false,
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
			trim: false,
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
			trim: false,
		});
		let value = json!("12345678901234567890.12345678");

		assert_eq!(field.clean(Some(&value)).unwrap(), value);
	}
}
