//! DROP DATABASE statement builder
//!
//! This module provides the `DropDatabaseStatement` type for building SQL DROP DATABASE queries.

use crate::{
	backend::QueryBuilder,
	types::{DynIden, IntoIden},
};

use crate::query::traits::{QueryBuilderTrait, QueryStatementBuilder, QueryStatementWriter};

/// DROP DATABASE statement builder
///
/// This struct provides a fluent API for constructing DROP DATABASE queries.
/// It supports both basic DROP DATABASE and PostgreSQL-specific options.
///
/// # Examples
///
/// ```rust
/// use reinhardt_query::prelude::*;
///
/// // DROP DATABASE mydb
/// let query = Query::drop_database()
///     .name("mydb");
///
/// // DROP DATABASE IF EXISTS mydb
/// let query = Query::drop_database()
///     .name("mydb")
///     .if_exists();
///
/// // DROP DATABASE mydb WITH (FORCE) (PostgreSQL)
/// let query = Query::drop_database()
///     .name("mydb")
///     .force();
/// ```
#[derive(Debug, Clone)]
pub struct DropDatabaseStatement {
	pub(crate) database_name: Option<DynIden>,
	pub(crate) if_exists: bool,
	pub(crate) force: bool,
	pub(crate) cascade: bool,
}

impl DropDatabaseStatement {
	/// Create a new DROP DATABASE statement
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::drop_database();
	/// ```
	pub fn new() -> Self {
		Self {
			database_name: None,
			if_exists: false,
			force: false,
			cascade: false,
		}
	}

	/// Take the ownership of data in the current [`DropDatabaseStatement`]
	pub fn take(&mut self) -> Self {
		let taken = Self {
			database_name: self.database_name.take(),
			if_exists: self.if_exists,
			force: self.force,
			cascade: self.cascade,
		};
		// Reset boolean fields to default after taking
		self.if_exists = false;
		self.force = false;
		self.cascade = false;
		taken
	}

	/// Set the database name
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::drop_database()
	///     .name("mydb");
	/// ```
	pub fn name<N>(&mut self, name: N) -> &mut Self
	where
		N: IntoIden,
	{
		self.database_name = Some(name.into_iden());
		self
	}

	/// Add IF EXISTS clause
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::drop_database()
	///     .name("mydb")
	///     .if_exists();
	/// ```
	pub fn if_exists(&mut self) -> &mut Self {
		self.if_exists = true;
		self
	}

	/// Add FORCE option (PostgreSQL 13+)
	///
	/// Forces termination of all connections to the database before dropping it.
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::drop_database()
	///     .name("mydb")
	///     .force();
	/// ```
	pub fn force(&mut self) -> &mut Self {
		self.force = true;
		self
	}

	/// Add CASCADE option
	///
	/// Automatically drops objects contained in the database.
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::drop_database()
	///     .name("mydb")
	///     .cascade();
	/// ```
	pub fn cascade(&mut self) -> &mut Self {
		self.cascade = true;
		self
	}
}

impl Default for DropDatabaseStatement {
	fn default() -> Self {
		Self::new()
	}
}

impl QueryStatementBuilder for DropDatabaseStatement {
	fn build_any(&self, query_builder: &dyn QueryBuilderTrait) -> (String, crate::value::Values) {
		// Downcast to concrete QueryBuilder type
		use std::any::Any;
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::PostgresQueryBuilder>()
		{
			return builder.build_drop_database(self);
		}
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::MySqlQueryBuilder>()
		{
			return builder.build_drop_database(self);
		}
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::SqliteQueryBuilder>()
		{
			return builder.build_drop_database(self);
		}
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::CockroachDBQueryBuilder>()
		{
			return builder.build_drop_database(self);
		}
		panic!("Unsupported query builder type");
	}
}

impl QueryStatementWriter for DropDatabaseStatement {}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::*;

	#[rstest]
	fn test_drop_database_new() {
		let stmt = DropDatabaseStatement::new();
		assert!(stmt.database_name.is_none());
		assert!(!stmt.if_exists);
		assert!(!stmt.force);
		assert!(!stmt.cascade);
	}

	#[rstest]
	fn test_drop_database_with_name() {
		let mut stmt = DropDatabaseStatement::new();
		stmt.name("mydb");
		assert_eq!(stmt.database_name.as_ref().unwrap().to_string(), "mydb");
	}

	#[rstest]
	fn test_drop_database_if_exists() {
		let mut stmt = DropDatabaseStatement::new();
		stmt.name("mydb").if_exists();
		assert!(stmt.if_exists);
	}

	#[rstest]
	fn test_drop_database_with_force() {
		let mut stmt = DropDatabaseStatement::new();
		stmt.name("mydb").force();
		assert!(stmt.force);
	}

	#[rstest]
	fn test_drop_database_with_cascade() {
		let mut stmt = DropDatabaseStatement::new();
		stmt.name("mydb").cascade();
		assert!(stmt.cascade);
	}

	#[rstest]
	fn test_drop_database_full_options() {
		let mut stmt = DropDatabaseStatement::new();
		stmt.name("mydb").if_exists().force().cascade();
		assert_eq!(stmt.database_name.as_ref().unwrap().to_string(), "mydb");
		assert!(stmt.if_exists);
		assert!(stmt.force);
		assert!(stmt.cascade);
	}

	#[rstest]
	fn test_drop_database_take() {
		let mut stmt = DropDatabaseStatement::new();
		stmt.name("mydb").force();
		let taken = stmt.take();
		assert!(stmt.database_name.is_none());
		assert!(!stmt.force);
		assert_eq!(taken.database_name.as_ref().unwrap().to_string(), "mydb");
		assert!(taken.force);
	}
}
