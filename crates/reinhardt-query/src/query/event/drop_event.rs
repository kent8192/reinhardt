//! DROP EVENT statement builder (MySQL-specific)
//!
//! This module provides a builder for MySQL DROP EVENT statements,
//! which are used to delete scheduled tasks.
//!
//! # Examples
//!
//! ```rust,ignore
//! use reinhardt_query::prelude::*;
//!
//! // DROP EVENT my_event
//! let stmt = Query::drop_event()
//!     .name("my_event");
//!
//! // DROP EVENT IF EXISTS my_event
//! let stmt = Query::drop_event()
//!     .if_exists()
//!     .name("my_event");
//! ```

use crate::types::{DynIden, IntoIden};

/// DROP EVENT statement builder (MySQL-specific)
///
/// This struct provides a builder for constructing DROP EVENT statements
/// for deleting MySQL scheduled tasks.
///
/// # Examples
///
/// ```rust,ignore
/// use reinhardt_query::prelude::*;
///
/// // Simple drop
/// let stmt = Query::drop_event()
///     .name("my_event");
///
/// // With IF EXISTS
/// let stmt = Query::drop_event()
///     .if_exists()
///     .name("my_event");
/// ```
#[derive(Debug, Clone)]
pub struct DropEventStatement {
	pub(crate) name: Option<DynIden>,
	pub(crate) if_exists: bool,
}

impl DropEventStatement {
	/// Create a new DROP EVENT statement
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::query::event::DropEventStatement;
	///
	/// let stmt = DropEventStatement::new();
	/// ```
	pub fn new() -> Self {
		Self {
			name: None,
			if_exists: false,
		}
	}

	/// Set event name
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let stmt = Query::drop_event()
	///     .name("my_event");
	/// ```
	pub fn name<N: IntoIden>(&mut self, name: N) -> &mut Self {
		self.name = Some(name.into_iden());
		self
	}

	/// Set IF EXISTS clause
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let stmt = Query::drop_event()
	///     .if_exists();
	/// ```
	pub fn if_exists(&mut self) -> &mut Self {
		self.if_exists = true;
		self
	}
}

impl Default for DropEventStatement {
	fn default() -> Self {
		Self::new()
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::*;

	#[rstest]
	fn test_drop_event_new() {
		let stmt = DropEventStatement::new();
		assert!(stmt.name.is_none());
		assert!(!stmt.if_exists);
	}

	#[rstest]
	fn test_drop_event_name() {
		let mut stmt = DropEventStatement::new();
		stmt.name("my_event");
		assert_eq!(stmt.name.as_ref().unwrap().to_string(), "my_event");
	}

	#[rstest]
	fn test_drop_event_if_exists() {
		let mut stmt = DropEventStatement::new();
		stmt.if_exists();
		assert!(stmt.if_exists);
	}

	#[rstest]
	fn test_drop_event_name_and_if_exists() {
		let mut stmt = DropEventStatement::new();
		stmt.name("my_event").if_exists();
		assert_eq!(stmt.name.as_ref().unwrap().to_string(), "my_event");
		assert!(stmt.if_exists);
	}

	#[rstest]
	fn test_drop_event_if_exists_and_name() {
		let mut stmt = DropEventStatement::new();
		stmt.if_exists().name("my_event");
		assert_eq!(stmt.name.as_ref().unwrap().to_string(), "my_event");
		assert!(stmt.if_exists);
	}
}
