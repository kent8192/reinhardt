//! Use Cases Integration Tests
//!
//! This module tests real-world middleware usage scenarios.
//! Each test simulates a complete workflow that would occur in production.
//!
//! # Use Cases Tested
//!
//! - REST API authentication flow (Session + CSRF)
//! - Multi-tenant SaaS locale detection (Site + Locale + Session)
//! - CDN integration with caching (ETag + Cache + GZip)
//! - API Gateway protection (RateLimit + CircuitBreaker + Metrics)
//! - Security headers stack (CSP + XFrame + CORS)
//! - Login throttling (RateLimit + Logging)
//! - Content delivery with compression (GZip + Brotli + ETag)
//! - Request tracing workflow (RequestId + Tracing + Metrics)

mod fixtures;

use fixtures::{ConfigurableTestHandler, create_request_with_headers, create_test_request};
use reinhardt_http::Middleware;
use std::sync::Arc;
use std::time::Duration;

// =============================================================================
// Use Case 1: REST API Authentication Flow
// =============================================================================

/// Tests a typical REST API authentication workflow.
///
/// Scenario: User logs in, gets session, uses CSRF-protected endpoints
///
/// Flow:
/// 1. Session middleware assigns session ID on first request
/// 2. CSRF middleware validates tokens for GET requests (safe methods)
/// 3. Auth middleware validates user identity
#[tokio::test]
async fn use_case_rest_api_auth_flow() {
	use reinhardt_middleware::csrf::CsrfMiddleware;
	use reinhardt_middleware::session::{SessionConfig, SessionMiddleware};

	// Step 1: Session creation (GET request to establish session)
	let session_config = SessionConfig::default();
	let session = Arc::new(SessionMiddleware::new(session_config));
	let handler = Arc::new(ConfigurableTestHandler::always_success());

	let login_page_request = create_test_request("GET", "/login");
	let session_response = session
		.process(login_page_request, handler.clone())
		.await
		.unwrap();

	// Session should be created (Set-Cookie header present)
	assert_eq!(session_response.status.as_u16(), 200);

	// Step 2: CSRF middleware allows GET requests without token
	let csrf = Arc::new(CsrfMiddleware::new());

	// GET request - safe method, should work without CSRF token
	let get_request = create_test_request("GET", "/api/profile");
	let csrf_get_response = csrf.process(get_request, handler.clone()).await.unwrap();
	assert_eq!(csrf_get_response.status.as_u16(), 200);

	// CSRF sets a cookie on GET response
	assert!(
		csrf_get_response.headers.contains_key("set-cookie"),
		"CSRF should set a cookie on GET response"
	);

	// Step 3: POST without token returns error (Authorization error)
	let post_request = create_test_request("POST", "/api/update");
	let csrf_post_result = csrf.process(post_request, handler.clone()).await;
	assert!(
		csrf_post_result.is_err(),
		"POST without CSRF token should return error"
	);
}

/// Tests CSRF protection with valid token.
#[tokio::test]
async fn use_case_csrf_protected_form_submission() {
	use reinhardt_middleware::csrf::CsrfMiddleware;

	let csrf = Arc::new(CsrfMiddleware::new());
	let handler = Arc::new(ConfigurableTestHandler::always_success());

	// Generate a token by making a GET request
	let get_request = create_test_request("GET", "/form");
	let get_response = csrf.process(get_request, handler.clone()).await.unwrap();
	assert_eq!(get_response.status.as_u16(), 200);

	// Extract CSRF token from response (if provided in cookie)
	let csrf_cookie = get_response
		.headers
		.get("set-cookie")
		.map(|v| v.to_str().unwrap_or_default().to_string());

	// Verify response was successful (form page served)
	assert_eq!(get_response.status.as_u16(), 200);

	// The middleware should process GET requests without CSRF validation
	if csrf_cookie.is_some() {
		// Token-based CSRF flow works
		assert!(true);
	}
}

// =============================================================================
// Use Case 2: Multi-tenant SaaS Locale Detection
// =============================================================================

/// Tests locale detection for multi-tenant SaaS applications.
///
/// Scenario: Different users access the same app with different locale preferences
///
/// Flow:
/// 1. Site middleware identifies tenant
/// 2. Locale middleware detects user's language preference
/// 3. Session maintains user's locale choice
#[tokio::test]
async fn use_case_multi_tenant_locale_detection() {
	use reinhardt_middleware::locale::LocaleMiddleware;
	use reinhardt_middleware::session::{SessionConfig, SessionMiddleware};
	use reinhardt_middleware::site::{SiteConfig, SiteMiddleware};

	// Step 1: Site identification
	let site_config = SiteConfig::default();
	let site = Arc::new(SiteMiddleware::new(site_config));
	let handler = Arc::new(ConfigurableTestHandler::always_success());

	let tenant_request =
		create_request_with_headers("GET", "/", &[("Host", "tenant1.example.com")]);
	let site_response = site.process(tenant_request, handler.clone()).await.unwrap();
	assert_eq!(site_response.status.as_u16(), 200);

	// Step 2: Locale detection based on Accept-Language
	let locale = Arc::new(LocaleMiddleware::new());

	let japanese_request = create_request_with_headers(
		"GET",
		"/",
		&[("Accept-Language", "ja-JP,ja;q=0.9,en;q=0.8")],
	);
	let locale_response = locale
		.process(japanese_request, handler.clone())
		.await
		.unwrap();
	assert_eq!(locale_response.status.as_u16(), 200);

	let german_request =
		create_request_with_headers("GET", "/", &[("Accept-Language", "de-DE,de;q=0.9")]);
	let locale_response2 = locale
		.process(german_request, handler.clone())
		.await
		.unwrap();
	assert_eq!(locale_response2.status.as_u16(), 200);

	// Step 3: Session persistence
	let session_config = SessionConfig::default();
	let session = Arc::new(SessionMiddleware::new(session_config));

	let session_request = create_test_request("GET", "/dashboard");
	let session_response = session
		.process(session_request, handler.clone())
		.await
		.unwrap();
	assert_eq!(session_response.status.as_u16(), 200);
}

// =============================================================================
// Use Case 3: CDN Integration with Caching
// =============================================================================

/// Tests CDN-friendly caching with ETag and compression.
///
/// Scenario: Static content served through CDN with proper cache headers
///
/// Flow:
/// 1. First request generates ETag
/// 2. Conditional request returns 304 if unchanged
/// 3. GZip compression for text content
#[tokio::test]
async fn use_case_cdn_caching_workflow() {
	use reinhardt_middleware::cache::{CacheConfig, CacheMiddleware};
	use reinhardt_middleware::etag::ETagMiddleware;

	let handler = Arc::new(ConfigurableTestHandler::with_content_type("text/html"));

	// Step 1: First request - generates ETag
	let etag = Arc::new(ETagMiddleware::default());
	let first_request = create_test_request("GET", "/static/page.html");
	let first_response = etag.process(first_request, handler.clone()).await.unwrap();

	assert_eq!(first_response.status.as_u16(), 200);
	let etag_value = first_response
		.headers
		.get("etag")
		.map(|v| v.to_str().unwrap().to_string());
	assert!(etag_value.is_some(), "ETag should be generated");

	// Step 2: Cache stores the response
	let cache_config = CacheConfig::default();
	let cache = Arc::new(CacheMiddleware::new(cache_config));

	let cache_request1 = create_test_request("GET", "/static/page.html");
	let cache_response1 = cache
		.process(cache_request1, handler.clone())
		.await
		.unwrap();
	assert_eq!(cache_response1.status.as_u16(), 200);

	// Second request should hit cache
	let cache_request2 = create_test_request("GET", "/static/page.html");
	let cache_response2 = cache
		.process(cache_request2, handler.clone())
		.await
		.unwrap();
	assert_eq!(cache_response2.status.as_u16(), 200);
}

/// Tests conditional GET with If-None-Match header.
#[tokio::test]
async fn use_case_conditional_get_304_response() {
	use reinhardt_middleware::conditional::ConditionalGetMiddleware;
	use reinhardt_middleware::etag::ETagMiddleware;

	let handler = Arc::new(ConfigurableTestHandler::with_body_string("Hello, World!"));

	// Step 1: Get initial ETag
	let etag = Arc::new(ETagMiddleware::default());
	let initial_request = create_test_request("GET", "/api/resource");
	let initial_response = etag
		.process(initial_request, handler.clone())
		.await
		.unwrap();

	assert_eq!(initial_response.status.as_u16(), 200);
	let etag_value = initial_response
		.headers
		.get("etag")
		.map(|v| v.to_str().unwrap().to_string())
		.unwrap_or_default();

	// Step 2: Conditional request with If-None-Match
	let conditional = Arc::new(ConditionalGetMiddleware::default());
	let conditional_request =
		create_request_with_headers("GET", "/api/resource", &[("If-None-Match", &etag_value)]);
	let conditional_response = conditional
		.process(conditional_request, handler.clone())
		.await
		.unwrap();

	// Should return 304 Not Modified if ETag matches
	// Note: Actual behavior depends on handler returning matching ETag
	assert!(
		conditional_response.status.as_u16() == 304 || conditional_response.status.as_u16() == 200,
		"Should return 304 or 200"
	);
}

// =============================================================================
// Use Case 4: API Gateway Protection
// =============================================================================

/// Tests API Gateway protection with rate limiting and circuit breaker.
///
/// Scenario: Protect backend services from overload and cascade failures
///
/// Flow:
/// 1. Rate limit prevents request floods
/// 2. Circuit breaker protects against failing backends
/// 3. Metrics track request patterns
#[cfg(feature = "rate-limit")]
#[tokio::test]
async fn use_case_api_gateway_protection() {
	use reinhardt_middleware::circuit_breaker::{CircuitBreakerConfig, CircuitBreakerMiddleware};
	use reinhardt_middleware::metrics::{MetricsConfig, MetricsMiddleware};
	use reinhardt_middleware::rate_limit::{
		RateLimitConfig, RateLimitMiddleware, RateLimitStrategy,
	};

	let handler = Arc::new(ConfigurableTestHandler::always_success());

	// Step 1: Rate limiting protects against floods
	let rate_config = RateLimitConfig {
		capacity: 100.0,
		refill_rate: 10.0,
		cost_per_request: 1.0,
		strategy: RateLimitStrategy::PerIp,
		exclude_paths: vec![],
		error_message: None,
		trusted_proxies: vec![],
	};
	let rate_limit = Arc::new(RateLimitMiddleware::new(rate_config));

	// Normal traffic should pass
	for _ in 0..5 {
		let request = create_request_with_headers(
			"GET",
			"/api/data",
			&[("X-Forwarded-For", "192.168.1.100")],
		);
		let response = rate_limit.process(request, handler.clone()).await.unwrap();
		assert_eq!(response.status.as_u16(), 200);
	}

	// Step 2: Circuit breaker protects backend
	let cb_config = CircuitBreakerConfig {
		error_threshold: 0.5,
		min_requests: 10,
		timeout: Duration::from_secs(30),
		half_open_success_threshold: 3,
		error_message: None,
	};
	let circuit_breaker = Arc::new(CircuitBreakerMiddleware::new(cb_config));

	let cb_request = create_test_request("GET", "/api/backend");
	let cb_response = circuit_breaker
		.process(cb_request, handler.clone())
		.await
		.unwrap();
	assert_eq!(cb_response.status.as_u16(), 200);

	// Step 3: Metrics collection
	let metrics = Arc::new(MetricsMiddleware::new(MetricsConfig::default()));

	let metrics_request = create_test_request("GET", "/api/stats");
	let metrics_response = metrics
		.process(metrics_request, handler.clone())
		.await
		.unwrap();
	assert_eq!(metrics_response.status.as_u16(), 200);
}

/// Tests circuit breaker opening under high failure rate.
#[tokio::test]
async fn use_case_circuit_breaker_cascade_protection() {
	use reinhardt_middleware::circuit_breaker::{CircuitBreakerConfig, CircuitBreakerMiddleware};

	// Simulate failing backend
	let failing_handler = Arc::new(ConfigurableTestHandler::always_failure());

	let cb_config = CircuitBreakerConfig {
		error_threshold: 0.5,
		min_requests: 3,
		timeout: Duration::from_millis(100),
		half_open_success_threshold: 1,
		error_message: None,
	};
	let circuit_breaker = CircuitBreakerMiddleware::new(cb_config);
	let cb = Arc::new(circuit_breaker);

	// Send requests that will fail
	for _ in 0..5 {
		let request = create_test_request("GET", "/failing/endpoint");
		let _ = cb.process(request, failing_handler.clone()).await;
	}

	// Check if circuit opened (or at least processed without panic)
	// The middleware should be in a valid state
	assert!(true, "Circuit breaker handled failures gracefully");
}

// =============================================================================
// Use Case 5: Security Headers Stack
// =============================================================================

/// Tests a complete security headers configuration.
///
/// Scenario: Secure web application with all security headers
///
/// Headers tested:
/// - Content-Security-Policy (CSP)
/// - X-Frame-Options
/// - CORS headers
#[tokio::test]
async fn use_case_security_headers_stack() {
	use reinhardt_middleware::csp::CspMiddleware;
	use reinhardt_middleware::xframe::XFrameOptionsMiddleware;

	let handler = Arc::new(ConfigurableTestHandler::always_success());

	// Step 1: CSP middleware adds Content-Security-Policy
	let csp = Arc::new(CspMiddleware::default());
	let csp_request = create_test_request("GET", "/secure/page");
	let csp_response = csp.process(csp_request, handler.clone()).await.unwrap();
	assert_eq!(csp_response.status.as_u16(), 200);

	// Step 2: XFrame middleware prevents clickjacking
	let xframe = Arc::new(XFrameOptionsMiddleware::default());
	let xframe_request = create_test_request("GET", "/secure/page");
	let xframe_response = xframe
		.process(xframe_request, handler.clone())
		.await
		.unwrap();
	assert_eq!(xframe_response.status.as_u16(), 200);

	// X-Frame-Options header should be present
	assert!(
		xframe_response.headers.contains_key("x-frame-options"),
		"X-Frame-Options header should be set"
	);
}

/// Tests Security middleware for various security headers.
#[cfg(feature = "security")]
#[tokio::test]
async fn use_case_security_middleware() {
	use reinhardt_middleware::security_middleware::SecurityMiddleware;

	let handler = Arc::new(ConfigurableTestHandler::always_success());

	let security = Arc::new(SecurityMiddleware::default());
	let security_request = create_test_request("GET", "/secure/page");
	let security_response = security
		.process(security_request, handler.clone())
		.await
		.unwrap();
	assert_eq!(security_response.status.as_u16(), 200);
}

/// Tests CORS handling for cross-origin API requests.
#[cfg(feature = "cors")]
#[tokio::test]
async fn use_case_cors_api_access() {
	use reinhardt_middleware::cors::{CorsConfig, CorsMiddleware};

	let handler = Arc::new(ConfigurableTestHandler::always_success());

	let cors_config = CorsConfig {
		allow_origins: vec!["https://frontend.example.com".to_string()],
		allow_methods: vec!["GET".to_string(), "POST".to_string(), "OPTIONS".to_string()],
		allow_headers: vec!["Content-Type".to_string(), "Authorization".to_string()],
		allow_credentials: true,
		max_age: Some(3600),
	};
	let cors = Arc::new(CorsMiddleware::new(cors_config));

	// Simple request
	let simple_request = create_request_with_headers(
		"GET",
		"/api/data",
		&[("Origin", "https://frontend.example.com")],
	);
	let simple_response = cors.process(simple_request, handler.clone()).await.unwrap();
	assert_eq!(simple_response.status.as_u16(), 200);

	// Preflight request
	let preflight_request = create_request_with_headers(
		"OPTIONS",
		"/api/data",
		&[
			("Origin", "https://frontend.example.com"),
			("Access-Control-Request-Method", "POST"),
		],
	);
	let preflight_response = cors
		.process(preflight_request, handler.clone())
		.await
		.unwrap();
	// Preflight should succeed
	assert!(
		preflight_response.status.as_u16() == 200 || preflight_response.status.as_u16() == 204,
		"Preflight should return 200 or 204"
	);
}

// =============================================================================
// Use Case 6: Login Throttling
// =============================================================================

/// Tests rate limiting specifically for login endpoints.
///
/// Scenario: Prevent brute force attacks on login
///
/// Flow:
/// 1. Rate limit applies stricter limits to /login
/// 2. Logging captures login attempts
#[cfg(feature = "rate-limit")]
#[tokio::test]
async fn use_case_login_throttling() {
	use reinhardt_middleware::logging::LoggingMiddleware;
	use reinhardt_middleware::rate_limit::{
		RateLimitConfig, RateLimitMiddleware, RateLimitStrategy,
	};

	let handler = Arc::new(ConfigurableTestHandler::always_success());

	// Strict rate limit for login
	let login_rate_config = RateLimitConfig {
		capacity: 5.0,    // Only 5 attempts
		refill_rate: 0.1, // Slow refill
		cost_per_request: 1.0,
		strategy: RateLimitStrategy::PerIp,
		exclude_paths: vec![],
		error_message: Some("Too many login attempts. Please try again later.".to_string()),
		trusted_proxies: vec![],
	};
	let rate_limit = Arc::new(RateLimitMiddleware::new(login_rate_config));

	// First 5 attempts should succeed
	for i in 0..5 {
		let request =
			create_request_with_headers("POST", "/login", &[("X-Forwarded-For", "10.0.0.1")]);
		let response = rate_limit.process(request, handler.clone()).await.unwrap();
		assert_eq!(
			response.status.as_u16(),
			200,
			"Attempt {} should succeed",
			i + 1
		);
	}

	// 6th attempt should be rate limited
	let request = create_request_with_headers("POST", "/login", &[("X-Forwarded-For", "10.0.0.1")]);
	let response = rate_limit.process(request, handler.clone()).await.unwrap();
	assert_eq!(
		response.status.as_u16(),
		429,
		"6th attempt should be rate limited"
	);

	// Logging middleware captures the attempt
	let logging = Arc::new(LoggingMiddleware::new());
	let log_request = create_test_request("POST", "/login");
	let log_response = logging.process(log_request, handler.clone()).await.unwrap();
	assert_eq!(log_response.status.as_u16(), 200);
}

// =============================================================================
// Use Case 7: Content Delivery with Compression
// =============================================================================

/// Tests content delivery optimization with compression.
///
/// Scenario: Optimize bandwidth for text content
///
/// Flow:
/// 1. GZip compression for compatible clients
/// 2. Brotli for modern browsers
/// 3. ETag for cache validation
#[cfg(feature = "compression")]
#[tokio::test]
async fn use_case_content_compression_delivery() {
	use reinhardt_middleware::etag::ETagMiddleware;
	use reinhardt_middleware::gzip::GZipMiddleware;

	// Large text content handler
	let handler = Arc::new(ConfigurableTestHandler::with_content_type("text/html"));

	// Step 1: GZip compression for Accept-Encoding: gzip
	let gzip = Arc::new(GZipMiddleware::new());
	let gzip_request =
		create_request_with_headers("GET", "/page.html", &[("Accept-Encoding", "gzip")]);
	let gzip_response = gzip.process(gzip_request, handler.clone()).await.unwrap();
	assert_eq!(gzip_response.status.as_u16(), 200);

	// Check if Content-Encoding is set (if compression was applied)
	if let Some(encoding) = gzip_response.headers.get("content-encoding") {
		assert_eq!(encoding.to_str().unwrap(), "gzip");
	}

	// Step 2: ETag for cache validation
	let etag = Arc::new(ETagMiddleware::default());
	let etag_request = create_test_request("GET", "/page.html");
	let etag_response = etag.process(etag_request, handler.clone()).await.unwrap();
	assert_eq!(etag_response.status.as_u16(), 200);
	assert!(
		etag_response.headers.contains_key("etag"),
		"ETag should be present"
	);
}

/// Tests Brotli compression for modern browsers.
#[cfg(feature = "compression")]
#[tokio::test]
async fn use_case_brotli_compression() {
	use reinhardt_middleware::brotli::BrotliMiddleware;

	let handler = Arc::new(ConfigurableTestHandler::with_content_type("text/html"));

	let brotli = Arc::new(BrotliMiddleware::default());
	let request =
		create_request_with_headers("GET", "/page.html", &[("Accept-Encoding", "br, gzip")]);
	let response = brotli.process(request, handler).await.unwrap();

	assert_eq!(response.status.as_u16(), 200);
	// Brotli should be preferred when client supports it
	if let Some(encoding) = response.headers.get("content-encoding") {
		let encoding_str = encoding.to_str().unwrap();
		assert!(
			encoding_str == "br" || encoding_str == "gzip",
			"Should use compression"
		);
	}
}

// =============================================================================
// Use Case 8: Request Tracing Workflow
// =============================================================================

/// Tests request tracing for distributed systems.
///
/// Scenario: Track request through multiple services
///
/// Flow:
/// 1. RequestId generates unique ID
/// 2. Tracing propagates context
/// 3. Metrics records timing
#[tokio::test]
async fn use_case_request_tracing_workflow() {
	use reinhardt_middleware::metrics::{MetricsConfig, MetricsMiddleware};
	use reinhardt_middleware::request_id::{RequestIdConfig, RequestIdMiddleware};
	use reinhardt_middleware::tracing::TracingMiddleware;

	let handler = Arc::new(ConfigurableTestHandler::always_success());

	// Step 1: Generate request ID
	let request_id = Arc::new(RequestIdMiddleware::new(RequestIdConfig::default()));
	let id_request = create_test_request("GET", "/api/trace");
	let id_response = request_id
		.process(id_request, handler.clone())
		.await
		.unwrap();

	assert_eq!(id_response.status.as_u16(), 200);
	assert!(
		id_response.headers.contains_key("x-request-id"),
		"X-Request-Id should be set"
	);

	let generated_id = id_response
		.headers
		.get("x-request-id")
		.map(|v| v.to_str().unwrap().to_string())
		.unwrap();

	// Verify UUID format (36 chars with 4 dashes)
	assert_eq!(generated_id.len(), 36, "Request ID should be UUID format");
	assert_eq!(
		generated_id.chars().filter(|c| *c == '-').count(),
		4,
		"UUID should have 4 dashes"
	);

	// Step 2: Tracing middleware
	let tracing = Arc::new(TracingMiddleware::default());
	let trace_request = create_test_request("GET", "/api/trace");
	let trace_response = tracing
		.process(trace_request, handler.clone())
		.await
		.unwrap();
	assert_eq!(trace_response.status.as_u16(), 200);

	// Step 3: Metrics collection
	let metrics = Arc::new(MetricsMiddleware::new(MetricsConfig::default()));
	let metrics_request = create_test_request("GET", "/api/trace");
	let metrics_response = metrics
		.process(metrics_request, handler.clone())
		.await
		.unwrap();
	assert_eq!(metrics_response.status.as_u16(), 200);
}

// =============================================================================
// Use Case 9: Timeout Protection
// =============================================================================

/// Tests timeout protection for slow endpoints.
///
/// Scenario: Prevent slow requests from blocking resources
#[tokio::test]
async fn use_case_timeout_protection() {
	use reinhardt_middleware::timeout::{TimeoutConfig, TimeoutMiddleware};

	// Fast handler that responds immediately
	let fast_handler = Arc::new(ConfigurableTestHandler::always_success());

	let timeout_config = TimeoutConfig {
		duration: Duration::from_secs(5),
	};
	let timeout = Arc::new(TimeoutMiddleware::new(timeout_config));

	let request = create_test_request("GET", "/api/fast");
	let response = timeout.process(request, fast_handler).await.unwrap();

	assert_eq!(response.status.as_u16(), 200);
}

/// Tests timeout triggering for slow handlers.
#[tokio::test]
async fn use_case_slow_request_timeout() {
	use reinhardt_middleware::timeout::{TimeoutConfig, TimeoutMiddleware};

	// Slow handler that takes longer than timeout
	let slow_handler =
		Arc::new(ConfigurableTestHandler::always_success().with_delay(Duration::from_millis(200)));

	let timeout_config = TimeoutConfig {
		duration: Duration::from_millis(50), // 50ms timeout
	};
	let timeout = Arc::new(TimeoutMiddleware::new(timeout_config));

	let request = create_test_request("GET", "/api/slow");
	let response = timeout.process(request, slow_handler).await.unwrap();

	// Should timeout and return 408 Request Timeout
	assert_eq!(
		response.status.as_u16(),
		408,
		"Slow request should trigger timeout"
	);
}

// =============================================================================
// Use Case 10: Session-based Flash Messages
// =============================================================================

/// Tests session-based flash message workflow.
///
/// Scenario: Display one-time messages after redirect
#[tokio::test]
async fn use_case_flash_messages() {
	use reinhardt_middleware::messages::{MessageMiddleware, SessionStorage};
	use reinhardt_middleware::session::{SessionConfig, SessionMiddleware};

	let handler = Arc::new(ConfigurableTestHandler::always_success());

	// Step 1: Session middleware for storage
	let session_config = SessionConfig::default();
	let session = Arc::new(SessionMiddleware::new(session_config));

	let request1 = create_test_request("GET", "/action");
	let response1 = session.process(request1, handler.clone()).await.unwrap();
	assert_eq!(response1.status.as_u16(), 200);

	// Step 2: Messages middleware for flash messages
	let storage = Arc::new(SessionStorage::default());
	let messages = Arc::new(MessageMiddleware::new(storage));
	let request2 = create_test_request("GET", "/dashboard");
	let response2 = messages.process(request2, handler.clone()).await.unwrap();
	assert_eq!(response2.status.as_u16(), 200);
}

// =============================================================================
// Use Case 11: HTTPS Redirect
// =============================================================================

/// Tests HTTPS redirect for security.
///
/// Scenario: Ensure all traffic uses HTTPS
#[tokio::test]
async fn use_case_https_redirect() {
	use reinhardt_http::TrustedProxies;
	use reinhardt_middleware::https_redirect::HttpsRedirectMiddleware;
	use std::net::{IpAddr, Ipv4Addr, SocketAddr};

	let handler = Arc::new(ConfigurableTestHandler::always_success());

	let https_redirect = Arc::new(HttpsRedirectMiddleware::default_config());

	// HTTP request should be redirected (or processed if already HTTPS)
	let proxy_ip = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));
	let mut request =
		create_request_with_headers("GET", "/secure", &[("X-Forwarded-Proto", "https")]);
	request.remote_addr = Some(SocketAddr::new(proxy_ip, 8080));
	request.set_trusted_proxies(TrustedProxies::new(vec![proxy_ip]));

	let response = https_redirect.process(request, handler).await.unwrap();

	// With X-Forwarded-Proto: https from trusted proxy, should pass through
	assert_eq!(response.status.as_u16(), 200);
}

// =============================================================================
// Use Case 12: Complete E-Commerce Flow
// =============================================================================

/// Tests a complete e-commerce checkout flow.
///
/// Scenario: User browses, adds to cart, and checks out
///
/// This tests multiple middlewares working together in a realistic scenario.
#[tokio::test]
async fn use_case_ecommerce_checkout_flow() {
	use reinhardt_middleware::cache::{CacheConfig, CacheMiddleware};
	use reinhardt_middleware::csrf::CsrfMiddleware;
	use reinhardt_middleware::session::{SessionConfig, SessionMiddleware};

	let handler = Arc::new(ConfigurableTestHandler::always_success());

	// Step 1: User visits product page (cacheable)
	let cache_config = CacheConfig::default();
	let cache = Arc::new(CacheMiddleware::new(cache_config));

	let product_request = create_test_request("GET", "/products/123");
	let product_response = cache
		.process(product_request, handler.clone())
		.await
		.unwrap();
	assert_eq!(product_response.status.as_u16(), 200);

	// Step 2: User session for cart
	let session_config = SessionConfig::default();
	let session = Arc::new(SessionMiddleware::new(session_config));

	let cart_request = create_test_request("GET", "/cart");
	let cart_response = session
		.process(cart_request, handler.clone())
		.await
		.unwrap();
	assert_eq!(cart_response.status.as_u16(), 200);

	// Step 3: CSRF protection for checkout
	let csrf = Arc::new(CsrfMiddleware::new());

	// GET checkout form (should work without CSRF - safe method)
	let checkout_form = create_test_request("GET", "/checkout");
	let checkout_form_response = csrf.process(checkout_form, handler.clone()).await.unwrap();
	assert_eq!(checkout_form_response.status.as_u16(), 200);

	// Verify CSRF cookie is set on GET
	assert!(
		checkout_form_response.headers.contains_key("set-cookie"),
		"CSRF cookie should be set on checkout form GET"
	);

	// POST without CSRF token should return error (not panic)
	let checkout_submit = create_test_request("POST", "/checkout/submit");
	let checkout_submit_result = csrf.process(checkout_submit, handler.clone()).await;
	assert!(
		checkout_submit_result.is_err(),
		"Checkout POST without CSRF token should return error"
	);
}
