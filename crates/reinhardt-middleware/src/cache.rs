//! Cache Middleware
//!
//! Provides caching for HTTP responses.
//! Supports various cache backends (memory, Redis, file).
//! Requests carrying credentials or authenticated state bypass the cache. Responses marked
//! private, non-reusable, cookie-setting, or unsupported variant-dependent are never shared.

use async_trait::async_trait;
use hyper::StatusCode;
use hyper::header::{AUTHORIZATION, CACHE_CONTROL, COOKIE, SET_COOKIE, VARY};
use reinhardt_http::{AuthState, Handler, IsAuthenticated, Middleware, Request, Response, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

/// Cache Entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheEntry {
	/// Status code
	status: u16,
	/// Headers
	headers: HashMap<String, String>,
	/// Body
	body: Vec<u8>,
	/// Cached timestamp
	#[serde(skip)]
	cached_at: Option<Instant>,
	/// TTL (seconds)
	ttl_secs: u64,
}

impl CacheEntry {
	/// Create a new entry
	fn new(response: &Response, ttl: Duration) -> Self {
		let mut headers = HashMap::new();
		for (key, value) in response.headers.iter() {
			if let Ok(value_str) = value.to_str() {
				headers.insert(key.to_string(), value_str.to_string());
			}
		}

		Self {
			status: response.status.as_u16(),
			headers,
			body: response.body.to_vec(),
			cached_at: Some(Instant::now()),
			ttl_secs: ttl.as_secs(),
		}
	}

	/// Check if expired
	fn is_expired(&self) -> bool {
		if let Some(cached_at) = self.cached_at {
			cached_at.elapsed().as_secs() >= self.ttl_secs
		} else {
			true
		}
	}

	/// Check if an entry is safe to serve from a shared cache.
	fn is_shareable(&self, key_strategy: CacheKeyStrategy) -> bool {
		!self.headers.contains_key(SET_COOKIE.as_str())
			&& !self
				.headers
				.get(CACHE_CONTROL.as_str())
				.is_some_and(|value| cache_control_forbids_shared_storage(value))
			&& self.headers.get(VARY.as_str()).is_none_or(|value| {
				matches!(key_strategy, CacheKeyStrategy::UrlAndHeaders)
					&& !value.split(',').any(|field| field.trim() == "*")
			})
	}

	/// Convert to response
	fn to_response(&self) -> Response {
		let status = StatusCode::from_u16(self.status).unwrap_or(StatusCode::OK);
		let mut response = Response::new(status).with_body(self.body.clone());

		for (key, value) in &self.headers {
			if let (Ok(header_name), Ok(header_value)) =
				(hyper::header::HeaderName::try_from(key), value.parse())
			{
				response.headers.insert(header_name, header_value);
			}
		}

		// Add cache header
		response.headers.insert(
			hyper::header::HeaderName::from_static("x-cache"),
			hyper::header::HeaderValue::from_static("HIT"),
		);

		response
	}
}

fn cache_control_forbids_shared_storage(value: &str) -> bool {
	value.split(',').any(|directive| {
		let directive = directive.trim();
		let name = directive
			.split_once('=')
			.map_or(directive, |(name, _)| name)
			.trim();
		name.eq_ignore_ascii_case("private")
			|| name.eq_ignore_ascii_case("no-store")
			|| name.eq_ignore_ascii_case("no-cache")
	})
}

/// Cache Storage
#[derive(Debug, Default)]
pub struct CacheStore {
	/// Entries
	entries: RwLock<HashMap<String, CacheEntry>>,
}

impl CacheStore {
	/// Create a new store
	pub fn new() -> Self {
		Self::default()
	}

	/// Get an entry
	pub fn get(&self, key: &str) -> Option<CacheEntry> {
		let entries = self.entries.read().unwrap_or_else(|e| e.into_inner());
		entries.get(key).cloned()
	}

	/// Set an entry
	pub fn set(&self, key: String, entry: CacheEntry) {
		let mut entries = self.entries.write().unwrap_or_else(|e| e.into_inner());
		entries.insert(key, entry);
	}

	/// Delete an entry
	pub fn delete(&self, key: &str) {
		let mut entries = self.entries.write().unwrap_or_else(|e| e.into_inner());
		entries.remove(key);
	}

	/// Clean up expired entries
	pub fn cleanup(&self) {
		let mut entries = self.entries.write().unwrap_or_else(|e| e.into_inner());
		entries.retain(|_, entry| !entry.is_expired());
	}

	/// Clear the store
	pub fn clear(&self) {
		let mut entries = self.entries.write().unwrap_or_else(|e| e.into_inner());
		entries.clear();
	}

	/// Get the number of entries
	pub fn len(&self) -> usize {
		let entries = self.entries.read().unwrap_or_else(|e| e.into_inner());
		entries.len()
	}

	/// Check if the store is empty
	pub fn is_empty(&self) -> bool {
		let entries = self.entries.read().unwrap_or_else(|e| e.into_inner());
		entries.is_empty()
	}
}

/// Cache key generation strategy
#[derive(Debug, Clone, Copy)]
pub enum CacheKeyStrategy {
	/// URL only
	UrlOnly,
	/// URL and method
	UrlAndMethod,
	/// URL and query parameters
	UrlAndQuery,
	/// URL and headers
	UrlAndHeaders,
}

/// Cache configuration
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct CacheConfig {
	/// Default TTL
	pub default_ttl: Duration,
	/// Cache key generation strategy
	pub key_strategy: CacheKeyStrategy,
	/// Cacheable methods
	pub cacheable_methods: Vec<String>,
	/// Cacheable status codes
	pub cacheable_status_codes: Vec<u16>,
	/// Paths to exclude
	pub exclude_paths: Vec<String>,
	/// Maximum cache size
	pub max_entries: Option<usize>,
}

impl CacheConfig {
	/// Create a new configuration
	///
	/// # Examples
	///
	/// ```
	/// use std::time::Duration;
	/// use reinhardt_middleware::cache::{CacheConfig, CacheKeyStrategy};
	///
	/// let config = CacheConfig::new(Duration::from_secs(300), CacheKeyStrategy::UrlOnly);
	/// assert_eq!(config.default_ttl, Duration::from_secs(300));
	/// ```
	pub fn new(default_ttl: Duration, key_strategy: CacheKeyStrategy) -> Self {
		Self {
			default_ttl,
			key_strategy,
			cacheable_methods: vec!["GET".to_string(), "HEAD".to_string()],
			cacheable_status_codes: vec![200, 203, 204, 206, 300, 301, 404, 405, 410, 414, 501],
			exclude_paths: Vec::new(),
			max_entries: Some(1000),
		}
	}

	/// Set cacheable methods
	///
	/// # Examples
	///
	/// ```
	/// use std::time::Duration;
	/// use reinhardt_middleware::cache::{CacheConfig, CacheKeyStrategy};
	///
	/// let config = CacheConfig::new(Duration::from_secs(300), CacheKeyStrategy::UrlOnly)
	///     .with_cacheable_methods(vec!["GET".to_string()]);
	/// ```
	pub fn with_cacheable_methods(mut self, methods: Vec<String>) -> Self {
		self.cacheable_methods = methods;
		self
	}

	/// Add paths to exclude
	///
	/// # Examples
	///
	/// ```
	/// use std::time::Duration;
	/// use reinhardt_middleware::cache::{CacheConfig, CacheKeyStrategy};
	///
	/// let config = CacheConfig::new(Duration::from_secs(300), CacheKeyStrategy::UrlOnly)
	///     .with_excluded_paths(vec!["/admin".to_string()]);
	/// ```
	pub fn with_excluded_paths(mut self, paths: Vec<String>) -> Self {
		self.exclude_paths.extend(paths);
		self
	}

	/// Set maximum number of entries
	///
	/// # Examples
	///
	/// ```
	/// use std::time::Duration;
	/// use reinhardt_middleware::cache::{CacheConfig, CacheKeyStrategy};
	///
	/// let config = CacheConfig::new(Duration::from_secs(300), CacheKeyStrategy::UrlOnly)
	///     .with_max_entries(5000);
	/// ```
	pub fn with_max_entries(mut self, max_entries: usize) -> Self {
		self.max_entries = Some(max_entries);
		self
	}
}

impl Default for CacheConfig {
	fn default() -> Self {
		Self::new(Duration::from_secs(300), CacheKeyStrategy::UrlOnly)
	}
}

/// Cache Middleware
///
/// # Examples
///
/// ```
/// use std::sync::Arc;
/// use std::time::Duration;
/// use reinhardt_middleware::cache::{CacheMiddleware, CacheConfig, CacheKeyStrategy};
/// use reinhardt_http::{Handler, Middleware, Request, Response};
/// use hyper::{StatusCode, Method, Version, HeaderMap};
/// use bytes::Bytes;
///
/// struct TestHandler;
///
/// #[async_trait::async_trait]
/// impl Handler for TestHandler {
///     async fn handle(&self, _request: Request) -> reinhardt_core::exception::Result<Response> {
///         Ok(Response::new(StatusCode::OK).with_body(Bytes::from("OK")))
///     }
/// }
///
/// # tokio_test::block_on(async {
/// let config = CacheConfig::new(Duration::from_secs(60), CacheKeyStrategy::UrlOnly);
/// let middleware = CacheMiddleware::new(config);
/// let handler = Arc::new(TestHandler);
///
/// let request = Request::builder()
///     .method(Method::GET)
///     .uri("/api/data")
///     .version(Version::HTTP_11)
///     .headers(HeaderMap::new())
///     .body(Bytes::new())
///     .build()
///     .unwrap();
///
/// let response = middleware.process(request, handler).await.unwrap();
/// assert_eq!(response.status, StatusCode::OK);
/// # });
/// ```
pub struct CacheMiddleware {
	config: CacheConfig,
	store: Arc<CacheStore>,
}

impl CacheMiddleware {
	/// Create a new cache middleware
	///
	/// # Examples
	///
	/// ```
	/// use std::time::Duration;
	/// use reinhardt_middleware::cache::{CacheMiddleware, CacheConfig, CacheKeyStrategy};
	///
	/// let config = CacheConfig::new(Duration::from_secs(300), CacheKeyStrategy::UrlOnly);
	/// let middleware = CacheMiddleware::new(config);
	/// ```
	pub fn new(config: CacheConfig) -> Self {
		Self {
			config,
			store: Arc::new(CacheStore::new()),
		}
	}

	/// Create with default configuration
	pub fn with_defaults() -> Self {
		Self::new(CacheConfig::default())
	}

	/// Create from an existing Arc-wrapped cache store
	///
	/// This is provided for cases where you already have an `Arc<CacheStore>`.
	/// In most cases, you should use `new()` instead, which creates the store internally.
	pub fn from_arc(config: CacheConfig, store: Arc<CacheStore>) -> Self {
		Self { config, store }
	}

	/// Get a reference to the cache store
	///
	/// # Examples
	///
	/// ```
	/// use std::time::Duration;
	/// use reinhardt_middleware::cache::{CacheMiddleware, CacheConfig, CacheKeyStrategy};
	///
	/// let middleware = CacheMiddleware::new(
	///     CacheConfig::new(Duration::from_secs(300), CacheKeyStrategy::UrlOnly)
	/// );
	///
	/// // Access the store
	/// let store = middleware.store();
	/// assert_eq!(store.len(), 0);
	/// ```
	pub fn store(&self) -> &CacheStore {
		&self.store
	}

	/// Get a cloned Arc of the store (for cases where you need ownership)
	///
	/// In most cases, you should use `store()` instead to get a reference.
	pub fn store_arc(&self) -> Arc<CacheStore> {
		Arc::clone(&self.store)
	}

	/// Check if path should be excluded
	fn should_exclude(&self, path: &str) -> bool {
		self.config
			.exclude_paths
			.iter()
			.any(|p| path.starts_with(p))
	}

	/// Check if method is cacheable
	fn is_cacheable_method(&self, method: &str) -> bool {
		self.config.cacheable_methods.iter().any(|m| m == method)
	}

	/// Check if status code is cacheable
	fn is_cacheable_status(&self, status: u16) -> bool {
		self.config.cacheable_status_codes.contains(&status)
	}

	/// Check if a request may carry user-specific state.
	fn is_private_request(request: &Request) -> bool {
		request.headers.contains_key(AUTHORIZATION)
			|| request.headers.contains_key(COOKIE)
			|| request.headers.contains_key("remote_user")
			|| AuthState::from_extensions(&request.extensions)
				.is_some_and(|state| state.is_authenticated())
			|| request
				.extensions
				.get::<IsAuthenticated>()
				.is_some_and(|state| state.0)
	}

	/// Check if a response is safe to store in a shared cache.
	fn is_shareable_response(&self, response: &Response) -> bool {
		!response.headers.contains_key(SET_COOKIE)
			&& response.headers.get_all(CACHE_CONTROL).iter().all(|value| {
				value
					.to_str()
					.is_ok_and(|value| !cache_control_forbids_shared_storage(value))
			}) && response.headers.get_all(VARY).iter().all(|value| {
			matches!(self.config.key_strategy, CacheKeyStrategy::UrlAndHeaders)
				&& value
					.to_str()
					.is_ok_and(|value| !value.split(',').any(|field| field.trim() == "*"))
		})
	}

	/// Generate cache key
	fn generate_cache_key(&self, request: &Request) -> String {
		let base = match self.config.key_strategy {
			CacheKeyStrategy::UrlOnly => request.uri.path().to_string(),
			CacheKeyStrategy::UrlAndMethod => {
				format!("{}:{}", request.method.as_str(), request.uri.path())
			}
			CacheKeyStrategy::UrlAndQuery => {
				let query = request.uri.query().unwrap_or("");
				format!(
					"{}:{}?{}",
					request.method.as_str(),
					request.uri.path(),
					query
				)
			}
			CacheKeyStrategy::UrlAndHeaders => {
				let headers_str = request
					.headers
					.iter()
					.map(|(k, v)| format!("{}={}", k, v.to_str().unwrap_or("")))
					.collect::<Vec<_>>()
					.join("&");
				format!(
					"{}:{}:{}",
					request.method.as_str(),
					request.uri.path(),
					headers_str
				)
			}
		};

		// Hash with SHA256
		let mut hasher = Sha256::new();
		hasher.update(base.as_bytes());
		let result = hasher.finalize();
		hex::encode(result)
	}
}

impl Default for CacheMiddleware {
	fn default() -> Self {
		Self::with_defaults()
	}
}

#[async_trait]
impl Middleware for CacheMiddleware {
	async fn process(&self, request: Request, handler: Arc<dyn Handler>) -> Result<Response> {
		let path = request.uri.path().to_string();
		let method = request.method.as_str().to_string();

		// Skip excluded paths
		if self.should_exclude(&path) {
			return handler.handle(request).await;
		}

		// Skip non-cacheable methods
		if !self.is_cacheable_method(&method) {
			return handler.handle(request).await;
		}

		// Credential-bearing and authenticated requests must never use a shared cache entry.
		let cache_key =
			(!Self::is_private_request(&request)).then(|| self.generate_cache_key(&request));

		// Check cache
		if let Some((cache_key, entry)) = cache_key
			.as_deref()
			.and_then(|key| self.store.get(key).map(|entry| (key, entry)))
		{
			if !entry.is_shareable(self.config.key_strategy) {
				self.store.delete(cache_key);
			} else if !entry.is_expired() {
				// Cache hit
				return Ok(entry.to_response());
			} else {
				// Delete expired entry
				self.store.delete(cache_key);
			}
		}

		// Convert errors to responses so post-processing always runs,
		// even when invoked outside MiddlewareChain. (#3244)
		let response = match handler.handle(request).await {
			Ok(resp) => resp,
			Err(e) => Response::from(e),
		};

		// Save to cache if status code is cacheable
		if let Some(cache_key) = cache_key
			&& self.is_cacheable_status(response.status.as_u16())
			&& self.is_shareable_response(&response)
		{
			let entry = CacheEntry::new(&response, self.config.default_ttl);
			self.store.set(cache_key, entry);

			// Clean up expired entries if max entries exceeded
			if let Some(max_entries) = self.config.max_entries
				&& self.store.len() > max_entries
			{
				self.store.cleanup();
			}
		}

		// Add X-Cache header
		let mut response = response;
		response.headers.insert(
			hyper::header::HeaderName::from_static("x-cache"),
			hyper::header::HeaderValue::from_static("MISS"),
		);

		Ok(response)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use bytes::Bytes;
	use hyper::{HeaderMap, Method, StatusCode, Version};

	struct TestHandler {
		status: StatusCode,
		call_count: Arc<RwLock<usize>>,
	}

	impl TestHandler {
		fn new(status: StatusCode) -> Self {
			Self {
				status,
				call_count: Arc::new(RwLock::new(0)),
			}
		}

		fn get_call_count(&self) -> usize {
			*self.call_count.read().unwrap()
		}
	}

	#[async_trait]
	impl Handler for TestHandler {
		async fn handle(&self, _request: Request) -> Result<Response> {
			*self.call_count.write().unwrap() += 1;
			Ok(Response::new(self.status).with_body(Bytes::from("OK")))
		}
	}

	struct IdentityHandler;

	#[async_trait]
	impl Handler for IdentityHandler {
		async fn handle(&self, request: Request) -> Result<Response> {
			let identity = AuthState::from_extensions(&request.extensions)
				.filter(|state| state.is_authenticated())
				.map(|state| state.user_id().to_string())
				.or_else(|| {
					request
						.headers
						.get(AUTHORIZATION)
						.and_then(|value| value.to_str().ok())
						.map(str::to_string)
				})
				.or_else(|| {
					request
						.headers
						.get(COOKIE)
						.and_then(|value| value.to_str().ok())
						.map(str::to_string)
				})
				.or_else(|| {
					request
						.headers
						.get("remote_user")
						.and_then(|value| value.to_str().ok())
						.map(str::to_string)
				})
				.unwrap_or_else(|| "public".to_string());
			Ok(Response::new(StatusCode::OK).with_body(identity))
		}
	}

	#[tokio::test]
	async fn authenticated_responses_are_not_shared_by_url_only_cache() {
		let middleware = CacheMiddleware::with_defaults();
		let handler = Arc::new(IdentityHandler);

		for identity in ["Bearer alice", "Bearer bob"] {
			let mut headers = HeaderMap::new();
			headers.insert(AUTHORIZATION, identity.parse().unwrap());
			let request = Request::builder()
				.method(Method::GET)
				.uri("/account")
				.version(Version::HTTP_11)
				.headers(headers)
				.body(Bytes::new())
				.build()
				.unwrap();

			let response = middleware.process(request, handler.clone()).await.unwrap();
			assert_eq!(response.body, identity);
			assert_eq!(response.headers.get("x-cache").unwrap(), "MISS");
		}

		for identity in ["session=alice", "session=bob"] {
			let mut headers = HeaderMap::new();
			headers.insert(COOKIE, identity.parse().unwrap());
			let request = Request::builder()
				.method(Method::GET)
				.uri("/account")
				.version(Version::HTTP_11)
				.headers(headers)
				.body(Bytes::new())
				.build()
				.unwrap();

			let response = middleware.process(request, handler.clone()).await.unwrap();
			assert_eq!(response.body, identity);
			assert_eq!(response.headers.get("x-cache").unwrap(), "MISS");
		}

		for identity in ["alice", "bob"] {
			let mut headers = HeaderMap::new();
			headers.insert("remote_user", identity.parse().unwrap());
			let request = Request::builder()
				.method(Method::GET)
				.uri("/account")
				.version(Version::HTTP_11)
				.headers(headers)
				.body(Bytes::new())
				.build()
				.unwrap();

			let response = middleware.process(request, handler.clone()).await.unwrap();
			assert_eq!(response.body, identity);
			assert_eq!(response.headers.get("x-cache").unwrap(), "MISS");
		}

		let request = Request::builder()
			.method(Method::GET)
			.uri("/account")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();
		request
			.extensions
			.insert(AuthState::authenticated("extension-user", false, true));
		let response = middleware.process(request, handler.clone()).await.unwrap();
		assert_eq!(response.body, "extension-user");
		assert_eq!(response.headers.get("x-cache").unwrap(), "MISS");

		for expected_cache in ["MISS", "HIT"] {
			let request = Request::builder()
				.method(Method::GET)
				.uri("/public")
				.version(Version::HTTP_11)
				.headers(HeaderMap::new())
				.body(Bytes::new())
				.build()
				.unwrap();
			let response = middleware.process(request, handler.clone()).await.unwrap();
			assert_eq!(response.body, "public");
			assert_eq!(response.headers.get("x-cache").unwrap(), expected_cache);
		}
	}

	#[rstest::rstest]
	#[case("Cache-Control", b"private")]
	#[case("Cache-Control", b"PUBLIC, NO-STORE=\"field\"")]
	#[case("Cache-Control", b"no-cache")]
	#[case("Cache-Control", b"private=\"field-\x80\"")]
	#[case("Set-Cookie", b"session=alice")]
	#[case("Vary", b"Authorization")]
	#[tokio::test]
	async fn private_response_headers_prevent_shared_storage(
		#[case] header_name: &str,
		#[case] header_value: &[u8],
	) {
		struct PrivateResponseHandler {
			header_name: hyper::header::HeaderName,
			header_value: hyper::header::HeaderValue,
			call_count: RwLock<usize>,
		}

		#[async_trait]
		impl Handler for PrivateResponseHandler {
			async fn handle(&self, _request: Request) -> Result<Response> {
				let mut count = self.call_count.write().unwrap();
				*count += 1;
				let mut response = Response::new(StatusCode::OK).with_body(count.to_string());
				response
					.headers
					.insert(self.header_name.clone(), self.header_value.clone());
				Ok(response)
			}
		}

		let middleware = CacheMiddleware::with_defaults();
		let handler = Arc::new(PrivateResponseHandler {
			header_name: header_name.parse().unwrap(),
			header_value: hyper::header::HeaderValue::from_bytes(header_value).unwrap(),
			call_count: RwLock::new(0),
		});

		for expected_body in ["1", "2"] {
			let request = Request::builder()
				.method(Method::GET)
				.uri("/account")
				.version(Version::HTTP_11)
				.headers(HeaderMap::new())
				.body(Bytes::new())
				.build()
				.unwrap();
			let response = middleware.process(request, handler.clone()).await.unwrap();
			assert_eq!(response.body, expected_body);
			assert_eq!(response.headers.get("x-cache").unwrap(), "MISS");
		}
	}

	#[tokio::test]
	async fn url_and_headers_cache_preserves_supported_vary_responses() {
		struct VaryHandler;

		#[async_trait]
		impl Handler for VaryHandler {
			async fn handle(&self, _request: Request) -> Result<Response> {
				Ok(Response::new(StatusCode::OK)
					.with_body("public")
					.with_header("Vary", "Accept-Encoding"))
			}
		}

		let middleware = CacheMiddleware::new(CacheConfig::new(
			Duration::from_secs(60),
			CacheKeyStrategy::UrlAndHeaders,
		));
		let handler = Arc::new(VaryHandler);

		for expected_cache in ["MISS", "HIT"] {
			let request = Request::builder()
				.method(Method::GET)
				.uri("/public")
				.version(Version::HTTP_11)
				.headers(HeaderMap::new())
				.body(Bytes::new())
				.build()
				.unwrap();
			let response = middleware.process(request, handler.clone()).await.unwrap();
			assert_eq!(response.headers.get("x-cache").unwrap(), expected_cache);
		}
	}

	#[tokio::test]
	async fn test_cache_miss() {
		let config = CacheConfig::new(Duration::from_secs(60), CacheKeyStrategy::UrlOnly);
		let middleware = CacheMiddleware::new(config);
		let handler = Arc::new(TestHandler::new(StatusCode::OK));

		let request = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();

		assert_eq!(response.status, StatusCode::OK);
		assert_eq!(response.headers.get("x-cache").unwrap(), "MISS");
	}

	#[tokio::test]
	async fn test_cache_hit() {
		let config = CacheConfig::new(Duration::from_secs(60), CacheKeyStrategy::UrlOnly);
		let middleware = Arc::new(CacheMiddleware::new(config));
		let handler = Arc::new(TestHandler::new(StatusCode::OK));

		// First request (cache miss)
		let request1 = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();
		let response1 = middleware.process(request1, handler.clone()).await.unwrap();
		assert_eq!(response1.headers.get("x-cache").unwrap(), "MISS");
		assert_eq!(handler.get_call_count(), 1);

		// Second request (cache hit)
		let request2 = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();
		let response2 = middleware.process(request2, handler.clone()).await.unwrap();
		assert_eq!(response2.headers.get("x-cache").unwrap(), "HIT");
		assert_eq!(handler.get_call_count(), 1); // Handler is not called
	}

	#[tokio::test]
	async fn test_cache_expiration() {
		let config = CacheConfig::new(Duration::from_millis(100), CacheKeyStrategy::UrlOnly);
		let middleware = Arc::new(CacheMiddleware::new(config));
		let handler = Arc::new(TestHandler::new(StatusCode::OK));

		// First request
		let request1 = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();
		let _response1 = middleware.process(request1, handler.clone()).await.unwrap();

		// Wait for expiration
		std::thread::sleep(Duration::from_millis(150));

		// Request after expiration (cache miss)
		let request2 = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();
		let response2 = middleware.process(request2, handler.clone()).await.unwrap();
		assert_eq!(response2.headers.get("x-cache").unwrap(), "MISS");
		assert_eq!(handler.get_call_count(), 2);
	}

	#[tokio::test]
	async fn test_non_cacheable_method() {
		let config = CacheConfig::new(Duration::from_secs(60), CacheKeyStrategy::UrlOnly);
		let middleware = CacheMiddleware::new(config);
		let handler = Arc::new(TestHandler::new(StatusCode::OK));

		let request = Request::builder()
			.method(Method::POST)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();

		assert_eq!(response.status, StatusCode::OK);
		assert!(!response.headers.contains_key("x-cache"));
	}

	#[tokio::test]
	async fn test_exclude_paths() {
		let config = CacheConfig::new(Duration::from_secs(60), CacheKeyStrategy::UrlOnly)
			.with_excluded_paths(vec!["/admin".to_string()]);
		let middleware = CacheMiddleware::new(config);
		let handler = Arc::new(TestHandler::new(StatusCode::OK));

		let request = Request::builder()
			.method(Method::GET)
			.uri("/admin/users")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();

		assert_eq!(response.status, StatusCode::OK);
		assert!(!response.headers.contains_key("x-cache"));
	}

	#[tokio::test]
	async fn test_different_urls() {
		let config = CacheConfig::new(Duration::from_secs(60), CacheKeyStrategy::UrlOnly);
		let middleware = Arc::new(CacheMiddleware::new(config));
		let handler = Arc::new(TestHandler::new(StatusCode::OK));

		// Request to /test1
		let request1 = Request::builder()
			.method(Method::GET)
			.uri("/test1")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();
		let _response1 = middleware.process(request1, handler.clone()).await.unwrap();

		// Request to /test2 (different cache entry)
		let request2 = Request::builder()
			.method(Method::GET)
			.uri("/test2")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();
		let response2 = middleware.process(request2, handler.clone()).await.unwrap();

		assert_eq!(response2.headers.get("x-cache").unwrap(), "MISS");
		assert_eq!(handler.get_call_count(), 2);
	}

	#[tokio::test]
	async fn test_cache_store() {
		let store = CacheStore::new();

		let response = Response::new(StatusCode::OK).with_body(Bytes::from("test"));
		let entry = CacheEntry::new(&response, Duration::from_secs(60));

		store.set("key1".to_string(), entry.clone());

		assert_eq!(store.len(), 1);
		assert!(!store.is_empty());

		let retrieved = store.get("key1").unwrap();
		assert_eq!(retrieved.status, 200);
		assert_eq!(retrieved.body, b"test");
	}

	#[tokio::test]
	async fn test_cache_cleanup() {
		let store = CacheStore::new();

		let response = Response::new(StatusCode::OK).with_body(Bytes::from("test"));
		let mut entry = CacheEntry::new(&response, Duration::from_millis(10));
		entry.cached_at = Some(Instant::now() - Duration::from_millis(20));

		store.set("key1".to_string(), entry);

		store.cleanup();

		assert_eq!(store.len(), 0);
		assert!(store.is_empty());
	}

	#[tokio::test]
	async fn test_multiple_status_codes_cached() {
		let config = CacheConfig::new(Duration::from_secs(60), CacheKeyStrategy::UrlOnly);
		let middleware = Arc::new(CacheMiddleware::new(config));

		// Test with 404 status (cached by default)
		let handler_404 = Arc::new(TestHandler::new(StatusCode::NOT_FOUND));
		let request1 = Request::builder()
			.method(Method::GET)
			.uri("/not-found")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();
		let response1 = middleware
			.process(request1, handler_404.clone())
			.await
			.unwrap();
		assert_eq!(response1.status, StatusCode::NOT_FOUND);
		assert_eq!(response1.headers.get("x-cache").unwrap(), "MISS");
		assert_eq!(handler_404.get_call_count(), 1);

		// Second request to same 404 URL (cache hit)
		let request1b = Request::builder()
			.method(Method::GET)
			.uri("/not-found")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();
		let response1b = middleware
			.process(request1b, handler_404.clone())
			.await
			.unwrap();
		assert_eq!(response1b.status, StatusCode::NOT_FOUND);
		assert_eq!(response1b.headers.get("x-cache").unwrap(), "HIT");
		assert_eq!(handler_404.get_call_count(), 1); // Not called again

		// Test with 500 status (also cached by default)
		let handler_500 = Arc::new(TestHandler::new(StatusCode::INTERNAL_SERVER_ERROR));
		let request2 = Request::builder()
			.method(Method::GET)
			.uri("/error")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();
		let response2 = middleware
			.process(request2, handler_500.clone())
			.await
			.unwrap();
		assert_eq!(response2.status, StatusCode::INTERNAL_SERVER_ERROR);
		assert_eq!(response2.headers.get("x-cache").unwrap(), "MISS");
	}

	#[tokio::test]
	async fn test_cache_key_strategy_url_and_method() {
		let config = CacheConfig::new(Duration::from_secs(60), CacheKeyStrategy::UrlAndMethod);
		let middleware = Arc::new(CacheMiddleware::new(config));
		let handler = Arc::new(TestHandler::new(StatusCode::OK));

		// GET request to /api
		let request1 = Request::builder()
			.method(Method::GET)
			.uri("/api")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();
		let response1 = middleware.process(request1, handler.clone()).await.unwrap();
		assert_eq!(response1.headers.get("x-cache").unwrap(), "MISS");
		assert_eq!(handler.get_call_count(), 1);

		// HEAD request to same URL (different cache key due to method)
		let handler2 = Arc::new(TestHandler::new(StatusCode::OK));
		let request2 = Request::builder()
			.method(Method::HEAD)
			.uri("/api")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();
		let response2 = middleware
			.process(request2, handler2.clone())
			.await
			.unwrap();
		// Different method should result in cache miss
		assert_eq!(response2.headers.get("x-cache").unwrap(), "MISS");
		assert_eq!(handler2.get_call_count(), 1);
	}

	#[rstest::rstest]
	fn test_rwlock_poison_recovery_cache_store() {
		// Arrange
		let store = Arc::new(CacheStore::new());

		// Act - poison the RwLock by panicking while holding a write guard
		let store_clone = Arc::clone(&store);
		let _ = std::thread::spawn(move || {
			let _guard = store_clone.entries.write().unwrap();
			panic!("intentional panic to poison lock");
		})
		.join();

		// Assert - operations still work after poison recovery
		let response = Response::new(StatusCode::OK).with_body(Bytes::from("test"));
		let entry = CacheEntry::new(&response, Duration::from_secs(60));
		store.set("key1".to_string(), entry);
		assert_eq!(store.len(), 1);
		assert!(!store.is_empty());
		assert!(store.get("key1").is_some());
		store.delete("key1");
		assert_eq!(store.len(), 0);
	}
}
