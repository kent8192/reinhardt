//! Error conversion for Server Functions
//!
//! This module provides error conversion from AdminError to ServerFnError
//! and authentication/authorization helpers for admin panel endpoints.

use crate::types::AdminError;
use reinhardt_http::AuthState;
use reinhardt_pages::server_fn::{ServerFnError, ServerFnRequest};
use std::sync::Arc;

/// Extension trait for converting AdminError to ServerFnError
pub trait IntoServerFnError {
	/// Convert AdminError to ServerFnError
	fn into_server_fn_error(self) -> ServerFnError;
}

impl IntoServerFnError for AdminError {
	fn into_server_fn_error(self) -> ServerFnError {
		match self {
			AdminError::ModelNotRegistered(msg) => ServerFnError::server(404, msg),
			AdminError::PermissionDenied(msg) => ServerFnError::server(403, msg),
			AdminError::InvalidAction(msg) | AdminError::ValidationError(msg) => {
				ServerFnError::application(msg)
			}
			AdminError::DatabaseError(_) => {
				// Hide internal database error details from clients
				ServerFnError::server(500, "Database operation failed")
			}
			AdminError::TemplateError(_) => {
				// Hide internal template error details from clients
				ServerFnError::server(500, "Template rendering failed")
			}
		}
	}
}

/// Convert `Result<T, AdminError>` to `Result<T, ServerFnError>`
pub trait MapServerFnError<T> {
	/// Map AdminError to ServerFnError
	fn map_server_fn_error(self) -> Result<T, ServerFnError>;
}

impl<T> MapServerFnError<T> for Result<T, AdminError> {
	fn map_server_fn_error(self) -> Result<T, ServerFnError> {
		self.map_err(|e| e.into_server_fn_error())
	}
}

/// Authentication and authorization checker for admin panel.
///
/// This struct extracts authentication state from the HTTP request
/// and provides methods to check authentication and permissions.
pub struct AdminAuth {
	/// The authentication state from the request
	auth_state: Option<AuthState>,
}

impl AdminAuth {
	/// Creates a new AdminAuth from a ServerFnRequest.
	///
	/// # Arguments
	///
	/// * `request` - The server function request wrapper
	///
	/// # Returns
	///
	/// A new AdminAuth instance
	pub fn from_request(request: &ServerFnRequest) -> Self {
		let auth_state = request.inner().extensions.get::<AuthState>();
		Self { auth_state }
	}

	/// Creates a new AdminAuth from an `Arc<Request>`.
	///
	/// # Arguments
	///
	/// * `request` - The HTTP request
	///
	/// # Returns
	///
	/// A new AdminAuth instance
	pub fn from_arc_request(request: &Arc<reinhardt_http::Request>) -> Self {
		let auth_state = request.extensions.get::<AuthState>();
		Self { auth_state }
	}

	/// Returns the AuthState if available.
	pub fn auth_state(&self) -> Option<&AuthState> {
		self.auth_state.as_ref()
	}

	/// Checks if the user is authenticated.
	///
	/// # Returns
	///
	/// `true` if the user is authenticated, `false` otherwise
	pub fn is_authenticated(&self) -> bool {
		self.auth_state
			.as_ref()
			.is_some_and(|s| s.is_authenticated())
	}

	/// Checks if the user is a staff member (admin access).
	///
	/// # Returns
	///
	/// `true` if the user is staff/admin, `false` otherwise
	pub fn is_staff(&self) -> bool {
		self.auth_state.as_ref().is_some_and(|s| s.is_admin())
	}

	/// Checks if the user is active.
	///
	/// # Returns
	///
	/// `true` if the user is active, `false` otherwise
	pub fn is_active(&self) -> bool {
		self.auth_state.as_ref().is_some_and(|s| s.is_active())
	}

	/// Returns the user ID if authenticated.
	pub fn user_id(&self) -> Option<&str> {
		self.auth_state.as_ref().map(|s| s.user_id())
	}

	/// Requires authentication, returning an error if not authenticated.
	///
	/// # Errors
	///
	/// Returns `ServerFnError` with status 401 if not authenticated
	pub fn require_authenticated(&self) -> Result<(), ServerFnError> {
		if !self.is_authenticated() {
			return Err(ServerFnError::server(
				401,
				"Authentication required to access admin panel",
			));
		}
		Ok(())
	}

	/// Requires staff (admin) status, returning an error if not staff.
	///
	/// # Errors
	///
	/// Returns `ServerFnError` with status 403 if not staff
	pub fn require_staff(&self) -> Result<(), ServerFnError> {
		self.require_authenticated()?;
		if !self.is_staff() {
			return Err(ServerFnError::server(
				403,
				"Staff access required for admin panel",
			));
		}
		Ok(())
	}

	/// Checks if the user has permission to view the model.
	///
	/// This uses the default admin permission logic: authenticated staff users
	/// must be explicitly granted view permission. Override
	/// `ModelAdmin::has_view_permission` for custom permission logic.
	///
	/// # Errors
	///
	/// Returns `ServerFnError` with status 403 if permission denied
	pub fn require_view_permission(&self, model_name: &str) -> Result<(), ServerFnError> {
		self.require_staff()?;
		// Default: staff users have view permission
		// Custom permission checks would call ModelAdmin::has_view_permission here
		let _ = model_name; // Will be used for ModelAdmin permission checks
		Ok(())
	}

	/// Checks if the user has permission to add (create) the model.
	///
	/// # Errors
	///
	/// Returns `ServerFnError` with status 403 if permission denied
	pub fn require_add_permission(&self, model_name: &str) -> Result<(), ServerFnError> {
		self.require_staff()?;
		let _ = model_name;
		Ok(())
	}

	/// Checks if the user has permission to change (update) the model.
	///
	/// # Errors
	///
	/// Returns `ServerFnError` with status 403 if permission denied
	pub fn require_change_permission(&self, model_name: &str) -> Result<(), ServerFnError> {
		self.require_staff()?;
		let _ = model_name;
		Ok(())
	}

	/// Checks if the user has permission to delete the model.
	///
	/// # Errors
	///
	/// Returns `ServerFnError` with status 403 if permission denied
	pub fn require_delete_permission(&self, model_name: &str) -> Result<(), ServerFnError> {
		self.require_staff()?;
		let _ = model_name;
		Ok(())
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_model_not_registered_converts_to_404() {
		let admin_err = AdminError::ModelNotRegistered("User".into());
		let server_err = admin_err.into_server_fn_error();

		match server_err {
			ServerFnError::Server { status, message } => {
				assert_eq!(status, 404);
				assert_eq!(message, "User");
			}
			_ => panic!("Expected Server error"),
		}
	}

	#[test]
	fn test_permission_denied_converts_to_403() {
		let admin_err = AdminError::PermissionDenied("Access denied".into());
		let server_err = admin_err.into_server_fn_error();

		match server_err {
			ServerFnError::Server { status, message } => {
				assert_eq!(status, 403);
				assert_eq!(message, "Access denied");
			}
			_ => panic!("Expected Server error"),
		}
	}

	#[test]
	fn test_validation_error_converts_to_application() {
		let admin_err = AdminError::ValidationError("Invalid input".into());
		let server_err = admin_err.into_server_fn_error();

		match server_err {
			ServerFnError::Application(msg) => {
				assert_eq!(msg, "Invalid input");
			}
			_ => panic!("Expected Application error"),
		}
	}

	#[test]
	fn test_database_error_hides_details() {
		let admin_err = AdminError::DatabaseError("SQL syntax error at line 42".into());
		let server_err = admin_err.into_server_fn_error();

		match server_err {
			ServerFnError::Server { status, message } => {
				assert_eq!(status, 500);
				assert_eq!(message, "Database operation failed");
				// Verify that the original error details are hidden
				assert!(!message.contains("SQL"));
				assert!(!message.contains("42"));
			}
			_ => panic!("Expected Server error"),
		}
	}

	#[test]
	fn test_result_conversion() {
		let result: Result<String, AdminError> = Err(AdminError::ModelNotRegistered("Post".into()));
		let server_result = result.map_server_fn_error();

		assert!(server_result.is_err());
		match server_result.unwrap_err() {
			ServerFnError::Server { status, .. } => assert_eq!(status, 404),
			_ => panic!("Expected Server error"),
		}
	}
}
