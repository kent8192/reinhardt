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

/// Permission types for model-level access control.
///
/// Used with [`AdminAuth::require_model_permission`] to specify which
/// permission to check against the `ModelAdmin`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelPermission {
	/// Permission to view model instances
	View,
	/// Permission to add (create) model instances
	Add,
	/// Permission to change (update) model instances
	Change,
	/// Permission to delete model instances
	Delete,
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

	/// Checks model-level permission using `ModelAdmin`, returning an error if denied.
	///
	/// This method first verifies staff status, then delegates to the
	/// `ModelAdmin`'s permission method for the specified permission type.
	///
	/// The caller is responsible for providing the authenticated user object
	/// extracted from the DI context via [`AdminAuthenticatedUser`]. The user
	/// is passed as a `&dyn AdminUser` trait object, which is produced by the
	/// type-erased user loader registered during admin route setup.
	///
	/// [`AdminAuthenticatedUser`]: crate::server::admin_auth::AdminAuthenticatedUser
	///
	/// # Arguments
	///
	/// * `model_admin` - The model admin to check permissions against
	/// * `user` - The authenticated user object as a trait object
	/// * `permission` - The type of permission to check
	///
	/// # Errors
	///
	/// Returns `ServerFnError` with status 401 if not authenticated,
	/// 403 if not staff or if model-level permission is denied
	pub async fn require_model_permission(
		&self,
		model_admin: &dyn crate::core::ModelAdmin,
		user: &dyn crate::core::AdminUser,
		permission: ModelPermission,
	) -> Result<(), ServerFnError> {
		self.require_staff()?;

		// require_staff() already guarantees auth_state is Some and authenticated,
		// so we can proceed directly to the permission check.
		let has_permission = match permission {
			ModelPermission::View => model_admin.has_view_permission(user).await,
			ModelPermission::Add => model_admin.has_add_permission(user).await,
			ModelPermission::Change => model_admin.has_change_permission(user).await,
			ModelPermission::Delete => model_admin.has_delete_permission(user).await,
		};

		if !has_permission {
			return Err(ServerFnError::server(403, "Permission denied"));
		}

		Ok(())
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use async_trait::async_trait;
	use rstest::rstest;
	use std::sync::Arc;

	// --- Helper structs for require_model_permission tests ---

	/// Test user implementing AdminUser for permission tests
	struct TestUser;

	impl crate::core::AdminUser for TestUser {
		fn is_active(&self) -> bool {
			true
		}
		fn is_staff(&self) -> bool {
			true
		}
		fn is_superuser(&self) -> bool {
			false
		}
		fn get_username(&self) -> &str {
			"test_user"
		}
	}

	/// Always denies all permissions (uses default trait behavior)
	struct DenyAllAdmin;

	#[async_trait]
	impl crate::core::ModelAdmin for DenyAllAdmin {
		fn model_name(&self) -> &str {
			"DenyModel"
		}
	}

	/// Always grants all permissions
	struct AllowAllAdmin;

	#[async_trait]
	impl crate::core::ModelAdmin for AllowAllAdmin {
		fn model_name(&self) -> &str {
			"AllowModel"
		}

		async fn has_view_permission(&self, _: &dyn crate::core::AdminUser) -> bool {
			true
		}
		async fn has_add_permission(&self, _: &dyn crate::core::AdminUser) -> bool {
			true
		}
		async fn has_change_permission(&self, _: &dyn crate::core::AdminUser) -> bool {
			true
		}
		async fn has_delete_permission(&self, _: &dyn crate::core::AdminUser) -> bool {
			true
		}
	}

	/// Grants only a specific permission type
	struct SelectiveAdmin {
		allowed: ModelPermission,
	}

	#[async_trait]
	impl crate::core::ModelAdmin for SelectiveAdmin {
		fn model_name(&self) -> &str {
			"SelectiveModel"
		}

		async fn has_view_permission(&self, _: &dyn crate::core::AdminUser) -> bool {
			self.allowed == ModelPermission::View
		}
		async fn has_add_permission(&self, _: &dyn crate::core::AdminUser) -> bool {
			self.allowed == ModelPermission::Add
		}
		async fn has_change_permission(&self, _: &dyn crate::core::AdminUser) -> bool {
			self.allowed == ModelPermission::Change
		}
		async fn has_delete_permission(&self, _: &dyn crate::core::AdminUser) -> bool {
			self.allowed == ModelPermission::Delete
		}
	}

	/// Create AdminAuth from an optional AuthState
	fn make_admin_auth(auth_state: Option<AuthState>) -> AdminAuth {
		let request = reinhardt_http::Request::builder()
			.uri("/admin/test")
			.build()
			.expect("Failed to build test request");
		if let Some(state) = auth_state {
			request.extensions.insert(state);
		}
		AdminAuth::from_arc_request(&Arc::new(request))
	}

	// --- require_model_permission tests ---

	#[rstest]
	#[tokio::test]
	async fn test_require_model_permission_staff_with_permission() {
		// Arrange
		let auth = make_admin_auth(Some(AuthState::authenticated("user1", true, true)));
		let admin = AllowAllAdmin;
		let user_obj = TestUser;

		// Act
		let result = auth
			.require_model_permission(
				&admin,
				&user_obj as &dyn crate::core::AdminUser,
				ModelPermission::View,
			)
			.await;

		// Assert
		assert!(result.is_ok());
	}

	#[rstest]
	#[tokio::test]
	async fn test_require_model_permission_staff_denied_by_model() {
		// Arrange
		let auth = make_admin_auth(Some(AuthState::authenticated("user1", true, true)));
		let admin = DenyAllAdmin;
		let user_obj = TestUser;

		// Act
		let result = auth
			.require_model_permission(
				&admin,
				&user_obj as &dyn crate::core::AdminUser,
				ModelPermission::View,
			)
			.await;

		// Assert
		assert!(result.is_err());
		match result.unwrap_err() {
			ServerFnError::Server { status, message } => {
				assert_eq!(status, 403);
				assert_eq!(message, "Permission denied");
			}
			other => panic!("Expected Server error with 403, got: {other:?}"),
		}
	}

	#[rstest]
	#[tokio::test]
	async fn test_require_model_permission_non_staff_denied() {
		// Arrange
		let auth = make_admin_auth(Some(AuthState::authenticated("user1", false, true)));
		let admin = AllowAllAdmin;
		let user_obj = TestUser;

		// Act
		let result = auth
			.require_model_permission(
				&admin,
				&user_obj as &dyn crate::core::AdminUser,
				ModelPermission::View,
			)
			.await;

		// Assert
		assert!(result.is_err());
		match result.unwrap_err() {
			ServerFnError::Server { status, message } => {
				assert_eq!(status, 403);
				assert_eq!(message, "Staff access required for admin panel");
			}
			other => panic!("Expected Server error with 403, got: {other:?}"),
		}
	}

	#[rstest]
	#[tokio::test]
	async fn test_require_model_permission_unauthenticated() {
		// Arrange
		let auth = make_admin_auth(None);
		let admin = AllowAllAdmin;
		let user_obj = TestUser;

		// Act
		let result = auth
			.require_model_permission(
				&admin,
				&user_obj as &dyn crate::core::AdminUser,
				ModelPermission::View,
			)
			.await;

		// Assert
		assert!(result.is_err());
		match result.unwrap_err() {
			ServerFnError::Server { status, message } => {
				assert_eq!(status, 401);
				assert_eq!(message, "Authentication required to access admin panel");
			}
			other => panic!("Expected Server error with 401, got: {other:?}"),
		}
	}

	#[rstest]
	#[case::view_matches_view(ModelPermission::View, ModelPermission::View, true)]
	#[case::view_does_not_match_add(ModelPermission::View, ModelPermission::Add, false)]
	#[case::add_matches_add(ModelPermission::Add, ModelPermission::Add, true)]
	#[case::change_does_not_match_delete(ModelPermission::Change, ModelPermission::Delete, false)]
	#[tokio::test]
	async fn test_require_model_permission_selective_permissions(
		#[case] granted: ModelPermission,
		#[case] requested: ModelPermission,
		#[case] expected_ok: bool,
	) {
		// Arrange
		let auth = make_admin_auth(Some(AuthState::authenticated("user1", true, true)));
		let admin = SelectiveAdmin { allowed: granted };
		let user_obj = TestUser;

		// Act
		let result = auth
			.require_model_permission(&admin, &user_obj as &dyn crate::core::AdminUser, requested)
			.await;

		// Assert
		assert_eq!(
			result.is_ok(),
			expected_ok,
			"granted={granted:?}, requested={requested:?}: expected is_ok()={expected_ok}"
		);
	}

	// --- Error conversion tests ---

	#[rstest]
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

	#[rstest]
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

	#[rstest]
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

	#[rstest]
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

	#[rstest]
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
