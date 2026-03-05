//! # Reinhardt Middleware
//!
//! Comprehensive HTTP middleware collection for the Reinhardt framework.
//!
//! ## Overview
//!
//! This crate provides a collection of middleware components for handling
//! cross-cutting concerns in web applications, including authentication,
//! security, caching, compression, and observability.
//!
//! ## Available Middleware
//!
//! ### Authentication & Authorization
//!
//! - **`AuthenticationMiddleware`**: Session-based user authentication
//!   (requires `sessions` feature)
//!
//! ### Security
//!
//! - **`CorsMiddleware`**: Cross-Origin Resource Sharing (requires `cors` feature)
//! - **[`CsrfMiddleware`]**: CSRF protection with token validation
//! - **[`CspMiddleware`]**: Content Security Policy headers
//! - **[`XFrameOptionsMiddleware`]**: Clickjacking protection via X-Frame-Options header
//! - **[`HttpsRedirectMiddleware`]**: Force HTTPS connections
//! - **`SecurityMiddleware`**: Combined security headers (requires `security` feature)
//!
//! ### Performance & Caching
//!
//! - **[`CacheMiddleware`]**: HTTP response caching with configurable strategies
//! - **`GZipMiddleware`**: Gzip compression (requires `compression` feature)
//! - **`BrotliMiddleware`**: Brotli compression (requires `compression` feature)
//! - **[`ETagMiddleware`]**: ETag generation and validation for conditional requests
//! - **[`ConditionalGetMiddleware`]**: Conditional GET support with Last-Modified headers
//!
//! ### Observability
//!
//! - **[`LoggingMiddleware`]**: Request/response logging with configurable formats
//! - **[`TracingMiddleware`]**: Distributed tracing with trace/span ID propagation
//! - **[`MetricsMiddleware`]**: Performance metrics collection
//! - **[`RequestIdMiddleware`]**: Unique request ID generation
//!
//! ### Rate Limiting & Resilience
//!
//! - **`RateLimitMiddleware`**: API rate limiting with multiple strategies
//!   (requires `rate-limit` feature)
//! - **[`CircuitBreakerMiddleware`]**: Circuit breaker pattern for fault tolerance
//! - **[`TimeoutMiddleware`]**: Request timeout handling
//!
//! ### Session & State
//!
//! - **[`SessionMiddleware`]**: Session management with pluggable storage backends
//! - **[`SiteMiddleware`]**: Multi-site support with site identification
//! - **[`LocaleMiddleware`]**: Internationalization and locale detection
//!
//! ### Utility
//!
//! - **[`CommonMiddleware`]**: Common HTTP functionality (trailing slashes, URL normalization)
//! - **[`BrokenLinkEmailsMiddleware`]**: Broken link notification via email
//! - **[`FlatpagesMiddleware`]**: Static page serving from database
//! - **[`RedirectFallbackMiddleware`]**: Fallback redirect handling
//!
//! ## Quick Start
//!
//! ```rust,ignore
//! use reinhardt_middleware::{LoggingMiddleware, CsrfMiddleware};
//! use reinhardt_core::types::MiddlewareChain;
//! use std::sync::Arc;
//!
//! // Create individual middleware instances
//! let logging = Arc::new(LoggingMiddleware::new());
//! let csrf = Arc::new(CsrfMiddleware::default());
//!
//! // Build middleware chain (wraps around your handler)
//! let chain = MiddlewareChain::new(handler)
//!     .with_middleware(logging)
//!     .with_middleware(csrf);
//! ```
//!
//! ## Architecture
//!
//! Key modules in this crate:
//!
//! - [`allowed_hosts`]: Restrict requests to configured host names
//! - [`auth`]: Session-based user authentication (requires `sessions` feature)
//! - [`cache`]: HTTP response caching with configurable key strategies
//! - [`circuit_breaker`]: Circuit breaker pattern for fault-tolerant backends
//! - [`common`]: Common HTTP functionality (trailing slash, URL normalization)
//! - [`cors`]: Cross-Origin Resource Sharing headers (requires `cors` feature)
//! - [`csp`]: Content Security Policy header generation
//! - [`csrf`]: CSRF token validation and protection
//! - [`etag`]: ETag generation and conditional request handling
//! - [`logging`]: Structured request/response logging
//! - [`metrics`]: Performance metrics collection and export
//! - [`rate_limit`]: API rate limiting (requires `rate-limit` feature)
//! - [`request_id`]: Unique request ID generation and propagation
//! - [`session`]: Session management with pluggable storage backends
//! - [`timeout`]: Request timeout enforcement
//! - [`tracing`]: Distributed tracing with trace/span ID propagation
//! - [`xframe`]: X-Frame-Options clickjacking protection
//!
//! ## Feature Flags
//!
//! | Feature | Default | Description |
//! |---------|---------|-------------|
//! | `cors` | disabled | Cross-Origin Resource Sharing middleware |
//! | `compression` | disabled | GZip and Brotli compression middleware |
//! | `rate-limit` | disabled | API rate limiting middleware |
//! | `security` | disabled | Combined security headers middleware |
//! | `sessions` | disabled | Session-based authentication middleware |
//! | `sqlx` | disabled | Database-backed session storage via SQLx |
//! | `full` | disabled | Enables all middleware features |
//!
//! ## Middleware Ordering
//!
//! Middleware execution order matters. A typical recommended order:
//!
//! 1. `RequestIdMiddleware` - Generate request ID first
//! 2. `LoggingMiddleware` - Log all requests
//! 3. `TracingMiddleware` - Start tracing span
//! 4. `SecurityMiddleware` - Apply security headers
//! 5. `CorsMiddleware` - Handle CORS preflight
//! 6. `SessionMiddleware` - Load session
//! 7. `AuthenticationMiddleware` - Authenticate user
//! 8. `CsrfMiddleware` - Validate CSRF token
//! 9. `RateLimitMiddleware` - Apply rate limits
//! 10. Application handlers

pub mod allowed_hosts;
pub mod auth;
pub mod broken_link;
#[cfg(feature = "compression")]
pub mod brotli;
pub mod cache;
pub mod circuit_breaker;
pub mod common;
pub mod conditional;
#[cfg(feature = "cors")]
pub mod cors;
pub mod csp;
pub mod csp_helpers;
pub mod csrf;
pub mod etag;
pub mod flatpages;
#[cfg(feature = "compression")]
pub mod gzip;
pub mod honeypot;
pub mod https_redirect;
pub mod locale;
pub mod logging;
pub mod messages;
pub mod metrics;
#[cfg(feature = "rate-limit")]
pub mod rate_limit;
pub mod redirect_fallback;
pub mod request_id;
#[cfg(feature = "security")]
pub mod security_middleware;
pub mod session;
pub mod site;
pub mod timeout;
pub mod tracing;
pub mod xframe;
pub mod xss;

// Re-export core middleware traits from reinhardt-http
pub use reinhardt_http::{Handler, Middleware, MiddlewareChain};

pub use allowed_hosts::{AllowedHostsConfig, AllowedHostsMiddleware};
#[cfg(feature = "sessions")]
pub use auth::AuthenticationMiddleware;
pub use broken_link::{BrokenLinkConfig, BrokenLinkEmailsMiddleware};
#[cfg(feature = "compression")]
pub use brotli::{BrotliConfig, BrotliMiddleware, BrotliQuality};
pub use cache::{CacheConfig, CacheKeyStrategy, CacheMiddleware, CacheStore};
pub use circuit_breaker::{CircuitBreakerConfig, CircuitBreakerMiddleware, CircuitState};
pub use common::{CommonConfig, CommonMiddleware};
pub use conditional::ConditionalGetMiddleware;
#[cfg(feature = "cors")]
pub use cors::CorsMiddleware;
pub use csp::{CspConfig, CspMiddleware, CspNonce};
pub use csp_helpers::{csp_nonce_attr, get_csp_nonce};
pub use csrf::{
	CSRF_ALLOWED_CHARS, CSRF_SECRET_LENGTH, CSRF_SESSION_KEY, CSRF_TOKEN_LENGTH, CsrfConfig,
	CsrfMeta, CsrfMiddleware, CsrfMiddlewareConfig, CsrfToken, InvalidTokenFormat,
	REASON_BAD_ORIGIN, REASON_BAD_REFERER, REASON_CSRF_TOKEN_MISSING, REASON_INCORRECT_LENGTH,
	REASON_INSECURE_REFERER, REASON_INVALID_CHARACTERS, REASON_MALFORMED_REFERER,
	REASON_NO_CSRF_COOKIE, REASON_NO_REFERER, RejectRequest, SameSite, check_origin, check_referer,
	check_token, get_secret, get_token, is_same_domain,
};
pub use etag::{ETagConfig, ETagMiddleware};
pub use flatpages::{Flatpage, FlatpageStore, FlatpagesConfig, FlatpagesMiddleware};
#[cfg(feature = "compression")]
pub use gzip::{GZipConfig, GZipMiddleware};
pub use honeypot::{HoneypotError, HoneypotField};
pub use https_redirect::{HttpsRedirectConfig, HttpsRedirectMiddleware};
pub use locale::{LocaleConfig, LocaleMiddleware};
pub use logging::{LoggingConfig, LoggingMiddleware};
pub use messages::{CookieStorage, Message, MessageLevel, MessageStorage, SessionStorage};
pub use metrics::{MetricsConfig, MetricsMiddleware, MetricsStore};
#[cfg(feature = "rate-limit")]
pub use rate_limit::{RateLimitConfig, RateLimitMiddleware, RateLimitStore, RateLimitStrategy};
pub use redirect_fallback::{RedirectFallbackMiddleware, RedirectResponseConfig};
pub use request_id::{REQUEST_ID_HEADER, RequestIdConfig, RequestIdMiddleware};
#[cfg(feature = "security")]
pub use security_middleware::{SecurityConfig, SecurityMiddleware};
pub use session::{SessionConfig, SessionData, SessionMiddleware, SessionStore};
pub use site::{SITE_ID_HEADER, Site, SiteConfig, SiteMiddleware, SiteRegistry};
pub use timeout::{TimeoutConfig, TimeoutMiddleware};
pub use tracing::{
	PARENT_SPAN_ID_HEADER, SPAN_ID_HEADER, Span, SpanStatus, TRACE_ID_HEADER, TraceStore,
	TracingConfig, TracingMiddleware,
};
pub use xframe::{XFrameOptions, XFrameOptionsMiddleware};
pub use xss::{XssConfig, XssError, XssProtector};

#[cfg(all(test, feature = "cors"))]
mod tests {
	use super::*;
	use bytes::Bytes;
	use hyper::{HeaderMap, Method, StatusCode, Version};
	use reinhardt_http::{Handler, Middleware, Request, Response};
	use std::sync::Arc;

	struct TestHandler;

	#[async_trait::async_trait]
	impl Handler for TestHandler {
		async fn handle(&self, _request: Request) -> reinhardt_core::exception::Result<Response> {
			Ok(Response::ok().with_body("test response".as_bytes()))
		}
	}

	#[tokio::test]
	async fn test_cors_middleware_simple_request() {
		use cors::CorsConfig;

		let config = CorsConfig {
			allow_origins: vec!["http://example.com".to_string()],
			allow_methods: vec!["GET".to_string(), "POST".to_string()],
			allow_headers: vec!["Content-Type".to_string()],
			allow_credentials: false,
			max_age: Some(3600),
		};

		let middleware = CorsMiddleware::new(config);
		let handler = Arc::new(TestHandler);

		let mut headers = HeaderMap::new();
		headers.insert("origin", "http://example.com".parse().unwrap());

		let request = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(headers)
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();

		assert_eq!(
			response.headers.get("Access-Control-Allow-Origin").unwrap(),
			"http://example.com"
		);
	}

	#[tokio::test]
	async fn test_cors_middleware_preflight_request() {
		use cors::CorsConfig;

		let config = CorsConfig {
			allow_origins: vec!["http://example.com".to_string()],
			allow_methods: vec!["GET".to_string(), "POST".to_string()],
			allow_headers: vec!["Content-Type".to_string()],
			allow_credentials: false,
			max_age: Some(3600),
		};

		let middleware = CorsMiddleware::new(config);
		let handler = Arc::new(TestHandler);

		let mut headers = HeaderMap::new();
		headers.insert("origin", "http://example.com".parse().unwrap());

		let request = Request::builder()
			.method(Method::OPTIONS)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(headers)
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();

		assert_eq!(response.status, StatusCode::NO_CONTENT);
		assert!(response.headers.contains_key("Access-Control-Allow-Origin"));
		assert!(
			response
				.headers
				.contains_key("Access-Control-Allow-Methods")
		);
		assert!(
			response
				.headers
				.contains_key("Access-Control-Allow-Headers")
		);
	}

	#[tokio::test]
	async fn test_cors_middleware_permissive() {
		let middleware = CorsMiddleware::permissive();
		let handler = Arc::new(TestHandler);
		let request = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();

		assert!(response.headers.contains_key("Access-Control-Allow-Origin"));
	}

	#[tokio::test]
	async fn test_logging_middleware() {
		let middleware = LoggingMiddleware::new();
		let handler = Arc::new(TestHandler);
		let request = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();

		assert_eq!(response.status, StatusCode::OK);
	}
}
