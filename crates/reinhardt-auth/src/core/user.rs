use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// User trait - Core authentication trait
///
/// This trait defines the basic interface that all user types must implement
/// in the Reinhardt authentication system. It provides methods for checking
/// user status and permissions.
///
/// # Examples
///
/// ```
/// use reinhardt_auth::{User, SimpleUser};
/// use uuid::Uuid;
///
/// let user = SimpleUser {
///     id: Uuid::new_v4(),
///     username: "alice".to_string(),
///     email: "alice@example.com".to_string(),
///     is_active: true,
///     is_admin: false,
///     is_staff: false,
///     is_superuser: false,
/// };
///
/// assert!(user.is_authenticated());
/// assert!(user.is_active());
/// assert_eq!(user.username(), "alice");
/// ```
pub trait User: Send + Sync {
	/// Returns the unique identifier for this user
	fn id(&self) -> String;

	/// Returns the username for this user
	fn username(&self) -> &str;

	/// Returns the username (alias for `username()`)
	///
	/// This method exists for Django compatibility.
	fn get_username(&self) -> &str {
		self.username()
	}

	/// Returns whether this user is authenticated
	///
	/// For concrete user types, this should always return `true`.
	/// `AnonymousUser` should return `false`.
	fn is_authenticated(&self) -> bool;

	/// Returns whether this user account is active
	///
	/// Inactive users cannot log in and should be denied access.
	fn is_active(&self) -> bool;

	/// Returns whether this user is an administrator
	///
	/// Admin users typically have elevated privileges in the system.
	fn is_admin(&self) -> bool;

	/// Returns whether this user is a staff member
	///
	/// Staff members typically have access to the admin interface.
	fn is_staff(&self) -> bool;

	/// Returns whether this user is a superuser
	///
	/// Superusers have all permissions without explicit assignment.
	fn is_superuser(&self) -> bool;
}

/// Simple user implementation with basic fields
///
/// This is a lightweight user struct suitable for most applications.
///
/// # Examples
///
/// ```
/// use reinhardt_auth::{User, SimpleUser};
/// use uuid::Uuid;
///
/// let user = SimpleUser {
///     id: Uuid::new_v4(),
///     username: "bob".to_string(),
///     email: "bob@example.com".to_string(),
///     is_active: true,
///     is_admin: true,
///     is_staff: true,
///     is_superuser: false,
/// };
///
/// assert!(user.is_authenticated());
/// assert!(user.is_admin());
/// assert!(user.is_staff());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SimpleUser {
	pub id: Uuid,
	pub username: String,
	pub email: String,
	pub is_active: bool,
	pub is_admin: bool,
	pub is_staff: bool,
	pub is_superuser: bool,
}

impl User for SimpleUser {
	fn id(&self) -> String {
		self.id.to_string()
	}

	fn username(&self) -> &str {
		&self.username
	}

	fn is_authenticated(&self) -> bool {
		true
	}

	fn is_active(&self) -> bool {
		self.is_active
	}

	fn is_admin(&self) -> bool {
		self.is_admin
	}

	fn is_staff(&self) -> bool {
		self.is_staff
	}

	fn is_superuser(&self) -> bool {
		self.is_superuser
	}
}

/// Anonymous user - represents a non-authenticated visitor
///
/// This type is used to represent users who are not logged in.
/// All permission checks return `false`, and `is_authenticated()` returns `false`.
///
/// # Examples
///
/// ```
/// use reinhardt_auth::{User, AnonymousUser};
///
/// let anon = AnonymousUser;
///
/// assert!(!anon.is_authenticated());
/// assert!(!anon.is_active());
/// assert!(!anon.is_admin());
/// assert_eq!(anon.username(), "");
/// assert_eq!(anon.id(), "");
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnonymousUser;

impl User for AnonymousUser {
	fn id(&self) -> String {
		String::new()
	}

	fn username(&self) -> &str {
		""
	}

	fn is_authenticated(&self) -> bool {
		false
	}

	fn is_active(&self) -> bool {
		false
	}

	fn is_admin(&self) -> bool {
		false
	}

	fn is_staff(&self) -> bool {
		false
	}

	fn is_superuser(&self) -> bool {
		false
	}
}
