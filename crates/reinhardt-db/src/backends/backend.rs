//! Database backend abstraction

use async_trait::async_trait;

use super::{
	error::Result,
	types::{DatabaseType, IsolationLevel, QueryResult, QueryValue, Row, TransactionExecutor},
};

/// Core database backend trait
#[async_trait]
pub trait DatabaseBackend: Send + Sync {
	/// Returns the database type
	fn database_type(&self) -> DatabaseType;

	/// Generates a placeholder for the given parameter index (1-based)
	fn placeholder(&self, index: usize) -> String;

	/// Returns whether the database supports RETURNING clause
	fn supports_returning(&self) -> bool;

	/// Returns whether the database supports ON CONFLICT clause
	fn supports_on_conflict(&self) -> bool;

	/// Returns whether DDL statements can be rolled back in transactions
	///
	/// PostgreSQL and SQLite support transactional DDL - CREATE TABLE, etc.
	/// can be rolled back if the transaction fails.
	///
	/// MySQL/MariaDB do NOT support transactional DDL - DDL statements cause
	/// an implicit commit, so they cannot be rolled back.
	fn supports_transactional_ddl(&self) -> bool {
		// Default implementation: check database type
		self.database_type().supports_transactional_ddl()
	}

	/// Executes a query that modifies the database
	async fn execute(&self, sql: &str, params: Vec<QueryValue>) -> Result<QueryResult>;

	/// Executes a query with structural pgvector operation context.
	async fn execute_with_context(
		&self,
		sql: &str,
		params: Vec<QueryValue>,
		context: Option<super::error::PgvectorOperationKind>,
	) -> Result<QueryResult> {
		self.execute(sql, params)
			.await
			.map_err(|error| super::error::decorate_error_with_pgvector_context(error, context))
	}

	/// Fetches a single row from the database
	async fn fetch_one(&self, sql: &str, params: Vec<QueryValue>) -> Result<Row>;

	/// Fetches one row with structural pgvector operation context.
	async fn fetch_one_with_context(
		&self,
		sql: &str,
		params: Vec<QueryValue>,
		context: Option<super::error::PgvectorOperationKind>,
	) -> Result<Row> {
		self.fetch_one(sql, params)
			.await
			.map_err(|error| super::error::decorate_error_with_pgvector_context(error, context))
	}

	/// Fetches all matching rows from the database
	async fn fetch_all(&self, sql: &str, params: Vec<QueryValue>) -> Result<Vec<Row>>;

	/// Fetches rows with structural pgvector operation context.
	async fn fetch_all_with_context(
		&self,
		sql: &str,
		params: Vec<QueryValue>,
		context: Option<super::error::PgvectorOperationKind>,
	) -> Result<Vec<Row>> {
		self.fetch_all(sql, params)
			.await
			.map_err(|error| super::error::decorate_error_with_pgvector_context(error, context))
	}

	/// Fetches an optional single row from the database
	async fn fetch_optional(&self, sql: &str, params: Vec<QueryValue>) -> Result<Option<Row>>;

	/// Fetches an optional row with structural pgvector operation context.
	async fn fetch_optional_with_context(
		&self,
		sql: &str,
		params: Vec<QueryValue>,
		context: Option<super::error::PgvectorOperationKind>,
	) -> Result<Option<Row>> {
		self.fetch_optional(sql, params)
			.await
			.map_err(|error| super::error::decorate_error_with_pgvector_context(error, context))
	}

	/// Begin a database transaction and return a dedicated executor
	///
	/// This method acquires a dedicated database connection and begins a
	/// transaction on it. All queries executed through the returned
	/// `TransactionExecutor` are guaranteed to run on the same physical
	/// connection, ensuring proper transaction isolation.
	///
	/// # Returns
	///
	/// A boxed `TransactionExecutor` that holds the dedicated connection
	/// and provides methods for executing queries within the transaction.
	async fn begin(&self) -> Result<Box<dyn TransactionExecutor>>;

	/// Begin a database transaction with a specific isolation level
	///
	/// This method is similar to `begin()`, but allows specifying the
	/// transaction isolation level. The isolation level controls the
	/// visibility of changes made by other concurrent transactions.
	///
	/// # Arguments
	///
	/// * `isolation_level` - The desired isolation level for the transaction
	///
	/// # Returns
	///
	/// A boxed `TransactionExecutor` that holds the dedicated connection
	/// with the specified isolation level.
	///
	/// # Default Implementation
	///
	/// Falls back to `begin()` with the database's default isolation level.
	/// Backends that support custom isolation levels should override this.
	async fn begin_with_isolation(
		&self,
		isolation_level: IsolationLevel,
	) -> Result<Box<dyn TransactionExecutor>> {
		let _ = isolation_level;
		// Default implementation: ignore isolation level and use default
		self.begin().await
	}

	/// Returns self as &dyn std::any::Any for downcasting
	fn as_any(&self) -> &dyn std::any::Any;
}
