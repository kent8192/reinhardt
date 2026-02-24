//! OPTIMIZE TABLE statement builder
//!
//! This module provides the `OptimizeTableStatement` type for building SQL OPTIMIZE TABLE queries.
//! **MySQL-only**: This statement is specific to MySQL and MariaDB.

use crate::{
	backend::QueryBuilder,
	types::{DynIden, IntoIden, OptimizeTableOption},
};

use crate::query::traits::{QueryBuilderTrait, QueryStatementBuilder, QueryStatementWriter};

/// OPTIMIZE TABLE statement builder
///
/// This struct provides a fluent API for constructing OPTIMIZE TABLE queries.
/// OPTIMIZE TABLE reorganizes the physical storage of table data and associated index data.
///
/// **MySQL-only**: Other backends will panic with a helpful error message.
///
/// # Examples
///
/// ```rust
/// use reinhardt_query::prelude::*;
///
/// // OPTIMIZE TABLE users
/// let query = Query::optimize_table()
///     .table("users");
///
/// // OPTIMIZE NO_WRITE_TO_BINLOG TABLE users, posts
/// let query = Query::optimize_table()
///     .table("users")
///     .table("posts")
///     .no_write_to_binlog();
///
/// // OPTIMIZE LOCAL TABLE users
/// let query = Query::optimize_table()
///     .table("users")
///     .local();
/// ```
#[derive(Debug, Clone, Default)]
pub struct OptimizeTableStatement {
	pub(crate) tables: Vec<DynIden>,
	pub(crate) no_write_to_binlog: bool,
	pub(crate) local: bool,
}

impl OptimizeTableStatement {
	/// Create a new OPTIMIZE TABLE statement
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::optimize_table();
	/// ```
	pub fn new() -> Self {
		Self::default()
	}

	/// Take the ownership of data in the current [`OptimizeTableStatement`]
	pub fn take(&mut self) -> Self {
		Self {
			tables: std::mem::take(&mut self.tables),
			no_write_to_binlog: std::mem::take(&mut self.no_write_to_binlog),
			local: std::mem::take(&mut self.local),
		}
	}

	/// Add a table to optimize
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::optimize_table()
	///     .table("users");
	/// ```
	pub fn table<T>(&mut self, table: T) -> &mut Self
	where
		T: IntoIden,
	{
		self.tables.push(table.into_iden());
		self
	}

	/// Set NO_WRITE_TO_BINLOG option
	///
	/// Suppresses binary logging for this operation (same as LOCAL).
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::optimize_table()
	///     .table("users")
	///     .no_write_to_binlog();
	/// ```
	pub fn no_write_to_binlog(&mut self) -> &mut Self {
		self.no_write_to_binlog = true;
		self
	}

	/// Set LOCAL option
	///
	/// Suppresses binary logging for this operation (same as NO_WRITE_TO_BINLOG).
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::optimize_table()
	///     .table("users")
	///     .local();
	/// ```
	pub fn local(&mut self) -> &mut Self {
		self.local = true;
		self
	}

	/// Set options from OptimizeTableOption
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	/// use reinhardt_query::types::OptimizeTableOption;
	///
	/// let opt = OptimizeTableOption::new().no_write_to_binlog(true);
	/// let query = Query::optimize_table()
	///     .table("users")
	///     .options(opt);
	/// ```
	pub fn options(&mut self, opt: OptimizeTableOption) -> &mut Self {
		self.no_write_to_binlog = opt.no_write_to_binlog;
		self.local = opt.local;
		self
	}
}

impl QueryStatementBuilder for OptimizeTableStatement {
	fn build_any(&self, query_builder: &dyn QueryBuilderTrait) -> (String, crate::value::Values) {
		// Downcast to concrete QueryBuilder type
		use std::any::Any;
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::PostgresQueryBuilder>()
		{
			return builder.build_optimize_table(self);
		}
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::MySqlQueryBuilder>()
		{
			return builder.build_optimize_table(self);
		}
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::SqliteQueryBuilder>()
		{
			return builder.build_optimize_table(self);
		}
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::CockroachDBQueryBuilder>()
		{
			return builder.build_optimize_table(self);
		}
		panic!("Unsupported query builder type");
	}
}

impl QueryStatementWriter for OptimizeTableStatement {}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::*;

	#[rstest]
	fn test_optimize_table_new() {
		let stmt = OptimizeTableStatement::new();
		assert!(stmt.tables.is_empty());
		assert!(!stmt.no_write_to_binlog);
		assert!(!stmt.local);
	}

	#[rstest]
	fn test_optimize_table_with_table() {
		let mut stmt = OptimizeTableStatement::new();
		stmt.table("users");
		assert_eq!(stmt.tables.len(), 1);
		assert_eq!(stmt.tables[0].to_string(), "users");
	}

	#[rstest]
	fn test_optimize_table_with_multiple_tables() {
		let mut stmt = OptimizeTableStatement::new();
		stmt.table("users").table("posts");
		assert_eq!(stmt.tables.len(), 2);
		assert_eq!(stmt.tables[0].to_string(), "users");
		assert_eq!(stmt.tables[1].to_string(), "posts");
	}

	#[rstest]
	fn test_optimize_table_no_write_to_binlog() {
		let mut stmt = OptimizeTableStatement::new();
		stmt.no_write_to_binlog();
		assert!(stmt.no_write_to_binlog);
		assert!(!stmt.local);
	}

	#[rstest]
	fn test_optimize_table_local() {
		let mut stmt = OptimizeTableStatement::new();
		stmt.local();
		assert!(!stmt.no_write_to_binlog);
		assert!(stmt.local);
	}

	#[rstest]
	fn test_optimize_table_with_option() {
		let opt = OptimizeTableOption::new().no_write_to_binlog(true);
		let mut stmt = OptimizeTableStatement::new();
		stmt.table("users").options(opt);
		assert_eq!(stmt.tables.len(), 1);
		assert!(stmt.no_write_to_binlog);
		assert!(!stmt.local);
	}

	#[rstest]
	fn test_optimize_table_take() {
		let mut stmt = OptimizeTableStatement::new();
		stmt.table("users").no_write_to_binlog();

		let taken = stmt.take();
		assert_eq!(taken.tables.len(), 1);
		assert!(taken.no_write_to_binlog);

		// Original should be reset
		assert!(stmt.tables.is_empty());
		assert!(!stmt.no_write_to_binlog);
	}
}
