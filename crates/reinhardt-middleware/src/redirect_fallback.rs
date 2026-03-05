//! Redirect fallback middleware
//!
//! Provides automatic redirection for 404 errors to a fallback URL.
//! Useful for handling missing pages gracefully.

use async_trait::async_trait;
use hyper::StatusCode;
use regex::Regex;
use reinhardt_http::{Handler, Middleware, Request, Response, Result};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Configuration for redirect fallback behavior
#[non_exhaustive]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedirectResponseConfig {
	/// The fallback URL to redirect to on 404 errors
	pub fallback_url: String,
	/// Optional path patterns to match (if None, matches all 404s)
	pub path_patterns: Option<Vec<String>>,
	/// Status code to use for redirect (default: 302 Found)
	pub redirect_status: Option<u16>,
}

impl RedirectResponseConfig {
	/// Create a new configuration with a fallback URL
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_middleware::RedirectResponseConfig;
	///
	/// let config = RedirectResponseConfig::new("/404".to_string());
	/// assert_eq!(config.fallback_url, "/404");
	/// ```
	pub fn new(fallback_url: String) -> Self {
		Self {
			fallback_url,
			path_patterns: None,
			redirect_status: None,
		}
	}

	/// Add path patterns to match
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_middleware::RedirectResponseConfig;
	///
	/// let config = RedirectResponseConfig::new("/404".to_string())
	///     .with_patterns(vec!["/api/.*".to_string()]);
	/// ```
	pub fn with_patterns(mut self, patterns: Vec<String>) -> Self {
		self.path_patterns = Some(patterns);
		self
	}

	/// Set custom redirect status code
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_middleware::RedirectResponseConfig;
	///
	/// let config = RedirectResponseConfig::new("/404".to_string())
	///     .with_status(301);
	/// ```
	pub fn with_status(mut self, status: u16) -> Self {
		self.redirect_status = Some(status);
		self
	}
}

/// Middleware that redirects 404 errors to a fallback URL
///
/// # Examples
///
/// ```
/// use std::sync::Arc;
/// use reinhardt_middleware::{RedirectFallbackMiddleware, RedirectResponseConfig};
/// use reinhardt_http::{Handler, Middleware, Request, Response};
/// use hyper::{StatusCode, Method, Version, HeaderMap};
/// use bytes::Bytes;
///
/// struct NotFoundHandler;
///
/// #[async_trait::async_trait]
/// impl Handler for NotFoundHandler {
///     async fn handle(&self, _request: Request) -> reinhardt_core::exception::Result<Response> {
///         Ok(Response::new(StatusCode::NOT_FOUND))
///     }
/// }
///
/// # tokio_test::block_on(async {
/// let config = RedirectResponseConfig::new("/404".to_string());
/// let middleware = RedirectFallbackMiddleware::new(config);
/// let handler = Arc::new(NotFoundHandler);
///
/// let request = Request::builder()
///     .method(Method::GET)
///     .uri("/missing")
///     .version(Version::HTTP_11)
///     .headers(HeaderMap::new())
///     .body(Bytes::new())
///     .build()
///     .unwrap();
///
/// let response = middleware.process(request, handler).await.unwrap();
/// assert_eq!(response.status, StatusCode::FOUND);
/// assert_eq!(
///     response.headers.get(hyper::header::LOCATION).unwrap(),
///     "/404"
/// );
/// # });
/// ```
pub struct RedirectFallbackMiddleware {
	config: RedirectResponseConfig,
	compiled_patterns: Option<Vec<Regex>>,
}

impl RedirectFallbackMiddleware {
	/// Create a new RedirectFallbackMiddleware with the given configuration
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_middleware::{RedirectFallbackMiddleware, RedirectResponseConfig};
	///
	/// let config = RedirectResponseConfig::new("/404".to_string());
	/// let middleware = RedirectFallbackMiddleware::new(config);
	/// ```
	pub fn new(config: RedirectResponseConfig) -> Self {
		let compiled_patterns = config
			.path_patterns
			.as_ref()
			.map(|patterns| patterns.iter().filter_map(|p| Regex::new(p).ok()).collect());

		Self {
			config,
			compiled_patterns,
		}
	}

	/// Check if the path matches any configured patterns
	fn matches_pattern(&self, path: &str) -> bool {
		match &self.compiled_patterns {
			None => true, // No patterns means match all
			Some(patterns) => patterns.iter().any(|re| re.is_match(path)),
		}
	}

	/// Get the redirect status code to use
	fn redirect_status(&self) -> StatusCode {
		self.config
			.redirect_status
			.and_then(|code| StatusCode::from_u16(code).ok())
			.unwrap_or(StatusCode::FOUND)
	}

	/// Check if we should redirect to avoid loops
	fn should_redirect(&self, path: &str) -> bool {
		// Prevent redirect loop: don't redirect if already at fallback URL
		path != self.config.fallback_url
	}
}

#[async_trait]
impl Middleware for RedirectFallbackMiddleware {
	async fn process(&self, request: Request, handler: Arc<dyn Handler>) -> Result<Response> {
		let path = request.uri.path().to_string();

		// Call the handler
		let response = handler.handle(request).await?;

		// Only redirect on 404 errors
		if response.status != StatusCode::NOT_FOUND {
			return Ok(response);
		}

		// Check if we should redirect (pattern match and loop prevention)
		if !self.matches_pattern(&path) || !self.should_redirect(&path) {
			return Ok(response);
		}

		// Create redirect response
		let mut redirect_response = Response::new(self.redirect_status());
		redirect_response.headers.insert(
			hyper::header::LOCATION,
			self.config.fallback_url.parse().unwrap(),
		);

		Ok(redirect_response)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use bytes::Bytes;
	use hyper::{HeaderMap, Method, StatusCode, Version};

	struct NotFoundHandler;

	#[async_trait]
	impl Handler for NotFoundHandler {
		async fn handle(&self, _request: Request) -> Result<Response> {
			Ok(Response::new(StatusCode::NOT_FOUND))
		}
	}

	struct OkHandler;

	#[async_trait]
	impl Handler for OkHandler {
		async fn handle(&self, _request: Request) -> Result<Response> {
			Ok(Response::new(StatusCode::OK).with_body(Bytes::from("OK")))
		}
	}

	#[tokio::test]
	async fn test_redirect_on_404() {
		let config = RedirectResponseConfig::new("/404".to_string());
		let middleware = RedirectFallbackMiddleware::new(config);
		let handler = Arc::new(NotFoundHandler);

		let request = Request::builder()
			.method(Method::GET)
			.uri("/missing")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();

		assert_eq!(response.status, StatusCode::FOUND);
		assert_eq!(
			response.headers.get(hyper::header::LOCATION).unwrap(),
			"/404"
		);
	}

	#[tokio::test]
	async fn test_no_redirect_on_200() {
		let config = RedirectResponseConfig::new("/404".to_string());
		let middleware = RedirectFallbackMiddleware::new(config);
		let handler = Arc::new(OkHandler);

		let request = Request::builder()
			.method(Method::GET)
			.uri("/existing")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();

		assert_eq!(response.status, StatusCode::OK);
		assert!(!response.headers.contains_key(hyper::header::LOCATION));
	}

	#[tokio::test]
	async fn test_pattern_matching_redirect() {
		let config = RedirectResponseConfig::new("/404".to_string())
			.with_patterns(vec!["/api/.*".to_string()]);
		let middleware = RedirectFallbackMiddleware::new(config);
		let handler = Arc::new(NotFoundHandler);

		// Should redirect for /api/* paths
		let request = Request::builder()
			.method(Method::GET)
			.uri("/api/missing")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();

		assert_eq!(response.status, StatusCode::FOUND);
		assert_eq!(
			response.headers.get(hyper::header::LOCATION).unwrap(),
			"/404"
		);
	}

	#[tokio::test]
	async fn test_pattern_no_match_no_redirect() {
		let config = RedirectResponseConfig::new("/404".to_string())
			.with_patterns(vec!["/api/.*".to_string()]);
		let middleware = RedirectFallbackMiddleware::new(config);
		let handler = Arc::new(NotFoundHandler);

		// Should NOT redirect for non-/api/* paths
		let request = Request::builder()
			.method(Method::GET)
			.uri("/other/missing")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();

		assert_eq!(response.status, StatusCode::NOT_FOUND);
		assert!(!response.headers.contains_key(hyper::header::LOCATION));
	}

	#[tokio::test]
	async fn test_custom_redirect_status() {
		let config = RedirectResponseConfig::new("/404".to_string()).with_status(301);
		let middleware = RedirectFallbackMiddleware::new(config);
		let handler = Arc::new(NotFoundHandler);

		let request = Request::builder()
			.method(Method::GET)
			.uri("/missing")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();

		assert_eq!(response.status, StatusCode::MOVED_PERMANENTLY);
		assert_eq!(
			response.headers.get(hyper::header::LOCATION).unwrap(),
			"/404"
		);
	}

	#[tokio::test]
	async fn test_prevent_redirect_loop() {
		let config = RedirectResponseConfig::new("/404".to_string());
		let middleware = RedirectFallbackMiddleware::new(config);
		let handler = Arc::new(NotFoundHandler);

		// Request to the fallback URL itself should not redirect
		let request = Request::builder()
			.method(Method::GET)
			.uri("/404")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();

		assert_eq!(response.status, StatusCode::NOT_FOUND);
		assert!(!response.headers.contains_key(hyper::header::LOCATION));
	}

	#[tokio::test]
	async fn test_multiple_pattern_matching() {
		let config = RedirectResponseConfig::new("/error".to_string())
			.with_patterns(vec!["/api/.*".to_string(), "/v1/.*".to_string()]);
		let middleware = RedirectFallbackMiddleware::new(config);
		let handler = Arc::new(NotFoundHandler);

		// Test first pattern
		let request1 = Request::builder()
			.method(Method::GET)
			.uri("/api/test")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		let response1 = middleware.process(request1, handler.clone()).await.unwrap();
		assert_eq!(response1.status, StatusCode::FOUND);

		// Test second pattern
		let request2 = Request::builder()
			.method(Method::GET)
			.uri("/v1/test")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		let response2 = middleware.process(request2, handler).await.unwrap();
		assert_eq!(response2.status, StatusCode::FOUND);
	}

	#[tokio::test]
	async fn test_different_http_methods() {
		let config = RedirectResponseConfig::new("/404".to_string());
		let middleware = RedirectFallbackMiddleware::new(config);
		let handler = Arc::new(NotFoundHandler);

		// Test POST
		let request = Request::builder()
			.method(Method::POST)
			.uri("/missing")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();

		assert_eq!(response.status, StatusCode::FOUND);
		assert_eq!(
			response.headers.get(hyper::header::LOCATION).unwrap(),
			"/404"
		);
	}

	#[tokio::test]
	async fn test_no_patterns_matches_all() {
		let config = RedirectResponseConfig::new("/fallback".to_string());
		let middleware = RedirectFallbackMiddleware::new(config);
		let handler = Arc::new(NotFoundHandler);

		// Any path should redirect when no patterns are specified
		let paths = vec!["/api/test", "/admin/test", "/any/path/here"];

		for path in paths {
			let request = Request::builder()
				.method(Method::GET)
				.uri(path)
				.version(Version::HTTP_11)
				.headers(HeaderMap::new())
				.body(Bytes::new())
				.build()
				.unwrap();

			let response = middleware.process(request, handler.clone()).await.unwrap();

			assert_eq!(response.status, StatusCode::FOUND);
			assert_eq!(
				response.headers.get(hyper::header::LOCATION).unwrap(),
				"/fallback"
			);
		}
	}

	#[tokio::test]
	async fn test_complex_pattern_matching() {
		let config = RedirectResponseConfig::new("/404".to_string())
			.with_patterns(vec!["/api/v[0-9]+/.*".to_string()]);
		let middleware = RedirectFallbackMiddleware::new(config);
		let handler = Arc::new(NotFoundHandler);

		// Should match /api/v1/, /api/v2/, etc.
		let request1 = Request::builder()
			.method(Method::GET)
			.uri("/api/v1/users")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		let response1 = middleware.process(request1, handler.clone()).await.unwrap();
		assert_eq!(response1.status, StatusCode::FOUND);

		// Should NOT match /api/version/
		let request2 = Request::builder()
			.method(Method::GET)
			.uri("/api/version/users")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		let response2 = middleware.process(request2, handler).await.unwrap();
		assert_eq!(response2.status, StatusCode::NOT_FOUND);
	}
}
