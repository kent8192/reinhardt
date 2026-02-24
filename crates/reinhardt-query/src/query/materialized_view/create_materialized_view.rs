//! CREATE MATERIALIZED VIEW statement builder
//!
//! This module provides the `CreateMaterializedViewStatement` type for building
//! SQL CREATE MATERIALIZED VIEW queries.
//!
//! Note: Materialized views are PostgreSQL and CockroachDB specific features.

use crate::{
	backend::QueryBuilder,
	query::SelectStatement,
	types::{IntoIden, MaterializedViewDef},
};

use crate::query::traits::{QueryBuilderTrait, QueryStatementBuilder, QueryStatementWriter};

/// CREATE MATERIALIZED VIEW statement builder
///
/// This struct provides a fluent API for constructing CREATE MATERIALIZED VIEW queries.
///
/// # Examples
///
/// ```rust,ignore
/// use reinhardt_query::prelude::*;
///
/// let select = Query::select()
///     .column(Expr::col("id"))
///     .column(Expr::col("name"))
///     .from("users")
///     .and_where(Expr::col("active").eq(true));
///
/// let query = Query::create_materialized_view()
///     .name("active_users_mv")
///     .as_select(select)
///     .with_data(true);
/// ```
#[derive(Debug, Clone)]
pub struct CreateMaterializedViewStatement {
	pub(crate) def: MaterializedViewDef,
	pub(crate) select: Option<SelectStatement>,
}

impl CreateMaterializedViewStatement {
	/// Create a new CREATE MATERIALIZED VIEW statement
	pub fn new() -> Self {
		Self {
			def: MaterializedViewDef::new(""),
			select: None,
		}
	}

	/// Take the ownership of data in the current statement
	pub fn take(&mut self) -> Self {
		Self {
			def: self.def.clone(),
			select: self.select.take(),
		}
	}

	/// Set the materialized view name
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::create_materialized_view()
	///     .name("active_users_mv");
	/// ```
	pub fn name<N>(&mut self, name: N) -> &mut Self
	where
		N: IntoIden,
	{
		self.def.name = name.into_iden();
		self
	}

	/// Set the SELECT statement for the materialized view
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let select = Query::select()
	///     .column(Expr::col("id"))
	///     .from("users");
	///
	/// let query = Query::create_materialized_view()
	///     .name("users_mv")
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
	/// let query = Query::create_materialized_view()
	///     .name("users_mv")
	///     .if_not_exists();
	/// ```
	pub fn if_not_exists(&mut self) -> &mut Self {
		self.def.if_not_exists = true;
		self
	}

	/// Set column names for the materialized view
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::create_materialized_view()
	///     .name("users_mv")
	///     .columns(["user_id", "user_name"]);
	/// ```
	pub fn columns<I, C>(&mut self, cols: I) -> &mut Self
	where
		I: IntoIterator<Item = C>,
		C: IntoIden,
	{
		for col in cols {
			self.def.columns.push(col.into_iden());
		}
		self
	}

	/// Set TABLESPACE for the materialized view
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::create_materialized_view()
	///     .name("users_mv")
	///     .tablespace("pg_default");
	/// ```
	pub fn tablespace<T>(&mut self, tablespace: T) -> &mut Self
	where
		T: IntoIden,
	{
		self.def.tablespace = Some(tablespace.into_iden());
		self
	}

	/// Set WITH DATA or WITH NO DATA clause
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// // WITH DATA (populate immediately)
	/// let query = Query::create_materialized_view()
	///     .name("users_mv")
	///     .with_data(true);
	///
	/// // WITH NO DATA (don't populate)
	/// let query = Query::create_materialized_view()
	///     .name("users_mv")
	///     .with_data(false);
	/// ```
	pub fn with_data(&mut self, with_data: bool) -> &mut Self {
		self.def.with_data = Some(with_data);
		self
	}
}

impl Default for CreateMaterializedViewStatement {
	fn default() -> Self {
		Self::new()
	}
}

impl QueryStatementBuilder for CreateMaterializedViewStatement {
	fn build_any(&self, query_builder: &dyn QueryBuilderTrait) -> (String, crate::value::Values) {
		// Downcast to concrete QueryBuilder type
		use std::any::Any;
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::PostgresQueryBuilder>()
		{
			return builder.build_create_materialized_view(self);
		}
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::CockroachDBQueryBuilder>()
		{
			return builder.build_create_materialized_view(self);
		}
		if let Some(_builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::MySqlQueryBuilder>()
		{
			panic!(
				"MySQL does not support materialized views. Use regular tables with triggers or scheduled queries instead."
			);
		}
		if let Some(_builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::SqliteQueryBuilder>()
		{
			panic!(
				"SQLite does not support materialized views. Use regular tables with triggers or application-level caching instead."
			);
		}
		panic!("Unsupported query builder type");
	}
}

impl QueryStatementWriter for CreateMaterializedViewStatement {}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::*;

	#[rstest]
	fn test_create_materialized_view_basic() {
		let mut stmt = CreateMaterializedViewStatement::new();
		stmt.name("my_mv");
		assert_eq!(stmt.def.name.to_string(), "my_mv");
		assert!(!stmt.def.if_not_exists);
		assert!(stmt.select.is_none());
	}

	#[rstest]
	fn test_create_materialized_view_if_not_exists() {
		let mut stmt = CreateMaterializedViewStatement::new();
		stmt.name("my_mv").if_not_exists();
		assert_eq!(stmt.def.name.to_string(), "my_mv");
		assert!(stmt.def.if_not_exists);
	}

	#[rstest]
	fn test_create_materialized_view_with_data() {
		let mut stmt = CreateMaterializedViewStatement::new();
		stmt.name("my_mv").with_data(true);
		assert_eq!(stmt.def.with_data, Some(true));
	}

	#[rstest]
	fn test_create_materialized_view_with_no_data() {
		let mut stmt = CreateMaterializedViewStatement::new();
		stmt.name("my_mv").with_data(false);
		assert_eq!(stmt.def.with_data, Some(false));
	}

	#[rstest]
	fn test_create_materialized_view_columns() {
		let mut stmt = CreateMaterializedViewStatement::new();
		stmt.name("my_mv").columns(["id", "name", "email"]);
		assert_eq!(stmt.def.columns.len(), 3);
		assert_eq!(stmt.def.columns[0].to_string(), "id");
		assert_eq!(stmt.def.columns[1].to_string(), "name");
		assert_eq!(stmt.def.columns[2].to_string(), "email");
	}

	#[rstest]
	fn test_create_materialized_view_tablespace() {
		let mut stmt = CreateMaterializedViewStatement::new();
		stmt.name("my_mv").tablespace("pg_default");
		assert_eq!(
			stmt.def.tablespace.as_ref().unwrap().to_string(),
			"pg_default"
		);
	}

	#[rstest]
	fn test_create_materialized_view_as_select() {
		let mut stmt = CreateMaterializedViewStatement::new();
		let select = SelectStatement::new();
		stmt.name("my_mv").as_select(select);
		assert!(stmt.select.is_some());
	}

	#[rstest]
	fn test_create_materialized_view_all_options() {
		let mut stmt = CreateMaterializedViewStatement::new();
		let select = SelectStatement::new();
		stmt.name("my_mv")
			.if_not_exists()
			.columns(["id", "name"])
			.tablespace("pg_default")
			.with_data(true)
			.as_select(select);

		assert_eq!(stmt.def.name.to_string(), "my_mv");
		assert!(stmt.def.if_not_exists);
		assert_eq!(stmt.def.columns.len(), 2);
		assert_eq!(
			stmt.def.tablespace.as_ref().unwrap().to_string(),
			"pg_default"
		);
		assert_eq!(stmt.def.with_data, Some(true));
		assert!(stmt.select.is_some());
	}

	#[rstest]
	fn test_create_materialized_view_default() {
		let stmt = CreateMaterializedViewStatement::default();
		assert_eq!(stmt.def.name.to_string(), "");
		assert!(!stmt.def.if_not_exists);
		assert!(stmt.select.is_none());
	}

	#[rstest]
	fn test_create_materialized_view_take() {
		let mut stmt = CreateMaterializedViewStatement::new();
		stmt.name("my_mv").if_not_exists();
		let taken = stmt.take();
		assert_eq!(taken.def.name.to_string(), "my_mv");
		assert!(taken.def.if_not_exists);
	}

	#[rstest]
	fn test_create_materialized_view_chaining() {
		let mut stmt = CreateMaterializedViewStatement::new();
		stmt.name("my_mv")
			.if_not_exists()
			.columns(["id"])
			.with_data(true);
		assert_eq!(stmt.def.name.to_string(), "my_mv");
		assert!(stmt.def.if_not_exists);
		assert_eq!(stmt.def.columns.len(), 1);
		assert_eq!(stmt.def.with_data, Some(true));
	}

	#[rstest]
	fn test_create_materialized_view_multiple_columns() {
		let mut stmt = CreateMaterializedViewStatement::new();
		stmt.name("my_mv").columns(["id", "name"]);
		stmt.columns(["email"]);
		assert_eq!(stmt.def.columns.len(), 3);
		assert_eq!(stmt.def.columns[0].to_string(), "id");
		assert_eq!(stmt.def.columns[1].to_string(), "name");
		assert_eq!(stmt.def.columns[2].to_string(), "email");
	}
}
