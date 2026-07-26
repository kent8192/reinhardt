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

	/// Returns whether contextual pgvector error hints are supported.
	fn supports_pgvector_error_hints(&self) -> bool {
		false
	}

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
		let result = self.execute(sql, params).await;
		if self.database_type() == DatabaseType::Postgres && self.supports_pgvector_error_hints() {
			result
				.map_err(|error| super::error::decorate_error_with_pgvector_context(error, context))
		} else {
			result
		}
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
		let result = self.fetch_one(sql, params).await;
		if self.database_type() == DatabaseType::Postgres && self.supports_pgvector_error_hints() {
			result
				.map_err(|error| super::error::decorate_error_with_pgvector_context(error, context))
		} else {
			result
		}
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
		let result = self.fetch_all(sql, params).await;
		if self.database_type() == DatabaseType::Postgres && self.supports_pgvector_error_hints() {
			result
				.map_err(|error| super::error::decorate_error_with_pgvector_context(error, context))
		} else {
			result
		}
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
		let result = self.fetch_optional(sql, params).await;
		if self.database_type() == DatabaseType::Postgres && self.supports_pgvector_error_hints() {
			result
				.map_err(|error| super::error::decorate_error_with_pgvector_context(error, context))
		} else {
			result
		}
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

#[cfg(test)]
mod tests {
	use super::*;

	struct ContextErrorBackendWithoutCapability {
		database_type: DatabaseType,
		supports_pgvector_error_hints: bool,
	}

	#[async_trait]
	impl DatabaseBackend for ContextErrorBackendWithoutCapability {
		fn database_type(&self) -> DatabaseType {
			self.database_type
		}

		fn supports_pgvector_error_hints(&self) -> bool {
			self.supports_pgvector_error_hints
		}

		fn placeholder(&self, _index: usize) -> String {
			"?".to_owned()
		}

		fn supports_returning(&self) -> bool {
			false
		}

		fn supports_on_conflict(&self) -> bool {
			false
		}

		async fn execute(&self, _sql: &str, _params: Vec<QueryValue>) -> Result<QueryResult> {
			Err(super::super::error::DatabaseError::new(
				super::super::error::DatabaseErrorKind::Query,
				"operator does not exist: vector <=> vector",
			)
			.with_code("42883")
			.into())
		}

		async fn fetch_one(&self, _sql: &str, _params: Vec<QueryValue>) -> Result<Row> {
			panic!("context default backend test does not fetch rows")
		}

		async fn fetch_all(&self, _sql: &str, _params: Vec<QueryValue>) -> Result<Vec<Row>> {
			panic!("context default backend test does not fetch rows")
		}

		async fn fetch_optional(
			&self,
			_sql: &str,
			_params: Vec<QueryValue>,
		) -> Result<Option<Row>> {
			panic!("context default backend test does not fetch rows")
		}

		async fn begin(&self) -> Result<Box<dyn TransactionExecutor>> {
			panic!("context default backend test does not begin transactions")
		}

		fn as_any(&self) -> &dyn std::any::Any {
			self
		}
	}

	#[rstest::rstest]
	#[case(DatabaseType::Mysql)]
	#[case(DatabaseType::Sqlite)]
	#[case(DatabaseType::Postgres)]
	#[tokio::test]
	async fn backend_default_without_capability_does_not_decorate_pgvector_shaped_error(
		#[case] database_type: DatabaseType,
	) {
		let backend = ContextErrorBackendWithoutCapability {
			database_type,
			supports_pgvector_error_hints: false,
		};

		let error = backend
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

	#[rstest::rstest]
	#[case(DatabaseType::Mysql)]
	#[case(DatabaseType::Sqlite)]
	#[tokio::test]
	async fn backend_default_requires_postgres_even_when_capability_is_enabled(
		#[case] database_type: DatabaseType,
	) {
		let backend = ContextErrorBackendWithoutCapability {
			database_type,
			supports_pgvector_error_hints: true,
		};

		let error = backend
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
}
