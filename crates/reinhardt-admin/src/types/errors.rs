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

/// Convert AdminError to reinhardt_core::exception::Error for seamless error handling
#[cfg(not(target_arch = "wasm32"))]
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
