//! Android Digital Asset Links (assetlinks.json) file handler.
//!
//! This module provides an HTTP handler for serving the assetlinks.json file at
//! `/.well-known/assetlinks.json`.

use std::sync::Arc;

use async_trait::async_trait;
use bytes::Bytes;
use reinhardt_core::exception::Result;
use reinhardt_http::{Handler, Request, Response};

use crate::config::AndroidConfig;
use crate::error::DeeplinkError;

/// Handler for serving the Android Digital Asset Links file.
///
/// This handler serves the assetlinks.json file required for Android App Links.
/// The JSON content is pre-computed and cached for optimal performance.
///
/// # Endpoint
///
/// - `GET /.well-known/assetlinks.json`
///
/// # Response Headers
///
/// - `Content-Type: application/json`
/// - `Cache-Control: max-age=86400, public` (24 hours; Asset Links files are static config)
/// - `X-Content-Type-Options: nosniff`
/// - `Access-Control-Allow-Origin: *` (required by Google Digital Asset Links specification)
///
/// # Example
///
/// ```rust
/// use reinhardt_deeplink::{AssetLinksHandler, AndroidConfig};
///
/// let config = AndroidConfig::builder()
///     .package_name("com.example.app")
///     .sha256_fingerprint("FA:C6:17:45:DC:09:03:78:6F:B9:ED:E6:2A:96:2B:39:9F:73:48:F0:BB:6F:89:9B:83:32:66:75:91:03:3B:9C")
///     .build()
///     .unwrap();
///
/// let handler = AssetLinksHandler::new(config).unwrap();
/// ```
#[derive(Clone)]
pub struct AssetLinksHandler {
	/// The Android configuration (shared ownership for efficient cloning).
	config: Arc<AndroidConfig>,

	/// Pre-computed JSON response body as bytes.
	cached_json: Bytes,
}

impl AssetLinksHandler {
	/// Creates a new AssetLinks handler with the given configuration.
	///
	/// The JSON content is pre-computed at construction time for optimal performance.
	///
	/// # Errors
	///
	/// Returns an error if JSON serialization fails.
	pub fn new(config: AndroidConfig) -> std::result::Result<Self, DeeplinkError> {
		let json_string = serde_json::to_string_pretty(&config)?;
		Ok(Self {
			config: Arc::new(config),
			cached_json: Bytes::from(json_string),
		})
	}

	/// Returns the cached JSON content as a string slice.
	pub fn json(&self) -> &str {
		std::str::from_utf8(&self.cached_json).expect(
			"cached_json was serialized from valid UTF-8 strings and cannot be invalid UTF-8",
		)
	}
}

#[async_trait]
impl Handler for AssetLinksHandler {
	/// Serves the pre-computed Asset Links JSON response.
	///
	/// This handler does not inspect the HTTP method because method enforcement
	/// is handled by the router layer (GET-only route registration). If used
	/// outside a router context, callers must ensure only GET requests are forwarded.
	async fn handle(&self, _request: Request) -> Result<Response> {
		// Access-Control-Allow-Origin: * is required per Google's Digital Asset
		// Links specification. Asset Links files must be publicly accessible
		// for Android to verify app-site associations.
		Ok(Response::ok()
			.with_header("Content-Type", "application/json")
			.with_header("Cache-Control", "max-age=86400, public")
			.with_header("X-Content-Type-Options", "nosniff")
			.with_header("Access-Control-Allow-Origin", "*")
			.with_body(self.cached_json.clone()))
	}
}

impl std::fmt::Debug for AssetLinksHandler {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("AssetLinksHandler")
			.field("config", &self.config)
			.field("cached_json_len", &self.cached_json.len())
			.finish()
	}
}

#[cfg(test)]
mod tests {
	use rstest::rstest;

	use super::*;

	const VALID_FINGERPRINT: &str = "FA:C6:17:45:DC:09:03:78:6F:B9:ED:E6:2A:96:2B:39:9F:73:48:F0:BB:6F:89:9B:83:32:66:75:91:03:3B:9C";

	fn create_test_config() -> AndroidConfig {
		AndroidConfig::builder()
			.package_name("com.example.app")
			.sha256_fingerprint(VALID_FINGERPRINT)
			.build()
			.unwrap()
	}

	#[rstest]
	fn test_handler_creation() {
		let config = create_test_config();
		let handler = AssetLinksHandler::new(config).unwrap();

		let json = handler.json();
		assert!(json.contains("android_app"));
		assert!(json.contains("com.example.app"));
	}

	#[rstest]
	fn test_json_array_format() {
		let config = create_test_config();
		let handler = AssetLinksHandler::new(config).unwrap();

		let json = handler.json();
		// Verify it starts as a JSON array
		let trimmed = json.trim();
		assert!(trimmed.starts_with('['), "JSON should be an array");
		assert!(trimmed.ends_with(']'), "JSON should be an array");
	}

	#[rstest]
	fn test_json_valid() {
		let config = create_test_config();
		let handler = AssetLinksHandler::new(config).unwrap();

		let json = handler.json();
		let parsed: Vec<serde_json::Value> = serde_json::from_str(json).unwrap();
		assert!(!parsed.is_empty());

		let statement = &parsed[0];
		assert!(statement.get("relation").is_some());
		assert!(statement.get("target").is_some());
	}

	#[rstest]
	#[tokio::test]
	async fn test_handler_response() {
		let config = create_test_config();
		let handler = AssetLinksHandler::new(config).unwrap();

		let request = Request::builder()
			.method(hyper::Method::GET)
			.uri("/.well-known/assetlinks.json")
			.build()
			.unwrap();

		let response = handler.handle(request).await.unwrap();

		assert_eq!(response.status, hyper::StatusCode::OK);
		assert!(response.headers.contains_key("content-type"));
		assert!(response.headers.contains_key("cache-control"));
	}

	#[rstest]
	fn test_handler_clone() {
		let config = create_test_config();
		let handler = AssetLinksHandler::new(config).unwrap();
		let cloned = handler.clone();

		assert_eq!(handler.json(), cloned.json());
	}
}
