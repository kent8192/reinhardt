//! Connection trait extension for reinhardt-query support
//!
//! This module provides extensions to the database connection trait
//! to support executing reinhardt-query statement objects directly.

use async_trait::async_trait;
use reinhardt_query::prelude::{
	DeleteStatement, InsertStatement, SelectStatement, UpdateStatement,
};

use crate::orm::query_types::{DbBackend, QueryStatement};

/// Universal row type supporting multiple database backends
///
/// This type supports PostgreSQL, MySQL, and SQLite through sqlx's unified interface.
/// The row type is automatically selected based on the database backend in use.
pub type Row = sqlx::any::AnyRow;

/// Result type for database operations
pub type DbResult<T> = Result<T, reinhardt_core::exception::Error>;

/// Connection trait extension for reinhardt-query support
#[async_trait]
pub trait ConnectionExt {
	/// Get database backend type
	fn backend(&self) -> DbBackend;

	/// Execute a reinhardt-query statement (INSERT, UPDATE, DELETE, DDL)
	async fn execute_statement(&self, stmt: &QueryStatement) -> DbResult<u64>;

	/// Query multiple rows with reinhardt-query SELECT statement
	async fn query_statement(&self, stmt: &SelectStatement) -> DbResult<Vec<Row>>;

	/// Query one row with reinhardt-query SELECT statement
	async fn query_one_statement(&self, stmt: &SelectStatement) -> DbResult<Row>;

	/// Execute a SELECT statement
	async fn execute_select(&self, stmt: &SelectStatement) -> DbResult<Vec<Row>> {
		self.query_statement(stmt).await
	}

	/// Execute an INSERT statement
	async fn execute_insert(&self, stmt: &InsertStatement) -> DbResult<u64>;

	/// Execute an UPDATE statement
	async fn execute_update(&self, stmt: &UpdateStatement) -> DbResult<u64>;

	/// Execute a DELETE statement
	async fn execute_delete(&self, stmt: &DeleteStatement) -> DbResult<u64>;
}

/// Default implementation helper for building SQL from statements
pub mod helpers {
	use super::*;
	use reinhardt_query::prelude::{
		MySqlQueryBuilder, PostgresQueryBuilder, QueryBuilder, SqliteQueryBuilder, Values,
	};

	/// Build SQL string and values from SelectStatement
	pub fn build_select(stmt: &SelectStatement, backend: DbBackend) -> (String, Values) {
		match backend {
			DbBackend::Postgres => PostgresQueryBuilder::new().build_select(stmt),
			DbBackend::Mysql => MySqlQueryBuilder::new().build_select(stmt),
			DbBackend::Sqlite => SqliteQueryBuilder::new().build_select(stmt),
		}
	}

	/// Build SQL string and values from InsertStatement
	pub fn build_insert(stmt: &InsertStatement, backend: DbBackend) -> (String, Values) {
		match backend {
			DbBackend::Postgres => PostgresQueryBuilder::new().build_insert(stmt),
			DbBackend::Mysql => MySqlQueryBuilder::new().build_insert(stmt),
			DbBackend::Sqlite => SqliteQueryBuilder::new().build_insert(stmt),
		}
	}

	/// Build SQL string and values from UpdateStatement
	pub fn build_update(stmt: &UpdateStatement, backend: DbBackend) -> (String, Values) {
		match backend {
			DbBackend::Postgres => PostgresQueryBuilder::new().build_update(stmt),
			DbBackend::Mysql => MySqlQueryBuilder::new().build_update(stmt),
			DbBackend::Sqlite => SqliteQueryBuilder::new().build_update(stmt),
		}
	}

	/// Build SQL string and values from DeleteStatement
	pub fn build_delete(stmt: &DeleteStatement, backend: DbBackend) -> (String, Values) {
		match backend {
			DbBackend::Postgres => PostgresQueryBuilder::new().build_delete(stmt),
			DbBackend::Mysql => MySqlQueryBuilder::new().build_delete(stmt),
			DbBackend::Sqlite => SqliteQueryBuilder::new().build_delete(stmt),
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use reinhardt_query::prelude::{Alias, Expr, Query};

	#[test]
	fn test_build_select_postgres() {
		use reinhardt_query::prelude::ExprTrait;

		let stmt = Query::select()
			.from(Alias::new("users"))
			.column(Alias::new("id"))
			.and_where(Expr::col(Alias::new("id")).eq(1))
			.to_owned();

		let (sql, values) = helpers::build_select(&stmt, DbBackend::Postgres);
		assert!(sql.contains("SELECT"));
		assert!(sql.contains("users"));
		assert_eq!(values.0.len(), 1);
	}

	#[test]
	fn test_build_select_mysql() {
		let stmt = Query::select()
			.from(Alias::new("users"))
			.column(Alias::new("id"))
			.to_owned();

		let (sql, _) = helpers::build_select(&stmt, DbBackend::Mysql);
		assert!(sql.contains("SELECT"));
		assert!(sql.contains("`users`")); // MySQL uses backticks
	}

	#[test]
	fn test_build_select_sqlite() {
		let stmt = Query::select()
			.from(Alias::new("users"))
			.column(Alias::new("id"))
			.to_owned();

		let (sql, _) = helpers::build_select(&stmt, DbBackend::Sqlite);
		assert!(sql.contains("SELECT"));
		assert!(sql.contains("\"users\"")); // SQLite uses double quotes
	}
}
