//! REFRESH MATERIALIZED VIEW statement builder
//!
//! This module provides the `RefreshMaterializedViewStatement` type for building
//! SQL REFRESH MATERIALIZED VIEW queries.

use crate::backend::QueryBuilder;
use crate::types::{DynIden, IntoIden};

use crate::query::traits::{QueryBuilderTrait, QueryStatementBuilder, QueryStatementWriter};

/// REFRESH MATERIALIZED VIEW statement builder
///
/// This struct provides a fluent API for constructing REFRESH MATERIALIZED VIEW queries.
///
/// # Examples
///
/// ```rust,ignore
/// use reinhardt_query::prelude::*;
///
/// // Basic refresh
/// let query = Query::refresh_materialized_view()
///     .name("my_mv");
///
/// // Refresh concurrently
/// let query = Query::refresh_materialized_view()
///     .name("my_mv")
///     .concurrently();
///
/// // Refresh with data
/// let query = Query::refresh_materialized_view()
///     .name("my_mv")
///     .with_data(true);
/// ```
#[derive(Debug, Clone)]
pub struct RefreshMaterializedViewStatement {
	pub(crate) name: Option<DynIden>,
	pub(crate) concurrently: bool,
	pub(crate) with_data: Option<bool>,
}

impl RefreshMaterializedViewStatement {
	/// Create a new REFRESH MATERIALIZED VIEW statement
	pub fn new() -> Self {
		Self {
			name: None,
			concurrently: false,
			with_data: None,
		}
	}

	/// Take the ownership of data in the current statement
	pub fn take(&mut self) -> Self {
		Self {
			name: self.name.take(),
			concurrently: self.concurrently,
			with_data: self.with_data,
		}
	}

	/// Set the materialized view name
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::refresh_materialized_view()
	///     .name("my_mv");
	/// ```
	pub fn name<N>(&mut self, name: N) -> &mut Self
	where
		N: IntoIden,
	{
		self.name = Some(name.into_iden());
		self
	}

	/// Add CONCURRENTLY clause
	///
	/// Allows the materialized view to be refreshed without blocking concurrent selects.
	/// Requires a unique index on the materialized view.
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::refresh_materialized_view()
	///     .name("my_mv")
	///     .concurrently();
	/// ```
	pub fn concurrently(&mut self) -> &mut Self {
		self.concurrently = true;
		self
	}

	/// Set WITH DATA or WITH NO DATA clause
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// // WITH DATA (default behavior)
	/// let query = Query::refresh_materialized_view()
	///     .name("my_mv")
	///     .with_data(true);
	///
	/// // WITH NO DATA (clears the materialized view)
	/// let query = Query::refresh_materialized_view()
	///     .name("my_mv")
	///     .with_data(false);
	/// ```
	pub fn with_data(&mut self, with_data: bool) -> &mut Self {
		self.with_data = Some(with_data);
		self
	}
}

impl Default for RefreshMaterializedViewStatement {
	fn default() -> Self {
		Self::new()
	}
}

impl QueryStatementBuilder for RefreshMaterializedViewStatement {
	fn build_any(&self, query_builder: &dyn QueryBuilderTrait) -> (String, crate::value::Values) {
		// Downcast to concrete QueryBuilder type
		use std::any::Any;
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::PostgresQueryBuilder>()
		{
			return builder.build_refresh_materialized_view(self);
		}
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::CockroachDBQueryBuilder>()
		{
			return builder.build_refresh_materialized_view(self);
		}
		if let Some(_builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::MySqlQueryBuilder>()
		{
			panic!("MySQL does not support materialized views");
		}
		if let Some(_builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::SqliteQueryBuilder>()
		{
			panic!("SQLite does not support materialized views");
		}
		panic!("Unsupported query builder type");
	}
}

impl QueryStatementWriter for RefreshMaterializedViewStatement {}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::*;

	#[rstest]
	fn test_refresh_materialized_view_basic() {
		let mut stmt = RefreshMaterializedViewStatement::new();
		stmt.name("my_mv");
		assert_eq!(stmt.name.as_ref().unwrap().to_string(), "my_mv");
		assert!(!stmt.concurrently);
		assert!(stmt.with_data.is_none());
	}

	#[rstest]
	fn test_refresh_materialized_view_concurrently() {
		let mut stmt = RefreshMaterializedViewStatement::new();
		stmt.name("my_mv").concurrently();
		assert!(stmt.concurrently);
	}

	#[rstest]
	fn test_refresh_materialized_view_with_data() {
		let mut stmt = RefreshMaterializedViewStatement::new();
		stmt.name("my_mv").with_data(true);
		assert_eq!(stmt.with_data, Some(true));
	}

	#[rstest]
	fn test_refresh_materialized_view_with_no_data() {
		let mut stmt = RefreshMaterializedViewStatement::new();
		stmt.name("my_mv").with_data(false);
		assert_eq!(stmt.with_data, Some(false));
	}

	#[rstest]
	fn test_refresh_materialized_view_all_options() {
		let mut stmt = RefreshMaterializedViewStatement::new();
		stmt.name("my_mv").concurrently().with_data(true);
		assert_eq!(stmt.name.as_ref().unwrap().to_string(), "my_mv");
		assert!(stmt.concurrently);
		assert_eq!(stmt.with_data, Some(true));
	}

	#[rstest]
	fn test_refresh_materialized_view_default() {
		let stmt = RefreshMaterializedViewStatement::default();
		assert!(stmt.name.is_none());
		assert!(!stmt.concurrently);
		assert!(stmt.with_data.is_none());
	}

	#[rstest]
	fn test_refresh_materialized_view_take() {
		let mut stmt = RefreshMaterializedViewStatement::new();
		stmt.name("my_mv").concurrently();
		let taken = stmt.take();
		assert_eq!(taken.name.as_ref().unwrap().to_string(), "my_mv");
		assert!(taken.concurrently);
		assert!(stmt.name.is_none());
	}
}
