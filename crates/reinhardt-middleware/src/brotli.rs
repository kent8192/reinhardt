//! Brotli Compression Middleware
//!
//! Compresses response content using Brotli encoding when the client supports it.
//! Brotli typically provides better compression ratios than gzip while maintaining
//! similar compression speeds.

use async_trait::async_trait;
use brotli::enc::BrotliEncoderParams;
use bytes::Bytes;
use hyper::header::{ACCEPT_ENCODING, CONTENT_ENCODING, CONTENT_LENGTH, CONTENT_TYPE};
use reinhardt_http::{Handler, Middleware, Request, Response, Result};
use std::sync::Arc;

/// Brotli compression quality level
#[derive(Debug, Clone, Copy)]
pub enum BrotliQuality {
	/// Fastest compression (quality 0-3)
	Fast,
	/// Balanced compression (quality 4-6)
	Balanced,
	/// Best compression (quality 7-11)
	Best,
}

impl BrotliQuality {
	fn to_value(self) -> u32 {
		match self {
			BrotliQuality::Fast => 1,
			BrotliQuality::Balanced => 6,
			BrotliQuality::Best => 11,
		}
	}
}

/// Brotli compression middleware configuration
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct BrotliConfig {
	/// Minimum response size to compress (in bytes)
	pub min_length: usize,
	/// Compression quality level
	pub quality: BrotliQuality,
	/// Content types that should be compressed
	pub compressible_types: Vec<String>,
	/// Window size (10-24, larger = better compression but more memory)
	pub window_size: u32,
}

impl BrotliConfig {
	/// Create a new configuration with custom settings
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_middleware::brotli::{BrotliConfig, BrotliQuality};
	///
	/// let config = BrotliConfig::new()
	///     .with_min_length(500)
	///     .with_quality(BrotliQuality::Best);
	/// ```
	pub fn new() -> Self {
		Self::default()
	}

	/// Set minimum response size
	pub fn with_min_length(mut self, min_length: usize) -> Self {
		self.min_length = min_length;
		self
	}

	/// Set compression quality
	pub fn with_quality(mut self, quality: BrotliQuality) -> Self {
		self.quality = quality;
		self
	}

	/// Set window size
	pub fn with_window_size(mut self, window_size: u32) -> Self {
		self.window_size = window_size.clamp(10, 24);
		self
	}

	/// Add a compressible content type
	pub fn with_compressible_type(mut self, content_type: String) -> Self {
		self.compressible_types.push(content_type);
		self
	}
}

impl Default for BrotliConfig {
	fn default() -> Self {
		Self {
			min_length: 200,
			quality: BrotliQuality::Balanced,
			compressible_types: vec![
				"text/".to_string(),
				"application/json".to_string(),
				"application/javascript".to_string(),
				"application/xml".to_string(),
				"application/xhtml+xml".to_string(),
			],
			window_size: 22, // Default Brotli window size
		}
	}
}

/// Brotli compression middleware
///
/// # Examples
///
/// ```
/// use reinhardt_middleware::brotli::BrotliMiddleware;
/// use std::sync::Arc;
///
/// let middleware = Arc::new(BrotliMiddleware::new());
/// ```
pub struct BrotliMiddleware {
	config: BrotliConfig,
}

impl BrotliMiddleware {
	/// Create a new BrotliMiddleware with default configuration
	///
	/// # Examples
	///
	/// ```
	/// use std::sync::Arc;
	/// use reinhardt_middleware::brotli::BrotliMiddleware;
	/// use reinhardt_http::{Handler, Middleware, Request, Response};
	/// use hyper::{StatusCode, Method, Version, HeaderMap};
	/// use bytes::Bytes;
	///
	/// struct TestHandler;
	///
	/// #[async_trait::async_trait]
	/// impl Handler for TestHandler {
	///     async fn handle(&self, _request: Request) -> reinhardt_core::exception::Result<Response> {
	///         let body = "This is a response body that will be compressed. ".repeat(10);
	///         let mut response = Response::new(StatusCode::OK).with_body(Bytes::from(body));
	///         response.headers.insert(
	///             hyper::header::CONTENT_TYPE,
	///             "text/html".parse().unwrap()
	///         );
	///         Ok(response)
	///     }
	/// }
	///
	/// # tokio_test::block_on(async {
	/// let middleware = BrotliMiddleware::new();
	/// let handler = Arc::new(TestHandler);
	///
	/// let mut headers = HeaderMap::new();
	/// headers.insert(hyper::header::ACCEPT_ENCODING, "br, gzip".parse().unwrap());
	///
	/// let request = Request::builder()
	///     .method(Method::GET)
	///     .uri("/page")
	///     .version(Version::HTTP_11)
	///     .headers(headers)
	///     .body(Bytes::new())
	///     .build()
	///     .unwrap();
	///
	/// let response = middleware.process(request, handler).await.unwrap();
	/// assert_eq!(response.headers.get(hyper::header::CONTENT_ENCODING).unwrap(), "br");
	/// # });
	/// ```
	pub fn new() -> Self {
		Self {
			config: BrotliConfig::default(),
		}
	}

	/// Create a new BrotliMiddleware with custom configuration
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_middleware::brotli::{BrotliMiddleware, BrotliConfig, BrotliQuality};
	///
	/// let config = BrotliConfig::new()
	///     .with_quality(BrotliQuality::Best)
	///     .with_min_length(1000);
	/// let middleware = BrotliMiddleware::with_config(config);
	/// ```
	pub fn with_config(config: BrotliConfig) -> Self {
		Self { config }
	}

	/// Check if the request accepts Brotli encoding
	fn accepts_brotli(&self, request: &Request) -> bool {
		if let Some(accept_encoding) = request.headers.get(ACCEPT_ENCODING)
			&& let Ok(value) = accept_encoding.to_str()
		{
			return value.to_lowercase().contains("br");
		}
		false
	}

	/// Check if the content type is compressible
	fn is_compressible(&self, content_type: &str) -> bool {
		self.config
			.compressible_types
			.iter()
			.any(|ct| content_type.starts_with(ct))
	}

	/// Compress data using Brotli
	fn compress(&self, data: &[u8]) -> std::io::Result<Vec<u8>> {
		let params = BrotliEncoderParams {
			quality: self.config.quality.to_value() as i32,
			lgwin: self.config.window_size as i32,
			..Default::default()
		};

		let mut output = Vec::new();
		let mut reader = std::io::Cursor::new(data);
		brotli::BrotliCompress(&mut reader, &mut output, &params)?;
		Ok(output)
	}
}

impl Default for BrotliMiddleware {
	fn default() -> Self {
		Self::new()
	}
}

#[async_trait]
impl Middleware for BrotliMiddleware {
	async fn process(&self, request: Request, handler: Arc<dyn Handler>) -> Result<Response> {
		// Check if client accepts Brotli
		if !self.accepts_brotli(&request) {
			return handler.handle(request).await;
		}

		// Call handler
		let mut response = handler.handle(request).await?;

		// Don't compress if already compressed
		if response.headers.contains_key(CONTENT_ENCODING) {
			return Ok(response);
		}

		// Check response size
		if response.body.len() < self.config.min_length {
			return Ok(response);
		}

		// Check content type
		let content_type = response
			.headers
			.get(CONTENT_TYPE)
			.and_then(|v| v.to_str().ok())
			.unwrap_or("");

		if !self.is_compressible(content_type) {
			return Ok(response);
		}

		// Compress response
		match self.compress(&response.body) {
			Ok(compressed) => {
				// Only use compression if it actually reduces size
				if compressed.len() < response.body.len() {
					response.body = Bytes::from(compressed);
					response
						.headers
						.insert(CONTENT_ENCODING, "br".parse().unwrap());
					response.headers.remove(CONTENT_LENGTH);
				}
				Ok(response)
			}
			Err(_) => {
				// If compression fails, return original response
				Ok(response)
			}
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use hyper::{HeaderMap, Method, StatusCode, Version};

	struct TestHandler {
		body: String,
		content_type: String,
	}

	impl TestHandler {
		fn new(body: String, content_type: String) -> Self {
			Self { body, content_type }
		}
	}

	#[async_trait]
	impl Handler for TestHandler {
		async fn handle(&self, _request: Request) -> Result<Response> {
			let mut response =
				Response::new(StatusCode::OK).with_body(Bytes::from(self.body.clone()));
			response
				.headers
				.insert(CONTENT_TYPE, self.content_type.parse().unwrap());
			Ok(response)
		}
	}

	#[tokio::test]
	async fn test_brotli_compression_basic() {
		let config = BrotliConfig::default();
		let middleware = BrotliMiddleware::with_config(config);
		let body = "This is a test body that should be compressed. ".repeat(10);
		let handler = Arc::new(TestHandler::new(body.clone(), "text/html".to_string()));

		let mut headers = HeaderMap::new();
		headers.insert(ACCEPT_ENCODING, "br".parse().unwrap());

		let request = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(headers)
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();

		assert_eq!(response.headers.get(CONTENT_ENCODING).unwrap(), "br");
		assert!(response.body.len() < body.len());
	}

	#[tokio::test]
	async fn test_no_compression_without_accept_encoding() {
		let middleware = BrotliMiddleware::new();
		let body = "Test body".repeat(50);
		let handler = Arc::new(TestHandler::new(body.clone(), "text/html".to_string()));

		let request = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();

		assert!(!response.headers.contains_key(CONTENT_ENCODING));
		assert_eq!(response.body.len(), body.len());
	}

	#[tokio::test]
	async fn test_no_compression_for_small_body() {
		let config = BrotliConfig {
			min_length: 1000,
			..Default::default()
		};
		let middleware = BrotliMiddleware::with_config(config);
		let body = "Small body";
		let handler = Arc::new(TestHandler::new(body.to_string(), "text/html".to_string()));

		let mut headers = HeaderMap::new();
		headers.insert(ACCEPT_ENCODING, "br".parse().unwrap());

		let request = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(headers)
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();

		assert!(!response.headers.contains_key(CONTENT_ENCODING));
	}

	#[tokio::test]
	async fn test_no_compression_for_non_text_content() {
		let middleware = BrotliMiddleware::new();
		let body = "Binary data".repeat(50);
		let handler = Arc::new(TestHandler::new(body.clone(), "image/png".to_string()));

		let mut headers = HeaderMap::new();
		headers.insert(ACCEPT_ENCODING, "br".parse().unwrap());

		let request = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(headers)
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();

		assert!(!response.headers.contains_key(CONTENT_ENCODING));
	}

	#[tokio::test]
	async fn test_compression_quality_levels() {
		for quality in &[
			BrotliQuality::Fast,
			BrotliQuality::Balanced,
			BrotliQuality::Best,
		] {
			let config = BrotliConfig {
				quality: *quality,
				..Default::default()
			};
			let middleware = BrotliMiddleware::with_config(config);
			let body = "Test compression quality. ".repeat(20);
			let handler = Arc::new(TestHandler::new(body.clone(), "text/html".to_string()));

			let mut headers = HeaderMap::new();
			headers.insert(ACCEPT_ENCODING, "br".parse().unwrap());

			let request = Request::builder()
				.method(Method::GET)
				.uri("/test")
				.version(Version::HTTP_11)
				.headers(headers)
				.body(Bytes::new())
				.build()
				.unwrap();

			let response = middleware.process(request, handler).await.unwrap();

			assert_eq!(response.headers.get(CONTENT_ENCODING).unwrap(), "br");
			assert!(response.body.len() < body.len());
		}
	}

	#[tokio::test]
	async fn test_json_compression() {
		let middleware = BrotliMiddleware::new();
		let body = r#"{"data": "This is JSON data that should be compressed."}"#.repeat(10);
		let handler = Arc::new(TestHandler::new(
			body.clone(),
			"application/json".to_string(),
		));

		let mut headers = HeaderMap::new();
		headers.insert(ACCEPT_ENCODING, "br, gzip".parse().unwrap());

		let request = Request::builder()
			.method(Method::GET)
			.uri("/api/data")
			.version(Version::HTTP_11)
			.headers(headers)
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();

		assert_eq!(response.headers.get(CONTENT_ENCODING).unwrap(), "br");
		assert!(response.body.len() < body.len());
	}

	#[tokio::test]
	async fn test_javascript_compression() {
		let middleware = BrotliMiddleware::new();
		let body = "function test() { console.log('hello'); }".repeat(10);
		let handler = Arc::new(TestHandler::new(
			body.clone(),
			"application/javascript".to_string(),
		));

		let mut headers = HeaderMap::new();
		headers.insert(ACCEPT_ENCODING, "br".parse().unwrap());

		let request = Request::builder()
			.method(Method::GET)
			.uri("/script.js")
			.version(Version::HTTP_11)
			.headers(headers)
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();

		assert_eq!(response.headers.get(CONTENT_ENCODING).unwrap(), "br");
	}

	#[tokio::test]
	async fn test_custom_compressible_types() {
		let config = BrotliConfig {
			compressible_types: vec!["application/custom".to_string()],
			..Default::default()
		};
		let middleware = BrotliMiddleware::with_config(config);
		let body = "Custom content type data. ".repeat(20);
		let handler = Arc::new(TestHandler::new(
			body.clone(),
			"application/custom".to_string(),
		));

		let mut headers = HeaderMap::new();
		headers.insert(ACCEPT_ENCODING, "br".parse().unwrap());

		let request = Request::builder()
			.method(Method::GET)
			.uri("/custom")
			.version(Version::HTTP_11)
			.headers(headers)
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();

		assert_eq!(response.headers.get(CONTENT_ENCODING).unwrap(), "br");
	}

	#[tokio::test]
	async fn test_window_size_configuration() {
		let config = BrotliConfig {
			window_size: 18,
			..Default::default()
		};
		let middleware = BrotliMiddleware::with_config(config);
		let body = "Test window size. ".repeat(20);
		let handler = Arc::new(TestHandler::new(body.clone(), "text/html".to_string()));

		let mut headers = HeaderMap::new();
		headers.insert(ACCEPT_ENCODING, "br".parse().unwrap());

		let request = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(headers)
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();

		assert_eq!(response.headers.get(CONTENT_ENCODING).unwrap(), "br");
	}
}
