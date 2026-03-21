//! Model-based Permissions
//!
//! Provides permissions based on model-level operations with Django-style permission checking.

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
///     let mut perm = DjangoModelPermissions::new();
///     perm.add_user_permission("alice", "blog.add_article");
///     perm.add_user_permission("alice", "blog.change_article");
///
///     let request = Request::builder()
///         .method(Method::POST)
///         .uri("/")
///         .body(Bytes::new())
///         .build()
///         .unwrap();
///
///     let context = PermissionContext {
///         request: &request,
///         is_authenticated: true,
///         is_admin: false,
///         is_active: true,
///         user: None,
///     };
///
///     assert!(perm.user_has_permission("alice", "blog.add_article").await);
///     assert!(perm.user_has_permission("alice", "blog.change_article").await);
///     assert!(!perm.user_has_permission("alice", "blog.delete_article").await);
/// }
/// ```
pub struct DjangoModelPermissions {
	/// User permissions map (username -> list of permissions)
	user_permissions: PermissionMap,
}

impl DjangoModelPermissions {
	/// Create a new Django model permission checker
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
		}
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

		// Admins always have permission
		if context.is_admin {
			return true;
		}

		// Check user_permissions map for the authenticated user
		if let Some(user) = &context.user {
			let perms = self.user_permissions.read().await;
			if let Some(user_perms) = perms.get(user.username()) {
				return !user_perms.is_empty();
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
			id: uuid::Uuid::new_v4(),
			username: username.to_string(),
			email: format!("{}@example.com", username),
			is_active: true,
			is_admin: false,
			is_staff: false,
			is_superuser: false,
		})
	}

	#[tokio::test(flavor = "multi_thread")]
	async fn test_django_model_permissions_non_admin_with_permissions() {
		// Arrange
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

		// Act & Assert
		assert!(perm.has_permission(&context).await);
	}

	#[tokio::test(flavor = "multi_thread")]
	async fn test_django_model_permissions_non_admin_empty_permissions() {
		// Arrange
		let mut perm = DjangoModelPermissions::new();
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
}
