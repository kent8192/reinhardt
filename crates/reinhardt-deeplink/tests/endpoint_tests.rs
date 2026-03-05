//! Endpoint HTTP handlers tests
//!
//! Tests for AasaHandler and AssetLinksHandler covering:
//! - Happy path: Handler creation, JSON content, response headers
//! - Error path: Invalid configurations
//! - Edge cases: Caching behavior, multiple statements
//! - Sanity: Content type, CORS headers

use reinhardt_deeplink::{AasaHandler, AndroidConfig, AssetLinksHandler, IosConfig};
use reinhardt_http::{Handler, Request};
use rstest::*;

const VALID_APP_ID: &str = "TEAM123456.com.example.app";
const VALID_FINGERPRINT: &str = "FA:C6:17:45:DC:09:03:78:6F:B9:ED:E6:2A:96:2B:39:9F:73:48:F0:BB:6F:89:9B:83:32:66:75:91:03:3B:9C";

// ============================================================================
// AasaHandler Tests
// ============================================================================

#[rstest]
fn test_aasa_handler_creation() {
	let config = IosConfig::builder()
		.app_id(VALID_APP_ID)
		.paths(&["/products/*"])
		.build();

	let handler = AasaHandler::new(config);
	assert!(handler.is_ok());
}

#[rstest]
fn test_aasa_handler_json_content() {
	let config = IosConfig::builder()
		.app_id(VALID_APP_ID)
		.paths(&["/products/*"])
		.build();

	let handler = AasaHandler::new(config).unwrap();
	let json = handler.json();

	assert!(json.contains("applinks"));
	assert!(json.contains(VALID_APP_ID));
	assert!(json.contains("/products/*"));
}

#[rstest]
fn test_aasa_handler_response_headers() {
	let config = IosConfig::builder()
		.app_id(VALID_APP_ID)
		.paths(&["/products/*"])
		.build();

	let handler = AasaHandler::new(config).unwrap();

	// Create a minimal request
	let request = Request::builder()
		.method(hyper::Method::GET)
		.uri("/.well-known/apple-app-site-association")
		.build()
		.unwrap();

	let response = tokio_runtime().block_on(handler.handle(request)).unwrap();

	assert_eq!(response.status, hyper::StatusCode::OK);
	assert!(response.headers.contains_key("content-type"));
	assert!(response.headers.contains_key("cache-control"));
	assert!(response.headers.contains_key("x-content-type-options"));
	assert!(response.headers.contains_key("access-control-allow-origin"));
}

#[rstest]
fn test_aasa_handler_async_handle() {
	let config = IosConfig::builder()
		.app_id(VALID_APP_ID)
		.paths(&["/products/*"])
		.build();

	let handler = AasaHandler::new(config).unwrap();
	let request = Request::builder()
		.method(hyper::Method::GET)
		.uri("/.well-known/apple-app-site-association")
		.build()
		.unwrap();

	let response = tokio_runtime().block_on(handler.handle(request)).unwrap();

	assert_eq!(response.status, hyper::StatusCode::OK);
}

#[rstest]
fn test_aasa_handler_clone() {
	let config = IosConfig::builder()
		.app_id(VALID_APP_ID)
		.paths(&["/products/*"])
		.build();

	let handler = AasaHandler::new(config).unwrap();
	let cloned = handler.clone();

	assert_eq!(handler.json(), cloned.json());
}

// ============================================================================
// AssetLinksHandler Tests
// ============================================================================

#[rstest]
fn test_assetlinks_handler_creation() {
	let config = AndroidConfig::builder()
		.package_name("com.example.app")
		.sha256_fingerprint(VALID_FINGERPRINT)
		.build()
		.unwrap();

	let handler = AssetLinksHandler::new(config);
	assert!(handler.is_ok());
}

#[rstest]
fn test_assetlinks_handler_json_content() {
	let config = AndroidConfig::builder()
		.package_name("com.example.app")
		.sha256_fingerprint(VALID_FINGERPRINT)
		.build()
		.unwrap();

	let handler = AssetLinksHandler::new(config).unwrap();
	let json = handler.json();

	assert!(json.contains("android_app"));
	assert!(json.contains("com.example.app"));
	assert!(json.contains(VALID_FINGERPRINT));
}

#[rstest]
fn test_assetlinks_handler_json_array_format() {
	let config = AndroidConfig::builder()
		.package_name("com.example.app")
		.sha256_fingerprint(VALID_FINGERPRINT)
		.build()
		.unwrap();

	let handler = AssetLinksHandler::new(config).unwrap();
	let json = handler.json();

	// Should be a JSON array
	let trimmed = json.trim();
	assert!(trimmed.starts_with('['));
	assert!(trimmed.ends_with(']'));
}

#[rstest]
fn test_assetlinks_handler_response_headers() {
	let config = AndroidConfig::builder()
		.package_name("com.example.app")
		.sha256_fingerprint(VALID_FINGERPRINT)
		.build()
		.unwrap();

	let handler = AssetLinksHandler::new(config).unwrap();

	let request = Request::builder()
		.method(hyper::Method::GET)
		.uri("/.well-known/assetlinks.json")
		.build()
		.unwrap();

	let response = tokio_runtime().block_on(handler.handle(request)).unwrap();

	assert_eq!(response.status, hyper::StatusCode::OK);
	assert!(response.headers.contains_key("content-type"));
	assert!(response.headers.contains_key("cache-control"));
	assert!(response.headers.contains_key("x-content-type-options"));
	assert!(response.headers.contains_key("access-control-allow-origin"));
}

#[rstest]
fn test_assetlinks_handler_async_handle() {
	let config = AndroidConfig::builder()
		.package_name("com.example.app")
		.sha256_fingerprint(VALID_FINGERPRINT)
		.build()
		.unwrap();

	let handler = AssetLinksHandler::new(config).unwrap();
	let request = Request::builder()
		.method(hyper::Method::GET)
		.uri("/.well-known/assetlinks.json")
		.build()
		.unwrap();

	let response = tokio_runtime().block_on(handler.handle(request)).unwrap();

	assert_eq!(response.status, hyper::StatusCode::OK);
}

#[rstest]
fn test_assetlinks_handler_clone() {
	let config = AndroidConfig::builder()
		.package_name("com.example.app")
		.sha256_fingerprint(VALID_FINGERPRINT)
		.build()
		.unwrap();

	let handler = AssetLinksHandler::new(config).unwrap();
	let cloned = handler.clone();

	assert_eq!(handler.json(), cloned.json());
}

// ============================================================================
// Error Path Tests
// ============================================================================

// Note: Current implementation doesn't have validation errors that can occur
// during handler creation, since configs are already validated before building.
// These tests document expected behavior if invalid configs were passed.

// ============================================================================
// Edge Cases Tests
// ============================================================================

#[rstest]
fn test_aasa_handler_caching_behavior() {
	let config = IosConfig::builder()
		.app_id(VALID_APP_ID)
		.paths(&["/products/*"])
		.build();

	let handler = AasaHandler::new(config).unwrap();

	// Verify JSON is pre-computed (cached) by checking it's valid
	let json = handler.json();
	let parsed: serde_json::Value = serde_json::from_str(json).unwrap();
	assert!(parsed.get("applinks").is_some());
}

#[rstest]
fn test_assetlinks_handler_multiple_statements() {
	let config = AndroidConfig::builder()
		.package_name("com.example.app")
		.sha256_fingerprint(VALID_FINGERPRINT)
		.additional_package("com.example.app2", &[VALID_FINGERPRINT])
		.build()
		.unwrap();

	let handler = AssetLinksHandler::new(config).unwrap();
	let json = handler.json();

	let parsed: Vec<serde_json::Value> = serde_json::from_str(json).unwrap();
	assert_eq!(parsed.len(), 2);
}

// ============================================================================
// Sanity Tests
// ============================================================================

#[rstest]
fn test_aasa_handler_content_type() {
	let config = IosConfig::builder()
		.app_id(VALID_APP_ID)
		.paths(&["/products/*"])
		.build();

	let handler = AasaHandler::new(config).unwrap();
	let request = Request::builder()
		.method(hyper::Method::GET)
		.uri("/.well-known/apple-app-site-association")
		.build()
		.unwrap();

	let response = tokio_runtime().block_on(handler.handle(request)).unwrap();

	let content_type = response.headers.get("content-type").unwrap();
	assert_eq!(content_type.to_str().unwrap(), "application/json");
}

#[rstest]
fn test_assetlinks_handler_content_type() {
	let config = AndroidConfig::builder()
		.package_name("com.example.app")
		.sha256_fingerprint(VALID_FINGERPRINT)
		.build()
		.unwrap();

	let handler = AssetLinksHandler::new(config).unwrap();
	let request = Request::builder()
		.method(hyper::Method::GET)
		.uri("/.well-known/assetlinks.json")
		.build()
		.unwrap();

	let response = tokio_runtime().block_on(handler.handle(request)).unwrap();

	let content_type = response.headers.get("content-type").unwrap();
	assert_eq!(content_type.to_str().unwrap(), "application/json");
}

#[rstest]
fn test_both_handlers_cache_control() {
	let ios_config = IosConfig::builder()
		.app_id(VALID_APP_ID)
		.paths(&["/products/*"])
		.build();

	let android_config = AndroidConfig::builder()
		.package_name("com.example.app")
		.sha256_fingerprint(VALID_FINGERPRINT)
		.build()
		.unwrap();

	let aasa_handler = AasaHandler::new(ios_config).unwrap();
	let assetlinks_handler = AssetLinksHandler::new(android_config).unwrap();

	let request_aasa = Request::builder()
		.method(hyper::Method::GET)
		.uri("/.well-known/apple-app-site-association")
		.build()
		.unwrap();

	let request_assetlinks = Request::builder()
		.method(hyper::Method::GET)
		.uri("/.well-known/assetlinks.json")
		.build()
		.unwrap();

	let response_aasa = tokio_runtime()
		.block_on(aasa_handler.handle(request_aasa))
		.unwrap();

	let response_assetlinks = tokio_runtime()
		.block_on(assetlinks_handler.handle(request_assetlinks))
		.unwrap();

	let cache_control_aasa = response_aasa.headers.get("cache-control").unwrap();
	let cache_control_assetlinks = response_assetlinks.headers.get("cache-control").unwrap();

	// Both should have cache-control header
	assert_eq!(
		cache_control_aasa.to_str().unwrap(),
		"max-age=86400, public"
	);
	assert_eq!(
		cache_control_assetlinks.to_str().unwrap(),
		"max-age=86400, public"
	);
}

#[rstest]
fn test_both_headers_cors() {
	let ios_config = IosConfig::builder()
		.app_id(VALID_APP_ID)
		.paths(&["/products/*"])
		.build();

	let android_config = AndroidConfig::builder()
		.package_name("com.example.app")
		.sha256_fingerprint(VALID_FINGERPRINT)
		.build()
		.unwrap();

	let aasa_handler = AasaHandler::new(ios_config).unwrap();
	let assetlinks_handler = AssetLinksHandler::new(android_config).unwrap();

	let request_aasa = Request::builder()
		.method(hyper::Method::GET)
		.uri("/.well-known/apple-app-site-association")
		.build()
		.unwrap();

	let request_assetlinks = Request::builder()
		.method(hyper::Method::GET)
		.uri("/.well-known/assetlinks.json")
		.build()
		.unwrap();

	let response_aasa = tokio_runtime()
		.block_on(aasa_handler.handle(request_aasa))
		.unwrap();

	let response_assetlinks = tokio_runtime()
		.block_on(assetlinks_handler.handle(request_assetlinks))
		.unwrap();

	let cors_aasa = response_aasa
		.headers
		.get("access-control-allow-origin")
		.unwrap();
	let cors_assetlinks = response_assetlinks
		.headers
		.get("access-control-allow-origin")
		.unwrap();

	// Both should have CORS header
	assert_eq!(cors_aasa.to_str().unwrap(), "*");
	assert_eq!(cors_assetlinks.to_str().unwrap(), "*");
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Creates a new tokio runtime for async test execution.
fn tokio_runtime() -> tokio::runtime::Runtime {
	tokio::runtime::Runtime::new().unwrap()
}
