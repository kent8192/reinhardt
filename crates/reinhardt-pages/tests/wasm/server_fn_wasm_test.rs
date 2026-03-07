//! Server Function WASM Integration Tests
//!
//! These tests verify server function client-side behavior in a browser environment,
//! including CSRF token injection into HTTP requests.
//!
//! **Run with**: `wasm-pack test --headless --chrome`

#![cfg(target_arch = "wasm32")]

use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

use reinhardt_pages::csrf::{CSRF_HEADER_NAME, csrf_headers};
use reinhardt_pages::testing::{cleanup_csrf_fixtures, setup_csrf_cookie, setup_csrf_meta_tag};

// ============================================================================
// csrf_headers() Integration Tests
// ============================================================================

/// Test csrf_headers() returns correct header tuple from cookie
#[wasm_bindgen_test]
fn test_csrf_headers_from_cookie() {
	cleanup_csrf_fixtures();

	setup_csrf_cookie("header_test_token");

	let headers = csrf_headers();
	assert!(headers.is_some());

	let (name, value) = headers.unwrap();
	assert_eq!(name, CSRF_HEADER_NAME);
	assert_eq!(name, "X-CSRFToken");
	assert_eq!(value, "header_test_token");

	cleanup_csrf_fixtures();
}

/// Test csrf_headers() returns correct header tuple from meta tag
#[wasm_bindgen_test]
fn test_csrf_headers_from_meta() {
	cleanup_csrf_fixtures();

	setup_csrf_meta_tag("meta_header_token");

	let headers = csrf_headers();
	assert!(headers.is_some());

	let (name, value) = headers.unwrap();
	assert_eq!(name, CSRF_HEADER_NAME);
	assert_eq!(value, "meta_header_token");

	cleanup_csrf_fixtures();
}

/// Test csrf_headers() returns None when no token available
#[wasm_bindgen_test]
fn test_csrf_headers_none_when_no_token() {
	cleanup_csrf_fixtures();

	let headers = csrf_headers();
	assert!(headers.is_none());

	cleanup_csrf_fixtures();
}

// ============================================================================
// HTTP Request Header Injection Tests
// ============================================================================

/// Test that CSRF header name matches Django convention
#[wasm_bindgen_test]
fn test_csrf_header_name_django_compatible() {
	// Django expects X-CSRFToken header for AJAX requests
	assert_eq!(CSRF_HEADER_NAME, "X-CSRFToken");
}

/// Test headers can be used with gloo_net Request builder pattern
#[wasm_bindgen_test]
fn test_csrf_headers_usable_with_request() {
	cleanup_csrf_fixtures();

	setup_csrf_cookie("request_test_token");

	// Verify headers can be destructured and used
	if let Some((header_name, header_value)) = csrf_headers() {
		assert_eq!(header_name, "X-CSRFToken");
		assert!(!header_value.is_empty());

		// Verify header value type is compatible with gloo_net
		let _: &str = header_name; // Static str
		let _: &str = &header_value; // String reference
	} else {
		panic!("Expected csrf_headers to return Some");
	}

	cleanup_csrf_fixtures();
}

// ============================================================================
// Token Priority in Headers Tests
// ============================================================================

/// Test headers use cookie token when available (highest priority)
#[wasm_bindgen_test]
fn test_csrf_headers_prefer_cookie() {
	cleanup_csrf_fixtures();

	setup_csrf_cookie("cookie_priority_header");
	setup_csrf_meta_tag("meta_priority_header");

	let headers = csrf_headers();
	let (_, value) = headers.unwrap();
	assert_eq!(value, "cookie_priority_header");

	cleanup_csrf_fixtures();
}

/// Test headers fall back to meta when no cookie
#[wasm_bindgen_test]
fn test_csrf_headers_fallback_to_meta() {
	cleanup_csrf_fixtures();

	setup_csrf_meta_tag("meta_fallback_header");

	let headers = csrf_headers();
	let (_, value) = headers.unwrap();
	assert_eq!(value, "meta_fallback_header");

	cleanup_csrf_fixtures();
}

// ============================================================================
// Server Function Client Stub Verification
// ============================================================================

/// Test that automatic CSRF injection produces valid header format
///
/// This test verifies the contract that the server_fn macro relies on:
/// csrf_headers() returns `Option<(&'static str, String)>` where the first
/// element is the header name and second is the value.
#[wasm_bindgen_test]
fn test_csrf_headers_contract() {
	cleanup_csrf_fixtures();

	setup_csrf_cookie("contract_test");

	let result = csrf_headers();

	// Contract: returns Option<(&'static str, String)>
	assert!(result.is_some(), "Should return Some when token exists");

	let (name, value) = result.unwrap();

	// Contract: name is a static str (header name constant)
	assert!(!name.is_empty(), "Header name should not be empty");

	// Contract: value is the token string
	assert_eq!(value, "contract_test", "Value should match set token");

	// Contract: name matches Django's expected header
	assert_eq!(
		name, "X-CSRFToken",
		"Header name should be X-CSRFToken for Django compatibility"
	);

	cleanup_csrf_fixtures();
}

/// Test that no CSRF header is added when token unavailable
#[wasm_bindgen_test]
fn test_no_csrf_header_when_unavailable() {
	cleanup_csrf_fixtures();

	let result = csrf_headers();

	// When no token, csrf_headers returns None
	// Server functions with no_csrf=false will not add any CSRF header
	assert!(result.is_none(), "Should return None when no token exists");

	cleanup_csrf_fixtures();
}

// ============================================================================
// Cleanup Verification Tests
// ============================================================================

/// Test that cleanup properly removes all CSRF fixtures
#[wasm_bindgen_test]
fn test_cleanup_removes_all_fixtures() {
	// Set up all fixture types
	setup_csrf_cookie("cleanup_test_cookie");
	setup_csrf_meta_tag("cleanup_test_meta");

	// Verify they exist
	assert!(csrf_headers().is_some());

	// Clean up
	cleanup_csrf_fixtures();

	// Verify all removed
	assert!(csrf_headers().is_none());
}
