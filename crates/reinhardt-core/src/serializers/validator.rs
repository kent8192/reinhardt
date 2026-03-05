//! Field and object-level validation for serializers
//!
//! Provides validation traits and utilities for serializer fields.

use serde_json::Value;
use std::collections::HashMap;

/// Result type for validation operations
pub type ValidationResult<T = ()> = Result<T, ValidationError>;

/// Error type for validation failures
#[non_exhaustive]
#[derive(Debug, Clone, thiserror::Error)]
pub enum ValidationError {
	/// Single field validation error
	#[error("Validation error on field '{field}': {message}")]
	FieldError { field: String, message: String },

	/// Multiple field validation errors
	#[error("Multiple validation errors: {0:?}")]
	MultipleErrors(Vec<ValidationError>),

	/// Object-level validation error
	#[error("Object validation error: {0}")]
	ObjectError(String),
}

impl ValidationError {
	/// Create a new field validation error
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::serializers::ValidationError;
	///
	/// let error = ValidationError::field_error("email", "Invalid email format");
	/// // Verify the error is created successfully
	/// let _: ValidationError = error;
	/// ```
	pub fn field_error(field: impl Into<String>, message: impl Into<String>) -> Self {
		Self::FieldError {
			field: field.into(),
			message: message.into(),
		}
	}

	/// Create a new object-level validation error
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::serializers::ValidationError;
	///
	/// let error = ValidationError::object_error("Password and confirmation do not match");
	/// // Verify the error is created successfully
	/// let _: ValidationError = error;
	/// ```
	pub fn object_error(message: impl Into<String>) -> Self {
		Self::ObjectError(message.into())
	}

	/// Combine multiple validation errors
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::serializers::ValidationError;
	///
	/// let errors = vec![
	///     ValidationError::field_error("email", "Required"),
	///     ValidationError::field_error("age", "Must be positive"),
	/// ];
	/// let combined = ValidationError::multiple(errors);
	/// // Verify the combined error is created successfully
	/// let _: ValidationError = combined;
	/// ```
	pub fn multiple(errors: Vec<ValidationError>) -> Self {
		Self::MultipleErrors(errors)
	}
}

/// Trait for field-level validators
///
/// Implementors can validate individual field values.
///
/// # Examples
///
/// ```
/// use reinhardt_core::serializers::{FieldValidator, ValidationResult, ValidationError};
/// use serde_json::{Value, json};
///
/// struct EmailValidator;
///
/// impl FieldValidator for EmailValidator {
///     fn validate(&self, value: &Value) -> ValidationResult {
///         if let Some(email) = value.as_str() {
///             if email.contains('@') {
///                 Ok(())
///             } else {
///                 Err(ValidationError::field_error("email", "Invalid email format"))
///             }
///         } else {
///             Err(ValidationError::field_error("email", "Must be a string"))
///         }
///     }
/// }
///
/// // Verify the validator implementation works correctly
/// let validator = EmailValidator;
/// assert!(validator.validate(&json!("test@example.com")).is_ok());
/// assert!(validator.validate(&json!("invalid")).is_err());
/// ```
pub trait FieldValidator {
	/// Validate a field value
	fn validate(&self, value: &Value) -> ValidationResult;
}

/// Trait for object-level validators
///
/// Implementors can validate entire objects with multiple fields.
///
/// # Examples
///
/// ```
/// use reinhardt_core::serializers::{ObjectValidator, ValidationResult, ValidationError};
/// use serde_json::{Value, json};
/// use std::collections::HashMap;
///
/// struct PasswordMatchValidator;
///
/// impl ObjectValidator for PasswordMatchValidator {
///     fn validate(&self, data: &HashMap<String, Value>) -> ValidationResult {
///         let password = data.get("password").and_then(|v| v.as_str());
///         let confirm = data.get("password_confirm").and_then(|v| v.as_str());
///
///         if password == confirm {
///             Ok(())
///         } else {
///             Err(ValidationError::object_error("Passwords do not match"))
///         }
///     }
/// }
///
/// // Verify the validator implementation works correctly
/// let validator = PasswordMatchValidator;
/// let mut data = HashMap::new();
/// data.insert("password".to_string(), json!("secret"));
/// data.insert("password_confirm".to_string(), json!("secret"));
/// assert!(validator.validate(&data).is_ok());
/// ```
pub trait ObjectValidator {
	/// Validate an entire object
	fn validate(&self, data: &HashMap<String, Value>) -> ValidationResult;
}

/// Trait for serializers that support field-level validation
///
/// Implementors can define `validate_<field_name>` methods that are
/// automatically called during validation.
pub trait FieldLevelValidation {
	/// Validate a specific field by name
	///
	/// This method looks for a `validate_<field_name>` method and calls it.
	/// If no such method exists, validation passes.
	fn validate_field(&self, field_name: &str, value: &Value) -> ValidationResult;

	/// Get all field validators
	fn get_field_validators(&self) -> HashMap<String, Box<dyn FieldValidator>>;
}

/// Trait for serializers that support object-level validation
///
/// Implementors can define a `validate` method that validates the entire
/// object after all fields have been validated.
pub trait ObjectLevelValidation {
	/// Validate the entire object
	///
	/// This is called after all field-level validations have passed.
	fn validate(&self, data: &HashMap<String, Value>) -> ValidationResult;
}

/// Helper function to validate all fields in a data object
///
/// # Examples
///
/// ```
/// use reinhardt_core::serializers::{validate_fields, FieldValidator, ValidationResult, ValidationError};
/// use serde_json::{Value, json};
/// use std::collections::HashMap;
///
/// struct PositiveNumberValidator;
///
/// impl FieldValidator for PositiveNumberValidator {
///     fn validate(&self, value: &Value) -> ValidationResult {
///         if let Some(num) = value.as_i64() {
///             if num > 0 {
///                 Ok(())
///             } else {
///                 Err(ValidationError::field_error("number", "Must be positive"))
///             }
///         } else {
///             Err(ValidationError::field_error("number", "Must be a number"))
///         }
///     }
/// }
///
/// let mut validators: HashMap<String, Box<dyn FieldValidator>> = HashMap::new();
/// validators.insert("age".to_string(), Box::new(PositiveNumberValidator));
///
/// let mut data = HashMap::new();
/// data.insert("age".to_string(), json!(25));
///
/// // Verify field validation succeeds for valid data
/// let result = validate_fields(&data, &validators);
/// assert!(result.is_ok());
/// ```
pub fn validate_fields(
	data: &HashMap<String, Value>,
	validators: &HashMap<String, Box<dyn FieldValidator>>,
) -> ValidationResult {
	let mut errors = Vec::new();

	for (field_name, validator) in validators {
		if let Some(value) = data.get(field_name)
			&& let Err(e) = validator.validate(value)
		{
			errors.push(e);
		}
	}

	if errors.is_empty() {
		Ok(())
	} else if errors.len() == 1 {
		Err(errors.into_iter().next().unwrap())
	} else {
		Err(ValidationError::multiple(errors))
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use serde_json::json;

	struct EmailValidator;

	impl FieldValidator for EmailValidator {
		fn validate(&self, value: &Value) -> ValidationResult {
			if let Some(email) = value.as_str() {
				if email.contains('@') {
					Ok(())
				} else {
					Err(ValidationError::field_error(
						"email",
						"Invalid email format",
					))
				}
			} else {
				Err(ValidationError::field_error("email", "Must be a string"))
			}
		}
	}

	struct PositiveNumberValidator;

	impl FieldValidator for PositiveNumberValidator {
		fn validate(&self, value: &Value) -> ValidationResult {
			if let Some(num) = value.as_i64() {
				if num > 0 {
					Ok(())
				} else {
					Err(ValidationError::field_error("number", "Must be positive"))
				}
			} else {
				Err(ValidationError::field_error("number", "Must be a number"))
			}
		}
	}

	struct PasswordMatchValidator;

	impl ObjectValidator for PasswordMatchValidator {
		fn validate(&self, data: &HashMap<String, Value>) -> ValidationResult {
			let password = data.get("password").and_then(|v| v.as_str());
			let confirm = data.get("password_confirm").and_then(|v| v.as_str());

			if password == confirm {
				Ok(())
			} else {
				Err(ValidationError::object_error("Passwords do not match"))
			}
		}
	}

	#[test]
	fn test_validation_error_field_error() {
		let error = ValidationError::field_error("email", "Required field");
		match error {
			ValidationError::FieldError { field, message } => {
				assert_eq!(field, "email");
				assert_eq!(message, "Required field");
			}
			_ => panic!("Expected FieldError"),
		}
	}

	#[test]
	fn test_validation_error_object_error() {
		let error = ValidationError::object_error("Invalid data");
		match error {
			ValidationError::ObjectError(msg) => {
				assert_eq!(msg, "Invalid data");
			}
			_ => panic!("Expected ObjectError"),
		}
	}

	#[test]
	fn test_validation_error_multiple() {
		let errors = vec![
			ValidationError::field_error("email", "Required"),
			ValidationError::field_error("age", "Must be positive"),
		];
		let combined = ValidationError::multiple(errors);
		match combined {
			ValidationError::MultipleErrors(errs) => {
				assert_eq!(errs.len(), 2);
			}
			_ => panic!("Expected MultipleErrors"),
		}
	}

	#[test]
	fn test_email_validator_valid() {
		let validator = EmailValidator;
		let value = json!("test@example.com");
		assert!(validator.validate(&value).is_ok());
	}

	#[test]
	fn test_email_validator_invalid() {
		let validator = EmailValidator;
		let value = json!("not-an-email");
		assert!(validator.validate(&value).is_err());
	}

	#[test]
	fn test_positive_number_validator_valid() {
		let validator = PositiveNumberValidator;
		let value = json!(42);
		assert!(validator.validate(&value).is_ok());
	}

	#[test]
	fn test_positive_number_validator_invalid() {
		let validator = PositiveNumberValidator;
		let value = json!(-5);
		assert!(validator.validate(&value).is_err());
	}

	#[test]
	fn test_password_match_validator_matching() {
		let validator = PasswordMatchValidator;
		let mut data = HashMap::new();
		data.insert("password".to_string(), json!("secret123"));
		data.insert("password_confirm".to_string(), json!("secret123"));
		assert!(validator.validate(&data).is_ok());
	}

	#[test]
	fn test_password_match_validator_not_matching() {
		let validator = PasswordMatchValidator;
		let mut data = HashMap::new();
		data.insert("password".to_string(), json!("secret123"));
		data.insert("password_confirm".to_string(), json!("different"));
		assert!(validator.validate(&data).is_err());
	}

	#[test]
	fn test_validate_fields_all_valid() {
		let mut validators: HashMap<String, Box<dyn FieldValidator>> = HashMap::new();
		validators.insert("email".to_string(), Box::new(EmailValidator));
		validators.insert("age".to_string(), Box::new(PositiveNumberValidator));

		let mut data = HashMap::new();
		data.insert("email".to_string(), json!("user@example.com"));
		data.insert("age".to_string(), json!(25));

		let result = validate_fields(&data, &validators);
		assert!(result.is_ok());
	}

	#[test]
	fn test_validate_fields_one_invalid() {
		let mut validators: HashMap<String, Box<dyn FieldValidator>> = HashMap::new();
		validators.insert("email".to_string(), Box::new(EmailValidator));
		validators.insert("age".to_string(), Box::new(PositiveNumberValidator));

		let mut data = HashMap::new();
		data.insert("email".to_string(), json!("invalid-email"));
		data.insert("age".to_string(), json!(25));

		let result = validate_fields(&data, &validators);
		assert!(result.is_err());
	}

	#[test]
	fn test_validate_fields_multiple_invalid() {
		let mut validators: HashMap<String, Box<dyn FieldValidator>> = HashMap::new();
		validators.insert("email".to_string(), Box::new(EmailValidator));
		validators.insert("age".to_string(), Box::new(PositiveNumberValidator));

		let mut data = HashMap::new();
		data.insert("email".to_string(), json!("invalid-email"));
		data.insert("age".to_string(), json!(-5));

		let result = validate_fields(&data, &validators);
		assert!(result.is_err());
		if let Err(ValidationError::MultipleErrors(errors)) = result {
			assert_eq!(errors.len(), 2);
		} else {
			panic!("Expected MultipleErrors");
		}
	}

	#[test]
	fn test_validate_fields_missing_field() {
		let mut validators: HashMap<String, Box<dyn FieldValidator>> = HashMap::new();
		validators.insert("email".to_string(), Box::new(EmailValidator));

		let data = HashMap::new(); // No email field

		// Missing fields are not validated (pass through)
		let result = validate_fields(&data, &validators);
		assert!(result.is_ok());
	}
}
