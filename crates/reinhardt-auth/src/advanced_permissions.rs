//! Advanced Permission System
//!
//! Provides object-level permissions, role-based access control,
//! and dynamic permission evaluation.

use crate::{Permission, PermissionContext};
use async_trait::async_trait;
use std::collections::{HashMap, HashSet};

/// Object-level permission
///
/// Checks permissions on specific object instances.
/// Requires both authentication AND the specific permission to be granted
/// for the given object.
///
/// # Examples
///
/// ```
/// use reinhardt_auth::advanced_permissions::ObjectPermission;
///
/// let mut perm = ObjectPermission::new("view", Some("article:123"));
/// perm.grant("alice", "view", "article:123");
/// assert_eq!(perm.permission(), "view");
/// assert_eq!(perm.object_id(), Some("article:123"));
/// ```
pub struct ObjectPermission {
	/// Permission name (e.g., "view", "edit", "delete")
	permission: String,
	/// Optional object identifier (e.g., "article:123")
	object_id: Option<String>,
	/// Granted permissions: (username, object_id) -> set of permission names
	grants: HashMap<(String, String), HashSet<String>>,
}

impl ObjectPermission {
	/// Create a new object permission
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_auth::advanced_permissions::ObjectPermission;
	///
	/// let perm = ObjectPermission::new("edit", Some("post:42"));
	/// assert_eq!(perm.permission(), "edit");
	/// assert_eq!(perm.object_id(), Some("post:42"));
	/// ```
	pub fn new(permission: impl Into<String>, object_id: Option<impl Into<String>>) -> Self {
		Self {
			permission: permission.into(),
			object_id: object_id.map(|id| id.into()),
			grants: HashMap::new(),
		}
	}

	/// Get permission name
	pub fn permission(&self) -> &str {
		&self.permission
	}

	/// Get object ID
	pub fn object_id(&self) -> Option<&str> {
		self.object_id.as_deref()
	}

	/// Grant a permission to a user for a specific object
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_auth::advanced_permissions::ObjectPermission;
	///
	/// let mut perm = ObjectPermission::new("view", Some("article:123"));
	/// perm.grant("alice", "view", "article:123");
	/// ```
	pub fn grant(
		&mut self,
		username: impl Into<String>,
		permission: impl Into<String>,
		object_id: impl Into<String>,
	) {
		let key = (username.into(), object_id.into());
		self.grants
			.entry(key)
			.or_default()
			.insert(permission.into());
	}

	/// Check if a user has a specific permission on a specific object
	pub fn user_has_object_permission(
		&self,
		username: &str,
		permission: &str,
		object_id: &str,
	) -> bool {
		self.grants
			.get(&(username.to_string(), object_id.to_string()))
			.is_some_and(|perms| perms.contains(permission))
	}
}

#[async_trait]
impl Permission for ObjectPermission {
	async fn has_permission(&self, context: &PermissionContext<'_>) -> bool {
		// Must be authenticated
		if !context.is_authenticated {
			return false;
		}

		// Must have a user in context to check object permissions
		let user = match &context.user {
			Some(u) => u,
			None => return false,
		};

		// If no object_id is specified, this is a class-level permission check;
		// authentication alone is sufficient
		let object_id = match &self.object_id {
			Some(id) => id,
			None => return true,
		};

		// Check if the user has the specific permission on this object
		self.user_has_object_permission(user.username(), &self.permission, object_id)
	}
}

/// Role-based permission
///
/// Assigns permissions based on user roles.
///
/// # Examples
///
/// ```
/// use reinhardt_auth::advanced_permissions::RoleBasedPermission;
///
/// let mut perm = RoleBasedPermission::new();
/// perm.add_role("admin", vec!["create", "read", "update", "delete"]);
/// perm.add_role("editor", vec!["read", "update"]);
/// perm.assign_user_role("alice", "admin");
///
/// assert!(perm.user_has_permission("alice", "create"));
/// assert!(perm.user_has_permission("alice", "delete"));
/// assert!(!perm.user_has_permission("bob", "read"));
/// ```
pub struct RoleBasedPermission {
	/// Roles mapped to their permissions
	roles: HashMap<String, Vec<String>>,
	/// User role mapping (username -> role)
	user_roles: HashMap<String, String>,
}

impl RoleBasedPermission {
	/// Create a new role-based permission system
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_auth::advanced_permissions::RoleBasedPermission;
	///
	/// let perm = RoleBasedPermission::new();
	/// ```
	pub fn new() -> Self {
		Self {
			roles: HashMap::new(),
			user_roles: HashMap::new(),
		}
	}

	/// Add a role with permissions
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_auth::advanced_permissions::RoleBasedPermission;
	///
	/// let mut perm = RoleBasedPermission::new();
	/// perm.add_role("admin", vec!["read", "write", "delete"]);
	/// ```
	pub fn add_role(&mut self, role: impl Into<String>, permissions: Vec<impl Into<String>>) {
		let perms = permissions.into_iter().map(|p| p.into()).collect();
		self.roles.insert(role.into(), perms);
	}

	/// Assign role to user
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_auth::advanced_permissions::RoleBasedPermission;
	///
	/// let mut perm = RoleBasedPermission::new();
	/// perm.add_role("user", vec!["read"]);
	/// perm.assign_user_role("alice", "user");
	/// ```
	pub fn assign_user_role(&mut self, username: impl Into<String>, role: impl Into<String>) {
		self.user_roles.insert(username.into(), role.into());
	}

	/// Check if user has specific permission
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_auth::advanced_permissions::RoleBasedPermission;
	///
	/// let mut perm = RoleBasedPermission::new();
	/// perm.add_role("editor", vec!["read", "write"]);
	/// perm.assign_user_role("bob", "editor");
	///
	/// assert!(perm.user_has_permission("bob", "read"));
	/// assert!(perm.user_has_permission("bob", "write"));
	/// assert!(!perm.user_has_permission("bob", "delete"));
	/// ```
	pub fn user_has_permission(&self, username: &str, permission: &str) -> bool {
		if let Some(role) = self.user_roles.get(username)
			&& let Some(perms) = self.roles.get(role)
		{
			return perms.iter().any(|p| p == permission);
		}
		false
	}
}

impl Default for RoleBasedPermission {
	fn default() -> Self {
		Self::new()
	}
}

#[async_trait]
impl Permission for RoleBasedPermission {
	async fn has_permission(&self, context: &PermissionContext<'_>) -> bool {
		if !context.is_authenticated {
			return false;
		}

		// Must have a user in context to check role-based permissions
		let user = match &context.user {
			Some(u) => u,
			None => return false,
		};

		// Check if the user has an assigned role with any permissions
		self.user_roles.contains_key(user.username())
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

	fn make_user(username: &str) -> Box<dyn crate::User> {
		Box::new(SimpleUser {
			id: Uuid::new_v4(),
			username: username.to_string(),
			email: format!("{}@example.com", username),
			is_active: true,
			is_admin: false,
			is_staff: false,
			is_superuser: false,
		})
	}

	#[rstest]
	fn test_object_permission_creation() {
		// Arrange & Act
		let perm = ObjectPermission::new("view", Some("article:123"));

		// Assert
		assert_eq!(perm.permission(), "view");
		assert_eq!(perm.object_id(), Some("article:123"));
	}

	#[rstest]
	fn test_object_permission_without_object_id() {
		// Arrange & Act
		let perm = ObjectPermission::new("create", None::<String>);

		// Assert
		assert_eq!(perm.permission(), "create");
		assert_eq!(perm.object_id(), None);
	}

	#[rstest]
	#[tokio::test]
	async fn test_object_permission_no_object_id_authenticated() {
		// Arrange - no object_id means class-level check, authentication alone suffices
		let perm = ObjectPermission::new("view", None::<String>);
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

		// Act & Assert
		assert!(perm.has_permission(&context).await);
	}

	#[rstest]
	#[tokio::test]
	async fn test_object_permission_denies_without_grant() {
		// Arrange - user is authenticated but has no grant for this object
		let perm = ObjectPermission::new("edit", Some("post:42"));
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

		// Act & Assert - authenticated but no specific grant, should deny
		assert!(!perm.has_permission(&context).await);
	}

	#[rstest]
	#[tokio::test]
	async fn test_object_permission_grants_with_matching_grant() {
		// Arrange - user has the specific permission on the object
		let mut perm = ObjectPermission::new("edit", Some("post:42"));
		perm.grant("alice", "edit", "post:42");

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

		// Act & Assert
		assert!(perm.has_permission(&context).await);
	}

	#[rstest]
	#[tokio::test]
	async fn test_object_permission_denies_wrong_user() {
		// Arrange - grant is for alice, but bob is requesting
		let mut perm = ObjectPermission::new("edit", Some("post:42"));
		perm.grant("alice", "edit", "post:42");

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
			user: Some(make_user("bob")),
		};

		// Act & Assert
		assert!(!perm.has_permission(&context).await);
	}

	#[rstest]
	#[tokio::test]
	async fn test_object_permission_unauthenticated() {
		// Arrange
		let perm = ObjectPermission::new("edit", Some("post:42"));
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

		// Act & Assert
		assert!(!perm.has_permission(&context).await);
	}

	#[rstest]
	#[tokio::test]
	async fn test_object_permission_authenticated_no_user() {
		// Arrange - authenticated but no user object in context
		let perm = ObjectPermission::new("edit", Some("post:42"));
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

		// Act & Assert - no user means we cannot check object permissions
		assert!(!perm.has_permission(&context).await);
	}

	#[rstest]
	fn test_role_based_permission_creation() {
		// Arrange & Act
		let perm = RoleBasedPermission::new();

		// Assert
		assert!(!perm.user_has_permission("alice", "read"));
	}

	#[rstest]
	fn test_role_based_permission_add_role() {
		// Arrange
		let mut perm = RoleBasedPermission::new();
		perm.add_role("admin", vec!["read", "write", "delete"]);
		perm.assign_user_role("alice", "admin");

		// Act & Assert
		assert!(perm.user_has_permission("alice", "read"));
		assert!(perm.user_has_permission("alice", "write"));
		assert!(perm.user_has_permission("alice", "delete"));
	}

	#[rstest]
	fn test_role_based_permission_different_roles() {
		// Arrange
		let mut perm = RoleBasedPermission::new();
		perm.add_role("admin", vec!["read", "write", "delete"]);
		perm.add_role("viewer", vec!["read"]);

		perm.assign_user_role("alice", "admin");
		perm.assign_user_role("bob", "viewer");

		// Act & Assert
		assert!(perm.user_has_permission("alice", "write"));
		assert!(perm.user_has_permission("bob", "read"));
		assert!(!perm.user_has_permission("bob", "write"));
	}

	#[rstest]
	fn test_role_based_permission_no_role() {
		// Arrange & Act
		let perm = RoleBasedPermission::new();

		// Assert
		assert!(!perm.user_has_permission("charlie", "read"));
	}

	#[rstest]
	#[tokio::test]
	async fn test_role_permission_trait_with_user_and_role() {
		// Arrange
		let mut perm = RoleBasedPermission::new();
		perm.add_role("user", vec!["read"]);
		perm.assign_user_role("alice", "user");

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

		// Act & Assert
		assert!(perm.has_permission(&context).await);
	}

	#[rstest]
	#[tokio::test]
	async fn test_role_permission_trait_denies_user_without_role() {
		// Arrange - bob has no assigned role
		let mut perm = RoleBasedPermission::new();
		perm.add_role("user", vec!["read"]);
		perm.assign_user_role("alice", "user");

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
			user: Some(make_user("bob")),
		};

		// Act & Assert - bob is authenticated but has no role
		assert!(!perm.has_permission(&context).await);
	}

	#[rstest]
	#[tokio::test]
	async fn test_role_permission_trait_unauthenticated() {
		// Arrange
		let perm = RoleBasedPermission::new();
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

		// Act & Assert
		assert!(!perm.has_permission(&context).await);
	}

	#[rstest]
	#[tokio::test]
	async fn test_role_permission_trait_no_user_in_context() {
		// Arrange - authenticated but no user object
		let mut perm = RoleBasedPermission::new();
		perm.add_role("user", vec!["read"]);

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

		// Act & Assert - no user means we cannot check role
		assert!(!perm.has_permission(&context).await);
	}
}
