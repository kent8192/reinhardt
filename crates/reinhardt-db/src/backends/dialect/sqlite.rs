//! SQLite dialect implementation

use async_trait::async_trait;
use sqlx::{Column, Row as SqlxRow, Sqlite, SqlitePool, Transaction, TypeInfo, sqlite::SqliteRow};
use std::sync::Arc;
use tracing::warn;

use crate::backends::{
	backend::DatabaseBackend,
	error::Result,
	types::{
		DatabaseType, IsolationLevel, QueryResult, QueryValue, Row, Savepoint, TransactionExecutor,
	},
};

/// SQLite database backend
pub struct SqliteBackend {
	pool: Arc<SqlitePool>,
}

impl SqliteBackend {
	pub fn new(pool: SqlitePool) -> Self {
		Self {
			pool: Arc::new(pool),
		}
	}

	pub fn pool(&self) -> &SqlitePool {
		&self.pool
	}

	fn bind_value<'q>(
		query: sqlx::query::Query<'q, sqlx::Sqlite, sqlx::sqlite::SqliteArguments<'q>>,
		value: &'q QueryValue,
	) -> sqlx::query::Query<'q, sqlx::Sqlite, sqlx::sqlite::SqliteArguments<'q>> {
		match value {
			QueryValue::Null => query.bind(None::<i32>),
			QueryValue::Bool(b) => query.bind(b),
			QueryValue::Int(i) => query.bind(i),
			QueryValue::Float(f) => query.bind(f),
			QueryValue::String(s) => query.bind(s),
			QueryValue::Bytes(b) => query.bind(b),
			QueryValue::Timestamp(dt) => query.bind(dt),
			// SQLite stores UUIDs as strings
			QueryValue::Uuid(u) => query.bind(u.to_string()),
			QueryValue::Now => {
				// SQLite uses datetime('now'), which should be part of SQL string
				// For binding, we use current UTC time
				query.bind(chrono::Utc::now())
			}
		}
	}

	fn convert_row(sqlite_row: SqliteRow) -> Result<Row> {
		let mut row = Row::new();
		for column in sqlite_row.columns() {
			let column_name = column.name();
			let type_name = column.type_info().name().to_uppercase();

			// First, check if the value is NULL by using Option<T>.
			// This is crucial because try_get::<i64> may return 0 for NULL values
			// in SQLite's RETURNING clause, causing incorrect type inference.
			// We check multiple Option types to ensure we detect NULL properly.
			let is_null = sqlite_row
				.try_get::<Option<String>, _>(column_name)
				.ok()
				.flatten()
				.is_none() && sqlite_row
				.try_get::<Option<i64>, _>(column_name)
				.ok()
				.flatten()
				.is_none() && sqlite_row
				.try_get::<Option<f64>, _>(column_name)
				.ok()
				.flatten()
				.is_none() && sqlite_row
				.try_get::<Option<Vec<u8>>, _>(column_name)
				.ok()
				.flatten()
				.is_none();

			if is_null {
				// All Option types returned None, so this is a NULL value
				row.insert(column_name.to_string(), QueryValue::Null);
				continue;
			}

			// Check declared column type first to handle BOOLEAN columns properly.
			// SQLite stores booleans as integers (0/1), so we need to check the declared type
			// before trying to read as integer, otherwise boolean columns get incorrectly
			// converted to QueryValue::Int instead of QueryValue::Bool.
			if type_name.contains("BOOL") {
				// Column is declared as BOOLEAN - convert integer 0/1 to boolean
				if let Ok(value) = sqlite_row.try_get::<i64, _>(column_name) {
					row.insert(column_name.to_string(), QueryValue::Bool(value != 0));
				} else if let Ok(value) = sqlite_row.try_get::<i32, _>(column_name) {
					row.insert(column_name.to_string(), QueryValue::Bool(value != 0));
				} else if let Ok(value) = sqlite_row.try_get::<bool, _>(column_name) {
					row.insert(column_name.to_string(), QueryValue::Bool(value));
				} else {
					row.insert(column_name.to_string(), QueryValue::Null);
				}
			} else if let Ok(value) = sqlite_row.try_get::<i64, _>(column_name) {
				row.insert(column_name.to_string(), QueryValue::Int(value));
			} else if let Ok(value) = sqlite_row.try_get::<i32, _>(column_name) {
				row.insert(column_name.to_string(), QueryValue::Int(value as i64));
			} else if let Ok(value) = sqlite_row.try_get::<bool, _>(column_name) {
				row.insert(column_name.to_string(), QueryValue::Bool(value));
			} else if let Ok(value) = sqlite_row.try_get::<f64, _>(column_name) {
				row.insert(column_name.to_string(), QueryValue::Float(value));
			} else if let Ok(value) = sqlite_row.try_get::<String, _>(column_name) {
				row.insert(column_name.to_string(), QueryValue::String(value));
			} else if let Ok(value) = sqlite_row.try_get::<Vec<u8>, _>(column_name) {
				row.insert(column_name.to_string(), QueryValue::Bytes(value));
			} else if let Ok(value) = sqlite_row.try_get::<chrono::NaiveDateTime, _>(column_name) {
				// SQLite stores timestamps as strings/integers, convert to DateTime<Utc>
				row.insert(
					column_name.to_string(),
					QueryValue::Timestamp(chrono::DateTime::from_naive_utc_and_offset(
						value,
						chrono::Utc,
					)),
				);
			} else if let Ok(value) =
				sqlite_row.try_get::<chrono::DateTime<chrono::Utc>, _>(column_name)
			{
				row.insert(column_name.to_string(), QueryValue::Timestamp(value));
			} else {
				// If we couldn't read the value, treat as NULL
				row.insert(column_name.to_string(), QueryValue::Null);
			}
		}
		Ok(row)
	}
}

#[async_trait]
impl DatabaseBackend for SqliteBackend {
	fn database_type(&self) -> DatabaseType {
		DatabaseType::Sqlite
	}

	fn placeholder(&self, _index: usize) -> String {
		"?".to_string()
	}

	fn supports_returning(&self) -> bool {
		true
	}

	fn supports_on_conflict(&self) -> bool {
		true
	}

	async fn execute(&self, sql: &str, params: Vec<QueryValue>) -> Result<QueryResult> {
		let mut query = sqlx::query(sql);
		for param in &params {
			query = Self::bind_value(query, param);
		}
		let result = query.execute(self.pool.as_ref()).await?;
		Ok(QueryResult {
			rows_affected: result.rows_affected(),
		})
	}

	async fn fetch_one(&self, sql: &str, params: Vec<QueryValue>) -> Result<Row> {
		let mut query = sqlx::query(sql);
		for param in &params {
			query = Self::bind_value(query, param);
		}
		let row = query.fetch_one(self.pool.as_ref()).await?;
		Self::convert_row(row)
	}

	async fn fetch_all(&self, sql: &str, params: Vec<QueryValue>) -> Result<Vec<Row>> {
		let mut query = sqlx::query(sql);
		for param in &params {
			query = Self::bind_value(query, param);
		}
		let rows = query.fetch_all(self.pool.as_ref()).await?;
		rows.into_iter().map(Self::convert_row).collect()
	}

	async fn fetch_optional(&self, sql: &str, params: Vec<QueryValue>) -> Result<Option<Row>> {
		let mut query = sqlx::query(sql);
		for param in &params {
			query = Self::bind_value(query, param);
		}
		let row = query.fetch_optional(self.pool.as_ref()).await?;
		row.map(Self::convert_row).transpose()
	}

	async fn begin(&self) -> Result<Box<dyn TransactionExecutor>> {
		let tx = self.pool.begin().await?;
		Ok(Box::new(SqliteTransactionExecutor::new(tx)))
	}

	/// Begin a transaction with the specified isolation level.
	///
	/// ## SQLite Isolation Level Limitations
	///
	/// SQLite does not support the standard SQL isolation levels (Read Uncommitted,
	/// Read Committed, Repeatable Read, Serializable). Instead, SQLite provides
	/// transaction modes: DEFERRED, IMMEDIATE, and EXCLUSIVE.
	///
	/// ### Behavior
	///
	/// - **Default (all levels except Serializable)**: Uses DEFERRED mode.
	///   The first read operation acquires a shared lock, and the first write
	///   operation upgrades to an exclusive lock.
	///
	/// - **Serializable**: A warning is logged because true serializable isolation
	///   requires EXCLUSIVE mode, which cannot be reliably set through connection
	///   pooling. However, SQLite in WAL (Write-Ahead Logging) mode provides
	///   snapshot isolation that is functionally similar to serializable isolation
	///   for most use cases.
	///
	/// ### WAL Mode Considerations
	///
	/// When SQLite is configured with WAL mode (recommended for concurrent access),
	/// readers don't block writers and writers don't block readers. Each transaction
	/// sees a consistent snapshot of the database, effectively providing serializable
	/// semantics for read operations.
	///
	/// ### For True EXCLUSIVE Transactions
	///
	/// If you need guaranteed exclusive access (e.g., for schema modifications),
	/// use raw SQL with the connection's `execute()` method:
	///
	/// ```sql
	/// BEGIN EXCLUSIVE;
	/// -- your operations
	/// COMMIT;
	/// ```
	async fn begin_with_isolation(
		&self,
		isolation_level: IsolationLevel,
	) -> Result<Box<dyn TransactionExecutor>> {
		// Generate the appropriate BEGIN statement for documentation purposes
		let _begin_sql = isolation_level.begin_transaction_sql(DatabaseType::Sqlite);

		// Warn users when Serializable is requested since SQLite's behavior differs
		if matches!(isolation_level, IsolationLevel::Serializable) {
			warn!(
				"SQLite does not support Serializable isolation level natively. \
				Using default DEFERRED mode. For WAL mode, this provides snapshot isolation. \
				For true exclusive access, use raw SQL: BEGIN EXCLUSIVE;"
			);
		}

		let tx = self.pool.begin().await?;
		Ok(Box::new(SqliteTransactionExecutor::new(tx)))
	}

	fn as_any(&self) -> &dyn std::any::Any {
		self
	}
}

/// SQLite transaction executor
pub struct SqliteTransactionExecutor {
	tx: Option<Transaction<'static, Sqlite>>,
}

impl SqliteTransactionExecutor {
	pub fn new(tx: Transaction<'static, Sqlite>) -> Self {
		Self { tx: Some(tx) }
	}

	fn bind_value<'q>(
		query: sqlx::query::Query<'q, sqlx::Sqlite, sqlx::sqlite::SqliteArguments<'q>>,
		value: &'q QueryValue,
	) -> sqlx::query::Query<'q, sqlx::Sqlite, sqlx::sqlite::SqliteArguments<'q>> {
		match value {
			QueryValue::Null => query.bind(None::<i32>),
			QueryValue::Bool(b) => query.bind(b),
			QueryValue::Int(i) => query.bind(i),
			QueryValue::Float(f) => query.bind(f),
			QueryValue::String(s) => query.bind(s),
			QueryValue::Bytes(b) => query.bind(b),
			QueryValue::Timestamp(dt) => query.bind(dt),
			// SQLite doesn't have native UUID type; bind as string
			QueryValue::Uuid(u) => query.bind(u.to_string()),
			QueryValue::Now => query.bind(chrono::Utc::now()),
		}
	}

	fn convert_row(sqlite_row: SqliteRow) -> Result<Row> {
		let mut row = Row::new();
		for column in sqlite_row.columns() {
			let column_name = column.name();
			let type_name = column.type_info().name().to_uppercase();

			// First, check if the value is NULL by using Option<T>.
			// This is crucial because try_get::<i64> may return 0 for NULL values
			// in SQLite's RETURNING clause, causing incorrect type inference.
			// We check multiple Option types to ensure we detect NULL properly.
			let is_null = sqlite_row
				.try_get::<Option<String>, _>(column_name)
				.ok()
				.flatten()
				.is_none() && sqlite_row
				.try_get::<Option<i64>, _>(column_name)
				.ok()
				.flatten()
				.is_none() && sqlite_row
				.try_get::<Option<f64>, _>(column_name)
				.ok()
				.flatten()
				.is_none() && sqlite_row
				.try_get::<Option<Vec<u8>>, _>(column_name)
				.ok()
				.flatten()
				.is_none();

			if is_null {
				// All Option types returned None, so this is a NULL value
				row.insert(column_name.to_string(), QueryValue::Null);
				continue;
			}

			// Check declared column type first to handle BOOLEAN columns properly.
			// SQLite stores booleans as integers (0/1), so we need to check the declared type
			// before trying to read as integer, otherwise boolean columns get incorrectly
			// converted to QueryValue::Int instead of QueryValue::Bool.
			if type_name.contains("BOOL") {
				// Column is declared as BOOLEAN - convert integer 0/1 to boolean
				if let Ok(value) = sqlite_row.try_get::<i64, _>(column_name) {
					row.insert(column_name.to_string(), QueryValue::Bool(value != 0));
				} else if let Ok(value) = sqlite_row.try_get::<i32, _>(column_name) {
					row.insert(column_name.to_string(), QueryValue::Bool(value != 0));
				} else if let Ok(value) = sqlite_row.try_get::<bool, _>(column_name) {
					row.insert(column_name.to_string(), QueryValue::Bool(value));
				} else {
					row.insert(column_name.to_string(), QueryValue::Null);
				}
			} else if let Ok(value) = sqlite_row.try_get::<i64, _>(column_name) {
				row.insert(column_name.to_string(), QueryValue::Int(value));
			} else if let Ok(value) = sqlite_row.try_get::<i32, _>(column_name) {
				row.insert(column_name.to_string(), QueryValue::Int(value as i64));
			} else if let Ok(value) = sqlite_row.try_get::<bool, _>(column_name) {
				row.insert(column_name.to_string(), QueryValue::Bool(value));
			} else if let Ok(value) = sqlite_row.try_get::<f64, _>(column_name) {
				row.insert(column_name.to_string(), QueryValue::Float(value));
			} else if let Ok(value) = sqlite_row.try_get::<String, _>(column_name) {
				row.insert(column_name.to_string(), QueryValue::String(value));
			} else if let Ok(value) = sqlite_row.try_get::<Vec<u8>, _>(column_name) {
				row.insert(column_name.to_string(), QueryValue::Bytes(value));
			} else if let Ok(value) = sqlite_row.try_get::<chrono::NaiveDateTime, _>(column_name) {
				// SQLite stores timestamps as strings/integers, convert to DateTime<Utc>
				row.insert(
					column_name.to_string(),
					QueryValue::Timestamp(chrono::DateTime::from_naive_utc_and_offset(
						value,
						chrono::Utc,
					)),
				);
			} else if let Ok(value) =
				sqlite_row.try_get::<chrono::DateTime<chrono::Utc>, _>(column_name)
			{
				row.insert(column_name.to_string(), QueryValue::Timestamp(value));
			} else {
				// If we couldn't read the value, treat as NULL
				row.insert(column_name.to_string(), QueryValue::Null);
			}
		}
		Ok(row)
	}
}

#[async_trait]
impl TransactionExecutor for SqliteTransactionExecutor {
	async fn execute(&mut self, sql: &str, params: Vec<QueryValue>) -> Result<QueryResult> {
		let tx = self.tx.as_mut().ok_or_else(|| {
			crate::backends::error::DatabaseError::TransactionError(
				"Transaction already consumed".to_string(),
			)
		})?;

		let mut query = sqlx::query(sql);
		for param in &params {
			query = Self::bind_value(query, param);
		}
		let result = query.execute(&mut **tx).await?;
		Ok(QueryResult {
			rows_affected: result.rows_affected(),
		})
	}

	async fn fetch_one(&mut self, sql: &str, params: Vec<QueryValue>) -> Result<Row> {
		let tx = self.tx.as_mut().ok_or_else(|| {
			crate::backends::error::DatabaseError::TransactionError(
				"Transaction already consumed".to_string(),
			)
		})?;

		let mut query = sqlx::query(sql);
		for param in &params {
			query = Self::bind_value(query, param);
		}
		let row = query.fetch_one(&mut **tx).await?;
		Self::convert_row(row)
	}

	async fn fetch_all(&mut self, sql: &str, params: Vec<QueryValue>) -> Result<Vec<Row>> {
		let tx = self.tx.as_mut().ok_or_else(|| {
			crate::backends::error::DatabaseError::TransactionError(
				"Transaction already consumed".to_string(),
			)
		})?;

		let mut query = sqlx::query(sql);
		for param in &params {
			query = Self::bind_value(query, param);
		}
		let rows = query.fetch_all(&mut **tx).await?;
		rows.into_iter().map(Self::convert_row).collect()
	}

	async fn fetch_optional(&mut self, sql: &str, params: Vec<QueryValue>) -> Result<Option<Row>> {
		let tx = self.tx.as_mut().ok_or_else(|| {
			crate::backends::error::DatabaseError::TransactionError(
				"Transaction already consumed".to_string(),
			)
		})?;

		let mut query = sqlx::query(sql);
		for param in &params {
			query = Self::bind_value(query, param);
		}
		let row = query.fetch_optional(&mut **tx).await?;
		row.map(Self::convert_row).transpose()
	}

	async fn commit(mut self: Box<Self>) -> Result<()> {
		let tx = self.tx.take().ok_or_else(|| {
			crate::backends::error::DatabaseError::TransactionError(
				"Transaction already consumed".to_string(),
			)
		})?;
		tx.commit().await?;
		Ok(())
	}

	async fn rollback(mut self: Box<Self>) -> Result<()> {
		let tx = self.tx.take().ok_or_else(|| {
			crate::backends::error::DatabaseError::TransactionError(
				"Transaction already consumed".to_string(),
			)
		})?;
		tx.rollback().await?;
		Ok(())
	}

	async fn savepoint(&mut self, name: &str) -> Result<()> {
		let tx = self.tx.as_mut().ok_or_else(|| {
			crate::backends::error::DatabaseError::TransactionError(
				"Transaction already consumed".to_string(),
			)
		})?;

		let sp = Savepoint::new(name);
		sqlx::query(&sp.to_sql()).execute(&mut **tx).await?;
		Ok(())
	}

	async fn release_savepoint(&mut self, name: &str) -> Result<()> {
		let tx = self.tx.as_mut().ok_or_else(|| {
			crate::backends::error::DatabaseError::TransactionError(
				"Transaction already consumed".to_string(),
			)
		})?;

		let sp = Savepoint::new(name);
		sqlx::query(&sp.release_sql()).execute(&mut **tx).await?;
		Ok(())
	}

	async fn rollback_to_savepoint(&mut self, name: &str) -> Result<()> {
		let tx = self.tx.as_mut().ok_or_else(|| {
			crate::backends::error::DatabaseError::TransactionError(
				"Transaction already consumed".to_string(),
			)
		})?;

		let sp = Savepoint::new(name);
		sqlx::query(&sp.rollback_sql()).execute(&mut **tx).await?;
		Ok(())
	}
}
