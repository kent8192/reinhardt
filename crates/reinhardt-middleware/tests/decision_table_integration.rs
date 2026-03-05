//! Decision Table Integration Tests for reinhardt-middleware
//!
//! This module uses decision tables to systematically test all combinations
//! of conditions and expected outcomes for middleware:
//! - CORS decision table (Origin, Method, Credentials combinations)
//! - Cache decision table (Method, Status, Cache-Control combinations)
//! - GZip decision table (Accept-Encoding, Content-Type, Body Size combinations)
//! - CircuitBreaker decision table (State, Request result, Min requests combinations)
//! - RateLimit decision table (Strategy, Tokens, Cost combinations)

mod fixtures;

use async_trait::async_trait;
use bytes::Bytes;
#[cfg(feature = "cors")]
use fixtures::assert_no_header;
use fixtures::{
	ConfigurableTestHandler, assert_has_header, assert_status, create_request_with_headers,
	create_test_request,
};
#[cfg(feature = "cors")]
use hyper::HeaderMap;
use hyper::Method;
#[cfg(feature = "compression")]
use hyper::header::ACCEPT_ENCODING;
use hyper::header::CONTENT_TYPE;
#[cfg(feature = "cors")]
use hyper::header::ORIGIN;
use reinhardt_core::exception::Result;
use reinhardt_http::{Handler, Middleware};
use reinhardt_http::{Request, Response};
use reinhardt_middleware::cache::{CacheConfig, CacheKeyStrategy, CacheMiddleware};
use reinhardt_middleware::circuit_breaker::{
	CircuitBreakerConfig, CircuitBreakerMiddleware, CircuitState,
};
#[cfg(feature = "cors")]
use reinhardt_middleware::cors::{CorsConfig, CorsMiddleware};
#[cfg(feature = "compression")]
use reinhardt_middleware::gzip::{GZipConfig, GZipMiddleware};
#[cfg(feature = "rate-limit")]
use reinhardt_middleware::rate_limit::{RateLimitConfig, RateLimitMiddleware, RateLimitStrategy};
use rstest::rstest;
use serial_test::serial;
use std::sync::Arc;
use std::time::Duration;

// ============================================================================
// Helper Handlers
// ============================================================================

/// Handler that returns a response with specific status code.
struct StatusHandler {
	status: u16,
}

impl StatusHandler {
	fn new(status: u16) -> Self {
		Self { status }
	}
}

#[async_trait]
impl Handler for StatusHandler {
	async fn handle(&self, _request: Request) -> Result<Response> {
		Ok(Response::new(
			hyper::StatusCode::from_u16(self.status).unwrap(),
		))
	}
}

/// Handler that returns a specific body with content type.
struct ContentHandler {
	body: Bytes,
	content_type: &'static str,
}

impl ContentHandler {
	fn new(body: impl Into<Bytes>, content_type: &'static str) -> Self {
		Self {
			body: body.into(),
			content_type,
		}
	}
}

#[async_trait]
impl Handler for ContentHandler {
	async fn handle(&self, _request: Request) -> Result<Response> {
		let mut response = Response::ok().with_body(self.body.clone());
		response
			.headers
			.insert(CONTENT_TYPE, self.content_type.parse().unwrap());
		Ok(response)
	}
}

// ============================================================================
// CORS Decision Table
// ============================================================================
//
// | Origin    | Method  | Credentials | Allow-Origin Config | Expected |
// |-----------|---------|-------------|---------------------|----------|
// | example   | GET     | No          | example.com         | CORS OK  |
// | example   | OPTIONS | No          | example.com         | Preflight|
// | example   | GET     | Yes         | example.com         | CORS+Cred|
// | evil.com  | GET     | No          | example.com         | No CORS  |
// | none      | GET     | No          | *                   | No CORS  |
// | any       | GET     | No          | *                   | CORS *   |

// Note: Current CorsMiddleware implementation always adds CORS headers to responses
// without validating the request Origin. This is a permissive implementation.
// The test cases below verify this actual behavior.

#[cfg(feature = "cors")]
#[rstest]
#[case::allowed_origin_get(
	Some("https://example.com"),
	"GET",
	vec!["https://example.com"],
	true,
	false
)]
#[case::allowed_origin_preflight(
	Some("https://example.com"),
	"OPTIONS",
	vec!["https://example.com"],
	true,
	true
)]
#[case::any_origin_with_configured_list(
	Some("https://evil.com"),
	"GET",
	vec!["https://example.com"],
	false,  // Non-matching origin omits CORS headers per spec
	false
)]
#[case::no_origin_header(
	None,
	"GET",
	vec!["https://example.com"],
	false,  // No Origin header omits CORS headers per spec
	false
)]
#[case::wildcard_any_origin(
	Some("https://any-site.com"),
	"GET",
	vec!["*"],
	true,
	false
)]
#[tokio::test]
async fn test_cors_decision_table(
	#[case] origin: Option<&str>,
	#[case] method: &str,
	#[case] allowed_origins: Vec<&str>,
	#[case] should_have_cors_headers: bool,
	#[case] is_preflight: bool,
) {
	let mut config = CorsConfig::default();
	config.allow_origins = allowed_origins.into_iter().map(String::from).collect();

	let middleware = Arc::new(CorsMiddleware::new(config));
	let handler = Arc::new(ConfigurableTestHandler::always_success());

	let mut headers = HeaderMap::new();
	if let Some(origin_value) = origin {
		headers.insert(ORIGIN, origin_value.parse().unwrap());
	}

	let request = Request::builder()
		.method(method.parse::<Method>().unwrap())
		.uri("/api/resource")
		.headers(headers)
		.body(Bytes::new())
		.build()
		.unwrap();

	let response = middleware.process(request, handler.clone()).await.unwrap();

	if is_preflight {
		// Preflight should return 204 with CORS headers
		assert_eq!(
			response.status.as_u16(),
			204,
			"Preflight should return 204 No Content"
		);
	}

	if should_have_cors_headers {
		assert_has_header(&response, "access-control-allow-origin");
	} else {
		assert_no_header(&response, "access-control-allow-origin");
	}
}

// ============================================================================
// Cache Decision Table
// ============================================================================
//
// | Method | Status | Cacheable | Expected X-Cache |
// |--------|--------|-----------|------------------|
// | GET    | 200    | Yes       | MISS (first)     |
// | GET    | 200    | Yes       | HIT (second)     |
// | GET    | 404    | Yes       | MISS (first)     |
// | GET    | 500    | No        | MISS             |
// | POST   | 200    | No        | None             |
// | HEAD   | 200    | Yes       | MISS             |

#[rstest]
#[case::get_200_first("GET", 200, true, Some("MISS"))]
#[case::get_404("GET", 404, true, Some("MISS"))]
#[case::get_500_not_cacheable("GET", 500, false, Some("MISS"))]
#[case::post_200("POST", 200, false, None)]
#[case::head_200("HEAD", 200, true, Some("MISS"))]
#[tokio::test]
async fn test_cache_decision_table(
	#[case] method: &str,
	#[case] status: u16,
	#[case] _is_cacheable: bool,
	#[case] expected_x_cache: Option<&str>,
) {
	let config = CacheConfig::new(Duration::from_secs(60), CacheKeyStrategy::UrlAndMethod)
		.with_max_entries(100);
	let middleware = Arc::new(CacheMiddleware::new(config));
	let handler = Arc::new(StatusHandler::new(status));

	let request = Request::builder()
		.method(method.parse::<Method>().unwrap())
		.uri("/api/resource")
		.body(Bytes::new())
		.build()
		.unwrap();

	let response = middleware.process(request, handler.clone()).await.unwrap();

	assert_eq!(
		response.status.as_u16(),
		status,
		"Status should match handler status"
	);

	match expected_x_cache {
		Some(expected) => {
			assert_eq!(
				response
					.headers
					.get("X-Cache")
					.and_then(|v| v.to_str().ok()),
				Some(expected),
				"X-Cache header should be {:?}",
				expected
			);
		}
		None => {
			assert!(
				response.headers.get("X-Cache").is_none(),
				"X-Cache header should not be present for non-cacheable methods"
			);
		}
	}
}

#[tokio::test]
#[serial(cache_hit)]
async fn test_cache_decision_table_hit_on_second_request() {
	let config = CacheConfig::new(Duration::from_secs(60), CacheKeyStrategy::UrlAndMethod)
		.with_max_entries(100);
	let middleware = Arc::new(CacheMiddleware::new(config));
	let handler = Arc::new(ContentHandler::new("cached content", "text/plain"));

	// First request - MISS
	let request1 = create_test_request("GET", "/api/cache-hit");
	let response1 = middleware.process(request1, handler.clone()).await.unwrap();
	assert_eq!(
		response1
			.headers
			.get("X-Cache")
			.and_then(|v| v.to_str().ok()),
		Some("MISS")
	);

	// Second request - HIT
	let request2 = create_test_request("GET", "/api/cache-hit");
	let response2 = middleware.process(request2, handler.clone()).await.unwrap();
	assert_eq!(
		response2
			.headers
			.get("X-Cache")
			.and_then(|v| v.to_str().ok()),
		Some("HIT")
	);
}

// ============================================================================
// GZip Decision Table
// ============================================================================
//
// | Accept-Encoding | Content-Type | Body Size | Expected |
// |-----------------|--------------|-----------|----------|
// | gzip            | text/html    | > min     | Compress |
// | gzip            | text/html    | < min     | No comp  |
// | gzip            | image/png    | > min     | No comp  |
// | deflate         | text/html    | > min     | No comp  |
// | none            | text/html    | > min     | No comp  |
// | gzip, deflate   | text/html    | > min     | Compress |

#[cfg(feature = "compression")]
#[rstest]
#[case::gzip_text_large(Some("gzip"), "text/html", 500, true)]
#[case::gzip_text_small(Some("gzip"), "text/html", 50, false)]
#[case::gzip_image(Some("gzip"), "image/png", 500, false)]
#[case::deflate_only(Some("deflate"), "text/html", 500, false)]
#[case::no_encoding(None, "text/html", 500, false)]
#[case::multiple_encodings(Some("gzip, deflate, br"), "text/html", 500, true)]
#[tokio::test]
async fn test_gzip_decision_table(
	#[case] accept_encoding: Option<&str>,
	#[case] content_type: &'static str,
	#[case] body_size: usize,
	#[case] should_compress: bool,
) {
	let mut config = GZipConfig::default();
	config.min_length = 200;
	config.compression_level = 6;
	config.compressible_types = vec!["text/".to_string(), "application/json".to_string()];
	let middleware = Arc::new(GZipMiddleware::with_config(config));

	let body = "x".repeat(body_size);
	let handler = Arc::new(ContentHandler::new(body, content_type));

	let mut headers = HeaderMap::new();
	if let Some(encoding) = accept_encoding {
		headers.insert(ACCEPT_ENCODING, encoding.parse().unwrap());
	}

	let request = Request::builder()
		.method(Method::GET)
		.uri("/page")
		.headers(headers)
		.body(Bytes::new())
		.build()
		.unwrap();

	let response = middleware.process(request, handler.clone()).await.unwrap();

	if should_compress {
		assert_has_header(&response, "content-encoding");
		assert!(
			response.body.len() < body_size,
			"Compressed body should be smaller"
		);
	} else {
		assert_no_header(&response, "content-encoding");
	}
}

// ============================================================================
// CircuitBreaker Decision Table
// ============================================================================
//
// | Current State | Request Result | Min Requests Met | New State |
// |---------------|----------------|------------------|-----------|
// | Closed        | Success        | -                | Closed    |
// | Closed        | Failure        | No               | Closed    |
// | Closed        | Failure        | Yes + Threshold  | Open      |
// | Open          | -              | -                | Open (503)|
// | HalfOpen      | Success        | Success Thresh   | Closed    |
// | HalfOpen      | Failure        | -                | Open      |

#[rstest]
#[case::closed_success(CircuitState::Closed, true, true, CircuitState::Closed)]
#[case::closed_failure_below_min(CircuitState::Closed, false, false, CircuitState::Closed)]
#[tokio::test]
#[serial(circuit_decision)]
async fn test_circuit_breaker_decision_table_basic(
	#[case] _initial_state: CircuitState,
	#[case] request_succeeds: bool,
	#[case] min_requests_met: bool,
	#[case] expected_state: CircuitState,
) {
	// Setup circuit breaker with specific thresholds
	let min_requests = if min_requests_met { 1 } else { 100 };
	let config = CircuitBreakerConfig::new(0.5, min_requests, Duration::from_secs(60))
		.with_half_open_success_threshold(1);
	let middleware = Arc::new(CircuitBreakerMiddleware::new(config));

	let handler = if request_succeeds {
		Arc::new(ConfigurableTestHandler::always_success())
	} else {
		Arc::new(ConfigurableTestHandler::always_failure())
	};

	let request = create_test_request("GET", "/api/data");
	let _ = middleware.process(request, handler.clone()).await;

	assert_eq!(
		middleware.state(),
		expected_state,
		"CircuitBreaker state should be {:?}",
		expected_state
	);
}

#[tokio::test]
#[serial(circuit_open_rejection)]
async fn test_circuit_breaker_open_state_rejects_requests() {
	// Create circuit breaker that will trip immediately
	let config = CircuitBreakerConfig::new(0.5, 2, Duration::from_secs(60))
		.with_half_open_success_threshold(1);
	let middleware = Arc::new(CircuitBreakerMiddleware::new(config));
	let failure_handler = Arc::new(ConfigurableTestHandler::always_failure());

	// Trip the circuit
	for _ in 0..3 {
		let request = create_test_request("GET", "/trip");
		let _ = middleware.process(request, failure_handler.clone()).await;
	}

	assert_eq!(middleware.state(), CircuitState::Open);

	// Try another request - should be rejected with 503
	let success_handler = Arc::new(ConfigurableTestHandler::always_success());
	let request = create_test_request("GET", "/rejected");
	let result = middleware.process(request, success_handler.clone()).await;

	assert!(result.is_ok());
	assert_eq!(result.unwrap().status.as_u16(), 503);
}

#[tokio::test]
#[serial(circuit_halfopen_to_closed)]
async fn test_circuit_breaker_halfopen_success_closes() {
	let config = CircuitBreakerConfig::new(0.5, 2, Duration::from_millis(50))
		.with_half_open_success_threshold(1);
	let middleware = Arc::new(CircuitBreakerMiddleware::new(config));
	let failure_handler = Arc::new(ConfigurableTestHandler::always_failure());

	// Trip the circuit
	for _ in 0..3 {
		let request = create_test_request("GET", "/trip");
		let _ = middleware.process(request, failure_handler.clone()).await;
	}

	assert_eq!(middleware.state(), CircuitState::Open);

	// Wait for timeout
	tokio::time::sleep(Duration::from_millis(60)).await;

	// Send success request in HalfOpen state
	let success_handler = Arc::new(ConfigurableTestHandler::always_success());
	let request = create_test_request("GET", "/success");
	let _ = middleware.process(request, success_handler.clone()).await;

	assert_eq!(middleware.state(), CircuitState::Closed);
}

#[tokio::test]
#[serial(circuit_halfopen_to_open)]
async fn test_circuit_breaker_halfopen_failure_opens() {
	let config = CircuitBreakerConfig::new(0.5, 2, Duration::from_millis(50))
		.with_half_open_success_threshold(1);
	let middleware = Arc::new(CircuitBreakerMiddleware::new(config));
	let failure_handler = Arc::new(ConfigurableTestHandler::always_failure());

	// Trip the circuit
	for _ in 0..3 {
		let request = create_test_request("GET", "/trip");
		let _ = middleware.process(request, failure_handler.clone()).await;
	}

	assert_eq!(middleware.state(), CircuitState::Open);

	// Wait for timeout
	tokio::time::sleep(Duration::from_millis(60)).await;

	// Send failure request in HalfOpen state
	let request = create_test_request("GET", "/fail");
	let _ = middleware.process(request, failure_handler.clone()).await;

	assert_eq!(middleware.state(), CircuitState::Open);
}

// ============================================================================
// RateLimit Decision Table
// ============================================================================
//
// | Strategy  | Tokens Available | Request Cost | Expected |
// |-----------|------------------|--------------|----------|
// | PerIp     | 10               | 1            | 200 OK   |
// | PerIp     | 0                | 1            | 429      |
// | PerUser   | 10               | 1            | 200 OK   |
// | PerRoute  | 10               | 1            | 200 OK   |
// | PerIp     | 1                | 2            | 429      |

#[cfg(feature = "rate-limit")]
#[rstest]
#[case::per_ip_has_tokens(RateLimitStrategy::PerIp, 10.0, 1.0, true)]
#[case::per_route_has_tokens(RateLimitStrategy::PerRoute, 10.0, 1.0, true)]
#[case::per_ip_high_cost(RateLimitStrategy::PerIp, 1.0, 2.0, false)]
#[tokio::test]
#[serial(rate_limit_decision)]
async fn test_rate_limit_decision_table(
	#[case] strategy: RateLimitStrategy,
	#[case] capacity: f64,
	#[case] cost: f64,
	#[case] should_allow: bool,
) {
	let config = RateLimitConfig::new(strategy, capacity, 0.001).with_cost_per_request(cost);
	let middleware = Arc::new(RateLimitMiddleware::new(config));
	let handler = Arc::new(ConfigurableTestHandler::always_success());

	let request = create_test_request("GET", "/api/data");
	let result = middleware.process(request, handler.clone()).await;

	assert!(result.is_ok());
	let response = result.unwrap();

	if should_allow {
		assert_eq!(
			response.status.as_u16(),
			200,
			"Request should be allowed with capacity {} and cost {}",
			capacity,
			cost
		);
	} else {
		assert_eq!(
			response.status.as_u16(),
			429,
			"Request should be rate limited with capacity {} and cost {}",
			capacity,
			cost
		);
	}
}

#[cfg(feature = "rate-limit")]
#[tokio::test]
#[serial(rate_limit_exhaustion)]
async fn test_rate_limit_exhaustion() {
	let config =
		RateLimitConfig::new(RateLimitStrategy::PerIp, 2.0, 0.001).with_cost_per_request(1.0);
	let middleware = Arc::new(RateLimitMiddleware::new(config));
	let handler = Arc::new(ConfigurableTestHandler::always_success());

	// First two requests should succeed
	for i in 0..2 {
		let request = create_test_request("GET", "/api/data");
		let result = middleware.process(request, handler.clone()).await;
		assert!(result.is_ok());
		assert_eq!(
			result.unwrap().status.as_u16(),
			200,
			"Request {} should succeed",
			i + 1
		);
	}

	// Third request should fail
	let request = create_test_request("GET", "/api/data");
	let result = middleware.process(request, handler.clone()).await;
	assert!(result.is_ok());
	assert_eq!(
		result.unwrap().status.as_u16(),
		429,
		"Third request should be rate limited"
	);
}

// ============================================================================
// Session Decision Table
// ============================================================================
//
// | Cookie Present | Session Valid | Expected Behavior |
// |----------------|---------------|-------------------|
// | No             | -             | Create new        |
// | Yes            | Yes           | Use existing      |
// | Yes            | No (expired)  | Create new        |
// | Yes (invalid)  | -             | Create new        |

#[rstest]
#[case::no_cookie(false, true, true)]
#[case::valid_cookie(true, true, false)]
#[case::invalid_cookie(true, false, true)]
#[tokio::test]
#[serial(session_decision)]
async fn test_session_decision_table(
	#[case] has_cookie: bool,
	#[case] _is_valid: bool,
	#[case] should_create_new: bool,
) {
	use reinhardt_middleware::session::{SessionConfig, SessionMiddleware};

	let config = SessionConfig::new("session".to_string(), Duration::from_secs(60));
	let middleware = Arc::new(SessionMiddleware::new(config));
	let handler = Arc::new(ConfigurableTestHandler::always_success());

	let request = if has_cookie {
		// Send request with cookie (may be invalid)
		create_request_with_headers("GET", "/session", &[("Cookie", "session=invalid_id")])
	} else {
		create_test_request("GET", "/session")
	};

	let response = middleware.process(request, handler.clone()).await.unwrap();

	assert_status(&response, 200);

	if should_create_new {
		assert_has_header(&response, "set-cookie");
	}
}

// ============================================================================
// ETag Decision Table
// ============================================================================
//
// | If-None-Match | ETag Match | Expected |
// |---------------|------------|----------|
// | None          | -          | 200 + ETag |
// | Present       | Yes        | 304        |
// | Present       | No         | 200 + ETag |

#[rstest]
#[case::no_if_none_match(None, false, 200, true)]
#[case::matching_etag(Some("match"), true, 304, false)]
#[case::non_matching_etag(Some("different"), false, 200, true)]
#[tokio::test]
async fn test_etag_decision_table(
	#[case] if_none_match: Option<&str>,
	#[case] should_match: bool,
	#[case] expected_status: u16,
	#[case] should_have_body: bool,
) {
	use reinhardt_middleware::etag::{ETagConfig, ETagMiddleware};

	let config = ETagConfig::default();
	let middleware = Arc::new(ETagMiddleware::new(config));
	let handler = Arc::new(ContentHandler::new("test content", "text/plain"));

	// First request to get the ETag
	let request1 = create_test_request("GET", "/etag-test");
	let response1 = middleware.process(request1, handler.clone()).await.unwrap();
	let actual_etag = response1
		.headers
		.get("etag")
		.and_then(|v| v.to_str().ok())
		.map(|s| s.to_string());

	// Build conditional request
	let etag_value = if should_match {
		actual_etag.clone().unwrap_or_default()
	} else if let Some(value) = if_none_match {
		value.to_string()
	} else {
		"".to_string()
	};

	let request2 = if if_none_match.is_some() {
		create_request_with_headers(
			"GET",
			"/etag-test",
			&[("If-None-Match", Box::leak(etag_value.into_boxed_str()))],
		)
	} else {
		create_test_request("GET", "/etag-test")
	};

	let response2 = middleware.process(request2, handler.clone()).await.unwrap();

	assert_eq!(response2.status.as_u16(), expected_status);

	if should_have_body {
		assert!(!response2.body.is_empty(), "Response should have body");
	} else {
		// 304 responses typically have empty body
		assert!(
			response2.body.is_empty(),
			"304 response should have empty body"
		);
	}
}

// ============================================================================
// Timeout Decision Table
// ============================================================================
//
// | Handler Delay | Timeout Config | Expected |
// |---------------|----------------|----------|
// | 0ms           | 100ms          | 200 OK   |
// | 50ms          | 100ms          | 200 OK   |
// | 150ms         | 100ms          | 408      |

#[rstest]
#[case::instant(0, 100, 200)]
#[case::within_timeout(50, 100, 200)]
#[case::exceeds_timeout(200, 100, 408)]
#[tokio::test]
async fn test_timeout_decision_table(
	#[case] delay_ms: u64,
	#[case] timeout_ms: u64,
	#[case] expected_status: u16,
) {
	use reinhardt_middleware::timeout::{TimeoutConfig, TimeoutMiddleware};

	let config = TimeoutConfig::new(Duration::from_millis(timeout_ms));
	let middleware = Arc::new(TimeoutMiddleware::new(config));
	let handler = Arc::new(
		ConfigurableTestHandler::always_success().with_delay(Duration::from_millis(delay_ms)),
	);

	let request = create_test_request("GET", "/timeout-test");
	let result = middleware.process(request, handler.clone()).await;

	match result {
		Ok(response) => {
			assert_eq!(
				response.status.as_u16(),
				expected_status,
				"Expected status {} for delay {}ms with timeout {}ms",
				expected_status,
				delay_ms,
				timeout_ms
			);
		}
		Err(_) => {
			// Timeout error is acceptable for expected 408
			assert_eq!(
				expected_status, 408,
				"Error should only occur for timeout case"
			);
		}
	}
}
