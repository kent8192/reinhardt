//! Multi-Authentication Backend Integration Tests
//!
//! This test suite verifies the integration of multiple authentication backends
//! (JWT + Session + Token) working together in a single request processing pipeline.
//!
//! Test Coverage:
//! - Multiple authentication backends with fallback chain
//! - Backend priority ordering
//! - Mixed authentication in single request
//! - Backend-specific user models
//! - Backend switching based on request type
//! - Real authentication backend integration with database

use bytes::Bytes;
use hyper::{HeaderMap, Method, Version};
use reinhardt_auth::sessions::{Session, backends::InMemorySessionBackend};
use reinhardt_auth::{
	AuthenticationBackend, AuthenticationError, CompositeAuthentication, RestAuthentication,
	SessionAuthentication, SimpleUser, TokenAuthentication, User,
};
use reinhardt_http::Request;
use reinhardt_test::fixtures::*;
use rstest::*;
use std::sync::Arc;
use uuid::Uuid;

// Use fully qualified path to avoid ambiguity with glob import
use ::testcontainers::{ContainerAsync, GenericImage};

use chrono::Duration;
use reinhardt_auth::{Claims, JwtAuth};

// ============================================================================
// Helper Functions
// ============================================================================

/// Create a test request with given method, URI, headers, and body
fn build_test_request(method: Method, uri: &str, headers: HeaderMap, body: Bytes) -> Request {
	Request::builder()
		.method(method)
		.uri(uri)
		.version(Version::HTTP_11)
		.headers(headers)
		.body(body)
		.build()
		.unwrap()
}

/// Create JWT token for testing
fn create_jwt_token(jwt_auth: &JwtAuth, user_id: &str, username: &str) -> String {
	let claims = Claims::new(
		user_id.to_string(),
		username.to_string(),
		Duration::hours(1),
	);
	jwt_auth.encode(&claims).unwrap()
}

/// Create session with user authentication data
async fn create_authenticated_session(
	session_backend: Arc<InMemorySessionBackend>,
	user_id: Uuid,
	username: &str,
	email: &str,
) -> String {
	// Clone the Arc to get a reference for Session
	let mut session = Session::new((*session_backend).clone());
	session.set("_auth_user_id", user_id.to_string()).unwrap();
	session
		.set("_auth_user_name", username.to_string())
		.unwrap();
	session.set("_auth_user_email", email.to_string()).unwrap();
	session.set("_auth_user_is_active", true).unwrap();
	session.set("_auth_user_is_admin", false).unwrap();
	session.set("_auth_user_is_staff", false).unwrap();
	session.set("_auth_user_is_superuser", false).unwrap();

	session.save().await.unwrap();
	session.session_key().unwrap().to_string()
}

// ============================================================================
// Test Suite 1: Backend Fallback Chain
// ============================================================================

/// Test composite authentication falls back from JWT to Session
#[tokio::test]
async fn test_composite_auth_jwt_to_session_fallback() {
	// Test intent: Verify CompositeAuthentication tries JWT first, then falls back to SessionAuthentication
	// when JWT header is missing. User successfully authenticated via session backend.
	// Not intent: Token authentication, invalid credentials, expired sessions
	let jwt_auth = Arc::new(JwtAuth::new(b"test_secret_key_12345"));
	let session_backend = Arc::new(InMemorySessionBackend::new());
	let session_auth = Arc::new(SessionAuthentication::new((*session_backend).clone()));

	// Create composite auth: JWT -> Session
	let composite = CompositeAuthentication::new()
		.with_backend((*jwt_auth).clone())
		.with_backend((*session_auth).clone());

	// Create session with authenticated user
	let user_id = Uuid::new_v4();
	let session_key = create_authenticated_session(
		session_backend.clone(),
		user_id,
		"alice",
		"alice@example.com",
	)
	.await;

	// Create request with session cookie (no JWT)
	let mut headers = HeaderMap::new();
	headers.insert(
		"Cookie",
		format!("sessionid={}", session_key).parse().unwrap(),
	);

	let request = build_test_request(Method::GET, "/api/profile", headers, Bytes::new());

	// Authenticate - should fallback to session
	let result = AuthenticationBackend::authenticate(&composite, &request)
		.await
		.unwrap();
	let user = result.unwrap();
	assert_eq!(user.id(), user_id.to_string());
	assert_eq!(user.get_username(), "alice");
}

/// Test composite authentication falls back from Session to Token
#[tokio::test]
async fn test_composite_auth_session_to_token_fallback() {
	// Test intent: Verify CompositeAuthentication tries Session first, then falls back to TokenAuthentication
	// when session cookie is missing. User successfully authenticated via token.
	// Not intent: JWT authentication, database backend, concurrent requests
	let session_backend = Arc::new(InMemorySessionBackend::new());
	let session_auth = Arc::new(SessionAuthentication::new((*session_backend).clone()));

	// Create token auth with test token
	let mut token_auth = TokenAuthentication::new();
	let user_id = Uuid::new_v4();
	token_auth.add_token("test_token_abc123", user_id.to_string());

	// Create composite auth: Session -> Token
	let composite = CompositeAuthentication::new()
		.with_backend((*session_auth).clone())
		.with_backend(token_auth);

	// Create request with token header (no session)
	let mut headers = HeaderMap::new();
	headers.insert("Authorization", "Token test_token_abc123".parse().unwrap());

	let request = build_test_request(Method::GET, "/api/data", headers, Bytes::new());

	// Authenticate - should fallback to token
	let result = AuthenticationBackend::authenticate(&composite, &request)
		.await
		.unwrap();
	let user = result.unwrap();
	assert_eq!(user.id(), user_id.to_string());
}

/// Test composite authentication tries all backends in chain
#[tokio::test]
async fn test_composite_auth_full_chain_fallback() {
	// Test intent: Verify CompositeAuthentication tries JWT -> Session -> Token in order,
	// successfully authenticating with the third backend when first two fail.
	// Not intent: Concurrent authentication, backend error handling, circular dependencies
	let jwt_auth = Arc::new(JwtAuth::new(b"test_secret_key"));
	let session_backend = Arc::new(InMemorySessionBackend::new());
	let session_auth = Arc::new(SessionAuthentication::new((*session_backend).clone()));

	let mut token_auth = TokenAuthentication::new();
	let user_id = Uuid::new_v4();
	token_auth.add_token("valid_token", user_id.to_string());

	// Create composite auth: JWT -> Session -> Token
	let composite = CompositeAuthentication::new()
		.with_backend((*jwt_auth).clone())
		.with_backend((*session_auth).clone())
		.with_backend(token_auth);

	// Create request with only token (JWT and Session will fail)
	let mut headers = HeaderMap::new();
	headers.insert("Authorization", "Token valid_token".parse().unwrap());

	let request = build_test_request(Method::POST, "/api/create", headers, Bytes::new());

	// Authenticate - should try JWT (fail), Session (fail), Token (success)
	let result = AuthenticationBackend::authenticate(&composite, &request)
		.await
		.unwrap();
	let user = result.unwrap();
	assert_eq!(user.id(), user_id.to_string());
}

/// Test composite authentication returns None when all backends fail
#[tokio::test]
async fn test_composite_auth_all_backends_fail() {
	// Test intent: Verify CompositeAuthentication returns None when all backends
	// fail to authenticate (no valid credentials provided).
	// Not intent: Backend errors, database failures, network timeouts
	let jwt_auth = Arc::new(JwtAuth::new(b"test_secret"));
	let session_backend = Arc::new(InMemorySessionBackend::new());
	let session_auth = Arc::new(SessionAuthentication::new((*session_backend).clone()));

	let token_auth = TokenAuthentication::new(); // No tokens added

	// Create composite auth
	let composite = CompositeAuthentication::new()
		.with_backend((*jwt_auth).clone())
		.with_backend((*session_auth).clone())
		.with_backend(token_auth);

	// Create request with no authentication headers
	let headers = HeaderMap::new();
	let request = build_test_request(Method::GET, "/api/public", headers, Bytes::new());

	// Authenticate - all backends should fail, return None
	let result = AuthenticationBackend::authenticate(&composite, &request)
		.await
		.unwrap();
	assert!(result.is_none());
}

// ============================================================================
// Test Suite 2: Backend Priority Ordering
// ============================================================================

/// Test backend priority with JWT first
#[tokio::test]
async fn test_backend_priority_jwt_first() {
	// Test intent: Verify CompositeAuthentication prioritizes JWT over Session
	// when both credentials are provided. JWT authentication succeeds first.
	// Not intent: Session fallback when JWT exists, invalid JWT handling
	let jwt_auth = Arc::new(JwtAuth::new(b"priority_test_secret"));
	let session_backend = Arc::new(InMemorySessionBackend::new());
	let session_auth = Arc::new(SessionAuthentication::new((*session_backend).clone()));

	// Create composite: JWT has higher priority
	let composite = CompositeAuthentication::new()
		.with_backend((*jwt_auth).clone())
		.with_backend((*session_auth).clone());

	// Create both JWT token and session
	let jwt_user_id = Uuid::new_v4();
	let jwt_token = create_jwt_token(&jwt_auth, &jwt_user_id.to_string(), "jwt_user");

	let session_user_id = Uuid::new_v4();
	let session_key = create_authenticated_session(
		session_backend,
		session_user_id,
		"session_user",
		"session@example.com",
	)
	.await;

	// Request with BOTH JWT and session cookie
	let mut headers = HeaderMap::new();
	headers.insert(
		"Authorization",
		format!("Bearer {}", jwt_token).parse().unwrap(),
	);
	headers.insert(
		"Cookie",
		format!("sessionid={}", session_key).parse().unwrap(),
	);

	let request = build_test_request(Method::GET, "/api/me", headers, Bytes::new());

	// Authenticate - JWT should be used (higher priority)
	let result = AuthenticationBackend::authenticate(&composite, &request)
		.await
		.unwrap();
	let user = result.unwrap();
	assert_eq!(user.id(), jwt_user_id.to_string());
	assert_eq!(user.get_username(), "jwt_user");
}

/// Test backend priority with Session first
#[tokio::test]
async fn test_backend_priority_session_first() {
	// Test intent: Verify CompositeAuthentication prioritizes Session over Token
	// when both credentials are provided in that backend order.
	// Not intent: JWT priority, backend reordering, invalid credentials
	let session_backend = Arc::new(InMemorySessionBackend::new());
	let session_auth = Arc::new(SessionAuthentication::new((*session_backend).clone()));

	let mut token_auth = TokenAuthentication::new();
	let token_user_id = Uuid::new_v4();
	token_auth.add_token("token123", token_user_id.to_string());

	// Create composite: Session has higher priority than Token
	let composite = CompositeAuthentication::new()
		.with_backend((*session_auth).clone())
		.with_backend(token_auth);

	// Create both session and token
	let session_user_id = Uuid::new_v4();
	let session_key = create_authenticated_session(
		session_backend,
		session_user_id,
		"session_user",
		"session@example.com",
	)
	.await;

	// Request with BOTH session and token
	let mut headers = HeaderMap::new();
	headers.insert(
		"Cookie",
		format!("sessionid={}", session_key).parse().unwrap(),
	);
	headers.insert("Authorization", "Token token123".parse().unwrap());

	let request = build_test_request(Method::POST, "/api/action", headers, Bytes::new());

	// Authenticate - Session should be used (higher priority)
	let result = AuthenticationBackend::authenticate(&composite, &request)
		.await
		.unwrap();
	let user = result.unwrap();
	assert_eq!(user.id(), session_user_id.to_string());
	assert_eq!(user.get_username(), "session_user");
}

/// Test backend order affects authentication result
#[tokio::test]
async fn test_backend_order_affects_result() {
	// Test intent: Verify changing backend registration order changes which backend
	// is tried first, demonstrating order-dependent authentication behavior.
	// Not intent: Concurrent modification, backend mutation, thread safety
	let jwt_auth = Arc::new(JwtAuth::new(b"order_test"));
	let session_backend = Arc::new(InMemorySessionBackend::new());
	let session_auth = Arc::new(SessionAuthentication::new((*session_backend).clone()));

	// Composite 1: JWT -> Session
	let composite1 = CompositeAuthentication::new()
		.with_backend((*jwt_auth).clone())
		.with_backend((*session_auth).clone());

	// Composite 2: Session -> JWT (reversed order)
	let composite2 = CompositeAuthentication::new()
		.with_backend((*session_auth).clone())
		.with_backend((*jwt_auth).clone());

	// Create both credentials
	let jwt_user_id = Uuid::new_v4();
	let jwt_token = create_jwt_token(&jwt_auth, &jwt_user_id.to_string(), "jwt_user");

	let session_user_id = Uuid::new_v4();
	let session_key = create_authenticated_session(
		session_backend,
		session_user_id,
		"session_user",
		"session@example.com",
	)
	.await;

	// Request with both credentials
	let mut headers = HeaderMap::new();
	headers.insert(
		"Authorization",
		format!("Bearer {}", jwt_token).parse().unwrap(),
	);
	headers.insert(
		"Cookie",
		format!("sessionid={}", session_key).parse().unwrap(),
	);
	let request = build_test_request(Method::GET, "/api/test", headers, Bytes::new());

	// Composite 1 should use JWT (first in order)
	let result1 = AuthenticationBackend::authenticate(&composite1, &request)
		.await
		.unwrap();
	let user1 = result1.unwrap();
	assert_eq!(user1.id(), jwt_user_id.to_string());
	assert_eq!(user1.get_username(), "jwt_user");

	// Composite 2 should use Session (first in order)
	let result2 = AuthenticationBackend::authenticate(&composite2, &request)
		.await
		.unwrap();
	let user2 = result2.unwrap();
	assert_eq!(user2.id(), session_user_id.to_string());
	assert_eq!(user2.get_username(), "session_user");
}

// ============================================================================
// Test Suite 3: Mixed Authentication in Single Request
// ============================================================================

/// Test single request with multiple valid credentials
#[tokio::test]
async fn test_single_request_multiple_valid_credentials() {
	// Test intent: Verify CompositeAuthentication correctly handles request with
	// multiple valid authentication methods (JWT + Session + Token), using first valid one.
	// Not intent: Credential merging, multi-factor auth, permission aggregation
	let jwt_auth = Arc::new(JwtAuth::new(b"multi_cred_secret"));
	let session_backend = Arc::new(InMemorySessionBackend::new());
	let session_auth = Arc::new(SessionAuthentication::new((*session_backend).clone()));

	let mut token_auth = TokenAuthentication::new();
	let token_user_id = Uuid::new_v4();
	token_auth.add_token("multi_token", token_user_id.to_string());

	// Create composite: JWT -> Session -> Token
	let composite = CompositeAuthentication::new()
		.with_backend((*jwt_auth).clone())
		.with_backend((*session_auth).clone())
		.with_backend(token_auth);

	// Create all three types of credentials
	let jwt_user_id = Uuid::new_v4();
	let jwt_token = create_jwt_token(&jwt_auth, &jwt_user_id.to_string(), "jwt_alice");

	let session_user_id = Uuid::new_v4();
	let session_key = create_authenticated_session(
		session_backend,
		session_user_id,
		"session_bob",
		"bob@example.com",
	)
	.await;

	// Request with ALL three credentials
	let mut headers = HeaderMap::new();
	headers.insert(
		"Authorization",
		format!("Bearer {}", jwt_token).parse().unwrap(),
	);
	headers.insert(
		"Cookie",
		format!("sessionid={}", session_key).parse().unwrap(),
	);
	// Note: Token header would conflict with JWT Bearer, so we'll test priority

	let request = build_test_request(Method::GET, "/api/data", headers, Bytes::new());

	// Authenticate - should use JWT (highest priority)
	let result = AuthenticationBackend::authenticate(&composite, &request)
		.await
		.unwrap();
	let user = result.unwrap();
	assert_eq!(user.id(), jwt_user_id.to_string());
	assert_eq!(user.get_username(), "jwt_alice");
}

/// Test single request with one valid and one invalid credential
#[tokio::test]
async fn test_single_request_mixed_valid_invalid_credentials() {
	// Test intent: Verify CompositeAuthentication skips invalid JWT and successfully
	// falls back to valid Session credential in same request.
	// Not intent: Error propagation, retry logic, credential validation order
	let jwt_auth = Arc::new(JwtAuth::new(b"mixed_secret"));
	let session_backend = Arc::new(InMemorySessionBackend::new());
	let session_auth = Arc::new(SessionAuthentication::new((*session_backend).clone()));

	// Create composite: JWT -> Session
	let composite = CompositeAuthentication::new()
		.with_backend((*jwt_auth).clone())
		.with_backend((*session_auth).clone());

	// Create valid session
	let session_user_id = Uuid::new_v4();
	let session_key = create_authenticated_session(
		session_backend,
		session_user_id,
		"valid_user",
		"valid@example.com",
	)
	.await;

	// Request with invalid JWT and valid Session
	let mut headers = HeaderMap::new();
	headers.insert(
		"Authorization",
		"Bearer invalid_jwt_token_xyz".parse().unwrap(),
	);
	headers.insert(
		"Cookie",
		format!("sessionid={}", session_key).parse().unwrap(),
	);

	let request = build_test_request(Method::POST, "/api/submit", headers, Bytes::new());

	// Authenticate - should skip invalid JWT, use valid Session
	let result = AuthenticationBackend::authenticate(&composite, &request)
		.await
		.unwrap();
	let user = result.unwrap();
	assert_eq!(user.id(), session_user_id.to_string());
	assert_eq!(user.get_username(), "valid_user");
}

/// Test single request with all invalid credentials
#[tokio::test]
async fn test_single_request_all_invalid_credentials() {
	// Test intent: Verify CompositeAuthentication returns None when request
	// contains multiple authentication attempts but all are invalid.
	// Not intent: Error counting, logging, rate limiting
	let jwt_auth = Arc::new(JwtAuth::new(b"all_invalid_secret"));
	let session_backend = Arc::new(InMemorySessionBackend::new());
	let session_auth = Arc::new(SessionAuthentication::new((*session_backend).clone()));

	let token_auth = TokenAuthentication::new(); // No valid tokens

	// Create composite
	let composite = CompositeAuthentication::new()
		.with_backend((*jwt_auth).clone())
		.with_backend((*session_auth).clone())
		.with_backend(token_auth);

	// Request with all invalid credentials
	let mut headers = HeaderMap::new();
	headers.insert("Authorization", "Bearer bad_jwt".parse().unwrap());
	headers.insert("Cookie", "sessionid=invalid_session".parse().unwrap());

	let request = build_test_request(Method::GET, "/api/secure", headers, Bytes::new());

	// Authenticate - all should fail, return None
	let result = AuthenticationBackend::authenticate(&composite, &request)
		.await
		.unwrap();
	assert!(result.is_none());
}

// ============================================================================
// Test Suite 4: Backend-Specific User Models
// ============================================================================

/// Test JWT backend returns JWT-specific user
#[tokio::test]
async fn test_jwt_backend_user_model() {
	// Test intent: Verify JWT authentication returns user model with JWT-specific
	// fields populated from token claims (user ID from claims.sub, username from claims.username).
	// Not intent: Custom user model, database user loading, permission checking
	let jwt_auth = Arc::new(JwtAuth::new(b"user_model_secret"));

	// Create JWT token with specific user data
	let user_id = Uuid::new_v4();
	let token = create_jwt_token(&jwt_auth, &user_id.to_string(), "jwt_specific_user");

	// Create request with JWT
	let mut headers = HeaderMap::new();
	headers.insert(
		"Authorization",
		format!("Bearer {}", token).parse().unwrap(),
	);
	let request = build_test_request(Method::GET, "/api/profile", headers, Bytes::new());

	// Authenticate via JWT
	let result = RestAuthentication::authenticate(jwt_auth.as_ref(), &request)
		.await
		.unwrap();
	let user = result.unwrap();

	// Verify JWT-specific user fields
	assert_eq!(user.id(), user_id.to_string());
	assert_eq!(user.get_username(), "jwt_specific_user");
	assert!(user.is_active()); // JWT users are active by default
	assert!(!user.is_admin()); // Not admin by default
}

/// Test Session backend returns Session-specific user
#[tokio::test]
async fn test_session_backend_user_model() {
	// Test intent: Verify Session authentication returns user model with all fields
	// populated from session data (ID, username, email, is_active, is_admin, is_staff, is_superuser).
	// Not intent: Session expiry, CSRF validation, cookie security
	let session_backend = Arc::new(InMemorySessionBackend::new());
	let session_auth = SessionAuthentication::new((*session_backend).clone());

	// Create session with comprehensive user data
	let user_id = Uuid::new_v4();
	let mut session = Session::new((*session_backend).clone());
	session.set("_auth_user_id", user_id.to_string()).unwrap();
	session
		.set("_auth_user_name", "session_user".to_string())
		.unwrap();
	session
		.set("_auth_user_email", "session@example.com".to_string())
		.unwrap();
	session.set("_auth_user_is_active", true).unwrap();
	session.set("_auth_user_is_admin", true).unwrap();
	session.set("_auth_user_is_staff", true).unwrap();
	session.set("_auth_user_is_superuser", false).unwrap();

	session.save().await.unwrap();
	let session_key = session.session_key().unwrap();

	// Create request with session cookie
	let mut headers = HeaderMap::new();
	headers.insert(
		"Cookie",
		format!("sessionid={}", session_key).parse().unwrap(),
	);
	let request = build_test_request(Method::GET, "/admin", headers, Bytes::new());

	// Authenticate via Session
	let result = RestAuthentication::authenticate(&session_auth, &request)
		.await
		.unwrap();
	let user = result.unwrap();

	// Verify Session-specific user fields
	assert_eq!(user.id(), user_id.to_string());
	assert_eq!(user.get_username(), "session_user");
	assert!(user.is_active());
	assert!(user.is_admin());
	assert!(user.is_staff());
	assert!(!user.is_superuser());
}

/// Test Token backend returns Token-specific user
#[tokio::test]
async fn test_token_backend_user_model() {
	// Test intent: Verify Token authentication returns minimal user model with
	// only ID field populated from token storage (username generated from ID).
	// Not intent: Token rotation, token blacklist, database lookup
	let mut token_auth = TokenAuthentication::new();
	let user_id = Uuid::new_v4();
	token_auth.add_token("specific_token_xyz", user_id.to_string());

	// Create request with token
	let mut headers = HeaderMap::new();
	headers.insert("Authorization", "Token specific_token_xyz".parse().unwrap());
	let request = build_test_request(Method::POST, "/api/update", headers, Bytes::new());

	// Authenticate via Token
	let result = RestAuthentication::authenticate(&token_auth, &request)
		.await
		.unwrap();
	let user = result.unwrap();

	// Verify Token-specific user fields
	// Token auth uses user_id as username
	assert_eq!(user.get_username(), user_id.to_string());
}

// ============================================================================
// Test Suite 5: Backend Switching Based on Request Type
// ============================================================================

/// Test API requests prefer JWT authentication
#[tokio::test]
async fn test_api_requests_prefer_jwt() {
	// Test intent: Verify API endpoint requests (path starts with /api/) successfully
	// authenticate with JWT when multiple backends are available.
	// Not intent: Path-based routing, middleware integration, request filtering
	let jwt_auth = Arc::new(JwtAuth::new(b"api_secret"));
	let session_backend = Arc::new(InMemorySessionBackend::new());
	let session_auth = Arc::new(SessionAuthentication::new((*session_backend).clone()));

	// Composite: JWT first (typical for APIs)
	let composite = CompositeAuthentication::new()
		.with_backend((*jwt_auth).clone())
		.with_backend((*session_auth).clone());

	// Create JWT token
	let user_id = Uuid::new_v4();
	let token = create_jwt_token(&jwt_auth, &user_id.to_string(), "api_user");

	// API request with JWT
	let mut headers = HeaderMap::new();
	headers.insert(
		"Authorization",
		format!("Bearer {}", token).parse().unwrap(),
	);
	let request = build_test_request(Method::GET, "/api/v1/users", headers, Bytes::new());

	// Authenticate - JWT should succeed for API request
	let result = AuthenticationBackend::authenticate(&composite, &request)
		.await
		.unwrap();
	let user = result.unwrap();
	assert_eq!(user.id(), user_id.to_string());
	assert_eq!(user.get_username(), "api_user");
}

/// Test web requests prefer Session authentication
#[tokio::test]
async fn test_web_requests_prefer_session() {
	// Test intent: Verify web page requests (non-API paths) successfully authenticate
	// with Session when multiple backends are configured in Session-first order.
	// Not intent: Cookie security, CSRF protection, session hijacking
	let session_backend = Arc::new(InMemorySessionBackend::new());
	let session_auth = Arc::new(SessionAuthentication::new((*session_backend).clone()));
	let jwt_auth = Arc::new(JwtAuth::new(b"web_secret"));

	// Composite: Session first (typical for web apps)
	let composite = CompositeAuthentication::new()
		.with_backend((*session_auth).clone())
		.with_backend((*jwt_auth).clone());

	// Create session
	let user_id = Uuid::new_v4();
	let session_key =
		create_authenticated_session(session_backend, user_id, "web_user", "web@example.com").await;

	// Web request with session cookie
	let mut headers = HeaderMap::new();
	headers.insert(
		"Cookie",
		format!("sessionid={}", session_key).parse().unwrap(),
	);
	let request = build_test_request(Method::GET, "/dashboard", headers, Bytes::new());

	// Authenticate - Session should succeed for web request
	let result = AuthenticationBackend::authenticate(&composite, &request)
		.await
		.unwrap();
	let user = result.unwrap();
	assert_eq!(user.id(), user_id.to_string());
	assert_eq!(user.get_username(), "web_user");
}

/// Test mobile app requests use Token authentication
#[tokio::test]
async fn test_mobile_requests_use_token() {
	// Test intent: Verify mobile app requests successfully authenticate with
	// Token when configured in typical mobile-first backend order.
	// Not intent: Device fingerprinting, token refresh, push notifications
	let mut token_auth = TokenAuthentication::new();
	let user_id = Uuid::new_v4();
	token_auth.add_token("mobile_app_token", user_id.to_string());

	let session_backend = Arc::new(InMemorySessionBackend::new());
	let session_auth = Arc::new(SessionAuthentication::new((*session_backend).clone()));

	// Composite: Token first for mobile apps
	let composite = CompositeAuthentication::new()
		.with_backend(token_auth)
		.with_backend((*session_auth).clone());

	// Mobile request with token
	let mut headers = HeaderMap::new();
	headers.insert("Authorization", "Token mobile_app_token".parse().unwrap());
	let request = build_test_request(Method::POST, "/api/sync", headers, Bytes::new());

	// Authenticate - Token should succeed for mobile request
	let result = AuthenticationBackend::authenticate(&composite, &request)
		.await
		.unwrap();
	let user = result.unwrap();
	assert_eq!(user.id(), user_id.to_string());
}

/// Test same user can authenticate via different backends
#[tokio::test]
async fn test_same_user_different_backends() {
	// Test intent: Verify same user can authenticate successfully via different
	// backends (JWT and Session) in separate requests, backend choice doesn't affect user identity.
	// Not intent: Concurrent sessions, session sharing, token synchronization
	let user_id = Uuid::new_v4();
	let username = "multi_backend_user";

	// Setup JWT backend
	let jwt_auth = Arc::new(JwtAuth::new(b"same_user_secret"));
	let jwt_token = create_jwt_token(&jwt_auth, &user_id.to_string(), username);

	// Setup Session backend
	let session_backend = Arc::new(InMemorySessionBackend::new());
	let session_auth = Arc::new(SessionAuthentication::new((*session_backend).clone()));
	let session_key =
		create_authenticated_session(session_backend, user_id, username, "user@example.com").await;

	// Composite with both backends
	let composite = CompositeAuthentication::new()
		.with_backend((*jwt_auth).clone())
		.with_backend((*session_auth).clone());

	// Request 1: Authenticate via JWT
	let mut jwt_headers = HeaderMap::new();
	jwt_headers.insert(
		"Authorization",
		format!("Bearer {}", jwt_token).parse().unwrap(),
	);
	let jwt_request = build_test_request(Method::GET, "/api/data", jwt_headers, Bytes::new());

	let jwt_result = AuthenticationBackend::authenticate(&composite, &jwt_request)
		.await
		.unwrap();
	let jwt_user = jwt_result.unwrap();
	assert_eq!(jwt_user.id(), user_id.to_string());
	assert_eq!(jwt_user.get_username(), username);

	// Request 2: Authenticate via Session (same user)
	let mut session_headers = HeaderMap::new();
	session_headers.insert(
		"Cookie",
		format!("sessionid={}", session_key).parse().unwrap(),
	);
	let session_request =
		build_test_request(Method::GET, "/dashboard", session_headers, Bytes::new());

	let session_result = AuthenticationBackend::authenticate(&composite, &session_request)
		.await
		.unwrap();
	let session_user = session_result.unwrap();
	assert_eq!(session_user.id(), user_id.to_string());
	assert_eq!(session_user.get_username(), username);
}

// ============================================================================
// Test Suite 6: Real Authentication Backend Integration (with Database)
// ============================================================================

/// Custom authentication backend that uses database for user lookup
#[derive(Clone)]
struct DatabaseAuthBackend {
	#[allow(dead_code)]
	db_url: String,
	pool: Arc<sqlx::PgPool>,
}

impl DatabaseAuthBackend {
	async fn new(pool: Arc<sqlx::PgPool>, db_url: String) -> Self {
		// Create users table
		sqlx::query(
			r#"
            CREATE TABLE IF NOT EXISTS auth_users (
                id UUID PRIMARY KEY,
                username VARCHAR(150) UNIQUE NOT NULL,
                email VARCHAR(254) NOT NULL,
                is_active BOOLEAN NOT NULL DEFAULT TRUE,
                is_staff BOOLEAN NOT NULL DEFAULT FALSE,
                is_superuser BOOLEAN NOT NULL DEFAULT FALSE
            )
            "#,
		)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create auth_users table");

		Self { db_url, pool }
	}

	async fn insert_user(
		&self,
		id: Uuid,
		username: &str,
		email: &str,
		is_active: bool,
		is_staff: bool,
	) {
		sqlx::query(
			r#"
            INSERT INTO auth_users (id, username, email, is_active, is_staff, is_superuser)
            VALUES ($1, $2, $3, $4, $5, false)
            "#,
		)
		.bind(id)
		.bind(username)
		.bind(email)
		.bind(is_active)
		.bind(is_staff)
		.execute(self.pool.as_ref())
		.await
		.expect("Failed to insert test user");
	}
}

#[async_trait::async_trait]
impl AuthenticationBackend for DatabaseAuthBackend {
	async fn authenticate(
		&self,
		request: &reinhardt_http::Request,
	) -> Result<Option<Box<dyn User>>, AuthenticationError> {
		// Custom header: X-User-ID
		let user_id_header = request
			.headers
			.get("X-User-ID")
			.and_then(|h| h.to_str().ok());

		if let Some(user_id_str) = user_id_header {
			if let Ok(user_id) = Uuid::parse_str(user_id_str) {
				// Load user from database
				if let Some(user) = self.get_user(&user_id.to_string()).await.ok().flatten() {
					return Ok(Some(user));
				}
			}
		}

		Ok(None)
	}

	async fn get_user(&self, user_id: &str) -> Result<Option<Box<dyn User>>, AuthenticationError> {
		let uuid = Uuid::parse_str(user_id).map_err(|_| AuthenticationError::InvalidCredentials)?;

		let result = sqlx::query_as::<_, (Uuid, String, String, bool, bool, bool)>(
			r#"
            SELECT id, username, email, is_active, is_staff, is_superuser
            FROM auth_users
            WHERE id = $1
            "#,
		)
		.bind(uuid)
		.fetch_optional(self.pool.as_ref())
		.await
		.map_err(|e| AuthenticationError::DatabaseError(e.to_string()))?;

		Ok(
			result.map(|(id, username, email, is_active, is_staff, is_superuser)| {
				Box::new(SimpleUser {
					id,
					username,
					email,
					is_active,
					is_admin: is_staff,
					is_staff,
					is_superuser,
				}) as Box<dyn User>
			}),
		)
	}
}

/// Test database backend authenticates user from database
#[rstest]
#[tokio::test]
async fn test_database_backend_authenticates_user(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<sqlx::PgPool>, u16, String),
) {
	// Test intent: Verify custom DatabaseAuthBackend loads user data from PostgreSQL
	// and creates authenticated user with all fields populated from database.
	// Not intent: Connection pooling, query optimization, migration management
	let (_container, pool, _port, db_url) = postgres_container.await;

	// Create database backend
	let db_backend = DatabaseAuthBackend::new(pool.clone(), db_url).await;

	// Insert test user into database
	let user_id = Uuid::new_v4();
	db_backend
		.insert_user(user_id, "db_alice", "alice@db.com", true, false)
		.await;

	// Create request with custom user ID header
	let mut headers = HeaderMap::new();
	headers.insert("X-User-ID", user_id.to_string().parse().unwrap());
	let request = build_test_request(Method::GET, "/api/data", headers, Bytes::new());

	// Authenticate via database backend
	let result = db_backend.authenticate(&request).await.unwrap();
	let user = result.unwrap();

	assert_eq!(user.id(), user_id.to_string());
	assert_eq!(user.get_username(), "db_alice");
	assert!(user.is_active());
	assert!(!user.is_staff());
}

/// Test composite with database backend fallback
#[rstest]
#[tokio::test]
async fn test_composite_with_database_fallback(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<sqlx::PgPool>, u16, String),
) {
	// Test intent: Verify CompositeAuthentication falls back to DatabaseAuthBackend
	// when JWT authentication fails, successfully loading user from PostgreSQL.
	// Not intent: Database failover, read replicas, caching strategies
	let (_container, pool, _port, db_url) = postgres_container.await;

	// Create backends
	let jwt_auth = Arc::new(JwtAuth::new(b"db_fallback_secret"));
	let db_backend = Arc::new(DatabaseAuthBackend::new(pool.clone(), db_url).await);

	// Composite: JWT -> Database
	let composite = CompositeAuthentication::new()
		.with_backend((*jwt_auth).clone())
		.with_backend((*db_backend).clone());

	// Insert test user into database
	let user_id = Uuid::new_v4();
	db_backend
		.insert_user(user_id, "fallback_user", "fallback@db.com", true, true)
		.await;

	// Request with database header (no JWT)
	let mut headers = HeaderMap::new();
	headers.insert("X-User-ID", user_id.to_string().parse().unwrap());
	let request = build_test_request(Method::POST, "/api/action", headers, Bytes::new());

	// Authenticate - should fallback to database
	let result = AuthenticationBackend::authenticate(&composite, &request)
		.await
		.unwrap();
	let user = result.unwrap();

	assert_eq!(user.id(), user_id.to_string());
	assert_eq!(user.get_username(), "fallback_user");
	assert!(user.is_staff());
}

/// Test database backend with multiple users
#[rstest]
#[tokio::test]
async fn test_database_backend_multiple_users(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<sqlx::PgPool>, u16, String),
) {
	// Test intent: Verify DatabaseAuthBackend correctly differentiates between
	// multiple users stored in database, returning correct user for each request.
	// Not intent: Concurrent user creation, user isolation, transaction handling
	let (_container, pool, _port, db_url) = postgres_container.await;

	let db_backend = DatabaseAuthBackend::new(pool.clone(), db_url).await;

	// Insert multiple users
	let user1_id = Uuid::new_v4();
	let user2_id = Uuid::new_v4();
	db_backend
		.insert_user(user1_id, "alice", "alice@db.com", true, false)
		.await;
	db_backend
		.insert_user(user2_id, "bob", "bob@db.com", true, true)
		.await;

	// Request for user 1
	let mut headers1 = HeaderMap::new();
	headers1.insert("X-User-ID", user1_id.to_string().parse().unwrap());
	let request1 = build_test_request(Method::GET, "/api/user1", headers1, Bytes::new());

	let result1 = db_backend.authenticate(&request1).await.unwrap();
	let user1 = result1.unwrap();
	assert_eq!(user1.get_username(), "alice");
	assert!(!user1.is_staff());

	// Request for user 2
	let mut headers2 = HeaderMap::new();
	headers2.insert("X-User-ID", user2_id.to_string().parse().unwrap());
	let request2 = build_test_request(Method::GET, "/api/user2", headers2, Bytes::new());

	let result2 = db_backend.authenticate(&request2).await.unwrap();
	let user2 = result2.unwrap();
	assert_eq!(user2.get_username(), "bob");
	assert!(user2.is_staff());
}

/// Test all backends (JWT + Session + Database) in chain
#[rstest]
#[tokio::test]
async fn test_all_backends_jwt_session_database_chain(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<sqlx::PgPool>, u16, String),
) {
	// Test intent: Verify CompositeAuthentication with full chain (JWT -> Session -> Database)
	// successfully falls back through all backends to authenticate via database.
	// Not intent: Performance optimization, caching, connection pooling
	let (_container, pool, _port, db_url) = postgres_container.await;

	// Create all three backends
	let jwt_auth = Arc::new(JwtAuth::new(b"full_chain_secret"));
	let session_backend = Arc::new(InMemorySessionBackend::new());
	let session_auth = Arc::new(SessionAuthentication::new((*session_backend).clone()));
	let db_backend = Arc::new(DatabaseAuthBackend::new(pool.clone(), db_url).await);

	// Composite: JWT -> Session -> Database
	let composite = CompositeAuthentication::new()
		.with_backend((*jwt_auth).clone())
		.with_backend((*session_auth).clone())
		.with_backend((*db_backend).clone());

	// Insert user into database
	let user_id = Uuid::new_v4();
	db_backend
		.insert_user(user_id, "chain_user", "chain@db.com", true, false)
		.await;

	// Request with only database header (JWT and Session will fail)
	let mut headers = HeaderMap::new();
	headers.insert("X-User-ID", user_id.to_string().parse().unwrap());
	let request = build_test_request(Method::GET, "/api/final", headers, Bytes::new());

	// Authenticate - should try JWT (fail), Session (fail), Database (success)
	let result = AuthenticationBackend::authenticate(&composite, &request)
		.await
		.unwrap();
	let user = result.unwrap();

	assert_eq!(user.id(), user_id.to_string());
	assert_eq!(user.get_username(), "chain_user");
}
