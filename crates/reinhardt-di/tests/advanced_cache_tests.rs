//! Tests for advanced caching strategies (dev-tools feature)

#![cfg(feature = "dev-tools")]

use reinhardt_di::advanced_cache::{CacheStats, LruCache, TtlCache};
use rstest::*;
use std::time::Duration;

/// Test that LruCache::new panics when capacity is zero
#[rstest]
#[should_panic(expected = "LruCache capacity must be greater than 0")]
fn lru_cache_new_with_zero_capacity_panics() {
	// Act
	let _cache: LruCache<String, i32> = LruCache::new(0);
}

/// Test LRU cache eviction when capacity is exceeded
#[rstest]
#[tokio::test]
async fn test_lru_cache_eviction() {
	let mut cache = LruCache::new(2);

	// Insert items up to capacity
	cache.insert("key1".to_string(), "value1".to_string());
	cache.insert("key2".to_string(), "value2".to_string());

	assert_eq!(cache.len(), 2);
	assert_eq!(cache.get(&"key1".to_string()), Some(&"value1".to_string()));
	assert_eq!(cache.get(&"key2".to_string()), Some(&"value2".to_string()));

	// Insert third item - should evict least recently used (key1)
	cache.insert("key3".to_string(), "value3".to_string());

	assert_eq!(cache.len(), 2);
	assert_eq!(cache.get(&"key1".to_string()), None); // evicted
	assert_eq!(cache.get(&"key2".to_string()), Some(&"value2".to_string()));
	assert_eq!(cache.get(&"key3".to_string()), Some(&"value3".to_string()));
}

/// Test LRU cache capacity limit enforcement
#[rstest]
#[tokio::test]
async fn test_lru_cache_capacity_limit() {
	let mut cache = LruCache::new(3);

	// Fill cache to capacity
	cache.insert("a".to_string(), 1);
	cache.insert("b".to_string(), 2);
	cache.insert("c".to_string(), 3);

	assert_eq!(cache.len(), 3);

	// Access "a" to make it recently used
	let _ = cache.get(&"a".to_string());

	// Insert new item - should evict "b" (least recently used)
	cache.insert("d".to_string(), 4);

	assert_eq!(cache.len(), 3);
	assert_eq!(cache.get(&"a".to_string()), Some(&1));
	assert_eq!(cache.get(&"b".to_string()), None); // evicted
	assert_eq!(cache.get(&"c".to_string()), Some(&3));
	assert_eq!(cache.get(&"d".to_string()), Some(&4));

	// Clear cache
	cache.clear();
	assert!(cache.is_empty());
}

/// Test TTL cache expiration behavior
#[rstest]
#[tokio::test]
async fn test_ttl_cache_expiration() {
	let mut cache = TtlCache::new(Duration::from_millis(50));

	// Insert item
	cache.insert("key".to_string(), "value".to_string());

	// Item should be available immediately
	assert_eq!(cache.get(&"key".to_string()), Some(&"value".to_string()));

	// Wait for expiration
	tokio::time::sleep(Duration::from_millis(60)).await;

	// Item should be expired
	assert_eq!(cache.get(&"key".to_string()), None);
	assert_eq!(cache.len(), 0); // cleanup_expired called by get()
}

/// Test TTL cache refresh functionality
#[rstest]
#[tokio::test]
async fn test_ttl_cache_refresh() {
	let mut cache = TtlCache::new(Duration::from_millis(100));

	// Insert item
	cache.insert("key".to_string(), "value1".to_string());
	assert_eq!(cache.get(&"key".to_string()), Some(&"value1".to_string()));

	// Wait a bit but not enough to expire
	tokio::time::sleep(Duration::from_millis(50)).await;

	// Update (refresh) the item with new value
	cache.insert("key".to_string(), "value2".to_string());

	// Wait for original TTL to pass
	tokio::time::sleep(Duration::from_millis(60)).await;

	// Item should still be available (refreshed TTL)
	assert_eq!(cache.get(&"key".to_string()), Some(&"value2".to_string()));

	// Cleanup test
	cache.clear();
	assert!(cache.is_empty());
}

/// Test cache hit rate tracking with CacheStats
#[rstest]
#[tokio::test]
async fn test_cache_hit_rate_tracking() {
	let mut stats = CacheStats::new();

	assert_eq!(stats.hits, 0);
	assert_eq!(stats.misses, 0);
	assert_eq!(stats.hit_rate(), 0.0);

	// Record 8 hits and 2 misses (80% hit rate)
	for _ in 0..8 {
		stats.record_hit();
	}
	for _ in 0..2 {
		stats.record_miss();
	}

	assert_eq!(stats.hits, 8);
	assert_eq!(stats.misses, 2);
	assert_eq!(stats.hit_rate(), 0.8);

	// Test eviction and insertion tracking
	stats.record_eviction();
	stats.record_insertion();

	assert_eq!(stats.evictions, 1);
	assert_eq!(stats.insertions, 1);

	// Test with LRU cache simulation
	let mut lru_cache = LruCache::new(2);
	let mut lru_stats = CacheStats::new();

	// First access (miss)
	if lru_cache.get(&"key1".to_string()).is_none() {
		lru_stats.record_miss();
		lru_cache.insert("key1".to_string(), "value1".to_string());
		lru_stats.record_insertion();
	}

	// Second access (hit)
	if lru_cache.get(&"key1".to_string()).is_some() {
		lru_stats.record_hit();
	}

	assert_eq!(lru_stats.hits, 1);
	assert_eq!(lru_stats.misses, 1);
	assert_eq!(lru_stats.hit_rate(), 0.5);
}
