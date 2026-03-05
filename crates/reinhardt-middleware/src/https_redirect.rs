//! HTTPS Redirect Middleware
//!
//! Automatically redirects HTTP requests to HTTPS.
//! Similar to Django's SECURE_SSL_REDIRECT setting.

use async_trait::async_trait;
use hyper::StatusCode;
use reinhardt_http::{Handler, Middleware, Request, Response, Result};
use std::sync::Arc;

/// Configuration for HTTPS redirect middleware
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct HttpsRedirectConfig {
	/// Enable HTTPS redirect
	pub enabled: bool,
	/// Exempt paths from HTTPS redirect (e.g., health checks)
	pub exempt_paths: Vec<String>,
	/// Redirect status code (301 or 302)
	pub status_code: StatusCode,
	/// Allowed host names for redirect (prevents host header injection).
	/// If empty, all requests without a valid allowed host are rejected with 400 Bad Request.
	pub allowed_hosts: Vec<String>,
}

impl Default for HttpsRedirectConfig {
	fn default() -> Self {
		Self {
			enabled: true,
			exempt_paths: vec![],
			status_code: StatusCode::MOVED_PERMANENTLY, // 301
			allowed_hosts: vec![],
		}
	}
}

/// Middleware to redirect HTTP requests to HTTPS
pub struct HttpsRedirectMiddleware {
	config: HttpsRedirectConfig,
}

impl HttpsRedirectMiddleware {
	/// Create a new HttpsRedirectMiddleware with the given configuration
	///
	/// # Arguments
	///
	/// * `config` - HTTPS redirect configuration
	///
	/// # Examples
	///
	/// ```
	/// use std::sync::Arc;
	/// use reinhardt_middleware::{HttpsRedirectMiddleware, HttpsRedirectConfig};
	/// use reinhardt_http::{Handler, Middleware, Request, Response};
	/// use hyper::{StatusCode, Method, Version, HeaderMap};
	/// use bytes::Bytes;
	///
	/// struct TestHandler;
	///
	/// #[async_trait::async_trait]
	/// impl Handler for TestHandler {
	///     async fn handle(&self, _request: Request) -> reinhardt_core::exception::Result<Response> {
	///         Ok(Response::new(StatusCode::OK))
	///     }
	/// }
	///
	/// # tokio_test::block_on(async {
	/// let mut config = HttpsRedirectConfig::default();
	/// config.enabled = true;
	/// config.exempt_paths = vec!["/health".to_string()];
	/// config.status_code = StatusCode::MOVED_PERMANENTLY;
	/// config.allowed_hosts = vec!["example.com".to_string()];
	///
	/// let middleware = HttpsRedirectMiddleware::new(config);
	/// let handler = Arc::new(TestHandler);
	///
	/// let mut headers = HeaderMap::new();
	/// headers.insert(hyper::header::HOST, "example.com".parse().unwrap());
	///
	/// let request = Request::builder()
	///     .method(Method::GET)
	///     .uri("/api/data")
	///     .version(Version::HTTP_11)
	///     .headers(headers)
	///     .body(Bytes::new())
	///     .build()
	///     .unwrap();
	///
	/// let response = middleware.process(request, handler).await.unwrap();
	/// assert_eq!(response.status, StatusCode::MOVED_PERMANENTLY);
	/// assert_eq!(response.headers.get("Location").unwrap(), "https://example.com/api/data");
	/// # });
	/// ```
	pub fn new(config: HttpsRedirectConfig) -> Self {
		Self { config }
	}
	/// Create with default configuration
	///
	/// Default configuration enables HTTPS redirect with 301 status code and no exempt paths.
	///
	/// # Examples
	///
	/// ```
	/// use std::sync::Arc;
	/// use reinhardt_middleware::{HttpsRedirectConfig, HttpsRedirectMiddleware};
	/// use reinhardt_http::{Handler, Middleware, Request, Response};
	/// use hyper::{StatusCode, Method, Version, HeaderMap};
	/// use bytes::Bytes;
	///
	/// struct TestHandler;
	///
	/// #[async_trait::async_trait]
	/// impl Handler for TestHandler {
	///     async fn handle(&self, _request: Request) -> reinhardt_core::exception::Result<Response> {
	///         Ok(Response::new(StatusCode::OK))
	///     }
	/// }
	///
	/// # tokio_test::block_on(async {
	/// let mut config = HttpsRedirectConfig::default();
	/// config.allowed_hosts = vec!["api.example.com".to_string()];
	/// let middleware = HttpsRedirectMiddleware::new(config);
	/// let handler = Arc::new(TestHandler);
	///
	/// let mut headers = HeaderMap::new();
	/// headers.insert(hyper::header::HOST, "api.example.com".parse().unwrap());
	///
	/// let request = Request::builder()
	///     .method(Method::GET)
	///     .uri("/users?page=1")
	///     .version(Version::HTTP_11)
	///     .headers(headers)
	///     .body(Bytes::new())
	///     .build()
	///     .unwrap();
	///
	/// let response = middleware.process(request, handler).await.unwrap();
	/// assert_eq!(response.status, StatusCode::MOVED_PERMANENTLY);
	/// assert_eq!(response.headers.get("Location").unwrap(), "https://api.example.com/users?page=1");
	/// # });
	/// ```
	pub fn default_config() -> Self {
		Self {
			config: HttpsRedirectConfig::default(),
		}
	}

	/// Check if a path is exempt from HTTPS redirect
	fn is_exempt(&self, path: &str) -> bool {
		self.config
			.exempt_paths
			.iter()
			.any(|exempt| path.starts_with(exempt))
	}

	/// Validate host header against allowed hosts list.
	/// Returns the validated host string if valid, or None if the host is not allowed.
	fn validate_host<'a>(&self, host: Option<&'a str>) -> Option<&'a str> {
		let host = host?;

		// Reject hosts containing path separators or whitespace (injection attempts)
		if host.contains('/') || host.contains('\\') || host.contains(char::is_whitespace) {
			return None;
		}

		// If no allowed hosts configured, reject all (secure by default)
		if self.config.allowed_hosts.is_empty() {
			return None;
		}

		// Strip port for comparison (e.g., "example.com:8080" -> "example.com")
		let host_without_port = host.split(':').next().unwrap_or(host);

		// Check against allowed hosts list
		let is_allowed = self.config.allowed_hosts.iter().any(|allowed| {
			let allowed_lower = allowed.to_lowercase();
			let host_lower = host_without_port.to_lowercase();
			allowed_lower == host_lower
		});

		if is_allowed { Some(host) } else { None }
	}
}

#[async_trait]
impl Middleware for HttpsRedirectMiddleware {
	async fn process(&self, request: Request, handler: Arc<dyn Handler>) -> Result<Response> {
		// If HTTPS redirect is disabled, just pass through
		if !self.config.enabled {
			return handler.handle(request).await;
		}

		// If request is already secure, pass through
		if request.is_secure() {
			return handler.handle(request).await;
		}

		// If path is exempt, pass through
		if self.is_exempt(request.path()) {
			return handler.handle(request).await;
		}

		// Validate host header against allowed hosts to prevent host header injection
		let host_value = request
			.headers
			.get(hyper::header::HOST)
			.and_then(|h| h.to_str().ok());

		let validated_host = match self.validate_host(host_value) {
			Some(host) => host,
			None => {
				// Reject requests with invalid or disallowed host headers
				return Ok(Response::new(StatusCode::BAD_REQUEST));
			}
		};

		// Build HTTPS redirect URL with validated host
		let https_url = format!(
			"https://{}{}",
			validated_host,
			request
				.uri
				.path_and_query()
				.map(|pq| pq.as_str())
				.unwrap_or("/")
		);

		// Return redirect response
		let mut response = Response::new(self.config.status_code);
		response
			.headers
			.insert(hyper::header::LOCATION, https_url.parse().unwrap());
		Ok(response)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use bytes::Bytes;
	use hyper::{HeaderMap, Method, StatusCode, Version};
	use reinhardt_http::Request;
	use rstest::rstest;

	struct TestHandler;

	#[async_trait]
	impl Handler for TestHandler {
		async fn handle(&self, _request: Request) -> Result<Response> {
			Ok(Response::ok().with_body(Bytes::from("test")))
		}
	}

	fn config_with_allowed_hosts(hosts: Vec<&str>) -> HttpsRedirectConfig {
		HttpsRedirectConfig {
			enabled: true,
			exempt_paths: vec![],
			status_code: StatusCode::MOVED_PERMANENTLY,
			allowed_hosts: hosts.into_iter().map(String::from).collect(),
		}
	}

	#[rstest]
	#[tokio::test]
	async fn test_redirect_http_to_https_with_allowed_host() {
		// Arrange
		let config = config_with_allowed_hosts(vec!["example.com"]);
		let middleware = HttpsRedirectMiddleware::new(config);
		let handler = Arc::new(TestHandler);

		let mut headers = HeaderMap::new();
		headers.insert(hyper::header::HOST, "example.com".parse().unwrap());

		let request = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(headers)
			.body(Bytes::new())
			.build()
			.unwrap();

		// Act
		let response = middleware.process(request, handler).await.unwrap();

		// Assert
		assert_eq!(response.status, StatusCode::MOVED_PERMANENTLY);
		assert_eq!(
			response.headers.get("Location").unwrap(),
			"https://example.com/test"
		);
	}

	#[rstest]
	#[tokio::test]
	async fn test_no_redirect_for_https() {
		// Arrange
		let config = config_with_allowed_hosts(vec!["example.com"]);
		let middleware = HttpsRedirectMiddleware::new(config);
		let handler = Arc::new(TestHandler);

		let mut headers = HeaderMap::new();
		headers.insert(hyper::header::HOST, "example.com".parse().unwrap());

		let request = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(headers)
			.body(Bytes::new())
			.secure(true)
			.build()
			.unwrap();

		// Act
		let response = middleware.process(request, handler).await.unwrap();

		// Assert
		assert_eq!(response.status, StatusCode::OK);
	}

	#[rstest]
	#[tokio::test]
	async fn test_exempt_paths() {
		// Arrange
		let config = HttpsRedirectConfig {
			enabled: true,
			exempt_paths: vec!["/health".to_string()],
			status_code: StatusCode::MOVED_PERMANENTLY,
			allowed_hosts: vec!["example.com".to_string()],
		};
		let middleware = HttpsRedirectMiddleware::new(config);
		let handler = Arc::new(TestHandler);

		let mut headers = HeaderMap::new();
		headers.insert(hyper::header::HOST, "example.com".parse().unwrap());

		let request = Request::builder()
			.method(Method::GET)
			.uri("/health")
			.version(Version::HTTP_11)
			.headers(headers)
			.body(Bytes::new())
			.build()
			.unwrap();

		// Act
		let response = middleware.process(request, handler).await.unwrap();

		// Assert - should not redirect exempt paths
		assert_eq!(response.status, StatusCode::OK);
	}

	#[rstest]
	#[tokio::test]
	async fn test_reject_disallowed_host() {
		// Arrange
		let config = config_with_allowed_hosts(vec!["example.com"]);
		let middleware = HttpsRedirectMiddleware::new(config);
		let handler = Arc::new(TestHandler);

		let mut headers = HeaderMap::new();
		headers.insert(hyper::header::HOST, "evil.com".parse().unwrap());

		let request = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(headers)
			.body(Bytes::new())
			.build()
			.unwrap();

		// Act
		let response = middleware.process(request, handler).await.unwrap();

		// Assert - should reject with 400 Bad Request
		assert_eq!(response.status, StatusCode::BAD_REQUEST);
		assert!(response.headers.get("Location").is_none());
	}

	#[rstest]
	#[tokio::test]
	async fn test_reject_host_with_path_separator() {
		// Arrange - host header injection attempt with path separator
		let config = config_with_allowed_hosts(vec!["example.com"]);
		let middleware = HttpsRedirectMiddleware::new(config);
		let handler = Arc::new(TestHandler);

		let mut headers = HeaderMap::new();
		headers.insert(hyper::header::HOST, "evil.com/redirect".parse().unwrap());

		let request = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(headers)
			.body(Bytes::new())
			.build()
			.unwrap();

		// Act
		let response = middleware.process(request, handler).await.unwrap();

		// Assert - should reject host with path separator
		assert_eq!(response.status, StatusCode::BAD_REQUEST);
	}

	#[rstest]
	#[tokio::test]
	async fn test_reject_missing_host_header() {
		// Arrange - no host header at all
		let config = config_with_allowed_hosts(vec!["example.com"]);
		let middleware = HttpsRedirectMiddleware::new(config);
		let handler = Arc::new(TestHandler);

		let request = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.body(Bytes::new())
			.build()
			.unwrap();

		// Act
		let response = middleware.process(request, handler).await.unwrap();

		// Assert - should reject when no host header present
		assert_eq!(response.status, StatusCode::BAD_REQUEST);
	}

	#[rstest]
	#[tokio::test]
	async fn test_reject_empty_allowed_hosts() {
		// Arrange - default config has empty allowed_hosts (secure by default)
		let middleware = HttpsRedirectMiddleware::default_config();
		let handler = Arc::new(TestHandler);

		let mut headers = HeaderMap::new();
		headers.insert(hyper::header::HOST, "example.com".parse().unwrap());

		let request = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(headers)
			.body(Bytes::new())
			.build()
			.unwrap();

		// Act
		let response = middleware.process(request, handler).await.unwrap();

		// Assert - should reject when no allowed hosts configured
		assert_eq!(response.status, StatusCode::BAD_REQUEST);
	}

	#[rstest]
	#[tokio::test]
	async fn test_allowed_host_with_port() {
		// Arrange - host header includes port
		let config = config_with_allowed_hosts(vec!["example.com"]);
		let middleware = HttpsRedirectMiddleware::new(config);
		let handler = Arc::new(TestHandler);

		let mut headers = HeaderMap::new();
		headers.insert(hyper::header::HOST, "example.com:8080".parse().unwrap());

		let request = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(headers)
			.body(Bytes::new())
			.build()
			.unwrap();

		// Act
		let response = middleware.process(request, handler).await.unwrap();

		// Assert - should allow host with port when hostname matches
		assert_eq!(response.status, StatusCode::MOVED_PERMANENTLY);
		assert_eq!(
			response.headers.get("Location").unwrap(),
			"https://example.com:8080/test"
		);
	}

	#[rstest]
	#[tokio::test]
	async fn test_case_insensitive_host_matching() {
		// Arrange
		let config = config_with_allowed_hosts(vec!["Example.COM"]);
		let middleware = HttpsRedirectMiddleware::new(config);
		let handler = Arc::new(TestHandler);

		let mut headers = HeaderMap::new();
		headers.insert(hyper::header::HOST, "example.com".parse().unwrap());

		let request = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(headers)
			.body(Bytes::new())
			.build()
			.unwrap();

		// Act
		let response = middleware.process(request, handler).await.unwrap();

		// Assert - host matching should be case-insensitive
		assert_eq!(response.status, StatusCode::MOVED_PERMANENTLY);
	}
}
