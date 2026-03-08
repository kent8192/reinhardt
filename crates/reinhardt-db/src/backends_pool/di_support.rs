//! Dependency injection support for database pools

use super::config::PoolConfig;

/// Database service for dependency injection
#[derive(Clone)]
pub struct DatabaseService {
	pool: std::sync::Arc<dyn std::any::Any + Send + Sync>,
}

impl DatabaseService {
	/// Creates a new instance.
	pub fn new<T: 'static + Send + Sync>(pool: T) -> Self {
		Self {
			pool: std::sync::Arc::new(pool),
		}
	}

	/// Performs the get operation.
	pub fn get<T: 'static>(&self) -> Option<&T> {
		self.pool.downcast_ref::<T>()
	}
}

/// Database URL wrapper
#[derive(Debug, Clone)]
pub struct DatabaseUrl(pub String);

impl DatabaseUrl {
	/// Creates a new instance.
	pub fn new(url: impl Into<String>) -> Self {
		Self(url.into())
	}

	/// Performs the as str operation.
	pub fn as_str(&self) -> &str {
		&self.0
	}
}

/// MySQL pool manager
pub struct MySqlManager {
	_config: PoolConfig,
}

impl MySqlManager {
	/// Creates a new instance.
	pub fn new(config: PoolConfig) -> Self {
		Self { _config: config }
	}
}

/// PostgreSQL pool manager
pub struct PostgresManager {
	_config: PoolConfig,
}

impl PostgresManager {
	/// Creates a new instance.
	pub fn new(config: PoolConfig) -> Self {
		Self { _config: config }
	}
}

/// SQLite pool manager
pub struct SqliteManager {
	_config: PoolConfig,
}

impl SqliteManager {
	/// Creates a new instance.
	pub fn new(config: PoolConfig) -> Self {
		Self { _config: config }
	}
}

// Type aliases for convenient use in dependency injection
/// Type alias for my sql pool.
pub type MySqlPool = super::pool::ConnectionPool<sqlx::MySql>;
/// Type alias for postgres pool.
pub type PostgresPool = super::pool::ConnectionPool<sqlx::Postgres>;
/// Type alias for sqlite pool.
pub type SqlitePool = super::pool::ConnectionPool<sqlx::Sqlite>;
