//! Session middleware for HTTP requests
//!
//! This module provides middleware that automatically loads and saves sessions
//! for each HTTP request/response cycle.
//!
//! ## Example
//!
//! ```rust,no_run,ignore
//! use reinhardt_auth::sessions::middleware::{SessionMiddleware, HttpSessionConfig, SameSite};
//! use reinhardt_auth::sessions::backends::InMemorySessionBackend;
//! use std::time::Duration;
//!
//! // Create session backend
//! let backend = InMemorySessionBackend::new();
//!
//! // Configure session middleware
//! let config = HttpSessionConfig {
//!     cookie_name: "sessionid".to_string(),
//!     cookie_path: "/".to_string(),
//!     cookie_domain: None,
//!     secure: true,
//!     httponly: true,
//!     samesite: SameSite::Lax,
//!     max_age: Some(Duration::from_secs(3600)),
//! };
//!
//! // Create middleware
//! let middleware = SessionMiddleware::new(backend, config);
//! ```

#[cfg(feature = "middleware")]
use super::backends::SessionBackend;
#[cfg(feature = "middleware")]
use super::session::Session;
#[cfg(feature = "middleware")]
use async_trait::async_trait;
#[cfg(feature = "middleware")]
use reinhardt_core::exception::Result;
#[cfg(feature = "middleware")]
use reinhardt_http::{Handler, Middleware};
#[cfg(feature = "middleware")]
use reinhardt_http::{Request, Response};
#[cfg(feature = "middleware")]
use std::sync::Arc;
#[cfg(feature = "middleware")]
use std::time::Duration;
#[cfg(feature = "middleware")]
use tokio::sync::RwLock;

#[cfg(feature = "middleware")]
/// SameSite cookie attribute
///
/// Controls when cookies are sent with cross-site requests.
///
/// ## Example
///
/// ```rust
/// use reinhardt_auth::sessions::middleware::SameSite;
///
/// let strict = SameSite::Strict;
/// let lax = SameSite::Lax;
/// let none = SameSite::None;
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SameSite {
	/// Cookies are only sent in a first-party context
	Strict,
	/// Cookies are sent on top-level navigation and with GET requests
	Lax,
	/// Cookies are sent with both first-party and cross-site requests
	None,
}

#[cfg(feature = "middleware")]
impl SameSite {
	/// Convert to cookie string value
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_auth::sessions::middleware::SameSite;
	///
	/// assert_eq!(SameSite::Strict.as_str(), "Strict");
	/// assert_eq!(SameSite::Lax.as_str(), "Lax");
	/// assert_eq!(SameSite::None.as_str(), "None");
	/// ```
	pub fn as_str(&self) -> &'static str {
		match self {
			SameSite::Strict => "Strict",
			SameSite::Lax => "Lax",
			SameSite::None => "None",
		}
	}
}

#[cfg(feature = "middleware")]
/// HTTP session configuration
///
/// Configures how session cookies are created and managed.
///
/// ## Example
///
/// ```rust
/// use reinhardt_auth::sessions::middleware::{HttpSessionConfig, SameSite};
/// use std::time::Duration;
///
/// let config = HttpSessionConfig {
///     cookie_name: "my_session".to_string(),
///     cookie_path: "/api".to_string(),
///     cookie_domain: Some("example.com".to_string()),
///     secure: true,
///     httponly: true,
///     samesite: SameSite::Strict,
///     max_age: Some(Duration::from_secs(7200)),
/// };
/// ```
#[derive(Debug, Clone)]
pub struct HttpSessionConfig {
	/// Name of the session cookie
	pub cookie_name: String,
	/// Path for the cookie
	pub cookie_path: String,
	/// Domain for the cookie (None = current domain)
	pub cookie_domain: Option<String>,
	/// Whether to set the Secure flag (HTTPS only)
	pub secure: bool,
	/// Whether to set the HttpOnly flag (no JavaScript access)
	pub httponly: bool,
	/// SameSite attribute
	pub samesite: SameSite,
	/// Maximum age for the cookie
	pub max_age: Option<Duration>,
}

#[cfg(feature = "middleware")]
impl Default for HttpSessionConfig {
	/// Create default session configuration
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_auth::sessions::middleware::{HttpSessionConfig, SameSite};
	///
	/// let config = HttpSessionConfig::default();
	/// assert_eq!(config.cookie_name, "sessionid");
	/// assert_eq!(config.cookie_path, "/");
	/// assert_eq!(config.samesite, SameSite::Lax);
	/// ```
	fn default() -> Self {
		Self {
			cookie_name: "sessionid".to_string(),
			cookie_path: "/".to_string(),
			cookie_domain: None,
			secure: true,
			httponly: true,
			samesite: SameSite::Lax,
			max_age: None,
		}
	}
}

#[cfg(feature = "middleware")]
/// Session middleware
///
/// Automatically loads sessions from cookies on request and saves them on response.
///
/// ## Example
///
/// ```rust
/// use reinhardt_auth::sessions::middleware::{SessionMiddleware, HttpSessionConfig};
/// use reinhardt_auth::sessions::backends::InMemorySessionBackend;
///
/// let backend = InMemorySessionBackend::new();
/// let config = HttpSessionConfig::default();
/// let middleware = SessionMiddleware::new(backend, config);
/// ```
pub struct SessionMiddleware<B: SessionBackend> {
	backend: B,
	config: HttpSessionConfig,
}

#[cfg(feature = "middleware")]
impl<B: SessionBackend> SessionMiddleware<B> {
	/// Create a new session middleware
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_auth::sessions::middleware::{SessionMiddleware, HttpSessionConfig};
	/// use reinhardt_auth::sessions::backends::InMemorySessionBackend;
	///
	/// let backend = InMemorySessionBackend::new();
	/// let config = HttpSessionConfig::default();
	/// let middleware = SessionMiddleware::new(backend, config);
	/// ```
	pub fn new(backend: B, config: HttpSessionConfig) -> Self {
		Self { backend, config }
	}

	/// Create with default configuration
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_auth::sessions::middleware::SessionMiddleware;
	/// use reinhardt_auth::sessions::backends::InMemorySessionBackend;
	///
	/// let backend = InMemorySessionBackend::new();
	/// let middleware = SessionMiddleware::with_defaults(backend);
	/// ```
	pub fn with_defaults(backend: B) -> Self {
		Self::new(backend, HttpSessionConfig::default())
	}

	/// Extract session key from cookie header
	fn get_session_key_from_cookie(&self, request: &Request) -> Option<String> {
		request.get_language_from_cookie(&self.config.cookie_name)
	}

	/// Build Set-Cookie header value
	fn build_set_cookie_header(&self, session_key: &str) -> String {
		let mut cookie = format!("{}={}", self.config.cookie_name, session_key);

		cookie.push_str(&format!("; Path={}", self.config.cookie_path));

		if let Some(ref domain) = self.config.cookie_domain {
			cookie.push_str(&format!("; Domain={}", domain));
		}

		if let Some(max_age) = self.config.max_age {
			cookie.push_str(&format!("; Max-Age={}", max_age.as_secs()));
		}

		if self.config.secure {
			cookie.push_str("; Secure");
		}

		if self.config.httponly {
			cookie.push_str("; HttpOnly");
		}

		cookie.push_str(&format!("; SameSite={}", self.config.samesite.as_str()));

		cookie
	}
}

#[cfg(feature = "middleware")]
#[async_trait]
impl<B: SessionBackend + 'static> Middleware for SessionMiddleware<B> {
	async fn process(&self, request: Request, next: Arc<dyn Handler>) -> Result<Response> {
		// Load session from cookie
		let session_key = self.get_session_key_from_cookie(&request);

		let session: Session<B> = if let Some(key) = session_key {
			Session::from_key(self.backend.clone(), key)
				.await
				.unwrap_or_else(|_| Session::new(self.backend.clone()))
		} else {
			Session::new(self.backend.clone())
		};

		// Store session in request extensions wrapped in Arc<RwLock> for shared access
		let shared_session = Arc::new(RwLock::new(session));
		request.extensions.insert(shared_session.clone());

		// Process the request
		let mut response = next.handle(request).await?;

		// Save session if modified
		// Acquire read lock to check if modified
		let is_modified = {
			let session_read = shared_session.read().await;
			session_read.is_modified()
		};

		if is_modified {
			// Acquire write lock to save
			let mut session_mut = shared_session.write().await;
			session_mut.save().await.map_err(|e| {
				reinhardt_core::exception::Error::Internal(format!("Failed to save session: {}", e))
			})?;

			// Add Set-Cookie header
			let session_key_str = session_mut.get_or_create_key();
			let cookie_value = self.build_set_cookie_header(session_key_str);

			response = response.with_header("Set-Cookie", &cookie_value);
		}

		Ok(response)
	}
}

#[cfg(all(test, feature = "middleware"))]
mod tests {
	use super::*;
	use crate::sessions::InMemorySessionBackend;
	use bytes::Bytes;
	use hyper::{HeaderMap, Method, StatusCode};
	use std::sync::Arc;

	// Mock handler for testing
	struct MockHandler;

	#[async_trait]
	impl Handler for MockHandler {
		async fn handle(&self, _request: Request) -> Result<Response> {
			Ok(Response::new(StatusCode::OK))
		}
	}

	// Handler that modifies session
	struct SessionModifyingHandler;

	#[async_trait]
	impl Handler for SessionModifyingHandler {
		async fn handle(&self, request: Request) -> Result<Response> {
			// Get the shared session from extensions
			if let Some(shared_session) = request
				.extensions
				.get::<Arc<RwLock<Session<InMemorySessionBackend>>>>()
			{
				// Acquire write lock to modify the session
				let mut session = shared_session.write().await;
				session.set("user_id", 42).unwrap();
				// Lock is automatically released when session goes out of scope
			}
			Ok(Response::new(StatusCode::OK))
		}
	}

	fn create_test_request() -> Request {
		Request::builder()
			.method(Method::GET)
			.uri("/")
			.body(Bytes::new())
			.build()
			.unwrap()
	}

	fn create_test_request_with_cookie(cookie_value: &str) -> Request {
		let mut headers = HeaderMap::new();
		headers.insert("cookie", cookie_value.parse().unwrap());

		Request::builder()
			.method(Method::GET)
			.uri("/")
			.headers(headers)
			.body(Bytes::new())
			.build()
			.unwrap()
	}

	#[tokio::test]
	async fn test_samesite_as_str() {
		assert_eq!(SameSite::Strict.as_str(), "Strict");
		assert_eq!(SameSite::Lax.as_str(), "Lax");
		assert_eq!(SameSite::None.as_str(), "None");
	}

	#[tokio::test]
	async fn test_http_session_config_default() {
		let config = HttpSessionConfig::default();
		assert_eq!(config.cookie_name, "sessionid");
		assert_eq!(config.cookie_path, "/");
		assert!(config.cookie_domain.is_none());
		assert!(config.secure);
		assert!(config.httponly);
		assert_eq!(config.samesite, SameSite::Lax);
		assert!(config.max_age.is_none());
	}

	#[tokio::test]
	async fn test_session_middleware_new() {
		let backend = InMemorySessionBackend::new();
		let config = HttpSessionConfig::default();
		let _middleware = SessionMiddleware::new(backend, config);
	}

	#[tokio::test]
	async fn test_session_middleware_with_defaults() {
		let backend = InMemorySessionBackend::new();
		let _middleware = SessionMiddleware::with_defaults(backend);
	}

	#[tokio::test]
	async fn test_build_set_cookie_header_basic() {
		let backend = InMemorySessionBackend::new();
		let config = HttpSessionConfig::default();
		let middleware = SessionMiddleware::new(backend, config);

		let cookie = middleware.build_set_cookie_header("test_session_key");

		assert!(cookie.contains("sessionid=test_session_key"));
		assert!(cookie.contains("Path=/"));
		assert!(cookie.contains("HttpOnly"));
		assert!(cookie.contains("SameSite=Lax"));
		assert!(cookie.contains("Secure"));
	}

	#[tokio::test]
	async fn test_build_set_cookie_header_with_all_options() {
		let backend = InMemorySessionBackend::new();
		let config = HttpSessionConfig {
			cookie_name: "custom_session".to_string(),
			cookie_path: "/api".to_string(),
			cookie_domain: Some("example.com".to_string()),
			secure: true,
			httponly: true,
			samesite: SameSite::Strict,
			max_age: Some(Duration::from_secs(3600)),
		};
		let middleware = SessionMiddleware::new(backend, config);

		let cookie = middleware.build_set_cookie_header("abc123");

		assert!(cookie.contains("custom_session=abc123"));
		assert!(cookie.contains("Path=/api"));
		assert!(cookie.contains("Domain=example.com"));
		assert!(cookie.contains("Max-Age=3600"));
		assert!(cookie.contains("Secure"));
		assert!(cookie.contains("HttpOnly"));
		assert!(cookie.contains("SameSite=Strict"));
	}

	#[tokio::test]
	async fn test_middleware_creates_new_session_without_cookie() {
		let backend = InMemorySessionBackend::new();
		let middleware = SessionMiddleware::with_defaults(backend);
		let handler = Arc::new(MockHandler);
		let request = create_test_request();

		let response = middleware.process(request, handler).await.unwrap();

		// No session modification, so no Set-Cookie header
		assert!(response.headers.get("set-cookie").is_none());
	}

	#[tokio::test]
	async fn test_middleware_sets_cookie_on_session_modification() {
		let backend = InMemorySessionBackend::new();
		let middleware = SessionMiddleware::with_defaults(backend);
		let handler = Arc::new(SessionModifyingHandler);
		let request = create_test_request();

		let response = middleware.process(request, handler).await.unwrap();

		// Session was modified, should have Set-Cookie header
		let set_cookie = response.headers.get("set-cookie");
		let cookie_value = set_cookie.unwrap().to_str().unwrap();
		assert!(cookie_value.starts_with("sessionid="));
		assert!(cookie_value.contains("Path=/"));
	}

	#[tokio::test]
	async fn test_middleware_loads_existing_session() {
		let backend = InMemorySessionBackend::new();

		// Pre-create a session
		let mut session = Session::new(backend.clone());
		session.set("existing_data", "test_value").unwrap();
		session.save().await.unwrap();
		let session_key = session.session_key().unwrap().to_string();

		let middleware = SessionMiddleware::with_defaults(backend);
		let handler = Arc::new(MockHandler);
		let request = create_test_request_with_cookie(&format!("sessionid={}", session_key));

		let _response = middleware.process(request, handler).await.unwrap();

		// Session should be loaded (we can't easily verify this without extracting it)
		// But at minimum, the middleware should not fail
	}
}
