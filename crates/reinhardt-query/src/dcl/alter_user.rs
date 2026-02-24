//! ALTER USER statement builder
//!
//! This module provides a fluent API for building ALTER USER statements for both
//! PostgreSQL and MySQL databases.
//!
//! # PostgreSQL
//!
//! PostgreSQL doesn't have a separate ALTER USER command; it's an alias for ALTER ROLE.
//! This builder wraps AlterRoleStatement.
//!
//! # MySQL
//!
//! MySQL has a native ALTER USER command that supports:
//! - User@host specification
//! - DEFAULT ROLE clause
//! - User options
//!
//! # Examples
//!
//! PostgreSQL example:
//!
//! ```
//! use reinhardt_query::dcl::{AlterUserStatement, RoleAttribute};
//!
//! let stmt = AlterUserStatement::new()
//!     .user("app_user")
//!     .attribute(RoleAttribute::Password("new_secret".to_string()));
//! ```
//!
//! MySQL example:
//!
//! ```
//! use reinhardt_query::dcl::{AlterUserStatement, UserOption};
//!
//! let stmt = AlterUserStatement::new()
//!     .user("app_user@localhost")
//!     .if_exists(true)
//!     .default_role(vec!["app_role".to_string()])
//!     .option(UserOption::AccountUnlock);
//! ```

use super::{RoleAttribute, UserOption, validate_name};

/// ALTER USER statement builder
///
/// This struct provides a fluent API for building ALTER USER statements.
///
/// # PostgreSQL
///
/// PostgreSQL ALTER USER is an alias for ALTER ROLE.
/// Use the `` `attribute()` `` method to modify role attributes.
///
/// # MySQL
///
/// MySQL ALTER USER supports:
/// - IF EXISTS clause
/// - User@host specification
/// - DEFAULT ROLE clause
/// - User options
///
/// # Examples
///
/// Alter a user password (PostgreSQL):
///
/// ```
/// use reinhardt_query::dcl::{AlterUserStatement, RoleAttribute};
///
/// let stmt = AlterUserStatement::new()
///     .user("app_user")
///     .attribute(RoleAttribute::Password("new_password".to_string()));
/// ```
///
/// Alter user with default role (MySQL):
///
/// ```
/// use reinhardt_query::dcl::{AlterUserStatement, UserOption};
///
/// let stmt = AlterUserStatement::new()
///     .user("app_user@localhost")
///     .if_exists(true)
///     .default_role(vec!["app_role".to_string()])
///     .option(UserOption::AccountUnlock);
/// ```
#[derive(Debug, Clone, Default)]
pub struct AlterUserStatement {
	/// User name (with optional @host for MySQL)
	pub user_name: String,
	/// IF EXISTS clause (MySQL only)
	pub if_exists: bool,
	/// PostgreSQL role attributes (PostgreSQL only)
	pub attributes: Vec<RoleAttribute>,
	/// MySQL DEFAULT ROLE clause (MySQL only)
	pub default_roles: Vec<String>,
	/// MySQL user options
	pub options: Vec<UserOption>,
}

impl AlterUserStatement {
	/// Create a new ALTER USER statement
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_query::dcl::AlterUserStatement;
	///
	/// let stmt = AlterUserStatement::new();
	/// ```
	pub fn new() -> Self {
		Self::default()
	}

	/// Set the user name
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_query::dcl::AlterUserStatement;
	///
	/// let stmt = AlterUserStatement::new()
	///     .user("app_user");
	/// ```
	///
	/// MySQL with host:
	///
	/// ```
	/// use reinhardt_query::dcl::AlterUserStatement;
	///
	/// let stmt = AlterUserStatement::new()
	///     .user("app_user@localhost");
	/// ```
	pub fn user(mut self, name: impl Into<String>) -> Self {
		self.user_name = name.into();
		self
	}

	/// Set IF EXISTS flag (MySQL only)
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_query::dcl::AlterUserStatement;
	///
	/// let stmt = AlterUserStatement::new()
	///     .user("app_user")
	///     .if_exists(true);
	/// ```
	pub fn if_exists(mut self, flag: bool) -> Self {
		self.if_exists = flag;
		self
	}

	/// Add a single PostgreSQL role attribute
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_query::dcl::{AlterUserStatement, RoleAttribute};
	///
	/// let stmt = AlterUserStatement::new()
	///     .user("app_user")
	///     .attribute(RoleAttribute::Password("new_secret".to_string()))
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
	/// use reinhardt_query::dcl::{AlterUserStatement, RoleAttribute};
	///
	/// let stmt = AlterUserStatement::new()
	///     .user("app_user")
	///     .attributes(vec![
	///         RoleAttribute::Password("new_secret".to_string()),
	///         RoleAttribute::NoCreateDb,
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
	/// use reinhardt_query::dcl::AlterUserStatement;
	///
	/// let stmt = AlterUserStatement::new()
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
	/// use reinhardt_query::dcl::{AlterUserStatement, UserOption};
	///
	/// let stmt = AlterUserStatement::new()
	///     .user("app_user")
	///     .option(UserOption::AccountUnlock);
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
	/// use reinhardt_query::dcl::{AlterUserStatement, UserOption};
	///
	/// let stmt = AlterUserStatement::new()
	///     .user("app_user")
	///     .options(vec![
	///         UserOption::AccountUnlock,
	///         UserOption::PasswordExpireNever,
	///     ]);
	/// ```
	pub fn options(mut self, opts: Vec<UserOption>) -> Self {
		self.options = opts;
		self
	}

	/// Validate the ALTER USER statement
	///
	/// # Validation Rules
	///
	/// 1. User name cannot be empty
	/// 2. At least one attribute, default role, or option must be specified
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_query::dcl::{AlterUserStatement, RoleAttribute};
	///
	/// let stmt = AlterUserStatement::new()
	///     .user("app_user")
	///     .attribute(RoleAttribute::Login);
	///
	/// assert!(stmt.validate().is_ok());
	/// ```
	///
	/// ```
	/// use reinhardt_query::dcl::AlterUserStatement;
	///
	/// let stmt = AlterUserStatement::new();
	/// assert!(stmt.validate().is_err());
	/// ```
	pub fn validate(&self) -> Result<(), String> {
		validate_name(&self.user_name, "User name")?;
		if self.attributes.is_empty() && self.default_roles.is_empty() && self.options.is_empty() {
			return Err(
				"At least one attribute, default role, or option must be specified".to_string(),
			);
		}
		Ok(())
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_alter_user_new() {
		let stmt = AlterUserStatement::new();
		assert!(stmt.user_name.is_empty());
		assert!(!stmt.if_exists);
		assert!(stmt.attributes.is_empty());
		assert!(stmt.default_roles.is_empty());
		assert!(stmt.options.is_empty());
	}

	#[test]
	fn test_alter_user_basic() {
		let stmt = AlterUserStatement::new()
			.user("app_user")
			.attribute(RoleAttribute::Login);
		assert_eq!(stmt.user_name, "app_user");
		assert!(stmt.validate().is_ok());
	}

	#[test]
	fn test_alter_user_if_exists() {
		let stmt = AlterUserStatement::new().user("app_user").if_exists(true);
		assert!(stmt.if_exists);
	}

	#[test]
	fn test_alter_user_with_attributes() {
		let stmt = AlterUserStatement::new()
			.user("app_user")
			.attribute(RoleAttribute::Password("new_secret".to_string()))
			.attribute(RoleAttribute::CreateDb);
		assert_eq!(stmt.attributes.len(), 2);
	}

	#[test]
	fn test_alter_user_with_default_role() {
		let stmt = AlterUserStatement::new()
			.user("app_user@localhost")
			.default_role(vec!["app_role".to_string()]);
		assert_eq!(stmt.default_roles.len(), 1);
	}

	#[test]
	fn test_alter_user_with_options() {
		let stmt = AlterUserStatement::new()
			.user("app_user")
			.option(UserOption::AccountUnlock);
		assert_eq!(stmt.options.len(), 1);
	}

	#[test]
	fn test_alter_user_validation_empty_name() {
		let stmt = AlterUserStatement::new();
		assert!(stmt.validate().is_err());
	}
}
