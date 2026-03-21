//! In-memory cache implementation

use super::cache_trait::Cache;
use super::entry::CacheEntry;
use super::layered::LayeredCacheStore;
use super::statistics::{CacheEntryInfo, CacheStatistics};
use async_trait::async_trait;
use reinhardt_core::exception::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime};
use tokio::sync::RwLock;
use tokio::task::AbortHandle;

/// Cleanup strategy for in-memory cache
#[derive(Clone, Copy, Debug)]
pub enum CleanupStrategy {
	/// Traditional O(n) full scan cleanup
	///
	/// Scans all entries to find and remove expired ones.
	/// Simple but slow for large caches.
	Naive,
	/// Layered O(1) amortized cleanup (Redis-style)
	///
	/// Uses three layers:
	/// - Passive expiration on access
	/// - Active random sampling
	/// - TTL index for batch cleanup
	///
	/// Much faster for large caches (100-1000x improvement).
	Layered,
}

/// In-memory cache backend
#[derive(Clone)]
pub struct InMemoryCache {
	store: Arc<RwLock<HashMap<String, CacheEntry>>>,
	layered_store: Option<LayeredCacheStore>,
	cleanup_strategy: CleanupStrategy,
	default_ttl: Option<Duration>,
	hits: Arc<AtomicU64>,
	misses: Arc<AtomicU64>,
	cleanup_interval: Option<Duration>,
	/// Handle for cancelling the background cleanup task
	cleanup_handle: Arc<std::sync::Mutex<Option<AbortHandle>>>,
}

impl InMemoryCache {
	/// Create a new in-memory cache with naive cleanup strategy
	///
	/// Uses traditional O(n) full scan for cleanup.
	/// Suitable for small caches or when simplicity is preferred.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_utils::cache::InMemoryCache;
	///
	/// let cache = InMemoryCache::new();
	/// // Cache is ready to use with no default TTL
	/// ```
	pub fn new() -> Self {
		Self {
			store: Arc::new(RwLock::new(HashMap::new())),
			layered_store: None,
			cleanup_strategy: CleanupStrategy::Naive,
			default_ttl: None,
			hits: Arc::new(AtomicU64::new(0)),
			misses: Arc::new(AtomicU64::new(0)),
			cleanup_interval: None,
			cleanup_handle: Arc::new(std::sync::Mutex::new(None)),
		}
	}

	/// Create a new in-memory cache with layered cleanup strategy
	///
	/// Uses Redis-style layered cleanup with O(1) amortized complexity:
	/// - Passive expiration on access
	/// - Active random sampling (default: 20 keys, 25% threshold)
	/// - TTL index for batch cleanup
	///
	/// Recommended for caches with many entries or frequent TTL usage.
	///
	/// # Performance
	///
	/// For caches with 100,000+ entries, layered cleanup is 100-1000x faster than naive cleanup.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_utils::cache::InMemoryCache;
	///
	/// let cache = InMemoryCache::with_layered_cleanup();
	/// // Use layered cleanup for better performance
	/// ```
	pub fn with_layered_cleanup() -> Self {
		Self {
			store: Arc::new(RwLock::new(HashMap::new())),
			layered_store: Some(LayeredCacheStore::new()),
			cleanup_strategy: CleanupStrategy::Layered,
			default_ttl: None,
			hits: Arc::new(AtomicU64::new(0)),
			misses: Arc::new(AtomicU64::new(0)),
			cleanup_interval: None,
			cleanup_handle: Arc::new(std::sync::Mutex::new(None)),
		}
	}

	/// Create a new in-memory cache with custom layered cleanup parameters
	///
	/// # Arguments
	///
	/// * `sample_size` - Number of keys to sample per cleanup round (default: 20)
	/// * `threshold` - Threshold for expired entries to trigger another round (default: 0.25)
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_utils::cache::InMemoryCache;
	///
	/// // Sample 50 keys per round, trigger next round if >30% expired
	/// let cache = InMemoryCache::with_custom_layered_cleanup(50, 0.30);
	/// ```
	pub fn with_custom_layered_cleanup(sample_size: usize, threshold: f32) -> Self {
		Self {
			store: Arc::new(RwLock::new(HashMap::new())),
			layered_store: Some(LayeredCacheStore::with_sampler(sample_size, threshold)),
			cleanup_strategy: CleanupStrategy::Layered,
			default_ttl: None,
			hits: Arc::new(AtomicU64::new(0)),
			misses: Arc::new(AtomicU64::new(0)),
			cleanup_interval: None,
			cleanup_handle: Arc::new(std::sync::Mutex::new(None)),
		}
	}
	/// Set a default TTL for all cache entries
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_utils::cache::{InMemoryCache, Cache};
	/// use std::time::Duration;
	///
	/// # async fn example() {
	/// let cache = InMemoryCache::new()
	///     .with_default_ttl(Duration::from_secs(1));
	///
	/// // Set a value without explicit TTL
	/// cache.set("key", &"value", None).await.unwrap();
	///
	/// // Wait for default TTL to expire
	///
	/// // Value should be expired
	/// let value: Option<String> = cache.get("key").await.unwrap();
	/// assert_eq!(value, None);
	/// # }
	/// ```
	pub fn with_default_ttl(mut self, ttl: Duration) -> Self {
		self.default_ttl = Some(ttl);
		self
	}
	/// Clean up expired entries
	///
	/// The cleanup strategy depends on how the cache was created:
	/// - Naive strategy: O(n) full scan (simple but slow for large caches)
	/// - Layered strategy: O(1) amortized (Redis-style, much faster)
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_utils::cache::{InMemoryCache, Cache};
	/// use std::time::Duration;
	///
	/// # async fn example() {
	/// // Naive cleanup
	/// let cache = InMemoryCache::new();
	///
	/// // Set a value with short TTL
	/// cache.set("key1", &"value", Some(Duration::from_millis(10))).await.unwrap();
	///
	/// // Wait for expiration
	///
	/// // Clean up expired entries (O(n) scan)
	/// cache.cleanup_expired().await;
	///
	/// // Verify the key is gone
	/// assert!(!cache.has_key("key1").await.unwrap());
	///
	/// // Layered cleanup (faster for large caches)
	/// let cache = InMemoryCache::with_layered_cleanup();
	/// cache.set("key2", &"value", Some(Duration::from_millis(10))).await.unwrap();
	/// cache.cleanup_expired().await; // O(1) amortized
	/// # }
	/// ```
	pub async fn cleanup_expired(&self) {
		match self.cleanup_strategy {
			CleanupStrategy::Naive => {
				let mut store = self.store.write().await;
				store.retain(|_, entry| !entry.is_expired());
			}
			CleanupStrategy::Layered => {
				if let Some(ref layered_store) = self.layered_store {
					layered_store.cleanup().await;
				}
			}
		}
	}

	/// Get cache statistics
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_utils::cache::{InMemoryCache, Cache};
	///
	/// # async fn example() {
	/// let cache = InMemoryCache::new();
	///
	/// // Set and get some values
	/// cache.set("key1", &"value1", None).await.unwrap();
	/// cache.set("key2", &"value2", None).await.unwrap();
	///
	/// let _: Option<String> = cache.get("key1").await.unwrap(); // Hit
	/// let _: Option<String> = cache.get("key2").await.unwrap(); // Hit
	/// let _: Option<String> = cache.get("key3").await.unwrap(); // Miss
	///
	/// let stats = cache.get_statistics().await;
	/// assert_eq!(stats.hits, 2);
	/// assert_eq!(stats.misses, 1);
	/// assert_eq!(stats.total_requests, 3);
	/// assert_eq!(stats.entry_count, 2);
	/// assert_eq!(stats.hit_rate(), 2.0 / 3.0);
	/// # }
	/// ```
	pub async fn get_statistics(&self) -> CacheStatistics {
		let hits = self.hits.load(Ordering::Relaxed);
		let misses = self.misses.load(Ordering::Relaxed);

		let (entry_count, memory_usage) = match self.cleanup_strategy {
			CleanupStrategy::Naive => {
				let store = self.store.read().await;
				let entry_count = store.len() as u64;
				let memory_usage = store
					.values()
					.map(|entry| entry.value.len() as u64)
					.sum::<u64>();
				(entry_count, memory_usage)
			}
			CleanupStrategy::Layered => {
				if let Some(ref layered_store) = self.layered_store {
					let store_clone = layered_store.get_store_clone().await;
					let entry_count = store_clone.len() as u64;
					let memory_usage = store_clone
						.values()
						.map(|entry| entry.value.len() as u64)
						.sum::<u64>();
					(entry_count, memory_usage)
				} else {
					(0, 0)
				}
			}
		};

		CacheStatistics {
			hits,
			misses,
			total_requests: hits + misses,
			entry_count,
			memory_usage,
		}
	}

	/// List all keys in the cache
	///
	/// Returns a vector of all keys currently stored in the cache,
	/// including expired entries that haven't been cleaned up yet.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_utils::cache::{InMemoryCache, Cache};
	///
	/// # async fn example() {
	/// let cache = InMemoryCache::new();
	///
	/// cache.set("key1", &"value1", None).await.unwrap();
	/// cache.set("key2", &"value2", None).await.unwrap();
	/// cache.set("key3", &"value3", None).await.unwrap();
	///
	/// let keys = cache.list_keys().await;
	/// assert_eq!(keys.len(), 3);
	/// assert!(keys.contains(&"key1".to_string()));
	/// assert!(keys.contains(&"key2".to_string()));
	/// assert!(keys.contains(&"key3".to_string()));
	/// # }
	/// ```
	pub async fn list_keys(&self) -> Vec<String> {
		match self.cleanup_strategy {
			CleanupStrategy::Naive => {
				let store = self.store.read().await;
				store.keys().cloned().collect()
			}
			CleanupStrategy::Layered => {
				if let Some(ref layered_store) = self.layered_store {
					layered_store.keys().await
				} else {
					Vec::new()
				}
			}
		}
	}

	/// Inspect cache entry timestamps without deserializing the value
	///
	/// Returns the creation and last access timestamps for a cache entry.
	/// This is useful for session metadata retrieval without deserializing the full session data.
	///
	/// # Arguments
	///
	/// * `key` - The cache key to inspect
	///
	/// # Returns
	///
	/// * `Ok(Some((created_at, accessed_at)))` - Entry found with timestamps
	/// * `Ok(None)` - Entry not found or expired
	/// * `Err(Error)` - Error occurred during inspection
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_utils::cache::{Cache, InMemoryCache};
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let cache = InMemoryCache::new();
	/// cache.set("session_123", &"session_data", None).await?;
	///
	/// if let Some((created, accessed)) = cache.inspect_entry_with_timestamps("session_123").await? {
	///     println!("Created at: {:?}", created);
	///     println!("Last accessed: {:?}", accessed);
	/// }
	/// # Ok(())
	/// # }
	/// ```
	pub async fn inspect_entry_with_timestamps(
		&self,
		key: &str,
	) -> Result<Option<(SystemTime, Option<SystemTime>)>> {
		match self.cleanup_strategy {
			CleanupStrategy::Naive => {
				let store = self.store.read().await;

				if let Some(entry) = store.get(key) {
					if entry.is_expired() {
						return Ok(None);
					}

					Ok(Some((entry.created_at, entry.accessed_at)))
				} else {
					Ok(None)
				}
			}
			CleanupStrategy::Layered => {
				if let Some(ref layered_store) = self.layered_store {
					Ok(layered_store.get_entry_timestamps(key).await)
				} else {
					Ok(None)
				}
			}
		}
	}

	/// Inspect a cache entry
	///
	/// Returns detailed information about a specific cache entry,
	/// or None if the entry doesn't exist.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_utils::cache::{InMemoryCache, Cache};
	/// use std::time::Duration;
	///
	/// # async fn example() {
	/// let cache = InMemoryCache::new();
	///
	/// // Set a value with TTL
	/// cache.set("key1", &"value1", Some(Duration::from_secs(300))).await.unwrap();
	///
	/// // Inspect the entry
	/// let info = cache.inspect_entry("key1").await;
	/// assert!(info.is_some());
	///
	/// let info = info.unwrap();
	/// assert_eq!(info.key, "key1");
	/// assert!(info.has_expiry);
	/// assert!(info.ttl_seconds.is_some());
	/// assert!(info.ttl_seconds.unwrap() <= 300);
	///
	/// // Non-existent key
	/// let info = cache.inspect_entry("nonexistent").await;
	/// assert!(info.is_none());
	/// # }
	/// ```
	pub async fn inspect_entry(&self, key: &str) -> Option<CacheEntryInfo> {
		let entry = match self.cleanup_strategy {
			CleanupStrategy::Naive => {
				let store = self.store.read().await;
				store.get(key).cloned()
			}
			CleanupStrategy::Layered => {
				if let Some(ref layered_store) = self.layered_store {
					layered_store.get_entry(key).await
				} else {
					None
				}
			}
		};

		entry.map(|entry| {
			let ttl_seconds = entry.expires_at.and_then(|expires_at| {
				expires_at
					.duration_since(SystemTime::now())
					.ok()
					.map(|d| d.as_secs())
			});

			CacheEntryInfo {
				key: key.to_string(),
				size: entry.value.len(),
				has_expiry: entry.expires_at.is_some(),
				ttl_seconds,
			}
		})
	}

	/// Start automatic cleanup of expired entries
	///
	/// Spawns a background task that periodically removes expired entries
	/// from the cache. The task runs at the specified interval.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_utils::cache::InMemoryCache;
	/// use std::time::Duration;
	///
	/// # async fn example() {
	/// let cache = InMemoryCache::new();
	///
	/// // Start cleanup every 60 seconds
	/// cache.start_auto_cleanup(Duration::from_secs(60));
	///
	/// // Cache will now automatically clean up expired entries
	/// # }
	/// ```
	pub fn start_auto_cleanup(&self, interval: Duration) {
		let mut handle_guard = self
			.cleanup_handle
			.lock()
			.unwrap_or_else(|e| e.into_inner());

		// Abort any previously running cleanup task to prevent duplicates
		if let Some(existing) = handle_guard.take() {
			existing.abort();
		}

		let cache = self.clone();
		let abort_handle = tokio::spawn(async move {
			let mut interval_timer = tokio::time::interval(interval);
			loop {
				interval_timer.tick().await;
				cache.cleanup_expired().await;
			}
		})
		.abort_handle();

		*handle_guard = Some(abort_handle);
	}

	/// Stop the background auto-cleanup task if one is running.
	///
	/// After calling this method, no further automatic cleanup will occur
	/// until `start_auto_cleanup` is called again.
	pub fn stop_auto_cleanup(&self) {
		let mut handle_guard = self
			.cleanup_handle
			.lock()
			.unwrap_or_else(|e| e.into_inner());
		if let Some(handle) = handle_guard.take() {
			handle.abort();
		}
	}

	/// Set cleanup interval and start automatic cleanup
	///
	/// This is a builder method that sets the cleanup interval
	/// and immediately starts the background cleanup task.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_utils::cache::InMemoryCache;
	/// use std::time::Duration;
	///
	/// # async fn example() {
	/// let cache = InMemoryCache::new()
	///     .with_auto_cleanup(Duration::from_secs(60));
	///
	/// // Cache will automatically clean up expired entries every 60 seconds
	/// # }
	/// ```
	pub fn with_auto_cleanup(mut self, interval: Duration) -> Self {
		self.cleanup_interval = Some(interval);
		self.start_auto_cleanup(interval);
		self
	}
}

impl Default for InMemoryCache {
	fn default() -> Self {
		Self::new()
	}
}

#[async_trait]
impl Cache for InMemoryCache {
	async fn get<T>(&self, key: &str) -> Result<Option<T>>
	where
		T: for<'de> Deserialize<'de> + Send,
	{
		match self.cleanup_strategy {
			CleanupStrategy::Naive => {
				// Use write lock to update accessed timestamp
				let mut store = self.store.write().await;

				if let Some(entry) = store.get_mut(key) {
					if entry.is_expired() {
						// Entry expired, count as miss
						self.misses.fetch_add(1, Ordering::Relaxed);
						return Ok(None);
					}

					// Cache hit - update access timestamp
					entry.touch();
					self.hits.fetch_add(1, Ordering::Relaxed);

					let value = serde_json::from_slice(&entry.value)
						.map_err(|e| Error::Serialization(e.to_string()))?;
					Ok(Some(value))
				} else {
					// Cache miss
					self.misses.fetch_add(1, Ordering::Relaxed);
					Ok(None)
				}
			}
			CleanupStrategy::Layered => {
				if let Some(ref layered_store) = self.layered_store {
					if let Some(data) = layered_store.get(key).await {
						// Cache hit
						self.hits.fetch_add(1, Ordering::Relaxed);
						let value = serde_json::from_slice(&data)
							.map_err(|e| Error::Serialization(e.to_string()))?;
						Ok(Some(value))
					} else {
						// Cache miss
						self.misses.fetch_add(1, Ordering::Relaxed);
						Ok(None)
					}
				} else {
					self.misses.fetch_add(1, Ordering::Relaxed);
					Ok(None)
				}
			}
		}
	}

	async fn set<T>(&self, key: &str, value: &T, ttl: Option<Duration>) -> Result<()>
	where
		T: Serialize + Send + Sync,
	{
		let serialized =
			serde_json::to_vec(value).map_err(|e| Error::Serialization(e.to_string()))?;

		let ttl = ttl.or(self.default_ttl);

		match self.cleanup_strategy {
			CleanupStrategy::Naive => {
				let entry = CacheEntry::new(serialized, ttl);
				let mut store = self.store.write().await;
				store.insert(key.to_string(), entry);
			}
			CleanupStrategy::Layered => {
				if let Some(ref layered_store) = self.layered_store {
					layered_store.set(key.to_string(), serialized, ttl).await;
				}
			}
		}

		Ok(())
	}

	async fn delete(&self, key: &str) -> Result<()> {
		match self.cleanup_strategy {
			CleanupStrategy::Naive => {
				let mut store = self.store.write().await;
				store.remove(key);
			}
			CleanupStrategy::Layered => {
				if let Some(ref layered_store) = self.layered_store {
					layered_store.delete(key).await;
				}
			}
		}
		Ok(())
	}

	async fn has_key(&self, key: &str) -> Result<bool> {
		match self.cleanup_strategy {
			CleanupStrategy::Naive => {
				let store = self.store.read().await;

				if let Some(entry) = store.get(key) {
					Ok(!entry.is_expired())
				} else {
					Ok(false)
				}
			}
			CleanupStrategy::Layered => {
				if let Some(ref layered_store) = self.layered_store {
					Ok(layered_store.has_key(key).await)
				} else {
					Ok(false)
				}
			}
		}
	}

	async fn clear(&self) -> Result<()> {
		match self.cleanup_strategy {
			CleanupStrategy::Naive => {
				let mut store = self.store.write().await;
				store.clear();
			}
			CleanupStrategy::Layered => {
				if let Some(ref layered_store) = self.layered_store {
					layered_store.clear().await;
				}
			}
		}
		Ok(())
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	/// Polls a condition until it returns true or timeout is reached.
	async fn poll_until<F, Fut>(
		timeout: std::time::Duration,
		interval: std::time::Duration,
		mut condition: F,
	) -> std::result::Result<(), String>
	where
		F: FnMut() -> Fut,
		Fut: std::future::Future<Output = bool>,
	{
		let start = std::time::Instant::now();
		while start.elapsed() < timeout {
			if condition().await {
				return Ok(());
			}
			tokio::time::sleep(interval).await;
		}
		Err(format!("Timeout after {:?} waiting for condition", timeout))
	}

	#[tokio::test]
	async fn test_in_memory_cache_basic() {
		let cache = InMemoryCache::new();

		// Set and get
		cache.set("key1", &"value1", None).await.unwrap();
		let value: Option<String> = cache.get("key1").await.unwrap();
		assert_eq!(value, Some("value1".to_string()));

		// Has key
		assert!(cache.has_key("key1").await.unwrap());
		assert!(!cache.has_key("key2").await.unwrap());

		// Delete
		cache.delete("key1").await.unwrap();
		let value: Option<String> = cache.get("key1").await.unwrap();
		assert_eq!(value, None);
	}

	#[tokio::test]
	async fn test_in_memory_cache_ttl() {
		let cache = InMemoryCache::new();

		// Set with short TTL
		cache
			.set("key1", &"value1", Some(Duration::from_millis(100)))
			.await
			.unwrap();

		// Should exist immediately
		let value: Option<String> = cache.get("key1").await.unwrap();
		assert_eq!(value, Some("value1".to_string()));

		// Poll until key expires
		poll_until(
			Duration::from_millis(200),
			Duration::from_millis(10),
			|| async {
				let value: Option<String> = cache.get("key1").await.unwrap();
				value.is_none()
			},
		)
		.await
		.expect("Key should expire within 200ms");
	}

	#[tokio::test]
	async fn test_in_memory_cache_many() {
		let cache = InMemoryCache::new();

		// Set many
		let mut values = HashMap::new();
		values.insert("key1".to_string(), "value1".to_string());
		values.insert("key2".to_string(), "value2".to_string());
		cache.set_many(values, None).await.unwrap();

		// Get many
		let results: HashMap<String, String> =
			cache.get_many(&["key1", "key2", "key3"]).await.unwrap();
		assert_eq!(results.len(), 2);
		assert_eq!(results.get("key1"), Some(&"value1".to_string()));
		assert_eq!(results.get("key2"), Some(&"value2".to_string()));

		// Delete many
		cache.delete_many(&["key1", "key2"]).await.unwrap();
		assert!(!cache.has_key("key1").await.unwrap());
		assert!(!cache.has_key("key2").await.unwrap());
	}

	#[tokio::test]
	async fn test_in_memory_cache_incr_decr() {
		let cache = InMemoryCache::new();

		// Increment from zero
		let value = cache.incr("counter", 5).await.unwrap();
		assert_eq!(value, 5);

		// Increment again
		let value = cache.incr("counter", 3).await.unwrap();
		assert_eq!(value, 8);

		// Decrement
		let value = cache.decr("counter", 2).await.unwrap();
		assert_eq!(value, 6);
	}

	#[tokio::test]
	async fn test_in_memory_cache_clear() {
		let cache = InMemoryCache::new();

		cache.set("key1", &"value1", None).await.unwrap();
		cache.set("key2", &"value2", None).await.unwrap();

		cache.clear().await.unwrap();

		assert!(!cache.has_key("key1").await.unwrap());
		assert!(!cache.has_key("key2").await.unwrap());
	}

	#[tokio::test]
	async fn test_cache_cleanup_expired() {
		let cache = InMemoryCache::new();

		// Set some values with different TTLs
		cache
			.set("key1", &"value1", Some(Duration::from_millis(100)))
			.await
			.unwrap();
		cache.set("key2", &"value2", None).await.unwrap();

		// Poll until first key expires
		poll_until(
			Duration::from_millis(200),
			Duration::from_millis(10),
			|| async {
				let value: Option<String> = cache.get("key1").await.unwrap();
				value.is_none()
			},
		)
		.await
		.expect("Key1 should expire within 200ms");

		// Cleanup expired entries
		cache.cleanup_expired().await;

		// key1 should be gone, key2 should remain
		assert!(!cache.has_key("key1").await.unwrap());
		assert!(cache.has_key("key2").await.unwrap());
	}

	#[tokio::test]
	async fn test_cache_statistics_basic() {
		let cache = InMemoryCache::new();

		// Initially, stats should be zero
		let stats = cache.get_statistics().await;
		assert_eq!(stats.hits, 0);
		assert_eq!(stats.misses, 0);
		assert_eq!(stats.total_requests, 0);
		assert_eq!(stats.entry_count, 0);
		assert_eq!(stats.memory_usage, 0);

		// Set some values
		cache.set("key1", &"value1", None).await.unwrap();
		cache.set("key2", &"value2", None).await.unwrap();

		// Check entry count
		let stats = cache.get_statistics().await;
		assert_eq!(stats.entry_count, 2);
		assert!(stats.memory_usage > 0);

		// Get existing keys (hits)
		let _: Option<String> = cache.get("key1").await.unwrap();
		let _: Option<String> = cache.get("key2").await.unwrap();

		let stats = cache.get_statistics().await;
		assert_eq!(stats.hits, 2);
		assert_eq!(stats.misses, 0);
		assert_eq!(stats.total_requests, 2);

		// Get non-existing key (miss)
		let _: Option<String> = cache.get("key3").await.unwrap();

		let stats = cache.get_statistics().await;
		assert_eq!(stats.hits, 2);
		assert_eq!(stats.misses, 1);
		assert_eq!(stats.total_requests, 3);
	}

	#[tokio::test]
	async fn test_cache_statistics_hit_miss_rate() {
		let cache = InMemoryCache::new();

		cache.set("key1", &"value1", None).await.unwrap();
		cache.set("key2", &"value2", None).await.unwrap();

		// 2 hits
		let _: Option<String> = cache.get("key1").await.unwrap();
		let _: Option<String> = cache.get("key2").await.unwrap();

		// 1 miss
		let _: Option<String> = cache.get("key3").await.unwrap();

		let stats = cache.get_statistics().await;
		assert_eq!(stats.hit_rate(), 2.0 / 3.0);
		assert_eq!(stats.miss_rate(), 1.0 / 3.0);
	}

	#[tokio::test]
	async fn test_cache_statistics_expired_counts_as_miss() {
		let cache = InMemoryCache::new();

		// Set with short TTL
		cache
			.set("key1", &"value1", Some(Duration::from_millis(10)))
			.await
			.unwrap();

		// Wait for key to expire (10ms TTL + 5ms buffer)
		tokio::time::sleep(Duration::from_millis(15)).await;

		// Access expired key - should count as miss
		let value: Option<String> = cache.get("key1").await.unwrap();
		assert!(value.is_none(), "Key should have expired");

		// Statistics should show exactly 1 miss and 0 hits
		let stats = cache.get_statistics().await;
		assert_eq!(stats.hits, 0, "Expected 0 hits, got {}", stats.hits);
		assert_eq!(stats.misses, 1, "Expected 1 miss, got {}", stats.misses);
	}

	#[tokio::test]
	async fn test_cache_statistics_memory_usage() {
		let cache = InMemoryCache::new();

		// Set some values
		cache.set("key1", &"short", None).await.unwrap();
		cache.set("key2", &"a longer value", None).await.unwrap();

		let stats = cache.get_statistics().await;
		assert!(stats.memory_usage > 0);

		// Memory usage should increase with more data
		let initial_usage = stats.memory_usage;

		cache
			.set("key3", &"even longer value here", None)
			.await
			.unwrap();

		let stats = cache.get_statistics().await;
		assert!(stats.memory_usage > initial_usage);
	}

	#[tokio::test]
	async fn test_list_keys() {
		let cache = InMemoryCache::new();

		// Initially empty
		let keys = cache.list_keys().await;
		assert_eq!(keys.len(), 0);

		// Add some keys
		cache.set("key1", &"value1", None).await.unwrap();
		cache.set("key2", &"value2", None).await.unwrap();
		cache.set("key3", &"value3", None).await.unwrap();

		let keys = cache.list_keys().await;
		assert_eq!(keys.len(), 3);
		assert!(keys.contains(&"key1".to_string()));
		assert!(keys.contains(&"key2".to_string()));
		assert!(keys.contains(&"key3".to_string()));

		// Delete a key
		cache.delete("key2").await.unwrap();

		let keys = cache.list_keys().await;
		assert_eq!(keys.len(), 2);
		assert!(keys.contains(&"key1".to_string()));
		assert!(!keys.contains(&"key2".to_string()));
		assert!(keys.contains(&"key3".to_string()));
	}

	#[tokio::test]
	async fn test_list_keys_includes_expired() {
		let cache = InMemoryCache::new();

		// Set a key with short TTL
		cache
			.set("expired_key", &"value", Some(Duration::from_millis(10)))
			.await
			.unwrap();

		// Set a key without TTL
		cache.set("valid_key", &"value", None).await.unwrap();

		// Poll until first key expires
		poll_until(
			Duration::from_millis(50),
			Duration::from_millis(5),
			|| async {
				let value: Option<String> = cache.get("expired_key").await.unwrap();
				value.is_none()
			},
		)
		.await
		.expect("Expired key should expire within 50ms");

		// list_keys should include expired keys (until cleanup)
		let keys = cache.list_keys().await;
		assert_eq!(keys.len(), 2);
		assert!(keys.contains(&"expired_key".to_string()));
		assert!(keys.contains(&"valid_key".to_string()));

		// After cleanup, expired key should be gone
		cache.cleanup_expired().await;
		let keys = cache.list_keys().await;
		assert_eq!(keys.len(), 1);
		assert!(!keys.contains(&"expired_key".to_string()));
		assert!(keys.contains(&"valid_key".to_string()));
	}

	#[tokio::test]
	async fn test_inspect_entry_basic() {
		let cache = InMemoryCache::new();

		// Non-existent key
		let info = cache.inspect_entry("nonexistent").await;
		assert!(info.is_none());

		// Add a key without expiry
		cache.set("key1", &"value1", None).await.unwrap();

		let info = cache.inspect_entry("key1").await;
		let info = info.unwrap();
		assert_eq!(info.key, "key1");
		assert!(!info.has_expiry);
		assert!(info.ttl_seconds.is_none());
		assert!(info.size > 0);
	}

	#[tokio::test]
	async fn test_inspect_entry_with_ttl() {
		let cache = InMemoryCache::new();

		// Set a value with TTL
		cache
			.set("key1", &"value1", Some(Duration::from_secs(300)))
			.await
			.unwrap();

		let info = cache.inspect_entry("key1").await;
		let info = info.unwrap();
		assert_eq!(info.key, "key1");
		assert!(info.has_expiry);
		assert!(info.ttl_seconds.is_some());

		// TTL should be <= 300 seconds
		let ttl = info.ttl_seconds.unwrap();
		assert!(ttl <= 300);
		assert!(ttl > 0);
	}

	#[tokio::test]
	async fn test_inspect_entry_size() {
		let cache = InMemoryCache::new();

		// Set values of different sizes
		cache.set("small", &"x", None).await.unwrap();
		cache.set("large", &"x".repeat(1000), None).await.unwrap();

		let small_info = cache.inspect_entry("small").await.unwrap();
		let large_info = cache.inspect_entry("large").await.unwrap();

		assert!(large_info.size > small_info.size);
	}

	#[tokio::test]
	async fn test_inspect_entry_expired() {
		let cache = InMemoryCache::new();

		// Set with short TTL
		cache
			.set("key1", &"value1", Some(Duration::from_millis(10)))
			.await
			.unwrap();

		// Inspect before expiration
		let info = cache.inspect_entry("key1").await;
		assert!(info.is_some());

		// Poll until key expires
		poll_until(
			Duration::from_millis(50),
			Duration::from_millis(5),
			|| async {
				let value: Option<String> = cache.get("key1").await.unwrap();
				value.is_none()
			},
		)
		.await
		.expect("Key should expire within 50ms");

		// Entry still exists (not cleaned up)
		let info = cache.inspect_entry("key1").await;
		let info = info.unwrap();
		assert!(info.has_expiry);
		// TTL should be None because it's expired
		assert!(info.ttl_seconds.is_none());

		// After cleanup, entry should be gone
		cache.cleanup_expired().await;
		let info = cache.inspect_entry("key1").await;
		assert!(info.is_none());
	}

	#[tokio::test]
	async fn test_start_auto_cleanup() {
		let cache = InMemoryCache::new();

		// Set some values with short TTL
		cache
			.set("key1", &"value1", Some(Duration::from_millis(50)))
			.await
			.unwrap();
		cache
			.set("key2", &"value2", Some(Duration::from_millis(50)))
			.await
			.unwrap();

		// Start auto cleanup with short interval
		cache.start_auto_cleanup(Duration::from_millis(30));

		// Keys should exist initially
		assert!(cache.has_key("key1").await.unwrap());
		assert!(cache.has_key("key2").await.unwrap());

		// Poll until auto-cleanup removes expired keys
		poll_until(
			Duration::from_millis(200),
			Duration::from_millis(10),
			|| async {
				!cache.has_key("key1").await.unwrap() && !cache.has_key("key2").await.unwrap()
			},
		)
		.await
		.expect("Keys should be auto-cleaned within 200ms");

		// Keys should be cleaned up automatically
		assert!(!cache.has_key("key1").await.unwrap());
		assert!(!cache.has_key("key2").await.unwrap());
	}

	#[tokio::test]
	async fn test_with_auto_cleanup() {
		let cache = InMemoryCache::new().with_auto_cleanup(Duration::from_millis(30));

		// Set values with short TTL
		cache
			.set("key1", &"value1", Some(Duration::from_millis(50)))
			.await
			.unwrap();
		cache
			.set("key2", &"value2", Some(Duration::from_millis(50)))
			.await
			.unwrap();

		// Keys should exist initially
		assert!(cache.has_key("key1").await.unwrap());
		assert!(cache.has_key("key2").await.unwrap());

		// Poll until auto-cleanup removes expired keys
		poll_until(
			Duration::from_millis(200),
			Duration::from_millis(10),
			|| async {
				!cache.has_key("key1").await.unwrap() && !cache.has_key("key2").await.unwrap()
			},
		)
		.await
		.expect("Keys should be auto-cleaned within 200ms");
	}

	#[tokio::test]
	async fn test_stop_auto_cleanup() {
		let cache = InMemoryCache::new();

		// Start auto cleanup
		cache.start_auto_cleanup(Duration::from_millis(30));

		// Set a value with short TTL
		cache
			.set("key1", &"value1", Some(Duration::from_millis(50)))
			.await
			.unwrap();

		// Stop cleanup before it can run
		cache.stop_auto_cleanup();

		// Wait long enough for cleanup to have run if it were still active
		tokio::time::sleep(Duration::from_millis(150)).await;

		// Key should be expired but not cleaned up from store (only passive expiration)
		let value: Option<String> = cache.get("key1").await.unwrap();
		assert!(value.is_none(), "Key should be expired");
	}

	#[tokio::test]
	async fn test_start_auto_cleanup_replaces_previous() {
		let cache = InMemoryCache::new();

		// Start cleanup twice - should not spawn duplicate tasks
		cache.start_auto_cleanup(Duration::from_millis(30));
		cache.start_auto_cleanup(Duration::from_millis(30));

		// Set a value with short TTL
		cache
			.set("key1", &"value1", Some(Duration::from_millis(50)))
			.await
			.unwrap();

		// Wait for cleanup
		poll_until(
			Duration::from_millis(200),
			Duration::from_millis(10),
			|| async { !cache.has_key("key1").await.unwrap() },
		)
		.await
		.expect("Key should be cleaned up");
	}

	#[tokio::test]
	async fn test_auto_cleanup_preserves_non_expired() {
		let cache = InMemoryCache::new();

		// Start auto cleanup
		cache.start_auto_cleanup(Duration::from_millis(30));

		// Set one key with short TTL and one without
		cache
			.set("short_lived", &"value1", Some(Duration::from_millis(50)))
			.await
			.unwrap();
		cache.set("long_lived", &"value2", None).await.unwrap();

		// Both should exist initially
		assert!(cache.has_key("short_lived").await.unwrap());
		assert!(cache.has_key("long_lived").await.unwrap());

		// Poll until auto-cleanup removes short_lived key
		poll_until(
			Duration::from_millis(200),
			Duration::from_millis(10),
			|| async {
				!cache.has_key("short_lived").await.unwrap()
					&& cache.has_key("long_lived").await.unwrap()
			},
		)
		.await
		.expect("Short-lived key should be cleaned, long-lived should remain");
	}

	// Layered cleanup strategy tests

	#[tokio::test]
	async fn test_layered_cache_basic() {
		let cache = InMemoryCache::with_layered_cleanup();

		// Set and get
		cache.set("key1", &"value1", None).await.unwrap();
		let value: Option<String> = cache.get("key1").await.unwrap();
		assert_eq!(value, Some("value1".to_string()));

		// Has key
		assert!(cache.has_key("key1").await.unwrap());
		assert!(!cache.has_key("key2").await.unwrap());

		// Delete
		cache.delete("key1").await.unwrap();
		let value: Option<String> = cache.get("key1").await.unwrap();
		assert_eq!(value, None);
	}

	#[tokio::test]
	async fn test_layered_cache_ttl() {
		let cache = InMemoryCache::with_layered_cleanup();

		// Set with short TTL
		cache
			.set("key1", &"value1", Some(Duration::from_millis(100)))
			.await
			.unwrap();

		// Should exist immediately
		let value: Option<String> = cache.get("key1").await.unwrap();
		assert_eq!(value, Some("value1".to_string()));

		// Poll until key expires (passive expiration on get)
		poll_until(
			Duration::from_millis(200),
			Duration::from_millis(10),
			|| async {
				let value: Option<String> = cache.get("key1").await.unwrap();
				value.is_none()
			},
		)
		.await
		.expect("Key should expire within 200ms");
	}

	#[tokio::test]
	async fn test_layered_cleanup_expired() {
		let cache = InMemoryCache::with_layered_cleanup();

		// Set some values with different TTLs
		cache
			.set("key1", &"value1", Some(Duration::from_millis(50)))
			.await
			.unwrap();
		cache.set("key2", &"value2", None).await.unwrap();

		// Wait for first key to expire
		tokio::time::sleep(Duration::from_millis(100)).await;

		// Cleanup expired entries
		cache.cleanup_expired().await;

		// key1 should be gone, key2 should remain
		assert!(!cache.has_key("key1").await.unwrap());
		assert!(cache.has_key("key2").await.unwrap());
	}

	#[tokio::test]
	async fn test_layered_cache_statistics() {
		let cache = InMemoryCache::with_layered_cleanup();

		// Set some values
		cache.set("key1", &"value1", None).await.unwrap();
		cache.set("key2", &"value2", None).await.unwrap();

		let stats = cache.get_statistics().await;
		assert_eq!(stats.entry_count, 2);

		// Get existing keys (hits)
		let _: Option<String> = cache.get("key1").await.unwrap();
		let _: Option<String> = cache.get("key2").await.unwrap();

		let stats = cache.get_statistics().await;
		assert_eq!(stats.hits, 2);
		assert_eq!(stats.misses, 0);

		// Get non-existing key (miss)
		let _: Option<String> = cache.get("key3").await.unwrap();

		let stats = cache.get_statistics().await;
		assert_eq!(stats.hits, 2);
		assert_eq!(stats.misses, 1);
	}

	#[tokio::test]
	async fn test_layered_list_keys() {
		let cache = InMemoryCache::with_layered_cleanup();

		// Initially empty
		let keys = cache.list_keys().await;
		assert_eq!(keys.len(), 0);

		// Add some keys
		cache.set("key1", &"value1", None).await.unwrap();
		cache.set("key2", &"value2", None).await.unwrap();
		cache.set("key3", &"value3", None).await.unwrap();

		let keys = cache.list_keys().await;
		assert_eq!(keys.len(), 3);
		assert!(keys.contains(&"key1".to_string()));
		assert!(keys.contains(&"key2".to_string()));
		assert!(keys.contains(&"key3".to_string()));
	}

	#[tokio::test]
	async fn test_layered_inspect_entry() {
		let cache = InMemoryCache::with_layered_cleanup();

		// Non-existent key
		let info = cache.inspect_entry("nonexistent").await;
		assert!(info.is_none());

		// Add a key with TTL
		cache
			.set("key1", &"value1", Some(Duration::from_secs(300)))
			.await
			.unwrap();

		let info = cache.inspect_entry("key1").await;
		let info = info.unwrap();
		assert_eq!(info.key, "key1");
		assert!(info.has_expiry);
		assert!(info.ttl_seconds.is_some());
		assert!(info.ttl_seconds.unwrap() <= 300);
	}

	#[tokio::test]
	async fn test_layered_large_dataset() {
		let cache = InMemoryCache::with_layered_cleanup();

		// Set many keys with short TTL
		let num_keys = 1000;
		for i in 0..num_keys {
			cache
				.set(
					&format!("key{}", i),
					&format!("value{}", i),
					Some(Duration::from_millis(50)),
				)
				.await
				.unwrap();
		}

		// All keys should exist
		let stats = cache.get_statistics().await;
		assert_eq!(stats.entry_count, num_keys);

		// Wait for expiration
		tokio::time::sleep(Duration::from_millis(60)).await;

		// Cleanup (should be fast with layered strategy)
		cache.cleanup_expired().await;

		// All keys should be gone
		let stats = cache.get_statistics().await;
		assert_eq!(stats.entry_count, 0);
	}

	#[tokio::test]
	async fn test_custom_layered_cleanup() {
		// Create cache with custom sampler (sample 50 keys, 30% threshold)
		let cache = InMemoryCache::with_custom_layered_cleanup(50, 0.30);

		// Set many keys
		for i in 0..100 {
			cache
				.set(
					&format!("key{}", i),
					&format!("value{}", i),
					Some(Duration::from_millis(50)),
				)
				.await
				.unwrap();
		}

		// Wait for expiration
		tokio::time::sleep(Duration::from_millis(100)).await;

		// Cleanup with custom sampler
		cache.cleanup_expired().await;

		// All keys should be cleaned up
		let stats = cache.get_statistics().await;
		assert_eq!(stats.entry_count, 0);
	}
}
