//! CSRF Token WASM Integration Tests
//!
//! These tests verify CSRF token retrieval functionality in an actual browser
//! environment. They test the integration between DOM elements (cookies, meta tags,
//! hidden inputs) and the CSRF token retrieval functions.
//!
//! **Run with**: `wasm-pack test --headless --chrome`

#![cfg(target_arch = "wasm32")]

use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

use reinhardt_pages::csrf::{
	CsrfManager, get_csrf_token, get_csrf_token_from_cookie, get_csrf_token_from_input,
	get_csrf_token_from_meta,
};
use reinhardt_pages::testing::{
	cleanup_csrf_fixtures, setup_csrf_cookie, setup_csrf_input, setup_csrf_meta_tag,
};

// ============================================================================
// CSRF Token from Cookie Tests
// ============================================================================

/// Test retrieving CSRF token from cookie
#[wasm_bindgen_test]
fn test_csrf_token_from_cookie() {
	cleanup_csrf_fixtures();

	setup_csrf_cookie("cookie_token_abc123");

	let token = get_csrf_token_from_cookie();
	assert_eq!(token, Some("cookie_token_abc123".to_string()));

	cleanup_csrf_fixtures();
}

/// Test empty cookie returns None
#[wasm_bindgen_test]
fn test_csrf_token_from_cookie_empty() {
	cleanup_csrf_fixtures();

	let token = get_csrf_token_from_cookie();
	assert!(token.is_none());

	cleanup_csrf_fixtures();
}

// ============================================================================
// CSRF Token from Meta Tag Tests
// ============================================================================

/// Test retrieving CSRF token from meta tag
#[wasm_bindgen_test]
fn test_csrf_token_from_meta_tag() {
	cleanup_csrf_fixtures();

	setup_csrf_meta_tag("meta_token_xyz789");

	let token = get_csrf_token_from_meta();
	assert_eq!(token, Some("meta_token_xyz789".to_string()));

	cleanup_csrf_fixtures();
}

/// Test missing meta tag returns None
#[wasm_bindgen_test]
fn test_csrf_token_from_meta_tag_missing() {
	cleanup_csrf_fixtures();

	let token = get_csrf_token_from_meta();
	assert!(token.is_none());

	cleanup_csrf_fixtures();
}

// ============================================================================
// CSRF Token from Hidden Input Tests
// ============================================================================

/// Test retrieving CSRF token from hidden form input
#[wasm_bindgen_test]
fn test_csrf_token_from_hidden_input() {
	cleanup_csrf_fixtures();

	setup_csrf_input("input_token_def456");

	let token = get_csrf_token_from_input();
	assert_eq!(token, Some("input_token_def456".to_string()));

	cleanup_csrf_fixtures();
}

/// Test missing input returns None
#[wasm_bindgen_test]
fn test_csrf_token_from_hidden_input_missing() {
	cleanup_csrf_fixtures();

	let token = get_csrf_token_from_input();
	assert!(token.is_none());

	cleanup_csrf_fixtures();
}

// ============================================================================
// Token Source Priority Tests
// ============================================================================

/// Test that get_csrf_token() retrieves from cookie first (highest priority)
#[wasm_bindgen_test]
fn test_csrf_token_priority_cookie_first() {
	cleanup_csrf_fixtures();

	setup_csrf_cookie("cookie_priority");
	setup_csrf_meta_tag("meta_priority");
	setup_csrf_input("input_priority");

	let token = get_csrf_token();
	assert_eq!(token, Some("cookie_priority".to_string()));

	cleanup_csrf_fixtures();
}

/// Test that get_csrf_token() falls back to meta tag when no cookie
#[wasm_bindgen_test]
fn test_csrf_token_priority_meta_fallback() {
	cleanup_csrf_fixtures();

	setup_csrf_meta_tag("meta_fallback");
	setup_csrf_input("input_fallback");

	let token = get_csrf_token();
	assert_eq!(token, Some("meta_fallback".to_string()));

	cleanup_csrf_fixtures();
}

/// Test that get_csrf_token() falls back to input when no cookie or meta
#[wasm_bindgen_test]
fn test_csrf_token_priority_input_fallback() {
	cleanup_csrf_fixtures();

	setup_csrf_input("input_only");

	let token = get_csrf_token();
	assert_eq!(token, Some("input_only".to_string()));

	cleanup_csrf_fixtures();
}

/// Test that get_csrf_token() returns None when no sources available
#[wasm_bindgen_test]
fn test_csrf_token_none_when_no_sources() {
	cleanup_csrf_fixtures();

	let token = get_csrf_token();
	assert!(token.is_none());

	cleanup_csrf_fixtures();
}

// ============================================================================
// CsrfManager Integration Tests
// ============================================================================

/// Test CsrfManager caches token after first fetch
#[wasm_bindgen_test]
fn test_csrf_manager_caching() {
	cleanup_csrf_fixtures();

	let manager = CsrfManager::new();

	// Initially no cached token
	assert!(manager.cached_token().is_none());

	// Set up cookie and fetch
	setup_csrf_cookie("manager_token");
	let token = manager.get_or_fetch_token();
	assert_eq!(token, Some("manager_token".to_string()));

	// Should be cached
	assert_eq!(manager.cached_token(), Some("manager_token".to_string()));

	// Remove cookie, but cached value should persist
	cleanup_csrf_fixtures();
	assert_eq!(manager.cached_token(), Some("manager_token".to_string()));

	// Clear manager cache
	manager.clear();
	assert!(manager.cached_token().is_none());
}

/// Test CsrfManager refresh updates cache
#[wasm_bindgen_test]
fn test_csrf_manager_refresh() {
	cleanup_csrf_fixtures();

	let manager = CsrfManager::new();

	// Initial fetch
	setup_csrf_cookie("initial_token");
	let _ = manager.get_or_fetch_token();
	assert_eq!(manager.cached_token(), Some("initial_token".to_string()));

	// Update cookie
	setup_csrf_cookie("refreshed_token");

	// Refresh should update cache
	let refreshed = manager.refresh();
	assert_eq!(refreshed, Some("refreshed_token".to_string()));
	assert_eq!(manager.cached_token(), Some("refreshed_token".to_string()));

	cleanup_csrf_fixtures();
}

/// Test CsrfManager manual token setting
#[wasm_bindgen_test]
fn test_csrf_manager_manual_set() {
	cleanup_csrf_fixtures();

	let manager = CsrfManager::new();

	manager.set_token("manual_token");
	assert_eq!(manager.cached_token(), Some("manual_token".to_string()));

	// get_or_fetch should return cached manual token
	let token = manager.get_or_fetch_token();
	assert_eq!(token, Some("manual_token".to_string()));

	cleanup_csrf_fixtures();
}

// ============================================================================
// Edge Case Tests
// ============================================================================

/// Test token with special characters
#[wasm_bindgen_test]
fn test_csrf_token_special_characters() {
	cleanup_csrf_fixtures();

	// Django tokens are typically alphanumeric but test edge case
	setup_csrf_cookie("token_with-underscore.dot");

	let token = get_csrf_token_from_cookie();
	assert_eq!(token, Some("token_with-underscore.dot".to_string()));

	cleanup_csrf_fixtures();
}

/// Test overwriting existing token
#[wasm_bindgen_test]
fn test_csrf_token_overwrite() {
	cleanup_csrf_fixtures();

	setup_csrf_cookie("first_token");
	assert_eq!(
		get_csrf_token_from_cookie(),
		Some("first_token".to_string())
	);

	setup_csrf_cookie("second_token");
	assert_eq!(
		get_csrf_token_from_cookie(),
		Some("second_token".to_string())
	);

	cleanup_csrf_fixtures();
}
