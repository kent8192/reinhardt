//! DROP VIEW statement builder
//!
//! This module provides the `DropViewStatement` type for building SQL DROP VIEW queries.

use crate::{
	backend::QueryBuilder,
	types::{DynIden, IntoIden},
};

use super::traits::{QueryBuilderTrait, QueryStatementBuilder, QueryStatementWriter};

/// DROP VIEW statement builder
///
/// This struct provides a fluent API for constructing DROP VIEW queries.
///
/// # Examples
///
/// ```rust,ignore
/// use reinhardt_query::prelude::*;
///
/// let query = Query::drop_view()
///     .name("active_users")
///     .if_exists();
/// ```
#[derive(Debug, Clone)]
pub struct DropViewStatement {
	pub(crate) names: Vec<DynIden>,
	pub(crate) if_exists: bool,
	pub(crate) cascade: bool,
	pub(crate) restrict: bool,
	pub(crate) materialized: bool,
}

impl DropViewStatement {
	/// Create a new DROP VIEW statement
	pub fn new() -> Self {
		Self {
			names: Vec::new(),
			if_exists: false,
			cascade: false,
			restrict: false,
			materialized: false,
		}
	}

	/// Take the ownership of data in the current [`DropViewStatement`]
	pub fn take(&mut self) -> Self {
		Self {
			names: std::mem::take(&mut self.names),
			if_exists: self.if_exists,
			cascade: self.cascade,
			restrict: self.restrict,
			materialized: self.materialized,
		}
	}

	/// Set the view name to drop
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::drop_view()
	///     .name("active_users");
	/// ```
	pub fn name<N>(&mut self, name: N) -> &mut Self
	where
		N: IntoIden,
	{
		self.names.push(name.into_iden());
		self
	}

	/// Set multiple view names to drop
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::drop_view()
	///     .names(vec!["view1", "view2", "view3"]);
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
	/// let query = Query::drop_view()
	///     .name("active_users")
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
	/// let query = Query::drop_view()
	///     .name("active_users")
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
	/// let query = Query::drop_view()
	///     .name("active_users")
	///     .restrict();
	/// ```
	pub fn restrict(&mut self) -> &mut Self {
		self.restrict = true;
		self.cascade = false;
		self
	}

	/// Set MATERIALIZED flag (PostgreSQL only)
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::drop_view()
	///     .name("active_users")
	///     .materialized(true);
	/// ```
	pub fn materialized(&mut self, materialized: bool) -> &mut Self {
		self.materialized = materialized;
		self
	}
}

impl Default for DropViewStatement {
	fn default() -> Self {
		Self::new()
	}
}

impl QueryStatementBuilder for DropViewStatement {
	fn build_any(&self, query_builder: &dyn QueryBuilderTrait) -> (String, crate::value::Values) {
		// Downcast to concrete QueryBuilder type
		use std::any::Any;
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::PostgresQueryBuilder>()
		{
			return builder.build_drop_view(self);
		}
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::MySqlQueryBuilder>()
		{
			return builder.build_drop_view(self);
		}
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::SqliteQueryBuilder>()
		{
			return builder.build_drop_view(self);
		}
		panic!("Unsupported query builder type");
	}
}

impl QueryStatementWriter for DropViewStatement {}
