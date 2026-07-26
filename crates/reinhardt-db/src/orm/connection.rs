//! Database connection management
//!
//! This module separates backend ownership from the copyable ORM connection
//! capability. [`DatabaseConnectionLease`] owns the registry lifetime and
//! [`DatabaseConnection`] resolves the backend for each operation.

use async_trait::async_trait;
use std::sync::Arc;

use reinhardt_core::exception::Result;

use super::connection_registry::{self, ConnectionSlot, Generation, RegisteredConnection};
use super::transaction::AtomicTransaction;

/// Re-export backends types
pub use crate::backends::connection::DatabaseConnection as BackendsConnection;
use crate::backends::types::DatabaseType;
pub use crate::backends::types::{
	IsolationLevel, QueryResult, QueryValue, Row, TransactionExecutor,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
/// Defines possible database backend values.
pub enum DatabaseBackend {
	/// Postgres variant.
	Postgres,
	/// MySql variant.
	MySql,
	/// Sqlite variant.
	Sqlite,
}

impl From<DatabaseType> for DatabaseBackend {
	fn from(database_type: DatabaseType) -> Self {
		match database_type {
			DatabaseType::Postgres => Self::Postgres,
			DatabaseType::Mysql => Self::MySql,
			DatabaseType::Sqlite => Self::Sqlite,
		}
	}
}

/// Query row wrapper for ORM compatibility
#[derive(serde::Serialize)]
pub struct QueryRow {
	/// The data.
	pub data: serde_json::Value,
	#[serde(skip)]
	json_null_fields: std::collections::HashSet<String>,
	#[serde(skip)]
	native_json_fields: std::collections::HashSet<String>,
	// Allow dead_code: field reserved for future connection metadata tracking
	#[allow(dead_code)]
	#[serde(skip)]
	inner: Option<Row>,
}

impl QueryRow {
	/// Creates a new instance.
	pub fn new(data: serde_json::Value) -> Self {
		Self {
			data,
			json_null_fields: std::collections::HashSet::new(),
			native_json_fields: std::collections::HashSet::new(),
			inner: None,
		}
	}

	/// Creates an instance from backend row.
	pub fn from_backend_row(row: Row) -> Self {
		// Convert Row to JSON for backward compatibility
		let mut map = serde_json::Map::new();
		let mut json_null_fields = std::collections::HashSet::new();
		let mut native_json_fields = std::collections::HashSet::new();
		for (key, value) in row.data.iter() {
			let json_value = match value.clone() {
				QueryValue::Null => serde_json::Value::Null,
				QueryValue::Bool(b) => serde_json::Value::Bool(b),
				QueryValue::Int(i) => serde_json::Value::Number(i.into()),
				QueryValue::Float(f) => serde_json::Number::from_f64(f)
					.map(serde_json::Value::Number)
					.unwrap_or(serde_json::Value::Null),
				QueryValue::String(s) => serde_json::Value::String(s),
				QueryValue::Bytes(b) => {
					// Encode bytes as base64 string
					use base64::Engine;
					serde_json::Value::String(base64::engine::general_purpose::STANDARD.encode(&b))
				}
				QueryValue::Timestamp(dt) => serde_json::Value::String(dt.to_rfc3339()),
				QueryValue::Uuid(u) => serde_json::Value::String(u.to_string()),
				QueryValue::Json(Some(value)) => {
					native_json_fields.insert(key.clone());
					if value.is_null() {
						json_null_fields.insert(key.clone());
					}
					value.as_ref().clone()
				}
				QueryValue::Json(None) => {
					native_json_fields.insert(key.clone());
					serde_json::Value::Null
				}
				#[cfg(feature = "pgvector")]
				QueryValue::Vector(values) => serde_json::Value::Array(
					values.into_iter().map(serde_json::Value::from).collect(),
				),
				QueryValue::StringArray(values) => serde_json::Value::Array(
					values.into_iter().map(serde_json::Value::String).collect(),
				),
				QueryValue::IntArray(values) => serde_json::Value::Array(
					values.into_iter().map(serde_json::Value::from).collect(),
				),
				QueryValue::BigIntArray(values) => serde_json::Value::Array(
					values.into_iter().map(serde_json::Value::from).collect(),
				),
				QueryValue::BoolArray(values) => serde_json::Value::Array(
					values.into_iter().map(serde_json::Value::from).collect(),
				),
				QueryValue::FloatArray(values) => serde_json::Value::Array(
					values.into_iter().map(serde_json::Value::from).collect(),
				),
				QueryValue::DoubleArray(values) => serde_json::Value::Array(
					values.into_iter().map(serde_json::Value::from).collect(),
				),
				QueryValue::UuidArray(values) => serde_json::Value::Array(
					values
						.into_iter()
						.map(|value| serde_json::Value::String(value.to_string()))
						.collect(),
				),
				// NOW() should never appear in Row data (it's resolved to actual timestamp in database)
				QueryValue::Now => panic!("QueryValue::Now should not appear in Row data"),
			};
			map.insert(key.clone(), json_value);
		}

		Self {
			data: serde_json::Value::Object(map),
			json_null_fields,
			native_json_fields,
			inner: Some(row),
		}
	}

	pub(crate) fn deserialize_model<M: super::Model>(
		&self,
	) -> std::result::Result<M, super::FieldCodecError> {
		super::json::deserialize_model_row::<M>(
			self.data.clone(),
			self.json_null_fields.clone(),
			self.native_json_fields.clone(),
		)
	}

	/// Database columns returned as native JSON values.
	pub(crate) fn native_json_fields(&self) -> &std::collections::HashSet<String> {
		&self.native_json_fields
	}

	/// Database columns returned as native JSON null values.
	pub(crate) fn json_null_fields(&self) -> &std::collections::HashSet<String> {
		&self.json_null_fields
	}

	/// Get a value from the row by column name
	///
	/// This method extracts a value from the row's JSON data by key.
	/// Supports common types like i64, f64, bool, and String.
	pub fn get<T: serde::de::DeserializeOwned>(&self, key: &str) -> Option<T> {
		self.data
			.get(key)
			.and_then(|v| serde_json::from_value(v.clone()).ok())
	}
}

#[async_trait]
/// Typed capability for executing ORM statements against one backend.
pub trait OrmExecutor: Send {
	/// Returns the backend used to generate SQL for this executor.
	fn backend(&self) -> DatabaseBackend;

	/// Returns whether contextual pgvector error hints are supported.
	fn supports_pgvector_error_hints(&self) -> bool {
		false
	}

	/// Executes a SQL statement and preserves backend-specific result metadata.
	async fn execute(&mut self, sql: &str, params: Vec<QueryValue>) -> Result<QueryResult>;

	/// Executes a SQL statement with structural pgvector operation context.
	async fn execute_with_context(
		&mut self,
		sql: &str,
		params: Vec<QueryValue>,
		context: Option<crate::backends::error::PgvectorOperationKind>,
	) -> Result<QueryResult> {
		let result = self.execute(sql, params).await;
		if self.backend() == DatabaseBackend::Postgres && self.supports_pgvector_error_hints() {
			result.map_err(|error| {
				crate::backends::error::decorate_error_with_pgvector_context(error, context)
			})
		} else {
			result
		}
	}

	/// Fetches one row.
	async fn fetch_one(&mut self, sql: &str, params: Vec<QueryValue>) -> Result<Row>;

	/// Fetches one row with structural pgvector operation context.
	async fn fetch_one_with_context(
		&mut self,
		sql: &str,
		params: Vec<QueryValue>,
		context: Option<crate::backends::error::PgvectorOperationKind>,
	) -> Result<Row> {
		let result = self.fetch_one(sql, params).await;
		if self.backend() == DatabaseBackend::Postgres && self.supports_pgvector_error_hints() {
			result.map_err(|error| {
				crate::backends::error::decorate_error_with_pgvector_context(error, context)
			})
		} else {
			result
		}
	}

	/// Fetches all matching rows.
	async fn fetch_all(&mut self, sql: &str, params: Vec<QueryValue>) -> Result<Vec<Row>>;

	/// Fetches all matching rows with structural pgvector operation context.
	async fn fetch_all_with_context(
		&mut self,
		sql: &str,
		params: Vec<QueryValue>,
		context: Option<crate::backends::error::PgvectorOperationKind>,
	) -> Result<Vec<Row>> {
		let result = self.fetch_all(sql, params).await;
		if self.backend() == DatabaseBackend::Postgres && self.supports_pgvector_error_hints() {
			result.map_err(|error| {
				crate::backends::error::decorate_error_with_pgvector_context(error, context)
			})
		} else {
			result
		}
	}

	/// Fetches an optional row without swallowing backend failures.
	async fn fetch_optional(&mut self, sql: &str, params: Vec<QueryValue>) -> Result<Option<Row>>;

	/// Fetches an optional row with structural pgvector operation context.
	async fn fetch_optional_with_context(
		&mut self,
		sql: &str,
		params: Vec<QueryValue>,
		context: Option<crate::backends::error::PgvectorOperationKind>,
	) -> Result<Option<Row>> {
		let result = self.fetch_optional(sql, params).await;
		if self.backend() == DatabaseBackend::Postgres && self.supports_pgvector_error_hints() {
			result.map_err(|error| {
				crate::backends::error::decorate_error_with_pgvector_context(error, context)
			})
		} else {
			result
		}
	}
}

/// Copyable capability for an ORM database connection.
///
/// A handle remains valid while at least one clone of its originating
/// [`DatabaseConnectionLease`] exists. Operations started after the last lease
/// drops return `DatabaseErrorKind::ConnectionHandleExpired`; operations that
/// already resolved the backend are allowed to finish.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DatabaseConnection {
	slot: ConnectionSlot,
	generation: Generation,
	backend: DatabaseBackend,
}

/// RAII owner that keeps a database connection handle valid.
///
/// Retain this owner in standalone application or server-bootstrap state. The
/// handle returned by [`Self::handle`] may be copied into concurrent operations,
/// but no copied handle extends the lease lifetime.
#[derive(Clone)]
pub struct DatabaseConnectionLease {
	registration: RegisteredConnection,
}

impl DatabaseConnectionLease {
	/// Registers a backend connection and owns its registry lifetime.
	pub fn register(owner: BackendsConnection) -> Result<Self> {
		Ok(Self {
			registration: connection_registry::register(owner)?,
		})
	}

	/// Returns the copyable ORM capability associated with this lease.
	pub fn handle(&self) -> DatabaseConnection {
		let (slot, generation, backend) = self.registration.handle_parts();
		DatabaseConnection {
			slot,
			generation,
			backend,
		}
	}
}

impl DatabaseConnection {
	fn resolve(self) -> Result<Arc<BackendsConnection>> {
		Ok(connection_registry::resolve(self.slot, self.generation)?)
	}

	/// Performs the backend operation.
	pub fn backend(&self) -> DatabaseBackend {
		self.backend
	}

	/// Execute a SQL query and return a single row
	pub async fn query_one(&self, sql: &str, params: Vec<QueryValue>) -> Result<QueryRow> {
		let owner = self.resolve()?;
		let row = owner.fetch_one(sql, params).await?;
		Ok(QueryRow::from_backend_row(row))
	}

	/// Execute a SQL query and return an optional row
	pub async fn query_optional(
		&self,
		sql: &str,
		params: Vec<QueryValue>,
	) -> Result<Option<QueryRow>> {
		let owner = self.resolve()?;
		let row = owner.fetch_optional(sql, params).await?;
		Ok(row.map(QueryRow::from_backend_row))
	}

	/// Execute a SQL statement (INSERT, UPDATE, DELETE, etc.)
	pub async fn execute(&self, sql: &str, params: Vec<QueryValue>) -> Result<u64> {
		let owner = self.resolve()?;
		let result = owner.execute(sql, params).await?;
		Ok(result.rows_affected)
	}

	/// Execute a SQL query and return all rows
	pub async fn query(&self, sql: &str, params: Vec<QueryValue>) -> Result<Vec<QueryRow>> {
		let owner = self.resolve()?;
		let rows = owner.fetch_all(sql, params).await?;
		Ok(rows.into_iter().map(QueryRow::from_backend_row).collect())
	}

	async fn begin_atomic(&self) -> Result<AtomicTransaction> {
		let owner = self.resolve()?;
		let executor = owner.begin().await?;
		Ok(AtomicTransaction::new(executor))
	}

	async fn begin_atomic_with_isolation(
		&self,
		level: super::transaction::IsolationLevel,
	) -> Result<AtomicTransaction> {
		let owner = self.resolve()?;
		let executor = owner
			.begin_with_isolation(level.to_backends_level())
			.await?;
		Ok(AtomicTransaction::new(executor))
	}

	/// Runs a closure inside one dedicated transaction connection.
	///
	/// Successful callbacks commit the transaction. Callback errors roll it back,
	/// while a rollback failure takes precedence over the callback error.
	pub async fn atomic<F, T, E>(&self, f: F) -> std::result::Result<T, E>
	where
		F: for<'txn> std::ops::AsyncFnOnce(
				&'txn mut AtomicTransaction,
			) -> std::result::Result<T, E>,
		E: std::error::Error + From<reinhardt_core::exception::Error>,
	{
		let transaction = self.begin_atomic().await.map_err(E::from)?;
		transaction.run(f).await
	}

	/// Runs a closure inside one dedicated transaction at the requested isolation level.
	pub async fn atomic_with_isolation<F, T, E>(
		&self,
		level: super::transaction::IsolationLevel,
		f: F,
	) -> std::result::Result<T, E>
	where
		F: for<'txn> std::ops::AsyncFnOnce(
				&'txn mut AtomicTransaction,
			) -> std::result::Result<T, E>,
		E: std::error::Error + From<reinhardt_core::exception::Error>,
	{
		let transaction = self
			.begin_atomic_with_isolation(level)
			.await
			.map_err(E::from)?;
		transaction.run(f).await
	}
}

#[async_trait]
impl OrmExecutor for DatabaseConnection {
	fn backend(&self) -> DatabaseBackend {
		self.resolve()
			.map(|owner| DatabaseBackend::from(owner.database_type()))
			.unwrap_or_else(|_| DatabaseConnection::backend(self))
	}

	fn supports_pgvector_error_hints(&self) -> bool {
		self.resolve()
			.is_ok_and(|owner| owner.supports_pgvector_error_hints())
	}

	async fn execute(&mut self, sql: &str, params: Vec<QueryValue>) -> Result<QueryResult> {
		let owner = self.resolve()?;
		owner.execute(sql, params).await
	}

	async fn execute_with_context(
		&mut self,
		sql: &str,
		params: Vec<QueryValue>,
		context: Option<crate::backends::error::PgvectorOperationKind>,
	) -> Result<QueryResult> {
		let owner = self.resolve()?;
		owner.execute_with_context(sql, params, context).await
	}

	async fn fetch_one(&mut self, sql: &str, params: Vec<QueryValue>) -> Result<Row> {
		let owner = self.resolve()?;
		owner.fetch_one(sql, params).await
	}

	async fn fetch_one_with_context(
		&mut self,
		sql: &str,
		params: Vec<QueryValue>,
		context: Option<crate::backends::error::PgvectorOperationKind>,
	) -> Result<Row> {
		let owner = self.resolve()?;
		owner.fetch_one_with_context(sql, params, context).await
	}

	async fn fetch_all(&mut self, sql: &str, params: Vec<QueryValue>) -> Result<Vec<Row>> {
		let owner = self.resolve()?;
		owner.fetch_all(sql, params).await
	}

	async fn fetch_all_with_context(
		&mut self,
		sql: &str,
		params: Vec<QueryValue>,
		context: Option<crate::backends::error::PgvectorOperationKind>,
	) -> Result<Vec<Row>> {
		let owner = self.resolve()?;
		owner.fetch_all_with_context(sql, params, context).await
	}

	async fn fetch_optional(&mut self, sql: &str, params: Vec<QueryValue>) -> Result<Option<Row>> {
		let owner = self.resolve()?;
		owner.fetch_optional(sql, params).await
	}

	async fn fetch_optional_with_context(
		&mut self,
		sql: &str,
		params: Vec<QueryValue>,
		context: Option<crate::backends::error::PgvectorOperationKind>,
	) -> Result<Option<Row>> {
		let owner = self.resolve()?;
		owner
			.fetch_optional_with_context(sql, params, context)
			.await
	}
}

/// Injectable implementation for DatabaseConnection
///
/// A `DatabaseConnection` handle must be registered in the DI context while its
/// `DatabaseConnectionLease` remains owned by server bootstrap.
///
/// # Example
///
/// ```rust,no_run
/// use reinhardt_db::{backends::DatabaseConnection as BackendsConnection, orm::DatabaseConnectionLease};
/// use reinhardt_di::InjectionContext;
///
/// # async fn example() {
/// let owner = BackendsConnection::connect_postgres("postgres://localhost/mydb").await.unwrap();
/// let lease = DatabaseConnectionLease::register(owner).unwrap();
/// let db = lease.handle();
///
/// // Then register it in the DI context as a singleton
/// let singleton_scope = reinhardt_di::SingletonScope::new();
/// let ctx = InjectionContext::builder(singleton_scope)
///     .singleton(db)
///     .build();
/// # }
/// ```
#[cfg(feature = "di")]
#[async_trait]
impl reinhardt_di::Injectable for DatabaseConnection {
	async fn inject(ctx: &reinhardt_di::InjectionContext) -> reinhardt_di::DiResult<Self> {
		if let Some(conn) = ctx.get_singleton::<Self>() {
			return Ok(*conn);
		}

		if let Some(conn) = ctx.get_request::<Self>() {
			return Ok(*conn);
		}

		Err(reinhardt_di::DiError::NotRegistered {
			type_name: std::any::type_name::<Self>().to_string(),
			hint: "Server bootstrap must register a DatabaseConnectionLease and inject its \
			       DatabaseConnection handle with InjectionContextBuilder::singleton()."
				.to_string(),
		})
	}

	async fn inject_uncached(ctx: &reinhardt_di::InjectionContext) -> reinhardt_di::DiResult<Self> {
		Self::inject(ctx).await
	}
}

#[cfg(test)]
mod tests {
	use std::sync::Arc;

	use async_trait::async_trait;
	use reinhardt_core::exception::Result;

	use super::{
		BackendsConnection, DatabaseBackend, DatabaseConnection, DatabaseConnectionLease,
		OrmExecutor, QueryRow,
	};
	use crate::backends::backend::DatabaseBackend as BackendsDatabaseBackend;
	use crate::backends::types::{DatabaseType, QueryResult, QueryValue, Row, TransactionExecutor};

	struct TestBackend;

	#[test]
	#[cfg(feature = "pgvector")]
	fn backend_vector_rows_preserve_numeric_json_arrays() {
		let mut row = Row::new();
		row.insert(
			"embedding".to_owned(),
			QueryValue::Vector(vec![1.0, 2.0, 3.0]),
		);

		let query_row = QueryRow::from_backend_row(row);

		assert_eq!(
			query_row.data["embedding"],
			serde_json::json!([1.0, 2.0, 3.0])
		);
	}

	#[async_trait]
	impl BackendsDatabaseBackend for TestBackend {
		fn database_type(&self) -> DatabaseType {
			DatabaseType::Sqlite
		}

		fn supports_pgvector_error_hints(&self) -> bool {
			true
		}

		fn placeholder(&self, index: usize) -> String {
			format!("${index}")
		}

		fn supports_returning(&self) -> bool {
			true
		}

		fn supports_on_conflict(&self) -> bool {
			true
		}

		async fn execute(&self, _sql: &str, _params: Vec<QueryValue>) -> Result<QueryResult> {
			unreachable!()
		}

		async fn fetch_one(&self, _sql: &str, _params: Vec<QueryValue>) -> Result<Row> {
			unreachable!()
		}

		async fn fetch_all(&self, _sql: &str, _params: Vec<QueryValue>) -> Result<Vec<Row>> {
			unreachable!()
		}

		async fn fetch_optional(
			&self,
			_sql: &str,
			_params: Vec<QueryValue>,
		) -> Result<Option<Row>> {
			unreachable!()
		}

		async fn begin(&self) -> Result<Box<dyn TransactionExecutor>> {
			unreachable!()
		}

		fn as_any(&self) -> &dyn std::any::Any {
			self
		}
	}

	fn mock_backends_connection() -> BackendsConnection {
		BackendsConnection::new(Arc::new(TestBackend))
	}

	fn assert_connection_traits<T: Copy + Clone + Send + Sync + 'static>() {}

	fn consume_connection(_connection: DatabaseConnection) {}

	#[test]
	fn database_connection_is_a_copy_capability() {
		assert_connection_traits::<DatabaseConnection>();

		let lease = DatabaseConnectionLease::register(mock_backends_connection()).unwrap();
		let connection = lease.handle();
		consume_connection(connection);
		consume_connection(connection);
	}

	#[test]
	fn database_connection_reports_inner_backend_and_pgvector_capability() {
		let lease = DatabaseConnectionLease::register(mock_backends_connection()).unwrap();
		let connection = lease.handle();

		assert_eq!(OrmExecutor::backend(&connection), DatabaseBackend::Sqlite);
		assert!(OrmExecutor::supports_pgvector_error_hints(&connection));
	}

	#[test]
	fn test_database_backend_converts_each_database_type() {
		assert_eq!(
			DatabaseBackend::from(DatabaseType::Postgres),
			DatabaseBackend::Postgres
		);
		assert_eq!(
			DatabaseBackend::from(DatabaseType::Mysql),
			DatabaseBackend::MySql
		);
		assert_eq!(
			DatabaseBackend::from(DatabaseType::Sqlite),
			DatabaseBackend::Sqlite
		);
	}

	#[cfg(feature = "sqlite")]
	#[tokio::test]
	async fn test_error_kind_for_missing_sqlite_column() {
		let owner = BackendsConnection::connect_sqlite("sqlite::memory:")
			.await
			.expect("the in-memory SQLite database must connect");
		let lease = DatabaseConnectionLease::register(owner).unwrap();
		let connection = lease.handle();
		connection
			.execute("CREATE TABLE records (id INTEGER PRIMARY KEY)", vec![])
			.await
			.expect("the fixture table must be created");

		let Err(error) = connection
			.query("SELECT missing_column FROM records", vec![])
			.await
		else {
			panic!("querying a missing column must fail");
		};

		assert_eq!(
			error.database_kind(),
			Some(crate::backends::DatabaseErrorKind::Query)
		);
	}

	#[cfg(feature = "sqlite")]
	#[tokio::test]
	async fn test_orm_executor_preserves_sqlite_query_result_metadata() {
		let owner = BackendsConnection::connect_sqlite("sqlite::memory:")
			.await
			.expect("the in-memory SQLite database must connect");
		let lease = DatabaseConnectionLease::register(owner).unwrap();
		let mut connection = lease.handle();

		let create_result = OrmExecutor::execute(
			&mut connection,
			"CREATE TABLE records (id INTEGER PRIMARY KEY, name TEXT NOT NULL)",
			vec![],
		)
		.await
		.expect("the fixture table must be created");
		assert_eq!(create_result.last_insert_id, None);

		let insert_result = OrmExecutor::execute(
			&mut connection,
			"INSERT INTO records (name) VALUES ('first')",
			vec![],
		)
		.await
		.expect("the fixture row must be inserted");

		assert_eq!(OrmExecutor::backend(&connection), DatabaseBackend::Sqlite);
		assert_eq!(insert_result.rows_affected, 1);
		assert_eq!(insert_result.last_insert_id, None);
	}

	#[cfg(feature = "sqlite")]
	#[tokio::test]
	async fn test_query_optional_preserves_sqlite_backend_errors() {
		let owner = BackendsConnection::connect_sqlite("sqlite::memory:")
			.await
			.expect("the in-memory SQLite database must connect");
		let lease = DatabaseConnectionLease::register(owner).unwrap();
		let mut connection = lease.handle();
		OrmExecutor::execute(
			&mut connection,
			"CREATE TABLE records (id INTEGER PRIMARY KEY)",
			vec![],
		)
		.await
		.expect("the fixture table must be created");

		let error = match connection
			.query_optional("SELECT missing_column FROM records", vec![])
			.await
		{
			Err(error) => error,
			Ok(_) => panic!("an invalid optional query must preserve its backend error"),
		};

		assert_eq!(
			error.database_kind(),
			Some(crate::backends::DatabaseErrorKind::Query)
		);
	}
}
