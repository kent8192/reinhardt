//! UPDATE statement builder
//!
//! This module provides the `UpdateStatement` type for building SQL UPDATE queries.

use crate::{
	expr::{Condition, ConditionHolder, IntoCondition, SimpleExpr},
	types::{DynIden, IntoIden, IntoTableRef, TableRef},
	value::{IntoValue, Values},
};

use super::{
	returning::ReturningClause,
	traits::{QueryBuilderTrait, QueryStatementBuilder, QueryStatementWriter},
};

/// UPDATE statement builder
///
/// This struct provides a fluent API for constructing UPDATE queries.
///
/// # Examples
///
/// ```rust,ignore
/// use reinhardt_query::prelude::*;
///
/// let query = Query::update()
///     .table("users")
///     .value("active", false)
///     .and_where(Expr::col("last_login").lt("2020-01-01"));
/// ```
#[derive(Debug, Clone)]
pub struct UpdateStatement {
	pub(crate) table: Option<TableRef>,
	pub(crate) values: Vec<(DynIden, SimpleExpr)>,
	pub(crate) r#where: ConditionHolder,
	pub(crate) returning: Option<ReturningClause>,
}

impl UpdateStatement {
	/// Create a new UPDATE statement
	pub fn new() -> Self {
		Self {
			table: None,
			values: Vec::new(),
			r#where: ConditionHolder::new(),
			returning: None,
		}
	}

	/// Take the ownership of data in the current [`UpdateStatement`]
	pub fn take(&mut self) -> Self {
		Self {
			table: self.table.take(),
			values: std::mem::take(&mut self.values),
			r#where: std::mem::replace(&mut self.r#where, ConditionHolder::new()),
			returning: self.returning.take(),
		}
	}

	/// Set the table to update
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::update()
	///     .table("users");
	/// ```
	pub fn table<T>(&mut self, tbl: T) -> &mut Self
	where
		T: IntoTableRef,
	{
		self.table = Some(tbl.into_table_ref());
		self
	}

	/// Set a column value
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::update()
	///     .table("users")
	///     .value("active", false)
	///     .value("name", "Alice");
	/// ```
	pub fn value<C, V>(&mut self, col: C, val: V) -> &mut Self
	where
		C: IntoIden,
		V: IntoValue,
	{
		self.values
			.push((col.into_iden(), SimpleExpr::Value(val.into_value())));
		self
	}

	/// Set a column to an expression value
	///
	/// Use this method when the value is an expression (e.g., `Expr::current_timestamp()`,
	/// `Expr::col("other_column")`, `Expr::cust("NULL")`) rather than a plain value.
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::update()
	///     .table("users")
	///     .value_expr("updated_at", Expr::current_timestamp())
	///     .value_expr("status", Expr::cust("NULL"));
	/// ```
	pub fn value_expr<C, E>(&mut self, col: C, expr: E) -> &mut Self
	where
		C: IntoIden,
		E: Into<SimpleExpr>,
	{
		self.values.push((col.into_iden(), expr.into()));
		self
	}

	/// Set multiple column values
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::update()
	///     .table("users")
	///     .values([
	///         ("name", "Alice".into()),
	///         ("email", "alice@example.com".into()),
	///     ]);
	/// ```
	pub fn values<I, C, V>(&mut self, values: I) -> &mut Self
	where
		I: IntoIterator<Item = (C, V)>,
		C: IntoIden,
		V: IntoValue,
	{
		for (col, val) in values {
			self.value(col, val);
		}
		self
	}

	/// Add a condition to the WHERE clause
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::update()
	///     .table("users")
	///     .value("active", false)
	///     .and_where(Expr::col("last_login").lt("2020-01-01"));
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
	/// let query = Query::update()
	///     .table("users")
	///     .value("active", false)
	///     .and_where(Expr::col("id").eq(1))
	///     .returning(["id", "updated_at"]);
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
	/// let query = Query::update()
	///     .table("users")
	///     .value("active", false)
	///     .and_where(Expr::col("id").eq(1))
	///     .returning_all();
	/// ```
	pub fn returning_all(&mut self) -> &mut Self {
		self.returning = Some(ReturningClause::all());
		self
	}
}

impl Default for UpdateStatement {
	fn default() -> Self {
		Self::new()
	}
}

impl QueryStatementBuilder for UpdateStatement {
	fn build_any(&self, query_builder: &dyn QueryBuilderTrait) -> (String, Values) {
		use crate::backend::{
			MySqlQueryBuilder, PostgresQueryBuilder, QueryBuilder, SqliteQueryBuilder,
		};
		use std::any::Any;

		let any_builder = query_builder as &dyn Any;

		if let Some(pg) = any_builder.downcast_ref::<PostgresQueryBuilder>() {
			return pg.build_update(self);
		}

		if let Some(mysql) = any_builder.downcast_ref::<MySqlQueryBuilder>() {
			return mysql.build_update(self);
		}

		if let Some(sqlite) = any_builder.downcast_ref::<SqliteQueryBuilder>() {
			return sqlite.build_update(self);
		}

		panic!(
			"Unsupported query builder type. Use PostgresQueryBuilder, MySqlQueryBuilder, or SqliteQueryBuilder."
		);
	}
}

impl QueryStatementWriter for UpdateStatement {}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::expr::{Expr, ExprTrait};

	#[test]
	fn test_update_basic() {
		let mut query = UpdateStatement::new();
		query
			.table("users")
			.value("name", "Alice")
			.value("email", "alice@example.com");

		assert!(query.table.is_some());
		assert_eq!(query.values.len(), 2);
	}

	#[test]
	fn test_update_with_where() {
		let mut query = UpdateStatement::new();
		query
			.table("users")
			.value("active", false)
			.and_where(Expr::col("id").eq(1));

		assert!(query.table.is_some());
		assert_eq!(query.values.len(), 1);
		assert!(!query.r#where.is_empty());
	}

	#[test]
	fn test_update_multiple_values() {
		let mut query = UpdateStatement::new();
		query
			.table("users")
			.values([("name", "Alice"), ("email", "alice@example.com")]);

		assert_eq!(query.values.len(), 2);
	}

	#[test]
	fn test_update_returning() {
		let mut query = UpdateStatement::new();
		query
			.table("users")
			.value("active", false)
			.returning(["id", "updated_at"]);

		assert!(query.returning.is_some());
		let returning = query.returning.unwrap();
		assert!(!returning.is_all());
	}

	#[test]
	fn test_update_returning_all() {
		let mut query = UpdateStatement::new();
		query.table("users").value("active", false).returning_all();

		assert!(query.returning.is_some());
		let returning = query.returning.unwrap();
		assert!(returning.is_all());
	}

	#[test]
	fn test_update_take() {
		let mut query = UpdateStatement::new();
		query.table("users").value("active", false);

		let taken = query.take();
		assert!(taken.table.is_some());
		assert!(query.table.is_none());
	}
}
