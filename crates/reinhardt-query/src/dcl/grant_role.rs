//! GRANT role membership statement builder
//!
//! This module provides a type-safe builder for `GRANT role TO user` statements.

use super::RoleSpecification;

/// GRANT role membership statement builder
///
/// Creates SQL `GRANT role TO user` statements for assigning role membership.
///
/// # PostgreSQL Syntax
///
/// ```sql
/// GRANT role_name [, ...] TO role_specification [, ...]
///     [ WITH ADMIN OPTION ]
///     [ GRANTED BY role_specification ]
/// ```
///
/// # MySQL Syntax
///
/// ```sql
/// GRANT role_name [, role_name] ... TO user_or_role [, user_or_role] ...
///     [ WITH ADMIN OPTION ]
/// ```
///
/// # Examples
///
/// ## Basic Usage
///
/// ```
/// use reinhardt_query::dcl::{GrantRoleStatement, RoleSpecification};
///
/// let stmt = GrantRoleStatement::new()
///     .role("developer")
///     .to(RoleSpecification::new("alice"));
///
/// assert!(stmt.validate().is_ok());
/// ```
///
/// ## Multiple Roles and Grantees
///
/// ```
/// use reinhardt_query::dcl::{GrantRoleStatement, RoleSpecification};
///
/// let stmt = GrantRoleStatement::new()
///     .roles(vec!["developer", "analyst"])
///     .to(RoleSpecification::new("alice"))
///     .to(RoleSpecification::new("bob"));
///
/// assert!(stmt.validate().is_ok());
/// ```
///
/// ## With Admin Option (PostgreSQL/MySQL)
///
/// ```
/// use reinhardt_query::dcl::{GrantRoleStatement, RoleSpecification};
///
/// let stmt = GrantRoleStatement::new()
///     .role("developer")
///     .to(RoleSpecification::new("alice"))
///     .with_admin_option();
///
/// assert!(stmt.validate().is_ok());
/// ```
///
/// ## Granted By (PostgreSQL only)
///
/// ```
/// use reinhardt_query::dcl::{GrantRoleStatement, RoleSpecification};
///
/// let stmt = GrantRoleStatement::new()
///     .role("developer")
///     .to(RoleSpecification::new("alice"))
///     .granted_by(RoleSpecification::current_user());
///
/// assert!(stmt.validate().is_ok());
/// ```
#[derive(Debug, Clone, Default)]
pub struct GrantRoleStatement {
	/// Roles to grant (multiple roles can be granted at once)
	pub roles: Vec<String>,

	/// Grantees (users/roles to receive the role membership)
	pub grantees: Vec<RoleSpecification>,

	/// `WITH ADMIN OPTION` flag
	///
	/// When enabled, grantees can grant this role to others.
	pub with_admin_option: bool,

	/// `GRANTED BY` clause (PostgreSQL only)
	///
	/// Specifies the grantor of the privilege.
	pub granted_by: Option<RoleSpecification>,
}

impl GrantRoleStatement {
	/// Create a new GRANT role statement builder
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_query::dcl::GrantRoleStatement;
	///
	/// let stmt = GrantRoleStatement::new();
	/// assert_eq!(stmt.roles.len(), 0);
	/// assert_eq!(stmt.grantees.len(), 0);
	/// assert_eq!(stmt.with_admin_option, false);
	/// assert!(stmt.granted_by.is_none());
	/// ```
	pub fn new() -> Self {
		Self::default()
	}

	/// Add a single role to grant
	///
	/// # Arguments
	///
	/// * `role_name` - The name of the role to grant
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_query::dcl::GrantRoleStatement;
	///
	/// let stmt = GrantRoleStatement::new()
	///     .role("developer");
	///
	/// assert_eq!(stmt.roles, vec!["developer"]);
	/// ```
	pub fn role(mut self, role_name: impl Into<String>) -> Self {
		self.roles.push(role_name.into());
		self
	}

	/// Add multiple roles to grant
	///
	/// # Arguments
	///
	/// * `role_names` - An iterator of role names
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_query::dcl::GrantRoleStatement;
	///
	/// let stmt = GrantRoleStatement::new()
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

	/// Add a single grantee (user/role to receive the membership)
	///
	/// # Arguments
	///
	/// * `grantee` - The grantee specification
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_query::dcl::{GrantRoleStatement, RoleSpecification};
	///
	/// let stmt = GrantRoleStatement::new()
	///     .to(RoleSpecification::new("alice"));
	///
	/// assert_eq!(stmt.grantees.len(), 1);
	/// ```
	pub fn to(mut self, grantee: RoleSpecification) -> Self {
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
	/// use reinhardt_query::dcl::{GrantRoleStatement, RoleSpecification};
	///
	/// let stmt = GrantRoleStatement::new()
	///     .to_all(vec![
	///         RoleSpecification::new("alice"),
	///         RoleSpecification::new("bob"),
	///     ]);
	///
	/// assert_eq!(stmt.grantees.len(), 2);
	/// ```
	pub fn to_all<I>(mut self, grantees: I) -> Self
	where
		I: IntoIterator<Item = RoleSpecification>,
	{
		self.grantees.extend(grantees);
		self
	}

	/// Enable `WITH ADMIN OPTION`
	///
	/// Allows the grantee to grant this role to other users.
	///
	/// Supported by both PostgreSQL and MySQL.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_query::dcl::GrantRoleStatement;
	///
	/// let stmt = GrantRoleStatement::new()
	///     .with_admin_option();
	///
	/// assert!(stmt.with_admin_option);
	/// ```
	pub fn with_admin_option(mut self) -> Self {
		self.with_admin_option = true;
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
	/// use reinhardt_query::dcl::{GrantRoleStatement, RoleSpecification};
	///
	/// let stmt = GrantRoleStatement::new()
	///     .granted_by(RoleSpecification::current_user());
	///
	/// assert!(stmt.granted_by.is_some());
	/// ```
	pub fn granted_by(mut self, grantor: RoleSpecification) -> Self {
		self.granted_by = Some(grantor);
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
	/// use reinhardt_query::dcl::{GrantRoleStatement, RoleSpecification};
	///
	/// // Valid statement
	/// let stmt = GrantRoleStatement::new()
	///     .role("developer")
	///     .to(RoleSpecification::new("alice"));
	/// assert!(stmt.validate().is_ok());
	///
	/// // Invalid: no roles
	/// let stmt = GrantRoleStatement::new()
	///     .to(RoleSpecification::new("alice"));
	/// assert!(stmt.validate().is_err());
	///
	/// // Invalid: no grantees
	/// let stmt = GrantRoleStatement::new()
	///     .role("developer");
	/// assert!(stmt.validate().is_err());
	/// ```
	pub fn validate(&self) -> Result<(), String> {
		if self.roles.is_empty() {
			return Err("At least one role must be specified".to_string());
		}

		for role in &self.roles {
			if role.trim().is_empty() {
				return Err("Role name cannot be empty or whitespace only".to_string());
			}
		}

		if self.grantees.is_empty() {
			return Err("At least one grantee must be specified".to_string());
		}

		Ok(())
	}
}
