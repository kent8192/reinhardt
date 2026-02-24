//! DROP MATERIALIZED VIEW statement builder
//!
//! This module provides the `DropMaterializedViewStatement` type for building
//! SQL DROP MATERIALIZED VIEW queries.

use crate::backend::QueryBuilder;
use crate::types::{DynIden, IntoIden};

use crate::query::traits::{QueryBuilderTrait, QueryStatementBuilder, QueryStatementWriter};

/// DROP MATERIALIZED VIEW statement builder
///
/// This struct provides a fluent API for constructing DROP MATERIALIZED VIEW queries.
///
/// # Examples
///
/// ```rust,ignore
/// use reinhardt_query::prelude::*;
///
/// // Drop single materialized view
/// let query = Query::drop_materialized_view()
///     .name("my_mv");
///
/// // Drop with IF EXISTS
/// let query = Query::drop_materialized_view()
///     .name("my_mv")
///     .if_exists();
///
/// // Drop with CASCADE
/// let query = Query::drop_materialized_view()
///     .name("my_mv")
///     .cascade();
/// ```
#[derive(Debug, Clone)]
pub struct DropMaterializedViewStatement {
	pub(crate) names: Vec<DynIden>,
	pub(crate) if_exists: bool,
	pub(crate) cascade: bool,
	pub(crate) restrict: bool,
}

impl DropMaterializedViewStatement {
	/// Create a new DROP MATERIALIZED VIEW statement
	pub fn new() -> Self {
		Self {
			names: Vec::new(),
			if_exists: false,
			cascade: false,
			restrict: false,
		}
	}

	/// Take the ownership of data in the current statement
	pub fn take(&mut self) -> Self {
		Self {
			names: std::mem::take(&mut self.names),
			if_exists: self.if_exists,
			cascade: self.cascade,
			restrict: self.restrict,
		}
	}

	/// Set the materialized view name
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::drop_materialized_view()
	///     .name("my_mv");
	/// ```
	pub fn name<N>(&mut self, name: N) -> &mut Self
	where
		N: IntoIden,
	{
		self.names.push(name.into_iden());
		self
	}

	/// Add multiple materialized view names
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::drop_materialized_view()
	///     .names(["mv1", "mv2", "mv3"]);
	/// ```
	pub fn names<I, N>(&mut self, names: I) -> &mut Self
	where
		I: IntoIterator<Item = N>,
		N: IntoIden,
	{
		for name in names {
			self.names.push(name.into_iden());
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
	/// let query = Query::drop_materialized_view()
	///     .name("my_mv")
	///     .if_exists();
	/// ```
	pub fn if_exists(&mut self) -> &mut Self {
		self.if_exists = true;
		self
	}

	/// Add CASCADE clause
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::drop_materialized_view()
	///     .name("my_mv")
	///     .cascade();
	/// ```
	pub fn cascade(&mut self) -> &mut Self {
		self.cascade = true;
		self
	}

	/// Add RESTRICT clause
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::drop_materialized_view()
	///     .name("my_mv")
	///     .restrict();
	/// ```
	pub fn restrict(&mut self) -> &mut Self {
		self.restrict = true;
		self
	}
}

impl Default for DropMaterializedViewStatement {
	fn default() -> Self {
		Self::new()
	}
}

impl QueryStatementBuilder for DropMaterializedViewStatement {
	fn build_any(&self, query_builder: &dyn QueryBuilderTrait) -> (String, crate::value::Values) {
		// Downcast to concrete QueryBuilder type
		use std::any::Any;
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::PostgresQueryBuilder>()
		{
			return builder.build_drop_materialized_view(self);
		}
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::CockroachDBQueryBuilder>()
		{
			return builder.build_drop_materialized_view(self);
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

impl QueryStatementWriter for DropMaterializedViewStatement {}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::*;

	#[rstest]
	fn test_drop_materialized_view_basic() {
		let mut stmt = DropMaterializedViewStatement::new();
		stmt.name("my_mv");
		assert_eq!(stmt.names.len(), 1);
		assert_eq!(stmt.names[0].to_string(), "my_mv");
		assert!(!stmt.if_exists);
		assert!(!stmt.cascade);
		assert!(!stmt.restrict);
	}

	#[rstest]
	fn test_drop_materialized_view_if_exists() {
		let mut stmt = DropMaterializedViewStatement::new();
		stmt.name("my_mv").if_exists();
		assert!(stmt.if_exists);
	}

	#[rstest]
	fn test_drop_materialized_view_cascade() {
		let mut stmt = DropMaterializedViewStatement::new();
		stmt.name("my_mv").cascade();
		assert!(stmt.cascade);
	}

	#[rstest]
	fn test_drop_materialized_view_restrict() {
		let mut stmt = DropMaterializedViewStatement::new();
		stmt.name("my_mv").restrict();
		assert!(stmt.restrict);
	}

	#[rstest]
	fn test_drop_materialized_view_multiple_names() {
		let mut stmt = DropMaterializedViewStatement::new();
		stmt.names(["mv1", "mv2", "mv3"]);
		assert_eq!(stmt.names.len(), 3);
		assert_eq!(stmt.names[0].to_string(), "mv1");
		assert_eq!(stmt.names[1].to_string(), "mv2");
		assert_eq!(stmt.names[2].to_string(), "mv3");
	}

	#[rstest]
	fn test_drop_materialized_view_all_options() {
		let mut stmt = DropMaterializedViewStatement::new();
		stmt.name("my_mv").if_exists().cascade();
		assert_eq!(stmt.names.len(), 1);
		assert!(stmt.if_exists);
		assert!(stmt.cascade);
	}

	#[rstest]
	fn test_drop_materialized_view_default() {
		let stmt = DropMaterializedViewStatement::default();
		assert!(stmt.names.is_empty());
		assert!(!stmt.if_exists);
		assert!(!stmt.cascade);
		assert!(!stmt.restrict);
	}

	#[rstest]
	fn test_drop_materialized_view_take() {
		let mut stmt = DropMaterializedViewStatement::new();
		stmt.name("my_mv").if_exists();
		let taken = stmt.take();
		assert_eq!(taken.names.len(), 1);
		assert!(taken.if_exists);
		assert!(stmt.names.is_empty());
	}
}
