//! Cache control middleware
//!
//! Provides Cache-Control header management for static files with
//! configurable policies based on file types and patterns.

use async_trait::async_trait;
use hyper::header::HeaderName;
use reinhardt_core::exception::Result;
use reinhardt_http::{Handler, Middleware};
use reinhardt_http::{Request, Response};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

/// Cache control directive
#[derive(Debug, Clone, PartialEq)]
pub enum CacheDirective {
	/// public - Response may be cached by any cache
	Public,
	/// private - Response is for single user and should not be cached by shared caches
	Private,
	/// no-cache - Must revalidate with origin server before using cached copy
	NoCache,
	/// no-store - Response must not be cached anywhere
	NoStore,
	/// must-revalidate - Must revalidate stale resources
	MustRevalidate,
	/// proxy-revalidate - Like must-revalidate but only for shared caches
	ProxyRevalidate,
	/// immutable - Response will not change and can be cached permanently
	Immutable,
}

impl CacheDirective {
	fn as_str(&self) -> &str {
		match self {
			CacheDirective::Public => "public",
			CacheDirective::Private => "private",
			CacheDirective::NoCache => "no-cache",
			CacheDirective::NoStore => "no-store",
			CacheDirective::MustRevalidate => "must-revalidate",
			CacheDirective::ProxyRevalidate => "proxy-revalidate",
			CacheDirective::Immutable => "immutable",
		}
	}
}

/// Cache control policy for specific file types or patterns
#[derive(Debug, Clone)]
pub struct CachePolicy {
	/// Cache directives
	pub directives: Vec<CacheDirective>,
	/// Maximum age in seconds
	pub max_age: Option<Duration>,
	/// S-maxage (for shared caches) in seconds
	pub s_maxage: Option<Duration>,
	/// Vary header value
	pub vary: Option<String>,
}

impl CachePolicy {
	/// Create a new cache policy
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_utils::staticfiles::caching::{CachePolicy, CacheDirective};
	/// use std::time::Duration;
	///
	/// let policy = CachePolicy::new()
	///     .with_directive(CacheDirective::Public)
	///     .with_max_age(Duration::from_secs(31536000));
	/// ```
	pub fn new() -> Self {
		Self {
			directives: Vec::new(),
			max_age: None,
			s_maxage: None,
			vary: None,
		}
	}

	/// Add a cache directive
	pub fn with_directive(mut self, directive: CacheDirective) -> Self {
		self.directives.push(directive);
		self
	}

	/// Set max-age
	pub fn with_max_age(mut self, max_age: Duration) -> Self {
		self.max_age = Some(max_age);
		self
	}

	/// Set s-maxage
	pub fn with_s_maxage(mut self, s_maxage: Duration) -> Self {
		self.s_maxage = Some(s_maxage);
		self
	}

	/// Set Vary header
	pub fn with_vary(mut self, vary: String) -> Self {
		self.vary = Some(vary);
		self
	}

	/// Create a policy for long-term caching (1 year)
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_utils::staticfiles::caching::CachePolicy;
	///
	/// let policy = CachePolicy::long_term();
	/// ```
	pub fn long_term() -> Self {
		Self::new()
			.with_directive(CacheDirective::Public)
			.with_directive(CacheDirective::Immutable)
			.with_max_age(Duration::from_secs(31536000)) // 1 year
	}

	/// Create a policy for short-term caching (5 minutes)
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_utils::staticfiles::caching::CachePolicy;
	///
	/// let policy = CachePolicy::short_term();
	/// ```
	pub fn short_term() -> Self {
		Self::new()
			.with_directive(CacheDirective::Public)
			.with_directive(CacheDirective::MustRevalidate)
			.with_max_age(Duration::from_secs(300)) // 5 minutes
	}

	/// Create a policy that prevents caching
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_utils::staticfiles::caching::CachePolicy;
	///
	/// let policy = CachePolicy::no_cache();
	/// ```
	pub fn no_cache() -> Self {
		Self::new()
			.with_directive(CacheDirective::NoCache)
			.with_directive(CacheDirective::NoStore)
			.with_directive(CacheDirective::MustRevalidate)
	}

	/// Generate Cache-Control header value
	pub fn to_header_value(&self) -> String {
		let mut parts = Vec::new();

		// Add directives
		for directive in &self.directives {
			parts.push(directive.as_str().to_string());
		}

		// Add max-age
		if let Some(max_age) = self.max_age {
			parts.push(format!("max-age={}", max_age.as_secs()));
		}

		// Add s-maxage
		if let Some(s_maxage) = self.s_maxage {
			parts.push(format!("s-maxage={}", s_maxage.as_secs()));
		}

		parts.join(", ")
	}
}

impl Default for CachePolicy {
	fn default() -> Self {
		Self::new()
	}
}

/// Cache control middleware configuration
#[derive(Debug, Clone)]
pub struct CacheControlConfig {
	/// Whether the middleware is enabled
	pub enabled: bool,
	/// Default cache policy
	pub default_policy: CachePolicy,
	/// File type specific policies (extension -> policy)
	pub type_policies: HashMap<String, CachePolicy>,
	/// Pattern-based policies (regex pattern -> policy)
	pub pattern_policies: Vec<(String, CachePolicy)>,
}

impl CacheControlConfig {
	/// Create a new configuration with sensible defaults
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_utils::staticfiles::caching::CacheControlConfig;
	///
	/// let config = CacheControlConfig::new();
	/// ```
	pub fn new() -> Self {
		let mut config = Self {
			enabled: true,
			default_policy: CachePolicy::short_term(),
			type_policies: HashMap::new(),
			pattern_policies: Vec::new(),
		};

		// Set up common file type policies
		config
			.type_policies
			.insert("css".to_string(), CachePolicy::long_term());
		config
			.type_policies
			.insert("js".to_string(), CachePolicy::long_term());
		config
			.type_policies
			.insert("woff".to_string(), CachePolicy::long_term());
		config
			.type_policies
			.insert("woff2".to_string(), CachePolicy::long_term());
		config
			.type_policies
			.insert("ttf".to_string(), CachePolicy::long_term());
		config
			.type_policies
			.insert("eot".to_string(), CachePolicy::long_term());
		config
			.type_policies
			.insert("png".to_string(), CachePolicy::long_term());
		config
			.type_policies
			.insert("jpg".to_string(), CachePolicy::long_term());
		config
			.type_policies
			.insert("jpeg".to_string(), CachePolicy::long_term());
		config
			.type_policies
			.insert("gif".to_string(), CachePolicy::long_term());
		config
			.type_policies
			.insert("svg".to_string(), CachePolicy::long_term());
		config
			.type_policies
			.insert("webp".to_string(), CachePolicy::long_term());
		config
			.type_policies
			.insert("ico".to_string(), CachePolicy::long_term());
		config
			.type_policies
			.insert("wasm".to_string(), CachePolicy::long_term());

		// HTML files should revalidate more frequently
		config
			.type_policies
			.insert("html".to_string(), CachePolicy::short_term());

		config
	}

	/// Disable caching
	pub fn disabled() -> Self {
		Self {
			enabled: false,
			default_policy: CachePolicy::no_cache(),
			type_policies: HashMap::new(),
			pattern_policies: Vec::new(),
		}
	}

	/// Set policy for a file type
	pub fn with_type_policy(mut self, extension: String, policy: CachePolicy) -> Self {
		self.type_policies.insert(extension, policy);
		self
	}

	/// Set default policy
	pub fn with_default_policy(mut self, policy: CachePolicy) -> Self {
		self.default_policy = policy;
		self
	}

	/// Get policy for a given path
	pub(crate) fn get_policy(&self, path: &str) -> &CachePolicy {
		// Try extension-based matching first
		if let Some(extension) = path.rsplit('.').next()
			&& let Some(policy) = self.type_policies.get(extension)
		{
			return policy;
		}

		// Try pattern-based matching
		for (pattern, policy) in &self.pattern_policies {
			if let Ok(regex) = regex::Regex::new(pattern)
				&& regex.is_match(path)
			{
				return policy;
			}
		}

		// Return default policy
		&self.default_policy
	}
}

impl Default for CacheControlConfig {
	fn default() -> Self {
		Self::new()
	}
}

/// Cache control middleware
///
/// # Examples
///
/// ```
/// use reinhardt_utils::staticfiles::caching::{CacheControlMiddleware, CacheControlConfig};
/// use std::sync::Arc;
///
/// let config = CacheControlConfig::new();
/// let middleware = Arc::new(CacheControlMiddleware::new(config));
/// ```
pub struct CacheControlMiddleware {
	config: CacheControlConfig,
}

impl CacheControlMiddleware {
	/// Create a new cache control middleware
	pub fn new(config: CacheControlConfig) -> Self {
		Self { config }
	}

	/// Create with default configuration
	pub fn default_config() -> Self {
		Self::new(CacheControlConfig::new())
	}
}

#[async_trait]
impl Middleware for CacheControlMiddleware {
	async fn process(&self, request: Request, handler: Arc<dyn Handler>) -> Result<Response> {
		// Skip if disabled
		if !self.config.enabled {
			return handler.handle(request).await;
		}

		// Get path before moving request
		let path = request.uri.path().to_string();

		// Convert errors to responses so post-processing always runs,
		// even when invoked outside MiddlewareChain. (#3244)
		let mut response = match handler.handle(request).await {
			Ok(resp) => resp,
			Err(e) => Response::from(e),
		};

		// Only add Cache-Control for successful responses
		if !response.status.is_success() {
			return Ok(response);
		}

		// Get appropriate policy for this path
		let policy = self.config.get_policy(&path);

		// Add Cache-Control header
		// These header name/value parses use known-valid static strings, so expect() is appropriate
		let cache_control_header: HeaderName = "cache-control"
			.parse()
			.expect("valid header name: cache-control");
		response.headers.insert(
			cache_control_header,
			policy
				.to_header_value()
				.parse()
				.expect("valid header value for cache-control"),
		);

		// Add Vary header if specified
		if let Some(vary) = &policy.vary {
			let vary_header: HeaderName = "vary".parse().expect("valid header name: vary");
			if let Ok(vary_value) = vary.parse() {
				response.headers.insert(vary_header, vary_value);
			} else {
				tracing::warn!("Invalid Vary header value: {}", vary);
			}
		}

		Ok(response)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use bytes::Bytes;
	use hyper::{HeaderMap, Method, StatusCode, Version};

	struct TestHandler {
		status: StatusCode,
	}

	impl TestHandler {
		fn ok() -> Self {
			Self {
				status: StatusCode::OK,
			}
		}

		fn not_found() -> Self {
			Self {
				status: StatusCode::NOT_FOUND,
			}
		}
	}

	#[async_trait]
	impl Handler for TestHandler {
		async fn handle(&self, _request: Request) -> Result<Response> {
			Ok(Response::new(self.status).with_body(Bytes::from("test")))
		}
	}

	#[tokio::test]
	async fn test_cache_policy_long_term() {
		let policy = CachePolicy::long_term();
		let header_value = policy.to_header_value();

		assert_eq!(header_value, "public, immutable, max-age=31536000");
	}

	#[tokio::test]
	async fn test_cache_policy_short_term() {
		let policy = CachePolicy::short_term();
		let header_value = policy.to_header_value();

		assert_eq!(header_value, "public, must-revalidate, max-age=300");
	}

	#[tokio::test]
	async fn test_cache_policy_no_cache() {
		let policy = CachePolicy::no_cache();
		let header_value = policy.to_header_value();

		assert_eq!(header_value, "no-cache, no-store, must-revalidate");
	}

	#[tokio::test]
	async fn test_css_file_gets_long_term_cache() {
		let config = CacheControlConfig::new();
		let middleware = Arc::new(CacheControlMiddleware::new(config));
		let handler = Arc::new(TestHandler::ok());

		let request = Request::builder()
			.method(Method::GET)
			.uri("/static/style.css")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();

		let cache_control = response
			.headers
			.get("cache-control")
			.unwrap()
			.to_str()
			.unwrap();
		assert_eq!(cache_control, "public, immutable, max-age=31536000");
	}

	#[tokio::test]
	async fn test_js_file_gets_long_term_cache() {
		let config = CacheControlConfig::new();
		let middleware = Arc::new(CacheControlMiddleware::new(config));
		let handler = Arc::new(TestHandler::ok());

		let request = Request::builder()
			.method(Method::GET)
			.uri("/static/app.js")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();

		let cache_control = response
			.headers
			.get("cache-control")
			.unwrap()
			.to_str()
			.unwrap();
		assert_eq!(cache_control, "public, immutable, max-age=31536000");
	}

	#[tokio::test]
	async fn test_html_file_gets_short_term_cache() {
		let config = CacheControlConfig::new();
		let middleware = Arc::new(CacheControlMiddleware::new(config));
		let handler = Arc::new(TestHandler::ok());

		let request = Request::builder()
			.method(Method::GET)
			.uri("/index.html")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();

		let cache_control = response
			.headers
			.get("cache-control")
			.unwrap()
			.to_str()
			.unwrap();
		assert_eq!(cache_control, "public, must-revalidate, max-age=300");
	}

	#[tokio::test]
	async fn test_image_files_get_long_term_cache() {
		let config = CacheControlConfig::new();
		let middleware = Arc::new(CacheControlMiddleware::new(config));
		let handler = Arc::new(TestHandler::ok());

		for ext in &["png", "jpg", "jpeg", "gif", "svg", "webp"] {
			let url = format!("/static/image.{}", ext);
			let request = Request::builder()
				.method(Method::GET)
				.uri(url.as_str())
				.version(Version::HTTP_11)
				.headers(HeaderMap::new())
				.body(Bytes::new())
				.build()
				.unwrap();

			let response = middleware.process(request, handler.clone()).await.unwrap();
			let cache_control = response
				.headers
				.get("cache-control")
				.unwrap()
				.to_str()
				.unwrap();
			assert_eq!(
				cache_control, "public, immutable, max-age=31536000",
				"Extension: {}",
				ext
			);
		}
	}

	#[tokio::test]
	async fn test_font_files_get_long_term_cache() {
		let config = CacheControlConfig::new();
		let middleware = Arc::new(CacheControlMiddleware::new(config));
		let handler = Arc::new(TestHandler::ok());

		for ext in &["woff", "woff2", "ttf", "eot"] {
			let url = format!("/static/font.{}", ext);
			let request = Request::builder()
				.method(Method::GET)
				.uri(url.as_str())
				.version(Version::HTTP_11)
				.headers(HeaderMap::new())
				.body(Bytes::new())
				.build()
				.unwrap();

			let response = middleware.process(request, handler.clone()).await.unwrap();
			let cache_control = response
				.headers
				.get("cache-control")
				.unwrap()
				.to_str()
				.unwrap();
			assert_eq!(
				cache_control, "public, immutable, max-age=31536000",
				"Extension: {}",
				ext
			);
		}
	}

	#[tokio::test]
	async fn test_unknown_extension_gets_default_policy() {
		let config = CacheControlConfig::new();
		let middleware = Arc::new(CacheControlMiddleware::new(config));
		let handler = Arc::new(TestHandler::ok());

		let request = Request::builder()
			.method(Method::GET)
			.uri("/static/file.unknown")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();

		let cache_control = response
			.headers
			.get("cache-control")
			.unwrap()
			.to_str()
			.unwrap();
		// Default policy is short_term
		assert_eq!(cache_control, "public, must-revalidate, max-age=300");
	}

	#[tokio::test]
	async fn test_custom_type_policy() {
		let config =
			CacheControlConfig::new().with_type_policy("txt".to_string(), CachePolicy::no_cache());
		let middleware = Arc::new(CacheControlMiddleware::new(config));
		let handler = Arc::new(TestHandler::ok());

		let request = Request::builder()
			.method(Method::GET)
			.uri("/static/file.txt")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();

		let cache_control = response
			.headers
			.get("cache-control")
			.unwrap()
			.to_str()
			.unwrap();
		assert_eq!(cache_control, "no-cache, no-store, must-revalidate");
	}

	#[tokio::test]
	async fn test_disabled_middleware() {
		let config = CacheControlConfig::disabled();
		let middleware = Arc::new(CacheControlMiddleware::new(config));
		let handler = Arc::new(TestHandler::ok());

		let request = Request::builder()
			.method(Method::GET)
			.uri("/static/style.css")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();

		// Should not have Cache-Control header
		assert!(!response.headers.contains_key("cache-control"));
	}

	#[tokio::test]
	async fn test_non_success_response_not_cached() {
		let config = CacheControlConfig::new();
		let middleware = Arc::new(CacheControlMiddleware::new(config));
		let handler = Arc::new(TestHandler::not_found());

		let request = Request::builder()
			.method(Method::GET)
			.uri("/static/nonexistent.css")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();

		// 404 responses should not get Cache-Control headers
		assert!(!response.headers.contains_key("cache-control"));
	}

	#[tokio::test]
	async fn test_vary_header() {
		let policy = CachePolicy::short_term().with_vary("Accept-Encoding".to_string());
		let config = CacheControlConfig::new().with_default_policy(policy);
		let middleware = Arc::new(CacheControlMiddleware::new(config));
		let handler = Arc::new(TestHandler::ok());

		let request = Request::builder()
			.method(Method::GET)
			.uri("/static/file.unknown")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();

		assert!(response.headers.contains_key("vary"));
		assert_eq!(
			response.headers.get("vary").unwrap().to_str().unwrap(),
			"Accept-Encoding"
		);
	}

	#[tokio::test]
	async fn test_s_maxage() {
		let policy = CachePolicy::new()
			.with_directive(CacheDirective::Public)
			.with_max_age(Duration::from_secs(300))
			.with_s_maxage(Duration::from_secs(3600));
		let header_value = policy.to_header_value();

		assert_eq!(header_value, "public, max-age=300, s-maxage=3600");
	}

	#[tokio::test]
	async fn test_wasm_file_gets_long_term_cache() {
		let config = CacheControlConfig::new();
		let middleware = Arc::new(CacheControlMiddleware::new(config));
		let handler = Arc::new(TestHandler::ok());

		let request = Request::builder()
			.method(Method::GET)
			.uri("/static/app.wasm")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();

		let cache_control = response
			.headers
			.get("cache-control")
			.unwrap()
			.to_str()
			.unwrap();
		assert_eq!(cache_control, "public, immutable, max-age=31536000");
	}

	#[tokio::test]
	async fn test_default_config_includes_wasm() {
		let config = CacheControlConfig::new();
		assert!(config.type_policies.contains_key("wasm"));
	}

	#[tokio::test]
	async fn test_multiple_directives() {
		let policy = CachePolicy::new()
			.with_directive(CacheDirective::Public)
			.with_directive(CacheDirective::MustRevalidate)
			.with_directive(CacheDirective::ProxyRevalidate)
			.with_max_age(Duration::from_secs(3600));
		let header_value = policy.to_header_value();

		assert_eq!(
			header_value,
			"public, must-revalidate, proxy-revalidate, max-age=3600"
		);
	}
}
