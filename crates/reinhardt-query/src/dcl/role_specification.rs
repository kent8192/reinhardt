//! Role specification types for DCL statements
//!
//! This module provides types for specifying roles and users in GRANT/REVOKE
//! role membership statements.

/// Role specification for GRANT/REVOKE role membership
///
/// Represents a role or user that can be granted or revoked role membership.
///
/// # PostgreSQL Support
///
/// PostgreSQL supports all variants:
/// - `RoleName`: Regular role name
/// - `CurrentRole`: Special keyword `CURRENT_ROLE`
/// - `CurrentUser`: Special keyword `CURRENT_USER`
/// - `SessionUser`: Special keyword `SESSION_USER`
///
/// # MySQL Support
///
/// MySQL supports:
/// - `RoleName`: Regular role name or `'user'@'host'` format
/// - `CurrentUser`: Special keyword `CURRENT_USER`
///
/// MySQL does not support `CurrentRole` or `SessionUser`.
///
/// # Examples
///
/// ```
/// use reinhardt_query::dcl::RoleSpecification;
///
/// // Regular role name
/// let role = RoleSpecification::new("developer");
/// assert_eq!(role, RoleSpecification::RoleName("developer".to_string()));
///
/// // PostgreSQL special keywords
/// let current_role = RoleSpecification::current_role();
/// let current_user = RoleSpecification::current_user();
/// let session_user = RoleSpecification::session_user();
///
/// // MySQL user@host format
/// let mysql_user = RoleSpecification::new("'alice'@'localhost'");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum RoleSpecification {
	/// Regular role name
	///
	/// For MySQL, this can include the `'user'@'host'` format.
	RoleName(String),

	/// PostgreSQL: `CURRENT_ROLE` keyword
	///
	/// Not supported by MySQL.
	CurrentRole,

	/// PostgreSQL/MySQL: `CURRENT_USER` keyword
	CurrentUser,

	/// PostgreSQL: `SESSION_USER` keyword
	///
	/// Not supported by MySQL.
	SessionUser,
}

impl RoleSpecification {
	/// Create a new role specification with a role name
	///
	/// # Arguments
	///
	/// * `name` - The role name (can be `'user'@'host'` format for MySQL)
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_query::dcl::RoleSpecification;
	///
	/// let role = RoleSpecification::new("developer");
	/// let mysql_user = RoleSpecification::new("'alice'@'localhost'");
	/// ```
	pub fn new(name: impl Into<String>) -> Self {
		Self::RoleName(name.into())
	}

	/// Create a `CURRENT_ROLE` specification (PostgreSQL only)
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_query::dcl::RoleSpecification;
	///
	/// let spec = RoleSpecification::current_role();
	/// assert_eq!(spec, RoleSpecification::CurrentRole);
	/// ```
	pub fn current_role() -> Self {
		Self::CurrentRole
	}

	/// Create a `CURRENT_USER` specification
	///
	/// Supported by both PostgreSQL and MySQL.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_query::dcl::RoleSpecification;
	///
	/// let spec = RoleSpecification::current_user();
	/// assert_eq!(spec, RoleSpecification::CurrentUser);
	/// ```
	pub fn current_user() -> Self {
		Self::CurrentUser
	}

	/// Create a `SESSION_USER` specification (PostgreSQL only)
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_query::dcl::RoleSpecification;
	///
	/// let spec = RoleSpecification::session_user();
	/// assert_eq!(spec, RoleSpecification::SessionUser);
	/// ```
	pub fn session_user() -> Self {
		Self::SessionUser
	}
}

/// Drop behavior for REVOKE statements (PostgreSQL only)
///
/// Specifies how dependent privileges should be handled when revoking
/// role membership.
///
/// # PostgreSQL Support
///
/// PostgreSQL supports both variants:
/// - `Cascade`: Automatically revoke dependent privileges
/// - `Restrict`: Reject the operation if dependent privileges exist
///
/// # MySQL Support
///
/// MySQL does not support drop behavior clauses.
///
/// # Examples
///
/// ```
/// use reinhardt_query::dcl::DropBehavior;
///
/// let cascade = DropBehavior::Cascade;
/// let restrict = DropBehavior::Restrict;
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DropBehavior {
	/// `CASCADE`: Automatically revoke dependent privileges
	///
	/// When specified, the REVOKE operation will also revoke any privileges
	/// that depend on the membership being revoked.
	Cascade,

	/// `RESTRICT`: Reject if dependent privileges exist
	///
	/// When specified, the REVOKE operation will fail if there are any
	/// privileges that depend on the membership being revoked.
	Restrict,
}
