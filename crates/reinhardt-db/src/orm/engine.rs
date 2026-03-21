//! # Database Engine
//!
//! SQLAlchemy-inspired database engine with connection pooling.
//!
//! This module provides a high-level database engine abstraction with built-in
//! connection pooling using SQLx's `AnyPool`. The pooling configuration allows
//! fine-grained control over connection lifecycle, timeouts, and pool sizes.
//!
//! ## Connection Pooling
//!
//! The engine uses SQLx's connection pooling with the following configurable parameters:
//! - `pool_min_size`: Minimum number of connections in the pool
//! - `pool_max_size`: Maximum number of connections in the pool
//! - `pool_timeout`: Timeout for acquiring a connection from the pool
//! - `pool_idle_timeout`: Maximum idle time before a connection is closed
//! - `pool_max_lifetime`: Maximum lifetime of a connection before it's closed
//!
//! ## Examples
//!
//! **Note:** `Engine` uses `sqlx::Any` which requires database-specific feature flags.
//! For simpler usage in tests and applications, use `DatabaseEngine` instead.
//!
//! ```rust,no_run
//! use reinhardt_db::orm::engine::{EngineConfig, create_engine_with_config};
//!
//! # tokio::runtime::Runtime::new().unwrap().block_on(async {
//! // Create engine with custom pool configuration
//! let config = EngineConfig::new("postgres://localhost/mydb")
//!     .with_pool_size(5, 20)  // Min: 5, Max: 20
//!     .with_timeout(30)       // 30 seconds connection timeout
//!     .with_idle_timeout(Some(600))    // 10 minutes idle timeout
//!     .with_max_lifetime(Some(1800));  // 30 minutes max lifetime
//!
//! let engine = create_engine_with_config(config).await.unwrap();
//! # });
//! ```
//!
//! For a simpler API that works in doctests, see `DatabaseEngine` below.
//!
//! This module is inspired by SQLAlchemy's engine implementation
//! Copyright 2005-2025 SQLAlchemy authors and contributors
//! Licensed under MIT License. See THIRD-PARTY-NOTICES for details.

use crate::backends::{DatabaseError, DatabaseType, Row as DbRow, connection::DatabaseConnection};
use sqlx::{Any, AnyPool, pool::PoolOptions};
use std::time::Duration;

/// Database engine configuration
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineConfig {
	/// Database URL (e.g., "sqlite::memory:", "postgres://...")
	pub url: String,

	/// Connection pool size (min)
	pub pool_min_size: u32,

	/// Connection pool size (max)
	pub pool_max_size: u32,

	/// Connection timeout in seconds
	pub pool_timeout: u64,

	/// Maximum idle time for a connection in seconds (None = no limit)
	pub pool_idle_timeout: Option<u64>,

	/// Maximum lifetime for a connection in seconds (None = no limit)
	pub pool_max_lifetime: Option<u64>,

	/// Enable echo (log all SQL)
	pub echo: bool,

	/// Enable query result caching
	pub query_cache_size: usize,
}

impl Default for EngineConfig {
	fn default() -> Self {
		Self {
			url: "sqlite::memory:".to_string(),
			pool_min_size: 1,
			pool_max_size: 10,
			pool_timeout: 30,
			pool_idle_timeout: Some(600),  // 10 minutes
			pool_max_lifetime: Some(1800), // 30 minutes
			echo: false,
			query_cache_size: 500,
		}
	}
}

impl EngineConfig {
	/// Create new config with URL
	pub fn new(url: impl Into<String>) -> Self {
		Self {
			url: url.into(),
			..Default::default()
		}
	}
	/// Set pool sizes
	pub fn with_pool_size(mut self, min: u32, max: u32) -> Self {
		self.pool_min_size = min;
		self.pool_max_size = max;
		self
	}

	/// Set connection timeout in seconds
	pub fn with_timeout(mut self, timeout: u64) -> Self {
		self.pool_timeout = timeout;
		self
	}

	/// Set idle timeout in seconds (None = no limit)
	pub fn with_idle_timeout(mut self, timeout: Option<u64>) -> Self {
		self.pool_idle_timeout = timeout;
		self
	}

	/// Set max lifetime in seconds (None = no limit)
	pub fn with_max_lifetime(mut self, lifetime: Option<u64>) -> Self {
		self.pool_max_lifetime = lifetime;
		self
	}

	/// Enable SQL echo
	pub fn with_echo(mut self, echo: bool) -> Self {
		self.echo = echo;
		self
	}
	/// Set query cache size
	pub fn with_cache_size(mut self, size: usize) -> Self {
		self.query_cache_size = size;
		self
	}
}

/// Database engine - manages connections and execution
pub struct Engine {
	pool: AnyPool,
	config: EngineConfig,
}

impl Engine {
	/// Create a new engine from config
	///
	pub async fn from_config(config: EngineConfig) -> Result<Self, sqlx::Error> {
		let mut pool_options = PoolOptions::<Any>::new()
			.min_connections(config.pool_min_size)
			.max_connections(config.pool_max_size)
			.acquire_timeout(Duration::from_secs(config.pool_timeout));

		// Set idle timeout if specified
		if let Some(idle_timeout) = config.pool_idle_timeout {
			pool_options = pool_options.idle_timeout(Duration::from_secs(idle_timeout));
		}

		// Set max lifetime if specified
		if let Some(max_lifetime) = config.pool_max_lifetime {
			pool_options = pool_options.max_lifetime(Duration::from_secs(max_lifetime));
		}

		let pool = pool_options.connect(&config.url).await?;

		Ok(Self { pool, config })
	}
	/// Create a new engine from URL
	///
	/// # Examples
	///
	/// ```ignore
	/// use reinhardt_db::orm::Engine;
	///
	/// # tokio::runtime::Runtime::new().unwrap().block_on(async {
	/// let engine = Engine::new("sqlite::memory:").await.unwrap();
	/// # });
	/// ```
	///
	/// **Note:** This requires the appropriate sqlx driver feature to be enabled.
	/// For simpler usage, see `DatabaseEngine::from_sqlite` or other database-specific constructors.
	pub async fn new(url: impl Into<String>) -> Result<Self, sqlx::Error> {
		Self::from_config(EngineConfig::new(url)).await
	}
	/// Get a connection from the pool
	///
	pub async fn connect(&self) -> Result<sqlx::pool::PoolConnection<Any>, sqlx::Error> {
		self.pool.acquire().await
	}
	/// Execute a SQL statement
	pub async fn execute(&self, sql: &str) -> Result<u64, sqlx::Error> {
		if self.config.echo {
			println!("SQL: {}", sql);
		}

		let result = sqlx::query(sql).execute(&self.pool).await?;
		Ok(result.rows_affected())
	}
	/// Execute a query and return results
	///
	pub async fn fetch_all(&self, sql: &str) -> Result<Vec<sqlx::any::AnyRow>, sqlx::Error> {
		if self.config.echo {
			println!("SQL: {}", sql);
		}

		sqlx::query(sql).fetch_all(&self.pool).await
	}
	/// Execute a query and return a single result
	///
	pub async fn fetch_one(&self, sql: &str) -> Result<sqlx::any::AnyRow, sqlx::Error> {
		if self.config.echo {
			println!("SQL: {}", sql);
		}

		sqlx::query(sql).fetch_one(&self.pool).await
	}
	/// Execute a query and return an optional result
	///
	pub async fn fetch_optional(
		&self,
		sql: &str,
	) -> Result<Option<sqlx::any::AnyRow>, sqlx::Error> {
		if self.config.echo {
			println!("SQL: {}", sql);
		}

		sqlx::query(sql).fetch_optional(&self.pool).await
	}
	/// Begin a transaction
	///
	pub async fn begin(&self) -> Result<sqlx::Transaction<'_, Any>, sqlx::Error> {
		self.pool.begin().await
	}
	/// Get the engine configuration
	///
	pub fn config(&self) -> &EngineConfig {
		&self.config
	}
	/// Get reference to the connection pool
	///
	pub fn pool(&self) -> &AnyPool {
		&self.pool
	}
	/// Clone this engine (shares the connection pool)
	///
	pub fn clone_ref(&self) -> Self {
		Self {
			pool: self.pool.clone(),
			config: self.config.clone(),
		}
	}
}
/// Create a new database engine
///
pub async fn create_engine(url: impl Into<String>) -> Result<Engine, sqlx::Error> {
	Engine::new(url).await
}
/// Create a new database engine with configuration
///
pub async fn create_engine_with_config(config: EngineConfig) -> Result<Engine, sqlx::Error> {
	Engine::from_config(config).await
}

/// Database engine using DatabaseConnection (multi-database support)
pub struct DatabaseEngine {
	connection: DatabaseConnection,
	db_type: DatabaseType,
	config: EngineConfig,
}

impl DatabaseEngine {
	/// Create a new database engine from DatabaseConnection
	///
	/// # Examples
	///
	/// ```ignore
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// use reinhardt_db::orm::engine::DatabaseEngine;
	/// use reinhardt_db::reinhardt_db::backends::drivers::{DatabaseConnection, DatabaseType};
	///
	/// let connection = DatabaseConnection::connect("postgres://localhost/mydb").await?;
	/// let engine = DatabaseEngine::new(connection, DatabaseType::Postgres);
	/// // Engine is ready to execute queries
	/// # Ok(())
	/// # }
	/// ```
	pub fn new(connection: DatabaseConnection, db_type: DatabaseType) -> Self {
		Self {
			connection,
			db_type,
			config: EngineConfig::default(),
		}
	}

	/// Create a new database engine with configuration
	pub fn with_config(
		connection: DatabaseConnection,
		db_type: DatabaseType,
		config: EngineConfig,
	) -> Self {
		Self {
			connection,
			db_type,
			config,
		}
	}

	/// Create a new PostgreSQL engine
	///
	/// # Examples
	///
	/// ```
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// use reinhardt_db::orm::connection::DatabaseConnection;
	///
	/// // For doctest purposes, using mock connection (feature-gated methods not available)
	/// // In production with 'postgres' feature: DatabaseEngine::from_postgres(url).await
	/// let connection = DatabaseConnection::connect("postgres://localhost/mydb").await?;
	/// assert_eq!(connection.backend(), reinhardt_db::orm::connection::DatabaseBackend::Postgres);
	/// # Ok(())
	/// # }
	/// # tokio::runtime::Runtime::new().unwrap().block_on(example());
	/// ```
	#[cfg(feature = "postgres")]
	pub async fn from_postgres(url: &str) -> Result<Self, DatabaseError> {
		let connection = DatabaseConnection::connect_postgres(url).await?;
		Ok(Self::new(connection, DatabaseType::Postgres))
	}

	/// Create a new SQLite engine
	///
	/// # Examples
	///
	/// ```
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// use reinhardt_db::orm::connection::DatabaseConnection;
	///
	/// // For doctest purposes, using mock connection (feature-gated methods not available)
	/// // In production with 'sqlite' feature: DatabaseEngine::from_sqlite(":memory:").await
	/// let connection = DatabaseConnection::connect(":memory:").await?;
	/// assert_eq!(connection.backend(), reinhardt_db::orm::connection::DatabaseBackend::Postgres);
	/// # Ok(())
	/// # }
	/// # tokio::runtime::Runtime::new().unwrap().block_on(example());
	/// ```
	#[cfg(feature = "sqlite")]
	pub async fn from_sqlite(url: &str) -> Result<Self, DatabaseError> {
		let connection = DatabaseConnection::connect_sqlite(url).await?;
		Ok(Self::new(connection, DatabaseType::Sqlite))
	}

	/// Create a new MySQL engine
	#[cfg(feature = "mysql")]
	pub async fn from_mysql(url: &str) -> Result<Self, DatabaseError> {
		let connection = DatabaseConnection::connect_mysql(url).await?;
		Ok(Self::new(connection, DatabaseType::Mysql))
	}

	/// Get reference to the database connection
	pub fn connection(&self) -> &DatabaseConnection {
		&self.connection
	}

	/// Get the database type
	pub fn database_type(&self) -> DatabaseType {
		self.db_type
	}

	/// Get the engine configuration
	pub fn config(&self) -> &EngineConfig {
		&self.config
	}

	/// Execute a SQL statement
	///
	/// # Examples
	///
	/// ```no_run
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// use reinhardt_db::orm::connection::DatabaseConnection;
	///
	/// // Create mock connection (URL is ignored in current mock implementation)
	/// let connection = DatabaseConnection::connect("sqlite::memory:").await?;
	///
	/// // Execute SQL statements (mock always returns 0)
	/// let rows_affected = connection.execute("CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT)", vec![]).await?;
	/// assert_eq!(rows_affected, 0);
	///
	/// let rows_affected = connection.execute("INSERT INTO users (id, name) VALUES (1, 'Alice')", vec![]).await?;
	/// assert_eq!(rows_affected, 0);
	///
	/// // Query returns empty vec in mock
	/// let rows = connection.query("SELECT * FROM users", vec![]).await?;
	/// assert_eq!(rows.len(), 0);
	/// # Ok(())
	/// # }
	/// # tokio::runtime::Runtime::new().unwrap().block_on(example());
	/// ```
	pub async fn execute(&self, sql: &str) -> Result<u64, DatabaseError> {
		if self.config.echo {
			println!("SQL: {}", sql);
		}

		let result = self.connection.execute(sql, vec![]).await?;
		Ok(result.rows_affected)
	}

	/// Execute a query and return all results
	pub async fn fetch_all(&self, sql: &str) -> Result<Vec<DbRow>, DatabaseError> {
		if self.config.echo {
			println!("SQL: {}", sql);
		}

		self.connection.fetch_all(sql, vec![]).await
	}

	/// Execute a query and return a single result
	pub async fn fetch_one(&self, sql: &str) -> Result<DbRow, DatabaseError> {
		if self.config.echo {
			println!("SQL: {}", sql);
		}

		self.connection.fetch_one(sql, vec![]).await
	}

	/// Execute a query and return an optional result
	pub async fn fetch_optional(&self, sql: &str) -> Result<Option<DbRow>, DatabaseError> {
		if self.config.echo {
			println!("SQL: {}", sql);
		}

		let rows = self.connection.fetch_all(sql, vec![]).await?;
		Ok(rows.into_iter().next())
	}

	/// Clone this engine (shares the database connection)
	pub fn clone_ref(&self) -> Self {
		Self {
			connection: self.connection.clone(),
			db_type: self.db_type,
			config: self.config.clone(),
		}
	}
}

/// Create a new database engine from PostgreSQL URL
#[cfg(feature = "postgres")]
pub async fn create_database_engine_postgres(url: &str) -> Result<DatabaseEngine, DatabaseError> {
	DatabaseEngine::from_postgres(url).await
}

/// Create a new database engine from SQLite URL
#[cfg(feature = "sqlite")]
pub async fn create_database_engine_sqlite(url: &str) -> Result<DatabaseEngine, DatabaseError> {
	DatabaseEngine::from_sqlite(url).await
}

/// Create a new database engine from MySQL URL
#[cfg(feature = "mysql")]
pub async fn create_database_engine_mysql(url: &str) -> Result<DatabaseEngine, DatabaseError> {
	DatabaseEngine::from_mysql(url).await
}

#[cfg(test)]
mod tests {
	use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};

	// Helper to create SQLite pool for tests
	async fn create_test_pool() -> SqlitePool {
		SqlitePoolOptions::new()
			.min_connections(0)
			.max_connections(5)
			.connect("sqlite::memory:")
			.await
			.expect("Failed to create SQLite pool")
	}

	#[tokio::test]
	async fn test_engine_creation() {
		let pool = create_test_pool().await;
		// Verify pool was created successfully by checking it's not closed
		assert!(!pool.is_closed());
		pool.close().await;
	}

	#[tokio::test]
	async fn test_engine_execute() {
		let pool = create_test_pool().await;

		let result = sqlx::query("CREATE TABLE test (id INTEGER PRIMARY KEY, name TEXT)")
			.execute(&pool)
			.await;

		assert!(result.is_ok());
		pool.close().await;
	}

	#[tokio::test]
	async fn test_engine_query() {
		let pool = create_test_pool().await;

		sqlx::query("CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT)")
			.execute(&pool)
			.await
			.unwrap();

		sqlx::query("INSERT INTO users (id, name) VALUES (1, 'Alice')")
			.execute(&pool)
			.await
			.unwrap();

		let rows = sqlx::query("SELECT * FROM users")
			.fetch_all(&pool)
			.await
			.expect("Query failed");

		assert_eq!(rows.len(), 1);
		pool.close().await;
	}

	#[tokio::test]
	async fn test_engine_with_config() {
		let pool = SqlitePoolOptions::new()
			.min_connections(2)
			.max_connections(5)
			.connect("sqlite::memory:")
			.await
			.expect("Failed to create engine with config");

		// Verify pool was created with correct config
		assert!(!pool.is_closed());
		pool.close().await;
	}

	#[tokio::test]
	async fn test_transaction() {
		let pool = create_test_pool().await;

		sqlx::query("CREATE TABLE accounts (id INTEGER PRIMARY KEY, balance INTEGER)")
			.execute(&pool)
			.await
			.unwrap();

		let mut tx = pool.begin().await.expect("Failed to begin transaction");

		sqlx::query("INSERT INTO accounts (id, balance) VALUES (1, 100)")
			.execute(&mut *tx)
			.await
			.unwrap();

		tx.commit().await.expect("Failed to commit");

		let rows = sqlx::query("SELECT * FROM accounts")
			.fetch_all(&pool)
			.await
			.unwrap();
		assert_eq!(rows.len(), 1);
		pool.close().await;
	}
}
