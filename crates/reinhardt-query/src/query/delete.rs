//! DELETE statement builder
//!
//! This module provides the `DeleteStatement` type for building SQL DELETE queries.

use crate::{
	expr::{Condition, ConditionHolder, IntoCondition},
	types::{IntoTableRef, TableRef},
	value::Values,
};

use super::{
	returning::ReturningClause,
	traits::{QueryBuilderTrait, QueryStatementBuilder, QueryStatementWriter},
};

/// DELETE statement builder
///
/// This struct provides a fluent API for constructing DELETE queries.
///
/// # Examples
///
/// ```rust,ignore
/// use reinhardt_query::prelude::*;
///
/// let query = Query::delete()
///     .from_table("users")
///     .and_where(Expr::col("active").eq(false));
/// ```
#[derive(Debug, Clone)]
pub struct DeleteStatement {
	pub(crate) table: Option<TableRef>,
	pub(crate) r#where: ConditionHolder,
	pub(crate) returning: Option<ReturningClause>,
}

impl DeleteStatement {
	/// Create a new DELETE statement
	pub fn new() -> Self {
		Self {
			table: None,
			r#where: ConditionHolder::new(),
			returning: None,
		}
	}

	/// Take the ownership of data in the current [`DeleteStatement`]
	pub fn take(&mut self) -> Self {
		Self {
			table: self.table.take(),
			r#where: std::mem::replace(&mut self.r#where, ConditionHolder::new()),
			returning: self.returning.take(),
		}
	}

	/// Set the table to delete from
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::delete()
	///     .from_table("users");
	/// ```
	pub fn from_table<T>(&mut self, tbl: T) -> &mut Self
	where
		T: IntoTableRef,
	{
		self.table = Some(tbl.into_table_ref());
		self
	}

	/// Add a condition to the WHERE clause
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::delete()
	///     .from_table("users")
	///     .and_where(Expr::col("deleted_at").is_not_null())
	///     .and_where(Expr::col("deleted_at").lt("2020-01-01"));
	/// ```
	pub fn and_where<C>(&mut self, condition: C) -> &mut Self
	where
		C: IntoCondition,
	{
		self.r#where.add_and(condition);
		self
	}

	/// Add a conditional WHERE clause.
	///
	/// This is an alias for [`and_where`](Self::and_where) that accepts a [`Condition`].
	pub fn cond_where(&mut self, condition: Condition) -> &mut Self {
		self.r#where.add_and(condition);
		self
	}

	/// Add a RETURNING clause
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::delete()
	///     .from_table("users")
	///     .and_where(Expr::col("id").eq(1))
	///     .returning(["id", "name"]);
	/// ```
	pub fn returning<I, C>(&mut self, cols: I) -> &mut Self
	where
		I: IntoIterator<Item = C>,
		C: crate::types::IntoColumnRef,
	{
		self.returning = Some(ReturningClause::columns(cols));
		self
	}

	/// Add a RETURNING * clause
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::delete()
	///     .from_table("users")
	///     .and_where(Expr::col("id").eq(1))
	///     .returning_all();
	/// ```
	pub fn returning_all(&mut self) -> &mut Self {
		self.returning = Some(ReturningClause::all());
		self
	}
}

impl Default for DeleteStatement {
	fn default() -> Self {
		Self::new()
	}
}

impl QueryStatementBuilder for DeleteStatement {
	fn build_any(&self, query_builder: &dyn QueryBuilderTrait) -> (String, Values) {
		use crate::backend::{
			MySqlQueryBuilder, PostgresQueryBuilder, QueryBuilder, SqliteQueryBuilder,
		};
		use std::any::Any;

		let any_builder = query_builder as &dyn Any;

		if let Some(pg) = any_builder.downcast_ref::<PostgresQueryBuilder>() {
			return pg.build_delete(self);
		}

		if let Some(mysql) = any_builder.downcast_ref::<MySqlQueryBuilder>() {
			return mysql.build_delete(self);
		}

		if let Some(sqlite) = any_builder.downcast_ref::<SqliteQueryBuilder>() {
			return sqlite.build_delete(self);
		}

		panic!(
			"Unsupported query builder type. Use PostgresQueryBuilder, MySqlQueryBuilder, or SqliteQueryBuilder."
		);
	}
}

impl QueryStatementWriter for DeleteStatement {}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::expr::{Expr, ExprTrait};

	#[test]
	fn test_delete_basic() {
		let mut query = DeleteStatement::new();
		query.from_table("users");

		assert!(query.table.is_some());
	}

	#[test]
	fn test_delete_with_where() {
		let mut query = DeleteStatement::new();
		query
			.from_table("users")
			.and_where(Expr::col("active").eq(false));

		assert!(query.table.is_some());
		assert!(!query.r#where.is_empty());
	}

	#[test]
	fn test_delete_multiple_conditions() {
		let mut query = DeleteStatement::new();
		query
			.from_table("users")
			.and_where(Expr::col("active").eq(false))
			.and_where(Expr::col("deleted_at").is_not_null());

		assert!(!query.r#where.is_empty());
	}

	#[test]
	fn test_delete_returning() {
		let mut query = DeleteStatement::new();
		query
			.from_table("users")
			.and_where(Expr::col("id").eq(1))
			.returning(["id", "name"]);

		assert!(query.returning.is_some());
		let returning = query.returning.unwrap();
		assert!(!returning.is_all());
	}

	#[test]
	fn test_delete_returning_all() {
		let mut query = DeleteStatement::new();
		query
			.from_table("users")
			.and_where(Expr::col("id").eq(1))
			.returning_all();

		assert!(query.returning.is_some());
		let returning = query.returning.unwrap();
		assert!(returning.is_all());
	}

	#[test]
	fn test_delete_take() {
		let mut query = DeleteStatement::new();
		query.from_table("users");

		let taken = query.take();
		assert!(taken.table.is_some());
		assert!(query.table.is_none());
	}
}
