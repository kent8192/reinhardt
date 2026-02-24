//! DROP TABLE statement builder
//!
//! This module provides the `DropTableStatement` type for building SQL DROP TABLE queries.

use crate::{
	backend::QueryBuilder,
	types::{IntoTableRef, TableRef},
};

use super::traits::{QueryBuilderTrait, QueryStatementBuilder, QueryStatementWriter};

/// DROP TABLE statement builder
///
/// This struct provides a fluent API for constructing DROP TABLE queries.
///
/// # Examples
///
/// ```rust,ignore
/// use reinhardt_query::prelude::*;
///
/// let query = Query::drop_table()
///     .table("users")
///     .if_exists();
/// ```
#[derive(Debug, Clone)]
pub struct DropTableStatement {
	pub(crate) tables: Vec<TableRef>,
	pub(crate) if_exists: bool,
	pub(crate) cascade: bool,
	pub(crate) restrict: bool,
}

impl DropTableStatement {
	/// Create a new DROP TABLE statement
	pub fn new() -> Self {
		Self {
			tables: Vec::new(),
			if_exists: false,
			cascade: false,
			restrict: false,
		}
	}

	/// Take the ownership of data in the current [`DropTableStatement`]
	pub fn take(&mut self) -> Self {
		Self {
			tables: std::mem::take(&mut self.tables),
			if_exists: self.if_exists,
			cascade: self.cascade,
			restrict: self.restrict,
		}
	}

	/// Set the table to drop
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::drop_table()
	///     .table("users");
	/// ```
	pub fn table<T>(&mut self, tbl: T) -> &mut Self
	where
		T: IntoTableRef,
	{
		self.tables.push(tbl.into_table_ref());
		self
	}

	/// Set multiple tables to drop
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::drop_table()
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

	/// Add IF EXISTS clause
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::drop_table()
	///     .table("users")
	///     .if_exists();
	/// ```
	pub fn if_exists(&mut self) -> &mut Self {
		self.if_exists = true;
		self
	}

	/// Add CASCADE clause
	///
	/// This option automatically drops dependent objects.
	/// Supported by PostgreSQL.
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::drop_table()
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
	/// This option prevents dropping if there are dependent objects (default behavior).
	/// Supported by PostgreSQL.
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::drop_table()
	///     .table("users")
	///     .restrict();
	/// ```
	pub fn restrict(&mut self) -> &mut Self {
		self.restrict = true;
		self.cascade = false;
		self
	}
}

impl Default for DropTableStatement {
	fn default() -> Self {
		Self::new()
	}
}

impl QueryStatementBuilder for DropTableStatement {
	fn build_any(&self, query_builder: &dyn QueryBuilderTrait) -> (String, crate::value::Values) {
		// Downcast to concrete QueryBuilder type
		use std::any::Any;
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::PostgresQueryBuilder>()
		{
			return builder.build_drop_table(self);
		}
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::MySqlQueryBuilder>()
		{
			return builder.build_drop_table(self);
		}
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::SqliteQueryBuilder>()
		{
			return builder.build_drop_table(self);
		}
		panic!("Unsupported query builder type");
	}
}

impl QueryStatementWriter for DropTableStatement {}
