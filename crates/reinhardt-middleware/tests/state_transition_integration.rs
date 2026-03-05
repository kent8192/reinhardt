//! State Transition Integration Tests for reinhardt-middleware
//!
//! This module tests state machine transitions in middleware components:
//!
//! ## CircuitBreaker States
//! - Closed: Normal operation, requests pass through
//! - Open: Circuit tripped, requests rejected immediately
//! - HalfOpen: Testing if service recovered
//!
//! ## RateLimit States
//! - Allow: Under capacity, requests allowed
//! - Throttle: Near capacity, requests may be delayed
//! - Block: Over capacity, requests rejected
//!
//! ## Session States
//! - New: Session created but not yet active
//! - Active: Session in use, TTL refreshed on access
//! - Expired: TTL exceeded, session invalid
//!
//! ## Cache States
//! - Miss: Entry not in cache
//! - Hit: Entry found and valid
//! - Stale: Entry found but expired

mod fixtures;

use fixtures::*;
use rstest::rstest;
use serial_test::serial;
use std::sync::Arc;
use std::time::Duration;

use reinhardt_http::Middleware;
use reinhardt_http::Request;
use reinhardt_middleware::cache::{CacheConfig, CacheKeyStrategy, CacheMiddleware};
use reinhardt_middleware::circuit_breaker::{
	CircuitBreakerConfig, CircuitBreakerMiddleware, CircuitState,
};

#[cfg(feature = "rate-limit")]
use reinhardt_middleware::rate_limit::{RateLimitConfig, RateLimitMiddleware, RateLimitStrategy};

// ============================================================================
// CircuitBreaker State Machine Tests
// ============================================================================

/// Test: Closed -> Open transition when error threshold exceeded
#[rstest]
#[tokio::test]
#[serial(circuit_breaker)]
async fn test_circuit_closed_to_open_on_error_threshold() {
	// Setup: Create circuit breaker with 50% error threshold, min 4 requests
	let config = CircuitBreakerConfig::new(0.5, 4, Duration::from_secs(60));
	let middleware = Arc::new(CircuitBreakerMiddleware::new(config));
	let failure_handler = Arc::new(ConfigurableTestHandler::always_failure());

	// Initial state should be Closed
	assert_eq!(
		middleware.state(),
		CircuitState::Closed,
		"Initial state should be Closed"
	);

	// Send 4 failures (100% error rate > 50% threshold)
	for i in 0..4 {
		let request = create_test_request("GET", "/api/data");
		let _ = middleware.process(request, failure_handler.clone()).await;

		if i < 3 {
			// Not enough requests yet
			assert_eq!(
				middleware.state(),
				CircuitState::Closed,
				"Should still be Closed at {} requests (min_requests=4)",
				i + 1
			);
		}
	}

	// Circuit should now be Open
	assert_eq!(
		middleware.state(),
		CircuitState::Open,
		"Circuit should transition to Open after exceeding error threshold"
	);
}

/// Test: Open -> HalfOpen transition after timeout
#[rstest]
#[tokio::test]
#[serial(circuit_breaker)]
async fn test_circuit_open_to_halfopen_after_timeout() {
	// Setup: Create circuit breaker with very short timeout
	let config = CircuitBreakerConfig::new(0.5, 2, Duration::from_millis(100));
	let middleware = Arc::new(CircuitBreakerMiddleware::new(config));
	let failure_handler = Arc::new(ConfigurableTestHandler::always_failure());
	let success_handler = Arc::new(ConfigurableTestHandler::always_success());

	// Force circuit to Open state
	for _ in 0..3 {
		let request = create_test_request("GET", "/api/data");
		let _ = middleware.process(request, failure_handler.clone()).await;
	}
	assert_eq!(middleware.state(), CircuitState::Open);

	// Wait for timeout to elapse
	tokio::time::sleep(Duration::from_millis(150)).await;

	// Send a request - this should trigger transition to HalfOpen
	let request = create_test_request("GET", "/api/data");
	let _ = middleware.process(request, success_handler.clone()).await;

	// State should now be HalfOpen or Closed (depending on if request succeeded)
	let state = middleware.state();
	assert!(
		state == CircuitState::HalfOpen || state == CircuitState::Closed,
		"State should be HalfOpen or Closed after timeout, got {:?}",
		state
	);
}

/// Test: HalfOpen -> Closed transition on success threshold
#[rstest]
#[tokio::test]
#[serial(circuit_breaker)]
async fn test_circuit_halfopen_to_closed_on_success() {
	// Setup: Create circuit breaker with low thresholds
	let config = CircuitBreakerConfig::new(0.5, 2, Duration::from_millis(50))
		.with_half_open_success_threshold(2);
	let middleware = Arc::new(CircuitBreakerMiddleware::new(config));
	let failure_handler = Arc::new(ConfigurableTestHandler::always_failure());
	let success_handler = Arc::new(ConfigurableTestHandler::always_success());

	// Force circuit to Open state
	for _ in 0..3 {
		let request = create_test_request("GET", "/api/data");
		let _ = middleware.process(request, failure_handler.clone()).await;
	}
	assert_eq!(middleware.state(), CircuitState::Open);

	// Wait for timeout
	tokio::time::sleep(Duration::from_millis(100)).await;

	// Send successful requests to close circuit
	for _ in 0..3 {
		let request = create_test_request("GET", "/api/data");
		let _ = middleware.process(request, success_handler.clone()).await;
	}

	// Circuit should now be Closed
	assert_eq!(
		middleware.state(),
		CircuitState::Closed,
		"Circuit should transition to Closed after successful requests in HalfOpen"
	);
}

/// Test: HalfOpen -> Open transition on failure
#[rstest]
#[tokio::test]
#[serial(circuit_breaker)]
async fn test_circuit_halfopen_to_open_on_failure() {
	// Setup: Create circuit breaker with short timeout
	let config = CircuitBreakerConfig::new(0.5, 2, Duration::from_millis(50))
		.with_half_open_success_threshold(5);
	let middleware = Arc::new(CircuitBreakerMiddleware::new(config));
	let failure_handler = Arc::new(ConfigurableTestHandler::always_failure());
	let success_handler = Arc::new(ConfigurableTestHandler::always_success());

	// Force circuit to Open state
	for _ in 0..3 {
		let request = create_test_request("GET", "/api/data");
		let _ = middleware.process(request, failure_handler.clone()).await;
	}
	assert_eq!(middleware.state(), CircuitState::Open);

	// Wait for timeout to enter HalfOpen
	tokio::time::sleep(Duration::from_millis(100)).await;

	// Send one successful request to enter HalfOpen
	let request = create_test_request("GET", "/api/data");
	let _ = middleware.process(request, success_handler.clone()).await;

	// Send a failure - should reopen circuit
	let request = create_test_request("GET", "/api/data");
	let _ = middleware.process(request, failure_handler.clone()).await;

	// Circuit should be back to Open
	assert_eq!(
		middleware.state(),
		CircuitState::Open,
		"Circuit should transition back to Open on failure in HalfOpen"
	);
}

/// Test: Full circuit breaker recovery cycle
#[rstest]
#[tokio::test]
#[serial(circuit_breaker)]
async fn test_circuit_full_recovery_cycle() {
	// Setup: Create circuit breaker
	let config = CircuitBreakerConfig::new(0.5, 2, Duration::from_millis(50))
		.with_half_open_success_threshold(2);
	let middleware = Arc::new(CircuitBreakerMiddleware::new(config));
	let failure_handler = Arc::new(ConfigurableTestHandler::always_failure());
	let success_handler = Arc::new(ConfigurableTestHandler::always_success());

	// Phase 1: Closed -> Open (failures)
	assert_eq!(middleware.state(), CircuitState::Closed);
	for _ in 0..3 {
		let request = create_test_request("GET", "/api/data");
		let _ = middleware.process(request, failure_handler.clone()).await;
	}
	assert_eq!(middleware.state(), CircuitState::Open);

	// Phase 2: Open -> HalfOpen (timeout)
	tokio::time::sleep(Duration::from_millis(100)).await;

	// Phase 3: HalfOpen -> Closed (successes)
	for _ in 0..3 {
		let request = create_test_request("GET", "/api/data");
		let _ = middleware.process(request, success_handler.clone()).await;
	}
	assert_eq!(middleware.state(), CircuitState::Closed);

	// Phase 4: Verify normal operation
	let request = create_test_request("GET", "/api/data");
	let response = middleware
		.process(request, success_handler.clone())
		.await
		.unwrap();
	assert_status(&response, 200);
}

/// Test: Manual circuit reset
#[rstest]
#[tokio::test]
#[serial(circuit_breaker)]
async fn test_circuit_manual_reset() {
	// Setup: Create circuit breaker
	let config = CircuitBreakerConfig::new(0.5, 2, Duration::from_secs(60));
	let middleware = Arc::new(CircuitBreakerMiddleware::new(config));
	let failure_handler = Arc::new(ConfigurableTestHandler::always_failure());

	// Force circuit to Open state
	for _ in 0..3 {
		let request = create_test_request("GET", "/api/data");
		let _ = middleware.process(request, failure_handler.clone()).await;
	}
	assert_eq!(middleware.state(), CircuitState::Open);

	// Manual reset
	middleware.reset();

	// Circuit should be Closed
	assert_eq!(
		middleware.state(),
		CircuitState::Closed,
		"Circuit should be Closed after manual reset"
	);
}

/// Test: Circuit stays closed with mixed results under threshold
#[rstest]
#[tokio::test]
#[serial(circuit_breaker)]
async fn test_circuit_stays_closed_under_threshold() {
	// Setup: Create circuit breaker with 50% threshold
	let config = CircuitBreakerConfig::new(0.5, 4, Duration::from_secs(60));
	let middleware = Arc::new(CircuitBreakerMiddleware::new(config));
	let success_handler = Arc::new(ConfigurableTestHandler::always_success());
	let failure_handler = Arc::new(ConfigurableTestHandler::always_failure());

	// Send 3 successes and 1 failure (25% error rate < 50% threshold)
	for _ in 0..3 {
		let request = create_test_request("GET", "/api/data");
		let _ = middleware.process(request, success_handler.clone()).await;
	}
	let request = create_test_request("GET", "/api/data");
	let _ = middleware.process(request, failure_handler.clone()).await;

	// Circuit should stay Closed
	assert_eq!(
		middleware.state(),
		CircuitState::Closed,
		"Circuit should stay Closed when error rate is under threshold"
	);
}

// ============================================================================
// RateLimit State Machine Tests
// ============================================================================

/// Test: Allow -> Block transition when tokens exhausted
#[cfg(feature = "rate-limit")]
#[rstest]
#[tokio::test]
#[serial(rate_limit)]
async fn test_ratelimit_allow_to_block_on_exhaustion() {
	// Setup: Create rate limiter with 3 tokens capacity, very slow refill
	let config =
		RateLimitConfig::new(RateLimitStrategy::PerIp, 3.0, 0.001).with_cost_per_request(1.0);
	let middleware = Arc::new(RateLimitMiddleware::new(config));
	let handler = Arc::new(ConfigurableTestHandler::always_success());

	// Send 3 requests (exhausts tokens)
	for i in 0..3 {
		let request = create_test_request("GET", "/api/data");
		let response = middleware.process(request, handler.clone()).await.unwrap();
		assert_status(&response, 200);

		// Check remaining tokens header
		if let Some(remaining) = response.headers.get("x-ratelimit-remaining") {
			let remaining_value: f64 = remaining.to_str().unwrap().parse().unwrap();
			assert_eq!(
				remaining_value,
				(2 - i) as f64,
				"Remaining tokens should decrease"
			);
		}
	}

	// Fourth request should be blocked
	let request = create_test_request("GET", "/api/data");
	let response = middleware.process(request, handler.clone()).await.unwrap();
	assert_status(&response, 429);
}

/// Test: Block -> Allow transition after refill
#[cfg(feature = "rate-limit")]
#[rstest]
#[tokio::test]
#[serial(rate_limit)]
async fn test_ratelimit_block_to_allow_after_refill() {
	// Setup: Create rate limiter with fast refill (10 tokens/sec = 1 token per 100ms)
	let config =
		RateLimitConfig::new(RateLimitStrategy::PerIp, 1.0, 10.0).with_cost_per_request(1.0);
	let middleware = Arc::new(RateLimitMiddleware::new(config));
	let handler = Arc::new(ConfigurableTestHandler::always_success());

	// Exhaust tokens
	let request = create_test_request("GET", "/api/data");
	let _ = middleware.process(request, handler.clone()).await.unwrap();

	// Should be blocked
	let request = create_test_request("GET", "/api/data");
	let response = middleware.process(request, handler.clone()).await.unwrap();
	assert_status(&response, 429);

	// Wait for refill (need at least 1 token = 100ms)
	tokio::time::sleep(Duration::from_millis(150)).await;

	// Should be allowed again
	let request = create_test_request("GET", "/api/data");
	let response = middleware.process(request, handler.clone()).await.unwrap();
	assert_status(&response, 200);
}

/// Test: Per-route isolation maintains separate state
#[cfg(feature = "rate-limit")]
#[rstest]
#[tokio::test]
#[serial(rate_limit)]
async fn test_ratelimit_per_route_isolation() {
	// Setup: Create per-route rate limiter
	let config =
		RateLimitConfig::new(RateLimitStrategy::PerRoute, 2.0, 0.001).with_cost_per_request(1.0);
	let middleware = Arc::new(RateLimitMiddleware::new(config));
	let handler = Arc::new(ConfigurableTestHandler::always_success());

	// Exhaust tokens for route A
	for _ in 0..2 {
		let request = create_test_request("GET", "/api/route-a");
		let _ = middleware.process(request, handler.clone()).await.unwrap();
	}

	// Route A should be blocked
	let request = create_test_request("GET", "/api/route-a");
	let response = middleware.process(request, handler.clone()).await.unwrap();
	assert_status(&response, 429);

	// Route B should still be allowed (separate bucket)
	let request = create_test_request("GET", "/api/route-b");
	let response = middleware.process(request, handler.clone()).await.unwrap();
	assert_status(&response, 200);
}

/// Test: Token partial consumption (throttle state)
#[cfg(feature = "rate-limit")]
#[rstest]
#[tokio::test]
#[serial(rate_limit)]
async fn test_ratelimit_partial_consumption() {
	// Setup: Create rate limiter with 5 tokens
	let config =
		RateLimitConfig::new(RateLimitStrategy::PerIp, 5.0, 0.001).with_cost_per_request(1.0);
	let middleware = Arc::new(RateLimitMiddleware::new(config));
	let handler = Arc::new(ConfigurableTestHandler::always_success());

	// Use 3 tokens
	for _ in 0..3 {
		let request = create_test_request("GET", "/api/data");
		let response = middleware.process(request, handler.clone()).await.unwrap();
		assert_status(&response, 200);
	}

	// Still 2 tokens left, should still be allowed
	let request = create_test_request("GET", "/api/data");
	let response = middleware.process(request, handler.clone()).await.unwrap();
	assert_status(&response, 200);

	// Last token
	let request = create_test_request("GET", "/api/data");
	let response = middleware.process(request, handler.clone()).await.unwrap();
	assert_status(&response, 200);

	// Now blocked
	let request = create_test_request("GET", "/api/data");
	let response = middleware.process(request, handler.clone()).await.unwrap();
	assert_status(&response, 429);
}

// ============================================================================
// Cache State Machine Tests
// ============================================================================

/// Test: Miss -> Hit transition when response is cached
#[rstest]
#[tokio::test]
#[serial(cache)]
async fn test_cache_miss_to_hit_on_cache() {
	// Setup: Create cache middleware
	let config = CacheConfig::new(Duration::from_secs(60), CacheKeyStrategy::UrlOnly);
	let middleware = Arc::new(CacheMiddleware::new(config));
	let handler = Arc::new(ConfigurableTestHandler::always_success());

	// First request: MISS
	let request = create_test_request("GET", "/api/data");
	let response = middleware.process(request, handler.clone()).await.unwrap();
	assert_header(&response, "x-cache", "MISS");
	assert_eq!(handler.count(), 1, "Handler should be called on MISS");

	// Second request: HIT
	let request = create_test_request("GET", "/api/data");
	let response = middleware.process(request, handler.clone()).await.unwrap();
	assert_header(&response, "x-cache", "HIT");
	assert_eq!(handler.count(), 1, "Handler should NOT be called on HIT");
}

/// Test: Hit -> Stale -> Miss transition after TTL expiration
#[rstest]
#[tokio::test]
#[serial(cache)]
async fn test_cache_hit_to_stale_after_ttl() {
	// Setup: Create cache middleware with very short TTL
	let config = CacheConfig::new(Duration::from_secs(1), CacheKeyStrategy::UrlOnly);
	let middleware = Arc::new(CacheMiddleware::new(config));
	let handler = Arc::new(ConfigurableTestHandler::always_success());

	// First request: MISS (caches response)
	let request = create_test_request("GET", "/api/data");
	let _ = middleware.process(request, handler.clone()).await.unwrap();
	assert_eq!(handler.count(), 1);

	// Second request: HIT
	let request = create_test_request("GET", "/api/data");
	let response = middleware.process(request, handler.clone()).await.unwrap();
	assert_header(&response, "x-cache", "HIT");
	assert_eq!(handler.count(), 1);

	// Wait for TTL to expire
	tokio::time::sleep(Duration::from_secs(2)).await;

	// Third request: MISS (stale entry deleted, fresh request)
	let request = create_test_request("GET", "/api/data");
	let response = middleware.process(request, handler.clone()).await.unwrap();
	assert_header(&response, "x-cache", "MISS");
	assert_eq!(
		handler.count(),
		2,
		"Handler should be called again after TTL"
	);
}

/// Test: Cache maintains separate state for different keys
#[rstest]
#[tokio::test]
#[serial(cache)]
async fn test_cache_per_key_isolation() {
	// Setup: Create cache middleware
	let config = CacheConfig::new(Duration::from_secs(60), CacheKeyStrategy::UrlOnly);
	let middleware = Arc::new(CacheMiddleware::new(config));
	let handler = Arc::new(ConfigurableTestHandler::always_success());

	// Cache path A
	let request = create_test_request("GET", "/api/path-a");
	let response = middleware.process(request, handler.clone()).await.unwrap();
	assert_header(&response, "x-cache", "MISS");

	// Cache path B
	let request = create_test_request("GET", "/api/path-b");
	let response = middleware.process(request, handler.clone()).await.unwrap();
	assert_header(&response, "x-cache", "MISS");

	// Path A should be HIT
	let request = create_test_request("GET", "/api/path-a");
	let response = middleware.process(request, handler.clone()).await.unwrap();
	assert_header(&response, "x-cache", "HIT");

	// Path B should be HIT
	let request = create_test_request("GET", "/api/path-b");
	let response = middleware.process(request, handler.clone()).await.unwrap();
	assert_header(&response, "x-cache", "HIT");

	// Handler should have been called only twice (initial MISS for each path)
	assert_eq!(handler.count(), 2);
}

/// Test: Cache bypass for non-cacheable methods
#[rstest]
#[tokio::test]
#[serial(cache)]
async fn test_cache_bypass_for_post() {
	// Setup: Create cache middleware
	let config = CacheConfig::new(Duration::from_secs(60), CacheKeyStrategy::UrlOnly);
	let middleware = Arc::new(CacheMiddleware::new(config));
	let handler = Arc::new(ConfigurableTestHandler::always_success());

	// POST request 1
	let request = create_request_with_body("POST", "/api/data", "body1");
	let response = middleware.process(request, handler.clone()).await.unwrap();
	assert_status(&response, 200);

	// POST request 2 - should not be cached
	let request = create_request_with_body("POST", "/api/data", "body2");
	let response = middleware.process(request, handler.clone()).await.unwrap();
	assert_status(&response, 200);

	// Both requests should hit handler (no caching for POST)
	assert_eq!(handler.count(), 2, "POST requests should not be cached");
}

// ============================================================================
// Session State Machine Tests
// ============================================================================

/// Test: Session creation (New -> Active)
#[rstest]
#[tokio::test]
#[serial(session)]
async fn test_session_new_to_active() {
	use reinhardt_middleware::session::{SessionConfig, SessionMiddleware};

	// Setup: Create session middleware
	let config = SessionConfig::new("sessionid".to_string(), Duration::from_secs(60));
	let middleware = Arc::new(SessionMiddleware::new(config));
	let handler = Arc::new(ConfigurableTestHandler::always_success());

	// First request without session
	let request = create_test_request("GET", "/page");
	let response = middleware.process(request, handler.clone()).await.unwrap();

	// Response should have Set-Cookie with session ID
	let cookie_header = response.headers.get("set-cookie");
	assert!(
		cookie_header.is_some(),
		"Response should have Set-Cookie header for new session"
	);
}

/// Test: Session expiration (Active -> Expired)
#[rstest]
#[tokio::test]
#[serial(session)]
async fn test_session_active_to_expired() {
	use reinhardt_middleware::session::{SessionConfig, SessionMiddleware};

	// Setup: Create session middleware with very short TTL
	let config = SessionConfig::new("sessionid".to_string(), Duration::from_millis(100));
	let middleware = Arc::new(SessionMiddleware::new(config));
	let handler = Arc::new(ConfigurableTestHandler::always_success());

	// Create session
	let request = create_test_request("GET", "/page");
	let response = middleware.process(request, handler.clone()).await.unwrap();

	// Extract session cookie value (clone to own the string)
	let cookie_header = response.headers.get("set-cookie").unwrap();
	let cookie_str = cookie_header.to_str().unwrap().to_string();

	// Wait for session to expire
	tokio::time::sleep(Duration::from_millis(200)).await;

	// Request with expired session cookie
	let request = Request::builder()
		.method(hyper::Method::GET)
		.uri("/page")
		.header("Cookie", cookie_str.as_str())
		.body(bytes::Bytes::new())
		.build()
		.unwrap();
	let response = middleware.process(request, handler.clone()).await.unwrap();

	// Should get new session (old one expired)
	let new_cookie = response.headers.get("set-cookie");
	assert!(
		new_cookie.is_some(),
		"Should receive new session after expiration"
	);
}

/// Test: Session cleanup removes expired sessions
#[rstest]
#[tokio::test]
#[serial(session)]
async fn test_session_cleanup() {
	use reinhardt_middleware::session::{SessionConfig, SessionMiddleware, SessionStore};

	// Setup: Create session store and middleware
	let store = Arc::new(SessionStore::new());
	let config = SessionConfig::new("sessionid".to_string(), Duration::from_millis(50));
	let middleware = Arc::new(SessionMiddleware::from_arc(config, store.clone()));
	let handler = Arc::new(ConfigurableTestHandler::always_success());

	// Create a session through middleware
	let request = create_test_request("GET", "/page");
	let response = middleware.process(request, handler.clone()).await.unwrap();

	// Extract session ID from Set-Cookie header
	let cookie_header = response.headers.get("set-cookie").unwrap();
	let cookie_str = cookie_header.to_str().unwrap();

	// Parse session ID from cookie (format: "sessionid=<id>; ...")
	let session_id = cookie_str
		.split(';')
		.next()
		.unwrap()
		.split('=')
		.nth(1)
		.unwrap();

	// Session should exist
	assert!(
		store.get(session_id).is_some(),
		"Session should exist after creation"
	);

	// Wait for session to expire
	tokio::time::sleep(Duration::from_millis(100)).await;

	// Cleanup expired sessions
	store.cleanup();

	// Session should be removed after cleanup
	assert!(
		store.get(session_id).is_none(),
		"Expired session should be removed after cleanup"
	);
}
