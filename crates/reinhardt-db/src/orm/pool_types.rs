/// Advanced connection pool types
/// Based on SQLAlchemy's pool implementations
use std::collections::{HashSet, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::{Mutex as TokioMutex, Semaphore};

/// Trait for database connection handles
///
/// This trait abstracts over different database connection types,
/// allowing the pool to work with any backend (PostgreSQL, MySQL, SQLite).
pub trait ConnectionHandle: Send + Sync + std::fmt::Debug {
	/// Check if the connection is still valid
	fn is_valid(&self) -> bool;

	/// Get the database type identifier
	fn database_type(&self) -> &str;

	/// Get a unique identifier for this connection
	fn connection_id(&self) -> &str;
}

pub trait ConnectionPool: Send + Sync {
	fn get_connection(&self) -> Result<PooledConnection, PoolError>;
	fn return_connection(&self, conn: PooledConnection);
	fn size(&self) -> usize;
	fn active_connections(&self) -> usize;
}

/// A pooled database connection with metadata and optional handle
///
/// This struct contains connection metadata for pool management,
/// and optionally holds an actual database connection handle.
#[derive(Clone)]
pub struct PooledConnection {
	/// Unique identifier for this connection within the pool
	pub id: usize,
	/// When this connection was first created
	pub created_at: Instant,
	/// When this connection was last used
	pub last_used: Instant,
	/// Actual database connection handle (optional)
	handle: Option<Arc<dyn ConnectionHandle>>,
}

impl PooledConnection {
	/// Create a new pooled connection with metadata only (no actual handle)
	pub fn new(id: usize) -> Self {
		let now = Instant::now();
		Self {
			id,
			created_at: now,
			last_used: now,
			handle: None,
		}
	}

	/// Create a new pooled connection with an actual database handle
	pub fn with_handle(id: usize, handle: Arc<dyn ConnectionHandle>) -> Self {
		let now = Instant::now();
		Self {
			id,
			created_at: now,
			last_used: now,
			handle: Some(handle),
		}
	}

	/// Get a reference to the connection handle, if present
	pub fn handle(&self) -> Option<&Arc<dyn ConnectionHandle>> {
		self.handle.as_ref()
	}

	/// Check if this connection has an actual database handle
	pub fn has_handle(&self) -> bool {
		self.handle.is_some()
	}

	/// Check if the connection handle is still valid
	pub fn is_valid(&self) -> bool {
		self.handle.as_ref().is_none_or(|h| h.is_valid())
	}

	/// Get the database type, if a handle is present
	pub fn database_type(&self) -> Option<&str> {
		self.handle.as_ref().map(|h| h.database_type())
	}

	/// Update the last_used timestamp
	pub fn touch(&mut self) {
		self.last_used = Instant::now();
	}

	/// Get the age of this connection (time since creation)
	pub fn age(&self) -> Duration {
		self.created_at.elapsed()
	}

	/// Get the idle time (time since last use)
	pub fn idle_time(&self) -> Duration {
		self.last_used.elapsed()
	}
}

#[non_exhaustive]
#[derive(Debug)]
pub enum PoolError {
	NoConnectionsAvailable,
	ConnectionFailed(String),
	Timeout,
	MaxConnectionsReached,
}

/// Internal pool state shared across threads
struct PoolState {
	available: VecDeque<PooledConnection>,
	active: usize,
	next_id: usize,
}

/// Queue-based connection pool (FIFO)
pub struct QueuePool {
	pub max_connections: usize,
	pub timeout: Duration,
	state: Arc<Mutex<PoolState>>,
}

impl QueuePool {
	/// Create a new queue-based connection pool with FIFO behavior
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::pool_types::QueuePool;
	/// use std::time::Duration;
	///
	/// let pool = QueuePool::new(10, Duration::from_secs(30));
	/// assert_eq!(pool.max_connections, 10);
	/// assert_eq!(pool.timeout, Duration::from_secs(30));
	/// ```
	pub fn new(max_connections: usize, timeout: Duration) -> Self {
		Self {
			max_connections,
			timeout,
			state: Arc::new(Mutex::new(PoolState {
				available: VecDeque::new(),
				active: 0,
				next_id: 1,
			})),
		}
	}

	fn create_connection(&self, id: usize) -> PooledConnection {
		PooledConnection::new(id)
	}
}

impl ConnectionPool for QueuePool {
	fn get_connection(&self) -> Result<PooledConnection, PoolError> {
		let mut state = self.state.lock().unwrap();

		// Try to get an existing connection from the pool
		if let Some(mut conn) = state.available.pop_front() {
			conn.last_used = Instant::now();
			state.active += 1;
			return Ok(conn);
		}

		// Create a new connection if under max limit
		if state.active < self.max_connections {
			let id = state.next_id;
			state.next_id += 1;
			state.active += 1;
			return Ok(self.create_connection(id));
		}

		// Pool is exhausted
		Err(PoolError::MaxConnectionsReached)
	}

	fn return_connection(&self, conn: PooledConnection) {
		let mut state = self.state.lock().unwrap();
		state.available.push_back(conn);
		if state.active > 0 {
			state.active -= 1;
		}
	}

	fn size(&self) -> usize {
		let state = self.state.lock().unwrap();
		state.available.len() + state.active
	}

	fn active_connections(&self) -> usize {
		let state = self.state.lock().unwrap();
		state.active
	}
}

/// No pooling - create connection on demand
pub struct NullPool {
	next_id: Arc<Mutex<usize>>,
}

impl NullPool {
	/// Create a new NullPool that creates connections on demand without pooling
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::pool_types::{NullPool, ConnectionPool};
	///
	/// let pool = NullPool::new();
	/// assert_eq!(pool.size(), 0); // No connections stored in pool
	/// assert_eq!(pool.active_connections(), 0);
	/// ```
	pub fn new() -> Self {
		Self {
			next_id: Arc::new(Mutex::new(1)),
		}
	}
}

impl Default for NullPool {
	fn default() -> Self {
		Self::new()
	}
}

impl ConnectionPool for NullPool {
	fn get_connection(&self) -> Result<PooledConnection, PoolError> {
		let mut id = self.next_id.lock().unwrap();
		let conn_id = *id;
		*id += 1;

		Ok(PooledConnection::new(conn_id))
	}

	fn return_connection(&self, _conn: PooledConnection) {
		// No-op for NullPool - connections are discarded
	}

	fn size(&self) -> usize {
		0
	}

	fn active_connections(&self) -> usize {
		0
	}
}

/// Single connection shared across all requests
pub struct StaticPool {
	connection: Arc<Mutex<Option<PooledConnection>>>,
}

impl StaticPool {
	/// Create a new StaticPool that maintains a single shared connection
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::pool_types::{StaticPool, ConnectionPool};
	///
	/// let pool = StaticPool::new();
	/// assert_eq!(pool.size(), 1); // Always one connection
	/// ```
	pub fn new() -> Self {
		Self {
			connection: Arc::new(Mutex::new(None)),
		}
	}
}

impl Default for StaticPool {
	fn default() -> Self {
		Self::new()
	}
}

impl ConnectionPool for StaticPool {
	fn get_connection(&self) -> Result<PooledConnection, PoolError> {
		let mut conn_opt = self.connection.lock().unwrap();

		// If connection doesn't exist, create it
		if conn_opt.is_none() {
			*conn_opt = Some(PooledConnection::new(1));
		}

		// Clone the connection (it's always the same logical connection)
		Ok(conn_opt.as_ref().unwrap().clone())
	}

	fn return_connection(&self, _conn: PooledConnection) {
		// No-op for StaticPool - connection is kept alive
	}

	fn size(&self) -> usize {
		1
	}

	fn active_connections(&self) -> usize {
		let conn_opt = self.connection.lock().unwrap();
		if conn_opt.is_some() { 1 } else { 0 }
	}
}

/// One connection per thread
pub struct SingletonThreadPool {
	connections: Arc<Mutex<std::collections::HashMap<std::thread::ThreadId, PooledConnection>>>,
	next_id: Arc<Mutex<usize>>,
}

impl SingletonThreadPool {
	/// Create a new SingletonThreadPool that maintains one connection per thread
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::pool_types::{SingletonThreadPool, ConnectionPool};
	///
	/// let pool = SingletonThreadPool::new();
	/// assert_eq!(pool.size(), 0); // No connections until first request
	/// ```
	pub fn new() -> Self {
		Self {
			connections: Arc::new(Mutex::new(std::collections::HashMap::new())),
			next_id: Arc::new(Mutex::new(1)),
		}
	}
}

impl Default for SingletonThreadPool {
	fn default() -> Self {
		Self::new()
	}
}

impl ConnectionPool for SingletonThreadPool {
	fn get_connection(&self) -> Result<PooledConnection, PoolError> {
		let thread_id = std::thread::current().id();
		let mut connections = self.connections.lock().unwrap();

		// If connection exists for this thread, return it (cloned)
		if let Some(conn) = connections.get(&thread_id) {
			return Ok(conn.clone());
		}

		// Create new connection for this thread
		let mut next_id = self.next_id.lock().unwrap();
		let id = *next_id;
		*next_id += 1;

		let conn = PooledConnection::new(id);

		connections.insert(thread_id, conn.clone());
		Ok(conn)
	}

	fn return_connection(&self, _conn: PooledConnection) {
		// Kept in thread-specific storage
	}

	fn size(&self) -> usize {
		let connections = self.connections.lock().unwrap();
		connections.len()
	}

	fn active_connections(&self) -> usize {
		let connections = self.connections.lock().unwrap();
		connections.len()
	}
}

/// Async-compatible queue pool
pub struct AsyncAdaptedQueuePool {
	pub max_connections: usize,
	semaphore: Arc<Semaphore>,
	queue: Arc<TokioMutex<VecDeque<PooledConnection>>>,
	next_id: Arc<TokioMutex<usize>>,
}

impl AsyncAdaptedQueuePool {
	/// Create a new async-compatible queue pool for use with async runtimes
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::pool_types::AsyncAdaptedQueuePool;
	///
	/// let pool = AsyncAdaptedQueuePool::new(20);
	/// assert_eq!(pool.max_connections, 20);
	/// ```
	pub fn new(max_connections: usize) -> Self {
		Self {
			max_connections,
			semaphore: Arc::new(Semaphore::new(max_connections)),
			queue: Arc::new(TokioMutex::new(VecDeque::new())),
			next_id: Arc::new(TokioMutex::new(1)),
		}
	}

	/// Asynchronously get a connection from the pool
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::pool_types::AsyncAdaptedQueuePool;
	///
	/// # tokio_test::block_on(async {
	/// let pool = AsyncAdaptedQueuePool::new(5);
	/// let conn = pool.get_connection().await.unwrap();
	/// assert!(conn.id > 0);
	/// pool.return_connection(conn).await;
	/// # });
	/// ```
	pub async fn get_connection(&self) -> Result<PooledConnection, PoolError> {
		let _permit = self.semaphore.acquire().await.map_err(|e| {
			PoolError::ConnectionFailed(format!("Failed to acquire semaphore: {}", e))
		})?;

		let mut queue = self.queue.lock().await;

		// Try to reuse existing connection
		if let Some(mut conn) = queue.pop_front() {
			conn.last_used = Instant::now();
			std::mem::forget(_permit); // Keep permit alive
			return Ok(conn);
		}

		// Create new connection
		let mut next_id = self.next_id.lock().await;
		let id = *next_id;
		*next_id += 1;

		std::mem::forget(_permit); // Keep permit alive

		Ok(PooledConnection::new(id))
	}

	/// Asynchronously return a connection to the pool
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::pool_types::AsyncAdaptedQueuePool;
	///
	/// # tokio_test::block_on(async {
	/// let pool = AsyncAdaptedQueuePool::new(5);
	/// let conn = pool.get_connection().await.unwrap();
	/// pool.return_connection(conn).await;
	/// # });
	/// ```
	pub async fn return_connection(&self, conn: PooledConnection) {
		let mut queue = self.queue.lock().await;
		queue.push_back(conn);
		self.semaphore.add_permits(1);
	}
}

/// Assertion pool for testing (detects connection leaks)
pub struct AssertionPool {
	active_connections: Arc<Mutex<HashSet<usize>>>,
	connection_counter: Arc<Mutex<usize>>,
}

impl AssertionPool {
	/// Create a new AssertionPool for testing connection management
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::pool_types::AssertionPool;
	///
	/// let pool = AssertionPool::new();
	/// let conn_id = pool.get_connection();
	/// assert_eq!(conn_id, 0);
	/// pool.return_connection(conn_id);
	/// ```
	pub fn new() -> Self {
		Self {
			active_connections: Arc::new(Mutex::new(HashSet::new())),
			connection_counter: Arc::new(Mutex::new(0)),
		}
	}

	/// Get a connection ID and track it as active
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::pool_types::AssertionPool;
	///
	/// let pool = AssertionPool::new();
	/// let conn_id = pool.get_connection();
	/// assert_eq!(conn_id, 0);
	///
	/// let conn_id2 = pool.get_connection();
	/// assert_eq!(conn_id2, 1);
	///
	/// pool.return_connection(conn_id);
	/// pool.return_connection(conn_id2);
	/// ```
	pub fn get_connection(&self) -> usize {
		let mut counter = self.connection_counter.lock().unwrap();
		let id = *counter;
		*counter += 1;

		let mut active = self.active_connections.lock().unwrap();
		active.insert(id);

		eprintln!("[AssertionPool] Connection {} acquired", id);
		id
	}

	/// Return a connection ID and remove it from active tracking
	///
	/// # Panics
	///
	/// Panics if the connection ID was not active
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::pool_types::AssertionPool;
	///
	/// let pool = AssertionPool::new();
	/// let conn_id = pool.get_connection();
	/// assert_eq!(pool.active_count(), 1);
	/// pool.return_connection(conn_id); // OK
	/// // Verify the connection was returned successfully
	/// assert_eq!(pool.active_count(), 0);
	/// ```
	///
	/// ```should_panic
	/// use reinhardt_db::orm::pool_types::AssertionPool;
	///
	/// let pool = AssertionPool::new();
	/// pool.return_connection(999); // Panics - connection not active
	/// ```
	pub fn return_connection(&self, id: usize) {
		let mut active = self.active_connections.lock().unwrap();
		if !active.remove(&id) {
			panic!("Attempted to return connection {} that was not active", id);
		}
		eprintln!("[AssertionPool] Connection {} returned", id);
	}

	/// Get the number of currently active connections
	pub fn active_count(&self) -> usize {
		let active = self.active_connections.lock().unwrap();
		active.len()
	}
}

impl Default for AssertionPool {
	fn default() -> Self {
		Self::new()
	}
}

impl Drop for AssertionPool {
	fn drop(&mut self) {
		let active = self.active_connections.lock().unwrap();
		if !active.is_empty() {
			panic!(
				"AssertionPool dropped with {} active connections: {:?}",
				active.len(),
				active
			);
		}
	}
}

pub struct PoolStatistics {
	pub total_connections: usize,
	pub active_connections: usize,
	pub idle_connections: usize,
	pub total_requests: usize,
	pub failed_requests: usize,
}

impl Default for PoolStatistics {
	fn default() -> Self {
		Self::new()
	}
}

impl PoolStatistics {
	/// Create a new PoolStatistics instance for tracking connection pool metrics
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::pool_types::PoolStatistics;
	///
	/// let stats = PoolStatistics::new();
	/// assert_eq!(stats.total_connections, 0);
	/// assert_eq!(stats.active_connections, 0);
	/// assert_eq!(stats.idle_connections, 0);
	/// assert_eq!(stats.total_requests, 0);
	/// assert_eq!(stats.failed_requests, 0);
	/// ```
	pub fn new() -> Self {
		Self {
			total_connections: 0,
			active_connections: 0,
			idle_connections: 0,
			total_requests: 0,
			failed_requests: 0,
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_queue_pool_creation() {
		let pool = QueuePool::new(10, Duration::from_secs(30));
		assert_eq!(pool.max_connections, 10);
	}

	#[test]
	fn test_queue_pool_timeout() {
		let pool = QueuePool::new(5, Duration::from_secs(10));
		assert_eq!(pool.timeout, Duration::from_secs(10));
	}

	#[test]
	fn test_null_pool_size() {
		let pool = NullPool::new();
		assert_eq!(pool.size(), 0);
	}

	#[test]
	fn test_static_pool_size() {
		let pool = StaticPool::new();
		assert_eq!(pool.size(), 1);
	}

	#[test]
	fn test_async_adapted_queue_pool_creation() {
		let pool = AsyncAdaptedQueuePool::new(20);
		assert_eq!(pool.max_connections, 20);
	}

	#[test]
	fn test_queue_pool_different_sizes() {
		let small_pool = QueuePool::new(1, Duration::from_secs(30));
		let large_pool = QueuePool::new(100, Duration::from_secs(30));

		assert_eq!(small_pool.max_connections, 1);
		assert_eq!(large_pool.max_connections, 100);
	}

	#[test]
	fn test_queue_pool_different_timeouts() {
		let short_timeout = QueuePool::new(10, Duration::from_secs(5));
		let long_timeout = QueuePool::new(10, Duration::from_millis(500));

		assert_eq!(short_timeout.timeout, Duration::from_secs(5));
		assert_eq!(long_timeout.timeout, Duration::from_millis(500));
	}

	#[test]
	fn test_multiple_pool_types() {
		let queue_pool = QueuePool::new(10, Duration::from_secs(30));
		let null_pool = NullPool::new();
		let static_pool = StaticPool::new();

		assert_eq!(queue_pool.max_connections, 10);
		assert_eq!(null_pool.size(), 0);
		assert_eq!(static_pool.size(), 1);
	}

	#[test]
	fn test_async_pool_different_sizes() {
		let pool1 = AsyncAdaptedQueuePool::new(5);
		let pool2 = AsyncAdaptedQueuePool::new(50);

		assert_eq!(pool1.max_connections, 5);
		assert_eq!(pool2.max_connections, 50);
	}

	#[test]
	fn test_pool_error_types() {
		let timeout_error = PoolError::Timeout;
		let max_error = PoolError::MaxConnectionsReached;

		matches!(timeout_error, PoolError::Timeout);
		matches!(max_error, PoolError::MaxConnectionsReached);
	}

	#[test]
	fn test_pooled_connection_creation() {
		let conn = PooledConnection::new(1);

		assert_eq!(conn.id, 1);
		assert!(conn.created_at.elapsed().as_secs() < 1);
		assert!(!conn.has_handle());
	}

	#[test]
	fn test_pooled_connection_different_ids() {
		let conn1 = PooledConnection::new(1);
		let conn2 = PooledConnection::new(2);

		assert_ne!(conn1.id, conn2.id);
	}

	#[test]
	fn test_pooled_connection_age() {
		let conn = PooledConnection::new(1);

		std::thread::sleep(std::time::Duration::from_millis(10));
		assert!(conn.age().as_millis() >= 10);
	}

	#[test]
	fn test_pooled_connection_idle_time() {
		let mut conn = PooledConnection::new(1);

		std::thread::sleep(std::time::Duration::from_millis(10));
		assert!(conn.idle_time().as_millis() >= 10);

		conn.touch();
		assert!(conn.idle_time().as_millis() < 5);
	}

	#[test]
	fn test_pooled_connection_is_valid_without_handle() {
		let conn = PooledConnection::new(1);
		assert!(conn.is_valid()); // No handle means always valid
		assert!(conn.database_type().is_none());
	}

	#[test]
	fn test_queue_pool_zero_timeout() {
		let pool = QueuePool::new(10, Duration::from_secs(0));
		assert_eq!(pool.timeout, Duration::from_secs(0));
	}

	#[test]
	fn test_large_queue_pool() {
		let pool = QueuePool::new(1000, Duration::from_secs(60));
		assert_eq!(pool.max_connections, 1000);
	}
}
