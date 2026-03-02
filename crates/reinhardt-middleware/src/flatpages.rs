//! Flatpages middleware
//!
//! Provides static page fallback functionality. When a request results in a 404,
//! the middleware attempts to serve content from a flatpages store.

use crate::xss::XssProtector;
use async_trait::async_trait;
use bytes::Bytes;
use hyper::StatusCode;
use reinhardt_http::{Handler, Middleware, Request, Response, Result};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Authentication state stored in request extensions
///
/// This should be populated by authentication middleware and can be
/// checked by other middleware like Flatpages.
///
/// # Examples
///
/// ```
/// use reinhardt_middleware::flatpages::AuthenticationState;
///
/// // Authenticated user
/// let auth = AuthenticationState::authenticated("user123".to_string());
/// assert!(auth.is_authenticated());
///
/// // Anonymous user
/// let auth = AuthenticationState::anonymous();
/// assert!(!auth.is_authenticated());
/// ```
#[derive(Debug, Clone)]
pub struct AuthenticationState {
	authenticated: bool,
	user_id: Option<String>,
}

impl AuthenticationState {
	/// Create authentication state for an authenticated user
	pub fn authenticated(user_id: String) -> Self {
		Self {
			authenticated: true,
			user_id: Some(user_id),
		}
	}

	/// Create authentication state for an anonymous user
	pub fn anonymous() -> Self {
		Self {
			authenticated: false,
			user_id: None,
		}
	}

	/// Check if user is authenticated
	pub fn is_authenticated(&self) -> bool {
		self.authenticated
	}

	/// Get user ID if authenticated
	pub fn user_id(&self) -> Option<&str> {
		self.user_id.as_deref()
	}
}

/// Flatpage content
#[derive(Debug, Clone, PartialEq)]
pub struct Flatpage {
	/// URL path
	pub url: String,
	/// Page title
	pub title: String,
	/// Page content (HTML)
	pub content: String,
	/// Whether template rendering is enabled
	pub enable_comments: bool,
	/// Registration required to view
	pub registration_required: bool,
}

impl Flatpage {
	/// Create a new flatpage
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_middleware::Flatpage;
	///
	/// let page = Flatpage::new(
	///     "/about/".to_string(),
	///     "About Us".to_string(),
	///     "<h1>About Us</h1>".to_string(),
	/// );
	/// ```
	pub fn new(url: String, title: String, content: String) -> Self {
		Self {
			url,
			title,
			content,
			enable_comments: false,
			registration_required: false,
		}
	}

	/// Enable comments
	pub fn with_comments(mut self) -> Self {
		self.enable_comments = true;
		self
	}

	/// Require registration
	pub fn require_registration(mut self) -> Self {
		self.registration_required = true;
		self
	}
}

/// Flatpages storage
#[derive(Debug, Default)]
pub struct FlatpageStore {
	pages: RwLock<HashMap<String, Flatpage>>,
}

impl FlatpageStore {
	/// Create a new flatpage store
	pub fn new() -> Self {
		Self::default()
	}

	/// Register a flatpage
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_middleware::{Flatpage, FlatpageStore};
	///
	/// let store = FlatpageStore::new();
	/// let page = Flatpage::new(
	///     "/about/".to_string(),
	///     "About".to_string(),
	///     "<h1>About</h1>".to_string(),
	/// );
	/// store.register(page);
	/// ```
	pub fn register(&self, page: Flatpage) {
		let url = page.url.clone();
		self.pages
			.write()
			.unwrap_or_else(|e| e.into_inner())
			.insert(url, page);
	}

	/// Get flatpage by URL
	pub fn get(&self, url: &str) -> Option<Flatpage> {
		self.pages
			.read()
			.unwrap_or_else(|e| e.into_inner())
			.get(url)
			.cloned()
	}

	/// Remove flatpage
	pub fn remove(&self, url: &str) -> Option<Flatpage> {
		self.pages
			.write()
			.unwrap_or_else(|e| e.into_inner())
			.remove(url)
	}

	/// Get all flatpages
	pub fn all(&self) -> Vec<Flatpage> {
		self.pages
			.read()
			.unwrap_or_else(|e| e.into_inner())
			.values()
			.cloned()
			.collect()
	}

	/// Clear all flatpages
	pub fn clear(&self) {
		self.pages
			.write()
			.unwrap_or_else(|e| e.into_inner())
			.clear();
	}
}

/// Flatpages middleware configuration
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct FlatpagesConfig {
	/// Whether the middleware is enabled
	pub enabled: bool,
	/// Whether to append trailing slash when looking up flatpages
	pub append_slash: bool,
	/// Whether to try without trailing slash
	pub try_without_slash: bool,
}

impl FlatpagesConfig {
	/// Create a new configuration with defaults
	pub fn new() -> Self {
		Self {
			enabled: true,
			append_slash: true,
			try_without_slash: true,
		}
	}

	/// Disable the middleware
	pub fn disabled() -> Self {
		Self {
			enabled: false,
			append_slash: true,
			try_without_slash: true,
		}
	}
}

impl Default for FlatpagesConfig {
	fn default() -> Self {
		Self::new()
	}
}

/// Flatpages middleware
///
/// # Examples
///
/// ```
/// use reinhardt_middleware::{Flatpage, FlatpageStore, FlatpagesMiddleware, FlatpagesConfig};
/// use std::sync::Arc;
///
/// let config = FlatpagesConfig::new();
/// let middleware = Arc::new(FlatpagesMiddleware::new(config));
///
/// // Register a flatpage
/// let page = Flatpage::new(
///     "/about/".to_string(),
///     "About Us".to_string(),
///     "<h1>About Us</h1>".to_string(),
/// );
/// middleware.store().register(page);
/// ```
pub struct FlatpagesMiddleware {
	config: FlatpagesConfig,
	store: Arc<FlatpageStore>,
}

impl FlatpagesMiddleware {
	/// Create a new flatpages middleware
	pub fn new(config: FlatpagesConfig) -> Self {
		Self {
			config,
			store: Arc::new(FlatpageStore::new()),
		}
	}

	/// Create from an existing Arc-wrapped flatpage store
	///
	/// This is provided for cases where you already have an `Arc<FlatpageStore>`.
	/// In most cases, you should use `new()` instead, which creates the store internally.
	pub fn from_arc(config: FlatpagesConfig, store: Arc<FlatpageStore>) -> Self {
		Self { config, store }
	}

	/// Get a reference to the flatpage store
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_middleware::{FlatpagesMiddleware, FlatpagesConfig, Flatpage};
	///
	/// let middleware = FlatpagesMiddleware::new(FlatpagesConfig::new());
	///
	/// // Register a page using the store accessor
	/// let page = Flatpage::new(
	///     "/about/".to_string(),
	///     "About".to_string(),
	///     "<h1>About</h1>".to_string(),
	/// );
	/// middleware.store().register(page);
	/// ```
	pub fn store(&self) -> &FlatpageStore {
		&self.store
	}

	/// Get a cloned Arc of the store (for cases where you need ownership)
	///
	/// In most cases, you should use `store()` instead to get a reference.
	pub fn store_arc(&self) -> Arc<FlatpageStore> {
		Arc::clone(&self.store)
	}

	/// Try to get flatpage with various URL transformations
	fn try_get_page(&self, url: &str) -> Option<Flatpage> {
		// Try exact match first
		if let Some(page) = self.store.get(url) {
			return Some(page);
		}

		// Try with trailing slash
		if self.config.append_slash && !url.ends_with('/') {
			let with_slash = format!("{}/", url);
			if let Some(page) = self.store.get(&with_slash) {
				return Some(page);
			}
		}

		// Try without trailing slash
		if self.config.try_without_slash && url.ends_with('/') && url.len() > 1 {
			let without_slash = &url[..url.len() - 1];
			if let Some(page) = self.store.get(without_slash) {
				return Some(page);
			}
		}

		None
	}
}

#[async_trait]
impl Middleware for FlatpagesMiddleware {
	async fn process(&self, request: Request, handler: Arc<dyn Handler>) -> Result<Response> {
		// Skip if disabled
		if !self.config.enabled {
			return handler.handle(request).await;
		}

		// Get path and authentication state before moving request
		let path = request.uri.path().to_string();
		let is_authenticated = request
			.extensions
			.get::<AuthenticationState>()
			.map(|auth| auth.is_authenticated())
			.unwrap_or(false);

		// Call handler first
		let response = handler.handle(request).await?;

		// Only intercept 404 responses
		if response.status != StatusCode::NOT_FOUND {
			return Ok(response);
		}

		// Try to find flatpage for this URL
		let path = path.as_str();
		if let Some(page) = self.try_get_page(path) {
			// Check authentication if registration_required
			if page.registration_required && !is_authenticated {
				// Return 401 Unauthorized with WWW-Authenticate header
				return Ok(Response::new(StatusCode::UNAUTHORIZED)
					.with_header("WWW-Authenticate", "Basic realm=\"Restricted\"")
					.with_body(Bytes::from("Authentication required to view this page")));
			}

			// Render flatpage content with HTML escaping to prevent stored XSS
			let escaped_title = XssProtector::escape_for_html_body(&page.title);
			let escaped_content = XssProtector::escape_for_html_body(&page.content);
			let html = format!(
				r#"<!DOCTYPE html>
<html>
<head>
    <title>{}</title>
</head>
<body>
    {}
</body>
</html>"#,
				escaped_title, escaped_content
			);

			return Ok(Response::new(StatusCode::OK).with_body(Bytes::from(html)));
		}

		// No flatpage found, return original 404
		Ok(response)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use bytes::Bytes;
	use hyper::{HeaderMap, Method, Version};
	use rstest::rstest;

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
			Ok(Response::new(self.status).with_body(Bytes::from("handler response")))
		}
	}

	#[tokio::test]
	async fn test_basic_flatpage() {
		let config = FlatpagesConfig::new();
		let middleware = Arc::new(FlatpagesMiddleware::new(config));

		// Register a flatpage
		let page = Flatpage::new(
			"/about/".to_string(),
			"About Us".to_string(),
			"<h1>About Us</h1>".to_string(),
		);
		middleware.store.register(page);

		let handler = Arc::new(TestHandler::not_found());
		let request = Request::builder()
			.method(Method::GET)
			.uri("/about/")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();

		assert_eq!(response.status, StatusCode::OK);
		let body = String::from_utf8_lossy(&response.body);
		assert!(body.contains("About Us"));
		// Content is HTML-escaped to prevent XSS
		assert!(body.contains("&lt;h1&gt;About Us&lt;/h1&gt;"));
	}

	#[tokio::test]
	async fn test_flatpage_with_trailing_slash() {
		let config = FlatpagesConfig::new();
		let middleware = Arc::new(FlatpagesMiddleware::new(config));

		// Register flatpage with trailing slash
		let page = Flatpage::new(
			"/contact/".to_string(),
			"Contact".to_string(),
			"<p>Contact us</p>".to_string(),
		);
		middleware.store.register(page);

		let handler = Arc::new(TestHandler::not_found());

		// Request without trailing slash
		let request = Request::builder()
			.method(Method::GET)
			.uri("/contact")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();

		assert_eq!(response.status, StatusCode::OK);
		let body = String::from_utf8_lossy(&response.body);
		assert!(body.contains("Contact"));
	}

	#[tokio::test]
	async fn test_flatpage_without_trailing_slash() {
		let config = FlatpagesConfig::new();
		let middleware = Arc::new(FlatpagesMiddleware::new(config));

		// Register flatpage without trailing slash
		let page = Flatpage::new(
			"/faq".to_string(),
			"FAQ".to_string(),
			"<p>Frequently Asked Questions</p>".to_string(),
		);
		middleware.store.register(page);

		let handler = Arc::new(TestHandler::not_found());

		// Request with trailing slash
		let request = Request::builder()
			.method(Method::GET)
			.uri("/faq/")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();

		assert_eq!(response.status, StatusCode::OK);
		let body = String::from_utf8_lossy(&response.body);
		assert!(body.contains("FAQ"));
	}

	#[tokio::test]
	async fn test_no_flatpage_found() {
		let config = FlatpagesConfig::new();
		let middleware = Arc::new(FlatpagesMiddleware::new(config));

		let handler = Arc::new(TestHandler::not_found());
		let request = Request::builder()
			.method(Method::GET)
			.uri("/nonexistent/")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();

		// Should return original 404
		assert_eq!(response.status, StatusCode::NOT_FOUND);
		let body = String::from_utf8_lossy(&response.body);
		assert_eq!(body, "handler response");
	}

	#[tokio::test]
	async fn test_non_404_response_passthrough() {
		let config = FlatpagesConfig::new();
		let middleware = Arc::new(FlatpagesMiddleware::new(config));

		// Register a flatpage
		let page = Flatpage::new(
			"/about/".to_string(),
			"About".to_string(),
			"<h1>About</h1>".to_string(),
		);
		middleware.store.register(page);

		// Handler returns OK, not 404
		let handler = Arc::new(TestHandler::ok());
		let request = Request::builder()
			.method(Method::GET)
			.uri("/about/")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();

		// Should return handler's OK response, not flatpage
		assert_eq!(response.status, StatusCode::OK);
		let body = String::from_utf8_lossy(&response.body);
		assert_eq!(body, "handler response");
	}

	#[tokio::test]
	async fn test_disabled_middleware() {
		let config = FlatpagesConfig::disabled();
		let middleware = Arc::new(FlatpagesMiddleware::new(config));

		// Register a flatpage
		let page = Flatpage::new(
			"/about/".to_string(),
			"About".to_string(),
			"<h1>About</h1>".to_string(),
		);
		middleware.store.register(page);

		let handler = Arc::new(TestHandler::not_found());
		let request = Request::builder()
			.method(Method::GET)
			.uri("/about/")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();

		// Should return 404 because middleware is disabled
		assert_eq!(response.status, StatusCode::NOT_FOUND);
	}

	#[tokio::test]
	async fn test_flatpage_store_operations() {
		let store = FlatpageStore::new();

		// Test register and get
		let page1 = Flatpage::new(
			"/page1/".to_string(),
			"Page 1".to_string(),
			"<p>Content 1</p>".to_string(),
		);
		store.register(page1.clone());

		let retrieved = store.get("/page1/").unwrap();
		assert_eq!(retrieved, page1);

		// Test all
		let page2 = Flatpage::new(
			"/page2/".to_string(),
			"Page 2".to_string(),
			"<p>Content 2</p>".to_string(),
		);
		store.register(page2);

		let all = store.all();
		assert_eq!(all.len(), 2);

		// Test remove
		let removed = store.remove("/page1/").unwrap();
		assert_eq!(removed, page1);
		assert!(store.get("/page1/").is_none());

		// Test clear
		store.clear();
		assert_eq!(store.all().len(), 0);
	}

	#[tokio::test]
	async fn test_flatpage_with_comments() {
		let page = Flatpage::new(
			"/test/".to_string(),
			"Test".to_string(),
			"<p>Test</p>".to_string(),
		)
		.with_comments();

		assert!(page.enable_comments);
	}

	#[tokio::test]
	async fn test_flatpage_require_registration() {
		let page = Flatpage::new(
			"/test/".to_string(),
			"Test".to_string(),
			"<p>Test</p>".to_string(),
		)
		.require_registration();

		assert!(page.registration_required);
	}

	#[tokio::test]
	async fn test_exact_match_priority() {
		let config = FlatpagesConfig::new();
		let middleware = Arc::new(FlatpagesMiddleware::new(config));

		// Register both versions
		let page_with_slash = Flatpage::new(
			"/test/".to_string(),
			"With Slash".to_string(),
			"<p>With slash</p>".to_string(),
		);
		let page_without_slash = Flatpage::new(
			"/test".to_string(),
			"Without Slash".to_string(),
			"<p>Without slash</p>".to_string(),
		);
		middleware.store.register(page_with_slash);
		middleware.store.register(page_without_slash);

		let handler = Arc::new(TestHandler::not_found());

		// Request with slash should match with-slash version
		let request = Request::builder()
			.method(Method::GET)
			.uri("/test/")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();
		let response = middleware.process(request, handler.clone()).await.unwrap();
		let body = String::from_utf8_lossy(&response.body);
		assert!(body.contains("With Slash"));

		// Request without slash should match without-slash version
		let request = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();
		let response = middleware.process(request, handler).await.unwrap();
		let body = String::from_utf8_lossy(&response.body);
		assert!(body.contains("Without Slash"));
	}

	#[tokio::test]
	async fn test_append_slash_disabled() {
		let mut config = FlatpagesConfig::new();
		config.append_slash = false;
		let middleware = Arc::new(FlatpagesMiddleware::new(config));

		// Register with slash
		let page = Flatpage::new(
			"/test/".to_string(),
			"Test".to_string(),
			"<p>Test</p>".to_string(),
		);
		middleware.store.register(page);

		let handler = Arc::new(TestHandler::not_found());

		// Request without slash should NOT match
		let request = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();
		let response = middleware.process(request, handler).await.unwrap();

		assert_eq!(response.status, StatusCode::NOT_FOUND);
	}

	#[tokio::test]
	async fn test_registration_required_authenticated_user() {
		let config = FlatpagesConfig::new();
		let middleware = Arc::new(FlatpagesMiddleware::new(config));

		// Register protected page
		let mut page = Flatpage::new(
			"/protected/".to_string(),
			"Protected Page".to_string(),
			"<p>Protected Content</p>".to_string(),
		);
		page.registration_required = true;
		middleware.store.register(page);

		let handler = Arc::new(TestHandler::not_found());

		// Create request with authentication state
		let request = Request::builder()
			.method(Method::GET)
			.uri("/protected/")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		// Add authentication state to request extensions
		request
			.extensions
			.insert(AuthenticationState::authenticated("user123".to_string()));

		let response = middleware.process(request, handler).await.unwrap();

		// Should serve the protected page
		assert_eq!(response.status, StatusCode::OK);
		let body = String::from_utf8_lossy(&response.body);
		assert!(body.contains("Protected Content"));
	}

	#[tokio::test]
	async fn test_registration_required_anonymous_user() {
		let config = FlatpagesConfig::new();
		let middleware = Arc::new(FlatpagesMiddleware::new(config));

		// Register protected page
		let mut page = Flatpage::new(
			"/protected/".to_string(),
			"Protected Page".to_string(),
			"<p>Protected Content</p>".to_string(),
		);
		page.registration_required = true;
		middleware.store.register(page);

		let handler = Arc::new(TestHandler::not_found());

		// Create request WITHOUT authentication state
		let request = Request::builder()
			.method(Method::GET)
			.uri("/protected/")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();

		// Should return 401 Unauthorized
		assert_eq!(response.status, StatusCode::UNAUTHORIZED);
		let body = String::from_utf8_lossy(&response.body);
		assert!(body.contains("Authentication required"));

		// Should include WWW-Authenticate header
		assert!(response.headers.contains_key("WWW-Authenticate"));
	}

	#[tokio::test]
	async fn test_no_registration_required_anonymous_user() {
		let config = FlatpagesConfig::new();
		let middleware = Arc::new(FlatpagesMiddleware::new(config));

		// Register public page (registration_required defaults to false)
		let page = Flatpage::new(
			"/public/".to_string(),
			"Public Page".to_string(),
			"<p>Public Content</p>".to_string(),
		);
		middleware.store.register(page);

		let handler = Arc::new(TestHandler::not_found());

		// Create request WITHOUT authentication state
		let request = Request::builder()
			.method(Method::GET)
			.uri("/public/")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();

		// Should serve the public page
		assert_eq!(response.status, StatusCode::OK);
		let body = String::from_utf8_lossy(&response.body);
		assert!(body.contains("Public Content"));
	}

	#[rstest]
	#[case::script_tag_in_title("<script>alert('xss')</script>", "Safe content", "<script>", false)]
	#[case::script_tag_in_content("Safe Title", "<script>alert('xss')</script>", "<script>", false)]
	#[case::img_onerror_in_content(
		"Safe Title",
		r#"<img src=x onerror="alert('xss')">"#,
		"<img",
		false
	)]
	#[case::event_handler_in_title(
		r#"" onmouseover="alert('xss')"#,
		"Safe content",
		r#"onmouseover=""#,
		false
	)]
	#[case::ampersand_escaped("Tom & Jerry", "A & B", "&amp;", true)]
	#[tokio::test]
	async fn test_flatpage_xss_prevention(
		#[case] title: &str,
		#[case] content: &str,
		#[case] pattern: &str,
		#[case] should_contain: bool,
	) {
		// Arrange
		let config = FlatpagesConfig::new();
		let middleware = Arc::new(FlatpagesMiddleware::new(config));
		let page = Flatpage::new(
			"/xss-test/".to_string(),
			title.to_string(),
			content.to_string(),
		);
		middleware.store.register(page);
		let handler = Arc::new(TestHandler::not_found());
		let request = Request::builder()
			.method(Method::GET)
			.uri("/xss-test/")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		// Act
		let response = middleware.process(request, handler).await.unwrap();

		// Assert
		assert_eq!(response.status, StatusCode::OK);
		let body = String::from_utf8_lossy(&response.body);
		assert_eq!(
			body.contains(pattern),
			should_contain,
			"Body should {} contain '{}'. Body: {}",
			if should_contain { "" } else { "NOT" },
			pattern,
			body
		);
	}

	#[rstest]
	#[tokio::test]
	async fn test_flatpage_xss_full_escape_verification() {
		// Arrange
		let config = FlatpagesConfig::new();
		let middleware = Arc::new(FlatpagesMiddleware::new(config));
		let page = Flatpage::new(
			"/escape-test/".to_string(),
			"<b>Title</b> & 'quotes' \"double\"".to_string(),
			"<script>alert(1)</script>".to_string(),
		);
		middleware.store.register(page);
		let handler = Arc::new(TestHandler::not_found());
		let request = Request::builder()
			.method(Method::GET)
			.uri("/escape-test/")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		// Act
		let response = middleware.process(request, handler).await.unwrap();

		// Assert
		assert_eq!(response.status, StatusCode::OK);
		let body = String::from_utf8_lossy(&response.body);
		// Verify title is escaped
		assert!(body.contains("&lt;b&gt;Title&lt;/b&gt;"));
		assert!(body.contains("&amp;"));
		assert!(body.contains("&#x27;quotes&#x27;"));
		assert!(body.contains("&quot;double&quot;"));
		// Verify content is escaped
		assert!(body.contains("&lt;script&gt;alert(1)&lt;/script&gt;"));
		// Verify no raw HTML tags in output
		assert!(!body.contains("<script>"));
		assert!(!body.contains("</script>"));
	}
}
