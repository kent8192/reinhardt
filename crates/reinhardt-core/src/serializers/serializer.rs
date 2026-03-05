//! Core serialization traits and implementations
//!
//! Provides the foundational `Serializer` and `Deserializer` traits along with
//! error types for serialization operations.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Core serializer trait for converting between input and output representations
///
/// # Type Parameters
///
/// - `Input`: The source type to serialize
/// - `Output`: The target serialized representation
///
/// # Examples
///
/// ```
/// use reinhardt_core::serializers::{Serializer, JsonSerializer};
/// use serde::{Serialize, Deserialize};
///
/// #[derive(Serialize, Deserialize)]
/// struct User { id: i64, name: String }
///
/// let user = User { id: 1, name: "Alice".to_string() };
/// let serializer = JsonSerializer::<User>::new();
/// let json = serializer.serialize(&user).unwrap();
/// assert!(json.contains("Alice"));
/// ```
pub trait Serializer {
	type Input;
	type Output;

	fn serialize(&self, input: &Self::Input) -> Result<Self::Output, SerializerError>;
	fn deserialize(&self, output: &Self::Output) -> Result<Self::Input, SerializerError>;
}

/// Errors that can occur during validation
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidatorError {
	/// Unique constraint violation
	UniqueViolation {
		field_name: String,
		value: String,
		message: String,
	},
	/// Unique together constraint violation
	UniqueTogetherViolation {
		field_names: Vec<String>,
		values: HashMap<String, String>,
		message: String,
	},
	/// Required field missing
	RequiredField { field_name: String, message: String },
	/// Field validation error
	FieldValidation {
		field_name: String,
		value: String,
		constraint: String,
		message: String,
	},
	/// Database error
	DatabaseError {
		message: String,
		source: Option<String>,
	},
	/// Custom validation error
	Custom { message: String },
}

impl std::fmt::Display for ValidatorError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			ValidatorError::UniqueViolation {
				field_name,
				value,
				message,
			} => write!(
				f,
				"Unique violation on field '{}' with value '{}': {}",
				field_name, value, message
			),
			ValidatorError::UniqueTogetherViolation {
				field_names,
				values,
				message,
			} => {
				// Format field_names as [username, email]
				let fields_str = format!("[{}]", field_names.join(", "));
				// Format values as (username=alice, email=alice@example.com)
				// Sort by key to ensure deterministic order
				let mut sorted_values: Vec<_> = values.iter().collect();
				sorted_values.sort_by_key(|(k, _)| *k);
				let values_str = sorted_values
					.into_iter()
					.map(|(k, v)| format!("{}={}", k, v))
					.collect::<Vec<_>>()
					.join(", ");
				write!(
					f,
					"Unique together violation on fields {} with values ({}): {}",
					fields_str, values_str, message
				)
			}
			ValidatorError::RequiredField {
				field_name,
				message,
			} => write!(f, "Required field '{}': {}", field_name, message),
			ValidatorError::FieldValidation {
				field_name,
				value,
				constraint,
				message,
			} => write!(
				f,
				"Field '{}' with value '{}' failed constraint '{}': {}",
				field_name, value, constraint, message
			),
			ValidatorError::DatabaseError { message, source } => {
				if let Some(src) = source {
					write!(f, "Database error: {} (source: {})", message, src)
				} else {
					write!(f, "Database error: {}", message)
				}
			}
			ValidatorError::Custom { message } => write!(f, "Validation error: {}", message),
		}
	}
}

impl std::error::Error for ValidatorError {}

impl ValidatorError {
	/// Returns the error message
	pub fn message(&self) -> &str {
		match self {
			ValidatorError::UniqueViolation { message, .. } => message,
			ValidatorError::UniqueTogetherViolation { message, .. } => message,
			ValidatorError::RequiredField { message, .. } => message,
			ValidatorError::FieldValidation { message, .. } => message,
			ValidatorError::DatabaseError { message, .. } => message,
			ValidatorError::Custom { message } => message,
		}
	}

	/// Returns the field names involved in this error
	pub fn field_names(&self) -> Vec<&str> {
		match self {
			ValidatorError::UniqueViolation { field_name, .. } => vec![field_name.as_str()],
			ValidatorError::UniqueTogetherViolation { field_names, .. } => {
				field_names.iter().map(|s| s.as_str()).collect()
			}
			ValidatorError::RequiredField { field_name, .. } => vec![field_name.as_str()],
			ValidatorError::FieldValidation { field_name, .. } => vec![field_name.as_str()],
			ValidatorError::DatabaseError { .. } => vec![],
			ValidatorError::Custom { .. } => vec![],
		}
	}

	/// Check if this is a uniqueness violation error
	pub fn is_uniqueness_violation(&self) -> bool {
		matches!(
			self,
			ValidatorError::UniqueViolation { .. } | ValidatorError::UniqueTogetherViolation { .. }
		)
	}

	/// Check if this is a database error
	pub fn is_database_error(&self) -> bool {
		matches!(self, ValidatorError::DatabaseError { .. })
	}
}

/// Errors that can occur during serialization
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SerializerError {
	/// Validation error
	Validation(ValidatorError),
	/// Serde serialization/deserialization error
	Serde { message: String },
	/// Other error
	Other { message: String },
}

impl std::fmt::Display for SerializerError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			SerializerError::Validation(e) => write!(f, "{}", e),
			SerializerError::Serde { message } => write!(f, "Serde error: {}", message),
			SerializerError::Other { message } => write!(f, "Serialization error: {}", message),
		}
	}
}

impl std::error::Error for SerializerError {
	fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
		match self {
			SerializerError::Validation(e) => Some(e),
			_ => None,
		}
	}
}

impl From<ValidatorError> for SerializerError {
	fn from(err: ValidatorError) -> Self {
		SerializerError::Validation(err)
	}
}

impl SerializerError {
	/// Create a new generic serializer error
	pub fn new(message: String) -> Self {
		SerializerError::Other { message }
	}

	/// Create a validation error from a ValidatorError
	pub fn validation(error: ValidatorError) -> Self {
		SerializerError::Validation(error)
	}

	/// Create a unique violation error
	pub fn unique_violation(field_name: String, value: String, message: String) -> Self {
		SerializerError::Validation(ValidatorError::UniqueViolation {
			field_name,
			value,
			message,
		})
	}

	/// Create a unique together violation error
	pub fn unique_together_violation(
		field_names: Vec<String>,
		values: HashMap<String, String>,
		message: String,
	) -> Self {
		SerializerError::Validation(ValidatorError::UniqueTogetherViolation {
			field_names,
			values,
			message,
		})
	}

	/// Create a required field error
	pub fn required_field(field_name: String, message: String) -> Self {
		SerializerError::Validation(ValidatorError::RequiredField {
			field_name,
			message,
		})
	}

	/// Create a field validation error
	pub fn field_validation(
		field_name: String,
		value: String,
		constraint: String,
		message: String,
	) -> Self {
		SerializerError::Validation(ValidatorError::FieldValidation {
			field_name,
			value,
			constraint,
			message,
		})
	}

	/// Create a database error
	pub fn database_error(message: String, source: Option<String>) -> Self {
		SerializerError::Validation(ValidatorError::DatabaseError { message, source })
	}

	/// Check if this is a validation error
	pub fn is_validation_error(&self) -> bool {
		matches!(self, SerializerError::Validation(_))
	}

	/// Returns the error message
	pub fn message(&self) -> String {
		match self {
			SerializerError::Validation(e) => e.message().to_string(),
			SerializerError::Serde { message } => message.clone(),
			SerializerError::Other { message } => message.clone(),
		}
	}

	/// Try to convert to ValidatorError if this is a validation error
	pub fn as_validator_error(&self) -> Option<&ValidatorError> {
		match self {
			SerializerError::Validation(e) => Some(e),
			_ => None,
		}
	}
}

// Integration with reinhardt_exception is moved to REST layer
// Base layer remains exception-agnostic

/// JSON serializer implementation
///
/// Provides JSON serialization/deserialization using serde_json.
///
/// # Examples
///
/// ```
/// use reinhardt_core::serializers::{Serializer, JsonSerializer};
/// use serde::{Serialize, Deserialize};
///
/// #[derive(Serialize, Deserialize, PartialEq, Debug)]
/// struct User { id: i64, name: String }
///
/// let user = User { id: 1, name: "Alice".to_string() };
/// let serializer = JsonSerializer::<User>::new();
///
/// let json = serializer.serialize(&user).unwrap();
/// let deserialized = serializer.deserialize(&json).unwrap();
/// assert_eq!(user.id, deserialized.id);
/// ```
#[derive(Debug, Clone)]
pub struct JsonSerializer<T> {
	_phantom: std::marker::PhantomData<T>,
}

impl<T> JsonSerializer<T> {
	/// Create a new JSON serializer
	pub fn new() -> Self {
		Self {
			_phantom: std::marker::PhantomData,
		}
	}
}

impl<T> Default for JsonSerializer<T> {
	fn default() -> Self {
		Self::new()
	}
}

impl<T> Serializer for JsonSerializer<T>
where
	T: Serialize + for<'de> Deserialize<'de>,
{
	type Input = T;
	type Output = String;

	fn serialize(&self, input: &Self::Input) -> Result<Self::Output, SerializerError> {
		serde_json::to_string(input).map_err(|e| SerializerError::Serde {
			message: format!("Serialization error: {}", e),
		})
	}

	fn deserialize(&self, output: &Self::Output) -> Result<Self::Input, SerializerError> {
		serde_json::from_str(output).map_err(|e| SerializerError::Serde {
			message: format!("Deserialization error: {}", e),
		})
	}
}

/// Deserializer trait for one-way deserialization
///
/// # Examples
///
/// ```
/// use reinhardt_core::serializers::Deserializer;
/// use serde::{Deserialize, Serialize};
///
/// struct JsonDeserializer;
///
/// impl Deserializer for JsonDeserializer {
///     type Input = String;
///     type Output = serde_json::Value;
///
///     fn deserialize(&self, input: &Self::Input) -> Result<Self::Output, reinhardt_core::serializers::SerializerError> {
///         serde_json::from_str(input).map_err(|e| reinhardt_core::serializers::SerializerError::Serde {
///             message: format!("Deserialization error: {}", e),
///         })
///     }
/// }
/// ```
pub trait Deserializer {
	type Input;
	type Output;

	fn deserialize(&self, input: &Self::Input) -> Result<Self::Output, SerializerError>;
}

#[cfg(test)]
mod tests {
	use super::*;

	#[derive(Serialize, Deserialize, PartialEq, Debug)]
	struct TestUser {
		id: i64,
		name: String,
	}

	#[test]
	fn test_json_serializer_roundtrip() {
		let user = TestUser {
			id: 1,
			name: "Alice".to_string(),
		};
		let serializer = JsonSerializer::<TestUser>::new();

		let json = serializer.serialize(&user).unwrap();
		let deserialized = serializer.deserialize(&json).unwrap();

		assert_eq!(user.id, deserialized.id);
		assert_eq!(user.name, deserialized.name);
	}

	#[test]
	fn test_json_serializer_serialize() {
		let user = TestUser {
			id: 1,
			name: "Alice".to_string(),
		};
		let serializer = JsonSerializer::<TestUser>::new();

		let json = serializer.serialize(&user).unwrap();
		assert!(json.contains("Alice"));
		assert!(json.contains("\"id\":1"));
	}

	#[test]
	fn test_json_serializer_deserialize() {
		let json = r#"{"id":1,"name":"Alice"}"#.to_string();
		let serializer = JsonSerializer::<TestUser>::new();

		let user = serializer.deserialize(&json).unwrap();
		assert_eq!(user.id, 1);
		assert_eq!(user.name, "Alice");
	}

	#[test]
	fn test_json_serializer_deserialize_error() {
		let invalid_json = r#"{"invalid"}"#.to_string();
		let serializer = JsonSerializer::<TestUser>::new();

		let result = serializer.deserialize(&invalid_json);
		assert!(result.is_err());
	}

	#[test]
	fn test_validator_error_display() {
		let err = ValidatorError::UniqueViolation {
			field_name: "email".to_string(),
			value: "test@example.com".to_string(),
			message: "Email already exists".to_string(),
		};
		assert!(err.to_string().contains("email"));
		assert!(err.to_string().contains("test@example.com"));
	}

	#[test]
	fn test_serializer_error_from_validator_error() {
		let validator_err = ValidatorError::Custom {
			message: "test error".to_string(),
		};
		let serializer_err: SerializerError = validator_err.into();

		match serializer_err {
			SerializerError::Validation(_) => {}
			_ => panic!("Expected Validation error"),
		}
	}
}
