//! Handler integration system for versioned endpoints
//!
//! Provides traits and utilities for creating version-aware handlers
//! that can respond differently based on API version.

use super::BaseVersioning;
use async_trait::async_trait;
use bytes::Bytes;
use reinhardt_core::exception::Result;
use reinhardt_http::Handler;
use reinhardt_http::{Request, Response};
use std::collections::HashMap;
use std::sync::Arc;

/// Trait for handlers that can respond differently based on API version
#[async_trait]
pub trait VersionedHandler: Send + Sync {
	/// Handle a request with version information
	async fn handle_versioned(&self, request: Request, version: &str) -> Result<Response>;

	/// Get supported versions for this handler
	fn supported_versions(&self) -> Vec<String>;

	/// Check if this handler supports a specific version
	fn supports_version(&self, version: &str) -> bool {
		self.supported_versions().contains(&version.to_string())
	}
}

/// Wrapper that makes any VersionedHandler compatible with the standard Handler trait
pub struct VersionedHandlerWrapper {
	inner: Arc<dyn VersionedHandler>,
	versioning: Arc<dyn BaseVersioning>,
}

impl VersionedHandlerWrapper {
	/// Create a new versioned handler wrapper
	pub fn new(handler: Arc<dyn VersionedHandler>, versioning: Arc<dyn BaseVersioning>) -> Self {
		Self {
			inner: handler,
			versioning,
		}
	}
}

#[async_trait]
impl Handler for VersionedHandlerWrapper {
	async fn handle(&self, request: Request) -> Result<Response> {
		// Determine version from request
		let version = self.versioning.determine_version(&request).await?;

		// Check if handler supports this version
		if !self.inner.supports_version(&version) {
			return Err(reinhardt_core::exception::Error::Validation(format!(
				"Handler does not support version: {}",
				version
			)));
		}

		// Delegate to versioned handler
		self.inner.handle_versioned(request, &version).await
	}
}

/// Simple versioned handler that returns different responses based on version
pub struct SimpleVersionedHandler {
	responses: HashMap<String, String>,
	default_response: String,
}

impl SimpleVersionedHandler {
	/// Create a new simple versioned handler
	pub fn new() -> Self {
		Self {
			responses: HashMap::new(),
			default_response: r#"{"message": "API version not supported"}"#.to_string(),
		}
	}

	/// Add a response for a specific version
	pub fn with_version_response(mut self, version: &str, response: &str) -> Self {
		self.responses
			.insert(version.to_string(), response.to_string());
		self
	}

	/// Set the default response for unsupported versions
	pub fn with_default_response(mut self, response: &str) -> Self {
		self.default_response = response.to_string();
		self
	}
}

impl Default for SimpleVersionedHandler {
	fn default() -> Self {
		Self::new()
	}
}

#[async_trait]
impl VersionedHandler for SimpleVersionedHandler {
	async fn handle_versioned(&self, _request: Request, version: &str) -> Result<Response> {
		let response_body = self
			.responses
			.get(version)
			.unwrap_or(&self.default_response)
			.clone();

		Ok(Response::ok().with_body(Bytes::from(response_body)))
	}

	fn supported_versions(&self) -> Vec<String> {
		self.responses.keys().cloned().collect()
	}
}

/// Handler that can be configured with different logic per version
pub struct ConfigurableVersionedHandler {
	handlers: HashMap<String, Box<dyn Handler + Send + Sync>>,
	default_handler: Option<Box<dyn Handler + Send + Sync>>,
}

impl ConfigurableVersionedHandler {
	/// Create a new configurable versioned handler
	pub fn new() -> Self {
		Self {
			handlers: HashMap::new(),
			default_handler: None,
		}
	}

	/// Add a handler for a specific version
	pub fn with_version_handler(
		mut self,
		version: &str,
		handler: Box<dyn Handler + Send + Sync>,
	) -> Self {
		self.handlers.insert(version.to_string(), handler);
		self
	}

	/// Set the default handler for unsupported versions
	pub fn with_default_handler(mut self, handler: Box<dyn Handler + Send + Sync>) -> Self {
		self.default_handler = Some(handler);
		self
	}
}

impl Default for ConfigurableVersionedHandler {
	fn default() -> Self {
		Self::new()
	}
}

#[async_trait]
impl VersionedHandler for ConfigurableVersionedHandler {
	async fn handle_versioned(&self, request: Request, version: &str) -> Result<Response> {
		if let Some(handler) = self.handlers.get(version) {
			handler.handle(request).await
		} else if let Some(default_handler) = &self.default_handler {
			default_handler.handle(request).await
		} else {
			Err(reinhardt_core::exception::Error::Validation(format!(
				"No handler available for version: {}",
				version
			)))
		}
	}

	fn supported_versions(&self) -> Vec<String> {
		self.handlers.keys().cloned().collect()
	}
}

/// Builder for creating versioned handlers with middleware
pub struct VersionedHandlerBuilder {
	versioning: Arc<dyn BaseVersioning>,
	handlers: HashMap<String, Arc<dyn VersionedHandler>>,
	default_handler: Option<Arc<dyn VersionedHandler>>,
}

impl VersionedHandlerBuilder {
	/// Create a new versioned handler builder
	pub fn new(versioning: Arc<dyn BaseVersioning>) -> Self {
		Self {
			versioning,
			handlers: HashMap::new(),
			default_handler: None,
		}
	}

	/// Add a handler for a specific version
	pub fn with_version_handler(
		mut self,
		version: &str,
		handler: Arc<dyn VersionedHandler>,
	) -> Self {
		self.handlers.insert(version.to_string(), handler);
		self
	}

	/// Set the default handler for unsupported versions
	pub fn with_default_handler(mut self, handler: Arc<dyn VersionedHandler>) -> Self {
		self.default_handler = Some(handler);
		self
	}

	/// Build the final versioned handler
	pub fn build(self) -> Arc<dyn Handler> {
		let versioned_handler = Arc::new(ConfigurableVersionedHandler {
			handlers: self
				.handlers
				.into_iter()
				.map(|(k, v)| {
					(
						k,
						Box::new(VersionedHandlerWrapper {
							inner: v,
							versioning: self.versioning.clone(),
						}) as Box<dyn Handler + Send + Sync>,
					)
				})
				.collect(),
			default_handler: self.default_handler.map(|h| {
				Box::new(VersionedHandlerWrapper {
					inner: h,
					versioning: self.versioning.clone(),
				}) as Box<dyn Handler + Send + Sync>
			}),
		});

		Arc::new(VersionedHandlerWrapper {
			inner: versioned_handler,
			versioning: self.versioning,
		})
	}
}

/// Utility for creating version-specific responses
pub struct VersionResponseBuilder {
	version: String,
	data: serde_json::Value,
}

impl VersionResponseBuilder {
	/// Create a new version response builder
	pub fn new(version: &str) -> Self {
		Self {
			version: version.to_string(),
			data: serde_json::json!({
				"version": version,
				"data": {}
			}),
		}
	}

	/// Add data to the response
	pub fn with_data(mut self, data: serde_json::Value) -> Self {
		self.data["data"] = data;
		self
	}

	/// Add a field to the response
	pub fn with_field(mut self, key: &str, value: serde_json::Value) -> Self {
		self.data["data"][key] = value;
		self
	}

	/// Add version-specific information
	pub fn with_version_info(mut self, info: serde_json::Value) -> Self {
		self.data["version_info"] = info;
		self
	}

	/// Get the version string
	pub fn version(&self) -> &str {
		&self.version
	}

	/// Build the response
	pub fn build(self) -> Response {
		let body = serde_json::to_string(&self.data).unwrap_or_else(|_| "{}".to_string());
		Response::ok().with_body(Bytes::from(body))
	}
}

/// Macro for creating versioned handlers easily
#[macro_export]
macro_rules! versioned_handler {
    ($versioning:expr, {
        $($version:literal => $handler:expr),* $(,)?
    }) => {{
        let mut builder = $crate::versioning::handler::VersionedHandlerBuilder::new($versioning);
        $(
            builder = builder.with_version_handler($version, $handler);
        )*
        builder.build()
    }};

    ($versioning:expr, {
        $($version:literal => $handler:expr),* $(,)?
    }, default => $default_handler:expr) => {{
        let mut builder = $crate::versioning::handler::VersionedHandlerBuilder::new($versioning);
        $(
            builder = builder.with_version_handler($version, $handler);
        )*
        builder.with_default_handler($default_handler).build()
    }};
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::versioning::{AcceptHeaderVersioning, URLPathVersioning};
	use bytes::Bytes;

	// Test handler for testing
	struct TestHandler {
		response: String,
	}

	#[async_trait]
	impl Handler for TestHandler {
		async fn handle(&self, _request: Request) -> Result<Response> {
			Ok(Response::ok().with_body(Bytes::from(self.response.clone())))
		}
	}

	#[tokio::test]
	async fn test_simple_versioned_handler() {
		let handler = SimpleVersionedHandler::new()
			.with_version_response("1.0", r#"{"message": "Version 1.0"}"#)
			.with_version_response("2.0", r#"{"message": "Version 2.0"}"#)
			.with_default_response(r#"{"error": "Unsupported version"}"#);

		assert!(handler.supports_version("1.0"));
		assert!(handler.supports_version("2.0"));
		assert!(!handler.supports_version("3.0"));

		let supported = handler.supported_versions();
		assert!(supported.contains(&"1.0".to_string()));
		assert!(supported.contains(&"2.0".to_string()));
	}

	#[tokio::test]
	async fn test_configurable_versioned_handler() {
		let _v1_handler = Arc::new(TestHandler {
			response: r#"{"version": "1.0"}"#.to_string(),
		});
		let _v2_handler = Arc::new(TestHandler {
			response: r#"{"version": "2.0"}"#.to_string(),
		});
		let _default_handler = Arc::new(TestHandler {
			response: r#"{"error": "Unsupported version"}"#.to_string(),
		});

		let handler = ConfigurableVersionedHandler::new()
			.with_version_handler(
				"1.0",
				Box::new(TestHandler {
					response: r#"{"version": "1.0"}"#.to_string(),
				}),
			)
			.with_version_handler(
				"2.0",
				Box::new(TestHandler {
					response: r#"{"version": "2.0"}"#.to_string(),
				}),
			)
			.with_default_handler(Box::new(TestHandler {
				response: r#"{"error": "Unsupported version"}"#.to_string(),
			}));

		assert!(handler.supports_version("1.0"));
		assert!(handler.supports_version("2.0"));
		assert!(!handler.supports_version("3.0"));
	}

	#[tokio::test]
	async fn test_versioned_handler_wrapper() {
		let versioning = Arc::new(
			URLPathVersioning::new()
				.with_default_version("1.0")
				.with_allowed_versions(vec!["1", "1.0", "2", "2.0"]),
		);

		let handler = SimpleVersionedHandler::new()
			.with_version_response("1", r#"{"version": "1"}"#)
			.with_version_response("2", r#"{"version": "2"}"#);

		let wrapper = VersionedHandlerWrapper::new(Arc::new(handler), versioning);

		// Test with v1 request
		let request = crate::versioning::test_utils::create_test_request("/v1/test", vec![]);
		let response = wrapper.handle(request).await.unwrap();
		let body = String::from_utf8(response.body.to_vec()).unwrap();
		assert!(body.contains("\"version\": \"1\""));

		// Test with v2 request
		let request = crate::versioning::test_utils::create_test_request("/v2/test", vec![]);
		let response = wrapper.handle(request).await.unwrap();
		let body = String::from_utf8(response.body.to_vec()).unwrap();
		assert!(body.contains("\"version\": \"2\""));
	}

	#[tokio::test]
	async fn test_version_response_builder() {
		let response = VersionResponseBuilder::new("1.0")
			.with_data(serde_json::json!({
				"users": ["alice", "bob"]
			}))
			.with_field("count", serde_json::json!(2))
			.with_version_info(serde_json::json!({
				"deprecated": false,
				"supported_until": "2024-12-31"
			}))
			.build();

		let body = String::from_utf8(response.body.to_vec()).unwrap();
		let data: serde_json::Value = serde_json::from_str(&body).unwrap();

		assert_eq!(data["version"], "1.0");
		assert_eq!(data["data"]["count"], 2);
		assert_eq!(data["data"]["users"], serde_json::json!(["alice", "bob"]));
		assert!(!data["version_info"]["deprecated"].as_bool().unwrap());
	}

	#[tokio::test]
	async fn test_versioned_handler_builder() {
		let versioning = Arc::new(
			AcceptHeaderVersioning::new()
				.with_default_version("1.0")
				.with_allowed_versions(vec!["1.0", "2.0"]),
		);

		let v1_handler = Arc::new(
			SimpleVersionedHandler::new().with_version_response("1.0", r#"{"version": "1.0"}"#),
		);
		let v2_handler = Arc::new(
			SimpleVersionedHandler::new().with_version_response("2.0", r#"{"version": "2.0"}"#),
		);

		let handler = VersionedHandlerBuilder::new(versioning)
			.with_version_handler("1.0", v1_handler)
			.with_version_handler("2.0", v2_handler)
			.build();

		// Test with Accept header versioning
		let request = crate::versioning::test_utils::create_test_request(
			"/test",
			vec![(
				"accept".to_string(),
				"application/json; version=2.0".to_string(),
			)],
		);
		let response = handler.handle(request).await.unwrap();
		let body = String::from_utf8(response.body.to_vec()).unwrap();
		assert!(body.contains("\"version\": \"2.0\""));
	}
}
