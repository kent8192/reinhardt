//! DROP TRIGGER statement builder
//!
//! This module provides the `DropTriggerStatement` type for building SQL DROP TRIGGER queries.

use crate::{
	backend::QueryBuilder,
	types::{DynIden, IntoIden, IntoTableRef, TableRef},
};

use super::traits::{QueryBuilderTrait, QueryStatementBuilder, QueryStatementWriter};

/// DROP TRIGGER statement builder
///
/// This struct provides a fluent API for constructing DROP TRIGGER queries.
///
/// # Backend Support
///
/// - **PostgreSQL**: Full support (IF EXISTS, CASCADE/RESTRICT, ON table optional)
/// - **MySQL**: Basic support (IF EXISTS, ON table required)
/// - **SQLite**: Basic support (IF EXISTS, no CASCADE/RESTRICT)
///
/// # Examples
///
/// ```rust,ignore
/// use reinhardt_query::prelude::*;
///
/// // Basic DROP TRIGGER
/// let query = Query::drop_trigger()
///     .name("audit_insert")
///     .on_table("users");
///
/// // With IF EXISTS
/// let query = Query::drop_trigger()
///     .name("audit_insert")
///     .if_exists()
///     .on_table("users");
///
/// // PostgreSQL with CASCADE
/// let query = Query::drop_trigger()
///     .name("audit_insert")
///     .on_table("users")
///     .cascade();
/// ```
#[derive(Debug, Clone)]
pub struct DropTriggerStatement {
	pub(crate) name: Option<DynIden>,
	pub(crate) table: Option<TableRef>,
	pub(crate) if_exists: bool,
	pub(crate) cascade: bool,
	pub(crate) restrict: bool,
}

impl DropTriggerStatement {
	/// Create a new DROP TRIGGER statement
	pub fn new() -> Self {
		Self {
			name: None,
			table: None,
			if_exists: false,
			cascade: false,
			restrict: false,
		}
	}

	/// Take the ownership of data in the current [`DropTriggerStatement`]
	pub fn take(&mut self) -> Self {
		Self {
			name: self.name.take(),
			table: self.table.take(),
			if_exists: self.if_exists,
			cascade: self.cascade,
			restrict: self.restrict,
		}
	}

	/// Set the trigger name to drop
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::drop_trigger()
	///     .name("audit_insert");
	/// ```
	pub fn name<N>(&mut self, name: N) -> &mut Self
	where
		N: IntoIden,
	{
		self.name = Some(name.into_iden());
		self
	}

	/// Set the table on which the trigger is defined
	///
	/// Required for MySQL, optional for PostgreSQL and SQLite.
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::drop_trigger()
	///     .name("audit_insert")
	///     .on_table("users");
	/// ```
	pub fn on_table<T>(&mut self, table: T) -> &mut Self
	where
		T: IntoTableRef,
	{
		self.table = Some(table.into_table_ref());
		self
	}

	/// Add IF EXISTS clause
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::drop_trigger()
	///     .name("audit_insert")
	///     .if_exists();
	/// ```
	pub fn if_exists(&mut self) -> &mut Self {
		self.if_exists = true;
		self
	}

	/// Add CASCADE clause (PostgreSQL only)
	///
	/// Automatically drop dependent objects.
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::drop_trigger()
	///     .name("audit_insert")
	///     .on_table("users")
	///     .cascade();
	/// ```
	pub fn cascade(&mut self) -> &mut Self {
		self.cascade = true;
		self.restrict = false;
		self
	}

	/// Add RESTRICT clause (PostgreSQL only)
	///
	/// Refuse to drop if there are dependent objects (default behavior).
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::drop_trigger()
	///     .name("audit_insert")
	///     .on_table("users")
	///     .restrict();
	/// ```
	pub fn restrict(&mut self) -> &mut Self {
		self.restrict = true;
		self.cascade = false;
		self
	}
}

impl Default for DropTriggerStatement {
	fn default() -> Self {
		Self::new()
	}
}

impl QueryStatementBuilder for DropTriggerStatement {
	fn build_any(&self, query_builder: &dyn QueryBuilderTrait) -> (String, crate::value::Values) {
		// Downcast to concrete QueryBuilder type
		use std::any::Any;
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::PostgresQueryBuilder>()
		{
			return builder.build_drop_trigger(self);
		}
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::MySqlQueryBuilder>()
		{
			return builder.build_drop_trigger(self);
		}
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::SqliteQueryBuilder>()
		{
			return builder.build_drop_trigger(self);
		}
		panic!("Unsupported query builder type");
	}
}

impl QueryStatementWriter for DropTriggerStatement {}
