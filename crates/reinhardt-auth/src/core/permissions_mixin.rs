use std::collections::HashSet;

/// PermissionsMixin trait - Django's PermissionsMixin equivalent
///
/// Provides permission management functionality for user models. This trait can be composed
/// with `BaseUser` or `FullUser` to add Django-style permission checking capabilities.
///
/// # Permission System
///
/// Reinhardt's permission system is inspired by Django's:
/// - Permissions are strings in the format `"app_label.permission_name"` (e.g., `"blog.add_post"`)
/// - Users can have permissions directly assigned or inherit them from groups
/// - Superusers automatically have all permissions
///
/// # Examples
///
/// Implementing PermissionsMixin for a custom user:
///
/// ```
/// use reinhardt_auth::{BaseUser, PermissionsMixin, PasswordHasher};
/// #[cfg(feature = "argon2-hasher")]
/// use reinhardt_auth::Argon2Hasher;
/// use uuid::Uuid;
/// use chrono::{DateTime, Utc};
/// use serde::{Serialize, Deserialize};
///
/// #[derive(Serialize, Deserialize)]
/// struct MyUser {
///     id: Uuid,
///     email: String,
///     password_hash: Option<String>,
///     last_login: Option<DateTime<Utc>>,
///     is_active: bool,
///     is_superuser: bool,
///     user_permissions: Vec<String>,
///     groups: Vec<String>,
/// }
///
/// #[cfg(feature = "argon2-hasher")]
/// impl BaseUser for MyUser {
///     type PrimaryKey = Uuid;
///     type Hasher = Argon2Hasher;
///
///     fn get_username_field() -> &'static str { "email" }
///     fn get_username(&self) -> &str { &self.email }
///     fn password_hash(&self) -> Option<&str> { self.password_hash.as_deref() }
///     fn set_password_hash(&mut self, hash: String) { self.password_hash = Some(hash); }
///     fn last_login(&self) -> Option<DateTime<Utc>> { self.last_login }
///     fn set_last_login(&mut self, time: DateTime<Utc>) { self.last_login = Some(time); }
///     fn is_active(&self) -> bool { self.is_active }
/// }
///
/// impl PermissionsMixin for MyUser {
///     fn is_superuser(&self) -> bool { self.is_superuser }
///     fn user_permissions(&self) -> &[String] { &self.user_permissions }
///     fn groups(&self) -> &[String] { &self.groups }
/// }
///
/// # #[cfg(feature = "argon2-hasher")]
/// # {
/// let mut user = MyUser {
///     id: Uuid::now_v7(),
///     email: "admin@example.com".to_string(),
///     password_hash: None,
///     last_login: None,
///     is_active: true,
///     is_superuser: false,
///     user_permissions: vec!["blog.add_post".to_string(), "blog.edit_post".to_string()],
///     groups: vec![],
/// };
///
/// assert!(user.has_perm("blog.add_post"));
/// assert!(user.has_perm("blog.edit_post"));
/// assert!(!user.has_perm("blog.delete_post"));
///
/// // Superusers have all permissions
/// user.is_superuser = true;
/// assert!(user.has_perm("blog.delete_post"));
/// assert!(user.has_perm("any.permission"));
/// # }
/// ```
pub trait PermissionsMixin: Send + Sync {
	/// Returns whether this user is a superuser
	///
	/// Superusers automatically have all permissions without explicit assignment.
	fn is_superuser(&self) -> bool;

	/// Returns the list of permissions directly assigned to this user
	///
	/// Permissions are typically in the format `"app_label.permission_name"`.
	/// For example: `"blog.add_post"`, `"blog.edit_post"`, `"auth.change_user"`.
	fn user_permissions(&self) -> &[String];

	/// Returns the list of groups this user belongs to
	///
	/// Groups are used to assign permissions to multiple users at once.
	/// This method returns group names or identifiers.
	fn groups(&self) -> &[String];

	/// Returns all permissions directly assigned to this user
	///
	/// This does not include group permissions. Use `get_all_permissions()` for the complete set.
	fn get_user_permissions(&self) -> HashSet<String> {
		self.user_permissions().iter().cloned().collect()
	}

	/// Returns all permissions from groups this user belongs to
	///
	/// When a global `GroupManager` is registered via
	/// [`register_group_manager`](crate::register_group_manager), this method
	/// automatically resolves permissions for the groups returned by
	/// [`groups()`](Self::groups). Otherwise returns an empty set.
	fn get_group_permissions(&self) -> HashSet<String> {
		if let Some(manager) = crate::group_management::get_group_manager() {
			manager.get_permissions_for_groups_sync(self.groups())
		} else {
			HashSet::new()
		}
	}

	/// Returns all permissions for this user (user permissions + group permissions)
	///
	/// This is the union of permissions directly assigned to the user and permissions
	/// inherited from groups.
	fn get_all_permissions(&self) -> HashSet<String> {
		let mut perms = self.get_user_permissions();
		perms.extend(self.get_group_permissions());
		perms
	}

	/// Checks if this user has a specific permission
	///
	/// Superusers always return `true` regardless of the permission.
	/// For other users, checks if the permission exists in their complete permission set.
	///
	/// # Arguments
	///
	/// * `perm` - Permission string in the format `"app_label.permission_name"`
	fn has_perm(&self, perm: &str) -> bool {
		if self.is_superuser() {
			return true;
		}
		self.get_all_permissions().contains(perm)
	}

	/// Checks if this user has all of the specified permissions
	///
	/// Superusers always return `true` regardless of the permissions.
	/// For other users, checks if all permissions exist in their complete permission set.
	///
	/// # Arguments
	///
	/// * `perms` - Slice of permission strings
	fn has_perms(&self, perms: &[&str]) -> bool {
		if self.is_superuser() {
			return true;
		}
		let all_perms = self.get_all_permissions();
		perms.iter().all(|p| all_perms.contains(*p))
	}

	/// Checks if this user has any permission for a specific app/module
	///
	/// Superusers always return `true`.
	/// For other users, checks if they have any permission starting with `"app_label."`.
	///
	/// # Arguments
	///
	/// * `app_label` - The application label (e.g., `"blog"`, `"auth"`)
	fn has_module_perms(&self, app_label: &str) -> bool {
		if self.is_superuser() {
			return true;
		}
		self.get_all_permissions()
			.iter()
			.any(|p| p.starts_with(&format!("{}.", app_label)))
	}
}
