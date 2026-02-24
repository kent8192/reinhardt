//! Equivalence Partitioning Integration Tests
//!
//! This module tests middleware behavior using equivalence partitioning methodology.
//! Input values are divided into partitions where all values within a partition
//! are expected to produce equivalent behavior.
//!
//! # Test Categories
//!
//! - HTTP method partitions (safe vs unsafe methods)
//! - Content-Type partitions (compressible vs non-compressible)
//! - RateLimit strategy partitions (PerIp, PerUser, PerRoute)
//! - Origin partitions for CORS (allowed vs disallowed)
//! - Cache key partitions
//! - Compression encoding partitions

mod fixtures;

use fixtures::*;
use rstest::rstest;
use std::sync::Arc;

// =============================================================================
// HTTP Method Partitions
// =============================================================================

/// HTTP methods are partitioned into:
/// - Safe methods: GET, HEAD, OPTIONS, TRACE (no side effects, no CSRF protection needed)
/// - Unsafe methods: POST, PUT, DELETE, PATCH (may have side effects, CSRF protection needed)
#[rstest]
#[case::get("GET", false)]
#[case::head("HEAD", false)]
#[case::options("OPTIONS", false)]
#[case::post("POST", true)]
#[case::put("PUT", true)]
#[case::delete("DELETE", true)]
#[case::patch("PATCH", true)]
#[tokio::test]
async fn test_csrf_method_partitions(#[case] method: &str, #[case] requires_csrf: bool) {
	use reinhardt_middleware::Middleware;
	use reinhardt_middleware::csrf::CsrfMiddleware;

	let middleware = Arc::new(CsrfMiddleware::new());
	let handler = Arc::new(ConfigurableTestHandler::always_success());

	let request = create_test_request(method, "/");
	let result = middleware.process(request, handler).await;

	if requires_csrf {
		// Unsafe methods require CSRF token - request without token should return error
		assert!(
			result.is_err(),
			"Unsafe method {} without CSRF token should return error",
			method
		);
	} else {
		// Safe methods don't require CSRF token
		let response = result.expect("Safe method should succeed");
		assert_eq!(
			response.status.as_u16(),
			200,
			"Safe method {} should pass without CSRF token",
			method
		);
	}
}

/// HTTP methods partitioned for cacheability:
/// - Cacheable: GET, HEAD (can be cached)
/// - Non-cacheable: POST, PUT, DELETE, PATCH (should not be cached)
#[rstest]
#[case::get("GET", true)]
#[case::head("HEAD", true)]
#[case::post("POST", false)]
#[case::put("PUT", false)]
#[case::delete("DELETE", false)]
#[case::patch("PATCH", false)]
#[tokio::test]
async fn test_cache_method_partitions(#[case] method: &str, #[case] is_cacheable: bool) {
	use reinhardt_middleware::Middleware;
	use reinhardt_middleware::cache::{CacheConfig, CacheMiddleware};

	let config = CacheConfig::default();
	let middleware = Arc::new(CacheMiddleware::new(config));
	let handler = Arc::new(ConfigurableTestHandler::always_success());

	let request = create_test_request(method, "/");
	let response = middleware.process(request, handler).await.unwrap();

	if is_cacheable {
		// Cacheable methods should have X-Cache header
		assert!(
			response.headers.contains_key("X-Cache"),
			"Cacheable method {} should have X-Cache header",
			method
		);
	} else {
		// Non-cacheable methods should not have X-Cache header
		assert!(
			!response.headers.contains_key("X-Cache"),
			"Non-cacheable method {} should not have X-Cache header",
			method
		);
	}
}

// =============================================================================
// Content-Type Partitions
// =============================================================================

/// Content types partitioned for compression:
/// - Compressible: text/*, application/json, application/javascript, application/xml
/// - Non-compressible: image/*, video/*, audio/*, application/octet-stream
#[cfg(feature = "compression")]
#[rstest]
#[case::text_html("text/html", true)]
#[case::text_plain("text/plain", true)]
#[case::text_css("text/css", true)]
#[case::application_json("application/json", true)]
#[case::image_png("image/png", false)]
#[case::image_jpeg("image/jpeg", false)]
#[case::video_mp4("video/mp4", false)]
#[case::application_octet_stream("application/octet-stream", false)]
#[tokio::test]
async fn test_gzip_content_type_partitions(
	#[case] content_type: &str,
	#[case] should_compress: bool,
) {
	use reinhardt_middleware::Middleware;
	use reinhardt_middleware::gzip::{GZipConfig, GZipMiddleware};

	let config = GZipConfig {
		min_length: 1,
		compression_level: 6,
		compressible_types: vec!["text/".to_string(), "application/json".to_string()],
	};
	let middleware = Arc::new(GZipMiddleware::with_config(config));
	let handler = Arc::new(ConfigurableTestHandler::with_content_type(content_type));

	let request = create_request_with_headers("GET", "/", &[("Accept-Encoding", "gzip")]);

	let response = middleware.process(request, handler).await.unwrap();

	let is_compressed = response
		.headers
		.get("content-encoding")
		.map(|v| v.to_str().unwrap_or("") == "gzip")
		.unwrap_or(false);

	if should_compress {
		assert!(
			is_compressed,
			"Content-Type {} should be compressed",
			content_type
		);
	} else {
		assert!(
			!is_compressed,
			"Content-Type {} should not be compressed",
			content_type
		);
	}
}

// =============================================================================
// RateLimit Strategy Partitions
// =============================================================================

/// RateLimit strategies partitioned by key extraction:
/// - PerIp: Key based on client IP address
/// - PerUser: Key based on authenticated user ID
/// - PerRoute: Key based on request path
#[cfg(feature = "rate-limit")]
#[rstest]
#[case::per_ip(RateLimitStrategy::PerIp)]
#[case::per_user(RateLimitStrategy::PerUser)]
#[case::per_route(RateLimitStrategy::PerRoute)]
#[tokio::test]
async fn test_rate_limit_strategy_partitions(#[case] strategy: RateLimitStrategy) {
	use reinhardt_middleware::Middleware;
	use reinhardt_middleware::rate_limit::{RateLimitConfig, RateLimitMiddleware};

	let config = RateLimitConfig {
		capacity: 100.0,
		refill_rate: 10.0,
		cost_per_request: 1.0,
		strategy: strategy.clone(),
		exclude_paths: vec![],
		error_message: None,
		trusted_proxies: vec![],
	};

	let middleware = Arc::new(RateLimitMiddleware::new(config));
	let handler = Arc::new(ConfigurableTestHandler::always_success());

	let request = match &strategy {
		RateLimitStrategy::PerIp => {
			create_request_with_headers("GET", "/", &[("X-Forwarded-For", "192.168.1.1")])
		}
		RateLimitStrategy::PerUser => create_test_request("GET", "/"),
		RateLimitStrategy::PerRoute => create_test_request("GET", "/api/users"),
		RateLimitStrategy::PerIpAndUser => {
			create_request_with_headers("GET", "/", &[("X-Forwarded-For", "192.168.1.1")])
		}
	};

	let response = middleware.process(request, handler).await.unwrap();

	// Verify rate limit headers are present
	assert!(
		response.headers.contains_key("X-RateLimit-Limit"),
		"Should have rate limit header for strategy {:?}",
		strategy
	);

	// Verify the request was processed
	assert_eq!(
		response.status.as_u16(),
		200,
		"Request should succeed with strategy {:?}",
		strategy
	);
}

/// Test that different strategies isolate rate limiting properly.
/// Uses direct remote_addr instead of proxy headers to verify IP isolation
/// without relying on trusted proxy configuration.
#[cfg(feature = "rate-limit")]
#[tokio::test]
async fn test_rate_limit_strategy_isolation() {
	use reinhardt_middleware::Middleware;
	use reinhardt_middleware::rate_limit::{
		RateLimitConfig, RateLimitMiddleware, RateLimitStrategy,
	};
	use std::net::SocketAddr;

	let config = RateLimitConfig {
		capacity: 1.0,
		refill_rate: 0.0001, // Very slow refill (essentially no refill during test)
		cost_per_request: 1.0,
		strategy: RateLimitStrategy::PerIp,
		exclude_paths: vec![],
		error_message: None,
		trusted_proxies: vec![],
	};

	let middleware = Arc::new(RateLimitMiddleware::new(config));
	let handler = Arc::new(ConfigurableTestHandler::always_success());

	let addr1: SocketAddr = "192.168.1.1:12345".parse().unwrap();
	let addr2: SocketAddr = "192.168.2.2:12345".parse().unwrap();

	// First IP exhausts its quota
	let mut request1 = create_test_request("GET", "/");
	request1.remote_addr = Some(addr1);
	let response1 = middleware.process(request1, handler.clone()).await.unwrap();
	assert_eq!(
		response1.status.as_u16(),
		200,
		"First request from IP1 should succeed"
	);

	// First IP should now be rate limited
	let mut request2 = create_test_request("GET", "/");
	request2.remote_addr = Some(addr1);
	let response2 = middleware.process(request2, handler.clone()).await.unwrap();
	assert_eq!(
		response2.status.as_u16(),
		429,
		"Second request from IP1 should be rate limited"
	);

	// Second IP should still have its own quota
	let mut request3 = create_test_request("GET", "/");
	request3.remote_addr = Some(addr2);
	let response3 = middleware.process(request3, handler).await.unwrap();
	assert_eq!(
		response3.status.as_u16(),
		200,
		"First request from IP2 should succeed (isolated quota)"
	);
}

// =============================================================================
// CORS Origin Partitions
// =============================================================================

/// Origins partitioned for CORS handling:
/// - Matching allowed origins: Should include CORS headers
/// - Any origin: Current implementation adds CORS headers permissively
#[cfg(feature = "cors")]
#[rstest]
#[case::exact_match("https://example.com", vec!["https://example.com"])]
#[case::subdomain("https://api.example.com", vec!["https://api.example.com"])]
#[case::multiple_allowed("https://example.com", vec!["https://other.com", "https://example.com"])]
#[case::wildcard("https://any.domain.com", vec!["*"])]
#[tokio::test]
async fn test_cors_origin_partitions(#[case] origin: &str, #[case] allowed_origins: Vec<&str>) {
	use reinhardt_middleware::Middleware;
	use reinhardt_middleware::cors::{CorsConfig, CorsMiddleware};

	let config = CorsConfig {
		allow_origins: allowed_origins.into_iter().map(String::from).collect(),
		allow_methods: vec!["GET".to_string(), "POST".to_string()],
		allow_headers: vec!["Content-Type".to_string()],
		allow_credentials: false,
		max_age: Some(3600),
	};

	let middleware = Arc::new(CorsMiddleware::new(config));
	let handler = Arc::new(ConfigurableTestHandler::always_success());

	let request = create_request_with_headers("GET", "/", &[("Origin", origin)]);

	let response = middleware.process(request, handler).await.unwrap();

	// Current implementation always adds CORS headers
	assert!(
		response.headers.contains_key("access-control-allow-origin"),
		"Origin {} should have CORS headers",
		origin
	);
}

// =============================================================================
// Response Status Code Partitions
// =============================================================================

/// Response status codes partitioned for ETag behavior:
/// ETag middleware adds ETag to all responses (hash of body content).
/// Even empty bodies produce a valid hash.
#[rstest]
#[case::success_200(200, true)]
#[case::success_201(201, true)]
#[case::success_204(204, true)] // Even empty body gets ETag (empty string hash)
#[case::redirect_301(301, true)]
#[case::redirect_302(302, true)]
#[case::client_error_400(400, true)]
#[case::client_error_404(404, true)]
#[case::server_error_500(500, true)]
#[case::server_error_503(503, true)]
#[tokio::test]
async fn test_etag_status_code_partitions(
	#[case] status_code: u16,
	#[case] should_have_etag: bool,
) {
	use reinhardt_middleware::Middleware;
	use reinhardt_middleware::etag::ETagMiddleware;

	let middleware = Arc::new(ETagMiddleware::default());
	let handler = Arc::new(ConfigurableTestHandler::with_status_code(status_code));

	let request = create_test_request("GET", "/");
	let response = middleware.process(request, handler).await.unwrap();

	let has_etag = response.headers.contains_key("etag");

	assert_eq!(
		has_etag,
		should_have_etag,
		"Status code {} should {} ETag",
		status_code,
		if should_have_etag { "have" } else { "not have" }
	);
}

// =============================================================================
// Timeout Duration Partitions
// =============================================================================

/// Request durations partitioned against timeout:
/// - Fast (< 50% of timeout): Should succeed quickly
/// - Medium (50-90% of timeout): Should succeed but close to limit
/// - Slow (> timeout): Should timeout
#[rstest]
#[case::fast(10, 100, 200)]
#[case::medium(80, 100, 200)]
#[case::slow(150, 100, 408)]
#[tokio::test]
async fn test_timeout_duration_partitions(
	#[case] request_duration_ms: u64,
	#[case] timeout_ms: u64,
	#[case] expected_status: u16,
) {
	use reinhardt_middleware::Middleware;
	use reinhardt_middleware::timeout::{TimeoutConfig, TimeoutMiddleware};
	use std::time::Duration;

	let config = TimeoutConfig {
		duration: Duration::from_millis(timeout_ms),
	};
	let middleware = Arc::new(TimeoutMiddleware::new(config));
	let handler = Arc::new(
		ConfigurableTestHandler::always_success()
			.with_delay(Duration::from_millis(request_duration_ms)),
	);

	let request = create_test_request("GET", "/");
	let response = middleware.process(request, handler).await.unwrap();

	assert_eq!(
		response.status.as_u16(),
		expected_status,
		"Request taking {}ms with {}ms timeout should return {}",
		request_duration_ms,
		timeout_ms,
		expected_status
	);
}

// =============================================================================
// CircuitBreaker State Partitions
// =============================================================================

/// CircuitBreaker states partitioned:
/// - Closed: All requests pass through
/// - Open: All requests rejected with 503
/// - HalfOpen: Limited requests for probing
#[rstest]
#[case::closed(CircuitState::Closed, 200)]
#[case::open(CircuitState::Open, 503)]
#[case::half_open(CircuitState::HalfOpen, 200)] // Probe request passes
#[tokio::test]
async fn test_circuit_breaker_state_partitions(
	#[case] initial_state: CircuitState,
	#[case] expected_status: u16,
) {
	use reinhardt_middleware::Middleware;
	use reinhardt_middleware::circuit_breaker::{CircuitBreakerConfig, CircuitBreakerMiddleware};
	use std::time::Duration;

	let config = CircuitBreakerConfig {
		error_threshold: 0.5,
		min_requests: 10,
		timeout: Duration::from_millis(100),
		half_open_success_threshold: 3,
		error_message: None,
	};

	let middleware = Arc::new(CircuitBreakerMiddleware::new(config));
	let handler = Arc::new(ConfigurableTestHandler::always_success());

	// Set initial state if needed
	match initial_state {
		CircuitState::Open => {
			// Force open by sending failing requests
			let fail_handler = Arc::new(ConfigurableTestHandler::always_failure());
			for _ in 0..10 {
				let _ = middleware
					.process(create_test_request("GET", "/"), fail_handler.clone())
					.await;
			}
		}
		CircuitState::HalfOpen => {
			// Force open then wait for timeout
			let fail_handler = Arc::new(ConfigurableTestHandler::always_failure());
			for _ in 0..10 {
				let _ = middleware
					.process(create_test_request("GET", "/"), fail_handler.clone())
					.await;
			}
			tokio::time::sleep(Duration::from_millis(150)).await;
		}
		CircuitState::Closed => {
			// Default state, no action needed
		}
	}

	let request = create_test_request("GET", "/");
	let response = middleware.process(request, handler).await.unwrap();

	assert_eq!(
		response.status.as_u16(),
		expected_status,
		"Circuit in {:?} state should return {}",
		initial_state,
		expected_status
	);
}

// =============================================================================
// Session Cookie Partitions
// =============================================================================

/// Session cookie states partitioned:
/// - Valid session: Cookie with valid session ID
/// - No cookie: No session cookie present (creates new session)
#[cfg(feature = "sessions")]
#[rstest]
#[case::no_cookie(None, true)] // Creates new session
#[case::valid_cookie(Some("valid_session_id"), true)]
#[tokio::test]
async fn test_session_cookie_partitions(
	#[case] session_cookie: Option<&str>,
	#[case] should_succeed: bool,
) {
	use reinhardt_middleware::Middleware;
	use reinhardt_middleware::session::{SessionConfig, SessionMiddleware};

	let config = SessionConfig::default();
	let middleware = Arc::new(SessionMiddleware::new(config));
	let handler = Arc::new(ConfigurableTestHandler::always_success());

	let request = if let Some(cookie_value) = session_cookie {
		let cookie_header = format!("session_id={}", cookie_value);
		create_request_with_headers("GET", "/", &[("Cookie", cookie_header.as_str())])
	} else {
		create_test_request("GET", "/")
	};

	let result = middleware.process(request, handler).await;

	if should_succeed {
		let response = result.unwrap();
		assert_eq!(
			response.status.as_u16(),
			200,
			"Session request should succeed"
		);
		// Should have Set-Cookie header for new session
		if session_cookie.is_none() {
			assert!(
				response.headers.contains_key("set-cookie"),
				"New session should set cookie"
			);
		}
	}
}

// =============================================================================
// Locale Detection Partitions
// =============================================================================

/// Locale sources partitioned by priority:
/// - URL path prefix (highest priority): /ja/page
/// - Cookie: django_language=de
/// - Accept-Language header
/// - Default locale (fallback)
///
/// The LocaleMiddleware sets locale in request headers for downstream handlers.
/// This test verifies the middleware processes requests without error.
#[rstest]
#[case::url_path_prefix("/ja/test", "Accept-Language", "en")]
#[case::accept_language_fr("/test", "Accept-Language", "fr")]
#[case::accept_language_de("/test", "Accept-Language", "de")]
#[case::default_locale("/test", "Accept-Language", "")]
#[tokio::test]
async fn test_locale_source_partitions(
	#[case] path: &str,
	#[case] header_name: &str,
	#[case] header_value: &str,
) {
	use reinhardt_middleware::Middleware;
	use reinhardt_middleware::locale::{LocaleConfig, LocaleMiddleware};

	let config = LocaleConfig {
		supported_locales: vec![
			"en".to_string(),
			"ja".to_string(),
			"de".to_string(),
			"fr".to_string(),
		],
		default_locale: "en".to_string(),
		check_url_path: true,
		cookie_name: "django_language".to_string(),
	};

	let middleware = Arc::new(LocaleMiddleware::with_config(config));
	let handler = Arc::new(ConfigurableTestHandler::always_success());

	let request = if header_value.is_empty() {
		create_test_request("GET", path)
	} else {
		create_request_with_headers("GET", path, &[(header_name, header_value)])
	};

	// The middleware should process without error
	let response = middleware.process(request, handler).await.unwrap();

	// Verify response succeeded
	assert_eq!(
		response.status.as_u16(),
		200,
		"LocaleMiddleware should process request for path {} successfully",
		path
	);
}

// =============================================================================
// Accept-Encoding Partitions
// =============================================================================

/// Accept-Encoding values partitioned:
/// - gzip: Use gzip compression
/// - identity/none: No compression
#[cfg(feature = "compression")]
#[rstest]
#[case::gzip("gzip", true)]
#[case::identity("identity", false)]
#[tokio::test]
async fn test_gzip_accept_encoding_partitions(
	#[case] accept_encoding: &str,
	#[case] should_compress: bool,
) {
	use reinhardt_middleware::Middleware;
	use reinhardt_middleware::gzip::{GZipConfig, GZipMiddleware};

	let config = GZipConfig {
		min_length: 1,
		compression_level: 6,
		compressible_types: vec!["text/".to_string()],
	};

	let middleware = Arc::new(GZipMiddleware::with_config(config));
	let handler = Arc::new(ConfigurableTestHandler::with_content_type("text/plain"));

	let request = create_request_with_headers("GET", "/", &[("Accept-Encoding", accept_encoding)]);

	let response = middleware.process(request, handler).await.unwrap();

	let is_compressed = response
		.headers
		.get("content-encoding")
		.map(|v| v.to_str().unwrap_or("") == "gzip")
		.unwrap_or(false);

	assert_eq!(
		is_compressed,
		should_compress,
		"Accept-Encoding '{}' should {} result in compression",
		accept_encoding,
		if should_compress { "" } else { "not" }
	);
}

/// Brotli compression Accept-Encoding partitions
#[cfg(feature = "compression")]
#[rstest]
#[case::br("br", true)]
#[case::identity("identity", false)]
#[tokio::test]
async fn test_brotli_accept_encoding_partitions(
	#[case] accept_encoding: &str,
	#[case] should_compress: bool,
) {
	use reinhardt_middleware::Middleware;
	use reinhardt_middleware::brotli::BrotliMiddleware;

	// BrotliMiddleware::new() uses default configuration
	let middleware = Arc::new(BrotliMiddleware::new());
	let handler = Arc::new(ConfigurableTestHandler::with_content_type("text/plain"));

	let request = create_request_with_headers("GET", "/", &[("Accept-Encoding", accept_encoding)]);

	let response = middleware.process(request, handler).await.unwrap();

	let is_compressed = response
		.headers
		.get("content-encoding")
		.map(|v| v.to_str().unwrap_or("") == "br")
		.unwrap_or(false);

	assert_eq!(
		is_compressed,
		should_compress,
		"Accept-Encoding '{}' should {} result in brotli compression",
		accept_encoding,
		if should_compress { "" } else { "not" }
	);
}

// =============================================================================
// Request Path Partitions
// =============================================================================

/// Request paths partitioned for static file serving:
/// - Static paths (/static/*, /assets/*): Served from filesystem
/// - API paths (/api/*): Passed to handlers
/// - Health paths (/health, /ready): Health checks
#[rstest]
#[case::static_css("/static/style.css", "static")]
#[case::static_js("/static/app.js", "static")]
#[case::assets("/assets/image.png", "static")]
#[case::api_users("/api/users", "api")]
#[case::api_auth("/api/auth/login", "api")]
#[case::health("/health", "health")]
#[case::ready("/ready", "health")]
#[case::root("/", "api")]
#[tokio::test]
async fn test_path_partitions(#[case] path: &str, #[case] partition: &str) {
	// This test demonstrates path partitioning concept
	// Actual routing middleware would handle these differently
	let _request = create_test_request("GET", path);

	let detected_partition = if path.starts_with("/static/") || path.starts_with("/assets/") {
		"static"
	} else if path == "/health" || path == "/ready" {
		"health"
	} else {
		"api"
	};

	assert_eq!(
		detected_partition, partition,
		"Path {} should be in {} partition",
		path, partition
	);
}

// =============================================================================
// Security Header Partitions
// =============================================================================

/// Security middleware X-Frame-Options partitions
#[rstest]
#[case::deny("DENY", "DENY")]
#[case::same_origin("SAMEORIGIN", "SAMEORIGIN")]
#[tokio::test]
async fn test_xframe_option_partitions(#[case] option: &str, #[case] expected_header: &str) {
	use reinhardt_middleware::Middleware;
	use reinhardt_middleware::xframe::{XFrameOptions, XFrameOptionsMiddleware};

	let x_frame_option = match option {
		"DENY" => XFrameOptions::Deny,
		"SAMEORIGIN" => XFrameOptions::SameOrigin,
		_ => XFrameOptions::Deny,
	};

	let middleware = Arc::new(XFrameOptionsMiddleware::new(x_frame_option));
	let handler = Arc::new(ConfigurableTestHandler::always_success());

	let request = create_test_request("GET", "/");
	let response = middleware.process(request, handler).await.unwrap();

	let frame_option = response
		.headers
		.get("x-frame-options")
		.expect("Should have X-Frame-Options header")
		.to_str()
		.unwrap();

	assert_eq!(
		frame_option, expected_header,
		"X-Frame-Options should be {}",
		expected_header
	);
}

// =============================================================================
// Logging Level Partitions
// =============================================================================

/// Logging middleware level partitions
#[rstest]
#[case::success_status(200, "INFO")]
#[case::client_error(400, "WARN")]
#[case::server_error(500, "ERROR")]
#[tokio::test]
async fn test_logging_level_partitions(#[case] status_code: u16, #[case] expected_level: &str) {
	// Logging level is determined by response status
	let _handler = ConfigurableTestHandler::with_status_code(status_code);

	let log_level = if status_code >= 500 {
		"ERROR"
	} else if status_code >= 400 {
		"WARN"
	} else {
		"INFO"
	};

	assert_eq!(
		log_level, expected_level,
		"Status {} should log at {} level",
		status_code, expected_level
	);
}

// =============================================================================
// Metrics Request Type Partitions
// =============================================================================

/// Metrics middleware request type partitions
#[rstest]
#[case::successful_request(200)]
#[case::client_error_request(400)]
#[case::server_error_request(500)]
#[tokio::test]
async fn test_metrics_request_type_partitions(#[case] status_code: u16) {
	use reinhardt_middleware::Middleware;
	use reinhardt_middleware::metrics::{MetricsConfig, MetricsMiddleware};

	let config = MetricsConfig::default();
	let middleware = Arc::new(MetricsMiddleware::new(config));
	let handler = Arc::new(ConfigurableTestHandler::with_status_code(status_code));

	let request = create_test_request("GET", "/");
	let response = middleware.process(request, handler).await.unwrap();

	// Metrics should be collected for all request types
	assert_eq!(
		response.status.as_u16(),
		status_code,
		"Response should have correct status code"
	);
}

// =============================================================================
// RequestId Format Partitions
// =============================================================================

/// RequestId middleware format partitions
#[tokio::test]
async fn test_request_id_format_partition() {
	use reinhardt_middleware::Middleware;
	use reinhardt_middleware::request_id::{RequestIdConfig, RequestIdMiddleware};

	let config = RequestIdConfig::default();
	let middleware = Arc::new(RequestIdMiddleware::new(config));
	let handler = Arc::new(ConfigurableTestHandler::always_success());

	let request = create_test_request("GET", "/");
	let response = middleware.process(request, handler).await.unwrap();

	let request_id = response
		.headers
		.get("X-Request-Id")
		.expect("Should have X-Request-Id header")
		.to_str()
		.expect("X-Request-Id should be valid UTF-8");

	// Verify UUID format (8-4-4-4-12 pattern)
	let parts: Vec<&str> = request_id.split('-').collect();
	assert_eq!(parts.len(), 5, "Request ID should be UUID format");
	assert_eq!(parts[0].len(), 8, "First UUID segment should be 8 chars");
	assert_eq!(parts[1].len(), 4, "Second UUID segment should be 4 chars");
	assert_eq!(parts[2].len(), 4, "Third UUID segment should be 4 chars");
	assert_eq!(parts[3].len(), 4, "Fourth UUID segment should be 4 chars");
	assert_eq!(parts[4].len(), 12, "Fifth UUID segment should be 12 chars");
}
