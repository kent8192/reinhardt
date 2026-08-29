//! Session Middleware
//!
//! Provides enhanced session management functionality.
//! Supports various backends including Cookie, Redis, and database.
//!
//! This module is split into responsibility-focused submodules:
//!
//! - `id` — session-ID newtypes and the request-scoped active ID holder
//! - `data` — the `SessionData` payload, read/write/rotate helpers
//! - `store` — in-memory `SessionStore` with lazy eviction
//! - `backend` — pluggable `AsyncSessionBackend` trait
//! - `config` — `SessionConfig` cookie/TTL knobs
//! - `middleware` — `SessionMiddleware` that wires it all together
//! - `injectable` — DI integration (`Injectable` impl for `SessionData`).
//!   Handlers that want the store directly use
//!   `#[inject] store: KeyedDepends<SessionStoreKey, Arc<SessionStore>>`; the
//!   middleware contributes both the raw store key used by `SessionData` and
//!   the keyed factory output used by `KeyedDepends`. See #4437.
//!
//! All public types are re-exported here so existing call sites that
//! used `crate::session::*` continue to work unchanged.

mod auth_ext;
mod backend;
mod config;
mod cookie;
mod data;
mod id;
mod injectable;
mod middleware;
#[cfg(feature = "social-auth")]
mod social_state;
mod store;
mod value;

// Test-only fixtures shared between in-crate unit tests and external
// integration tests. Hidden from the public API surface and only compiled
// when `cfg(test)` is active (for unit tests) or the `test-support`
// feature is enabled (for integration tests). See Issue #4462.
#[cfg(any(test, feature = "test-support"))]
#[doc(hidden)]
pub mod test_support;

pub use auth_ext::SessionAuthExt;
pub use backend::{AsyncSessionBackend, AtomicSessionBackend};
pub use config::SessionConfig;
pub use data::{SessionData, USER_ID_SESSION_KEY};
pub use id::{ActiveSessionId, SessionCookieName, SessionId};
pub use middleware::SessionMiddleware;
#[cfg(feature = "social-auth")]
pub use social_state::AsyncSessionStateStore;
pub use store::{SessionStore, SessionStoreKey};
pub use value::{
	OptionalSessionValue, OptionalSessionValueNamed, SessionKey, SessionValue, SessionValueNamed,
	UserIdKey,
};

#[cfg(test)]
mod tests {
	use super::*;
	use async_trait::async_trait;
	use bytes::Bytes;
	use hyper::{HeaderMap, Method, StatusCode, Version};
	use reinhardt_http::{Handler, Middleware, Request, Response, Result};
	use std::sync::{Arc, RwLock};
	use std::thread;
	use std::time::{Duration, SystemTime};

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
	async fn test_session_save_cleans_expired_entries_while_above_threshold() {
		let store = SessionStore::with_cleanup_threshold(1);

		let first_valid_session = SessionData::new(Duration::from_secs(3600));
		let first_valid_id = first_valid_session.id.clone();
		store.save(first_valid_session);

		let second_valid_session = SessionData::new(Duration::from_secs(3600));
		let second_valid_id = second_valid_session.id.clone();
		store.save(second_valid_session);

		assert_eq!(store.len(), 2);

		let mut expired_session = SessionData::new(Duration::from_secs(3600));
		let expired_id = expired_session.id.clone();
		expired_session.expires_at = SystemTime::now() - Duration::from_millis(20);
		store.save(expired_session);

		assert_eq!(store.len(), 2);
		assert!(store.get(&expired_id).is_none());
		assert!(store.get(&first_valid_id).is_some());
		assert!(store.get(&second_valid_id).is_some());
	}

	#[tokio::test]
	async fn test_session_save_uses_amortized_cleanup_while_above_threshold() {
		let store = SessionStore::with_cleanup_threshold(2);

		let first_valid_session = SessionData::new(Duration::from_secs(3600));
		let second_valid_session = SessionData::new(Duration::from_secs(3600));
		let third_valid_session = SessionData::new(Duration::from_secs(3600));
		store.save(first_valid_session);
		store.save(second_valid_session);
		store.save(third_valid_session);

		let mut first_expired_session = SessionData::new(Duration::from_secs(3600));
		let first_expired_id = first_expired_session.id.clone();
		first_expired_session.expires_at = SystemTime::now() - Duration::from_millis(20);
		store.save(first_expired_session);

		assert_eq!(store.len(), 4);
		assert!(store.get(&first_expired_id).is_some());

		let mut second_expired_session = SessionData::new(Duration::from_secs(3600));
		let second_expired_id = second_expired_session.id.clone();
		second_expired_session.expires_at = SystemTime::now() - Duration::from_millis(20);
		store.save(second_expired_session);

		assert_eq!(store.len(), 3);
		assert!(store.get(&first_expired_id).is_none());
		assert!(store.get(&second_expired_id).is_none());
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

	/// Handler that rotates the session ID via `SessionData::regenerate_id`,
	/// emulating session-fixation prevention on login. Replays #3827.
	struct RotatingHandler {
		store: Arc<SessionStore>,
	}

	#[async_trait]
	impl Handler for RotatingHandler {
		async fn handle(&self, request: Request) -> Result<Response> {
			let active_id = request
				.extensions
				.get::<ActiveSessionId>()
				.expect("ActiveSessionId should be present");
			let original_id = active_id.get();

			let mut session = self
				.store
				.get(&original_id)
				.expect("session created by middleware should be present");
			session.id_holder = Some(active_id);

			let old_id = session.regenerate_id();
			session
				.set("user_id".to_string(), "user-42".to_string())
				.unwrap();
			self.store.delete(&old_id);
			self.store.save(session);

			Ok(Response::new(StatusCode::OK).with_body(Bytes::from("OK")))
		}
	}

	/// Regression test for #3827: a handler that rotates the session ID for
	/// session-fixation prevention must end up with the new ID in the
	/// response `Set-Cookie`, and that cookie must point at a stored session.
	#[tokio::test]
	async fn test_handler_id_rotation_propagates_to_cookie() {
		// Arrange
		let config = SessionConfig::new("sessionid".to_string(), Duration::from_secs(3600));
		let store = Arc::new(SessionStore::new());
		let middleware = SessionMiddleware::from_arc(config, Arc::clone(&store));
		let handler = Arc::new(RotatingHandler {
			store: Arc::clone(&store),
		});
		let request = Request::builder()
			.method(Method::POST)
			.uri("/login")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		// Act
		let response = middleware.process(request, handler).await.unwrap();

		// Assert: extract the session ID the client will receive…
		let cookie = response
			.headers
			.get("set-cookie")
			.expect("Set-Cookie should be set")
			.to_str()
			.unwrap();
		let cookie_session_id = cookie
			.split(';')
			.next()
			.unwrap()
			.split('=')
			.nth(1)
			.unwrap()
			.to_string();

		// …and verify the store contains exactly that session, with the user_id
		// the handler wrote during rotation.
		let stored = store
			.get(&cookie_session_id)
			.expect("Session referenced by Set-Cookie must exist in store");
		assert_eq!(stored.id, cookie_session_id);
		assert_eq!(
			stored.get::<String>("user_id").as_deref(),
			Some("user-42"),
			"Rotated session must carry the data written by the handler"
		);
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

#[cfg(test)]
mod async_backend_tests {
	use super::*;
	use async_trait::async_trait;
	use reinhardt_http::Result;
	use std::collections::HashMap;
	use std::sync::{Arc, RwLock};
	use std::time::Duration;

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
