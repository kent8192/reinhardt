//! DROP INDEX statement builder
//!
//! This module provides the `DropIndexStatement` type for building SQL DROP INDEX queries.

use crate::{
	backend::QueryBuilder,
	types::{DynIden, IntoIden, IntoTableRef, TableRef},
};

use super::traits::{QueryBuilderTrait, QueryStatementBuilder, QueryStatementWriter};

/// DROP INDEX statement builder
///
/// This struct provides a fluent API for constructing DROP INDEX queries.
///
/// # Examples
///
/// ```rust,ignore
/// use reinhardt_query::prelude::*;
///
/// let query = Query::drop_index()
///     .name("idx_email")
///     .table("users")  // Required for MySQL
///     .if_exists();
/// ```
#[derive(Debug, Clone)]
pub struct DropIndexStatement {
	pub(crate) name: Option<DynIden>,
	pub(crate) table: Option<TableRef>,
	pub(crate) if_exists: bool,
	pub(crate) cascade: bool,
	pub(crate) restrict: bool,
}

impl DropIndexStatement {
	/// Create a new DROP INDEX statement
	pub fn new() -> Self {
		Self {
			name: None,
			table: None,
			if_exists: false,
			cascade: false,
			restrict: false,
		}
	}

	/// Take the ownership of data in the current [`DropIndexStatement`]
	pub fn take(&mut self) -> Self {
		Self {
			name: self.name.take(),
			table: self.table.take(),
			if_exists: self.if_exists,
			cascade: self.cascade,
			restrict: self.restrict,
		}
	}

	/// Set the index name to drop
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::drop_index()
	///     .name("idx_email");
	/// ```
	pub fn name<T>(&mut self, name: T) -> &mut Self
	where
		T: IntoIden,
	{
		self.name = Some(name.into_iden());
		self
	}

	/// Set the table (required for MySQL)
	///
	/// MySQL requires the table name when dropping an index.
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::drop_index()
	///     .name("idx_email")
	///     .table("users");
	/// ```
	pub fn table<T>(&mut self, tbl: T) -> &mut Self
	where
		T: IntoTableRef,
	{
		self.table = Some(tbl.into_table_ref());
		self
	}

	/// Add IF EXISTS clause
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::drop_index()
	///     .name("idx_email")
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
	/// let query = Query::drop_index()
	///     .name("idx_email")
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
	/// let query = Query::drop_index()
	///     .name("idx_email")
	///     .restrict();
	/// ```
	pub fn restrict(&mut self) -> &mut Self {
		self.restrict = true;
		self.cascade = false;
		self
	}
}

impl Default for DropIndexStatement {
	fn default() -> Self {
		Self::new()
	}
}

impl QueryStatementBuilder for DropIndexStatement {
	fn build_any(&self, query_builder: &dyn QueryBuilderTrait) -> (String, crate::value::Values) {
		// Downcast to concrete QueryBuilder type
		use std::any::Any;
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::PostgresQueryBuilder>()
		{
			return builder.build_drop_index(self);
		}
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::MySqlQueryBuilder>()
		{
			return builder.build_drop_index(self);
		}
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::SqliteQueryBuilder>()
		{
			return builder.build_drop_index(self);
		}
		panic!("Unsupported query builder type");
	}
}

impl QueryStatementWriter for DropIndexStatement {}
