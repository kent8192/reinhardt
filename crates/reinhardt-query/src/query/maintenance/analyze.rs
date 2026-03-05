//! ANALYZE statement builder
//!
//! This module provides the `AnalyzeStatement` type for building SQL ANALYZE queries.

use crate::{
	backend::QueryBuilder,
	types::{AnalyzeTable, IntoIden},
};

use crate::query::traits::{QueryBuilderTrait, QueryStatementBuilder, QueryStatementWriter};

/// ANALYZE statement builder
///
/// This struct provides a fluent API for constructing ANALYZE queries.
/// ANALYZE collects statistics about table contents for the query planner.
///
/// # Examples
///
/// ```rust
/// use reinhardt_query::prelude::*;
///
/// // ANALYZE (all tables)
/// let query = Query::analyze();
///
/// // ANALYZE users
/// let query = Query::analyze()
///     .table("users");
///
/// // ANALYZE VERBOSE users
/// let query = Query::analyze()
///     .table("users")
///     .verbose();
///
/// // ANALYZE users (email, name)
/// let query = Query::analyze()
///     .table_columns("users", ["email", "name"]);
/// ```
#[derive(Debug, Clone, Default)]
pub struct AnalyzeStatement {
	pub(crate) tables: Vec<AnalyzeTable>,
	pub(crate) verbose: bool,
}

impl AnalyzeStatement {
	/// Create a new ANALYZE statement
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::analyze();
	/// ```
	pub fn new() -> Self {
		Self::default()
	}

	/// Take the ownership of data in the current [`AnalyzeStatement`]
	pub fn take(&mut self) -> Self {
		Self {
			tables: std::mem::take(&mut self.tables),
			verbose: std::mem::take(&mut self.verbose),
		}
	}

	/// Add a table to analyze
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::analyze()
	///     .table("users");
	/// ```
	pub fn table<T>(&mut self, table: T) -> &mut Self
	where
		T: IntoIden,
	{
		self.tables.push(AnalyzeTable::new(table));
		self
	}

	/// Add a table with specific columns to analyze
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::analyze()
	///     .table_columns("users", ["email", "name"]);
	/// ```
	pub fn table_columns<T, I, C>(&mut self, table: T, columns: I) -> &mut Self
	where
		T: IntoIden,
		I: IntoIterator<Item = C>,
		C: IntoIden,
	{
		let mut tbl = AnalyzeTable::new(table);
		for col in columns {
			tbl = tbl.add_column(col);
		}
		self.tables.push(tbl);
		self
	}

	/// Set VERBOSE option
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::analyze()
	///     .verbose();
	/// ```
	pub fn verbose(&mut self) -> &mut Self {
		self.verbose = true;
		self
	}
}

impl QueryStatementBuilder for AnalyzeStatement {
	fn build_any(&self, query_builder: &dyn QueryBuilderTrait) -> (String, crate::value::Values) {
		// Downcast to concrete QueryBuilder type
		use std::any::Any;
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::PostgresQueryBuilder>()
		{
			return builder.build_analyze(self);
		}
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::MySqlQueryBuilder>()
		{
			return builder.build_analyze(self);
		}
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::SqliteQueryBuilder>()
		{
			return builder.build_analyze(self);
		}
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::CockroachDBQueryBuilder>()
		{
			return builder.build_analyze(self);
		}
		panic!("Unsupported query builder type");
	}
}

impl QueryStatementWriter for AnalyzeStatement {}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::*;

	#[rstest]
	fn test_analyze_new() {
		let stmt = AnalyzeStatement::new();
		assert!(stmt.tables.is_empty());
		assert!(!stmt.verbose);
	}

	#[rstest]
	fn test_analyze_with_table() {
		let mut stmt = AnalyzeStatement::new();
		stmt.table("users");
		assert_eq!(stmt.tables.len(), 1);
		assert_eq!(stmt.tables[0].table.to_string(), "users");
		assert!(stmt.tables[0].columns.is_empty());
	}

	#[rstest]
	fn test_analyze_with_multiple_tables() {
		let mut stmt = AnalyzeStatement::new();
		stmt.table("users").table("posts");
		assert_eq!(stmt.tables.len(), 2);
		assert_eq!(stmt.tables[0].table.to_string(), "users");
		assert_eq!(stmt.tables[1].table.to_string(), "posts");
	}

	#[rstest]
	fn test_analyze_with_columns() {
		let mut stmt = AnalyzeStatement::new();
		stmt.table_columns("users", ["email", "name"]);
		assert_eq!(stmt.tables.len(), 1);
		assert_eq!(stmt.tables[0].table.to_string(), "users");
		assert_eq!(stmt.tables[0].columns.len(), 2);
		assert_eq!(stmt.tables[0].columns[0].to_string(), "email");
		assert_eq!(stmt.tables[0].columns[1].to_string(), "name");
	}

	#[rstest]
	fn test_analyze_verbose() {
		let mut stmt = AnalyzeStatement::new();
		stmt.verbose();
		assert!(stmt.verbose);
	}

	#[rstest]
	fn test_analyze_combined() {
		let mut stmt = AnalyzeStatement::new();
		stmt.table("users")
			.table_columns("posts", ["title", "content"])
			.verbose();
		assert_eq!(stmt.tables.len(), 2);
		assert_eq!(stmt.tables[0].table.to_string(), "users");
		assert!(stmt.tables[0].columns.is_empty());
		assert_eq!(stmt.tables[1].table.to_string(), "posts");
		assert_eq!(stmt.tables[1].columns.len(), 2);
		assert!(stmt.verbose);
	}

	#[rstest]
	fn test_analyze_take() {
		let mut stmt = AnalyzeStatement::new();
		stmt.table("users").verbose();

		let taken = stmt.take();
		assert_eq!(taken.tables.len(), 1);
		assert!(taken.verbose);

		// Original should be reset
		assert!(stmt.tables.is_empty());
		assert!(!stmt.verbose);
	}
}
