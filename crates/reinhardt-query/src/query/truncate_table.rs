//! TRUNCATE TABLE statement builder
//!
//! This module provides the `TruncateTableStatement` type for building SQL TRUNCATE TABLE queries.

use crate::{
	backend::QueryBuilder,
	types::{IntoTableRef, TableRef},
};

use super::traits::{QueryBuilderTrait, QueryStatementBuilder, QueryStatementWriter};

/// TRUNCATE TABLE statement builder
///
/// This struct provides a fluent API for constructing TRUNCATE TABLE queries.
///
/// # Examples
///
/// ```rust,ignore
/// use reinhardt_query::prelude::*;
///
/// let query = Query::truncate_table()
///     .table("users")
///     .restart_identity();
/// ```
#[derive(Debug, Clone)]
pub struct TruncateTableStatement {
	pub(crate) tables: Vec<TableRef>,
	pub(crate) cascade: bool,
	pub(crate) restrict: bool,
	pub(crate) restart_identity: bool,
}

impl TruncateTableStatement {
	/// Create a new TRUNCATE TABLE statement
	pub fn new() -> Self {
		Self {
			tables: Vec::new(),
			cascade: false,
			restrict: false,
			restart_identity: false,
		}
	}

	/// Take the ownership of data in the current [`TruncateTableStatement`]
	pub fn take(&mut self) -> Self {
		Self {
			tables: std::mem::take(&mut self.tables),
			cascade: self.cascade,
			restrict: self.restrict,
			restart_identity: self.restart_identity,
		}
	}

	/// Set the table to truncate
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::truncate_table()
	///     .table("users");
	/// ```
	pub fn table<T>(&mut self, tbl: T) -> &mut Self
	where
		T: IntoTableRef,
	{
		self.tables.push(tbl.into_table_ref());
		self
	}

	/// Set multiple tables to truncate
	///
	/// Note: MySQL does not support truncating multiple tables in a single statement.
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::truncate_table()
	///     .tables(vec!["users", "posts", "comments"]);
	/// ```
	pub fn tables<I, T>(&mut self, tbls: I) -> &mut Self
	where
		I: IntoIterator<Item = T>,
		T: IntoTableRef,
	{
		for tbl in tbls {
			self.tables.push(tbl.into_table_ref());
		}
		self
	}

	/// Add CASCADE clause
	///
	/// This option automatically truncates dependent tables.
	/// Supported by PostgreSQL only.
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::truncate_table()
	///     .table("users")
	///     .cascade();
	/// ```
	pub fn cascade(&mut self) -> &mut Self {
		self.cascade = true;
		self.restrict = false;
		self
	}

	/// Add RESTRICT clause
	///
	/// This option prevents truncating if there are dependent tables (default behavior).
	/// Supported by PostgreSQL only.
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::truncate_table()
	///     .table("users")
	///     .restrict();
	/// ```
	pub fn restrict(&mut self) -> &mut Self {
		self.restrict = true;
		self.cascade = false;
		self
	}

	/// Add RESTART IDENTITY clause
	///
	/// This option resets sequences associated with columns.
	/// Supported by PostgreSQL only.
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::truncate_table()
	///     .table("users")
	///     .restart_identity();
	/// ```
	pub fn restart_identity(&mut self) -> &mut Self {
		self.restart_identity = true;
		self
	}
}

impl Default for TruncateTableStatement {
	fn default() -> Self {
		Self::new()
	}
}

impl QueryStatementBuilder for TruncateTableStatement {
	fn build_any(&self, query_builder: &dyn QueryBuilderTrait) -> (String, crate::value::Values) {
		// Downcast to concrete QueryBuilder type
		use std::any::Any;
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::PostgresQueryBuilder>()
		{
			return builder.build_truncate_table(self);
		}
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::MySqlQueryBuilder>()
		{
			return builder.build_truncate_table(self);
		}
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::SqliteQueryBuilder>()
		{
			return builder.build_truncate_table(self);
		}
		panic!("Unsupported query builder type");
	}
}

impl QueryStatementWriter for TruncateTableStatement {}
