#![cfg(native)]

//! JSON Schema validation support
//!
//! This module provides JSON Schema-based validation using the `jsonschema` crate.
//! It supports multiple JSON Schema draft versions and integrates with Reinhardt's
//! `Validator` trait.
//!
//! # Example
//!
//! ```rust
//! use reinhardt_core::validators::schema::{SchemaValidator, SchemaDraft};
//! use reinhardt_core::validators::Validator;
//! use serde_json::json;
//!
//! // Create a validator with a schema
//! let schema = json!({
//!     "type": "object",
//!     "properties": {
//!         "name": { "type": "string", "minLength": 1 },
//!         "age": { "type": "integer", "minimum": 0 }
//!     },
//!     "required": ["name"]
//! });
//!
//! let validator = SchemaValidator::new(&schema).unwrap();
//!
//! // Validate JSON values
//! let valid = json!({"name": "Alice", "age": 30});
//! assert!(validator.validate(&valid).is_ok());
//!
//! let invalid = json!({"age": -5});  // missing required "name" and negative age
//! assert!(validator.validate(&invalid).is_err());
//! ```

use super::Validator;
use super::errors::{ValidationError, ValidationResult};
use serde_json::Value;

/// Supported JSON Schema draft versions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SchemaDraft {
	/// JSON Schema Draft 4
	Draft4,
	/// JSON Schema Draft 6
	Draft6,
	/// JSON Schema Draft 7
	#[default]
	Draft7,
	/// JSON Schema Draft 2019-09
	Draft201909,
	/// JSON Schema Draft 2020-12
	Draft202012,
}

impl SchemaDraft {
	/// Returns the schema URI for this draft version
	#[must_use]
	pub fn schema_uri(&self) -> &'static str {
		match self {
			Self::Draft4 => "http://json-schema.org/draft-04/schema#",
			Self::Draft6 => "http://json-schema.org/draft-06/schema#",
			Self::Draft7 => "http://json-schema.org/draft-07/schema#",
			Self::Draft201909 => "https://json-schema.org/draft/2019-09/schema",
			Self::Draft202012 => "https://json-schema.org/draft/2020-12/schema",
		}
	}

	/// Returns the human-readable name for this draft
	#[must_use]
	pub fn name(&self) -> &'static str {
		match self {
			Self::Draft4 => "Draft 4",
			Self::Draft6 => "Draft 6",
			Self::Draft7 => "Draft 7",
			Self::Draft201909 => "Draft 2019-09",
			Self::Draft202012 => "Draft 2020-12",
		}
	}
}

/// Error type for schema validation operations
#[non_exhaustive]
#[derive(Debug)]
pub enum SchemaError {
	/// The schema is invalid
	InvalidSchema(String),
	/// JSON parsing error
	JsonParseError(serde_json::Error),
	/// Validation error with details
	ValidationFailed(Vec<String>),
}

impl std::fmt::Display for SchemaError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::InvalidSchema(msg) => write!(f, "Invalid schema: {msg}"),
			Self::JsonParseError(e) => write!(f, "JSON parse error: {e}"),
			Self::ValidationFailed(errors) => {
				write!(f, "Validation failed: {}", errors.join("; "))
			}
		}
	}
}

impl std::error::Error for SchemaError {
	fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
		match self {
			Self::JsonParseError(e) => Some(e),
			_ => None,
		}
	}
}

impl From<serde_json::Error> for SchemaError {
	fn from(e: serde_json::Error) -> Self {
		Self::JsonParseError(e)
	}
}

/// A JSON Schema validator that can validate JSON values and strings
///
/// This validator wraps the `jsonschema` crate and provides integration with
/// Reinhardt's `Validator` trait.
///
/// # Example
///
/// ```rust
/// use reinhardt_core::validators::schema::{SchemaValidator, SchemaDraft};
/// use reinhardt_core::validators::Validator;
/// use serde_json::json;
///
/// // Using automatic draft detection
/// let schema = json!({"type": "string", "minLength": 3});
/// let validator = SchemaValidator::new(&schema).unwrap();
/// assert!(validator.is_valid(&json!("hello")));
/// assert!(!validator.is_valid(&json!("hi")));
///
/// // Using a specific draft
/// let validator = SchemaValidator::with_draft(&schema, SchemaDraft::Draft202012).unwrap();
/// assert!(validator.is_valid(&json!("hello")));
/// ```
pub struct SchemaValidator {
	validator: jsonschema::Validator,
	schema: Value,
	draft: Option<SchemaDraft>,
	custom_message: Option<String>,
}

impl SchemaValidator {
	/// Creates a new schema validator with automatic draft detection
	///
	/// # Errors
	///
	/// Returns an error if the schema is invalid.
	pub fn new(schema: &Value) -> Result<Self, SchemaError> {
		let validator = jsonschema::validator_for(schema)
			.map_err(|e| SchemaError::InvalidSchema(e.to_string()))?;

		Ok(Self {
			validator,
			schema: schema.clone(),
			draft: None,
			custom_message: None,
		})
	}

	/// Creates a new schema validator with a specific draft version
	///
	/// # Errors
	///
	/// Returns an error if the schema is invalid for the specified draft.
	pub fn with_draft(schema: &Value, draft: SchemaDraft) -> Result<Self, SchemaError> {
		let validator = match draft {
			SchemaDraft::Draft4 => jsonschema::draft4::new(schema),
			SchemaDraft::Draft6 => jsonschema::draft6::new(schema),
			SchemaDraft::Draft7 => jsonschema::draft7::new(schema),
			SchemaDraft::Draft201909 => jsonschema::draft201909::new(schema),
			SchemaDraft::Draft202012 => jsonschema::draft202012::new(schema),
		}
		.map_err(|e| SchemaError::InvalidSchema(e.to_string()))?;

		Ok(Self {
			validator,
			schema: schema.clone(),
			draft: Some(draft),
			custom_message: None,
		})
	}

	/// Sets a custom error message for validation failures
	#[must_use]
	pub fn with_message(mut self, message: impl Into<String>) -> Self {
		self.custom_message = Some(message.into());
		self
	}

	/// Returns the schema used by this validator
	#[must_use]
	pub fn schema(&self) -> &Value {
		&self.schema
	}

	/// Returns the draft version if explicitly specified
	#[must_use]
	pub fn draft(&self) -> Option<SchemaDraft> {
		self.draft
	}

	/// Checks if a value is valid according to the schema
	#[must_use]
	pub fn is_valid(&self, instance: &Value) -> bool {
		self.validator.is_valid(instance)
	}

	/// Validates a value and returns all errors
	pub fn validate_all(&self, instance: &Value) -> Result<(), Vec<String>> {
		let errors: Vec<String> = self
			.validator
			.iter_errors(instance)
			.map(|e| format!("{} at {}", e, e.instance_path))
			.collect();

		if errors.is_empty() {
			Ok(())
		} else {
			Err(errors)
		}
	}

	/// Parses and validates a JSON string
	///
	/// # Errors
	///
	/// Returns an error if the string is not valid JSON or doesn't match the schema.
	pub fn validate_str(&self, json_str: &str) -> Result<(), SchemaError> {
		let value: Value = serde_json::from_str(json_str)?;
		self.validate_all(&value)
			.map_err(SchemaError::ValidationFailed)
	}
}

impl std::fmt::Debug for SchemaValidator {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("SchemaValidator")
			.field("schema", &self.schema)
			.field("draft", &self.draft)
			.field("custom_message", &self.custom_message)
			.finish()
	}
}

impl Clone for SchemaValidator {
	fn clone(&self) -> Self {
		// Re-create the validator from the schema
		// This is necessary because jsonschema::Validator doesn't implement Clone
		if let Some(draft) = self.draft {
			Self::with_draft(&self.schema, draft)
				.expect("Schema was valid during creation")
				.with_message_opt(self.custom_message.clone())
		} else {
			Self::new(&self.schema)
				.expect("Schema was valid during creation")
				.with_message_opt(self.custom_message.clone())
		}
	}
}

impl SchemaValidator {
	fn with_message_opt(mut self, message: Option<String>) -> Self {
		self.custom_message = message;
		self
	}
}

impl Validator<Value> for SchemaValidator {
	fn validate(&self, value: &Value) -> ValidationResult<()> {
		match self.validator.validate(value) {
			Ok(()) => Ok(()),
			Err(error) => {
				let message = self.custom_message.clone().unwrap_or_else(|| {
					format!(
						"Schema validation failed: {} at {}",
						error, error.instance_path
					)
				});
				Err(ValidationError::Custom(message))
			}
		}
	}
}

impl Validator<str> for SchemaValidator {
	fn validate(&self, value: &str) -> ValidationResult<()> {
		let parsed: Value = serde_json::from_str(value)
			.map_err(|e| ValidationError::InvalidJSON(format!("Invalid JSON: {e}")))?;
		self.validate(&parsed)
	}
}

/// Builder for creating schema validators with custom options
#[derive(Debug, Default)]
pub struct SchemaValidatorBuilder {
	schema: Option<Value>,
	draft: Option<SchemaDraft>,
	custom_message: Option<String>,
}

impl SchemaValidatorBuilder {
	/// Creates a new builder
	#[must_use]
	pub fn new() -> Self {
		Self::default()
	}

	/// Sets the schema
	#[must_use]
	pub fn schema(mut self, schema: Value) -> Self {
		self.schema = Some(schema);
		self
	}

	/// Sets the draft version
	#[must_use]
	pub fn draft(mut self, draft: SchemaDraft) -> Self {
		self.draft = Some(draft);
		self
	}

	/// Sets a custom error message
	#[must_use]
	pub fn message(mut self, message: impl Into<String>) -> Self {
		self.custom_message = Some(message.into());
		self
	}

	/// Builds the validator
	///
	/// # Errors
	///
	/// Returns an error if no schema was provided or the schema is invalid.
	pub fn build(self) -> Result<SchemaValidator, SchemaError> {
		let schema = self
			.schema
			.ok_or_else(|| SchemaError::InvalidSchema("No schema provided".to_string()))?;

		let mut validator = if let Some(draft) = self.draft {
			SchemaValidator::with_draft(&schema, draft)?
		} else {
			SchemaValidator::new(&schema)?
		};

		if let Some(message) = self.custom_message {
			validator = validator.with_message(message);
		}

		Ok(validator)
	}
}

/// Convenience function to validate a JSON value against a schema
///
/// # Example
///
/// ```rust
/// use reinhardt_core::validators::schema::validate_json;
/// use serde_json::json;
///
/// let schema = json!({"type": "number", "minimum": 0});
/// let value = json!(42);
/// assert!(validate_json(&schema, &value).is_ok());
///
/// let invalid = json!(-5);
/// assert!(validate_json(&schema, &invalid).is_err());
/// ```
pub fn validate_json(schema: &Value, instance: &Value) -> Result<(), SchemaError> {
	let validator = SchemaValidator::new(schema)?;
	validator
		.validate_all(instance)
		.map_err(SchemaError::ValidationFailed)
}

/// Convenience function to check if a JSON value is valid against a schema
///
/// # Example
///
/// ```rust
/// use reinhardt_core::validators::schema::is_valid_json;
/// use serde_json::json;
///
/// let schema = json!({"type": "string"});
/// assert!(is_valid_json(&schema, &json!("hello")));
/// assert!(!is_valid_json(&schema, &json!(123)));
/// ```
#[must_use]
pub fn is_valid_json(schema: &Value, instance: &Value) -> bool {
	jsonschema::is_valid(schema, instance)
}

#[cfg(test)]
mod tests {
	use super::*;
	use serde_json::json;
	use std::error::Error;

	#[test]
	fn test_basic_validation() {
		let schema = json!({"type": "string"});
		let validator = SchemaValidator::new(&schema).unwrap();

		assert!(validator.validate(&json!("hello")).is_ok());
		assert!(validator.validate(&json!(123)).is_err());
	}

	#[test]
	fn test_object_validation() {
		let schema = json!({
			"type": "object",
			"properties": {
				"name": { "type": "string" },
				"age": { "type": "integer", "minimum": 0 }
			},
			"required": ["name"]
		});

		let validator = SchemaValidator::new(&schema).unwrap();

		// Valid object
		assert!(
			validator
				.validate(&json!({"name": "Alice", "age": 30}))
				.is_ok()
		);

		// Valid without optional field
		assert!(validator.validate(&json!({"name": "Bob"})).is_ok());

		// Missing required field
		assert!(validator.validate(&json!({"age": 25})).is_err());

		// Wrong type for age
		assert!(
			validator
				.validate(&json!({"name": "Carol", "age": "thirty"}))
				.is_err()
		);

		// Negative age
		assert!(
			validator
				.validate(&json!({"name": "Dave", "age": -5}))
				.is_err()
		);
	}

	#[test]
	fn test_array_validation() {
		let schema = json!({
			"type": "array",
			"items": { "type": "integer" },
			"minItems": 1,
			"maxItems": 5
		});

		let validator = SchemaValidator::new(&schema).unwrap();

		assert!(validator.validate(&json!([1, 2, 3])).is_ok());
		assert!(validator.validate(&json!([])).is_err()); // too few items
		assert!(validator.validate(&json!([1, 2, 3, 4, 5, 6])).is_err()); // too many items
		assert!(validator.validate(&json!([1, "two", 3])).is_err()); // wrong type
	}

	#[test]
	fn test_draft_specific_validation() {
		let schema = json!({
			"type": "string",
			"minLength": 3
		});

		for draft in [
			SchemaDraft::Draft4,
			SchemaDraft::Draft6,
			SchemaDraft::Draft7,
			SchemaDraft::Draft201909,
			SchemaDraft::Draft202012,
		] {
			let validator = SchemaValidator::with_draft(&schema, draft).unwrap();
			assert!(validator.validate(&json!("hello")).is_ok());
			assert!(validator.validate(&json!("hi")).is_err());
			assert_eq!(validator.draft(), Some(draft));
		}
	}

	#[test]
	fn test_draft_enum() {
		assert_eq!(SchemaDraft::default(), SchemaDraft::Draft7);

		assert_eq!(
			SchemaDraft::Draft4.schema_uri(),
			"http://json-schema.org/draft-04/schema#"
		);
		assert_eq!(
			SchemaDraft::Draft202012.schema_uri(),
			"https://json-schema.org/draft/2020-12/schema"
		);

		assert_eq!(SchemaDraft::Draft7.name(), "Draft 7");
		assert_eq!(SchemaDraft::Draft201909.name(), "Draft 2019-09");
	}

	#[test]
	fn test_string_validation() {
		let schema = json!({"type": "number", "minimum": 0});
		let validator = SchemaValidator::new(&schema).unwrap();

		// Valid JSON string
		assert!(validator.validate("42").is_ok());

		// Invalid JSON string
		assert!(validator.validate("-5").is_err());

		// Invalid JSON syntax
		assert!(validator.validate("not json").is_err());
	}

	#[test]
	fn test_validate_all_errors() {
		let schema = json!({
			"type": "object",
			"properties": {
				"name": { "type": "string" },
				"age": { "type": "integer" }
			},
			"required": ["name", "age"]
		});

		let validator = SchemaValidator::new(&schema).unwrap();
		let invalid = json!({"name": 123}); // wrong type and missing required field

		let result = validator.validate_all(&invalid);
		assert!(result.is_err());

		let errors = result.unwrap_err();
		assert!(errors.len() >= 2); // At least two errors: wrong type and missing field
	}

	#[test]
	fn test_custom_message() {
		let schema = json!({"type": "string"});
		let validator = SchemaValidator::new(&schema)
			.unwrap()
			.with_message("Value must be a valid string");

		let result = validator.validate(&json!(123));
		assert!(result.is_err());

		let error = result.unwrap_err();
		assert!(error.to_string().contains("Value must be a valid string"));
	}

	#[test]
	fn test_builder_pattern() {
		let validator = SchemaValidatorBuilder::new()
			.schema(json!({"type": "integer"}))
			.draft(SchemaDraft::Draft202012)
			.message("Must be an integer")
			.build()
			.unwrap();

		assert!(validator.validate(&json!(42)).is_ok());
		assert!(validator.validate(&json!("hello")).is_err());
		assert_eq!(validator.draft(), Some(SchemaDraft::Draft202012));
	}

	#[test]
	fn test_builder_no_schema_error() {
		let result = SchemaValidatorBuilder::new().build();
		assert!(result.is_err());
	}

	#[test]
	fn test_convenience_functions() {
		let schema = json!({"type": "boolean"});

		// validate_json
		assert!(validate_json(&schema, &json!(true)).is_ok());
		assert!(validate_json(&schema, &json!("true")).is_err());

		// is_valid_json
		assert!(is_valid_json(&schema, &json!(false)));
		assert!(!is_valid_json(&schema, &json!(0)));
	}

	#[test]
	fn test_is_valid_method() {
		let schema = json!({"type": "null"});
		let validator = SchemaValidator::new(&schema).unwrap();

		assert!(validator.is_valid(&json!(null)));
		assert!(!validator.is_valid(&json!("null")));
	}

	#[test]
	fn test_schema_accessor() {
		let schema = json!({"type": "string", "maxLength": 10});
		let validator = SchemaValidator::new(&schema).unwrap();

		assert_eq!(validator.schema(), &schema);
	}

	#[test]
	fn test_clone() {
		let schema = json!({"type": "number"});
		let validator = SchemaValidator::new(&schema)
			.unwrap()
			.with_message("Custom message");

		let cloned = validator.clone();

		assert_eq!(cloned.schema(), validator.schema());
		assert!(cloned.validate(&json!(42)).is_ok());
		assert!(cloned.validate(&json!("hello")).is_err());
	}

	#[test]
	fn test_clone_with_draft() {
		let schema = json!({"type": "boolean"});
		let validator = SchemaValidator::with_draft(&schema, SchemaDraft::Draft201909).unwrap();

		let cloned = validator.clone();

		assert_eq!(cloned.draft(), Some(SchemaDraft::Draft201909));
		assert!(cloned.validate(&json!(true)).is_ok());
	}

	#[test]
	fn test_invalid_schema() {
		let invalid_schema = json!({
			"type": "invalid_type"
		});

		// jsonschema 0.26+ validates schema structure and rejects invalid type values
		let validator = SchemaValidator::new(&invalid_schema);
		assert!(validator.is_err());

		if let Err(e) = validator {
			assert!(e.to_string().contains("Invalid schema"));
		}
	}

	#[test]
	fn test_complex_schema() {
		let schema = json!({
			"type": "object",
			"properties": {
				"id": {
					"type": "integer",
					"minimum": 1
				},
				"email": {
					"type": "string",
					"format": "email"
				},
				"tags": {
					"type": "array",
					"items": { "type": "string" },
					"uniqueItems": true
				},
				"metadata": {
					"type": "object",
					"additionalProperties": { "type": "string" }
				}
			},
			"required": ["id", "email"]
		});

		let validator = SchemaValidator::new(&schema).unwrap();

		// Valid complex object
		let valid = json!({
			"id": 1,
			"email": "test@example.com",
			"tags": ["rust", "validation"],
			"metadata": {
				"created_by": "admin"
			}
		});
		assert!(validator.validate(&valid).is_ok());

		// Missing required field
		let missing_email = json!({"id": 1});
		assert!(validator.validate(&missing_email).is_err());

		// Invalid id (too small)
		let invalid_id = json!({"id": 0, "email": "test@example.com"});
		assert!(validator.validate(&invalid_id).is_err());
	}

	#[test]
	fn test_validate_str_method() {
		let schema = json!({"type": "array", "items": {"type": "number"}});
		let validator = SchemaValidator::new(&schema).unwrap();

		assert!(validator.validate_str("[1, 2, 3]").is_ok());
		assert!(validator.validate_str("[1, \"two\", 3]").is_err());
		assert!(validator.validate_str("invalid json").is_err());
	}

	#[test]
	fn test_schema_error_display() {
		let invalid_json_err =
			SchemaError::JsonParseError(serde_json::from_str::<Value>("invalid").unwrap_err());
		assert!(invalid_json_err.to_string().contains("JSON parse error"));

		let schema_err = SchemaError::InvalidSchema("test error".to_string());
		assert!(schema_err.to_string().contains("Invalid schema"));
		assert!(schema_err.to_string().contains("test error"));

		let validation_err =
			SchemaError::ValidationFailed(vec!["error1".to_string(), "error2".to_string()]);
		let display = validation_err.to_string();
		assert!(display.contains("error1"));
		assert!(display.contains("error2"));
	}

	#[test]
	fn test_schema_error_source() {
		let json_err = serde_json::from_str::<Value>("invalid").unwrap_err();
		let schema_err = SchemaError::JsonParseError(json_err);
		assert!(schema_err.source().is_some());

		let invalid_err = SchemaError::InvalidSchema("test".to_string());
		assert!(invalid_err.source().is_none());

		let validation_err = SchemaError::ValidationFailed(vec![]);
		assert!(validation_err.source().is_none());
	}

	#[test]
	fn test_debug_impl() {
		let schema = json!({"type": "string"});
		let validator = SchemaValidator::new(&schema).unwrap();

		let debug_str = format!("{:?}", validator);
		assert!(debug_str.contains("SchemaValidator"));
		assert!(debug_str.contains("schema"));
	}
}
