//! RENAME USER statement builder (MySQL only)
//!
//! This module provides a fluent API for building RENAME USER statements for MySQL.
//!
//! # MySQL Only
//!
//! RENAME USER is a MySQL-specific command that allows renaming multiple users
//! in a single statement.
//!
//! # PostgreSQL & SQLite
//!
//! These databases do not support RENAME USER. Attempting to generate SQL for
//! these backends will result in a panic.
//!
//! # Examples
//!
//! Rename a single user:
//!
//! ```
//! use reinhardt_query::dcl::RenameUserStatement;
//!
//! let stmt = RenameUserStatement::new()
//!     .rename("old_user@localhost", "new_user@localhost");
//! ```
//!
//! Rename multiple users:
//!
//! ```
//! use reinhardt_query::dcl::RenameUserStatement;
//!
//! let stmt = RenameUserStatement::new()
//!     .rename("user1@localhost", "renamed1@localhost")
//!     .rename("user2@localhost", "renamed2@localhost");
//! ```

use super::validate_name;

/// RENAME USER statement builder (MySQL only)
///
/// This struct provides a fluent API for building RENAME USER statements.
/// This is a MySQL-specific feature.
///
/// # MySQL
///
/// MySQL RENAME USER supports renaming multiple users in a single statement:
/// `RENAME USER old1 TO new1, old2 TO new2`
///
/// # Examples
///
/// Rename a single user:
///
/// ```
/// use reinhardt_query::dcl::RenameUserStatement;
///
/// let stmt = RenameUserStatement::new()
///     .rename("old_user@localhost", "new_user@localhost");
/// ```
///
/// Rename multiple users at once:
///
/// ```
/// use reinhardt_query::dcl::RenameUserStatement;
///
/// let stmt = RenameUserStatement::new()
///     .rename("user1@localhost", "renamed1@localhost")
///     .rename("user2@localhost", "renamed2@localhost");
/// ```
#[derive(Debug, Clone, Default)]
pub struct RenameUserStatement {
	/// List of (old_name, new_name) pairs
	pub renames: Vec<(String, String)>,
}

impl RenameUserStatement {
	/// Create a new RENAME USER statement
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_query::dcl::RenameUserStatement;
	///
	/// let stmt = RenameUserStatement::new();
	/// ```
	pub fn new() -> Self {
		Self::default()
	}

	/// Add a rename pair
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_query::dcl::RenameUserStatement;
	///
	/// let stmt = RenameUserStatement::new()
	///     .rename("old_user@localhost", "new_user@localhost");
	/// ```
	pub fn rename(mut self, old_name: impl Into<String>, new_name: impl Into<String>) -> Self {
		self.renames.push((old_name.into(), new_name.into()));
		self
	}

	/// Set all rename pairs at once
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_query::dcl::RenameUserStatement;
	///
	/// let stmt = RenameUserStatement::new()
	///     .renames(vec![
	///         ("old1@localhost".to_string(), "new1@localhost".to_string()),
	///         ("old2@localhost".to_string(), "new2@localhost".to_string()),
	///     ]);
	/// ```
	pub fn renames(mut self, pairs: Vec<(String, String)>) -> Self {
		self.renames = pairs;
		self
	}

	/// Validate the RENAME USER statement
	///
	/// # Validation Rules
	///
	/// 1. At least one rename pair must be specified
	/// 2. Old and new names cannot be empty
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_query::dcl::RenameUserStatement;
	///
	/// let stmt = RenameUserStatement::new()
	///     .rename("old_user", "new_user");
	///
	/// assert!(stmt.validate().is_ok());
	/// ```
	///
	/// ```
	/// use reinhardt_query::dcl::RenameUserStatement;
	///
	/// let stmt = RenameUserStatement::new();
	/// assert!(stmt.validate().is_err());
	/// ```
	pub fn validate(&self) -> Result<(), String> {
		if self.renames.is_empty() {
			return Err("At least one rename pair must be specified".to_string());
		}
		for (old, new) in &self.renames {
			validate_name(old, "Old user name")?;
			validate_name(new, "New user name")?;
		}
		Ok(())
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_rename_user_new() {
		let stmt = RenameUserStatement::new();
		assert!(stmt.renames.is_empty());
	}

	#[test]
	fn test_rename_user_basic() {
		let stmt = RenameUserStatement::new().rename("old_user", "new_user");
		assert_eq!(stmt.renames.len(), 1);
		assert_eq!(stmt.renames[0].0, "old_user");
		assert_eq!(stmt.renames[0].1, "new_user");
		assert!(stmt.validate().is_ok());
	}

	#[test]
	fn test_rename_user_multiple() {
		let stmt = RenameUserStatement::new()
			.rename("user1", "renamed1")
			.rename("user2", "renamed2");
		assert_eq!(stmt.renames.len(), 2);
	}

	#[test]
	fn test_rename_user_validation_empty() {
		let stmt = RenameUserStatement::new();
		assert!(stmt.validate().is_err());
	}

	#[test]
	fn test_rename_user_validation_empty_old_name() {
		let stmt = RenameUserStatement::new().rename("", "new_user");
		assert!(stmt.validate().is_err());
	}

	#[test]
	fn test_rename_user_validation_empty_new_name() {
		let stmt = RenameUserStatement::new().rename("old_user", "");
		assert!(stmt.validate().is_err());
	}
}
