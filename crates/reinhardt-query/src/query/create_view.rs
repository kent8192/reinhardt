//! CREATE VIEW statement builder
//!
//! This module provides the `CreateViewStatement` type for building SQL CREATE VIEW queries.

use crate::{
	backend::QueryBuilder,
	query::SelectStatement,
	types::{DynIden, IntoIden},
};

use super::traits::{QueryBuilderTrait, QueryStatementBuilder, QueryStatementWriter};

/// CREATE VIEW statement builder
///
/// This struct provides a fluent API for constructing CREATE VIEW queries.
///
/// # Examples
///
/// ```rust,ignore
/// use reinhardt_query::prelude::*;
///
/// let select = Query::select()
///     .column(Expr::col("name"))
///     .column(Expr::col("email"))
///     .from("users")
///     .and_where(Expr::col("active").eq(true));
///
/// let query = Query::create_view()
///     .name("active_users")
///     .as_select(select)
///     .if_not_exists();
/// ```
#[derive(Debug, Clone)]
pub struct CreateViewStatement {
	pub(crate) name: Option<DynIden>,
	pub(crate) select: Option<SelectStatement>,
	pub(crate) if_not_exists: bool,
	pub(crate) or_replace: bool,
	pub(crate) columns: Vec<DynIden>,
	pub(crate) materialized: bool,
}

impl CreateViewStatement {
	/// Create a new CREATE VIEW statement
	pub fn new() -> Self {
		Self {
			name: None,
			select: None,
			if_not_exists: false,
			or_replace: false,
			columns: Vec::new(),
			materialized: false,
		}
	}

	/// Take the ownership of data in the current [`CreateViewStatement`]
	pub fn take(&mut self) -> Self {
		Self {
			name: self.name.take(),
			select: self.select.take(),
			if_not_exists: self.if_not_exists,
			or_replace: self.or_replace,
			columns: std::mem::take(&mut self.columns),
			materialized: self.materialized,
		}
	}

	/// Set the view name
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::create_view()
	///     .name("active_users");
	/// ```
	pub fn name<N>(&mut self, name: N) -> &mut Self
	where
		N: IntoIden,
	{
		self.name = Some(name.into_iden());
		self
	}

	/// Set the SELECT statement for the view
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let select = Query::select()
	///     .column(Expr::col("name"))
	///     .from("users");
	///
	/// let query = Query::create_view()
	///     .name("user_names")
	///     .as_select(select);
	/// ```
	pub fn as_select(&mut self, select: SelectStatement) -> &mut Self {
		self.select = Some(select);
		self
	}

	/// Add IF NOT EXISTS clause
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::create_view()
	///     .name("active_users")
	///     .if_not_exists();
	/// ```
	pub fn if_not_exists(&mut self) -> &mut Self {
		self.if_not_exists = true;
		self
	}

	/// Add OR REPLACE clause
	///
	/// Note: Cannot be used with IF NOT EXISTS
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::create_view()
	///     .name("active_users")
	///     .or_replace();
	/// ```
	pub fn or_replace(&mut self) -> &mut Self {
		self.or_replace = true;
		self
	}

	/// Set column names for the view
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::create_view()
	///     .name("active_users")
	///     .columns(["user_name", "user_email"]);
	/// ```
	pub fn columns<I, C>(&mut self, cols: I) -> &mut Self
	where
		I: IntoIterator<Item = C>,
		C: IntoIden,
	{
		for col in cols {
			self.columns.push(col.into_iden());
		}
		self
	}

	/// Set MATERIALIZED flag (PostgreSQL only)
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::create_view()
	///     .name("active_users")
	///     .materialized(true);
	/// ```
	pub fn materialized(&mut self, materialized: bool) -> &mut Self {
		self.materialized = materialized;
		self
	}
}

impl Default for CreateViewStatement {
	fn default() -> Self {
		Self::new()
	}
}

impl QueryStatementBuilder for CreateViewStatement {
	fn build_any(&self, query_builder: &dyn QueryBuilderTrait) -> (String, crate::value::Values) {
		// Downcast to concrete QueryBuilder type
		use std::any::Any;
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::PostgresQueryBuilder>()
		{
			return builder.build_create_view(self);
		}
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::MySqlQueryBuilder>()
		{
			return builder.build_create_view(self);
		}
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::SqliteQueryBuilder>()
		{
			return builder.build_create_view(self);
		}
		panic!("Unsupported query builder type");
	}
}

impl QueryStatementWriter for CreateViewStatement {}
