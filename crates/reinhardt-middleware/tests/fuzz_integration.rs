//! Fuzz Integration Tests
//!
//! This module uses proptest to fuzz test middleware with random inputs
//! to find edge cases, security vulnerabilities, and crashes.
//!
//! # Fuzz Targets
//!
//! - CSRF token validation with random byte sequences
//! - CORS Origin validation with malformed URLs
//! - Accept-Language parsing with malformed locale strings
//! - Cookie parsing with malformed values
//! - X-Forwarded-For IP extraction with injection attempts
//! - Session ID validation with collision attempts

mod fixtures;

use fixtures::{ConfigurableTestHandler, create_request_with_headers, create_test_request};
use proptest::prelude::*;
use reinhardt_http::Middleware;
use std::sync::Arc;

// =============================================================================
// CSRF Token Fuzzing
// =============================================================================

proptest! {
	#![proptest_config(ProptestConfig::with_cases(50))]

	/// Fuzz CSRF middleware with random token-like strings.
	/// The middleware should never panic on malformed tokens.
	#[test]
	fn fuzz_csrf_random_tokens(token in "[a-zA-Z0-9!@#$%^&*()_+=\\[\\]{};':\",./<>?]{0,200}") {
		let rt = tokio::runtime::Runtime::new().unwrap();
		rt.block_on(async {
			use reinhardt_middleware::csrf::CsrfMiddleware;

			let middleware = Arc::new(CsrfMiddleware::new());
			let handler = Arc::new(ConfigurableTestHandler::always_success());

			// Create POST request with random CSRF header
			let request = create_request_with_headers(
				"POST",
				"/submit",
				&[("X-CSRF-Token", &token)]
			);

			// Should not panic, may return error for invalid tokens
			let _ = middleware.process(request, handler).await;
		});
	}

	/// Fuzz CSRF with random cookie values.
	#[test]
	fn fuzz_csrf_random_cookies(cookie in "[a-zA-Z0-9_=-]{0,100}") {
		let rt = tokio::runtime::Runtime::new().unwrap();
		rt.block_on(async {
			use reinhardt_middleware::csrf::CsrfMiddleware;

			let middleware = Arc::new(CsrfMiddleware::new());
			let handler = Arc::new(ConfigurableTestHandler::always_success());

			let cookie_value = format!("csrftoken={}", cookie);
			let request = create_request_with_headers(
				"POST",
				"/submit",
				&[
					("Cookie", &cookie_value),
					("X-CSRF-Token", &cookie),
				]
			);

			// Should not panic
			let _ = middleware.process(request, handler).await;
		});
	}
}

// =============================================================================
// CORS Origin Fuzzing
// =============================================================================

#[cfg(feature = "cors")]
proptest! {
	#![proptest_config(ProptestConfig::with_cases(50))]

	/// Fuzz CORS middleware with malformed origin headers.
	#[test]
	fn fuzz_cors_malformed_origins(origin in "[a-zA-Z0-9:/._-]{0,200}") {
		let rt = tokio::runtime::Runtime::new().unwrap();
		rt.block_on(async {
			use reinhardt_middleware::cors::{CorsConfig, CorsMiddleware};

			let mut config = CorsConfig::default();
			config.allow_origins = vec!["https://example.com".to_string()];
			config.allow_methods = vec!["GET".to_string(), "POST".to_string()];
			config.allow_headers = vec!["Content-Type".to_string()];
			config.allow_credentials = false;
			config.max_age = Some(3600);

			let middleware = Arc::new(CorsMiddleware::new(config));
			let handler = Arc::new(ConfigurableTestHandler::always_success());

			let request = create_request_with_headers("GET", "/", &[("Origin", &origin)]);

			// Should not panic on malformed origins
			let result = middleware.process(request, handler).await;
			assert!(result.is_ok(), "CORS should handle malformed origins gracefully");
		});
	}

	/// Fuzz CORS with injection attempts in origin.
	#[test]
	fn fuzz_cors_injection_attempts(
		protocol in "(https?|ftp|file|javascript|data)://",
		host in "[a-zA-Z0-9.-]{1,50}",
		suffix in "[<>\"'\\\\;]?"
	) {
		let rt = tokio::runtime::Runtime::new().unwrap();
		rt.block_on(async {
			use reinhardt_middleware::cors::{CorsConfig, CorsMiddleware};

			let origin = format!("{}{}{}", protocol, host, suffix);

			let mut config = CorsConfig::default();
			config.allow_origins = vec!["https://trusted.com".to_string()];
			config.allow_methods = vec!["GET".to_string()];
			config.allow_headers = vec![];
			config.allow_credentials = false;
			config.max_age = None;

			let middleware = Arc::new(CorsMiddleware::new(config));
			let handler = Arc::new(ConfigurableTestHandler::always_success());

			let request = create_request_with_headers("GET", "/", &[("Origin", &origin)]);

			// Should handle injection attempts safely
			let result = middleware.process(request, handler).await;
			assert!(result.is_ok());
		});
	}
}

// =============================================================================
// Accept-Language Fuzzing
// =============================================================================

proptest! {
	#![proptest_config(ProptestConfig::with_cases(50))]

	/// Fuzz locale middleware with malformed Accept-Language headers.
	#[test]
	fn fuzz_locale_malformed_language(lang in "[a-zA-Z0-9_,-;=.]{0,200}") {
		let rt = tokio::runtime::Runtime::new().unwrap();
		rt.block_on(async {
			use reinhardt_middleware::locale::LocaleMiddleware;

			let middleware = Arc::new(LocaleMiddleware::new());
			let handler = Arc::new(ConfigurableTestHandler::always_success());

			let request = create_request_with_headers("GET", "/", &[("Accept-Language", &lang)]);

			// Should not panic on malformed language headers
			let result = middleware.process(request, handler).await;
			assert!(result.is_ok(), "Locale should handle malformed Accept-Language");
		});
	}

	/// Fuzz locale with extremely long language values.
	#[test]
	fn fuzz_locale_long_values(count in 1usize..100) {
		let rt = tokio::runtime::Runtime::new().unwrap();
		rt.block_on(async {
			use reinhardt_middleware::locale::LocaleMiddleware;

			let middleware = Arc::new(LocaleMiddleware::new());
			let handler = Arc::new(ConfigurableTestHandler::always_success());

			// Create very long Accept-Language header
			let long_lang = "en-US;q=0.9,".repeat(count);
			let request = create_request_with_headers("GET", "/", &[("Accept-Language", &long_lang)]);

			// Should handle long values gracefully
			let result = middleware.process(request, handler).await;
			assert!(result.is_ok());
		});
	}
}

// =============================================================================
// X-Forwarded-For IP Fuzzing
// =============================================================================

#[cfg(feature = "rate-limit")]
proptest! {
	#![proptest_config(ProptestConfig::with_cases(50))]

	/// Fuzz rate limit with malformed IP addresses.
	#[test]
	fn fuzz_ratelimit_malformed_ips(ip in "[0-9.a-fA-F:]{0,100}") {
		let rt = tokio::runtime::Runtime::new().unwrap();
		rt.block_on(async {
			use reinhardt_middleware::rate_limit::{RateLimitConfig, RateLimitMiddleware, RateLimitStrategy};

			let config = RateLimitConfig::new(RateLimitStrategy::PerIp, 100.0, 10.0);

			let middleware = Arc::new(RateLimitMiddleware::new(config));
			let handler = Arc::new(ConfigurableTestHandler::always_success());

			let request = create_request_with_headers("GET", "/", &[("X-Forwarded-For", &ip)]);

			// Should not panic on malformed IPs
			let result = middleware.process(request, handler).await;
			assert!(result.is_ok());
		});
	}

	/// Fuzz rate limit with IP injection attempts.
	#[test]
	fn fuzz_ratelimit_ip_injection(
		real_ip in "[0-9]{1,3}\\.[0-9]{1,3}\\.[0-9]{1,3}\\.[0-9]{1,3}",
		injected in "[a-zA-Z0-9,; ]{0,50}"
	) {
		let rt = tokio::runtime::Runtime::new().unwrap();
		rt.block_on(async {
			use reinhardt_middleware::rate_limit::{RateLimitConfig, RateLimitMiddleware, RateLimitStrategy};

			let config = RateLimitConfig::new(RateLimitStrategy::PerIp, 100.0, 10.0);

			let middleware = Arc::new(RateLimitMiddleware::new(config));
			let handler = Arc::new(ConfigurableTestHandler::always_success());

			// Try to inject additional IPs
			let spoofed_ip = format!("{}, {}", injected, real_ip);
			let request = create_request_with_headers("GET", "/", &[("X-Forwarded-For", &spoofed_ip)]);

			let result = middleware.process(request, handler).await;
			assert!(result.is_ok());
		});
	}
}

// =============================================================================
// Cookie Parsing Fuzzing
// =============================================================================

proptest! {
	#![proptest_config(ProptestConfig::with_cases(50))]

	/// Fuzz middleware with malformed cookie headers.
	#[test]
	fn fuzz_cookie_malformed_values(cookie in "[a-zA-Z0-9=;, _-]{0,200}") {
		let rt = tokio::runtime::Runtime::new().unwrap();
		rt.block_on(async {
			use reinhardt_middleware::etag::ETagMiddleware;

			let middleware = Arc::new(ETagMiddleware::default());
			let handler = Arc::new(ConfigurableTestHandler::always_success());

			let request = create_request_with_headers("GET", "/", &[("Cookie", &cookie)]);

			// Should handle malformed cookies gracefully
			let result = middleware.process(request, handler).await;
			assert!(result.is_ok());
		});
	}

	/// Fuzz with cookie injection attempts.
	#[test]
	fn fuzz_cookie_injection(
		name in "[a-zA-Z_]{1,20}",
		value in "[a-zA-Z0-9+/=]{0,100}",
		suffix in "[<>\"']{0,1}"
	) {
		let rt = tokio::runtime::Runtime::new().unwrap();
		rt.block_on(async {
			use reinhardt_middleware::cache::{CacheConfig, CacheMiddleware};

			let cookie = format!("{}={}{}", name, value, suffix);

			let config = CacheConfig::default();
			let middleware = Arc::new(CacheMiddleware::new(config));
			let handler = Arc::new(ConfigurableTestHandler::always_success());

			let request = create_request_with_headers("GET", "/", &[("Cookie", &cookie)]);

			let result = middleware.process(request, handler).await;
			assert!(result.is_ok());
		});
	}
}

// =============================================================================
// Session ID Fuzzing
// =============================================================================

#[cfg(feature = "sessions")]
proptest! {
	#![proptest_config(ProptestConfig::with_cases(50))]

	/// Fuzz session middleware with random session IDs.
	#[test]
	fn fuzz_session_random_ids(session_id in "[a-zA-Z0-9_=-]{0,100}") {
		let rt = tokio::runtime::Runtime::new().unwrap();
		rt.block_on(async {
			use reinhardt_middleware::session::{SessionConfig, SessionMiddleware};

			let config = SessionConfig::default();
			let middleware = Arc::new(SessionMiddleware::new(config));
			let handler = Arc::new(ConfigurableTestHandler::always_success());

			let cookie = format!("session_id={}", session_id);
			let request = create_request_with_headers("GET", "/", &[("Cookie", &cookie)]);

			// Should handle random session IDs gracefully
			let result = middleware.process(request, handler).await;
			assert!(result.is_ok());
		});
	}

	/// Fuzz session with collision-prone IDs.
	#[test]
	fn fuzz_session_collision_ids(base in "[0-9a-f]{32}", variant in 0u8..255) {
		let rt = tokio::runtime::Runtime::new().unwrap();
		rt.block_on(async {
			use reinhardt_middleware::session::{SessionConfig, SessionMiddleware};

			let config = SessionConfig::default();
			let middleware = Arc::new(SessionMiddleware::new(config));
			let handler = Arc::new(ConfigurableTestHandler::always_success());

			// Create similar session IDs
			let session_id = format!("{}{:02x}", base, variant);
			let cookie = format!("session_id={}", session_id);
			let request = create_request_with_headers("GET", "/", &[("Cookie", &cookie)]);

			let result = middleware.process(request, handler).await;
			assert!(result.is_ok());
		});
	}
}

// =============================================================================
// Header Value Fuzzing
// =============================================================================

proptest! {
	#![proptest_config(ProptestConfig::with_cases(30))]

	/// Fuzz with Unicode in header values.
	/// Note: The generated Unicode value is currently not used in the test request.
	/// Future improvement: use create_request_with_headers to test Unicode header handling.
	#[test]
	fn fuzz_unicode_headers(_value in "\\PC{1,50}") {
		let rt = tokio::runtime::Runtime::new().unwrap();
		rt.block_on(async {
			use reinhardt_middleware::etag::ETagMiddleware;

			let middleware = Arc::new(ETagMiddleware::default());
			let handler = Arc::new(ConfigurableTestHandler::always_success());

			// Try various headers with Unicode
			let request = create_test_request("GET", "/");

			// ETag middleware should handle request regardless of content
			let result = middleware.process(request, handler).await;
			assert!(result.is_ok());
		});
	}

	/// Fuzz request paths with special characters.
	#[test]
	fn fuzz_special_paths(path in "/[a-zA-Z0-9%._~:/?#\\[\\]@!$&'()*+,;=-]{0,100}") {
		let rt = tokio::runtime::Runtime::new().unwrap();
		rt.block_on(async {
			use reinhardt_middleware::cache::{CacheConfig, CacheMiddleware};

			let config = CacheConfig::default();
			let middleware = Arc::new(CacheMiddleware::new(config));
			let handler = Arc::new(ConfigurableTestHandler::always_success());

			// Create request with special path
			let safe_path = if path.is_empty() { "/" } else { &path };
			let request = create_test_request("GET", safe_path);

			// Should handle special paths gracefully
			let result = middleware.process(request, handler).await;
			assert!(result.is_ok());
		});
	}
}

// =============================================================================
// Timeout Fuzzing
// =============================================================================

proptest! {
	#![proptest_config(ProptestConfig::with_cases(20))]

	/// Fuzz timeout with various duration values.
	#[test]
	fn fuzz_timeout_durations(ms in 1u64..10000) {
		let rt = tokio::runtime::Runtime::new().unwrap();
		rt.block_on(async {
			use reinhardt_middleware::timeout::{TimeoutConfig, TimeoutMiddleware};
			use std::time::Duration;

			let config = TimeoutConfig::new(Duration::from_millis(ms));

			let middleware = Arc::new(TimeoutMiddleware::new(config));
			let handler = Arc::new(ConfigurableTestHandler::always_success());

			let request = create_test_request("GET", "/");

			// Fast handler should complete within any reasonable timeout
			let result = middleware.process(request, handler).await;
			assert!(result.is_ok());
		});
	}
}
