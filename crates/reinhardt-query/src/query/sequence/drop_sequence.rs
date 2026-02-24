//! DROP SEQUENCE statement builder
//!
//! This module provides the `DropSequenceStatement` type for building SQL DROP SEQUENCE queries.

use crate::{
	backend::QueryBuilder,
	types::{DynIden, IntoIden},
};

use crate::query::traits::{QueryBuilderTrait, QueryStatementBuilder, QueryStatementWriter};

/// DROP SEQUENCE statement builder
///
/// This struct provides a fluent API for constructing DROP SEQUENCE queries.
///
/// # Examples
///
/// ```rust
/// use reinhardt_query::prelude::*;
///
/// // DROP SEQUENCE my_seq
/// let query = Query::drop_sequence()
///     .name("my_seq");
///
/// // DROP SEQUENCE IF EXISTS my_seq
/// let query = Query::drop_sequence()
///     .name("my_seq")
///     .if_exists();
///
/// // DROP SEQUENCE my_seq CASCADE
/// let query = Query::drop_sequence()
///     .name("my_seq")
///     .cascade();
/// ```
#[derive(Debug, Clone)]
pub struct DropSequenceStatement {
	pub(crate) name: DynIden,
	pub(crate) if_exists: bool,
	pub(crate) cascade: bool,
	pub(crate) restrict: bool,
}

impl DropSequenceStatement {
	/// Create a new DROP SEQUENCE statement
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::drop_sequence();
	/// ```
	pub fn new() -> Self {
		Self {
			name: "".into_iden(),
			if_exists: false,
			cascade: false,
			restrict: false,
		}
	}

	/// Take the ownership of data in the current [`DropSequenceStatement`]
	pub fn take(&mut self) -> Self {
		let taken = Self {
			name: self.name.clone(),
			if_exists: self.if_exists,
			cascade: self.cascade,
			restrict: self.restrict,
		};
		// Reset self to empty state
		self.name = "".into_iden();
		self.if_exists = false;
		self.cascade = false;
		self.restrict = false;
		taken
	}

	/// Set the sequence name
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::drop_sequence()
	///     .name("my_seq");
	/// ```
	pub fn name<N>(&mut self, name: N) -> &mut Self
	where
		N: IntoIden,
	{
		self.name = name.into_iden();
		self
	}

	/// Add IF EXISTS clause
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::drop_sequence()
	///     .name("my_seq")
	///     .if_exists();
	/// ```
	pub fn if_exists(&mut self) -> &mut Self {
		self.if_exists = true;
		self
	}

	/// Add CASCADE clause
	///
	/// This will also drop objects that depend on the sequence.
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::drop_sequence()
	///     .name("my_seq")
	///     .cascade();
	/// ```
	pub fn cascade(&mut self) -> &mut Self {
		self.cascade = true;
		self.restrict = false;
		self
	}

	/// Add RESTRICT clause
	///
	/// This will refuse to drop the sequence if any objects depend on it (default behavior).
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::drop_sequence()
	///     .name("my_seq")
	///     .restrict();
	/// ```
	pub fn restrict(&mut self) -> &mut Self {
		self.restrict = true;
		self.cascade = false;
		self
	}
}

impl Default for DropSequenceStatement {
	fn default() -> Self {
		Self::new()
	}
}

impl QueryStatementBuilder for DropSequenceStatement {
	fn build_any(&self, query_builder: &dyn QueryBuilderTrait) -> (String, crate::value::Values) {
		// Downcast to concrete QueryBuilder type
		use std::any::Any;
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::PostgresQueryBuilder>()
		{
			return builder.build_drop_sequence(self);
		}
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::MySqlQueryBuilder>()
		{
			return builder.build_drop_sequence(self);
		}
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::SqliteQueryBuilder>()
		{
			return builder.build_drop_sequence(self);
		}
		panic!("Unsupported query builder type");
	}
}

impl QueryStatementWriter for DropSequenceStatement {}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::*;

	#[rstest]
	fn test_drop_sequence_new() {
		let stmt = DropSequenceStatement::new();
		assert!(stmt.name.to_string().is_empty());
		assert!(!stmt.if_exists);
		assert!(!stmt.cascade);
		assert!(!stmt.restrict);
	}

	#[rstest]
	fn test_drop_sequence_with_name() {
		let mut stmt = DropSequenceStatement::new();
		stmt.name("my_seq");
		assert_eq!(stmt.name.to_string(), "my_seq");
	}

	#[rstest]
	fn test_drop_sequence_if_exists() {
		let mut stmt = DropSequenceStatement::new();
		stmt.name("my_seq").if_exists();
		assert!(stmt.if_exists);
	}

	#[rstest]
	fn test_drop_sequence_cascade() {
		let mut stmt = DropSequenceStatement::new();
		stmt.name("my_seq").cascade();
		assert!(stmt.cascade);
		assert!(!stmt.restrict);
	}

	#[rstest]
	fn test_drop_sequence_restrict() {
		let mut stmt = DropSequenceStatement::new();
		stmt.name("my_seq").restrict();
		assert!(stmt.restrict);
		assert!(!stmt.cascade);
	}

	#[rstest]
	fn test_drop_sequence_cascade_then_restrict() {
		let mut stmt = DropSequenceStatement::new();
		stmt.name("my_seq").cascade().restrict();
		assert!(stmt.restrict);
		assert!(!stmt.cascade);
	}

	#[rstest]
	fn test_drop_sequence_restrict_then_cascade() {
		let mut stmt = DropSequenceStatement::new();
		stmt.name("my_seq").restrict().cascade();
		assert!(stmt.cascade);
		assert!(!stmt.restrict);
	}

	#[rstest]
	fn test_drop_sequence_take() {
		let mut stmt = DropSequenceStatement::new();
		stmt.name("my_seq").if_exists().cascade();
		let taken = stmt.take();
		assert!(stmt.name.to_string().is_empty());
		assert!(!stmt.if_exists);
		assert!(!stmt.cascade);
		assert_eq!(taken.name.to_string(), "my_seq");
		assert!(taken.if_exists);
		assert!(taken.cascade);
	}
}
