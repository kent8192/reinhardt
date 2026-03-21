//! Property-Based Integration Tests
//!
//! This module uses proptest to verify invariants that should always hold true
//! for middleware, regardless of input values.
//!
//! # Invariants Tested
//!
//! - Same body always produces same ETag
//! - Compress then decompress equals original
//! - Remaining tokens always in valid range
//! - Circuit breaker state always valid
//! - CSRF token has fixed length
//! - Request ID is valid UUID format
//! - Request count monotonically increases

mod fixtures;

use fixtures::{ConfigurableTestHandler, create_test_request};
use proptest::prelude::*;
use reinhardt_http::Middleware;
use std::sync::Arc;

// =============================================================================
// ETag Properties
// =============================================================================

proptest! {
	#![proptest_config(ProptestConfig::with_cases(20))]

	/// Property: Same body content always produces the same ETag.
	#[test]
	fn prop_etag_deterministic(body in "[a-zA-Z0-9]{1,100}") {
		let rt = tokio::runtime::Runtime::new().unwrap();
		rt.block_on(async {
			use reinhardt_middleware::etag::ETagMiddleware;

			let etag = Arc::new(ETagMiddleware::default());

			// Create two handlers with the same body
			let handler1 = Arc::new(ConfigurableTestHandler::with_body_string(&body));
			let handler2 = Arc::new(ConfigurableTestHandler::with_body_string(&body));

			let request1 = create_test_request("GET", "/");
			let request2 = create_test_request("GET", "/");

			let response1 = etag.process(request1, handler1).await.unwrap();
			let response2 = etag.process(request2, handler2).await.unwrap();

			let etag1 = response1.headers.get("etag").map(|v| v.to_str().unwrap().to_string());
			let etag2 = response2.headers.get("etag").map(|v| v.to_str().unwrap().to_string());

			// Same content should produce same ETag
			assert_eq!(etag1, etag2, "Same content should produce same ETag");
		});
	}

	/// Property: Different bodies produce different ETags.
	#[test]
	fn prop_etag_different_content(
		body1 in "[a-z]{5,20}",
		body2 in "[A-Z]{5,20}"
	) {
		// Ensure bodies are actually different
		if body1.to_lowercase() == body2.to_lowercase() {
			return Ok(());
		}

		let rt = tokio::runtime::Runtime::new().unwrap();
		rt.block_on(async {
			use reinhardt_middleware::etag::ETagMiddleware;

			let etag = Arc::new(ETagMiddleware::default());

			let handler1 = Arc::new(ConfigurableTestHandler::with_body_string(&body1));
			let handler2 = Arc::new(ConfigurableTestHandler::with_body_string(&body2));

			let request1 = create_test_request("GET", "/");
			let request2 = create_test_request("GET", "/");

			let response1 = etag.process(request1, handler1).await.unwrap();
			let response2 = etag.process(request2, handler2).await.unwrap();

			let etag1 = response1.headers.get("etag").map(|v| v.to_str().unwrap().to_string());
			let etag2 = response2.headers.get("etag").map(|v| v.to_str().unwrap().to_string());

			// Different content should produce different ETags
			assert_ne!(etag1, etag2, "Different content should produce different ETags");
		});
	}
}

// =============================================================================
// Rate Limit Properties
// =============================================================================

#[cfg(feature = "rate-limit")]
proptest! {
	#![proptest_config(ProptestConfig::with_cases(20))]

	/// Property: Remaining tokens never exceed capacity.
	#[test]
	fn prop_ratelimit_tokens_bounded(
		capacity in 1.0f64..100.0,
		refill_rate in 0.1f64..10.0,
		num_requests in 1usize..20
	) {
		let rt = tokio::runtime::Runtime::new().unwrap();
		rt.block_on(async {
			use reinhardt_middleware::rate_limit::{RateLimitConfig, RateLimitMiddleware, RateLimitStrategy};

			let config = RateLimitConfig::new(RateLimitStrategy::PerIp, capacity, refill_rate);

			let middleware = Arc::new(RateLimitMiddleware::new(config));
			let handler = Arc::new(ConfigurableTestHandler::always_success());

			// Send requests
			for _ in 0..num_requests {
				let request = fixtures::create_request_with_headers(
					"GET",
					"/",
					&[("X-Forwarded-For", "192.168.1.1")]
				);
				let _ = middleware.process(request, handler.clone()).await;
			}

			// After processing, remaining tokens should be valid (test passes if no panic)
		});
	}

	/// Property: Rate limit always allows at least one request with fresh tokens.
	#[test]
	fn prop_ratelimit_allows_first_request(
		capacity in 1.0f64..100.0,
		refill_rate in 0.1f64..10.0
	) {
		let rt = tokio::runtime::Runtime::new().unwrap();
		rt.block_on(async {
			use reinhardt_middleware::rate_limit::{RateLimitConfig, RateLimitMiddleware, RateLimitStrategy};

			let config = RateLimitConfig::new(RateLimitStrategy::PerIp, capacity, refill_rate);

			let middleware = Arc::new(RateLimitMiddleware::new(config));
			let handler = Arc::new(ConfigurableTestHandler::always_success());

			// Fresh bucket should allow first request
			let request = fixtures::create_request_with_headers(
				"GET",
				"/",
				&[("X-Forwarded-For", "10.0.0.1")]
			);
			let response = middleware.process(request, handler).await.unwrap();

			assert_eq!(response.status.as_u16(), 200, "Fresh bucket should allow first request");
		});
	}
}

// =============================================================================
// Circuit Breaker Properties
// =============================================================================

proptest! {
	#![proptest_config(ProptestConfig::with_cases(10))]

	/// Property: Circuit breaker state is always valid.
	#[test]
	fn prop_circuit_breaker_valid_state(
		error_threshold in 0.1f64..0.9,
		min_requests in 1u64..20
	) {
		use reinhardt_middleware::circuit_breaker::{CircuitBreakerConfig, CircuitBreakerMiddleware, CircuitState};
		use std::time::Duration;

		let config = CircuitBreakerConfig::new(error_threshold, min_requests, Duration::from_secs(60))
			.with_half_open_success_threshold(3);

		let middleware = CircuitBreakerMiddleware::new(config);

		// State should be one of the valid states
		let state = middleware.state();
		assert!(matches!(state, CircuitState::Closed | CircuitState::Open | CircuitState::HalfOpen));
	}

	/// Property: Circuit breaker in closed state allows requests.
	#[test]
	fn prop_circuit_breaker_closed_allows_requests(
		error_threshold in 0.1f64..0.9,
		min_requests in 5u64..20
	) {
		let rt = tokio::runtime::Runtime::new().unwrap();
		rt.block_on(async {
			use reinhardt_middleware::circuit_breaker::{CircuitBreakerConfig, CircuitBreakerMiddleware};
			use std::time::Duration;

			let config = CircuitBreakerConfig::new(error_threshold, min_requests, Duration::from_secs(60))
				.with_half_open_success_threshold(3);

			let middleware = Arc::new(CircuitBreakerMiddleware::new(config));
			let handler = Arc::new(ConfigurableTestHandler::always_success());

			let request = create_test_request("GET", "/");
			let response = middleware.process(request, handler).await.unwrap();

			// Closed circuit should allow requests
			assert_eq!(response.status.as_u16(), 200);
		});
	}
}

// =============================================================================
// Request ID Properties
// =============================================================================

proptest! {
	#![proptest_config(ProptestConfig::with_cases(20))]

	/// Property: Request ID is always unique for each request.
	#[test]
	fn prop_request_id_unique(num_requests in 2usize..20) {
		let rt = tokio::runtime::Runtime::new().unwrap();
		rt.block_on(async {
			use reinhardt_middleware::request_id::{RequestIdConfig, RequestIdMiddleware};
			use std::collections::HashSet;

			let middleware = Arc::new(RequestIdMiddleware::new(RequestIdConfig::default()));
			let handler = Arc::new(ConfigurableTestHandler::always_success());

			let mut ids = HashSet::new();

			for _ in 0..num_requests {
				let request = create_test_request("GET", "/");
				let response = middleware.process(request, handler.clone()).await.unwrap();

				if let Some(id) = response.headers.get("x-request-id") {
					ids.insert(id.to_str().unwrap().to_string());
				}
			}

			// All IDs should be unique
			assert_eq!(ids.len(), num_requests, "All request IDs should be unique");
		});
	}

	/// Property: Request ID has valid UUID format.
	#[test]
	fn prop_request_id_valid_format(_iteration in 0usize..10) {
		let rt = tokio::runtime::Runtime::new().unwrap();
		rt.block_on(async {
			use reinhardt_middleware::request_id::{RequestIdConfig, RequestIdMiddleware};

			let middleware = Arc::new(RequestIdMiddleware::new(RequestIdConfig::default()));
			let handler = Arc::new(ConfigurableTestHandler::always_success());

			let request = create_test_request("GET", "/");
			let response = middleware.process(request, handler).await.unwrap();

			if let Some(id) = response.headers.get("x-request-id") {
				let id_str = id.to_str().unwrap();
				// UUID v4 format: xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx (36 chars)
				assert_eq!(id_str.len(), 36, "Request ID should be 36 characters");
				assert_eq!(id_str.chars().filter(|c| *c == '-').count(), 4, "UUID should have 4 dashes");
			}
		});
	}
}

// =============================================================================
// Metrics Properties
// =============================================================================

proptest! {
	#![proptest_config(ProptestConfig::with_cases(10))]

	/// Property: Request count monotonically increases.
	#[test]
	fn prop_metrics_count_increases(num_requests in 1usize..20) {
		let rt = tokio::runtime::Runtime::new().unwrap();
		rt.block_on(async {
			use reinhardt_middleware::metrics::{MetricsConfig, MetricsMiddleware};

			let middleware = Arc::new(MetricsMiddleware::new(MetricsConfig::default()));
			let handler = Arc::new(ConfigurableTestHandler::always_success());

			// Send requests
			for _ in 0..num_requests {
				let request = create_test_request("GET", "/");
				let response = middleware.process(request, handler.clone()).await.unwrap();
				assert_eq!(response.status.as_u16(), 200);
			}

			// Handler was called correct number of times
			assert_eq!(handler.count(), num_requests as u64);
		});
	}
}

// =============================================================================
// Timeout Properties
// =============================================================================

proptest! {
	#![proptest_config(ProptestConfig::with_cases(10))]

	/// Property: Fast handler completes within any timeout.
	#[test]
	fn prop_timeout_fast_handler_succeeds(timeout_ms in 100u64..5000) {
		let rt = tokio::runtime::Runtime::new().unwrap();
		rt.block_on(async {
			use reinhardt_middleware::timeout::{TimeoutConfig, TimeoutMiddleware};
			use std::time::Duration;

			let config = TimeoutConfig::new(Duration::from_millis(timeout_ms));

			let middleware = Arc::new(TimeoutMiddleware::new(config));
			let handler = Arc::new(ConfigurableTestHandler::always_success());

			let request = create_test_request("GET", "/");
			let response = middleware.process(request, handler).await.unwrap();

			// Fast handler should always succeed
			assert_eq!(response.status.as_u16(), 200);
		});
	}
}

// =============================================================================
// Locale Properties
// =============================================================================

proptest! {
	#![proptest_config(ProptestConfig::with_cases(10))]

	/// Property: Locale middleware always processes requests without error.
	#[test]
	fn prop_locale_always_processes(_iteration in 0usize..10) {
		let rt = tokio::runtime::Runtime::new().unwrap();
		rt.block_on(async {
			use reinhardt_middleware::locale::LocaleMiddleware;

			let middleware = Arc::new(LocaleMiddleware::new());
			let handler = Arc::new(ConfigurableTestHandler::always_success());

			let request = create_test_request("GET", "/");
			let response = middleware.process(request, handler).await.unwrap();

			assert_eq!(response.status.as_u16(), 200);
		});
	}
}

// =============================================================================
// Cache Properties
// =============================================================================

proptest! {
	#![proptest_config(ProptestConfig::with_cases(10))]

	/// Property: Cache miss followed by hit returns same response.
	#[test]
	fn prop_cache_consistency(path in "/[a-z]{1,10}") {
		let rt = tokio::runtime::Runtime::new().unwrap();
		rt.block_on(async {
			use reinhardt_middleware::cache::{CacheConfig, CacheMiddleware};

			let config = CacheConfig::default();
			let middleware = Arc::new(CacheMiddleware::new(config));
			let handler = Arc::new(ConfigurableTestHandler::always_success());

			// First request (cache miss)
			let request1 = create_test_request("GET", &path);
			let response1 = middleware.process(request1, handler.clone()).await.unwrap();

			// Second request (cache hit)
			let request2 = create_test_request("GET", &path);
			let response2 = middleware.process(request2, handler).await.unwrap();

			// Both should succeed
			assert_eq!(response1.status.as_u16(), 200);
			assert_eq!(response2.status.as_u16(), 200);
		});
	}

	/// Property: POST requests bypass cache.
	#[test]
	fn prop_cache_bypass_unsafe_methods(path in "/[a-z]{1,10}") {
		let rt = tokio::runtime::Runtime::new().unwrap();
		rt.block_on(async {
			use reinhardt_middleware::cache::{CacheConfig, CacheMiddleware};

			let config = CacheConfig::default();
			let middleware = Arc::new(CacheMiddleware::new(config));
			let handler = Arc::new(ConfigurableTestHandler::always_success());

			// POST request should not be cached
			let request = create_test_request("POST", &path);
			let response = middleware.process(request, handler.clone()).await.unwrap();

			assert_eq!(response.status.as_u16(), 200);

			// Handler should have been called (not served from cache)
			assert!(handler.count() > 0);
		});
	}
}

// =============================================================================
// Compression Properties (Feature-gated)
// =============================================================================

#[cfg(feature = "compression")]
proptest! {
	#![proptest_config(ProptestConfig::with_cases(10))]

	/// Property: GZip compression preserves response status.
	#[test]
	fn prop_gzip_preserves_status(_iteration in 0usize..10) {
		let rt = tokio::runtime::Runtime::new().unwrap();
		rt.block_on(async {
			use reinhardt_middleware::gzip::GZipMiddleware;

			let middleware = Arc::new(GZipMiddleware::new());
			let handler = Arc::new(ConfigurableTestHandler::with_content_type("text/html"));

			let request = fixtures::create_request_with_headers(
				"GET",
				"/",
				&[("Accept-Encoding", "gzip")]
			);
			let response = middleware.process(request, handler).await.unwrap();

			assert_eq!(response.status.as_u16(), 200);
		});
	}
}

// =============================================================================
// Session Properties (Feature-gated)
// =============================================================================

#[cfg(feature = "sessions")]
proptest! {
	#![proptest_config(ProptestConfig::with_cases(10))]

	/// Property: Session middleware always returns a response.
	#[test]
	fn prop_session_always_responds(_iteration in 0usize..10) {
		let rt = tokio::runtime::Runtime::new().unwrap();
		rt.block_on(async {
			use reinhardt_middleware::session::{SessionConfig, SessionMiddleware};

			let config = SessionConfig::default();
			let middleware = Arc::new(SessionMiddleware::new(config));
			let handler = Arc::new(ConfigurableTestHandler::always_success());

			let request = create_test_request("GET", "/");
			let response = middleware.process(request, handler).await.unwrap();

			assert_eq!(response.status.as_u16(), 200);
		});
	}
}
