//! Database connection management
//!
//! This module provides the main `DatabaseConnection` type which wraps
//! the backend-specific connection implementations.

use async_trait::async_trait;

/// Re-export backends types
pub use crate::backends::connection::DatabaseConnection as BackendsConnection;
pub use crate::backends::types::{IsolationLevel, QueryValue, Row, TransactionExecutor};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DatabaseBackend {
	Postgres,
	MySql,
	Sqlite,
}

/// Query row wrapper for ORM compatibility
#[derive(serde::Serialize)]
pub struct QueryRow {
	pub data: serde_json::Value,
	// Allow dead_code: field reserved for future connection metadata tracking
	#[allow(dead_code)]
	#[serde(skip)]
	inner: Option<Row>,
}

impl QueryRow {
	pub fn new(data: serde_json::Value) -> Self {
		Self { data, inner: None }
	}

	pub fn from_backend_row(row: Row) -> Self {
		// Convert Row to JSON for backward compatibility
		let mut map = serde_json::Map::new();
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
				// NOW() should never appear in Row data (it's resolved to actual timestamp in database)
				QueryValue::Now => panic!("QueryValue::Now should not appear in Row data"),
			};
			map.insert(key.clone(), json_value);
		}

		Self {
			data: serde_json::Value::Object(map),
			inner: Some(row),
		}
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
pub trait DatabaseExecutor: Send + Sync {
	async fn execute(&self, sql: &str) -> Result<u64, anyhow::Error>;
	async fn query(&self, sql: &str) -> Result<Vec<QueryRow>, anyhow::Error>;
}

/// Database connection wrapper
#[derive(Clone)]
pub struct DatabaseConnection {
	backend: DatabaseBackend,
	inner: BackendsConnection,
}

impl DatabaseConnection {
	pub fn new(backend: DatabaseBackend, inner: BackendsConnection) -> Self {
		Self { backend, inner }
	}

	/// Connect to a database from a connection URL
	///
	/// Automatically detects the database type from the URL scheme:
	/// - `postgres://` or `postgresql://` → PostgreSQL
	/// - `mysql://` → MySQL
	/// - `sqlite://` or `sqlite:` → SQLite
	///
	/// # Examples
	///
	/// ```no_run
	/// # async fn example() {
	/// use reinhardt_db::orm::connection::DatabaseConnection;
	///
	/// let conn = DatabaseConnection::connect("postgres://localhost/mydb").await.unwrap();
	/// # }
	/// # tokio::runtime::Runtime::new().unwrap().block_on(example());
	/// ```
	pub async fn connect(url: &str) -> Result<Self, anyhow::Error> {
		Self::connect_with_pool_size(url, None).await
	}

	/// Connect to a PostgreSQL database
	///
	/// # Examples
	///
	/// ```no_run
	/// # async fn example() {
	/// use reinhardt_db::orm::connection::DatabaseConnection;
	///
	/// let conn = DatabaseConnection::connect_postgres("postgres://localhost/mydb").await.unwrap();
	/// # }
	/// # tokio::runtime::Runtime::new().unwrap().block_on(example());
	/// ```
	#[cfg(feature = "postgres")]
	pub async fn connect_postgres(url: &str) -> Result<Self, anyhow::Error> {
		let inner = BackendsConnection::connect_postgres(url).await?;
		Ok(Self {
			backend: DatabaseBackend::Postgres,
			inner,
		})
	}

	/// Connect to a MySQL database
	///
	/// # Examples
	///
	/// ```no_run
	/// # async fn example() {
	/// use reinhardt_db::orm::connection::DatabaseConnection;
	///
	/// let conn = DatabaseConnection::connect_mysql("mysql://localhost/mydb").await.unwrap();
	/// # }
	/// # tokio::runtime::Runtime::new().unwrap().block_on(example());
	/// ```
	#[cfg(feature = "mysql")]
	pub async fn connect_mysql(url: &str) -> Result<Self, anyhow::Error> {
		let inner = BackendsConnection::connect_mysql(url).await?;
		Ok(Self {
			backend: DatabaseBackend::MySql,
			inner,
		})
	}

	/// Connect to a SQLite database
	///
	/// # Examples
	///
	/// ```no_run
	/// # async fn example() {
	/// use reinhardt_db::orm::connection::DatabaseConnection;
	///
	/// let conn = DatabaseConnection::connect_sqlite("sqlite::memory:").await.unwrap();
	/// # }
	/// # tokio::runtime::Runtime::new().unwrap().block_on(example());
	/// ```
	#[cfg(feature = "sqlite")]
	pub async fn connect_sqlite(url: &str) -> Result<Self, anyhow::Error> {
		let inner = BackendsConnection::connect_sqlite(url).await?;
		Ok(Self {
			backend: DatabaseBackend::Sqlite,
			inner,
		})
	}

	/// Connect to a database with a specific connection pool size
	///
	/// # Arguments
	///
	/// * `url` - Database connection URL
	/// * `pool_size` - Maximum number of connections in the pool (None = use default)
	///
	/// # Examples
	///
	/// ```no_run
	/// # async fn example() {
	/// use reinhardt_db::orm::connection::DatabaseConnection;
	///
	/// // Use larger pool for high-concurrency scenarios
	/// let conn = DatabaseConnection::connect_with_pool_size(
	///     "postgres://localhost/mydb",
	///     Some(50)
	/// ).await.unwrap();
	/// # }
	/// # tokio::runtime::Runtime::new().unwrap().block_on(example());
	/// ```
	// Allow unused_variables because pool_size is only used with Postgres backend.
	// MySQL and SQLite backends don't support pool size configuration yet.
	#[allow(unused_variables)]
	pub async fn connect_with_pool_size(
		url: &str,
		pool_size: Option<u32>,
	) -> Result<Self, anyhow::Error> {
		let backend_type = if url.starts_with("postgres://") || url.starts_with("postgresql://") {
			DatabaseBackend::Postgres
		} else if url.starts_with("mysql://") {
			DatabaseBackend::MySql
		} else if url.starts_with("sqlite://") || url.starts_with("sqlite:") {
			DatabaseBackend::Sqlite
		} else {
			return Err(anyhow::anyhow!("Unsupported database URL scheme: {}", url));
		};

		#[cfg(feature = "postgres")]
		if backend_type == DatabaseBackend::Postgres {
			let inner = BackendsConnection::connect_postgres_with_pool_size(url, pool_size).await?;
			return Ok(Self {
				backend: backend_type,
				inner,
			});
		}

		#[cfg(feature = "mysql")]
		if backend_type == DatabaseBackend::MySql {
			let inner = BackendsConnection::connect_mysql(url).await?;
			return Ok(Self {
				backend: backend_type,
				inner,
			});
		}

		#[cfg(feature = "sqlite")]
		if backend_type == DatabaseBackend::Sqlite {
			let inner = BackendsConnection::connect_sqlite(url).await?;
			return Ok(Self {
				backend: backend_type,
				inner,
			});
		}

		Err(anyhow::anyhow!(
			"Database backend not compiled in. Enable the '{}' feature.",
			match backend_type {
				DatabaseBackend::Postgres => "postgres",
				DatabaseBackend::MySql => "mysql",
				DatabaseBackend::Sqlite => "sqlite",
			}
		))
	}

	pub fn backend(&self) -> DatabaseBackend {
		self.backend
	}

	/// Get a reference to the inner backends connection
	///
	/// This provides access to the low-level connection for operations
	/// that require direct database access.
	pub fn inner(&self) -> &BackendsConnection {
		&self.inner
	}

	/// Consume self and return the inner backends connection
	///
	/// This is useful when you need to pass ownership of the connection
	/// to functions that expect a `BackendsConnection`.
	pub fn into_inner(self) -> BackendsConnection {
		self.inner
	}

	/// Execute a SQL query and return a single row
	pub async fn query_one(
		&self,
		sql: &str,
		params: Vec<QueryValue>,
	) -> Result<QueryRow, anyhow::Error> {
		let row = self.inner.fetch_one(sql, params).await?;
		Ok(QueryRow::from_backend_row(row))
	}

	/// Execute a SQL query and return an optional row
	pub async fn query_optional(
		&self,
		sql: &str,
		params: Vec<QueryValue>,
	) -> Result<Option<QueryRow>, anyhow::Error> {
		match self.inner.fetch_one(sql, params).await {
			Ok(row) => Ok(Some(QueryRow::from_backend_row(row))),
			Err(_) => Ok(None),
		}
	}

	/// Execute a SQL statement (INSERT, UPDATE, DELETE, etc.)
	pub async fn execute(&self, sql: &str, params: Vec<QueryValue>) -> Result<u64, anyhow::Error> {
		let result = self.inner.execute(sql, params).await?;
		Ok(result.rows_affected)
	}

	/// Execute a SQL query and return all rows
	pub async fn query(
		&self,
		sql: &str,
		params: Vec<QueryValue>,
	) -> Result<Vec<QueryRow>, anyhow::Error> {
		let rows = self.inner.fetch_all(sql, params).await?;
		Ok(rows.into_iter().map(QueryRow::from_backend_row).collect())
	}

	/// Begin a database transaction
	///
	/// # Examples
	///
	/// ```no_run
	/// # async fn example() {
	/// use reinhardt_db::orm::connection::DatabaseConnection;
	///
	/// let conn = DatabaseConnection::connect("sqlite::memory:").await.unwrap();
	/// let result = conn.begin_transaction().await;
	/// assert!(result.is_ok());
	/// # }
	/// # tokio::runtime::Runtime::new().unwrap().block_on(example());
	/// ```
	pub async fn begin_transaction(&self) -> Result<(), anyhow::Error> {
		self.execute("BEGIN TRANSACTION", vec![]).await?;
		Ok(())
	}

	/// Begin a transaction with a specific isolation level
	///
	/// # Examples
	///
	/// ```no_run
	/// # async fn example() {
	/// use reinhardt_db::orm::connection::DatabaseConnection;
	/// use reinhardt_db::orm::transaction::IsolationLevel;
	///
	/// let conn = DatabaseConnection::connect("sqlite::memory:").await.unwrap();
	/// let result = conn.begin_transaction_with_isolation(IsolationLevel::Serializable).await;
	/// assert!(result.is_ok());
	/// # }
	/// # tokio::runtime::Runtime::new().unwrap().block_on(example());
	/// ```
	pub async fn begin_transaction_with_isolation(
		&self,
		level: super::transaction::IsolationLevel,
	) -> Result<(), anyhow::Error> {
		let sql = format!("BEGIN TRANSACTION ISOLATION LEVEL {}", level.to_sql());
		self.execute(&sql, vec![]).await?;
		Ok(())
	}

	/// Commit the current transaction
	///
	/// # Examples
	///
	/// ```no_run
	/// # async fn example() {
	/// use reinhardt_db::orm::connection::DatabaseConnection;
	///
	/// let conn = DatabaseConnection::connect("sqlite::memory:").await.unwrap();
	/// conn.begin_transaction().await.unwrap();
	/// // ... perform operations ...
	/// let result = conn.commit_transaction().await;
	/// assert!(result.is_ok());
	/// # }
	/// # tokio::runtime::Runtime::new().unwrap().block_on(example());
	/// ```
	pub async fn commit_transaction(&self) -> Result<(), anyhow::Error> {
		self.execute("COMMIT", vec![]).await?;
		Ok(())
	}

	/// Rollback the current transaction
	///
	/// # Examples
	///
	/// ```no_run
	/// # async fn example() {
	/// use reinhardt_db::orm::connection::DatabaseConnection;
	///
	/// let conn = DatabaseConnection::connect("sqlite::memory:").await.unwrap();
	/// conn.begin_transaction().await.unwrap();
	/// // ... error occurs ...
	/// let result = conn.rollback_transaction().await;
	/// assert!(result.is_ok());
	/// # }
	/// # tokio::runtime::Runtime::new().unwrap().block_on(example());
	/// ```
	pub async fn rollback_transaction(&self) -> Result<(), anyhow::Error> {
		self.execute("ROLLBACK", vec![]).await?;
		Ok(())
	}

	/// Create a savepoint for nested transactions
	///
	/// # Examples
	///
	/// ```no_run
	/// # async fn example() {
	/// use reinhardt_db::orm::connection::DatabaseConnection;
	///
	/// let conn = DatabaseConnection::connect("sqlite::memory:").await.unwrap();
	/// conn.begin_transaction().await.unwrap();
	/// let result = conn.savepoint("sp1").await;
	/// assert!(result.is_ok());
	/// // ... nested operations ...
	/// conn.release_savepoint("sp1").await.unwrap();
	/// # }
	/// # tokio::runtime::Runtime::new().unwrap().block_on(example());
	/// ```
	pub async fn savepoint(&self, name: &str) -> Result<(), anyhow::Error> {
		let sql = format!("SAVEPOINT {}", name);
		self.execute(&sql, vec![]).await?;
		Ok(())
	}

	/// Release a savepoint
	pub async fn release_savepoint(&self, name: &str) -> Result<(), anyhow::Error> {
		let sql = format!("RELEASE SAVEPOINT {}", name);
		self.execute(&sql, vec![]).await?;
		Ok(())
	}

	/// Rollback to a savepoint
	pub async fn rollback_to_savepoint(&self, name: &str) -> Result<(), anyhow::Error> {
		let sql = format!("ROLLBACK TO SAVEPOINT {}", name);
		self.execute(&sql, vec![]).await?;
		Ok(())
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
	///
	/// # Example
	///
	/// ```no_run
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// use reinhardt_db::orm::connection::DatabaseConnection;
	///
	/// let conn = DatabaseConnection::connect("postgres://localhost/mydb").await?;
	/// let mut tx = conn.begin().await?;
	///
	/// tx.execute("INSERT INTO users (name) VALUES ($1)", vec!["Alice".into()]).await?;
	/// tx.commit().await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn begin(&self) -> Result<Box<dyn TransactionExecutor>, anyhow::Error> {
		Ok(self.inner.begin().await?)
	}

	/// Begin a transaction with a specific isolation level using TransactionExecutor
	///
	/// This method returns a `TransactionExecutor` that provides dedicated connection
	/// semantics with the specified isolation level. All queries executed through
	/// the returned executor are guaranteed to run on the same physical connection.
	///
	/// # Arguments
	///
	/// * `level` - The desired isolation level for the transaction
	///
	/// # Examples
	///
	/// ```no_run
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// use reinhardt_db::orm::connection::{DatabaseConnection, IsolationLevel};
	///
	/// let conn = DatabaseConnection::connect("postgres://localhost/mydb").await?;
	/// let mut tx = conn.begin_with_isolation(IsolationLevel::Serializable).await?;
	///
	/// tx.execute("INSERT INTO users (name) VALUES ($1)", vec!["Alice".into()]).await?;
	/// tx.commit().await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn begin_with_isolation(
		&self,
		level: IsolationLevel,
	) -> Result<Box<dyn TransactionExecutor>, anyhow::Error> {
		Ok(self.inner.begin_with_isolation(level).await?)
	}
}

#[async_trait]
impl DatabaseExecutor for DatabaseConnection {
	async fn execute(&self, sql: &str) -> Result<u64, anyhow::Error> {
		self.execute(sql, vec![]).await
	}

	async fn query(&self, sql: &str) -> Result<Vec<QueryRow>, anyhow::Error> {
		self.query(sql, vec![]).await
	}
}

/// Injectable implementation for DatabaseConnection
///
/// DatabaseConnection must be explicitly registered in the DI context using
/// `InjectionContextBuilder::singleton()`. It cannot be auto-injected because
/// it requires runtime configuration (connection URL, pool settings, etc.).
///
/// # Example
///
/// ```rust,no_run
/// use reinhardt_db::orm::DatabaseConnection;
/// use reinhardt_di::InjectionContext;
///
/// # async fn example() {
/// // First, establish a database connection
/// let db = DatabaseConnection::connect("postgres://localhost/mydb").await.unwrap();
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
		// Try singleton scope first (primary expected location)
		if let Some(conn) = ctx.get_singleton::<Self>() {
			return Ok(std::sync::Arc::try_unwrap(conn).unwrap_or_else(|arc| (*arc).clone()));
		}

		// Try request scope as fallback
		if let Some(conn) = ctx.get_request::<Self>() {
			return Ok(std::sync::Arc::try_unwrap(conn).unwrap_or_else(|arc| (*arc).clone()));
		}

		// Not registered - provide helpful error
		Err(reinhardt_di::DiError::NotRegistered {
			type_name: std::any::type_name::<Self>().to_string(),
			hint: "Use InjectionContextBuilder::singleton(db_connection) to register a \
			       DatabaseConnection. Create it with DatabaseConnection::connect(), \
			       connect_postgres(), connect_sqlite(), or connect_mysql()."
				.to_string(),
		})
	}

	async fn inject_uncached(ctx: &reinhardt_di::InjectionContext) -> reinhardt_di::DiResult<Self> {
		// For DatabaseConnection, inject_uncached behaves the same as inject
		// since database connections are typically shared (singleton or request-scoped)
		Self::inject(ctx).await
	}
}
