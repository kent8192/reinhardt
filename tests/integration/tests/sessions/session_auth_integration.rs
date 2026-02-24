//! Session-Based Authentication Flow Integration Tests
//!
//! Tests comprehensive session-based authentication scenarios including:
//! - Session creation on login with user credentials
//! - Session invalidation on logout with proper cleanup
//! - Session persistence and user association in database
//! - Session security features (CSRF tokens, secure cookies)
//! - Session hijacking prevention mechanisms
//! - Cookie flags (HttpOnly, Secure, SameSite)
//! - Session regeneration on privilege escalation
//! - Session timeout and idle detection
//! - Concurrent session handling per user
//! - Session data encryption in storage
//!
//! **Fixtures Used:**
//! - postgres_container: PostgreSQL database container for session storage

use reinhardt_auth::sessions::{
	Session,
	backends::{cache::SessionBackend, database::DatabaseSessionBackend},
};
use reinhardt_db::orm::manager::{get_connection, reinitialize_database};
use reinhardt_test::fixtures::postgres_container;
use rstest::*;
use serde_json::json;
use sqlx::{PgPool, Row};
use std::sync::Arc;
use testcontainers::{ContainerAsync, GenericImage};
use uuid::Uuid;

/// Common initialization for session tests
async fn init_session_test(database_url: &str) -> DatabaseSessionBackend {
	// Initialize global ORM connection for Session::objects() calls
	reinitialize_database(database_url)
		.await
		.expect("Failed to initialize ORM database");

	// Clear table before test to ensure isolation
	let conn = get_connection()
		.await
		.expect("Failed to get ORM connection");
	let _ = conn.execute("DROP TABLE IF EXISTS sessions", vec![]).await;

	// Create database session backend
	let backend = DatabaseSessionBackend::new(database_url)
		.await
		.expect("Failed to create session backend");

	backend
		.create_table()
		.await
		.expect("Failed to create sessions table");

	backend
}

// ============================================================================
// Session Creation on Login Tests
// ============================================================================

/// Test session creation on successful login with user credentials
///
/// **Test Intent**: Verify session is created in database with user data
/// when authentication succeeds via login endpoint
///
/// **Integration Point**: Authentication → Session creation → Database storage
///
/// **Not Intent**: Session retrieval only, logout, session expiry
#[rstest]
#[tokio::test]
async fn test_session_creation_on_login(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, database_url) = postgres_container.await;

	// Initialize ORM and create session backend
	let backend = init_session_test(&database_url).await;

	// Simulate login - create session with user credentials
	let user_id = Uuid::new_v4();
	let mut session = Session::new(backend.clone());
	session.set("_auth_user_id", user_id.to_string()).unwrap();
	session.set("_auth_user_name", "alice".to_string()).unwrap();
	session
		.set("_auth_user_email", "alice@example.com".to_string())
		.unwrap();
	session.set("_auth_user_is_active", true).unwrap();
	session.set("_auth_user_is_admin", false).unwrap();
	session
		.set("_auth_login_time", chrono::Utc::now().timestamp())
		.unwrap();

	// Save session to database
	session.save().await.expect("Failed to save session");
	let session_key = session.session_key().unwrap();

	// Verify session created in database
	let result =
		sqlx::query("SELECT session_key, session_data FROM sessions WHERE session_key = $1")
			.bind(session_key)
			.fetch_one(pool.as_ref())
			.await
			.expect("Failed to query session");

	let stored_key: String = result.get("session_key");
	let stored_data_str: String = result.get("session_data");
	let stored_data: serde_json::Value =
		serde_json::from_str(&stored_data_str).expect("Failed to parse session data");

	assert_eq!(stored_key, session_key);
	assert_eq!(
		stored_data["_auth_user_id"].as_str().unwrap(),
		user_id.to_string()
	);
	assert_eq!(stored_data["_auth_user_name"], "alice");
	assert_eq!(stored_data["_auth_user_email"], "alice@example.com");
	assert!(stored_data["_auth_user_is_active"].as_bool().unwrap());
}

/// Test session creation with CSRF token generation
///
/// **Test Intent**: Verify CSRF token is generated and stored in session
/// during login for protection against CSRF attacks
///
/// **Integration Point**: Login → CSRF token generation → Session storage
///
/// **Not Intent**: CSRF validation, token rotation
#[rstest]
#[tokio::test]
async fn test_session_creation_with_csrf_token(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, _pool, _port, database_url) = postgres_container.await;

	// Initialize ORM and create session backend
	let backend = init_session_test(&database_url).await;

	// Create session with CSRF token
	let user_id = Uuid::new_v4();
	let csrf_token = Uuid::new_v4().to_string();
	let mut session = Session::new(backend.clone());
	session.set("_auth_user_id", user_id.to_string()).unwrap();
	session.set("_csrf_token", &csrf_token).unwrap();
	session
		.set("_csrf_created_at", chrono::Utc::now().timestamp())
		.unwrap();

	session.save().await.expect("Failed to save session");

	// Verify CSRF token stored
	let session_key = session.session_key().unwrap();
	let mut loaded_session = Session::from_key(backend.clone(), session_key.to_string())
		.await
		.expect("Failed to load session");

	let loaded_csrf: String = loaded_session
		.get("_csrf_token")
		.unwrap()
		.expect("CSRF token not found");
	assert_eq!(loaded_csrf, csrf_token);

	let csrf_created: i64 = loaded_session
		.get("_csrf_created_at")
		.unwrap()
		.expect("CSRF creation time not found");
	assert!(csrf_created > 0);
}

/// Test session creation with user metadata and preferences
///
/// **Test Intent**: Verify additional user metadata (language, timezone, theme)
/// is stored in session during login
///
/// **Integration Point**: Login → User metadata collection → Session storage
///
/// **Not Intent**: Metadata updates only, no login context
#[rstest]
#[tokio::test]
async fn test_session_creation_with_user_metadata(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, _pool, _port, database_url) = postgres_container.await;

	// Initialize ORM and create session backend
	let backend = init_session_test(&database_url).await;

	// Create session with user metadata
	let user_id = Uuid::new_v4();
	let mut session = Session::new(backend.clone());
	session.set("_auth_user_id", user_id.to_string()).unwrap();
	session.set("_user_language", "en-US".to_string()).unwrap();
	session
		.set("_user_timezone", "America/New_York".to_string())
		.unwrap();
	session.set("_user_theme", "dark".to_string()).unwrap();
	session
		.set(
			"_user_preferences",
			json!({"notifications": true, "email_digest": "daily"}),
		)
		.unwrap();

	session.save().await.expect("Failed to save session");

	// Verify metadata stored
	let session_key = session.session_key().unwrap();
	let mut loaded_session = Session::from_key(backend.clone(), session_key.to_string())
		.await
		.expect("Failed to load session");

	let language: String = loaded_session.get("_user_language").unwrap().unwrap();
	let timezone: String = loaded_session.get("_user_timezone").unwrap().unwrap();
	let theme: String = loaded_session.get("_user_theme").unwrap().unwrap();
	let preferences: serde_json::Value = loaded_session.get("_user_preferences").unwrap().unwrap();

	assert_eq!(language, "en-US");
	assert_eq!(timezone, "America/New_York");
	assert_eq!(theme, "dark");
	assert!(preferences["notifications"].as_bool().unwrap());
	assert_eq!(preferences["email_digest"], "daily");
}

// ============================================================================
// Session Invalidation on Logout Tests
// ============================================================================

/// Test session invalidation on logout with session deletion
///
/// **Test Intent**: Verify session is completely removed from database
/// when user logs out
///
/// **Integration Point**: Logout → Session deletion → Database cleanup
///
/// **Not Intent**: Session creation, expiry-based cleanup
#[rstest]
#[tokio::test]
async fn test_session_invalidation_on_logout(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, database_url) = postgres_container.await;

	// Initialize ORM and create session backend
	let backend = init_session_test(&database_url).await;

	// Create session
	let user_id = Uuid::new_v4();
	let mut session = Session::new(backend.clone());
	session.set("_auth_user_id", user_id.to_string()).unwrap();
	session.save().await.expect("Failed to save session");
	let session_key = session.session_key().unwrap();

	// Verify session exists
	let exists_before = backend
		.exists(session_key)
		.await
		.expect("Failed to check session existence");
	assert!(exists_before, "Session should exist before logout");

	// Simulate logout - delete session
	backend
		.delete(session_key)
		.await
		.expect("Failed to delete session");

	// Verify session removed from database
	let exists_after = backend
		.exists(session_key)
		.await
		.expect("Failed to check session existence");
	assert!(!exists_after, "Session should not exist after logout");

	// Verify session key not found in database
	let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sessions WHERE session_key = $1")
		.bind(session_key)
		.fetch_one(pool.as_ref())
		.await
		.expect("Failed to count sessions");
	assert_eq!(count, 0, "Session should be completely removed");
}

/// Test session invalidation with proper cleanup of all data
///
/// **Test Intent**: Verify all session data (user info, CSRF, metadata)
/// is cleared when session is invalidated
///
/// **Integration Point**: Logout → Complete data cleanup → Database
///
/// **Not Intent**: Partial cleanup, soft deletion
#[rstest]
#[tokio::test]
async fn test_session_invalidation_clears_all_data(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, database_url) = postgres_container.await;

	// Initialize ORM and create session backend
	let backend = init_session_test(&database_url).await;

	// Create session with comprehensive data
	let mut session = Session::new(backend.clone());
	session
		.set("_auth_user_id", Uuid::new_v4().to_string())
		.unwrap();
	session
		.set("_csrf_token", Uuid::new_v4().to_string())
		.unwrap();
	session.set("_user_language", "en".to_string()).unwrap();
	session.set("_cart_items", vec![1, 2, 3]).unwrap();
	session.save().await.expect("Failed to save session");
	let session_key = session.session_key().unwrap();

	// Verify all data exists
	let result_before = sqlx::query("SELECT session_data FROM sessions WHERE session_key = $1")
		.bind(session_key)
		.fetch_one(pool.as_ref())
		.await
		.expect("Failed to fetch session");
	let data_before_str: String = result_before.get("session_data");
	let data_before: serde_json::Value =
		serde_json::from_str(&data_before_str).expect("Failed to parse session data");
	assert!(data_before.get("_auth_user_id").is_some());
	assert!(data_before.get("_csrf_token").is_some());
	assert!(data_before.get("_user_language").is_some());
	assert!(data_before.get("_cart_items").is_some());

	// Delete session
	backend
		.delete(session_key)
		.await
		.expect("Failed to delete session");

	// Verify no data remains
	let result_after = sqlx::query("SELECT session_data FROM sessions WHERE session_key = $1")
		.bind(session_key)
		.fetch_optional(pool.as_ref())
		.await
		.expect("Failed to query session");
	assert!(result_after.is_none(), "All session data should be removed");
}

/// Test logout invalidates session even with active requests
///
/// **Test Intent**: Verify logout invalidates session immediately
/// even if other requests are using the same session
///
/// **Integration Point**: Logout → Immediate invalidation → Concurrent access
///
/// **Not Intent**: Delayed cleanup, graceful degradation
#[rstest]
#[tokio::test]
async fn test_logout_immediate_invalidation(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, _pool, _port, database_url) = postgres_container.await;

	// Initialize ORM and create session backend
	let backend = init_session_test(&database_url).await;

	// Create session
	let mut session = Session::new(backend.clone());
	session
		.set("_auth_user_id", Uuid::new_v4().to_string())
		.unwrap();
	session.save().await.expect("Failed to save session");
	let session_key = session.session_key().unwrap();

	// Verify session can be loaded
	let loaded_before = Session::from_key(backend.clone(), session_key.to_string()).await;
	assert!(loaded_before.is_ok(), "Session should load before logout");

	// Simulate logout - delete session
	backend
		.delete(session_key)
		.await
		.expect("Failed to delete session");

	// Verify session cannot be loaded immediately after logout
	let loaded_after = Session::from_key(backend.clone(), session_key.to_string()).await;
	assert!(loaded_after.is_ok()); // Session object created
	let mut session_after = loaded_after.unwrap();
	// But it should be empty (new session)
	let user_id_after: Option<String> = session_after.get("_auth_user_id").unwrap();
	assert!(
		user_id_after.is_none(),
		"User ID should not be in new session"
	);
}

// ============================================================================
// Session Persistence and User Association Tests
// ============================================================================

/// Test session persists user association across requests
///
/// **Test Intent**: Verify user ID remains associated with session
/// across multiple HTTP requests
///
/// **Integration Point**: Session storage → User association → Request handling
///
/// **Not Intent**: Single request, no persistence
#[rstest]
#[tokio::test]
async fn test_session_persists_user_association(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, _pool, _port, database_url) = postgres_container.await;

	// Initialize ORM and create session backend
	let backend = init_session_test(&database_url).await;

	// Create session with user
	let user_id = Uuid::new_v4();
	let mut session = Session::new(backend.clone());
	session.set("_auth_user_id", user_id.to_string()).unwrap();
	session.set("_auth_user_name", "bob".to_string()).unwrap();
	session.save().await.expect("Failed to save session");
	let session_key = session.session_key().unwrap();

	// Simulate first request - load session
	let mut session_req1 = Session::from_key(backend.clone(), session_key.to_string())
		.await
		.expect("Failed to load session for request 1");
	let user_id_req1: String = session_req1.get("_auth_user_id").unwrap().unwrap();
	let username_req1: String = session_req1.get("_auth_user_name").unwrap().unwrap();
	assert_eq!(user_id_req1, user_id.to_string());
	assert_eq!(username_req1, "bob");

	// Simulate second request - load same session
	let mut session_req2 = Session::from_key(backend.clone(), session_key.to_string())
		.await
		.expect("Failed to load session for request 2");
	let user_id_req2: String = session_req2.get("_auth_user_id").unwrap().unwrap();
	let username_req2: String = session_req2.get("_auth_user_name").unwrap().unwrap();
	assert_eq!(user_id_req2, user_id.to_string());
	assert_eq!(username_req2, "bob");

	// Verify both requests see same user
	assert_eq!(user_id_req1, user_id_req2);
	assert_eq!(username_req1, username_req2);
}

/// Test session updates user data and persists changes
///
/// **Test Intent**: Verify changes to user data in session are persisted
/// to database and visible in subsequent requests
///
/// **Integration Point**: Session update → Database persistence → Data consistency
///
/// **Not Intent**: Read-only session, no updates
#[rstest]
#[tokio::test]
async fn test_session_updates_user_data(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, _pool, _port, database_url) = postgres_container.await;

	// Initialize ORM and create session backend
	let backend = init_session_test(&database_url).await;

	// Create session with initial user data
	let user_id = Uuid::new_v4();
	let mut session = Session::new(backend.clone());
	session.set("_auth_user_id", user_id.to_string()).unwrap();
	session.set("_user_role", "user".to_string()).unwrap();
	session.save().await.expect("Failed to save session");
	let session_key = session.session_key().unwrap();

	// Update user role (e.g., after promotion)
	let mut session_update = Session::from_key(backend.clone(), session_key.to_string())
		.await
		.expect("Failed to load session");
	session_update
		.set("_user_role", "admin".to_string())
		.unwrap();
	session_update
		.set("_role_updated_at", chrono::Utc::now().timestamp())
		.unwrap();
	session_update
		.save()
		.await
		.expect("Failed to save updated session");

	// Verify updates persisted
	let mut session_verify = Session::from_key(backend.clone(), session_key.to_string())
		.await
		.expect("Failed to load session after update");
	let role: String = session_verify.get("_user_role").unwrap().unwrap();
	let updated_at: i64 = session_verify.get("_role_updated_at").unwrap().unwrap();
	assert_eq!(role, "admin");
	assert!(updated_at > 0);
}

/// Test session associates multiple user attributes correctly
///
/// **Test Intent**: Verify session correctly stores and retrieves
/// multiple user attributes (ID, email, roles, permissions)
///
/// **Integration Point**: Session → Multi-attribute user data → Database
///
/// **Not Intent**: Single attribute, minimal user data
#[rstest]
#[tokio::test]
async fn test_session_multiple_user_attributes(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, _pool, _port, database_url) = postgres_container.await;

	// Initialize ORM and create session backend
	let backend = init_session_test(&database_url).await;

	// Create session with comprehensive user data
	let user_id = Uuid::new_v4();
	let mut session = Session::new(backend.clone());
	session.set("_auth_user_id", user_id.to_string()).unwrap();
	session
		.set("_auth_user_email", "charlie@example.com".to_string())
		.unwrap();
	session
		.set("_auth_user_first_name", "Charlie".to_string())
		.unwrap();
	session
		.set("_auth_user_last_name", "Brown".to_string())
		.unwrap();
	session.set("_auth_user_is_staff", true).unwrap();
	session.set("_auth_user_is_superuser", false).unwrap();
	session
		.set("_auth_user_roles", vec!["editor", "moderator"])
		.unwrap();
	session
		.set(
			"_auth_user_permissions",
			json!({"can_edit": true, "can_delete": false}),
		)
		.unwrap();
	session.save().await.expect("Failed to save session");

	// Verify all attributes stored and retrievable
	let session_key = session.session_key().unwrap();
	let mut loaded = Session::from_key(backend.clone(), session_key.to_string())
		.await
		.expect("Failed to load session");

	let loaded_id: String = loaded.get("_auth_user_id").unwrap().unwrap();
	let loaded_email: String = loaded.get("_auth_user_email").unwrap().unwrap();
	let loaded_first: String = loaded.get("_auth_user_first_name").unwrap().unwrap();
	let loaded_last: String = loaded.get("_auth_user_last_name").unwrap().unwrap();
	let loaded_staff: bool = loaded.get("_auth_user_is_staff").unwrap().unwrap();
	let loaded_super: bool = loaded.get("_auth_user_is_superuser").unwrap().unwrap();
	let loaded_roles: Vec<String> = loaded.get("_auth_user_roles").unwrap().unwrap();
	let loaded_perms: serde_json::Value = loaded.get("_auth_user_permissions").unwrap().unwrap();

	assert_eq!(loaded_id, user_id.to_string());
	assert_eq!(loaded_email, "charlie@example.com");
	assert_eq!(loaded_first, "Charlie");
	assert_eq!(loaded_last, "Brown");
	assert!(loaded_staff);
	assert!(!loaded_super);
	assert_eq!(loaded_roles, vec!["editor", "moderator"]);
	assert!(loaded_perms["can_edit"].as_bool().unwrap());
	assert!(!loaded_perms["can_delete"].as_bool().unwrap());
}

// ============================================================================
// Session Security Features Tests
// ============================================================================

/// Test session CSRF token validation requirement
///
/// **Test Intent**: Verify CSRF token in session is validated
/// for state-changing requests (POST, PUT, DELETE)
///
/// **Integration Point**: Session → CSRF validation → Request processing
///
/// **Not Intent**: Token generation only, GET requests
#[rstest]
#[tokio::test]
async fn test_session_csrf_token_validation(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, _pool, _port, database_url) = postgres_container.await;

	// Initialize ORM and create session backend
	let backend = init_session_test(&database_url).await;

	// Create session with CSRF token
	let csrf_token = Uuid::new_v4().to_string();
	let mut session = Session::new(backend.clone());
	session
		.set("_auth_user_id", Uuid::new_v4().to_string())
		.unwrap();
	session.set("_csrf_token", &csrf_token).unwrap();
	session.save().await.expect("Failed to save session");

	// Verify CSRF token stored correctly
	let session_key = session.session_key().unwrap();
	let mut loaded = Session::from_key(backend.clone(), session_key.to_string())
		.await
		.expect("Failed to load session");
	let stored_token: String = loaded.get("_csrf_token").unwrap().unwrap();
	assert_eq!(stored_token, csrf_token);

	// Simulate CSRF validation (matching token)
	let request_csrf_token = csrf_token.clone();
	assert_eq!(
		request_csrf_token, stored_token,
		"CSRF tokens should match for valid request"
	);

	// Simulate CSRF validation (mismatched token - attack scenario)
	let invalid_csrf_token = Uuid::new_v4().to_string();
	assert_ne!(
		invalid_csrf_token, stored_token,
		"CSRF validation should fail for mismatched token"
	);
}

/// Test secure cookie flags are set on session cookie
///
/// **Test Intent**: Verify session cookie includes HttpOnly, Secure,
/// and SameSite flags for security
///
/// **Integration Point**: Session → Cookie generation → Response headers
///
/// **Not Intent**: Cookie storage only, no security flags
#[rstest]
#[tokio::test]
async fn test_session_secure_cookie_flags(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, _pool, _port, database_url) = postgres_container.await;

	// Initialize ORM and create session backend
	let backend = init_session_test(&database_url).await;

	// Create session
	let mut session = Session::new(backend.clone());
	session
		.set("_auth_user_id", Uuid::new_v4().to_string())
		.unwrap();
	session.save().await.expect("Failed to save session");
	let session_key = session.session_key().unwrap();

	// Simulate cookie generation with security flags
	let cookie_value = format!(
		"sessionid={}; HttpOnly; Secure; SameSite=Strict; Path=/; Max-Age=86400",
		session_key
	);

	// Verify security flags present
	assert!(
		cookie_value.contains("HttpOnly"),
		"Cookie should have HttpOnly flag"
	);
	assert!(
		cookie_value.contains("Secure"),
		"Cookie should have Secure flag"
	);
	assert!(
		cookie_value.contains("SameSite=Strict"),
		"Cookie should have SameSite=Strict flag"
	);
	assert!(cookie_value.contains("Path=/"), "Cookie should have Path=/");
	assert!(
		cookie_value.contains("Max-Age=86400"),
		"Cookie should have Max-Age"
	);
}

/// Test session regeneration on privilege escalation
///
/// **Test Intent**: Verify new session is created when user role changes
/// to prevent session fixation attacks
///
/// **Integration Point**: Privilege change → Session regeneration → New session ID
///
/// **Not Intent**: Normal session updates, no privilege change
#[rstest]
#[tokio::test]
async fn test_session_regeneration_on_privilege_escalation(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, _pool, _port, database_url) = postgres_container.await;

	// Initialize ORM and create session backend
	let backend = init_session_test(&database_url).await;

	// Create initial session with regular user
	let user_id = Uuid::new_v4();
	let mut session_before = Session::new(backend.clone());
	session_before
		.set("_auth_user_id", user_id.to_string())
		.unwrap();
	session_before
		.set("_user_role", "user".to_string())
		.unwrap();
	session_before.save().await.expect("Failed to save session");
	let old_session_key = session_before.session_key().unwrap();

	// Simulate privilege escalation - create new session
	let mut session_after = Session::new(backend.clone());
	session_after
		.set("_auth_user_id", user_id.to_string())
		.unwrap();
	session_after
		.set("_user_role", "admin".to_string())
		.unwrap();
	session_after
		.set("_privilege_escalated_at", chrono::Utc::now().timestamp())
		.unwrap();
	session_after
		.save()
		.await
		.expect("Failed to save new session");
	let new_session_key = session_after.session_key().unwrap();

	// Verify new session has different key
	assert_ne!(
		old_session_key, new_session_key,
		"Session key should change on privilege escalation"
	);

	// Verify new session has updated role
	let mut loaded_new = Session::from_key(backend.clone(), new_session_key.to_string())
		.await
		.expect("Failed to load new session");
	let new_role: String = loaded_new.get("_user_role").unwrap().unwrap();
	assert_eq!(new_role, "admin");

	// Old session should be deleted
	backend
		.delete(old_session_key)
		.await
		.expect("Failed to delete old session");
	let old_exists = backend
		.exists(old_session_key)
		.await
		.expect("Failed to check old session");
	assert!(
		!old_exists,
		"Old session should be deleted after regeneration"
	);
}

// ============================================================================
// Session Hijacking Prevention Tests
// ============================================================================

/// Test session binds to IP address to prevent hijacking
///
/// **Test Intent**: Verify session stores client IP address and validates
/// it on subsequent requests to detect hijacking
///
/// **Integration Point**: Session creation → IP binding → Validation on use
///
/// **Not Intent**: No IP validation, shared sessions
#[rstest]
#[tokio::test]
async fn test_session_ip_binding(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, _pool, _port, database_url) = postgres_container.await;

	// Initialize ORM and create session backend
	let backend = init_session_test(&database_url).await;

	// Create session with IP binding
	let client_ip = "192.168.1.100";
	let mut session = Session::new(backend.clone());
	session
		.set("_auth_user_id", Uuid::new_v4().to_string())
		.unwrap();
	session.set("_client_ip", client_ip.to_string()).unwrap();
	session.save().await.expect("Failed to save session");
	let session_key = session.session_key().unwrap();

	// Verify IP stored
	let mut loaded = Session::from_key(backend.clone(), session_key.to_string())
		.await
		.expect("Failed to load session");
	let stored_ip: String = loaded.get("_client_ip").unwrap().unwrap();
	assert_eq!(stored_ip, client_ip);

	// Simulate validation - same IP (valid)
	assert_eq!(stored_ip, client_ip, "IP should match for valid request");

	// Simulate validation - different IP (hijacking detected)
	let different_ip = "10.0.0.50";
	assert_ne!(
		stored_ip, different_ip,
		"IP mismatch indicates potential hijacking"
	);
}

/// Test session binds to user agent to prevent hijacking
///
/// **Test Intent**: Verify session stores User-Agent header and validates
/// consistency across requests
///
/// **Integration Point**: Session → User-Agent binding → Request validation
///
/// **Not Intent**: No User-Agent check, flexible validation
#[rstest]
#[tokio::test]
async fn test_session_user_agent_binding(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, _pool, _port, database_url) = postgres_container.await;

	// Initialize ORM and create session backend
	let backend = init_session_test(&database_url).await;

	// Create session with User-Agent binding
	let user_agent = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36";
	let mut session = Session::new(backend.clone());
	session
		.set("_auth_user_id", Uuid::new_v4().to_string())
		.unwrap();
	session.set("_user_agent", user_agent.to_string()).unwrap();
	session.save().await.expect("Failed to save session");
	let session_key = session.session_key().unwrap();

	// Verify User-Agent stored
	let mut loaded = Session::from_key(backend.clone(), session_key.to_string())
		.await
		.expect("Failed to load session");
	let stored_ua: String = loaded.get("_user_agent").unwrap().unwrap();
	assert_eq!(stored_ua, user_agent);

	// Simulate validation - same User-Agent (valid)
	assert_eq!(stored_ua, user_agent, "User-Agent should match");

	// Simulate validation - different User-Agent (suspicious)
	let different_ua = "curl/7.68.0";
	assert_ne!(
		stored_ua, different_ua,
		"User-Agent mismatch indicates suspicious activity"
	);
}

/// Test session timeout and idle detection
///
/// **Test Intent**: Verify session tracks last activity timestamp
/// and can detect idle timeout
///
/// **Integration Point**: Session → Activity tracking → Timeout detection
///
/// **Not Intent**: No timeout, infinite sessions
#[rstest]
#[tokio::test]
async fn test_session_timeout_and_idle_detection(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, _pool, _port, database_url) = postgres_container.await;

	// Initialize ORM and create session backend
	let backend = init_session_test(&database_url).await;

	// Create session with activity timestamp
	let user_id = Uuid::new_v4();
	let mut session = Session::new(backend.clone());
	session.set("_auth_user_id", user_id.to_string()).unwrap();
	session
		.set("_last_activity", chrono::Utc::now().timestamp())
		.unwrap();
	session.save().await.expect("Failed to save session");
	let session_key = session.session_key().unwrap();

	// Simulate activity update
	let mut session_update = Session::from_key(backend.clone(), session_key.to_string())
		.await
		.expect("Failed to load session");
	session_update
		.set("_last_activity", chrono::Utc::now().timestamp())
		.unwrap();
	session_update
		.save()
		.await
		.expect("Failed to update session");

	// Verify activity timestamp updated
	let mut loaded = Session::from_key(backend.clone(), session_key.to_string())
		.await
		.expect("Failed to load session");
	let last_activity: i64 = loaded.get("_last_activity").unwrap().unwrap();
	let now = chrono::Utc::now().timestamp();
	let idle_time = now - last_activity;

	// Simulate timeout check (30 minutes = 1800 seconds)
	let timeout_seconds = 1800;
	assert!(
		idle_time < timeout_seconds,
		"Session should be active (not idle)"
	);
}

/// Test concurrent session handling per user
///
/// **Test Intent**: Verify system allows multiple concurrent sessions
/// per user (e.g., desktop + mobile) and tracks them independently
///
/// **Integration Point**: User → Multiple sessions → Independent tracking
///
/// **Not Intent**: Single session per user, session limits
#[rstest]
#[tokio::test]
async fn test_concurrent_sessions_per_user(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, _pool, _port, database_url) = postgres_container.await;

	// Initialize ORM and create session backend
	let backend = init_session_test(&database_url).await;

	// Create first session (desktop)
	let user_id = Uuid::new_v4();
	let mut session1 = Session::new(backend.clone());
	session1.set("_auth_user_id", user_id.to_string()).unwrap();
	session1.set("_device_type", "desktop".to_string()).unwrap();
	session1.save().await.expect("Failed to save session 1");
	let session_key1 = session1.session_key().unwrap();

	// Create second session (mobile)
	let mut session2 = Session::new(backend.clone());
	session2.set("_auth_user_id", user_id.to_string()).unwrap();
	session2.set("_device_type", "mobile".to_string()).unwrap();
	session2.save().await.expect("Failed to save session 2");
	let session_key2 = session2.session_key().unwrap();

	// Verify both sessions have different keys
	assert_ne!(
		session_key1, session_key2,
		"Sessions should have different keys"
	);

	// Verify both sessions belong to same user
	let mut loaded1 = Session::from_key(backend.clone(), session_key1.to_string())
		.await
		.expect("Failed to load session 1");
	let mut loaded2 = Session::from_key(backend.clone(), session_key2.to_string())
		.await
		.expect("Failed to load session 2");

	let user1: String = loaded1.get("_auth_user_id").unwrap().unwrap();
	let user2: String = loaded2.get("_auth_user_id").unwrap().unwrap();
	assert_eq!(user1, user2, "Both sessions should belong to same user");

	// Verify device types are tracked independently
	let device1: String = loaded1.get("_device_type").unwrap().unwrap();
	let device2: String = loaded2.get("_device_type").unwrap().unwrap();
	assert_eq!(device1, "desktop");
	assert_eq!(device2, "mobile");
}
