//! MySQL dialect implementation

use async_trait::async_trait;
use futures::StreamExt;
use sqlx::{
	Column, Executor, MySql, MySqlPool, Row as SqlxRow, Transaction, TypeInfo, ValueRef,
	mysql::MySqlRow,
};
use std::sync::Arc;

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

#[cfg(feature = "pgvector")]
fn vector_unsupported_error() -> DatabaseError {
	DatabaseError::new(
		DatabaseErrorKind::Type,
		"PostgreSQL vector values are not supported by the MySQL backend",
	)
}

fn optional_last_insert_id(last_insert_id: u64) -> Option<u64> {
	(last_insert_id != 0).then_some(last_insert_id)
}

/// Returns whether SQLx identified a MySQL column as a boolean.
///
/// SQLx normally exposes `TINYINT(1)` as `BOOLEAN`, but some MySQL-compatible
/// servers and result paths preserve the wire spelling. Calling `try_get::<bool>`
/// on every integer column would otherwise coerce primary keys and aggregate
/// counts to booleans before their numeric decoders have a chance to run.
fn is_boolean_type(type_name: &str) -> bool {
	matches!(type_name, "BOOLEAN" | "BOOL" | "TINYINT(1)")
}

/// Builds a MySQL savepoint statement with a validated identifier.
///
/// MySQL does not accept the ANSI double-quoted identifier form emitted by
/// [`Savepoint`]. Its constructor still validates the caller-provided name
/// before this dialect renders it with MySQL backticks.
///
/// Workaround for launchbadge/sqlx#3613 (tracked in reinhardt-web#5699).
/// Remove this workaround when reinhardt-db upgrades to an SQLx release that
/// includes the upstream lifetime fix.
///
/// Ideal implementation (without workaround):
/// `sqlx::raw_sql(&sql).execute(&mut **tx).await?;`
///
/// SQLx 0.8's convenience method cannot be used in this `async_trait` path.
/// Calling [`Executor::execute`] directly still selects MySQL's non-prepared
/// protocol. Validation prevents this raw SQL path from interpolating an
/// arbitrary identifier.
fn mysql_savepoint_sql(name: &str) -> String {
	let savepoint = Savepoint::new(name);
	format!("SAVEPOINT `{}`", savepoint.name())
}

/// Builds a MySQL release-savepoint statement with a validated identifier.
fn mysql_release_savepoint_sql(name: &str) -> String {
	let savepoint = Savepoint::new(name);
	format!("RELEASE SAVEPOINT `{}`", savepoint.name())
}

/// Builds a MySQL rollback-to-savepoint statement with a validated identifier.
fn mysql_rollback_to_savepoint_sql(name: &str) -> String {
	let savepoint = Savepoint::new(name);
	format!("ROLLBACK TO SAVEPOINT `{}`", savepoint.name())
}

/// MySQL database backend
pub struct MySqlBackend {
	pool: Arc<MySqlPool>,
}

impl MySqlBackend {
	/// Creates a new MySQL backend with the given pool.
	pub fn new(pool: MySqlPool) -> Self {
		Self {
			pool: Arc::new(pool),
		}
	}

	/// Returns a reference to the underlying MySQL pool.
	pub fn pool(&self) -> &MySqlPool {
		&self.pool
	}

	fn bind_value<'q>(
		query: sqlx::query::Query<'q, sqlx::MySql, sqlx::mysql::MySqlArguments>,
		value: &'q QueryValue,
	) -> Result<sqlx::query::Query<'q, sqlx::MySql, sqlx::mysql::MySqlArguments>> {
		Ok(match value {
			QueryValue::Null => query.bind(None::<i32>),
			QueryValue::Bool(b) => query.bind(b),
			QueryValue::Int(i) => query.bind(i),
			QueryValue::Float(f) => query.bind(f),
			QueryValue::String(s) => query.bind(s),
			QueryValue::Bytes(b) => query.bind(b),
			QueryValue::Timestamp(dt) => query.bind(dt),
			QueryValue::NaiveTimestamp(dt) => query.bind(dt),
			// MySQL stores UUIDs as BINARY(16) or CHAR(36); we bind as string
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
				// MySQL uses NOW() function, which should be part of SQL string
				// For binding, we use current UTC time
				query.bind(chrono::Utc::now())
			}
		})
	}

	fn convert_row(mysql_row: MySqlRow) -> Result<Row> {
		let mut row = Row::new();
		for column in mysql_row.columns() {
			let column_name = column.name();
			if mysql_row
				.try_get_raw(column_name)
				.map_err(map_sqlx_error)?
				.is_null()
			{
				row.insert(column_name.to_string(), QueryValue::Null);
				continue;
			}
			let type_name = column.type_info().name().to_uppercase();
			if type_name == "JSON" {
				match mysql_row.try_get::<Option<serde_json::Value>, _>(column_name) {
					Ok(Some(value)) => row.insert(
						column_name.to_string(),
						QueryValue::Json(Some(Box::new(value))),
					),
					Ok(None) => row.insert(column_name.to_string(), QueryValue::Null),
					Err(error) => return Err(map_sqlx_error(error).into()),
				};
				continue;
			}
			if matches!(type_name.as_str(), "DECIMAL" | "NEWDECIMAL") {
				match mysql_row.try_get::<Option<rust_decimal::Decimal>, _>(column_name) {
					Ok(Some(value)) => {
						row.insert(
							column_name.to_string(),
							QueryValue::String(value.to_string()),
						);
					}
					Ok(None) => row.insert(column_name.to_string(), QueryValue::Null),
					Err(error) => return Err(map_sqlx_error(error).into()),
				};
				continue;
			}
			if is_boolean_type(&type_name)
				&& let Ok(value) = mysql_row.try_get::<bool, _>(column_name)
			{
				row.insert(column_name.to_string(), QueryValue::Bool(value));
			} else if let Ok(value) = mysql_row.try_get::<u64, _>(column_name) {
				let value = match i64::try_from(value) {
					Ok(value) => QueryValue::Int(value),
					Err(_) => QueryValue::String(value.to_string()),
				};
				row.insert(column_name.to_string(), value);
			} else if let Ok(value) = mysql_row.try_get::<i64, _>(column_name) {
				row.insert(column_name.to_string(), QueryValue::Int(value));
			} else if let Ok(value) = mysql_row.try_get::<i32, _>(column_name) {
				row.insert(column_name.to_string(), QueryValue::Int(value as i64));
			} else if let Ok(value) = mysql_row.try_get::<f64, _>(column_name) {
				row.insert(column_name.to_string(), QueryValue::Float(value));
			} else if let Ok(value) = mysql_row.try_get::<chrono::NaiveDate, _>(column_name) {
				row.insert(
					column_name.to_string(),
					QueryValue::String(value.to_string()),
				);
			} else if let Ok(value) = mysql_row.try_get::<chrono::NaiveTime, _>(column_name) {
				row.insert(
					column_name.to_string(),
					QueryValue::String(value.to_string()),
				);
			} else if let Ok(value) = mysql_row.try_get::<String, _>(column_name) {
				row.insert(column_name.to_string(), QueryValue::String(value));
			} else if let Ok(value) = mysql_row.try_get::<Vec<u8>, _>(column_name) {
				// MySQL 8.0 information_schema returns binary collation columns that
				// sqlx reports as LONGBLOB. Attempt UTF-8 conversion to recover string data.
				match String::from_utf8(value.clone()) {
					Ok(s) => row.insert(column_name.to_string(), QueryValue::String(s)),
					Err(_) => row.insert(column_name.to_string(), QueryValue::Bytes(value)),
				};
			} else if let Ok(value) = mysql_row.try_get::<chrono::NaiveDateTime, _>(column_name) {
				// MySQL DATETIME without timezone
				row.insert(column_name.to_string(), QueryValue::NaiveTimestamp(value));
			} else if let Ok(value) =
				mysql_row.try_get::<chrono::DateTime<chrono::Utc>, _>(column_name)
			{
				row.insert(column_name.to_string(), QueryValue::Timestamp(value));
			} else if mysql_row.try_get::<Option<i32>, _>(column_name).is_ok() {
				row.insert(column_name.to_string(), QueryValue::Null);
			}
		}
		Ok(row)
	}
}

#[async_trait]
impl DatabaseBackend for MySqlBackend {
	fn database_type(&self) -> DatabaseType {
		DatabaseType::Mysql
	}

	fn supports_row_streaming(&self) -> bool {
		true
	}

	fn placeholder(&self, _index: usize) -> String {
		"?".to_string()
	}

	fn supports_returning(&self) -> bool {
		false
	}

	fn supports_on_conflict(&self) -> bool {
		false
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
		let last_insert_id = result.last_insert_id();
		Ok(QueryResult {
			rows_affected: result.rows_affected(),
			last_insert_id: optional_last_insert_id(last_insert_id),
		})
	}

	async fn fetch_one(&self, sql: &str, params: Vec<QueryValue>) -> Result<Row> {
		let mut query = sqlx::query(sql);
		for param in &params {
			query = Self::bind_value(query, param)?;
		}
		let mysql_row = query
			.fetch_one(self.pool.as_ref())
			.await
			.map_err(map_sqlx_error)?;
		Self::convert_row(mysql_row)
	}

	async fn fetch_all(&self, sql: &str, params: Vec<QueryValue>) -> Result<Vec<Row>> {
		let mut query = sqlx::query(sql);
		for param in &params {
			query = Self::bind_value(query, param)?;
		}
		let mysql_rows = query
			.fetch_all(self.pool.as_ref())
			.await
			.map_err(map_sqlx_error)?;
		mysql_rows.into_iter().map(Self::convert_row).collect()
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
		let mysql_row = query
			.fetch_optional(self.pool.as_ref())
			.await
			.map_err(map_sqlx_error)?;
		mysql_row.map(Self::convert_row).transpose()
	}

	async fn begin(&self) -> Result<Box<dyn TransactionExecutor>> {
		let tx = self.pool.begin().await.map_err(map_sqlx_error)?;
		Ok(Box::new(MySqlTransactionExecutor::new(tx)))
	}

	async fn begin_write(&self) -> Result<Box<dyn TransactionExecutor>> {
		// READ COMMITTED avoids InnoDB next-key gap locks for missing-row
		// SELECT ... FOR UPDATE lookups. Unique constraints still serialize
		// competing inserts, after which the loser can lock and update the winner.
		self.begin_with_isolation(IsolationLevel::ReadCommitted)
			.await
	}

	async fn begin_with_isolation(
		&self,
		isolation_level: IsolationLevel,
	) -> Result<Box<dyn TransactionExecutor>> {
		// MySQL requires SET TRANSACTION ISOLATION LEVEL before BEGIN.
		// We must set isolation and start the transaction on the same connection
		// to avoid a race condition where they run on different pool connections.
		//
		// Strategy: acquire a connection, set isolation level, send BEGIN manually,
		// then wrap the connection in a raw transaction executor that manages
		// COMMIT/ROLLBACK explicitly.
		let conn = self.pool.acquire().await.map_err(map_sqlx_error)?;
		let mut conn = CloseOnDropGuard::new(conn);

		let set_sql = format!(
			"SET TRANSACTION ISOLATION LEVEL {}",
			isolation_level.to_sql(DatabaseType::Mysql)
		);
		// MySQL rejects transaction-control statements sent through its prepared protocol.
		Executor::execute(&mut **conn.connection_mut(), sqlx::raw_sql(&set_sql))
			.await
			.map_err(map_sqlx_error)?;
		Executor::execute(&mut **conn.connection_mut(), sqlx::raw_sql("BEGIN"))
			.await
			.map_err(map_sqlx_error)?;

		Ok(Box::new(MySqlRawTransactionExecutor::new(conn)))
	}

	fn as_any(&self) -> &dyn std::any::Any {
		self
	}
}

/// MySQL transaction executor
pub struct MySqlTransactionExecutor {
	tx: Option<Transaction<'static, MySql>>,
}

impl MySqlTransactionExecutor {
	/// Creates a new MySQL transaction executor.
	pub fn new(tx: Transaction<'static, MySql>) -> Self {
		Self { tx: Some(tx) }
	}

	fn bind_value<'q>(
		query: sqlx::query::Query<'q, sqlx::MySql, sqlx::mysql::MySqlArguments>,
		value: &'q QueryValue,
	) -> Result<sqlx::query::Query<'q, sqlx::MySql, sqlx::mysql::MySqlArguments>> {
		Ok(match value {
			QueryValue::Null => query.bind(None::<i32>),
			QueryValue::Bool(b) => query.bind(b),
			QueryValue::Int(i) => query.bind(i),
			QueryValue::Float(f) => query.bind(f),
			QueryValue::String(s) => query.bind(s),
			QueryValue::Bytes(b) => query.bind(b),
			QueryValue::Timestamp(dt) => query.bind(dt),
			QueryValue::NaiveTimestamp(dt) => query.bind(dt),
			// MySQL stores UUIDs as BINARY(16) or CHAR(36); we bind as string
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

	fn convert_row(mysql_row: MySqlRow) -> Result<Row> {
		MySqlBackend::convert_row(mysql_row)
	}
}

#[async_trait]
impl TransactionExecutor for MySqlTransactionExecutor {
	fn backend(&self) -> DatabaseType {
		DatabaseType::Mysql
	}

	async fn execute(&mut self, sql: &str, params: Vec<QueryValue>) -> Result<QueryResult> {
		let tx = self.tx.as_mut().ok_or_else(transaction_consumed_error)?;

		let mut query = sqlx::query(sql);
		for param in &params {
			query = Self::bind_value(query, param)?;
		}
		let result = query.execute(&mut **tx).await.map_err(map_sqlx_error)?;
		let last_insert_id = result.last_insert_id();
		Ok(QueryResult {
			rows_affected: result.rows_affected(),
			last_insert_id: optional_last_insert_id(last_insert_id),
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
		let sql = mysql_savepoint_sql(name);
		let tx = self.tx.as_mut().ok_or_else(transaction_consumed_error)?;

		Executor::execute(&mut **tx, sqlx::raw_sql(&sql))
			.await
			.map_err(map_sqlx_error)?;
		Ok(())
	}

	async fn release_savepoint(&mut self, name: &str) -> Result<()> {
		let sql = mysql_release_savepoint_sql(name);
		let tx = self.tx.as_mut().ok_or_else(transaction_consumed_error)?;

		Executor::execute(&mut **tx, sqlx::raw_sql(&sql))
			.await
			.map_err(map_sqlx_error)?;
		Ok(())
	}

	async fn rollback_to_savepoint(&mut self, name: &str) -> Result<()> {
		let sql = mysql_rollback_to_savepoint_sql(name);
		let tx = self.tx.as_mut().ok_or_else(transaction_consumed_error)?;

		Executor::execute(&mut **tx, sqlx::raw_sql(&sql))
			.await
			.map_err(map_sqlx_error)?;
		Ok(())
	}
}

/// Marks a connection to close instead of returning it to its pool when dropped.
trait CloseOnDrop {
	fn mark_for_close_on_drop(&mut self);
}

impl CloseOnDrop for sqlx::pool::PoolConnection<MySql> {
	fn mark_for_close_on_drop(&mut self) {
		self.close_on_drop();
	}
}

/// Keeps a manually started transaction connection safe while an operation awaits.
///
/// The guard is armed until successful finalization disarms it. Dropping it on
/// an error path or when the future is cancelled marks the connection to close
/// instead of returning it to the pool with an active transaction.
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
			.expect("an armed finalization guard must own its connection")
	}

	fn disarm(mut self) -> T {
		self.close_on_drop = false;
		self.connection
			.take()
			.expect("an armed finalization guard must own its connection")
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

/// MySQL raw transaction executor for isolation-level-aware transactions.
///
/// Unlike `MySqlTransactionExecutor` which wraps sqlx's `Transaction`, this type
/// manages a manually-started transaction on a pool connection. This is necessary
/// when we need to execute `SET TRANSACTION ISOLATION LEVEL` before `BEGIN` on
/// the same connection to avoid the race condition with connection pools.
struct MySqlRawTransactionExecutor {
	conn: Option<CloseOnDropGuard<sqlx::pool::PoolConnection<MySql>>>,
}

impl MySqlRawTransactionExecutor {
	/// Creates a new raw transaction executor wrapping a pool connection
	/// that already has an active transaction (BEGIN was sent manually).
	fn new(conn: CloseOnDropGuard<sqlx::pool::PoolConnection<MySql>>) -> Self {
		Self { conn: Some(conn) }
	}

	fn connection_mut(&mut self) -> Result<&mut sqlx::pool::PoolConnection<MySql>> {
		let conn = self.conn.as_mut().ok_or_else(transaction_consumed_error)?;
		Ok(conn.connection_mut())
	}

	fn bind_value<'q>(
		query: sqlx::query::Query<'q, sqlx::MySql, sqlx::mysql::MySqlArguments>,
		value: &'q QueryValue,
	) -> Result<sqlx::query::Query<'q, sqlx::MySql, sqlx::mysql::MySqlArguments>> {
		MySqlTransactionExecutor::bind_value(query, value)
	}

	fn convert_row(mysql_row: MySqlRow) -> Result<Row> {
		MySqlTransactionExecutor::convert_row(mysql_row)
	}
}

impl Drop for MySqlRawTransactionExecutor {
	fn drop(&mut self) {
		if let Some(conn) = self.conn.as_mut() {
			// A manually issued BEGIN must never return an active connection to the pool.
			conn.mark_for_close_on_drop();
		}
	}
}

#[async_trait]
impl TransactionExecutor for MySqlRawTransactionExecutor {
	fn backend(&self) -> DatabaseType {
		DatabaseType::Mysql
	}

	async fn execute(&mut self, sql: &str, params: Vec<QueryValue>) -> Result<QueryResult> {
		let conn = self.connection_mut()?;

		let mut query = sqlx::query(sql);
		for param in &params {
			query = Self::bind_value(query, param)?;
		}
		let result = query.execute(&mut **conn).await.map_err(map_sqlx_error)?;
		let last_insert_id = result.last_insert_id();
		Ok(QueryResult {
			rows_affected: result.rows_affected(),
			last_insert_id: optional_last_insert_id(last_insert_id),
		})
	}

	async fn fetch_one(&mut self, sql: &str, params: Vec<QueryValue>) -> Result<Row> {
		let conn = self.connection_mut()?;

		let mut query = sqlx::query(sql);
		for param in &params {
			query = Self::bind_value(query, param)?;
		}
		let row = query.fetch_one(&mut **conn).await.map_err(map_sqlx_error)?;
		Self::convert_row(row)
	}

	async fn fetch_all(&mut self, sql: &str, params: Vec<QueryValue>) -> Result<Vec<Row>> {
		let conn = self.connection_mut()?;

		let mut query = sqlx::query(sql);
		for param in &params {
			query = Self::bind_value(query, param)?;
		}
		let rows = query.fetch_all(&mut **conn).await.map_err(map_sqlx_error)?;
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
		let conn = self.connection_mut()?;
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
			let rows = query.fetch(&mut **conn);
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
		let conn = self.connection_mut()?;

		let mut query = sqlx::query(sql);
		for param in &params {
			query = Self::bind_value(query, param)?;
		}
		let row = query
			.fetch_optional(&mut **conn)
			.await
			.map_err(map_sqlx_error)?;
		row.map(Self::convert_row).transpose()
	}

	async fn commit(mut self: Box<Self>) -> Result<()> {
		let mut conn = self.conn.take().ok_or_else(transaction_consumed_error)?;
		let result = sqlx::query("COMMIT")
			.execute(&mut **conn.connection_mut())
			.await;
		match result {
			Ok(_) => {
				let connection = conn.disarm();
				drop(connection);
				Ok(())
			}
			Err(error) => Err(map_sqlx_error(error).into()),
		}
	}

	async fn rollback(mut self: Box<Self>) -> Result<()> {
		let mut conn = self.conn.take().ok_or_else(transaction_consumed_error)?;
		let result =
			Executor::execute(&mut **conn.connection_mut(), sqlx::raw_sql("ROLLBACK")).await;
		match result {
			Ok(_) => {
				let connection = conn.disarm();
				drop(connection);
				Ok(())
			}
			Err(error) => Err(map_sqlx_error(error).into()),
		}
	}

	async fn savepoint(&mut self, name: &str) -> Result<()> {
		let sql = mysql_savepoint_sql(name);
		let conn = self.connection_mut()?;

		Executor::execute(&mut **conn, sqlx::raw_sql(&sql))
			.await
			.map_err(map_sqlx_error)?;
		Ok(())
	}

	async fn release_savepoint(&mut self, name: &str) -> Result<()> {
		let sql = mysql_release_savepoint_sql(name);
		let conn = self.connection_mut()?;

		Executor::execute(&mut **conn, sqlx::raw_sql(&sql))
			.await
			.map_err(map_sqlx_error)?;
		Ok(())
	}

	async fn rollback_to_savepoint(&mut self, name: &str) -> Result<()> {
		let sql = mysql_rollback_to_savepoint_sql(name);
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
		CloseOnDrop, CloseOnDropGuard, MySqlBackend, MySqlRawTransactionExecutor,
		MySqlTransactionExecutor, is_boolean_type, mysql_release_savepoint_sql,
		mysql_rollback_to_savepoint_sql, mysql_savepoint_sql, optional_last_insert_id,
	};
	use crate::backends::types::{DatabaseType, QueryValue, TransactionExecutor};
	use std::sync::Arc;
	use std::sync::atomic::{AtomicUsize, Ordering};

	struct TestCloseOnDropConnection {
		close_on_drop_calls: Arc<AtomicUsize>,
	}

	impl CloseOnDrop for TestCloseOnDropConnection {
		fn mark_for_close_on_drop(&mut self) {
			self.close_on_drop_calls.fetch_add(1, Ordering::SeqCst);
		}
	}

	#[test]
	fn test_close_on_drop_guard_marks_dropped_finalization_connection() {
		let close_on_drop_calls = Arc::new(AtomicUsize::new(0));

		{
			let _guard = CloseOnDropGuard::new(TestCloseOnDropConnection {
				close_on_drop_calls: Arc::clone(&close_on_drop_calls),
			});
		}

		assert_eq!(close_on_drop_calls.load(Ordering::SeqCst), 1);
	}

	#[test]
	fn test_close_on_drop_guard_disarms_successful_finalization_connection() {
		let close_on_drop_calls = Arc::new(AtomicUsize::new(0));
		let guard = CloseOnDropGuard::new(TestCloseOnDropConnection {
			close_on_drop_calls: Arc::clone(&close_on_drop_calls),
		});

		let connection = guard.disarm();
		drop(connection);

		assert_eq!(close_on_drop_calls.load(Ordering::SeqCst), 0);
	}

	#[test]
	fn test_transaction_executors_report_mysql_backend() {
		let transaction_executor = MySqlTransactionExecutor { tx: None };
		let raw_transaction_executor = MySqlRawTransactionExecutor { conn: None };

		assert_eq!(transaction_executor.backend(), DatabaseType::Mysql);
		assert_eq!(raw_transaction_executor.backend(), DatabaseType::Mysql);
	}

	#[test]
	#[cfg(feature = "pgvector")]
	fn mysql_rejects_vector_parameters_without_a_fallback_encoding() {
		let error = MySqlBackend::bind_value(
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
				.contains("not supported by the MySQL backend")
		);
	}

	#[test]
	fn test_optional_last_insert_id_maps_zero_and_nonzero_values() {
		assert_eq!(optional_last_insert_id(0), None);
		assert_eq!(optional_last_insert_id(42), Some(42));
	}

	#[test]
	fn test_boolean_type_recognizes_mysql_aliases_without_matching_other_integers() {
		assert!(is_boolean_type("BOOLEAN"));
		assert!(is_boolean_type("BOOL"));
		assert!(is_boolean_type("TINYINT(1)"));
		assert!(!is_boolean_type("TINYINT"));
		assert!(!is_boolean_type("TINYINT UNSIGNED"));
	}

	#[test]
	fn test_savepoint_sql_uses_mysql_quoted_validated_identifiers() {
		assert_eq!(
			mysql_savepoint_sql("reinhardt_atomic_0"),
			"SAVEPOINT `reinhardt_atomic_0`"
		);
		assert_eq!(
			mysql_release_savepoint_sql("reinhardt_atomic_0"),
			"RELEASE SAVEPOINT `reinhardt_atomic_0`"
		);
		assert_eq!(
			mysql_rollback_to_savepoint_sql("reinhardt_atomic_0"),
			"ROLLBACK TO SAVEPOINT `reinhardt_atomic_0`"
		);
	}
}
