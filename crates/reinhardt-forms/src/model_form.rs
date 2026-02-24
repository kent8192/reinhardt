//! ModelForm implementation for ORM integration
//!
//! ModelForms automatically generate forms from ORM models, handling field
//! inference, validation, and saving.

use crate::{CharField, EmailField, FloatField, Form, FormError, FormField, IntegerField, Widget};
use serde_json::Value;
use std::collections::HashMap;
use std::marker::PhantomData;

/// Field type metadata for ModelForm field inference
#[derive(Debug, Clone)]
pub enum FieldType {
	Char { max_length: Option<usize> },
	Text,
	Integer,
	Float,
	Boolean,
	DateTime,
	Date,
	Time,
	Email,
	Url,
	Json,
}

/// Trait for models that can be used with ModelForm
///
/// This trait is specifically for form models. For ORM models, use `reinhardt_db::orm::Model`.
pub trait FormModel: Send + Sync {
	/// Get the model's field names
	fn field_names() -> Vec<String>;

	/// Get field type metadata for form field inference
	///
	/// # Examples
	///
	/// ```no_run
	/// # use reinhardt_forms::model_form::FieldType;
	/// fn field_type(name: &str) -> Option<FieldType> {
	///     match name {
	///         "name" => Some(FieldType::Char { max_length: Some(100) }),
	///         "email" => Some(FieldType::Email),
	///         "age" => Some(FieldType::Integer),
	///         _ => None,
	///     }
	/// }
	/// ```
	fn field_type(_name: &str) -> Option<FieldType> {
		None
	}

	/// Get a field value by name
	fn get_field(&self, name: &str) -> Option<Value>;

	/// Set a field value by name
	fn set_field(&mut self, name: &str, value: Value) -> Result<(), String>;

	/// Save the model to the database
	fn save(&mut self) -> Result<(), String>;

	/// Validate the model
	fn validate(&self) -> Result<(), Vec<String>> {
		Ok(())
	}

	/// Convert model instance to a choice label for display in forms
	///
	/// Default implementation returns the string representation of the primary key.
	/// Override this method to provide custom display labels.
	///
	/// # Examples
	///
	/// ```no_run
	/// # struct Example { id: i32, name: String }
	/// # impl Example {
	/// fn to_choice_label(&self) -> String {
	///     format!("{} - {}", self.id, self.name)
	/// }
	/// # }
	/// ```
	fn to_choice_label(&self) -> String {
		// Default: use the "id" field or empty string
		self.get_field("id")
			.and_then(|v| v.as_i64().map(|i| i.to_string()))
			.or_else(|| {
				self.get_field("id")
					.and_then(|v| v.as_str().map(|s| s.to_string()))
			})
			.unwrap_or_default()
	}

	/// Get the primary key value as a string for form field validation
	///
	/// Default implementation uses the "id" field.
	///
	/// # Examples
	///
	/// ```no_run
	/// # struct Example { id: i32 }
	/// # impl Example {
	/// fn to_choice_value(&self) -> String {
	///     self.id.to_string()
	/// }
	/// # }
	/// ```
	fn to_choice_value(&self) -> String {
		self.get_field("id")
			.and_then(|v| v.as_i64().map(|i| i.to_string()))
			.or_else(|| {
				self.get_field("id")
					.and_then(|v| v.as_str().map(|s| s.to_string()))
			})
			.unwrap_or_default()
	}
}

/// ModelForm configuration
#[derive(Debug, Clone, Default)]
pub struct ModelFormConfig {
	/// Fields to include in the form (None = all fields)
	pub fields: Option<Vec<String>>,
	/// Fields to exclude from the form
	pub exclude: Vec<String>,
	/// Custom widgets for specific fields
	pub widgets: HashMap<String, crate::Widget>,
	/// Custom labels for specific fields
	pub labels: HashMap<String, String>,
	/// Custom help text for specific fields
	pub help_texts: HashMap<String, String>,
}

impl ModelFormConfig {
	pub fn new() -> Self {
		Self::default()
	}
	pub fn fields(mut self, fields: Vec<String>) -> Self {
		self.fields = Some(fields);
		self
	}
	pub fn exclude(mut self, exclude: Vec<String>) -> Self {
		self.exclude = exclude;
		self
	}
	pub fn widget(mut self, field: String, widget: crate::Widget) -> Self {
		self.widgets.insert(field, widget);
		self
	}
	pub fn label(mut self, field: String, label: String) -> Self {
		self.labels.insert(field, label);
		self
	}
	pub fn help_text(mut self, field: String, text: String) -> Self {
		self.help_texts.insert(field, text);
		self
	}
}

/// A form that is automatically generated from a Model
pub struct ModelForm<T: FormModel> {
	form: Form,
	instance: Option<T>,
	#[allow(dead_code)]
	config: ModelFormConfig,
	_phantom: PhantomData<T>,
}

impl<T: FormModel> ModelForm<T> {
	/// Create a form field from field type metadata
	fn create_form_field(
		name: &str,
		field_type: FieldType,
		config: &ModelFormConfig,
	) -> Box<dyn FormField> {
		let label = config.labels.get(name).cloned();
		let help_text = config.help_texts.get(name).cloned();
		let widget = config.widgets.get(name).cloned();

		match field_type {
			FieldType::Char { max_length } => {
				let mut field = CharField::new(name.to_string());
				if let Some(label) = label {
					field.label = Some(label);
				}
				if let Some(help) = help_text {
					field.help_text = Some(help);
				}
				if let Some(w) = widget {
					field.widget = w;
				}
				field.max_length = max_length;
				Box::new(field)
			}
			FieldType::Text => {
				let mut field = CharField::new(name.to_string());
				if let Some(label) = label {
					field.label = Some(label);
				}
				if let Some(help) = help_text {
					field.help_text = Some(help);
				}
				if let Some(w) = widget {
					field.widget = w;
				} else {
					field.widget = Widget::TextArea;
				}
				Box::new(field)
			}
			FieldType::Email => {
				let mut field = EmailField::new(name.to_string());
				if let Some(label) = label {
					field.label = Some(label);
				}
				if let Some(help) = help_text {
					field.help_text = Some(help);
				}
				if let Some(w) = widget {
					field.widget = w;
				}
				Box::new(field)
			}
			FieldType::Integer => {
				let mut field = IntegerField::new(name.to_string());
				if let Some(label) = label {
					field.label = Some(label);
				}
				if let Some(help) = help_text {
					field.help_text = Some(help);
				}
				if let Some(w) = widget {
					field.widget = w;
				}
				Box::new(field)
			}
			FieldType::Float => {
				let mut field = FloatField::new(name.to_string());
				if let Some(label) = label {
					field.label = Some(label);
				}
				if let Some(help) = help_text {
					field.help_text = Some(help);
				}
				if let Some(w) = widget {
					field.widget = w;
				}
				Box::new(field)
			}
			// For unsupported types, default to CharField
			_ => {
				let mut field = CharField::new(name.to_string());
				if let Some(label) = label {
					field.label = Some(label);
				}
				if let Some(help) = help_text {
					field.help_text = Some(help);
				}
				if let Some(w) = widget {
					field.widget = w;
				}
				Box::new(field)
			}
		}
	}

	/// Create a new ModelForm from a model instance
	///
	/// # Examples
	///
	/// ```ignore
	/// use reinhardt_forms::{ModelForm, ModelFormConfig};
	///
	// Assuming we have a model that implements the Model trait
	/// let config = ModelFormConfig::new();
	/// let form = ModelForm::new(Some(instance), config);
	/// ```
	pub fn new(instance: Option<T>, config: ModelFormConfig) -> Self {
		let mut form = Form::new();

		// Get field names from model
		let all_fields = T::field_names();

		// Filter fields based on config
		let fields_to_include: Vec<String> = if let Some(ref include) = config.fields {
			include
				.iter()
				.filter(|f| !config.exclude.contains(f))
				.cloned()
				.collect()
		} else {
			all_fields
				.iter()
				.filter(|f| !config.exclude.contains(f))
				.cloned()
				.collect()
		};

		// Infer field types from model metadata and add to form
		for field_name in &fields_to_include {
			if let Some(field_type) = T::field_type(field_name) {
				let form_field = Self::create_form_field(field_name, field_type, &config);
				form.add_field(form_field);
			}
		}

		// If instance exists, populate initial data from the instance
		if let Some(ref inst) = instance {
			let mut initial = HashMap::new();
			for field_name in &fields_to_include {
				if let Some(value) = inst.get_field(field_name) {
					initial.insert(field_name.clone(), value);
				}
			}
			form.bind(initial);
		}

		Self {
			form,
			instance,
			config,
			_phantom: PhantomData,
		}
	}
	/// Create a new ModelForm without an instance (for creation)
	///
	/// # Examples
	///
	/// ```ignore
	/// use reinhardt_forms::{ModelForm, ModelFormConfig};
	///
	/// let config = ModelFormConfig::new();
	/// let form = ModelForm::<MyModel>::empty(config);
	/// ```
	pub fn empty(config: ModelFormConfig) -> Self {
		Self::new(None, config)
	}
	/// Bind data to the form
	///
	/// # Examples
	///
	/// ```ignore
	/// use reinhardt_forms::{ModelForm, ModelFormConfig};
	/// use std::collections::HashMap;
	/// use serde_json::json;
	///
	/// let config = ModelFormConfig::new();
	/// let mut form = ModelForm::<MyModel>::empty(config);
	/// let mut data = HashMap::new();
	/// data.insert("field".to_string(), json!("value"));
	/// form.bind(data);
	/// ```
	pub fn bind(&mut self, data: HashMap<String, Value>) -> &mut Self {
		// Bind data to the underlying form
		self.form.bind(data);
		self
	}
	/// Check if the form is valid
	///
	/// # Examples
	///
	/// ```ignore
	/// use reinhardt_forms::{ModelForm, ModelFormConfig};
	///
	/// let config = ModelFormConfig::new();
	/// let mut form = ModelForm::<MyModel>::empty(config);
	/// let is_valid = form.is_valid();
	/// ```
	pub fn is_valid(&mut self) -> bool {
		// Validate the model if instance exists
		if let Some(ref instance) = self.instance
			&& let Err(_errors) = instance.validate()
		{
			return false;
		}

		true
	}
	/// Save the form data to the model instance
	///
	/// Returns `FormError::NoInstance` if no model instance is available.
	///
	/// # Examples
	///
	/// ```ignore
	/// use reinhardt_forms::{ModelForm, ModelFormConfig};
	///
	/// let config = ModelFormConfig::new();
	/// let mut form = ModelForm::<MyModel>::empty(config);
	/// // Returns Err(FormError::NoInstance) without an instance
	/// assert!(form.save().is_err());
	/// ```
	pub fn save(&mut self) -> Result<T, FormError> {
		if !self.is_valid() {
			return Err(FormError::Validation("Form is not valid".to_string()));
		}

		// Get existing instance or return error
		let mut instance = self.instance.take().ok_or(FormError::NoInstance)?;

		// Set field values from form's cleaned_data
		let cleaned_data = self.form.cleaned_data();
		for (field_name, value) in cleaned_data.iter() {
			if let Err(e) = instance.set_field(field_name, value.clone()) {
				return Err(FormError::Validation(format!(
					"Failed to set field {}: {}",
					field_name, e
				)));
			}
		}

		// Save the instance
		if let Err(e) = instance.save() {
			return Err(FormError::Validation(format!("Failed to save: {}", e)));
		}

		Ok(instance)
	}
	/// Set a field value directly on the model instance.
	///
	/// This is used by `InlineFormSet` to set foreign key values on child
	/// instances before saving.
	///
	/// If no instance exists, this method is a no-op.
	pub fn set_field_value(&mut self, field_name: &str, value: Value) {
		if let Some(ref mut instance) = self.instance {
			// Silently ignore errors from set_field, as the field may not exist
			// on all model types (defensive approach for inline formsets)
			let _ = instance.set_field(field_name, value);
		}
	}

	pub fn form(&self) -> &Form {
		&self.form
	}
	pub fn form_mut(&mut self) -> &mut Form {
		&mut self.form
	}
	pub fn instance(&self) -> Option<&T> {
		self.instance.as_ref()
	}
}

/// Builder for creating ModelForm instances
pub struct ModelFormBuilder<T: FormModel> {
	config: ModelFormConfig,
	_phantom: PhantomData<T>,
}

impl<T: FormModel> ModelFormBuilder<T> {
	pub fn new() -> Self {
		Self {
			config: ModelFormConfig::default(),
			_phantom: PhantomData,
		}
	}
	pub fn fields(mut self, fields: Vec<String>) -> Self {
		self.config.fields = Some(fields);
		self
	}
	pub fn exclude(mut self, exclude: Vec<String>) -> Self {
		self.config.exclude = exclude;
		self
	}
	pub fn widget(mut self, field: String, widget: crate::Widget) -> Self {
		self.config.widgets.insert(field, widget);
		self
	}
	pub fn label(mut self, field: String, label: String) -> Self {
		self.config.labels.insert(field, label);
		self
	}
	pub fn help_text(mut self, field: String, text: String) -> Self {
		self.config.help_texts.insert(field, text);
		self
	}
	/// Build the ModelForm with the configured settings
	///
	/// # Examples
	///
	/// ```ignore
	/// use reinhardt_forms::{ModelFormBuilder, ModelFormConfig};
	///
	/// let config = ModelFormConfig::new();
	/// let builder = ModelFormBuilder::<MyModel>::new();
	/// let form = builder.build(None);
	/// ```
	pub fn build(self, instance: Option<T>) -> ModelForm<T> {
		ModelForm::new(instance, self.config)
	}
}

impl<T: FormModel> Default for ModelFormBuilder<T> {
	fn default() -> Self {
		Self::new()
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	// Mock model for testing
	#[derive(Debug)]
	struct TestModel {
		id: i32,
		name: String,
		email: String,
	}

	impl FormModel for TestModel {
		fn field_names() -> Vec<String> {
			vec!["id".to_string(), "name".to_string(), "email".to_string()]
		}

		fn field_type(name: &str) -> Option<FieldType> {
			match name {
				"id" => Some(FieldType::Integer),
				"name" => Some(FieldType::Char {
					max_length: Some(100),
				}),
				"email" => Some(FieldType::Email),
				_ => None,
			}
		}

		fn get_field(&self, name: &str) -> Option<Value> {
			match name {
				"id" => Some(Value::Number(self.id.into())),
				"name" => Some(Value::String(self.name.clone())),
				"email" => Some(Value::String(self.email.clone())),
				_ => None,
			}
		}

		fn set_field(&mut self, name: &str, value: Value) -> Result<(), String> {
			match name {
				"id" => {
					if let Value::Number(n) = value {
						self.id = n.as_i64().unwrap() as i32;
						Ok(())
					} else {
						Err("Invalid type for id".to_string())
					}
				}
				"name" => {
					if let Value::String(s) = value {
						self.name = s;
						Ok(())
					} else {
						Err("Invalid type for name".to_string())
					}
				}
				"email" => {
					if let Value::String(s) = value {
						self.email = s;
						Ok(())
					} else {
						Err("Invalid type for email".to_string())
					}
				}
				_ => Err(format!("Unknown field: {}", name)),
			}
		}

		fn save(&mut self) -> Result<(), String> {
			// Mock save
			Ok(())
		}
	}

	#[rstest]
	fn test_model_form_config() {
		// Arrange
		let config = ModelFormConfig::new()
			.fields(vec!["name".to_string(), "email".to_string()])
			.exclude(vec!["id".to_string()]);

		// Assert
		assert_eq!(
			config.fields,
			Some(vec!["name".to_string(), "email".to_string()])
		);
		assert_eq!(config.exclude, vec!["id".to_string()]);
	}

	#[rstest]
	fn test_model_form_builder() {
		// Arrange
		let instance = TestModel {
			id: 1,
			name: "John".to_string(),
			email: "john@example.com".to_string(),
		};

		// Act
		let form = ModelFormBuilder::<TestModel>::new()
			.fields(vec!["name".to_string(), "email".to_string()])
			.build(Some(instance));

		// Assert
		assert!(form.instance().is_some());
	}

	#[rstest]
	fn test_model_field_names() {
		// Act
		let fields = TestModel::field_names();

		// Assert
		assert_eq!(
			fields,
			vec!["id".to_string(), "name".to_string(), "email".to_string()]
		);
	}

	#[rstest]
	fn test_save_without_instance_returns_no_instance_error() {
		// Arrange
		let config = ModelFormConfig::new();
		let mut form = ModelForm::<TestModel>::empty(config);

		// Act
		let result = form.save();

		// Assert
		assert!(result.is_err());
		let err = result.unwrap_err();
		assert!(
			matches!(err, FormError::NoInstance),
			"Expected FormError::NoInstance, got: {err}"
		);
	}
}
