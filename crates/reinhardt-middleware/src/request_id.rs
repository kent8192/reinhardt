//! Request ID middleware
//!
//! Generates or propagates unique request IDs for tracking and logging.
//! Adds X-Request-ID header to both requests and responses.

use async_trait::async_trait;
use hyper::header::HeaderName;
use reinhardt_http::{Handler, Middleware, Request, Response, Result};
use std::sync::Arc;
use uuid::Uuid;

/// Header name for request ID
pub const REQUEST_ID_HEADER: &str = "X-Request-ID";

/// Configuration for request ID generation
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct RequestIdConfig {
	/// Generate new request ID if not present in request
	pub generate_if_missing: bool,
	/// Always generate new request ID (ignore incoming header)
	pub always_generate: bool,
	/// Custom header name (default: X-Request-ID)
	pub header_name: String,
}

impl RequestIdConfig {
	/// Create a new default configuration
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_middleware::RequestIdConfig;
	///
	/// let config = RequestIdConfig::new();
	/// assert!(config.generate_if_missing);
	/// assert!(!config.always_generate);
	/// ```
	pub fn new() -> Self {
		Self {
			generate_if_missing: true,
			always_generate: false,
			header_name: REQUEST_ID_HEADER.to_string(),
		}
	}

	/// Always generate new request IDs (ignore incoming headers)
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_middleware::RequestIdConfig;
	///
	/// let config = RequestIdConfig::new().always_generate();
	/// assert!(config.always_generate);
	/// ```
	pub fn always_generate(mut self) -> Self {
		self.always_generate = true;
		self
	}

	/// Use a custom header name
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_middleware::RequestIdConfig;
	///
	/// let config = RequestIdConfig::new().with_header("X-Correlation-ID".to_string());
	/// assert_eq!(config.header_name, "X-Correlation-ID");
	/// ```
	pub fn with_header(mut self, header_name: String) -> Self {
		self.header_name = header_name;
		self
	}

	/// Don't generate request ID if missing
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_middleware::RequestIdConfig;
	///
	/// let config = RequestIdConfig::new().no_generation();
	/// assert!(!config.generate_if_missing);
	/// ```
	pub fn no_generation(mut self) -> Self {
		self.generate_if_missing = false;
		self
	}
}

impl Default for RequestIdConfig {
	fn default() -> Self {
		Self::new()
	}
}

/// Middleware for managing request IDs
///
/// Generates or propagates unique request IDs for request tracking and correlation.
///
/// # Examples
///
/// ```
/// use std::sync::Arc;
/// use reinhardt_middleware::{RequestIdMiddleware, RequestIdConfig};
/// use reinhardt_http::{Handler, Middleware, Request, Response};
/// use hyper::{StatusCode, Method, Version, HeaderMap};
/// use bytes::Bytes;
///
/// struct TestHandler;
///
/// #[async_trait::async_trait]
/// impl Handler for TestHandler {
///     async fn handle(&self, _request: Request) -> reinhardt_core::exception::Result<Response> {
///         Ok(Response::new(StatusCode::OK).with_body(Bytes::from("OK")))
///     }
/// }
///
/// # tokio_test::block_on(async {
/// let config = RequestIdConfig::new();
/// let middleware = RequestIdMiddleware::new(config);
/// let handler = Arc::new(TestHandler);
///
/// let request = Request::builder()
///     .method(Method::GET)
///     .uri("/test")
///     .version(Version::HTTP_11)
///     .headers(HeaderMap::new())
///     .body(Bytes::new())
///     .build()
///     .unwrap();
///
/// let response = middleware.process(request, handler).await.unwrap();
/// assert!(response.headers.contains_key("X-Request-ID"));
/// # });
/// ```
pub struct RequestIdMiddleware {
	config: RequestIdConfig,
}

impl RequestIdMiddleware {
	/// Create a new RequestIdMiddleware with the given configuration
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_middleware::{RequestIdMiddleware, RequestIdConfig};
	///
	/// let config = RequestIdConfig::new();
	/// let middleware = RequestIdMiddleware::new(config);
	/// ```
	pub fn new(config: RequestIdConfig) -> Self {
		Self { config }
	}

	/// Create a new RequestIdMiddleware with default configuration
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_middleware::RequestIdMiddleware;
	///
	/// let middleware = RequestIdMiddleware::default();
	/// ```
	pub fn with_defaults() -> Self {
		Self::new(RequestIdConfig::default())
	}

	/// Generate a new request ID
	fn generate_id(&self) -> String {
		Uuid::new_v4().to_string()
	}

	/// Get or generate request ID from request
	fn get_or_generate_id(&self, request: &Request) -> String {
		// Always generate if configured
		if self.config.always_generate {
			return self.generate_id();
		}

		// Try to get from existing header
		if let Some(existing_id) = request.headers.get(&self.config.header_name)
			&& let Ok(id_str) = existing_id.to_str()
			&& !id_str.is_empty()
		{
			return id_str.to_string();
		}

		// Generate if missing and configured to do so
		if self.config.generate_if_missing {
			self.generate_id()
		} else {
			String::new()
		}
	}
}

impl Default for RequestIdMiddleware {
	fn default() -> Self {
		Self::with_defaults()
	}
}

#[async_trait]
impl Middleware for RequestIdMiddleware {
	async fn process(&self, mut request: Request, handler: Arc<dyn Handler>) -> Result<Response> {
		// Get or generate request ID
		let request_id = self.get_or_generate_id(&request);

		// Add to request headers if not empty
		if !request_id.is_empty() {
			let header_name: HeaderName = self.config.header_name.parse().unwrap();
			request
				.headers
				.insert(header_name, request_id.parse().unwrap());
		}

		// Call the handler
		let mut response = handler.handle(request).await?;

		// Add request ID to response headers
		if !request_id.is_empty() {
			let header_name: HeaderName = self.config.header_name.parse().unwrap();
			response
				.headers
				.insert(header_name, request_id.parse().unwrap());
		}

		Ok(response)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use bytes::Bytes;
	use hyper::{HeaderMap, Method, StatusCode, Version};

	struct TestHandler;

	#[async_trait]
	impl Handler for TestHandler {
		async fn handle(&self, request: Request) -> Result<Response> {
			// Echo request ID in response body if present
			let request_id = request
				.headers
				.get(REQUEST_ID_HEADER)
				.and_then(|v| v.to_str().ok())
				.unwrap_or("none");
			Ok(Response::new(StatusCode::OK).with_body(Bytes::from(request_id.to_string())))
		}
	}

	#[tokio::test]
	async fn test_generate_request_id() {
		let config = RequestIdConfig::new();
		let middleware = RequestIdMiddleware::new(config);
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

		// Should have generated a request ID
		assert!(response.headers.contains_key(REQUEST_ID_HEADER));
		let request_id = response
			.headers
			.get(REQUEST_ID_HEADER)
			.unwrap()
			.to_str()
			.unwrap();
		assert!(!request_id.is_empty());
		// Should be a valid UUID format
		assert!(Uuid::parse_str(request_id).is_ok());
	}

	#[tokio::test]
	async fn test_propagate_existing_request_id() {
		let config = RequestIdConfig::new();
		let middleware = RequestIdMiddleware::new(config);
		let handler = Arc::new(TestHandler);

		let existing_id = "existing-request-id-123";
		let mut headers = HeaderMap::new();
		headers.insert(REQUEST_ID_HEADER, existing_id.parse().unwrap());

		let request = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(headers)
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();

		// Should propagate the existing request ID
		assert_eq!(
			response.headers.get(REQUEST_ID_HEADER).unwrap(),
			existing_id
		);
	}

	#[tokio::test]
	async fn test_always_generate_new_id() {
		let config = RequestIdConfig::new().always_generate();
		let middleware = RequestIdMiddleware::new(config);
		let handler = Arc::new(TestHandler);

		let existing_id = "existing-request-id-123";
		let mut headers = HeaderMap::new();
		headers.insert(REQUEST_ID_HEADER, existing_id.parse().unwrap());

		let request = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(headers)
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();

		// Should generate a new ID, not use the existing one
		let new_id = response
			.headers
			.get(REQUEST_ID_HEADER)
			.unwrap()
			.to_str()
			.unwrap();
		assert_ne!(new_id, existing_id);
		assert!(Uuid::parse_str(new_id).is_ok());
	}

	#[tokio::test]
	async fn test_custom_header_name() {
		let config = RequestIdConfig::new().with_header("X-Correlation-ID".to_string());
		let middleware = RequestIdMiddleware::new(config);
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

		// Should use custom header name
		assert!(response.headers.contains_key("X-Correlation-ID"));
		assert!(!response.headers.contains_key(REQUEST_ID_HEADER));
	}

	#[tokio::test]
	async fn test_no_generation_if_missing() {
		let config = RequestIdConfig::new().no_generation();
		let middleware = RequestIdMiddleware::new(config);
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

		// Should NOT generate a request ID when missing
		assert!(!response.headers.contains_key(REQUEST_ID_HEADER));
	}

	#[tokio::test]
	async fn test_request_id_in_handler() {
		let config = RequestIdConfig::new();
		let middleware = RequestIdMiddleware::new(config);
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

		// Handler should have received the request ID
		let body_str = std::str::from_utf8(&response.body).unwrap();
		assert_ne!(body_str, "none");
		assert!(Uuid::parse_str(body_str).is_ok());
	}

	#[tokio::test]
	async fn test_multiple_requests_different_ids() {
		let config = RequestIdConfig::new();
		let middleware = Arc::new(RequestIdMiddleware::new(config));
		let handler = Arc::new(TestHandler);

		let mut ids = Vec::new();
		for _ in 0..5 {
			let request = Request::builder()
				.method(Method::GET)
				.uri("/test")
				.version(Version::HTTP_11)
				.headers(HeaderMap::new())
				.body(Bytes::new())
				.build()
				.unwrap();

			let response = middleware.process(request, handler.clone()).await.unwrap();
			let id = response
				.headers
				.get(REQUEST_ID_HEADER)
				.unwrap()
				.to_str()
				.unwrap()
				.to_string();
			ids.push(id);
		}

		// All IDs should be unique
		let unique_ids: std::collections::HashSet<_> = ids.iter().collect();
		assert_eq!(unique_ids.len(), 5);
	}

	#[tokio::test]
	async fn test_empty_request_id_header_generates_new() {
		let config = RequestIdConfig::new();
		let middleware = RequestIdMiddleware::new(config);
		let handler = Arc::new(TestHandler);

		let mut headers = HeaderMap::new();
		headers.insert(REQUEST_ID_HEADER, "".parse().unwrap());

		let request = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(headers)
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();

		// Should generate a new ID when header is empty
		let request_id = response
			.headers
			.get(REQUEST_ID_HEADER)
			.unwrap()
			.to_str()
			.unwrap();
		assert!(!request_id.is_empty());
		assert!(Uuid::parse_str(request_id).is_ok());
	}

	#[tokio::test]
	async fn test_default_middleware() {
		let middleware = RequestIdMiddleware::default();
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

		// Default should generate request IDs
		assert!(response.headers.contains_key(REQUEST_ID_HEADER));
	}
}
