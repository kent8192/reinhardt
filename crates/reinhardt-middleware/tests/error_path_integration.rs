//! Error Path Integration Tests for reinhardt-middleware
//!
//! This module contains tests that verify middleware behavior when errors occur.
//! These tests ensure proper error handling, status codes, and error messages.
//!
//! Test categories:
//! - CSRF: Missing/invalid token errors
//! - RateLimit: Capacity exceeded (429)
//! - CircuitBreaker: Open state (503)
//! - Auth: Unauthenticated/inactive user (401)
//! - Timeout: Request timeout (408)
//! - Locale: Invalid Accept-Language fallback
//! - Compression: Fallback on failure

mod fixtures;

use fixtures::*;
use rstest::rstest;
use serial_test::serial;
use std::sync::Arc;

use reinhardt_http::Middleware;
use reinhardt_http::Request;
use reinhardt_middleware::csrf::CsrfMiddleware;
use reinhardt_middleware::locale::LocaleMiddleware;

#[cfg(feature = "rate-limit")]
use reinhardt_middleware::rate_limit::{RateLimitConfig, RateLimitMiddleware, RateLimitStrategy};

use reinhardt_middleware::circuit_breaker::{
	CircuitBreakerConfig, CircuitBreakerMiddleware, CircuitState,
};

use bytes::Bytes;
use hyper::Method;
use std::time::Duration;

// ============================================================================
// CSRF Middleware Error Tests
// ============================================================================

/// Test: POST without CSRF token returns error
#[rstest]
#[tokio::test]
#[serial(csrf)]
async fn test_csrf_post_without_token_returns_error() {
	// Setup: Create CSRF middleware
	let middleware = Arc::new(CsrfMiddleware::new());
	let handler = Arc::new(ConfigurableTestHandler::always_success());

	// Execute: Send POST request without CSRF token
	let request = create_test_request("POST", "/form/submit");
	let result = middleware.process(request, handler.clone()).await;

	// Verify: Request is rejected with an error
	assert!(result.is_err(), "CSRF validation should fail without token");
	// Handler should not be called
	assert_eq!(
		handler.count(),
		0,
		"Handler should not be called for CSRF error"
	);
}

/// Test: POST with invalid CSRF token returns error
#[rstest]
#[tokio::test]
#[serial(csrf)]
async fn test_csrf_post_with_invalid_token_returns_error() {
	// Setup: Create CSRF middleware
	let middleware = Arc::new(CsrfMiddleware::new());
	let handler = Arc::new(ConfigurableTestHandler::always_success());

	// Execute: Send POST request with invalid CSRF token
	let request = Request::builder()
		.method(Method::POST)
		.uri("/form/submit")
		.header("X-CSRF-Token", "invalid_token_12345")
		.header("Cookie", "csrftoken=different_token_67890")
		.header("Referer", "https://example.com/")
		.body(Bytes::new())
		.build()
		.unwrap();
	let result = middleware.process(request, handler.clone()).await;

	// Verify: Request is rejected with an error
	assert!(
		result.is_err(),
		"CSRF validation should fail with invalid token"
	);
	assert_eq!(
		handler.count(),
		0,
		"Handler should not be called for CSRF error"
	);
}

/// Test: POST with mismatched CSRF token (header vs cookie) returns error
#[rstest]
#[tokio::test]
#[serial(csrf)]
async fn test_csrf_post_with_mismatched_token_returns_error() {
	// Setup: Create CSRF middleware
	let middleware = Arc::new(CsrfMiddleware::new());
	let handler = Arc::new(ConfigurableTestHandler::always_success());

	// Execute: Send POST with mismatched token
	let request = Request::builder()
		.method(Method::POST)
		.uri("/api/data")
		.header("X-CSRF-Token", "token_a")
		.header("Cookie", "csrftoken=token_b")
		.header("Referer", "https://example.com/")
		.body(Bytes::new())
		.build()
		.unwrap();
	let result = middleware.process(request, handler.clone()).await;

	// Verify: Request is rejected with an error
	assert!(
		result.is_err(),
		"CSRF validation should fail with mismatched tokens"
	);
}

// ============================================================================
// RateLimit Middleware Error Tests
// ============================================================================

/// Test: RateLimit exceeded returns 429 Too Many Requests
#[cfg(feature = "rate-limit")]
#[rstest]
#[tokio::test]
#[serial(rate_limit)]
async fn test_rate_limit_exceeded_returns_429() {
	// Setup: Create very strict rate limiter (only 1 request allowed, very slow refill)
	// refill_rate must be > 0 to avoid Duration conversion error
	let config =
		RateLimitConfig::new(RateLimitStrategy::PerIp, 1.0, 0.001).with_cost_per_request(1.0);
	let middleware = Arc::new(RateLimitMiddleware::new(config));
	let handler = Arc::new(ConfigurableTestHandler::always_success());

	// Execute: First request should succeed
	let request1 = create_test_request("GET", "/api/data");
	let response1 = middleware.process(request1, handler.clone()).await.unwrap();
	assert_status(&response1, 200);

	// Execute: Second request should be rate limited
	let request2 = create_test_request("GET", "/api/data");
	let response2 = middleware.process(request2, handler.clone()).await.unwrap();

	// Verify: Request is rejected with 429
	assert_status(&response2, 429);
}

/// Test: RateLimit adds Retry-After header when exceeded
#[cfg(feature = "rate-limit")]
#[rstest]
#[tokio::test]
#[serial(rate_limit)]
async fn test_rate_limit_adds_retry_after_header() {
	// Setup: Create very strict rate limiter
	let config =
		RateLimitConfig::new(RateLimitStrategy::PerIp, 1.0, 1.0).with_cost_per_request(1.0);
	let middleware = Arc::new(RateLimitMiddleware::new(config));
	let handler = Arc::new(ConfigurableTestHandler::always_success());

	// Exhaust the rate limit
	let request1 = create_test_request("GET", "/api/data");
	let _ = middleware.process(request1, handler.clone()).await.unwrap();

	// Execute: Send another request
	let request2 = create_test_request("GET", "/api/data");
	let response2 = middleware.process(request2, handler.clone()).await.unwrap();

	// Verify: Response has Retry-After header
	assert_status(&response2, 429);
	assert!(
		response2.headers.get("retry-after").is_some(),
		"Response should have Retry-After header"
	);
}

/// Test: RateLimit per-user isolation
#[cfg(feature = "rate-limit")]
#[rstest]
#[tokio::test]
#[serial(rate_limit)]
async fn test_rate_limit_per_user_isolation() {
	// Setup: Create per-user rate limiter (refill_rate must be > 0)
	let config =
		RateLimitConfig::new(RateLimitStrategy::PerUser, 1.0, 0.001).with_cost_per_request(1.0);
	let middleware = Arc::new(RateLimitMiddleware::new(config));
	let handler = Arc::new(ConfigurableTestHandler::always_success());

	// Note: PerUser strategy extracts user ID from request.extensions,
	// not from headers. We need to set the user ID in extensions.

	// Execute: Request from user 1 (using extensions)
	let request1 = Request::builder()
		.method(Method::GET)
		.uri("/api/data")
		.body(Bytes::new())
		.build()
		.unwrap();
	request1.extensions.insert("user_1".to_string());
	let _ = middleware.process(request1, handler.clone()).await.unwrap();

	// User 1 is now rate limited
	let request2 = Request::builder()
		.method(Method::GET)
		.uri("/api/data")
		.body(Bytes::new())
		.build()
		.unwrap();
	request2.extensions.insert("user_1".to_string());
	let response2 = middleware.process(request2, handler.clone()).await.unwrap();
	assert_status(&response2, 429);

	// But user 2 should still be allowed
	let request3 = Request::builder()
		.method(Method::GET)
		.uri("/api/data")
		.body(Bytes::new())
		.build()
		.unwrap();
	request3.extensions.insert("user_2".to_string());
	let response3 = middleware.process(request3, handler.clone()).await.unwrap();

	// Verify: User 2 is not rate limited
	assert_status(&response3, 200);
}

// ============================================================================
// CircuitBreaker Middleware Error Tests
// ============================================================================

/// Test: CircuitBreaker Open state returns 503
#[rstest]
#[tokio::test]
#[serial(circuit_breaker)]
async fn test_circuit_breaker_open_state_returns_503() {
	// Setup: Create circuit breaker with very low threshold
	// new(error_threshold, min_requests, timeout)
	let config = CircuitBreakerConfig::new(0.5, 2, Duration::from_secs(60))
		.with_half_open_success_threshold(2);
	let middleware = Arc::new(CircuitBreakerMiddleware::new(config));

	// Create handlers
	let failure_handler = Arc::new(ConfigurableTestHandler::always_failure());

	// Force failures to open the circuit
	for _ in 0..3 {
		let request = create_test_request("GET", "/api/data");
		let _ = middleware.process(request, failure_handler.clone()).await;
	}

	// Verify circuit is now open
	assert_eq!(
		middleware.state(),
		CircuitState::Open,
		"Circuit should be open after failures"
	);

	// Execute: Send request when circuit is open
	let success_handler = Arc::new(ConfigurableTestHandler::always_success());
	let request = create_test_request("GET", "/api/data");
	let response = middleware
		.process(request, success_handler.clone())
		.await
		.unwrap();

	// Verify: Request is rejected with 503
	assert_status(&response, 503);
	// Handler should not be called when circuit is open
	assert_eq!(
		success_handler.count(),
		0,
		"Handler should not be called when circuit is open"
	);
}

/// Test: CircuitBreaker failure increments error count
#[rstest]
#[tokio::test]
#[serial(circuit_breaker)]
async fn test_circuit_breaker_counts_failures() {
	// Setup: Create circuit breaker
	// new(error_threshold, min_requests, timeout)
	let config = CircuitBreakerConfig::new(0.5, 5, Duration::from_secs(60));
	let middleware = Arc::new(CircuitBreakerMiddleware::new(config));
	let failure_handler = Arc::new(ConfigurableTestHandler::always_failure());

	// Execute: Send requests that fail
	for _ in 0..2 {
		let request = create_test_request("GET", "/api/data");
		let _ = middleware.process(request, failure_handler.clone()).await;
	}

	// Verify: Circuit should still be closed (min_requests not met)
	assert_eq!(
		middleware.state(),
		CircuitState::Closed,
		"Circuit should still be closed (min_requests not met)"
	);

	// Continue failures to reach min_requests
	for _ in 0..3 {
		let request = create_test_request("GET", "/api/data");
		let _ = middleware.process(request, failure_handler.clone()).await;
	}

	// Verify: Circuit should now be open
	assert_eq!(
		middleware.state(),
		CircuitState::Open,
		"Circuit should be open after exceeding threshold"
	);
}

// ============================================================================
// Locale Middleware Error Tests
// ============================================================================

/// Test: Invalid Accept-Language header falls back to default
#[rstest]
#[tokio::test]
#[serial(locale)]
async fn test_locale_invalid_accept_language_fallback() {
	// Setup: Create locale middleware with default locale
	let middleware = Arc::new(LocaleMiddleware::new());
	let handler = Arc::new(ConfigurableTestHandler::always_success());

	// Execute: Send request with invalid Accept-Language
	let request = Request::builder()
		.method(Method::GET)
		.uri("/page")
		.header("Accept-Language", "invalid-locale-format")
		.body(Bytes::new())
		.build()
		.unwrap();
	let response = middleware.process(request, handler.clone()).await.unwrap();

	// Verify: Request is processed (fallback to default)
	assert_status(&response, 200);
}

/// Test: Empty Accept-Language uses default locale
#[rstest]
#[tokio::test]
#[serial(locale)]
async fn test_locale_empty_accept_language_uses_default() {
	// Setup: Create locale middleware
	let middleware = Arc::new(LocaleMiddleware::new());
	let handler = Arc::new(ConfigurableTestHandler::always_success());

	// Execute: Send request without Accept-Language
	let request = create_test_request("GET", "/page");
	let response = middleware.process(request, handler.clone()).await.unwrap();

	// Verify: Request is processed with default locale
	assert_status(&response, 200);
}

/// Test: Unsupported locale falls back to default
#[rstest]
#[tokio::test]
#[serial(locale)]
async fn test_locale_unsupported_locale_fallback() {
	// Setup: Create locale middleware (default might only support common locales)
	let middleware = Arc::new(LocaleMiddleware::new());
	let handler = Arc::new(ConfigurableTestHandler::always_success());

	// Execute: Send request with very obscure locale
	let request = Request::builder()
		.method(Method::GET)
		.uri("/page")
		.header("Accept-Language", "x-klingon")
		.body(Bytes::new())
		.build()
		.unwrap();
	let response = middleware.process(request, handler.clone()).await.unwrap();

	// Verify: Request is processed (falls back gracefully)
	assert_status(&response, 200);
}

// ============================================================================
// Handler Error Propagation Tests
// ============================================================================

/// Test: Middleware propagates handler errors correctly
#[rstest]
#[tokio::test]
#[serial(error_propagation)]
async fn test_middleware_propagates_handler_error_status() {
	// Setup: Create middleware chain with a failing handler
	let middleware = Arc::new(LocaleMiddleware::new());
	let handler = Arc::new(ConfigurableTestHandler::always_failure());

	// Execute: Send request
	let request = create_test_request("GET", "/api/data");
	let response = middleware.process(request, handler).await.unwrap();

	// Verify: Error status is propagated
	assert_status(&response, 500);
}

/// Test: Middleware chain with multiple middlewares handles errors
#[rstest]
#[tokio::test]
#[serial(error_chain)]
async fn test_middleware_chain_error_handling() {
	use reinhardt_middleware::csp::CspMiddleware;

	// Setup: Create a middleware that wraps another
	let inner_middleware = Arc::new(CspMiddleware::new());
	let outer_middleware = Arc::new(LocaleMiddleware::new());
	let failure_handler = Arc::new(ConfigurableTestHandler::always_failure());

	// Create a composite handler that applies inner middleware first
	struct MiddlewareHandler {
		middleware: Arc<dyn Middleware>,
		handler: Arc<dyn reinhardt_http::Handler>,
	}

	#[async_trait::async_trait]
	impl reinhardt_http::Handler for MiddlewareHandler {
		async fn handle(
			&self,
			request: Request,
		) -> reinhardt_core::exception::Result<reinhardt_http::Response> {
			self.middleware.process(request, self.handler.clone()).await
		}
	}

	let composite_handler = Arc::new(MiddlewareHandler {
		middleware: inner_middleware,
		handler: failure_handler,
	});

	// Execute: Send request through chain
	let request = create_test_request("GET", "/api/data");
	let response = outer_middleware
		.process(request, composite_handler)
		.await
		.unwrap();

	// Verify: Error status is propagated through chain
	assert_status(&response, 500);
}

// ============================================================================
// Cache Error Tests
// ============================================================================

/// Test: Cache does not cache error responses
#[rstest]
#[tokio::test]
#[serial(cache)]
async fn test_cache_does_not_cache_error_responses(cache_middleware: Arc<CacheMiddleware>) {
	// Setup: Handler that returns 500
	let failure_handler = Arc::new(ConfigurableTestHandler::always_failure());

	// Execute: First request (500 error)
	let request1 = create_test_request("GET", "/api/error");
	let response1 = cache_middleware
		.process(request1, failure_handler.clone())
		.await
		.unwrap();
	assert_status(&response1, 500);

	// Execute: Second request - should also hit handler (not cached)
	let success_handler = Arc::new(ConfigurableTestHandler::always_success());
	let request2 = create_test_request("GET", "/api/error");
	let response2 = cache_middleware
		.process(request2, success_handler.clone())
		.await
		.unwrap();

	// Verify: Second request hit the new handler (200), not cached 500
	assert_status(&response2, 200);
	assert_eq!(
		success_handler.count(),
		1,
		"Handler should be called (error not cached)"
	);
}

/// Test: Cache does not cache non-GET requests
#[rstest]
#[tokio::test]
#[serial(cache)]
async fn test_cache_does_not_cache_post_requests(cache_middleware: Arc<CacheMiddleware>) {
	let handler = Arc::new(ConfigurableTestHandler::always_success());

	// Execute: First POST request
	let request1 = Request::builder()
		.method(Method::POST)
		.uri("/api/data")
		.body(Bytes::new())
		.build()
		.unwrap();
	let _ = cache_middleware
		.process(request1, handler.clone())
		.await
		.unwrap();

	// Execute: Second POST request
	let request2 = Request::builder()
		.method(Method::POST)
		.uri("/api/data")
		.body(Bytes::new())
		.build()
		.unwrap();
	let _ = cache_middleware
		.process(request2, handler.clone())
		.await
		.unwrap();

	// Verify: Both requests hit the handler (no caching for POST)
	assert_eq!(
		handler.count(),
		2,
		"POST requests should not be cached - handler should be called twice"
	);
}

use reinhardt_middleware::cache::CacheMiddleware;
