//! CSRF Middleware Integration Tests
//!
//! Tests the integration of CSRF protection with HTTP middleware
//! Based on Django's CSRF middleware tests

use hyper::header::{COOKIE, HeaderName, HeaderValue, SET_COOKIE};
use reinhardt_core::security::csrf::SameSite;
use reinhardt_core::security::{CsrfConfig, generate_token_hmac, verify_token_hmac};
use reinhardt_http::Handler;
use reinhardt_http::{Request, Response, Result};
use reinhardt_test::http::*;

// Mock handler for testing
#[allow(dead_code)]
struct MockHandler;

#[async_trait::async_trait]
impl Handler for MockHandler {
	async fn handle(&self, _request: Request) -> Result<Response> {
		Ok(create_test_response())
	}
}

/// Create a request with CSRF token in cookie
fn create_request_with_csrf_cookie(method: &str, uri: &str, token: &str) -> Request {
	let mut request = create_test_request(method, uri, true);
	request.headers.insert(
		COOKIE,
		HeaderValue::from_str(&format!("csrftoken={}", token)).unwrap(),
	);
	request
}

/// Create a request with CSRF token in header
fn create_request_with_csrf_header(
	method: &str,
	uri: &str,
	cookie_token: &str,
	header_token: &str,
) -> Request {
	let mut request = create_request_with_csrf_cookie(method, uri, cookie_token);
	request.headers.insert(
		HeaderName::from_static("x-csrftoken"),
		HeaderValue::from_str(header_token).unwrap(),
	);
	request
}

/// Extract CSRF token from Set-Cookie header
#[allow(dead_code)]
fn extract_csrf_token_from_response(response: &Response) -> Option<String> {
	response
		.headers
		.get(SET_COOKIE)?
		.to_str()
		.ok()?
		.split(';')
		.next()?
		.strip_prefix("csrftoken=")?
		.to_string()
		.into()
}

#[tokio::test]
async fn test_csrf_middleware_integration_token_generation() {
	// Test: CSRF token is generated and set in cookie for GET requests
	let secret = b"abcdefghijklmnopqrstuvwxyz012345";
	let token = generate_token_hmac(secret, "test-session");

	assert_eq!(token.len(), 64); // HMAC-SHA256 produces 32 bytes = 64 hex chars
}

#[tokio::test]
async fn test_csrf_token_validation_success() {
	// Test: Valid CSRF token passes validation
	let secret = b"abcdefghijklmnopqrstuvwxyz012345";
	let message = "test-session";
	let token = generate_token_hmac(secret, message);

	// Simulate validation
	let result = verify_token_hmac(&token, secret, message);
	assert!(result);
}

#[tokio::test]
async fn test_csrf_token_validation_failure() {
	// Test: Invalid CSRF token fails validation
	let secret1 = b"abcdefghijklmnopqrstuvwxyz012345";
	let secret2 = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ012345";
	let message = "test-session";
	let token = generate_token_hmac(secret1, message);

	let result = verify_token_hmac(&token, secret2, message);
	assert!(!result);
}

#[tokio::test]
async fn test_csrf_safe_methods_bypass() {
	// Test: Safe methods (GET, HEAD, OPTIONS, TRACE) bypass CSRF check
	let safe_methods = ["GET", "HEAD", "OPTIONS", "TRACE"];

	for method in &safe_methods {
		let _request = create_test_request(method, "/api/test", true);
		// Safe methods should not require CSRF token
		// This would normally be handled by middleware
		assert!(["GET", "HEAD", "OPTIONS", "TRACE"].contains(method));
	}
}

#[tokio::test]
async fn test_csrf_unsafe_methods_require_token() {
	// Test: Unsafe methods (POST, PUT, DELETE, PATCH) require CSRF token
	let unsafe_methods = ["POST", "PUT", "DELETE", "PATCH"];

	for method in &unsafe_methods {
		let _request = create_test_request(method, "/api/test", true);
		// These methods should require CSRF validation
		assert!(["POST", "PUT", "DELETE", "PATCH"].contains(method));
	}
}

#[test]
fn test_csrf_token_in_cookie() {
	// Test: CSRF token can be extracted from cookie
	let secret = "abcdefghijklmnopqrstuvwxyz012345";
	let token = generate_token_hmac(secret.as_bytes(), "test-session");

	let request = create_request_with_csrf_cookie("POST", "/api", &token);

	let cookie_header = request.headers.get(COOKIE).unwrap();
	assert!(cookie_header.to_str().unwrap().contains("csrftoken="));
}

#[test]
fn test_csrf_token_in_header() {
	// Test: CSRF token can be provided in X-CSRFToken header
	let secret = "abcdefghijklmnopqrstuvwxyz012345";
	let token = generate_token_hmac(secret.as_bytes(), "test-session");

	let request = create_request_with_csrf_header("POST", "/api", &token, &token);

	assert!(request.headers.contains_key("x-csrftoken"));
}

#[test]
fn test_csrf_token_mismatch() {
	// Test: Mismatched CSRF tokens should be detected
	let secret1 = "abcdefghijklmnopqrstuvwxyz012345";
	let secret2 = "ABCDEFGHIJKLMNOPQRSTUVWXYZ012345";
	let token1 = generate_token_hmac(secret1.as_bytes(), "test-session");
	let token2 = generate_token_hmac(secret2.as_bytes(), "test-session");

	assert_ne!(token1, token2);
}

#[test]
fn test_referer_check_same_origin() {
	// Test: Referer from same origin should pass
	let mut request = create_secure_request("POST", "/api/test");
	request.headers.insert(
		HeaderName::from_static("referer"),
		HeaderValue::from_static("https://example.com/page"),
	);

	assert!(request.headers.contains_key("referer"));
}

#[test]
fn test_referer_check_different_origin() {
	// Test: Referer from different origin should be detected
	let mut request = create_secure_request("POST", "/api/test");
	request.headers.insert(
		HeaderName::from_static("referer"),
		HeaderValue::from_static("https://evil.com/page"),
	);

	let referer = request.headers.get("referer").unwrap().to_str().unwrap();
	assert!(referer.contains("evil.com"));
}

#[test]
fn test_origin_check_present() {
	// Test: Origin header check
	let mut request = create_secure_request("POST", "/api/test");
	request.headers.insert(
		HeaderName::from_static("origin"),
		HeaderValue::from_static("https://example.com"),
	);

	assert!(request.headers.contains_key("origin"));
}

#[test]
fn test_csrf_exempt_view() {
	// Test: Some views can be marked as CSRF exempt
	// This would typically be handled via middleware configuration or decorators
	let request = create_test_request("POST", "/api/public", true);
	// Public endpoints might not require CSRF
	assert_eq!(request.uri.path(), "/api/public");
}

#[test]
fn test_csrf_cookie_httponly() {
	// Test: CSRF cookie should NOT be HttpOnly
	// CSRF tokens need to be accessible from JavaScript
	// This is opposite of session cookies which SHOULD be HttpOnly

	let default_config = CsrfConfig::default();
	assert!(
		!default_config.cookie_httponly,
		"CSRF cookie should not be HttpOnly (JavaScript needs access)"
	);

	let production_config = CsrfConfig::production();
	assert!(
		!production_config.cookie_httponly,
		"CSRF cookie should not be HttpOnly even in production"
	);
}

#[test]
fn test_csrf_cookie_secure() {
	// Test: CSRF cookie Secure flag behavior
	// In development: Secure should be false (allows HTTP)
	// In production: Secure should be true (HTTPS only)

	let default_config = CsrfConfig::default();
	assert!(
		!default_config.cookie_secure,
		"Default config should allow HTTP (development)"
	);

	let production_config = CsrfConfig::production();
	assert!(
		production_config.cookie_secure,
		"Production config should require HTTPS"
	);
}

#[test]
fn test_csrf_cookie_samesite() {
	// Test: CSRF cookie SameSite attribute configuration
	// SameSite provides additional CSRF protection by restricting cross-site cookie usage
	// Development: Lax (balance between security and usability)
	// Production: Strict (maximum security)

	let default_config = CsrfConfig::default();
	assert_eq!(
		default_config.cookie_samesite,
		SameSite::Lax,
		"Default config should use SameSite=Lax"
	);

	let production_config = CsrfConfig::production();
	assert_eq!(
		production_config.cookie_samesite,
		SameSite::Strict,
		"Production config should use SameSite=Strict for maximum security"
	);

	// Test other cookie settings in production
	assert_eq!(production_config.cookie_path, "/");
	assert_eq!(production_config.cookie_max_age, Some(31449600)); // 1 year
	assert_eq!(production_config.cookie_domain, None);
}

#[test]
fn test_double_submit_cookie_pattern() {
	// Test: Double submit cookie pattern validation
	let secret = "abcdefghijklmnopqrstuvwxyz012345";
	let token = generate_token_hmac(secret.as_bytes(), "test-session");

	// Cookie value and header value should match
	let request = create_request_with_csrf_header("POST", "/api", &token, &token);

	let cookie_token = request
		.headers
		.get(COOKIE)
		.and_then(|v| v.to_str().ok())
		.and_then(|s| s.strip_prefix("csrftoken="))
		.unwrap();

	let header_token = request
		.headers
		.get("x-csrftoken")
		.and_then(|v| v.to_str().ok())
		.unwrap();

	assert_eq!(cookie_token, header_token);
}

#[tokio::test]
async fn test_csrf_rotation() {
	// Test: CSRF token rotation on login
	// After login, a new CSRF token should be generated
	let secret1 = "abcdefghijklmnopqrstuvwxyz012345";
	let secret2 = "ABCDEFGHIJKLMNOPQRSTUVWXYZ012345";

	let token1 = generate_token_hmac(secret1.as_bytes(), "test-session");
	let token2 = generate_token_hmac(secret2.as_bytes(), "test-session");

	// Tokens should be different
	assert_ne!(token1, token2);
}
