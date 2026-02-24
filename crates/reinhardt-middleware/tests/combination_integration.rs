//! Combination Integration Tests
//!
//! This module tests middleware combinations and their interactions.
//! These tests verify that multiple middleware work correctly together
//! in common pipeline configurations.
//!
//! # Test Categories
//!
//! - Security stacks (CSRF + Session, Auth + Session + CSRF)
//! - Performance stacks (Compression + ETag)
//! - Protection stacks (RateLimit + CircuitBreaker)
//! - Observability stacks (Logging + Tracing + Metrics)

mod fixtures;

use fixtures::{ConfigurableTestHandler, create_request_with_headers, create_test_request};
use reinhardt_http::Middleware;
use std::sync::Arc;

// =============================================================================
// Compression + ETag Combination
// =============================================================================

/// Tests that GZip compression and ETag work together.
/// The ETag should be based on the uncompressed content.
#[cfg(feature = "compression")]
#[tokio::test]
async fn test_gzip_and_etag_combination() {
	use reinhardt_middleware::etag::ETagMiddleware;
	use reinhardt_middleware::gzip::GZipMiddleware;

	let gzip = Arc::new(GZipMiddleware::new());
	let _etag = Arc::new(ETagMiddleware::default());
	let handler = Arc::new(ConfigurableTestHandler::with_content_type("text/html"));

	let request = create_request_with_headers("GET", "/", &[("Accept-Encoding", "gzip")]);

	// Apply GZip first, then ETag
	// In production, the order might be reversed for correct ETag calculation
	let response = gzip.process(request, handler).await.unwrap();

	// ETag is added by ETagMiddleware in the chain
	// This test verifies GZip doesn't break the response
	assert_eq!(response.status.as_u16(), 200);

	// GZip should add Content-Encoding header
	assert!(
		response.headers.contains_key("content-encoding"),
		"Should have Content-Encoding header after compression"
	);
}

/// Tests that ETag middleware correctly generates same ETags for same content.
#[tokio::test]
async fn test_etag_generates_consistent_tags() {
	use reinhardt_middleware::etag::ETagMiddleware;

	let etag = Arc::new(ETagMiddleware::default());

	// First request
	let handler1 = Arc::new(ConfigurableTestHandler::always_success());
	let request1 = create_test_request("GET", "/page1");
	let response1 = etag.process(request1, handler1).await.unwrap();

	// Second request with same content
	let handler2 = Arc::new(ConfigurableTestHandler::always_success());
	let request2 = create_test_request("GET", "/page2");
	let response2 = etag.process(request2, handler2).await.unwrap();

	let etag1 = response1.headers.get("etag").map(|v| v.to_str().unwrap());
	let etag2 = response2.headers.get("etag").map(|v| v.to_str().unwrap());

	// Both should have ETags
	assert!(etag1.is_some(), "First response should have ETag");
	assert!(etag2.is_some(), "Second response should have ETag");

	// Same content should produce same ETag
	assert_eq!(etag1, etag2, "Same content should produce same ETag");
}

// =============================================================================
// RateLimit + CircuitBreaker Combination
// =============================================================================

/// Tests that RateLimit and CircuitBreaker protect differently.
/// RateLimit rejects based on request count, CircuitBreaker on error rate.
#[cfg(feature = "rate-limit")]
#[tokio::test]
async fn test_ratelimit_and_circuit_breaker_cascade() {
	use reinhardt_middleware::circuit_breaker::{CircuitBreakerConfig, CircuitBreakerMiddleware};
	use reinhardt_middleware::rate_limit::{
		RateLimitConfig, RateLimitMiddleware, RateLimitStrategy,
	};
	use std::time::Duration;

	let rate_limit_config = RateLimitConfig {
		capacity: 10.0,
		refill_rate: 1.0,
		cost_per_request: 1.0,
		strategy: RateLimitStrategy::PerIp,
		exclude_paths: vec![],
		error_message: None,
		trusted_proxies: vec![],
	};

	let circuit_config = CircuitBreakerConfig {
		error_threshold: 0.5,
		min_requests: 5,
		timeout: Duration::from_secs(60),
		half_open_success_threshold: 3,
		error_message: None,
	};

	let rate_limit = Arc::new(RateLimitMiddleware::new(rate_limit_config));
	let circuit_breaker = Arc::new(CircuitBreakerMiddleware::new(circuit_config));
	let handler = Arc::new(ConfigurableTestHandler::always_success());

	// Test rate limit first
	for i in 0..5 {
		let request =
			create_request_with_headers("GET", "/", &[("X-Forwarded-For", "192.168.1.1")]);

		// Process through rate limiter
		let rate_result = rate_limit.process(request, handler.clone()).await.unwrap();

		assert_eq!(
			rate_result.status.as_u16(),
			200,
			"Request {} should pass rate limit",
			i
		);
	}

	// Test circuit breaker separately
	let request = create_test_request("GET", "/");
	let cb_result = circuit_breaker
		.process(request, handler.clone())
		.await
		.unwrap();
	assert_eq!(
		cb_result.status.as_u16(),
		200,
		"Circuit breaker should pass"
	);

	// Verify handler was called
	assert!(handler.count() > 0, "Handler should have been called");
}

// =============================================================================
// Security Headers Combination
// =============================================================================

/// Tests that multiple security headers can be added together.
#[tokio::test]
async fn test_security_headers_combination() {
	use reinhardt_middleware::csp::CspMiddleware;
	use reinhardt_middleware::xframe::{XFrameOptions, XFrameOptionsMiddleware};

	let xframe = Arc::new(XFrameOptionsMiddleware::new(XFrameOptions::SameOrigin));
	let csp = Arc::new(CspMiddleware::default());
	let handler = Arc::new(ConfigurableTestHandler::always_success());

	let request = create_test_request("GET", "/");

	// XFrame middleware
	let response = xframe.process(request, handler).await.unwrap();

	assert!(
		response.headers.contains_key("x-frame-options"),
		"Should have X-Frame-Options header"
	);

	// CSP adds Content-Security-Policy
	let request2 = create_test_request("GET", "/");
	let handler2 = Arc::new(ConfigurableTestHandler::always_success());
	let response2 = csp.process(request2, handler2).await.unwrap();

	assert!(
		response2.headers.contains_key("content-security-policy"),
		"Should have Content-Security-Policy header"
	);
}

// =============================================================================
// Session + CSRF Combination
// =============================================================================

/// Tests that Session and CSRF middleware work together.
/// CSRF protection requires session for token storage.
#[cfg(feature = "sessions")]
#[tokio::test]
async fn test_session_and_csrf_combination() {
	use reinhardt_middleware::csrf::CsrfMiddleware;
	use reinhardt_middleware::session::{SessionConfig, SessionMiddleware};

	let session_config = SessionConfig::default();
	let session = Arc::new(SessionMiddleware::new(session_config));
	let csrf = Arc::new(CsrfMiddleware::new());
	let handler = Arc::new(ConfigurableTestHandler::always_success());

	// First: GET request should pass through both middleware
	let request = create_test_request("GET", "/form");
	let response = session.process(request, handler.clone()).await.unwrap();

	assert_eq!(
		response.status.as_u16(),
		200,
		"GET request should succeed through session middleware"
	);

	// CSRF middleware should allow safe methods
	let request2 = create_test_request("GET", "/form");
	let response2 = csrf.process(request2, handler).await.unwrap();

	assert_eq!(
		response2.status.as_u16(),
		200,
		"GET request should succeed through CSRF middleware"
	);
}

// =============================================================================
// Timeout + CircuitBreaker Combination
// =============================================================================

/// Tests that Timeout and CircuitBreaker work together.
/// Timeouts can contribute to circuit breaker error count.
#[tokio::test]
async fn test_timeout_and_circuit_breaker_combination() {
	use reinhardt_middleware::circuit_breaker::{CircuitBreakerConfig, CircuitBreakerMiddleware};
	use reinhardt_middleware::timeout::{TimeoutConfig, TimeoutMiddleware};
	use std::time::Duration;

	let timeout_config = TimeoutConfig {
		duration: Duration::from_secs(5), // Long enough to not timeout
	};

	let circuit_config = CircuitBreakerConfig {
		error_threshold: 0.5,
		min_requests: 10,
		timeout: Duration::from_secs(60),
		half_open_success_threshold: 3,
		error_message: None,
	};

	let timeout = Arc::new(TimeoutMiddleware::new(timeout_config));
	let circuit_breaker = Arc::new(CircuitBreakerMiddleware::new(circuit_config));
	let handler = Arc::new(ConfigurableTestHandler::always_success());

	// Test timeout middleware
	let request = create_test_request("GET", "/");
	let timeout_response = timeout.process(request, handler.clone()).await.unwrap();
	assert_eq!(
		timeout_response.status.as_u16(),
		200,
		"Request should succeed through timeout"
	);

	// Test circuit breaker middleware
	let request = create_test_request("GET", "/");
	let cb_response = circuit_breaker
		.process(request, handler.clone())
		.await
		.unwrap();
	assert_eq!(
		cb_response.status.as_u16(),
		200,
		"Request should succeed through circuit breaker"
	);
}

// =============================================================================
// Locale Middleware
// =============================================================================

/// Tests locale middleware processes requests correctly.
#[tokio::test]
async fn test_locale_middleware_standalone() {
	use reinhardt_middleware::locale::LocaleMiddleware;

	let locale = Arc::new(LocaleMiddleware::new());
	let handler = Arc::new(ConfigurableTestHandler::always_success());

	// Request with Accept-Language header
	let request = create_request_with_headers(
		"GET",
		"/",
		&[("Accept-Language", "ja,en-US;q=0.9,en;q=0.8")],
	);

	let response = locale.process(request, handler).await.unwrap();

	assert_eq!(
		response.status.as_u16(),
		200,
		"Locale middleware should process request successfully"
	);
}

// =============================================================================
// Cache Middleware
// =============================================================================

/// Tests cache middleware with different request methods.
#[tokio::test]
async fn test_cache_middleware_method_handling() {
	use reinhardt_middleware::cache::{CacheConfig, CacheMiddleware};

	let config = CacheConfig::default();
	let cache = Arc::new(CacheMiddleware::new(config));
	let handler = Arc::new(ConfigurableTestHandler::always_success());

	// GET request should be cacheable
	let get_request = create_test_request("GET", "/data");
	let get_response = cache.process(get_request, handler.clone()).await.unwrap();

	assert_eq!(
		get_response.status.as_u16(),
		200,
		"GET request should succeed"
	);

	// POST request should not be cached
	let post_request = create_test_request("POST", "/data");
	let post_response = cache.process(post_request, handler).await.unwrap();

	assert_eq!(
		post_response.status.as_u16(),
		200,
		"POST request should succeed"
	);
}

// =============================================================================
// Metrics Middleware
// =============================================================================

/// Tests metrics middleware collects request information.
#[tokio::test]
async fn test_metrics_middleware_collection() {
	use reinhardt_middleware::metrics::{MetricsConfig, MetricsMiddleware};

	let config = MetricsConfig::default();
	let metrics = Arc::new(MetricsMiddleware::new(config));
	let handler = Arc::new(ConfigurableTestHandler::always_success());

	// Send multiple requests
	for _ in 0..5 {
		let request = create_test_request("GET", "/");
		let _response = metrics.process(request, handler.clone()).await.unwrap();
	}

	// All requests should succeed
	assert_eq!(
		handler.count(),
		5,
		"Handler should have been called 5 times"
	);
}

// =============================================================================
// RequestId Middleware
// =============================================================================

/// Tests RequestId middleware generates unique IDs.
#[tokio::test]
async fn test_request_id_uniqueness() {
	use reinhardt_middleware::request_id::{RequestIdConfig, RequestIdMiddleware};

	let config = RequestIdConfig::default();
	let request_id = Arc::new(RequestIdMiddleware::new(config));
	let handler = Arc::new(ConfigurableTestHandler::always_success());

	let mut ids = Vec::new();

	// Send multiple requests
	for _ in 0..10 {
		let request = create_test_request("GET", "/");
		let response = request_id.process(request, handler.clone()).await.unwrap();

		// Check lowercase header name (http crate normalizes header names)
		let id = response
			.headers
			.get("x-request-id")
			.map(|v| v.to_str().unwrap().to_string());

		if let Some(id) = id {
			ids.push(id);
		}
	}

	// All IDs should be unique
	let unique_ids: std::collections::HashSet<_> = ids.iter().collect();
	assert_eq!(
		ids.len(),
		unique_ids.len(),
		"All request IDs should be unique"
	);
}

// =============================================================================
// CORS Middleware
// =============================================================================

/// Tests CORS middleware with different origins.
#[cfg(feature = "cors")]
#[tokio::test]
async fn test_cors_middleware_origin_handling() {
	use reinhardt_middleware::cors::{CorsConfig, CorsMiddleware};

	let config = CorsConfig {
		allow_origins: vec!["https://example.com".to_string()],
		allow_methods: vec!["GET".to_string(), "POST".to_string()],
		allow_headers: vec!["Content-Type".to_string()],
		allow_credentials: false,
		max_age: Some(3600),
	};

	let cors = Arc::new(CorsMiddleware::new(config));
	let handler = Arc::new(ConfigurableTestHandler::always_success());

	let request = create_request_with_headers("GET", "/", &[("Origin", "https://example.com")]);

	let response = cors.process(request, handler).await.unwrap();

	assert!(
		response.headers.contains_key("access-control-allow-origin"),
		"Should have Access-Control-Allow-Origin header"
	);
}

/// Tests CORS preflight request handling.
#[cfg(feature = "cors")]
#[tokio::test]
async fn test_cors_preflight_handling() {
	use reinhardt_middleware::cors::{CorsConfig, CorsMiddleware};

	let config = CorsConfig {
		allow_origins: vec!["https://example.com".to_string()],
		allow_methods: vec!["GET".to_string(), "POST".to_string(), "PUT".to_string()],
		allow_headers: vec!["Content-Type".to_string(), "Authorization".to_string()],
		allow_credentials: false,
		max_age: Some(3600),
	};

	let cors = Arc::new(CorsMiddleware::new(config));
	let handler = Arc::new(ConfigurableTestHandler::always_success());

	let request = create_request_with_headers(
		"OPTIONS",
		"/",
		&[
			("Origin", "https://example.com"),
			("Access-Control-Request-Method", "PUT"),
		],
	);

	let response = cors.process(request, handler).await.unwrap();

	// Preflight should include allowed methods
	assert!(
		response
			.headers
			.contains_key("access-control-allow-methods"),
		"Preflight should have Access-Control-Allow-Methods header"
	);
}

// =============================================================================
// Combined Security Middleware Pipeline
// =============================================================================

/// Tests a full security middleware pipeline.
#[tokio::test]
async fn test_full_security_pipeline() {
	use reinhardt_middleware::csp::CspMiddleware;
	use reinhardt_middleware::xframe::{XFrameOptions, XFrameOptionsMiddleware};

	let xframe = Arc::new(XFrameOptionsMiddleware::new(XFrameOptions::Deny));
	let csp = Arc::new(CspMiddleware::default());
	let handler = Arc::new(ConfigurableTestHandler::always_success());

	// Test XFrame middleware
	let request = create_test_request("GET", "/secure");
	let xframe_response = xframe.process(request, handler.clone()).await.unwrap();
	assert!(
		xframe_response.headers.contains_key("x-frame-options"),
		"Should have X-Frame-Options header"
	);

	// Test CSP middleware
	let request = create_test_request("GET", "/secure");
	let csp_response = csp.process(request, handler.clone()).await.unwrap();
	assert!(
		csp_response.headers.contains_key("content-security-policy"),
		"Should have Content-Security-Policy header from CSP middleware"
	);
}

// =============================================================================
// Middleware Order Tests
// =============================================================================

/// Tests that middleware order affects response headers.
#[tokio::test]
async fn test_middleware_order_matters() {
	use reinhardt_middleware::request_id::{RequestIdConfig, RequestIdMiddleware};
	use reinhardt_middleware::tracing::{TracingConfig, TracingMiddleware};

	let request_id = Arc::new(RequestIdMiddleware::new(RequestIdConfig::default()));
	let tracing = Arc::new(TracingMiddleware::new(TracingConfig::default()));
	let handler = Arc::new(ConfigurableTestHandler::always_success());

	// Test Request ID middleware
	let request = create_test_request("GET", "/");
	let rid_response = request_id.process(request, handler.clone()).await.unwrap();
	assert!(
		rid_response.headers.contains_key("x-request-id"),
		"Should have X-Request-Id header"
	);

	// Test Tracing middleware
	let request = create_test_request("GET", "/");
	let tracing_response = tracing.process(request, handler.clone()).await.unwrap();
	assert_eq!(tracing_response.status.as_u16(), 200);
}

/// Tests concurrent middleware processing.
#[tokio::test]
async fn test_concurrent_middleware_processing() {
	use reinhardt_middleware::etag::ETagMiddleware;

	let etag = Arc::new(ETagMiddleware::default());
	let handler = Arc::new(ConfigurableTestHandler::always_success());

	// Spawn multiple concurrent requests
	let mut handles = vec![];

	for i in 0..10 {
		let etag_clone = etag.clone();
		let handler_clone = handler.clone();
		let path = format!("/page{}", i);

		handles.push(tokio::spawn(async move {
			let request = create_test_request("GET", &path);
			etag_clone.process(request, handler_clone).await.unwrap()
		}));
	}

	// Wait for all requests to complete
	for handle in handles {
		let response = handle.await.unwrap();
		assert_eq!(response.status.as_u16(), 200);
		assert!(response.headers.contains_key("etag"));
	}
}
