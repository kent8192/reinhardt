//! Edge Cases Integration Tests for reinhardt-middleware
//!
//! This module tests middleware behavior in unusual or unexpected situations:
//! - Empty/null inputs
//! - Extreme values (very large responses, long strings)
//! - Concurrency issues
//! - Character encoding (Unicode, special characters)
//! - Protocol variations (HTTP/1.0, multiple header values)

mod fixtures;

use async_trait::async_trait;
use bytes::Bytes;
#[cfg(feature = "compression")]
use fixtures::assert_no_header;
use fixtures::{
	ConfigurableTestHandler, assert_has_header, assert_status, create_request_with_headers,
	create_test_request,
};
#[cfg(feature = "compression")]
use hyper::header::ACCEPT_ENCODING;
use hyper::header::CONTENT_TYPE;
use hyper::{HeaderMap, Method, Version};
use reinhardt_core::exception::Result;
use reinhardt_http::{Handler, Middleware};
use reinhardt_http::{Request, Response};
use reinhardt_middleware::cache::{CacheConfig, CacheKeyStrategy, CacheMiddleware};
use reinhardt_middleware::circuit_breaker::{CircuitBreakerConfig, CircuitBreakerMiddleware};
use reinhardt_middleware::etag::{ETagConfig, ETagMiddleware};
#[cfg(feature = "compression")]
use reinhardt_middleware::gzip::{GZipConfig, GZipMiddleware};
#[cfg(feature = "rate-limit")]
use reinhardt_middleware::rate_limit::{RateLimitConfig, RateLimitMiddleware, RateLimitStrategy};
use reinhardt_middleware::session::{SessionConfig, SessionMiddleware};
use reinhardt_middleware::timeout::{TimeoutConfig, TimeoutMiddleware};
use serial_test::serial;
use std::sync::Arc;
use std::time::Duration;

// ============================================================================
// Helper Handlers
// ============================================================================

/// Handler that returns an empty body.
struct EmptyBodyHandler;

#[async_trait]
impl Handler for EmptyBodyHandler {
	async fn handle(&self, _request: Request) -> Result<Response> {
		Ok(Response::ok())
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

/// Handler that echoes request headers.
/// Note: This handler is available for future edge case tests involving header manipulation.
#[allow(dead_code)]
struct HeaderEchoHandler;

#[async_trait]
impl Handler for HeaderEchoHandler {
	async fn handle(&self, request: Request) -> Result<Response> {
		let mut body = String::new();
		for (name, value) in request.headers.iter() {
			body.push_str(&format!(
				"{}: {}\n",
				name.as_str(),
				value.to_str().unwrap_or("<invalid>")
			));
		}
		Ok(Response::ok().with_body(Bytes::from(body)))
	}
}

// ============================================================================
// Empty Input Edge Cases
// ============================================================================

#[tokio::test]
async fn test_etag_empty_body() {
	let config = ETagConfig::default();
	let middleware = Arc::new(ETagMiddleware::new(config));
	let handler = Arc::new(EmptyBodyHandler);

	let request = create_test_request("GET", "/empty");
	let response = middleware.process(request, handler.clone()).await.unwrap();

	// ETag should still be generated for empty body (consistent hash)
	assert_status(&response, 200);
	assert_has_header(&response, "etag");
}

#[cfg(feature = "compression")]
#[tokio::test]
async fn test_gzip_empty_body() {
	let mut config = GZipConfig::default();
	config.min_length = 0; // Allow compression of any size
	config.compression_level = 6;
	config.compressible_types = vec!["text/".to_string()];
	let middleware = Arc::new(GZipMiddleware::with_config(config));
	let handler = Arc::new(ContentHandler::new("", "text/html"));

	let request = create_request_with_headers("GET", "/empty", &[("Accept-Encoding", "gzip")]);
	let response = middleware.process(request, handler.clone()).await.unwrap();

	// Empty body should not be compressed
	assert_status(&response, 200);
	assert_no_header(&response, "content-encoding");
}

#[tokio::test]
async fn test_cache_empty_response() {
	let config = CacheConfig::new(Duration::from_secs(60), CacheKeyStrategy::UrlAndMethod)
		.with_max_entries(100);
	let middleware = Arc::new(CacheMiddleware::new(config));
	let handler = Arc::new(EmptyBodyHandler);

	// First request - cache miss
	let request1 = create_test_request("GET", "/empty");
	let response1 = middleware.process(request1, handler.clone()).await.unwrap();
	assert_eq!(
		response1
			.headers
			.get("X-Cache")
			.and_then(|v| v.to_str().ok()),
		Some("MISS")
	);

	// Second request - should be cache hit
	let request2 = create_test_request("GET", "/empty");
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
// Unicode and Special Character Edge Cases
// ============================================================================

#[tokio::test]
async fn test_session_unicode_cookie_value() {
	let config = SessionConfig::new("session_id".to_string(), Duration::from_secs(60));
	let middleware = Arc::new(SessionMiddleware::new(config));
	let handler = Arc::new(ConfigurableTestHandler::always_success());

	// Request with Unicode characters in User-Agent (shouldn't affect session)
	let mut headers = HeaderMap::new();
	headers.insert(
		"User-Agent",
		"テスト・ブラウザ/1.0 (日本語)".parse().unwrap(),
	);

	let request = Request::builder()
		.method(Method::GET)
		.uri("/unicode-test")
		.headers(headers)
		.body(Bytes::new())
		.build()
		.unwrap();

	let response = middleware.process(request, handler.clone()).await.unwrap();

	// Session should be created successfully
	assert_status(&response, 200);
	assert_has_header(&response, "set-cookie");
}

#[tokio::test]
async fn test_cache_unicode_url() {
	let config = CacheConfig::new(Duration::from_secs(60), CacheKeyStrategy::UrlAndMethod)
		.with_max_entries(100);
	let middleware = Arc::new(CacheMiddleware::new(config));
	let handler = Arc::new(ContentHandler::new(
		"Japanese content: 日本語",
		"text/plain",
	));

	// Request with Unicode in URL
	let request1 = create_test_request("GET", "/api/search?q=%E6%97%A5%E6%9C%AC%E8%AA%9E");
	let response1 = middleware.process(request1, handler.clone()).await.unwrap();
	assert_eq!(
		response1
			.headers
			.get("X-Cache")
			.and_then(|v| v.to_str().ok()),
		Some("MISS")
	);

	// Same URL should hit cache
	let request2 = create_test_request("GET", "/api/search?q=%E6%97%A5%E6%9C%AC%E8%AA%9E");
	let response2 = middleware.process(request2, handler.clone()).await.unwrap();
	assert_eq!(
		response2
			.headers
			.get("X-Cache")
			.and_then(|v| v.to_str().ok()),
		Some("HIT")
	);
}

#[tokio::test]
async fn test_etag_unicode_body() {
	let config = ETagConfig::default();
	let middleware = Arc::new(ETagMiddleware::new(config));
	let handler = Arc::new(ContentHandler::new(
		"こんにちは世界！🌍🌏🌎",
		"text/plain; charset=utf-8",
	));

	let request = create_test_request("GET", "/unicode");
	let response = middleware.process(request, handler.clone()).await.unwrap();

	// ETag should be generated for Unicode content
	assert_status(&response, 200);
	assert_has_header(&response, "etag");

	// Verify the body is preserved correctly
	let body_str = String::from_utf8_lossy(&response.body);
	assert!(body_str.contains("こんにちは"));
	assert!(body_str.contains("🌍"));
}

// ============================================================================
// Multiple Header Values Edge Cases
// ============================================================================

#[cfg(feature = "compression")]
#[tokio::test]
async fn test_gzip_multiple_accept_encoding_headers() {
	let mut config = GZipConfig::default();
	config.min_length = 100;
	config.compression_level = 6;
	config.compressible_types = vec!["text/".to_string()];
	let middleware = Arc::new(GZipMiddleware::with_config(config));
	let handler = Arc::new(ContentHandler::new(
		"This is a test response body that is long enough to be compressed by gzip middleware.",
		"text/html",
	));

	// Multiple Accept-Encoding values
	let mut headers = HeaderMap::new();
	headers.append(ACCEPT_ENCODING, "deflate".parse().unwrap());
	headers.append(ACCEPT_ENCODING, "gzip".parse().unwrap());
	headers.append(ACCEPT_ENCODING, "br".parse().unwrap());

	let request = Request::builder()
		.method(Method::GET)
		.uri("/multi-encoding")
		.headers(headers)
		.body(Bytes::new())
		.build()
		.unwrap();

	let response = middleware.process(request, handler.clone()).await.unwrap();

	// Should recognize gzip from multiple values
	assert_status(&response, 200);
	// GZip middleware checks if "gzip" is contained in Accept-Encoding
}

#[cfg(feature = "compression")]
#[tokio::test]
async fn test_gzip_q_value_accept_encoding() {
	let mut config = GZipConfig::default();
	config.min_length = 100;
	config.compression_level = 6;
	config.compressible_types = vec!["text/".to_string()];
	let middleware = Arc::new(GZipMiddleware::with_config(config));
	let handler = Arc::new(ContentHandler::new(
		"This is a test response body that is long enough to be compressed by gzip middleware.",
		"text/html",
	));

	// Accept-Encoding with quality values
	let request = create_request_with_headers(
		"GET",
		"/q-value",
		&[("Accept-Encoding", "gzip;q=0.5, deflate;q=1.0, br;q=0.8")],
	);

	let response = middleware.process(request, handler.clone()).await.unwrap();

	assert_status(&response, 200);
	// Middleware should recognize gzip is acceptable
}

// ============================================================================
// HTTP Version Edge Cases
// ============================================================================

#[tokio::test]
async fn test_session_http10_client() {
	let config = SessionConfig::new("session".to_string(), Duration::from_secs(60));
	let middleware = Arc::new(SessionMiddleware::new(config));
	let handler = Arc::new(ConfigurableTestHandler::always_success());

	// HTTP/1.0 request
	let request = Request::builder()
		.method(Method::GET)
		.uri("/http10")
		.version(Version::HTTP_10)
		.body(Bytes::new())
		.build()
		.unwrap();

	let response = middleware.process(request, handler.clone()).await.unwrap();

	// Session should work with HTTP/1.0
	assert_status(&response, 200);
	assert_has_header(&response, "set-cookie");
}

#[tokio::test]
async fn test_cache_http10_client() {
	let config = CacheConfig::new(Duration::from_secs(60), CacheKeyStrategy::UrlAndMethod)
		.with_max_entries(100);
	let middleware = Arc::new(CacheMiddleware::new(config));
	let handler = Arc::new(ContentHandler::new("HTTP/1.0 content", "text/plain"));

	// HTTP/1.0 request
	let request = Request::builder()
		.method(Method::GET)
		.uri("/http10-cache")
		.version(Version::HTTP_10)
		.body(Bytes::new())
		.build()
		.unwrap();

	let response = middleware.process(request, handler.clone()).await.unwrap();

	// Cache should work with HTTP/1.0
	assert_status(&response, 200);
	assert_has_header(&response, "X-Cache");
}

// ============================================================================
// Concurrent Access Edge Cases
// ============================================================================

#[tokio::test]
#[serial(concurrent_session)]
async fn test_concurrent_session_creation() {
	let config = SessionConfig::new("concurrent_session".to_string(), Duration::from_secs(60));
	let middleware = Arc::new(SessionMiddleware::new(config));
	let handler = Arc::new(ConfigurableTestHandler::always_success());

	// Spawn multiple concurrent requests
	let mut handles = Vec::new();
	for i in 0..10 {
		let m = middleware.clone();
		let h = handler.clone();
		handles.push(tokio::spawn(async move {
			let request = Request::builder()
				.method(Method::GET)
				.uri(format!("/concurrent/{}", i))
				.body(Bytes::new())
				.build()
				.unwrap();
			m.process(request, h).await
		}));
	}

	// All requests should succeed
	let mut success_count = 0;
	for handle in handles {
		if let Ok(Ok(response)) = handle.await {
			if response.status.as_u16() == 200 {
				success_count += 1;
			}
		}
	}

	assert_eq!(
		success_count, 10,
		"All concurrent session creations should succeed"
	);
}

#[cfg(feature = "rate-limit")]
#[tokio::test]
#[serial(concurrent_rate_limit)]
async fn test_concurrent_rate_limit_enforcement() {
	let config =
		RateLimitConfig::new(RateLimitStrategy::PerIp, 5.0, 0.001).with_cost_per_request(1.0);
	let middleware = Arc::new(RateLimitMiddleware::new(config));
	let handler = Arc::new(ConfigurableTestHandler::always_success());

	// Spawn 10 concurrent requests (capacity is only 5)
	let mut handles = Vec::new();
	for _ in 0..10 {
		let m = middleware.clone();
		let h = handler.clone();
		handles.push(tokio::spawn(async move {
			let request = create_test_request("GET", "/concurrent-rate");
			m.process(request, h).await
		}));
	}

	// Count successes and failures
	let mut success_count = 0;
	let mut rate_limited_count = 0;
	for handle in handles {
		if let Ok(Ok(response)) = handle.await {
			if response.status.as_u16() == 200 {
				success_count += 1;
			} else if response.status.as_u16() == 429 {
				rate_limited_count += 1;
			}
		}
	}

	// Exactly 5 should succeed, 5 should be rate limited
	assert_eq!(
		success_count, 5,
		"Only {} requests should succeed with capacity 5",
		success_count
	);
	assert_eq!(
		rate_limited_count, 5,
		"Remaining {} requests should be rate limited",
		rate_limited_count
	);
}

#[tokio::test]
#[serial(concurrent_circuit_breaker)]
async fn test_concurrent_circuit_breaker_state() {
	let config = CircuitBreakerConfig::new(0.5, 3, Duration::from_secs(60))
		.with_half_open_success_threshold(1);
	let middleware = Arc::new(CircuitBreakerMiddleware::new(config));
	let handler = Arc::new(ConfigurableTestHandler::always_failure());

	// Spawn concurrent failing requests
	let mut handles = Vec::new();
	for _ in 0..10 {
		let m = middleware.clone();
		let h = handler.clone();
		handles.push(tokio::spawn(async move {
			let request = create_test_request("GET", "/concurrent-cb");
			m.process(request, h).await
		}));
	}

	// Wait for all to complete
	for handle in handles {
		let _ = handle.await;
	}

	// Circuit should be open after failures exceed threshold
	assert_eq!(
		middleware.state(),
		reinhardt_middleware::circuit_breaker::CircuitState::Open,
		"Circuit should be open after concurrent failures"
	);
}

// ============================================================================
// Large Value Edge Cases
// ============================================================================

#[cfg(feature = "compression")]
#[tokio::test]
async fn test_gzip_very_large_response() {
	let mut config = GZipConfig::default();
	config.min_length = 100;
	config.compression_level = 1; // Fast compression for large data
	config.compressible_types = vec!["text/".to_string()];
	let middleware = Arc::new(GZipMiddleware::with_config(config));

	// 10MB response body
	let body_size = 10 * 1024 * 1024;
	let body = "x".repeat(body_size);
	let handler = Arc::new(ContentHandler::new(body, "text/plain"));

	let request = create_request_with_headers("GET", "/large", &[("Accept-Encoding", "gzip")]);
	let response = middleware.process(request, handler.clone()).await.unwrap();

	assert_status(&response, 200);
	assert_has_header(&response, "content-encoding");
	assert!(
		response.body.len() < body_size,
		"Large body should be compressed"
	);
}

#[tokio::test]
async fn test_cache_many_entries() {
	let max_entries = 100;
	let config = CacheConfig::new(Duration::from_secs(60), CacheKeyStrategy::UrlAndMethod)
		.with_max_entries(max_entries);
	let middleware = Arc::new(CacheMiddleware::new(config));
	let handler = Arc::new(ContentHandler::new("cached content", "text/plain"));

	// Create more entries than max_entries
	for i in 0..(max_entries * 2) {
		let url = format!("/cache/{}", i);
		let request = create_test_request("GET", &url);
		let _ = middleware.process(request, handler.clone()).await;
	}

	// Should not panic or crash with many entries
	// Just verify middleware still works
	let request = create_test_request("GET", "/cache/test");
	let response = middleware.process(request, handler.clone()).await.unwrap();
	assert_status(&response, 200);
}

// ============================================================================
// Special Header Edge Cases
// ============================================================================

#[tokio::test]
async fn test_etag_conditional_request_with_quotes() {
	let config = ETagConfig::default();
	let middleware = Arc::new(ETagMiddleware::new(config));
	let handler = Arc::new(ContentHandler::new("test content", "text/plain"));

	// First request to get ETag
	let request1 = create_test_request("GET", "/etag-quotes");
	let response1 = middleware.process(request1, handler.clone()).await.unwrap();

	let etag = response1
		.headers
		.get("etag")
		.and_then(|v| v.to_str().ok())
		.map(|s| s.to_string());

	assert!(etag.is_some(), "ETag should be present");

	// Conditional request with proper quoted ETag
	let etag_value = etag.unwrap();
	let request2 = create_request_with_headers(
		"GET",
		"/etag-quotes",
		&[("If-None-Match", Box::leak(etag_value.into_boxed_str()))],
	);
	let response2 = middleware.process(request2, handler.clone()).await.unwrap();

	assert_eq!(
		response2.status.as_u16(),
		304,
		"Should return 304 Not Modified"
	);
}

#[tokio::test]
async fn test_session_malformed_cookie() {
	let config = SessionConfig::new("session".to_string(), Duration::from_secs(60));
	let middleware = Arc::new(SessionMiddleware::new(config));
	let handler = Arc::new(ConfigurableTestHandler::always_success());

	// Malformed cookie value
	let request =
		create_request_with_headers("GET", "/malformed", &[("Cookie", "session=;;;invalid===")]);

	let response = middleware.process(request, handler.clone()).await.unwrap();

	// Should handle malformed cookie gracefully and create new session
	assert_status(&response, 200);
	assert_has_header(&response, "set-cookie");
}

#[tokio::test]
async fn test_session_expired_cookie() {
	let config = SessionConfig::new("session".to_string(), Duration::from_millis(50));
	let middleware = Arc::new(SessionMiddleware::new(config));
	let handler = Arc::new(ConfigurableTestHandler::always_success());

	// Create a session
	let request1 = create_test_request("GET", "/create");
	let response1 = middleware.process(request1, handler.clone()).await.unwrap();

	let cookie = response1
		.headers
		.get("set-cookie")
		.and_then(|v| v.to_str().ok())
		.and_then(|s| s.split(';').next())
		.map(|s| s.to_string());

	assert!(cookie.is_some());

	// Wait for session to expire
	tokio::time::sleep(Duration::from_millis(100)).await;

	// Use expired cookie
	let cookie_str = cookie.unwrap();
	let request2 = create_request_with_headers(
		"GET",
		"/expired",
		&[("Cookie", Box::leak(cookie_str.into_boxed_str()))],
	);
	let response2 = middleware.process(request2, handler.clone()).await.unwrap();

	// Should create a new session
	assert_status(&response2, 200);
	assert_has_header(&response2, "set-cookie");
}

// ============================================================================
// Timeout Edge Cases
// ============================================================================

#[tokio::test]
async fn test_timeout_immediate_response() {
	let config = TimeoutConfig::new(Duration::from_secs(1));
	let middleware = Arc::new(TimeoutMiddleware::new(config));
	let handler = Arc::new(ConfigurableTestHandler::always_success());

	let request = create_test_request("GET", "/immediate");
	let result = middleware.process(request, handler.clone()).await;

	// Immediate response should succeed
	assert!(result.is_ok());
	assert_status(&result.unwrap(), 200);
}

#[tokio::test]
async fn test_timeout_just_before_deadline() {
	let config = TimeoutConfig::new(Duration::from_millis(200));
	let middleware = Arc::new(TimeoutMiddleware::new(config));
	let handler =
		Arc::new(ConfigurableTestHandler::always_success().with_delay(Duration::from_millis(50)));

	let request = create_test_request("GET", "/just-before");
	let result = middleware.process(request, handler.clone()).await;

	// Should complete before timeout
	assert!(result.is_ok());
	assert_status(&result.unwrap(), 200);
}

// ============================================================================
// Content-Type Edge Cases
// ============================================================================

#[cfg(feature = "compression")]
#[tokio::test]
async fn test_gzip_binary_content() {
	let mut config = GZipConfig::default();
	config.min_length = 10;
	config.compression_level = 6;
	config.compressible_types = vec!["text/".to_string(), "application/json".to_string()];
	let middleware = Arc::new(GZipMiddleware::with_config(config));

	// Binary content (image/png)
	let handler = Arc::new(ContentHandler::new(
		vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A], // PNG header
		"image/png",
	));

	let request = create_request_with_headers("GET", "/binary", &[("Accept-Encoding", "gzip")]);
	let response = middleware.process(request, handler.clone()).await.unwrap();

	// Binary content should not be compressed (not in compressible_types)
	assert_status(&response, 200);
	assert_no_header(&response, "content-encoding");
}

#[cfg(feature = "compression")]
#[tokio::test]
async fn test_gzip_charset_content_type() {
	let mut config = GZipConfig::default();
	config.min_length = 100;
	config.compression_level = 6;
	config.compressible_types = vec!["text/".to_string()];
	let middleware = Arc::new(GZipMiddleware::with_config(config));

	// Content-Type with charset
	let handler = Arc::new(ContentHandler::new(
		"This is a long text that should be compressed by gzip middleware. ".repeat(3),
		"text/html; charset=utf-8",
	));

	let request = create_request_with_headers("GET", "/charset", &[("Accept-Encoding", "gzip")]);
	let response = middleware.process(request, handler.clone()).await.unwrap();

	// Should compress despite charset in Content-Type
	assert_status(&response, 200);
	assert_has_header(&response, "content-encoding");
}

// ============================================================================
// Method Edge Cases
// ============================================================================

#[tokio::test]
async fn test_cache_head_request() {
	let config = CacheConfig::new(Duration::from_secs(60), CacheKeyStrategy::UrlAndMethod)
		.with_max_entries(100);
	let middleware = Arc::new(CacheMiddleware::new(config));
	let handler = Arc::new(ContentHandler::new("content", "text/plain"));

	// HEAD request
	let request = Request::builder()
		.method(Method::HEAD)
		.uri("/head-request")
		.body(Bytes::new())
		.build()
		.unwrap();

	let response = middleware.process(request, handler.clone()).await.unwrap();

	// HEAD requests may or may not be cached depending on implementation
	assert_status(&response, 200);
}

#[tokio::test]
async fn test_cache_post_not_cached() {
	let config = CacheConfig::new(Duration::from_secs(60), CacheKeyStrategy::UrlAndMethod)
		.with_max_entries(100);
	let middleware = Arc::new(CacheMiddleware::new(config));
	let handler = Arc::new(ContentHandler::new("post response", "text/plain"));

	// POST request should not be cached (by default)
	// Cache middleware skips non-cacheable methods entirely, so no X-Cache header is added
	let request = Request::builder()
		.method(Method::POST)
		.uri("/post-request")
		.body(Bytes::from("request body"))
		.build()
		.unwrap();

	let response1 = middleware.process(request, handler.clone()).await.unwrap();

	// Second POST request
	let request2 = Request::builder()
		.method(Method::POST)
		.uri("/post-request")
		.body(Bytes::from("request body"))
		.build()
		.unwrap();

	let response2 = middleware.process(request2, handler.clone()).await.unwrap();

	// POST requests are not cached - they bypass the cache logic entirely
	// so no X-Cache header is added
	assert!(
		response1.headers.get("X-Cache").is_none(),
		"POST request should not have X-Cache header (non-cacheable method)"
	);
	assert!(
		response2.headers.get("X-Cache").is_none(),
		"POST request should not have X-Cache header (non-cacheable method)"
	);

	// Both responses should succeed
	assert_status(&response1, 200);
	assert_status(&response2, 200);
}
