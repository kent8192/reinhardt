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
use reinhardt_conf::SecuritySettings;
#[allow(deprecated)]
use reinhardt_conf::Settings;
use reinhardt_http::{Handler, Middleware, Request, Response, Result};
use std::sync::Arc;

/// Security middleware configuration
///
/// # Deprecation
///
/// This type is deprecated in favor of [`SecuritySettings`] from `reinhardt-conf`.
/// Use [`SecurityMiddleware::from_security_settings`] to construct middleware
/// from a [`SecuritySettings`] fragment.
///
/// For advanced configuration with middleware-specific fields (e.g.,
/// `referrer_policy`, `cross_origin_opener_policy`), use
/// [`SecurityMiddleware::with_config`] which remains available as an escape hatch.
#[deprecated(
	since = "0.2.0",
	note = "use SecuritySettings from reinhardt-conf with SecurityMiddleware::from_security_settings() instead"
)]
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

#[allow(deprecated)]
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

#[allow(deprecated)] // Settings and SecurityConfig are both deprecated
impl From<&Settings> for SecurityConfig {
	fn from(settings: &Settings) -> Self {
		let security = &settings.core.security;
		let hsts_enabled = security.secure_hsts_seconds.is_some();
		let hsts_seconds = security
			.secure_hsts_seconds
			.map(|s| u32::try_from(s).unwrap_or(u32::MAX))
			.unwrap_or(0);

		Self {
			ssl_redirect: security.secure_ssl_redirect,
			hsts_enabled,
			hsts_seconds,
			hsts_include_subdomains: security.secure_hsts_include_subdomains,
			hsts_preload: security.secure_hsts_preload,
			secure_proxy_ssl_header: security.secure_proxy_ssl_header.clone(),
			..Self::default()
		}
	}
}

#[allow(deprecated)] // SecurityConfig is deprecated in favor of SecuritySettings
impl From<&SecuritySettings> for SecurityConfig {
	fn from(settings: &SecuritySettings) -> Self {
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
	#[allow(deprecated)] // SecurityConfig is deprecated; internal field retained
	config: SecurityConfig,
}

#[allow(deprecated)] // SecurityConfig is deprecated; SecurityMiddleware methods still use it internally
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
	/// let request = Request::builder()
	///     .method(Method::GET)
	///     .uri("/api/data")
	///     .version(Version::HTTP_11)
	///     .headers(HeaderMap::new())
	///     .secure(true)
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
	/// let request = Request::builder()
	///     .method(Method::GET)
	///     .uri("/secure")
	///     .version(Version::HTTP_11)
	///     .headers(HeaderMap::new())
	///     .secure(true)
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
	/// #[allow(deprecated)]
	/// let mut settings = Settings::new(PathBuf::from("/app"), "secret".to_string());
	/// settings.core.security.secure_ssl_redirect = true;
	/// settings.core.security.secure_hsts_seconds = Some(31536000);
	///
	/// #[allow(deprecated)]
	/// let middleware = SecurityMiddleware::from_settings(&settings);
	/// ```
	#[deprecated(
		since = "0.2.0",
		note = "use SecurityMiddleware::from_security_settings() instead"
	)]
	#[allow(deprecated)] // Settings is deprecated in favor of composable fragments
	pub fn from_settings(settings: &Settings) -> Self {
		Self {
			config: SecurityConfig::from(settings),
		}
	}

	/// Create a new SecurityMiddleware from a [`SecuritySettings`] fragment
	///
	/// Maps security-related fields from `SecuritySettings` to the internal
	/// configuration. Middleware-specific defaults (e.g., `content_type_nosniff`,
	/// `referrer_policy`) are preserved from [`SecurityConfig::default`].
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_conf::SecuritySettings;
	/// use reinhardt_middleware::SecurityMiddleware;
	///
	/// let settings = SecuritySettings {
	///     secure_ssl_redirect: true,
	///     secure_hsts_seconds: Some(31536000),
	///     ..Default::default()
	/// };
	///
	/// let middleware = SecurityMiddleware::from_security_settings(&settings);
	/// ```
	pub fn from_security_settings(settings: &SecuritySettings) -> Self {
		Self {
			config: SecurityConfig::from(settings),
		}
	}

	/// Check if request is secure (HTTPS)
	///
	/// Delegates to `Request::is_secure()` which already validates trusted proxies
	/// before honoring X-Forwarded-Proto headers. If a custom `secure_proxy_ssl_header`
	/// is configured, it is only trusted when the request comes from a trusted proxy.
	fn is_secure(&self, request: &Request) -> bool {
		// Check configured proxy SSL header (only from trusted proxies)
		if let Some((ref header_name, ref header_value)) = self.config.secure_proxy_ssl_header
			&& let Some(val) = request.headers.get(header_name.as_str())
			&& let Ok(val_str) = val.to_str()
		{
			// Only trust the custom header when request is from a trusted proxy
			if request.is_from_trusted_proxy() {
				return val_str.eq_ignore_ascii_case(header_value);
			}
			// Untrusted source: ignore the spoofable header
		}

		// Delegate to Request::is_secure() which checks:
		// 1. Actual TLS connection (is_secure flag)
		// 2. X-Forwarded-Proto only from trusted proxies
		request.is_secure()
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
			if let Ok(header_value) = hsts_value.parse() {
				response
					.headers
					.insert("Strict-Transport-Security", header_value);
			}
		}

		// X-Content-Type-Options
		if self.config.content_type_nosniff {
			response.headers.insert(
				"X-Content-Type-Options",
				HeaderValue::from_static("nosniff"),
			);
		}

		// Referrer-Policy
		if let Some(ref policy) = self.config.referrer_policy
			&& let Ok(header_value) = policy.parse()
		{
			response.headers.insert("Referrer-Policy", header_value);
		}

		// Cross-Origin-Opener-Policy
		if let Some(ref policy) = self.config.cross_origin_opener_policy
			&& let Ok(header_value) = policy.parse()
		{
			response
				.headers
				.insert("Cross-Origin-Opener-Policy", header_value);
		}

		// X-Frame-Options
		if let Some(ref value) = self.config.x_frame_options
			&& let Ok(header_value) = value.parse()
		{
			response.headers.insert("X-Frame-Options", header_value);
		}
	}
}

impl Default for SecurityMiddleware {
	fn default() -> Self {
		Self::new()
	}
}

#[allow(deprecated)] // SecurityConfig is deprecated; internal usage retained
#[async_trait]
impl Middleware for SecurityMiddleware {
	async fn process(&self, request: Request, handler: Arc<dyn Handler>) -> Result<Response> {
		let is_secure = self.is_secure(&request);

		// SSL redirect for all HTTP methods
		if self.config.ssl_redirect && !is_secure {
			let redirect_url = self.build_https_url(&request);
			let mut response = Response::new(StatusCode::PERMANENT_REDIRECT);
			response.headers.insert(
				LOCATION,
				redirect_url
					.parse()
					.unwrap_or_else(|_| HeaderValue::from_static("/")),
			);
			return Ok(response);
		}

		// Call handler — convert errors to responses so security headers are
		// always applied, even when invoked outside MiddlewareChain.
		let mut response = match handler.handle(request).await {
			Ok(resp) => resp,
			Err(e) => Response::from(e),
		};

		// Add security headers
		self.add_security_headers(&mut response, is_secure);

		Ok(response)
	}
}

#[cfg(test)]
#[allow(deprecated)] // Tests use deprecated SecurityConfig for backward compatibility testing
mod tests {
	use super::*;
	use bytes::Bytes;
	use hyper::{HeaderMap, Method, Version};
	use reinhardt_conf::SecuritySettings;
	use reinhardt_http::{Error, TrustedProxies};
	use rstest::rstest;
	use std::net::{IpAddr, Ipv4Addr, SocketAddr};

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

		// Use secure(true) to indicate actual TLS connection
		let request = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.secure(true)
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

		// Use secure(true) to indicate actual TLS connection
		let request = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.secure(true)
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

		let request = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.secure(true)
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
		#[allow(deprecated)]
		let mut settings = Settings::new(std::path::PathBuf::from("/app"), "test-secret".to_string());
		settings.core.security.secure_ssl_redirect = true;
		settings.core.security.secure_hsts_seconds = Some(63072000);
		settings.core.security.secure_hsts_include_subdomains = true;
		settings.core.security.secure_hsts_preload = true;
		settings.core.security.secure_proxy_ssl_header =
			Some(("X-Forwarded-Proto".to_string(), "https".to_string()));

		// Act
		#[allow(deprecated)]
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
		#[allow(deprecated)]
		let settings = Settings::default();

		// Act
		#[allow(deprecated)]
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
		#[allow(deprecated)]
		let mut settings = Settings::new(std::path::PathBuf::from("/app"), "test-secret".to_string());
		settings.core.security.secure_ssl_redirect = true;
		settings.core.security.secure_hsts_seconds = Some(31536000);

		// Act
		#[allow(deprecated)]
		let middleware = SecurityMiddleware::from_settings(&settings);
		let handler = Arc::new(TestHandler);

		let request = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.secure(true)
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
		// Arrange - custom proxy SSL header from a trusted proxy
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

		let proxy_ip = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));
		let proxy_addr = SocketAddr::new(proxy_ip, 8080);

		let mut headers = HeaderMap::new();
		headers.insert("X-Custom-Proto", "https".parse().unwrap());

		let request = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(headers)
			.remote_addr(proxy_addr)
			.body(Bytes::new())
			.build()
			.unwrap();
		request.set_trusted_proxies(TrustedProxies::new(vec![proxy_ip]));

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
		// Arrange - custom proxy header with wrong value from trusted proxy
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

		let proxy_ip = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));
		let proxy_addr = SocketAddr::new(proxy_ip, 8080);

		let mut headers = HeaderMap::new();
		// Header present but with wrong value
		headers.insert("X-Custom-Proto", "http".parse().unwrap());

		let request = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(headers)
			.remote_addr(proxy_addr)
			.body(Bytes::new())
			.build()
			.unwrap();
		request.set_trusted_proxies(TrustedProxies::new(vec![proxy_ip]));

		// Act
		let response = middleware.process(request, handler).await.unwrap();

		// Assert - HSTS should not be set because request is not secure
		assert_eq!(response.status, StatusCode::OK);
		assert!(!response.headers.contains_key("Strict-Transport-Security"));
	}

	#[rstest::rstest]
	#[case::custom_proxy_ssl_header_untrusted(
		Some(("X-Custom-Proto".to_string(), "https".to_string())),
		"X-Custom-Proto".to_string(),
		"https".to_string(),
		true,
		"custom proxy SSL header from untrusted source"
	)]
	#[case::x_forwarded_proto_untrusted(
		None,
		"x-forwarded-proto".to_string(),
		"https".to_string(),
		false,
		"X-Forwarded-Proto spoofed without trusted proxy"
	)]
	#[tokio::test]
	async fn test_proxy_header_ignored_without_trusted_proxy(
		#[case] secure_proxy_ssl_header: Option<(String, String)>,
		#[case] header_name: String,
		#[case] header_value: String,
		#[case] content_type_nosniff: bool,
		#[case] _scenario: String,
	) {
		// Arrange - proxy-related header from untrusted source should be ignored
		let config = SecurityConfig {
			hsts_enabled: true,
			hsts_seconds: 31536000,
			hsts_include_subdomains: false,
			hsts_preload: false,
			ssl_redirect: false,
			content_type_nosniff,
			referrer_policy: None,
			cross_origin_opener_policy: None,
			x_frame_options: None,
			secure_proxy_ssl_header,
		};
		let middleware = SecurityMiddleware::with_config(config);
		let handler = Arc::new(TestHandler);

		let mut headers = HeaderMap::new();
		// Attacker sends a proxy header directly (no trusted proxy configured)
		headers.insert(
			hyper::header::HeaderName::from_bytes(header_name.as_bytes()).unwrap(),
			header_value.parse().unwrap(),
		);

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

		// Assert - HSTS should NOT be set because header is from untrusted source
		assert_eq!(response.status, StatusCode::OK);
		assert!(!response.headers.contains_key("Strict-Transport-Security"));
	}

	/// Handler that always returns an error to simulate inner handler failure.
	struct ErrorHandler;

	#[async_trait]
	impl Handler for ErrorHandler {
		async fn handle(&self, _request: Request) -> Result<Response> {
			Err(Error::Http("handler error".to_string()))
		}
	}

	#[rstest]
	#[tokio::test]
	async fn test_security_headers_applied_on_handler_error() {
		// Arrange
		let config = SecurityConfig {
			hsts_enabled: true,
			hsts_seconds: 31536000,
			hsts_include_subdomains: false,
			hsts_preload: false,
			ssl_redirect: false,
			content_type_nosniff: true,
			referrer_policy: Some("same-origin".to_string()),
			cross_origin_opener_policy: None,
			x_frame_options: Some("DENY".to_string()),
			secure_proxy_ssl_header: None,
		};
		let middleware = SecurityMiddleware::with_config(config);
		let handler: Arc<dyn Handler> = Arc::new(ErrorHandler);

		let request = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.secure(true)
			.body(Bytes::new())
			.build()
			.unwrap();

		// Act
		let response = middleware.process(request, handler).await.unwrap();

		// Assert — error is converted to response with security headers applied
		assert!(response.status.is_client_error());
		assert_eq!(
			response.headers.get("X-Content-Type-Options").unwrap(),
			"nosniff"
		);
		assert_eq!(
			response.headers.get("Strict-Transport-Security").unwrap(),
			"max-age=31536000"
		);
		assert_eq!(
			response.headers.get("Referrer-Policy").unwrap(),
			"same-origin"
		);
		assert_eq!(response.headers.get("X-Frame-Options").unwrap(), "DENY");
	}

	#[rstest]
	fn test_from_security_settings_conversion() {
		// Arrange
		let settings = SecuritySettings {
			secure_ssl_redirect: true,
			secure_hsts_seconds: Some(63072000),
			secure_hsts_include_subdomains: true,
			secure_hsts_preload: true,
			secure_proxy_ssl_header: Some(("X-Forwarded-Proto".to_string(), "https".to_string())),
			..Default::default()
		};

		// Act
		let config = SecurityConfig::from(&settings);

		// Assert
		assert!(config.ssl_redirect);
		assert!(config.hsts_enabled);
		assert_eq!(config.hsts_seconds, 63072000);
		assert!(config.hsts_include_subdomains);
		assert!(config.hsts_preload);
		assert_eq!(
			config.secure_proxy_ssl_header,
			Some(("X-Forwarded-Proto".to_string(), "https".to_string()))
		);
		// Middleware-specific defaults preserved
		assert!(config.content_type_nosniff);
		assert_eq!(config.referrer_policy, Some("same-origin".to_string()));
		assert_eq!(config.x_frame_options, Some("DENY".to_string()));
	}

	#[rstest]
	fn test_from_security_settings_defaults() {
		// Arrange
		let settings = SecuritySettings::default();

		// Act
		let config = SecurityConfig::from(&settings);

		// Assert
		assert!(!config.ssl_redirect);
		assert!(!config.hsts_enabled);
		assert_eq!(config.hsts_seconds, 0);
		assert!(!config.hsts_include_subdomains);
		assert!(!config.hsts_preload);
		assert!(config.secure_proxy_ssl_header.is_none());
		// Middleware-specific defaults preserved
		assert!(config.content_type_nosniff);
		assert_eq!(config.referrer_policy, Some("same-origin".to_string()));
	}

	#[tokio::test]
	async fn test_from_security_settings_constructor() {
		// Arrange
		let settings = SecuritySettings {
			secure_ssl_redirect: false,
			secure_hsts_seconds: Some(31536000),
			..Default::default()
		};

		// Act
		let middleware = SecurityMiddleware::from_security_settings(&settings);
		let handler = Arc::new(TestHandler);

		let request = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.secure(true)
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
		assert_eq!(
			response.headers.get("X-Content-Type-Options").unwrap(),
			"nosniff"
		);
	}
}
