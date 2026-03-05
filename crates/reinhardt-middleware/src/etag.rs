//! ETag Middleware
//!
//! Provides automatic ETag generation and validation for responses.
//! Supports conditional requests to reduce bandwidth.

use async_trait::async_trait;
use hyper::StatusCode;
use reinhardt_http::{Handler, Middleware, Request, Response, Result};
use sha2::{Digest, Sha256};
use std::sync::Arc;

/// ETag configuration
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct ETagConfig {
	/// Whether to use weak ETags
	pub use_weak_etag: bool,
	/// Paths to exclude
	pub exclude_paths: Vec<String>,
	/// Methods to exclude
	pub exclude_methods: Vec<String>,
}

impl ETagConfig {
	/// Create a new configuration
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_middleware::etag::ETagConfig;
	///
	/// let config = ETagConfig::new();
	/// assert!(!config.use_weak_etag);
	/// ```
	pub fn new() -> Self {
		Self {
			use_weak_etag: false,
			exclude_paths: Vec::new(),
			exclude_methods: vec!["POST".to_string(), "PUT".to_string(), "PATCH".to_string()],
		}
	}

	/// Use weak ETags
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_middleware::etag::ETagConfig;
	///
	/// let config = ETagConfig::new().with_weak_etag();
	/// assert!(config.use_weak_etag);
	/// ```
	pub fn with_weak_etag(mut self) -> Self {
		self.use_weak_etag = true;
		self
	}

	/// Add paths to exclude
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_middleware::etag::ETagConfig;
	///
	/// let config = ETagConfig::new()
	///     .with_excluded_paths(vec!["/admin".to_string()]);
	/// ```
	pub fn with_excluded_paths(mut self, paths: Vec<String>) -> Self {
		self.exclude_paths.extend(paths);
		self
	}

	/// Set methods to exclude
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_middleware::etag::ETagConfig;
	///
	/// let config = ETagConfig::new()
	///     .with_excluded_methods(vec!["POST".to_string()]);
	/// ```
	pub fn with_excluded_methods(mut self, methods: Vec<String>) -> Self {
		self.exclude_methods = methods;
		self
	}
}

impl Default for ETagConfig {
	fn default() -> Self {
		Self::new()
	}
}

/// ETag Middleware
///
/// # Examples
///
/// ```
/// use std::sync::Arc;
/// use reinhardt_middleware::etag::{ETagMiddleware, ETagConfig};
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
/// let config = ETagConfig::new();
/// let middleware = ETagMiddleware::new(config);
/// let handler = Arc::new(TestHandler);
///
/// let request = Request::builder()
///     .method(Method::GET)
///     .uri("/api/data")
///     .version(Version::HTTP_11)
///     .headers(HeaderMap::new())
///     .body(Bytes::new())
///     .build()
///     .unwrap();
///
/// let response = middleware.process(request, handler).await.unwrap();
/// assert_eq!(response.status, StatusCode::OK);
/// assert!(response.headers.contains_key("etag"));
/// # });
/// ```
pub struct ETagMiddleware {
	config: ETagConfig,
}

impl ETagMiddleware {
	/// Create a new ETag middleware
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_middleware::etag::{ETagMiddleware, ETagConfig};
	///
	/// let config = ETagConfig::new();
	/// let middleware = ETagMiddleware::new(config);
	/// ```
	pub fn new(config: ETagConfig) -> Self {
		Self { config }
	}

	/// Create with default configuration
	pub fn with_defaults() -> Self {
		Self::new(ETagConfig::default())
	}

	/// Check if path should be excluded
	fn should_exclude_path(&self, path: &str) -> bool {
		self.config
			.exclude_paths
			.iter()
			.any(|p| path.starts_with(p))
	}

	/// Check if method should be excluded
	fn should_exclude_method(&self, method: &str) -> bool {
		self.config
			.exclude_methods
			.iter()
			.any(|m| m.eq_ignore_ascii_case(method))
	}

	/// Generate ETag from body
	fn generate_etag(&self, body: &[u8]) -> String {
		let mut hasher = Sha256::new();
		hasher.update(body);
		let result = hasher.finalize();
		let hash = hex::encode(result);

		// Use first 16 characters (shortened version)
		let short_hash = &hash[..16];

		if self.config.use_weak_etag {
			format!("W/\"{}\"", short_hash)
		} else {
			format!("\"{}\"", short_hash)
		}
	}
}

impl Default for ETagMiddleware {
	fn default() -> Self {
		Self::with_defaults()
	}
}

#[async_trait]
impl Middleware for ETagMiddleware {
	async fn process(&self, request: Request, handler: Arc<dyn Handler>) -> Result<Response> {
		let path = request.uri.path().to_string();
		let method = request.method.as_str().to_string();

		// Skip excluded paths or methods
		if self.should_exclude_path(&path) || self.should_exclude_method(&method) {
			return handler.handle(request).await;
		}

		// Extract If-None-Match and If-Match headers before moving request
		let if_none_match = request
			.headers
			.get(hyper::header::IF_NONE_MATCH)
			.and_then(|v| v.to_str().ok())
			.map(|s| s.to_string());

		let if_match = request
			.headers
			.get(hyper::header::IF_MATCH)
			.and_then(|v| v.to_str().ok())
			.map(|s| s.to_string());

		// Call handler
		let response = handler.handle(request).await?;

		// Generate ETag
		let etag = self.generate_etag(&response.body);

		// Check If-None-Match header (for GET/HEAD requests)
		if (method == "GET" || method == "HEAD")
			&& if_none_match.as_ref().is_some_and(|inm| {
				inm.split(',')
					.any(|tag| tag.trim().trim_matches('"') == etag.trim_matches('"'))
			}) {
			// Return 304 Not Modified
			let mut not_modified = Response::new(StatusCode::NOT_MODIFIED);
			not_modified.headers.insert(
				hyper::header::ETAG,
				hyper::header::HeaderValue::from_str(&etag)
					.unwrap_or_else(|_| hyper::header::HeaderValue::from_static("\"\"")),
			);
			return Ok(not_modified);
		}

		// Check If-Match header (for PUT/PATCH/DELETE requests)
		if (method == "PUT" || method == "PATCH" || method == "DELETE")
			&& if_match
				.as_ref()
				.is_some_and(|im| !im.contains(&etag) && im != "*")
		{
			// Return 412 Precondition Failed
			return Ok(Response::new(StatusCode::PRECONDITION_FAILED)
				.with_body(b"Precondition Failed".to_vec()));
		}

		// Add ETag header to response
		let mut response = response;
		response.headers.insert(
			hyper::header::ETAG,
			hyper::header::HeaderValue::from_str(&etag)
				.unwrap_or_else(|_| hyper::header::HeaderValue::from_static("\"\"")),
		);

		Ok(response)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use bytes::Bytes;
	use hyper::{HeaderMap, Method, StatusCode, Version};

	struct TestHandler {
		body: Vec<u8>,
	}

	impl TestHandler {
		fn new(body: Vec<u8>) -> Self {
			Self { body }
		}
	}

	#[async_trait]
	impl Handler for TestHandler {
		async fn handle(&self, _request: Request) -> Result<Response> {
			Ok(Response::new(StatusCode::OK).with_body(self.body.clone()))
		}
	}

	#[tokio::test]
	async fn test_etag_generation() {
		let config = ETagConfig::new();
		let middleware = ETagMiddleware::new(config);
		let handler = Arc::new(TestHandler::new(b"test body".to_vec()));

		let request = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();

		assert_eq!(response.status, StatusCode::OK);
		assert!(response.headers.contains_key("etag"));

		let etag = response.headers.get("etag").unwrap().to_str().unwrap();
		assert!(etag.starts_with('"'));
		assert!(etag.ends_with('"'));
	}

	#[tokio::test]
	async fn test_weak_etag() {
		let config = ETagConfig::new().with_weak_etag();
		let middleware = ETagMiddleware::new(config);
		let handler = Arc::new(TestHandler::new(b"test body".to_vec()));

		let request = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();

		let etag = response.headers.get("etag").unwrap().to_str().unwrap();
		assert!(etag.starts_with("W/"));
	}

	#[tokio::test]
	async fn test_if_none_match_hit() {
		let config = ETagConfig::new();
		let middleware = Arc::new(ETagMiddleware::new(config));
		let handler = Arc::new(TestHandler::new(b"test body".to_vec()));

		// Get ETag from first request
		let request1 = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();
		let response1 = middleware.process(request1, handler.clone()).await.unwrap();
		let etag = response1.headers.get("etag").unwrap().clone();

		// Second request with If-None-Match header
		let mut headers = HeaderMap::new();
		headers.insert(hyper::header::IF_NONE_MATCH, etag);
		let request2 = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(headers)
			.body(Bytes::new())
			.build()
			.unwrap();
		let response2 = middleware.process(request2, handler).await.unwrap();

		assert_eq!(response2.status, StatusCode::NOT_MODIFIED);
		assert!(response2.headers.contains_key("etag"));
		assert!(response2.body.is_empty());
	}

	#[tokio::test]
	async fn test_if_none_match_miss() {
		let config = ETagConfig::new();
		let middleware = ETagMiddleware::new(config);
		let handler = Arc::new(TestHandler::new(b"test body".to_vec()));

		// Request with different ETag
		let mut headers = HeaderMap::new();
		headers.insert(
			hyper::header::IF_NONE_MATCH,
			hyper::header::HeaderValue::from_static("\"different-etag\""),
		);
		let request = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(headers)
			.body(Bytes::new())
			.build()
			.unwrap();
		let response = middleware.process(request, handler).await.unwrap();

		assert_eq!(response.status, StatusCode::OK);
		assert!(response.headers.contains_key("etag"));
		assert!(!response.body.is_empty());
	}

	#[tokio::test]
	async fn test_if_match_success() {
		let config = ETagConfig::new();
		let middleware = Arc::new(ETagMiddleware::new(config));
		let handler = Arc::new(TestHandler::new(b"test body".to_vec()));

		// Get ETag from first request
		let request1 = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();
		let response1 = middleware.process(request1, handler.clone()).await.unwrap();
		let etag = response1.headers.get("etag").unwrap().clone();

		// PUT request with If-Match header
		let mut headers = HeaderMap::new();
		headers.insert(hyper::header::IF_MATCH, etag);
		let request2 = Request::builder()
			.method(Method::PUT)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(headers)
			.body(Bytes::new())
			.build()
			.unwrap();
		let response2 = middleware.process(request2, handler).await.unwrap();

		// PUT method is excluded, so ETag check is skipped
		assert_eq!(response2.status, StatusCode::OK);
	}

	#[tokio::test]
	async fn test_exclude_paths() {
		let config = ETagConfig::new().with_excluded_paths(vec!["/admin".to_string()]);
		let middleware = ETagMiddleware::new(config);
		let handler = Arc::new(TestHandler::new(b"test body".to_vec()));

		let request = Request::builder()
			.method(Method::GET)
			.uri("/admin/users")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();

		assert_eq!(response.status, StatusCode::OK);
		assert!(!response.headers.contains_key("etag"));
	}

	#[tokio::test]
	async fn test_exclude_methods() {
		let config = ETagConfig::new().with_excluded_methods(vec!["POST".to_string()]);
		let middleware = ETagMiddleware::new(config);
		let handler = Arc::new(TestHandler::new(b"test body".to_vec()));

		let request = Request::builder()
			.method(Method::POST)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();

		assert_eq!(response.status, StatusCode::OK);
		assert!(!response.headers.contains_key("etag"));
	}

	#[tokio::test]
	async fn test_same_body_same_etag() {
		let config = ETagConfig::new();
		let middleware = Arc::new(ETagMiddleware::new(config));
		let handler = Arc::new(TestHandler::new(b"test body".to_vec()));

		// Two requests with same body
		let request1 = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();
		let response1 = middleware.process(request1, handler.clone()).await.unwrap();
		let etag1 = response1.headers.get("etag").unwrap().to_str().unwrap();

		let request2 = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();
		let response2 = middleware.process(request2, handler).await.unwrap();
		let etag2 = response2.headers.get("etag").unwrap().to_str().unwrap();

		assert_eq!(etag1, etag2);
	}

	#[tokio::test]
	async fn test_different_body_different_etag() {
		let config = ETagConfig::new();
		let middleware = Arc::new(ETagMiddleware::new(config));

		let handler1 = Arc::new(TestHandler::new(b"body1".to_vec()));
		let request1 = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();
		let response1 = middleware
			.process(request1, handler1.clone())
			.await
			.unwrap();
		let etag1 = response1.headers.get("etag").unwrap().to_str().unwrap();

		let handler2 = Arc::new(TestHandler::new(b"body2".to_vec()));
		let request2 = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();
		let response2 = middleware.process(request2, handler2).await.unwrap();
		let etag2 = response2.headers.get("etag").unwrap().to_str().unwrap();

		assert_ne!(etag1, etag2);
	}
}
