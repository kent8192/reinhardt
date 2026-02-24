//! RESET ROLE statement builder (PostgreSQL only)
//!
//! This module provides a fluent API for building RESET ROLE statements for PostgreSQL.
//!
//! # PostgreSQL Only
//!
//! RESET ROLE is a PostgreSQL-specific command that resets the current role to the
//! session default. It's equivalent to `SET ROLE NONE`.
//!
//! # MySQL & SQLite
//!
//! These databases do not support RESET ROLE. Attempting to generate SQL for
//! these backends will result in a panic.
//!
//! # Examples
//!
//! ```
//! use reinhardt_query::dcl::ResetRoleStatement;
//!
//! let stmt = ResetRoleStatement::new();
//! ```

/// RESET ROLE statement builder (PostgreSQL only)
///
/// This struct provides a fluent API for building RESET ROLE statements.
/// This is a PostgreSQL-specific feature.
///
/// # PostgreSQL
///
/// PostgreSQL RESET ROLE resets the current role to the session default.
/// This is a simple statement with no parameters: `RESET ROLE`
///
/// # Examples
///
/// ```
/// use reinhardt_query::dcl::ResetRoleStatement;
///
/// let stmt = ResetRoleStatement::new();
/// assert!(stmt.validate().is_ok());
/// ```
#[derive(Debug, Clone, Default)]
pub struct ResetRoleStatement;

impl ResetRoleStatement {
	/// Create a new RESET ROLE statement
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_query::dcl::ResetRoleStatement;
	///
	/// let stmt = ResetRoleStatement::new();
	/// ```
	pub fn new() -> Self {
		Self
	}

	/// Validate the RESET ROLE statement
	///
	/// RESET ROLE has no parameters, so this always returns Ok.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_query::dcl::ResetRoleStatement;
	///
	/// let stmt = ResetRoleStatement::new();
	/// assert!(stmt.validate().is_ok());
	/// ```
	pub fn validate(&self) -> Result<(), String> {
		Ok(())
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_reset_role_new() {
		let _stmt = ResetRoleStatement::new();
	}

	#[test]
	fn test_reset_role_validation() {
		let stmt = ResetRoleStatement::new();
		assert!(stmt.validate().is_ok());
	}
}
