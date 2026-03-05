//! NoSQL database error types
//!
//! This module provides a unified error type for all NoSQL database operations.

use std::fmt;

/// Result type for NoSQL operations
pub type Result<T> = std::result::Result<T, NoSQLError>;

/// Unified error type for NoSQL operations
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NoSQLError {
	/// Connection error
	ConnectionError(String),

	/// Query/operation execution error
	ExecutionError(String),

	/// Document/data not found
	NotFound(String),

	/// Serialization/deserialization error
	SerializationError(String),

	/// Invalid operation for the current backend
	InvalidOperation(String),

	/// Configuration error
	ConfigError(String),

	/// Timeout error
	Timeout(String),

	/// Authentication error
	AuthenticationError(String),

	/// Permission denied error
	PermissionDenied(String),

	/// Database-specific error (contains the original error message)
	DatabaseError(String),

	/// Feature not supported by this backend
	UnsupportedFeature(String),
}

impl fmt::Display for NoSQLError {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			NoSQLError::ConnectionError(msg) => write!(f, "Connection error: {}", msg),
			NoSQLError::ExecutionError(msg) => write!(f, "Execution error: {}", msg),
			NoSQLError::NotFound(msg) => write!(f, "Not found: {}", msg),
			NoSQLError::SerializationError(msg) => write!(f, "Serialization error: {}", msg),
			NoSQLError::InvalidOperation(msg) => write!(f, "Invalid operation: {}", msg),
			NoSQLError::ConfigError(msg) => write!(f, "Configuration error: {}", msg),
			NoSQLError::Timeout(msg) => write!(f, "Timeout: {}", msg),
			NoSQLError::AuthenticationError(msg) => write!(f, "Authentication error: {}", msg),
			NoSQLError::PermissionDenied(msg) => write!(f, "Permission denied: {}", msg),
			NoSQLError::DatabaseError(msg) => write!(f, "Database error: {}", msg),
			NoSQLError::UnsupportedFeature(msg) => write!(f, "Unsupported feature: {}", msg),
		}
	}
}

impl std::error::Error for NoSQLError {}

// Convenience conversion implementations for common error types
impl From<serde_json::Error> for NoSQLError {
	fn from(err: serde_json::Error) -> Self {
		NoSQLError::SerializationError(err.to_string())
	}
}

#[cfg(feature = "mongodb")]
impl From<mongodb::error::Error> for NoSQLError {
	fn from(err: mongodb::error::Error) -> Self {
		use mongodb::error::ErrorKind;

		match *err.kind {
			ErrorKind::Authentication { .. } => NoSQLError::AuthenticationError(err.to_string()),
			ErrorKind::InvalidArgument { .. } => NoSQLError::InvalidOperation(err.to_string()),
			ErrorKind::Io(_) => NoSQLError::ConnectionError(err.to_string()),
			_ => NoSQLError::DatabaseError(err.to_string()),
		}
	}
}

// In bson v3.x, both ser::Error and de::Error are type aliases for bson::error::Error
#[cfg(feature = "mongodb")]
impl From<bson::error::Error> for NoSQLError {
	fn from(err: bson::error::Error) -> Self {
		NoSQLError::SerializationError(err.to_string())
	}
}

#[cfg(feature = "redis")]
impl From<redis::RedisError> for NoSQLError {
	fn from(err: redis::RedisError) -> Self {
		if err.is_timeout() {
			NoSQLError::Timeout(err.to_string())
		} else if err.is_connection_refusal() || err.is_connection_dropped() {
			NoSQLError::ConnectionError(err.to_string())
		} else {
			NoSQLError::DatabaseError(err.to_string())
		}
	}
}

// ============================================================================
// ODM-specific error types
// ============================================================================

/// Result type for ODM operations.
pub type OdmResult<T> = std::result::Result<T, OdmError>;

/// Error type for ODM operations.
#[non_exhaustive]
#[derive(Debug)]
pub enum OdmError {
	/// Validation failed.
	Validation(ValidationError),

	/// MongoDB error.
	#[cfg(feature = "mongodb")]
	Mongo(mongodb::error::Error),

	/// Document not found.
	NotFound,

	/// Duplicate key error.
	DuplicateKey { field: String },

	/// Serialization error.
	Serialization(String),

	/// Backend operation error (database, connection, execution, etc.).
	BackendError(String),

	/// Deserialization error.
	#[cfg(feature = "mongodb")]
	Deserialization(bson::error::Error),
}

impl fmt::Display for OdmError {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			OdmError::Validation(err) => write!(f, "Validation failed: {}", err),
			#[cfg(feature = "mongodb")]
			OdmError::Mongo(err) => write!(f, "MongoDB error: {}", err),
			OdmError::NotFound => write!(f, "Document not found"),
			OdmError::DuplicateKey { field } => write!(f, "Duplicate key: {}", field),
			OdmError::Serialization(msg) => write!(f, "Serialization error: {}", msg),
			OdmError::BackendError(msg) => write!(f, "Backend error: {}", msg),
			#[cfg(feature = "mongodb")]
			OdmError::Deserialization(err) => write!(f, "Deserialization error: {}", err),
		}
	}
}

impl std::error::Error for OdmError {}

impl From<ValidationError> for OdmError {
	fn from(err: ValidationError) -> Self {
		OdmError::Validation(err)
	}
}

impl From<NoSQLError> for OdmError {
	fn from(err: NoSQLError) -> Self {
		match err {
			NoSQLError::SerializationError(msg) => OdmError::Serialization(msg),
			NoSQLError::NotFound(_) => OdmError::NotFound,
			other => OdmError::BackendError(other.to_string()),
		}
	}
}

#[cfg(feature = "mongodb")]
impl From<mongodb::error::Error> for OdmError {
	fn from(err: mongodb::error::Error) -> Self {
		convert_mongo_error(err)
	}
}

/// Convert a MongoDB error to an `OdmError`, detecting duplicate key violations (code 11000).
#[cfg(feature = "mongodb")]
pub(crate) fn convert_mongo_error(err: mongodb::error::Error) -> OdmError {
	if let mongodb::error::ErrorKind::Write(mongodb::error::WriteFailure::WriteError(
		ref write_error,
	)) = *err.kind
		&& write_error.code == 11000
	{
		let field = extract_duplicate_field(&write_error.message);
		return OdmError::DuplicateKey { field };
	}
	OdmError::Mongo(err)
}

/// Extract the field name from a MongoDB duplicate key error message.
///
/// MongoDB error messages typically contain patterns like:
/// `dup key: { email: "test@example.com" }` or
/// `index: collection.$email_1 dup key: ...`
#[cfg(feature = "mongodb")]
fn extract_duplicate_field(message: &str) -> String {
	// Try to extract from "dup key: { field: value }" pattern
	if let Some(start) = message.find("dup key: {") {
		let after_brace = &message[start + 11..];
		if let Some(colon_pos) = after_brace.find(':') {
			return after_brace[..colon_pos].trim().to_string();
		}
	}
	// Try to extract from index name pattern like "$field_1"
	if let Some(start) = message.find(".$") {
		let after_dollar = &message[start + 2..];
		if let Some(underscore_pos) = after_dollar.find('_') {
			return after_dollar[..underscore_pos].to_string();
		}
	}
	"unknown".to_string()
}

#[cfg(feature = "mongodb")]
impl From<bson::error::Error> for OdmError {
	fn from(err: bson::error::Error) -> Self {
		OdmError::Deserialization(err)
	}
}

/// Validation error type.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationError {
	/// Required field is missing or empty.
	Required(&'static str),

	/// Invalid email format.
	InvalidEmail,

	/// Invalid URL format.
	InvalidUrl,

	/// Value out of range.
	OutOfRange {
		field: &'static str,
		min: i64,
		max: i64,
	},

	/// Custom validation error.
	Custom(String),
}

impl fmt::Display for ValidationError {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			ValidationError::Required(field) => write!(f, "Field required: {}", field),
			ValidationError::InvalidEmail => write!(f, "Invalid email format"),
			ValidationError::InvalidUrl => write!(f, "Invalid URL format"),
			ValidationError::OutOfRange { field, min, max } => {
				write!(
					f,
					"Value out of range: {} must be between {} and {}",
					field, min, max
				)
			}
			ValidationError::Custom(msg) => write!(f, "Custom validation failed: {}", msg),
		}
	}
}

impl std::error::Error for ValidationError {}
