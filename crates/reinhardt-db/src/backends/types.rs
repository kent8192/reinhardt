//! Common type definitions for database abstraction

use super::error::DatabaseError;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Database type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DatabaseType {
	Postgres,
	Sqlite,
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
	Null,
	Bool(bool),
	Int(i64),
	Float(f64),
	String(String),
	Bytes(Vec<u8>),
	Timestamp(chrono::DateTime<chrono::Utc>),
	/// UUID value for PostgreSQL uuid columns
	Uuid(Uuid),
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

impl From<Uuid> for QueryValue {
	fn from(u: Uuid) -> Self {
		QueryValue::Uuid(u)
	}
}

/// Query result
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryResult {
	pub rows_affected: u64,
}

/// Row from query result
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
	pub data: HashMap<String, QueryValue>,
}

impl Row {
	pub fn new() -> Self {
		Self {
			data: HashMap::new(),
		}
	}

	pub fn insert(&mut self, key: String, value: QueryValue) {
		self.data.insert(key, value);
	}

	pub fn get<T: TryFrom<QueryValue>>(&self, key: &str) -> std::result::Result<T, DatabaseError>
	where
		DatabaseError: From<<T as TryFrom<QueryValue>>::Error>,
	{
		self.data
			.get(key)
			.cloned()
			.ok_or_else(|| DatabaseError::ColumnNotFound(key.to_string()))
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
			_ => Err(DatabaseError::TypeError(format!(
				"Cannot convert {:?} to i64",
				value
			))),
		}
	}
}

impl TryFrom<QueryValue> for i32 {
	type Error = DatabaseError;

	fn try_from(value: QueryValue) -> std::result::Result<Self, Self::Error> {
		match value {
			QueryValue::Int(i) => i32::try_from(i)
				.map_err(|_| DatabaseError::TypeError(format!("Value {} out of range for i32", i))),
			_ => Err(DatabaseError::TypeError(format!(
				"Cannot convert {:?} to i32",
				value
			))),
		}
	}
}

impl TryFrom<QueryValue> for u64 {
	type Error = DatabaseError;

	fn try_from(value: QueryValue) -> std::result::Result<Self, Self::Error> {
		match value {
			QueryValue::Int(i) => u64::try_from(i)
				.map_err(|_| DatabaseError::TypeError(format!("Value {} out of range for u64", i))),
			_ => Err(DatabaseError::TypeError(format!(
				"Cannot convert {:?} to u64",
				value
			))),
		}
	}
}

impl TryFrom<QueryValue> for u32 {
	type Error = DatabaseError;

	fn try_from(value: QueryValue) -> std::result::Result<Self, Self::Error> {
		match value {
			QueryValue::Int(i) => u32::try_from(i)
				.map_err(|_| DatabaseError::TypeError(format!("Value {} out of range for u32", i))),
			_ => Err(DatabaseError::TypeError(format!(
				"Cannot convert {:?} to u32",
				value
			))),
		}
	}
}

impl TryFrom<QueryValue> for String {
	type Error = DatabaseError;

	fn try_from(value: QueryValue) -> std::result::Result<Self, Self::Error> {
		match value {
			QueryValue::String(s) => Ok(s),
			_ => Err(DatabaseError::TypeError(format!(
				"Cannot convert {:?} to String",
				value
			))),
		}
	}
}

impl TryFrom<QueryValue> for bool {
	type Error = DatabaseError;

	fn try_from(value: QueryValue) -> std::result::Result<Self, Self::Error> {
		match value {
			QueryValue::Bool(b) => Ok(b),
			_ => Err(DatabaseError::TypeError(format!(
				"Cannot convert {:?} to bool",
				value
			))),
		}
	}
}

impl TryFrom<QueryValue> for f64 {
	type Error = DatabaseError;

	fn try_from(value: QueryValue) -> std::result::Result<Self, Self::Error> {
		match value {
			QueryValue::Float(f) => Ok(f),
			_ => Err(DatabaseError::TypeError(format!(
				"Cannot convert {:?} to f64",
				value
			))),
		}
	}
}

impl TryFrom<QueryValue> for chrono::DateTime<chrono::Utc> {
	type Error = DatabaseError;

	fn try_from(value: QueryValue) -> std::result::Result<Self, Self::Error> {
		match value {
			QueryValue::Timestamp(dt) => Ok(dt),
			_ => Err(DatabaseError::TypeError(format!(
				"Cannot convert {:?} to DateTime<Utc>",
				value
			))),
		}
	}
}

impl TryFrom<QueryValue> for Uuid {
	type Error = DatabaseError;

	fn try_from(value: QueryValue) -> std::result::Result<Self, Self::Error> {
		match value {
			QueryValue::Uuid(u) => Ok(u),
			QueryValue::String(s) => Uuid::parse_str(&s)
				.map_err(|_| DatabaseError::TypeError(format!("Invalid UUID string: {}", s))),
			_ => Err(DatabaseError::TypeError(format!(
				"Cannot convert {:?} to Uuid",
				value
			))),
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
	/// Execute a query that modifies the database within the transaction
	async fn execute(
		&mut self,
		sql: &str,
		params: Vec<QueryValue>,
	) -> super::error::Result<QueryResult>;

	/// Fetch a single row within the transaction
	async fn fetch_one(&mut self, sql: &str, params: Vec<QueryValue>) -> super::error::Result<Row>;

	/// Fetch all matching rows within the transaction
	async fn fetch_all(
		&mut self,
		sql: &str,
		params: Vec<QueryValue>,
	) -> super::error::Result<Vec<Row>>;

	/// Fetch an optional single row within the transaction
	async fn fetch_optional(
		&mut self,
		sql: &str,
		params: Vec<QueryValue>,
	) -> super::error::Result<Option<Row>>;

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
		Err(super::error::DatabaseError::NotSupported(
			"Savepoints are not supported by this backend".to_string(),
		))
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
		Err(super::error::DatabaseError::NotSupported(
			"Savepoints are not supported by this backend".to_string(),
		))
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
		Err(super::error::DatabaseError::NotSupported(
			"Savepoints are not supported by this backend".to_string(),
		))
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

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
