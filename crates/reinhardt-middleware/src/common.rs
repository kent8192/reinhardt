//! Common middleware utilities
//!
//! Provides URL normalization and common request processing patterns.

use async_trait::async_trait;
use hyper::StatusCode;
use hyper::header::HOST;
use reinhardt_http::{Handler, Middleware, Request, Response, Result};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Common middleware configuration
#[non_exhaustive]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommonConfig {
	/// Append trailing slash to URLs that don't have one (except URLs with file extensions)
	pub append_slash: bool,
	/// Prepend 'www.' to the domain if not present
	pub prepend_www: bool,
}

impl CommonConfig {
	/// Create a new CommonConfig with default settings
	///
	/// Default configuration:
	/// - `append_slash`: true - Adds trailing slashes to URLs
	/// - `prepend_www`: false - Does not add www prefix
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_middleware::common::CommonConfig;
	///
	/// let config = CommonConfig::new();
	/// assert!(config.append_slash);
	/// assert!(!config.prepend_www);
	/// ```
	pub fn new() -> Self {
		Self {
			append_slash: true,
			prepend_www: false,
		}
	}
}

impl Default for CommonConfig {
	fn default() -> Self {
		Self::new()
	}
}

/// Common middleware for URL normalization
///
/// Handles common URL transformations:
/// - Appending trailing slashes to URLs
/// - Prepending 'www.' to domain names
///
/// # Examples
///
/// ```
/// use std::sync::Arc;
/// use reinhardt_middleware::{CommonMiddleware, CommonConfig};
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
/// let mut config = CommonConfig::new();
/// config.append_slash = true;
/// config.prepend_www = false;
///
/// let middleware = CommonMiddleware::with_config(config);
/// let handler = Arc::new(TestHandler);
///
/// let request = Request::builder()
///     .method(Method::GET)
///     .uri("/path/to/page")
///     .version(Version::HTTP_11)
///     .headers(HeaderMap::new())
///     .body(Bytes::new())
///     .build()
///     .unwrap();
///
/// let response = middleware.process(request, handler).await.unwrap();
/// // URL without trailing slash redirects to /path/to/page/
/// assert_eq!(response.status, StatusCode::MOVED_PERMANENTLY);
/// # });
/// ```
pub struct CommonMiddleware {
	config: CommonConfig,
}

impl CommonMiddleware {
	/// Create a new CommonMiddleware with default configuration
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_middleware::CommonMiddleware;
	///
	/// let middleware = CommonMiddleware::new();
	/// ```
	pub fn new() -> Self {
		Self {
			config: CommonConfig::default(),
		}
	}

	/// Create a new CommonMiddleware with custom configuration
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_middleware::{CommonMiddleware, CommonConfig};
	///
	/// let mut config = CommonConfig::new();
	/// config.append_slash = true;
	/// config.prepend_www = true;
	///
	/// let middleware = CommonMiddleware::with_config(config);
	/// ```
	pub fn with_config(config: CommonConfig) -> Self {
		Self { config }
	}

	/// Check if the URL path should have a trailing slash appended
	fn should_append_slash(&self, path: &str) -> bool {
		if !self.config.append_slash {
			return false;
		}

		// Already ends with slash
		if path.ends_with('/') {
			return false;
		}

		// Check if path looks like a file (has extension)
		if let Some(last_segment) = path.rsplit('/').next()
			&& last_segment.contains('.')
		{
			return false;
		}

		true
	}

	/// Check if the host should have www prepended
	fn should_prepend_www(&self, host: &str) -> bool {
		if !self.config.prepend_www {
			return false;
		}

		// Already has www
		if host.starts_with("www.") {
			return false;
		}

		// Localhost and IPs should not get www
		if host.starts_with("localhost") || host.starts_with("127.") || host.starts_with("192.168.")
		{
			return false;
		}

		true
	}

	/// Build the redirect URL
	fn build_redirect_url(&self, request: &Request) -> Option<String> {
		let path = request.uri.path();
		let query = request.uri.query();

		let host = request
			.headers
			.get(HOST)
			.and_then(|h| h.to_str().ok())
			.unwrap_or("localhost");

		let mut redirect_needed = false;
		let mut new_path = path.to_string();
		let mut new_host = host.to_string();

		// Check if we need to append slash
		if self.should_append_slash(path) {
			new_path.push('/');
			redirect_needed = true;
		}

		// Check if we need to prepend www
		if self.should_prepend_www(host) {
			new_host = format!("www.{}", host);
			redirect_needed = true;
		}

		if !redirect_needed {
			return None;
		}

		// Build the full URL
		let scheme = if request.headers.contains_key("X-Forwarded-Proto") {
			request
				.headers
				.get("X-Forwarded-Proto")
				.and_then(|h| h.to_str().ok())
				.unwrap_or("http")
		} else {
			"http"
		};

		let url = if let Some(q) = query {
			format!("{}://{}{}?{}", scheme, new_host, new_path, q)
		} else {
			format!("{}://{}{}", scheme, new_host, new_path)
		};

		Some(url)
	}
}

impl Default for CommonMiddleware {
	fn default() -> Self {
		Self::new()
	}
}

#[async_trait]
impl Middleware for CommonMiddleware {
	async fn process(&self, request: Request, handler: Arc<dyn Handler>) -> Result<Response> {
		// Check if we need to redirect
		if let Some(redirect_url) = self.build_redirect_url(&request) {
			let mut response = Response::new(StatusCode::MOVED_PERMANENTLY);
			response
				.headers
				.insert(hyper::header::LOCATION, redirect_url.parse().unwrap());
			return Ok(response);
		}

		// No redirect needed, proceed with the handler
		handler.handle(request).await
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use bytes::Bytes;
	use hyper::{HeaderMap, Method, Version};

	struct TestHandler;

	#[async_trait]
	impl Handler for TestHandler {
		async fn handle(&self, _request: Request) -> Result<Response> {
			Ok(Response::new(StatusCode::OK).with_body("test response".as_bytes()))
		}
	}

	#[tokio::test]
	async fn test_append_slash_redirects() {
		let config = CommonConfig {
			append_slash: true,
			prepend_www: false,
		};
		let middleware = CommonMiddleware::with_config(config);
		let handler = Arc::new(TestHandler);

		let request = Request::builder()
			.method(Method::GET)
			.uri("/path/to/page")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();

		assert_eq!(response.status, StatusCode::MOVED_PERMANENTLY);
		let location = response.headers.get(hyper::header::LOCATION).unwrap();
		assert!(location.to_str().unwrap().contains("/path/to/page/"));
	}

	#[tokio::test]
	async fn test_no_redirect_with_trailing_slash() {
		let config = CommonConfig {
			append_slash: true,
			prepend_www: false,
		};
		let middleware = CommonMiddleware::with_config(config);
		let handler = Arc::new(TestHandler);

		let request = Request::builder()
			.method(Method::GET)
			.uri("/path/to/page/")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();

		assert_eq!(response.status, StatusCode::OK);
	}

	#[tokio::test]
	async fn test_no_redirect_for_file_extensions() {
		let config = CommonConfig {
			append_slash: true,
			prepend_www: false,
		};
		let middleware = CommonMiddleware::with_config(config);
		let handler = Arc::new(TestHandler);

		let request = Request::builder()
			.method(Method::GET)
			.uri("/static/file.css")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();

		assert_eq!(response.status, StatusCode::OK);
	}

	#[tokio::test]
	async fn test_append_slash_with_query_params() {
		let config = CommonConfig {
			append_slash: true,
			prepend_www: false,
		};
		let middleware = CommonMiddleware::with_config(config);
		let handler = Arc::new(TestHandler);

		let request = Request::builder()
			.method(Method::GET)
			.uri("/search?q=test")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();

		assert_eq!(response.status, StatusCode::MOVED_PERMANENTLY);
		let location = response.headers.get(hyper::header::LOCATION).unwrap();
		let loc_str = location.to_str().unwrap();
		assert!(loc_str.contains("/search/"));
		assert!(loc_str.contains("?q=test"));
	}

	#[tokio::test]
	async fn test_prepend_www() {
		let config = CommonConfig {
			append_slash: false,
			prepend_www: true,
		};
		let middleware = CommonMiddleware::with_config(config);
		let handler = Arc::new(TestHandler);

		let mut headers = HeaderMap::new();
		headers.insert(HOST, "example.com".parse().unwrap());

		let request = Request::builder()
			.method(Method::GET)
			.uri("/page/")
			.version(Version::HTTP_11)
			.headers(headers)
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();

		assert_eq!(response.status, StatusCode::MOVED_PERMANENTLY);
		let location = response.headers.get(hyper::header::LOCATION).unwrap();
		assert!(location.to_str().unwrap().contains("www.example.com"));
	}

	#[tokio::test]
	async fn test_no_prepend_www_for_localhost() {
		let config = CommonConfig {
			append_slash: false,
			prepend_www: true,
		};
		let middleware = CommonMiddleware::with_config(config);
		let handler = Arc::new(TestHandler);

		let mut headers = HeaderMap::new();
		headers.insert(HOST, "localhost:8000".parse().unwrap());

		let request = Request::builder()
			.method(Method::GET)
			.uri("/page/")
			.version(Version::HTTP_11)
			.headers(headers)
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();

		assert_eq!(response.status, StatusCode::OK);
	}

	#[tokio::test]
	async fn test_no_prepend_www_when_already_present() {
		let config = CommonConfig {
			append_slash: false,
			prepend_www: true,
		};
		let middleware = CommonMiddleware::with_config(config);
		let handler = Arc::new(TestHandler);

		let mut headers = HeaderMap::new();
		headers.insert(HOST, "www.example.com".parse().unwrap());

		let request = Request::builder()
			.method(Method::GET)
			.uri("/page/")
			.version(Version::HTTP_11)
			.headers(headers)
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();

		assert_eq!(response.status, StatusCode::OK);
	}

	#[tokio::test]
	async fn test_both_transformations() {
		let config = CommonConfig {
			append_slash: true,
			prepend_www: true,
		};
		let middleware = CommonMiddleware::with_config(config);
		let handler = Arc::new(TestHandler);

		let mut headers = HeaderMap::new();
		headers.insert(HOST, "example.com".parse().unwrap());

		let request = Request::builder()
			.method(Method::GET)
			.uri("/page")
			.version(Version::HTTP_11)
			.headers(headers)
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();

		assert_eq!(response.status, StatusCode::MOVED_PERMANENTLY);
		let location = response.headers.get(hyper::header::LOCATION).unwrap();
		let loc_str = location.to_str().unwrap();
		assert!(loc_str.contains("www.example.com"));
		assert!(loc_str.contains("/page/"));
	}

	#[tokio::test]
	async fn test_both_disabled() {
		let config = CommonConfig {
			append_slash: false,
			prepend_www: false,
		};
		let middleware = CommonMiddleware::with_config(config);
		let handler = Arc::new(TestHandler);

		let mut headers = HeaderMap::new();
		headers.insert(HOST, "example.com".parse().unwrap());

		let request = Request::builder()
			.method(Method::GET)
			.uri("/page")
			.version(Version::HTTP_11)
			.headers(headers)
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();

		assert_eq!(response.status, StatusCode::OK);
	}
}
