//! Rate Limiting Middleware
//!
//! Provides request rate limiting per route or per user.
//! Uses the Token Bucket algorithm to restrict excessive requests.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use hyper::StatusCode;
use reinhardt_http::{Handler, Middleware, Request, Response, Result};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

/// Rate Limiting Bucket
#[derive(Debug, Clone)]
struct Bucket {
	/// Number of tokens
	tokens: f64,
	/// Maximum number of tokens
	capacity: f64,
	/// Time when tokens were last refilled
	last_refill: Instant,
	/// Refill rate (tokens per second)
	refill_rate: f64,
}

impl Bucket {
	/// Create a new bucket
	fn new(capacity: f64, refill_rate: f64) -> Self {
		Self {
			tokens: capacity,
			capacity,
			last_refill: Instant::now(),
			refill_rate,
		}
	}

	/// Refill tokens
	fn refill(&mut self) {
		let now = Instant::now();
		let elapsed = now.duration_since(self.last_refill).as_secs_f64();
		let new_tokens = elapsed * self.refill_rate;

		self.tokens = (self.tokens + new_tokens).min(self.capacity);
		self.last_refill = now;
	}

	/// Consume tokens
	fn consume(&mut self, tokens: f64) -> bool {
		self.refill();

		if self.tokens >= tokens {
			self.tokens -= tokens;
			true
		} else {
			false
		}
	}

	/// Get the time until the next token becomes available
	fn time_until_next_token(&self) -> Duration {
		if self.tokens >= 1.0 {
			Duration::from_secs(0)
		} else {
			let tokens_needed = 1.0 - self.tokens;
			let seconds = tokens_needed / self.refill_rate;
			Duration::from_secs_f64(seconds)
		}
	}
}

/// Rate Limiting Storage
#[derive(Debug, Default)]
pub struct RateLimitStore {
	/// Buckets per key
	buckets: RwLock<HashMap<String, Bucket>>,
	/// Request history
	history: RwLock<HashMap<String, Vec<DateTime<Utc>>>>,
}

impl RateLimitStore {
	/// Create a new store
	pub fn new() -> Self {
		Self::default()
	}

	/// Get or create a bucket
	fn get_or_create_bucket(&self, key: &str, capacity: f64, refill_rate: f64) -> Bucket {
		let mut buckets = self.buckets.write().unwrap();
		buckets
			.entry(key.to_string())
			.or_insert_with(|| Bucket::new(capacity, refill_rate))
			.clone()
	}

	/// Update a bucket
	fn update_bucket(&self, key: &str, bucket: Bucket) {
		let mut buckets = self.buckets.write().unwrap();
		buckets.insert(key.to_string(), bucket);
	}

	/// Record a request
	pub fn record_request(&self, key: &str) {
		let mut history = self.history.write().unwrap();
		history.entry(key.to_string()).or_default().push(Utc::now());
	}

	/// Get the number of requests within a specified duration
	pub fn get_request_count(&self, key: &str, duration: Duration) -> usize {
		let history = self.history.read().unwrap();
		if let Some(requests) = history.get(key) {
			let cutoff = Utc::now() - chrono::Duration::from_std(duration).unwrap();
			requests.iter().filter(|&&time| time > cutoff).count()
		} else {
			0
		}
	}

	/// Clean up old request history and stale rate limit buckets
	///
	/// Removes request history entries older than `max_age` and evicts
	/// rate limit buckets that have not been refilled within `max_age`,
	/// preventing unbounded memory growth from accumulated stale entries.
	pub fn cleanup(&self, max_age: Duration) {
		// Prune old history entries
		let mut history = self.history.write().unwrap();
		let cutoff = Utc::now() - chrono::Duration::from_std(max_age).unwrap();

		for requests in history.values_mut() {
			requests.retain(|&time| time > cutoff);
		}

		history.retain(|_, requests| !requests.is_empty());
		drop(history);

		// Evict stale buckets that have not been accessed within max_age
		let mut buckets = self.buckets.write().unwrap();
		let now = Instant::now();
		buckets.retain(|_, bucket| now.duration_since(bucket.last_refill) < max_age);
	}

	/// Reset the store
	pub fn reset(&self) {
		self.buckets.write().unwrap().clear();
		self.history.write().unwrap().clear();
	}
}

// Re-export RateLimitStrategy from reinhardt-core
pub use reinhardt_core::RateLimitStrategy;

/// Rate Limiting Configuration
#[derive(Debug, Clone)]
pub struct RateLimitConfig {
	/// Strategy
	pub strategy: RateLimitStrategy,
	/// Bucket capacity (maximum number of tokens)
	pub capacity: f64,
	/// Refill rate (tokens per second)
	pub refill_rate: f64,
	/// Token consumption per request
	pub cost_per_request: f64,
	/// Paths to exclude
	pub exclude_paths: Vec<String>,
	/// Custom error message
	pub error_message: Option<String>,
	/// Trusted proxy IP addresses/CIDRs.
	/// Only requests from these IPs will have their X-Forwarded-For/X-Real-IP headers trusted.
	/// If empty, proxy headers are never trusted and remote_addr is always used.
	pub trusted_proxies: Vec<String>,
}

impl RateLimitConfig {
	/// Create a new configuration
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_middleware::rate_limit::{RateLimitConfig, RateLimitStrategy};
	///
	/// let config = RateLimitConfig::new(RateLimitStrategy::PerRoute, 100.0, 10.0);
	/// assert_eq!(config.capacity, 100.0);
	/// assert_eq!(config.refill_rate, 10.0);
	/// ```
	pub fn new(strategy: RateLimitStrategy, capacity: f64, refill_rate: f64) -> Self {
		Self {
			strategy,
			capacity,
			refill_rate,
			cost_per_request: 1.0,
			exclude_paths: Vec::new(),
			error_message: None,
			trusted_proxies: Vec::new(),
		}
	}

	/// Set the cost per request
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_middleware::rate_limit::{RateLimitConfig, RateLimitStrategy};
	///
	/// let config = RateLimitConfig::new(RateLimitStrategy::PerRoute, 100.0, 10.0)
	///     .with_cost_per_request(2.0);
	/// assert_eq!(config.cost_per_request, 2.0);
	/// ```
	pub fn with_cost_per_request(mut self, cost: f64) -> Self {
		self.cost_per_request = cost;
		self
	}

	/// Add paths to exclude
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_middleware::rate_limit::{RateLimitConfig, RateLimitStrategy};
	///
	/// let config = RateLimitConfig::new(RateLimitStrategy::PerRoute, 100.0, 10.0)
	///     .with_excluded_paths(vec!["/health".to_string()]);
	/// ```
	pub fn with_excluded_paths(mut self, paths: Vec<String>) -> Self {
		self.exclude_paths.extend(paths);
		self
	}

	/// Set a custom error message
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_middleware::rate_limit::{RateLimitConfig, RateLimitStrategy};
	///
	/// let config = RateLimitConfig::new(RateLimitStrategy::PerRoute, 100.0, 10.0)
	///     .with_error_message("Too many requests".to_string());
	/// ```
	pub fn with_error_message(mut self, message: String) -> Self {
		self.error_message = Some(message);
		self
	}

	/// Set trusted proxy addresses.
	///
	/// Only requests originating from these IP addresses will have their
	/// `X-Forwarded-For` and `X-Real-IP` headers trusted for client IP extraction.
	/// Supports both individual IPs (e.g., "10.0.0.1") and CIDR notation (e.g., "10.0.0.0/8").
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_middleware::rate_limit::{RateLimitConfig, RateLimitStrategy};
	///
	/// let config = RateLimitConfig::new(RateLimitStrategy::PerIp, 100.0, 10.0)
	///     .with_trusted_proxies(vec!["10.0.0.0/8".to_string(), "172.16.0.0/12".to_string()]);
	/// ```
	pub fn with_trusted_proxies(mut self, proxies: Vec<String>) -> Self {
		self.trusted_proxies = proxies;
		self
	}
}

impl Default for RateLimitConfig {
	fn default() -> Self {
		Self::new(RateLimitStrategy::PerRoute, 100.0, 10.0)
	}
}

/// Rate Limiting Middleware
///
/// # Examples
///
/// ```
/// use std::sync::Arc;
/// use reinhardt_middleware::rate_limit::{RateLimitMiddleware, RateLimitConfig, RateLimitStrategy};
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
/// let config = RateLimitConfig::new(RateLimitStrategy::PerRoute, 10.0, 1.0);
/// let middleware = RateLimitMiddleware::new(config);
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
pub struct RateLimitMiddleware {
	config: RateLimitConfig,
	store: Arc<RateLimitStore>,
}

impl RateLimitMiddleware {
	/// Create a new rate limiting middleware
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_middleware::rate_limit::{RateLimitMiddleware, RateLimitConfig, RateLimitStrategy};
	///
	/// let config = RateLimitConfig::new(RateLimitStrategy::PerRoute, 100.0, 10.0);
	/// let middleware = RateLimitMiddleware::new(config);
	/// ```
	pub fn new(config: RateLimitConfig) -> Self {
		Self {
			config,
			store: Arc::new(RateLimitStore::new()),
		}
	}

	/// Create with default configuration
	pub fn with_defaults() -> Self {
		Self::new(RateLimitConfig::default())
	}

	/// Create a new RateLimitMiddleware from an Arc-wrapped RateLimitStore
	///
	/// This allows sharing the same rate limit store across multiple middleware instances.
	///
	/// # Examples
	///
	/// ```
	/// use std::sync::Arc;
	/// use reinhardt_middleware::rate_limit::{RateLimitMiddleware, RateLimitConfig, RateLimitStore, RateLimitStrategy};
	///
	/// let store = Arc::new(RateLimitStore::new());
	/// let config = RateLimitConfig::new(RateLimitStrategy::PerRoute, 100.0, 10.0);
	/// let middleware = RateLimitMiddleware::from_arc(config, store);
	/// ```
	pub fn from_arc(config: RateLimitConfig, store: Arc<RateLimitStore>) -> Self {
		Self { config, store }
	}

	/// Get a reference to the rate limit store
	///
	/// This is the preferred method for accessing the store when you only need
	/// to read data or call methods that don't require ownership.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_middleware::rate_limit::{RateLimitMiddleware, RateLimitConfig, RateLimitStrategy};
	/// use std::time::Duration;
	///
	/// let middleware = RateLimitMiddleware::new(
	///     RateLimitConfig::new(RateLimitStrategy::PerRoute, 100.0, 10.0)
	/// );
	/// let count = middleware.store().get_request_count("route:/api/data", Duration::from_secs(60));
	/// println!("Request count: {}", count);
	/// ```
	pub fn store(&self) -> &RateLimitStore {
		&self.store
	}

	/// Get a cloned Arc of the rate limit store
	///
	/// Use this when you need ownership of the Arc, for example when passing
	/// the store to another component that requires `Arc<RateLimitStore>`.
	///
	/// In most cases, you should prefer `store()` which returns a reference.
	///
	/// # Examples
	///
	/// ```
	/// use std::sync::Arc;
	/// use reinhardt_middleware::rate_limit::{RateLimitMiddleware, RateLimitConfig, RateLimitStore, RateLimitStrategy};
	///
	/// let middleware = RateLimitMiddleware::new(
	///     RateLimitConfig::new(RateLimitStrategy::PerRoute, 100.0, 10.0)
	/// );
	/// let store_arc: Arc<RateLimitStore> = middleware.store_arc();
	/// // Now you can pass store_arc to other components
	/// ```
	pub fn store_arc(&self) -> Arc<RateLimitStore> {
		Arc::clone(&self.store)
	}

	/// Check if a path should be excluded
	fn should_exclude(&self, path: &str) -> bool {
		self.config
			.exclude_paths
			.iter()
			.any(|p| path.starts_with(p))
	}

	/// Generate a request key
	fn generate_key(&self, request: &Request) -> String {
		match self.config.strategy {
			RateLimitStrategy::PerRoute => {
				format!("route:{}", request.uri.path())
			}
			RateLimitStrategy::PerUser => {
				if let Some(user_id) = self.extract_user_id(request) {
					format!("user:{}", user_id)
				} else {
					"user:anonymous".to_string()
				}
			}
			RateLimitStrategy::PerIp => {
				format!("ip:{}", self.extract_client_ip(request))
			}
			RateLimitStrategy::PerIpAndUser => {
				let ip = self.extract_client_ip(request);
				if let Some(user_id) = self.extract_user_id(request) {
					format!("ip_user:{}:{}", ip, user_id)
				} else {
					format!("ip_user:{}:anonymous", ip)
				}
			}
		}
	}

	/// Extract user ID from request
	///
	/// Attempts to retrieve the authenticated user ID from request extensions.
	fn extract_user_id(&self, request: &Request) -> Option<String> {
		// Try to get user ID from extensions
		// The user ID might be stored as String or i64
		if let Some(user_id) = request.extensions.get::<String>() {
			Some(user_id)
		} else {
			request
				.extensions
				.get::<i64>()
				.map(|user_id| user_id.to_string())
		}
	}

	/// Extract client IP address from request
	///
	/// Only trusts proxy headers (X-Forwarded-For, X-Real-IP) when the request
	/// originates from a configured trusted proxy address. Otherwise, uses the
	/// direct connection IP (remote_addr).
	///
	/// Extraction order:
	/// 1. If remote_addr is from a trusted proxy:
	///    a. X-Forwarded-For header (first IP in the list)
	///    b. X-Real-IP header
	/// 2. remote_addr field from the request
	/// 3. Falls back to 127.0.0.1 if none available
	fn extract_client_ip(&self, request: &Request) -> String {
		let peer_ip = request.remote_addr.map(|addr| addr.ip());

		// Only trust proxy headers if the direct connection is from a trusted proxy
		let from_trusted_proxy = peer_ip.map(|ip| self.is_trusted_proxy(ip)).unwrap_or(false);

		if from_trusted_proxy {
			// 1a. Check X-Forwarded-For header
			if let Some(xff) = request.headers.get("X-Forwarded-For")
				&& let Ok(xff_str) = xff.to_str()
				&& let Some(first_ip) = xff_str.split(',').next()
			{
				let trimmed = first_ip.trim();
				if trimmed.parse::<std::net::IpAddr>().is_ok() {
					return trimmed.to_string();
				}
			}

			// 1b. Check X-Real-IP header
			if let Some(xri) = request.headers.get("X-Real-IP")
				&& let Ok(ip_str) = xri.to_str()
			{
				let trimmed = ip_str.trim();
				if trimmed.parse::<std::net::IpAddr>().is_ok() {
					return trimmed.to_string();
				}
			}
		}

		// 2. Use remote_addr (direct connection IP)
		if let Some(ip) = peer_ip {
			return ip.to_string();
		}

		// 3. Fallback to localhost
		"127.0.0.1".to_string()
	}

	/// Check if an IP address belongs to a trusted proxy
	fn is_trusted_proxy(&self, ip: std::net::IpAddr) -> bool {
		self.config.trusted_proxies.iter().any(|proxy| {
			// Try parsing as CIDR network
			if let Ok(network) = proxy.parse::<ipnet::IpNet>() {
				return network.contains(&ip);
			}
			// Try parsing as single IP
			if let Ok(proxy_ip) = proxy.parse::<std::net::IpAddr>() {
				return proxy_ip == ip;
			}
			false
		})
	}

	/// Create a rate limit error response
	fn rate_limit_error(&self, retry_after: Duration) -> Response {
		let message = self
			.config
			.error_message
			.clone()
			.unwrap_or_else(|| "Rate limit exceeded".to_string());

		Response::new(StatusCode::TOO_MANY_REQUESTS)
			.with_header("Retry-After", &retry_after.as_secs().to_string())
			.with_header("X-RateLimit-Limit", &self.config.capacity.to_string())
			.with_header("X-RateLimit-Remaining", "0")
			.with_body(message.into_bytes())
	}
}

impl Default for RateLimitMiddleware {
	fn default() -> Self {
		Self::with_defaults()
	}
}

#[async_trait]
impl Middleware for RateLimitMiddleware {
	async fn process(&self, request: Request, handler: Arc<dyn Handler>) -> Result<Response> {
		let path = request.uri.path().to_string();

		// Skip excluded paths
		if self.should_exclude(&path) {
			return handler.handle(request).await;
		}

		// Generate key
		let key = self.generate_key(&request);

		// Get bucket
		let mut bucket =
			self.store
				.get_or_create_bucket(&key, self.config.capacity, self.config.refill_rate);

		// Consume tokens
		if bucket.consume(self.config.cost_per_request) {
			// Record request
			self.store.record_request(&key);

			// Update bucket
			self.store.update_bucket(&key, bucket.clone());

			// Call handler
			let mut response = handler.handle(request).await?;

			// Add rate limiting headers
			response.headers.insert(
				hyper::header::HeaderName::from_static("x-ratelimit-limit"),
				hyper::header::HeaderValue::from_str(&self.config.capacity.to_string())
					.unwrap_or_else(|_| hyper::header::HeaderValue::from_static("100")),
			);
			response.headers.insert(
				hyper::header::HeaderName::from_static("x-ratelimit-remaining"),
				hyper::header::HeaderValue::from_str(&bucket.tokens.floor().to_string())
					.unwrap_or_else(|_| hyper::header::HeaderValue::from_static("0")),
			);

			Ok(response)
		} else {
			// Rate limit exceeded
			let retry_after = bucket.time_until_next_token();
			Ok(self.rate_limit_error(retry_after))
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use bytes::Bytes;
	use hyper::{HeaderMap, Method, StatusCode, Version};
	use std::thread;
	use std::time::Duration;

	struct TestHandler {
		status: StatusCode,
	}

	impl TestHandler {
		fn new(status: StatusCode) -> Self {
			Self { status }
		}
	}

	#[async_trait]
	impl Handler for TestHandler {
		async fn handle(&self, _request: Request) -> Result<Response> {
			Ok(Response::new(self.status).with_body(Bytes::from("OK")))
		}
	}

	#[tokio::test]
	async fn test_rate_limit_allowed() {
		let config = RateLimitConfig::new(RateLimitStrategy::PerRoute, 10.0, 1.0);
		let middleware = RateLimitMiddleware::new(config);
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
		assert!(response.headers.contains_key("x-ratelimit-limit"));
		assert!(response.headers.contains_key("x-ratelimit-remaining"));
	}

	#[tokio::test]
	async fn test_rate_limit_exceeded() {
		let config = RateLimitConfig::new(RateLimitStrategy::PerRoute, 2.0, 0.1);
		let middleware = Arc::new(RateLimitMiddleware::new(config));
		let handler = Arc::new(TestHandler::new(StatusCode::OK));

		// First 2 requests succeed
		for _ in 0..2 {
			let request = Request::builder()
				.method(Method::GET)
				.uri("/test")
				.version(Version::HTTP_11)
				.headers(HeaderMap::new())
				.body(Bytes::new())
				.build()
				.unwrap();
			let response = middleware.process(request, handler.clone()).await.unwrap();
			assert_eq!(response.status, StatusCode::OK);
		}

		// 3rd request exceeds rate limit
		let request = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();
		let response = middleware.process(request, handler).await.unwrap();

		assert_eq!(response.status, StatusCode::TOO_MANY_REQUESTS);
		assert!(response.headers.contains_key("retry-after"));
	}

	#[tokio::test]
	async fn test_bucket_refill() {
		let config = RateLimitConfig::new(RateLimitStrategy::PerRoute, 2.0, 2.0);
		let middleware = Arc::new(RateLimitMiddleware::new(config));
		let handler = Arc::new(TestHandler::new(StatusCode::OK));

		// First 2 requests succeed
		for _ in 0..2 {
			let request = Request::builder()
				.method(Method::GET)
				.uri("/test")
				.version(Version::HTTP_11)
				.headers(HeaderMap::new())
				.body(Bytes::new())
				.build()
				.unwrap();
			let response = middleware.process(request, handler.clone()).await.unwrap();
			assert_eq!(response.status, StatusCode::OK);
		}

		// Wait (tokens are refilled)
		thread::sleep(Duration::from_secs(1));

		// Next request should succeed
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
	}

	#[tokio::test]
	async fn test_exclude_paths() {
		let config = RateLimitConfig::new(RateLimitStrategy::PerRoute, 1.0, 0.1)
			.with_excluded_paths(vec!["/health".to_string()]);
		let middleware = Arc::new(RateLimitMiddleware::new(config));
		let handler = Arc::new(TestHandler::new(StatusCode::OK));

		// Multiple requests to excluded paths are not limited
		for _ in 0..5 {
			let request = Request::builder()
				.method(Method::GET)
				.uri("/health")
				.version(Version::HTTP_11)
				.headers(HeaderMap::new())
				.body(Bytes::new())
				.build()
				.unwrap();
			let response = middleware.process(request, handler.clone()).await.unwrap();
			assert_eq!(response.status, StatusCode::OK);
		}
	}

	#[tokio::test]
	async fn test_custom_error_message() {
		let config = RateLimitConfig::new(RateLimitStrategy::PerRoute, 1.0, 0.1)
			.with_error_message("Custom error message".to_string());
		let middleware = Arc::new(RateLimitMiddleware::new(config));
		let handler = Arc::new(TestHandler::new(StatusCode::OK));

		// First request succeeds
		let request1 = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();
		let _response1 = middleware.process(request1, handler.clone()).await.unwrap();

		// 2nd request exceeds rate limit
		let request2 = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();
		let response2 = middleware.process(request2, handler).await.unwrap();

		assert_eq!(response2.status, StatusCode::TOO_MANY_REQUESTS);
		let body = String::from_utf8(response2.body.to_vec()).unwrap();
		assert_eq!(body, "Custom error message");
	}

	#[tokio::test]
	async fn test_different_routes() {
		let config = RateLimitConfig::new(RateLimitStrategy::PerRoute, 1.0, 0.1);
		let middleware = Arc::new(RateLimitMiddleware::new(config));
		let handler = Arc::new(TestHandler::new(StatusCode::OK));

		// First request to /test1 succeeds
		let request1 = Request::builder()
			.method(Method::GET)
			.uri("/test1")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();
		let response1 = middleware.process(request1, handler.clone()).await.unwrap();
		assert_eq!(response1.status, StatusCode::OK);

		// First request to /test2 also succeeds (separate bucket)
		let request2 = Request::builder()
			.method(Method::GET)
			.uri("/test2")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();
		let response2 = middleware.process(request2, handler).await.unwrap();
		assert_eq!(response2.status, StatusCode::OK);
	}

	#[tokio::test]
	async fn test_rate_limit_store() {
		let store = RateLimitStore::new();

		store.record_request("test");
		store.record_request("test");
		store.record_request("test");

		let count = store.get_request_count("test", Duration::from_secs(60));
		assert_eq!(count, 3);
	}

	#[tokio::test]
	async fn test_store_cleanup() {
		let store = RateLimitStore::new();

		store.record_request("test");
		thread::sleep(Duration::from_millis(100));

		store.cleanup(Duration::from_millis(50));

		let count = store.get_request_count("test", Duration::from_secs(60));
		assert_eq!(count, 0);
	}

	#[tokio::test]
	async fn test_extract_ip_from_x_forwarded_for_trusted_proxy() {
		// Arrange
		let config = RateLimitConfig::new(RateLimitStrategy::PerIp, 10.0, 1.0)
			.with_trusted_proxies(vec!["10.0.0.1".to_string()]);
		let middleware = RateLimitMiddleware::new(config);

		let mut headers = HeaderMap::new();
		headers.insert(
			"X-Forwarded-For",
			"203.0.113.195, 70.41.3.18, 150.172.238.178"
				.parse()
				.unwrap(),
		);

		let mut request = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(headers)
			.body(Bytes::new())
			.build()
			.unwrap();
		request.remote_addr = Some("10.0.0.1:12345".parse().unwrap());

		// Act
		let ip = middleware.extract_client_ip(&request);

		// Assert
		assert_eq!(ip, "203.0.113.195");
	}

	#[tokio::test]
	async fn test_extract_ip_ignores_xff_from_untrusted_source() {
		// Arrange
		let config = RateLimitConfig::new(RateLimitStrategy::PerIp, 10.0, 1.0)
			.with_trusted_proxies(vec!["10.0.0.1".to_string()]);
		let middleware = RateLimitMiddleware::new(config);

		let mut headers = HeaderMap::new();
		headers.insert("X-Forwarded-For", "203.0.113.195".parse().unwrap());

		let mut request = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(headers)
			.body(Bytes::new())
			.build()
			.unwrap();
		// Request comes from an untrusted IP
		request.remote_addr = Some("192.168.1.100:54321".parse().unwrap());

		// Act
		let ip = middleware.extract_client_ip(&request);

		// Assert - should use remote_addr, not X-Forwarded-For
		assert_eq!(ip, "192.168.1.100");
	}

	#[tokio::test]
	async fn test_extract_ip_from_x_real_ip_trusted_proxy() {
		// Arrange
		let config = RateLimitConfig::new(RateLimitStrategy::PerIp, 10.0, 1.0)
			.with_trusted_proxies(vec!["10.0.0.0/8".to_string()]);
		let middleware = RateLimitMiddleware::new(config);

		let mut headers = HeaderMap::new();
		headers.insert("X-Real-IP", "198.51.100.42".parse().unwrap());

		let mut request = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(headers)
			.body(Bytes::new())
			.build()
			.unwrap();
		request.remote_addr = Some("10.0.0.5:8080".parse().unwrap());

		// Act
		let ip = middleware.extract_client_ip(&request);

		// Assert
		assert_eq!(ip, "198.51.100.42");
	}

	#[tokio::test]
	async fn test_extract_ip_from_remote_addr() {
		// Arrange
		let config = RateLimitConfig::new(RateLimitStrategy::PerIp, 10.0, 1.0);
		let middleware = RateLimitMiddleware::new(config);

		let mut request = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();
		request.remote_addr = Some("192.0.2.123:8080".parse().unwrap());

		// Act
		let ip = middleware.extract_client_ip(&request);

		// Assert
		assert_eq!(ip, "192.0.2.123");
	}

	#[tokio::test]
	async fn test_extract_ip_fallback_to_localhost() {
		// Arrange
		let config = RateLimitConfig::new(RateLimitStrategy::PerIp, 10.0, 1.0);
		let middleware = RateLimitMiddleware::new(config);

		let request = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		// Act
		let ip = middleware.extract_client_ip(&request);

		// Assert
		assert_eq!(ip, "127.0.0.1");
	}

	#[tokio::test]
	async fn test_extract_ip_no_trusted_proxies_ignores_headers() {
		// Arrange - no trusted proxies configured
		let config = RateLimitConfig::new(RateLimitStrategy::PerIp, 10.0, 1.0);
		let middleware = RateLimitMiddleware::new(config);

		let mut headers = HeaderMap::new();
		headers.insert("X-Forwarded-For", "203.0.113.195".parse().unwrap());
		headers.insert("X-Real-IP", "198.51.100.42".parse().unwrap());

		let mut request = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(headers)
			.body(Bytes::new())
			.build()
			.unwrap();
		request.remote_addr = Some("192.168.1.1:8080".parse().unwrap());

		// Act
		let ip = middleware.extract_client_ip(&request);

		// Assert - should use remote_addr since no trusted proxies
		assert_eq!(ip, "192.168.1.1");
	}

	#[tokio::test]
	async fn test_trusted_proxy_cidr_range() {
		// Arrange
		let config = RateLimitConfig::new(RateLimitStrategy::PerIp, 10.0, 1.0)
			.with_trusted_proxies(vec!["172.16.0.0/12".to_string()]);
		let middleware = RateLimitMiddleware::new(config);

		let mut headers = HeaderMap::new();
		headers.insert("X-Forwarded-For", "203.0.113.195".parse().unwrap());

		let mut request = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(headers)
			.body(Bytes::new())
			.build()
			.unwrap();
		request.remote_addr = Some("172.16.5.10:8080".parse().unwrap());

		// Act
		let ip = middleware.extract_client_ip(&request);

		// Assert - 172.16.5.10 is within 172.16.0.0/12
		assert_eq!(ip, "203.0.113.195");
	}

	#[tokio::test]
	async fn test_extract_user_id_from_string() {
		let config = RateLimitConfig::new(RateLimitStrategy::PerUser, 10.0, 1.0);
		let middleware = RateLimitMiddleware::new(config);

		let request = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		// Insert user ID as String
		request.extensions.insert("user123".to_string());

		let user_id = middleware.extract_user_id(&request);
		assert_eq!(user_id, Some("user123".to_string()));
	}

	#[tokio::test]
	async fn test_extract_user_id_from_i64() {
		let config = RateLimitConfig::new(RateLimitStrategy::PerUser, 10.0, 1.0);
		let middleware = RateLimitMiddleware::new(config);

		let request = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		// Insert user ID as i64
		request.extensions.insert(42i64);

		let user_id = middleware.extract_user_id(&request);
		assert_eq!(user_id, Some("42".to_string()));
	}

	#[tokio::test]
	async fn test_extract_user_id_anonymous() {
		let config = RateLimitConfig::new(RateLimitStrategy::PerUser, 10.0, 1.0);
		let middleware = RateLimitMiddleware::new(config);

		let request = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		let user_id = middleware.extract_user_id(&request);
		assert_eq!(user_id, None);
	}

	#[tokio::test]
	async fn test_generate_key_per_user_authenticated() {
		let config = RateLimitConfig::new(RateLimitStrategy::PerUser, 10.0, 1.0);
		let middleware = RateLimitMiddleware::new(config);

		let request = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		request.extensions.insert("user456".to_string());

		let key = middleware.generate_key(&request);
		assert_eq!(key, "user:user456");
	}

	#[tokio::test]
	async fn test_generate_key_per_user_anonymous() {
		let config = RateLimitConfig::new(RateLimitStrategy::PerUser, 10.0, 1.0);
		let middleware = RateLimitMiddleware::new(config);

		let request = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		let key = middleware.generate_key(&request);
		assert_eq!(key, "user:anonymous");
	}

	#[tokio::test]
	async fn test_generate_key_per_ip() {
		// Arrange
		let config = RateLimitConfig::new(RateLimitStrategy::PerIp, 10.0, 1.0)
			.with_trusted_proxies(vec!["10.0.0.1".to_string()]);
		let middleware = RateLimitMiddleware::new(config);

		let mut headers = HeaderMap::new();
		headers.insert("X-Forwarded-For", "203.0.113.195".parse().unwrap());

		let mut request = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(headers)
			.body(Bytes::new())
			.build()
			.unwrap();
		request.remote_addr = Some("10.0.0.1:8080".parse().unwrap());

		// Act
		let key = middleware.generate_key(&request);

		// Assert
		assert_eq!(key, "ip:203.0.113.195");
	}

	#[tokio::test]
	async fn test_rate_limit_per_ip_different_ips() {
		// Arrange
		let config = RateLimitConfig::new(RateLimitStrategy::PerIp, 1.0, 0.1)
			.with_trusted_proxies(vec!["10.0.0.1".to_string()]);
		let middleware = Arc::new(RateLimitMiddleware::new(config));
		let handler = Arc::new(TestHandler::new(StatusCode::OK));

		// Act - first request from IP1 via trusted proxy
		let mut headers1 = HeaderMap::new();
		headers1.insert("X-Forwarded-For", "203.0.113.1".parse().unwrap());
		let mut request1 = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(headers1)
			.body(Bytes::new())
			.build()
			.unwrap();
		request1.remote_addr = Some("10.0.0.1:8080".parse().unwrap());
		let response1 = middleware.process(request1, handler.clone()).await.unwrap();

		// Assert
		assert_eq!(response1.status, StatusCode::OK);

		// Act - first request from IP2 via trusted proxy (different bucket)
		let mut headers2 = HeaderMap::new();
		headers2.insert("X-Forwarded-For", "203.0.113.2".parse().unwrap());
		let mut request2 = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(headers2)
			.body(Bytes::new())
			.build()
			.unwrap();
		request2.remote_addr = Some("10.0.0.1:8080".parse().unwrap());
		let response2 = middleware.process(request2, handler).await.unwrap();

		// Assert
		assert_eq!(response2.status, StatusCode::OK);
	}

	#[tokio::test]
	async fn test_rate_limit_per_user_different_users() {
		let config = RateLimitConfig::new(RateLimitStrategy::PerUser, 1.0, 0.1);
		let middleware = Arc::new(RateLimitMiddleware::new(config));
		let handler = Arc::new(TestHandler::new(StatusCode::OK));

		// First request from user1
		let request1 = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();
		request1.extensions.insert("user1".to_string());
		let response1 = middleware.process(request1, handler.clone()).await.unwrap();
		assert_eq!(response1.status, StatusCode::OK);

		// First request from user2 should also succeed (different bucket)
		let request2 = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();
		request2.extensions.insert("user2".to_string());
		let response2 = middleware.process(request2, handler).await.unwrap();
		assert_eq!(response2.status, StatusCode::OK);
	}
}
