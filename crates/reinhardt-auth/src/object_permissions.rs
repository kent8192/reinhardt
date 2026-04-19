//! Object-Level Permissions
//!
//! Provides permission checking on individual object instances.

// This module uses the deprecated User trait for backward compatibility.
// ObjectPermission accepts &dyn User to preserve existing permission APIs.
#![allow(deprecated)]
use crate::User;
use crate::{Permission, PermissionContext};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

// Type alias to simplify complex permission map type
/// Permission map keyed by (username, object_id) tuple, storing a list of permission strings
pub type PermissionMap = Arc<RwLock<HashMap<(String, String), Vec<String>>>>;

/// Object permission checker trait
///
/// Allows for custom permission logic on specific object instances.
///
/// # Examples
///
/// ```
/// use reinhardt_auth::object_permissions::ObjectPermissionChecker;
/// use reinhardt_auth::{User, SimpleUser};
/// use async_trait::async_trait;
/// use uuid::Uuid;
///
/// struct ArticlePermissionChecker;
///
/// #[async_trait]
/// impl ObjectPermissionChecker for ArticlePermissionChecker {
///     async fn has_object_permission(
///         &self,
///         user: &dyn User,
///         object_id: &str,
///         permission: &str,
///     ) -> bool {
///         // Example: Check if user is the owner
///         user.username() == "alice" && permission == "change"
///     }
/// }
///
/// #[tokio::main]
/// async fn main() {
///     let checker = ArticlePermissionChecker;
///     let user = SimpleUser {
///         id: Uuid::now_v7(),
///         username: "alice".to_string(),
///         email: "alice@example.com".to_string(),
///         is_active: true,
///         is_admin: false,
///         is_staff: false,
///         is_superuser: false,
///     };
///
///     assert!(checker.has_object_permission(&user, "article:123", "change").await);
///     assert!(!checker.has_object_permission(&user, "article:123", "delete").await);
/// }
/// ```
#[async_trait]
pub trait ObjectPermissionChecker: Send + Sync {
	/// Check if user has permission for specific object
	///
	/// # Arguments
	///
	/// * `user` - The user to check permissions for
	/// * `object_id` - Identifier for the object
	/// * `permission` - Permission to check (e.g., "view", "change", "delete")
	///
	/// # Returns
	///
	/// `true` if user has permission, `false` otherwise
	async fn has_object_permission(
		&self,
		user: &dyn User,
		object_id: &str,
		permission: &str,
	) -> bool;
}

/// Object permission manager
///
/// Manages object-level permissions using a permission map.
///
/// # Examples
///
/// ```
/// use reinhardt_auth::object_permissions::{ObjectPermissionManager, ObjectPermissionChecker};
/// use reinhardt_auth::{SimpleUser, User};
/// use uuid::Uuid;
///
/// #[tokio::main]
/// async fn main() {
///     let mut manager = ObjectPermissionManager::new();
///
///     // Grant alice permission to change article:123
///     manager.grant_permission("alice", "article:123", "change").await;
///
///     let user = SimpleUser {
///         id: Uuid::now_v7(),
///         username: "alice".to_string(),
///         email: "alice@example.com".to_string(),
///         is_active: true,
///         is_admin: false,
///         is_staff: false,
///         is_superuser: false,
///     };
///
///     assert!(manager.has_object_permission(&user, "article:123", "change").await);
///     assert!(!manager.has_object_permission(&user, "article:123", "delete").await);
///
///     // Revoke permission
///     manager.revoke_permission("alice", "article:123", "change").await;
///     assert!(!manager.has_object_permission(&user, "article:123", "change").await);
/// }
/// ```
pub struct ObjectPermissionManager {
	/// Permissions map: (username, object_id) -> list of permissions
	permissions: PermissionMap,
}

impl ObjectPermissionManager {
	/// Create a new object permission manager
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_auth::object_permissions::ObjectPermissionManager;
	///
	/// let manager = ObjectPermissionManager::new();
	/// ```
	pub fn new() -> Self {
		Self {
			permissions: Arc::new(RwLock::new(HashMap::new())),
		}
	}

	/// Grant permission to user for specific object
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_auth::object_permissions::ObjectPermissionManager;
	///
	/// #[tokio::main]
	/// async fn main() {
	///     let mut manager = ObjectPermissionManager::new();
	///     manager.grant_permission("alice", "article:123", "change").await;
	///     manager.grant_permission("alice", "article:123", "view").await;
	/// }
	/// ```
	pub async fn grant_permission(&mut self, username: &str, object_id: &str, permission: &str) {
		let mut perms = self.permissions.write().await;
		let key = (username.to_string(), object_id.to_string());
		perms.entry(key).or_default().push(permission.to_string());
	}

	/// Revoke permission from user for specific object
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_auth::object_permissions::ObjectPermissionManager;
	///
	/// #[tokio::main]
	/// async fn main() {
	///     let mut manager = ObjectPermissionManager::new();
	///     manager.grant_permission("alice", "article:123", "change").await;
	///     manager.revoke_permission("alice", "article:123", "change").await;
	/// }
	/// ```
	pub async fn revoke_permission(&mut self, username: &str, object_id: &str, permission: &str) {
		let mut perms = self.permissions.write().await;
		let key = (username.to_string(), object_id.to_string());
		if let Some(user_perms) = perms.get_mut(&key) {
			user_perms.retain(|p| p != permission);
			if user_perms.is_empty() {
				perms.remove(&key);
			}
		}
	}

	/// Revoke all permissions for user on specific object
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_auth::object_permissions::ObjectPermissionManager;
	///
	/// #[tokio::main]
	/// async fn main() {
	///     let mut manager = ObjectPermissionManager::new();
	///     manager.grant_permission("alice", "article:123", "change").await;
	///     manager.grant_permission("alice", "article:123", "view").await;
	///     manager.revoke_all_permissions("alice", "article:123").await;
	/// }
	/// ```
	pub async fn revoke_all_permissions(&mut self, username: &str, object_id: &str) {
		let mut perms = self.permissions.write().await;
		let key = (username.to_string(), object_id.to_string());
		perms.remove(&key);
	}

	/// List all permissions for user on specific object
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_auth::object_permissions::ObjectPermissionManager;
	///
	/// #[tokio::main]
	/// async fn main() {
	///     let mut manager = ObjectPermissionManager::new();
	///     manager.grant_permission("alice", "article:123", "change").await;
	///     manager.grant_permission("alice", "article:123", "view").await;
	///
	///     let perms = manager.list_permissions("alice", "article:123").await;
	///     assert_eq!(perms.len(), 2);
	/// }
	/// ```
	pub async fn list_permissions(&self, username: &str, object_id: &str) -> Vec<String> {
		let perms = self.permissions.read().await;
		let key = (username.to_string(), object_id.to_string());
		perms.get(&key).cloned().unwrap_or_default()
	}
}

impl Default for ObjectPermissionManager {
	fn default() -> Self {
		Self::new()
	}
}

#[async_trait]
impl ObjectPermissionChecker for ObjectPermissionManager {
	async fn has_object_permission(
		&self,
		user: &dyn User,
		object_id: &str,
		permission: &str,
	) -> bool {
		let perms = self.permissions.read().await;
		let key = (user.username().to_string(), object_id.to_string());
		if let Some(user_perms) = perms.get(&key) {
			return user_perms.iter().any(|p| p == permission);
		}
		false
	}
}

/// Object permission with Permission trait support
///
/// Wraps an `ObjectPermissionChecker` for use with the `Permission` trait.
///
/// # Examples
///
/// ```
/// use reinhardt_auth::object_permissions::{ObjectPermission, ObjectPermissionManager};
/// use reinhardt_auth::{Permission, PermissionContext};
/// use reinhardt_auth::{SimpleUser, User};
/// use bytes::Bytes;
/// use hyper::{Method};
/// use reinhardt_http::Request;
/// use uuid::Uuid;
///
/// #[tokio::main]
/// async fn main() {
///     let mut manager = ObjectPermissionManager::new();
///     manager.grant_permission("alice", "article:123", "view").await;
///
///     let perm = ObjectPermission::new(manager, "article:123", "view");
///
///     let user = SimpleUser {
///         id: Uuid::now_v7(),
///         username: "alice".to_string(),
///         email: "alice@example.com".to_string(),
///         is_active: true,
///         is_admin: false,
///         is_staff: false,
///         is_superuser: false,
///     };
///
///     let request = Request::builder()
///         .method(Method::GET)
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
///         user: Some(Box::new(user)),
///     };
///
///     assert!(perm.has_permission(&context).await);
/// }
/// ```
pub struct ObjectPermission<T: ObjectPermissionChecker + Send + Sync> {
	checker: T,
	object_id: String,
	permission: String,
}

impl<T: ObjectPermissionChecker + Send + Sync> ObjectPermission<T> {
	/// Create a new object permission
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_auth::object_permissions::{ObjectPermission, ObjectPermissionManager};
	///
	/// let manager = ObjectPermissionManager::new();
	/// let perm = ObjectPermission::new(manager, "article:123", "view");
	/// ```
	pub fn new(checker: T, object_id: impl Into<String>, permission: impl Into<String>) -> Self {
		Self {
			checker,
			object_id: object_id.into(),
			permission: permission.into(),
		}
	}
}

#[async_trait]
impl<T: ObjectPermissionChecker + Send + Sync> Permission for ObjectPermission<T> {
	async fn has_permission(&self, context: &PermissionContext<'_>) -> bool {
		if !context.is_authenticated {
			return false;
		}

		if let Some(ref user) = context.user {
			return self
				.checker
				.has_object_permission(user.as_ref(), &self.object_id, &self.permission)
				.await;
		}

		false
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::SimpleUser;
	use bytes::Bytes;
	use hyper::Method;
	use reinhardt_http::Request;
	use rstest::rstest;
	use uuid::Uuid;

	#[tokio::test]
	async fn test_object_permission_manager_grant() {
		let mut manager = ObjectPermissionManager::new();
		manager
			.grant_permission("alice", "article:123", "view")
			.await;
		manager
			.grant_permission("alice", "article:123", "change")
			.await;

		let user = SimpleUser {
			id: Uuid::now_v7(),
			username: "alice".to_string(),
			email: "alice@example.com".to_string(),
			is_active: true,
			is_admin: false,
			is_staff: false,
			is_superuser: false,
		};

		assert!(
			manager
				.has_object_permission(&user, "article:123", "view")
				.await
		);
		assert!(
			manager
				.has_object_permission(&user, "article:123", "change")
				.await
		);
		assert!(
			!manager
				.has_object_permission(&user, "article:123", "delete")
				.await
		);
	}

	#[tokio::test]
	async fn test_object_permission_manager_revoke() {
		let mut manager = ObjectPermissionManager::new();
		manager
			.grant_permission("alice", "article:123", "view")
			.await;
		manager
			.grant_permission("alice", "article:123", "change")
			.await;

		let user = SimpleUser {
			id: Uuid::now_v7(),
			username: "alice".to_string(),
			email: "alice@example.com".to_string(),
			is_active: true,
			is_admin: false,
			is_staff: false,
			is_superuser: false,
		};

		manager
			.revoke_permission("alice", "article:123", "view")
			.await;

		assert!(
			!manager
				.has_object_permission(&user, "article:123", "view")
				.await
		);
		assert!(
			manager
				.has_object_permission(&user, "article:123", "change")
				.await
		);
	}

	#[tokio::test]
	async fn test_object_permission_manager_revoke_all() {
		let mut manager = ObjectPermissionManager::new();
		manager
			.grant_permission("alice", "article:123", "view")
			.await;
		manager
			.grant_permission("alice", "article:123", "change")
			.await;

		let user = SimpleUser {
			id: Uuid::now_v7(),
			username: "alice".to_string(),
			email: "alice@example.com".to_string(),
			is_active: true,
			is_admin: false,
			is_staff: false,
			is_superuser: false,
		};

		manager.revoke_all_permissions("alice", "article:123").await;

		assert!(
			!manager
				.has_object_permission(&user, "article:123", "view")
				.await
		);
		assert!(
			!manager
				.has_object_permission(&user, "article:123", "change")
				.await
		);
	}

	#[tokio::test]
	async fn test_object_permission_manager_list() {
		let mut manager = ObjectPermissionManager::new();
		manager
			.grant_permission("alice", "article:123", "view")
			.await;
		manager
			.grant_permission("alice", "article:123", "change")
			.await;

		let perms = manager.list_permissions("alice", "article:123").await;
		assert_eq!(perms.len(), 2);
		assert!(perms.contains(&"view".to_string()));
		assert!(perms.contains(&"change".to_string()));
	}

	#[tokio::test]
	async fn test_object_permission_manager_different_objects() {
		let mut manager = ObjectPermissionManager::new();
		manager
			.grant_permission("alice", "article:123", "view")
			.await;
		manager
			.grant_permission("alice", "article:456", "change")
			.await;

		let user = SimpleUser {
			id: Uuid::now_v7(),
			username: "alice".to_string(),
			email: "alice@example.com".to_string(),
			is_active: true,
			is_admin: false,
			is_staff: false,
			is_superuser: false,
		};

		assert!(
			manager
				.has_object_permission(&user, "article:123", "view")
				.await
		);
		assert!(
			!manager
				.has_object_permission(&user, "article:123", "change")
				.await
		);
		assert!(
			!manager
				.has_object_permission(&user, "article:456", "view")
				.await
		);
		assert!(
			manager
				.has_object_permission(&user, "article:456", "change")
				.await
		);
	}

	#[tokio::test]
	async fn test_object_permission_trait_authenticated() {
		let mut manager = ObjectPermissionManager::new();
		manager
			.grant_permission("alice", "article:123", "view")
			.await;

		let perm = ObjectPermission::new(manager, "article:123", "view");

		let user = SimpleUser {
			id: Uuid::now_v7(),
			username: "alice".to_string(),
			email: "alice@example.com".to_string(),
			is_active: true,
			is_admin: false,
			is_staff: false,
			is_superuser: false,
		};

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
			user: Some(Box::new(user)),
		};

		assert!(perm.has_permission(&context).await);
	}

	#[tokio::test]
	async fn test_object_permission_trait_unauthenticated() {
		let manager = ObjectPermissionManager::new();
		let perm = ObjectPermission::new(manager, "article:123", "view");

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
	async fn test_object_permission_trait_no_permission() {
		let manager = ObjectPermissionManager::new();
		let perm = ObjectPermission::new(manager, "article:123", "delete");

		let user = SimpleUser {
			id: Uuid::now_v7(),
			username: "alice".to_string(),
			email: "alice@example.com".to_string(),
			is_active: true,
			is_admin: false,
			is_staff: false,
			is_superuser: false,
		};

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
			user: Some(Box::new(user)),
		};

		assert!(!perm.has_permission(&context).await);
	}

	#[rstest]
	#[tokio::test]
	async fn test_bulk_check_multiple_objects() {
		// Arrange
		let mut manager = ObjectPermissionManager::new();
		manager.grant_permission("alice", "article:1", "view").await;
		manager
			.grant_permission("alice", "article:2", "change")
			.await;
		// article:3 has no permissions granted for alice

		let user = SimpleUser {
			id: Uuid::now_v7(),
			username: "alice".to_string(),
			email: "alice@example.com".to_string(),
			is_active: true,
			is_admin: false,
			is_staff: false,
			is_superuser: false,
		};

		// Act
		let result_obj1 = manager
			.has_object_permission(&user, "article:1", "view")
			.await;
		let result_obj2 = manager
			.has_object_permission(&user, "article:2", "change")
			.await;
		let result_obj3 = manager
			.has_object_permission(&user, "article:3", "view")
			.await;

		// Assert
		assert!(result_obj1);
		assert!(result_obj2);
		assert!(!result_obj3);
	}

	#[rstest]
	#[tokio::test]
	async fn test_different_users_same_object() {
		// Arrange
		let mut manager = ObjectPermissionManager::new();
		manager
			.grant_permission("alice", "article:42", "view")
			.await;

		let user_a = SimpleUser {
			id: Uuid::now_v7(),
			username: "alice".to_string(),
			email: "alice@example.com".to_string(),
			is_active: true,
			is_admin: false,
			is_staff: false,
			is_superuser: false,
		};
		let user_b = SimpleUser {
			id: Uuid::now_v7(),
			username: "bob".to_string(),
			email: "bob@example.com".to_string(),
			is_active: true,
			is_admin: false,
			is_staff: false,
			is_superuser: false,
		};

		// Act
		let result_a = manager
			.has_object_permission(&user_a, "article:42", "view")
			.await;
		let result_b = manager
			.has_object_permission(&user_b, "article:42", "view")
			.await;

		// Assert
		assert!(result_a);
		assert!(!result_b);
	}
}
