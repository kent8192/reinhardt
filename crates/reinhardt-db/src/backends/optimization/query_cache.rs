//! Query caching for improved performance
//!
//! Provides caching for prepared statements and query results:
//! - LRU cache for prepared statements
//! - Query result caching with TTL
//! - Cache invalidation strategies

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

/// Query cache configuration
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct QueryCacheConfig {
	/// Maximum number of cached queries
	pub max_size: usize,

	/// Time-to-live for cached entries
	pub ttl: Duration,

	/// Enable query plan caching
	pub cache_plans: bool,
}

impl Default for QueryCacheConfig {
	fn default() -> Self {
		Self {
			max_size: 1000,
			ttl: Duration::from_secs(5 * 60), // 5 minutes
			cache_plans: true,
		}
	}
}

/// A cached query with metadata
#[derive(Debug, Clone)]
pub struct CachedQuery {
	/// SQL query string
	pub sql: String,

	/// Query parameters (hashed)
	pub params_hash: u64,

	/// Cached result (if any)
	pub result: Option<Vec<u8>>,

	/// Timestamp when cached
	pub cached_at: Instant,

	/// Number of times this query was executed
	pub hit_count: usize,
}

/// Query cache implementation
pub struct QueryCache {
	config: QueryCacheConfig,
	cache: Arc<RwLock<HashMap<String, CachedQuery>>>,
}

impl QueryCache {
	/// Create a new query cache
	pub fn new(config: QueryCacheConfig) -> Self {
		Self {
			config,
			cache: Arc::new(RwLock::new(HashMap::new())),
		}
	}

	/// Get a cached query
	pub fn get(&self, sql: &str, params_hash: u64) -> Option<CachedQuery> {
		let cache = self.cache.read().ok()?;
		let cached = cache.get(sql)?;

		// Check if cache is expired
		if cached.cached_at.elapsed() > self.config.ttl {
			return None;
		}

		// Check if params match
		if cached.params_hash != params_hash {
			return None;
		}

		Some(cached.clone())
	}

	/// Cache a query
	pub fn set(&self, sql: String, params_hash: u64, result: Option<Vec<u8>>) {
		let mut cache = match self.cache.write() {
			Ok(c) => c,
			Err(_) => return,
		};

		// Evict oldest entry if cache is full
		if cache.len() >= self.config.max_size
			&& let Some((oldest_key, _)) = cache
				.iter()
				.min_by_key(|(_, v)| v.cached_at)
				.map(|(k, v)| (k.clone(), v.cached_at))
		{
			cache.remove(&oldest_key);
		}

		let cached = CachedQuery {
			sql: sql.clone(),
			params_hash,
			result,
			cached_at: Instant::now(),
			hit_count: 0,
		};

		cache.insert(sql, cached);
	}

	/// Increment hit count for a cached query
	pub fn record_hit(&self, sql: &str) {
		if let Ok(mut cache) = self.cache.write()
			&& let Some(cached) = cache.get_mut(sql)
		{
			cached.hit_count += 1;
		}
	}

	/// Clear all cached queries
	pub fn clear(&self) {
		if let Ok(mut cache) = self.cache.write() {
			cache.clear();
		}
	}

	/// Get cache statistics
	pub fn stats(&self) -> CacheStats {
		let cache = match self.cache.read() {
			Ok(c) => c,
			Err(_) => return CacheStats::default(),
		};

		let total_entries = cache.len();
		let total_hits: usize = cache.values().map(|v| v.hit_count).sum();
		let expired_entries = cache
			.values()
			.filter(|v| v.cached_at.elapsed() > self.config.ttl)
			.count();

		CacheStats {
			total_entries,
			total_hits,
			expired_entries,
		}
	}
}

/// Cache statistics
#[derive(Debug, Default, Clone)]
pub struct CacheStats {
	pub total_entries: usize,
	pub total_hits: usize,
	pub expired_entries: usize,
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_query_cache_basic() {
		let cache = QueryCache::new(QueryCacheConfig::default());
		let sql = "SELECT * FROM users WHERE id = $1".to_string();
		let params_hash = 12345u64;

		// Cache should be empty initially
		assert!(cache.get(&sql, params_hash).is_none());

		// Cache a query
		cache.set(sql.clone(), params_hash, Some(vec![1, 2, 3]));

		// Should retrieve cached query
		let cached = cache.get(&sql, params_hash).unwrap();
		assert_eq!(cached.sql, sql);
		assert_eq!(cached.params_hash, params_hash);
	}

	#[test]
	fn test_cache_expiration() {
		let config = QueryCacheConfig {
			max_size: 10,
			ttl: Duration::from_millis(100),
			cache_plans: true,
		};
		let cache = QueryCache::new(config);
		let sql = "SELECT * FROM users".to_string();
		let params_hash = 0u64;

		cache.set(sql.clone(), params_hash, None);

		// Should be cached immediately
		assert!(cache.get(&sql, params_hash).is_some());

		// Wait for expiration
		std::thread::sleep(Duration::from_millis(150));

		// Should be expired
		assert!(cache.get(&sql, params_hash).is_none());
	}

	#[test]
	fn test_cache_stats() {
		let cache = QueryCache::new(QueryCacheConfig::default());
		cache.set("query1".to_string(), 1, None);
		cache.set("query2".to_string(), 2, None);

		cache.record_hit("query1");
		cache.record_hit("query1");
		cache.record_hit("query2");

		let stats = cache.stats();
		assert_eq!(stats.total_entries, 2);
		assert_eq!(stats.total_hits, 3);
	}
}
