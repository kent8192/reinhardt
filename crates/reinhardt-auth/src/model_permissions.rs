//! Model-based Permissions
//!
//! Provides permissions based on model-level operations with Django-style permission checking.

// This module uses the deprecated User trait for backward compatibility.
// DjangoModelPermissions reads username from Box<dyn User> in PermissionContext.
#![allow(deprecated)]
use crate::{Permission, PermissionContext};
use async_trait::async_trait;
use std::collections::HashMap;
use std::marker::PhantomData;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Permission map for Django-style permissions
///
/// Maps `app_label.action_model` format permissions to users.
type PermissionMap = Arc<RwLock<HashMap<String, Vec<String>>>>;

/// Django-style model permissions
///
/// Checks permissions for CRUD operations on Django models using
/// the `app_label.action_model` format (e.g., "blog.add_article", "blog.change_article").
///
/// HTTP methods are mapped to required permission actions:
/// - POST -> `add_<model>`
/// - PUT/PATCH -> `change_<model>`
/// - DELETE -> `delete_<model>`
/// - GET/HEAD/OPTIONS -> `view_<model>`
///
/// # Examples
///
/// ```
/// use reinhardt_auth::model_permissions::DjangoModelPermissions;
/// use reinhardt_auth::{Permission, PermissionContext};
/// use bytes::Bytes;
/// use hyper::{Method};
/// use reinhardt_http::Request;
///
/// #[tokio::main]
/// async fn main() {
///     let mut perm = DjangoModelPermissions::with_model_name("blog.article");
///     perm.add_user_permission("alice", "blog.add_article");
///     perm.add_user_permission("alice", "blog.change_article");
///
///     assert!(perm.user_has_permission("alice", "blog.add_article").await);
///     assert!(perm.user_has_permission("alice", "blog.change_article").await);
///     assert!(!perm.user_has_permission("alice", "blog.delete_article").await);
/// }
/// ```
pub struct DjangoModelPermissions {
	/// User permissions map (username -> list of permissions)
	user_permissions: PermissionMap,
	/// Model name in `app_label.model` format (e.g., "blog.article")
	/// Used to derive required permissions from HTTP methods.
	model_name: Option<String>,
}

impl DjangoModelPermissions {
	/// Create a new Django model permission checker without a model name
	///
	/// Without a model name, HTTP method-based permission derivation is not available.
	/// The [`has_permission`](Permission::has_permission) implementation will fall back to
	/// allowing only admin users (based on `context.is_admin`); explicit permission
	/// string checks are only available via the direct API
	/// [`user_has_permission`](Self::user_has_permission).
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_auth::model_permissions::DjangoModelPermissions;
	///
	/// let perm = DjangoModelPermissions::new();
	/// ```
	pub fn new() -> Self {
		Self {
			user_permissions: Arc::new(RwLock::new(HashMap::new())),
			model_name: None,
		}
	}

	/// Create a new Django model permission checker with a model name
	///
	/// The model name **must** be in `app_label.model` format (e.g., `"blog.article"`).
	/// This enables HTTP method-based permission derivation:
	/// - POST -> `app_label.add_model`
	/// - PUT/PATCH -> `app_label.change_model`
	/// - DELETE -> `app_label.delete_model`
	/// - GET/HEAD/OPTIONS -> `app_label.view_model`
	///
	/// # Panics
	///
	/// Panics if `model_name` does not contain exactly one `.` separator
	/// (i.e., both `app_label` and `model` parts must be non-empty).
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_auth::model_permissions::DjangoModelPermissions;
	///
	/// let perm = DjangoModelPermissions::with_model_name("blog.article");
	/// ```
	///
	/// ```should_panic
	/// use reinhardt_auth::model_permissions::DjangoModelPermissions;
	///
	/// // Missing dot separator - panics
	/// let _perm = DjangoModelPermissions::with_model_name("blogarticle");
	/// ```
	pub fn with_model_name(model_name: &str) -> Self {
		let Some((app_label, model)) = model_name.split_once('.') else {
			panic!(
				"model_name must be in `app_label.model` format, got: {:?}",
				model_name
			);
		};
		assert!(
			!app_label.is_empty() && !model.is_empty(),
			"both app_label and model must be non-empty in `app_label.model` format, got: {:?}",
			model_name
		);
		Self {
			user_permissions: Arc::new(RwLock::new(HashMap::new())),
			model_name: Some(model_name.to_string()),
		}
	}

	/// Get the required permissions for an HTTP method based on the configured model name
	///
	/// Returns a list of permission strings required for the given HTTP method.
	/// Returns an empty list if no model name is configured.
	fn get_required_permissions(&self, method: &str) -> Vec<String> {
		let Some(ref model_name) = self.model_name else {
			return Vec::new();
		};

		// Split "app_label.model" into parts
		let Some((app_label, model)) = model_name.split_once('.') else {
			return Vec::new();
		};

		let actions = match method {
			"POST" => vec!["add"],
			"PUT" | "PATCH" => vec!["change"],
			"DELETE" => vec!["delete"],
			"GET" | "HEAD" | "OPTIONS" => vec!["view"],
			_ => return Vec::new(),
		};

		actions
			.into_iter()
			.map(|action| format!("{}.{}_{}", app_label, action, model))
			.collect()
	}

	/// Add permission to user
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_auth::model_permissions::DjangoModelPermissions;
	///
	/// let mut perm = DjangoModelPermissions::new();
	/// // Note: This method uses tokio::block_in_place internally
	/// perm.add_user_permission("alice", "blog.add_article");
	/// perm.add_user_permission("bob", "blog.view_article");
	/// ```
	pub fn add_user_permission(&mut self, username: &str, permission: &str) {
		let user_perms = Arc::clone(&self.user_permissions);
		tokio::task::block_in_place(|| {
			tokio::runtime::Handle::current().block_on(async {
				let mut perms = user_perms.write().await;
				perms
					.entry(username.to_string())
					.or_default()
					.push(permission.to_string());
			})
		});
	}

	/// Check if user has specific permission
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_auth::model_permissions::DjangoModelPermissions;
	///
	/// #[tokio::main]
	/// async fn main() {
	///     let mut perm = DjangoModelPermissions::new();
	///     perm.add_user_permission("alice", "blog.add_article");
	///
	///     assert!(perm.user_has_permission("alice", "blog.add_article").await);
	///     assert!(!perm.user_has_permission("alice", "blog.delete_article").await);
	/// }
	/// ```
	pub async fn user_has_permission(&self, username: &str, permission: &str) -> bool {
		let perms = self.user_permissions.read().await;
		if let Some(user_perms) = perms.get(username) {
			return user_perms.iter().any(|p| p == permission);
		}
		false
	}
}

impl Default for DjangoModelPermissions {
	fn default() -> Self {
		Self::new()
	}
}

#[async_trait]
impl Permission for DjangoModelPermissions {
	async fn has_permission(&self, context: &PermissionContext<'_>) -> bool {
		if !context.is_authenticated {
			return false;
		}

		// Get the required permissions for this HTTP method and model
		let required_perms = self.get_required_permissions(context.request.method.as_str());

		// If no model_name is configured, no method-based permissions can be derived.
		// Fall back to checking if the user has any permissions at all (legacy behavior
		// only for admin users).
		if required_perms.is_empty() {
			// Without a model_name, only admins are granted blanket access
			return context.is_admin;
		}

		// Check if the user has ALL required permissions
		if let Some(user) = &context.user {
			let perms = self.user_permissions.read().await;
			if let Some(user_perms) = perms.get(user.username()) {
				return required_perms
					.iter()
					.all(|required| user_perms.iter().any(|p| p == required));
			}
		}

		false
	}
}

/// Django model permissions with anonymous read-only access
///
/// Extends `DjangoModelPermissions` to allow read-only (GET, HEAD, OPTIONS)
/// access for unauthenticated users.
///
/// # Examples
///
/// ```
/// use reinhardt_auth::model_permissions::DjangoModelPermissionsOrAnonReadOnly;
/// use reinhardt_auth::{Permission, PermissionContext};
/// use bytes::Bytes;
/// use hyper::{Method};
/// use reinhardt_http::Request;
///
/// #[tokio::main]
/// async fn main() {
///     let perm = DjangoModelPermissionsOrAnonReadOnly::new();
///
///     // GET request - allowed for unauthenticated
///     let get_request = Request::builder()
///         .method(Method::GET)
///         .uri("/")
///         .body(Bytes::new())
///         .build()
///         .unwrap();
///     let context = PermissionContext {
///         request: &get_request,
///         is_authenticated: false,
///         is_admin: false,
///         is_active: false,
///         user: None,
///     };
///     assert!(perm.has_permission(&context).await);
///
///     // POST request - requires authentication
///     let post_request = Request::builder()
///         .method(Method::POST)
///         .uri("/")
///         .body(Bytes::new())
///         .build()
///         .unwrap();
///     let context = PermissionContext {
///         request: &post_request,
///         is_authenticated: false,
///         is_admin: false,
///         is_active: false,
///         user: None,
///     };
///     assert!(!perm.has_permission(&context).await);
/// }
/// ```
pub struct DjangoModelPermissionsOrAnonReadOnly {
	base: DjangoModelPermissions,
}

impl DjangoModelPermissionsOrAnonReadOnly {
	/// Create a new permission checker with anonymous read-only access
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_auth::model_permissions::DjangoModelPermissionsOrAnonReadOnly;
	///
	/// let perm = DjangoModelPermissionsOrAnonReadOnly::new();
	/// ```
	pub fn new() -> Self {
		Self {
			base: DjangoModelPermissions::new(),
		}
	}

	/// Add permission to user
	pub fn add_user_permission(&mut self, username: &str, permission: &str) {
		self.base.add_user_permission(username, permission);
	}

	/// Check if user has specific permission
	pub async fn user_has_permission(&self, username: &str, permission: &str) -> bool {
		self.base.user_has_permission(username, permission).await
	}
}

impl Default for DjangoModelPermissionsOrAnonReadOnly {
	fn default() -> Self {
		Self::new()
	}
}

#[async_trait]
impl Permission for DjangoModelPermissionsOrAnonReadOnly {
	async fn has_permission(&self, context: &PermissionContext<'_>) -> bool {
		// Allow read-only methods for unauthenticated users
		if !context.is_authenticated {
			return matches!(context.request.method.as_str(), "GET" | "HEAD" | "OPTIONS");
		}

		// For authenticated users, use base permission check
		self.base.has_permission(context).await
	}
}

/// Model permission
///
/// Checks permissions for CRUD operations on specific model types.
///
/// # Examples
///
/// ```
/// use reinhardt_auth::model_permissions::ModelPermission;
///
/// #[derive(Debug)]
/// struct Article;
///
/// let perm = ModelPermission::<Article>::new("create");
/// assert_eq!(perm.operation(), "create");
/// ```
pub struct ModelPermission<T> {
	/// Operation (create, read, update, delete)
	operation: String,
	_phantom: PhantomData<T>,
}

impl<T> ModelPermission<T> {
	/// Create a new model permission
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_auth::model_permissions::ModelPermission;
	///
	/// #[derive(Debug)]
	/// struct Post;
	///
	/// let perm = ModelPermission::<Post>::new("update");
	/// assert_eq!(perm.operation(), "update");
	/// ```
	pub fn new(operation: impl Into<String>) -> Self {
		Self {
			operation: operation.into(),
			_phantom: PhantomData,
		}
	}

	/// Get operation name
	pub fn operation(&self) -> &str {
		&self.operation
	}
}

#[async_trait]
impl<T: Send + Sync> Permission for ModelPermission<T> {
	async fn has_permission(&self, context: &PermissionContext<'_>) -> bool {
		context.is_authenticated
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use bytes::Bytes;
	use hyper::Method;
	use reinhardt_http::Request;

	#[derive(Debug)]
	struct TestModel;

	#[test]
	fn test_model_permission_creation() {
		let perm = ModelPermission::<TestModel>::new("create");
		assert_eq!(perm.operation(), "create");
	}

	#[test]
	fn test_model_permission_operations() {
		let create = ModelPermission::<TestModel>::new("create");
		let read = ModelPermission::<TestModel>::new("read");
		let update = ModelPermission::<TestModel>::new("update");
		let delete = ModelPermission::<TestModel>::new("delete");

		assert_eq!(create.operation(), "create");
		assert_eq!(read.operation(), "read");
		assert_eq!(update.operation(), "update");
		assert_eq!(delete.operation(), "delete");
	}

	#[tokio::test]
	async fn test_model_permission_authenticated() {
		let perm = ModelPermission::<TestModel>::new("read");
		let request = Request::builder()
			.method(Method::GET)
			.uri("/")
			.body(Bytes::new())
			.build()
			.unwrap();

		let context = PermissionContext {
			request: &request,
			is_authenticated: true,
			is_admin: false,
			is_active: true,
			user: None,
		};

		assert!(perm.has_permission(&context).await);
	}

	#[tokio::test]
	async fn test_model_permission_unauthenticated() {
		let perm = ModelPermission::<TestModel>::new("create");
		let request = Request::builder()
			.method(Method::POST)
			.uri("/")
			.body(Bytes::new())
			.build()
			.unwrap();

		let context = PermissionContext {
			request: &request,
			is_authenticated: false,
			is_admin: false,
			is_active: false,
			user: None,
		};

		assert!(!perm.has_permission(&context).await);
	}

	#[derive(Debug)]
	struct Article;

	#[derive(Debug)]
	struct Comment;

	#[tokio::test]
	async fn test_different_model_types() {
		let article_perm = ModelPermission::<Article>::new("update");
		let comment_perm = ModelPermission::<Comment>::new("delete");

		let request = Request::builder()
			.method(Method::PUT)
			.uri("/")
			.body(Bytes::new())
			.build()
			.unwrap();

		let context = PermissionContext {
			request: &request,
			is_authenticated: true,
			is_admin: false,
			is_active: true,
			user: None,
		};

		assert!(article_perm.has_permission(&context).await);
		assert!(comment_perm.has_permission(&context).await);
	}

	#[tokio::test]
	async fn test_django_model_permissions_creation() {
		let perm = DjangoModelPermissions::new();
		assert!(!perm.user_has_permission("alice", "blog.add_article").await);
	}

	#[tokio::test(flavor = "multi_thread")]
	async fn test_django_model_permissions_add_permission() {
		let mut perm = DjangoModelPermissions::new();
		perm.add_user_permission("alice", "blog.add_article");
		perm.add_user_permission("alice", "blog.change_article");

		assert!(perm.user_has_permission("alice", "blog.add_article").await);
		assert!(
			perm.user_has_permission("alice", "blog.change_article")
				.await
		);
		assert!(
			!perm
				.user_has_permission("alice", "blog.delete_article")
				.await
		);
	}

	#[tokio::test(flavor = "multi_thread")]
	async fn test_django_model_permissions_different_users() {
		let mut perm = DjangoModelPermissions::new();
		perm.add_user_permission("alice", "blog.add_article");
		perm.add_user_permission("bob", "blog.view_article");

		assert!(perm.user_has_permission("alice", "blog.add_article").await);
		assert!(!perm.user_has_permission("alice", "blog.view_article").await);
		assert!(perm.user_has_permission("bob", "blog.view_article").await);
		assert!(!perm.user_has_permission("bob", "blog.add_article").await);
	}

	#[tokio::test]
	async fn test_django_model_permissions_trait_authenticated_admin() {
		let perm = DjangoModelPermissions::new();
		let request = Request::builder()
			.method(Method::POST)
			.uri("/")
			.body(Bytes::new())
			.build()
			.unwrap();

		let context = PermissionContext {
			request: &request,
			is_authenticated: true,
			is_admin: true,
			is_active: true,
			user: None,
		};

		assert!(perm.has_permission(&context).await);
	}

	#[tokio::test]
	async fn test_django_model_permissions_trait_authenticated_not_admin() {
		let perm = DjangoModelPermissions::new();
		let request = Request::builder()
			.method(Method::POST)
			.uri("/")
			.body(Bytes::new())
			.build()
			.unwrap();

		let context = PermissionContext {
			request: &request,
			is_authenticated: true,
			is_admin: false,
			is_active: true,
			user: None,
		};

		assert!(!perm.has_permission(&context).await);
	}

	#[tokio::test]
	async fn test_django_model_permissions_trait_unauthenticated() {
		let perm = DjangoModelPermissions::new();
		let request = Request::builder()
			.method(Method::GET)
			.uri("/")
			.body(Bytes::new())
			.build()
			.unwrap();

		let context = PermissionContext {
			request: &request,
			is_authenticated: false,
			is_admin: false,
			is_active: false,
			user: None,
		};

		assert!(!perm.has_permission(&context).await);
	}

	#[tokio::test]
	async fn test_django_model_permissions_or_anon_read_only_get() {
		let perm = DjangoModelPermissionsOrAnonReadOnly::new();
		let request = Request::builder()
			.method(Method::GET)
			.uri("/")
			.body(Bytes::new())
			.build()
			.unwrap();

		let context = PermissionContext {
			request: &request,
			is_authenticated: false,
			is_admin: false,
			is_active: false,
			user: None,
		};

		assert!(perm.has_permission(&context).await);
	}

	#[tokio::test]
	async fn test_django_model_permissions_or_anon_read_only_post() {
		let perm = DjangoModelPermissionsOrAnonReadOnly::new();
		let request = Request::builder()
			.method(Method::POST)
			.uri("/")
			.body(Bytes::new())
			.build()
			.unwrap();

		let context = PermissionContext {
			request: &request,
			is_authenticated: false,
			is_admin: false,
			is_active: false,
			user: None,
		};

		assert!(!perm.has_permission(&context).await);
	}

	#[tokio::test]
	async fn test_django_model_permissions_or_anon_read_only_authenticated() {
		let perm = DjangoModelPermissionsOrAnonReadOnly::new();
		let request = Request::builder()
			.method(Method::POST)
			.uri("/")
			.body(Bytes::new())
			.build()
			.unwrap();

		let context = PermissionContext {
			request: &request,
			is_authenticated: true,
			is_admin: true,
			is_active: true,
			user: None,
		};

		assert!(perm.has_permission(&context).await);
	}

	fn make_user(username: &str) -> Box<dyn crate::User> {
		Box::new(crate::SimpleUser {
			id: uuid::Uuid::now_v7(),
			username: username.to_string(),
			email: format!("{}@example.com", username),
			is_active: true,
			is_admin: false,
			is_staff: false,
			is_superuser: false,
		})
	}

	#[tokio::test(flavor = "multi_thread")]
	async fn test_django_model_permissions_non_admin_with_matching_permissions() {
		// Arrange
		let mut perm = DjangoModelPermissions::with_model_name("blog.article");
		perm.add_user_permission("alice", "blog.add_article");

		let request = Request::builder()
			.method(Method::POST)
			.uri("/")
			.body(Bytes::new())
			.build()
			.unwrap();

		let context = PermissionContext {
			request: &request,
			is_authenticated: true,
			is_admin: false,
			is_active: true,
			user: Some(make_user("alice")),
		};

		// Act & Assert - user has the required "blog.add_article" for POST
		assert!(perm.has_permission(&context).await);
	}

	#[rstest::rstest]
	#[tokio::test(flavor = "multi_thread")]
	async fn test_django_model_permissions_non_admin_wrong_permissions() {
		// Arrange - user has add but tries to delete
		let mut perm = DjangoModelPermissions::with_model_name("blog.article");
		perm.add_user_permission("alice", "blog.add_article");

		let request = Request::builder()
			.method(Method::DELETE)
			.uri("/")
			.body(Bytes::new())
			.build()
			.unwrap();

		let context = PermissionContext {
			request: &request,
			is_authenticated: true,
			is_admin: false,
			is_active: true,
			user: Some(make_user("alice")),
		};

		// Act & Assert - user does NOT have "blog.delete_article"
		assert!(!perm.has_permission(&context).await);
	}

	#[tokio::test(flavor = "multi_thread")]
	async fn test_django_model_permissions_non_admin_empty_permissions() {
		// Arrange
		let mut perm = DjangoModelPermissions::with_model_name("blog.article");
		// Don't add any permissions for alice

		let request = Request::builder()
			.method(Method::POST)
			.uri("/")
			.body(Bytes::new())
			.build()
			.unwrap();

		let context = PermissionContext {
			request: &request,
			is_authenticated: true,
			is_admin: false,
			is_active: true,
			user: Some(make_user("alice")),
		};

		// Act & Assert
		assert!(!perm.has_permission(&context).await);
	}

	#[rstest::rstest]
	#[tokio::test(flavor = "multi_thread")]
	async fn test_django_model_permissions_no_model_name_non_admin_denied() {
		// Arrange - no model_name configured, non-admin user
		let mut perm = DjangoModelPermissions::new();
		perm.add_user_permission("alice", "blog.add_article");

		let request = Request::builder()
			.method(Method::POST)
			.uri("/")
			.body(Bytes::new())
			.build()
			.unwrap();

		let context = PermissionContext {
			request: &request,
			is_authenticated: true,
			is_admin: false,
			is_active: true,
			user: Some(make_user("alice")),
		};

		// Act & Assert - without model_name, only admins get blanket access
		assert!(!perm.has_permission(&context).await);
	}

	#[rstest::rstest]
	#[tokio::test]
	async fn test_django_model_permissions_no_model_name_admin_allowed() {
		// Arrange - no model_name configured, admin user
		let perm = DjangoModelPermissions::new();

		let request = Request::builder()
			.method(Method::POST)
			.uri("/")
			.body(Bytes::new())
			.build()
			.unwrap();

		let context = PermissionContext {
			request: &request,
			is_authenticated: true,
			is_admin: true,
			is_active: true,
			user: Some(make_user("admin")),
		};

		// Act & Assert - admin gets blanket access even without model_name
		assert!(perm.has_permission(&context).await);
	}

	#[rstest::rstest]
	#[tokio::test(flavor = "multi_thread")]
	async fn test_django_model_permissions_method_to_permission_mapping() {
		// Arrange
		let mut perm = DjangoModelPermissions::with_model_name("blog.article");
		perm.add_user_permission("alice", "blog.view_article");
		perm.add_user_permission("alice", "blog.add_article");
		perm.add_user_permission("alice", "blog.change_article");
		// Note: alice does NOT have blog.delete_article

		let user = make_user("alice");

		// Act & Assert - GET requires view_article
		let request = Request::builder()
			.method(Method::GET)
			.uri("/")
			.body(Bytes::new())
			.build()
			.unwrap();
		let context = PermissionContext {
			request: &request,
			is_authenticated: true,
			is_admin: false,
			is_active: true,
			user: Some(make_user("alice")),
		};
		assert!(perm.has_permission(&context).await);

		// POST requires add_article
		let request = Request::builder()
			.method(Method::POST)
			.uri("/")
			.body(Bytes::new())
			.build()
			.unwrap();
		let context = PermissionContext {
			request: &request,
			is_authenticated: true,
			is_admin: false,
			is_active: true,
			user: Some(make_user("alice")),
		};
		assert!(perm.has_permission(&context).await);

		// PUT requires change_article
		let request = Request::builder()
			.method(Method::PUT)
			.uri("/")
			.body(Bytes::new())
			.build()
			.unwrap();
		let context = PermissionContext {
			request: &request,
			is_authenticated: true,
			is_admin: false,
			is_active: true,
			user: Some(make_user("alice")),
		};
		assert!(perm.has_permission(&context).await);

		// DELETE requires delete_article - alice doesn't have it
		let request = Request::builder()
			.method(Method::DELETE)
			.uri("/")
			.body(Bytes::new())
			.build()
			.unwrap();
		let context = PermissionContext {
			request: &request,
			is_authenticated: true,
			is_admin: false,
			is_active: true,
			user: Some(make_user("alice")),
		};
		assert!(!perm.has_permission(&context).await);
	}
}
