//! INSERT statement builder
//!
//! This module provides the `InsertStatement` type for building SQL INSERT queries.

use crate::{
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
	pub(crate) on_conflict: Option<super::on_conflict::OnConflict>,
}

impl InsertStatement {
	/// Create a new INSERT statement
	pub fn new() -> Self {
		Self {
			table: None,
			columns: Vec::new(),
			source: InsertSource::Values(Vec::new()),
			returning: None,
			on_conflict: None,
		}
	}

	/// Take the ownership of data in the current [`InsertStatement`]
	pub fn take(&mut self) -> Self {
		Self {
			table: self.table.take(),
			columns: std::mem::take(&mut self.columns),
			source: std::mem::replace(&mut self.source, InsertSource::Values(Vec::new())),
			returning: self.returning.take(),
			on_conflict: self.on_conflict.take(),
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
		self.source = InsertSource::Subquery(Box::new(select));
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
