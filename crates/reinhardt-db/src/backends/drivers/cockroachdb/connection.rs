//! CockroachDB Connection Wrapper
//!
//! This module provides a connection wrapper for CockroachDB that extends
//! PostgreSQL connectivity with CockroachDB-specific features and optimizations.

use sqlx::{PgPool, Row};
use std::sync::Arc;
use std::time::Duration;

use crate::backends::error::{DatabaseError, Result};

/// CockroachDB connection configuration
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct CockroachDBConnectionConfig {
	/// Connection URL
	pub url: String,
	/// Maximum number of connections in the pool
	pub max_connections: u32,
	/// Minimum number of idle connections
	pub min_connections: u32,
	/// Connection timeout
	pub connect_timeout: Duration,
	/// Idle timeout for connections
	pub idle_timeout: Duration,
	/// Application name for connection tracking
	pub application_name: Option<String>,
}

impl Default for CockroachDBConnectionConfig {
	fn default() -> Self {
		Self {
			url: "postgresql://localhost:26257/defaultdb".to_string(),
			max_connections: 10,
			min_connections: 2,
			connect_timeout: Duration::from_secs(30),
			idle_timeout: Duration::from_secs(600),
			application_name: None,
		}
	}
}

impl CockroachDBConnectionConfig {
	/// Create a new configuration from a connection URL
	///
	/// # Example
	///
	/// ```rust
	/// # use reinhardt_db::backends::drivers::cockroachdb::connection::CockroachDBConnectionConfig;
	/// let config = CockroachDBConnectionConfig::new("postgresql://localhost:26257/mydb");
	/// assert_eq!(config.url, "postgresql://localhost:26257/mydb");
	/// assert_eq!(config.max_connections, 10); // Default value
	/// ```
	pub fn new(url: impl Into<String>) -> Self {
		Self {
			url: url.into(),
			..Default::default()
		}
	}

	/// Set maximum number of connections
	///
	/// # Example
	///
	/// ```rust
	/// # use reinhardt_db::backends::drivers::cockroachdb::connection::CockroachDBConnectionConfig;
	/// let config = CockroachDBConnectionConfig::new("postgresql://localhost:26257/mydb")
	///     .with_max_connections(20);
	/// assert_eq!(config.max_connections, 20);
	/// assert_eq!(config.url, "postgresql://localhost:26257/mydb");
	/// ```
	pub fn with_max_connections(mut self, max: u32) -> Self {
		self.max_connections = max;
		self
	}

	/// Set minimum number of idle connections
	///
	/// # Example
	///
	/// ```rust
	/// # use reinhardt_db::backends::drivers::cockroachdb::connection::CockroachDBConnectionConfig;
	/// let config = CockroachDBConnectionConfig::new("postgresql://localhost:26257/mydb")
	///     .with_min_connections(5);
	/// assert_eq!(config.min_connections, 5);
	/// assert_eq!(config.max_connections, 10); // Default value
	/// ```
	pub fn with_min_connections(mut self, min: u32) -> Self {
		self.min_connections = min;
		self
	}

	/// Set connection timeout
	///
	/// # Example
	///
	/// ```rust
	/// # use reinhardt_db::backends::drivers::cockroachdb::connection::CockroachDBConnectionConfig;
	/// # use std::time::Duration;
	/// let config = CockroachDBConnectionConfig::new("postgresql://localhost:26257/mydb")
	///     .with_connect_timeout(Duration::from_secs(10));
	/// assert_eq!(config.connect_timeout, Duration::from_secs(10));
	/// assert_eq!(config.url, "postgresql://localhost:26257/mydb");
	/// ```
	pub fn with_connect_timeout(mut self, timeout: Duration) -> Self {
		self.connect_timeout = timeout;
		self
	}

	/// Set idle timeout
	///
	/// # Example
	///
	/// ```rust
	/// # use reinhardt_db::backends::drivers::cockroachdb::connection::CockroachDBConnectionConfig;
	/// # use std::time::Duration;
	/// let config = CockroachDBConnectionConfig::new("postgresql://localhost:26257/mydb")
	///     .with_idle_timeout(Duration::from_secs(300));
	/// assert_eq!(config.idle_timeout, Duration::from_secs(300));
	/// assert_eq!(config.connect_timeout, Duration::from_secs(30)); // Default value
	/// ```
	pub fn with_idle_timeout(mut self, timeout: Duration) -> Self {
		self.idle_timeout = timeout;
		self
	}

	/// Set application name
	///
	/// # Example
	///
	/// ```rust
	/// # use reinhardt_db::backends::drivers::cockroachdb::connection::CockroachDBConnectionConfig;
	/// let config = CockroachDBConnectionConfig::new("postgresql://localhost:26257/mydb")
	///     .with_application_name("my-app");
	/// assert_eq!(config.application_name, Some("my-app".to_string()));
	/// assert_eq!(config.url, "postgresql://localhost:26257/mydb");
	/// ```
	pub fn with_application_name(mut self, name: impl Into<String>) -> Self {
		self.application_name = Some(name.into());
		self
	}
}

/// CockroachDB connection wrapper
///
/// Wraps a PostgreSQL connection pool with CockroachDB-specific functionality.
///
/// # Examples
///
/// ```no_run
/// use reinhardt_db::backends::drivers::cockroachdb::connection::{
///     CockroachDBConnection, CockroachDBConnectionConfig
/// };
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let config = CockroachDBConnectionConfig::new("postgresql://localhost:26257/mydb");
/// let conn = CockroachDBConnection::connect(config).await?;
///
/// // Check if connection is valid
/// assert!(conn.ping().await.is_ok());
/// # Ok(())
/// # }
/// ```
#[derive(Clone)]
pub struct CockroachDBConnection {
	pool: Arc<PgPool>,
}

impl CockroachDBConnection {
	/// Connect to CockroachDB using the provided configuration
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_db::backends::drivers::cockroachdb::connection::{
	///     CockroachDBConnection, CockroachDBConnectionConfig
	/// };
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let config = CockroachDBConnectionConfig::new("postgresql://localhost:26257/mydb");
	/// let conn = CockroachDBConnection::connect(config).await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn connect(config: CockroachDBConnectionConfig) -> Result<Self> {
		let mut url = config.url.clone();
		if let Some(app_name) = config.application_name {
			url = format!("{}?application_name={}", url, app_name);
		}

		let pool = PgPool::connect(&url).await.map_err(DatabaseError::from)?;

		Ok(Self {
			pool: Arc::new(pool),
		})
	}

	/// Create from an existing PgPool
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_db::backends::drivers::cockroachdb::connection::CockroachDBConnection;
	/// use sqlx::PgPool;
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let pool = PgPool::connect("postgresql://localhost:26257/mydb").await?;
	/// let conn = CockroachDBConnection::from_pool(pool);
	/// # Ok(())
	/// # }
	/// ```
	pub fn from_pool(pool: PgPool) -> Self {
		Self {
			pool: Arc::new(pool),
		}
	}

	/// Create from an `Arc<PgPool>`
	pub fn from_pool_arc(pool: Arc<PgPool>) -> Self {
		Self { pool }
	}

	/// Get a reference to the underlying pool
	pub fn pool(&self) -> &PgPool {
		&self.pool
	}

	/// Get an Arc reference to the underlying pool
	pub fn pool_arc(&self) -> Arc<PgPool> {
		Arc::clone(&self.pool)
	}

	/// Ping the database to check connection health
	///
	/// # Examples
	///
	/// ```no_run
	/// # use reinhardt_db::backends::drivers::cockroachdb::connection::{
	/// #     CockroachDBConnection, CockroachDBConnectionConfig
	/// # };
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// # let config = CockroachDBConnectionConfig::new("postgresql://localhost:26257/mydb");
	/// # let conn = CockroachDBConnection::connect(config).await?;
	/// assert!(conn.ping().await.is_ok());
	/// # Ok(())
	/// # }
	/// ```
	pub async fn ping(&self) -> Result<()> {
		sqlx::query("SELECT 1")
			.execute(self.pool.as_ref())
			.await
			.map_err(DatabaseError::from)?;
		Ok(())
	}

	/// Get CockroachDB version
	///
	/// # Examples
	///
	/// ```no_run
	/// # use reinhardt_db::backends::drivers::cockroachdb::connection::{
	/// #     CockroachDBConnection, CockroachDBConnectionConfig
	/// # };
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// # let config = CockroachDBConnectionConfig::new("postgresql://localhost:26257/mydb");
	/// # let conn = CockroachDBConnection::connect(config).await?;
	/// let version = conn.version().await?;
	/// println!("CockroachDB version: {}", version);
	/// # Ok(())
	/// # }
	/// ```
	pub async fn version(&self) -> Result<String> {
		let row = sqlx::query("SELECT version()")
			.fetch_one(self.pool.as_ref())
			.await
			.map_err(DatabaseError::from)?;

		row.try_get(0).map_err(DatabaseError::from)
	}

	/// Get current database name
	///
	/// # Examples
	///
	/// ```no_run
	/// # use reinhardt_db::backends::drivers::cockroachdb::connection::{
	/// #     CockroachDBConnection, CockroachDBConnectionConfig
	/// # };
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// # let config = CockroachDBConnectionConfig::new("postgresql://localhost:26257/mydb");
	/// # let conn = CockroachDBConnection::connect(config).await?;
	/// let db_name = conn.current_database().await?;
	/// println!("Current database: {}", db_name);
	/// # Ok(())
	/// # }
	/// ```
	pub async fn current_database(&self) -> Result<String> {
		let row = sqlx::query("SELECT current_database()")
			.fetch_one(self.pool.as_ref())
			.await
			.map_err(DatabaseError::from)?;

		row.try_get(0).map_err(DatabaseError::from)
	}

	/// List all regions in the cluster
	///
	/// # Examples
	///
	/// ```no_run
	/// # use reinhardt_db::backends::drivers::cockroachdb::connection::{
	/// #     CockroachDBConnection, CockroachDBConnectionConfig
	/// # };
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// # let config = CockroachDBConnectionConfig::new("postgresql://localhost:26257/mydb");
	/// # let conn = CockroachDBConnection::connect(config).await?;
	/// let regions = conn.list_regions().await?;
	/// for region in regions {
	///     println!("Region: {}", region);
	/// }
	/// # Ok(())
	/// # }
	/// ```
	pub async fn list_regions(&self) -> Result<Vec<String>> {
		let rows = sqlx::query("SHOW REGIONS")
			.fetch_all(self.pool.as_ref())
			.await
			.map_err(DatabaseError::from)?;

		let mut regions = Vec::new();
		for row in rows {
			let region: String = row.try_get(0).map_err(DatabaseError::from)?;
			regions.push(region);
		}

		Ok(regions)
	}

	/// Get the primary region for the current database
	///
	/// # Examples
	///
	/// ```no_run
	/// # use reinhardt_db::backends::drivers::cockroachdb::connection::{
	/// #     CockroachDBConnection, CockroachDBConnectionConfig
	/// # };
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// # let config = CockroachDBConnectionConfig::new("postgresql://localhost:26257/mydb");
	/// # let conn = CockroachDBConnection::connect(config).await?;
	/// if let Some(region) = conn.primary_region().await? {
	///     println!("Primary region: {}", region);
	/// }
	/// # Ok(())
	/// # }
	/// ```
	pub async fn primary_region(&self) -> Result<Option<String>> {
		let row = sqlx::query("SHOW PRIMARY REGION")
			.fetch_optional(self.pool.as_ref())
			.await
			.map_err(DatabaseError::from)?;

		if let Some(row) = row {
			Ok(Some(row.try_get(0).map_err(DatabaseError::from)?))
		} else {
			Ok(None)
		}
	}

	/// Close the connection pool
	///
	/// # Examples
	///
	/// ```no_run
	/// # use reinhardt_db::backends::drivers::cockroachdb::connection::{
	/// #     CockroachDBConnection, CockroachDBConnectionConfig
	/// # };
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// # let config = CockroachDBConnectionConfig::new("postgresql://localhost:26257/mydb");
	/// let conn = CockroachDBConnection::connect(config).await?;
	/// conn.close().await;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn close(&self) {
		self.pool.close().await;
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_config_default() {
		let config = CockroachDBConnectionConfig::default();
		assert_eq!(config.url, "postgresql://localhost:26257/defaultdb");
		assert_eq!(config.max_connections, 10);
		assert_eq!(config.min_connections, 2);
	}

	#[test]
	fn test_config_new() {
		let config = CockroachDBConnectionConfig::new("postgresql://localhost:26257/mydb");
		assert_eq!(config.url, "postgresql://localhost:26257/mydb");
	}

	#[test]
	fn test_config_with_max_connections() {
		let config = CockroachDBConnectionConfig::new("postgresql://localhost:26257/mydb")
			.with_max_connections(20);
		assert_eq!(config.max_connections, 20);
	}

	#[test]
	fn test_config_with_min_connections() {
		let config = CockroachDBConnectionConfig::new("postgresql://localhost:26257/mydb")
			.with_min_connections(5);
		assert_eq!(config.min_connections, 5);
	}

	#[test]
	fn test_config_with_connect_timeout() {
		let config = CockroachDBConnectionConfig::new("postgresql://localhost:26257/mydb")
			.with_connect_timeout(Duration::from_secs(10));
		assert_eq!(config.connect_timeout, Duration::from_secs(10));
	}

	#[test]
	fn test_config_with_idle_timeout() {
		let config = CockroachDBConnectionConfig::new("postgresql://localhost:26257/mydb")
			.with_idle_timeout(Duration::from_secs(300));
		assert_eq!(config.idle_timeout, Duration::from_secs(300));
	}

	#[test]
	fn test_config_with_application_name() {
		let config = CockroachDBConnectionConfig::new("postgresql://localhost:26257/mydb")
			.with_application_name("my-app");
		assert_eq!(config.application_name, Some("my-app".to_string()));
	}

	#[test]
	fn test_config_chaining() {
		let config = CockroachDBConnectionConfig::new("postgresql://localhost:26257/mydb")
			.with_max_connections(20)
			.with_min_connections(5)
			.with_connect_timeout(Duration::from_secs(10))
			.with_application_name("my-app");

		assert_eq!(config.max_connections, 20);
		assert_eq!(config.min_connections, 5);
		assert_eq!(config.connect_timeout, Duration::from_secs(10));
		assert_eq!(config.application_name, Some("my-app".to_string()));
	}

	#[tokio::test]
	async fn test_connection_from_pool() {
		let pool = PgPool::connect_lazy("postgresql://localhost:26257/testdb")
			.expect("Failed to create lazy pool");
		let conn = CockroachDBConnection::from_pool(pool);

		assert!(Arc::strong_count(&conn.pool) >= 1);
	}

	#[tokio::test]
	async fn test_connection_clone() {
		let pool = Arc::new(
			PgPool::connect_lazy("postgresql://localhost:26257/testdb")
				.expect("Failed to create lazy pool"),
		);
		let conn1 = CockroachDBConnection::from_pool_arc(pool.clone());
		let conn2 = conn1.clone();

		// Both should reference the same pool
		assert!(Arc::ptr_eq(&conn1.pool, &conn2.pool));
	}
}
