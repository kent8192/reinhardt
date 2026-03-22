//! Distributed task locking mechanism
//!
//! This module provides locking primitives for distributed task systems,
//! preventing multiple workers from executing the same task simultaneously.

use crate::{TaskId, TaskResult};
use async_trait::async_trait;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use uuid::Uuid;

/// Opaque token returned by a successful lock acquisition.
///
/// The token proves lock ownership and must be presented when releasing
/// or extending the lock. This prevents workers from accidentally
/// releasing locks they do not own.
///
/// # Examples
///
/// ```rust
/// use reinhardt_tasks::LockToken;
///
/// let token = LockToken::generate();
/// assert!(!token.as_str().is_empty());
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LockToken(String);

impl LockToken {
	/// Generate a new unique lock token
	pub fn generate() -> Self {
		Self(Uuid::new_v4().to_string())
	}

	/// Get the string representation of the token
	pub fn as_str(&self) -> &str {
		&self.0
	}
}

/// Distributed lock trait for task synchronization
///
/// # Examples
///
/// ```rust,no_run
/// use reinhardt_tasks::{TaskLock, TaskId, LockToken};
/// use async_trait::async_trait;
/// use std::time::Duration;
///
/// struct MyLock;
///
/// #[async_trait]
/// impl TaskLock for MyLock {
///     async fn acquire(&self, task_id: TaskId, ttl: Duration) -> reinhardt_tasks::TaskResult<Option<LockToken>> {
///         // Acquire lock implementation
///         Ok(Some(LockToken::generate()))
///     }
///
///     async fn release(&self, task_id: TaskId, token: &LockToken) -> reinhardt_tasks::TaskResult<bool> {
///         // Release lock implementation
///         Ok(true)
///     }
///
///     async fn is_locked(&self, task_id: TaskId) -> reinhardt_tasks::TaskResult<bool> {
///         // Check lock status
///         Ok(false)
///     }
/// }
/// ```
#[async_trait]
pub trait TaskLock: Send + Sync {
	/// Acquire a lock for a task
	///
	/// Returns `Some(LockToken)` if lock was acquired, `None` if already locked
	/// by another worker.
	async fn acquire(&self, task_id: TaskId, ttl: Duration) -> TaskResult<Option<LockToken>>;

	/// Release a lock for a task
	///
	/// Returns `true` if the lock was released, `false` if the token does not
	/// match (i.e. the caller does not own the lock).
	async fn release(&self, task_id: TaskId, token: &LockToken) -> TaskResult<bool>;

	/// Check if a task is locked
	async fn is_locked(&self, task_id: TaskId) -> TaskResult<bool>;

	/// Extend the TTL of an existing lock
	///
	/// Implementors should override this with a backend-specific atomic operation
	/// to avoid race conditions where another worker could steal the lock between
	/// release and re-acquire.
	async fn extend(&self, task_id: TaskId, token: &LockToken, ttl: Duration) -> TaskResult<bool> {
		// Default: check-then-release-then-acquire is non-atomic.
		// Concrete implementations should override with atomic operations.
		if self.is_locked(task_id).await? {
			self.release(task_id, token).await?;
			self.acquire(task_id, ttl).await.map(|t| t.is_some())
		} else {
			Ok(false)
		}
	}
}

/// In-memory task lock for single-process testing
///
/// # Examples
///
/// ```rust
/// use reinhardt_tasks::{MemoryTaskLock, TaskLock, TaskId};
/// use std::time::Duration;
///
/// # async fn example() -> reinhardt_tasks::TaskResult<()> {
/// let lock = MemoryTaskLock::new();
/// let task_id = TaskId::new();
///
/// // Acquire lock
/// let token = lock.acquire(task_id, Duration::from_secs(60)).await?;
/// assert!(token.is_some());
///
/// // Check if locked
/// let is_locked = lock.is_locked(task_id).await?;
/// assert!(is_locked);
///
/// // Release lock
/// let released = lock.release(task_id, &token.unwrap()).await?;
/// assert!(released);
/// # Ok(())
/// # }
/// ```
pub struct MemoryTaskLock {
	/// Map of task ID to (expiry timestamp in ms, token string)
	locks: Arc<RwLock<std::collections::HashMap<TaskId, (i128, String)>>>,
}

impl MemoryTaskLock {
	/// Create a new in-memory task lock
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_tasks::MemoryTaskLock;
	///
	/// let lock = MemoryTaskLock::new();
	/// ```
	pub fn new() -> Self {
		Self {
			locks: Arc::new(RwLock::new(std::collections::HashMap::new())),
		}
	}

	/// Clean up expired locks
	async fn cleanup_expired(&self) {
		let mut locks = self.locks.write().await;
		let now = chrono::Utc::now().timestamp_millis() as i128;
		locks.retain(|_, (expiry, _)| *expiry > now);
	}
}

impl Default for MemoryTaskLock {
	fn default() -> Self {
		Self::new()
	}
}

#[async_trait]
impl TaskLock for MemoryTaskLock {
	async fn acquire(&self, task_id: TaskId, ttl: Duration) -> TaskResult<Option<LockToken>> {
		self.cleanup_expired().await;

		let mut locks = self.locks.write().await;
		let now = chrono::Utc::now().timestamp_millis() as i128;
		let expiry = now + ttl.as_millis() as i128;

		if let Some(&(existing_expiry, _)) = locks.get(&task_id)
			&& existing_expiry > now
		{
			return Ok(None);
		}

		let token = LockToken::generate();
		locks.insert(task_id, (expiry, token.as_str().to_string()));
		Ok(Some(token))
	}

	async fn release(&self, task_id: TaskId, token: &LockToken) -> TaskResult<bool> {
		let mut locks = self.locks.write().await;
		if let Some((_, stored_token)) = locks.get(&task_id)
			&& stored_token == token.as_str()
		{
			locks.remove(&task_id);
			return Ok(true);
		}
		Ok(false)
	}

	async fn is_locked(&self, task_id: TaskId) -> TaskResult<bool> {
		self.cleanup_expired().await;

		let locks = self.locks.read().await;
		let now = chrono::Utc::now().timestamp_millis() as i128;

		Ok(locks
			.get(&task_id)
			.map(|(expiry, _)| *expiry > now)
			.unwrap_or(false))
	}

	/// Atomically extend the TTL of an existing lock.
	///
	/// Unlike the default trait implementation which releases then re-acquires,
	/// this holds the write lock throughout the operation to prevent another
	/// worker from stealing the lock in between.
	async fn extend(&self, task_id: TaskId, token: &LockToken, ttl: Duration) -> TaskResult<bool> {
		let mut locks = self.locks.write().await;
		let now = chrono::Utc::now().timestamp_millis() as i128;

		if let Some((expiry, stored_token)) = locks.get_mut(&task_id)
			&& *expiry > now
			&& stored_token.as_str() == token.as_str()
		{
			// Lock is still valid and owned by caller; atomically update its expiry
			*expiry = now + ttl.as_millis() as i128;
			return Ok(true);
		}

		Ok(false)
	}
}

#[cfg(feature = "redis-backend")]
/// Redis-based distributed task lock
///
/// Uses atomic `SET key value PX ms NX` for lock acquisition and Lua scripts
/// for ownership-verified release and extension.
///
/// # Examples
///
/// ```no_run
/// use reinhardt_tasks::{RedisTaskLock, TaskLock, TaskId};
/// use std::time::Duration;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let lock = RedisTaskLock::new("redis://127.0.0.1/").await?;
/// let task_id = TaskId::new();
///
/// // Acquire distributed lock
/// let token = lock.acquire(task_id, Duration::from_secs(30)).await?;
/// if let Some(token) = token {
///     // Execute task
///     // ...
///     lock.release(task_id, &token).await?;
/// }
/// # Ok(())
/// # }
/// ```
pub struct RedisTaskLock {
	connection: Arc<redis::aio::ConnectionManager>,
	key_prefix: String,
}

#[cfg(feature = "redis-backend")]
impl RedisTaskLock {
	/// Create a new Redis-based task lock
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_tasks::RedisTaskLock;
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let lock = RedisTaskLock::new("redis://localhost/").await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn new(redis_url: &str) -> Result<Self, redis::RedisError> {
		let client = redis::Client::open(redis_url)?;
		let connection = redis::aio::ConnectionManager::new(client).await?;

		Ok(Self {
			connection: Arc::new(connection),
			key_prefix: "reinhardt:locks:".to_string(),
		})
	}

	/// Create a Redis task lock with custom key prefix
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_tasks::RedisTaskLock;
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let lock = RedisTaskLock::with_prefix(
	///     "redis://localhost/",
	///     "myapp:locks:".to_string()
	/// ).await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn with_prefix(
		redis_url: &str,
		key_prefix: String,
	) -> Result<Self, redis::RedisError> {
		let client = redis::Client::open(redis_url)?;
		let connection = redis::aio::ConnectionManager::new(client).await?;

		Ok(Self {
			connection: Arc::new(connection),
			key_prefix,
		})
	}

	fn lock_key(&self, task_id: TaskId) -> String {
		format!("{}task:{}", self.key_prefix, task_id)
	}
}

#[cfg(feature = "redis-backend")]
#[async_trait]
impl TaskLock for RedisTaskLock {
	async fn acquire(&self, task_id: TaskId, ttl: Duration) -> TaskResult<Option<LockToken>> {
		use crate::TaskError;

		let mut conn = (*self.connection).clone();
		let key = self.lock_key(task_id);
		let token = LockToken::generate();
		let ttl_ms = ttl.as_millis() as i64;

		// Atomic SET key value PX ms NX
		let result: Result<Option<String>, redis::RedisError> = redis::cmd("SET")
			.arg(&key)
			.arg(token.as_str())
			.arg("PX")
			.arg(ttl_ms)
			.arg("NX")
			.query_async(&mut conn)
			.await;

		match result {
			Ok(Some(_)) => Ok(Some(token)),
			Ok(None) => Ok(None),
			Err(e) => Err(TaskError::ExecutionFailed(format!(
				"Failed to acquire lock: {}",
				e
			))),
		}
	}

	async fn release(&self, task_id: TaskId, token: &LockToken) -> TaskResult<bool> {
		use crate::TaskError;

		let mut conn = (*self.connection).clone();
		let key = self.lock_key(task_id);

		// Lua script: compare token, delete only if matching
		let script = redis::Script::new(
			"if redis.call('get', KEYS[1]) == ARGV[1] then return redis.call('del', KEYS[1]) else return 0 end",
		);

		let result: Result<i32, redis::RedisError> = script
			.key(&key)
			.arg(token.as_str())
			.invoke_async(&mut conn)
			.await;

		match result {
			Ok(1) => Ok(true),
			Ok(_) => Ok(false),
			Err(e) => Err(TaskError::ExecutionFailed(format!(
				"Failed to release lock: {}",
				e
			))),
		}
	}

	async fn is_locked(&self, task_id: TaskId) -> TaskResult<bool> {
		use crate::TaskError;
		use redis::AsyncCommands;

		let mut conn = (*self.connection).clone();
		let key = self.lock_key(task_id);

		let result: Result<bool, redis::RedisError> = conn.exists(&key).await;

		result.map_err(|e| TaskError::ExecutionFailed(format!("Failed to check lock: {}", e)))
	}

	/// Atomically extend the TTL using a Lua script with millisecond precision.
	///
	/// Verifies ownership before extending, preventing unauthorized extensions.
	async fn extend(&self, task_id: TaskId, token: &LockToken, ttl: Duration) -> TaskResult<bool> {
		use crate::TaskError;

		let mut conn = (*self.connection).clone();
		let key = self.lock_key(task_id);
		let ttl_ms = ttl.as_millis() as i64;

		// Lua script: compare token, pexpire only if matching
		let script = redis::Script::new(
			"if redis.call('get', KEYS[1]) == ARGV[1] then return redis.call('pexpire', KEYS[1], ARGV[2]) else return 0 end",
		);

		let result: Result<i32, redis::RedisError> = script
			.key(&key)
			.arg(token.as_str())
			.arg(ttl_ms)
			.invoke_async(&mut conn)
			.await;

		match result {
			Ok(1) => Ok(true),
			Ok(_) => Ok(false),
			Err(e) => Err(TaskError::ExecutionFailed(format!(
				"Failed to extend lock: {}",
				e
			))),
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;
	use std::time::Duration;

	#[rstest]
	#[tokio::test]
	async fn test_memory_lock_acquire() {
		// Arrange
		let lock = MemoryTaskLock::new();
		let task_id = TaskId::new();

		// Act
		let token = lock
			.acquire(task_id, Duration::from_secs(60))
			.await
			.unwrap();

		// Assert
		assert!(token.is_some());
	}

	#[rstest]
	#[tokio::test]
	async fn test_memory_lock_already_locked() {
		// Arrange
		let lock = MemoryTaskLock::new();
		let task_id = TaskId::new();
		lock.acquire(task_id, Duration::from_secs(60))
			.await
			.unwrap();

		// Act
		let token = lock
			.acquire(task_id, Duration::from_secs(60))
			.await
			.unwrap();

		// Assert
		assert!(token.is_none());
	}

	#[rstest]
	#[tokio::test]
	async fn test_memory_lock_release() {
		// Arrange
		let lock = MemoryTaskLock::new();
		let task_id = TaskId::new();
		let token = lock
			.acquire(task_id, Duration::from_secs(60))
			.await
			.unwrap()
			.unwrap();

		// Act
		let released = lock.release(task_id, &token).await.unwrap();

		// Assert
		assert!(released);
		let is_locked = lock.is_locked(task_id).await.unwrap();
		assert!(!is_locked);
	}

	#[rstest]
	#[tokio::test]
	async fn test_memory_lock_release_wrong_token() {
		// Arrange
		let lock = MemoryTaskLock::new();
		let task_id = TaskId::new();
		lock.acquire(task_id, Duration::from_secs(60))
			.await
			.unwrap();
		let wrong_token = LockToken::generate();

		// Act
		let released = lock.release(task_id, &wrong_token).await.unwrap();

		// Assert - release must fail with wrong token
		assert!(!released);
		let is_locked = lock.is_locked(task_id).await.unwrap();
		assert!(is_locked);
	}

	#[rstest]
	#[tokio::test]
	async fn test_memory_lock_expiry() {
		// Arrange
		let lock = MemoryTaskLock::new();
		let task_id = TaskId::new();
		lock.acquire(task_id, Duration::from_millis(50))
			.await
			.unwrap();

		// Act
		tokio::time::sleep(Duration::from_millis(100)).await;

		// Assert
		let is_locked = lock.is_locked(task_id).await.unwrap();
		assert!(!is_locked);
	}

	#[rstest]
	#[tokio::test]
	async fn test_memory_lock_extend() {
		// Arrange
		let lock = MemoryTaskLock::new();
		let task_id = TaskId::new();
		let token = lock
			.acquire(task_id, Duration::from_secs(60))
			.await
			.unwrap()
			.unwrap();

		// Act
		let extended = lock
			.extend(task_id, &token, Duration::from_secs(120))
			.await
			.unwrap();

		// Assert
		assert!(extended);
		let is_locked = lock.is_locked(task_id).await.unwrap();
		assert!(is_locked);
	}

	#[rstest]
	#[tokio::test]
	async fn test_memory_lock_extend_returns_false_for_unlocked_task() {
		// Arrange
		let lock = MemoryTaskLock::new();
		let task_id = TaskId::new();
		let token = LockToken::generate();

		// Act - extend without acquiring first
		let extended = lock
			.extend(task_id, &token, Duration::from_secs(120))
			.await
			.unwrap();

		// Assert
		assert!(!extended);
	}

	#[rstest]
	#[tokio::test]
	async fn test_memory_lock_extend_returns_false_for_expired_lock() {
		// Arrange
		let lock = MemoryTaskLock::new();
		let task_id = TaskId::new();
		let token = lock
			.acquire(task_id, Duration::from_millis(50))
			.await
			.unwrap()
			.unwrap();
		tokio::time::sleep(Duration::from_millis(100)).await;

		// Act - extend an expired lock
		let extended = lock
			.extend(task_id, &token, Duration::from_secs(120))
			.await
			.unwrap();

		// Assert
		assert!(!extended);
	}

	#[rstest]
	#[tokio::test]
	async fn test_memory_lock_extend_returns_false_for_wrong_token() {
		// Arrange
		let lock = MemoryTaskLock::new();
		let task_id = TaskId::new();
		lock.acquire(task_id, Duration::from_secs(60))
			.await
			.unwrap();
		let wrong_token = LockToken::generate();

		// Act - extend with wrong token
		let extended = lock
			.extend(task_id, &wrong_token, Duration::from_secs(120))
			.await
			.unwrap();

		// Assert
		assert!(!extended);
	}

	#[rstest]
	#[tokio::test]
	async fn test_memory_lock_extend_is_atomic() {
		// Arrange - verify that extend does not release the lock at any point
		let lock = Arc::new(MemoryTaskLock::new());
		let task_id = TaskId::new();
		let token = lock
			.acquire(task_id, Duration::from_millis(200))
			.await
			.unwrap()
			.unwrap();

		// Act - extend the lock
		let extended = lock
			.extend(task_id, &token, Duration::from_secs(60))
			.await
			.unwrap();

		// Assert - lock should still be held and not have been released
		assert!(extended);
		// A second acquire should fail because the lock was never released
		let second_acquire = lock
			.acquire(task_id, Duration::from_secs(60))
			.await
			.unwrap();
		assert!(second_acquire.is_none());
	}
}
