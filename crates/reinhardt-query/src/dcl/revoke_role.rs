//! REVOKE role membership statement builder
//!
//! This module provides a type-safe builder for `REVOKE role FROM user` statements.

use super::{DropBehavior, RoleSpecification};

/// REVOKE role membership statement builder
///
/// Creates SQL `REVOKE role FROM user` statements for removing role membership.
///
/// # PostgreSQL Syntax
///
/// ```sql
/// REVOKE [ ADMIN OPTION FOR ] role_name [, ...] FROM role_specification [, ...]
///     [ GRANTED BY role_specification ]
///     [ CASCADE | RESTRICT ]
/// ```
///
/// # MySQL Syntax
///
/// ```sql
/// REVOKE [ ADMIN OPTION FOR ] role_name [, role_name] ... FROM user_or_role [, user_or_role] ...
/// ```
///
/// # Examples
///
/// ## Basic Usage
///
/// ```
/// use reinhardt_query::dcl::{RevokeRoleStatement, RoleSpecification};
///
/// let stmt = RevokeRoleStatement::new()
///     .role("developer")
///     .from(RoleSpecification::new("alice"));
///
/// assert!(stmt.validate().is_ok());
/// ```
///
/// ## Multiple Roles and Grantees
///
/// ```
/// use reinhardt_query::dcl::{RevokeRoleStatement, RoleSpecification};
///
/// let stmt = RevokeRoleStatement::new()
///     .roles(vec!["developer", "analyst"])
///     .from(RoleSpecification::new("alice"))
///     .from(RoleSpecification::new("bob"));
///
/// assert!(stmt.validate().is_ok());
/// ```
///
/// ## Admin Option For (PostgreSQL/MySQL)
///
/// ```
/// use reinhardt_query::dcl::{RevokeRoleStatement, RoleSpecification};
///
/// let stmt = RevokeRoleStatement::new()
///     .role("developer")
///     .from(RoleSpecification::new("alice"))
///     .admin_option_for();
///
/// assert!(stmt.validate().is_ok());
/// ```
///
/// ## Cascade (PostgreSQL only)
///
/// ```
/// use reinhardt_query::dcl::{RevokeRoleStatement, RoleSpecification};
///
/// let stmt = RevokeRoleStatement::new()
///     .role("developer")
///     .from(RoleSpecification::new("alice"))
///     .cascade();
///
/// assert!(stmt.validate().is_ok());
/// ```
///
/// ## Restrict (PostgreSQL only)
///
/// ```
/// use reinhardt_query::dcl::{RevokeRoleStatement, RoleSpecification};
///
/// let stmt = RevokeRoleStatement::new()
///     .role("developer")
///     .from(RoleSpecification::new("alice"))
///     .restrict();
///
/// assert!(stmt.validate().is_ok());
/// ```
#[derive(Debug, Clone, Default)]
pub struct RevokeRoleStatement {
	/// Roles to revoke (multiple roles can be revoked at once)
	pub roles: Vec<String>,

	/// Grantees (users/roles to lose the role membership)
	pub grantees: Vec<RoleSpecification>,

	/// `ADMIN OPTION FOR` flag
	///
	/// When enabled, only the admin option is revoked, not the role itself.
	pub admin_option_for: bool,

	/// `GRANTED BY` clause (PostgreSQL only)
	///
	/// Specifies the grantor of the privilege.
	pub granted_by: Option<RoleSpecification>,

	/// Drop behavior: `CASCADE` or `RESTRICT` (PostgreSQL only)
	pub drop_behavior: Option<DropBehavior>,
}

impl RevokeRoleStatement {
	/// Create a new REVOKE role statement builder
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_query::dcl::RevokeRoleStatement;
	///
	/// let stmt = RevokeRoleStatement::new();
	/// assert_eq!(stmt.roles.len(), 0);
	/// assert_eq!(stmt.grantees.len(), 0);
	/// assert_eq!(stmt.admin_option_for, false);
	/// assert!(stmt.granted_by.is_none());
	/// assert!(stmt.drop_behavior.is_none());
	/// ```
	pub fn new() -> Self {
		Self::default()
	}

	/// Add a single role to revoke
	///
	/// # Arguments
	///
	/// * `role_name` - The name of the role to revoke
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_query::dcl::RevokeRoleStatement;
	///
	/// let stmt = RevokeRoleStatement::new()
	///     .role("developer");
	///
	/// assert_eq!(stmt.roles, vec!["developer"]);
	/// ```
	pub fn role(mut self, role_name: impl Into<String>) -> Self {
		self.roles.push(role_name.into());
		self
	}

	/// Add multiple roles to revoke
	///
	/// # Arguments
	///
	/// * `role_names` - An iterator of role names
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_query::dcl::RevokeRoleStatement;
	///
	/// let stmt = RevokeRoleStatement::new()
	///     .roles(vec!["developer", "analyst"]);
	///
	/// assert_eq!(stmt.roles, vec!["developer", "analyst"]);
	/// ```
	pub fn roles<I, S>(mut self, role_names: I) -> Self
	where
		I: IntoIterator<Item = S>,
		S: Into<String>,
	{
		self.roles.extend(role_names.into_iter().map(Into::into));
		self
	}

	/// Add a single grantee (user/role to lose the membership)
	///
	/// # Arguments
	///
	/// * `grantee` - The grantee specification
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_query::dcl::{RevokeRoleStatement, RoleSpecification};
	///
	/// let stmt = RevokeRoleStatement::new()
	///     .from(RoleSpecification::new("alice"));
	///
	/// assert_eq!(stmt.grantees.len(), 1);
	/// ```
	pub fn from(mut self, grantee: RoleSpecification) -> Self {
		self.grantees.push(grantee);
		self
	}

	/// Add multiple grantees
	///
	/// # Arguments
	///
	/// * `grantees` - An iterator of grantee specifications
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_query::dcl::{RevokeRoleStatement, RoleSpecification};
	///
	/// let stmt = RevokeRoleStatement::new()
	///     .from_all(vec![
	///         RoleSpecification::new("alice"),
	///         RoleSpecification::new("bob"),
	///     ]);
	///
	/// assert_eq!(stmt.grantees.len(), 2);
	/// ```
	pub fn from_all<I>(mut self, grantees: I) -> Self
	where
		I: IntoIterator<Item = RoleSpecification>,
	{
		self.grantees.extend(grantees);
		self
	}

	/// Enable `ADMIN OPTION FOR`
	///
	/// Revokes only the admin option, not the role membership itself.
	///
	/// Supported by both PostgreSQL and MySQL.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_query::dcl::RevokeRoleStatement;
	///
	/// let stmt = RevokeRoleStatement::new()
	///     .admin_option_for();
	///
	/// assert!(stmt.admin_option_for);
	/// ```
	pub fn admin_option_for(mut self) -> Self {
		self.admin_option_for = true;
		self
	}

	/// Set the grantor with `GRANTED BY` clause (PostgreSQL only)
	///
	/// # Arguments
	///
	/// * `grantor` - The role specification for the grantor
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_query::dcl::{RevokeRoleStatement, RoleSpecification};
	///
	/// let stmt = RevokeRoleStatement::new()
	///     .granted_by(RoleSpecification::current_user());
	///
	/// assert!(stmt.granted_by.is_some());
	/// ```
	pub fn granted_by(mut self, grantor: RoleSpecification) -> Self {
		self.granted_by = Some(grantor);
		self
	}

	/// Set drop behavior to `CASCADE` (PostgreSQL only)
	///
	/// Automatically revokes dependent privileges.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_query::dcl::{RevokeRoleStatement, DropBehavior};
	///
	/// let stmt = RevokeRoleStatement::new()
	///     .cascade();
	///
	/// assert_eq!(stmt.drop_behavior, Some(DropBehavior::Cascade));
	/// ```
	pub fn cascade(mut self) -> Self {
		self.drop_behavior = Some(DropBehavior::Cascade);
		self
	}

	/// Set drop behavior to `RESTRICT` (PostgreSQL only)
	///
	/// Rejects the operation if dependent privileges exist.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_query::dcl::{RevokeRoleStatement, DropBehavior};
	///
	/// let stmt = RevokeRoleStatement::new()
	///     .restrict();
	///
	/// assert_eq!(stmt.drop_behavior, Some(DropBehavior::Restrict));
	/// ```
	pub fn restrict(mut self) -> Self {
		self.drop_behavior = Some(DropBehavior::Restrict);
		self
	}

	/// Validate the statement
	///
	/// Ensures that:
	/// 1. At least one role is specified
	/// 2. All role names are non-empty
	/// 3. At least one grantee is specified
	///
	/// # Returns
	///
	/// `Ok(())` if valid, `Err(String)` with error message if invalid
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_query::dcl::{RevokeRoleStatement, RoleSpecification};
	///
	/// // Valid statement
	/// let stmt = RevokeRoleStatement::new()
	///     .role("developer")
	///     .from(RoleSpecification::new("alice"));
	/// assert!(stmt.validate().is_ok());
	///
	/// // Invalid: no roles
	/// let stmt = RevokeRoleStatement::new()
	///     .from(RoleSpecification::new("alice"));
	/// assert!(stmt.validate().is_err());
	///
	/// // Invalid: no grantees
	/// let stmt = RevokeRoleStatement::new()
	///     .role("developer");
	/// assert!(stmt.validate().is_err());
	/// ```
	pub fn validate(&self) -> Result<(), String> {
		if self.roles.is_empty() {
			return Err("At least one role must be specified".to_string());
		}

		for role in &self.roles {
			if role.is_empty() {
				return Err("Role name cannot be empty".to_string());
			}
		}

		if self.grantees.is_empty() {
			return Err("At least one grantee must be specified".to_string());
		}

		Ok(())
	}
}
