//! CREATE USER statement builder
//!
//! This module provides a fluent API for building CREATE USER statements for both
//! PostgreSQL and MySQL databases.
//!
//! # PostgreSQL
//!
//! PostgreSQL doesn't have a separate CREATE USER command; it's an alias for CREATE ROLE WITH LOGIN.
//! This builder wraps CreateRoleStatement with the LOGIN attribute.
//!
//! # MySQL
//!
//! MySQL has a native CREATE USER command that supports user@host specification,
//! DEFAULT ROLE, and user options.
//!
//! # Examples
//!
//! PostgreSQL example:
//!
//! ```
//! use reinhardt_query::dcl::{CreateUserStatement, RoleAttribute};
//!
//! let stmt = CreateUserStatement::new()
//!     .user("app_user")
//!     .attribute(RoleAttribute::Password("secret".to_string()));
//! ```
//!
//! MySQL example:
//!
//! ```
//! use reinhardt_query::dcl::{CreateUserStatement, UserOption};
//!
//! let stmt = CreateUserStatement::new()
//!     .user("app_user@localhost")
//!     .if_not_exists(true)
//!     .default_role(vec!["app_role".to_string()])
//!     .option(UserOption::Comment("Application user".to_string()));
//! ```

use super::{RoleAttribute, UserOption, validate_name};

/// CREATE USER statement builder
///
/// This struct provides a fluent API for building CREATE USER statements.
///
/// # PostgreSQL
///
/// PostgreSQL CREATE USER is an alias for CREATE ROLE WITH LOGIN.
/// Use the `` `attribute()` `` method to add role attributes.
///
/// # MySQL
///
/// MySQL CREATE USER supports:
/// - IF NOT EXISTS clause
/// - User@host specification
/// - DEFAULT ROLE clause
/// - User options
///
/// # Examples
///
/// Create a simple user:
///
/// ```
/// use reinhardt_query::dcl::CreateUserStatement;
///
/// let stmt = CreateUserStatement::new()
///     .user("app_user");
/// ```
///
/// Create a user with password (PostgreSQL):
///
/// ```
/// use reinhardt_query::dcl::{CreateUserStatement, RoleAttribute};
///
/// let stmt = CreateUserStatement::new()
///     .user("app_user")
///     .attribute(RoleAttribute::Password("password123".to_string()))
///     .attribute(RoleAttribute::ConnectionLimit(5));
/// ```
///
/// Create a user with default role (MySQL):
///
/// ```
/// use reinhardt_query::dcl::{CreateUserStatement, UserOption};
///
/// let stmt = CreateUserStatement::new()
///     .user("app_user@localhost")
///     .if_not_exists(true)
///     .default_role(vec!["app_role".to_string()])
///     .option(UserOption::Comment("My application user".to_string()));
/// ```
#[derive(Debug, Clone, Default)]
pub struct CreateUserStatement {
	/// User name (with optional @host for MySQL)
	pub user_name: String,
	/// IF NOT EXISTS clause (MySQL only)
	pub if_not_exists: bool,
	/// PostgreSQL role attributes (PostgreSQL only)
	pub attributes: Vec<RoleAttribute>,
	/// MySQL DEFAULT ROLE clause (MySQL only)
	pub default_roles: Vec<String>,
	/// MySQL user options
	pub options: Vec<UserOption>,
}

impl CreateUserStatement {
	/// Create a new CREATE USER statement
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_query::dcl::CreateUserStatement;
	///
	/// let stmt = CreateUserStatement::new();
	/// ```
	pub fn new() -> Self {
		Self::default()
	}

	/// Set the user name
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_query::dcl::CreateUserStatement;
	///
	/// let stmt = CreateUserStatement::new()
	///     .user("app_user");
	/// ```
	///
	/// MySQL with host:
	///
	/// ```
	/// use reinhardt_query::dcl::CreateUserStatement;
	///
	/// let stmt = CreateUserStatement::new()
	///     .user("app_user@localhost");
	/// ```
	pub fn user(mut self, name: impl Into<String>) -> Self {
		self.user_name = name.into();
		self
	}

	/// Set IF NOT EXISTS flag (MySQL only)
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_query::dcl::CreateUserStatement;
	///
	/// let stmt = CreateUserStatement::new()
	///     .user("app_user")
	///     .if_not_exists(true);
	/// ```
	pub fn if_not_exists(mut self, flag: bool) -> Self {
		self.if_not_exists = flag;
		self
	}

	/// Add a single PostgreSQL role attribute
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_query::dcl::{CreateUserStatement, RoleAttribute};
	///
	/// let stmt = CreateUserStatement::new()
	///     .user("app_user")
	///     .attribute(RoleAttribute::Password("secret".to_string()))
	///     .attribute(RoleAttribute::CreateDb);
	/// ```
	pub fn attribute(mut self, attr: RoleAttribute) -> Self {
		self.attributes.push(attr);
		self
	}

	/// Set all PostgreSQL role attributes at once
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_query::dcl::{CreateUserStatement, RoleAttribute};
	///
	/// let stmt = CreateUserStatement::new()
	///     .user("app_user")
	///     .attributes(vec![
	///         RoleAttribute::Password("secret".to_string()),
	///         RoleAttribute::CreateDb,
	///         RoleAttribute::ConnectionLimit(10),
	///     ]);
	/// ```
	pub fn attributes(mut self, attrs: Vec<RoleAttribute>) -> Self {
		self.attributes = attrs;
		self
	}

	/// Set DEFAULT ROLE clause (MySQL only)
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_query::dcl::CreateUserStatement;
	///
	/// let stmt = CreateUserStatement::new()
	///     .user("app_user@localhost")
	///     .default_role(vec!["app_role".to_string(), "admin_role".to_string()]);
	/// ```
	pub fn default_role(mut self, roles: Vec<String>) -> Self {
		self.default_roles = roles;
		self
	}

	/// Add a single MySQL user option
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_query::dcl::{CreateUserStatement, UserOption};
	///
	/// let stmt = CreateUserStatement::new()
	///     .user("app_user")
	///     .option(UserOption::Comment("Application user".to_string()));
	/// ```
	pub fn option(mut self, opt: UserOption) -> Self {
		self.options.push(opt);
		self
	}

	/// Set all MySQL user options at once
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_query::dcl::{CreateUserStatement, UserOption};
	///
	/// let stmt = CreateUserStatement::new()
	///     .user("app_user")
	///     .options(vec![
	///         UserOption::Comment("Application user".to_string()),
	///         UserOption::AccountLock,
	///     ]);
	/// ```
	pub fn options(mut self, opts: Vec<UserOption>) -> Self {
		self.options = opts;
		self
	}

	/// Validate the CREATE USER statement
	///
	/// # Validation Rules
	///
	/// 1. User name cannot be empty
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_query::dcl::CreateUserStatement;
	///
	/// let stmt = CreateUserStatement::new()
	///     .user("app_user");
	///
	/// assert!(stmt.validate().is_ok());
	/// ```
	///
	/// ```
	/// use reinhardt_query::dcl::CreateUserStatement;
	///
	/// let stmt = CreateUserStatement::new();
	/// assert!(stmt.validate().is_err());
	/// ```
	pub fn validate(&self) -> Result<(), String> {
		validate_name(&self.user_name, "User name")?;
		Ok(())
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_create_user_new() {
		let stmt = CreateUserStatement::new();
		assert!(stmt.user_name.is_empty());
		assert!(!stmt.if_not_exists);
		assert!(stmt.attributes.is_empty());
		assert!(stmt.default_roles.is_empty());
		assert!(stmt.options.is_empty());
	}

	#[test]
	fn test_create_user_basic() {
		let stmt = CreateUserStatement::new().user("app_user");
		assert_eq!(stmt.user_name, "app_user");
		assert!(stmt.validate().is_ok());
	}

	#[test]
	fn test_create_user_if_not_exists() {
		let stmt = CreateUserStatement::new()
			.user("app_user")
			.if_not_exists(true);
		assert!(stmt.if_not_exists);
	}

	#[test]
	fn test_create_user_with_attributes() {
		let stmt = CreateUserStatement::new()
			.user("app_user")
			.attribute(RoleAttribute::Password("secret".to_string()))
			.attribute(RoleAttribute::CreateDb);
		assert_eq!(stmt.attributes.len(), 2);
	}

	#[test]
	fn test_create_user_with_default_role() {
		let stmt = CreateUserStatement::new()
			.user("app_user@localhost")
			.default_role(vec!["app_role".to_string()]);
		assert_eq!(stmt.default_roles.len(), 1);
	}

	#[test]
	fn test_create_user_with_options() {
		let stmt = CreateUserStatement::new()
			.user("app_user")
			.option(UserOption::Comment("Test user".to_string()));
		assert_eq!(stmt.options.len(), 1);
	}

	#[test]
	fn test_create_user_validation_empty_name() {
		let stmt = CreateUserStatement::new();
		assert!(stmt.validate().is_err());
	}
}
