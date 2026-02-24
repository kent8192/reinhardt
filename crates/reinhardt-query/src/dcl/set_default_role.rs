//! SET DEFAULT ROLE statement builder (MySQL only)
//!
//! This module provides a fluent API for building SET DEFAULT ROLE statements for MySQL.
//!
//! # MySQL Only
//!
//! SET DEFAULT ROLE is a MySQL-specific command that sets which roles should be
//! activated by default when a user connects.
//!
//! # PostgreSQL & SQLite
//!
//! These databases do not support SET DEFAULT ROLE. Attempting to generate SQL for
//! these backends will result in a panic.
//!
//! # Examples
//!
//! Set specific default roles:
//!
//! ```
//! use reinhardt_query::dcl::{SetDefaultRoleStatement, DefaultRoleSpec};
//!
//! let stmt = SetDefaultRoleStatement::new()
//!     .roles(DefaultRoleSpec::RoleList(vec!["app_role".to_string()]))
//!     .user("app_user@localhost");
//! ```
//!
//! Set all roles as default:
//!
//! ```
//! use reinhardt_query::dcl::{SetDefaultRoleStatement, DefaultRoleSpec};
//!
//! let stmt = SetDefaultRoleStatement::new()
//!     .roles(DefaultRoleSpec::All)
//!     .user("app_user@localhost");
//! ```

use super::validate_name;

/// Default role specification for SET DEFAULT ROLE statement
///
/// This enum specifies which roles should be set as default.
///
/// # Variants
///
/// - `` `RoleList` `` - Specific list of roles
/// - `` `All` `` - All granted roles
/// - `` `None` `` - No default roles
#[derive(Debug, Clone, PartialEq)]
pub enum DefaultRoleSpec {
	/// SET DEFAULT ROLE role1, role2, ... TO user
	RoleList(Vec<String>),
	/// SET DEFAULT ROLE ALL TO user
	All,
	/// SET DEFAULT ROLE NONE TO user
	None,
}

/// SET DEFAULT ROLE statement builder (MySQL only)
///
/// This struct provides a fluent API for building SET DEFAULT ROLE statements.
/// This is a MySQL-specific feature.
///
/// # MySQL
///
/// MySQL SET DEFAULT ROLE sets which roles are activated by default when users connect.
/// Supports:
/// - Specific roles: `SET DEFAULT ROLE role1, role2 TO user`
/// - All roles: `SET DEFAULT ROLE ALL TO user`
/// - No roles: `SET DEFAULT ROLE NONE TO user`
///
/// # Examples
///
/// Set specific default roles:
///
/// ```
/// use reinhardt_query::dcl::{SetDefaultRoleStatement, DefaultRoleSpec};
///
/// let stmt = SetDefaultRoleStatement::new()
///     .roles(DefaultRoleSpec::RoleList(vec!["app_role".to_string()]))
///     .user("app_user@localhost");
/// ```
///
/// Set all roles as default:
///
/// ```
/// use reinhardt_query::dcl::{SetDefaultRoleStatement, DefaultRoleSpec};
///
/// let stmt = SetDefaultRoleStatement::new()
///     .roles(DefaultRoleSpec::All)
///     .users(vec!["user1@localhost".to_string(), "user2@localhost".to_string()]);
/// ```
///
/// Clear default roles:
///
/// ```
/// use reinhardt_query::dcl::{SetDefaultRoleStatement, DefaultRoleSpec};
///
/// let stmt = SetDefaultRoleStatement::new()
///     .roles(DefaultRoleSpec::None)
///     .user("app_user@localhost");
/// ```
#[derive(Debug, Clone, Default)]
pub struct SetDefaultRoleStatement {
	/// Role specification (ALL, NONE, or specific roles)
	pub role_spec: Option<DefaultRoleSpec>,
	/// Target users (with optional @host)
	pub user_names: Vec<String>,
}

impl SetDefaultRoleStatement {
	/// Create a new SET DEFAULT ROLE statement
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_query::dcl::SetDefaultRoleStatement;
	///
	/// let stmt = SetDefaultRoleStatement::new();
	/// ```
	pub fn new() -> Self {
		Self::default()
	}

	/// Set the role specification
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_query::dcl::{SetDefaultRoleStatement, DefaultRoleSpec};
	///
	/// let stmt = SetDefaultRoleStatement::new()
	///     .roles(DefaultRoleSpec::RoleList(vec!["app_role".to_string()]));
	/// ```
	pub fn roles(mut self, spec: DefaultRoleSpec) -> Self {
		self.role_spec = Some(spec);
		self
	}

	/// Add a single target user
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_query::dcl::{SetDefaultRoleStatement, DefaultRoleSpec};
	///
	/// let stmt = SetDefaultRoleStatement::new()
	///     .roles(DefaultRoleSpec::All)
	///     .user("app_user@localhost");
	/// ```
	pub fn user(mut self, name: impl Into<String>) -> Self {
		self.user_names.push(name.into());
		self
	}

	/// Set all target users at once
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_query::dcl::{SetDefaultRoleStatement, DefaultRoleSpec};
	///
	/// let stmt = SetDefaultRoleStatement::new()
	///     .roles(DefaultRoleSpec::All)
	///     .users(vec!["user1@localhost".to_string(), "user2@localhost".to_string()]);
	/// ```
	pub fn users(mut self, names: Vec<String>) -> Self {
		self.user_names = names;
		self
	}

	/// Validate the SET DEFAULT ROLE statement
	///
	/// # Validation Rules
	///
	/// 1. Role specification must be set
	/// 2. At least one user must be specified
	/// 3. For RoleList variant, role list cannot be empty
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_query::dcl::{SetDefaultRoleStatement, DefaultRoleSpec};
	///
	/// let stmt = SetDefaultRoleStatement::new()
	///     .roles(DefaultRoleSpec::All)
	///     .user("app_user@localhost");
	///
	/// assert!(stmt.validate().is_ok());
	/// ```
	///
	/// ```
	/// use reinhardt_query::dcl::SetDefaultRoleStatement;
	///
	/// let stmt = SetDefaultRoleStatement::new();
	/// assert!(stmt.validate().is_err());
	/// ```
	pub fn validate(&self) -> Result<(), String> {
		if self.role_spec.is_none() {
			return Err("Role specification must be set".to_string());
		}
		if self.user_names.is_empty() {
			return Err("At least one user must be specified".to_string());
		}
		for user_name in &self.user_names {
			validate_name(user_name, "User name")?;
		}
		if let Some(DefaultRoleSpec::RoleList(roles)) = &self.role_spec
			&& roles.is_empty()
		{
			return Err("Role list cannot be empty".to_string());
		}
		Ok(())
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_set_default_role_new() {
		let stmt = SetDefaultRoleStatement::new();
		assert!(stmt.role_spec.is_none());
		assert!(stmt.user_names.is_empty());
	}

	#[test]
	fn test_set_default_role_basic() {
		let stmt = SetDefaultRoleStatement::new()
			.roles(DefaultRoleSpec::All)
			.user("app_user@localhost");
		assert!(matches!(stmt.role_spec, Some(DefaultRoleSpec::All)));
		assert_eq!(stmt.user_names.len(), 1);
		assert!(stmt.validate().is_ok());
	}

	#[test]
	fn test_set_default_role_role_list() {
		let stmt = SetDefaultRoleStatement::new()
			.roles(DefaultRoleSpec::RoleList(vec!["role1".to_string()]))
			.user("app_user");
		assert!(matches!(stmt.role_spec, Some(DefaultRoleSpec::RoleList(_))));
		assert!(stmt.validate().is_ok());
	}

	#[test]
	fn test_set_default_role_none() {
		let stmt = SetDefaultRoleStatement::new()
			.roles(DefaultRoleSpec::None)
			.user("app_user");
		assert!(matches!(stmt.role_spec, Some(DefaultRoleSpec::None)));
		assert!(stmt.validate().is_ok());
	}

	#[test]
	fn test_set_default_role_multiple_users() {
		let stmt = SetDefaultRoleStatement::new()
			.roles(DefaultRoleSpec::All)
			.user("user1")
			.user("user2");
		assert_eq!(stmt.user_names.len(), 2);
	}

	#[test]
	fn test_set_default_role_validation_no_spec() {
		let stmt = SetDefaultRoleStatement::new().user("app_user");
		assert!(stmt.validate().is_err());
	}

	#[test]
	fn test_set_default_role_validation_no_users() {
		let stmt = SetDefaultRoleStatement::new().roles(DefaultRoleSpec::All);
		assert!(stmt.validate().is_err());
	}

	#[test]
	fn test_set_default_role_validation_empty_role_list() {
		let stmt = SetDefaultRoleStatement::new()
			.roles(DefaultRoleSpec::RoleList(vec![]))
			.user("app_user");
		assert!(stmt.validate().is_err());
	}
}
