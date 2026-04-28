//! Response caching support for ViewSets
//!
//! Provides automatic caching for read-only operations (list and retrieve).
//! Supports TTL-based expiration and cache invalidation.

use async_trait::async_trait;
use reinhardt_http::{Request, Response, Result};
use reinhardt_utils::cache::Cache;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

/// Cache configuration for ViewSets
#[derive(Debug, Clone)]
pub struct CacheConfig {
	/// Cache key prefix
	pub key_prefix: String,
	/// Time-to-live for cached responses
	pub ttl: Option<Duration>,
	/// Whether to cache list() responses
	pub cache_list: bool,
	/// Whether to cache retrieve() responses
	pub cache_retrieve: bool,
}

impl CacheConfig {
	/// Create a new cache configuration
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_views::viewsets::CacheConfig;
	/// use std::time::Duration;
	///
	/// let config = CacheConfig::new("users")
	///     .with_ttl(Duration::from_secs(300))
	///     .cache_all();
	///
	/// assert_eq!(config.key_prefix, "users");
	/// assert!(config.cache_list);
	/// assert!(config.cache_retrieve);
	/// ```
	pub fn new(key_prefix: impl Into<String>) -> Self {
		Self {
			key_prefix: key_prefix.into(),
			ttl: None,
			cache_list: true,
			cache_retrieve: true,
		}
	}

	/// Set TTL for cached responses
	pub fn with_ttl(mut self, ttl: Duration) -> Self {
		self.ttl = Some(ttl);
		self
	}

	/// Enable caching for list() operations
	pub fn cache_list_only(mut self) -> Self {
		self.cache_list = true;
		self.cache_retrieve = false;
		self
	}

	/// Enable caching for retrieve() operations
	pub fn cache_retrieve_only(mut self) -> Self {
		self.cache_list = false;
		self.cache_retrieve = true;
		self
	}

	/// Enable caching for all read operations
	pub fn cache_all(mut self) -> Self {
		self.cache_list = true;
		self.cache_retrieve = true;
		self
	}
}

impl Default for CacheConfig {
	fn default() -> Self {
		Self::new("viewset")
			.with_ttl(Duration::from_secs(300)) // 5 minutes default TTL
			.cache_all()
	}
}

/// Cached response wrapper
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedResponse {
	/// HTTP status code
	pub status: u16,
	/// Response body
	pub body: Vec<u8>,
	/// Response headers (simplified)
	pub headers: Vec<(String, String)>,
}

impl CachedResponse {
	/// Create from a Response
	pub fn from_response(response: &Response) -> Self {
		let headers = response
			.headers
			.iter()
			.map(|(k, v)| (k.as_str().to_string(), v.to_str().unwrap_or("").to_string()))
			.collect();

		Self {
			status: response.status.as_u16(),
			body: response.body.to_vec(),
			headers,
		}
	}

	/// Convert to Response
	pub fn to_response(&self) -> Response {
		use hyper::StatusCode;

		let mut response =
			Response::new(StatusCode::from_u16(self.status).unwrap_or(StatusCode::OK));

		response.body = self.body.clone().into();

		for (key, value) in &self.headers {
			if let Ok(header_name) = hyper::header::HeaderName::from_bytes(key.as_bytes())
				&& let Ok(header_value) = hyper::header::HeaderValue::from_str(value)
			{
				response.headers.insert(header_name, header_value);
			}
		}

		response
	}
}

/// Cached ViewSet wrapper
///
/// # Example
///
/// ```
/// use reinhardt_views::viewsets::{CachedViewSet, CacheConfig, ModelViewSet};
/// use reinhardt_utils::cache::InMemoryCache;
/// use reinhardt_db::orm::{FieldSelector, Model};
/// use std::time::Duration;
///
/// #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
/// struct User {
///     id: Option<i64>,
///     name: String,
/// }
///
/// #[derive(Clone)]
/// struct UserFields;
/// impl FieldSelector for UserFields { fn with_alias(self, _: &str) -> Self { self } }
/// impl Model for User {
///     type PrimaryKey = i64;
///     type Fields = UserFields;
///     fn table_name() -> &'static str { "users" }
///     fn primary_key(&self) -> Option<i64> { self.id }
///     fn set_primary_key(&mut self, v: i64) { self.id = Some(v); }
///     fn new_fields() -> Self::Fields { UserFields }
/// }
///
/// #[derive(Debug, Clone)]
/// struct UserSerializer;
///
/// # async fn example() {
/// let cache = InMemoryCache::new();
/// let inner_viewset = ModelViewSet::<User, UserSerializer>::new("users");
/// let config = CacheConfig::new("users")
///     .with_ttl(Duration::from_secs(300))
///     .cache_all();
///
/// let cached_viewset = CachedViewSet::new(inner_viewset, cache, config);
/// # }
/// ```
pub struct CachedViewSet<V, C> {
	/// Inner ViewSet
	inner: Arc<V>,
	/// Cache backend
	cache: Arc<C>,
	/// Cache configuration
	config: CacheConfig,
	/// Tag for cache invalidation (format: "viewset:{key_prefix}")
	cache_tag: String,
	/// Tracked cache keys for selective invalidation
	cached_keys: Arc<RwLock<HashSet<String>>>,
}

impl<V, C> CachedViewSet<V, C>
where
	C: Cache,
{
	/// Create a new cached ViewSet
	pub fn new(inner: V, cache: C, config: CacheConfig) -> Self {
		let cache_tag = format!("viewset:{}", config.key_prefix);
		Self {
			inner: Arc::new(inner),
			cache: Arc::new(cache),
			config,
			cache_tag,
			cached_keys: Arc::new(RwLock::new(HashSet::new())),
		}
	}

	/// Get the cache tag for this ViewSet
	pub fn cache_tag(&self) -> &str {
		&self.cache_tag
	}

	/// Get the cache key for a list operation
	fn list_cache_key(&self, query_string: &str) -> String {
		format!("{}:list:{}", self.config.key_prefix, query_string)
	}

	/// Get the cache key for a retrieve operation
	fn retrieve_cache_key(&self, id: &str) -> String {
		format!("{}:retrieve:{}", self.config.key_prefix, id)
	}

	/// Get the inner ViewSet
	pub fn inner(&self) -> Arc<V> {
		self.inner.clone()
	}

	/// Get the cache backend
	pub fn cache(&self) -> Arc<C> {
		self.cache.clone()
	}

	/// Invalidate all cached responses for this ViewSet
	///
	/// This method only invalidates cache entries created by this ViewSet,
	/// not the entire cache. It uses tracked cache keys for selective invalidation.
	pub async fn invalidate_all(&self) -> Result<()> {
		// Get and clear the tracked keys
		let keys: Vec<String> = {
			let mut cached_keys = self.cached_keys.write().await;
			cached_keys.drain().collect()
		};

		// Delete all tracked keys from the cache
		for key in &keys {
			// Ignore errors for individual key deletions (key may have expired)
			let _ = self.cache.delete(key).await;
		}

		Ok(())
	}

	/// Track a cache key for later invalidation
	async fn track_cache_key(&self, key: &str) {
		let mut cached_keys = self.cached_keys.write().await;
		cached_keys.insert(key.to_string());
	}

	/// Invalidate cached response for a specific item
	pub async fn invalidate_item(&self, id: &str) -> Result<()> {
		let key = self.retrieve_cache_key(id);

		// Remove from tracked keys
		{
			let mut cached_keys = self.cached_keys.write().await;
			cached_keys.remove(&key);
		}

		self.cache.delete(&key).await?;
		Ok(())
	}
}

/// Trait for cached read operations
#[async_trait]
pub trait CachedViewSetTrait: Send + Sync {
	/// Cached list operation
	async fn cached_list(&self, request: Request) -> Result<Response>;

	/// Cached retrieve operation
	async fn cached_retrieve(&self, request: Request, id: String) -> Result<Response>;

	/// Invalidate cache for a specific item
	async fn invalidate(&self, id: &str) -> Result<()>;

	/// Invalidate all cached items
	async fn invalidate_all(&self) -> Result<()>;
}

#[async_trait]
impl<V, C> CachedViewSetTrait for CachedViewSet<V, C>
where
	V: crate::viewsets::ListMixin + crate::viewsets::RetrieveMixin + Send + Sync + 'static,
	C: Cache + Send + Sync + 'static,
{
	async fn cached_list(&self, request: Request) -> Result<Response> {
		if !self.config.cache_list {
			// Caching disabled, passthrough to inner viewset
			return self.inner.list(request).await;
		}

		let query_string = request.uri.query().unwrap_or("");
		let cache_key = self.list_cache_key(query_string);

		// Try to get from cache
		if let Some(cached) = self.cache.get::<CachedResponse>(&cache_key).await? {
			return Ok(cached.to_response());
		}

		// Cache miss - call inner viewset and cache result
		let response = self.inner.list(request).await?;
		let cached = CachedResponse::from_response(&response);

		// Cache the response with configured TTL and track the key
		self.cache.set(&cache_key, &cached, self.config.ttl).await?;
		self.track_cache_key(&cache_key).await;

		Ok(response)
	}

	async fn cached_retrieve(&self, request: Request, id: String) -> Result<Response> {
		if !self.config.cache_retrieve {
			// Caching disabled, passthrough to inner viewset
			return self.inner.retrieve(request, id).await;
		}

		let cache_key = self.retrieve_cache_key(&id);

		// Try to get from cache
		if let Some(cached) = self.cache.get::<CachedResponse>(&cache_key).await? {
			return Ok(cached.to_response());
		}

		// Cache miss - call inner viewset and cache result
		let response = self.inner.retrieve(request, id.clone()).await?;
		let cached = CachedResponse::from_response(&response);

		// Cache the response with configured TTL and track the key
		self.cache.set(&cache_key, &cached, self.config.ttl).await?;
		self.track_cache_key(&cache_key).await;

		Ok(response)
	}

	async fn invalidate(&self, id: &str) -> Result<()> {
		self.invalidate_item(id).await
	}

	async fn invalidate_all(&self) -> Result<()> {
		self.invalidate_all().await
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use bytes::Bytes;
	use hyper::StatusCode;
	use reinhardt_utils::cache::InMemoryCache;

	#[test]
	fn test_cache_config_builder() {
		let config = CacheConfig::new("users")
			.with_ttl(Duration::from_secs(300))
			.cache_all();

		assert_eq!(config.key_prefix, "users");
		assert_eq!(config.ttl, Some(Duration::from_secs(300)));
		assert!(config.cache_list);
		assert!(config.cache_retrieve);
	}

	#[test]
	fn test_cache_config_list_only() {
		let config = CacheConfig::new("posts").cache_list_only();

		assert!(config.cache_list);
		assert!(!config.cache_retrieve);
	}

	#[test]
	fn test_cache_config_retrieve_only() {
		let config = CacheConfig::new("posts").cache_retrieve_only();

		assert!(!config.cache_list);
		assert!(config.cache_retrieve);
	}

	#[test]
	fn test_cached_response_conversion() {
		let mut original = Response::new(StatusCode::OK);
		original.body = Bytes::from("test body");
		let cached = CachedResponse::from_response(&original);

		assert_eq!(cached.status, 200);
		assert_eq!(cached.body, b"test body");

		let restored = cached.to_response();
		assert_eq!(restored.status, StatusCode::OK);
		assert_eq!(restored.body, Bytes::from("test body"));
	}

	#[test]
	fn test_cached_viewset_creation() {
		#[derive(Debug, Clone)]
		struct TestViewSet {
			// Allow dead_code: field set during construction to simulate a named viewset in test
			#[allow(dead_code)]
			name: String,
		}

		let inner = TestViewSet {
			name: "users".to_string(),
		};
		let cache = InMemoryCache::new();
		let config = CacheConfig::new("users").cache_all();

		let cached_viewset = CachedViewSet::new(inner, cache, config);
		assert_eq!(cached_viewset.config.key_prefix, "users");
	}

	#[test]
	fn test_cache_keys() {
		#[derive(Debug, Clone)]
		struct TestViewSet;

		let inner = TestViewSet;
		let cache = InMemoryCache::new();
		let config = CacheConfig::new("users");

		let cached_viewset = CachedViewSet::new(inner, cache, config);

		let list_key = cached_viewset.list_cache_key("page=1&limit=10");
		assert_eq!(list_key, "users:list:page=1&limit=10");

		let retrieve_key = cached_viewset.retrieve_cache_key("123");
		assert_eq!(retrieve_key, "users:retrieve:123");
	}

	#[tokio::test]
	async fn test_invalidate_item() {
		#[derive(Debug, Clone)]
		struct TestViewSet;

		let inner = TestViewSet;
		let cache = InMemoryCache::new();
		let config = CacheConfig::new("users");

		let cached_viewset = CachedViewSet::new(inner, cache.clone(), config);

		// Set a cached value
		let cached_response = CachedResponse {
			status: 200,
			body: b"cached data".to_vec(),
			headers: vec![],
		};
		cache
			.set("users:retrieve:123", &cached_response, None)
			.await
			.unwrap();

		// Verify it exists
		let cached: Option<CachedResponse> = cache.get("users:retrieve:123").await.unwrap();
		assert!(cached.is_some());

		// Invalidate
		cached_viewset.invalidate_item("123").await.unwrap();

		// Verify it's gone
		let cached: Option<CachedResponse> = cache.get("users:retrieve:123").await.unwrap();
		assert!(cached.is_none());
	}

	#[tokio::test]
	async fn test_invalidate_all() {
		#[derive(Debug, Clone)]
		struct TestViewSet;

		let inner = TestViewSet;
		let cache = InMemoryCache::new();
		let config = CacheConfig::new("users");

		let cached_viewset = CachedViewSet::new(inner, cache.clone(), config);

		// Set multiple cached values and track them
		let cached_response = CachedResponse {
			status: 200,
			body: b"cached data".to_vec(),
			headers: vec![],
		};

		// Set and track cache keys (simulating what cached_list/cached_retrieve do)
		cache
			.set("users:retrieve:123", &cached_response, None)
			.await
			.unwrap();
		cached_viewset.track_cache_key("users:retrieve:123").await;

		cache
			.set("users:list:page=1", &cached_response, None)
			.await
			.unwrap();
		cached_viewset.track_cache_key("users:list:page=1").await;

		// Verify they exist
		let cached1: Option<CachedResponse> = cache.get("users:retrieve:123").await.unwrap();
		let cached2: Option<CachedResponse> = cache.get("users:list:page=1").await.unwrap();
		assert!(cached1.is_some());
		assert!(cached2.is_some());

		// Invalidate all
		cached_viewset.invalidate_all().await.unwrap();

		// Verify all are gone
		let cached1: Option<CachedResponse> = cache.get("users:retrieve:123").await.unwrap();
		let cached2: Option<CachedResponse> = cache.get("users:list:page=1").await.unwrap();
		assert!(cached1.is_none());
		assert!(cached2.is_none());
	}

	#[tokio::test]
	async fn test_invalidate_all_does_not_affect_other_viewsets() {
		#[derive(Debug, Clone)]
		struct TestViewSet;

		let cache = InMemoryCache::new();

		// Create two ViewSets with different prefixes
		let users_viewset =
			CachedViewSet::new(TestViewSet, cache.clone(), CacheConfig::new("users"));
		let posts_viewset =
			CachedViewSet::new(TestViewSet, cache.clone(), CacheConfig::new("posts"));

		let cached_response = CachedResponse {
			status: 200,
			body: b"cached data".to_vec(),
			headers: vec![],
		};

		// Set and track cache keys for both viewsets
		cache
			.set("users:retrieve:1", &cached_response, None)
			.await
			.unwrap();
		users_viewset.track_cache_key("users:retrieve:1").await;

		cache
			.set("posts:retrieve:1", &cached_response, None)
			.await
			.unwrap();
		posts_viewset.track_cache_key("posts:retrieve:1").await;

		// Invalidate only users viewset
		users_viewset.invalidate_all().await.unwrap();

		// Users cache should be gone
		let users_cached: Option<CachedResponse> = cache.get("users:retrieve:1").await.unwrap();
		assert!(users_cached.is_none());

		// Posts cache should still exist
		let posts_cached: Option<CachedResponse> = cache.get("posts:retrieve:1").await.unwrap();
		assert!(posts_cached.is_some());
	}

	#[test]
	fn test_cache_config_default() {
		let config = CacheConfig::default();
		assert_eq!(config.key_prefix, "viewset");
		assert!(config.cache_list);
		assert!(config.cache_retrieve);
		assert_eq!(config.ttl, Some(Duration::from_secs(300))); // 5 minutes default TTL
	}
}
