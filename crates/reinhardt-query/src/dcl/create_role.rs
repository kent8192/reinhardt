//! CREATE ROLE statement builder
//!
//! This module provides a fluent API for building CREATE ROLE statements for both
//! PostgreSQL and MySQL databases.
//!
//! # Examples
//!
//! PostgreSQL example:
//!
//! ```
//! use reinhardt_query::dcl::{CreateRoleStatement, RoleAttribute};
//!
//! let stmt = CreateRoleStatement::new()
//!     .role("app_user")
//!     .attribute(RoleAttribute::Login)
//!     .attribute(RoleAttribute::Password("secret".to_string()));
//! ```
//!
//! MySQL example:
//!
//! ```
//! use reinhardt_query::dcl::{CreateRoleStatement, UserOption};
//!
//! let stmt = CreateRoleStatement::new()
//!     .role("app_role")
//!     .if_not_exists(true)
//!     .option(UserOption::Comment("Application role".to_string()));
//! ```

use super::{RoleAttribute, UserOption, validate_name};

/// CREATE ROLE statement builder
///
/// This struct provides a fluent API for building CREATE ROLE statements.
/// It supports both PostgreSQL attributes and MySQL options.
///
/// # PostgreSQL
///
/// PostgreSQL CREATE ROLE accepts various attributes that control role privileges
/// and settings. Use the `` `attribute()` `` method to add attributes.
///
/// # MySQL
///
/// MySQL CREATE ROLE supports IF NOT EXISTS clause and user options.
/// Use the `` `if_not_exists()` `` and `` `option()` `` methods.
///
/// # Examples
///
/// Create a simple role:
///
/// ```
/// use reinhardt_query::dcl::CreateRoleStatement;
///
/// let stmt = CreateRoleStatement::new()
///     .role("developer");
/// ```
///
/// Create a login role with password (PostgreSQL):
///
/// ```
/// use reinhardt_query::dcl::{CreateRoleStatement, RoleAttribute};
///
/// let stmt = CreateRoleStatement::new()
///     .role("app_user")
///     .attribute(RoleAttribute::Login)
///     .attribute(RoleAttribute::Password("password123".to_string()))
///     .attribute(RoleAttribute::ConnectionLimit(5));
/// ```
///
/// Create a role with IF NOT EXISTS (MySQL):
///
/// ```
/// use reinhardt_query::dcl::{CreateRoleStatement, UserOption};
///
/// let stmt = CreateRoleStatement::new()
///     .role("app_role")
///     .if_not_exists(true)
///     .option(UserOption::Comment("My application role".to_string()));
/// ```
#[derive(Debug, Clone, Default)]
pub struct CreateRoleStatement {
	/// Role name
	pub role_name: String,
	/// IF NOT EXISTS clause (MySQL only)
	pub if_not_exists: bool,
	/// PostgreSQL role attributes
	pub attributes: Vec<RoleAttribute>,
	/// MySQL user options
	pub options: Vec<UserOption>,
}

impl CreateRoleStatement {
	/// Create a new CREATE ROLE statement
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_query::dcl::CreateRoleStatement;
	///
	/// let stmt = CreateRoleStatement::new();
	/// ```
	pub fn new() -> Self {
		Self::default()
	}

	/// Set the role name
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_query::dcl::CreateRoleStatement;
	///
	/// let stmt = CreateRoleStatement::new()
	///     .role("developer");
	/// ```
	pub fn role(mut self, name: impl Into<String>) -> Self {
		self.role_name = name.into();
		self
	}

	/// Set IF NOT EXISTS flag (MySQL only)
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_query::dcl::CreateRoleStatement;
	///
	/// let stmt = CreateRoleStatement::new()
	///     .role("app_role")
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
	/// use reinhardt_query::dcl::{CreateRoleStatement, RoleAttribute};
	///
	/// let stmt = CreateRoleStatement::new()
	///     .role("app_user")
	///     .attribute(RoleAttribute::Login)
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
	/// use reinhardt_query::dcl::{CreateRoleStatement, RoleAttribute};
	///
	/// let stmt = CreateRoleStatement::new()
	///     .role("app_user")
	///     .attributes(vec![
	///         RoleAttribute::Login,
	///         RoleAttribute::CreateDb,
	///         RoleAttribute::ConnectionLimit(10),
	///     ]);
	/// ```
	pub fn attributes(mut self, attrs: Vec<RoleAttribute>) -> Self {
		self.attributes = attrs;
		self
	}

	/// Add a single MySQL user option
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_query::dcl::{CreateRoleStatement, UserOption};
	///
	/// let stmt = CreateRoleStatement::new()
	///     .role("app_role")
	///     .option(UserOption::Comment("Application role".to_string()));
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
	/// use reinhardt_query::dcl::{CreateRoleStatement, UserOption};
	///
	/// let stmt = CreateRoleStatement::new()
	///     .role("app_role")
	///     .options(vec![
	///         UserOption::Comment("Application role".to_string()),
	///         UserOption::AccountLock,
	///     ]);
	/// ```
	pub fn options(mut self, opts: Vec<UserOption>) -> Self {
		self.options = opts;
		self
	}

	/// Validate the CREATE ROLE statement
	///
	/// # Validation Rules
	///
	/// 1. Role name cannot be empty
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_query::dcl::CreateRoleStatement;
	///
	/// let stmt = CreateRoleStatement::new()
	///     .role("developer");
	///
	/// assert!(stmt.validate().is_ok());
	/// ```
	///
	/// ```
	/// use reinhardt_query::dcl::CreateRoleStatement;
	///
	/// let stmt = CreateRoleStatement::new();
	/// assert!(stmt.validate().is_err());
	/// ```
	pub fn validate(&self) -> Result<(), String> {
		validate_name(&self.role_name, "Role name")?;
		Ok(())
	}
}
