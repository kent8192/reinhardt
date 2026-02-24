//! DROP ROLE statement builder
//!
//! # Examples
//!
//! ```
//! use reinhardt_query::dcl::DropRoleStatement;
//!
//! let stmt = DropRoleStatement::new()
//!     .role("app_user")
//!     .if_exists(true);
//! ```

/// DROP ROLE statement builder
///
/// # Examples
///
/// ```
/// use reinhardt_query::dcl::DropRoleStatement;
///
/// let stmt = DropRoleStatement::new()
///     .roles(vec!["user1".to_string(), "user2".to_string()])
///     .if_exists(true);
/// ```
#[derive(Debug, Clone, Default)]
pub struct DropRoleStatement {
	/// Role names to drop
	pub role_names: Vec<String>,
	/// IF EXISTS clause
	pub if_exists: bool,
}

impl DropRoleStatement {
	/// Create a new empty DROP ROLE statement
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_query::dcl::DropRoleStatement;
	///
	/// let stmt = DropRoleStatement::new();
	/// ```
	pub fn new() -> Self {
		Self::default()
	}

	/// Add a single role name to drop
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_query::dcl::DropRoleStatement;
	///
	/// let stmt = DropRoleStatement::new()
	///     .role("app_user");
	/// ```
	pub fn role(mut self, name: impl Into<String>) -> Self {
		self.role_names.push(name.into());
		self
	}

	/// Set all role names to drop at once
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_query::dcl::DropRoleStatement;
	///
	/// let stmt = DropRoleStatement::new()
	///     .roles(vec!["user1".to_string(), "user2".to_string()]);
	/// ```
	pub fn roles(mut self, names: Vec<String>) -> Self {
		self.role_names = names;
		self
	}

	/// Set the IF EXISTS flag
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_query::dcl::DropRoleStatement;
	///
	/// let stmt = DropRoleStatement::new()
	///     .role("app_user")
	///     .if_exists(true);
	/// ```
	pub fn if_exists(mut self, flag: bool) -> Self {
		self.if_exists = flag;
		self
	}

	/// Validate the DROP ROLE statement
	///
	/// # Validation Rules
	///
	/// 1. At least one role name is required
	///
	/// # Examples
	///
	/// Valid statement:
	/// ```
	/// use reinhardt_query::dcl::DropRoleStatement;
	///
	/// let stmt = DropRoleStatement::new()
	///     .role("app_user");
	///
	/// assert!(stmt.validate().is_ok());
	/// ```
	///
	/// Invalid statement (no role names):
	/// ```
	/// use reinhardt_query::dcl::DropRoleStatement;
	///
	/// let stmt = DropRoleStatement::new();
	/// assert!(stmt.validate().is_err());
	/// ```
	pub fn validate(&self) -> Result<(), String> {
		if self.role_names.is_empty() {
			return Err("At least one role name is required".to_string());
		}
		// Validate each role name is non-empty after trimming whitespace
		for (idx, role_name) in self.role_names.iter().enumerate() {
			let trimmed = role_name.trim();
			if trimmed.is_empty() {
				return Err(format!(
					"Role name at index {} cannot be empty or whitespace only",
					idx
				));
			}
		}
		Ok(())
	}
}
