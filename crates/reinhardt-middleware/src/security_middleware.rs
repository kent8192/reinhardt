//! Security Middleware
//!
//! Provides comprehensive security headers and redirects:
//! - HSTS (HTTP Strict Transport Security)
//! - SSL/HTTPS redirects
//! - X-Content-Type-Options
//! - X-Frame-Options
//! - Referrer-Policy
//! - Cross-Origin-Opener-Policy (COOP)

use async_trait::async_trait;
use hyper::StatusCode;
use hyper::header::{HeaderValue, LOCATION};
use reinhardt_conf::Settings;
use reinhardt_http::{Handler, Middleware, Request, Response, Result};
use std::sync::Arc;

/// Security middleware configuration
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct SecurityConfig {
	/// Enable HSTS (HTTP Strict Transport Security)
	pub hsts_enabled: bool,
	/// HSTS max-age in seconds (default: 31536000 = 1 year)
	pub hsts_seconds: u32,
	/// Include subdomains in HSTS
	pub hsts_include_subdomains: bool,
	/// Include preload directive in HSTS
	pub hsts_preload: bool,
	/// Redirect HTTP to HTTPS
	pub ssl_redirect: bool,
	/// Set X-Content-Type-Options: nosniff
	pub content_type_nosniff: bool,
	/// Referrer-Policy value
	pub referrer_policy: Option<String>,
	/// Cross-Origin-Opener-Policy value
	pub cross_origin_opener_policy: Option<String>,
	/// X-Frame-Options value (e.g., "DENY", "SAMEORIGIN")
	pub x_frame_options: Option<String>,
	/// Proxy SSL header name and expected value for identifying secure requests
	/// Example: Some(("HTTP_X_FORWARDED_PROTO".to_string(), "https".to_string()))
	pub secure_proxy_ssl_header: Option<(String, String)>,
}

impl Default for SecurityConfig {
	fn default() -> Self {
		Self {
			hsts_enabled: true,
			hsts_seconds: 31536000, // 1 year
			hsts_include_subdomains: false,
			hsts_preload: false,
			ssl_redirect: false,
			content_type_nosniff: true,
			referrer_policy: Some("same-origin".to_string()),
			cross_origin_opener_policy: None,
			x_frame_options: Some("DENY".to_string()),
			secure_proxy_ssl_header: None,
		}
	}
}

impl From<&Settings> for SecurityConfig {
	fn from(settings: &Settings) -> Self {
		let hsts_enabled = settings.secure_hsts_seconds.is_some();
		let hsts_seconds = settings
			.secure_hsts_seconds
			.map(|s| u32::try_from(s).unwrap_or(u32::MAX))
			.unwrap_or(0);

		Self {
			ssl_redirect: settings.secure_ssl_redirect,
			hsts_enabled,
			hsts_seconds,
			hsts_include_subdomains: settings.secure_hsts_include_subdomains,
			hsts_preload: settings.secure_hsts_preload,
			secure_proxy_ssl_header: settings.secure_proxy_ssl_header.clone(),
			..Self::default()
		}
	}
}

/// Security middleware for HTTP security headers and redirects
pub struct SecurityMiddleware {
	config: SecurityConfig,
}

impl SecurityMiddleware {
	/// Create a new SecurityMiddleware with default configuration
	///
	/// # Examples
	///
	/// ```
	/// use std::sync::Arc;
	/// use reinhardt_middleware::SecurityMiddleware;
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
	/// let middleware = SecurityMiddleware::new();
	/// let handler = Arc::new(TestHandler);
	///
	/// let mut headers = HeaderMap::new();
	/// headers.insert("x-forwarded-proto", "https".parse().unwrap());
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
	/// assert!(response.headers.contains_key("Strict-Transport-Security"));
	/// assert_eq!(response.headers.get("X-Content-Type-Options").unwrap(), "nosniff");
	/// # });
	/// ```
	pub fn new() -> Self {
		Self {
			config: SecurityConfig::default(),
		}
	}
	/// Create a new SecurityMiddleware with custom configuration
	///
	/// # Examples
	///
	/// ```
	/// use std::sync::Arc;
	/// use reinhardt_middleware::{SecurityMiddleware, SecurityConfig};
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
	/// let mut config = SecurityConfig::default();
	/// config.hsts_enabled = true;
	/// config.hsts_seconds = 31536000;
	/// config.hsts_include_subdomains = true;
	/// config.hsts_preload = true;
	/// config.ssl_redirect = false;
	/// config.content_type_nosniff = true;
	/// config.referrer_policy = Some("strict-origin-when-cross-origin".to_string());
	/// config.cross_origin_opener_policy = Some("same-origin".to_string());
	/// config.x_frame_options = Some("DENY".to_string());
	/// config.secure_proxy_ssl_header = None;
	///
	/// let middleware = SecurityMiddleware::with_config(config);
	/// let handler = Arc::new(TestHandler);
	///
	/// let mut headers = HeaderMap::new();
	/// headers.insert("x-forwarded-proto", "https".parse().unwrap());
	///
	/// let request = Request::builder()
	///     .method(Method::GET)
	///     .uri("/secure")
	///     .version(Version::HTTP_11)
	///     .headers(headers)
	///     .body(Bytes::new())
	///     .build()
	///     .unwrap();
	///
	/// let response = middleware.process(request, handler).await.unwrap();
	/// let hsts = response.headers.get("Strict-Transport-Security").unwrap().to_str().unwrap();
	/// assert!(hsts.contains("max-age=31536000"));
	/// assert!(hsts.contains("includeSubDomains"));
	/// assert!(hsts.contains("preload"));
	/// assert_eq!(response.headers.get("Referrer-Policy").unwrap(), "strict-origin-when-cross-origin");
	/// # });
	/// ```
	pub fn with_config(config: SecurityConfig) -> Self {
		Self { config }
	}

	/// Create a new SecurityMiddleware from application `Settings`
	///
	/// Maps security-related fields from `Settings` to `SecurityConfig`.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_conf::Settings;
	/// use reinhardt_middleware::SecurityMiddleware;
	/// use std::path::PathBuf;
	///
	/// let mut settings = Settings::new(PathBuf::from("/app"), "secret".to_string());
	/// settings.secure_ssl_redirect = true;
	/// settings.secure_hsts_seconds = Some(31536000);
	///
	/// let middleware = SecurityMiddleware::from_settings(&settings);
	/// ```
	pub fn from_settings(settings: &Settings) -> Self {
		Self {
			config: SecurityConfig::from(settings),
		}
	}

	/// Check if request is secure (HTTPS)
	fn is_secure(&self, request: &Request) -> bool {
		// Check configured proxy SSL header first
		if let Some((ref header_name, ref header_value)) = self.config.secure_proxy_ssl_header
			&& let Some(val) = request.headers.get(header_name.as_str())
			&& let Ok(val_str) = val.to_str()
		{
			return val_str.eq_ignore_ascii_case(header_value);
		}

		// Check X-Forwarded-Proto header
		if let Some(proto) = request.headers.get("x-forwarded-proto")
			&& let Ok(proto_str) = proto.to_str()
		{
			return proto_str.eq_ignore_ascii_case("https");
		}

		// Check if URI scheme is https
		request.uri.scheme_str() == Some("https")
	}

	/// Build HSTS header value
	fn build_hsts_header(&self) -> String {
		let mut parts = vec![format!("max-age={}", self.config.hsts_seconds)];

		if self.config.hsts_include_subdomains {
			parts.push("includeSubDomains".to_string());
		}

		if self.config.hsts_preload {
			parts.push("preload".to_string());
		}

		parts.join("; ")
	}

	/// Build redirect URL for HTTPS
	fn build_https_url(&self, request: &Request) -> String {
		let host = request
			.headers
			.get(hyper::header::HOST)
			.and_then(|h| h.to_str().ok())
			.unwrap_or("localhost");

		let path_and_query = request
			.uri
			.path_and_query()
			.map(|pq| pq.as_str())
			.unwrap_or("/");

		format!("https://{}{}", host, path_and_query)
	}

	/// Add security headers to response
	fn add_security_headers(&self, response: &mut Response, is_secure: bool) {
		// HSTS header (only on HTTPS)
		if self.config.hsts_enabled && is_secure {
			let hsts_value = self.build_hsts_header();
			response
				.headers
				.insert("Strict-Transport-Security", hsts_value.parse().unwrap());
		}

		// X-Content-Type-Options
		if self.config.content_type_nosniff {
			response.headers.insert(
				"X-Content-Type-Options",
				HeaderValue::from_static("nosniff"),
			);
		}

		// Referrer-Policy
		if let Some(ref policy) = self.config.referrer_policy {
			response
				.headers
				.insert("Referrer-Policy", policy.parse().unwrap());
		}

		// Cross-Origin-Opener-Policy
		if let Some(ref policy) = self.config.cross_origin_opener_policy {
			response
				.headers
				.insert("Cross-Origin-Opener-Policy", policy.parse().unwrap());
		}

		// X-Frame-Options
		if let Some(ref value) = self.config.x_frame_options {
			response
				.headers
				.insert("X-Frame-Options", value.parse().unwrap());
		}
	}
}

impl Default for SecurityMiddleware {
	fn default() -> Self {
		Self::new()
	}
}

#[async_trait]
impl Middleware for SecurityMiddleware {
	async fn process(&self, request: Request, handler: Arc<dyn Handler>) -> Result<Response> {
		let is_secure = self.is_secure(&request);

		// SSL redirect for all HTTP methods
		if self.config.ssl_redirect && !is_secure {
			let redirect_url = self.build_https_url(&request);
			let mut response = Response::new(StatusCode::PERMANENT_REDIRECT);
			response
				.headers
				.insert(LOCATION, redirect_url.parse().unwrap());
			return Ok(response);
		}

		// Call handler
		let mut response = handler.handle(request).await?;

		// Add security headers
		self.add_security_headers(&mut response, is_secure);

		Ok(response)
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
			Ok(Response::new(StatusCode::OK).with_body(Bytes::from("content")))
		}
	}

	#[tokio::test]
	async fn test_hsts_header_on_secure_request() {
		let config = SecurityConfig {
			hsts_enabled: true,
			hsts_seconds: 31536000,
			hsts_include_subdomains: false,
			hsts_preload: false,
			ssl_redirect: false,
			content_type_nosniff: true,
			referrer_policy: None,
			cross_origin_opener_policy: None,
			x_frame_options: None,
			secure_proxy_ssl_header: None,
		};
		let middleware = SecurityMiddleware::with_config(config);
		let handler = Arc::new(TestHandler);

		let mut headers = HeaderMap::new();
		headers.insert("x-forwarded-proto", "https".parse().unwrap());

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
		assert_eq!(
			response.headers.get("Strict-Transport-Security").unwrap(),
			"max-age=31536000"
		);
	}

	#[tokio::test]
	async fn test_security_middleware_hsts_full() {
		let config = SecurityConfig {
			hsts_enabled: true,
			hsts_seconds: 63072000,
			hsts_include_subdomains: true,
			hsts_preload: true,
			ssl_redirect: false,
			content_type_nosniff: true,
			referrer_policy: None,
			cross_origin_opener_policy: None,
			x_frame_options: None,
			secure_proxy_ssl_header: None,
		};
		let middleware = SecurityMiddleware::with_config(config);
		let handler = Arc::new(TestHandler);

		let mut headers = HeaderMap::new();
		headers.insert("x-forwarded-proto", "https".parse().unwrap());

		let request = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(headers)
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();

		let hsts_header = response
			.headers
			.get("Strict-Transport-Security")
			.unwrap()
			.to_str()
			.unwrap();
		assert!(hsts_header.contains("max-age=63072000"));
		assert!(hsts_header.contains("includeSubDomains"));
		assert!(hsts_header.contains("preload"));
	}

	#[tokio::test]
	async fn test_no_hsts_on_insecure_request() {
		let config = SecurityConfig {
			hsts_enabled: true,
			hsts_seconds: 31536000,
			hsts_include_subdomains: false,
			hsts_preload: false,
			ssl_redirect: false,
			content_type_nosniff: true,
			referrer_policy: None,
			cross_origin_opener_policy: None,
			x_frame_options: None,
			secure_proxy_ssl_header: None,
		};
		let middleware = SecurityMiddleware::with_config(config);
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

		assert_eq!(response.status, StatusCode::OK);
		assert!(!response.headers.contains_key("Strict-Transport-Security"));
	}

	#[tokio::test]
	async fn test_ssl_redirect() {
		let config = SecurityConfig {
			hsts_enabled: false,
			hsts_seconds: 0,
			hsts_include_subdomains: false,
			hsts_preload: false,
			ssl_redirect: true,
			content_type_nosniff: false,
			referrer_policy: None,
			cross_origin_opener_policy: None,
			x_frame_options: None,
			secure_proxy_ssl_header: None,
		};
		let middleware = SecurityMiddleware::with_config(config);
		let handler = Arc::new(TestHandler);

		let mut headers = HeaderMap::new();
		headers.insert(hyper::header::HOST, "example.com".parse().unwrap());

		let request = Request::builder()
			.method(Method::GET)
			.uri("/test?key=value")
			.version(Version::HTTP_11)
			.headers(headers)
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();

		assert_eq!(response.status, StatusCode::PERMANENT_REDIRECT);
		assert_eq!(
			response.headers.get(LOCATION).unwrap(),
			"https://example.com/test?key=value"
		);
	}

	#[tokio::test]
	async fn test_ssl_redirect_applies_to_all_methods() {
		let config = SecurityConfig {
			hsts_enabled: false,
			hsts_seconds: 0,
			hsts_include_subdomains: false,
			hsts_preload: false,
			ssl_redirect: true,
			content_type_nosniff: false,
			referrer_policy: None,
			cross_origin_opener_policy: None,
			x_frame_options: None,
			secure_proxy_ssl_header: None,
		};
		let middleware = SecurityMiddleware::with_config(config);

		let mut headers = HeaderMap::new();
		headers.insert(hyper::header::HOST, "example.com".parse().unwrap());

		// POST should also be redirected to HTTPS
		let handler = Arc::new(TestHandler);
		let request = Request::builder()
			.method(Method::POST)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(headers.clone())
			.body(Bytes::new())
			.build()
			.unwrap();
		let response = middleware.process(request, handler).await.unwrap();
		assert_eq!(response.status, StatusCode::PERMANENT_REDIRECT);

		// PUT should also be redirected to HTTPS
		let handler = Arc::new(TestHandler);
		let request = Request::builder()
			.method(Method::PUT)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(headers.clone())
			.body(Bytes::new())
			.build()
			.unwrap();
		let response = middleware.process(request, handler).await.unwrap();
		assert_eq!(response.status, StatusCode::PERMANENT_REDIRECT);

		// DELETE should also be redirected to HTTPS
		let handler = Arc::new(TestHandler);
		let request = Request::builder()
			.method(Method::DELETE)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(headers.clone())
			.body(Bytes::new())
			.build()
			.unwrap();
		let response = middleware.process(request, handler).await.unwrap();
		assert_eq!(response.status, StatusCode::PERMANENT_REDIRECT);
	}

	#[tokio::test]
	async fn test_content_type_nosniff_header() {
		let middleware = SecurityMiddleware::new();
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

		assert_eq!(
			response.headers.get("X-Content-Type-Options").unwrap(),
			"nosniff"
		);
	}

	#[tokio::test]
	async fn test_referrer_policy_header() {
		let config = SecurityConfig {
			hsts_enabled: false,
			hsts_seconds: 0,
			hsts_include_subdomains: false,
			hsts_preload: false,
			ssl_redirect: false,
			content_type_nosniff: false,
			referrer_policy: Some("strict-origin-when-cross-origin".to_string()),
			cross_origin_opener_policy: None,
			x_frame_options: None,
			secure_proxy_ssl_header: None,
		};
		let middleware = SecurityMiddleware::with_config(config);
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

		assert_eq!(
			response.headers.get("Referrer-Policy").unwrap(),
			"strict-origin-when-cross-origin"
		);
	}

	#[tokio::test]
	async fn test_cross_origin_opener_policy_header() {
		let config = SecurityConfig {
			hsts_enabled: false,
			hsts_seconds: 0,
			hsts_include_subdomains: false,
			hsts_preload: false,
			ssl_redirect: false,
			content_type_nosniff: false,
			referrer_policy: None,
			cross_origin_opener_policy: Some("same-origin".to_string()),
			x_frame_options: None,
			secure_proxy_ssl_header: None,
		};
		let middleware = SecurityMiddleware::with_config(config);
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

		assert_eq!(
			response.headers.get("Cross-Origin-Opener-Policy").unwrap(),
			"same-origin"
		);
	}

	#[tokio::test]
	async fn test_all_security_headers_together() {
		let config = SecurityConfig {
			hsts_enabled: true,
			hsts_seconds: 31536000,
			hsts_include_subdomains: true,
			hsts_preload: false,
			ssl_redirect: false,
			content_type_nosniff: true,
			referrer_policy: Some("no-referrer".to_string()),
			cross_origin_opener_policy: Some("same-origin-allow-popups".to_string()),
			x_frame_options: None,
			secure_proxy_ssl_header: None,
		};
		let middleware = SecurityMiddleware::with_config(config);
		let handler = Arc::new(TestHandler);

		let mut headers = HeaderMap::new();
		headers.insert("x-forwarded-proto", "https".parse().unwrap());

		let request = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(headers)
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();

		assert!(response.headers.contains_key("Strict-Transport-Security"));
		assert_eq!(
			response.headers.get("X-Content-Type-Options").unwrap(),
			"nosniff"
		);
		assert_eq!(
			response.headers.get("Referrer-Policy").unwrap(),
			"no-referrer"
		);
		assert_eq!(
			response.headers.get("Cross-Origin-Opener-Policy").unwrap(),
			"same-origin-allow-popups"
		);
	}

	#[tokio::test]
	async fn test_from_settings_conversion() {
		// Arrange
		let mut settings =
			Settings::new(std::path::PathBuf::from("/app"), "test-secret".to_string());
		settings.secure_ssl_redirect = true;
		settings.secure_hsts_seconds = Some(63072000);
		settings.secure_hsts_include_subdomains = true;
		settings.secure_hsts_preload = true;
		settings.secure_proxy_ssl_header =
			Some(("X-Forwarded-Proto".to_string(), "https".to_string()));

		// Act
		let config = SecurityConfig::from(&settings);

		// Assert
		assert_eq!(config.ssl_redirect, true);
		assert_eq!(config.hsts_enabled, true);
		assert_eq!(config.hsts_seconds, 63072000);
		assert_eq!(config.hsts_include_subdomains, true);
		assert_eq!(config.hsts_preload, true);
		assert_eq!(
			config.secure_proxy_ssl_header,
			Some(("X-Forwarded-Proto".to_string(), "https".to_string()))
		);
	}

	#[tokio::test]
	async fn test_from_settings_defaults() {
		// Arrange
		let settings = Settings::default();

		// Act
		let config = SecurityConfig::from(&settings);

		// Assert
		assert_eq!(config.ssl_redirect, false);
		assert_eq!(config.hsts_enabled, false);
		assert_eq!(config.hsts_seconds, 0);
		assert_eq!(config.hsts_include_subdomains, false);
		assert_eq!(config.hsts_preload, false);
		assert_eq!(config.secure_proxy_ssl_header, None);
		// Default values from SecurityConfig::default() are preserved
		assert_eq!(config.content_type_nosniff, true);
		assert_eq!(config.referrer_policy, Some("same-origin".to_string()));
	}

	#[tokio::test]
	async fn test_from_settings_constructor() {
		// Arrange
		let mut settings =
			Settings::new(std::path::PathBuf::from("/app"), "test-secret".to_string());
		settings.secure_ssl_redirect = true;
		settings.secure_hsts_seconds = Some(31536000);

		// Act
		let middleware = SecurityMiddleware::from_settings(&settings);
		let handler = Arc::new(TestHandler);

		let mut headers = HeaderMap::new();
		headers.insert("x-forwarded-proto", "https".parse().unwrap());

		let request = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(headers)
			.body(Bytes::new())
			.build()
			.unwrap();

		// Assert
		let response = middleware.process(request, handler).await.unwrap();
		assert_eq!(response.status, StatusCode::OK);
		assert_eq!(
			response.headers.get("Strict-Transport-Security").unwrap(),
			"max-age=31536000"
		);
	}

	#[tokio::test]
	async fn test_is_secure_with_proxy_ssl_header() {
		// Arrange
		let config = SecurityConfig {
			hsts_enabled: true,
			hsts_seconds: 31536000,
			hsts_include_subdomains: false,
			hsts_preload: false,
			ssl_redirect: false,
			content_type_nosniff: true,
			referrer_policy: None,
			cross_origin_opener_policy: None,
			x_frame_options: None,
			secure_proxy_ssl_header: Some(("X-Custom-Proto".to_string(), "https".to_string())),
		};
		let middleware = SecurityMiddleware::with_config(config);
		let handler = Arc::new(TestHandler);

		let mut headers = HeaderMap::new();
		headers.insert("X-Custom-Proto", "https".parse().unwrap());

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
		assert_eq!(response.status, StatusCode::OK);
		assert_eq!(
			response.headers.get("Strict-Transport-Security").unwrap(),
			"max-age=31536000"
		);
	}

	#[tokio::test]
	async fn test_is_secure_proxy_ssl_header_mismatch() {
		// Arrange
		let config = SecurityConfig {
			hsts_enabled: true,
			hsts_seconds: 31536000,
			hsts_include_subdomains: false,
			hsts_preload: false,
			ssl_redirect: false,
			content_type_nosniff: true,
			referrer_policy: None,
			cross_origin_opener_policy: None,
			x_frame_options: None,
			secure_proxy_ssl_header: Some(("X-Custom-Proto".to_string(), "https".to_string())),
		};
		let middleware = SecurityMiddleware::with_config(config);
		let handler = Arc::new(TestHandler);

		let mut headers = HeaderMap::new();
		// Header present but with wrong value
		headers.insert("X-Custom-Proto", "http".parse().unwrap());

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

		// Assert - HSTS should not be set because request is not secure
		assert_eq!(response.status, StatusCode::OK);
		assert!(!response.headers.contains_key("Strict-Transport-Security"));
	}
}
