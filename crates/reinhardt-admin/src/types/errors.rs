//! Error types for admin panel

use thiserror::Error;

/// Admin panel error type
#[derive(Debug, Error)]
pub enum AdminError {
	/// Model not registered with admin
	#[error("Model '{0}' is not registered with admin")]
	ModelNotRegistered(String),

	/// Permission denied
	#[error("Permission denied: {0}")]
	PermissionDenied(String),

	/// Invalid action
	#[error("Invalid action: {0}")]
	InvalidAction(String),

	/// Database error
	#[error("Database error: {0}")]
	DatabaseError(String),

	/// Validation error
	#[error("Validation error: {0}")]
	ValidationError(String),

	/// Template rendering error
	#[error("Template rendering error: {0}")]
	TemplateError(String),
}

/// Result type for admin panel operations
pub type AdminResult<T> = Result<T, AdminError>;

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	#[rstest]
	#[case::model_not_registered(
		AdminError::ModelNotRegistered("User".to_string()),
		"Model 'User' is not registered with admin"
	)]
	#[case::permission_denied(
		AdminError::PermissionDenied("insufficient role".to_string()),
		"Permission denied: insufficient role"
	)]
	#[case::invalid_action(
		AdminError::InvalidAction("bulk_delete".to_string()),
		"Invalid action: bulk_delete"
	)]
	#[case::database_error(
		AdminError::DatabaseError("connection timeout".to_string()),
		"Database error: connection timeout"
	)]
	#[case::validation_error(
		AdminError::ValidationError("email is required".to_string()),
		"Validation error: email is required"
	)]
	#[case::template_error(
		AdminError::TemplateError("missing variable".to_string()),
		"Template rendering error: missing variable"
	)]
	fn test_admin_error_display_all_variants(#[case] error: AdminError, #[case] expected: &str) {
		// Act
		let display = error.to_string();

		// Assert
		assert_eq!(display, expected);
	}

	#[cfg(server)]
	#[rstest]
	#[case::model_not_registered(
		AdminError::ModelNotRegistered("Article".to_string()),
		"Article"
	)]
	#[case::permission_denied(
		AdminError::PermissionDenied("no access".to_string()),
		"no access"
	)]
	#[case::invalid_action(
		AdminError::InvalidAction("export".to_string()),
		"export"
	)]
	#[case::database_error(
		AdminError::DatabaseError("deadlock".to_string()),
		"deadlock"
	)]
	#[case::validation_error(
		AdminError::ValidationError("too long".to_string()),
		"too long"
	)]
	#[case::template_error(
		AdminError::TemplateError("syntax error".to_string()),
		"syntax error"
	)]
	fn test_admin_error_to_core_error_mapping(
		#[case] admin_error: AdminError,
		#[case] expected_msg: &str,
	) {
		// Act
		let core_error: reinhardt_core::exception::Error = admin_error.into();

		// Assert
		let display = core_error.to_string();
		assert!(
			display.contains(expected_msg),
			"Core error display '{}' should contain '{}'",
			display,
			expected_msg
		);
	}

	#[rstest]
	fn test_admin_error_model_not_registered_includes_model_name() {
		// Arrange
		let model_name = "CustomWidget";
		let error = AdminError::ModelNotRegistered(model_name.to_string());

		// Act
		let display = error.to_string();

		// Assert
		assert!(
			display.contains(model_name),
			"Error message '{}' should contain model name '{}'",
			display,
			model_name
		);
	}

	#[rstest]
	fn test_admin_error_permission_denied_includes_reason() {
		// Arrange
		let reason = "user lacks admin privileges";
		let error = AdminError::PermissionDenied(reason.to_string());

		// Act
		let display = error.to_string();

		// Assert
		assert!(
			display.contains(reason),
			"Error message '{}' should contain reason '{}'",
			display,
			reason
		);
	}

	#[rstest]
	#[case::model_not_registered(AdminError::ModelNotRegistered("M".to_string()))]
	#[case::permission_denied(AdminError::PermissionDenied("P".to_string()))]
	#[case::invalid_action(AdminError::InvalidAction("A".to_string()))]
	#[case::database_error(AdminError::DatabaseError("D".to_string()))]
	#[case::validation_error(AdminError::ValidationError("V".to_string()))]
	#[case::template_error(AdminError::TemplateError("T".to_string()))]
	fn test_admin_error_debug_format_differs_from_display(#[case] error: AdminError) {
		// Act
		let debug_output = format!("{:?}", error);
		let display_output = format!("{}", error);

		// Assert
		assert_ne!(
			debug_output, display_output,
			"Debug '{debug_output}' and Display '{display_output}' should differ"
		);
	}
}

/// Convert AdminError to reinhardt_core::exception::Error for seamless error handling
#[cfg(server)]
impl From<AdminError> for reinhardt_core::exception::Error {
	fn from(err: AdminError) -> Self {
		match err {
			AdminError::ModelNotRegistered(msg) => reinhardt_core::exception::Error::NotFound(msg),
			AdminError::PermissionDenied(msg) => {
				reinhardt_core::exception::Error::Authorization(msg)
			}
			AdminError::InvalidAction(msg) => reinhardt_core::exception::Error::Http(msg),
			AdminError::DatabaseError(msg) => reinhardt_core::exception::Error::Database(msg),
			AdminError::ValidationError(msg) => reinhardt_core::exception::Error::Validation(msg),
			AdminError::TemplateError(msg) => {
				reinhardt_core::exception::Error::Other(anyhow::anyhow!(msg))
			}
		}
	}
}
