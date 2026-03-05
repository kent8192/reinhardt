use bytes::Bytes;
use hyper::{HeaderMap, Method, Version};
use reinhardt_auth::sessions::{Session, backends::InMemorySessionBackend};
use reinhardt_auth::{RestAuthentication, SessionAuthentication};
use reinhardt_http::Request;
use uuid::Uuid;

/// Test SessionAuthentication with InMemorySessionBackend
#[tokio::test]
async fn test_session_authentication_with_inmemory_backend() {
	// Test intent: Verify SessionAuthentication successfully authenticates user
	// from valid session data stored in InMemorySessionBackend with all user fields
	// Not intent: Database backend, session expiry, concurrent access, CSRF validation
	let session_backend = InMemorySessionBackend::new();
	let auth = SessionAuthentication::new(session_backend.clone());

	// Create a test session with user data
	let user_id = Uuid::new_v4();
	let mut session = Session::new(session_backend.clone());
	session.set("_auth_user_id", user_id.to_string()).unwrap();
	session.set("_auth_user_name", "alice".to_string()).unwrap();
	session
		.set("_auth_user_email", "alice@example.com".to_string())
		.unwrap();
	session.set("_auth_user_is_active", true).unwrap();
	session.set("_auth_user_is_admin", false).unwrap();
	session.set("_auth_user_is_staff", false).unwrap();
	session.set("_auth_user_is_superuser", false).unwrap();

	// Save session to backend
	session.save().await.unwrap();
	let session_key = session.session_key().unwrap();

	// Create request with session cookie
	let mut headers = HeaderMap::new();
	headers.insert(
		"Cookie",
		format!("sessionid={}", session_key).parse().unwrap(),
	);

	let request = Request::builder()
		.method(Method::GET)
		.uri("/test")
		.version(Version::HTTP_11)
		.headers(headers)
		.body(Bytes::new())
		.build()
		.unwrap();

	// Test authentication
	let result = RestAuthentication::authenticate(&auth, &request)
		.await
		.unwrap();
	let user = result.unwrap();
	assert_eq!(user.id(), user_id.to_string());
	assert_eq!(user.get_username(), "alice");
	assert!(user.is_active());
	assert!(!user.is_admin());
}

/// Test SessionAuthentication without session cookie
#[tokio::test]
async fn test_session_authentication_no_cookie() {
	// Test intent: Verify SessionAuthentication returns None when
	// request has no session cookie (Cookie header missing)
	// Not intent: Invalid cookie format, expired sessions, cookie parsing errors
	let session_backend = InMemorySessionBackend::new();
	let auth = SessionAuthentication::new(session_backend);

	// Create request without session cookie
	let headers = HeaderMap::new();
	let request = Request::builder()
		.method(Method::GET)
		.uri("/test")
		.version(Version::HTTP_11)
		.headers(headers)
		.body(Bytes::new())
		.build()
		.unwrap();

	// Should return None (no authentication)
	let result = RestAuthentication::authenticate(&auth, &request)
		.await
		.unwrap();
	assert!(result.is_none());
}

/// Test SessionAuthentication with invalid session key
#[tokio::test]
async fn test_session_authentication_invalid_session_key() {
	// Test intent: Verify SessionAuthentication returns None when
	// session key in cookie does not exist in session backend
	// Not intent: Malformed session key format, session hijacking detection, brute force protection
	let session_backend = InMemorySessionBackend::new();
	let auth = SessionAuthentication::new(session_backend);

	// Create request with invalid session cookie
	let mut headers = HeaderMap::new();
	headers.insert("Cookie", "sessionid=invalid_key".parse().unwrap());

	let request = Request::builder()
		.method(Method::GET)
		.uri("/test")
		.version(Version::HTTP_11)
		.headers(headers)
		.body(Bytes::new())
		.build()
		.unwrap();

	// Should return None (invalid session)
	let result = RestAuthentication::authenticate(&auth, &request)
		.await
		.unwrap();
	assert!(result.is_none());
}

/// Test SessionAuthentication with session without user data
#[tokio::test]
async fn test_session_authentication_no_user_data() {
	// Test intent: Verify SessionAuthentication returns None when
	// valid session exists but lacks _auth_user_id field in session data
	// Not intent: Partial user data handling, session corruption detection, auto-logout
	let session_backend = InMemorySessionBackend::new();
	let auth = SessionAuthentication::new(session_backend.clone());

	// Create a session without user data (but with some other data to make it valid)
	let mut session = Session::new(session_backend.clone());
	session.set("_some_data", "value".to_string()).unwrap();
	session.save().await.unwrap();
	let session_key = session.session_key().unwrap();

	// Create request with session cookie
	let mut headers = HeaderMap::new();
	headers.insert(
		"Cookie",
		format!("sessionid={}", session_key).parse().unwrap(),
	);

	let request = Request::builder()
		.method(Method::GET)
		.uri("/test")
		.version(Version::HTTP_11)
		.headers(headers)
		.body(Bytes::new())
		.build()
		.unwrap();

	// Should return None (no user data in session)
	let result = RestAuthentication::authenticate(&auth, &request)
		.await
		.unwrap();
	assert!(result.is_none());
}

/// Test SessionAuthentication with custom cookie name
#[tokio::test]
async fn test_session_authentication_custom_cookie_name() {
	// Test intent: Verify SessionAuthentication correctly reads session key
	// from custom cookie name via SessionAuthConfig configuration
	// Not intent: Multiple cookie names, cookie domain/path settings, cookie security flags
	use reinhardt_auth::SessionAuthConfig;

	let session_backend = InMemorySessionBackend::new();
	let config = SessionAuthConfig {
		cookie_name: "custom_session".to_string(),
		enforce_csrf: true,
	};
	let auth = SessionAuthentication::with_config(config, session_backend.clone());

	// Create a test session with user data
	let user_id = Uuid::new_v4();
	let mut session = Session::new(session_backend.clone());
	session.set("_auth_user_id", user_id.to_string()).unwrap();
	session.save().await.unwrap();
	let session_key = session.session_key().unwrap();

	// Create request with custom cookie name
	let mut headers = HeaderMap::new();
	headers.insert(
		"Cookie",
		format!("custom_session={}", session_key).parse().unwrap(),
	);

	let request = Request::builder()
		.method(Method::GET)
		.uri("/test")
		.version(Version::HTTP_11)
		.headers(headers)
		.body(Bytes::new())
		.build()
		.unwrap();

	// Test authentication
	let result = RestAuthentication::authenticate(&auth, &request)
		.await
		.unwrap();
	let user = result.unwrap();
	assert_eq!(user.id(), user_id.to_string());
}
