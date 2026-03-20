//! Advanced caching strategies for dependency injection
//!
//! This module provides sophisticated caching mechanisms beyond the basic request/singleton
//! scopes, including LRU caching, TTL-based expiration, and size-limited caches.
//!
//! ## Example
//!
//! ```rust
//! use reinhardt_di::advanced_cache::{LruCache, TtlCache};
//! use std::time::Duration;
//!
//! // LRU cache with maximum capacity
//! let mut lru = LruCache::new(100);
//! lru.insert("key".to_string(), "value".to_string());
//!
//! // TTL cache with expiration
//! let mut ttl = TtlCache::new(Duration::from_secs(60));
//! ttl.insert("key".to_string(), "value".to_string());
//! ```

#[cfg(feature = "dev-tools")]
use indexmap::IndexMap;
#[cfg(feature = "dev-tools")]
use std::collections::HashMap;
#[cfg(feature = "dev-tools")]
use std::hash::Hash;
#[cfg(feature = "dev-tools")]
use std::time::{Duration, Instant};

/// LRU (Least Recently Used) cache implementation
///
/// Evicts the least recently used items when capacity is exceeded.
/// Uses IndexMap for O(1) access, insertion, and removal operations.
#[cfg(feature = "dev-tools")]
#[derive(Debug)]
pub struct LruCache<K, V>
where
	K: Eq + Hash + Clone,
{
	capacity: usize,
	map: IndexMap<K, V>,
}

#[cfg(feature = "dev-tools")]
impl<K, V> LruCache<K, V>
where
	K: Eq + Hash + Clone,
{
	/// Create a new LRU cache with the given capacity
	///
	/// # Panics
	///
	/// Panics if `capacity` is 0, since a zero-capacity cache cannot store any items.
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_di::advanced_cache::LruCache;
	///
	/// let cache: LruCache<String, i32> = LruCache::new(100);
	/// ```
	pub fn new(capacity: usize) -> Self {
		assert!(capacity > 0, "LruCache capacity must be greater than 0");
		Self {
			capacity,
			map: IndexMap::new(),
		}
	}

	/// Insert a key-value pair into the cache
	///
	/// If the cache is at capacity, the least recently used item is evicted.
	/// Uses IndexMap for O(1) operations instead of O(n) with VecDeque::retain.
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_di::advanced_cache::LruCache;
	///
	/// let mut cache = LruCache::new(2);
	/// cache.insert("a".to_string(), 1);
	/// cache.insert("b".to_string(), 2);
	/// cache.insert("c".to_string(), 3);
	///
	/// assert!(cache.get(&"a".to_string()).is_none());
	/// assert_eq!(cache.get(&"b".to_string()), Some(&2));
	/// assert_eq!(cache.get(&"c".to_string()), Some(&3));
	/// ```
	pub fn insert(&mut self, key: K, value: V) {
		if self.map.contains_key(&key) {
			// Move to end (most recently used): O(1)
			self.map.shift_remove(&key);
		} else if self.map.len() >= self.capacity {
			// Remove least recently used (first entry): O(1)
			self.map.shift_remove_index(0);
		}

		// Insert at end (most recently used): O(1)
		self.map.insert(key, value);
	}

	/// Get a value from the cache
	///
	/// Updates the item's position to mark it as recently used.
	/// Uses IndexMap for O(1) operations instead of O(n) with VecDeque::retain.
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_di::advanced_cache::LruCache;
	///
	/// let mut cache = LruCache::new(10);
	/// cache.insert("key".to_string(), 42);
	///
	/// assert_eq!(cache.get(&"key".to_string()), Some(&42));
	/// assert_eq!(cache.get(&"missing".to_string()), None);
	/// ```
	pub fn get(&mut self, key: &K) -> Option<&V> {
		if self.map.contains_key(key) {
			// Move to end (most recently used): O(1)
			let value = self.map.shift_remove(key)?;
			self.map.insert(key.clone(), value);
			self.map.get(key)
		} else {
			None
		}
	}

	/// Remove a value from the cache
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_di::advanced_cache::LruCache;
	///
	/// let mut cache = LruCache::new(10);
	/// cache.insert("key".to_string(), 42);
	/// cache.remove(&"key".to_string());
	///
	/// assert!(cache.get(&"key".to_string()).is_none());
	/// ```
	pub fn remove(&mut self, key: &K) -> Option<V> {
		self.map.shift_remove(key)
	}

	/// Clear all items from the cache
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_di::advanced_cache::LruCache;
	///
	/// let mut cache = LruCache::new(10);
	/// cache.insert("a".to_string(), 1);
	/// cache.insert("b".to_string(), 2);
	/// cache.clear();
	///
	/// assert_eq!(cache.len(), 0);
	/// ```
	pub fn clear(&mut self) {
		self.map.clear();
	}

	/// Get the number of items in the cache
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_di::advanced_cache::LruCache;
	///
	/// let mut cache = LruCache::new(10);
	/// cache.insert("a".to_string(), 1);
	/// cache.insert("b".to_string(), 2);
	///
	/// assert_eq!(cache.len(), 2);
	/// ```
	pub fn len(&self) -> usize {
		self.map.len()
	}

	/// Check if the cache is empty
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_di::advanced_cache::LruCache;
	///
	/// let cache: LruCache<String, i32> = LruCache::new(10);
	/// assert!(cache.is_empty());
	/// ```
	pub fn is_empty(&self) -> bool {
		self.map.is_empty()
	}
}

/// Entry in a TTL cache with expiration time
#[cfg(feature = "dev-tools")]
#[derive(Debug, Clone)]
struct TtlEntry<V> {
	value: V,
	expires_at: Instant,
}

/// TTL (Time To Live) cache implementation
///
/// Automatically expires entries after a specified duration.
#[cfg(feature = "dev-tools")]
#[derive(Debug)]
pub struct TtlCache<K, V>
where
	K: Eq + Hash + Clone,
{
	ttl: Duration,
	map: HashMap<K, TtlEntry<V>>,
}

#[cfg(feature = "dev-tools")]
impl<K, V> TtlCache<K, V>
where
	K: Eq + Hash + Clone,
{
	/// Create a new TTL cache with the given time-to-live
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_di::advanced_cache::TtlCache;
	/// use std::time::Duration;
	///
	/// let cache: TtlCache<String, i32> = TtlCache::new(Duration::from_secs(60));
	/// ```
	pub fn new(ttl: Duration) -> Self {
		Self {
			ttl,
			map: HashMap::new(),
		}
	}

	/// Insert a key-value pair with TTL expiration
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_di::advanced_cache::TtlCache;
	/// use std::time::Duration;
	///
	/// let mut cache = TtlCache::new(Duration::from_secs(60));
	/// cache.insert("key".to_string(), 42);
	/// ```
	pub fn insert(&mut self, key: K, value: V) {
		let entry = TtlEntry {
			value,
			expires_at: Instant::now() + self.ttl,
		};
		self.map.insert(key, entry);
	}

	/// Get a value from the cache if not expired
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_di::advanced_cache::TtlCache;
	/// use std::time::Duration;
	///
	/// let mut cache = TtlCache::new(Duration::from_secs(60));
	/// cache.insert("key".to_string(), 42);
	///
	/// assert_eq!(cache.get(&"key".to_string()), Some(&42));
	/// ```
	pub fn get(&mut self, key: &K) -> Option<&V> {
		self.cleanup_expired();
		self.map.get(key).map(|entry| &entry.value)
	}

	/// Remove a value from the cache
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_di::advanced_cache::TtlCache;
	/// use std::time::Duration;
	///
	/// let mut cache = TtlCache::new(Duration::from_secs(60));
	/// cache.insert("key".to_string(), 42);
	/// cache.remove(&"key".to_string());
	///
	/// assert!(cache.get(&"key".to_string()).is_none());
	/// ```
	pub fn remove(&mut self, key: &K) -> Option<V> {
		self.map.remove(key).map(|entry| entry.value)
	}

	/// Remove all expired entries
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_di::advanced_cache::TtlCache;
	/// use std::time::Duration;
	///
	/// let mut cache = TtlCache::new(Duration::from_millis(1));
	/// cache.insert("key".to_string(), 42);
	///
	/// std::thread::sleep(Duration::from_millis(10));
	/// cache.cleanup_expired();
	///
	/// assert!(cache.get(&"key".to_string()).is_none());
	/// ```
	pub fn cleanup_expired(&mut self) {
		let now = Instant::now();
		self.map.retain(|_, entry| entry.expires_at > now);
	}

	/// Clear all items from the cache
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_di::advanced_cache::TtlCache;
	/// use std::time::Duration;
	///
	/// let mut cache = TtlCache::new(Duration::from_secs(60));
	/// cache.insert("key".to_string(), 42);
	/// cache.clear();
	///
	/// assert_eq!(cache.len(), 0);
	/// ```
	pub fn clear(&mut self) {
		self.map.clear();
	}

	/// Get the number of items in the cache (including expired)
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_di::advanced_cache::TtlCache;
	/// use std::time::Duration;
	///
	/// let mut cache = TtlCache::new(Duration::from_secs(60));
	/// cache.insert("a".to_string(), 1);
	/// cache.insert("b".to_string(), 2);
	///
	/// assert_eq!(cache.len(), 2);
	/// ```
	pub fn len(&self) -> usize {
		self.map.len()
	}

	/// Check if the cache is empty
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_di::advanced_cache::TtlCache;
	/// use std::time::Duration;
	///
	/// let cache: TtlCache<String, i32> = TtlCache::new(Duration::from_secs(60));
	/// assert!(cache.is_empty());
	/// ```
	pub fn is_empty(&self) -> bool {
		self.map.is_empty()
	}
}

/// Cache statistics for monitoring
#[cfg(feature = "dev-tools")]
#[derive(Debug, Clone, Default)]
pub struct CacheStats {
	/// Total number of cache hits
	pub hits: u64,
	/// Total number of cache misses
	pub misses: u64,
	/// Total number of evictions
	pub evictions: u64,
	/// Total number of insertions
	pub insertions: u64,
}

#[cfg(feature = "dev-tools")]
impl CacheStats {
	/// Create new cache statistics
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_di::advanced_cache::CacheStats;
	///
	/// let stats = CacheStats::new();
	/// assert_eq!(stats.hits, 0);
	/// assert_eq!(stats.misses, 0);
	/// ```
	pub fn new() -> Self {
		Self::default()
	}

	/// Calculate the cache hit rate
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_di::advanced_cache::CacheStats;
	///
	/// let mut stats = CacheStats::new();
	/// stats.hits = 80;
	/// stats.misses = 20;
	///
	/// assert_eq!(stats.hit_rate(), 0.8);
	/// ```
	pub fn hit_rate(&self) -> f64 {
		let total = self.hits + self.misses;
		if total == 0 {
			0.0
		} else {
			self.hits as f64 / total as f64
		}
	}

	/// Record a cache hit
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_di::advanced_cache::CacheStats;
	///
	/// let mut stats = CacheStats::new();
	/// stats.record_hit();
	/// assert_eq!(stats.hits, 1);
	/// ```
	pub fn record_hit(&mut self) {
		self.hits += 1;
	}

	/// Record a cache miss
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_di::advanced_cache::CacheStats;
	///
	/// let mut stats = CacheStats::new();
	/// stats.record_miss();
	/// assert_eq!(stats.misses, 1);
	/// ```
	pub fn record_miss(&mut self) {
		self.misses += 1;
	}

	/// Record a cache eviction
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_di::advanced_cache::CacheStats;
	///
	/// let mut stats = CacheStats::new();
	/// stats.record_eviction();
	/// assert_eq!(stats.evictions, 1);
	/// ```
	pub fn record_eviction(&mut self) {
		self.evictions += 1;
	}

	/// Record a cache insertion
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_di::advanced_cache::CacheStats;
	///
	/// let mut stats = CacheStats::new();
	/// stats.record_insertion();
	/// assert_eq!(stats.insertions, 1);
	/// ```
	pub fn record_insertion(&mut self) {
		self.insertions += 1;
	}
}
