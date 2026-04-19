//! Session Middleware
//!
//! Provides enhanced session management functionality.
//! Supports various backends including Cookie, Redis, and database.

use async_trait::async_trait;
#[allow(deprecated)]
use reinhardt_conf::Settings;
use reinhardt_di::{DiError, DiResult, Injectable, InjectionContext};
use reinhardt_http::{Handler, Middleware, Request, Response, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, SystemTime};
use uuid::Uuid;

/// Newtype wrapper for session ID stored in request extensions.
///
/// Handlers can retrieve the current session ID from the request
/// extensions without parsing cookies manually.
///
/// # Example
///
/// ```rust,ignore
/// fn handle(&self, request: Request) -> Result<Response> {
///     if let Some(session_id) = request.extensions.get::<SessionId>() {
///         println!("Session: {}", session_id.as_str());
///     }
///     // ...
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionId(String);

impl SessionId {
	/// Create a new `SessionId` from the given string.
	pub fn new(id: String) -> Self {
		Self(id)
	}

	/// Returns the session ID as a string slice.
	pub fn as_str(&self) -> &str {
		&self.0
	}
}

impl AsRef<str> for SessionId {
	fn as_ref(&self) -> &str {
		self.as_str()
	}
}

impl std::fmt::Display for SessionId {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.write_str(self.as_str())
	}
}

/// Newtype wrapper for the configured session cookie name.
///
/// Stored in request extensions by `SessionMiddleware` so that
/// `Injectable` implementations can retrieve the configured cookie name
/// instead of hardcoding it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionCookieName(String);

impl SessionCookieName {
	/// Create a new `SessionCookieName`.
	pub fn new(name: String) -> Self {
		Self(name)
	}

	/// Returns the cookie name as a string slice.
	pub fn as_str(&self) -> &str {
		&self.0
	}
}

/// Session data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionData {
	/// Session ID
	pub id: String,
	/// Data
	pub data: HashMap<String, serde_json::Value>,
	/// Creation timestamp
	pub created_at: SystemTime,
	/// Last access timestamp
	pub last_accessed: SystemTime,
	/// Expiration timestamp
	pub expires_at: SystemTime,
}

impl SessionData {
	/// Create a new session
	pub fn new(ttl: Duration) -> Self {
		let now = SystemTime::now();
		Self {
			id: Uuid::new_v4().to_string(),
			data: HashMap::new(),
			created_at: now,
			last_accessed: now,
			expires_at: now + ttl,
		}
	}

	/// Check if session is valid
	fn is_valid(&self) -> bool {
		SystemTime::now() < self.expires_at
	}

	/// Update last access timestamp
	pub fn touch(&mut self, ttl: Duration) {
		let now = SystemTime::now();
		self.last_accessed = now;
		self.expires_at = now + ttl;
	}

	/// Get a value
	pub fn get<T>(&self, key: &str) -> Option<T>
	where
		T: for<'de> Deserialize<'de>,
	{
		self.data
			.get(key)
			.and_then(|v| serde_json::from_value(v.clone()).ok())
	}

	/// Set a value
	pub fn set<T>(&mut self, key: String, value: T) -> Result<()>
	where
		T: Serialize,
	{
		self.data.insert(
			key,
			serde_json::to_value(value)
				.map_err(|e| reinhardt_core::exception::Error::Serialization(e.to_string()))?,
		);
		Ok(())
	}

	/// Delete a value
	pub fn delete(&mut self, key: &str) {
		self.data.remove(key);
	}

	/// Check if a key exists
	pub fn contains_key(&self, key: &str) -> bool {
		self.data.contains_key(key)
	}

	/// Clear the session
	pub fn clear(&mut self) {
		self.data.clear();
	}
}

/// Session store with automatic lazy eviction of expired sessions
///
/// Performs periodic cleanup of expired sessions to prevent unbounded
/// memory growth. Cleanup runs automatically when the session count
/// exceeds a configurable threshold.
#[derive(Debug, Default)]
pub struct SessionStore {
	/// Sessions
	sessions: RwLock<HashMap<String, SessionData>>,
	/// Maximum number of sessions before triggering automatic cleanup
	max_sessions_before_cleanup: std::sync::atomic::AtomicUsize,
}

impl SessionStore {
	/// Default cleanup threshold: trigger cleanup when session count exceeds 10,000
	const DEFAULT_CLEANUP_THRESHOLD: usize = 10_000;

	/// Create a new store
	pub fn new() -> Self {
		Self {
			sessions: RwLock::new(HashMap::new()),
			max_sessions_before_cleanup: std::sync::atomic::AtomicUsize::new(
				Self::DEFAULT_CLEANUP_THRESHOLD,
			),
		}
	}

	/// Get a session
	pub fn get(&self, id: &str) -> Option<SessionData> {
		let sessions = self.sessions.read().unwrap_or_else(|e| e.into_inner());
		sessions.get(id).cloned()
	}

	/// Save a session, with automatic cleanup when threshold is exceeded
	pub fn save(&self, session: SessionData) {
		let mut sessions = self.sessions.write().unwrap_or_else(|e| e.into_inner());
		sessions.insert(session.id.clone(), session);

		// Lazy eviction: clean up expired sessions when threshold is exceeded
		let threshold = self
			.max_sessions_before_cleanup
			.load(std::sync::atomic::Ordering::Relaxed);
		if sessions.len() > threshold {
			sessions.retain(|_, s| s.is_valid());
		}
	}

	/// Delete a session
	pub fn delete(&self, id: &str) {
		let mut sessions = self.sessions.write().unwrap_or_else(|e| e.into_inner());
		sessions.remove(id);
	}

	/// Clean up expired sessions
	pub fn cleanup(&self) {
		let mut sessions = self.sessions.write().unwrap_or_else(|e| e.into_inner());
		sessions.retain(|_, session| session.is_valid());
	}

	/// Clear the store
	pub fn clear(&self) {
		let mut sessions = self.sessions.write().unwrap_or_else(|e| e.into_inner());
		sessions.clear();
	}

	/// Get the number of sessions
	pub fn len(&self) -> usize {
		let sessions = self.sessions.read().unwrap_or_else(|e| e.into_inner());
		sessions.len()
	}

	/// Check if the store is empty
	pub fn is_empty(&self) -> bool {
		let sessions = self.sessions.read().unwrap_or_else(|e| e.into_inner());
		sessions.is_empty()
	}
}

/// Async trait for pluggable session storage backends.
///
/// Implement this trait to integrate any async-capable session store
/// (e.g. Redis, DynamoDB, PostgreSQL) with the session middleware layer.
///
/// # Example
///
/// ```rust,ignore
/// use std::time::Duration;
/// use reinhardt_middleware::session::{AsyncSessionBackend, SessionData};
/// use reinhardt_http::Result;
///
/// struct MyBackend;
///
/// #[async_trait::async_trait]
/// impl AsyncSessionBackend for MyBackend {
///     async fn load(&self, id: &str) -> Result<Option<SessionData>> { Ok(None) }
///     async fn save(&self, session: &SessionData) -> Result<()> { Ok(()) }
///     async fn destroy(&self, id: &str) -> Result<()> { Ok(()) }
///     async fn touch(&self, id: &str, ttl: Duration) -> Result<()> { Ok(()) }
/// }
/// ```
#[async_trait]
pub trait AsyncSessionBackend: Send + Sync {
	/// Load a session by ID. Returns `None` if the session does not exist
	/// or has expired.
	async fn load(&self, id: &str) -> Result<Option<SessionData>>;

	/// Persist a session (insert or update).
	async fn save(&self, session: &SessionData) -> Result<()>;

	/// Remove a session by ID.
	async fn destroy(&self, id: &str) -> Result<()>;

	/// Refresh the TTL of an existing session without rewriting the full payload.
	async fn touch(&self, id: &str, ttl: Duration) -> Result<()>;
}

/// Session configuration
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct SessionConfig {
	/// Cookie name
	pub cookie_name: String,
	/// Session TTL
	pub ttl: Duration,
	/// HTTPS-only cookie
	pub secure: bool,
	/// HttpOnly flag
	pub http_only: bool,
	/// SameSite attribute
	pub same_site: Option<String>,
	/// Domain
	pub domain: Option<String>,
	/// Path
	pub path: String,
}

impl SessionConfig {
	/// Create a new configuration
	///
	/// # Examples
	///
	/// ```
	/// use std::time::Duration;
	/// use reinhardt_middleware::session::SessionConfig;
	///
	/// let config = SessionConfig::new("sessionid".to_string(), Duration::from_secs(3600));
	/// assert_eq!(config.cookie_name, "sessionid");
	/// assert_eq!(config.ttl, Duration::from_secs(3600));
	/// ```
	pub fn new(cookie_name: String, ttl: Duration) -> Self {
		Self {
			cookie_name,
			ttl,
			secure: true,
			http_only: true,
			same_site: Some("Lax".to_string()),
			domain: None,
			path: "/".to_string(),
		}
	}

	/// Enable secure cookie
	///
	/// # Examples
	///
	/// ```
	/// use std::time::Duration;
	/// use reinhardt_middleware::session::SessionConfig;
	///
	/// let config = SessionConfig::new("sessionid".to_string(), Duration::from_secs(3600))
	///     .with_secure();
	/// assert!(config.secure);
	/// ```
	pub fn with_secure(mut self) -> Self {
		self.secure = true;
		self
	}

	/// Set HttpOnly flag
	///
	/// # Examples
	///
	/// ```
	/// use std::time::Duration;
	/// use reinhardt_middleware::session::SessionConfig;
	///
	/// let config = SessionConfig::new("sessionid".to_string(), Duration::from_secs(3600))
	///     .with_http_only(false);
	/// assert!(!config.http_only);
	/// ```
	pub fn with_http_only(mut self, http_only: bool) -> Self {
		self.http_only = http_only;
		self
	}

	/// Set SameSite attribute
	///
	/// # Examples
	///
	/// ```
	/// use std::time::Duration;
	/// use reinhardt_middleware::session::SessionConfig;
	///
	/// let config = SessionConfig::new("sessionid".to_string(), Duration::from_secs(3600))
	///     .with_same_site("Strict".to_string());
	/// ```
	pub fn with_same_site(mut self, same_site: String) -> Self {
		self.same_site = Some(same_site);
		self
	}

	/// Set domain
	///
	/// # Examples
	///
	/// ```
	/// use std::time::Duration;
	/// use reinhardt_middleware::session::SessionConfig;
	///
	/// let config = SessionConfig::new("sessionid".to_string(), Duration::from_secs(3600))
	///     .with_domain("example.com".to_string());
	/// ```
	pub fn with_domain(mut self, domain: String) -> Self {
		self.domain = Some(domain);
		self
	}

	/// Set path
	///
	/// # Examples
	///
	/// ```
	/// use std::time::Duration;
	/// use reinhardt_middleware::session::SessionConfig;
	///
	/// let config = SessionConfig::new("sessionid".to_string(), Duration::from_secs(3600))
	///     .with_path("/app".to_string());
	/// assert_eq!(config.path, "/app");
	/// ```
	pub fn with_path(mut self, path: String) -> Self {
		self.path = path;
		self
	}

	/// Create a `SessionConfig` from application `Settings`
	///
	/// Maps `Settings.core.security.session_cookie_secure` to `SessionConfig.secure`.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_conf::Settings;
	/// use reinhardt_middleware::session::SessionConfig;
	///
	/// #[allow(deprecated)]
	/// let settings = Settings::default();
	/// #[allow(deprecated)]
	/// let config = SessionConfig::from_settings(&settings);
	/// assert!(!config.secure);
	/// ```
	#[allow(deprecated)] // Settings is deprecated in favor of composable fragments
	pub fn from_settings(settings: &Settings) -> Self {
		Self {
			secure: settings.core.security.session_cookie_secure,
			..Self::default()
		}
	}
}

impl Default for SessionConfig {
	fn default() -> Self {
		Self::new("sessionid".to_string(), Duration::from_secs(3600))
	}
}

/// Session middleware
///
/// # Examples
///
/// ```
/// use std::sync::Arc;
/// use std::time::Duration;
/// use reinhardt_middleware::session::{SessionMiddleware, SessionConfig};
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
/// let config = SessionConfig::new("sessionid".to_string(), Duration::from_secs(3600));
/// let middleware = SessionMiddleware::new(config);
/// let handler = Arc::new(TestHandler);
///
/// let request = Request::builder()
///     .method(Method::GET)
///     .uri("/api/data")
///     .version(Version::HTTP_11)
///     .headers(HeaderMap::new())
///     .body(Bytes::new())
///     .build()
///     .unwrap();
///
/// let response = middleware.process(request, handler).await.unwrap();
/// assert_eq!(response.status, StatusCode::OK);
/// # });
/// ```
pub struct SessionMiddleware {
	config: SessionConfig,
	store: Arc<SessionStore>,
}

impl SessionMiddleware {
	/// Create a new session middleware
	///
	/// # Examples
	///
	/// ```
	/// use std::time::Duration;
	/// use reinhardt_middleware::session::{SessionMiddleware, SessionConfig};
	///
	/// let config = SessionConfig::new("sessionid".to_string(), Duration::from_secs(3600));
	/// let middleware = SessionMiddleware::new(config);
	/// ```
	pub fn new(config: SessionConfig) -> Self {
		Self {
			config,
			store: Arc::new(SessionStore::new()),
		}
	}

	/// Create a `SessionMiddleware` from application `Settings`
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_conf::Settings;
	/// use reinhardt_middleware::session::SessionMiddleware;
	///
	/// #[allow(deprecated)]
	/// let settings = Settings::default();
	/// #[allow(deprecated)]
	/// let middleware = SessionMiddleware::from_settings(&settings);
	/// ```
	#[allow(deprecated)] // Settings is deprecated in favor of composable fragments
	pub fn from_settings(settings: &Settings) -> Self {
		Self::new(SessionConfig::from_settings(settings))
	}

	/// Create with default configuration
	pub fn with_defaults() -> Self {
		Self::new(SessionConfig::default())
	}

	/// Create from an existing Arc-wrapped session store
	///
	/// This is provided for cases where you already have an `Arc<SessionStore>`.
	/// In most cases, you should use `new()` instead, which creates the store internally.
	pub fn from_arc(config: SessionConfig, store: Arc<SessionStore>) -> Self {
		Self { config, store }
	}

	/// Get a reference to the session store
	///
	/// # Examples
	///
	/// ```
	/// use std::time::Duration;
	/// use reinhardt_middleware::session::{SessionMiddleware, SessionConfig};
	///
	/// let middleware = SessionMiddleware::new(
	///     SessionConfig::new("sessionid".to_string(), Duration::from_secs(3600))
	/// );
	///
	/// // Access the store
	/// let store = middleware.store();
	/// assert_eq!(store.len(), 0);
	/// ```
	pub fn store(&self) -> &SessionStore {
		&self.store
	}

	/// Get a cloned Arc of the store (for cases where you need ownership)
	///
	/// In most cases, you should use `store()` instead to get a reference.
	pub fn store_arc(&self) -> Arc<SessionStore> {
		Arc::clone(&self.store)
	}

	/// Get session ID from request
	fn get_session_id(&self, request: &Request) -> Option<String> {
		if let Some(cookie_header) = request.headers.get(hyper::header::COOKIE)
			&& let Ok(cookie_str) = cookie_header.to_str()
		{
			for cookie in cookie_str.split(';') {
				let parts: Vec<&str> = cookie.trim().splitn(2, '=').collect();
				if parts.len() == 2 && parts[0] == self.config.cookie_name {
					return Some(parts[1].to_string());
				}
			}
		}
		None
	}

	/// Build Set-Cookie header
	fn build_cookie_header(&self, session_id: &str) -> String {
		let mut parts = vec![format!("{}={}", self.config.cookie_name, session_id)];

		parts.push(format!("Path={}", self.config.path));

		if let Some(domain) = &self.config.domain {
			parts.push(format!("Domain={}", domain));
		}

		if self.config.http_only {
			parts.push("HttpOnly".to_string());
		}

		if self.config.secure {
			parts.push("Secure".to_string());
		}

		if let Some(same_site) = &self.config.same_site {
			parts.push(format!("SameSite={}", same_site));
		}

		parts.push(format!("Max-Age={}", self.config.ttl.as_secs()));

		parts.join("; ")
	}
}

impl Default for SessionMiddleware {
	fn default() -> Self {
		Self::with_defaults()
	}
}

#[async_trait]
impl Middleware for SessionMiddleware {
	async fn process(&self, request: Request, handler: Arc<dyn Handler>) -> Result<Response> {
		// Get or generate session ID
		let session_id = self.get_session_id(&request);
		let mut session = if let Some(id) = session_id.clone() {
			self.store
				.get(&id)
				.filter(|s| s.is_valid())
				.unwrap_or_else(|| SessionData::new(self.config.ttl))
		} else {
			SessionData::new(self.config.ttl)
		};

		// Touch the session
		session.touch(self.config.ttl);

		// Save the session
		self.store.save(session.clone());

		// Inject session ID and cookie name into request extensions
		// so downstream handlers and Injectable impls can access them
		request
			.extensions
			.insert(SessionId::new(session.id.clone()));
		request
			.extensions
			.insert(SessionCookieName::new(self.config.cookie_name.clone()));

		// Call the handler
		// Convert errors to responses so post-processing (e.g., security headers)
		// always runs, even when invoked outside MiddlewareChain. (#3244)
		let mut response = match handler.handle(request).await {
			Ok(resp) => resp,
			Err(e) => Response::from(e),
		};

		// Append Set-Cookie header (use append to preserve existing Set-Cookie headers)
		let cookie = self.build_cookie_header(&session.id);
		response.headers.append(
			hyper::header::SET_COOKIE,
			hyper::header::HeaderValue::from_str(&cookie).map_err(|e| {
				reinhardt_core::exception::Error::Internal(format!(
					"Failed to create cookie header: {}",
					e
				))
			})?,
		);

		Ok(response)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use bytes::Bytes;
	use hyper::{HeaderMap, Method, StatusCode, Version};
	use std::thread;

	struct TestHandler;

	#[async_trait]
	impl Handler for TestHandler {
		async fn handle(&self, _request: Request) -> Result<Response> {
			Ok(Response::new(StatusCode::OK).with_body(Bytes::from("OK")))
		}
	}

	#[tokio::test]
	async fn test_session_creation() {
		let config = SessionConfig::new("sessionid".to_string(), Duration::from_secs(3600));
		let middleware = SessionMiddleware::new(config);
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
		assert!(response.headers.contains_key("set-cookie"));

		let cookie = response
			.headers
			.get("set-cookie")
			.unwrap()
			.to_str()
			.unwrap();
		assert!(cookie.starts_with("sessionid="));
	}

	#[tokio::test]
	async fn test_session_persistence() {
		let config = SessionConfig::new("sessionid".to_string(), Duration::from_secs(3600));
		let middleware = Arc::new(SessionMiddleware::new(config));
		let handler = Arc::new(TestHandler);

		// First request
		let request1 = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();
		let response1 = middleware.process(request1, handler.clone()).await.unwrap();
		let cookie1 = response1
			.headers
			.get("set-cookie")
			.unwrap()
			.to_str()
			.unwrap();

		// Extract session ID
		let session_id = cookie1
			.split(';')
			.next()
			.unwrap()
			.split('=')
			.nth(1)
			.unwrap();

		// Second request (with same session ID)
		let mut headers = HeaderMap::new();
		headers.insert(
			hyper::header::COOKIE,
			hyper::header::HeaderValue::from_str(&format!("sessionid={}", session_id)).unwrap(),
		);
		let request2 = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(headers)
			.body(Bytes::new())
			.build()
			.unwrap();
		let response2 = middleware.process(request2, handler).await.unwrap();

		assert_eq!(response2.status, StatusCode::OK);

		// Same session ID should be returned
		let cookie2 = response2
			.headers
			.get("set-cookie")
			.unwrap()
			.to_str()
			.unwrap();
		assert!(cookie2.contains(session_id));
	}

	#[tokio::test]
	async fn test_session_expiration() {
		let config = SessionConfig::new("sessionid".to_string(), Duration::from_millis(100));
		let middleware = Arc::new(SessionMiddleware::new(config));
		let handler = Arc::new(TestHandler);

		// First request
		let request1 = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();
		let response1 = middleware.process(request1, handler.clone()).await.unwrap();
		let cookie1 = response1
			.headers
			.get("set-cookie")
			.unwrap()
			.to_str()
			.unwrap();
		let session_id1 = cookie1
			.split(';')
			.next()
			.unwrap()
			.split('=')
			.nth(1)
			.unwrap();

		// Wait until expiration
		thread::sleep(Duration::from_millis(150));

		// Request after expiration
		let mut headers = HeaderMap::new();
		headers.insert(
			hyper::header::COOKIE,
			hyper::header::HeaderValue::from_str(&format!("sessionid={}", session_id1)).unwrap(),
		);
		let request2 = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(headers)
			.body(Bytes::new())
			.build()
			.unwrap();
		let response2 = middleware.process(request2, handler).await.unwrap();

		// New session ID should be created
		let cookie2 = response2
			.headers
			.get("set-cookie")
			.unwrap()
			.to_str()
			.unwrap();
		let session_id2 = cookie2
			.split(';')
			.next()
			.unwrap()
			.split('=')
			.nth(1)
			.unwrap();

		assert_ne!(session_id1, session_id2);
	}

	#[tokio::test]
	async fn test_cookie_attributes() {
		let config = SessionConfig::new("sessionid".to_string(), Duration::from_secs(3600))
			.with_secure()
			.with_http_only(true)
			.with_same_site("Strict".to_string())
			.with_path("/app".to_string());
		let middleware = SessionMiddleware::new(config);
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

		let cookie = response
			.headers
			.get("set-cookie")
			.unwrap()
			.to_str()
			.unwrap();
		assert!(cookie.contains("Secure"));
		assert!(cookie.contains("HttpOnly"));
		assert!(cookie.contains("SameSite=Strict"));
		assert!(cookie.contains("Path=/app"));
	}

	#[tokio::test]
	async fn test_session_data() {
		let mut session = SessionData::new(Duration::from_secs(3600));

		session.set("user_id".to_string(), 123).unwrap();
		session
			.set("username".to_string(), "alice".to_string())
			.unwrap();

		let user_id: i32 = session.get("user_id").unwrap();
		assert_eq!(user_id, 123);

		let username: String = session.get("username").unwrap();
		assert_eq!(username, "alice");

		assert!(session.contains_key("user_id"));
		assert!(!session.contains_key("email"));

		session.delete("username");
		assert!(!session.contains_key("username"));
	}

	#[tokio::test]
	async fn test_session_store() {
		let store = SessionStore::new();

		let session1 = SessionData::new(Duration::from_secs(3600));
		let id1 = session1.id.clone();
		store.save(session1);

		let session2 = SessionData::new(Duration::from_secs(3600));
		let id2 = session2.id.clone();
		store.save(session2);

		assert_eq!(store.len(), 2);
		assert!(!store.is_empty());

		let retrieved1 = store.get(&id1).unwrap();
		assert_eq!(retrieved1.id, id1);

		store.delete(&id1);
		assert_eq!(store.len(), 1);
		assert!(store.get(&id1).is_none());
		assert!(store.get(&id2).is_some());
	}

	#[tokio::test]
	async fn test_session_cleanup() {
		let store = SessionStore::new();

		let mut session1 = SessionData::new(Duration::from_millis(10));
		session1.expires_at = SystemTime::now() - Duration::from_millis(20);
		store.save(session1);

		let session2 = SessionData::new(Duration::from_secs(3600));
		let id2 = session2.id.clone();
		store.save(session2);

		store.cleanup();

		assert_eq!(store.len(), 1);
		assert!(store.get(&id2).is_some());
	}

	#[tokio::test]
	async fn test_with_defaults_constructor() {
		let middleware = SessionMiddleware::with_defaults();
		let handler = Arc::new(TestHandler);

		let request = Request::builder()
			.method(Method::GET)
			.uri("/page")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();

		assert_eq!(response.status, StatusCode::OK);
		assert!(response.headers.contains_key("set-cookie"));

		let cookie = response
			.headers
			.get("set-cookie")
			.unwrap()
			.to_str()
			.unwrap();
		// Default cookie name should be "sessionid"
		assert!(cookie.starts_with("sessionid="));
		// Default path should be "/"
		assert!(cookie.contains("Path=/"));
	}

	#[tokio::test]
	async fn test_custom_cookie_name() {
		let config = SessionConfig::new("my_session".to_string(), Duration::from_secs(3600));
		let middleware = SessionMiddleware::new(config);
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

		let cookie = response
			.headers
			.get("set-cookie")
			.unwrap()
			.to_str()
			.unwrap();
		// Custom cookie name should be used
		assert!(cookie.starts_with("my_session="));
		assert!(!cookie.starts_with("sessionid="));
	}

	#[rstest::rstest]
	#[tokio::test]
	async fn test_session_config_from_settings_secure_enabled() {
		// Arrange
		#[allow(deprecated)]
		let mut settings = Settings::new(std::path::PathBuf::from("/app"), "test-secret".to_string());
		settings.core.security.session_cookie_secure = true;

		// Act
		#[allow(deprecated)]
		let config = SessionConfig::from_settings(&settings);

		// Assert
		assert_eq!(config.secure, true);
	}

	#[rstest::rstest]
	#[tokio::test]
	async fn test_session_config_from_settings_defaults() {
		// Arrange
		#[allow(deprecated)]
		let settings = Settings::default();

		// Act
		#[allow(deprecated)]
		let config = SessionConfig::from_settings(&settings);

		// Assert
		assert_eq!(config.secure, false);
		assert_eq!(config.cookie_name, "sessionid");
		assert_eq!(config.ttl, Duration::from_secs(3600));
	}

	#[rstest::rstest]
	#[tokio::test]
	async fn test_session_middleware_from_settings() {
		// Arrange
		#[allow(deprecated)]
		let mut settings = Settings::new(std::path::PathBuf::from("/app"), "test-secret".to_string());
		settings.core.security.session_cookie_secure = true;
		#[allow(deprecated)]
		let middleware = SessionMiddleware::from_settings(&settings);
		let handler = Arc::new(TestHandler);

		let request = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		// Act
		let response = middleware.process(request, handler).await.unwrap();

		// Assert
		assert_eq!(response.status, StatusCode::OK);
		let cookie = response
			.headers
			.get("set-cookie")
			.unwrap()
			.to_str()
			.unwrap();
		assert!(cookie.contains("Secure"));
	}

	#[rstest::rstest]
	fn test_rwlock_poison_recovery_session_store() {
		// Arrange
		let store = Arc::new(SessionStore::new());
		let session = SessionData::new(Duration::from_secs(3600));
		let session_id = session.id.clone();
		store.save(session);

		// Act - poison the RwLock by panicking while holding a write guard
		let store_clone = Arc::clone(&store);
		let _ = thread::spawn(move || {
			let _guard = store_clone.sessions.write().unwrap();
			panic!("intentional panic to poison lock");
		})
		.join();

		// Assert - operations still work after poison recovery
		assert!(store.get(&session_id).is_some());
		assert_eq!(store.len(), 1);
		assert!(!store.is_empty());
		store.delete(&session_id);
		assert_eq!(store.len(), 0);
	}

	/// Handler that captures the session ID from request extensions
	struct SessionIdCapturingHandler {
		captured: Arc<RwLock<Option<SessionId>>>,
	}

	#[async_trait]
	impl Handler for SessionIdCapturingHandler {
		async fn handle(&self, request: Request) -> Result<Response> {
			// Capture session ID from extensions
			let session_id = request.extensions.get::<SessionId>();
			let mut guard = self.captured.write().unwrap();
			*guard = session_id;
			Ok(Response::new(StatusCode::OK).with_body(Bytes::from("OK")))
		}
	}

	#[rstest::rstest]
	#[tokio::test]
	async fn test_session_id_injected_into_request_extensions() {
		// Arrange
		let config = SessionConfig::new("sessionid".to_string(), Duration::from_secs(3600));
		let middleware = SessionMiddleware::new(config);
		let captured = Arc::new(RwLock::new(None));
		let handler = Arc::new(SessionIdCapturingHandler {
			captured: Arc::clone(&captured),
		});

		let request = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		// Act
		let _response = middleware.process(request, handler).await.unwrap();

		// Assert - handler received request with session ID in extensions
		let guard = captured.read().unwrap();
		let session_id = guard
			.as_ref()
			.expect("SessionId should be present in extensions");
		assert!(
			!session_id.as_str().is_empty(),
			"Session ID should not be empty"
		);
	}

	#[rstest::rstest]
	#[tokio::test]
	async fn test_session_id_in_extensions_matches_cookie() {
		// Arrange
		let config = SessionConfig::new("sessionid".to_string(), Duration::from_secs(3600));
		let middleware = SessionMiddleware::new(config);
		let captured = Arc::new(RwLock::new(None));
		let handler = Arc::new(SessionIdCapturingHandler {
			captured: Arc::clone(&captured),
		});

		let request = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		// Act
		let response = middleware.process(request, handler).await.unwrap();

		// Assert - session ID in extensions matches the one in Set-Cookie header
		let guard = captured.read().unwrap();
		let session_id = guard.as_ref().expect("SessionId should be present");

		let cookie = response
			.headers
			.get("set-cookie")
			.unwrap()
			.to_str()
			.unwrap();
		let cookie_session_id = cookie.split(';').next().unwrap().split('=').nth(1).unwrap();

		assert_eq!(session_id.as_str(), cookie_session_id);
	}

	#[rstest::rstest]
	#[tokio::test]
	async fn test_session_id_in_extensions_preserved_for_existing_session() {
		// Arrange
		let config = SessionConfig::new("sessionid".to_string(), Duration::from_secs(3600));
		let middleware = Arc::new(SessionMiddleware::new(config));
		let captured = Arc::new(RwLock::new(None));

		// First request to create session
		let handler1 = Arc::new(TestHandler);
		let request1 = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();
		let response1 = middleware.process(request1, handler1).await.unwrap();
		let cookie = response1
			.headers
			.get("set-cookie")
			.unwrap()
			.to_str()
			.unwrap();
		let original_session_id = cookie
			.split(';')
			.next()
			.unwrap()
			.split('=')
			.nth(1)
			.unwrap()
			.to_string();

		// Second request with existing session cookie
		let handler2 = Arc::new(SessionIdCapturingHandler {
			captured: Arc::clone(&captured),
		});
		let mut headers = HeaderMap::new();
		headers.insert(
			hyper::header::COOKIE,
			hyper::header::HeaderValue::from_str(&format!("sessionid={}", original_session_id))
				.unwrap(),
		);
		let request2 = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(headers)
			.body(Bytes::new())
			.build()
			.unwrap();

		// Act
		let _response2 = middleware.process(request2, handler2).await.unwrap();

		// Assert - session ID in extensions matches the original session
		let guard = captured.read().unwrap();
		let session_id = guard.as_ref().expect("SessionId should be present");
		assert_eq!(session_id.as_str(), original_session_id);
	}

	/// Handler that captures the cookie name from request extensions
	struct CookieNameCapturingHandler {
		captured: Arc<RwLock<Option<SessionCookieName>>>,
	}

	#[async_trait]
	impl Handler for CookieNameCapturingHandler {
		async fn handle(&self, request: Request) -> Result<Response> {
			let cookie_name = request.extensions.get::<SessionCookieName>();
			let mut guard = self.captured.write().unwrap();
			*guard = cookie_name;
			Ok(Response::new(StatusCode::OK).with_body(Bytes::from("OK")))
		}
	}

	#[rstest::rstest]
	#[tokio::test]
	async fn test_session_cookie_name_injected_into_extensions() {
		// Arrange
		let config = SessionConfig::new("custom_session".to_string(), Duration::from_secs(3600));
		let middleware = SessionMiddleware::new(config);
		let captured = Arc::new(RwLock::new(None));
		let handler = Arc::new(CookieNameCapturingHandler {
			captured: Arc::clone(&captured),
		});

		let request = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		// Act
		let _response = middleware.process(request, handler).await.unwrap();

		// Assert - handler received the configured cookie name in extensions
		let guard = captured.read().unwrap();
		let cookie_name = guard
			.as_ref()
			.expect("SessionCookieName should be present in extensions");
		assert_eq!(
			cookie_name.as_str(),
			"custom_session",
			"Cookie name should match configured value, not hardcoded 'sessionid'"
		);
	}

	/// Handler that returns a response with an existing Set-Cookie header
	struct HandlerWithSetCookie;

	#[async_trait]
	impl Handler for HandlerWithSetCookie {
		async fn handle(&self, _request: Request) -> Result<Response> {
			let mut response = Response::new(StatusCode::OK).with_body(Bytes::from("OK"));
			response.headers.insert(
				hyper::header::SET_COOKIE,
				hyper::header::HeaderValue::from_static("csrftoken=xyz789; Path=/"),
			);
			Ok(response)
		}
	}

	#[rstest::rstest]
	#[tokio::test]
	async fn test_session_set_cookie_appends_not_replaces() {
		// Arrange
		let config = SessionConfig::new("sessionid".to_string(), Duration::from_secs(3600));
		let middleware = SessionMiddleware::new(config);
		let handler = Arc::new(HandlerWithSetCookie);

		let request = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		// Act
		let response = middleware.process(request, handler).await.unwrap();

		// Assert - both Set-Cookie headers should be present
		let set_cookies: Vec<&hyper::header::HeaderValue> = response
			.headers
			.get_all(hyper::header::SET_COOKIE)
			.iter()
			.collect();
		assert_eq!(
			set_cookies.len(),
			2,
			"Expected both the original CSRF cookie and session cookie"
		);

		let cookies_str: Vec<&str> = set_cookies.iter().map(|v| v.to_str().unwrap()).collect();
		assert!(
			cookies_str.iter().any(|c| c.contains("csrftoken=xyz789")),
			"Original Set-Cookie header should be preserved"
		);
		assert!(
			cookies_str.iter().any(|c| c.contains("sessionid=")),
			"Session Set-Cookie header should be appended"
		);
	}
}

// ============================================================================
// Injectable Implementations for Dependency Injection
// ============================================================================

/// Default session cookie name used when no `SessionCookieName` extension is present.
const DEFAULT_SESSION_COOKIE_NAME: &str = "sessionid";

/// Helper function to extract session ID from HTTP request cookies.
///
/// Searches for a cookie with the specified name in the Cookie header.
///
/// # Arguments
///
/// * `request` - The HTTP request to extract the session ID from
/// * `cookie_name` - The name of the session cookie (e.g., "sessionid")
///
/// # Returns
///
/// * `Ok(String)` - The session ID if found and valid
/// * `Err(DiError)` - If the cookie header is missing, invalid, or the session cookie is not found
fn extract_session_id_from_request(request: &Request, cookie_name: &str) -> DiResult<String> {
	let cookie_header = request
		.headers
		.get(hyper::header::COOKIE)
		.ok_or_else(|| DiError::NotFound("Cookie header not found".to_string()))?;

	let cookie_str = cookie_header
		.to_str()
		.map_err(|e| DiError::ProviderError(format!("Invalid cookie header: {}", e)))?;

	for cookie in cookie_str.split(';') {
		let parts: Vec<&str> = cookie.trim().splitn(2, '=').collect();
		if parts.len() == 2 && parts[0] == cookie_name {
			return Ok(parts[1].to_string());
		}
	}

	Err(DiError::NotFound(format!(
		"Session cookie '{}' not found",
		cookie_name
	)))
}

#[async_trait]
impl Injectable for SessionData {
	async fn inject(ctx: &InjectionContext) -> DiResult<Self> {
		// Get SessionStore from SingletonScope
		let store = ctx.get_singleton::<Arc<SessionStore>>().ok_or_else(|| {
			DiError::NotFound(
				"SessionStore not found in SingletonScope. \
                     Ensure SessionMiddleware is configured and its store is registered."
					.to_string(),
			)
		})?;

		// Get Request from context
		let request = ctx.get_request::<Request>().ok_or_else(|| {
			DiError::NotFound("Request not found in InjectionContext".to_string())
		})?;

		// Extract configured cookie name from request extensions.
		// Extensions::get returns an owned value, so we extract it once and
		// use a reference for the lookup to avoid additional allocation.
		let ext_cookie_name = request.extensions.get::<SessionCookieName>();
		let cookie_name = ext_cookie_name
			.as_ref()
			.map(|cn| cn.as_str())
			.unwrap_or(DEFAULT_SESSION_COOKIE_NAME);

		// Extract session ID from Cookie header
		let session_id = extract_session_id_from_request(&request, cookie_name)?;

		// Load SessionData from store
		store
			.get(&session_id)
			.filter(|s| s.is_valid())
			.ok_or_else(|| {
				DiError::NotFound("Valid session not found. Session may have expired.".to_string())
			})
	}
}

/// Wrapper for `Arc<SessionStore>` to enable dependency injection
///
/// This wrapper type is necessary because we cannot implement Injectable
/// for `Arc<SessionStore>` directly due to Rust's orphan rules.
#[derive(Clone)]
pub struct SessionStoreRef(pub Arc<SessionStore>);

impl SessionStoreRef {
	/// Get a reference to the inner SessionStore
	pub fn inner(&self) -> &SessionStore {
		&self.0
	}

	/// Get a clone of the inner `Arc<SessionStore>`
	pub fn arc(&self) -> Arc<SessionStore> {
		Arc::clone(&self.0)
	}
}

#[async_trait]
impl Injectable for SessionStoreRef {
	async fn inject(ctx: &InjectionContext) -> DiResult<Self> {
		ctx.get_singleton::<Arc<SessionStore>>()
			.map(|arc_store| SessionStoreRef(Arc::clone(&*arc_store)))
			.ok_or_else(|| {
				DiError::NotFound(
					"SessionStore not found in SingletonScope. \
                     Ensure SessionMiddleware is configured and its store is registered."
						.to_string(),
				)
			})
	}
}

#[cfg(test)]
mod async_backend_tests {
	use super::*;
	use std::collections::HashMap;
	use std::sync::{Arc, RwLock};

	/// In-memory MockBackend for testing `AsyncSessionBackend`.
	struct MockBackend {
		sessions: RwLock<HashMap<String, SessionData>>,
	}

	impl MockBackend {
		fn new() -> Self {
			Self {
				sessions: RwLock::new(HashMap::new()),
			}
		}
	}

	#[async_trait]
	impl AsyncSessionBackend for MockBackend {
		async fn load(&self, id: &str) -> Result<Option<SessionData>> {
			let sessions = self.sessions.read().unwrap_or_else(|e| e.into_inner());
			Ok(sessions.get(id).cloned())
		}

		async fn save(&self, session: &SessionData) -> Result<()> {
			let mut sessions = self.sessions.write().unwrap_or_else(|e| e.into_inner());
			sessions.insert(session.id.clone(), session.clone());
			Ok(())
		}

		async fn destroy(&self, id: &str) -> Result<()> {
			let mut sessions = self.sessions.write().unwrap_or_else(|e| e.into_inner());
			sessions.remove(id);
			Ok(())
		}

		async fn touch(&self, id: &str, ttl: Duration) -> Result<()> {
			let mut sessions = self.sessions.write().unwrap_or_else(|e| e.into_inner());
			if let Some(session) = sessions.get_mut(id) {
				session.touch(ttl);
			}
			Ok(())
		}
	}

	#[tokio::test]
	async fn test_mock_backend_load_nonexistent() {
		let backend = MockBackend::new();
		let result = backend.load("nonexistent-id").await.unwrap();
		assert!(result.is_none());
	}

	#[tokio::test]
	async fn test_mock_backend_save_and_load() {
		let backend = MockBackend::new();
		let session = SessionData::new(Duration::from_secs(3600));
		let id = session.id.clone();

		backend.save(&session).await.unwrap();

		let loaded = backend.load(&id).await.unwrap();
		assert!(loaded.is_some());
		assert_eq!(loaded.unwrap().id, id);
	}

	#[tokio::test]
	async fn test_mock_backend_save_overwrites() {
		let backend = MockBackend::new();
		let mut session = SessionData::new(Duration::from_secs(3600));
		let id = session.id.clone();

		backend.save(&session).await.unwrap();

		// Update a value and save again
		session.set("key".to_string(), "value").unwrap();
		backend.save(&session).await.unwrap();

		let loaded = backend.load(&id).await.unwrap().unwrap();
		let val: String = loaded.get("key").unwrap();
		assert_eq!(val, "value");
	}

	#[tokio::test]
	async fn test_mock_backend_destroy() {
		let backend = MockBackend::new();
		let session = SessionData::new(Duration::from_secs(3600));
		let id = session.id.clone();

		backend.save(&session).await.unwrap();
		assert!(backend.load(&id).await.unwrap().is_some());

		backend.destroy(&id).await.unwrap();
		assert!(backend.load(&id).await.unwrap().is_none());
	}

	#[tokio::test]
	async fn test_mock_backend_destroy_nonexistent_is_ok() {
		let backend = MockBackend::new();
		// Destroying a session that doesn't exist should not return an error
		let result = backend.destroy("ghost-id").await;
		assert!(result.is_ok());
	}

	#[tokio::test]
	async fn test_mock_backend_touch_updates_expiry() {
		let backend = MockBackend::new();
		let session = SessionData::new(Duration::from_secs(3600));
		let id = session.id.clone();
		let original_expires = session.expires_at;

		backend.save(&session).await.unwrap();

		// Touch with a longer TTL
		backend.touch(&id, Duration::from_secs(7200)).await.unwrap();

		let loaded = backend.load(&id).await.unwrap().unwrap();
		assert!(
			loaded.expires_at > original_expires,
			"expires_at should be extended after touch"
		);
	}

	#[tokio::test]
	async fn test_mock_backend_touch_nonexistent_is_ok() {
		let backend = MockBackend::new();
		// Touching a non-existent session is a no-op (not an error)
		let result = backend.touch("ghost-id", Duration::from_secs(3600)).await;
		assert!(result.is_ok());
	}

	#[tokio::test]
	async fn test_backend_dyn_dispatch() {
		// Verify the trait is object-safe and usable via Arc<dyn AsyncSessionBackend>
		let backend: Arc<dyn AsyncSessionBackend> = Arc::new(MockBackend::new());
		let session = SessionData::new(Duration::from_secs(3600));
		let id = session.id.clone();

		backend.save(&session).await.unwrap();
		let loaded = backend.load(&id).await.unwrap();
		assert!(loaded.is_some());

		backend.touch(&id, Duration::from_secs(1800)).await.unwrap();
		backend.destroy(&id).await.unwrap();
		assert!(backend.load(&id).await.unwrap().is_none());
	}
}
