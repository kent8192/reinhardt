//! Connection pool implementation

use super::config::PoolConfig;
use super::errors::{PoolError, PoolResult};
use super::events::{PoolEvent, PoolEventListener};
use sqlx::{Database, MySql, Pool, Postgres, Sqlite};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::RwLock;

/// Mask the password in a database URL for safe display.
///
/// Handles standard URL formats like `scheme://user:password@host/db`
/// and replaces the password portion with `***`.
/// Correctly handles passwords containing `@` by using the last `@` as
/// the user-info delimiter.
pub(crate) fn mask_url_password(url: &str) -> String {
	// Try to parse as a standard URL with scheme://user:pass@host format
	if let Some(scheme_end) = url.find("://") {
		let after_scheme = &url[scheme_end + 3..];

		// Use the last @ as the user-info delimiter, since passwords may contain @
		if let Some(at_pos) = after_scheme.rfind('@') {
			let user_info = &after_scheme[..at_pos];

			// Find the first colon separating user from password
			if let Some(colon_pos) = user_info.find(':') {
				let scheme_and_user = &url[..scheme_end + 3 + colon_pos + 1];
				let rest = &url[scheme_end + 3 + at_pos..];
				return format!("{}***{}", scheme_and_user, rest);
			}
		}
	}

	// No password found, return as-is
	url.to_string()
}

/// A database connection pool
pub struct ConnectionPool<DB: Database> {
	pool: Pool<DB>,
	config: PoolConfig,
	url: String,
	listeners: Arc<RwLock<Vec<Arc<dyn PoolEventListener>>>>,
	first_connect_fired: Arc<AtomicBool>,
}

impl ConnectionPool<Postgres> {
	/// Create a new PostgreSQL connection pool
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::pool::{ConnectionPool, PoolConfig};
	///
	/// # async fn example() {
	/// let config = PoolConfig::default();
	/// // For doctest purposes, using SQLite in-memory instead of PostgreSQL
	/// let pool = ConnectionPool::new_sqlite("sqlite::memory:", config).await.unwrap();
	/// assert!(pool.url().contains("memory"));
	/// assert_eq!(pool.config().max_connections, 10);
	/// # }
	/// # tokio::runtime::Runtime::new().unwrap().block_on(example());
	/// ```
	pub async fn new_postgres(url: &str, config: PoolConfig) -> PoolResult<Self> {
		config.validate().map_err(PoolError::Config)?;

		let pool = sqlx::postgres::PgPoolOptions::new()
			.min_connections(config.min_connections)
			.max_connections(config.max_connections)
			.acquire_timeout(config.acquire_timeout)
			.idle_timeout(config.idle_timeout)
			.max_lifetime(config.max_lifetime)
			.test_before_acquire(config.test_before_acquire)
			.connect(url)
			.await?;

		Ok(Self {
			pool,
			config,
			url: url.to_string(),
			listeners: Arc::new(RwLock::new(Vec::new())),
			first_connect_fired: Arc::new(AtomicBool::new(false)),
		})
	}
}

impl ConnectionPool<MySql> {
	/// Create a new MySQL connection pool
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::pool::{ConnectionPool, PoolConfig};
	///
	/// # async fn example() {
	/// let config = PoolConfig::default();
	/// // For doctest purposes, using SQLite in-memory instead of MySQL
	/// let pool = ConnectionPool::new_sqlite("sqlite::memory:", config).await.unwrap();
	/// assert!(pool.url().contains("memory"));
	/// assert_eq!(pool.config().max_connections, 10);
	/// # }
	/// # tokio::runtime::Runtime::new().unwrap().block_on(example());
	/// ```
	pub async fn new_mysql(url: &str, config: PoolConfig) -> PoolResult<Self> {
		config.validate().map_err(PoolError::Config)?;

		let pool = sqlx::mysql::MySqlPoolOptions::new()
			.min_connections(config.min_connections)
			.max_connections(config.max_connections)
			.acquire_timeout(config.acquire_timeout)
			.idle_timeout(config.idle_timeout)
			.max_lifetime(config.max_lifetime)
			.test_before_acquire(config.test_before_acquire)
			.connect(url)
			.await?;

		Ok(Self {
			pool,
			config,
			url: url.to_string(),
			listeners: Arc::new(RwLock::new(Vec::new())),
			first_connect_fired: Arc::new(AtomicBool::new(false)),
		})
	}
}

impl ConnectionPool<Sqlite> {
	/// Create a new SQLite connection pool
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::pool::{ConnectionPool, PoolConfig};
	///
	/// # async fn example() {
	/// let config = PoolConfig::default();
	/// // Using in-memory SQLite for doctest
	/// let pool = ConnectionPool::new_sqlite("sqlite::memory:", config).await.unwrap();
	/// assert!(pool.url().contains("memory"));
	/// assert!(pool.config().max_connections > 0);
	/// # }
	/// # tokio::runtime::Runtime::new().unwrap().block_on(example());
	/// ```
	pub async fn new_sqlite(url: &str, config: PoolConfig) -> PoolResult<Self> {
		config.validate().map_err(PoolError::Config)?;

		let pool = sqlx::sqlite::SqlitePoolOptions::new()
			.min_connections(config.min_connections)
			.max_connections(config.max_connections)
			.acquire_timeout(config.acquire_timeout)
			.idle_timeout(config.idle_timeout)
			.max_lifetime(config.max_lifetime)
			.test_before_acquire(config.test_before_acquire)
			.connect(url)
			.await?;

		Ok(Self {
			pool,
			config,
			url: url.to_string(),
			listeners: Arc::new(RwLock::new(Vec::new())),
			first_connect_fired: Arc::new(AtomicBool::new(false)),
		})
	}
}

impl<DB> ConnectionPool<DB>
where
	DB: sqlx::Database,
{
	/// Add an event listener
	///
	pub async fn add_listener(&self, listener: Arc<dyn PoolEventListener>) {
		let mut listeners = self.listeners.write().await;
		listeners.push(listener);
	}

	/// Emit an event to all listeners
	pub(crate) async fn emit_event(&self, event: PoolEvent) {
		let listeners = self.listeners.read().await;
		for listener in listeners.iter() {
			listener.on_event(event.clone()).await;
		}
	}
	/// Acquire a connection from the pool with event emission
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_db::pool::{ConnectionPool, PoolConfig};
	///
	/// # async fn example() {
	/// let config = PoolConfig::default();
	/// let pool = ConnectionPool::new_postgres("postgresql://user:pass@localhost/test", config)
	///     .await
	///     .unwrap();
	///
	/// // Acquire a connection
	/// let conn = pool.acquire().await;
	/// assert!(conn.is_ok());
	/// # }
	/// ```
	pub async fn acquire(&self) -> PoolResult<PooledConnection<DB>> {
		// Check if this is the first connection
		let is_first = !self.first_connect_fired.swap(true, Ordering::SeqCst);

		let conn = self.pool.acquire().await?;
		let connection_id = uuid::Uuid::new_v4().to_string();

		if is_first {
			// Emit first_connect event (using ConnectionCreated as proxy)
			self.emit_event(PoolEvent::connection_created(connection_id.clone()))
				.await;
		}

		// Emit checkout event
		self.emit_event(PoolEvent::connection_acquired(connection_id.clone()))
			.await;

		Ok(PooledConnection {
			conn,
			pool_ref: self.clone_arc(),
			connection_id,
		})
	}

	/// Clone as Arc for sharing with PooledConnection
	fn clone_arc(&self) -> Arc<Self> {
		Arc::new(Self {
			pool: self.pool.clone(),
			config: self.config.clone(),
			url: self.url.clone(),
			listeners: self.listeners.clone(),
			first_connect_fired: self.first_connect_fired.clone(),
		})
	}
	/// Get the underlying pool
	///
	pub fn inner(&self) -> &Pool<DB> {
		&self.pool
	}
	/// Get pool configuration
	///
	pub fn config(&self) -> &PoolConfig {
		&self.config
	}
	/// Close the pool
	///
	/// Attempts to gracefully close the pool with a 5-second timeout.
	/// If active connections are not returned within this time, the pool
	/// will be forcefully closed.
	pub async fn close(&self) {
		use tokio::time::{Duration, timeout};

		// Try to close gracefully with a timeout
		let close_future = self.pool.close();
		if timeout(Duration::from_secs(5), close_future).await.is_err() {
			// Timeout occurred - pool had active connections
			// The pool will be forcefully closed when dropped
		}
	}
	/// Get the database URL with password masked for safe display
	///
	/// Returns the database URL with any password replaced by `***`
	/// to prevent credential exposure in logs and debug output.
	/// Use `url_raw()` when the actual password is needed for reconnection.
	pub fn url(&self) -> String {
		mask_url_password(&self.url)
	}

	/// Get the raw database URL including credentials
	///
	/// This method returns the unmasked URL containing the actual password.
	/// Use with caution - prefer `url()` for logging and display purposes.
	// Allow dead_code: preserved for internal use by reconnection logic (e.g., `recreate()`)
	#[allow(dead_code)]
	pub(crate) fn url_raw(&self) -> &str {
		&self.url
	}
}

// Database-specific recreate implementations
impl ConnectionPool<Postgres> {
	/// Recreate the pool with the same configuration
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::pool::{ConnectionPool, PoolConfig};
	///
	/// # async fn example() {
	/// let config = PoolConfig::default();
	/// // For doctest purposes, using SQLite in-memory instead of PostgreSQL
	/// let mut pool = ConnectionPool::new_sqlite("sqlite::memory:", config)
	///     .await
	///     .unwrap();
	///
	/// // Recreate the pool
	/// pool.recreate().await.unwrap();
	/// assert_eq!(pool.config().max_connections, 10);
	/// # }
	/// # tokio::runtime::Runtime::new().unwrap().block_on(example());
	/// ```
	pub async fn recreate(&mut self) -> PoolResult<()> {
		// Close existing pool
		self.pool.close().await;

		// Create new pool with same configuration
		let new_pool = sqlx::postgres::PgPoolOptions::new()
			.min_connections(self.config.min_connections)
			.max_connections(self.config.max_connections)
			.acquire_timeout(self.config.acquire_timeout)
			.idle_timeout(self.config.idle_timeout)
			.max_lifetime(self.config.max_lifetime)
			.test_before_acquire(self.config.test_before_acquire)
			.connect(&self.url)
			.await?;

		self.pool = new_pool;
		self.first_connect_fired.store(false, Ordering::SeqCst);

		Ok(())
	}
}

impl ConnectionPool<MySql> {
	/// Recreate the pool with the same configuration
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::pool::{ConnectionPool, PoolConfig};
	///
	/// # async fn example() {
	/// let config = PoolConfig::default();
	/// // For doctest purposes, using SQLite in-memory instead of MySQL
	/// let mut pool = ConnectionPool::new_sqlite("sqlite::memory:", config)
	///     .await
	///     .unwrap();
	///
	/// // Recreate the pool
	/// pool.recreate().await.unwrap();
	/// assert_eq!(pool.config().max_connections, 10);
	/// # }
	/// # tokio::runtime::Runtime::new().unwrap().block_on(example());
	/// ```
	pub async fn recreate(&mut self) -> PoolResult<()> {
		// Close existing pool
		self.pool.close().await;

		// Create new pool with same configuration
		let new_pool = sqlx::mysql::MySqlPoolOptions::new()
			.min_connections(self.config.min_connections)
			.max_connections(self.config.max_connections)
			.acquire_timeout(self.config.acquire_timeout)
			.idle_timeout(self.config.idle_timeout)
			.max_lifetime(self.config.max_lifetime)
			.test_before_acquire(self.config.test_before_acquire)
			.connect(&self.url)
			.await?;

		self.pool = new_pool;
		self.first_connect_fired.store(false, Ordering::SeqCst);

		Ok(())
	}
}

impl ConnectionPool<Sqlite> {
	/// Recreate the pool with the same configuration
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::pool::{ConnectionPool, PoolConfig};
	///
	/// # async fn example() {
	/// let config = PoolConfig::default();
	/// let mut pool = ConnectionPool::new_sqlite("sqlite::memory:", config)
	///     .await
	///     .unwrap();
	///
	/// // Recreate the pool
	/// pool.recreate().await.unwrap();
	/// assert!(pool.url().contains("memory"));
	/// # }
	/// # tokio::runtime::Runtime::new().unwrap().block_on(example());
	/// ```
	pub async fn recreate(&mut self) -> PoolResult<()> {
		// Close existing pool
		self.pool.close().await;

		// Create new pool with same configuration
		let new_pool = sqlx::sqlite::SqlitePoolOptions::new()
			.min_connections(self.config.min_connections)
			.max_connections(self.config.max_connections)
			.acquire_timeout(self.config.acquire_timeout)
			.idle_timeout(self.config.idle_timeout)
			.max_lifetime(self.config.max_lifetime)
			.test_before_acquire(self.config.test_before_acquire)
			.connect(&self.url)
			.await?;

		self.pool = new_pool;
		self.first_connect_fired.store(false, Ordering::SeqCst);

		Ok(())
	}
}

/// A pooled connection wrapper with event emission
pub struct PooledConnection<DB: sqlx::Database> {
	conn: sqlx::pool::PoolConnection<DB>,
	pool_ref: Arc<ConnectionPool<DB>>,
	connection_id: String,
}

impl<DB: sqlx::Database> PooledConnection<DB> {
	/// Documentation for `inner`
	///
	pub fn inner(&mut self) -> &mut sqlx::pool::PoolConnection<DB> {
		&mut self.conn
	}
	/// Get the unique identifier for this connection
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_db::pool::{ConnectionPool, PoolConfig};
	///
	/// # async fn example() {
	/// let config = PoolConfig::default();
	/// let pool = ConnectionPool::new_postgres("postgresql://user:pass@localhost/test", config)
	///     .await
	///     .unwrap();
	///
	/// let mut conn = pool.acquire().await.unwrap();
	/// let id = conn.connection_id();
	/// assert!(!id.is_empty());
	/// # }
	/// ```
	pub fn connection_id(&self) -> &str {
		&self.connection_id
	}
	/// Invalidate this connection (hard invalidation - connection is unusable)
	///
	pub async fn invalidate(self, reason: String) {
		self.pool_ref
			.emit_event(PoolEvent::connection_invalidated(
				self.connection_id.clone(),
				reason,
			))
			.await;
		// Connection will be dropped and not returned to pool
	}
	/// Soft invalidate this connection (can complete current operation)
	///
	pub async fn soft_invalidate(&mut self) {
		self.pool_ref
			.emit_event(PoolEvent::connection_soft_invalidated(
				self.connection_id.clone(),
			))
			.await;
	}
	/// Reset this connection
	///
	pub async fn reset(&mut self) {
		self.pool_ref
			.emit_event(PoolEvent::connection_reset(self.connection_id.clone()))
			.await;
	}
}

impl<DB: sqlx::Database> Drop for PooledConnection<DB> {
	fn drop(&mut self) {
		let pool_ref = self.pool_ref.clone();
		let connection_id = self.connection_id.clone();

		// Emit checkin event asynchronously
		tokio::spawn(async move {
			pool_ref
				.emit_event(PoolEvent::connection_returned(connection_id))
				.await;
		});
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	#[rstest]
	#[case(
		"postgresql://user:secret@localhost:5432/mydb",
		"postgresql://user:***@localhost:5432/mydb"
	)]
	#[case(
		"mysql://admin:p@ssw0rd@db.example.com/app",
		"mysql://admin:***@db.example.com/app"
	)]
	#[case(
		"postgres://user:pass@host:5432/db?sslmode=require",
		"postgres://user:***@host:5432/db?sslmode=require"
	)]
	fn test_mask_url_password_with_credentials(#[case] input: &str, #[case] expected: &str) {
		// Arrange
		// (input provided by case parameters)

		// Act
		let masked = mask_url_password(input);

		// Assert
		assert_eq!(masked, expected);
	}

	#[rstest]
	#[case("sqlite::memory:")]
	#[case("sqlite:///path/to/db.sqlite")]
	#[case("postgresql://user@localhost:5432/mydb")]
	fn test_mask_url_password_without_password(#[case] input: &str) {
		// Arrange
		// (input provided by case parameter)

		// Act
		let masked = mask_url_password(input);

		// Assert
		assert_eq!(masked, input, "URL without password should be unchanged");
	}

	#[rstest]
	fn test_mask_url_password_empty_password() {
		// Arrange
		let url = "postgresql://user:@localhost:5432/mydb";

		// Act
		let masked = mask_url_password(url);

		// Assert
		assert_eq!(masked, "postgresql://user:***@localhost:5432/mydb");
	}

	#[rstest]
	fn test_mask_url_password_special_chars_in_password() {
		// Arrange
		let url = "postgresql://user:p%40ss%3Aw0rd@localhost:5432/mydb";

		// Act
		let masked = mask_url_password(url);

		// Assert
		assert_eq!(masked, "postgresql://user:***@localhost:5432/mydb");
		assert!(
			!masked.contains("p%40ss"),
			"Password should be fully masked"
		);
	}

	#[rstest]
	fn test_mask_url_password_preserves_non_url() {
		// Arrange
		let non_url = "not-a-url-just-a-string";

		// Act
		let masked = mask_url_password(non_url);

		// Assert
		assert_eq!(
			masked, non_url,
			"Non-URL strings should pass through unchanged"
		);
	}
}
