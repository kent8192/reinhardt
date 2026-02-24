//! SET ROLE statement builder
//!
//! This module provides a fluent API for building SET ROLE statements for both
//! PostgreSQL and MySQL databases.
//!
//! # PostgreSQL
//!
//! PostgreSQL supports:
//! - `SET ROLE role_name` - Set current role
//! - `SET ROLE NONE` - Reset to session user
//!
//! # MySQL
//!
//! MySQL supports:
//! - `SET ROLE role_name` - Activate specific role
//! - `SET ROLE NONE` - Deactivate all roles
//! - `SET ROLE ALL` - Activate all granted roles
//! - `SET ROLE ALL EXCEPT role1, role2` - Activate all except specified roles
//! - `SET ROLE DEFAULT` - Activate default roles
//!
//! # Examples
//!
//! Set specific role:
//!
//! ```
//! use reinhardt_query::dcl::{SetRoleStatement, RoleTarget};
//!
//! let stmt = SetRoleStatement::new()
//!     .role(RoleTarget::Named("admin".to_string()));
//! ```
//!
//! Reset role (PostgreSQL):
//!
//! ```
//! use reinhardt_query::dcl::{SetRoleStatement, RoleTarget};
//!
//! let stmt = SetRoleStatement::new()
//!     .role(RoleTarget::None);
//! ```
//!
//! Activate all roles except specific ones (MySQL):
//!
//! ```
//! use reinhardt_query::dcl::{SetRoleStatement, RoleTarget};
//!
//! let stmt = SetRoleStatement::new()
//!     .role(RoleTarget::AllExcept(vec!["restricted_role".to_string()]));
//! ```

/// Role target for SET ROLE statement
///
/// This enum specifies what role(s) to activate or deactivate.
///
/// # Variants
///
/// - `` `Named` `` - Set to a specific role (PostgreSQL & MySQL)
/// - `` `None` `` - Deactivate all roles (PostgreSQL & MySQL)
/// - `` `All` `` - Activate all granted roles (MySQL only)
/// - `` `AllExcept` `` - Activate all except specified roles (MySQL only)
/// - `` `Default` `` - Activate default roles (MySQL only)
#[derive(Debug, Clone, PartialEq)]
pub enum RoleTarget {
	/// SET ROLE role_name
	Named(String),
	/// SET ROLE NONE
	None,
	/// SET ROLE ALL (MySQL only)
	All,
	/// SET ROLE ALL EXCEPT role_list (MySQL only)
	AllExcept(Vec<String>),
	/// SET ROLE DEFAULT (MySQL only)
	Default,
}

/// SET ROLE statement builder
///
/// This struct provides a fluent API for building SET ROLE statements.
///
/// # PostgreSQL
///
/// PostgreSQL SET ROLE changes the current role for the session.
/// Supports:
/// - Named role: `SET ROLE role_name`
/// - Reset: `SET ROLE NONE`
///
/// # MySQL
///
/// MySQL SET ROLE activates roles for the session.
/// Supports:
/// - Named role: `SET ROLE role_name`
/// - None: `SET ROLE NONE`
/// - All: `SET ROLE ALL`
/// - All except: `SET ROLE ALL EXCEPT role_list`
/// - Default: `SET ROLE DEFAULT`
///
/// # Examples
///
/// Set to a specific role:
///
/// ```
/// use reinhardt_query::dcl::{SetRoleStatement, RoleTarget};
///
/// let stmt = SetRoleStatement::new()
///     .role(RoleTarget::Named("admin".to_string()));
/// ```
///
/// Deactivate all roles:
///
/// ```
/// use reinhardt_query::dcl::{SetRoleStatement, RoleTarget};
///
/// let stmt = SetRoleStatement::new()
///     .role(RoleTarget::None);
/// ```
///
/// Activate all granted roles (MySQL):
///
/// ```
/// use reinhardt_query::dcl::{SetRoleStatement, RoleTarget};
///
/// let stmt = SetRoleStatement::new()
///     .role(RoleTarget::All);
/// ```
///
/// Activate all except specific roles (MySQL):
///
/// ```
/// use reinhardt_query::dcl::{SetRoleStatement, RoleTarget};
///
/// let stmt = SetRoleStatement::new()
///     .role(RoleTarget::AllExcept(vec!["restricted".to_string()]));
/// ```
#[derive(Debug, Clone, Default)]
pub struct SetRoleStatement {
	/// Role target
	pub target: Option<RoleTarget>,
}

impl SetRoleStatement {
	/// Create a new SET ROLE statement
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_query::dcl::SetRoleStatement;
	///
	/// let stmt = SetRoleStatement::new();
	/// ```
	pub fn new() -> Self {
		Self::default()
	}

	/// Set the role target
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_query::dcl::{SetRoleStatement, RoleTarget};
	///
	/// let stmt = SetRoleStatement::new()
	///     .role(RoleTarget::Named("admin".to_string()));
	/// ```
	pub fn role(mut self, target: RoleTarget) -> Self {
		self.target = Some(target);
		self
	}

	/// Validate the SET ROLE statement
	///
	/// # Validation Rules
	///
	/// 1. Role target must be specified
	/// 2. For Named variant, role name cannot be empty
	/// 3. For AllExcept variant, exception list cannot be empty
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_query::dcl::{SetRoleStatement, RoleTarget};
	///
	/// let stmt = SetRoleStatement::new()
	///     .role(RoleTarget::Named("admin".to_string()));
	///
	/// assert!(stmt.validate().is_ok());
	/// ```
	///
	/// ```
	/// use reinhardt_query::dcl::SetRoleStatement;
	///
	/// let stmt = SetRoleStatement::new();
	/// assert!(stmt.validate().is_err());
	/// ```
	pub fn validate(&self) -> Result<(), String> {
		match &self.target {
			None => Err("Role target must be specified".to_string()),
			Some(RoleTarget::Named(name)) if name.trim().is_empty() => {
				Err("Role name cannot be empty or whitespace only".to_string())
			}
			Some(RoleTarget::AllExcept(list)) if list.is_empty() => {
				Err("AllExcept role list cannot be empty".to_string())
			}
			Some(RoleTarget::AllExcept(list)) => {
				for role in list {
					if role.trim().is_empty() {
						return Err(
							"Role name in AllExcept list cannot be empty or whitespace only"
								.to_string(),
						);
					}
				}
				Ok(())
			}
			_ => Ok(()),
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_set_role_new() {
		let stmt = SetRoleStatement::new();
		assert!(stmt.target.is_none());
	}

	#[test]
	fn test_set_role_named() {
		let stmt = SetRoleStatement::new().role(RoleTarget::Named("admin".to_string()));
		assert!(matches!(stmt.target, Some(RoleTarget::Named(_))));
		assert!(stmt.validate().is_ok());
	}

	#[test]
	fn test_set_role_none() {
		let stmt = SetRoleStatement::new().role(RoleTarget::None);
		assert!(matches!(stmt.target, Some(RoleTarget::None)));
		assert!(stmt.validate().is_ok());
	}

	#[test]
	fn test_set_role_all() {
		let stmt = SetRoleStatement::new().role(RoleTarget::All);
		assert!(matches!(stmt.target, Some(RoleTarget::All)));
		assert!(stmt.validate().is_ok());
	}

	#[test]
	fn test_set_role_all_except() {
		let stmt =
			SetRoleStatement::new().role(RoleTarget::AllExcept(vec!["restricted".to_string()]));
		assert!(matches!(stmt.target, Some(RoleTarget::AllExcept(_))));
		assert!(stmt.validate().is_ok());
	}

	#[test]
	fn test_set_role_default() {
		let stmt = SetRoleStatement::new().role(RoleTarget::Default);
		assert!(matches!(stmt.target, Some(RoleTarget::Default)));
		assert!(stmt.validate().is_ok());
	}

	#[test]
	fn test_set_role_validation_no_target() {
		let stmt = SetRoleStatement::new();
		assert!(stmt.validate().is_err());
	}

	#[test]
	fn test_set_role_validation_empty_name() {
		let stmt = SetRoleStatement::new().role(RoleTarget::Named("".to_string()));
		assert!(stmt.validate().is_err());
	}

	#[test]
	fn test_set_role_validation_empty_except_list() {
		let stmt = SetRoleStatement::new().role(RoleTarget::AllExcept(vec![]));
		assert!(stmt.validate().is_err());
	}
}
