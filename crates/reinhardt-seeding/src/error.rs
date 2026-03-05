//! Error types for the seeding module.
//!
//! This module defines the error types used throughout the reinhardt-seeding crate.

use thiserror::Error;

/// Errors that can occur during seeding operations.
#[derive(Debug, Error)]
pub enum SeedingError {
	/// Model was not found in the registry.
	#[error("Model not found: {0}")]
	ModelNotFound(String),

	/// Invalid fixture format detected.
	#[error("Invalid fixture format: {0}")]
	InvalidFormat(String),

	/// Error parsing fixture data.
	#[error("Parse error: {0}")]
	ParseError(String),

	/// Error serializing data to fixture format.
	#[error("Serialization error: {0}")]
	SerializationError(String),

	/// Database operation failed.
	#[error("Database error: {0}")]
	DatabaseError(String),

	/// I/O operation failed.
	#[error("IO error: {0}")]
	IoError(#[from] std::io::Error),

	/// Factory operation failed.
	#[error("Factory error: {0}")]
	FactoryError(String),

	/// Faker data generation failed.
	#[error("Faker error: {0}")]
	FakerError(String),

	/// Validation failed for a specific field.
	#[error("Validation error: {field}: {message}")]
	ValidationError {
		/// Field that failed validation.
		field: String,
		/// Validation error message.
		message: String,
	},

	/// JSON serialization/deserialization error.
	#[error("JSON error: {0}")]
	JsonError(#[from] serde_json::Error),

	/// YAML serialization/deserialization error (when yaml feature is enabled).
	#[cfg(feature = "yaml")]
	#[error("YAML error: {0}")]
	YamlError(#[from] serde_yaml::Error),

	/// Fixture file not found.
	#[error("Fixture file not found: {0}")]
	FileNotFound(String),

	/// Unsupported file extension.
	#[error("Unsupported file extension: {0}")]
	UnsupportedExtension(String),

	/// Transaction error.
	#[error("Transaction error: {0}")]
	TransactionError(String),

	/// Registry error.
	#[error("Registry error: {0}")]
	RegistryError(String),
}

/// Result type alias for seeding operations.
pub type SeedingResult<T> = Result<T, SeedingError>;

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	#[rstest]
	fn test_model_not_found_error() {
		let error = SeedingError::ModelNotFound("auth.User".to_string());
		assert_eq!(error.to_string(), "Model not found: auth.User");
	}

	#[rstest]
	fn test_validation_error() {
		let error = SeedingError::ValidationError {
			field: "email".to_string(),
			message: "invalid email format".to_string(),
		};
		assert_eq!(
			error.to_string(),
			"Validation error: email: invalid email format"
		);
	}

	#[rstest]
	fn test_io_error_from() {
		let io_error = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
		let seeding_error: SeedingError = io_error.into();
		assert!(matches!(seeding_error, SeedingError::IoError(_)));
	}

	#[rstest]
	fn test_json_error_from() {
		let json_str = "invalid json";
		let json_error: serde_json::Error =
			serde_json::from_str::<serde_json::Value>(json_str).unwrap_err();
		let seeding_error: SeedingError = json_error.into();
		assert!(matches!(seeding_error, SeedingError::JsonError(_)));
	}
}
