//! Hybrid cache - Multi-tier caching (memory + distributed)
//!
//! Provides a two-level caching strategy combining fast local memory cache (L1)
//! with distributed cache (L2) for better performance and scalability.
//!
//! # Features
//!
//! - **L1 cache**: Fast in-memory cache for frequently accessed data
//! - **L2 cache**: Distributed cache (Redis/Memcached) for shared data
//! - **Automatic promotion**: L2 hits are promoted to L1 for faster subsequent access
//! - **Write-through**: Writes update both L1 and L2 simultaneously
//!
//! # Examples
//!
//! ```rust,ignore
//! use reinhardt_utils::cache::{Cache, HybridCache, InMemoryCache, RedisCache};
//! use std::time::Duration;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Create L1 (memory) and L2 (Redis) caches
//! let l1_cache = InMemoryCache::new();
//! let l2_cache = RedisCache::new("redis://localhost:6379").await?;
//!
//! // Create hybrid cache
//! let cache = HybridCache::new(l1_cache, l2_cache);
//!
//! // Set a value (writes to both L1 and L2)
//! cache.set("user:123", &"John Doe", Some(Duration::from_secs(300))).await?;
//!
//! // First get hits L1 (fast)
//! let name: Option<String> = cache.get("user:123").await?;
//! assert_eq!(name, Some("John Doe".to_string()));
//!
//! // Even if L1 is cleared, L2 still has the data
//! cache.l1().clear().await?;
//! let name: Option<String> = cache.get("user:123").await?;
//! // Value is retrieved from L2 and promoted to L1
//! assert_eq!(name, Some("John Doe".to_string()));
//! # Ok(())
//! # }
//! ```

use super::Cache;
use async_trait::async_trait;
use reinhardt_core::exception::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

/// Hybrid cache with two-level caching strategy
///
/// Combines a fast local cache (L1) with a distributed cache (L2)
/// for optimal performance and scalability.
///
/// # Type Parameters
///
/// - `L1`: Fast local cache (typically `InMemoryCache`)
/// - `L2`: Distributed cache (typically `RedisCache` or `MemcachedCache`)
#[derive(Clone)]
pub struct HybridCache<L1, L2>
where
	L1: Cache + Clone,
	L2: Cache + Clone,
{
	l1: Arc<L1>,
	l2: Arc<L2>,
}

impl<L1, L2> HybridCache<L1, L2>
where
	L1: Cache + Clone,
	L2: Cache + Clone,
{
	/// Create a new hybrid cache with the given L1 and L2 caches
	///
	/// # Examples
	///
	/// ```ignore
	/// use reinhardt_utils::cache::{HybridCache, InMemoryCache, RedisCache};
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let l1 = InMemoryCache::new();
	/// let l2 = RedisCache::new("redis://localhost:6379").await?;
	/// let cache = HybridCache::new(l1, l2);
	/// # Ok(())
	/// # }
	/// ```
	pub fn new(l1: L1, l2: L2) -> Self {
		Self {
			l1: Arc::new(l1),
			l2: Arc::new(l2),
		}
	}

	/// Get a reference to the L1 cache
	///
	/// # Examples
	///
	/// ```ignore
	/// use reinhardt_utils::cache::{Cache, HybridCache, InMemoryCache, RedisCache};
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let l1 = InMemoryCache::new();
	/// let l2 = RedisCache::new("redis://localhost:6379").await?;
	/// let cache = HybridCache::new(l1, l2);
	///
	/// // Clear only L1 cache
	/// cache.l1().clear().await?;
	/// # Ok(())
	/// # }
	/// ```
	pub fn l1(&self) -> &L1 {
		&self.l1
	}

	/// Get a reference to the L2 cache
	///
	/// # Examples
	///
	/// ```ignore
	/// use reinhardt_utils::cache::{Cache, HybridCache, InMemoryCache, RedisCache};
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let l1 = InMemoryCache::new();
	/// let l2 = RedisCache::new("redis://localhost:6379").await?;
	/// let cache = HybridCache::new(l1, l2);
	///
	/// // Clear only L2 cache
	/// cache.l2().clear().await?;
	/// # Ok(())
	/// # }
	/// ```
	pub fn l2(&self) -> &L2 {
		&self.l2
	}

	/// Promote a value from L2 to L1
	// Reserved for future L2-to-L1 cache promotion logic
	#[allow(dead_code)]
	async fn promote<T>(&self, key: &str, value: &T, ttl: Option<Duration>) -> Result<()>
	where
		T: Serialize + Send + Sync,
	{
		self.l1.set(key, value, ttl).await
	}
}

#[async_trait]
impl<L1, L2> Cache for HybridCache<L1, L2>
where
	L1: Cache + Clone + 'static,
	L2: Cache + Clone + 'static,
{
	async fn get<T>(&self, key: &str) -> Result<Option<T>>
	where
		T: for<'de> Deserialize<'de> + Serialize + Send + Sync,
	{
		// Try L1 first (fast path)
		if let Some(value) = self.l1.get::<T>(key).await? {
			return Ok(Some(value));
		}

		// Try L2 (slow path)
		if let Some(value) = self.l2.get::<T>(key).await? {
			// Promote to L1 for faster subsequent access
			// Use None for TTL to let L1 handle its own expiration policy
			self.l1.set(key, &value, None).await?;
			return Ok(Some(value));
		}

		Ok(None)
	}

	async fn set<T>(&self, key: &str, value: &T, ttl: Option<Duration>) -> Result<()>
	where
		T: Serialize + Send + Sync,
	{
		// Write-through: update both L1 and L2
		self.l1.set(key, value, ttl).await?;
		self.l2.set(key, value, ttl).await?;
		Ok(())
	}

	async fn delete(&self, key: &str) -> Result<()> {
		// Delete from both caches
		self.l1.delete(key).await?;
		self.l2.delete(key).await?;
		Ok(())
	}

	async fn has_key(&self, key: &str) -> Result<bool> {
		// Check L1 first (fast path)
		if self.l1.has_key(key).await? {
			return Ok(true);
		}

		// Check L2 (slow path)
		self.l2.has_key(key).await
	}

	async fn clear(&self) -> Result<()> {
		// Clear both caches
		self.l1.clear().await?;
		self.l2.clear().await?;
		Ok(())
	}

	async fn get_many<T>(&self, keys: &[&str]) -> Result<HashMap<String, T>>
	where
		T: for<'de> Deserialize<'de> + Serialize + Send + Sync,
	{
		let mut results = HashMap::new();

		// Try L1 first
		let l1_results = self.l1.get_many::<T>(keys).await?;
		results.extend(l1_results);

		// Find keys not in L1
		let missing_keys: Vec<&str> = keys
			.iter()
			.filter(|k| !results.contains_key(**k))
			.copied()
			.collect();

		if !missing_keys.is_empty() {
			// Try L2 for missing keys
			let l2_results = self.l2.get_many::<T>(&missing_keys).await?;

			// Promote L2 results to L1 for faster subsequent access
			for (key, value) in &l2_results {
				self.l1.set(key, value, None).await?;
			}

			results.extend(l2_results);
		}

		Ok(results)
	}

	async fn set_many<T>(&self, values: HashMap<String, T>, ttl: Option<Duration>) -> Result<()>
	where
		T: Serialize + Send + Sync,
	{
		// Write-through: update both L1 and L2
		for (key, value) in values.iter() {
			self.l1.set(key, value, ttl).await?;
			self.l2.set(key, value, ttl).await?;
		}
		Ok(())
	}

	async fn delete_many(&self, keys: &[&str]) -> Result<()> {
		// Delete from both caches
		self.l1.delete_many(keys).await?;
		self.l2.delete_many(keys).await?;
		Ok(())
	}

	async fn incr(&self, key: &str, delta: i64) -> Result<i64> {
		// Increment in L2 (source of truth)
		let result = self.l2.incr(key, delta).await?;

		// Update L1 with new value
		self.l1.set(key, &result, None).await?;

		Ok(result)
	}

	async fn decr(&self, key: &str, delta: i64) -> Result<i64> {
		// Decrement in L2 (source of truth)
		let result = self.l2.decr(key, delta).await?;

		// Update L1 with new value
		self.l1.set(key, &result, None).await?;

		Ok(result)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::cache::InMemoryCache;

	#[tokio::test]
	async fn test_hybrid_cache_l1_hit() {
		let l1 = InMemoryCache::new();
		let l2 = InMemoryCache::new();
		let cache = HybridCache::new(l1, l2);

		// Set in both caches
		cache.set("key1", &"value1", None).await.unwrap();

		// Get should hit L1 (fast path)
		let value: Option<String> = cache.get("key1").await.unwrap();
		assert_eq!(value, Some("value1".to_string()));
	}

	#[tokio::test]
	async fn test_hybrid_cache_l2_hit_promotion() {
		let l1 = InMemoryCache::new();
		let l2 = InMemoryCache::new();
		let cache = HybridCache::new(l1.clone(), l2.clone());

		// Set only in L2
		l2.set("key1", &"value1", None).await.unwrap();

		// Get should hit L2 and promote to L1
		let value: Option<String> = cache.get("key1").await.unwrap();
		assert_eq!(value, Some("value1".to_string()));

		// Verify promotion to L1
		let l1_value: Option<String> = l1.get("key1").await.unwrap();
		assert_eq!(l1_value, Some("value1".to_string()));
	}

	#[tokio::test]
	async fn test_hybrid_cache_miss() {
		let l1 = InMemoryCache::new();
		let l2 = InMemoryCache::new();
		let cache = HybridCache::new(l1, l2);

		// Get should miss both caches
		let value: Option<String> = cache.get("nonexistent").await.unwrap();
		assert_eq!(value, None);
	}

	#[tokio::test]
	async fn test_hybrid_cache_write_through() {
		let l1 = InMemoryCache::new();
		let l2 = InMemoryCache::new();
		let cache = HybridCache::new(l1.clone(), l2.clone());

		// Set should write to both caches
		cache.set("key1", &"value1", None).await.unwrap();

		// Verify both caches have the value
		let l1_value: Option<String> = l1.get("key1").await.unwrap();
		let l2_value: Option<String> = l2.get("key1").await.unwrap();
		assert_eq!(l1_value, Some("value1".to_string()));
		assert_eq!(l2_value, Some("value1".to_string()));
	}

	#[tokio::test]
	async fn test_hybrid_cache_delete_both() {
		let l1 = InMemoryCache::new();
		let l2 = InMemoryCache::new();
		let cache = HybridCache::new(l1.clone(), l2.clone());

		// Set in both caches
		cache.set("key1", &"value1", None).await.unwrap();

		// Delete should remove from both caches
		cache.delete("key1").await.unwrap();

		// Verify both caches are empty
		let l1_value: Option<String> = l1.get("key1").await.unwrap();
		let l2_value: Option<String> = l2.get("key1").await.unwrap();
		assert_eq!(l1_value, None);
		assert_eq!(l2_value, None);
	}

	#[tokio::test]
	async fn test_hybrid_cache_has_key() {
		let l1 = InMemoryCache::new();
		let l2 = InMemoryCache::new();
		let cache = HybridCache::new(l1.clone(), l2.clone());

		// Set only in L2
		l2.set("key1", &"value1", None).await.unwrap();

		// Has_key should return true
		assert!(cache.has_key("key1").await.unwrap());
		assert!(!cache.has_key("nonexistent").await.unwrap());
	}

	#[tokio::test]
	async fn test_hybrid_cache_clear_both() {
		let l1 = InMemoryCache::new();
		let l2 = InMemoryCache::new();
		let cache = HybridCache::new(l1.clone(), l2.clone());

		// Set in both caches
		cache.set("key1", &"value1", None).await.unwrap();
		cache.set("key2", &"value2", None).await.unwrap();

		// Clear should remove all from both caches
		cache.clear().await.unwrap();

		// Verify both caches are empty
		let l1_value: Option<String> = l1.get("key1").await.unwrap();
		let l2_value: Option<String> = l2.get("key1").await.unwrap();
		assert_eq!(l1_value, None);
		assert_eq!(l2_value, None);
	}

	#[tokio::test]
	async fn test_hybrid_cache_get_many_l1_hit() {
		let l1 = InMemoryCache::new();
		let l2 = InMemoryCache::new();
		let cache = HybridCache::new(l1, l2);

		// Set in both caches
		cache.set("key1", &"value1", None).await.unwrap();
		cache.set("key2", &"value2", None).await.unwrap();

		// Get many should hit L1
		let results: HashMap<String, String> =
			cache.get_many(&["key1", "key2", "key3"]).await.unwrap();

		assert_eq!(results.len(), 2);
		assert_eq!(results.get("key1"), Some(&"value1".to_string()));
		assert_eq!(results.get("key2"), Some(&"value2".to_string()));
	}

	#[tokio::test]
	async fn test_hybrid_cache_get_many_l2_promotion() {
		let l1 = InMemoryCache::new();
		let l2 = InMemoryCache::new();
		let cache = HybridCache::new(l1.clone(), l2.clone());

		// Set in L1 and L2
		l1.set("key1", &"value1", None).await.unwrap();
		l2.set("key2", &"value2", None).await.unwrap();

		// Get many should hit L1 for key1 and L2 for key2
		let results: HashMap<String, String> = cache.get_many(&["key1", "key2"]).await.unwrap();

		assert_eq!(results.len(), 2);
		assert_eq!(results.get("key1"), Some(&"value1".to_string()));
		assert_eq!(results.get("key2"), Some(&"value2".to_string()));

		// Verify key2 was promoted to L1
		let l1_value: Option<String> = l1.get("key2").await.unwrap();
		assert_eq!(l1_value, Some("value2".to_string()));
	}

	#[tokio::test]
	async fn test_hybrid_cache_set_many() {
		let l1 = InMemoryCache::new();
		let l2 = InMemoryCache::new();
		let cache = HybridCache::new(l1.clone(), l2.clone());

		// Set many
		let mut values = HashMap::new();
		values.insert("key1".to_string(), "value1".to_string());
		values.insert("key2".to_string(), "value2".to_string());
		cache.set_many(values, None).await.unwrap();

		// Verify both caches have the values
		let l1_value: Option<String> = l1.get("key1").await.unwrap();
		let l2_value: Option<String> = l2.get("key1").await.unwrap();
		assert_eq!(l1_value, Some("value1".to_string()));
		assert_eq!(l2_value, Some("value1".to_string()));
	}

	#[tokio::test]
	async fn test_hybrid_cache_delete_many() {
		let l1 = InMemoryCache::new();
		let l2 = InMemoryCache::new();
		let cache = HybridCache::new(l1.clone(), l2.clone());

		// Set in both caches
		cache.set("key1", &"value1", None).await.unwrap();
		cache.set("key2", &"value2", None).await.unwrap();

		// Delete many
		cache.delete_many(&["key1", "key2"]).await.unwrap();

		// Verify both caches are empty
		let l1_value: Option<String> = l1.get("key1").await.unwrap();
		let l2_value: Option<String> = l2.get("key1").await.unwrap();
		assert_eq!(l1_value, None);
		assert_eq!(l2_value, None);
	}

	#[tokio::test]
	async fn test_hybrid_cache_incr() {
		let l1 = InMemoryCache::new();
		let l2 = InMemoryCache::new();
		let cache = HybridCache::new(l1.clone(), l2.clone());

		// Increment
		let value = cache.incr("counter", 5).await.unwrap();
		assert_eq!(value, 5);

		// Verify both caches have the updated value
		let l1_value: Option<i64> = l1.get("counter").await.unwrap();
		let l2_value: Option<i64> = l2.get("counter").await.unwrap();
		assert_eq!(l1_value, Some(5));
		assert_eq!(l2_value, Some(5));
	}

	#[tokio::test]
	async fn test_hybrid_cache_decr() {
		let l1 = InMemoryCache::new();
		let l2 = InMemoryCache::new();
		let cache = HybridCache::new(l1.clone(), l2.clone());

		// Set initial value
		cache.set("counter", &10i64, None).await.unwrap();

		// Decrement
		let value = cache.decr("counter", 3).await.unwrap();
		assert_eq!(value, 7);

		// Verify both caches have the updated value
		let l1_value: Option<i64> = l1.get("counter").await.unwrap();
		let l2_value: Option<i64> = l2.get("counter").await.unwrap();
		assert_eq!(l1_value, Some(7));
		assert_eq!(l2_value, Some(7));
	}

	#[tokio::test]
	async fn test_hybrid_cache_l1_l2_access() {
		let l1 = InMemoryCache::new();
		let l2 = InMemoryCache::new();
		let cache = HybridCache::new(l1.clone(), l2.clone());

		// Set in both caches
		cache.set("key1", &"value1", None).await.unwrap();

		// Clear L1 directly
		cache.l1().clear().await.unwrap();

		// Get should still work (hits L2 and promotes to L1)
		let value: Option<String> = cache.get("key1").await.unwrap();
		assert_eq!(value, Some("value1".to_string()));

		// Verify promotion to L1
		let l1_value: Option<String> = cache.l1().get("key1").await.unwrap();
		assert_eq!(l1_value, Some("value1".to_string()));
	}
}
