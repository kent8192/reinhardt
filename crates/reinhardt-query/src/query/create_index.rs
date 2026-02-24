//! CREATE INDEX statement builder
//!
//! This module provides the `CreateIndexStatement` type for building SQL CREATE INDEX queries.

use crate::{
	backend::QueryBuilder,
	expr::SimpleExpr,
	types::{DynIden, IntoIden, IntoTableRef, Order, TableRef},
};

use super::traits::{QueryBuilderTrait, QueryStatementBuilder, QueryStatementWriter};

/// CREATE INDEX statement builder
///
/// This struct provides a fluent API for constructing CREATE INDEX queries.
///
/// # Examples
///
/// ```rust,ignore
/// use reinhardt_query::prelude::*;
///
/// let query = Query::create_index()
///     .name("idx_email")
///     .table("users")
///     .col("email")
///     .unique();
/// ```
#[derive(Debug, Clone)]
pub struct CreateIndexStatement {
	pub(crate) name: Option<DynIden>,
	pub(crate) table: Option<TableRef>,
	pub(crate) columns: Vec<IndexColumn>,
	pub(crate) unique: bool,
	pub(crate) if_not_exists: bool,
	pub(crate) r#where: Option<SimpleExpr>,
	pub(crate) using: Option<IndexMethod>,
}

/// Index column specification
///
/// This struct represents a column in an index, including its name and sort order.
#[derive(Debug, Clone)]
pub struct IndexColumn {
	pub(crate) name: DynIden,
	pub(crate) order: Option<Order>,
}

/// Index method (PostgreSQL and MySQL)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum IndexMethod {
	/// BTREE - B-Tree index (default for most databases)
	BTree,
	/// HASH - Hash index
	Hash,
	/// GIST - Generalized Search Tree (PostgreSQL)
	Gist,
	/// GIN - Generalized Inverted Index (PostgreSQL)
	Gin,
	/// BRIN - Block Range Index (PostgreSQL)
	Brin,
	/// FULLTEXT - Full-text index (MySQL)
	FullText,
	/// SPATIAL - Spatial index (MySQL)
	Spatial,
}

impl IndexMethod {
	/// Get the SQL keyword for this index method
	pub fn as_str(&self) -> &'static str {
		match self {
			Self::BTree => "BTREE",
			Self::Hash => "HASH",
			Self::Gist => "GIST",
			Self::Gin => "GIN",
			Self::Brin => "BRIN",
			Self::FullText => "FULLTEXT",
			Self::Spatial => "SPATIAL",
		}
	}
}

impl CreateIndexStatement {
	/// Create a new CREATE INDEX statement
	pub fn new() -> Self {
		Self {
			name: None,
			table: None,
			columns: Vec::new(),
			unique: false,
			if_not_exists: false,
			r#where: None,
			using: None,
		}
	}

	/// Take the ownership of data in the current [`CreateIndexStatement`]
	pub fn take(&mut self) -> Self {
		Self {
			name: self.name.take(),
			table: self.table.take(),
			columns: std::mem::take(&mut self.columns),
			unique: self.unique,
			if_not_exists: self.if_not_exists,
			r#where: self.r#where.take(),
			using: self.using.take(),
		}
	}

	/// Set the index name
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::create_index()
	///     .name("idx_email");
	/// ```
	pub fn name<T>(&mut self, name: T) -> &mut Self
	where
		T: IntoIden,
	{
		self.name = Some(name.into_iden());
		self
	}

	/// Set the table
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::create_index()
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

	/// Add a column to the index
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::create_index()
	///     .name("idx_name_email")
	///     .table("users")
	///     .col("name")
	///     .col("email");
	/// ```
	pub fn col<C>(&mut self, column: C) -> &mut Self
	where
		C: IntoIden,
	{
		self.columns.push(IndexColumn {
			name: column.into_iden(),
			order: None,
		});
		self
	}

	/// Add a column with sort order
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	/// use reinhardt_query::types::Order;
	///
	/// let query = Query::create_index()
	///     .name("idx_created_at")
	///     .table("posts")
	///     .col_order("created_at", Order::Desc);
	/// ```
	pub fn col_order<C>(&mut self, column: C, order: Order) -> &mut Self
	where
		C: IntoIden,
	{
		self.columns.push(IndexColumn {
			name: column.into_iden(),
			order: Some(order),
		});
		self
	}

	/// Add multiple columns to the index
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::create_index()
	///     .name("idx_name_email")
	///     .table("users")
	///     .cols(vec!["name", "email"]);
	/// ```
	pub fn cols<I, C>(&mut self, columns: I) -> &mut Self
	where
		I: IntoIterator<Item = C>,
		C: IntoIden,
	{
		for col in columns {
			self.col(col);
		}
		self
	}

	/// Set UNIQUE attribute
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::create_index()
	///     .name("idx_email")
	///     .table("users")
	///     .col("email")
	///     .unique();
	/// ```
	pub fn unique(&mut self) -> &mut Self {
		self.unique = true;
		self
	}

	/// Add IF NOT EXISTS clause
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::create_index()
	///     .name("idx_email")
	///     .table("users")
	///     .col("email")
	///     .if_not_exists();
	/// ```
	pub fn if_not_exists(&mut self) -> &mut Self {
		self.if_not_exists = true;
		self
	}

	/// Add WHERE clause for partial index
	///
	/// Partial indexes are supported by PostgreSQL and SQLite.
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::create_index()
	///     .name("idx_active_users")
	///     .table("users")
	///     .col("email")
	///     .r#where(Expr::col("active").eq(true));
	/// ```
	pub fn r#where(&mut self, condition: SimpleExpr) -> &mut Self {
		self.r#where = Some(condition);
		self
	}

	/// Set index method using USING clause
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	/// use reinhardt_query::query::IndexMethod;
	///
	/// let query = Query::create_index()
	///     .name("idx_email")
	///     .table("users")
	///     .col("email")
	///     .using(IndexMethod::Hash);
	/// ```
	pub fn using(&mut self, method: IndexMethod) -> &mut Self {
		self.using = Some(method);
		self
	}
}

impl Default for CreateIndexStatement {
	fn default() -> Self {
		Self::new()
	}
}

impl QueryStatementBuilder for CreateIndexStatement {
	fn build_any(&self, query_builder: &dyn QueryBuilderTrait) -> (String, crate::value::Values) {
		// Downcast to concrete QueryBuilder type
		use std::any::Any;
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::PostgresQueryBuilder>()
		{
			return builder.build_create_index(self);
		}
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::MySqlQueryBuilder>()
		{
			return builder.build_create_index(self);
		}
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::SqliteQueryBuilder>()
		{
			return builder.build_create_index(self);
		}
		panic!("Unsupported query builder type");
	}
}

impl QueryStatementWriter for CreateIndexStatement {}
