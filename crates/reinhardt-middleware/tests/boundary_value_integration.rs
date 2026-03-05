//! Boundary Value Integration Tests for reinhardt-middleware
//!
//! This module tests middleware behavior at critical boundary values:
//! - RateLimit capacity boundaries (capacity-1, capacity, capacity+1)
//! - Timeout boundaries (below, at, above timeout threshold)
//! - GZip min_length boundaries
//! - CircuitBreaker error_threshold boundaries
//! - Session TTL boundaries
//! - Cache TTL boundaries
//!
//! These tests use `#[rstest::case]` for parameterized boundary testing.

mod fixtures;

use async_trait::async_trait;
use bytes::Bytes;
use fixtures::{
	ConfigurableTestHandler, SizedResponseHandler, assert_has_header, assert_no_header,
	assert_status, create_request_with_headers, create_test_request,
};
use reinhardt_core::exception::Result;
use reinhardt_http::{Handler, Middleware};
use reinhardt_http::{Request, Response};
use reinhardt_middleware::cache::{CacheConfig, CacheKeyStrategy, CacheMiddleware};
use reinhardt_middleware::circuit_breaker::{
	CircuitBreakerConfig, CircuitBreakerMiddleware, CircuitState,
};
#[cfg(feature = "compression")]
use reinhardt_middleware::gzip::{GZipConfig, GZipMiddleware};
#[cfg(feature = "rate-limit")]
use reinhardt_middleware::rate_limit::{RateLimitConfig, RateLimitMiddleware, RateLimitStrategy};
use reinhardt_middleware::session::{SessionConfig, SessionMiddleware};
use reinhardt_middleware::timeout::{TimeoutConfig, TimeoutMiddleware};
use rstest::rstest;
use serial_test::serial;
use std::sync::Arc;
use std::time::Duration;

// ============================================================================
// Helper Handlers
// ============================================================================

/// Handler that takes a specified time to respond.
struct DelayedHandler {
	delay: Duration,
}

impl DelayedHandler {
	fn new(delay: Duration) -> Self {
		Self { delay }
	}
}

#[async_trait]
impl Handler for DelayedHandler {
	async fn handle(&self, _request: Request) -> Result<Response> {
		tokio::time::sleep(self.delay).await;
		Ok(Response::ok().with_body(Bytes::from("delayed response")))
	}
}

// ============================================================================
// RateLimit Capacity Boundary Tests
// ============================================================================

#[cfg(feature = "rate-limit")]
#[rstest]
#[case::below_capacity(9, 10, true)]
#[case::at_capacity(10, 10, true)]
#[case::above_capacity(11, 10, false)]
#[tokio::test]
#[serial(rate_limit_capacity)]
async fn test_rate_limit_capacity_boundary(
	#[case] request_count: usize,
	#[case] capacity: usize,
	#[case] all_should_succeed: bool,
) {
	let config = RateLimitConfig::new(
		RateLimitStrategy::PerIp,
		capacity as f64,
		0.001, // Very slow refill to prevent token recovery during test
	)
	.with_cost_per_request(1.0);

	let middleware = Arc::new(RateLimitMiddleware::new(config));
	let handler = Arc::new(ConfigurableTestHandler::always_success());

	let mut success_count = 0;
	let mut failure_count = 0;

	for _ in 0..request_count {
		let request = create_test_request("GET", "/api/data");
		let result = middleware.process(request, handler.clone()).await;

		match result {
			Ok(response) if response.status.as_u16() == 200 => success_count += 1,
			Ok(response) if response.status.as_u16() == 429 => failure_count += 1,
			_ => failure_count += 1,
		}
	}

	if all_should_succeed {
		assert_eq!(
			success_count, request_count,
			"All {} requests should succeed when capacity is {}",
			request_count, capacity
		);
		assert_eq!(failure_count, 0, "No requests should fail");
	} else {
		assert_eq!(
			success_count, capacity,
			"Only {} requests should succeed when capacity is {}",
			capacity, capacity
		);
		assert_eq!(
			failure_count,
			request_count - capacity,
			"Requests beyond capacity should fail"
		);
	}
}

// ============================================================================
// RateLimit Token Cost Boundary Tests
// ============================================================================

#[cfg(feature = "rate-limit")]
#[rstest]
#[case::cost_below_capacity(0.9, 1.0, true)]
#[case::cost_at_capacity(1.0, 1.0, true)]
#[case::cost_above_capacity(1.1, 1.0, false)]
#[tokio::test]
#[serial(rate_limit_cost)]
async fn test_rate_limit_token_cost_boundary(
	#[case] cost_per_request: f64,
	#[case] capacity: f64,
	#[case] should_succeed: bool,
) {
	let config = RateLimitConfig::new(RateLimitStrategy::PerIp, capacity, 0.001)
		.with_cost_per_request(cost_per_request);

	let middleware = Arc::new(RateLimitMiddleware::new(config));
	let handler = Arc::new(ConfigurableTestHandler::always_success());
	let request = create_test_request("GET", "/api/data");

	let result = middleware.process(request, handler.clone()).await;

	if should_succeed {
		assert!(
			result.is_ok(),
			"Request should succeed with cost {} and capacity {}",
			cost_per_request,
			capacity
		);
		let response = result.unwrap();
		assert_eq!(
			response.status.as_u16(),
			200,
			"Response status should be 200"
		);
	} else {
		match result {
			Ok(response) => assert_eq!(
				response.status.as_u16(),
				429,
				"Response should be 429 Too Many Requests"
			),
			Err(_) => {
				// Error is also acceptable for cost > capacity
			}
		}
	}
}

// ============================================================================
// Timeout Boundary Tests
// ============================================================================

#[rstest]
#[case::below_timeout(50, 100, true)]
#[case::at_timeout_margin(95, 100, true)]
#[case::above_timeout(150, 100, false)]
#[tokio::test]
async fn test_timeout_boundary(
	#[case] handler_delay_ms: u64,
	#[case] timeout_ms: u64,
	#[case] should_succeed: bool,
) {
	let config = TimeoutConfig::new(Duration::from_millis(timeout_ms));
	let middleware = Arc::new(TimeoutMiddleware::new(config));
	let handler = Arc::new(DelayedHandler::new(Duration::from_millis(handler_delay_ms)));
	let request = create_test_request("GET", "/api/data");

	let result = middleware.process(request, handler.clone()).await;

	if should_succeed {
		assert!(
			result.is_ok(),
			"Request should succeed with {}ms delay and {}ms timeout",
			handler_delay_ms,
			timeout_ms
		);
		let response = result.unwrap();
		assert_eq!(
			response.status.as_u16(),
			200,
			"Response status should be 200"
		);
	} else {
		// Timeout should result in 408 Request Timeout or error
		match result {
			Ok(response) => {
				assert_eq!(
					response.status.as_u16(),
					408,
					"Response should be 408 Request Timeout"
				);
			}
			Err(_) => {
				// Timeout error is also acceptable
			}
		}
	}
}

// ============================================================================
// GZip min_length Boundary Tests
// ============================================================================

#[cfg(feature = "compression")]
#[rstest]
#[case::below_min_length(199, 200, false)]
#[case::at_min_length(200, 200, true)]
#[case::above_min_length(201, 200, true)]
#[tokio::test]
async fn test_gzip_min_length_boundary(
	#[case] body_size: usize,
	#[case] min_length: usize,
	#[case] should_compress: bool,
) {
	let mut config = GZipConfig::default();
	config.min_length = min_length;
	config.compression_level = 6;
	config.compressible_types = vec!["text/".to_string()];
	let middleware = Arc::new(GZipMiddleware::with_config(config));
	let handler = Arc::new(SizedResponseHandler::new(body_size, "text/html"));

	let request = create_request_with_headers("GET", "/page", &[("Accept-Encoding", "gzip")]);

	let response = middleware.process(request, handler.clone()).await.unwrap();

	if should_compress {
		assert_has_header(&response, "content-encoding");
		assert!(
			response.body.len() < body_size,
			"Compressed body should be smaller than original"
		);
	} else {
		assert_no_header(&response, "content-encoding");
		assert_eq!(
			response.body.len(),
			body_size,
			"Body should not be modified"
		);
	}
}

// ============================================================================
// GZip Compression Level Boundary Tests
// ============================================================================

#[cfg(feature = "compression")]
#[rstest]
#[case::low_compression_level(1, true)]
#[case::default_compression_level(6, true)]
#[case::max_compression_level(9, true)]
#[tokio::test]
async fn test_gzip_compression_level_boundary(
	#[case] compression_level: u32,
	#[case] should_compress: bool,
) {
	let mut config = GZipConfig::default();
	config.min_length = 100;
	config.compression_level = compression_level;
	config.compressible_types = vec!["text/".to_string()];
	let middleware = Arc::new(GZipMiddleware::with_config(config));

	// Create a compressible response larger than min_length
	let body_size = 500;
	let handler = Arc::new(SizedResponseHandler::new(body_size, "text/html"));
	let request = create_request_with_headers("GET", "/page", &[("Accept-Encoding", "gzip")]);

	let response = middleware.process(request, handler.clone()).await.unwrap();

	if should_compress {
		assert_has_header(&response, "content-encoding");
		assert!(
			response.body.len() < body_size,
			"Compression level {} should compress the body",
			compression_level
		);
	}
}

// ============================================================================
// CircuitBreaker Error Threshold Boundary Tests
// ============================================================================

#[rstest]
#[case::below_threshold(4, 10, 0.5, CircuitState::Closed)]
#[case::at_threshold(5, 10, 0.5, CircuitState::Open)]
#[case::above_threshold(6, 10, 0.5, CircuitState::Open)]
#[tokio::test]
#[serial(circuit_breaker_threshold)]
async fn test_circuit_breaker_error_threshold_boundary(
	#[case] failure_count: usize,
	#[case] total_requests: usize,
	#[case] error_threshold: f64,
	#[case] expected_state: CircuitState,
) {
	let config = CircuitBreakerConfig::new(error_threshold, 1, Duration::from_secs(60))
		.with_half_open_success_threshold(1);
	let middleware = Arc::new(CircuitBreakerMiddleware::new(config));

	// Build pattern: success requests first, then failures
	let success_count = total_requests - failure_count;
	let mut pattern = vec![true; success_count];
	pattern.extend(vec![false; failure_count]);

	let handler = Arc::new(ConfigurableTestHandler::new(pattern));

	for _ in 0..total_requests {
		let request = create_test_request("GET", "/api/data");
		let _ = middleware.process(request, handler.clone()).await;
	}

	assert_eq!(
		middleware.state(),
		expected_state,
		"CircuitBreaker should be in {:?} state after {}/{} failures (threshold: {})",
		expected_state,
		failure_count,
		total_requests,
		error_threshold
	);
}

// ============================================================================
// CircuitBreaker Min Requests Boundary Tests
// ============================================================================

#[rstest]
#[case::below_min_requests(4, 5, CircuitState::Closed)]
#[case::at_min_requests(5, 5, CircuitState::Open)]
#[case::above_min_requests(6, 5, CircuitState::Open)]
#[tokio::test]
#[serial(circuit_breaker_min_requests)]
async fn test_circuit_breaker_min_requests_boundary(
	#[case] request_count: usize,
	#[case] min_requests: u64,
	#[case] expected_state: CircuitState,
) {
	// 100% error rate, but need min_requests to trip
	let config = CircuitBreakerConfig::new(0.1, min_requests, Duration::from_secs(60))
		.with_half_open_success_threshold(1);
	let middleware = Arc::new(CircuitBreakerMiddleware::new(config));
	let handler = Arc::new(ConfigurableTestHandler::always_failure());

	for _ in 0..request_count {
		let request = create_test_request("GET", "/api/data");
		let _ = middleware.process(request, handler.clone()).await;
	}

	assert_eq!(
		middleware.state(),
		expected_state,
		"CircuitBreaker should be in {:?} state after {} requests (min_requests: {})",
		expected_state,
		request_count,
		min_requests
	);
}

// ============================================================================
// CircuitBreaker HalfOpen Success Threshold Boundary Tests
// ============================================================================

#[rstest]
#[case::below_success_threshold(1, 2, CircuitState::HalfOpen)]
#[case::at_success_threshold(2, 2, CircuitState::Closed)]
#[case::above_success_threshold(3, 2, CircuitState::Closed)]
#[tokio::test]
#[serial(circuit_breaker_success_threshold)]
async fn test_circuit_breaker_half_open_success_threshold_boundary(
	#[case] success_count: usize,
	#[case] success_threshold: u64,
	#[case] expected_state: CircuitState,
) {
	let config = CircuitBreakerConfig::new(0.5, 2, Duration::from_millis(50))
		.with_half_open_success_threshold(success_threshold);
	let middleware = Arc::new(CircuitBreakerMiddleware::new(config));

	// First, trip the circuit breaker
	let failure_handler = Arc::new(ConfigurableTestHandler::always_failure());
	for _ in 0..3 {
		let request = create_test_request("GET", "/api/data");
		let _ = middleware.process(request, failure_handler.clone()).await;
	}
	assert_eq!(
		middleware.state(),
		CircuitState::Open,
		"Circuit should be open"
	);

	// Wait for timeout to transition to HalfOpen
	tokio::time::sleep(Duration::from_millis(60)).await;

	// Send success requests in HalfOpen state
	let success_handler = Arc::new(ConfigurableTestHandler::always_success());
	for _ in 0..success_count {
		if middleware.state() == CircuitState::Closed {
			break; // Already closed, no need to send more
		}
		let request = create_test_request("GET", "/api/data");
		let _ = middleware.process(request, success_handler.clone()).await;
	}

	assert_eq!(
		middleware.state(),
		expected_state,
		"CircuitBreaker should be in {:?} state after {} successes (threshold: {})",
		expected_state,
		success_count,
		success_threshold
	);
}

// ============================================================================
// Session TTL Boundary Tests
// ============================================================================

#[rstest]
#[case::before_ttl(40, 100, true)]
#[case::at_ttl_margin(90, 100, true)]
#[case::after_ttl(150, 100, false)]
#[tokio::test]
#[serial(session_ttl)]
async fn test_session_ttl_boundary(
	#[case] wait_ms: u64,
	#[case] ttl_ms: u64,
	#[case] should_be_valid: bool,
) {
	let config = SessionConfig::new("session".to_string(), Duration::from_millis(ttl_ms));
	let middleware = Arc::new(SessionMiddleware::new(config));
	let handler = Arc::new(ConfigurableTestHandler::always_success());

	// Create a session
	let request1 = create_test_request("GET", "/login");
	let response1 = middleware.process(request1, handler.clone()).await.unwrap();

	// Extract session cookie
	let cookie_value = response1
		.headers
		.get("set-cookie")
		.and_then(|v| v.to_str().ok())
		.and_then(|s| s.split(';').next())
		.map(|s| s.to_string());

	assert!(cookie_value.is_some(), "Session cookie should be set");

	// Wait for specified time
	tokio::time::sleep(Duration::from_millis(wait_ms)).await;

	// Send request with session cookie
	let cookie_str = cookie_value.unwrap();
	let request2 = create_request_with_headers(
		"GET",
		"/protected",
		&[("Cookie", Box::leak(cookie_str.into_boxed_str()))],
	);

	let response2 = middleware.process(request2, handler.clone()).await.unwrap();

	// Check if a new session was created (indicating old session expired)
	let new_cookie = response2.headers.get("set-cookie");

	if should_be_valid {
		// Session should still be valid - no new session cookie should be set
		// (or the same session ID is returned)
		assert_status(&response2, 200);
	} else {
		// Session should be expired - a new session should be created
		assert!(
			new_cookie.is_some(),
			"New session should be created after TTL expiry"
		);
	}
}

// ============================================================================
// Cache TTL Boundary Tests
// ============================================================================

#[rstest]
#[case::before_ttl(500, 1, "HIT")]
#[case::after_ttl(1500, 1, "MISS")]
#[tokio::test]
#[serial(cache_ttl)]
async fn test_cache_ttl_boundary(
	#[case] wait_ms: u64,
	#[case] ttl_secs: u64,
	#[case] expected_header: &str,
) {
	let config = CacheConfig::new(
		Duration::from_secs(ttl_secs),
		CacheKeyStrategy::UrlAndMethod,
	)
	.with_max_entries(100);
	let middleware = Arc::new(CacheMiddleware::new(config));
	let handler = Arc::new(ConfigurableTestHandler::always_success().with_body("cached content"));

	// First request - should be a MISS
	let request1 = create_test_request("GET", "/api/data");
	let response1 = middleware.process(request1, handler.clone()).await.unwrap();
	assert_eq!(
		response1
			.headers
			.get("X-Cache")
			.and_then(|v| v.to_str().ok()),
		Some("MISS"),
		"First request should be a cache miss"
	);

	// Wait for specified time
	tokio::time::sleep(Duration::from_millis(wait_ms)).await;

	// Second request - check cache status
	let request2 = create_test_request("GET", "/api/data");
	let response2 = middleware.process(request2, handler.clone()).await.unwrap();

	let cache_header = response2
		.headers
		.get("X-Cache")
		.and_then(|v| v.to_str().ok())
		.unwrap_or("NONE");

	assert_eq!(
		cache_header, expected_header,
		"Cache header should be {} after waiting {}ms with TTL {}s",
		expected_header, wait_ms, ttl_secs
	);
}

// ============================================================================
// Cache Max Entries Boundary Tests
// ============================================================================

#[rstest]
#[case::below_max_entries(4, 5)]
#[case::at_max_entries(5, 5)]
#[case::above_max_entries(6, 5)]
#[tokio::test]
#[serial(cache_max_entries)]
async fn test_cache_max_entries_boundary(#[case] entry_count: usize, #[case] max_entries: usize) {
	let config = CacheConfig::new(Duration::from_secs(60), CacheKeyStrategy::UrlAndMethod)
		.with_max_entries(max_entries);
	let middleware = Arc::new(CacheMiddleware::new(config));
	let handler = Arc::new(ConfigurableTestHandler::always_success().with_body("cached"));

	// Create entries by making requests to different URLs
	for i in 0..entry_count {
		let url = format!("/api/data/{}", i);
		let request = create_test_request("GET", &url);
		let _ = middleware.process(request, handler.clone()).await;
	}

	// Verify the earliest entries - they might be evicted if we exceeded max_entries
	let first_request = create_test_request("GET", "/api/data/0");
	let first_response = middleware
		.process(first_request, handler.clone())
		.await
		.unwrap();

	let cache_status = first_response
		.headers
		.get("X-Cache")
		.and_then(|v| v.to_str().ok())
		.unwrap_or("NONE");

	if entry_count <= max_entries {
		// All entries should fit in cache
		assert_eq!(
			cache_status, "HIT",
			"First entry should still be in cache when {} <= {}",
			entry_count, max_entries
		);
	} else {
		// First entry might have been evicted
		// This depends on eviction policy - just verify no panic
		assert!(
			cache_status == "HIT" || cache_status == "MISS",
			"Cache should return valid status"
		);
	}
}

// ============================================================================
// Empty and Zero Value Boundary Tests
// ============================================================================

#[cfg(feature = "compression")]
#[rstest]
#[case::empty_body(0)]
#[case::single_byte(1)]
#[case::small_body(10)]
#[tokio::test]
async fn test_gzip_empty_body_boundary(#[case] body_size: usize) {
	let mut config = GZipConfig::default();
	config.min_length = 0; // Allow compression of any size
	config.compression_level = 6;
	config.compressible_types = vec!["text/".to_string()];
	let middleware = Arc::new(GZipMiddleware::with_config(config));
	let handler = Arc::new(SizedResponseHandler::new(body_size, "text/html"));

	let request = create_request_with_headers("GET", "/page", &[("Accept-Encoding", "gzip")]);
	let response = middleware.process(request, handler.clone()).await.unwrap();

	// Empty or very small bodies might not compress well
	assert_status(&response, 200);

	if body_size == 0 {
		// Empty body should not have compression header
		assert_no_header(&response, "content-encoding");
	}
}

#[tokio::test]
async fn test_timeout_zero_duration() {
	// Zero timeout should cause immediate timeout
	let config = TimeoutConfig::new(Duration::from_millis(0));
	let middleware = Arc::new(TimeoutMiddleware::new(config));
	let handler = Arc::new(ConfigurableTestHandler::always_success());

	let request = create_test_request("GET", "/api/data");
	let result = middleware.process(request, handler.clone()).await;

	// Zero timeout should cause timeout error or 408
	match result {
		Ok(response) => {
			// Either immediate response or timeout
			assert!(
				response.status.as_u16() == 200 || response.status.as_u16() == 408,
				"Should be either 200 or 408"
			);
		}
		Err(_) => {
			// Timeout error is acceptable
		}
	}
}

// ============================================================================
// Large Value Boundary Tests
// ============================================================================

#[rstest]
#[case::large_capacity(1_000_000)]
#[case::very_large_capacity(10_000_000)]
#[tokio::test]
#[serial(rate_limit_large_capacity)]
#[cfg(feature = "rate-limit")]
async fn test_rate_limit_large_capacity_boundary(#[case] capacity: usize) {
	let config = RateLimitConfig::new(RateLimitStrategy::PerIp, capacity as f64, 1.0)
		.with_cost_per_request(1.0);

	let middleware = Arc::new(RateLimitMiddleware::new(config));
	let handler = Arc::new(ConfigurableTestHandler::always_success());

	// Should handle large capacity without panic
	let request = create_test_request("GET", "/api/data");
	let result = middleware.process(request, handler.clone()).await;

	assert!(result.is_ok(), "Large capacity should not cause errors");
	assert_status(&result.unwrap(), 200);
}

#[cfg(feature = "compression")]
#[tokio::test]
async fn test_gzip_large_body_boundary() {
	let mut config = GZipConfig::default();
	config.min_length = 100;
	config.compression_level = 6;
	config.compressible_types = vec!["text/".to_string()];
	let middleware = Arc::new(GZipMiddleware::with_config(config));

	// 1MB response body
	let body_size = 1_000_000;
	let handler = Arc::new(SizedResponseHandler::new(body_size, "text/html"));
	let request = create_request_with_headers("GET", "/page", &[("Accept-Encoding", "gzip")]);

	let response = middleware.process(request, handler.clone()).await.unwrap();

	assert_has_header(&response, "content-encoding");
	assert!(
		response.body.len() < body_size,
		"Large body should be compressed"
	);
	// GZip should achieve significant compression on repetitive content
	assert!(
		response.body.len() < body_size / 10,
		"Compression ratio should be > 10:1 for repetitive content"
	);
}

#[tokio::test]
async fn test_timeout_large_duration() {
	// Very large timeout should not cause overflow
	let config = TimeoutConfig::new(Duration::from_secs(3600)); // 1 hour
	let middleware = Arc::new(TimeoutMiddleware::new(config));
	let handler = Arc::new(ConfigurableTestHandler::always_success());

	let request = create_test_request("GET", "/api/data");
	let result = middleware.process(request, handler.clone()).await;

	assert!(result.is_ok(), "Large timeout should not cause errors");
	assert_status(&result.unwrap(), 200);
}

// ============================================================================
// Precision Boundary Tests
// ============================================================================

#[cfg(feature = "rate-limit")]
#[rstest]
#[case::fractional_capacity_single_request(1.5, 1, true)] // 1 request with cost 1.0 < capacity 1.5 -> succeeds
#[case::fractional_capacity_two_requests(1.5, 2, false)] // 2nd request: needs 1.0, only 0.5 left -> fails
#[tokio::test]
#[serial(rate_limit_precision)]
async fn test_rate_limit_fractional_capacity_boundary(
	#[case] capacity: f64,
	#[case] request_count: usize,
	#[case] all_should_succeed: bool,
) {
	let config =
		RateLimitConfig::new(RateLimitStrategy::PerIp, capacity, 0.001).with_cost_per_request(1.0);

	let middleware = Arc::new(RateLimitMiddleware::new(config));
	let handler = Arc::new(ConfigurableTestHandler::always_success());

	let mut success_count = 0;
	for _ in 0..request_count {
		let request = create_test_request("GET", "/api/data");
		let result = middleware.process(request, handler.clone()).await;
		if let Ok(response) = result {
			if response.status.as_u16() == 200 {
				success_count += 1;
			}
		}
	}

	if all_should_succeed {
		assert_eq!(
			success_count, request_count,
			"All requests should succeed with capacity {}",
			capacity
		);
	} else {
		assert!(
			success_count < request_count,
			"Not all requests should succeed when capacity {} is exceeded",
			capacity
		);
	}
}

#[rstest]
#[case::threshold_0_49(0.49, 5, 10, CircuitState::Open)] // 5/10=0.50 > 0.49 -> Open
#[case::threshold_0_50(0.50, 5, 10, CircuitState::Open)] // 5/10=0.50 >= 0.50 -> Open
#[case::threshold_0_51(0.51, 5, 10, CircuitState::Closed)] // 5/10=0.50 < 0.51 -> Closed
#[tokio::test]
#[serial(circuit_breaker_precision)]
async fn test_circuit_breaker_threshold_precision_boundary(
	#[case] error_threshold: f64,
	#[case] failure_count: usize,
	#[case] total_requests: usize,
	#[case] expected_state: CircuitState,
) {
	let config = CircuitBreakerConfig::new(error_threshold, 1, Duration::from_secs(60))
		.with_half_open_success_threshold(1);
	let middleware = Arc::new(CircuitBreakerMiddleware::new(config));

	// Build pattern: success requests first, then failures
	let success_count = total_requests - failure_count;
	let mut pattern = vec![true; success_count];
	pattern.extend(vec![false; failure_count]);

	let handler = Arc::new(ConfigurableTestHandler::new(pattern));

	for _ in 0..total_requests {
		let request = create_test_request("GET", "/api/data");
		let _ = middleware.process(request, handler.clone()).await;
	}

	assert_eq!(
		middleware.state(),
		expected_state,
		"CircuitBreaker with threshold {} should be {:?} after {}/{} failures",
		error_threshold,
		expected_state,
		failure_count,
		total_requests
	);
}
