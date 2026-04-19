//! Cookie-based session authentication middleware.
//!
//! Reads a session ID from an HTTP cookie, validates it against an
//! `AsyncSessionBackend`, and injects `AuthState` into request
//! extensions.
//!
//! This middleware uses **best-effort authentication**: valid sessions
//! produce `AuthState::authenticated()`, while missing or invalid
//! sessions produce `AuthState::anonymous()`. Requests are never
//! rejected — authorization is delegated to endpoint-level guards.

#[cfg(feature = "sessions")]
use async_trait::async_trait;
#[cfg(feature = "sessions")]
use std::sync::Arc;
#[cfg(feature = "sessions")]
use std::time::{Duration, SystemTime};

#[cfg(feature = "sessions")]
use crate::session::AsyncSessionBackend;
#[cfg(feature = "sessions")]
use reinhardt_http::{
	AuthState, Handler, IsActive, IsAdmin, IsAuthenticated, Middleware, Request, Response, Result,
};

/// Configuration for cookie-based session authentication.
#[cfg(feature = "sessions")]
#[derive(Debug, Clone)]
pub struct CookieSessionConfig {
	/// Cookie name to read the session ID from.
	pub cookie_name: String,
	/// Sliding TTL: session expiry is extended by this amount on each request.
	pub sliding_ttl: Duration,
	/// Absolute maximum lifetime from session creation.
	pub absolute_max: Duration,
	/// Whether the cookie requires HTTPS.
	pub secure: bool,
	/// SameSite attribute value.
	pub same_site: String,
	/// Paths that skip authentication entirely.
	pub skip_paths: Vec<String>,
}

#[cfg(feature = "sessions")]
impl Default for CookieSessionConfig {
	fn default() -> Self {
		Self {
			cookie_name: "sessionid".to_string(),
			sliding_ttl: Duration::from_secs(30 * 60),
			absolute_max: Duration::from_secs(24 * 60 * 60),
			secure: true,
			same_site: "Lax".to_string(),
			skip_paths: Vec::new(),
		}
	}
}

/// Middleware that authenticates requests via a session cookie.
///
/// On each request the middleware:
/// 1. Extracts the session ID from the configured cookie.
/// 2. Loads the session from the `AsyncSessionBackend`.
/// 3. Checks absolute expiry (`created_at + absolute_max`).
/// 4. Builds an `AuthState` (authenticated or anonymous) and inserts
///    it into request extensions.
/// 5. After the downstream handler responds, fires a background `touch()`
///    to refresh the sliding TTL.
///
/// # Examples
///
/// ```rust,no_run
/// use std::sync::Arc;
/// use reinhardt_middleware::CookieSessionAuthMiddleware;
/// use reinhardt_middleware::session::{AsyncSessionBackend, SessionData};
/// use reinhardt_http::Result;
/// use std::time::Duration;
///
/// # struct MyBackend;
/// # #[async_trait::async_trait]
/// # impl AsyncSessionBackend for MyBackend {
/// #     async fn load(&self, _id: &str) -> Result<Option<SessionData>> { Ok(None) }
/// #     async fn save(&self, _s: &SessionData) -> Result<()> { Ok(()) }
/// #     async fn destroy(&self, _id: &str) -> Result<()> { Ok(()) }
/// #     async fn touch(&self, _id: &str, _ttl: Duration) -> Result<()> { Ok(()) }
/// # }
///
/// let backend = Arc::new(MyBackend);
/// let mw = CookieSessionAuthMiddleware::new(backend);
/// ```
#[cfg(feature = "sessions")]
pub struct CookieSessionAuthMiddleware<B: AsyncSessionBackend> {
	backend: Arc<B>,
	config: CookieSessionConfig,
}

#[cfg(feature = "sessions")]
impl<B: AsyncSessionBackend> CookieSessionAuthMiddleware<B> {
	/// Create a new middleware with the given backend and default config.
	pub fn new(backend: Arc<B>) -> Self {
		Self {
			backend,
			config: CookieSessionConfig::default(),
		}
	}

	/// Create a new middleware with the given backend and custom config.
	pub fn with_config(backend: Arc<B>, config: CookieSessionConfig) -> Self {
		Self { backend, config }
	}

	/// Extract the session ID from the `Cookie` header.
	fn extract_session_id(request: &Request, cookie_name: &str) -> Option<String> {
		request
			.headers
			.get("Cookie")
			.and_then(|v| v.to_str().ok())
			.and_then(|cookies| {
				cookies.split(';').find_map(|pair| {
					let pair = pair.trim();
					let (name, value) = pair.split_once('=')?;
					if name.trim() == cookie_name {
						Some(value.trim().to_string())
					} else {
						None
					}
				})
			})
	}

	/// Check whether the given path should skip authentication.
	fn is_skip_path(path: &str, skip_paths: &[String]) -> bool {
		skip_paths.iter().any(|skip| {
			if skip.ends_with('/') {
				// Prefix match
				path.starts_with(skip.as_str())
			} else {
				// Exact match
				path == skip
			}
		})
	}
}

#[cfg(feature = "sessions")]
#[async_trait]
impl<B: AsyncSessionBackend + 'static> Middleware for CookieSessionAuthMiddleware<B> {
	async fn process(&self, request: Request, next: Arc<dyn Handler>) -> Result<Response> {
		// Extract session ID from cookie
		let session_id = Self::extract_session_id(&request, &self.config.cookie_name);

		let auth_state = match session_id {
			None => AuthState::anonymous(),
			Some(ref sid) => {
				match self.backend.load(sid).await {
					Ok(Some(session)) => {
						// Check absolute max lifetime
						let now = SystemTime::now();
						let absolute_expiry = session
							.created_at
							.checked_add(self.config.absolute_max)
							.unwrap_or(session.created_at);

						if absolute_expiry < now {
							// Session exceeded absolute max — destroy it
							let _ = self.backend.destroy(sid).await;
							AuthState::anonymous()
						} else {
							// Extract user info from session data
							let user_id: String = session
								.data
								.get("user_id")
								.and_then(|v| serde_json::from_value(v.clone()).ok())
								.unwrap_or_default();

							if user_id.is_empty() {
								// Corrupted session: user_id missing or empty
								let _ = self.backend.destroy(sid).await;
								AuthState::anonymous()
							} else {
								let is_staff: bool = session
									.data
									.get("is_staff")
									.and_then(|v| serde_json::from_value(v.clone()).ok())
									.unwrap_or(false);
								let is_superuser: bool = session
									.data
									.get("is_superuser")
									.and_then(|v| serde_json::from_value(v.clone()).ok())
									.unwrap_or(false);

								let is_admin = is_staff || is_superuser;
								let is_active = true;

								// Insert backward compat values
								request.extensions.insert(user_id.clone());
								request.extensions.insert(IsAuthenticated(true));
								request.extensions.insert(IsAdmin(is_admin));
								request.extensions.insert(IsActive(is_active));

								AuthState::authenticated(user_id, is_admin, is_active)
							}
						}
					}
					_ => AuthState::anonymous(),
				}
			}
		};

		request.extensions.insert(auth_state);
		let response = next.handle(request).await?;

		// Fire-and-forget: touch session to reset sliding TTL
		if let Some(ref sid) = session_id {
			let backend = Arc::clone(&self.backend);
			let sid = sid.clone();
			let ttl = self.config.sliding_ttl;
			tokio::spawn(async move {
				let _ = backend.touch(&sid, ttl).await;
			});
		}

		Ok(response)
	}

	fn should_continue(&self, request: &Request) -> bool {
		!Self::is_skip_path(request.uri.path(), &self.config.skip_paths)
	}
}

#[cfg(all(test, feature = "sessions"))]
mod tests {
	use super::*;
	use bytes::Bytes;
	use hyper::{HeaderMap, Method, Version};
	use reinhardt_http::{AuthState, Handler, Middleware, Request, Response};
	use rstest::rstest;
	use std::collections::HashMap;
	use std::sync::Mutex;
	use std::time::{Duration, SystemTime};

	use crate::session::SessionData;

	/// In-memory session backend for tests.
	struct MockBackend {
		sessions: Mutex<HashMap<String, SessionData>>,
		destroyed: Mutex<Vec<String>>,
		touched: Mutex<Vec<(String, Duration)>>,
	}

	impl MockBackend {
		fn new() -> Self {
			Self {
				sessions: Mutex::new(HashMap::new()),
				destroyed: Mutex::new(Vec::new()),
				touched: Mutex::new(Vec::new()),
			}
		}

		fn insert(&self, session: SessionData) {
			self.sessions
				.lock()
				.unwrap()
				.insert(session.id.clone(), session);
		}

		fn was_destroyed(&self, id: &str) -> bool {
			self.destroyed.lock().unwrap().contains(&id.to_string())
		}
	}

	#[async_trait::async_trait]
	impl AsyncSessionBackend for MockBackend {
		async fn load(&self, id: &str) -> Result<Option<SessionData>> {
			Ok(self.sessions.lock().unwrap().get(id).cloned())
		}

		async fn save(&self, session: &SessionData) -> Result<()> {
			self.sessions
				.lock()
				.unwrap()
				.insert(session.id.clone(), session.clone());
			Ok(())
		}

		async fn destroy(&self, id: &str) -> Result<()> {
			self.sessions.lock().unwrap().remove(id);
			self.destroyed.lock().unwrap().push(id.to_string());
			Ok(())
		}

		async fn touch(&self, id: &str, ttl: Duration) -> Result<()> {
			self.touched.lock().unwrap().push((id.to_string(), ttl));
			Ok(())
		}
	}

	struct TestHandler;

	#[async_trait::async_trait]
	impl Handler for TestHandler {
		async fn handle(&self, request: Request) -> Result<Response> {
			let auth_state = request.extensions.get::<AuthState>();
			Ok(Response::ok().with_json(&serde_json::json!({
				"has_auth_state": auth_state.is_some(),
				"is_authenticated": auth_state.as_ref().map(|s| s.is_authenticated()).unwrap_or(false),
				"user_id": auth_state.as_ref().map(|s| s.user_id().to_string()).unwrap_or_default(),
				"is_admin": auth_state.as_ref().map(|s| s.is_admin()).unwrap_or(false),
				"is_active": auth_state.as_ref().map(|s| s.is_active()).unwrap_or(false),
			}))?)
		}
	}

	fn create_request_with_cookie(cookie: &str) -> Request {
		let mut headers = HeaderMap::new();
		headers.insert("Cookie", cookie.parse().unwrap());
		Request::builder()
			.method(Method::GET)
			.uri("/api/resource")
			.version(Version::HTTP_11)
			.headers(headers)
			.body(Bytes::new())
			.build()
			.unwrap()
	}

	fn create_request_with_path(path: &str) -> Request {
		Request::builder()
			.method(Method::GET)
			.uri(path)
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap()
	}

	fn create_request_without_cookie() -> Request {
		Request::builder()
			.method(Method::GET)
			.uri("/api/resource")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap()
	}

	fn make_session(id: &str, user_id: &str, is_staff: bool, is_superuser: bool) -> SessionData {
		let now = SystemTime::now();
		let mut data = HashMap::new();
		data.insert(
			"user_id".to_string(),
			serde_json::Value::String(user_id.to_string()),
		);
		data.insert("is_staff".to_string(), serde_json::Value::Bool(is_staff));
		data.insert(
			"is_superuser".to_string(),
			serde_json::Value::Bool(is_superuser),
		);
		SessionData {
			id: id.to_string(),
			data,
			created_at: now,
			last_accessed: now,
			expires_at: now + Duration::from_secs(3600),
			id_holder: None,
		}
	}

	fn parse_response_body(response: &Response) -> serde_json::Value {
		let body_str = String::from_utf8(response.body.to_vec()).unwrap();
		serde_json::from_str(&body_str).unwrap()
	}

	#[rstest]
	#[tokio::test]
	async fn test_valid_session_produces_authenticated_state() {
		// Arrange
		let backend = Arc::new(MockBackend::new());
		backend.insert(make_session("sess-123", "user-42", false, false));
		let middleware = CookieSessionAuthMiddleware::new(Arc::clone(&backend));
		let handler = Arc::new(TestHandler);
		let request = create_request_with_cookie("sessionid=sess-123");

		// Act
		let response = middleware.process(request, handler).await.unwrap();

		// Assert
		let body = parse_response_body(&response);
		assert_eq!(body["is_authenticated"], true);
		assert_eq!(body["user_id"], "user-42");
		assert_eq!(body["is_admin"], false);
		assert_eq!(body["is_active"], true);
	}

	#[rstest]
	#[tokio::test]
	async fn test_no_cookie_produces_anonymous_state() {
		// Arrange
		let backend = Arc::new(MockBackend::new());
		let middleware = CookieSessionAuthMiddleware::new(Arc::clone(&backend));
		let handler = Arc::new(TestHandler);
		let request = create_request_without_cookie();

		// Act
		let response = middleware.process(request, handler).await.unwrap();

		// Assert
		let body = parse_response_body(&response);
		assert_eq!(body["is_authenticated"], false);
	}

	#[rstest]
	#[tokio::test]
	async fn test_invalid_session_id_produces_anonymous_state() {
		// Arrange
		let backend = Arc::new(MockBackend::new());
		let middleware = CookieSessionAuthMiddleware::new(Arc::clone(&backend));
		let handler = Arc::new(TestHandler);
		let request = create_request_with_cookie("sessionid=nonexistent");

		// Act
		let response = middleware.process(request, handler).await.unwrap();

		// Assert
		let body = parse_response_body(&response);
		assert_eq!(body["is_authenticated"], false);
	}

	#[rstest]
	#[tokio::test]
	async fn test_absolute_max_exceeded_produces_anonymous_and_destroys_session() {
		// Arrange
		let backend = Arc::new(MockBackend::new());
		let mut session = make_session("sess-expired", "user-99", false, false);
		// Set created_at far in the past so absolute_max is exceeded
		session.created_at = SystemTime::now() - Duration::from_secs(48 * 3600);
		backend.insert(session);

		let middleware = CookieSessionAuthMiddleware::new(Arc::clone(&backend));
		let handler = Arc::new(TestHandler);
		let request = create_request_with_cookie("sessionid=sess-expired");

		// Act
		let response = middleware.process(request, handler).await.unwrap();

		// Assert
		let body = parse_response_body(&response);
		assert_eq!(body["is_authenticated"], false);
		assert!(backend.was_destroyed("sess-expired"));
	}

	#[rstest]
	#[tokio::test]
	async fn test_skip_path_bypasses_auth() {
		// Arrange
		let backend = Arc::new(MockBackend::new());
		let config = CookieSessionConfig {
			skip_paths: vec!["/health".to_string(), "/static/".to_string()],
			..Default::default()
		};
		let middleware = CookieSessionAuthMiddleware::with_config(Arc::clone(&backend), config);

		// Act & Assert — exact match
		let request = create_request_with_path("/health");
		assert!(!middleware.should_continue(&request));

		// Prefix match
		let request = create_request_with_path("/static/style.css");
		assert!(!middleware.should_continue(&request));

		// Non-skipped path
		let request = create_request_with_path("/api/users");
		assert!(middleware.should_continue(&request));
	}

	#[rstest]
	#[tokio::test]
	async fn test_staff_user_produces_admin_state() {
		// Arrange
		let backend = Arc::new(MockBackend::new());
		backend.insert(make_session("sess-staff", "admin-1", true, false));
		let middleware = CookieSessionAuthMiddleware::new(Arc::clone(&backend));
		let handler = Arc::new(TestHandler);
		let request = create_request_with_cookie("sessionid=sess-staff");

		// Act
		let response = middleware.process(request, handler).await.unwrap();

		// Assert
		let body = parse_response_body(&response);
		assert_eq!(body["is_authenticated"], true);
		assert_eq!(body["user_id"], "admin-1");
		assert_eq!(body["is_admin"], true);
		assert_eq!(body["is_active"], true);
	}

	#[rstest]
	#[tokio::test]
	async fn test_superuser_produces_admin_state() {
		// Arrange
		let backend = Arc::new(MockBackend::new());
		backend.insert(make_session("sess-super", "super-1", false, true));
		let middleware = CookieSessionAuthMiddleware::new(Arc::clone(&backend));
		let handler = Arc::new(TestHandler);
		let request = create_request_with_cookie("sessionid=sess-super");

		// Act
		let response = middleware.process(request, handler).await.unwrap();

		// Assert
		let body = parse_response_body(&response);
		assert_eq!(body["is_authenticated"], true);
		assert_eq!(body["user_id"], "super-1");
		assert_eq!(body["is_admin"], true);
		assert_eq!(body["is_active"], true);
	}

	/// Create a session with no "user_id" key in data.
	fn make_session_without_user_id(id: &str) -> SessionData {
		let mut session = make_session(id, "", false, false);
		session.data.remove("user_id");
		session
	}

	#[rstest]
	#[tokio::test]
	async fn test_empty_user_id_produces_anonymous_and_destroys_session() {
		// Arrange
		let backend = Arc::new(MockBackend::new());
		backend.insert(make_session("sess-empty", "", false, false));
		let middleware = CookieSessionAuthMiddleware::new(Arc::clone(&backend));
		let handler = Arc::new(TestHandler);
		let request = create_request_with_cookie("sessionid=sess-empty");

		// Act
		let response = middleware.process(request, handler).await.unwrap();

		// Assert
		let body = parse_response_body(&response);
		assert_eq!(body["is_authenticated"], false);
		assert!(backend.was_destroyed("sess-empty"));
	}

	#[rstest]
	#[tokio::test]
	async fn test_missing_user_id_produces_anonymous_and_destroys_session() {
		// Arrange
		let backend = Arc::new(MockBackend::new());
		backend.insert(make_session_without_user_id("sess-no-uid"));
		let middleware = CookieSessionAuthMiddleware::new(Arc::clone(&backend));
		let handler = Arc::new(TestHandler);
		let request = create_request_with_cookie("sessionid=sess-no-uid");

		// Act
		let response = middleware.process(request, handler).await.unwrap();

		// Assert
		let body = parse_response_body(&response);
		assert_eq!(body["is_authenticated"], false);
		assert!(backend.was_destroyed("sess-no-uid"));
	}

	#[rstest]
	#[tokio::test]
	async fn test_multiple_cookies_extracts_correct_one() {
		// Arrange
		let backend = Arc::new(MockBackend::new());
		backend.insert(make_session("sess-multi", "user-multi", false, false));
		let middleware = CookieSessionAuthMiddleware::new(Arc::clone(&backend));
		let handler = Arc::new(TestHandler);
		let request = create_request_with_cookie("other=foo; sessionid=sess-multi; another=bar");

		// Act
		let response = middleware.process(request, handler).await.unwrap();

		// Assert
		let body = parse_response_body(&response);
		assert_eq!(body["is_authenticated"], true);
		assert_eq!(body["user_id"], "user-multi");
	}
}
