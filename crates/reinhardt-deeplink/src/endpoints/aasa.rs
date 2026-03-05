//! Apple App Site Association (AASA) file handler.
//!
//! This module provides an HTTP handler for serving the AASA file at
//! `/.well-known/apple-app-site-association`.

use std::sync::Arc;

use async_trait::async_trait;
use bytes::Bytes;
use reinhardt_core::exception::Result;
use reinhardt_http::{Handler, Request, Response};

use crate::config::IosConfig;
use crate::error::DeeplinkError;

/// Handler for serving the Apple App Site Association file.
///
/// This handler serves the AASA file required for iOS Universal Links.
/// The JSON content is pre-computed and cached for optimal performance.
///
/// # Endpoints
///
/// - `GET /.well-known/apple-app-site-association`
/// - `GET /.well-known/apple-app-site-association.json` (alternative)
///
/// # Response Headers
///
/// - `Content-Type: application/json`
/// - `Cache-Control: max-age=86400, public` (24 hours; AASA files are static config)
/// - `X-Content-Type-Options: nosniff`
/// - `Access-Control-Allow-Origin: *` (required by Apple Universal Links specification)
///
/// # Example
///
/// ```rust
/// use reinhardt_deeplink::{AasaHandler, IosConfig};
///
/// let config = IosConfig::builder()
///     .app_id("TEAM123456.com.example.app")
///     .paths(&["/products/*"])
///     .build();
///
/// let handler = AasaHandler::new(config).unwrap();
/// ```
#[derive(Clone)]
pub struct AasaHandler {
	/// The iOS configuration (shared ownership for efficient cloning).
	config: Arc<IosConfig>,

	/// Pre-computed JSON response body as bytes.
	cached_json: Bytes,
}

impl AasaHandler {
	/// Creates a new AASA handler with the given configuration.
	///
	/// The JSON content is pre-computed at construction time for optimal performance.
	///
	/// # Errors
	///
	/// Returns an error if JSON serialization fails.
	pub fn new(config: IosConfig) -> std::result::Result<Self, DeeplinkError> {
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
impl Handler for AasaHandler {
	/// Serves the pre-computed AASA JSON response.
	///
	/// This handler does not inspect the HTTP method because method enforcement
	/// is handled by the router layer (GET-only route registration). If used
	/// outside a router context, callers must ensure only GET requests are forwarded.
	async fn handle(&self, _request: Request) -> Result<Response> {
		// Access-Control-Allow-Origin: * is required per Apple's Universal Links
		// specification. AASA files must be publicly accessible without
		// authentication for iOS to verify app-site associations.
		Ok(Response::ok()
			.with_header("Content-Type", "application/json")
			.with_header("Cache-Control", "max-age=86400, public")
			.with_header("X-Content-Type-Options", "nosniff")
			.with_header("Access-Control-Allow-Origin", "*")
			.with_body(self.cached_json.clone()))
	}
}

impl std::fmt::Debug for AasaHandler {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("AasaHandler")
			.field("config", &self.config)
			.field("cached_json_len", &self.cached_json.len())
			.finish()
	}
}

#[cfg(test)]
mod tests {
	use rstest::rstest;

	use super::*;

	fn create_test_config() -> IosConfig {
		IosConfig::builder()
			.app_id("TEAM123456.com.example.app")
			.paths(&["/products/*", "/users/*"])
			.build()
	}

	#[rstest]
	fn test_handler_creation() {
		let config = create_test_config();
		let handler = AasaHandler::new(config).unwrap();

		let json = handler.json();
		assert!(json.contains("applinks"));
		assert!(json.contains("TEAM123456.com.example.app"));
	}

	#[rstest]
	fn test_json_format() {
		let config = create_test_config();
		let handler = AasaHandler::new(config).unwrap();

		let json = handler.json();
		// Verify it's valid JSON
		let parsed: serde_json::Value = serde_json::from_str(json).unwrap();
		assert!(parsed.get("applinks").is_some());
	}

	#[rstest]
	#[tokio::test]
	async fn test_handler_response() {
		let config = create_test_config();
		let handler = AasaHandler::new(config).unwrap();

		// Create a minimal request
		let request = Request::builder()
			.method(hyper::Method::GET)
			.uri("/.well-known/apple-app-site-association")
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
		let handler = AasaHandler::new(config).unwrap();
		let cloned = handler.clone();

		assert_eq!(handler.json(), cloned.json());
	}
}
