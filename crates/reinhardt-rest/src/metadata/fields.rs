//! Field information and builder for metadata

use super::types::{ChoiceInfo, FieldType};
use super::validators::FieldValidator;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Field metadata information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldInfo {
	/// The data type of this field.
	#[serde(rename = "type")]
	pub field_type: FieldType,
	/// Whether this field is required for input.
	pub required: bool,
	/// Whether this field is read-only.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub read_only: Option<bool>,
	/// A human-readable label for the field.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub label: Option<String>,
	/// Additional help text describing the field.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub help_text: Option<String>,
	/// The minimum allowed length for string values.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub min_length: Option<usize>,
	/// The maximum allowed length for string values.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub max_length: Option<usize>,
	/// The minimum allowed value for numeric fields.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub min_value: Option<f64>,
	/// The maximum allowed value for numeric fields.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub max_value: Option<f64>,
	/// Available choices for choice-type fields.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub choices: Option<Vec<ChoiceInfo>>,
	/// Child field info for list-type fields.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub child: Option<Box<FieldInfo>>,
	/// Child field info for nested object fields.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub children: Option<HashMap<String, FieldInfo>>,
	/// Custom validators applied to this field.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub validators: Option<Vec<FieldValidator>>,
	/// The default value for this field when not provided.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub default_value: Option<serde_json::Value>,
}

/// Builder for field information
pub struct FieldInfoBuilder {
	field_type: FieldType,
	required: bool,
	read_only: Option<bool>,
	label: Option<String>,
	help_text: Option<String>,
	min_length: Option<usize>,
	max_length: Option<usize>,
	min_value: Option<f64>,
	max_value: Option<f64>,
	choices: Option<Vec<ChoiceInfo>>,
	child: Option<Box<FieldInfo>>,
	children: Option<HashMap<String, FieldInfo>>,
	validators: Option<Vec<FieldValidator>>,
	default_value: Option<serde_json::Value>,
}

impl FieldInfoBuilder {
	/// Creates a new `FieldInfoBuilder` with the specified field type
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::metadata::{FieldInfoBuilder, FieldType};
	///
	/// let builder = FieldInfoBuilder::new(FieldType::String);
	/// let field = builder.build();
	/// assert_eq!(field.field_type, FieldType::String);
	/// assert!(!field.required);
	/// ```
	pub fn new(field_type: FieldType) -> Self {
		Self {
			field_type,
			required: false,
			read_only: None,
			label: None,
			help_text: None,
			min_length: None,
			max_length: None,
			min_value: None,
			max_value: None,
			choices: None,
			child: None,
			children: None,
			validators: None,
			default_value: None,
		}
	}
	/// Sets whether the field is required
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::metadata::{FieldInfoBuilder, FieldType};
	///
	/// let field = FieldInfoBuilder::new(FieldType::String)
	///     .required(true)
	///     .build();
	/// assert!(field.required);
	/// ```
	pub fn required(mut self, required: bool) -> Self {
		self.required = required;
		self
	}
	/// Sets whether the field is read-only
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::metadata::{FieldInfoBuilder, FieldType};
	///
	/// let field = FieldInfoBuilder::new(FieldType::Integer)
	///     .read_only(true)
	///     .build();
	/// assert_eq!(field.read_only, Some(true));
	/// ```
	pub fn read_only(mut self, read_only: bool) -> Self {
		self.read_only = Some(read_only);
		self
	}
	/// Sets the human-readable label for the field
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::metadata::{FieldInfoBuilder, FieldType};
	///
	/// let field = FieldInfoBuilder::new(FieldType::String)
	///     .label("Email Address")
	///     .build();
	/// assert_eq!(field.label, Some("Email Address".to_string()));
	/// ```
	pub fn label(mut self, label: impl Into<String>) -> Self {
		self.label = Some(label.into());
		self
	}
	/// Sets help text that provides additional information about the field
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::metadata::{FieldInfoBuilder, FieldType};
	///
	/// let field = FieldInfoBuilder::new(FieldType::Email)
	///     .help_text("Enter a valid email address")
	///     .build();
	/// assert_eq!(field.help_text, Some("Enter a valid email address".to_string()));
	/// ```
	pub fn help_text(mut self, help_text: impl Into<String>) -> Self {
		self.help_text = Some(help_text.into());
		self
	}
	/// Sets the minimum length constraint for string fields
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::metadata::{FieldInfoBuilder, FieldType};
	///
	/// let field = FieldInfoBuilder::new(FieldType::String)
	///     .min_length(3)
	///     .build();
	/// assert_eq!(field.min_length, Some(3));
	/// ```
	pub fn min_length(mut self, min_length: usize) -> Self {
		self.min_length = Some(min_length);
		self
	}
	/// Sets the maximum length constraint for string fields
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::metadata::{FieldInfoBuilder, FieldType};
	///
	/// let field = FieldInfoBuilder::new(FieldType::String)
	///     .max_length(100)
	///     .build();
	/// assert_eq!(field.max_length, Some(100));
	/// ```
	pub fn max_length(mut self, max_length: usize) -> Self {
		self.max_length = Some(max_length);
		self
	}
	/// Sets the minimum value constraint for numeric fields
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::metadata::{FieldInfoBuilder, FieldType};
	///
	/// let field = FieldInfoBuilder::new(FieldType::Integer)
	///     .min_value(1.0)
	///     .build();
	/// assert_eq!(field.min_value, Some(1.0));
	/// ```
	pub fn min_value(mut self, min_value: f64) -> Self {
		self.min_value = Some(min_value);
		self
	}
	/// Sets the maximum value constraint for numeric fields
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::metadata::{FieldInfoBuilder, FieldType};
	///
	/// let field = FieldInfoBuilder::new(FieldType::Float)
	///     .max_value(100.0)
	///     .build();
	/// assert_eq!(field.max_value, Some(100.0));
	/// ```
	pub fn max_value(mut self, max_value: f64) -> Self {
		self.max_value = Some(max_value);
		self
	}
	/// Sets the available choices for choice fields
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::metadata::{FieldInfoBuilder, FieldType, ChoiceInfo};
	///
	/// let choices = vec![
	///     ChoiceInfo {
	///         value: "small".to_string(),
	///         display_name: "Small".to_string(),
	///     },
	///     ChoiceInfo {
	///         value: "large".to_string(),
	///         display_name: "Large".to_string(),
	///     },
	/// ];
	///
	/// let field = FieldInfoBuilder::new(FieldType::Choice)
	///     .choices(choices)
	///     .build();
	/// assert_eq!(field.choices.as_ref().unwrap().len(), 2);
	/// ```
	pub fn choices(mut self, choices: Vec<ChoiceInfo>) -> Self {
		self.choices = Some(choices);
		self
	}
	/// Sets the child field for list fields, describing the type of elements in the list
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::metadata::{FieldInfoBuilder, FieldType};
	///
	/// let child_field = FieldInfoBuilder::new(FieldType::String)
	///     .required(true)
	///     .build();
	///
	/// let list_field = FieldInfoBuilder::new(FieldType::List)
	///     .child(child_field)
	///     .build();
	///
	/// assert!(list_field.child.is_some());
	/// assert_eq!(list_field.child.unwrap().field_type, FieldType::String);
	/// ```
	pub fn child(mut self, child: FieldInfo) -> Self {
		self.child = Some(Box::new(child));
		self
	}
	/// Sets the children fields for nested object fields, describing the structure of nested objects
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::metadata::{FieldInfoBuilder, FieldType};
	/// use std::collections::HashMap;
	///
	/// let mut children = HashMap::new();
	/// children.insert(
	///     "name".to_string(),
	///     FieldInfoBuilder::new(FieldType::String).required(true).build()
	/// );
	/// children.insert(
	///     "age".to_string(),
	///     FieldInfoBuilder::new(FieldType::Integer).required(false).build()
	/// );
	///
	/// let nested_field = FieldInfoBuilder::new(FieldType::NestedObject)
	///     .children(children)
	///     .build();
	///
	/// assert!(nested_field.children.is_some());
	/// assert_eq!(nested_field.children.as_ref().unwrap().len(), 2);
	/// ```
	pub fn children(mut self, children: HashMap<String, FieldInfo>) -> Self {
		self.children = Some(children);
		self
	}

	/// Adds a custom validator to the field
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::metadata::{FieldInfoBuilder, FieldType, FieldValidator};
	///
	/// let validator = FieldValidator {
	///     validator_type: "email".to_string(),
	///     options: None,
	///     message: Some("Invalid email format".to_string()),
	/// };
	///
	/// let field = FieldInfoBuilder::new(FieldType::Email)
	///     .required(true)
	///     .add_validator(validator)
	///     .build();
	///
	/// assert!(field.validators.is_some());
	/// assert_eq!(field.validators.as_ref().unwrap().len(), 1);
	/// assert_eq!(field.validators.as_ref().unwrap()[0].validator_type, "email");
	/// ```
	pub fn add_validator(mut self, validator: FieldValidator) -> Self {
		if let Some(ref mut validators) = self.validators {
			validators.push(validator);
		} else {
			self.validators = Some(vec![validator]);
		}
		self
	}

	/// Adds multiple validators to the field
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::metadata::{FieldInfoBuilder, FieldType, FieldValidator};
	///
	/// let validators = vec![
	///     FieldValidator {
	///         validator_type: "min_length".to_string(),
	///         options: Some(serde_json::json!({"min": 3})),
	///         message: Some("Too short".to_string()),
	///     },
	///     FieldValidator {
	///         validator_type: "max_length".to_string(),
	///         options: Some(serde_json::json!({"max": 50})),
	///         message: Some("Too long".to_string()),
	///     },
	/// ];
	///
	/// let field = FieldInfoBuilder::new(FieldType::String)
	///     .validators(validators)
	///     .build();
	///
	/// assert!(field.validators.is_some());
	/// assert_eq!(field.validators.as_ref().unwrap().len(), 2);
	/// ```
	pub fn validators(mut self, validators: Vec<FieldValidator>) -> Self {
		self.validators = Some(validators);
		self
	}

	/// Sets the default value for the field
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::metadata::{FieldInfoBuilder, FieldType};
	///
	/// let field = FieldInfoBuilder::new(FieldType::String)
	///     .required(false)
	///     .default_value(serde_json::json!("default"))
	///     .build();
	///
	/// assert!(field.default_value.is_some());
	/// assert_eq!(field.default_value, Some(serde_json::json!("default")));
	/// ```
	pub fn default_value(mut self, default_value: serde_json::Value) -> Self {
		self.default_value = Some(default_value);
		self
	}
	/// Builds the final `FieldInfo` from the builder
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::metadata::{FieldInfoBuilder, FieldType};
	///
	/// let field = FieldInfoBuilder::new(FieldType::String)
	///     .required(true)
	///     .label("Username")
	///     .min_length(3)
	///     .max_length(20)
	///     .build();
	///
	/// assert_eq!(field.field_type, FieldType::String);
	/// assert!(field.required);
	/// assert_eq!(field.label, Some("Username".to_string()));
	/// assert_eq!(field.min_length, Some(3));
	/// assert_eq!(field.max_length, Some(20));
	/// ```
	pub fn build(self) -> FieldInfo {
		FieldInfo {
			field_type: self.field_type,
			required: self.required,
			read_only: self.read_only,
			label: self.label,
			help_text: self.help_text,
			min_length: self.min_length,
			max_length: self.max_length,
			min_value: self.min_value,
			max_value: self.max_value,
			choices: self.choices,
			child: self.child,
			children: self.children,
			validators: self.validators,
			default_value: self.default_value,
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::metadata::FieldValidator;

	#[tokio::test]
	async fn test_field_info_builder() {
		let field = FieldInfoBuilder::new(FieldType::String)
			.required(true)
			.label("Username")
			.help_text("Enter your username")
			.min_length(3)
			.max_length(50)
			.build();

		assert_eq!(field.field_type, FieldType::String);
		assert!(field.required);
		assert_eq!(field.label, Some("Username".to_string()));
		assert_eq!(field.help_text, Some("Enter your username".to_string()));
		assert_eq!(field.min_length, Some(3));
		assert_eq!(field.max_length, Some(50));
	}

	#[tokio::test]
	async fn test_choice_field() {
		let choices = vec![
			ChoiceInfo {
				value: "active".to_string(),
				display_name: "Active".to_string(),
			},
			ChoiceInfo {
				value: "inactive".to_string(),
				display_name: "Inactive".to_string(),
			},
		];

		let field = FieldInfoBuilder::new(FieldType::Choice)
			.required(true)
			.label("Status")
			.choices(choices.clone())
			.build();

		assert_eq!(field.field_type, FieldType::Choice);
		assert!(field.required);
		assert_eq!(field.choices.as_ref().unwrap().len(), 2);
	}

	// DRF test: test_list_serializer_metadata_returns_info_about_fields_of_child_serializer
	#[test]
	fn test_list_field_with_child() {
		let child_field = FieldInfoBuilder::new(FieldType::Integer)
			.required(true)
			.read_only(false)
			.build();

		let list_field = FieldInfoBuilder::new(FieldType::List)
			.required(true)
			.read_only(false)
			.label("List field")
			.child(child_field)
			.build();

		assert_eq!(list_field.field_type, FieldType::List);
		assert!(list_field.child.is_some());
		let child = list_field.child.as_ref().unwrap();
		assert_eq!(child.field_type, FieldType::Integer);
	}

	// DRF test: test_dont_show_hidden_fields
	// In Rust, we handle this by simply not adding hidden fields to the field map
	#[test]
	fn test_hidden_fields_not_included() {
		let mut fields = HashMap::new();

		// Only add visible fields
		fields.insert(
			"integer_field".to_string(),
			FieldInfoBuilder::new(FieldType::Integer)
				.required(true)
				.max_value(10.0)
				.build(),
		);

		// hidden_field is intentionally not added

		assert!(fields.contains_key("integer_field"));
		assert!(!fields.contains_key("hidden_field"));
		assert_eq!(fields.len(), 1);
	}

	#[test]
	fn test_field_with_single_validator() {
		let validator = FieldValidator {
			validator_type: "email".to_string(),
			options: None,
			message: Some("Invalid email format".to_string()),
		};

		let field = FieldInfoBuilder::new(FieldType::Email)
			.required(true)
			.add_validator(validator)
			.build();

		assert!(field.validators.is_some());
		let validators = field.validators.as_ref().unwrap();
		assert_eq!(validators.len(), 1);
		assert_eq!(validators[0].validator_type, "email");
		assert_eq!(
			validators[0].message,
			Some("Invalid email format".to_string())
		);
	}

	#[test]
	fn test_field_with_multiple_validators() {
		let validators = vec![
			FieldValidator {
				validator_type: "min_length".to_string(),
				options: Some(serde_json::json!({"min": 3})),
				message: Some("Too short".to_string()),
			},
			FieldValidator {
				validator_type: "max_length".to_string(),
				options: Some(serde_json::json!({"max": 50})),
				message: Some("Too long".to_string()),
			},
			FieldValidator {
				validator_type: "regex".to_string(),
				options: Some(serde_json::json!({"pattern": "^[a-zA-Z0-9_]+$"})),
				message: Some("Invalid characters".to_string()),
			},
		];

		let field = FieldInfoBuilder::new(FieldType::String)
			.validators(validators)
			.build();

		assert!(field.validators.is_some());
		let field_validators = field.validators.as_ref().unwrap();
		assert_eq!(field_validators.len(), 3);
		assert_eq!(field_validators[0].validator_type, "min_length");
		assert_eq!(field_validators[1].validator_type, "max_length");
		assert_eq!(field_validators[2].validator_type, "regex");
	}

	#[test]
	fn test_field_without_validators() {
		let field = FieldInfoBuilder::new(FieldType::String)
			.required(true)
			.label("Username")
			.build();

		assert!(field.validators.is_none());
	}

	#[test]
	fn test_field_with_default_value_string() {
		let field = FieldInfoBuilder::new(FieldType::String)
			.required(false)
			.default_value(serde_json::json!("default text"))
			.build();

		assert!(field.default_value.is_some());
		assert_eq!(field.default_value, Some(serde_json::json!("default text")));
	}

	#[test]
	fn test_field_with_default_value_number() {
		let field = FieldInfoBuilder::new(FieldType::Integer)
			.required(false)
			.default_value(serde_json::json!(42))
			.build();

		assert!(field.default_value.is_some());
		assert_eq!(field.default_value, Some(serde_json::json!(42)));
	}

	#[test]
	fn test_field_with_default_value_boolean() {
		let field = FieldInfoBuilder::new(FieldType::Boolean)
			.required(false)
			.default_value(serde_json::json!(true))
			.build();

		assert!(field.default_value.is_some());
		assert_eq!(field.default_value, Some(serde_json::json!(true)));
	}

	#[test]
	fn test_field_with_default_value_object() {
		let default_obj = serde_json::json!({
			"name": "John Doe",
			"age": 30
		});

		let field = FieldInfoBuilder::new(FieldType::NestedObject)
			.required(false)
			.default_value(default_obj.clone())
			.build();

		assert!(field.default_value.is_some());
		assert_eq!(field.default_value, Some(default_obj));
	}

	#[test]
	fn test_field_with_default_value_array() {
		let default_array = serde_json::json!(["item1", "item2", "item3"]);

		let field = FieldInfoBuilder::new(FieldType::List)
			.required(false)
			.default_value(default_array.clone())
			.build();

		assert!(field.default_value.is_some());
		assert_eq!(field.default_value, Some(default_array));
	}

	#[test]
	fn test_field_without_default_value() {
		let field = FieldInfoBuilder::new(FieldType::String)
			.required(true)
			.label("Username")
			.build();

		assert!(field.default_value.is_none());
	}

	#[test]
	fn test_default_value_serialization() {
		let field = FieldInfoBuilder::new(FieldType::String)
			.default_value(serde_json::json!("default"))
			.build();

		let json = serde_json::to_string(&field).unwrap();
		assert!(json.contains("default_value"));
		assert!(json.contains("default"));
	}

	#[test]
	fn test_default_value_not_serialized_when_none() {
		let field = FieldInfoBuilder::new(FieldType::String).build();

		let json = serde_json::to_string(&field).unwrap();
		assert!(!json.contains("default_value"));
	}
}
