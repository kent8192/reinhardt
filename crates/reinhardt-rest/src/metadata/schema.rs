//! OpenAPI 3.0 schema generation from field metadata

use super::fields::FieldInfo;
use super::types::FieldType;
use serde::{Deserialize, Serialize};
use serde_json::Value;
#[cfg(test)]
use serde_json::json;
use std::collections::HashMap;

/// OpenAPI 3.0 schema representation
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct FieldSchema {
	/// The JSON Schema type (e.g., `"string"`, `"integer"`, `"object"`).
	#[serde(rename = "type", skip_serializing_if = "Option::is_none")]
	pub schema_type: Option<String>,
	/// The format hint (e.g., `"date-time"`, `"email"`, `"uri"`).
	#[serde(skip_serializing_if = "Option::is_none")]
	pub format: Option<String>,
	/// A description of what this field represents.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub description: Option<String>,
	/// The minimum numeric value allowed.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub minimum: Option<f64>,
	/// The maximum numeric value allowed.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub maximum: Option<f64>,
	/// The minimum string length allowed.
	#[serde(rename = "minLength", skip_serializing_if = "Option::is_none")]
	pub min_length: Option<usize>,
	/// The maximum string length allowed.
	#[serde(rename = "maxLength", skip_serializing_if = "Option::is_none")]
	pub max_length: Option<usize>,
	/// A regex pattern the value must match.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub pattern: Option<String>,
	/// Allowed enum values for choice fields.
	#[serde(rename = "enum", skip_serializing_if = "Option::is_none")]
	pub enum_values: Option<Vec<String>>,
	/// Schema for array items.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub items: Option<Box<FieldSchema>>,
	/// Schemas for object properties.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub properties: Option<HashMap<String, FieldSchema>>,
	/// List of required property names within an object schema.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub required: Option<Vec<String>>,
	/// The default value for this field.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub default: Option<Value>,
	/// Whether this field is read-only.
	#[serde(rename = "readOnly", skip_serializing_if = "Option::is_none")]
	pub read_only: Option<bool>,
	/// Whether this field is write-only.
	#[serde(rename = "writeOnly", skip_serializing_if = "Option::is_none")]
	pub write_only: Option<bool>,
	/// Whether this field accepts null values.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub nullable: Option<bool>,
}

/// Generates an OpenAPI schema from field metadata
///
/// # Examples
///
/// ```
/// use reinhardt_rest::metadata::{FieldInfoBuilder, FieldType, generate_field_schema};
///
/// let field = FieldInfoBuilder::new(FieldType::String)
///     .required(true)
///     .min_length(3)
///     .max_length(50)
///     .build();
///
/// let schema = generate_field_schema(&field);
/// assert_eq!(schema.schema_type, Some("string".to_string()));
/// assert_eq!(schema.min_length, Some(3));
/// assert_eq!(schema.max_length, Some(50));
/// ```
pub fn generate_field_schema(field: &FieldInfo) -> FieldSchema {
	let mut schema = FieldSchema::default();

	// Map FieldType to OpenAPI type and format
	match &field.field_type {
		FieldType::Boolean => {
			schema.schema_type = Some("boolean".to_string());
		}
		FieldType::String => {
			schema.schema_type = Some("string".to_string());
		}
		FieldType::Integer => {
			schema.schema_type = Some("integer".to_string());
			schema.format = Some("int64".to_string());
		}
		FieldType::Float => {
			schema.schema_type = Some("number".to_string());
			schema.format = Some("float".to_string());
		}
		FieldType::Decimal => {
			schema.schema_type = Some("number".to_string());
			schema.format = Some("double".to_string());
		}
		FieldType::Date => {
			schema.schema_type = Some("string".to_string());
			schema.format = Some("date".to_string());
		}
		FieldType::DateTime => {
			schema.schema_type = Some("string".to_string());
			schema.format = Some("date-time".to_string());
		}
		FieldType::Time => {
			schema.schema_type = Some("string".to_string());
			schema.format = Some("time".to_string());
		}
		FieldType::Duration => {
			schema.schema_type = Some("string".to_string());
			schema.format = Some("duration".to_string());
		}
		FieldType::Email => {
			schema.schema_type = Some("string".to_string());
			schema.format = Some("email".to_string());
		}
		FieldType::Url => {
			schema.schema_type = Some("string".to_string());
			schema.format = Some("uri".to_string());
		}
		FieldType::Uuid => {
			schema.schema_type = Some("string".to_string());
			schema.format = Some("uuid".to_string());
		}
		FieldType::Choice => {
			schema.schema_type = Some("string".to_string());
			if let Some(choices) = &field.choices {
				schema.enum_values = Some(choices.iter().map(|c| c.value.clone()).collect());
			}
		}
		FieldType::MultipleChoice => {
			schema.schema_type = Some("array".to_string());
			if let Some(choices) = &field.choices {
				let item_schema = FieldSchema {
					schema_type: Some("string".to_string()),
					enum_values: Some(choices.iter().map(|c| c.value.clone()).collect()),
					..Default::default()
				};
				schema.items = Some(Box::new(item_schema));
			}
		}
		FieldType::File => {
			schema.schema_type = Some("string".to_string());
			schema.format = Some("binary".to_string());
		}
		FieldType::Image => {
			schema.schema_type = Some("string".to_string());
			schema.format = Some("binary".to_string());
		}
		FieldType::List => {
			schema.schema_type = Some("array".to_string());
			if let Some(child) = &field.child {
				schema.items = Some(Box::new(generate_field_schema(child)));
			}
		}
		FieldType::NestedObject => {
			schema.schema_type = Some("object".to_string());
			if let Some(children) = &field.children {
				let mut properties = HashMap::new();
				let mut required_fields = Vec::new();

				for (name, child_field) in children {
					properties.insert(name.clone(), generate_field_schema(child_field));
					if child_field.required {
						required_fields.push(name.clone());
					}
				}

				schema.properties = Some(properties);
				if !required_fields.is_empty() {
					schema.required = Some(required_fields);
				}
			}
		}
		FieldType::Field => {
			// Generic field type
			schema.schema_type = Some("string".to_string());
		}
	}

	// Add constraints
	if let Some(min_length) = field.min_length {
		schema.min_length = Some(min_length);
	}
	if let Some(max_length) = field.max_length {
		schema.max_length = Some(max_length);
	}
	if let Some(min_value) = field.min_value {
		schema.minimum = Some(min_value);
	}
	if let Some(max_value) = field.max_value {
		schema.maximum = Some(max_value);
	}

	// Add description from help_text or label
	if let Some(help_text) = &field.help_text {
		schema.description = Some(help_text.clone());
	} else if let Some(label) = &field.label {
		schema.description = Some(label.clone());
	}

	// Add default value
	if let Some(default_value) = &field.default_value {
		schema.default = Some(default_value.clone());
	}

	// Add read-only flag
	if let Some(true) = field.read_only {
		schema.read_only = Some(true);
	}

	// Extract regex pattern from validators for OpenAPI schema
	if let Some(validators) = &field.validators {
		for validator in validators {
			if let Some(pattern) = validator.extract_pattern() {
				schema.pattern = Some(pattern);
				break;
			}
		}
	}

	schema
}

/// Generates a complete OpenAPI schema object from a map of fields
///
/// # Examples
///
/// ```
/// use reinhardt_rest::metadata::{FieldInfoBuilder, FieldType, generate_object_schema};
/// use std::collections::HashMap;
///
/// let mut fields = HashMap::new();
/// fields.insert(
///     "name".to_string(),
///     FieldInfoBuilder::new(FieldType::String)
///         .required(true)
///         .build()
/// );
/// fields.insert(
///     "age".to_string(),
///     FieldInfoBuilder::new(FieldType::Integer)
///         .required(false)
///         .build()
/// );
///
/// let schema = generate_object_schema(&fields);
/// assert_eq!(schema.schema_type, Some("object".to_string()));
/// assert_eq!(schema.required, Some(vec!["name".to_string()]));
/// ```
pub fn generate_object_schema(fields: &HashMap<String, FieldInfo>) -> FieldSchema {
	let mut schema = FieldSchema {
		schema_type: Some("object".to_string()),
		..Default::default()
	};

	let mut properties = HashMap::new();
	let mut required_fields = Vec::new();

	for (name, field) in fields {
		properties.insert(name.clone(), generate_field_schema(field));
		if field.required {
			required_fields.push(name.clone());
		}
	}

	schema.properties = Some(properties);
	if !required_fields.is_empty() {
		required_fields.sort(); // Ensure consistent ordering
		schema.required = Some(required_fields);
	}

	schema
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::metadata::fields::FieldInfoBuilder;
	use crate::metadata::types::ChoiceInfo;
	use crate::metadata::validators::FieldValidator;

	#[test]
	fn test_generate_string_schema() {
		let field = FieldInfoBuilder::new(FieldType::String)
			.min_length(3)
			.max_length(50)
			.build();

		let schema = generate_field_schema(&field);
		assert_eq!(schema.schema_type, Some("string".to_string()));
		assert_eq!(schema.min_length, Some(3));
		assert_eq!(schema.max_length, Some(50));
	}

	#[test]
	fn test_generate_integer_schema() {
		let field = FieldInfoBuilder::new(FieldType::Integer)
			.min_value(1.0)
			.max_value(100.0)
			.build();

		let schema = generate_field_schema(&field);
		assert_eq!(schema.schema_type, Some("integer".to_string()));
		assert_eq!(schema.format, Some("int64".to_string()));
		assert_eq!(schema.minimum, Some(1.0));
		assert_eq!(schema.maximum, Some(100.0));
	}

	#[test]
	fn test_generate_email_schema() {
		let field = FieldInfoBuilder::new(FieldType::Email).build();

		let schema = generate_field_schema(&field);
		assert_eq!(schema.schema_type, Some("string".to_string()));
		assert_eq!(schema.format, Some("email".to_string()));
	}

	#[test]
	fn test_generate_datetime_schema() {
		let field = FieldInfoBuilder::new(FieldType::DateTime).build();

		let schema = generate_field_schema(&field);
		assert_eq!(schema.schema_type, Some("string".to_string()));
		assert_eq!(schema.format, Some("date-time".to_string()));
	}

	#[test]
	fn test_generate_choice_schema() {
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
			.choices(choices)
			.build();

		let schema = generate_field_schema(&field);
		assert_eq!(schema.schema_type, Some("string".to_string()));
		assert_eq!(
			schema.enum_values,
			Some(vec!["active".to_string(), "inactive".to_string()])
		);
	}

	#[test]
	fn test_generate_list_schema() {
		let child = FieldInfoBuilder::new(FieldType::String)
			.min_length(1)
			.build();

		let field = FieldInfoBuilder::new(FieldType::List).child(child).build();

		let schema = generate_field_schema(&field);
		assert_eq!(schema.schema_type, Some("array".to_string()));
		assert!(schema.items.is_some());

		let items = schema.items.unwrap();
		assert_eq!(items.schema_type, Some("string".to_string()));
		assert_eq!(items.min_length, Some(1));
	}

	#[test]
	fn test_generate_nested_object_schema() {
		let mut children = HashMap::new();
		children.insert(
			"name".to_string(),
			FieldInfoBuilder::new(FieldType::String)
				.required(true)
				.build(),
		);
		children.insert(
			"age".to_string(),
			FieldInfoBuilder::new(FieldType::Integer).build(),
		);

		let field = FieldInfoBuilder::new(FieldType::NestedObject)
			.children(children)
			.build();

		let schema = generate_field_schema(&field);
		assert_eq!(schema.schema_type, Some("object".to_string()));
		assert!(schema.properties.is_some());

		let properties = schema.properties.unwrap();
		assert_eq!(properties.len(), 2);
		assert!(properties.contains_key("name"));
		assert!(properties.contains_key("age"));

		assert_eq!(schema.required, Some(vec!["name".to_string()]));
	}

	#[test]
	fn test_generate_schema_with_description() {
		let field = FieldInfoBuilder::new(FieldType::String)
			.help_text("Enter your username")
			.build();

		let schema = generate_field_schema(&field);
		assert_eq!(schema.description, Some("Enter your username".to_string()));
	}

	#[test]
	fn test_generate_schema_with_label_fallback() {
		let field = FieldInfoBuilder::new(FieldType::String)
			.label("Username")
			.build();

		let schema = generate_field_schema(&field);
		assert_eq!(schema.description, Some("Username".to_string()));
	}

	#[test]
	fn test_generate_schema_with_default_value() {
		let field = FieldInfoBuilder::new(FieldType::String)
			.default_value(json!("default_text"))
			.build();

		let schema = generate_field_schema(&field);
		assert_eq!(schema.default, Some(json!("default_text")));
	}

	#[test]
	fn test_generate_schema_with_read_only() {
		let field = FieldInfoBuilder::new(FieldType::Integer)
			.read_only(true)
			.build();

		let schema = generate_field_schema(&field);
		assert_eq!(schema.read_only, Some(true));
	}

	#[test]
	fn test_generate_schema_with_regex_pattern() {
		let validator = FieldValidator {
			validator_type: "regex".to_string(),
			options: Some(json!({"pattern": "^[a-zA-Z0-9_]+$"})),
			message: Some("Invalid format".to_string()),
		};

		let field = FieldInfoBuilder::new(FieldType::String)
			.add_validator(validator)
			.build();

		let schema = generate_field_schema(&field);
		assert_eq!(schema.pattern, Some("^[a-zA-Z0-9_]+$".to_string()));
	}

	#[test]
	fn test_generate_object_schema_basic() {
		let mut fields = HashMap::new();
		fields.insert(
			"name".to_string(),
			FieldInfoBuilder::new(FieldType::String)
				.required(true)
				.build(),
		);
		fields.insert(
			"email".to_string(),
			FieldInfoBuilder::new(FieldType::Email)
				.required(true)
				.build(),
		);
		fields.insert(
			"age".to_string(),
			FieldInfoBuilder::new(FieldType::Integer).build(),
		);

		let schema = generate_object_schema(&fields);
		assert_eq!(schema.schema_type, Some("object".to_string()));
		assert!(schema.properties.is_some());

		let properties = schema.properties.unwrap();
		assert_eq!(properties.len(), 3);

		let required = schema.required.unwrap();
		assert_eq!(required.len(), 2);
		assert!(required.contains(&"name".to_string()));
		assert!(required.contains(&"email".to_string()));
	}

	#[test]
	fn test_generate_object_schema_empty() {
		let fields = HashMap::new();
		let schema = generate_object_schema(&fields);

		assert_eq!(schema.schema_type, Some("object".to_string()));
		assert!(schema.properties.is_some());
		assert_eq!(schema.properties.unwrap().len(), 0);
		assert!(schema.required.is_none());
	}

	#[test]
	fn test_schema_serialization() {
		let field = FieldInfoBuilder::new(FieldType::String)
			.min_length(3)
			.max_length(50)
			.build();

		let schema = generate_field_schema(&field);
		let json = serde_json::to_string(&schema).unwrap();

		assert!(json.contains("\"type\":\"string\""));
		assert!(json.contains("\"minLength\":3"));
		assert!(json.contains("\"maxLength\":50"));
	}
}
