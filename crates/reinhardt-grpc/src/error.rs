//! gRPC error types with safe error response handling
//!
//! This module provides error types for gRPC services with built-in
//! sanitization to prevent information leakage through error messages.
//!
//! # Security
//!
//! Internal error details (stack traces, file paths, type names) are
//! never exposed to clients. The [`ErrorSanitizer`] controls whether
//! error responses include detailed or generic messages.
//!
//! In production mode (the default), internal errors return a generic
//! "Internal server error" message. In debug mode, the original error
//! message is preserved for development convenience.

use std::borrow::Cow;
use thiserror::Error;

/// Pattern fragments that indicate an error message contains internal details.
///
/// Messages matching these patterns are sanitized in production mode.
const SENSITIVE_PATTERNS: &[&str] = &[
	// File system paths
	"/home/",
	"/usr/",
	"/tmp/",
	"/var/",
	"\\Users\\",
	"C:\\",
	// Stack trace indicators
	"at line",
	"panicked at",
	"stack backtrace",
	"thread '",
	// Rust internal details
	"src/",
	".rs:",
	"RUST_BACKTRACE",
	// Type information
	"core::result::Result",
	"std::io::Error",
	"tokio::",
	"hyper::",
	"tonic::",
];

#[derive(Debug, Error)]
pub enum GrpcError {
	#[error("Connection error: {0}")]
	Connection(String),

	#[error("Service error: {0}")]
	Service(String),

	#[error("Not found: {0}")]
	NotFound(String),

	#[error("Invalid argument: {0}")]
	InvalidArgument(String),

	#[error("Internal error: {0}")]
	Internal(String),
}

pub type GrpcResult<T> = Result<T, GrpcError>;

impl From<tonic::Status> for GrpcError {
	fn from(status: tonic::Status) -> Self {
		match status.code() {
			tonic::Code::NotFound => GrpcError::NotFound(status.message().to_string()),
			tonic::Code::InvalidArgument => {
				GrpcError::InvalidArgument(status.message().to_string())
			}
			tonic::Code::Unavailable => GrpcError::Connection(status.message().to_string()),
			_ => GrpcError::Internal(status.message().to_string()),
		}
	}
}

/// Controls whether error responses include detailed or generic messages.
///
/// In production mode, internal error details are replaced with generic
/// messages to prevent information leakage. In debug mode, the original
/// error messages are preserved for easier development debugging.
///
/// # Example
///
/// ```rust
/// use reinhardt_grpc::error::ErrorSanitizer;
///
/// // Production mode (default): sanitizes internal details
/// let sanitizer = ErrorSanitizer::production();
/// assert!(!sanitizer.is_debug());
///
/// // Debug mode: preserves error details
/// let sanitizer = ErrorSanitizer::debug();
/// assert!(sanitizer.is_debug());
/// ```
#[derive(Debug, Clone)]
pub struct ErrorSanitizer {
	debug_mode: bool,
}

impl ErrorSanitizer {
	/// Create a sanitizer in production mode (default).
	///
	/// Internal error details are replaced with generic messages.
	pub fn production() -> Self {
		Self { debug_mode: false }
	}

	/// Create a sanitizer in debug mode.
	///
	/// Error details are preserved in responses for development use.
	pub fn debug() -> Self {
		Self { debug_mode: true }
	}

	/// Returns whether the sanitizer is in debug mode.
	pub fn is_debug(&self) -> bool {
		self.debug_mode
	}

	/// Convert a `GrpcError` into a `tonic::Status`, sanitizing internal
	/// details in production mode.
	///
	/// - `NotFound` and `InvalidArgument` errors are always passed through
	///   (they contain client-relevant information).
	/// - `Connection`, `Service`, and `Internal` errors have their messages
	///   sanitized in production mode.
	pub fn to_status(&self, error: &GrpcError) -> tonic::Status {
		match error {
			// Client errors: safe to expose details
			GrpcError::NotFound(msg) => tonic::Status::not_found(msg),
			GrpcError::InvalidArgument(msg) => tonic::Status::invalid_argument(msg),

			// Internal errors: sanitize in production
			GrpcError::Connection(msg) => {
				if self.debug_mode {
					tonic::Status::unavailable(msg)
				} else {
					tracing::error!(original_message = %msg, "gRPC connection error");
					tonic::Status::unavailable("Service temporarily unavailable")
				}
			}
			GrpcError::Service(msg) => {
				if self.debug_mode {
					tonic::Status::internal(msg)
				} else {
					tracing::error!(original_message = %msg, "gRPC service error");
					tonic::Status::internal("Internal server error")
				}
			}
			GrpcError::Internal(msg) => {
				if self.debug_mode {
					tonic::Status::internal(msg)
				} else {
					tracing::error!(original_message = %msg, "gRPC internal error");
					tonic::Status::internal("Internal server error")
				}
			}
		}
	}

	/// Sanitize an arbitrary error message, removing sensitive patterns.
	///
	/// If the message contains patterns that look like internal details
	/// (file paths, stack traces, type names), it is replaced with a
	/// generic message in production mode.
	///
	/// Returns `Cow::Borrowed` for static replacement messages to avoid
	/// heap allocations on error paths.
	pub fn sanitize_message<'a>(&self, message: &'a str) -> Cow<'a, str> {
		if self.debug_mode {
			return Cow::Borrowed(message);
		}

		if contains_sensitive_pattern(message) {
			tracing::warn!(
				original_message = %message,
				"Sanitized error message containing sensitive information"
			);
			Cow::Borrowed("Internal server error")
		} else {
			Cow::Borrowed(message)
		}
	}
}

impl Default for ErrorSanitizer {
	fn default() -> Self {
		Self::production()
	}
}

/// Check if a message contains any sensitive patterns that should be sanitized.
fn contains_sensitive_pattern(message: &str) -> bool {
	SENSITIVE_PATTERNS
		.iter()
		.any(|pattern| message.contains(pattern))
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	// -- Existing tests migrated to rstest --

	#[rstest]
	fn error_display_connection() {
		// Arrange
		let err = GrpcError::Connection("test error".to_string());

		// Act
		let display = err.to_string();

		// Assert
		assert_eq!(display, "Connection error: test error");
	}

	#[rstest]
	fn error_display_not_found() {
		// Arrange
		let err = GrpcError::NotFound("item".to_string());

		// Act
		let display = err.to_string();

		// Assert
		assert_eq!(display, "Not found: item");
	}

	#[rstest]
	fn from_tonic_status_not_found() {
		// Arrange
		let status = tonic::Status::not_found("User not found");

		// Act
		let error = GrpcError::from(status);

		// Assert
		assert!(matches!(error, GrpcError::NotFound(ref msg) if msg == "User not found"));
	}

	#[rstest]
	fn from_tonic_status_invalid_argument() {
		// Arrange
		let status = tonic::Status::invalid_argument("Invalid ID");

		// Act
		let error = GrpcError::from(status);

		// Assert
		assert!(matches!(error, GrpcError::InvalidArgument(ref msg) if msg == "Invalid ID"));
	}

	#[rstest]
	fn from_tonic_status_unavailable() {
		// Arrange
		let status = tonic::Status::unavailable("Service unavailable");

		// Act
		let error = GrpcError::from(status);

		// Assert
		assert!(matches!(error, GrpcError::Connection(_)));
	}

	// -- New security tests --

	#[rstest]
	fn sanitizer_production_mode_is_default() {
		// Arrange & Act
		let sanitizer = ErrorSanitizer::default();

		// Assert
		assert!(!sanitizer.is_debug());
	}

	#[rstest]
	fn sanitizer_debug_mode() {
		// Arrange & Act
		let sanitizer = ErrorSanitizer::debug();

		// Assert
		assert!(sanitizer.is_debug());
	}

	#[rstest]
	fn sanitizer_production_hides_internal_error() {
		// Arrange
		let sanitizer = ErrorSanitizer::production();
		let error = GrpcError::Internal("panicked at src/main.rs:42".to_string());

		// Act
		let status = sanitizer.to_status(&error);

		// Assert
		assert_eq!(status.code(), tonic::Code::Internal);
		assert_eq!(status.message(), "Internal server error");
	}

	#[rstest]
	fn sanitizer_production_hides_connection_error() {
		// Arrange
		let sanitizer = ErrorSanitizer::production();
		let error = GrpcError::Connection("TCP error on /var/run/socket".to_string());

		// Act
		let status = sanitizer.to_status(&error);

		// Assert
		assert_eq!(status.code(), tonic::Code::Unavailable);
		assert_eq!(status.message(), "Service temporarily unavailable");
	}

	#[rstest]
	fn sanitizer_production_hides_service_error() {
		// Arrange
		let sanitizer = ErrorSanitizer::production();
		let error = GrpcError::Service("hyper::Error: connection reset".to_string());

		// Act
		let status = sanitizer.to_status(&error);

		// Assert
		assert_eq!(status.code(), tonic::Code::Internal);
		assert_eq!(status.message(), "Internal server error");
	}

	#[rstest]
	fn sanitizer_production_preserves_not_found() {
		// Arrange
		let sanitizer = ErrorSanitizer::production();
		let error = GrpcError::NotFound("User with ID 123 not found".to_string());

		// Act
		let status = sanitizer.to_status(&error);

		// Assert
		assert_eq!(status.code(), tonic::Code::NotFound);
		assert_eq!(status.message(), "User with ID 123 not found");
	}

	#[rstest]
	fn sanitizer_production_preserves_invalid_argument() {
		// Arrange
		let sanitizer = ErrorSanitizer::production();
		let error = GrpcError::InvalidArgument("Field 'name' is required".to_string());

		// Act
		let status = sanitizer.to_status(&error);

		// Assert
		assert_eq!(status.code(), tonic::Code::InvalidArgument);
		assert_eq!(status.message(), "Field 'name' is required");
	}

	#[rstest]
	fn sanitizer_debug_preserves_all_details() {
		// Arrange
		let sanitizer = ErrorSanitizer::debug();
		let original_msg = "panicked at src/main.rs:42: stack backtrace";
		let error = GrpcError::Internal(original_msg.to_string());

		// Act
		let status = sanitizer.to_status(&error);

		// Assert
		assert_eq!(status.code(), tonic::Code::Internal);
		assert_eq!(status.message(), original_msg);
	}

	#[rstest]
	#[case("/home/user/.config/secret.key", true)]
	#[case("/usr/local/bin/service", true)]
	#[case("/tmp/crash-dump-12345", true)]
	#[case("panicked at src/handler.rs:99", true)]
	#[case("stack backtrace:", true)]
	#[case("core::result::Result<T, E>", true)]
	#[case("hyper::Error: broken pipe", true)]
	#[case("User not found", false)]
	#[case("Invalid argument: name is required", false)]
	#[case("Query timeout exceeded", false)]
	fn contains_sensitive_pattern_detection(#[case] message: &str, #[case] expected: bool) {
		// Act
		let result = contains_sensitive_pattern(message);

		// Assert
		assert_eq!(result, expected, "Pattern detection failed for: {message}");
	}

	#[rstest]
	fn sanitize_message_production_removes_paths() {
		// Arrange
		let sanitizer = ErrorSanitizer::production();

		// Act
		let result = sanitizer.sanitize_message("Error at /home/user/app/src/main.rs:42");

		// Assert
		assert_eq!(result, "Internal server error");
	}

	#[rstest]
	fn sanitize_message_production_preserves_safe_messages() {
		// Arrange
		let sanitizer = ErrorSanitizer::production();

		// Act
		let result = sanitizer.sanitize_message("User not found");

		// Assert
		assert_eq!(result, "User not found");
	}

	#[rstest]
	fn sanitize_message_debug_preserves_all() {
		// Arrange
		let sanitizer = ErrorSanitizer::debug();

		// Act
		let result = sanitizer.sanitize_message("Error at /home/user/app/src/main.rs:42");

		// Assert
		assert_eq!(result, "Error at /home/user/app/src/main.rs:42");
	}

	#[rstest]
	fn sanitizer_clone_preserves_mode() {
		// Arrange
		let sanitizer = ErrorSanitizer::debug();

		// Act
		let cloned = sanitizer.clone();

		// Assert
		assert_eq!(cloned.is_debug(), sanitizer.is_debug());
	}
}
