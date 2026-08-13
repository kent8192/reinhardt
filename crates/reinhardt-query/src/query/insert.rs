//! INSERT statement builder
//!
//! This module provides the `InsertStatement` type for building SQL INSERT queries.

use crate::{
	expr::SimpleExpr,
	types::{DynIden, IntoIden, IntoTableRef, TableRef},
	value::{IntoValue, Value, Values},
};

use super::{
	returning::ReturningClause,
	select::SelectStatement,
	traits::{QueryBuilderTrait, QueryStatementBuilder, QueryStatementWriter},
};

/// Source of data for INSERT statement
///
/// This enum represents the data source for an INSERT statement.
/// It can be either explicit values (VALUES clause) or a subquery (SELECT statement).
#[derive(Debug, Clone)]
pub enum InsertSource {
	/// Explicit values for INSERT (VALUES clause)
	Values(Vec<Vec<Value>>),
	/// Subquery for INSERT FROM SELECT
	Subquery(Box<SelectStatement>),
}

impl Default for InsertSource {
	fn default() -> Self {
		Self::Values(Vec::new())
	}
}

/// INSERT statement builder
///
/// This struct provides a fluent API for constructing INSERT queries.
///
/// # Examples
///
/// ```rust,ignore
/// use reinhardt_query::prelude::*;
///
/// let query = Query::insert()
///     .into_table("users")
///     .columns(["name", "email"])
///     .values_panic(["Alice", "alice@example.com"])
///     .values_panic(["Bob", "bob@example.com"]);
/// ```
#[derive(Debug, Clone)]
pub struct InsertStatement {
	pub(crate) table: Option<TableRef>,
	pub(crate) columns: Vec<DynIden>,
	pub(crate) source: InsertSource,
	pub(crate) returning: Option<ReturningClause>,
	pub(crate) returning_exprs: Option<Vec<SimpleExpr>>,
	pub(crate) on_conflict: Option<super::on_conflict::OnConflict>,
	pub(crate) overriding_system_value: bool,
	pub(crate) default_values: bool,
}

impl InsertStatement {
	/// Create a new INSERT statement
	pub fn new() -> Self {
		Self {
			table: None,
			columns: Vec::new(),
			source: InsertSource::Values(Vec::new()),
			returning: None,
			returning_exprs: None,
			on_conflict: None,
			overriding_system_value: false,
			default_values: false,
		}
	}

	/// Take the ownership of data in the current [`InsertStatement`]
	pub fn take(&mut self) -> Self {
		Self {
			table: self.table.take(),
			columns: std::mem::take(&mut self.columns),
			source: std::mem::replace(&mut self.source, InsertSource::Values(Vec::new())),
			returning: self.returning.take(),
			returning_exprs: self.returning_exprs.take(),
			on_conflict: self.on_conflict.take(),
			overriding_system_value: std::mem::take(&mut self.overriding_system_value),
			default_values: std::mem::take(&mut self.default_values),
		}
	}

	/// Set the table to insert into
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::insert()
	///     .into_table("users");
	/// ```
	pub fn into_table<T>(&mut self, tbl: T) -> &mut Self
	where
		T: IntoTableRef,
	{
		self.table = Some(tbl.into_table_ref());
		self
	}

	/// Add a column to insert into
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::insert()
	///     .into_table("users")
	///     .column("name")
	///     .column("email");
	/// ```
	pub fn column<C>(&mut self, col: C) -> &mut Self
	where
		C: IntoIden,
	{
		self.default_values = false;
		self.columns.push(col.into_iden());
		self
	}

	/// Add multiple columns to insert into
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::insert()
	///     .into_table("users")
	///     .columns(["name", "email", "created_at"]);
	/// ```
	pub fn columns<I, C>(&mut self, cols: I) -> &mut Self
	where
		I: IntoIterator<Item = C>,
		C: IntoIden,
	{
		for col in cols {
			self.column(col);
		}
		self
	}

	/// Add values for the columns
	///
	/// Returns `Err` if the number of values doesn't match the number of columns.
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let result = Query::insert()
	///     .into_table("users")
	///     .columns(["name", "email"])
	///     .values(vec!["Alice".into(), "alice@example.com".into()]);
	/// ```
	pub fn values(&mut self, values: Vec<Value>) -> Result<&mut Self, String> {
		if !self.columns.is_empty() && values.len() != self.columns.len() {
			return Err(format!(
				"Number of values ({}) doesn't match number of columns ({})",
				values.len(),
				self.columns.len()
			));
		}
		self.default_values = false;
		match &mut self.source {
			InsertSource::Values(vals) => vals.push(values),
			InsertSource::Subquery(_) => {
				self.source = InsertSource::Values(vec![values]);
			}
		}
		Ok(self)
	}

	/// Add values for the columns (panics on mismatch)
	///
	/// # Panics
	///
	/// Panics if the number of values doesn't match the number of columns.
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::insert()
	///     .into_table("users")
	///     .columns(["name", "email"])
	///     .values_panic(["Alice", "alice@example.com"])
	///     .values_panic(["Bob", "bob@example.com"]);
	/// ```
	pub fn values_panic<I, V>(&mut self, values: I) -> &mut Self
	where
		I: IntoIterator<Item = V>,
		V: IntoValue,
	{
		let values: Vec<Value> = values.into_iter().map(|v| v.into_value()).collect();
		if !self.columns.is_empty() && values.len() != self.columns.len() {
			panic!(
				"Number of values ({}) doesn't match number of columns ({})",
				values.len(),
				self.columns.len()
			);
		}
		self.default_values = false;
		match &mut self.source {
			InsertSource::Values(vals) => vals.push(values),
			InsertSource::Subquery(_) => {
				self.source = InsertSource::Values(vec![values]);
			}
		}
		self
	}

	/// Add a RETURNING clause with multiple columns
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::insert()
	///     .into_table("users")
	///     .columns(["name", "email"])
	///     .values_panic(["Alice", "alice@example.com"])
	///     .returning(["id", "created_at"]);
	/// ```
	pub fn returning<I, C>(&mut self, cols: I) -> &mut Self
	where
		I: IntoIterator<Item = C>,
		C: crate::types::IntoColumnRef,
	{
		self.returning = Some(ReturningClause::columns(cols));
		self.returning_exprs = None;
		self
	}

	/// Add a RETURNING clause for a single column
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::insert()
	///     .into_table("users")
	///     .columns(["name"])
	///     .values_panic(["Alice"])
	///     .returning_col(Alias::new("id"));
	/// ```
	pub fn returning_col<C>(&mut self, col: C) -> &mut Self
	where
		C: crate::types::IntoColumnRef,
	{
		self.returning = Some(ReturningClause::columns([col]));
		self.returning_exprs = None;
		self
	}

	/// Add a RETURNING clause with expressions.
	///
	/// Expressions can alias physical database columns to caller-visible names.
	pub fn returning_exprs<I, E>(&mut self, expressions: I) -> &mut Self
	where
		I: IntoIterator<Item = E>,
		E: Into<SimpleExpr>,
	{
		self.returning = None;
		self.returning_exprs = Some(expressions.into_iter().map(Into::into).collect());
		self
	}

	/// Set ON CONFLICT clause for upsert behavior.
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	/// use reinhardt_query::query::OnConflict;
	///
	/// let query = Query::insert()
	///     .into_table("users")
	///     .columns(["id", "name"])
	///     .values_panic([1, "Alice"])
	///     .on_conflict(OnConflict::column("id").update_columns(["name"]));
	/// ```
	pub fn on_conflict(&mut self, on_conflict: super::on_conflict::OnConflict) -> &mut Self {
		self.on_conflict = Some(on_conflict);
		self
	}

	/// Add PostgreSQL's `OVERRIDING SYSTEM VALUE` clause.
	///
	/// This permits explicit values for columns declared as `GENERATED ALWAYS AS IDENTITY`.
	/// Non-PostgreSQL query builders ignore the clause.
	pub fn overriding_system_value(&mut self) -> &mut Self {
		self.overriding_system_value = true;
		self
	}

	/// Add a RETURNING * clause
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::insert()
	///     .into_table("users")
	///     .columns(["name", "email"])
	///     .values_panic(["Alice", "alice@example.com"])
	///     .returning_all();
	/// ```
	pub fn returning_all(&mut self) -> &mut Self {
		self.returning = Some(ReturningClause::all());
		self.returning_exprs = None;
		self
	}

	/// Use a subquery as the data source for INSERT
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let select = Query::select()
	///     .column("name")
	///     .column("email")
	///     .from("temp_users");
	///
	/// let query = Query::insert()
	///     .into_table("users")
	///     .columns(["name", "email"])
	///     .from_subquery(select);
	/// ```
	pub fn from_subquery(&mut self, select: SelectStatement) -> &mut Self {
		self.default_values = false;
		self.source = InsertSource::Subquery(Box::new(select));
		self
	}

	/// Insert one row using the table's column defaults.
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::insert()
	///     .into_table("settings")
	///     .default_values();
	/// ```
	pub fn default_values(&mut self) -> &mut Self {
		self.columns.clear();
		self.source = InsertSource::Values(Vec::new());
		self.default_values = true;
		self
	}

	/// Get the values if this is a VALUES source
	///
	/// Returns `None` if the source is a subquery.
	pub fn get_values(&self) -> Option<&Vec<Vec<Value>>> {
		match &self.source {
			InsertSource::Values(vals) => Some(vals),
			InsertSource::Subquery(_) => None,
		}
	}
}

impl Default for InsertStatement {
	fn default() -> Self {
		Self::new()
	}
}

impl QueryStatementBuilder for InsertStatement {
	fn build_any(&self, query_builder: &dyn QueryBuilderTrait) -> (String, Values) {
		use crate::backend::{
			MySqlQueryBuilder, PostgresQueryBuilder, QueryBuilder, SqliteQueryBuilder,
		};
		use std::any::Any;

		let any_builder = query_builder as &dyn Any;

		if let Some(pg) = any_builder.downcast_ref::<PostgresQueryBuilder>() {
			return pg.build_insert(self);
		}

		if let Some(mysql) = any_builder.downcast_ref::<MySqlQueryBuilder>() {
			return mysql.build_insert(self);
		}

		if let Some(sqlite) = any_builder.downcast_ref::<SqliteQueryBuilder>() {
			return sqlite.build_insert(self);
		}

		panic!(
			"Unsupported query builder type. Use PostgresQueryBuilder, MySqlQueryBuilder, or SqliteQueryBuilder."
		);
	}
}

impl QueryStatementWriter for InsertStatement {}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::Query;

	#[test]
	fn test_insert_basic() {
		let mut query = InsertStatement::new();
		query
			.into_table("users")
			.columns(["name", "email"])
			.values_panic(["Alice", "alice@example.com"]);

		assert!(query.table.is_some());
		assert_eq!(query.columns.len(), 2);
		let values = query.get_values().expect("should have values");
		assert_eq!(values.len(), 1);
		assert_eq!(values[0].len(), 2);
	}

	#[test]
	fn test_insert_multiple_rows() {
		let mut query = InsertStatement::new();
		query
			.into_table("users")
			.columns(["name", "email"])
			.values_panic(["Alice", "alice@example.com"])
			.values_panic(["Bob", "bob@example.com"]);

		let values = query.get_values().expect("should have values");
		assert_eq!(values.len(), 2);
	}

	#[test]
	fn test_insert_default_values_builds_for_each_backend() {
		use crate::backend::{MySqlQueryBuilder, PostgresQueryBuilder, SqliteQueryBuilder};

		// Arrange
		let mut query = InsertStatement::new();
		query.into_table("users").default_values();
		let query = query.take();

		// Act / Assert
		assert_eq!(
			query.build(PostgresQueryBuilder).0,
			"INSERT INTO \"users\" DEFAULT VALUES"
		);
		assert_eq!(
			query.build(SqliteQueryBuilder).0,
			"INSERT INTO \"users\" DEFAULT VALUES"
		);
		assert_eq!(
			query.build(MySqlQueryBuilder).0,
			"INSERT INTO `users` () VALUES ()"
		);
	}

	#[test]
	fn test_mysql_default_values_preserves_do_nothing_conflict_handling() {
		use crate::backend::MySqlQueryBuilder;
		use crate::query::OnConflict;

		// Arrange
		let mut query = InsertStatement::new();
		query
			.into_table("settings")
			.default_values()
			.on_conflict(OnConflict::column("key").do_nothing());

		// Act
		let sql = query.build(MySqlQueryBuilder).0;

		// Assert
		assert_eq!(
			sql,
			"INSERT INTO `settings` () VALUES () ON DUPLICATE KEY UPDATE `key` = `key`"
		);
	}

	#[test]
	fn test_insert_without_source_does_not_default_values() {
		use crate::backend::PostgresQueryBuilder;

		// Arrange
		let mut query = InsertStatement::new();
		query.into_table("users");

		// Act / Assert
		assert_eq!(query.build(PostgresQueryBuilder).0, "INSERT INTO \"users\"");
	}

	#[test]
	#[should_panic(expected = "Number of values")]
	fn test_insert_values_mismatch() {
		let mut query = InsertStatement::new();
		query
			.into_table("users")
			.columns(["name", "email"])
			.values_panic(["Alice"]); // Should panic: 1 value, 2 columns
	}

	#[test]
	fn test_insert_returning() {
		let mut query = InsertStatement::new();
		query
			.into_table("users")
			.columns(["name"])
			.values_panic(["Alice"])
			.returning(["id", "created_at"]);

		assert!(query.returning.is_some());
		let returning = query.returning.unwrap();
		assert!(!returning.is_all());
	}

	#[test]
	fn test_insert_returning_all() {
		let mut query = InsertStatement::new();
		query
			.into_table("users")
			.columns(["name"])
			.values_panic(["Alice"])
			.returning_all();

		assert!(query.returning.is_some());
		let returning = query.returning.unwrap();
		assert!(returning.is_all());
	}

	#[test]
	fn test_insert_take() {
		let mut query = InsertStatement::new();
		query
			.into_table("users")
			.columns(["name"])
			.values_panic(["Alice"]);

		let taken = query.take();
		assert!(taken.table.is_some());
		assert!(query.table.is_none());
	}

	#[test]
	fn test_insert_from_subquery() {
		let mut query = InsertStatement::new();
		let select = Query::select()
			.column("name")
			.column("email")
			.from("temp_users")
			.to_owned();

		query
			.into_table("users")
			.columns(["name", "email"])
			.from_subquery(select);

		assert!(query.table.is_some());
		assert_eq!(query.columns.len(), 2);
		assert!(
			query.get_values().is_none(),
			"should not have values when using subquery"
		);
	}
}
