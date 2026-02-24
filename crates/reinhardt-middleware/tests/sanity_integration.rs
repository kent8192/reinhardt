//! Sanity Integration Tests
//!
//! This module performs API sanity checks to verify that middleware APIs
//! behave correctly and safely in basic scenarios.
//!
//! # Test Categories
//!
//! - Default implementations work correctly
//! - State retrieval methods return valid values
//! - Reset/cleanup methods work as expected
//! - Config builder patterns chain correctly
//! - Thread safety for shared middleware

mod fixtures;

use fixtures::{ConfigurableTestHandler, create_test_request};
use reinhardt_http::Middleware;
use std::sync::Arc;

// =============================================================================
// Circuit Breaker API Sanity
// =============================================================================

/// Tests that CircuitBreaker::get_state returns a valid state.
#[test]
fn sanity_circuit_breaker_get_state() {
	use reinhardt_middleware::circuit_breaker::{
		CircuitBreakerConfig, CircuitBreakerMiddleware, CircuitState,
	};
	use std::time::Duration;

	let config = CircuitBreakerConfig {
		error_threshold: 0.5,
		min_requests: 5,
		timeout: Duration::from_secs(60),
		half_open_success_threshold: 3,
		error_message: None,
	};

	let middleware = CircuitBreakerMiddleware::new(config);

	// Initial state should be Closed
	let state = middleware.get_state();
	assert!(
		matches!(state, CircuitState::Closed),
		"Initial state should be Closed"
	);
}

/// Tests that CircuitBreaker::reset returns circuit to Closed state.
#[test]
fn sanity_circuit_breaker_reset() {
	use reinhardt_middleware::circuit_breaker::{
		CircuitBreakerConfig, CircuitBreakerMiddleware, CircuitState,
	};
	use std::time::Duration;

	let config = CircuitBreakerConfig {
		error_threshold: 0.5,
		min_requests: 1,
		timeout: Duration::from_secs(60),
		half_open_success_threshold: 1,
		error_message: None,
	};

	let middleware = CircuitBreakerMiddleware::new(config);

	// Reset should ensure circuit is closed
	middleware.reset();

	let state = middleware.get_state();
	assert!(
		matches!(state, CircuitState::Closed),
		"State after reset should be Closed"
	);
}

// =============================================================================
// Default Implementation Sanity
// =============================================================================

/// Tests that ETagMiddleware::default returns a valid middleware.
#[tokio::test]
async fn sanity_etag_default() {
	use reinhardt_middleware::etag::ETagMiddleware;

	let middleware = Arc::new(ETagMiddleware::default());
	let handler = Arc::new(ConfigurableTestHandler::always_success());

	let request = create_test_request("GET", "/");
	let response = middleware.process(request, handler).await.unwrap();

	assert_eq!(response.status.as_u16(), 200);
}

/// Tests that CspMiddleware::default returns a valid middleware.
#[tokio::test]
async fn sanity_csp_default() {
	use reinhardt_middleware::csp::CspMiddleware;

	let middleware = Arc::new(CspMiddleware::default());
	let handler = Arc::new(ConfigurableTestHandler::always_success());

	let request = create_test_request("GET", "/");
	let response = middleware.process(request, handler).await.unwrap();

	assert_eq!(response.status.as_u16(), 200);
	assert!(response.headers.contains_key("content-security-policy"));
}

/// Tests that CacheConfig::default creates valid config.
#[tokio::test]
async fn sanity_cache_default_config() {
	use reinhardt_middleware::cache::{CacheConfig, CacheMiddleware};

	let config = CacheConfig::default();
	let middleware = Arc::new(CacheMiddleware::new(config));
	let handler = Arc::new(ConfigurableTestHandler::always_success());

	let request = create_test_request("GET", "/");
	let response = middleware.process(request, handler).await.unwrap();

	assert_eq!(response.status.as_u16(), 200);
}

/// Tests that MetricsConfig::default creates valid config.
#[tokio::test]
async fn sanity_metrics_default_config() {
	use reinhardt_middleware::metrics::{MetricsConfig, MetricsMiddleware};

	let config = MetricsConfig::default();
	let middleware = Arc::new(MetricsMiddleware::new(config));
	let handler = Arc::new(ConfigurableTestHandler::always_success());

	let request = create_test_request("GET", "/");
	let response = middleware.process(request, handler).await.unwrap();

	assert_eq!(response.status.as_u16(), 200);
}

/// Tests that RequestIdConfig::default creates valid config.
#[tokio::test]
async fn sanity_request_id_default_config() {
	use reinhardt_middleware::request_id::{RequestIdConfig, RequestIdMiddleware};

	let config = RequestIdConfig::default();
	let middleware = Arc::new(RequestIdMiddleware::new(config));
	let handler = Arc::new(ConfigurableTestHandler::always_success());

	let request = create_test_request("GET", "/");
	let response = middleware.process(request, handler).await.unwrap();

	assert_eq!(response.status.as_u16(), 200);
	assert!(response.headers.contains_key("x-request-id"));
}

/// Tests that TracingConfig::default creates valid config.
#[tokio::test]
async fn sanity_tracing_default_config() {
	use reinhardt_middleware::tracing::{TracingConfig, TracingMiddleware};

	let config = TracingConfig::default();
	let middleware = Arc::new(TracingMiddleware::new(config));
	let handler = Arc::new(ConfigurableTestHandler::always_success());

	let request = create_test_request("GET", "/");
	let response = middleware.process(request, handler).await.unwrap();

	assert_eq!(response.status.as_u16(), 200);
}

// =============================================================================
// Session API Sanity (Feature-gated)
// =============================================================================

#[cfg(feature = "sessions")]
mod session_sanity {
	use super::*;

	/// Tests that SessionConfig::default creates valid config.
	#[tokio::test]
	async fn sanity_session_default_config() {
		use reinhardt_middleware::session::{SessionConfig, SessionMiddleware};

		let config = SessionConfig::default();
		let middleware = Arc::new(SessionMiddleware::new(config));
		let handler = Arc::new(ConfigurableTestHandler::always_success());

		let request = create_test_request("GET", "/");
		let response = middleware.process(request, handler).await.unwrap();

		assert_eq!(response.status.as_u16(), 200);
	}
}

// =============================================================================
// Rate Limit API Sanity (Feature-gated)
// =============================================================================

#[cfg(feature = "rate-limit")]
mod rate_limit_sanity {
	use super::*;

	/// Tests that RateLimitConfig fields are properly initialized.
	#[tokio::test]
	async fn sanity_rate_limit_config_fields() {
		use reinhardt_middleware::rate_limit::{
			RateLimitConfig, RateLimitMiddleware, RateLimitStrategy,
		};

		let config = RateLimitConfig {
			capacity: 100.0,
			refill_rate: 10.0,
			cost_per_request: 1.0,
			strategy: RateLimitStrategy::PerIp,
			exclude_paths: vec!["/health".to_string()],
			error_message: Some("Rate limit exceeded".to_string()),
			trusted_proxies: vec![],
		};

		let middleware = Arc::new(RateLimitMiddleware::new(config));
		let handler = Arc::new(ConfigurableTestHandler::always_success());

		let request =
			fixtures::create_request_with_headers("GET", "/", &[("X-Forwarded-For", "127.0.0.1")]);
		let response = middleware.process(request, handler).await.unwrap();

		assert_eq!(response.status.as_u16(), 200);
	}

	/// Tests that excluded paths bypass rate limiting.
	#[tokio::test]
	async fn sanity_rate_limit_exclude_paths() {
		use reinhardt_middleware::rate_limit::{
			RateLimitConfig, RateLimitMiddleware, RateLimitStrategy,
		};

		let config = RateLimitConfig {
			capacity: 1.0,
			refill_rate: 0.0001,
			cost_per_request: 1.0,
			strategy: RateLimitStrategy::PerIp,
			exclude_paths: vec!["/health".to_string(), "/metrics".to_string()],
			error_message: None,
			trusted_proxies: vec![],
		};

		let middleware = Arc::new(RateLimitMiddleware::new(config));
		let handler = Arc::new(ConfigurableTestHandler::always_success());

		// First request to exhaust tokens
		let request1 = fixtures::create_request_with_headers(
			"GET",
			"/api",
			&[("X-Forwarded-For", "10.0.0.1")],
		);
		let _response1 = middleware.process(request1, handler.clone()).await.unwrap();

		// Excluded path should still work (if implemented)
		let request2 = fixtures::create_request_with_headers(
			"GET",
			"/health",
			&[("X-Forwarded-For", "10.0.0.1")],
		);
		let response2 = middleware.process(request2, handler).await.unwrap();

		// Health endpoint should always be accessible
		assert_eq!(response2.status.as_u16(), 200);
	}
}

// =============================================================================
// CORS API Sanity (Feature-gated)
// =============================================================================

#[cfg(feature = "cors")]
mod cors_sanity {
	use super::*;

	/// Tests that CorsConfig fields are properly initialized.
	#[tokio::test]
	async fn sanity_cors_config_fields() {
		use reinhardt_middleware::cors::{CorsConfig, CorsMiddleware};

		let config = CorsConfig {
			allow_origins: vec!["https://example.com".to_string()],
			allow_methods: vec!["GET".to_string(), "POST".to_string()],
			allow_headers: vec!["Content-Type".to_string(), "Authorization".to_string()],
			allow_credentials: true,
			max_age: Some(3600),
		};

		let middleware = Arc::new(CorsMiddleware::new(config));
		let handler = Arc::new(ConfigurableTestHandler::always_success());

		let request =
			fixtures::create_request_with_headers("GET", "/", &[("Origin", "https://example.com")]);
		let response = middleware.process(request, handler).await.unwrap();

		assert_eq!(response.status.as_u16(), 200);
		assert!(response.headers.contains_key("access-control-allow-origin"));
	}

	/// Tests that empty origins list is handled.
	#[tokio::test]
	async fn sanity_cors_empty_origins() {
		use reinhardt_middleware::cors::{CorsConfig, CorsMiddleware};

		let config = CorsConfig {
			allow_origins: vec![],
			allow_methods: vec!["GET".to_string()],
			allow_headers: vec![],
			allow_credentials: false,
			max_age: None,
		};

		let middleware = Arc::new(CorsMiddleware::new(config));
		let handler = Arc::new(ConfigurableTestHandler::always_success());

		let request =
			fixtures::create_request_with_headers("GET", "/", &[("Origin", "https://unknown.com")]);
		let response = middleware.process(request, handler).await.unwrap();

		// Should handle gracefully even with empty origins
		assert_eq!(response.status.as_u16(), 200);
	}
}

// =============================================================================
// Compression API Sanity (Feature-gated)
// =============================================================================

#[cfg(feature = "compression")]
mod compression_sanity {
	use super::*;

	/// Tests that GZipMiddleware::new creates valid middleware.
	#[tokio::test]
	async fn sanity_gzip_new() {
		use reinhardt_middleware::gzip::GZipMiddleware;

		let middleware = Arc::new(GZipMiddleware::new());
		let handler = Arc::new(ConfigurableTestHandler::with_content_type("text/html"));

		let request =
			fixtures::create_request_with_headers("GET", "/", &[("Accept-Encoding", "gzip")]);
		let response = middleware.process(request, handler).await.unwrap();

		assert_eq!(response.status.as_u16(), 200);
	}

	/// Tests that GZipConfig fields work properly.
	#[tokio::test]
	async fn sanity_gzip_config() {
		use reinhardt_middleware::gzip::{GZipConfig, GZipMiddleware};

		let config = GZipConfig {
			min_length: 100,
			compression_level: 6,
			compressible_types: vec!["text/".to_string(), "application/json".to_string()],
		};

		let middleware = Arc::new(GZipMiddleware::with_config(config));
		let handler = Arc::new(ConfigurableTestHandler::with_content_type("text/html"));

		let request =
			fixtures::create_request_with_headers("GET", "/", &[("Accept-Encoding", "gzip")]);
		let response = middleware.process(request, handler).await.unwrap();

		assert_eq!(response.status.as_u16(), 200);
	}
}

// =============================================================================
// Thread Safety Sanity
// =============================================================================

/// Tests that middleware can be shared across threads.
#[tokio::test]
async fn sanity_middleware_thread_safe() {
	use reinhardt_middleware::etag::ETagMiddleware;
	use std::thread;

	let middleware = Arc::new(ETagMiddleware::default());

	let handles: Vec<_> = (0..4)
		.map(|_| {
			let mw = middleware.clone();
			thread::spawn(move || {
				// Just verify the Arc can be moved to another thread
				let _state = Arc::strong_count(&mw);
			})
		})
		.collect();

	for handle in handles {
		handle.join().unwrap();
	}

	// Middleware should still be usable
	let handler = Arc::new(ConfigurableTestHandler::always_success());
	let request = create_test_request("GET", "/");
	let response = middleware.process(request, handler).await.unwrap();

	assert_eq!(response.status.as_u16(), 200);
}

/// Tests concurrent access to middleware.
#[tokio::test]
async fn sanity_middleware_concurrent_access() {
	use reinhardt_middleware::request_id::{RequestIdConfig, RequestIdMiddleware};

	let middleware = Arc::new(RequestIdMiddleware::new(RequestIdConfig::default()));
	let handler = Arc::new(ConfigurableTestHandler::always_success());

	let mut handles = vec![];

	for _ in 0..10 {
		let mw = middleware.clone();
		let h = handler.clone();
		handles.push(tokio::spawn(async move {
			let request = create_test_request("GET", "/");
			mw.process(request, h).await.unwrap()
		}));
	}

	for handle in handles {
		let response = handle.await.unwrap();
		assert_eq!(response.status.as_u16(), 200);
		assert!(response.headers.contains_key("x-request-id"));
	}
}

// =============================================================================
// XFrame Options Sanity
// =============================================================================

/// Tests XFrameOptionsMiddleware with different options.
#[tokio::test]
async fn sanity_xframe_options_deny() {
	use reinhardt_middleware::xframe::{XFrameOptions, XFrameOptionsMiddleware};

	let middleware = Arc::new(XFrameOptionsMiddleware::new(XFrameOptions::Deny));
	let handler = Arc::new(ConfigurableTestHandler::always_success());

	let request = create_test_request("GET", "/");
	let response = middleware.process(request, handler).await.unwrap();

	assert_eq!(response.status.as_u16(), 200);
	assert!(response.headers.contains_key("x-frame-options"));
}

/// Tests XFrameOptionsMiddleware with SameOrigin.
#[tokio::test]
async fn sanity_xframe_options_sameorigin() {
	use reinhardt_middleware::xframe::{XFrameOptions, XFrameOptionsMiddleware};

	let middleware = Arc::new(XFrameOptionsMiddleware::new(XFrameOptions::SameOrigin));
	let handler = Arc::new(ConfigurableTestHandler::always_success());

	let request = create_test_request("GET", "/");
	let response = middleware.process(request, handler).await.unwrap();

	assert_eq!(response.status.as_u16(), 200);

	let xframe_value = response.headers.get("x-frame-options").unwrap();
	assert_eq!(xframe_value.to_str().unwrap(), "SAMEORIGIN");
}

// =============================================================================
// Timeout Config Sanity
// =============================================================================

/// Tests TimeoutConfig with various durations.
#[tokio::test]
async fn sanity_timeout_config_durations() {
	use reinhardt_middleware::timeout::{TimeoutConfig, TimeoutMiddleware};
	use std::time::Duration;

	// Test with different timeout values
	for ms in [100, 500, 1000, 5000] {
		let config = TimeoutConfig {
			duration: Duration::from_millis(ms),
		};

		let middleware = Arc::new(TimeoutMiddleware::new(config));
		let handler = Arc::new(ConfigurableTestHandler::always_success());

		let request = create_test_request("GET", "/");
		let response = middleware.process(request, handler).await.unwrap();

		assert_eq!(response.status.as_u16(), 200);
	}
}

// =============================================================================
// Locale Middleware Sanity
// =============================================================================

/// Tests LocaleMiddleware::new creates valid middleware.
#[tokio::test]
async fn sanity_locale_new() {
	use reinhardt_middleware::locale::LocaleMiddleware;

	let middleware = Arc::new(LocaleMiddleware::new());
	let handler = Arc::new(ConfigurableTestHandler::always_success());

	let request = create_test_request("GET", "/");
	let response = middleware.process(request, handler).await.unwrap();

	assert_eq!(response.status.as_u16(), 200);
}

// =============================================================================
// CSRF Middleware Sanity
// =============================================================================

/// Tests CsrfMiddleware::new creates valid middleware.
#[tokio::test]
async fn sanity_csrf_new() {
	use reinhardt_middleware::csrf::CsrfMiddleware;

	let middleware = Arc::new(CsrfMiddleware::new());
	let handler = Arc::new(ConfigurableTestHandler::always_success());

	// GET requests should pass without token
	let request = create_test_request("GET", "/");
	let response = middleware.process(request, handler).await.unwrap();

	assert_eq!(response.status.as_u16(), 200);
}

/// Tests CSRF allows safe methods.
#[tokio::test]
async fn sanity_csrf_safe_methods() {
	use reinhardt_middleware::csrf::CsrfMiddleware;

	let middleware = Arc::new(CsrfMiddleware::new());
	let handler = Arc::new(ConfigurableTestHandler::always_success());

	for method in ["GET", "HEAD", "OPTIONS"] {
		let request = create_test_request(method, "/");
		let response = middleware.process(request, handler.clone()).await.unwrap();

		assert_eq!(
			response.status.as_u16(),
			200,
			"Method {} should be allowed without CSRF token",
			method
		);
	}
}

// =============================================================================
// Logging Middleware Sanity
// =============================================================================

/// Tests LoggingMiddleware with default config.
#[tokio::test]
async fn sanity_logging_middleware() {
	use reinhardt_middleware::logging::LoggingMiddleware;

	let middleware = Arc::new(LoggingMiddleware::new());
	let handler = Arc::new(ConfigurableTestHandler::always_success());

	let request = create_test_request("GET", "/");
	let response = middleware.process(request, handler).await.unwrap();

	assert_eq!(response.status.as_u16(), 200);
}
