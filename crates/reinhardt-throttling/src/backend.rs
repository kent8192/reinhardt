use super::ThrottleError;
use super::time_provider::{SystemTimeProvider, TimeProvider};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::RwLock;
use tokio::time::{Duration, Instant};

#[async_trait]
pub trait ThrottleBackend: Send + Sync {
	async fn increment(&self, key: &str, window: u64) -> Result<usize, String>;
	async fn get_count(&self, key: &str) -> Result<usize, String>;

	/// Increment with Duration instead of u64
	async fn increment_duration(
		&self,
		key: &str,
		window: Duration,
	) -> Result<usize, ThrottleError> {
		self.increment(key, window.as_secs())
			.await
			.map_err(ThrottleError::ThrottleError)
	}

	/// Get wait time for rate limit
	async fn get_wait_time(&self, _key: &str) -> Result<Option<Duration>, ThrottleError> {
		// Default implementation returns None (not implemented)
		Ok(None)
	}
}

/// Entry stored per rate-limit key in memory backend
#[derive(Clone)]
struct WindowEntry {
	count: usize,
	window_start: Instant,
	window_secs: u64,
}

/// Probabilistic eviction runs roughly once per this many increment operations.
const EVICTION_INTERVAL: u64 = 100;

#[derive(Clone)]
pub struct MemoryBackend<T: TimeProvider = SystemTimeProvider> {
	storage: Arc<RwLock<HashMap<String, WindowEntry>>>,
	time_provider: Arc<T>,
	/// Counter for probabilistic eviction scheduling
	ops_counter: Arc<AtomicU64>,
}

impl MemoryBackend<SystemTimeProvider> {
	/// Creates a new `MemoryBackend` with the default system time provider.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_throttling::MemoryBackend;
	///
	/// let backend = MemoryBackend::new();
	// Backend is ready to track rate limits in memory
	/// ```
	pub fn new() -> Self {
		Self {
			storage: Arc::new(RwLock::new(HashMap::new())),
			time_provider: Arc::new(SystemTimeProvider::new()),
			ops_counter: Arc::new(AtomicU64::new(0)),
		}
	}
}

impl<T: TimeProvider> MemoryBackend<T> {
	/// Create a new MemoryBackend with a custom time provider
	pub fn with_time_provider(time_provider: Arc<T>) -> Self {
		Self {
			storage: Arc::new(RwLock::new(HashMap::new())),
			time_provider,
			ops_counter: Arc::new(AtomicU64::new(0)),
		}
	}

	/// Evict expired entries from the storage map.
	///
	/// Called probabilistically on each increment to bound memory growth
	/// without adding per-request overhead.
	async fn maybe_evict_expired(&self) {
		let count = self.ops_counter.fetch_add(1, Ordering::Relaxed);
		if !count.is_multiple_of(EVICTION_INTERVAL) {
			return;
		}
		let now = self.time_provider.now();
		let mut storage = self.storage.write().await;
		storage.retain(|_, entry| {
			now.duration_since(entry.window_start) <= Duration::from_secs(entry.window_secs)
		});
	}
}

impl Default for MemoryBackend<SystemTimeProvider> {
	fn default() -> Self {
		Self::new()
	}
}

#[async_trait]
impl<T: TimeProvider> ThrottleBackend for MemoryBackend<T> {
	async fn increment(&self, key: &str, window_secs: u64) -> Result<usize, String> {
		// Periodically evict expired entries to prevent unbounded memory growth
		self.maybe_evict_expired().await;

		let mut storage = self.storage.write().await;
		let now = self.time_provider.now();
		let entry = storage.entry(key.to_string()).or_insert(WindowEntry {
			count: 0,
			window_start: now,
			window_secs,
		});
		if now.duration_since(entry.window_start) > Duration::from_secs(window_secs) {
			*entry = WindowEntry {
				count: 1,
				window_start: now,
				window_secs,
			};
			Ok(1)
		} else {
			entry.count += 1;
			// Update the stored window duration in case it changed
			entry.window_secs = window_secs;
			Ok(entry.count)
		}
	}
	async fn get_count(&self, key: &str) -> Result<usize, String> {
		let storage = self.storage.read().await;
		match storage.get(key) {
			Some(entry) => {
				let now = self.time_provider.now();
				// Return 0 if the window has expired
				if now.duration_since(entry.window_start) > Duration::from_secs(entry.window_secs) {
					Ok(0)
				} else {
					Ok(entry.count)
				}
			}
			None => Ok(0),
		}
	}
}

#[cfg(feature = "redis-backend")]
pub struct RedisThrottleBackend {
	client: redis::Client,
}

/// Lua script for atomic INCR + EXPIRE in Redis rate limiting.
/// Prevents race condition where INCR succeeds but EXPIRE fails, leaving permanent keys.
#[cfg(feature = "redis-backend")]
const INCREMENT_SCRIPT: &str = r#"
	local count = redis.call('INCR', KEYS[1])
	if count == 1 then
		redis.call('EXPIRE', KEYS[1], ARGV[1])
	end
	return count
"#;

#[cfg(feature = "redis-backend")]
impl RedisThrottleBackend {
	/// Creates a new `RedisThrottleBackend` connected to the specified Redis URL.
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_throttling::RedisThrottleBackend;
	///
	/// let backend = RedisThrottleBackend::new("redis://127.0.0.1:6379").unwrap();
	// Backend is now connected to Redis for distributed rate limiting
	/// ```
	pub fn new(url: &str) -> Result<Self, String> {
		let client = redis::Client::open(url).map_err(|e| e.to_string())?;
		Ok(Self { client })
	}
}

#[cfg(feature = "redis-backend")]
#[async_trait]
impl ThrottleBackend for RedisThrottleBackend {
	async fn increment(&self, key: &str, window: u64) -> Result<usize, String> {
		use redis::Script;
		let mut conn = self
			.client
			.get_multiplexed_async_connection()
			.await
			.map_err(|e| e.to_string())?;

		// Safely convert u64 to i64 for Redis EXPIRE command, which requires
		// a signed integer. Values exceeding i64::MAX are clamped to prevent overflow.
		let expire_secs = i64::try_from(window).unwrap_or(i64::MAX);

		let script = Script::new(INCREMENT_SCRIPT);
		let count: usize = script
			.key(key)
			.arg(expire_secs)
			.invoke_async(&mut conn)
			.await
			.map_err(|e| e.to_string())?;
		Ok(count)
	}
	async fn get_count(&self, key: &str) -> Result<usize, String> {
		use redis::AsyncCommands;
		let mut conn = self
			.client
			.get_multiplexed_async_connection()
			.await
			.map_err(|e| e.to_string())?;
		conn.get(key).await.map_err(|e| e.to_string())
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::time_provider::MockTimeProvider;
	use rstest::rstest;

	#[rstest]
	#[tokio::test]
	async fn test_memory_backend_increment() {
		// Arrange
		let backend = MemoryBackend::new();
		let key = "test_key";

		// Act & Assert
		let count1 = backend.increment(key, 60).await.unwrap();
		assert_eq!(count1, 1);

		let count2 = backend.increment(key, 60).await.unwrap();
		assert_eq!(count2, 2);

		let count3 = backend.increment(key, 60).await.unwrap();
		assert_eq!(count3, 3);
	}

	#[rstest]
	#[tokio::test]
	async fn test_memory_backend_get_count() {
		// Arrange
		let backend = MemoryBackend::new();
		let key = "test_key";

		// Assert - initial count
		let initial_count = backend.get_count(key).await.unwrap();
		assert_eq!(initial_count, 0);

		// Act
		backend.increment(key, 60).await.unwrap();
		backend.increment(key, 60).await.unwrap();

		// Assert
		let count = backend.get_count(key).await.unwrap();
		assert_eq!(count, 2);
	}

	#[rstest]
	#[tokio::test]
	async fn test_memory_backend_increment_duration() {
		// Arrange
		let backend = MemoryBackend::new();
		let key = "test_key";

		// Act
		let count = backend
			.increment_duration(key, Duration::from_secs(60))
			.await
			.unwrap();

		// Assert
		assert_eq!(count, 1);
	}

	#[rstest]
	#[tokio::test]
	async fn test_memory_backend_separate_keys() {
		// Arrange
		let backend = MemoryBackend::new();

		// Act
		backend.increment("key1", 60).await.unwrap();
		backend.increment("key1", 60).await.unwrap();
		backend.increment("key2", 60).await.unwrap();

		// Assert
		let count1 = backend.get_count("key1").await.unwrap();
		let count2 = backend.get_count("key2").await.unwrap();
		assert_eq!(count1, 2);
		assert_eq!(count2, 1);
	}

	#[rstest]
	#[tokio::test]
	async fn test_memory_backend_default() {
		// Arrange
		let backend = MemoryBackend::default();

		// Act
		let count = backend.increment("test", 60).await.unwrap();

		// Assert
		assert_eq!(count, 1);
	}

	#[rstest]
	#[tokio::test]
	async fn test_get_count_returns_zero_after_window_expiry() {
		// Arrange
		use tokio::time::Instant;
		let time_provider = Arc::new(MockTimeProvider::new(Instant::now()));
		let backend = MemoryBackend::with_time_provider(time_provider.clone());
		let key = "test_key";
		let window_secs = 5;

		// Act - increment within window
		backend.increment(key, window_secs).await.unwrap();
		backend.increment(key, window_secs).await.unwrap();
		backend.increment(key, window_secs).await.unwrap();

		// Assert - count should be 3 within window
		let count = backend.get_count(key).await.unwrap();
		assert_eq!(count, 3);

		// Act - advance time past the window
		time_provider.advance(std::time::Duration::from_secs(6));

		// Assert - count should be 0 after window expiry
		let count_after = backend.get_count(key).await.unwrap();
		assert_eq!(count_after, 0);
	}

	#[rstest]
	#[tokio::test]
	async fn test_get_count_returns_count_within_window() {
		// Arrange
		use tokio::time::Instant;
		let time_provider = Arc::new(MockTimeProvider::new(Instant::now()));
		let backend = MemoryBackend::with_time_provider(time_provider.clone());
		let key = "test_key";
		let window_secs = 60;

		// Act
		backend.increment(key, window_secs).await.unwrap();
		backend.increment(key, window_secs).await.unwrap();

		// Advance time but stay within window
		time_provider.advance(std::time::Duration::from_secs(30));

		// Assert - count should still be valid
		let count = backend.get_count(key).await.unwrap();
		assert_eq!(count, 2);
	}

	#[rstest]
	#[tokio::test]
	async fn test_increment_resets_after_window_expiry() {
		// Arrange
		use tokio::time::Instant;
		let time_provider = Arc::new(MockTimeProvider::new(Instant::now()));
		let backend = MemoryBackend::with_time_provider(time_provider.clone());
		let key = "test_key";
		let window_secs = 5;

		// Act - fill up counter
		backend.increment(key, window_secs).await.unwrap();
		backend.increment(key, window_secs).await.unwrap();
		backend.increment(key, window_secs).await.unwrap();
		assert_eq!(backend.get_count(key).await.unwrap(), 3);

		// Act - advance time past window
		time_provider.advance(std::time::Duration::from_secs(6));

		// Assert - get_count should return 0 (expired)
		assert_eq!(backend.get_count(key).await.unwrap(), 0);

		// Act - new increment should reset counter
		let count = backend.increment(key, window_secs).await.unwrap();
		assert_eq!(count, 1);
		assert_eq!(backend.get_count(key).await.unwrap(), 1);
	}
}
