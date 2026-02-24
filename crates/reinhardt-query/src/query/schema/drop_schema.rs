//! DROP SCHEMA statement builder
//!
//! This module provides the `DropSchemaStatement` type for building SQL DROP SCHEMA queries.

use crate::{
	backend::QueryBuilder,
	types::{DynIden, IntoIden},
};

use crate::query::traits::{QueryBuilderTrait, QueryStatementBuilder, QueryStatementWriter};

/// DROP SCHEMA statement builder
///
/// This struct provides a fluent API for constructing DROP SCHEMA queries.
///
/// # Examples
///
/// ```rust
/// use reinhardt_query::prelude::*;
///
/// // DROP SCHEMA my_schema
/// let query = Query::drop_schema()
///     .name("my_schema");
///
/// // DROP SCHEMA IF EXISTS my_schema
/// let query = Query::drop_schema()
///     .name("my_schema")
///     .if_exists();
///
/// // DROP SCHEMA my_schema CASCADE
/// let query = Query::drop_schema()
///     .name("my_schema")
///     .cascade();
/// ```
#[derive(Debug, Clone)]
pub struct DropSchemaStatement {
	pub(crate) schema_name: Option<DynIden>,
	pub(crate) if_exists: bool,
	pub(crate) cascade: bool,
}

impl DropSchemaStatement {
	/// Create a new DROP SCHEMA statement
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::drop_schema();
	/// ```
	pub fn new() -> Self {
		Self {
			schema_name: None,
			if_exists: false,
			cascade: false,
		}
	}

	/// Take the ownership of data in the current [`DropSchemaStatement`]
	pub fn take(&mut self) -> Self {
		let taken = Self {
			schema_name: self.schema_name.take(),
			if_exists: self.if_exists,
			cascade: self.cascade,
		};
		self.if_exists = false;
		self.cascade = false;
		taken
	}

	/// Set the schema name
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::drop_schema()
	///     .name("my_schema");
	/// ```
	pub fn name<N>(&mut self, name: N) -> &mut Self
	where
		N: IntoIden,
	{
		self.schema_name = Some(name.into_iden());
		self
	}

	/// Add IF EXISTS clause
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::drop_schema()
	///     .name("my_schema")
	///     .if_exists();
	/// ```
	pub fn if_exists(&mut self) -> &mut Self {
		self.if_exists = true;
		self
	}

	/// Add CASCADE clause
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::drop_schema()
	///     .name("my_schema")
	///     .cascade();
	/// ```
	pub fn cascade(&mut self) -> &mut Self {
		self.cascade = true;
		self
	}
}

impl Default for DropSchemaStatement {
	fn default() -> Self {
		Self::new()
	}
}

impl QueryStatementBuilder for DropSchemaStatement {
	fn build_any(&self, query_builder: &dyn QueryBuilderTrait) -> (String, crate::value::Values) {
		// Downcast to concrete QueryBuilder type
		use std::any::Any;
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::PostgresQueryBuilder>()
		{
			return builder.build_drop_schema(self);
		}
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::MySqlQueryBuilder>()
		{
			return builder.build_drop_schema(self);
		}
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::SqliteQueryBuilder>()
		{
			return builder.build_drop_schema(self);
		}
		panic!("Unsupported query builder type");
	}
}

impl QueryStatementWriter for DropSchemaStatement {}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::*;

	#[rstest]
	fn test_drop_schema_new() {
		let stmt = DropSchemaStatement::new();
		assert!(stmt.schema_name.is_none());
		assert!(!stmt.if_exists);
		assert!(!stmt.cascade);
	}

	#[rstest]
	fn test_drop_schema_with_name() {
		let mut stmt = DropSchemaStatement::new();
		stmt.name("my_schema");
		assert_eq!(stmt.schema_name.as_ref().unwrap().to_string(), "my_schema");
	}

	#[rstest]
	fn test_drop_schema_if_exists() {
		let mut stmt = DropSchemaStatement::new();
		stmt.name("my_schema").if_exists();
		assert!(stmt.if_exists);
	}

	#[rstest]
	fn test_drop_schema_cascade() {
		let mut stmt = DropSchemaStatement::new();
		stmt.name("my_schema").cascade();
		assert!(stmt.cascade);
	}

	#[rstest]
	fn test_drop_schema_all_options() {
		let mut stmt = DropSchemaStatement::new();
		stmt.name("my_schema").if_exists().cascade();
		assert_eq!(stmt.schema_name.as_ref().unwrap().to_string(), "my_schema");
		assert!(stmt.if_exists);
		assert!(stmt.cascade);
	}

	#[rstest]
	fn test_drop_schema_take() {
		let mut stmt = DropSchemaStatement::new();
		stmt.name("my_schema").if_exists();
		let taken = stmt.take();
		assert!(stmt.schema_name.is_none());
		assert!(!stmt.if_exists);
		assert_eq!(taken.schema_name.as_ref().unwrap().to_string(), "my_schema");
		assert!(taken.if_exists);
	}
}
