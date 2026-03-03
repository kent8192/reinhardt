/// Lambda Statement - Cached query compilation
/// Based on SQLAlchemy's lambda_stmt
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Lambda statement for query caching
pub struct LambdaStmt {
	pub cache_key: String,
	lambda_fn: Box<dyn Fn() -> String + Send + Sync>,
}

impl LambdaStmt {
	/// Create a new cached lambda statement
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::lambda_stmt::LambdaStmt;
	///
	/// let stmt = LambdaStmt::new("get_active_users", || {
	///     "SELECT * FROM users WHERE active = true".to_string()
	/// });
	///
	/// let result = stmt.execute().unwrap();
	/// assert_eq!(result, "SELECT * FROM users WHERE active = true");
	/// ```
	pub fn new<F>(cache_key: impl Into<String>, lambda_fn: F) -> Self
	where
		F: Fn() -> String + Send + Sync + 'static,
	{
		Self {
			cache_key: cache_key.into(),
			lambda_fn: Box::new(lambda_fn),
		}
	}
	/// Execute the lambda function and cache the compiled query
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::lambda_stmt::LambdaStmt;
	///
	/// let stmt = LambdaStmt::new("user_query", || {
	///     "SELECT id, name FROM users".to_string()
	/// });
	///
	/// let result = stmt.execute();
	/// assert!(result.is_ok());
	/// assert_eq!(result.unwrap(), "SELECT id, name FROM users");
	/// ```
	pub fn execute(&self) -> Result<String, String> {
		// Check cache first
		if let Some(cached) = QUERY_CACHE.get(&self.cache_key) {
			CACHE_STATS.write().unwrap().hits += 1;
			return Ok(cached);
		}

		// Execute the lambda function to generate the query
		let query = (self.lambda_fn)();

		// Cache the compiled query
		QUERY_CACHE.set(self.cache_key.clone(), query.clone());
		CACHE_STATS.write().unwrap().misses += 1;

		Ok(query)
	}
	/// Check if this query has been cached
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::lambda_stmt::LambdaStmt;
	///
	/// let stmt = LambdaStmt::new("check_cache", || {
	///     "SELECT * FROM products".to_string()
	/// });
	///
	/// assert!(!stmt.is_cached());
	/// stmt.execute().unwrap();
	/// assert!(stmt.is_cached());
	/// ```
	pub fn is_cached(&self) -> bool {
		QUERY_CACHE.get(&self.cache_key).is_some()
	}
}

/// Cache for compiled queries
pub struct QueryCache {
	cache: Arc<RwLock<HashMap<String, String>>>,
}

impl QueryCache {
	/// Create a new empty query cache
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::lambda_stmt::QueryCache;
	///
	/// let cache = QueryCache::new();
	/// assert_eq!(cache.size(), 0);
	/// ```
	pub fn new() -> Self {
		Self {
			cache: Arc::new(RwLock::new(HashMap::new())),
		}
	}
	/// Get a cached query by key
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::lambda_stmt::QueryCache;
	///
	/// let cache = QueryCache::new();
	/// cache.set("key1".to_string(), "SELECT * FROM users".to_string());
	/// assert_eq!(cache.get("key1"), Some("SELECT * FROM users".to_string()));
	/// ```
	pub fn get(&self, key: &str) -> Option<String> {
		self.cache.read().unwrap().get(key).cloned()
	}
	/// Store a compiled query in the cache
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::lambda_stmt::QueryCache;
	///
	/// let cache = QueryCache::new();
	/// cache.set("users".to_string(), "SELECT id, name FROM users".to_string());
	/// assert!(cache.contains("users"));
	/// ```
	pub fn set(&self, key: String, value: String) {
		self.cache.write().unwrap().insert(key, value);
	}
	/// Clear all cached queries
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::lambda_stmt::QueryCache;
	///
	/// let cache = QueryCache::new();
	/// cache.set("key1".to_string(), "value1".to_string());
	/// assert_eq!(cache.size(), 1);
	/// cache.clear();
	/// assert_eq!(cache.size(), 0);
	/// ```
	pub fn clear(&self) {
		self.cache.write().unwrap().clear();
	}
	/// Get the number of cached queries
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::lambda_stmt::QueryCache;
	///
	/// let cache = QueryCache::new();
	/// cache.set("a".to_string(), "query1".to_string());
	/// cache.set("b".to_string(), "query2".to_string());
	/// assert_eq!(cache.size(), 2);
	/// ```
	pub fn size(&self) -> usize {
		self.cache.read().unwrap().len()
	}
	/// Remove a specific query from the cache
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::lambda_stmt::QueryCache;
	///
	/// let cache = QueryCache::new();
	/// cache.set("key1".to_string(), "value1".to_string());
	/// let removed = cache.remove("key1");
	/// assert_eq!(removed, Some("value1".to_string()));
	/// assert_eq!(cache.size(), 0);
	/// ```
	pub fn remove(&self, key: &str) -> Option<String> {
		self.cache.write().unwrap().remove(key)
	}
	/// Check if a query key exists in the cache
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::lambda_stmt::QueryCache;
	///
	/// let cache = QueryCache::new();
	/// cache.set("key1".to_string(), "value1".to_string());
	/// assert!(cache.contains("key1"));
	/// assert!(!cache.contains("key2"));
	/// ```
	pub fn contains(&self, key: &str) -> bool {
		self.cache.read().unwrap().contains_key(key)
	}
}

impl Default for QueryCache {
	fn default() -> Self {
		Self::new()
	}
}

use once_cell::sync::Lazy;

pub static QUERY_CACHE: Lazy<QueryCache> = Lazy::new(QueryCache::new);
pub static CACHE_STATS: Lazy<Arc<RwLock<CacheStatistics>>> =
	Lazy::new(|| Arc::new(RwLock::new(CacheStatistics::new())));

// Type alias for lambda function
type LambdaFunction = Box<dyn Fn() -> String + Send + Sync>;
type LambdaFunctionMap = Arc<RwLock<HashMap<String, LambdaFunction>>>;

// Lambda function registry
pub struct LambdaRegistry {
	// Allow dead_code: function registry stored for future lambda statement execution
	#[allow(dead_code)]
	functions: LambdaFunctionMap,
}

impl Default for LambdaRegistry {
	fn default() -> Self {
		Self::new()
	}
}

impl LambdaRegistry {
	/// Create a new lambda function registry
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::lambda_stmt::LambdaRegistry;
	///
	/// let registry = LambdaRegistry::new();
	/// ```
	pub fn new() -> Self {
		Self {
			functions: Arc::new(RwLock::new(HashMap::new())),
		}
	}
}

// Cache statistics
#[derive(Debug, Clone)]
pub struct CacheStatistics {
	pub hits: usize,
	pub misses: usize,
	pub total_size: usize,
}

impl CacheStatistics {
	/// Create new cache statistics with zero counts
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::lambda_stmt::CacheStatistics;
	///
	/// let stats = CacheStatistics::new();
	/// assert_eq!(stats.hits, 0);
	/// assert_eq!(stats.misses, 0);
	/// ```
	pub fn new() -> Self {
		Self {
			hits: 0,
			misses: 0,
			total_size: 0,
		}
	}
	/// Calculate the cache hit rate as a percentage
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::lambda_stmt::CacheStatistics;
	///
	/// let mut stats = CacheStatistics::new();
	/// stats.hits = 7;
	/// stats.misses = 3;
	/// assert_eq!(stats.hit_rate(), 0.7);
	/// ```
	pub fn hit_rate(&self) -> f64 {
		if self.hits + self.misses == 0 {
			0.0
		} else {
			self.hits as f64 / (self.hits + self.misses) as f64
		}
	}
	/// Reset all statistics to zero
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::lambda_stmt::CacheStatistics;
	///
	/// let mut stats = CacheStatistics::new();
	/// stats.hits = 10;
	/// stats.misses = 5;
	/// stats.reset();
	/// assert_eq!(stats.hits, 0);
	/// assert_eq!(stats.misses, 0);
	/// ```
	pub fn reset(&mut self) {
		self.hits = 0;
		self.misses = 0;
		self.total_size = 0;
	}
}

impl Default for CacheStatistics {
	fn default() -> Self {
		Self::new()
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_lambda_stmt_cache_operations() {
		let cache = QueryCache::new();
		cache.set("key1".to_string(), "value1".to_string());
		assert_eq!(cache.get("key1"), Some("value1".to_string()));
		cache.clear();
		assert_eq!(cache.get("key1"), None);
	}

	#[test]
	fn test_lambda_execution() {
		let stmt = LambdaStmt::new("test_query", || {
			"SELECT * FROM users WHERE active = true".to_string()
		});

		let result = stmt.execute().unwrap();
		assert_eq!(result, "SELECT * FROM users WHERE active = true");

		// Second execution should hit cache
		let result2 = stmt.execute().unwrap();
		assert_eq!(result2, result);
		assert!(stmt.is_cached());
	}

	#[test]
	fn test_cache_statistics() {
		let stats = CacheStatistics::new();
		assert_eq!(stats.hits, 0);
		assert_eq!(stats.misses, 0);
		assert_eq!(stats.hit_rate(), 0.0);
	}
}
