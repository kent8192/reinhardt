//! # Async Query API
//!
//! Async/await support for database queries.
//!
//! This module is inspired by SQLAlchemy's async support
//! Copyright 2005-2025 SQLAlchemy authors and contributors
//! Licensed under MIT License. See THIRD-PARTY-NOTICES for details.

use super::engine::Engine;
use super::types::DatabaseDialect;
use crate::orm::Model;
use crate::orm::expressions::Q;
use crate::orm::query_execution::QueryCompiler;
use reinhardt_query::prelude::{
	MySqlQueryBuilder, PostgresQueryBuilder, QueryStatementBuilder, SqliteQueryBuilder,
};
use std::marker::PhantomData;

/// Async query builder
pub struct AsyncQuery<T: Model> {
	engine: Engine,
	compiler: QueryCompiler,
	table: String,
	columns: Vec<String>,
	where_clauses: Vec<Q>,
	order_by: Vec<String>,
	limit: Option<usize>,
	offset: Option<usize>,
	_phantom: PhantomData<T>,
}

impl<T: Model> AsyncQuery<T> {
	/// Create a new async query
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_db::orm::{Engine, Model};
	/// use reinhardt_db::orm::async_query::AsyncQuery;
	/// use reinhardt_db::orm::query_execution::QueryCompiler;
	/// use reinhardt_db::orm::types::DatabaseDialect;
	/// use serde::{Serialize, Deserialize};
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// #[derive(Debug, Clone, Serialize, Deserialize)]
	/// struct User {
	///     id: Option<i32>,
	///     name: String,
	/// }
	///
	/// # #[derive(Clone)]
	/// # struct UserFields;
	/// # impl reinhardt_db::orm::model::FieldSelector for UserFields {
	/// #     fn with_alias(self, _alias: &str) -> Self {
	/// #         self
	/// #     }
	/// # }
	/// #
	/// impl Model for User {
	///     type PrimaryKey = i32;
	/// #     type Fields = UserFields;
	///     fn table_name() -> &'static str {
	///         "users"
	///     }
	/// #     fn new_fields() -> Self::Fields {
	/// #         UserFields
	/// #     }
	///     fn primary_key(&self) -> Option<Self::PrimaryKey> {
	///         self.id
	///     }
	///     fn set_primary_key(&mut self, value: Self::PrimaryKey) {
	///         self.id = Some(value);
	///     }
	/// }
	///
	/// let engine = Engine::new("sqlite::memory:").await?;
	/// let compiler = QueryCompiler::new(DatabaseDialect::SQLite);
	/// let query: AsyncQuery<User> = AsyncQuery::new(engine, compiler);
	/// // Query is ready to execute
	/// assert_eq!(User::table_name(), "users");
	/// # Ok(())
	/// # }
	/// ```
	pub fn new(engine: Engine, compiler: QueryCompiler) -> Self {
		Self {
			engine,
			compiler,
			table: T::table_name().to_string(),
			columns: Vec::new(),
			where_clauses: Vec::new(),
			order_by: Vec::new(),
			limit: None,
			offset: None,
			_phantom: PhantomData,
		}
	}
	/// Select specific columns
	///
	pub fn select(mut self, columns: Vec<impl Into<String>>) -> Self {
		self.columns = columns.into_iter().map(|c| c.into()).collect();
		self
	}
	/// Add WHERE clause
	///
	pub fn filter(mut self, condition: Q) -> Self {
		self.where_clauses.push(condition);
		self
	}
	/// Add ORDER BY clause
	///
	pub fn order_by(mut self, column: impl Into<String>) -> Self {
		self.order_by.push(column.into());
		self
	}
	/// Set LIMIT
	///
	pub fn limit(mut self, limit: usize) -> Self {
		self.limit = Some(limit);
		self
	}
	/// Set OFFSET
	///
	pub fn offset(mut self, offset: usize) -> Self {
		self.offset = Some(offset);
		self
	}
	/// Compile the query to SQL
	///
	pub fn to_sql(&self) -> String {
		let cols: Vec<&str> = self.columns.iter().map(|s| s.as_str()).collect();

		let combined_where = if self.where_clauses.is_empty() {
			None
		} else {
			Some(
				self.where_clauses
					.iter()
					.fold(self.where_clauses[0].clone(), |acc, q| acc.and(q.clone())),
			)
		};

		let order_refs: Vec<&str> = self.order_by.iter().map(|s| s.as_str()).collect();

		let stmt = self.compiler.compile_select::<T>(
			&self.table,
			&cols,
			combined_where.as_ref(),
			&order_refs,
			self.limit,
			self.offset,
		);

		// Convert statement to SQL string based on dialect
		match self.compiler.dialect() {
			DatabaseDialect::PostgreSQL => stmt.to_string(PostgresQueryBuilder),
			DatabaseDialect::MySQL => stmt.to_string(MySqlQueryBuilder),
			DatabaseDialect::SQLite => stmt.to_string(SqliteQueryBuilder),
			// MSSQL: PostgreSQL builder used as fallback since reinhardt-query lacks MssqlQueryBuilder.
			// Some PostgreSQL-specific syntax may not be compatible with MSSQL.
			DatabaseDialect::MSSQL => stmt.to_string(PostgresQueryBuilder),
		}
	}
	/// Execute query and fetch all results
	///
	pub async fn all(&self) -> Result<Vec<sqlx::any::AnyRow>, sqlx::Error> {
		let sql = self.to_sql();
		self.engine.fetch_all(&sql).await
	}
	/// Execute query and fetch first result
	///
	pub async fn first(&self) -> Result<Option<sqlx::any::AnyRow>, sqlx::Error> {
		let sql = self.to_sql();
		self.engine.fetch_optional(&sql).await
	}
	/// Execute query and fetch one result (error if not exactly one)
	///
	pub async fn one(&self) -> Result<sqlx::any::AnyRow, sqlx::Error> {
		let sql = self.to_sql();
		self.engine.fetch_one(&sql).await
	}
	/// Count the number of rows
	///
	pub async fn count(&self) -> Result<i64, sqlx::Error> {
		let mut count_query = self.clone();
		count_query.columns = vec!["COUNT(*)".to_string()];
		count_query.limit = None;
		count_query.offset = None;

		let sql = count_query.to_sql();
		let row = self.engine.fetch_one(&sql).await?;

		// Extract count value from row
		use sqlx::Row;
		let count: i64 = row.try_get(0).unwrap_or(0);
		Ok(count)
	}
	/// Check if any rows exist
	///
	pub async fn exists(&self) -> Result<bool, sqlx::Error> {
		let count = self.count().await?;
		Ok(count > 0)
	}
}

// Implement Clone for AsyncQuery
impl<T: Model> Clone for AsyncQuery<T> {
	fn clone(&self) -> Self {
		Self {
			engine: self.engine.clone_ref(),
			compiler: self.compiler.clone(),
			table: self.table.clone(),
			columns: self.columns.clone(),
			where_clauses: self.where_clauses.clone(),
			order_by: self.order_by.clone(),
			limit: self.limit,
			offset: self.offset,
			_phantom: PhantomData,
		}
	}
}

/// Async session for executing queries
pub struct AsyncSession {
	engine: Engine,
	compiler: QueryCompiler,
}

impl AsyncSession {
	/// Create a new async session
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_db::orm::Engine;
	/// use reinhardt_db::orm::async_query::AsyncSession;
	/// use reinhardt_db::orm::query_execution::QueryCompiler;
	/// use reinhardt_db::orm::types::DatabaseDialect;
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let engine = Engine::new("sqlite::memory:").await?;
	/// let compiler = QueryCompiler::new(DatabaseDialect::SQLite);
	/// let session = AsyncSession::new(engine, compiler);
	/// // Session is ready to execute queries
	/// # Ok(())
	/// # }
	/// ```
	pub fn new(engine: Engine, compiler: QueryCompiler) -> Self {
		Self { engine, compiler }
	}
	/// Start a query for a model
	///
	pub fn query<T: Model>(&self) -> AsyncQuery<T> {
		AsyncQuery::new(self.engine.clone_ref(), self.compiler.clone())
	}
	/// Execute raw SQL
	///
	pub async fn execute(&self, sql: &str) -> Result<u64, sqlx::Error> {
		self.engine.execute(sql).await
	}
	/// Begin a transaction
	///
	pub async fn begin(&self) -> Result<sqlx::Transaction<'_, sqlx::Any>, sqlx::Error> {
		self.engine.begin().await
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use reinhardt_core::validators::TableName;
	use serde::{Deserialize, Serialize};

	// Allow dead_code: test model struct for async query tests
	#[allow(dead_code)]
	#[derive(Debug, Clone, Serialize, Deserialize)]
	struct TestModel {
		id: Option<i64>,
		name: String,
	}

	#[derive(Clone)]
	struct TestModelFields;
	impl crate::orm::model::FieldSelector for TestModelFields {
		fn with_alias(self, _alias: &str) -> Self {
			self
		}
	}

	// Allow dead_code: test constant for async query tests
	#[allow(dead_code)]
	const TEST_MODEL_TABLE: TableName = TableName::new_const("test_model");

	impl Model for TestModel {
		type PrimaryKey = i64;
		type Fields = TestModelFields;

		fn table_name() -> &'static str {
			TEST_MODEL_TABLE.as_str()
		}

		fn new_fields() -> Self::Fields {
			TestModelFields
		}

		fn primary_key(&self) -> Option<Self::PrimaryKey> {
			self.id
		}

		fn set_primary_key(&mut self, value: Self::PrimaryKey) {
			self.id = Some(value);
		}
	}

	// ========================================================================
	// SQLite Tests
	// ========================================================================

	#[cfg(feature = "sqlite")]
	mod sqlite_tests {
		use super::*;
		use serial_test::serial;
		use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};
		use std::time::Duration;

		async fn create_sqlite_pool() -> Result<SqlitePool, sqlx::Error> {
			SqlitePoolOptions::new()
				.min_connections(1)
				.max_connections(5)
				.acquire_timeout(Duration::from_secs(10))
				.connect("sqlite::memory:")
				.await
		}

		#[tokio::test]
		#[serial(async_query_sqlite)]
		async fn test_sqlite_async_query_builder() {
			let pool = create_sqlite_pool()
				.await
				.expect("Failed to create SQLite pool");

			// Test basic SQL generation with QueryCompiler
			let compiler = QueryCompiler::new(DatabaseDialect::SQLite);
			let stmt = compiler.compile_select::<TestModel>(
				TestModel::table_name(),
				&[],
				Some(&Q::new("age", ">=", "18")),
				&["name"],
				Some(10),
				None,
			);

			let sql = stmt.to_string(reinhardt_query::prelude::SqliteQueryBuilder);
			assert!(sql.contains("test_model"));
			assert!(sql.contains("ORDER BY"));

			pool.close().await;
		}

		#[tokio::test]
		#[serial(async_query_sqlite)]
		async fn test_sqlite_async_query_execution() {
			let pool = create_sqlite_pool()
				.await
				.expect("Failed to create SQLite pool");

			sqlx::query("CREATE TABLE test_models (id INTEGER PRIMARY KEY, name TEXT)")
				.execute(&pool)
				.await
				.expect("Failed to create table");

			sqlx::query("INSERT INTO test_models (id, name) VALUES (1, 'Alice'), (2, 'Bob')")
				.execute(&pool)
				.await
				.expect("Failed to insert data");

			let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM test_models")
				.fetch_one(&pool)
				.await
				.expect("Count failed");
			assert_eq!(count, 2);

			pool.close().await;
		}

		#[tokio::test]
		#[serial(async_query_sqlite)]
		async fn test_sqlite_async_session() {
			let pool = create_sqlite_pool()
				.await
				.expect("Failed to create SQLite pool");

			sqlx::query("CREATE TABLE test_models (id INTEGER PRIMARY KEY, name TEXT)")
				.execute(&pool)
				.await
				.unwrap();

			sqlx::query("INSERT INTO test_models (id, name) VALUES (1, 'Test')")
				.execute(&pool)
				.await
				.expect("Insert failed");

			let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM test_models)")
				.fetch_one(&pool)
				.await
				.expect("Exists check failed");
			assert!(exists);

			pool.close().await;
		}
	}
}
