//! CHECK TABLE statement builder
//!
//! This module provides the `CheckTableStatement` type for building SQL CHECK TABLE queries.
//! **MySQL-only**: This statement is specific to MySQL and MariaDB.

use crate::{
	backend::QueryBuilder,
	types::{CheckTableOption, DynIden, IntoIden},
};

use crate::query::traits::{QueryBuilderTrait, QueryStatementBuilder, QueryStatementWriter};

/// CHECK TABLE statement builder
///
/// This struct provides a fluent API for constructing CHECK TABLE queries.
/// CHECK TABLE checks a table or tables for errors.
///
/// **MySQL-only**: Other backends will panic with a helpful error message.
///
/// # Examples
///
/// ```rust
/// use reinhardt_query::prelude::*;
/// use reinhardt_query::types::CheckTableOption;
///
/// // CHECK TABLE users
/// let query = Query::check_table()
///     .table("users");
///
/// // CHECK TABLE users QUICK
/// let query = Query::check_table()
///     .table("users")
///     .option(CheckTableOption::Quick);
///
/// // CHECK TABLE users, posts EXTENDED
/// let query = Query::check_table()
///     .table("users")
///     .table("posts")
///     .option(CheckTableOption::Extended);
/// ```
#[derive(Debug, Clone)]
pub struct CheckTableStatement {
	pub(crate) tables: Vec<DynIden>,
	pub(crate) option: CheckTableOption,
}

impl CheckTableStatement {
	/// Create a new CHECK TABLE statement
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::check_table();
	/// ```
	pub fn new() -> Self {
		Self {
			tables: Vec::new(),
			option: CheckTableOption::default(),
		}
	}

	/// Take the ownership of data in the current [`CheckTableStatement`]
	pub fn take(&mut self) -> Self {
		Self {
			tables: std::mem::take(&mut self.tables),
			option: self.option,
		}
	}

	/// Add a table to check
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::check_table()
	///     .table("users");
	/// ```
	pub fn table<T>(&mut self, table: T) -> &mut Self
	where
		T: IntoIden,
	{
		self.tables.push(table.into_iden());
		self
	}

	/// Set check option
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	/// use reinhardt_query::types::CheckTableOption;
	///
	/// let query = Query::check_table()
	///     .table("users")
	///     .option(CheckTableOption::Quick);
	/// ```
	pub fn option(&mut self, option: CheckTableOption) -> &mut Self {
		self.option = option;
		self
	}
}

impl Default for CheckTableStatement {
	fn default() -> Self {
		Self::new()
	}
}

impl QueryStatementBuilder for CheckTableStatement {
	fn build_any(&self, query_builder: &dyn QueryBuilderTrait) -> (String, crate::value::Values) {
		// Downcast to concrete QueryBuilder type
		use std::any::Any;
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::PostgresQueryBuilder>()
		{
			return builder.build_check_table(self);
		}
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::MySqlQueryBuilder>()
		{
			return builder.build_check_table(self);
		}
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::SqliteQueryBuilder>()
		{
			return builder.build_check_table(self);
		}
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::CockroachDBQueryBuilder>()
		{
			return builder.build_check_table(self);
		}
		panic!("Unsupported query builder type");
	}
}

impl QueryStatementWriter for CheckTableStatement {}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::*;

	#[rstest]
	fn test_check_table_new() {
		let stmt = CheckTableStatement::new();
		assert!(stmt.tables.is_empty());
		assert_eq!(stmt.option, CheckTableOption::Medium);
	}

	#[rstest]
	fn test_check_table_with_table() {
		let mut stmt = CheckTableStatement::new();
		stmt.table("users");
		assert_eq!(stmt.tables.len(), 1);
		assert_eq!(stmt.tables[0].to_string(), "users");
	}

	#[rstest]
	fn test_check_table_with_multiple_tables() {
		let mut stmt = CheckTableStatement::new();
		stmt.table("users").table("posts");
		assert_eq!(stmt.tables.len(), 2);
		assert_eq!(stmt.tables[0].to_string(), "users");
		assert_eq!(stmt.tables[1].to_string(), "posts");
	}

	#[rstest]
	fn test_check_table_with_quick_option() {
		let mut stmt = CheckTableStatement::new();
		stmt.table("users").option(CheckTableOption::Quick);
		assert_eq!(stmt.tables.len(), 1);
		assert_eq!(stmt.option, CheckTableOption::Quick);
	}

	#[rstest]
	fn test_check_table_with_extended_option() {
		let mut stmt = CheckTableStatement::new();
		stmt.table("users").option(CheckTableOption::Extended);
		assert_eq!(stmt.tables.len(), 1);
		assert_eq!(stmt.option, CheckTableOption::Extended);
	}

	#[rstest]
	fn test_check_table_with_for_upgrade_option() {
		let mut stmt = CheckTableStatement::new();
		stmt.table("users").option(CheckTableOption::ForUpgrade);
		assert_eq!(stmt.tables.len(), 1);
		assert_eq!(stmt.option, CheckTableOption::ForUpgrade);
	}

	#[rstest]
	fn test_check_table_take() {
		let mut stmt = CheckTableStatement::new();
		stmt.table("users").option(CheckTableOption::Quick);

		let taken = stmt.take();
		assert_eq!(taken.tables.len(), 1);
		assert_eq!(taken.option, CheckTableOption::Quick);

		// Original should be reset
		assert!(stmt.tables.is_empty());
		assert_eq!(stmt.option, CheckTableOption::Quick); // option is Copy, so it's not moved
	}
}
