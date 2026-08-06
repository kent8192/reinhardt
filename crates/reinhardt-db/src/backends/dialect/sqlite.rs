//! SQLite dialect implementation

use async_trait::async_trait;
use futures::StreamExt;
use sqlx::{
	Column, Executor, Row as SqlxRow, Sqlite, SqlitePool, Transaction, TypeInfo,
	pool::PoolConnection, sqlite::SqliteRow,
};
use std::sync::Arc;
use tracing::warn;

use crate::backends::{
	backend::DatabaseBackend,
	error::{DatabaseError, DatabaseErrorKind, Result, map_sqlx_error},
	types::{
		DatabaseType, IsolationLevel, QueryResult, QueryValue, Row, RowStream, Savepoint,
		TransactionExecutor,
	},
};

fn transaction_consumed_error() -> DatabaseError {
	DatabaseError::new(
		DatabaseErrorKind::Transaction,
		"Transaction already consumed",
	)
}

const SQLITE_BEGIN_WRITE_SQL: &str = "BEGIN IMMEDIATE";
const SQLITE_COMMIT_SQL: &str = "COMMIT";

#[async_trait]
trait SqliteTransactionControl {
	async fn execute_control(&mut self, sql: &str) -> Result<()>;
}

#[async_trait]
impl SqliteTransactionControl for PoolConnection<Sqlite> {
	async fn execute_control(&mut self, sql: &str) -> Result<()> {
		Executor::execute(&mut **self, sqlx::raw_sql(sql))
			.await
			.map_err(map_sqlx_error)?;
		Ok(())
	}
}

async fn begin_sqlite_write<C: SqliteTransactionControl + Send>(conn: &mut C) -> Result<()> {
	conn.execute_control(SQLITE_BEGIN_WRITE_SQL).await
}

async fn commit_sqlite_write<C: SqliteTransactionControl + Send>(conn: &mut C) -> Result<()> {
	conn.execute_control(SQLITE_COMMIT_SQL).await
}

#[cfg(feature = "pgvector")]
fn vector_unsupported_error() -> DatabaseError {
	DatabaseError::new(
		DatabaseErrorKind::Type,
		"PostgreSQL vector values are not supported by the SQLite backend",
	)
}

/// SQLite database backend
pub struct SqliteBackend {
	pool: Arc<SqlitePool>,
}

impl SqliteBackend {
	/// Creates a new SQLite backend with the given pool.
	pub fn new(pool: SqlitePool) -> Self {
		Self {
			pool: Arc::new(pool),
		}
	}

	/// Returns a reference to the underlying SQLite pool.
	pub fn pool(&self) -> &SqlitePool {
		&self.pool
	}

	pub(crate) fn bind_value<'q>(
		query: sqlx::query::Query<'q, sqlx::Sqlite, sqlx::sqlite::SqliteArguments<'q>>,
		value: &'q QueryValue,
	) -> Result<sqlx::query::Query<'q, sqlx::Sqlite, sqlx::sqlite::SqliteArguments<'q>>> {
		Ok(match value {
			QueryValue::Null => query.bind(None::<i32>),
			QueryValue::Bool(b) => query.bind(b),
			QueryValue::Int(i) => query.bind(i),
			QueryValue::Float(f) => query.bind(f),
			QueryValue::String(s) => query.bind(s),
			QueryValue::Bytes(b) => query.bind(b),
			QueryValue::Timestamp(dt) => query.bind(dt),
			QueryValue::NaiveTimestamp(dt) => query.bind(dt),
			// SQLite stores UUIDs as strings
			QueryValue::Uuid(u) => query.bind(u.to_string()),
			QueryValue::Json(value) => query.bind(value.as_deref().cloned().map(sqlx::types::Json)),
			#[cfg(feature = "pgvector")]
			QueryValue::Vector(_) => return Err(vector_unsupported_error().into()),
			QueryValue::StringArray(values) => {
				query.bind(serde_json::to_string(values).expect("string arrays serialize"))
			}
			QueryValue::IntArray(values) => {
				query.bind(serde_json::to_string(values).expect("integer arrays serialize"))
			}
			QueryValue::BigIntArray(values) => {
				query.bind(serde_json::to_string(values).expect("big integer arrays serialize"))
			}
			QueryValue::BoolArray(values) => {
				query.bind(serde_json::to_string(values).expect("boolean arrays serialize"))
			}
			QueryValue::FloatArray(values) => {
				query.bind(serde_json::to_string(values).expect("float arrays serialize"))
			}
			QueryValue::DoubleArray(values) => {
				query.bind(serde_json::to_string(values).expect("double arrays serialize"))
			}
			QueryValue::UuidArray(values) => {
				query.bind(serde_json::to_string(values).expect("UUID arrays serialize"))
			}
			QueryValue::Now => {
				// SQLite uses datetime('now'), which should be part of SQL string
				// For binding, we use current UTC time
				query.bind(chrono::Utc::now())
			}
		})
	}

	pub(crate) fn convert_row(sqlite_row: SqliteRow) -> Result<Row> {
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
				row.insert(column_name.to_string(), QueryValue::NaiveTimestamp(value));
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

	fn supports_row_streaming(&self) -> bool {
		true
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
			query = Self::bind_value(query, param)?;
		}
		let result = query
			.execute(self.pool.as_ref())
			.await
			.map_err(map_sqlx_error)?;
		Ok(QueryResult {
			rows_affected: result.rows_affected(),
			last_insert_id: None,
		})
	}

	async fn fetch_one(&self, sql: &str, params: Vec<QueryValue>) -> Result<Row> {
		let mut query = sqlx::query(sql);
		for param in &params {
			query = Self::bind_value(query, param)?;
		}
		let row = query
			.fetch_one(self.pool.as_ref())
			.await
			.map_err(map_sqlx_error)?;
		Self::convert_row(row)
	}

	async fn fetch_all(&self, sql: &str, params: Vec<QueryValue>) -> Result<Vec<Row>> {
		let mut query = sqlx::query(sql);
		for param in &params {
			query = Self::bind_value(query, param)?;
		}
		let rows = query
			.fetch_all(self.pool.as_ref())
			.await
			.map_err(map_sqlx_error)?;
		rows.into_iter().map(Self::convert_row).collect()
	}

	fn fetch_stream<'a>(
		&'a self,
		sql: String,
		params: Vec<QueryValue>,
		chunk_size: usize,
	) -> Result<RowStream<'a>> {
		if chunk_size == 0 {
			return Err(DatabaseError::new(
				DatabaseErrorKind::Configuration,
				"Row stream chunk_size must be greater than zero",
			)
			.into());
		}
		let pool = Arc::clone(&self.pool);
		Ok(Box::pin(async_stream::stream! {
			let mut query = sqlx::query(&sql);
			for param in &params {
				query = match Self::bind_value(query, param) {
					Ok(query) => query,
					Err(error) => {
						yield Err(error);
						return;
					}
				};
			}
			let rows = query.fetch(pool.as_ref());
			futures::pin_mut!(rows);
			let rows = rows.ready_chunks(chunk_size);
			futures::pin_mut!(rows);
			while let Some(chunk) = rows.next().await {
				for row in chunk {
					yield row
						.map_err(|error| map_sqlx_error(error).into())
						.and_then(Self::convert_row);
				}
			}
		}))
	}

	async fn fetch_optional(&self, sql: &str, params: Vec<QueryValue>) -> Result<Option<Row>> {
		let mut query = sqlx::query(sql);
		for param in &params {
			query = Self::bind_value(query, param)?;
		}
		let row = query
			.fetch_optional(self.pool.as_ref())
			.await
			.map_err(map_sqlx_error)?;
		row.map(Self::convert_row).transpose()
	}

	async fn begin(&self) -> Result<Box<dyn TransactionExecutor>> {
		let tx = self.pool.begin().await.map_err(map_sqlx_error)?;
		Ok(Box::new(SqliteTransactionExecutor::new(tx)))
	}

	async fn begin_write(&self) -> Result<Box<dyn TransactionExecutor>> {
		let conn = self.pool.acquire().await.map_err(map_sqlx_error)?;
		let mut conn = CloseOnDropGuard::new(conn);
		begin_sqlite_write(conn.connection_mut()).await?;
		Ok(Box::new(SqliteRawTransactionExecutor::new(conn)))
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

		let tx = self.pool.begin().await.map_err(map_sqlx_error)?;
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
	/// Creates a new SQLite transaction executor.
	pub fn new(tx: Transaction<'static, Sqlite>) -> Self {
		Self { tx: Some(tx) }
	}

	fn bind_value<'q>(
		query: sqlx::query::Query<'q, sqlx::Sqlite, sqlx::sqlite::SqliteArguments<'q>>,
		value: &'q QueryValue,
	) -> Result<sqlx::query::Query<'q, sqlx::Sqlite, sqlx::sqlite::SqliteArguments<'q>>> {
		Ok(match value {
			QueryValue::Null => query.bind(None::<i32>),
			QueryValue::Bool(b) => query.bind(b),
			QueryValue::Int(i) => query.bind(i),
			QueryValue::Float(f) => query.bind(f),
			QueryValue::String(s) => query.bind(s),
			QueryValue::Bytes(b) => query.bind(b),
			QueryValue::Timestamp(dt) => query.bind(dt),
			QueryValue::NaiveTimestamp(dt) => query.bind(dt),
			// SQLite doesn't have native UUID type; bind as string
			QueryValue::Uuid(u) => query.bind(u.to_string()),
			QueryValue::Json(value) => query.bind(value.as_deref().cloned().map(sqlx::types::Json)),
			#[cfg(feature = "pgvector")]
			QueryValue::Vector(_) => return Err(vector_unsupported_error().into()),
			QueryValue::StringArray(values) => {
				query.bind(serde_json::to_string(values).expect("string arrays serialize"))
			}
			QueryValue::IntArray(values) => {
				query.bind(serde_json::to_string(values).expect("integer arrays serialize"))
			}
			QueryValue::BigIntArray(values) => {
				query.bind(serde_json::to_string(values).expect("big integer arrays serialize"))
			}
			QueryValue::BoolArray(values) => {
				query.bind(serde_json::to_string(values).expect("boolean arrays serialize"))
			}
			QueryValue::FloatArray(values) => {
				query.bind(serde_json::to_string(values).expect("float arrays serialize"))
			}
			QueryValue::DoubleArray(values) => {
				query.bind(serde_json::to_string(values).expect("double arrays serialize"))
			}
			QueryValue::UuidArray(values) => {
				query.bind(serde_json::to_string(values).expect("UUID arrays serialize"))
			}
			QueryValue::Now => query.bind(chrono::Utc::now()),
		})
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
				row.insert(column_name.to_string(), QueryValue::NaiveTimestamp(value));
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
	fn backend(&self) -> DatabaseType {
		DatabaseType::Sqlite
	}

	async fn execute(&mut self, sql: &str, params: Vec<QueryValue>) -> Result<QueryResult> {
		let tx = self.tx.as_mut().ok_or_else(transaction_consumed_error)?;

		let mut query = sqlx::query(sql);
		for param in &params {
			query = Self::bind_value(query, param)?;
		}
		let result = query.execute(&mut **tx).await.map_err(map_sqlx_error)?;
		Ok(QueryResult {
			rows_affected: result.rows_affected(),
			last_insert_id: None,
		})
	}

	async fn fetch_one(&mut self, sql: &str, params: Vec<QueryValue>) -> Result<Row> {
		let tx = self.tx.as_mut().ok_or_else(transaction_consumed_error)?;

		let mut query = sqlx::query(sql);
		for param in &params {
			query = Self::bind_value(query, param)?;
		}
		let row = query.fetch_one(&mut **tx).await.map_err(map_sqlx_error)?;
		Self::convert_row(row)
	}

	async fn fetch_all(&mut self, sql: &str, params: Vec<QueryValue>) -> Result<Vec<Row>> {
		let tx = self.tx.as_mut().ok_or_else(transaction_consumed_error)?;

		let mut query = sqlx::query(sql);
		for param in &params {
			query = Self::bind_value(query, param)?;
		}
		let rows = query.fetch_all(&mut **tx).await.map_err(map_sqlx_error)?;
		rows.into_iter().map(Self::convert_row).collect()
	}

	fn fetch_stream<'a>(
		&'a mut self,
		sql: String,
		params: Vec<QueryValue>,
		chunk_size: usize,
	) -> Result<RowStream<'a>> {
		if chunk_size == 0 {
			return Err(DatabaseError::new(
				DatabaseErrorKind::Configuration,
				"Row stream chunk_size must be greater than zero",
			)
			.into());
		}
		let tx = self.tx.as_mut().ok_or_else(transaction_consumed_error)?;
		Ok(Box::pin(async_stream::stream! {
			let mut query = sqlx::query(&sql);
			for param in &params {
				query = match Self::bind_value(query, param) {
					Ok(query) => query,
					Err(error) => {
						yield Err(error);
						return;
					}
				};
			}
			let rows = query.fetch(&mut **tx);
			futures::pin_mut!(rows);
			let rows = rows.ready_chunks(chunk_size);
			futures::pin_mut!(rows);
			while let Some(chunk) = rows.next().await {
				for row in chunk {
					yield row
						.map_err(|error| map_sqlx_error(error).into())
						.and_then(Self::convert_row);
				}
			}
		}))
	}

	async fn fetch_optional(&mut self, sql: &str, params: Vec<QueryValue>) -> Result<Option<Row>> {
		let tx = self.tx.as_mut().ok_or_else(transaction_consumed_error)?;

		let mut query = sqlx::query(sql);
		for param in &params {
			query = Self::bind_value(query, param)?;
		}
		let row = query
			.fetch_optional(&mut **tx)
			.await
			.map_err(map_sqlx_error)?;
		row.map(Self::convert_row).transpose()
	}

	async fn commit(mut self: Box<Self>) -> Result<()> {
		let tx = self.tx.take().ok_or_else(transaction_consumed_error)?;
		tx.commit().await.map_err(map_sqlx_error)?;
		Ok(())
	}

	async fn rollback(mut self: Box<Self>) -> Result<()> {
		let tx = self.tx.take().ok_or_else(transaction_consumed_error)?;
		tx.rollback().await.map_err(map_sqlx_error)?;
		Ok(())
	}

	async fn savepoint(&mut self, name: &str) -> Result<()> {
		let tx = self.tx.as_mut().ok_or_else(transaction_consumed_error)?;

		let sp = Savepoint::new(name);
		sqlx::query(&sp.to_sql())
			.execute(&mut **tx)
			.await
			.map_err(map_sqlx_error)?;
		Ok(())
	}

	async fn release_savepoint(&mut self, name: &str) -> Result<()> {
		let tx = self.tx.as_mut().ok_or_else(transaction_consumed_error)?;

		let sp = Savepoint::new(name);
		sqlx::query(&sp.release_sql())
			.execute(&mut **tx)
			.await
			.map_err(map_sqlx_error)?;
		Ok(())
	}

	async fn rollback_to_savepoint(&mut self, name: &str) -> Result<()> {
		let tx = self.tx.as_mut().ok_or_else(transaction_consumed_error)?;

		let sp = Savepoint::new(name);
		sqlx::query(&sp.rollback_sql())
			.execute(&mut **tx)
			.await
			.map_err(map_sqlx_error)?;
		Ok(())
	}
}

trait CloseOnDrop {
	fn mark_for_close_on_drop(&mut self);
}

impl CloseOnDrop for PoolConnection<Sqlite> {
	fn mark_for_close_on_drop(&mut self) {
		self.close_on_drop();
	}
}

struct CloseOnDropGuard<T: CloseOnDrop> {
	connection: Option<T>,
	close_on_drop: bool,
}

impl<T: CloseOnDrop> CloseOnDropGuard<T> {
	fn new(connection: T) -> Self {
		Self {
			connection: Some(connection),
			close_on_drop: true,
		}
	}

	fn connection_mut(&mut self) -> &mut T {
		self.connection
			.as_mut()
			.expect("an armed SQLite transaction guard must own its connection")
	}

	fn disarm(mut self) -> T {
		self.close_on_drop = false;
		self.connection
			.take()
			.expect("an armed SQLite transaction guard must own its connection")
	}

	fn mark_for_close_on_drop(&mut self) {
		if self.close_on_drop {
			if let Some(connection) = self.connection.as_mut() {
				connection.mark_for_close_on_drop();
			}
			self.close_on_drop = false;
		}
	}
}

impl<T: CloseOnDrop> Drop for CloseOnDropGuard<T> {
	fn drop(&mut self) {
		self.mark_for_close_on_drop();
	}
}

struct SqliteRawTransactionExecutor {
	conn: Option<CloseOnDropGuard<PoolConnection<Sqlite>>>,
}

impl SqliteRawTransactionExecutor {
	fn new(conn: CloseOnDropGuard<PoolConnection<Sqlite>>) -> Self {
		Self { conn: Some(conn) }
	}

	fn connection_mut(&mut self) -> Result<&mut PoolConnection<Sqlite>> {
		let conn = self.conn.as_mut().ok_or_else(transaction_consumed_error)?;
		Ok(conn.connection_mut())
	}
}

impl Drop for SqliteRawTransactionExecutor {
	fn drop(&mut self) {
		if let Some(conn) = self.conn.as_mut() {
			conn.mark_for_close_on_drop();
		}
	}
}

#[async_trait]
impl TransactionExecutor for SqliteRawTransactionExecutor {
	fn backend(&self) -> DatabaseType {
		DatabaseType::Sqlite
	}

	async fn execute(&mut self, sql: &str, params: Vec<QueryValue>) -> Result<QueryResult> {
		let conn = self.connection_mut()?;
		let mut query = sqlx::query(sql);
		for param in &params {
			query = SqliteBackend::bind_value(query, param)?;
		}
		let result = query.execute(&mut **conn).await.map_err(map_sqlx_error)?;
		Ok(QueryResult {
			rows_affected: result.rows_affected(),
			last_insert_id: None,
		})
	}

	async fn fetch_one(&mut self, sql: &str, params: Vec<QueryValue>) -> Result<Row> {
		let conn = self.connection_mut()?;
		let mut query = sqlx::query(sql);
		for param in &params {
			query = SqliteBackend::bind_value(query, param)?;
		}
		let row = query.fetch_one(&mut **conn).await.map_err(map_sqlx_error)?;
		SqliteBackend::convert_row(row)
	}

	async fn fetch_all(&mut self, sql: &str, params: Vec<QueryValue>) -> Result<Vec<Row>> {
		let conn = self.connection_mut()?;
		let mut query = sqlx::query(sql);
		for param in &params {
			query = SqliteBackend::bind_value(query, param)?;
		}
		let rows = query.fetch_all(&mut **conn).await.map_err(map_sqlx_error)?;
		rows.into_iter().map(SqliteBackend::convert_row).collect()
	}

	fn fetch_stream<'a>(
		&'a mut self,
		sql: String,
		params: Vec<QueryValue>,
		chunk_size: usize,
	) -> Result<RowStream<'a>> {
		if chunk_size == 0 {
			return Err(DatabaseError::new(
				DatabaseErrorKind::Configuration,
				"Row stream chunk_size must be greater than zero",
			)
			.into());
		}
		let conn = self.connection_mut()?;
		Ok(Box::pin(async_stream::stream! {
			let mut query = sqlx::query(&sql);
			for param in &params {
				query = match SqliteBackend::bind_value(query, param) {
					Ok(query) => query,
					Err(error) => {
						yield Err(error);
						return;
					}
				};
			}
			let rows = query.fetch(&mut **conn);
			futures::pin_mut!(rows);
			let rows = rows.ready_chunks(chunk_size);
			futures::pin_mut!(rows);
			while let Some(chunk) = rows.next().await {
				for row in chunk {
					yield row
						.map_err(|error| map_sqlx_error(error).into())
						.and_then(SqliteBackend::convert_row);
				}
			}
		}))
	}

	async fn fetch_optional(&mut self, sql: &str, params: Vec<QueryValue>) -> Result<Option<Row>> {
		let conn = self.connection_mut()?;
		let mut query = sqlx::query(sql);
		for param in &params {
			query = SqliteBackend::bind_value(query, param)?;
		}
		let row = query
			.fetch_optional(&mut **conn)
			.await
			.map_err(map_sqlx_error)?;
		row.map(SqliteBackend::convert_row).transpose()
	}

	async fn commit(mut self: Box<Self>) -> Result<()> {
		let mut conn = self.conn.take().ok_or_else(transaction_consumed_error)?;
		commit_sqlite_write(conn.connection_mut()).await?;
		let connection = conn.disarm();
		drop(connection);
		Ok(())
	}

	async fn rollback(mut self: Box<Self>) -> Result<()> {
		let mut conn = self.conn.take().ok_or_else(transaction_consumed_error)?;
		Executor::execute(&mut **conn.connection_mut(), sqlx::raw_sql("ROLLBACK"))
			.await
			.map_err(map_sqlx_error)?;
		let connection = conn.disarm();
		drop(connection);
		Ok(())
	}

	async fn savepoint(&mut self, name: &str) -> Result<()> {
		let sql = Savepoint::new(name).to_sql();
		let conn = self.connection_mut()?;
		Executor::execute(&mut **conn, sqlx::raw_sql(&sql))
			.await
			.map_err(map_sqlx_error)?;
		Ok(())
	}

	async fn release_savepoint(&mut self, name: &str) -> Result<()> {
		let sql = Savepoint::new(name).release_sql();
		let conn = self.connection_mut()?;
		Executor::execute(&mut **conn, sqlx::raw_sql(&sql))
			.await
			.map_err(map_sqlx_error)?;
		Ok(())
	}

	async fn rollback_to_savepoint(&mut self, name: &str) -> Result<()> {
		let sql = Savepoint::new(name).rollback_sql();
		let conn = self.connection_mut()?;
		Executor::execute(&mut **conn, sqlx::raw_sql(&sql))
			.await
			.map_err(map_sqlx_error)?;
		Ok(())
	}
}

#[cfg(test)]
mod tests {
	use super::{
		CloseOnDrop, CloseOnDropGuard, SqliteBackend, SqliteTransactionControl,
		SqliteTransactionExecutor, begin_sqlite_write, commit_sqlite_write,
	};
	use crate::backends::backend::DatabaseBackend;
	use crate::backends::error::Result;
	use crate::backends::types::{DatabaseType, QueryValue, TransactionExecutor};
	use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
	use std::str::FromStr;
	use std::sync::{
		Arc,
		atomic::{AtomicBool, Ordering},
	};
	use std::time::Duration;

	struct RecordingTransactionControl {
		calls: Vec<String>,
	}

	#[async_trait::async_trait]
	impl SqliteTransactionControl for RecordingTransactionControl {
		async fn execute_control(&mut self, sql: &str) -> Result<()> {
			self.calls.push(sql.to_owned());
			Ok(())
		}
	}

	struct TestCloseOnDrop {
		closed: Arc<AtomicBool>,
	}

	impl CloseOnDrop for TestCloseOnDrop {
		fn mark_for_close_on_drop(&mut self) {
			self.closed.store(true, Ordering::Release);
		}
	}

	#[test]
	fn test_transaction_executor_reports_sqlite_backend() {
		let executor = SqliteTransactionExecutor { tx: None };

		assert_eq!(executor.backend(), DatabaseType::Sqlite);
	}

	#[tokio::test]
	async fn sqlite_begin_write_records_immediate_begin_and_commit() {
		let mut control = RecordingTransactionControl { calls: Vec::new() };

		begin_sqlite_write(&mut control)
			.await
			.expect("record BEGIN IMMEDIATE");
		commit_sqlite_write(&mut control)
			.await
			.expect("record COMMIT");

		assert_eq!(control.calls, ["BEGIN IMMEDIATE", "COMMIT"]);
	}

	#[test]
	fn sqlite_begin_write_drop_guard_closes_when_still_armed() {
		let closed = Arc::new(AtomicBool::new(false));
		{
			let _guard = CloseOnDropGuard::new(TestCloseOnDrop {
				closed: Arc::clone(&closed),
			});
		}

		assert!(closed.load(Ordering::Acquire));
	}

	async fn sqlite_begin_write_backend() -> (tempfile::TempDir, std::sync::Arc<SqliteBackend>) {
		let directory = tempfile::tempdir().expect("create SQLite test directory");
		let database_path = directory.path().join("write-intent.sqlite3");
		let options =
			SqliteConnectOptions::from_str(&format!("sqlite://{}", database_path.display()))
				.expect("build SQLite connection options")
				.create_if_missing(true)
				.busy_timeout(Duration::from_secs(1));
		let pool = SqlitePoolOptions::new()
			.max_connections(2)
			.connect_with(options)
			.await
			.expect("connect SQLite test pool");
		(directory, std::sync::Arc::new(SqliteBackend::new(pool)))
	}

	#[tokio::test]
	async fn sqlite_begin_write_blocks_a_second_writer_before_reads() {
		let (_directory, backend) = sqlite_begin_write_backend().await;
		let first = backend.begin_write().await.expect("begin first writer");
		let second_backend = std::sync::Arc::clone(&backend);
		let mut second = tokio::spawn(async move { second_backend.begin_write().await });

		assert!(
			tokio::time::timeout(Duration::from_millis(100), &mut second)
				.await
				.is_err(),
			"a second writer must remain blocked while BEGIN IMMEDIATE is active"
		);
		first.rollback().await.expect("roll back first writer");
		let second = tokio::time::timeout(Duration::from_secs(2), second)
			.await
			.expect("second writer must enter after rollback")
			.expect("second writer task must not panic")
			.expect("begin second writer");
		second.rollback().await.expect("roll back second writer");
	}

	#[tokio::test]
	async fn sqlite_begin_write_drop_discards_the_active_connection() {
		let (_directory, backend) = sqlite_begin_write_backend().await;
		let unfinished = backend
			.begin_write()
			.await
			.expect("begin unfinished writer");

		drop(unfinished);

		let replacement = tokio::time::timeout(Duration::from_secs(2), backend.begin_write())
			.await
			.expect("replacement writer must not remain blocked")
			.expect("an unfinished transaction must not poison a pooled connection");
		replacement
			.commit()
			.await
			.expect("commit replacement writer");
	}

	#[test]
	#[cfg(feature = "pgvector")]
	fn sqlite_rejects_vector_parameters_without_a_fallback_encoding() {
		let error = SqliteBackend::bind_value(
			sqlx::query("SELECT ?"),
			&QueryValue::Vector(Some(vec![1.0, 2.0, 3.0])),
		)
		.err()
		.unwrap();

		assert_eq!(
			error.database_kind(),
			Some(reinhardt_core::exception::DatabaseErrorKind::Type)
		);
		assert!(
			error
				.to_string()
				.contains("not supported by the SQLite backend")
		);
	}
}
