//! Query result caching for filter backends
//!
//! Provides automatic caching of filter query results with TTL support.
//!
//! This module is only available with the `caching` feature enabled.
//!
//! # Examples
//!
//! ```rust,no_run
//! use reinhardt_rest::filters::{CachedFilterBackend, FilterBackend};
//! use reinhardt_utils::cache::InMemoryCache;
//! use std::time::Duration;
//! use std::collections::HashMap;
//!
//! # async fn example() {
//! // Create a cache
//! let cache = InMemoryCache::new();
//!
//! // Wrap any filter backend with caching
//! let cached_backend = CachedFilterBackend::new(cache, Duration::from_secs(300));
//!
//! // Use it like any other filter backend
//! let params = HashMap::new();
//! let sql = "SELECT * FROM users".to_string();
//! let result = cached_backend.filter_queryset(&params, sql).await;
//! # }
//! ```

use super::{FilterBackend, FilterResult};
use async_trait::async_trait;

#[cfg(feature = "caching")]
use reinhardt_utils::cache::Cache;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

/// Generate a cache key from filter parameters
///
/// # Examples
///
/// ```
/// use reinhardt_rest::filters::generate_cache_key;
/// use std::collections::HashMap;
///
/// let mut params = HashMap::new();
/// params.insert("search".to_string(), "rust".to_string());
/// params.insert("ordering".to_string(), "-created_at".to_string());
///
/// let key = generate_cache_key(&params, "SELECT * FROM articles");
/// assert!(!key.is_empty());
/// ```
pub fn generate_cache_key(query_params: &HashMap<String, String>, sql: &str) -> String {
	let mut hasher = Sha256::new();

	// Sort parameters for consistent hashing
	let mut sorted_params: Vec<_> = query_params.iter().collect();
	sorted_params.sort_by_key(|(k, _)| *k);

	for (key, value) in sorted_params {
		hasher.update(key.as_bytes());
		hasher.update(b"=");
		hasher.update(value.as_bytes());
		hasher.update(b"&");
	}

	hasher.update(sql.as_bytes());

	let result = hasher.finalize();
	format!("filter_cache:{}", hex::encode(result))
}

/// Cached filter result
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CachedResult {
	sql: String,
}

/// A filter backend that caches query results
///
/// Wraps any cache implementation and provides automatic caching of filter results.
///
/// This type is only available with the `caching` feature enabled.
///
/// # Examples
///
/// ```rust,no_run
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// use reinhardt_rest::filters::{CachedFilterBackend, FilterBackend};
/// use reinhardt_utils::cache::InMemoryCache;
/// use std::time::Duration;
/// use std::collections::HashMap;
///
/// # async fn example() {
/// let cache = InMemoryCache::new();
/// let backend = CachedFilterBackend::new(cache, Duration::from_secs(300));
///
/// let params = HashMap::new();
/// let sql = "SELECT * FROM users".to_string();
///
/// // First call - misses cache, processes query
/// let result1 = backend.filter_queryset(&params, sql.clone()).await.unwrap();
///
/// // Second call - hits cache, returns cached result
/// let result2 = backend.filter_queryset(&params, sql).await.unwrap();
///
/// assert_eq!(result1, result2);
/// # }
/// # Ok(())
/// # }
/// ```
#[cfg(feature = "caching")]
pub struct CachedFilterBackend<C: Cache> {
	cache: Arc<C>,
	ttl: Duration,
	inner: Option<Arc<dyn FilterBackend>>,
}

#[cfg(feature = "caching")]
impl<C: Cache> CachedFilterBackend<C> {
	/// Create a new cached filter backend
	///
	/// # Arguments
	///
	/// * `cache` - The cache implementation to use
	/// * `ttl` - Time-to-live for cached results
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::filters::CachedFilterBackend;
	/// use reinhardt_utils::cache::InMemoryCache;
	/// use std::time::Duration;
	///
	/// let cache = InMemoryCache::new();
	/// let backend = CachedFilterBackend::new(cache, Duration::from_secs(300));
	/// ```
	pub fn new(cache: C, ttl: Duration) -> Self {
		Self {
			cache: Arc::new(cache),
			ttl,
			inner: None,
		}
	}

	/// Wrap an existing filter backend with caching
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::filters::{CachedFilterBackend, SimpleSearchBackend};
	/// use reinhardt_utils::cache::InMemoryCache;
	/// use std::time::Duration;
	///
	/// let cache = InMemoryCache::new();
	/// let search_backend = SimpleSearchBackend::new("search").with_field("title");
	/// let backend = CachedFilterBackend::new(cache, Duration::from_secs(300))
	///     .with_inner(Box::new(search_backend));
	/// ```
	pub fn with_inner(mut self, inner: Box<dyn FilterBackend>) -> Self {
		self.inner = Some(Arc::from(inner));
		self
	}

	/// Get cache statistics (if available)
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::filters::CachedFilterBackend;
	/// use reinhardt_utils::cache::InMemoryCache;
	/// use std::time::Duration;
	///
	/// let cache = InMemoryCache::new();
	/// let backend = CachedFilterBackend::new(cache, Duration::from_secs(300));
	/// let ttl = backend.ttl();
	/// assert_eq!(ttl, Duration::from_secs(300));
	/// ```
	pub fn ttl(&self) -> Duration {
		self.ttl
	}
}

#[cfg(feature = "caching")]
#[async_trait]
impl<C: Cache> FilterBackend for CachedFilterBackend<C> {
	async fn filter_queryset(
		&self,
		query_params: &HashMap<String, String>,
		sql: String,
	) -> FilterResult<String> {
		let cache_key = generate_cache_key(query_params, &sql);

		// Try to get from cache
		if let Ok(Some(cached)) = self.cache.get::<CachedResult>(&cache_key).await {
			return Ok(cached.sql);
		}

		// Process the query
		let result_sql = if let Some(inner) = &self.inner {
			inner.filter_queryset(query_params, sql).await?
		} else {
			sql
		};

		// Cache the result
		let cached_result = CachedResult {
			sql: result_sql.clone(),
		};

		if let Err(e) = self
			.cache
			.set(&cache_key, &cached_result, Some(self.ttl))
			.await
		{
			// Log error but don't fail the request
			eprintln!("Failed to cache filter result: {:?}", e);
		}

		Ok(result_sql)
	}
}

/// Cache statistics for monitoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheStats {
	/// Number of cache hits
	pub hits: u64,
	/// Number of cache misses
	pub misses: u64,
	/// Total number of entries
	pub entries: u64,
}

impl CacheStats {
	/// Calculate the cache hit rate
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::filters::CacheStats;
	///
	/// let stats = CacheStats {
	///     hits: 75,
	///     misses: 25,
	///     entries: 50,
	/// };
	///
	/// let hit_rate = stats.hit_rate();
	/// assert_eq!(hit_rate, 0.75);
	/// ```
	pub fn hit_rate(&self) -> f64 {
		let total = self.hits + self.misses;
		if total == 0 {
			0.0
		} else {
			self.hits as f64 / total as f64
		}
	}
}

#[cfg(all(test, feature = "caching"))]
mod tests {
	use super::*;
	use crate::filters::backend::SimpleSearchBackend;
	use reinhardt_utils::cache::InMemoryCache;

	#[test]
	fn test_generate_cache_key_consistent() {
		let mut params1 = HashMap::new();
		params1.insert("search".to_string(), "rust".to_string());
		params1.insert("ordering".to_string(), "-created_at".to_string());

		let mut params2 = HashMap::new();
		params2.insert("ordering".to_string(), "-created_at".to_string());
		params2.insert("search".to_string(), "rust".to_string());

		let sql = "SELECT * FROM articles";

		let key1 = generate_cache_key(&params1, sql);
		let key2 = generate_cache_key(&params2, sql);

		assert_eq!(key1, key2);
	}

	#[test]
	fn test_generate_cache_key_different_params() {
		let mut params1 = HashMap::new();
		params1.insert("search".to_string(), "rust".to_string());

		let mut params2 = HashMap::new();
		params2.insert("search".to_string(), "python".to_string());

		let sql = "SELECT * FROM articles";

		let key1 = generate_cache_key(&params1, sql);
		let key2 = generate_cache_key(&params2, sql);

		assert_ne!(key1, key2);
	}

	#[test]
	fn test_generate_cache_key_different_sql() {
		let mut params = HashMap::new();
		params.insert("search".to_string(), "rust".to_string());

		let key1 = generate_cache_key(&params, "SELECT * FROM articles");
		let key2 = generate_cache_key(&params, "SELECT * FROM users");

		assert_ne!(key1, key2);
	}

	#[tokio::test]
	async fn test_cached_filter_backend_simple() {
		let cache = InMemoryCache::new();
		let backend = CachedFilterBackend::new(cache, Duration::from_secs(300));

		let params = HashMap::new();
		let sql = "SELECT * FROM users".to_string();

		let result1 = backend.filter_queryset(&params, sql.clone()).await.unwrap();
		let result2 = backend.filter_queryset(&params, sql).await.unwrap();

		assert_eq!(result1, result2);
	}

	#[tokio::test]
	async fn test_cached_filter_backend_with_inner() {
		let cache = InMemoryCache::new();
		let search_backend = SimpleSearchBackend::new("search").with_field("title");
		let backend = CachedFilterBackend::new(cache, Duration::from_secs(300))
			.with_inner(Box::new(search_backend));

		let mut params = HashMap::new();
		params.insert("search".to_string(), "rust".to_string());

		let sql = "SELECT * FROM articles".to_string();

		let result1 = backend.filter_queryset(&params, sql.clone()).await.unwrap();
		let result2 = backend.filter_queryset(&params, sql).await.unwrap();

		assert_eq!(result1, result2);
		assert!(result1.contains("WHERE"));
		assert!(result1.contains("`title` LIKE '%rust%'"));
	}

	#[tokio::test]
	async fn test_cached_filter_backend_ttl() {
		let cache = InMemoryCache::new();
		let backend = CachedFilterBackend::new(cache, Duration::from_millis(100));

		assert_eq!(backend.ttl(), Duration::from_millis(100));
	}

	#[test]
	fn test_cache_stats_hit_rate() {
		let stats = CacheStats {
			hits: 75,
			misses: 25,
			entries: 50,
		};

		assert_eq!(stats.hit_rate(), 0.75);
	}

	#[test]
	fn test_cache_stats_hit_rate_zero() {
		let stats = CacheStats {
			hits: 0,
			misses: 0,
			entries: 0,
		};

		assert_eq!(stats.hit_rate(), 0.0);
	}

	#[test]
	fn test_cache_stats_hit_rate_perfect() {
		let stats = CacheStats {
			hits: 100,
			misses: 0,
			entries: 50,
		};

		assert_eq!(stats.hit_rate(), 1.0);
	}
}
