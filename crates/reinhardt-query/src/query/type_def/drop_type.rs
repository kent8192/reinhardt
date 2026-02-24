//! DROP TYPE statement builder
//!
//! This module provides the `DropTypeStatement` type for building SQL DROP TYPE queries.

use crate::{
	backend::QueryBuilder,
	types::{DynIden, IntoIden},
};

use crate::query::traits::{QueryBuilderTrait, QueryStatementBuilder, QueryStatementWriter};

/// DROP TYPE statement builder
///
/// This struct provides a fluent API for constructing DROP TYPE queries.
///
/// # Examples
///
/// ```rust
/// use reinhardt_query::prelude::*;
///
/// // DROP TYPE my_type
/// let query = Query::drop_type()
///     .name("my_type");
///
/// // DROP TYPE IF EXISTS my_type
/// let query = Query::drop_type()
///     .name("my_type")
///     .if_exists();
///
/// // DROP TYPE my_type CASCADE
/// let query = Query::drop_type()
///     .name("my_type")
///     .cascade();
/// ```
#[derive(Debug, Clone)]
pub struct DropTypeStatement {
	pub(crate) name: DynIden,
	pub(crate) if_exists: bool,
	pub(crate) cascade: bool,
	pub(crate) restrict: bool,
}

impl DropTypeStatement {
	/// Create a new DROP TYPE statement
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::drop_type();
	/// ```
	pub fn new() -> Self {
		Self {
			name: "".into_iden(),
			if_exists: false,
			cascade: false,
			restrict: false,
		}
	}

	/// Take the ownership of data in the current [`DropTypeStatement`]
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

	/// Set the type name
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::drop_type()
	///     .name("my_type");
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
	/// let query = Query::drop_type()
	///     .name("my_type")
	///     .if_exists();
	/// ```
	pub fn if_exists(&mut self) -> &mut Self {
		self.if_exists = true;
		self
	}

	/// Add CASCADE clause
	///
	/// This will also drop objects that depend on the type.
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::drop_type()
	///     .name("my_type")
	///     .cascade();
	/// ```
	pub fn cascade(&mut self) -> &mut Self {
		self.cascade = true;
		self.restrict = false;
		self
	}

	/// Add RESTRICT clause
	///
	/// This will refuse to drop the type if any objects depend on it (default behavior).
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::drop_type()
	///     .name("my_type")
	///     .restrict();
	/// ```
	pub fn restrict(&mut self) -> &mut Self {
		self.restrict = true;
		self.cascade = false;
		self
	}
}

impl Default for DropTypeStatement {
	fn default() -> Self {
		Self::new()
	}
}

impl QueryStatementBuilder for DropTypeStatement {
	fn build_any(&self, query_builder: &dyn QueryBuilderTrait) -> (String, crate::value::Values) {
		// Downcast to concrete QueryBuilder type
		use std::any::Any;
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::PostgresQueryBuilder>()
		{
			return builder.build_drop_type(self);
		}
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::MySqlQueryBuilder>()
		{
			return builder.build_drop_type(self);
		}
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::SqliteQueryBuilder>()
		{
			return builder.build_drop_type(self);
		}
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::CockroachDBQueryBuilder>()
		{
			return builder.build_drop_type(self);
		}
		panic!("Unsupported query builder type");
	}
}

impl QueryStatementWriter for DropTypeStatement {}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::types::Alias;
	use rstest::rstest;

	#[rstest]
	fn test_drop_type_new() {
		let stmt = DropTypeStatement::new();
		assert!(stmt.name.to_string().is_empty());
		assert!(!stmt.if_exists);
		assert!(!stmt.cascade);
		assert!(!stmt.restrict);
	}

	#[rstest]
	fn test_drop_type_with_name() {
		let mut stmt = DropTypeStatement::new();
		stmt.name("my_type");
		assert_eq!(stmt.name.to_string(), "my_type");
	}

	#[rstest]
	fn test_drop_type_if_exists() {
		let mut stmt = DropTypeStatement::new();
		stmt.name("my_type").if_exists();
		assert!(stmt.if_exists);
	}

	#[rstest]
	fn test_drop_type_cascade() {
		let mut stmt = DropTypeStatement::new();
		stmt.name("my_type").cascade();
		assert!(stmt.cascade);
		assert!(!stmt.restrict);
	}

	#[rstest]
	fn test_drop_type_restrict() {
		let mut stmt = DropTypeStatement::new();
		stmt.name("my_type").restrict();
		assert!(stmt.restrict);
		assert!(!stmt.cascade);
	}

	#[rstest]
	fn test_drop_type_cascade_then_restrict() {
		let mut stmt = DropTypeStatement::new();
		stmt.name("my_type").cascade().restrict();
		assert!(stmt.restrict);
		assert!(!stmt.cascade);
	}

	#[rstest]
	fn test_drop_type_restrict_then_cascade() {
		let mut stmt = DropTypeStatement::new();
		stmt.name("my_type").restrict().cascade();
		assert!(stmt.cascade);
		assert!(!stmt.restrict);
	}

	#[rstest]
	fn test_drop_type_take() {
		let mut stmt = DropTypeStatement::new();
		stmt.name("my_type").if_exists().cascade();
		let taken = stmt.take();
		assert!(stmt.name.to_string().is_empty());
		assert!(!stmt.if_exists);
		assert!(!stmt.cascade);
		assert_eq!(taken.name.to_string(), "my_type");
		assert!(taken.if_exists);
		assert!(taken.cascade);
	}

	#[rstest]
	fn test_drop_type_with_alias() {
		let mut stmt = DropTypeStatement::new();
		stmt.name(Alias::new("custom_type"));
		assert_eq!(stmt.name.to_string(), "custom_type");
	}
}
