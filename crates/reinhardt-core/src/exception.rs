use thiserror::Error;

pub mod param_error;
pub use param_error::{ParamErrorContext, ParamType};

/// The main error type for the Reinhardt framework.
///
/// This enum represents all possible errors that can occur within the Reinhardt
/// ecosystem. Each variant corresponds to a specific error category with an
/// associated HTTP status code.
///
/// # Examples
///
/// ```
/// use reinhardt_core::exception::Error;
///
/// // Create an HTTP error
/// let http_err = Error::Http("Invalid request format".to_string());
/// assert_eq!(http_err.to_string(), "HTTP error: Invalid request format");
/// assert_eq!(http_err.status_code(), 400);
///
/// // Create a database error
/// let db_err = Error::Database("Connection timeout".to_string());
/// assert_eq!(db_err.status_code(), 500);
///
/// // Create an authentication error
/// let auth_err = Error::Authentication("Invalid token".to_string());
/// assert_eq!(auth_err.status_code(), 401);
/// ```
#[non_exhaustive]
#[derive(Error, Debug)]
pub enum Error {
	/// HTTP-related errors (status code: 400)
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::exception::Error;
	///
	/// let error = Error::Http("Malformed request body".to_string());
	/// assert_eq!(error.status_code(), 400);
	/// assert!(error.to_string().contains("HTTP error"));
	/// ```
	#[error("HTTP error: {0}")]
	Http(String),

	/// Database-related errors (status code: 500)
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::exception::Error;
	///
	/// let error = Error::Database("Query execution failed".to_string());
	/// assert_eq!(error.status_code(), 500);
	/// assert!(error.to_string().contains("Database error"));
	/// ```
	#[error("Database error: {0}")]
	Database(String),

	/// Serialization/deserialization errors (status code: 400)
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::exception::Error;
	///
	/// let error = Error::Serialization("Invalid JSON format".to_string());
	/// assert_eq!(error.status_code(), 400);
	/// assert!(error.to_string().contains("Serialization error"));
	/// ```
	#[error("Serialization error: {0}")]
	Serialization(String),

	/// Validation errors (status code: 400)
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::exception::Error;
	///
	/// let error = Error::Validation("Email format is invalid".to_string());
	/// assert_eq!(error.status_code(), 400);
	/// assert!(error.to_string().contains("Validation error"));
	/// ```
	#[error("Validation error: {0}")]
	Validation(String),

	/// Authentication errors (status code: 401)
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::exception::Error;
	///
	/// let error = Error::Authentication("Invalid credentials".to_string());
	/// assert_eq!(error.status_code(), 401);
	/// assert!(error.to_string().contains("Authentication error"));
	/// ```
	#[error("Authentication error: {0}")]
	Authentication(String),

	/// Authorization errors (status code: 403)
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::exception::Error;
	///
	/// let error = Error::Authorization("Insufficient permissions".to_string());
	/// assert_eq!(error.status_code(), 403);
	/// assert!(error.to_string().contains("Authorization error"));
	/// ```
	#[error("Authorization error: {0}")]
	Authorization(String),

	/// Resource not found errors (status code: 404)
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::exception::Error;
	///
	/// let error = Error::NotFound("User with ID 123 not found".to_string());
	/// assert_eq!(error.status_code(), 404);
	/// assert!(error.to_string().contains("Not found"));
	/// ```
	#[error("Not found: {0}")]
	NotFound(String),

	/// Template not found errors (status code: 404)
	#[error("Template not found: {0}")]
	TemplateNotFound(String),

	/// Method not allowed errors (status code: 405)
	///
	/// This error occurs when the HTTP method used is not supported for the
	/// requested resource, even though the resource exists.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::exception::Error;
	///
	/// let error = Error::MethodNotAllowed("Method PATCH not allowed for /api/articles/1".to_string());
	/// assert_eq!(error.status_code(), 405);
	/// assert!(error.to_string().contains("Method not allowed"));
	/// ```
	#[error("Method not allowed: {0}")]
	MethodNotAllowed(String),

	/// Conflict errors (status code: 409)
	///
	/// This error occurs when the request could not be completed due to a
	/// conflict with the current state of the resource. Commonly used for
	/// duplicate resources or conflicting operations.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::exception::Error;
	///
	/// let error = Error::Conflict("User with this email already exists".to_string());
	/// assert_eq!(error.status_code(), 409);
	/// assert!(error.to_string().contains("Conflict"));
	/// ```
	#[error("Conflict: {0}")]
	Conflict(String),

	/// Internal server errors (status code: 500)
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::exception::Error;
	///
	/// let error = Error::Internal("Unexpected server error".to_string());
	/// assert_eq!(error.status_code(), 500);
	/// assert!(error.to_string().contains("Internal server error"));
	/// ```
	#[error("Internal server error: {0}")]
	Internal(String),

	/// Configuration errors (status code: 500)
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::exception::Error;
	///
	/// let error = Error::ImproperlyConfigured("Missing DATABASE_URL".to_string());
	/// assert_eq!(error.status_code(), 500);
	/// assert!(error.to_string().contains("Improperly configured"));
	/// ```
	#[error("Improperly configured: {0}")]
	ImproperlyConfigured(String),

	/// Body already consumed error (status code: 400)
	///
	/// This error occurs when attempting to read a request body that has already
	/// been consumed.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::exception::Error;
	///
	/// let error = Error::BodyAlreadyConsumed;
	/// assert_eq!(error.status_code(), 400);
	/// assert_eq!(error.to_string(), "Body already consumed");
	/// ```
	#[error("Body already consumed")]
	BodyAlreadyConsumed,

	/// Parse errors (status code: 400)
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::exception::Error;
	///
	/// let error = Error::ParseError("Invalid integer value".to_string());
	/// assert_eq!(error.status_code(), 400);
	/// assert!(error.to_string().contains("Parse error"));
	/// ```
	#[error("Parse error: {0}")]
	ParseError(String),

	/// Missing Content-Type header
	#[error("Missing Content-Type header")]
	MissingContentType,

	/// Invalid page error for pagination (status code: 400)
	#[error("Invalid page: {0}")]
	InvalidPage(String),

	/// Invalid cursor error for pagination (status code: 400)
	#[error("Invalid cursor: {0}")]
	InvalidCursor(String),

	/// Invalid limit error for pagination (status code: 400)
	#[error("Invalid limit: {0}")]
	InvalidLimit(String),

	/// Missing parameter error for URL reverse (status code: 400)
	#[error("Missing parameter: {0}")]
	MissingParameter(String),

	/// Parameter validation errors with detailed context (status code: 400)
	///
	/// This variant provides structured error information for HTTP parameter
	/// extraction failures, including field names, expected types, and raw values.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::exception::{Error, ParamErrorContext, ParamType};
	///
	/// let ctx = ParamErrorContext::new(ParamType::Json, "missing field 'email'")
	///     .with_field("email")
	///     .with_expected_type::<String>();
	/// let error = Error::ParamValidation(Box::new(ctx));
	/// assert_eq!(error.status_code(), 400);
	/// ```
	#[error("{}", .0.format_error())]
	// Box wrapper to reduce enum size (clippy::result_large_err mitigation)
	// ParamErrorContext contains multiple String fields which make the enum large
	ParamValidation(Box<ParamErrorContext>),

	/// Wraps any other error type using `anyhow::Error` (status code: 500)
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::exception::Error;
	/// use anyhow::anyhow;
	///
	/// let other_error = anyhow!("Something went wrong");
	/// let error: Error = other_error.into();
	/// assert_eq!(error.status_code(), 500);
	/// ```
	#[error(transparent)]
	Other(#[from] anyhow::Error),
}

/// A convenient `Result` type alias using `reinhardt_core::exception::Error` as the error type.
///
/// This type alias is used throughout the Reinhardt framework to simplify
/// function signatures that return results.
///
/// # Examples
///
/// ```
/// use reinhardt_core::exception::{Error, Result};
///
/// fn validate_email(email: &str) -> Result<()> {
///     if email.contains('@') {
///         Ok(())
///     } else {
///         Err(Error::Validation("Email must contain @".to_string()))
///     }
/// }
///
/// // Successful validation
/// assert!(validate_email("user@example.com").is_ok());
///
/// // Failed validation
/// let result = validate_email("invalid-email");
/// assert!(result.is_err());
/// match result {
///     Err(Error::Validation(msg)) => assert!(msg.contains("@")),
///     _ => panic!("Expected validation error"),
/// }
/// ```
pub type Result<T> = std::result::Result<T, Error>;

/// Categorical classification of `Error` variants.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ErrorKind {
	Http,
	Database,
	Serialization,
	Validation,
	Authentication,
	Authorization,
	NotFound,
	MethodNotAllowed,
	Conflict,
	Internal,
	ImproperlyConfigured,
	BodyAlreadyConsumed,
	Parse,
	ParamValidation,
	Other,
}

impl Error {
	/// Returns the HTTP status code associated with this error.
	///
	/// Each error variant maps to an appropriate HTTP status code that can be
	/// used when converting errors to HTTP responses.
	///
	/// # Status Code Mapping
	///
	/// - `Http`, `Serialization`, `Validation`, `BodyAlreadyConsumed`, `ParseError`: 400 (Bad Request)
	/// - `Authentication`: 401 (Unauthorized)
	/// - `Authorization`: 403 (Forbidden)
	/// - `NotFound`, `TemplateNotFound`: 404 (Not Found)
	/// - `MethodNotAllowed`: 405 (Method Not Allowed)
	/// - `Conflict`: 409 (Conflict)
	/// - `Database`, `Internal`, `ImproperlyConfigured`, `Other`: 500 (Internal Server Error)
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::exception::Error;
	///
	/// // Client errors (4xx)
	/// assert_eq!(Error::Http("Bad request".to_string()).status_code(), 400);
	/// assert_eq!(Error::Validation("Invalid input".to_string()).status_code(), 400);
	/// assert_eq!(Error::Authentication("No token".to_string()).status_code(), 401);
	/// assert_eq!(Error::Authorization("No access".to_string()).status_code(), 403);
	/// assert_eq!(Error::NotFound("Resource missing".to_string()).status_code(), 404);
	///
	/// // Server errors (5xx)
	/// assert_eq!(Error::Database("Connection failed".to_string()).status_code(), 500);
	/// assert_eq!(Error::Internal("Crash".to_string()).status_code(), 500);
	/// assert_eq!(Error::ImproperlyConfigured("Bad config".to_string()).status_code(), 500);
	///
	/// // Edge cases
	/// assert_eq!(Error::BodyAlreadyConsumed.status_code(), 400);
	/// assert_eq!(Error::ParseError("Invalid data".to_string()).status_code(), 400);
	/// ```
	///
	/// # Using with anyhow errors
	///
	/// ```
	/// use reinhardt_core::exception::Error;
	/// use anyhow::anyhow;
	///
	/// let anyhow_error = anyhow!("Unexpected error");
	/// let error: Error = anyhow_error.into();
	/// assert_eq!(error.status_code(), 500);
	/// ```
	pub fn status_code(&self) -> u16 {
		match self {
			Error::Http(_) => 400,
			Error::Database(_) => 500,
			Error::Serialization(_) => 400,
			Error::Validation(_) => 400,
			Error::Authentication(_) => 401,
			Error::Authorization(_) => 403,
			Error::NotFound(_) => 404,
			Error::TemplateNotFound(_) => 404,
			Error::MethodNotAllowed(_) => 405,
			Error::Conflict(_) => 409,
			Error::Internal(_) => 500,
			Error::ImproperlyConfigured(_) => 500,
			Error::BodyAlreadyConsumed => 400,
			Error::ParseError(_) => 400,
			Error::MissingContentType => 400,
			Error::InvalidPage(_) => 400,
			Error::InvalidCursor(_) => 400,
			Error::InvalidLimit(_) => 400,
			Error::MissingParameter(_) => 400,
			Error::ParamValidation(_) => 400,
			Error::Other(_) => 500,
		}
	}

	/// Returns the categorical `ErrorKind` for this error.
	pub fn kind(&self) -> ErrorKind {
		match self {
			Error::Http(_) => ErrorKind::Http,
			Error::Database(_) => ErrorKind::Database,
			Error::Serialization(_) => ErrorKind::Serialization,
			Error::Validation(_) => ErrorKind::Validation,
			Error::Authentication(_) => ErrorKind::Authentication,
			Error::Authorization(_) => ErrorKind::Authorization,
			Error::NotFound(_) => ErrorKind::NotFound,
			Error::TemplateNotFound(_) => ErrorKind::NotFound,
			Error::MethodNotAllowed(_) => ErrorKind::MethodNotAllowed,
			Error::Conflict(_) => ErrorKind::Conflict,
			Error::Internal(_) => ErrorKind::Internal,
			Error::ImproperlyConfigured(_) => ErrorKind::ImproperlyConfigured,
			Error::BodyAlreadyConsumed => ErrorKind::BodyAlreadyConsumed,
			Error::ParseError(_) => ErrorKind::Parse,
			Error::MissingContentType => ErrorKind::Http,
			Error::InvalidPage(_) => ErrorKind::Validation,
			Error::InvalidCursor(_) => ErrorKind::Validation,
			Error::InvalidLimit(_) => ErrorKind::Validation,
			Error::MissingParameter(_) => ErrorKind::Validation,
			Error::ParamValidation(_) => ErrorKind::ParamValidation,
			Error::Other(_) => ErrorKind::Other,
		}
	}
}

// Common conversions to the unified Error without introducing cross-crate deps.
impl From<serde_json::Error> for Error {
	fn from(err: serde_json::Error) -> Self {
		Error::Serialization(err.to_string())
	}
}

impl From<std::io::Error> for Error {
	fn from(err: std::io::Error) -> Self {
		Error::Internal(format!("IO error: {}", err))
	}
}

impl From<http::Error> for Error {
	fn from(err: http::Error) -> Self {
		Error::Http(err.to_string())
	}
}

impl From<String> for Error {
	fn from(msg: String) -> Self {
		Error::Internal(msg)
	}
}

impl From<&str> for Error {
	fn from(msg: &str) -> Self {
		Error::Internal(msg.to_string())
	}
}

impl From<validator::ValidationErrors> for Error {
	fn from(err: validator::ValidationErrors) -> Self {
		Error::Validation(format!("Validation failed: {}", err))
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_error_kind_mapping() {
		// HTTP errors
		assert_eq!(Error::Http("test".to_string()).kind(), ErrorKind::Http);
		assert_eq!(Error::MissingContentType.kind(), ErrorKind::Http);

		// Database errors
		assert_eq!(
			Error::Database("test".to_string()).kind(),
			ErrorKind::Database
		);

		// Serialization errors
		assert_eq!(
			Error::Serialization("test".to_string()).kind(),
			ErrorKind::Serialization
		);

		// Validation errors
		assert_eq!(
			Error::Validation("test".to_string()).kind(),
			ErrorKind::Validation
		);
		assert_eq!(
			Error::InvalidPage("test".to_string()).kind(),
			ErrorKind::Validation
		);
		assert_eq!(
			Error::InvalidCursor("test".to_string()).kind(),
			ErrorKind::Validation
		);
		assert_eq!(
			Error::InvalidLimit("test".to_string()).kind(),
			ErrorKind::Validation
		);
		assert_eq!(
			Error::MissingParameter("test".to_string()).kind(),
			ErrorKind::Validation
		);

		// Authentication errors
		assert_eq!(
			Error::Authentication("test".to_string()).kind(),
			ErrorKind::Authentication
		);

		// Authorization errors
		assert_eq!(
			Error::Authorization("test".to_string()).kind(),
			ErrorKind::Authorization
		);

		// NotFound errors
		assert_eq!(
			Error::NotFound("test".to_string()).kind(),
			ErrorKind::NotFound
		);
		assert_eq!(
			Error::TemplateNotFound("test".to_string()).kind(),
			ErrorKind::NotFound
		);

		// MethodNotAllowed errors
		assert_eq!(
			Error::MethodNotAllowed("test".to_string()).kind(),
			ErrorKind::MethodNotAllowed
		);

		// Internal errors
		assert_eq!(
			Error::Internal("test".to_string()).kind(),
			ErrorKind::Internal
		);

		// ImproperlyConfigured errors
		assert_eq!(
			Error::ImproperlyConfigured("test".to_string()).kind(),
			ErrorKind::ImproperlyConfigured
		);

		// BodyAlreadyConsumed errors
		assert_eq!(
			Error::BodyAlreadyConsumed.kind(),
			ErrorKind::BodyAlreadyConsumed
		);

		// Parse errors
		assert_eq!(
			Error::ParseError("test".to_string()).kind(),
			ErrorKind::Parse
		);

		// Other errors
		assert_eq!(
			Error::Other(anyhow::anyhow!("test")).kind(),
			ErrorKind::Other
		);
	}

	#[test]
	fn test_from_serde_json_error() {
		let json_error = serde_json::from_str::<i32>("invalid").unwrap_err();
		let error: Error = json_error.into();

		assert_eq!(error.status_code(), 400);
		assert_eq!(error.kind(), ErrorKind::Serialization);
		assert!(error.to_string().contains("Serialization error"));
	}

	#[test]
	fn test_from_io_error() {
		let io_error = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
		let error: Error = io_error.into();

		assert_eq!(error.status_code(), 500);
		assert_eq!(error.kind(), ErrorKind::Internal);
		assert!(error.to_string().contains("IO error"));
	}

	#[test]
	fn test_status_codes_comprehensive() {
		// 400 errors
		assert_eq!(Error::Http("test".to_string()).status_code(), 400);
		assert_eq!(Error::Serialization("test".to_string()).status_code(), 400);
		assert_eq!(Error::Validation("test".to_string()).status_code(), 400);
		assert_eq!(Error::BodyAlreadyConsumed.status_code(), 400);
		assert_eq!(Error::ParseError("test".to_string()).status_code(), 400);
		assert_eq!(Error::MissingContentType.status_code(), 400);
		assert_eq!(Error::InvalidPage("test".to_string()).status_code(), 400);
		assert_eq!(Error::InvalidCursor("test".to_string()).status_code(), 400);
		assert_eq!(Error::InvalidLimit("test".to_string()).status_code(), 400);
		assert_eq!(
			Error::MissingParameter("test".to_string()).status_code(),
			400
		);

		// 401 error
		assert_eq!(Error::Authentication("test".to_string()).status_code(), 401);

		// 403 error
		assert_eq!(Error::Authorization("test".to_string()).status_code(), 403);

		// 404 errors
		assert_eq!(Error::NotFound("test".to_string()).status_code(), 404);
		assert_eq!(
			Error::TemplateNotFound("test".to_string()).status_code(),
			404
		);

		// 405 error
		assert_eq!(
			Error::MethodNotAllowed("test".to_string()).status_code(),
			405
		);

		// 500 errors
		assert_eq!(Error::Database("test".to_string()).status_code(), 500);
		assert_eq!(Error::Internal("test".to_string()).status_code(), 500);
		assert_eq!(
			Error::ImproperlyConfigured("test".to_string()).status_code(),
			500
		);
		assert_eq!(Error::Other(anyhow::anyhow!("test")).status_code(), 500);
	}

	#[test]
	fn test_template_not_found_error() {
		let error = Error::TemplateNotFound("index.html".to_string());
		assert_eq!(error.status_code(), 404);
		assert_eq!(error.kind(), ErrorKind::NotFound);
		assert!(error.to_string().contains("Template not found"));
		assert!(error.to_string().contains("index.html"));
	}

	#[test]
	fn test_pagination_errors() {
		let page_error = Error::InvalidPage("page must be positive".to_string());
		assert_eq!(page_error.status_code(), 400);
		assert_eq!(page_error.kind(), ErrorKind::Validation);

		let cursor_error = Error::InvalidCursor("invalid base64".to_string());
		assert_eq!(cursor_error.status_code(), 400);
		assert_eq!(cursor_error.kind(), ErrorKind::Validation);

		let limit_error = Error::InvalidLimit("limit too large".to_string());
		assert_eq!(limit_error.status_code(), 400);
		assert_eq!(limit_error.kind(), ErrorKind::Validation);
	}

	#[test]
	fn test_missing_parameter_error() {
		let error = Error::MissingParameter("user_id".to_string());
		assert_eq!(error.status_code(), 400);
		assert_eq!(error.kind(), ErrorKind::Validation);
		assert!(error.to_string().contains("Missing parameter"));
		assert!(error.to_string().contains("user_id"));
	}
}
