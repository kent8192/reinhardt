//! Field and object-level validation for serializers
//!
//! Provides validation traits and utilities for serializer fields.

use super::fields::{
	BooleanField, CharField, ChoiceField, DateField, DateTimeField, EmailField, FieldError,
	FloatField, IntegerField, URLField,
};
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
	FieldError {
		/// Name of the field that failed validation.
		field: String,
		/// Human-readable error message.
		message: String,
	},

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

	/// Return field validation messages grouped by field name.
	///
	/// Object-level errors are not included because they have no field key.
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_core::serializers::ValidationError;
	///
	/// let error = ValidationError::multiple(vec![
	///     ValidationError::field_error("start", "Too long"),
	///     ValidationError::field_error("priority", "Too small"),
	/// ]);
	/// let errors = error.field_errors();
	/// assert_eq!(errors["start"], ["Too long"]);
	/// assert_eq!(errors["priority"], ["Too small"]);
	/// ```
	pub fn field_errors(&self) -> HashMap<String, Vec<String>> {
		let mut field_errors = HashMap::new();
		self.collect_field_errors(&mut field_errors);
		field_errors
	}

	fn collect_field_errors(&self, field_errors: &mut HashMap<String, Vec<String>>) {
		match self {
			Self::FieldError { field, message } => field_errors
				.entry(field.clone())
				.or_default()
				.push(message.clone()),
			Self::MultipleErrors(errors) => {
				for error in errors {
					error.collect_field_errors(field_errors);
				}
			}
			Self::ObjectError(_) => {}
		}
	}

	fn with_field(self, field: &str) -> Self {
		match self {
			Self::FieldError { message, .. } | Self::ObjectError(message) => {
				Self::field_error(field, message)
			}
			Self::MultipleErrors(errors) => Self::multiple(
				errors
					.into_iter()
					.map(|error| error.with_field(field))
					.collect(),
			),
		}
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

	/// Whether a missing JSON key should fail [`validate_fields`].
	///
	/// Custom validators default to `false` so omitted keys remain a pass-through.
	/// Built-in serializer fields return their `required` configuration.
	fn is_required(&self) -> bool {
		false
	}
}

macro_rules! impl_typed_field_validator {
	($($field:ty),+ $(,)?) => {
		$(
			impl FieldValidator for $field {
				fn validate(&self, value: &Value) -> ValidationResult {
					self.to_internal_value(Some(value))
						.map(|_| ())
						.map_err(|error| ValidationError::object_error(error.to_string()))
				}

				fn is_required(&self) -> bool {
					self.required
				}
			}
		)+
	};
}

impl_typed_field_validator!(
	CharField,
	IntegerField,
	FloatField,
	BooleanField,
	EmailField,
	URLField,
	ChoiceField,
	DateField,
	DateTimeField,
);

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
/// Missing keys skip custom validators by default. Built-in serializer fields
/// report [`FieldError::Required`] when [`FieldValidator::is_required`] is true.
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
		match data.get(field_name) {
			Some(value) => {
				if let Err(e) = validator.validate(value) {
					errors.push(e.with_field(field_name));
				}
			}
			None if validator.is_required() => {
				errors.push(ValidationError::field_error(
					field_name,
					FieldError::Required.to_string(),
				));
			}
			None => {}
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

		// Missing custom validators are not required (pass through)
		let result = validate_fields(&data, &validators);
		assert!(result.is_ok());
	}
}
