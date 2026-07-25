//! ModelForm implementation for ORM integration
//!
//! ModelForms automatically generate forms from ORM models, handling field
//! inference, validation, and saving.

mod error;
mod field_factory;

pub use error::ModelFormError;

use crate::Form;
use reinhardt_core::model_form::{
	AllEditableModelFields, ModelFormPayload, ModelFormPayloadError, ModelFormPolicy,
	ModelFormSchema,
};
use reinhardt_db::orm::{Model, OrmExecutor};
use serde_json::Value;
use std::collections::HashMap;
use std::marker::PhantomData;

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

	/// Applies supplied payload values to an existing candidate.
	fn apply_payload<P: ModelFormPolicy>(
		&mut self,
		data: &Self::Data<P>,
	) -> Result<(), ModelFormError>;

	/// Persists this candidate using the caller-owned ORM executor.
	async fn save(&mut self, executor: &mut dyn OrmExecutor) -> Result<(), ModelFormError>;

	/// Convert model instance to a choice label for display in forms
	///
	/// Default implementation returns the string representation of the primary key.
	/// Override this method to provide custom display labels.
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
	model_validator: Option<Box<ModelValidator<T>>>,
	_policy: PhantomData<P>,
}

impl<T, P> ModelForm<T, P>
where
	T: FormModel,
	P: ModelFormPolicy,
{
	fn initialize(data: T::Data<P>, instance: Option<T>) -> Self {
		let supplied_fields = data.supplied_fields();
		let mut form = Form::new();
		let mut form_data = HashMap::new();

		for descriptor in T::Schema::fields() {
			if descriptor.editable
				&& P::allows(descriptor.name)
				&& supplied_fields.contains(&descriptor.name)
			{
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
			model_validator: None,
			_policy: PhantomData,
		}
	}

	/// Creates a model form for a new instance.
	pub fn from_payload(data: T::Data<P>) -> Self {
		Self::initialize(data, None)
	}

	/// Creates a model form that applies a payload to an existing instance.
	pub fn from_payload_and_instance(data: T::Data<P>, instance: T) -> Self {
		Self::initialize(data, Some(instance))
	}

	/// Installs a model-level validator that runs after cleaned values are applied.
	pub fn with_model_validator(
		mut self,
		validator: impl Fn(&T) -> Result<(), Vec<String>> + Send + Sync + 'static,
	) -> Self {
		self.model_validator = Some(Box::new(validator));
		self
	}
	fn clean_payload(&mut self) -> Result<(), ModelFormError> {
		if let Some(field) = self.data.forbidden_fields().first() {
			return Err(ModelFormError::ForbiddenInput { field });
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
		self.clean_payload()?;
		let mut candidate = match &self.instance {
			Some(instance) => instance.clone(),
			None => T::build_from_payload(&self.data)?,
		};
		candidate.apply_payload(&self.data)?;

		if let Some(validator) = &self.model_validator {
			validator(&candidate).map_err(|errors| ModelFormError::ModelValidation { errors })?;
		}

		Ok(candidate)
	}

	/// Returns whether the current payload can produce a valid model candidate.
	pub fn is_valid(&mut self) -> bool {
		self.build_instance().is_ok()
	}

	/// Persists a validated candidate through the caller-owned executor.
	pub async fn save(&mut self, executor: &mut dyn OrmExecutor) -> Result<T, ModelFormError> {
		let mut candidate = self.build_instance()?;
		FormModel::save(&mut candidate, executor).await?;
		self.instance = Some(candidate.clone());
		Ok(candidate)
	}

	/// Replaces one payload field, primarily for inline foreign-key assignment.
	pub fn set_field_value(
		&mut self,
		field_name: &str,
		value: Value,
	) -> Result<(), ModelFormError> {
		let Some(field_name) = T::Schema::fields()
			.iter()
			.find(|descriptor| descriptor.name == field_name)
			.map(|descriptor| descriptor.name)
		else {
			return Err(ModelFormError::FieldValidation {
				errors: HashMap::from([(
					field_name.to_owned(),
					vec!["unknown model form field".to_owned()],
				)]),
			});
		};
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
		&mut self.form
	}
	/// Returns a reference to the model instance, if one exists.
	pub fn instance(&self) -> Option<&T> {
		self.instance.as_ref()
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use std::collections::VecDeque;

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
	}

	impl RetryExecutor {
		fn new(rows: impl IntoIterator<Item = Result<Row, Error>>) -> Self {
			Self {
				rows: rows.into_iter().collect(),
				fetch_one_calls: 0,
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

		async fn fetch_one(&mut self, _sql: &str, _params: Vec<QueryValue>) -> Result<Row, Error> {
			self.fetch_one_calls += 1;
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
	fn generated_model_form_retries_after_persistence_failure() {
		let data = question_payload("Retryable", 17);
		let mut executor = RetryExecutor::new([
			Err(Error::database_with_source(
				DatabaseErrorKind::Timeout,
				"temporary database timeout",
				std::io::Error::new(std::io::ErrorKind::TimedOut, "driver timeout"),
			)),
			Ok(question_row(23, "Retryable", 17, true)),
		]);
		let mut form = ModelForm::<Question, QuestionPolicy>::from_payload(data);

		let first_error = tokio_test::block_on(form.save(&mut executor)).unwrap_err();
		assert_eq!(
			first_error.database_error().map(DatabaseError::kind),
			Some(DatabaseErrorKind::Timeout)
		);
		assert!(form.instance().is_none());

		let saved = tokio_test::block_on(form.save(&mut executor)).unwrap();
		assert_eq!(saved.id, Some(23));
		assert_eq!(form.instance(), Some(&saved));
		assert_eq!(executor.fetch_one_calls, 2);
	}

	#[test]
	fn descriptor_factory_maps_all_supported_field_kinds() {
		let cases = [
			(
				ModelFormFieldKind::Text {
					max_length: Some(20),
					multiline: false,
				},
				json!("text"),
			),
			(
				ModelFormFieldKind::Email {
					max_length: Some(50),
				},
				json!("person@example.com"),
			),
			(
				ModelFormFieldKind::Url {
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
			(ModelFormFieldKind::Float, json!(1.5)),
			(ModelFormFieldKind::Decimal, json!("1.25")),
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
				max_length: Some(3),
				multiline: true,
			},
			required: false,
			has_default: false,
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
			editable: true,
			generated_relation_id: false,
		});

		assert!(!text.required());
		assert!(text.clean(Some(&json!("four"))).is_err());
		assert!(integer.clean(Some(&json!(1))).is_err());
		assert!(integer.clean(Some(&json!(5))).is_err());
	}
}
