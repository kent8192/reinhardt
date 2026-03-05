//! Shared test fixtures for reinhardt-middleware tests
//!
//! This module provides reusable fixtures for testing middleware components.
//! All fixtures are designed to work with rstest and can be composed together.
//!
//! Note: Some fixtures require specific features to be enabled:
//! - `rate-limit` - RateLimit middleware fixtures
//!
//! Run with all features: `cargo test --features full`

// Allow dead code in test fixtures module: these utility functions and handlers
// are intentionally provided for test scenarios across multiple test files.
// Not all utilities are used in every test file.
#![allow(dead_code)]
// Allow unreachable_pub: this is a test module where pub items are accessed
// by other test files through mod fixtures; the items are reachable within tests.
#![allow(unreachable_pub)]

use async_trait::async_trait;
use bytes::Bytes;
use reinhardt_core::exception::Result;
use reinhardt_http::{Handler, Middleware};
use reinhardt_http::{Request, Response};
use reinhardt_middleware::cache::{CacheConfig, CacheKeyStrategy, CacheMiddleware};
pub use reinhardt_middleware::circuit_breaker::CircuitState;
use reinhardt_middleware::circuit_breaker::{CircuitBreakerConfig, CircuitBreakerMiddleware};
#[cfg(feature = "rate-limit")]
pub use reinhardt_middleware::rate_limit::RateLimitStrategy;
#[cfg(feature = "rate-limit")]
use reinhardt_middleware::rate_limit::{RateLimitConfig, RateLimitMiddleware};
use reinhardt_middleware::session::{SessionConfig, SessionMiddleware};
use rstest::fixture;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

// ============================================================================
// Configurable Test Handler
// ============================================================================

/// A test handler that can simulate different success/failure patterns.
///
/// This handler is useful for testing middleware behavior under various conditions:
/// - Always success
/// - Always failure
/// - Alternating success/failure
/// - Custom patterns
pub struct ConfigurableTestHandler {
	/// Counter for tracking request count
	pub request_count: AtomicU64,
	/// Pattern for responses: true = success (200), false = failure (500)
	pub response_pattern: Vec<bool>,
	/// Status code for successful responses
	pub success_status: u16,
	/// Status code for failed responses
	pub failure_status: u16,
	/// Optional response body
	pub response_body: Option<Bytes>,
	/// Optional delay before responding
	pub delay: Option<Duration>,
	/// Optional Content-Type header for response
	pub content_type: Option<String>,
}

impl ConfigurableTestHandler {
	/// Creates a new handler with the given pattern.
	pub fn new(pattern: Vec<bool>) -> Self {
		Self {
			request_count: AtomicU64::new(0),
			response_pattern: pattern,
			success_status: 200,
			failure_status: 500,
			response_body: None,
			delay: None,
			content_type: None,
		}
	}

	/// Creates a handler that always succeeds.
	pub fn always_success() -> Self {
		Self::new(vec![true])
	}

	/// Creates a handler that always fails.
	pub fn always_failure() -> Self {
		Self::new(vec![false])
	}

	/// Creates a handler that alternates between success and failure.
	pub fn alternating() -> Self {
		Self::new(vec![true, false])
	}

	/// Creates a handler that fails after N successful requests.
	pub fn fail_after(n: usize) -> Self {
		let mut pattern = vec![true; n];
		pattern.push(false);
		Self::new(pattern)
	}

	/// Creates a handler that succeeds after N failed requests.
	pub fn succeed_after(n: usize) -> Self {
		let mut pattern = vec![false; n];
		pattern.push(true);
		Self::new(pattern)
	}

	/// Sets the success status code.
	pub fn with_success_status(mut self, status: u16) -> Self {
		self.success_status = status;
		self
	}

	/// Sets the failure status code.
	pub fn with_failure_status(mut self, status: u16) -> Self {
		self.failure_status = status;
		self
	}

	/// Sets the response body.
	pub fn with_body(mut self, body: impl Into<Bytes>) -> Self {
		self.response_body = Some(body.into());
		self
	}

	/// Creates a handler that returns a specific body string.
	pub fn with_body_string(body: &str) -> Self {
		let mut handler = Self::always_success();
		handler.response_body = Some(Bytes::from(body.to_string()));
		handler
	}

	/// Sets the delay before responding (builder pattern).
	pub fn with_delay(mut self, delay: Duration) -> Self {
		self.delay = Some(delay);
		self
	}

	/// Creates a handler that returns a specific Content-Type.
	/// Uses a larger response body to enable meaningful compression testing.
	pub fn with_content_type(content_type: &str) -> Self {
		let mut handler = Self::always_success();
		handler.content_type = Some(content_type.to_string());
		// Large enough body to compress effectively (gzip adds ~20 bytes overhead)
		let long_body =
			"This is a test response body that should be long enough to compress. ".repeat(10);
		handler.response_body = Some(Bytes::from(long_body));
		handler
	}

	/// Creates a handler that returns a specific status code.
	pub fn with_status_code(status_code: u16) -> Self {
		let mut handler = Self::new(vec![true]);
		handler.success_status = status_code;
		// 204 No Content should have no body
		if status_code >= 200 && status_code < 300 && status_code != 204 {
			handler.response_body = Some(Bytes::from("success body"));
		}
		handler
	}

	/// Returns the current request count.
	pub fn count(&self) -> u64 {
		self.request_count.load(Ordering::SeqCst)
	}

	/// Resets the request count to zero.
	pub fn reset_count(&self) {
		self.request_count.store(0, Ordering::SeqCst);
	}
}

#[async_trait]
impl Handler for ConfigurableTestHandler {
	async fn handle(&self, _request: Request) -> Result<Response> {
		// Apply delay if configured
		if let Some(delay) = self.delay {
			tokio::time::sleep(delay).await;
		}

		let count = self.request_count.fetch_add(1, Ordering::SeqCst);
		let pattern_index = count as usize % self.response_pattern.len();
		let is_success = self.response_pattern[pattern_index];

		let status = if is_success {
			self.success_status
		} else {
			self.failure_status
		};

		let body = self
			.response_body
			.clone()
			.unwrap_or_else(|| Bytes::from(""));

		let mut response =
			Response::new(hyper::StatusCode::from_u16(status).unwrap()).with_body(body);

		// Add Content-Type header if configured
		if let Some(ref content_type) = self.content_type {
			response
				.headers
				.insert("Content-Type", content_type.parse().unwrap());
		}

		Ok(response)
	}
}

// ============================================================================
// Sized Response Handler
// ============================================================================

/// A handler that returns a response with a specific body size.
///
/// Useful for testing compression and body size-related middleware.
pub struct SizedResponseHandler {
	/// The size of the response body in bytes
	body_size: usize,
	/// The Content-Type of the response
	content_type: String,
}

impl SizedResponseHandler {
	/// Creates a new handler with the specified body size and content type.
	pub fn new(body_size: usize, content_type: &str) -> Self {
		Self {
			body_size,
			content_type: content_type.to_string(),
		}
	}
}

#[async_trait]
impl Handler for SizedResponseHandler {
	async fn handle(&self, _request: Request) -> Result<Response> {
		// Create a body with repetitive content for better compression testing
		let body = if self.body_size == 0 {
			Bytes::new()
		} else {
			let pattern = "x".repeat(100);
			let repeated = pattern.repeat(self.body_size / 100 + 1);
			Bytes::from(repeated[..self.body_size].to_string())
		};

		let mut response = Response::ok().with_body(body);
		response
			.headers
			.insert("Content-Type", self.content_type.parse().unwrap());

		Ok(response)
	}
}

// ============================================================================
// Echo Handler
// ============================================================================

/// A handler that echoes back request information.
///
/// Useful for testing request modification by middleware.
pub struct EchoHandler;

#[async_trait]
impl Handler for EchoHandler {
	async fn handle(&self, request: Request) -> Result<Response> {
		let body = format!(
			"Method: {}\nPath: {}\nHeaders: {:?}",
			request.method, request.uri, request.headers
		);
		Ok(Response::ok().with_body(Bytes::from(body)))
	}
}

// ============================================================================
// rstest Fixtures - CircuitBreaker
// ============================================================================

/// Fast timeout configuration for CircuitBreaker testing.
///
/// Uses short timeouts to speed up tests:
/// - 50% error threshold
/// - 5 minimum requests
/// - 100ms timeout
/// - 2 success threshold for half-open
#[fixture]
pub fn circuit_breaker_config_fast() -> CircuitBreakerConfig {
	CircuitBreakerConfig::new(0.5, 5, Duration::from_millis(100))
		.with_half_open_success_threshold(2)
}

/// Standard CircuitBreaker middleware with fast config.
#[fixture]
pub fn circuit_breaker_middleware(
	circuit_breaker_config_fast: CircuitBreakerConfig,
) -> Arc<CircuitBreakerMiddleware> {
	Arc::new(CircuitBreakerMiddleware::new(circuit_breaker_config_fast))
}

// ============================================================================
// rstest Fixtures - RateLimit
// ============================================================================

/// Strict rate limit configuration for boundary testing.
///
/// Uses small capacity for quick exhaustion:
/// - Per-IP strategy
/// - 10 token capacity
/// - 1 token/sec refill rate
/// - 1 token cost per request
#[cfg(feature = "rate-limit")]
#[fixture]
pub fn rate_limit_config_strict() -> RateLimitConfig {
	RateLimitConfig::new(RateLimitStrategy::PerIp, 10.0, 1.0).with_cost_per_request(1.0)
}

/// Standard RateLimit middleware with strict config.
#[cfg(feature = "rate-limit")]
#[fixture]
pub fn rate_limit_middleware(
	rate_limit_config_strict: RateLimitConfig,
) -> Arc<RateLimitMiddleware> {
	Arc::new(RateLimitMiddleware::new(rate_limit_config_strict))
}

// ============================================================================
// rstest Fixtures - Session
// ============================================================================

/// Fast session configuration for testing.
///
/// Uses short TTL for quick expiration:
/// - 500ms TTL
/// - "test_session" cookie name
#[fixture]
pub fn session_config_fast() -> SessionConfig {
	SessionConfig::new("test_session".to_string(), Duration::from_millis(500))
}

/// Standard Session middleware with fast config.
#[fixture]
pub fn session_middleware(session_config_fast: SessionConfig) -> Arc<SessionMiddleware> {
	Arc::new(SessionMiddleware::new(session_config_fast))
}

// ============================================================================
// rstest Fixtures - Cache
// ============================================================================

/// Fast cache configuration for testing.
///
/// Uses short TTL for quick expiration:
/// - 1 second TTL (Note: CacheEntry uses seconds precision, so sub-second TTL becomes 0)
/// - URL + Method key strategy
/// - 100 max entries
#[fixture]
pub fn cache_config_fast() -> CacheConfig {
	CacheConfig::new(Duration::from_secs(1), CacheKeyStrategy::UrlAndMethod).with_max_entries(100)
}

/// Standard Cache middleware with fast config.
#[fixture]
pub fn cache_middleware(cache_config_fast: CacheConfig) -> Arc<CacheMiddleware> {
	Arc::new(CacheMiddleware::new(cache_config_fast))
}

// ============================================================================
// rstest Fixtures - Test Handlers
// ============================================================================

/// A handler that always returns success.
#[fixture]
pub fn success_handler() -> Arc<ConfigurableTestHandler> {
	Arc::new(ConfigurableTestHandler::always_success())
}

/// A handler that always returns failure.
#[fixture]
pub fn failure_handler() -> Arc<ConfigurableTestHandler> {
	Arc::new(ConfigurableTestHandler::always_failure())
}

/// A handler that alternates between success and failure.
#[fixture]
pub fn alternating_handler() -> Arc<ConfigurableTestHandler> {
	Arc::new(ConfigurableTestHandler::alternating())
}

/// An echo handler for request inspection.
#[fixture]
pub fn echo_handler() -> Arc<EchoHandler> {
	Arc::new(EchoHandler)
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Creates a simple test request with default method (GET) and path (/).
pub fn create_request() -> Request {
	create_test_request("GET", "/")
}

/// Creates a test request with the given method.
pub fn create_request_with_method(method: &str) -> Request {
	create_test_request(method, "/")
}

/// Creates a test request with the given path.
pub fn create_request_with_path(path: &str) -> Request {
	create_test_request("GET", path)
}

/// Creates a test request with the given method and path.
pub fn create_test_request(method: &str, path: &str) -> Request {
	Request::builder()
		.method(method.parse().unwrap())
		.uri(path)
		.build()
		.unwrap()
}

/// Creates a test request with custom headers.
///
/// Note: Headers are converted to owned values internally.
pub fn create_request_with_headers(method: &str, path: &str, headers: &[(&str, &str)]) -> Request {
	use http::header::HeaderName;

	let mut request = Request::builder()
		.method(method.parse().unwrap())
		.uri(path)
		.build()
		.unwrap();

	for (name, value) in headers {
		let header_name =
			HeaderName::from_bytes(name.to_lowercase().as_bytes()).expect("Invalid header name");
		request
			.headers
			.insert(header_name, (*value).parse().unwrap());
	}

	request
}

/// Creates a test request with a body.
pub fn create_request_with_body(method: &str, path: &str, body: impl Into<Bytes>) -> Request {
	Request::builder()
		.method(method.parse().unwrap())
		.uri(path)
		.body(body.into())
		.build()
		.unwrap()
}

/// Creates a test request with a remote address.
pub fn create_request_with_ip(method: &str, path: &str, ip: &str) -> Request {
	Request::builder()
		.method(method.parse().unwrap())
		.uri(path)
		.remote_addr(ip.parse().unwrap())
		.build()
		.unwrap()
}

/// Waits for the CircuitBreaker to reach the expected state.
///
/// Returns true if the state was reached within the timeout, false otherwise.
pub async fn wait_for_circuit_state(
	middleware: &CircuitBreakerMiddleware,
	expected_state: CircuitState,
	timeout: Duration,
) -> bool {
	let start = std::time::Instant::now();
	while start.elapsed() < timeout {
		if middleware.state() == expected_state {
			return true;
		}
		tokio::time::sleep(Duration::from_millis(10)).await;
	}
	false
}

/// Sends multiple concurrent requests to a handler.
///
/// Returns a vector of responses in arbitrary order.
pub async fn send_concurrent_requests<H: Handler + 'static>(
	count: usize,
	handler: Arc<H>,
	request_factory: impl Fn() -> Request + Send + Sync + 'static,
) -> Vec<Response> {
	let request_factory = Arc::new(request_factory);
	let handles: Vec<_> = (0..count)
		.map(|_| {
			let factory = request_factory.clone();
			let h = handler.clone();
			tokio::spawn(async move {
				let request = factory();
				h.handle(request).await
			})
		})
		.collect();

	let mut responses = Vec::with_capacity(count);
	for handle in handles {
		if let Ok(Ok(response)) = handle.await {
			responses.push(response);
		}
	}
	responses
}

/// Sends multiple sequential requests to a middleware.
///
/// Returns a vector of (request_index, response) tuples.
pub async fn send_sequential_requests<M: Middleware + 'static, H: Handler + 'static>(
	count: usize,
	middleware: Arc<M>,
	handler: Arc<H>,
	request_factory: impl Fn(usize) -> Request,
) -> Vec<(usize, Response)> {
	let mut results = Vec::with_capacity(count);

	for i in 0..count {
		let request = request_factory(i);
		if let Ok(response) = middleware.process(request, handler.clone()).await {
			results.push((i, response));
		}
	}

	results
}

// ============================================================================
// Assertion Helpers
// ============================================================================

/// Asserts that a response has the expected status code.
pub fn assert_status(response: &Response, expected: u16) {
	assert_eq!(
		response.status.as_u16(),
		expected,
		"Expected status {}, got {}",
		expected,
		response.status.as_u16()
	);
}

/// Asserts that a response has a specific header with the expected value.
pub fn assert_header(response: &Response, name: &str, expected: &str) {
	let header_value = response
		.headers
		.get(name)
		.expect(&format!("Expected header '{}' not found", name))
		.to_str()
		.expect("Header value is not valid UTF-8");

	assert_eq!(
		header_value, expected,
		"Expected header '{}' to be '{}', got '{}'",
		name, expected, header_value
	);
}

/// Asserts that a response contains a specific header.
pub fn assert_has_header(response: &Response, name: &str) {
	assert!(
		response.headers.contains_key(name),
		"Expected header '{}' not found in response",
		name
	);
}

/// Asserts that a response does not contain a specific header.
pub fn assert_no_header(response: &Response, name: &str) {
	assert!(
		!response.headers.contains_key(name),
		"Expected header '{}' to be absent, but it was present",
		name
	);
}

/// Asserts that the response body contains the expected substring.
pub fn assert_body_contains(response: &Response, expected: &str) {
	let body_str = String::from_utf8_lossy(&response.body);
	assert!(
		body_str.contains(expected),
		"Expected body to contain '{}', got: '{}'",
		expected,
		body_str
	);
}

/// Asserts that the response body equals the expected value.
pub fn assert_body_eq(response: &Response, expected: &str) {
	let body_str = String::from_utf8_lossy(&response.body);
	assert_eq!(
		body_str, expected,
		"Expected body '{}', got '{}'",
		expected, body_str
	);
}

#[cfg(test)]
mod tests {
	use super::*;

	#[tokio::test]
	async fn test_configurable_handler_always_success() {
		let handler = ConfigurableTestHandler::always_success();
		let request = create_test_request("GET", "/test");

		let response = handler.handle(request).await.unwrap();

		assert_eq!(response.status.as_u16(), 200);
		assert_eq!(handler.count(), 1);
	}

	#[tokio::test]
	async fn test_configurable_handler_always_failure() {
		let handler = ConfigurableTestHandler::always_failure();
		let request = create_test_request("GET", "/test");

		let response = handler.handle(request).await.unwrap();

		assert_eq!(response.status.as_u16(), 500);
		assert_eq!(handler.count(), 1);
	}

	#[tokio::test]
	async fn test_configurable_handler_alternating() {
		let handler = ConfigurableTestHandler::alternating();

		// First request: success
		let request1 = create_test_request("GET", "/test");
		let response1 = handler.handle(request1).await.unwrap();
		assert_eq!(response1.status.as_u16(), 200);

		// Second request: failure
		let request2 = create_test_request("GET", "/test");
		let response2 = handler.handle(request2).await.unwrap();
		assert_eq!(response2.status.as_u16(), 500);

		// Third request: success (pattern repeats)
		let request3 = create_test_request("GET", "/test");
		let response3 = handler.handle(request3).await.unwrap();
		assert_eq!(response3.status.as_u16(), 200);

		assert_eq!(handler.count(), 3);
	}

	#[tokio::test]
	async fn test_configurable_handler_fail_after() {
		let handler = ConfigurableTestHandler::fail_after(3);

		// First 3 requests: success
		for _ in 0..3 {
			let request = create_test_request("GET", "/test");
			let response = handler.handle(request).await.unwrap();
			assert_eq!(response.status.as_u16(), 200);
		}

		// Fourth request: failure
		let request = create_test_request("GET", "/test");
		let response = handler.handle(request).await.unwrap();
		assert_eq!(response.status.as_u16(), 500);

		assert_eq!(handler.count(), 4);
	}

	#[tokio::test]
	async fn test_configurable_handler_with_delay() {
		let handler =
			ConfigurableTestHandler::always_success().with_delay(Duration::from_millis(50));

		let start = std::time::Instant::now();
		let request = create_test_request("GET", "/test");
		let _response = handler.handle(request).await.unwrap();
		let elapsed = start.elapsed();

		assert!(
			elapsed >= Duration::from_millis(50),
			"Expected delay of at least 50ms, got {:?}",
			elapsed
		);
	}

	#[tokio::test]
	async fn test_create_request_with_headers() {
		let request = create_request_with_headers(
			"POST",
			"/api/users",
			&[
				("Content-Type", "application/json"),
				("Authorization", "Bearer token123"),
			],
		);

		assert_eq!(request.method, hyper::Method::POST);
		assert_eq!(request.uri.path(), "/api/users");
		assert_eq!(
			request
				.headers
				.get("content-type")
				.unwrap()
				.to_str()
				.unwrap(),
			"application/json"
		);
		assert_eq!(
			request
				.headers
				.get("authorization")
				.unwrap()
				.to_str()
				.unwrap(),
			"Bearer token123"
		);
	}
}
