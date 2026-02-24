//! ALTER ROLE statement builder
//!
//! # Examples
//!
//! ```
//! use reinhardt_query::dcl::{AlterRoleStatement, RoleAttribute};
//!
//! let stmt = AlterRoleStatement::new()
//!     .role("app_user")
//!     .attribute(RoleAttribute::Login)
//!     .attribute(RoleAttribute::CreateDb);
//! ```

use super::{RoleAttribute, UserOption, validate_name};

/// ALTER ROLE statement builder
///
/// # Examples
///
/// ```
/// use reinhardt_query::dcl::{AlterRoleStatement, RoleAttribute};
///
/// let stmt = AlterRoleStatement::new()
///     .role("app_user")
///     .attribute(RoleAttribute::CreateDb)
///     .rename_to("new_user");
/// ```
#[derive(Debug, Clone, Default)]
pub struct AlterRoleStatement {
	/// Role name to alter
	pub role_name: String,
	/// PostgreSQL role attributes
	pub attributes: Vec<RoleAttribute>,
	/// MySQL user options
	pub options: Vec<UserOption>,
	/// New role name (RENAME TO)
	pub rename_to: Option<String>,
}

impl AlterRoleStatement {
	/// Create a new empty ALTER ROLE statement
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_query::dcl::AlterRoleStatement;
	///
	/// let stmt = AlterRoleStatement::new();
	/// ```
	pub fn new() -> Self {
		Self::default()
	}

	/// Set the role name to alter
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_query::dcl::AlterRoleStatement;
	///
	/// let stmt = AlterRoleStatement::new()
	///     .role("app_user");
	/// ```
	pub fn role(mut self, name: impl Into<String>) -> Self {
		self.role_name = name.into();
		self
	}

	/// Add a single role attribute
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_query::dcl::{AlterRoleStatement, RoleAttribute};
	///
	/// let stmt = AlterRoleStatement::new()
	///     .role("app_user")
	///     .attribute(RoleAttribute::Login);
	/// ```
	pub fn attribute(mut self, attr: RoleAttribute) -> Self {
		self.attributes.push(attr);
		self
	}

	/// Set all role attributes at once
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_query::dcl::{AlterRoleStatement, RoleAttribute};
	///
	/// let stmt = AlterRoleStatement::new()
	///     .role("app_user")
	///     .attributes(vec![RoleAttribute::Login, RoleAttribute::CreateDb]);
	/// ```
	pub fn attributes(mut self, attrs: Vec<RoleAttribute>) -> Self {
		self.attributes = attrs;
		self
	}

	/// Add a single user option (MySQL)
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_query::dcl::{AlterRoleStatement, UserOption};
	///
	/// let stmt = AlterRoleStatement::new()
	///     .role("app_user")
	///     .option(UserOption::AccountLock);
	/// ```
	pub fn option(mut self, opt: UserOption) -> Self {
		self.options.push(opt);
		self
	}

	/// Set all user options at once (MySQL)
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_query::dcl::{AlterRoleStatement, UserOption};
	///
	/// let stmt = AlterRoleStatement::new()
	///     .role("app_user")
	///     .options(vec![UserOption::AccountLock, UserOption::PasswordExpire]);
	/// ```
	pub fn options(mut self, opts: Vec<UserOption>) -> Self {
		self.options = opts;
		self
	}

	/// Set the new role name (RENAME TO)
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_query::dcl::AlterRoleStatement;
	///
	/// let stmt = AlterRoleStatement::new()
	///     .role("old_user")
	///     .rename_to("new_user");
	/// ```
	pub fn rename_to(mut self, new_name: impl Into<String>) -> Self {
		self.rename_to = Some(new_name.into());
		self
	}

	/// Validate the ALTER ROLE statement
	///
	/// # Validation Rules
	///
	/// 1. Role name cannot be empty
	/// 2. At least one attribute, option, or rename must be specified
	///
	/// # Examples
	///
	/// Valid statement:
	/// ```
	/// use reinhardt_query::dcl::{AlterRoleStatement, RoleAttribute};
	///
	/// let stmt = AlterRoleStatement::new()
	///     .role("app_user")
	///     .attribute(RoleAttribute::Login);
	///
	/// assert!(stmt.validate().is_ok());
	/// ```
	///
	/// Invalid statement (empty role name):
	/// ```
	/// use reinhardt_query::dcl::AlterRoleStatement;
	///
	/// let stmt = AlterRoleStatement::new();
	/// assert!(stmt.validate().is_err());
	/// ```
	pub fn validate(&self) -> Result<(), String> {
		validate_name(&self.role_name, "Role name")?;
		if self.attributes.is_empty() && self.options.is_empty() && self.rename_to.is_none() {
			return Err("At least one attribute, option, or rename must be specified".to_string());
		}
		Ok(())
	}
}
