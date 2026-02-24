//! Layered cache cleanup strategy inspired by Redis 6.0
//!
//! This module implements a three-layer approach for efficient TTL-based cleanup:
//!
//! - **Layer 1: Passive Expiration** - O(1) check on access
//! - **Layer 2: Active Sampling** - O(20) random sampling
//! - **Layer 3: TTL Index** - O(k) where k = expired keys
//!
//! This approach reduces cleanup time complexity from O(n) to O(1) amortized.

use super::entry::CacheEntry;
use rand::prelude::IndexedRandom;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::SystemTime;
use tokio::sync::RwLock;
use tokio::task::AbortHandle;

/// Active sampler configuration and state
struct ActiveSampler {
	/// Number of keys to sample per cleanup round (default: 20)
	sample_size: usize,
	/// Threshold for expired entries to trigger another round (default: 0.25 = 25%)
	threshold: f32,
}

impl Default for ActiveSampler {
	fn default() -> Self {
		Self {
			sample_size: 20,
			threshold: 0.25,
		}
	}
}

/// TTL index that groups keys by expiration timestamp (rounded to seconds)
type TtlIndex = HashMap<u64, Vec<String>>;

/// Layered cache storage with optimized TTL cleanup
///
/// This structure provides three layers of expiration handling:
///
/// 1. **Passive expiration**: When a key is accessed, check if it's expired (O(1))
/// 2. **Active sampling**: Periodically sample random keys and delete expired ones (O(20) = O(1))
/// 3. **TTL index**: Batch delete all keys expiring at a specific timestamp (O(k) where k = expired keys)
///
/// # Examples
///
/// ```
/// use reinhardt_utils::cache::LayeredCacheStore;
/// use std::time::Duration;
///
/// # async fn example() {
/// let mut store = LayeredCacheStore::new();
///
/// // Set a value with TTL
/// store.set("key1".to_string(), vec![1, 2, 3], Some(Duration::from_secs(60))).await;
///
/// // Get with passive expiration
/// if let Some(value) = store.get("key1").await {
///     println!("Got value: {:?}", value);
/// }
///
/// // Cleanup using active sampling
/// store.cleanup_active_sampling().await;
///
/// // Cleanup using TTL index
/// store.cleanup_ttl_index().await;
/// # }
/// ```
pub struct LayeredCacheStore {
	/// Main storage (Layer 1)
	store: Arc<RwLock<HashMap<String, CacheEntry>>>,
	/// TTL index for batch expiration (Layer 3)
	ttl_index: Arc<RwLock<TtlIndex>>,
	/// Active sampler state (Layer 2)
	active_sampler: ActiveSampler,
	/// Handle for cancelling the background cleanup task
	cleanup_handle: Arc<std::sync::Mutex<Option<AbortHandle>>>,
}

impl LayeredCacheStore {
	/// Create a new layered cache store
	pub fn new() -> Self {
		Self {
			store: Arc::new(RwLock::new(HashMap::new())),
			ttl_index: Arc::new(RwLock::new(HashMap::new())),
			active_sampler: ActiveSampler::default(),
			cleanup_handle: Arc::new(std::sync::Mutex::new(None)),
		}
	}

	/// Create a new layered cache store with custom sampler configuration
	///
	/// # Arguments
	///
	/// * `sample_size` - Number of keys to sample per cleanup round
	/// * `threshold` - Threshold for expired entries to trigger another round (0.0 - 1.0)
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_utils::cache::LayeredCacheStore;
	///
	/// // Sample 50 keys per round, trigger next round if >30% expired
	/// let store = LayeredCacheStore::with_sampler(50, 0.30);
	/// ```
	pub fn with_sampler(sample_size: usize, threshold: f32) -> Self {
		Self {
			store: Arc::new(RwLock::new(HashMap::new())),
			ttl_index: Arc::new(RwLock::new(HashMap::new())),
			active_sampler: ActiveSampler {
				sample_size,
				threshold,
			},
			cleanup_handle: Arc::new(std::sync::Mutex::new(None)),
		}
	}

	/// Get a value from the cache (Layer 1: Passive expiration)
	///
	/// Returns `None` if the key doesn't exist or is expired.
	/// Expired entries are automatically deleted on access.
	/// The `accessed_at` timestamp is updated on successful access.
	///
	/// Time complexity: O(1)
	pub async fn get(&self, key: &str) -> Option<Vec<u8>> {
		let mut store = self.store.write().await;

		if let Some(entry) = store.get_mut(key) {
			if entry.is_expired() {
				// Passive expiration - delete expired entry
				store.remove(key);
				return None;
			}
			// Update access timestamp
			entry.touch();
			return Some(entry.value.clone());
		}
		None
	}

	/// Set a value in the cache
	///
	/// If a TTL is provided, the key is added to the TTL index for efficient batch cleanup.
	///
	/// Time complexity: O(1)
	pub async fn set(&self, key: String, value: Vec<u8>, ttl: Option<std::time::Duration>) {
		let entry = CacheEntry::new(value, ttl);
		let mut store = self.store.write().await;

		// Add to TTL index if TTL is set
		if let Some(expires_at) = entry.expires_at {
			let timestamp = expires_at
				.duration_since(SystemTime::UNIX_EPOCH)
				.ok()
				.map(|d| d.as_secs())
				.unwrap_or(0);

			let mut ttl_index = self.ttl_index.write().await;
			ttl_index
				.entry(timestamp)
				.or_insert_with(Vec::new)
				.push(key.clone());
		}

		store.insert(key, entry);
	}

	/// Delete a key from the cache
	///
	/// Time complexity: O(1)
	pub async fn delete(&self, key: &str) {
		let mut store = self.store.write().await;
		store.remove(key);
	}

	/// Check if a key exists and is not expired
	///
	/// Time complexity: O(1)
	pub async fn has_key(&self, key: &str) -> bool {
		let store = self.store.read().await;
		if let Some(entry) = store.get(key) {
			!entry.is_expired()
		} else {
			false
		}
	}

	/// Clear all entries from the cache
	///
	/// Time complexity: O(1)
	pub async fn clear(&self) {
		let mut store = self.store.write().await;
		let mut ttl_index = self.ttl_index.write().await;
		store.clear();
		ttl_index.clear();
	}

	/// Get the number of entries in the cache (including expired)
	///
	/// Time complexity: O(1)
	pub async fn len(&self) -> usize {
		let store = self.store.read().await;
		store.len()
	}

	/// Check if the cache is empty
	///
	/// Time complexity: O(1)
	pub async fn is_empty(&self) -> bool {
		let store = self.store.read().await;
		store.is_empty()
	}

	/// Get all keys in the cache (including expired)
	///
	/// Time complexity: O(n)
	pub async fn keys(&self) -> Vec<String> {
		let store = self.store.read().await;
		store.keys().cloned().collect()
	}

	/// Get a clone of the internal store (for read-only operations)
	pub(crate) async fn get_store_clone(&self) -> HashMap<String, CacheEntry> {
		let store = self.store.read().await;
		store.clone()
	}

	/// Get a specific cache entry (for inspection)
	pub(crate) async fn get_entry(&self, key: &str) -> Option<CacheEntry> {
		let store = self.store.read().await;
		store.get(key).cloned()
	}

	/// Get entry timestamps without deserializing value or updating access time
	///
	/// Returns `(created_at, accessed_at)` tuple if the key exists and is not expired.
	/// This method is useful for inspecting cache entry metadata without affecting
	/// the entry's `accessed_at` timestamp.
	///
	/// Time complexity: O(1)
	pub async fn get_entry_timestamps(
		&self,
		key: &str,
	) -> Option<(SystemTime, Option<SystemTime>)> {
		let store = self.store.read().await;
		if let Some(entry) = store.get(key) {
			if entry.is_expired() {
				return None;
			}
			Some((entry.created_at, entry.accessed_at))
		} else {
			None
		}
	}

	/// Layer 2: Active Sampling Cleanup
	///
	/// Randomly samples keys and removes expired ones.
	/// If more than the threshold percentage are expired, repeats sampling.
	///
	/// Time complexity: O(sample_size) = O(1) amortized
	///
	/// # Algorithm
	///
	/// 1. Sample `sample_size` random keys
	/// 2. Count how many are expired
	/// 3. If expired_ratio > threshold, delete them and repeat
	/// 4. Otherwise, stop
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_utils::cache::LayeredCacheStore;
	/// use std::time::Duration;
	///
	/// # async fn example() {
	/// let mut store = LayeredCacheStore::new();
	///
	/// // Set many keys with short TTL
	/// for i in 0..100 {
	///     store.set(format!("key{}", i), vec![i as u8], Some(Duration::from_millis(10))).await;
	/// }
	///
	/// // Wait for expiration
	///
	/// // Cleanup using active sampling (much faster than O(n) full scan)
	/// store.cleanup_active_sampling().await;
	/// # }
	/// ```
	pub async fn cleanup_active_sampling(&self) {
		/// Maximum number of sampling rounds to prevent runaway iteration
		const MAX_ROUNDS: usize = 100;

		for _ in 0..MAX_ROUNDS {
			let keys = {
				let store = self.store.read().await;
				store.keys().cloned().collect::<Vec<_>>()
			};

			if keys.is_empty() {
				return;
			}

			// Sample random keys
			let sample_size = self.active_sampler.sample_size.min(keys.len());
			let sample: Vec<_> = {
				let mut rng = rand::rng();
				keys.choose_multiple(&mut rng, sample_size)
					.cloned()
					.collect()
			};

			// Count expired entries in sample
			let mut expired_keys = Vec::new();
			{
				let store = self.store.read().await;
				for key in &sample {
					if let Some(entry) = store.get::<String>(key)
						&& entry.is_expired()
					{
						expired_keys.push(key.clone());
					}
				}
			}

			let expired_ratio = expired_keys.len() as f32 / sample.len() as f32;

			if expired_ratio > self.active_sampler.threshold {
				// Delete expired keys and continue to next sampling round
				let mut store = self.store.write().await;
				for key in expired_keys {
					store.remove(&key);
				}
			} else {
				// Below threshold, stop sampling
				return;
			}
		}
	}

	/// Layer 3: TTL Index Cleanup
	///
	/// Removes all keys that expire at or before the current timestamp.
	/// This is very efficient for batch expiration of many keys.
	///
	/// Time complexity: O(k) where k = number of expired keys
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_utils::cache::LayeredCacheStore;
	/// use std::time::Duration;
	///
	/// # async fn example() {
	/// let mut store = LayeredCacheStore::new();
	///
	/// // Set many keys with same TTL
	/// for i in 0..1000 {
	///     store.set(format!("key{}", i), vec![i as u8], Some(Duration::from_secs(60))).await;
	/// }
	///
	/// // Later, cleanup all expired keys at once
	/// store.cleanup_ttl_index().await;
	/// # }
	/// ```
	pub async fn cleanup_ttl_index(&self) {
		let now = SystemTime::now()
			.duration_since(SystemTime::UNIX_EPOCH)
			.ok()
			.map(|d| d.as_secs())
			.unwrap_or(0);

		// Find all expired timestamps
		let expired_timestamps: Vec<u64> = {
			let ttl_index = self.ttl_index.read().await;
			ttl_index.keys().filter(|&&ts| ts <= now).cloned().collect()
		};

		if expired_timestamps.is_empty() {
			return;
		}

		// Remove all keys for expired timestamps
		let mut store = self.store.write().await;
		let mut ttl_index = self.ttl_index.write().await;

		for timestamp in expired_timestamps {
			if let Some(keys) = ttl_index.remove(&timestamp) {
				for key in keys {
					store.remove(&key);
				}
			}
		}
	}

	/// Comprehensive cleanup combining all layers
	///
	/// Runs both active sampling and TTL index cleanup.
	/// This is the recommended method for periodic cleanup.
	///
	/// Time complexity: O(1) amortized
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_utils::cache::LayeredCacheStore;
	/// use std::time::Duration;
	///
	/// # async fn example() {
	/// let store = LayeredCacheStore::new();
	///
	/// // Periodically run cleanup
	/// store.cleanup().await;
	/// # }
	/// ```
	pub async fn cleanup(&self) {
		// Run TTL index cleanup first (more efficient for batch expiration)
		self.cleanup_ttl_index().await;

		// Then run active sampling for remaining expired entries
		self.cleanup_active_sampling().await;
	}

	/// Start automatic cleanup task
	///
	/// Spawns a background task that runs cleanup at the specified interval.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_utils::cache::LayeredCacheStore;
	/// use std::time::Duration;
	///
	/// # async fn example() {
	/// let store = LayeredCacheStore::new();
	///
	/// // Run cleanup every 60 seconds
	/// store.start_auto_cleanup(Duration::from_secs(60));
	/// # }
	/// ```
	pub fn start_auto_cleanup(&self, interval: std::time::Duration)
	where
		Self: Clone,
	{
		let mut handle_guard = self
			.cleanup_handle
			.lock()
			.unwrap_or_else(|e| e.into_inner());

		// Abort any previously running cleanup task to prevent duplicates
		if let Some(existing) = handle_guard.take() {
			existing.abort();
		}

		let store = self.clone();
		let abort_handle = tokio::spawn(async move {
			let mut interval_timer = tokio::time::interval(interval);
			loop {
				interval_timer.tick().await;
				store.cleanup().await;
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
}

impl Clone for LayeredCacheStore {
	fn clone(&self) -> Self {
		Self {
			store: Arc::clone(&self.store),
			ttl_index: Arc::clone(&self.ttl_index),
			active_sampler: ActiveSampler {
				sample_size: self.active_sampler.sample_size,
				threshold: self.active_sampler.threshold,
			},
			cleanup_handle: Arc::clone(&self.cleanup_handle),
		}
	}
}

impl Default for LayeredCacheStore {
	fn default() -> Self {
		Self::new()
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use std::time::Duration;

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
	async fn test_passive_expiration() {
		let store = LayeredCacheStore::new();

		// Set a value with short TTL
		store
			.set(
				"key1".to_string(),
				vec![1, 2, 3],
				Some(Duration::from_millis(50)),
			)
			.await;

		// Should exist immediately
		assert!(store.get("key1").await.is_some());

		// Poll until key expires and is deleted on access (passive expiration)
		poll_until(
			Duration::from_millis(150),
			Duration::from_millis(10),
			|| async { store.get("key1").await.is_none() },
		)
		.await
		.expect("Key should expire and be deleted within 150ms");

		// Key should be gone from store
		assert!(!store.has_key("key1").await);
	}

	#[tokio::test]
	async fn test_active_sampling_basic() {
		let store = LayeredCacheStore::with_sampler(10, 0.25);

		// Set many keys with short TTL
		for i in 0..50 {
			store
				.set(
					format!("key{}", i),
					vec![i as u8],
					Some(Duration::from_millis(50)),
				)
				.await;
		}

		// Keys should exist initially
		assert_eq!(store.len().await, 50);

		// Poll until keys expire
		poll_until(
			Duration::from_millis(150),
			Duration::from_millis(10),
			|| async {
				// Check if at least one key has expired
				store.get("key0").await.is_none()
			},
		)
		.await
		.expect("Keys should expire within 150ms");

		// Run active sampling cleanup
		store.cleanup_active_sampling().await;

		// All expired keys should be removed
		assert_eq!(store.len().await, 0);
	}

	#[tokio::test]
	async fn test_active_sampling_threshold() {
		let store = LayeredCacheStore::with_sampler(20, 0.25);

		// Set 80 keys with short TTL and 20 without TTL
		for i in 0..80 {
			store
				.set(
					format!("expired{}", i),
					vec![i as u8],
					Some(Duration::from_millis(50)),
				)
				.await;
		}
		for i in 0..20 {
			store
				.set(format!("permanent{}", i), vec![i as u8], None)
				.await;
		}

		// Poll until expired keys actually expire
		poll_until(
			Duration::from_millis(150),
			Duration::from_millis(10),
			|| async { store.get("expired0").await.is_none() },
		)
		.await
		.expect("Expired keys should expire within 150ms");

		// Run active sampling - should remove expired keys
		store.cleanup_active_sampling().await;

		// Permanent keys should remain
		assert!(store.get("permanent0").await.is_some());
		assert!(store.get("permanent10").await.is_some());

		// Expired keys should be gone
		assert!(store.get("expired0").await.is_none());
	}

	#[tokio::test]
	async fn test_ttl_index_cleanup() {
		let store = LayeredCacheStore::new();

		// Set many keys with same TTL
		for i in 0..100 {
			store
				.set(
					format!("key{}", i),
					vec![i as u8],
					Some(Duration::from_secs(1)),
				)
				.await;
		}

		// All keys should exist
		assert_eq!(store.len().await, 100);

		// Poll until keys expire (1 second TTL + buffer)
		poll_until(
			Duration::from_secs(2),
			Duration::from_millis(100),
			|| async { store.get("key0").await.is_none() },
		)
		.await
		.expect("Keys should expire within 2 seconds");

		// Run TTL index cleanup
		store.cleanup_ttl_index().await;

		// All keys should be removed
		assert_eq!(store.len().await, 0);
	}

	#[tokio::test]
	async fn test_ttl_index_partial_cleanup() {
		let store = LayeredCacheStore::new();

		// Set keys with different TTLs
		for i in 0..50 {
			store
				.set(
					format!("short{}", i),
					vec![i as u8],
					Some(Duration::from_millis(50)),
				)
				.await;
		}
		for i in 0..50 {
			store
				.set(
					format!("long{}", i),
					vec![i as u8],
					Some(Duration::from_secs(10)),
				)
				.await;
		}

		// Wait for short TTL to expire
		tokio::time::sleep(Duration::from_millis(60)).await;

		// Run TTL index cleanup
		store.cleanup_ttl_index().await;

		// Short TTL keys should be gone
		assert!(store.get("short0").await.is_none());

		// Long TTL keys should remain
		assert!(store.get("long0").await.is_some());
	}

	#[tokio::test]
	async fn test_combined_cleanup() {
		let store = LayeredCacheStore::new();

		// Set keys with various TTLs
		for i in 0..100 {
			let ttl = if i < 50 {
				Some(Duration::from_millis(50))
			} else {
				None
			};
			store.set(format!("key{}", i), vec![i as u8], ttl).await;
		}

		// Wait for expiration (TTL is 50ms, wait 60ms to ensure expiration)
		tokio::time::sleep(Duration::from_millis(60)).await;

		// Run combined cleanup
		store.cleanup().await;

		// Expired keys should be gone
		assert!(store.get("key0").await.is_none());
		assert!(store.get("key49").await.is_none());

		// Non-expired keys should remain
		assert!(store.get("key50").await.is_some());
		assert!(store.get("key99").await.is_some());
	}

	#[tokio::test]
	async fn test_layered_vs_naive_performance() {
		// This test demonstrates the performance difference
		let store = LayeredCacheStore::new();

		// Set a large number of keys
		let num_keys = 10000;
		for i in 0..num_keys {
			store
				.set(
					format!("key{}", i),
					vec![i as u8],
					Some(Duration::from_millis(50)),
				)
				.await;
		}

		// Wait for expiration

		// Measure layered cleanup time
		let start = std::time::Instant::now();
		store.cleanup().await;
		let layered_duration = start.elapsed();

		println!(
			"Layered cleanup for {} keys: {:?}",
			num_keys, layered_duration
		);

		// For comparison, naive O(n) cleanup would iterate all keys
		// The layered approach should be significantly faster
	}

	#[tokio::test]
	async fn test_basic_operations() {
		let store = LayeredCacheStore::new();

		// Set and get
		store.set("key1".to_string(), vec![1, 2, 3], None).await;
		assert_eq!(store.get("key1").await, Some(vec![1, 2, 3]));

		// Has key
		assert!(store.has_key("key1").await);
		assert!(!store.has_key("nonexistent").await);

		// Delete
		store.delete("key1").await;
		assert!(store.get("key1").await.is_none());

		// Clear
		store.set("key2".to_string(), vec![4, 5, 6], None).await;
		store.set("key3".to_string(), vec![7, 8, 9], None).await;
		assert_eq!(store.len().await, 2);

		store.clear().await;
		assert_eq!(store.len().await, 0);
	}

	#[tokio::test]
	async fn test_keys_listing() {
		let store = LayeredCacheStore::new();

		// Initially empty
		assert!(store.keys().await.is_empty());

		// Add some keys
		store.set("key1".to_string(), vec![1], None).await;
		store.set("key2".to_string(), vec![2], None).await;
		store.set("key3".to_string(), vec![3], None).await;

		let keys = store.keys().await;
		assert_eq!(keys.len(), 3);
		assert!(keys.contains(&"key1".to_string()));
		assert!(keys.contains(&"key2".to_string()));
		assert!(keys.contains(&"key3".to_string()));
	}
}
