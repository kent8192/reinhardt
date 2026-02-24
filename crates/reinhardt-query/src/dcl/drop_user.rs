//! DROP USER statement builder
//!
//! This module provides a fluent API for building DROP USER statements for both
//! PostgreSQL and MySQL databases.
//!
//! # PostgreSQL
//!
//! PostgreSQL doesn't have a separate DROP USER command; it's an alias for DROP ROLE.
//! This builder wraps DropRoleStatement.
//!
//! # MySQL
//!
//! MySQL has a native DROP USER command that supports user@host specification.
//!
//! # Examples
//!
//! PostgreSQL example:
//!
//! ```
//! use reinhardt_query::dcl::DropUserStatement;
//!
//! let stmt = DropUserStatement::new()
//!     .user("app_user");
//! ```
//!
//! MySQL example:
//!
//! ```
//! use reinhardt_query::dcl::DropUserStatement;
//!
//! let stmt = DropUserStatement::new()
//!     .user("app_user@localhost")
//!     .if_exists(true);
//! ```

/// DROP USER statement builder
///
/// This struct provides a fluent API for building DROP USER statements.
///
/// # PostgreSQL
///
/// PostgreSQL DROP USER is an alias for DROP ROLE.
///
/// # MySQL
///
/// MySQL DROP USER supports:
/// - IF EXISTS clause
/// - User@host specification
/// - Multiple users
///
/// # Examples
///
/// Drop a single user:
///
/// ```
/// use reinhardt_query::dcl::DropUserStatement;
///
/// let stmt = DropUserStatement::new()
///     .user("app_user");
/// ```
///
/// Drop multiple users (MySQL):
///
/// ```
/// use reinhardt_query::dcl::DropUserStatement;
///
/// let stmt = DropUserStatement::new()
///     .users(vec!["user1@localhost".to_string(), "user2@localhost".to_string()])
///     .if_exists(true);
/// ```
#[derive(Debug, Clone, Default)]
pub struct DropUserStatement {
	/// User names (with optional @host for MySQL)
	pub user_names: Vec<String>,
	/// IF EXISTS clause
	pub if_exists: bool,
}

impl DropUserStatement {
	/// Create a new DROP USER statement
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_query::dcl::DropUserStatement;
	///
	/// let stmt = DropUserStatement::new();
	/// ```
	pub fn new() -> Self {
		Self::default()
	}

	/// Add a single user to drop
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_query::dcl::DropUserStatement;
	///
	/// let stmt = DropUserStatement::new()
	///     .user("app_user");
	/// ```
	///
	/// MySQL with host:
	///
	/// ```
	/// use reinhardt_query::dcl::DropUserStatement;
	///
	/// let stmt = DropUserStatement::new()
	///     .user("app_user@localhost");
	/// ```
	pub fn user(mut self, name: impl Into<String>) -> Self {
		self.user_names.push(name.into());
		self
	}

	/// Set all users to drop at once
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_query::dcl::DropUserStatement;
	///
	/// let stmt = DropUserStatement::new()
	///     .users(vec!["user1".to_string(), "user2".to_string()]);
	/// ```
	pub fn users(mut self, names: Vec<String>) -> Self {
		self.user_names = names;
		self
	}

	/// Set IF EXISTS flag
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_query::dcl::DropUserStatement;
	///
	/// let stmt = DropUserStatement::new()
	///     .user("app_user")
	///     .if_exists(true);
	/// ```
	pub fn if_exists(mut self, flag: bool) -> Self {
		self.if_exists = flag;
		self
	}

	/// Validate the DROP USER statement
	///
	/// # Validation Rules
	///
	/// 1. At least one user must be specified
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_query::dcl::DropUserStatement;
	///
	/// let stmt = DropUserStatement::new()
	///     .user("app_user");
	///
	/// assert!(stmt.validate().is_ok());
	/// ```
	///
	/// ```
	/// use reinhardt_query::dcl::DropUserStatement;
	///
	/// let stmt = DropUserStatement::new();
	/// assert!(stmt.validate().is_err());
	/// ```
	pub fn validate(&self) -> Result<(), String> {
		if self.user_names.is_empty() {
			return Err("At least one user must be specified".to_string());
		}
		// Validate each user name is non-empty after trimming whitespace
		for (idx, user_name) in self.user_names.iter().enumerate() {
			let trimmed = user_name.trim();
			if trimmed.is_empty() {
				return Err(format!(
					"User name at index {} cannot be empty or whitespace only",
					idx
				));
			}
		}
		Ok(())
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_drop_user_new() {
		let stmt = DropUserStatement::new();
		assert!(stmt.user_names.is_empty());
		assert!(!stmt.if_exists);
	}

	#[test]
	fn test_drop_user_basic() {
		let stmt = DropUserStatement::new().user("app_user");
		assert_eq!(stmt.user_names.len(), 1);
		assert_eq!(stmt.user_names[0], "app_user");
		assert!(stmt.validate().is_ok());
	}

	#[test]
	fn test_drop_user_multiple() {
		let stmt = DropUserStatement::new().user("user1").user("user2");
		assert_eq!(stmt.user_names.len(), 2);
	}

	#[test]
	fn test_drop_user_if_exists() {
		let stmt = DropUserStatement::new().user("app_user").if_exists(true);
		assert!(stmt.if_exists);
	}

	#[test]
	fn test_drop_user_validation_empty() {
		let stmt = DropUserStatement::new();
		assert!(stmt.validate().is_err());
	}
}
