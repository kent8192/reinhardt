//! GZip Compression Middleware
//!
//! Compresses response content using gzip encoding when the client supports it.

use async_trait::async_trait;
use bytes::Bytes;
use flate2::Compression;
use flate2::write::GzEncoder;
use hyper::header::{ACCEPT_ENCODING, CONTENT_ENCODING, CONTENT_LENGTH, CONTENT_TYPE};
use reinhardt_http::{Handler, Middleware, Request, Response, Result};
use std::io::Write;
use std::sync::Arc;

/// GZip compression middleware configuration
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct GZipConfig {
	/// Minimum response size to compress (in bytes)
	pub min_length: usize,
	/// Compression level (0-9, where 9 is maximum compression)
	pub compression_level: u32,
	/// Content types that should be compressed
	pub compressible_types: Vec<String>,
}

impl Default for GZipConfig {
	fn default() -> Self {
		Self {
			min_length: 200,
			compression_level: 6,
			compressible_types: vec![
				"text/".to_string(),
				"application/json".to_string(),
				"application/javascript".to_string(),
				"application/xml".to_string(),
				"application/xhtml+xml".to_string(),
			],
		}
	}
}

/// GZip compression middleware
pub struct GZipMiddleware {
	config: GZipConfig,
}

impl GZipMiddleware {
	/// Create a new GZipMiddleware with default configuration
	///
	/// Default configuration compresses responses larger than 200 bytes using compression level 6.
	///
	/// # Examples
	///
	/// ```
	/// use std::sync::Arc;
	/// use reinhardt_middleware::GZipMiddleware;
	/// use reinhardt_http::{Handler, Middleware, Request, Response};
	/// use hyper::{StatusCode, Method, Version, HeaderMap};
	/// use bytes::Bytes;
	///
	/// struct TestHandler;
	///
	/// #[async_trait::async_trait]
	/// impl Handler for TestHandler {
	///     async fn handle(&self, _request: Request) -> reinhardt_core::exception::Result<Response> {
	///         let body = "This is a long response body that will be compressed by gzip middleware. ".repeat(10);
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
	/// let middleware = GZipMiddleware::new();
	/// let handler = Arc::new(TestHandler);
	///
	/// let mut headers = HeaderMap::new();
	/// headers.insert(hyper::header::ACCEPT_ENCODING, "gzip, deflate".parse().unwrap());
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
	/// assert_eq!(response.headers.get(hyper::header::CONTENT_ENCODING).unwrap(), "gzip");
	/// # });
	/// ```
	pub fn new() -> Self {
		Self {
			config: GZipConfig::default(),
		}
	}
	/// Create a new GZipMiddleware with custom configuration
	///
	/// # Arguments
	///
	/// * `config` - Custom GZip configuration
	///
	/// # Examples
	///
	/// ```
	/// use std::sync::Arc;
	/// use reinhardt_middleware::{GZipMiddleware, GZipConfig};
	/// use reinhardt_http::{Handler, Middleware, Request, Response};
	/// use hyper::{StatusCode, Method, Version, HeaderMap};
	/// use bytes::Bytes;
	///
	/// struct TestHandler;
	///
	/// #[async_trait::async_trait]
	/// impl Handler for TestHandler {
	///     async fn handle(&self, _request: Request) -> reinhardt_core::exception::Result<Response> {
	///         let body = "Small response";
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
	/// let mut config = GZipConfig::default();
	/// config.min_length = 1000;
	/// config.compression_level = 9;
	/// config.compressible_types = vec!["text/".to_string(), "application/json".to_string()];
	///
	/// let middleware = GZipMiddleware::with_config(config);
	/// let handler = Arc::new(TestHandler);
	///
	/// let mut headers = HeaderMap::new();
	/// headers.insert(hyper::header::ACCEPT_ENCODING, "gzip".parse().unwrap());
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
	/// // Small response not compressed due to min_length=1000
	/// assert!(!response.headers.contains_key(hyper::header::CONTENT_ENCODING));
	/// # });
	/// ```
	pub fn with_config(config: GZipConfig) -> Self {
		Self { config }
	}

	/// Check if the client accepts gzip encoding
	fn accepts_gzip(&self, request: &Request) -> bool {
		if let Some(accept_encoding) = request.headers.get(ACCEPT_ENCODING)
			&& let Ok(encoding_str) = accept_encoding.to_str()
		{
			return encoding_str.contains("gzip");
		}
		false
	}

	/// Check if the content type should be compressed
	fn should_compress(&self, content_type: &str, body_len: usize) -> bool {
		if body_len < self.config.min_length {
			return false;
		}

		self.config
			.compressible_types
			.iter()
			.any(|ct| content_type.starts_with(ct))
	}

	/// Compress the response body using gzip
	fn compress_body(&self, body: &[u8]) -> Result<Vec<u8>> {
		let mut encoder =
			GzEncoder::new(Vec::new(), Compression::new(self.config.compression_level));
		encoder
			.write_all(body)
			.map_err(|e| reinhardt_core::exception::Error::Internal(e.to_string()))?;
		encoder
			.finish()
			.map_err(|e| reinhardt_core::exception::Error::Internal(e.to_string()))
	}
}

impl Default for GZipMiddleware {
	fn default() -> Self {
		Self::new()
	}
}

#[async_trait]
impl Middleware for GZipMiddleware {
	async fn process(&self, request: Request, handler: Arc<dyn Handler>) -> Result<Response> {
		// Check if client accepts gzip
		let accepts_gzip = self.accepts_gzip(&request);

		// Call the next handler
		let mut response = handler.handle(request).await?;

		// Only compress if client accepts gzip and response is not already compressed
		if !accepts_gzip || response.headers.contains_key(CONTENT_ENCODING) {
			return Ok(response);
		}

		// Check content type
		let content_type = response
			.headers
			.get(CONTENT_TYPE)
			.and_then(|ct| ct.to_str().ok())
			.unwrap_or("");

		let body_len = response.body.len();

		// Check if we should compress this response
		if !self.should_compress(content_type, body_len) {
			return Ok(response);
		}

		// Compress the body
		let compressed = self.compress_body(&response.body)?;

		// Only use compressed version if it's actually smaller
		if compressed.len() < body_len {
			response.body = Bytes::from(compressed);
			response
				.headers
				.insert(CONTENT_ENCODING, "gzip".parse().unwrap());
			response.headers.insert(
				CONTENT_LENGTH,
				response.body.len().to_string().parse().unwrap(),
			);
		}

		Ok(response)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use hyper::{HeaderMap, Method, StatusCode, Version};
	use reinhardt_http::Response;

	struct TestHandler {
		response_body: &'static str,
		content_type: &'static str,
	}

	#[async_trait]
	impl Handler for TestHandler {
		async fn handle(&self, _request: Request) -> Result<Response> {
			let mut response =
				Response::new(StatusCode::OK).with_body(self.response_body.as_bytes());
			response
				.headers
				.insert(CONTENT_TYPE, self.content_type.parse().unwrap());
			Ok(response)
		}
	}

	#[tokio::test]
	async fn test_gzip_compression() {
		let middleware = GZipMiddleware::new();
		// Make sure the content is long enough to be compressed (> 200 bytes by default)
		let long_content = "This is a test response that should be compressed because it's long enough and is text/html content type. ".repeat(5);
		let handler = Arc::new(TestHandler {
			response_body: Box::leak(long_content.into_boxed_str()),
			content_type: "text/html",
		});

		let mut headers = HeaderMap::new();
		headers.insert(ACCEPT_ENCODING, "gzip, deflate".parse().unwrap());

		let request = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(headers)
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();

		assert_eq!(response.headers.get(CONTENT_ENCODING).unwrap(), "gzip");
		assert!(!response.body.is_empty()); // Has compressed content
	}

	#[tokio::test]
	async fn test_no_gzip_if_client_doesnt_accept() {
		let middleware = GZipMiddleware::new();
		let body = "This is a test response";
		let handler = Arc::new(TestHandler {
			response_body: body,
			content_type: "text/html",
		});

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
		assert_eq!(response.body, Bytes::from(body));
	}

	#[tokio::test]
	async fn test_no_gzip_for_small_response() {
		let config = GZipConfig {
			min_length: 1000,
			..Default::default()
		};
		let middleware = GZipMiddleware::with_config(config);
		let handler = Arc::new(TestHandler {
			response_body: "Small response",
			content_type: "text/html",
		});

		let mut headers = HeaderMap::new();
		headers.insert(ACCEPT_ENCODING, "gzip".parse().unwrap());

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
	async fn test_no_gzip_for_non_compressible_type() {
		let middleware = GZipMiddleware::new();
		let handler = Arc::new(TestHandler {
			response_body: "This is a long response that could be compressed but is an image",
			content_type: "image/png",
		});

		let mut headers = HeaderMap::new();
		headers.insert(ACCEPT_ENCODING, "gzip".parse().unwrap());

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
	async fn test_gzip_non_200_response() {
		// Test that compression works for non-200 status codes
		struct NotFoundHandler;

		#[async_trait]
		impl Handler for NotFoundHandler {
			async fn handle(&self, _request: Request) -> Result<Response> {
				let content = "Not found page with enough content to compress".repeat(10);
				let mut response =
					Response::new(StatusCode::NOT_FOUND).with_body(Bytes::from(content));
				response
					.headers
					.insert(CONTENT_TYPE, "text/html".parse().unwrap());
				Ok(response)
			}
		}

		let middleware = GZipMiddleware::new();
		let handler = Arc::new(NotFoundHandler);

		let mut headers = HeaderMap::new();
		headers.insert(ACCEPT_ENCODING, "gzip".parse().unwrap());

		let request = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(headers)
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();

		assert_eq!(response.status, StatusCode::NOT_FOUND);
		assert_eq!(response.headers.get(CONTENT_ENCODING).unwrap(), "gzip");
	}

	#[tokio::test]
	async fn test_no_compress_already_compressed() {
		// Test that already compressed responses are not re-compressed
		struct CompressedHandler;

		#[async_trait]
		impl Handler for CompressedHandler {
			async fn handle(&self, _request: Request) -> Result<Response> {
				let mut response = Response::new(StatusCode::OK)
					.with_body("Already compressed content".as_bytes());
				response
					.headers
					.insert(CONTENT_TYPE, "text/html".parse().unwrap());
				response
					.headers
					.insert(CONTENT_ENCODING, "deflate".parse().unwrap());
				Ok(response)
			}
		}

		let middleware = GZipMiddleware::new();
		let handler = Arc::new(CompressedHandler);

		let mut headers = HeaderMap::new();
		headers.insert(ACCEPT_ENCODING, "gzip".parse().unwrap());

		let request = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(headers)
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();

		// Should keep the original encoding
		assert_eq!(response.headers.get(CONTENT_ENCODING).unwrap(), "deflate");
	}

	#[tokio::test]
	async fn test_gzip_json_content() {
		// Test that JSON content is compressed
		struct JsonHandler;

		#[async_trait]
		impl Handler for JsonHandler {
			async fn handle(&self, _request: Request) -> Result<Response> {
				let json_data = r#"{"key": "value", "data": "This is a JSON response that should be compressed"}"#.repeat(5);
				let mut response = Response::new(StatusCode::OK).with_body(Bytes::from(json_data));
				response
					.headers
					.insert(CONTENT_TYPE, "application/json".parse().unwrap());
				Ok(response)
			}
		}

		let middleware = GZipMiddleware::new();
		let handler = Arc::new(JsonHandler);

		let mut headers = HeaderMap::new();
		headers.insert(ACCEPT_ENCODING, "gzip, deflate".parse().unwrap());

		let request = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(headers)
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();

		assert_eq!(response.headers.get(CONTENT_ENCODING).unwrap(), "gzip");
	}

	#[tokio::test]
	async fn test_content_length_updated() {
		// Test that Content-Length is updated after compression
		struct LongHandler;

		#[async_trait]
		impl Handler for LongHandler {
			async fn handle(&self, _request: Request) -> Result<Response> {
				let content =
					"This is a test response that should be compressed because it's long enough"
						.repeat(3);
				let mut response = Response::new(StatusCode::OK).with_body(Bytes::from(content));
				response
					.headers
					.insert(CONTENT_TYPE, "text/html".parse().unwrap());
				Ok(response)
			}
		}

		let middleware = GZipMiddleware::new();
		let handler = Arc::new(LongHandler);

		let mut headers = HeaderMap::new();
		headers.insert(ACCEPT_ENCODING, "gzip".parse().unwrap());

		let request = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(headers)
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();

		assert_eq!(response.headers.get(CONTENT_ENCODING).unwrap(), "gzip");
		let content_length: usize = response
			.headers
			.get(CONTENT_LENGTH)
			.unwrap()
			.to_str()
			.unwrap()
			.parse()
			.unwrap();
		assert_eq!(content_length, response.body.len());
	}
}
