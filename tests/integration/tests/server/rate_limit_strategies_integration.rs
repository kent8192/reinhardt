//! Integration tests for rate limiting strategies
//!
//! Tests the rate limiter middleware with various scenarios including:
//! - Fixed window rate limiting
//! - Independent client rate limits
//! - Retry-After header handling
//! - Window reset behavior
//! - Rate limit with timeout combination

use http::StatusCode;
use reinhardt_http::Handler;
use reinhardt_http::ViewResult;
use reinhardt_http::{Request, Response};
use reinhardt_macros::get;
use reinhardt_server::{RateLimitConfig, RateLimitHandler, RateLimitStrategy};
use reinhardt_test::APIClient;
use reinhardt_test::fixtures::*;
use reinhardt_urls::routers::ServerRouter as Router;
use std::sync::Arc;
use std::time::Duration;

// ============================================================================
// Test Handlers
// ============================================================================

/// Basic test handler that returns success
#[get("/test", name = "test")]
async fn test_handler() -> ViewResult<Response> {
	Ok(Response::ok().with_body("Success"))
}

/// Slow handler for timeout testing
#[derive(Clone)]
struct SlowHandler {
	delay: Duration,
}

#[async_trait::async_trait]
impl Handler for SlowHandler {
	async fn handle(&self, _request: Request) -> reinhardt_core::exception::Result<Response> {
		tokio::time::sleep(self.delay).await;
		Ok(Response::ok().with_body("Completed"))
	}
}

// ============================================================================
// Test Cases
// ============================================================================

/// Test 1: Fixed window rate limiting
///
/// This test verifies that the FixedWindow strategy correctly limits requests
/// within a time window and rejects requests that exceed the limit.
#[tokio::test]
async fn test_fixed_window_rate_limit() {
	// Configure rate limit: 3 requests per 1 second window
	let router = Arc::new(Router::new().endpoint(test_handler));
	let config = RateLimitConfig::new(3, Duration::from_secs(1), RateLimitStrategy::FixedWindow);
	let rate_limit_handler = Arc::new(RateLimitHandler::new(router, config));

	let server = TestServer::builder()
		.handler(rate_limit_handler)
		.build()
		.await
		.expect("Failed to create server with rate limit");

	let client = APIClient::with_base_url(&server.url);

	// First 3 requests should succeed
	for i in 1..=3 {
		let response = client.get("/test").await.expect("Failed to send request");

		assert_eq!(
			response.status(),
			StatusCode::OK,
			"Request {} should succeed",
			i
		);
		assert_eq!(
			response.text(),
			"Success",
			"Request {} should return success body",
			i
		);
	}

	// 4th request should be rate limited
	let response = client.get("/test").await.expect("Failed to send request");

	assert_eq!(
		response.status(),
		StatusCode::TOO_MANY_REQUESTS,
		"4th request should be rate limited"
	);
	assert_eq!(
		response.text(),
		"Rate limit exceeded",
		"Rate limited response should have correct body"
	);
}

/// Test 2: Independent client rate limits
///
/// Verifies that rate limits are tracked independently per client IP address.
/// Different clients should have separate rate limit counters.
#[tokio::test]
async fn test_independent_client_rate_limits() {
	// Configure rate limit: 2 requests per window
	// Trust 127.0.0.1 (test server's loopback address) so X-Forwarded-For headers are honored
	let router = Arc::new(Router::new().endpoint(test_handler));
	let config = RateLimitConfig::new(2, Duration::from_secs(1), RateLimitStrategy::FixedWindow)
		.with_trusted_proxies(vec!["127.0.0.1".to_string()]);
	let rate_limit_handler = Arc::new(RateLimitHandler::new(router, config));

	let server = TestServer::builder()
		.handler(rate_limit_handler)
		.build()
		.await
		.expect("Failed to create server with rate limit");

	let client1 = APIClient::with_base_url(&server.url);
	let client2 = APIClient::with_base_url(&server.url);

	// Client 1: Use up its limit (2 requests)
	for i in 1..=2 {
		let response = client1
			.get_with_headers("/test", &[("X-Forwarded-For", "192.168.1.100")])
			.await
			.expect("Failed to send request");

		assert_eq!(
			response.status(),
			StatusCode::OK,
			"Client 1 request {} should succeed",
			i
		);
	}

	// Client 1: 3rd request should be rate limited
	let response = client1
		.get_with_headers("/test", &[("X-Forwarded-For", "192.168.1.100")])
		.await
		.expect("Failed to send request");

	assert_eq!(
		response.status(),
		StatusCode::TOO_MANY_REQUESTS,
		"Client 1 should be rate limited"
	);

	// Client 2: Should still be able to make requests (independent limit)
	let response = client2
		.get_with_headers("/test", &[("X-Forwarded-For", "192.168.1.200")])
		.await
		.expect("Failed to send request");

	assert_eq!(
		response.status(),
		StatusCode::OK,
		"Client 2 should not be rate limited (independent counter)"
	);
}

/// Test 3: Retry-After header
///
/// Verifies that the rate limiter does not include Retry-After header by default.
/// This test documents the current behavior - if Retry-After header support is
/// added in the future, this test should be updated accordingly.
#[tokio::test]
async fn test_retry_after_header() {
	// Configure rate limit: 1 request per window
	let router = Arc::new(Router::new().endpoint(test_handler));
	let config = RateLimitConfig::new(1, Duration::from_secs(60), RateLimitStrategy::FixedWindow);
	let rate_limit_handler = Arc::new(RateLimitHandler::new(router, config));

	let server = TestServer::builder()
		.handler(rate_limit_handler)
		.build()
		.await
		.expect("Failed to create server with rate limit");

	let client = APIClient::with_base_url(&server.url);

	// First request succeeds
	let response = client.get("/test").await.expect("Failed to send request");

	assert_eq!(response.status(), StatusCode::OK);

	// Second request should be rate limited
	let response = client.get("/test").await.expect("Failed to send request");

	assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);

	// Currently, RateLimitHandler does not include Retry-After header
	// This test documents the current behavior
	assert!(
		response.headers().get("Retry-After").is_none(),
		"Retry-After header is not currently implemented"
	);
}

/// Test 4: Window reset
///
/// Verifies that rate limit counters are reset after the time window expires.
/// After the window resets, clients should be able to make new requests.
#[tokio::test]
async fn test_window_reset() {
	// Configure rate limit: 2 requests per 500ms window
	let router = Arc::new(Router::new().endpoint(test_handler));
	let config = RateLimitConfig::new(
		2,
		Duration::from_millis(500),
		RateLimitStrategy::FixedWindow,
	);
	let rate_limit_handler = Arc::new(RateLimitHandler::new(router, config));

	let server = TestServer::builder()
		.handler(rate_limit_handler)
		.build()
		.await
		.expect("Failed to create server with rate limit");

	let client = APIClient::with_base_url(&server.url);

	// Use up the limit (2 requests)
	for i in 1..=2 {
		let response = client.get("/test").await.expect("Failed to send request");

		assert_eq!(
			response.status(),
			StatusCode::OK,
			"Request {} should succeed",
			i
		);
	}

	// 3rd request should be rate limited
	let response = client.get("/test").await.expect("Failed to send request");

	assert_eq!(
		response.status(),
		StatusCode::TOO_MANY_REQUESTS,
		"3rd request should be rate limited"
	);

	// Wait for window to reset (600ms to ensure complete reset)
	tokio::time::sleep(Duration::from_millis(600)).await;

	// After window reset, requests should succeed again
	let response = client.get("/test").await.expect("Failed to send request");

	assert_eq!(
		response.status(),
		StatusCode::OK,
		"Request after window reset should succeed"
	);
	assert_eq!(
		response.text(),
		"Success",
		"Request after reset should return success body"
	);
}

/// Test 5: Rate limit with timeout
///
/// Verifies that rate limiting works correctly when combined with timeout middleware.
/// The middleware chain should handle both rate limiting and timeouts properly.
#[tokio::test]
async fn test_rate_limit_with_timeout() {
	use reinhardt_server::TimeoutHandler;

	// Create slow handler (delays for 2 seconds)
	let slow_handler: Arc<dyn Handler> = Arc::new(SlowHandler {
		delay: Duration::from_secs(2),
	});

	// Wrap with timeout (1 second timeout)
	let timeout_handler: Arc<dyn Handler> =
		Arc::new(TimeoutHandler::new(slow_handler, Duration::from_secs(1)));

	// Wrap with rate limit (5 requests per minute)
	let config = RateLimitConfig::per_minute(5);
	let rate_limit_handler = Arc::new(RateLimitHandler::new(timeout_handler, config));

	let server = TestServer::builder()
		.handler(rate_limit_handler)
		.build()
		.await
		.expect("Failed to create server with middleware chain");

	// APIClient uses reqwest internally which has no default timeout
	// Server timeout (1s) is shorter than handler delay (2s), so server will timeout
	let client = APIClient::with_base_url(&server.url);

	// First request should timeout (slow handler takes 2s, timeout is 1s)
	let response = client.get("/").await.expect("Failed to send request");

	assert_eq!(
		response.status(),
		StatusCode::REQUEST_TIMEOUT,
		"Request should timeout"
	);

	// Second request should also timeout (not rate limited yet, only 1 request made)
	let response = client.get("/").await.expect("Failed to send request");

	assert_eq!(
		response.status(),
		StatusCode::REQUEST_TIMEOUT,
		"Second request should also timeout"
	);

	// Make 3 more requests to approach rate limit (total 5 requests)
	for i in 3..=5 {
		let response = client.get("/").await.expect("Failed to send request");

		assert_eq!(
			response.status(),
			StatusCode::REQUEST_TIMEOUT,
			"Request {} should timeout",
			i
		);
	}

	// 6th request should be rate limited (not timeout)
	let response = client.get("/").await.expect("Failed to send request");

	assert_eq!(
		response.status(),
		StatusCode::TOO_MANY_REQUESTS,
		"6th request should be rate limited, not timeout"
	);
	assert_eq!(
		response.text(),
		"Rate limit exceeded",
		"Rate limited response should have correct body"
	);
}
