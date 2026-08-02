//! Common type definitions for database abstraction

use super::error::{DatabaseError, DatabaseErrorKind};
use futures::{Stream, StreamExt};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::pin::Pin;
use uuid::Uuid;

/// A lifetime-bound stream of database rows.
///
/// Dropping the stream releases any driver cursor, transaction borrow, or pool
/// connection retained by the backend.
pub type RowStream<'a> = Pin<Box<dyn Stream<Item = super::error::Result<Row>> + Send + 'a>>;

/// Database type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DatabaseType {
	/// Postgres variant.
	Postgres,
	/// Sqlite variant.
	Sqlite,
	/// Mysql variant.
	Mysql,
}

impl DatabaseType {
	/// Check if this database type supports transactional DDL
	///
	/// Transactional DDL means that DDL statements (CREATE TABLE, ALTER TABLE, etc.)
	/// can be rolled back if the transaction fails.
	///
	/// - PostgreSQL: Supports transactional DDL
	/// - SQLite: Supports transactional DDL
	/// - MySQL/MariaDB: Does NOT support transactional DDL (DDL causes implicit commit)
	/// - MongoDB: Not applicable (schemaless)
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::backends::types::DatabaseType;
	///
	/// assert!(DatabaseType::Postgres.supports_transactional_ddl());
	/// assert!(DatabaseType::Sqlite.supports_transactional_ddl());
	/// assert!(!DatabaseType::Mysql.supports_transactional_ddl());
	/// ```
	pub fn supports_transactional_ddl(&self) -> bool {
		matches!(self, DatabaseType::Postgres | DatabaseType::Sqlite)
	}
}

/// Query value types
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum QueryValue {
	/// Null variant.
	Null,
	/// Bool variant.
	Bool(bool),
	/// Int variant.
	Int(i64),
	/// Float variant.
	Float(f64),
	/// String variant.
	String(String),
	/// Bytes variant.
	Bytes(Vec<u8>),
	/// Timestamp variant.
	Timestamp(chrono::DateTime<chrono::Utc>),
	/// Timezone-naive timestamp variant.
	NaiveTimestamp(chrono::NaiveDateTime),
	/// UUID value for PostgreSQL uuid columns
	Uuid(Uuid),
	/// JSON value, preserving the distinction between JSON null and SQL NULL.
	Json(Option<Box<serde_json::Value>>),
	/// Native PostgreSQL dense-vector parameter, including a type-preserving SQL NULL.
	#[cfg(feature = "pgvector")]
	Vector(Option<Vec<f32>>),
	/// PostgreSQL-compatible string array parameter.
	StringArray(Vec<String>),
	/// PostgreSQL-compatible 32-bit integer array parameter.
	IntArray(Vec<i32>),
	/// PostgreSQL-compatible 64-bit integer array parameter.
	BigIntArray(Vec<i64>),
	/// PostgreSQL-compatible boolean array parameter.
	BoolArray(Vec<bool>),
	/// PostgreSQL-compatible 32-bit floating-point array parameter.
	FloatArray(Vec<f32>),
	/// PostgreSQL-compatible 64-bit floating-point array parameter.
	DoubleArray(Vec<f64>),
	/// PostgreSQL-compatible UUID array parameter.
	UuidArray(Vec<Uuid>),
	/// Represents SQL NOW() function
	Now,
}

impl From<&str> for QueryValue {
	fn from(s: &str) -> Self {
		QueryValue::String(s.to_string())
	}
}

impl From<String> for QueryValue {
	fn from(s: String) -> Self {
		QueryValue::String(s)
	}
}

impl From<i64> for QueryValue {
	fn from(i: i64) -> Self {
		QueryValue::Int(i)
	}
}

impl From<i32> for QueryValue {
	fn from(i: i32) -> Self {
		QueryValue::Int(i as i64)
	}
}

impl From<f64> for QueryValue {
	fn from(f: f64) -> Self {
		QueryValue::Float(f)
	}
}

impl From<bool> for QueryValue {
	fn from(b: bool) -> Self {
		QueryValue::Bool(b)
	}
}

impl From<chrono::DateTime<chrono::Utc>> for QueryValue {
	fn from(dt: chrono::DateTime<chrono::Utc>) -> Self {
		QueryValue::Timestamp(dt)
	}
}

impl From<chrono::NaiveDateTime> for QueryValue {
	fn from(dt: chrono::NaiveDateTime) -> Self {
		QueryValue::NaiveTimestamp(dt)
	}
}

impl From<Uuid> for QueryValue {
	fn from(u: Uuid) -> Self {
		QueryValue::Uuid(u)
	}
}

/// Query result
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryResult {
	/// The rows affected.
	pub rows_affected: u64,
	/// The generated ID returned by this exact insert operation, if available.
	pub last_insert_id: Option<u64>,
}

/// Row from query result
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
	/// The data.
	pub data: HashMap<String, QueryValue>,
}

impl Row {
	/// Creates a new instance.
	pub fn new() -> Self {
		Self {
			data: HashMap::new(),
		}
	}

	/// Performs the insert operation.
	pub fn insert(&mut self, key: String, value: QueryValue) {
		self.data.insert(key, value);
	}

	/// Performs the get operation.
	pub fn get<T: TryFrom<QueryValue>>(&self, key: &str) -> std::result::Result<T, DatabaseError>
	where
		DatabaseError: From<<T as TryFrom<QueryValue>>::Error>,
	{
		self.data
			.get(key)
			.cloned()
			.ok_or_else(|| DatabaseError::new(DatabaseErrorKind::ColumnNotFound, key.to_string()))
			.and_then(|v| v.try_into().map_err(Into::into))
	}
}

impl Default for Row {
	fn default() -> Self {
		Self::new()
	}
}

// Type conversions for QueryValue
impl TryFrom<QueryValue> for i64 {
	type Error = DatabaseError;

	fn try_from(value: QueryValue) -> std::result::Result<Self, Self::Error> {
		match value {
			QueryValue::Int(i) => Ok(i),
			_ => Err(DatabaseError::new(
				DatabaseErrorKind::Type,
				format!("Cannot convert {:?} to i64", value),
			)),
		}
	}
}

impl TryFrom<QueryValue> for i32 {
	type Error = DatabaseError;

	fn try_from(value: QueryValue) -> std::result::Result<Self, Self::Error> {
		match value {
			QueryValue::Int(i) => i32::try_from(i).map_err(|_| {
				DatabaseError::new(
					DatabaseErrorKind::Type,
					format!("Value {} out of range for i32", i),
				)
			}),
			_ => Err(DatabaseError::new(
				DatabaseErrorKind::Type,
				format!("Cannot convert {:?} to i32", value),
			)),
		}
	}
}

impl TryFrom<QueryValue> for u64 {
	type Error = DatabaseError;

	fn try_from(value: QueryValue) -> std::result::Result<Self, Self::Error> {
		match value {
			QueryValue::Int(i) => u64::try_from(i).map_err(|_| {
				DatabaseError::new(
					DatabaseErrorKind::Type,
					format!("Value {} out of range for u64", i),
				)
			}),
			_ => Err(DatabaseError::new(
				DatabaseErrorKind::Type,
				format!("Cannot convert {:?} to u64", value),
			)),
		}
	}
}

impl TryFrom<QueryValue> for u32 {
	type Error = DatabaseError;

	fn try_from(value: QueryValue) -> std::result::Result<Self, Self::Error> {
		match value {
			QueryValue::Int(i) => u32::try_from(i).map_err(|_| {
				DatabaseError::new(
					DatabaseErrorKind::Type,
					format!("Value {} out of range for u32", i),
				)
			}),
			_ => Err(DatabaseError::new(
				DatabaseErrorKind::Type,
				format!("Cannot convert {:?} to u32", value),
			)),
		}
	}
}

impl TryFrom<QueryValue> for String {
	type Error = DatabaseError;

	fn try_from(value: QueryValue) -> std::result::Result<Self, Self::Error> {
		match value {
			QueryValue::String(s) => Ok(s),
			_ => Err(DatabaseError::new(
				DatabaseErrorKind::Type,
				format!("Cannot convert {:?} to String", value),
			)),
		}
	}
}

impl TryFrom<QueryValue> for bool {
	type Error = DatabaseError;

	fn try_from(value: QueryValue) -> std::result::Result<Self, Self::Error> {
		match value {
			QueryValue::Bool(b) => Ok(b),
			_ => Err(DatabaseError::new(
				DatabaseErrorKind::Type,
				format!("Cannot convert {:?} to bool", value),
			)),
		}
	}
}

impl TryFrom<QueryValue> for f64 {
	type Error = DatabaseError;

	fn try_from(value: QueryValue) -> std::result::Result<Self, Self::Error> {
		match value {
			QueryValue::Float(f) => Ok(f),
			_ => Err(DatabaseError::new(
				DatabaseErrorKind::Type,
				format!("Cannot convert {:?} to f64", value),
			)),
		}
	}
}

impl TryFrom<QueryValue> for chrono::DateTime<chrono::Utc> {
	type Error = DatabaseError;

	fn try_from(value: QueryValue) -> std::result::Result<Self, Self::Error> {
		match value {
			QueryValue::Timestamp(dt) => Ok(dt),
			_ => Err(DatabaseError::new(
				DatabaseErrorKind::Type,
				format!("Cannot convert {:?} to DateTime<Utc>", value),
			)),
		}
	}
}

impl TryFrom<QueryValue> for chrono::NaiveDateTime {
	type Error = DatabaseError;

	fn try_from(value: QueryValue) -> std::result::Result<Self, Self::Error> {
		match value {
			QueryValue::NaiveTimestamp(dt) => Ok(dt),
			_ => Err(DatabaseError::new(
				DatabaseErrorKind::Type,
				format!("Cannot convert {:?} to NaiveDateTime", value),
			)),
		}
	}
}

impl TryFrom<QueryValue> for Uuid {
	type Error = DatabaseError;

	fn try_from(value: QueryValue) -> std::result::Result<Self, Self::Error> {
		match value {
			QueryValue::Uuid(u) => Ok(u),
			QueryValue::String(s) => Uuid::parse_str(&s).map_err(|_| {
				DatabaseError::new(
					DatabaseErrorKind::Type,
					format!("Invalid UUID string: {}", s),
				)
			}),
			_ => Err(DatabaseError::new(
				DatabaseErrorKind::Type,
				format!("Cannot convert {:?} to Uuid", value),
			)),
		}
	}
}

/// Transaction isolation levels for controlling database concurrency behavior
///
/// These isolation levels follow the SQL standard and are supported by most
/// relational databases, though implementation details may vary.
///
/// # Examples
///
/// ```
/// use reinhardt_db::backends::types::{IsolationLevel, DatabaseType};
///
/// let level = IsolationLevel::Serializable;
/// let sql = level.to_sql(DatabaseType::Postgres);
/// assert!(sql.contains("SERIALIZABLE"));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum IsolationLevel {
	/// Allows dirty reads, non-repeatable reads, and phantom reads.
	/// Lowest isolation level with highest concurrency.
	ReadUncommitted,
	/// Prevents dirty reads but allows non-repeatable reads and phantom reads.
	/// This is the default isolation level for most databases.
	#[default]
	ReadCommitted,
	/// Prevents dirty reads and non-repeatable reads but allows phantom reads.
	RepeatableRead,
	/// Highest isolation level. Prevents dirty reads, non-repeatable reads,
	/// and phantom reads. Transactions are fully serializable.
	Serializable,
}

impl IsolationLevel {
	/// Convert the isolation level to SQL syntax for the given database type
	///
	/// # Arguments
	///
	/// * `db_type` - The target database type
	///
	/// # Returns
	///
	/// SQL string representation suitable for SET TRANSACTION or BEGIN statements
	pub fn to_sql(&self, db_type: DatabaseType) -> &'static str {
		match (self, db_type) {
			// PostgreSQL, MySQL, and SQLite all use similar syntax
			(IsolationLevel::ReadUncommitted, _) => "READ UNCOMMITTED",
			(IsolationLevel::ReadCommitted, _) => "READ COMMITTED",
			(IsolationLevel::RepeatableRead, _) => "REPEATABLE READ",
			(IsolationLevel::Serializable, _) => "SERIALIZABLE",
		}
	}

	/// Generate the SQL statement to begin a transaction with this isolation level
	///
	/// # Arguments
	///
	/// * `db_type` - The target database type
	///
	/// # Returns
	///
	/// Complete SQL statement to begin a transaction with the specified isolation level
	pub fn begin_transaction_sql(&self, db_type: DatabaseType) -> String {
		match db_type {
			DatabaseType::Postgres => {
				format!("BEGIN ISOLATION LEVEL {}", self.to_sql(db_type))
			}
			DatabaseType::Mysql => {
				// MySQL requires SET TRANSACTION before START TRANSACTION
				format!(
					"SET TRANSACTION ISOLATION LEVEL {}; START TRANSACTION",
					self.to_sql(db_type)
				)
			}
			DatabaseType::Sqlite => {
				// SQLite only supports DEFERRED, IMMEDIATE, or EXCLUSIVE
				// We map Serializable to EXCLUSIVE, others to default behavior
				match self {
					IsolationLevel::Serializable => "BEGIN EXCLUSIVE".to_string(),
					_ => "BEGIN".to_string(),
				}
			}
		}
	}
}

/// Savepoint for nested transaction support
///
/// Savepoints allow creating checkpoint within a transaction that can be
/// rolled back to without affecting the entire transaction.
///
/// # Examples
///
/// ```
/// use reinhardt_db::backends::types::Savepoint;
///
/// let sp = Savepoint::new("sp1");
/// assert_eq!(sp.to_sql(), "SAVEPOINT \"sp1\"");
/// assert_eq!(sp.release_sql(), "RELEASE SAVEPOINT \"sp1\"");
/// assert_eq!(sp.rollback_sql(), "ROLLBACK TO SAVEPOINT \"sp1\"");
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Savepoint {
	/// The name of the savepoint
	name: String,
}

impl Savepoint {
	/// Create a new savepoint with the given name.
	///
	/// # Panics
	///
	/// Panics if the name contains invalid characters. Only alphanumeric
	/// characters and underscores are allowed (must not start with a digit).
	pub fn new(name: impl Into<String>) -> Self {
		let name = name.into();
		validate_savepoint_name(&name).unwrap_or_else(|e| panic!("Invalid savepoint name: {}", e));
		Self { name }
	}

	/// Get the savepoint name
	pub fn name(&self) -> &str {
		&self.name
	}

	/// Generate SQL to create this savepoint
	pub fn to_sql(&self) -> String {
		format!("SAVEPOINT \"{}\"", self.name.replace('"', "\"\""))
	}

	/// Generate SQL to release (commit) this savepoint
	pub fn release_sql(&self) -> String {
		format!("RELEASE SAVEPOINT \"{}\"", self.name.replace('"', "\"\""))
	}

	/// Generate SQL to rollback to this savepoint
	pub fn rollback_sql(&self) -> String {
		format!(
			"ROLLBACK TO SAVEPOINT \"{}\"",
			self.name.replace('"', "\"\"")
		)
	}
}

/// Validate a savepoint name to prevent SQL injection.
///
/// Only alphanumeric characters and underscores are allowed.
/// The name must not be empty and must not start with a digit.
fn validate_savepoint_name(name: &str) -> Result<(), String> {
	if name.is_empty() {
		return Err("Savepoint name cannot be empty".to_string());
	}

	if !name.chars().all(|c| c.is_alphanumeric() || c == '_') {
		return Err(format!(
			"Savepoint name '{}' contains invalid characters. Only alphanumeric characters and underscores are allowed",
			name
		));
	}

	if let Some(first_char) = name.chars().next()
		&& first_char.is_numeric()
	{
		return Err(format!(
			"Savepoint name '{}' cannot start with a number",
			name
		));
	}

	Ok(())
}

/// Transaction executor trait for database-specific transaction handling
///
/// This trait represents a dedicated database connection that is used for
/// transaction operations. All queries executed through this executor
/// are guaranteed to run on the same physical connection, ensuring
/// proper transaction isolation.
///
/// # Implementation Notes
///
/// SQLx connection pools distribute queries across multiple connections.
/// To ensure transaction consistency, we need to acquire a dedicated
/// connection via `pool.begin()` which returns a `Transaction` that
/// maintains connection affinity.
#[async_trait::async_trait]
pub trait TransactionExecutor: Send + Sync {
	/// Return the database backend used by this transaction executor.
	///
	/// PostgreSQL is retained as the compatibility default for executors that
	/// predate backend reporting.
	fn backend(&self) -> DatabaseType {
		DatabaseType::Postgres
	}

	/// Returns whether this PostgreSQL-protocol executor targets CockroachDB.
	fn is_cockroachdb(&self) -> bool {
		false
	}

	/// Reports the row-locking features supported by this server.
	///
	/// Executors connected to older server versions should override this method.
	/// PostgreSQL gained `NO KEY UPDATE` in 9.3 and `SKIP LOCKED` in 9.5.
	/// MySQL row-lock options require 8.0.1 or newer.
	fn row_lock_capabilities(&self) -> RowLockCapabilities {
		match self.backend() {
			DatabaseType::Postgres if self.is_cockroachdb() => RowLockCapabilities::cockroachdb(),
			DatabaseType::Postgres => RowLockCapabilities::postgres(),
			DatabaseType::Mysql => RowLockCapabilities::mysql(),
			DatabaseType::Sqlite => RowLockCapabilities::unsupported(),
		}
	}

	/// Returns whether contextual pgvector error hints are supported.
	fn supports_pgvector_error_hints(&self) -> bool {
		false
	}

	/// Execute a query that modifies the database within the transaction
	async fn execute(
		&mut self,
		sql: &str,
		params: Vec<QueryValue>,
	) -> super::error::Result<QueryResult>;

	/// Execute with structural pgvector operation context.
	async fn execute_with_context(
		&mut self,
		sql: &str,
		params: Vec<QueryValue>,
		context: Option<super::error::PgvectorOperationKind>,
	) -> super::error::Result<QueryResult> {
		let result = self.execute(sql, params).await;
		if self.backend() == DatabaseType::Postgres && self.supports_pgvector_error_hints() {
			result
				.map_err(|error| super::error::decorate_error_with_pgvector_context(error, context))
		} else {
			result
		}
	}

	/// Fetch a single row within the transaction
	async fn fetch_one(&mut self, sql: &str, params: Vec<QueryValue>) -> super::error::Result<Row>;

	/// Fetch one row with structural pgvector operation context.
	async fn fetch_one_with_context(
		&mut self,
		sql: &str,
		params: Vec<QueryValue>,
		context: Option<super::error::PgvectorOperationKind>,
	) -> super::error::Result<Row> {
		let result = self.fetch_one(sql, params).await;
		if self.backend() == DatabaseType::Postgres && self.supports_pgvector_error_hints() {
			result
				.map_err(|error| super::error::decorate_error_with_pgvector_context(error, context))
		} else {
			result
		}
	}

	/// Fetch all matching rows within the transaction
	async fn fetch_all(
		&mut self,
		sql: &str,
		params: Vec<QueryValue>,
	) -> super::error::Result<Vec<Row>>;

	/// Fetch rows with structural pgvector operation context.
	async fn fetch_all_with_context(
		&mut self,
		sql: &str,
		params: Vec<QueryValue>,
		context: Option<super::error::PgvectorOperationKind>,
	) -> super::error::Result<Vec<Row>> {
		let result = self.fetch_all(sql, params).await;
		if self.backend() == DatabaseType::Postgres && self.supports_pgvector_error_hints() {
			result
				.map_err(|error| super::error::decorate_error_with_pgvector_context(error, context))
		} else {
			result
		}
	}

	/// Streams matching rows without eager materialization.
	///
	/// `chunk_size` is a driver fetch or bounded-buffer hint. Implementations
	/// must not emulate streaming with repeated `LIMIT` and `OFFSET` queries.
	fn fetch_stream<'a>(
		&'a mut self,
		_sql: String,
		_params: Vec<QueryValue>,
		_chunk_size: usize,
	) -> super::error::Result<RowStream<'a>> {
		Err(DatabaseError::new(
			DatabaseErrorKind::Unsupported,
			"Row streaming is not supported by this transaction executor",
		)
		.into())
	}

	/// Streams rows with structural pgvector operation context.
	fn fetch_stream_with_context<'a>(
		&'a mut self,
		sql: String,
		params: Vec<QueryValue>,
		chunk_size: usize,
		context: Option<super::error::PgvectorOperationKind>,
	) -> super::error::Result<RowStream<'a>> {
		let decorate =
			self.backend() == DatabaseType::Postgres && self.supports_pgvector_error_hints();
		let stream = self.fetch_stream(sql, params, chunk_size)?;
		if decorate {
			Ok(Box::pin(stream.map(move |result| {
				result.map_err(|error| {
					super::error::decorate_error_with_pgvector_context(error, context)
				})
			})))
		} else {
			Ok(stream)
		}
	}

	/// Fetch an optional single row within the transaction
	async fn fetch_optional(
		&mut self,
		sql: &str,
		params: Vec<QueryValue>,
	) -> super::error::Result<Option<Row>>;

	/// Fetch an optional row with structural pgvector operation context.
	async fn fetch_optional_with_context(
		&mut self,
		sql: &str,
		params: Vec<QueryValue>,
		context: Option<super::error::PgvectorOperationKind>,
	) -> super::error::Result<Option<Row>> {
		let result = self.fetch_optional(sql, params).await;
		if self.backend() == DatabaseType::Postgres && self.supports_pgvector_error_hints() {
			result
				.map_err(|error| super::error::decorate_error_with_pgvector_context(error, context))
		} else {
			result
		}
	}

	/// Commit the transaction
	async fn commit(self: Box<Self>) -> super::error::Result<()>;

	/// Rollback the transaction
	async fn rollback(self: Box<Self>) -> super::error::Result<()>;

	/// Create a savepoint within the transaction
	///
	/// Savepoints allow creating checkpoints within a transaction that can be
	/// rolled back to without affecting the entire transaction.
	///
	/// # Arguments
	///
	/// * `name` - The name of the savepoint to create
	///
	/// # Default Implementation
	///
	/// Returns an error indicating savepoints are not supported. Backends that
	/// support savepoints should override this method.
	async fn savepoint(&mut self, name: &str) -> super::error::Result<()> {
		let _ = name;
		Err(DatabaseError::new(
			DatabaseErrorKind::Unsupported,
			"Savepoints are not supported by this backend",
		)
		.into())
	}

	/// Release (commit) a savepoint
	///
	/// Releasing a savepoint removes the checkpoint and makes the changes
	/// within it part of the enclosing transaction.
	///
	/// # Arguments
	///
	/// * `name` - The name of the savepoint to release
	///
	/// # Default Implementation
	///
	/// Returns an error indicating savepoints are not supported.
	async fn release_savepoint(&mut self, name: &str) -> super::error::Result<()> {
		let _ = name;
		Err(DatabaseError::new(
			DatabaseErrorKind::Unsupported,
			"Savepoints are not supported by this backend",
		)
		.into())
	}

	/// Rollback to a savepoint
	///
	/// Rolling back to a savepoint undoes all changes made after the savepoint
	/// was created, while keeping the transaction open.
	///
	/// # Arguments
	///
	/// * `name` - The name of the savepoint to rollback to
	///
	/// # Default Implementation
	///
	/// Returns an error indicating savepoints are not supported.
	async fn rollback_to_savepoint(&mut self, name: &str) -> super::error::Result<()> {
		let _ = name;
		Err(DatabaseError::new(
			DatabaseErrorKind::Unsupported,
			"Savepoints are not supported by this backend",
		)
		.into())
	}
}

/// Server capabilities used to validate `QuerySet` row-lock clauses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RowLockCapabilities {
	/// Whether blocking `FOR UPDATE` is supported.
	pub update: bool,
	/// Whether `FOR NO KEY UPDATE` is supported as a distinct lock strength.
	pub no_key_update: bool,
	/// Whether `NOWAIT` is supported.
	pub nowait: bool,
	/// Whether `SKIP LOCKED` is supported.
	pub skip_locked: bool,
	/// Whether an explicit lock target list is supported.
	pub targets: bool,
}

impl RowLockCapabilities {
	/// Capabilities for a PostgreSQL server with the supplied major and minor version.
	pub const fn postgres_for_version(major: u16, minor: u16) -> Self {
		let version = major * 100 + minor;
		Self {
			update: true,
			no_key_update: version >= 903,
			nowait: version >= 801,
			skip_locked: version >= 905,
			targets: true,
		}
	}

	/// Capabilities for a MySQL server with the supplied semantic version.
	pub const fn mysql_for_version(major: u16, minor: u16, patch: u16) -> Self {
		let supports_wait_options = major > 8 || (major == 8 && (minor > 0 || patch >= 1));
		Self {
			update: true,
			no_key_update: false,
			nowait: supports_wait_options,
			skip_locked: supports_wait_options,
			targets: supports_wait_options,
		}
	}

	/// Capabilities for a MariaDB server with the supplied semantic version.
	pub const fn mariadb_for_version(major: u16, minor: u16, _patch: u16) -> Self {
		let version = major * 100 + minor;
		Self {
			update: true,
			no_key_update: false,
			nowait: version >= 1003,
			skip_locked: version >= 1006,
			targets: false,
		}
	}

	/// Capabilities for PostgreSQL 9.5 and newer.
	pub const fn postgres() -> Self {
		Self::postgres_for_version(9, 5)
	}

	/// Capabilities for MySQL 8.0.1 and newer.
	pub const fn mysql() -> Self {
		Self::mysql_for_version(8, 0, 1)
	}

	/// Capabilities for the built-in CockroachDB v23.1 lock profile.
	///
	/// CockroachDB v23.1 supports `FOR UPDATE` and `NOWAIT`, but not
	/// `SKIP LOCKED` or explicit lock targets. Custom transaction executors for
	/// servers with different capabilities should override
	/// [`TransactionExecutor::row_lock_capabilities`].
	pub const fn cockroachdb() -> Self {
		Self {
			update: true,
			no_key_update: false,
			nowait: true,
			skip_locked: false,
			targets: false,
		}
	}

	/// Capabilities for a backend or server version without row locking.
	pub const fn unsupported() -> Self {
		Self {
			update: false,
			no_key_update: false,
			nowait: false,
			skip_locked: false,
			targets: false,
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	struct LegacyExecutor;

	struct ContextErrorTransactionWithoutCapability {
		backend: DatabaseType,
		supports_pgvector_error_hints: bool,
	}

	#[async_trait::async_trait]
	impl TransactionExecutor for ContextErrorTransactionWithoutCapability {
		fn backend(&self) -> DatabaseType {
			self.backend
		}

		fn supports_pgvector_error_hints(&self) -> bool {
			self.supports_pgvector_error_hints
		}

		async fn execute(
			&mut self,
			_sql: &str,
			_params: Vec<QueryValue>,
		) -> super::super::error::Result<QueryResult> {
			Err(super::super::error::DatabaseError::new(
				DatabaseErrorKind::Query,
				"operator does not exist: vector <=> vector",
			)
			.with_code("42883")
			.into())
		}

		async fn fetch_one(
			&mut self,
			_sql: &str,
			_params: Vec<QueryValue>,
		) -> super::super::error::Result<Row> {
			panic!("context default transaction test does not fetch rows")
		}

		async fn fetch_all(
			&mut self,
			_sql: &str,
			_params: Vec<QueryValue>,
		) -> super::super::error::Result<Vec<Row>> {
			panic!("context default transaction test does not fetch rows")
		}

		async fn fetch_optional(
			&mut self,
			_sql: &str,
			_params: Vec<QueryValue>,
		) -> super::super::error::Result<Option<Row>> {
			panic!("context default transaction test does not fetch rows")
		}

		async fn commit(self: Box<Self>) -> super::super::error::Result<()> {
			Ok(())
		}

		async fn rollback(self: Box<Self>) -> super::super::error::Result<()> {
			Ok(())
		}
	}

	#[async_trait::async_trait]
	impl TransactionExecutor for LegacyExecutor {
		async fn execute(
			&mut self,
			_sql: &str,
			_params: Vec<QueryValue>,
		) -> super::super::error::Result<QueryResult> {
			Err(super::super::error::DatabaseError::new(
				DatabaseErrorKind::Unsupported,
				"legacy executor does not execute test queries",
			)
			.into())
		}

		async fn fetch_one(
			&mut self,
			_sql: &str,
			_params: Vec<QueryValue>,
		) -> super::super::error::Result<Row> {
			Err(super::super::error::DatabaseError::new(
				DatabaseErrorKind::Unsupported,
				"legacy executor does not fetch test rows",
			)
			.into())
		}

		async fn fetch_all(
			&mut self,
			_sql: &str,
			_params: Vec<QueryValue>,
		) -> super::super::error::Result<Vec<Row>> {
			Err(super::super::error::DatabaseError::new(
				DatabaseErrorKind::Unsupported,
				"legacy executor does not fetch test rows",
			)
			.into())
		}

		async fn fetch_optional(
			&mut self,
			_sql: &str,
			_params: Vec<QueryValue>,
		) -> super::super::error::Result<Option<Row>> {
			Err(super::super::error::DatabaseError::new(
				DatabaseErrorKind::Unsupported,
				"legacy executor does not fetch test rows",
			)
			.into())
		}

		async fn commit(self: Box<Self>) -> super::super::error::Result<()> {
			Ok(())
		}

		async fn rollback(self: Box<Self>) -> super::super::error::Result<()> {
			Ok(())
		}
	}

	#[rstest]
	fn transaction_executor_defaults_to_postgres_for_legacy_implementations() {
		let executor = LegacyExecutor;

		assert_eq!(executor.backend(), DatabaseType::Postgres);
	}

	#[rstest]
	#[case(DatabaseType::Mysql)]
	#[case(DatabaseType::Sqlite)]
	#[case(DatabaseType::Postgres)]
	#[tokio::test]
	async fn transaction_default_without_capability_does_not_decorate_pgvector_shaped_error(
		#[case] backend: DatabaseType,
	) {
		let mut executor = ContextErrorTransactionWithoutCapability {
			backend,
			supports_pgvector_error_hints: false,
		};

		let error = executor
			.execute_with_context(
				"SELECT embedding <=> ? FROM users",
				Vec::new(),
				Some(super::super::error::PgvectorOperationKind::DistanceOperator),
			)
			.await
			.unwrap_err();

		assert_eq!(
			error.database_error().and_then(|error| error.code()),
			Some("42883")
		);
		assert!(!error.to_string().contains("CreateExtension::new"));
	}

	#[rstest]
	#[case(DatabaseType::Mysql)]
	#[case(DatabaseType::Sqlite)]
	#[tokio::test]
	async fn transaction_default_requires_postgres_even_when_capability_is_enabled(
		#[case] backend: DatabaseType,
	) {
		let mut executor = ContextErrorTransactionWithoutCapability {
			backend,
			supports_pgvector_error_hints: true,
		};

		let error = executor
			.execute_with_context(
				"SELECT embedding <=> ? FROM users",
				Vec::new(),
				Some(super::super::error::PgvectorOperationKind::DistanceOperator),
			)
			.await
			.unwrap_err();

		assert_eq!(
			error.database_error().and_then(|error| error.code()),
			Some("42883")
		);
		assert!(!error.to_string().contains("CreateExtension::new"));
	}

	// ==================== Savepoint name validation tests ====================

	#[rstest]
	fn test_savepoint_valid_name() {
		// Arrange & Act
		let sp = Savepoint::new("sp1");

		// Assert
		assert_eq!(sp.name(), "sp1");
		assert_eq!(sp.to_sql(), "SAVEPOINT \"sp1\"");
		assert_eq!(sp.release_sql(), "RELEASE SAVEPOINT \"sp1\"");
		assert_eq!(sp.rollback_sql(), "ROLLBACK TO SAVEPOINT \"sp1\"");
	}

	#[rstest]
	fn test_savepoint_valid_underscore_name() {
		// Arrange & Act
		let sp = Savepoint::new("my_savepoint_1");

		// Assert
		assert_eq!(sp.to_sql(), "SAVEPOINT \"my_savepoint_1\"");
	}

	#[rstest]
	#[should_panic(expected = "Invalid savepoint name")]
	fn test_savepoint_rejects_sql_injection_semicolon() {
		// Arrange & Act: attacker tries SQL injection with semicolon
		Savepoint::new("sp1; DROP TABLE users; --");
	}

	#[rstest]
	#[should_panic(expected = "Invalid savepoint name")]
	fn test_savepoint_rejects_sql_injection_quotes() {
		// Arrange & Act: attacker tries to break out with quotes
		Savepoint::new("sp1\" ; DROP TABLE users; --");
	}

	#[rstest]
	#[should_panic(expected = "Invalid savepoint name")]
	fn test_savepoint_rejects_empty_name() {
		// Arrange & Act
		Savepoint::new("");
	}

	#[rstest]
	#[should_panic(expected = "Invalid savepoint name")]
	fn test_savepoint_rejects_name_starting_with_number() {
		// Arrange & Act
		Savepoint::new("1invalid");
	}

	#[rstest]
	#[should_panic(expected = "Invalid savepoint name")]
	fn test_savepoint_rejects_spaces() {
		// Arrange & Act
		Savepoint::new("sp 1");
	}

	#[rstest]
	fn test_validate_savepoint_name_valid() {
		// Arrange & Act & Assert
		assert!(validate_savepoint_name("sp1").is_ok());
		assert!(validate_savepoint_name("my_savepoint").is_ok());
		assert!(validate_savepoint_name("_internal").is_ok());
	}

	#[rstest]
	fn test_validate_savepoint_name_rejects_injection() {
		// Arrange & Act & Assert
		assert!(validate_savepoint_name("sp; DROP TABLE").is_err());
		assert!(validate_savepoint_name("sp\"injection").is_err());
		assert!(validate_savepoint_name("sp' OR '1'='1").is_err());
		assert!(validate_savepoint_name("").is_err());
	}
}
