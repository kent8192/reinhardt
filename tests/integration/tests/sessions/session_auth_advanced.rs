//! Advanced Session-Based Authentication Integration Tests
//!
//! Tests advanced session authentication scenarios including:
//! - Remember-me token integration for automatic login
//! - Multi-factor authentication (MFA) verification flow
//! - Password change invalidating all user sessions
//! - Complete session data removal on logout
//!
//! **Fixtures Used:**
//! - postgres_container: PostgreSQL database container for session storage
//!
//! **Dependencies:**
//! - reinhardt-sessions: Session management
//! - sqlx: Database operations
//! - serial_test: Test serialization for shared state

use reinhardt_auth::sessions::backends::cache::SessionBackend;
use reinhardt_auth::sessions::{Session, backends::database::DatabaseSessionBackend};
use reinhardt_db::orm::manager::{get_connection, reinitialize_database};
use reinhardt_test::fixtures::postgres_container;
use rstest::*;
use serial_test::serial;
use sqlx::{PgPool, Row};
use std::sync::Arc;
use testcontainers::{ContainerAsync, GenericImage};
use uuid::Uuid;

// ============================================================================
// Remember-Me Token Integration Tests
// ============================================================================

/// Test remember_me token enables automatic login after session expiry
///
/// **Test Intent**: Verify remember_me token stored in session allows
/// automatic re-authentication without password when session expires
///
/// **Integration Point**: Session → Remember-me token → Automatic login
///
/// **Not Intent**: Password-based login, token generation only
#[rstest]
#[serial(sessions)]
#[tokio::test]
async fn test_remember_me_token_integration(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, database_url) = postgres_container.await;

	// Initialize global ORM connection for Session::objects() calls
	reinitialize_database(&database_url)
		.await
		.expect("Failed to initialize ORM database");

	// Clear table before test to ensure isolation
	let conn = get_connection()
		.await
		.expect("Failed to get ORM connection");
	let _ = conn.execute("DROP TABLE IF EXISTS sessions", vec![]).await;

	// Create database session backend
	let backend = DatabaseSessionBackend::new(&database_url)
		.await
		.expect("Failed to create session backend");

	backend
		.create_table()
		.await
		.expect("Failed to create sessions table");

	// Create initial session with user and remember_me token
	let user_id = Uuid::new_v4();
	let remember_me_token = Uuid::new_v4().to_string();
	let mut session = Session::new(backend.clone());
	session.set("_auth_user_id", user_id.to_string()).unwrap();
	session.set("_auth_user_name", "alice".to_string()).unwrap();
	session
		.set("_remember_me_token", &remember_me_token)
		.unwrap();
	session
		.set("_remember_me_created_at", chrono::Utc::now().timestamp())
		.unwrap();
	session
		.set("_remember_me_expires_in_days", 30) // 30 days
		.unwrap();
	session.save().await.expect("Failed to save session");
	let old_session_key = session.session_key().unwrap();

	// Verify initial session has remember_me token
	let result_before = sqlx::query("SELECT session_data FROM sessions WHERE session_key = $1")
		.bind(old_session_key)
		.fetch_one(pool.as_ref())
		.await
		.expect("Failed to query session");
	let data_before_str: String = result_before.get("session_data");
	let data_before: serde_json::Value =
		serde_json::from_str(&data_before_str).expect("Failed to parse session data");
	assert_eq!(
		data_before["_remember_me_token"].as_str().unwrap(),
		remember_me_token
	);
	assert!(data_before["_remember_me_created_at"].as_i64().is_some());
	assert_eq!(data_before["_remember_me_expires_in_days"], 30);

	// Simulate session expiry - delete session
	backend
		.delete(old_session_key)
		.await
		.expect("Failed to delete expired session");

	// Simulate automatic login via remember_me token
	// (In real implementation, this would check remember_me token from cookie)
	let mut new_session = Session::new(backend.clone());
	new_session
		.set("_auth_user_id", user_id.to_string())
		.unwrap();
	new_session
		.set("_auth_user_name", "alice".to_string())
		.unwrap();
	new_session
		.set("_remember_me_token", &remember_me_token)
		.unwrap();
	new_session
		.set("_auto_login_timestamp", chrono::Utc::now().timestamp())
		.unwrap();
	new_session
		.save()
		.await
		.expect("Failed to save new session");
	let new_session_key = new_session.session_key().unwrap();

	// Verify new session created with same user and remember_me token
	assert_ne!(
		old_session_key, new_session_key,
		"New session should have different key"
	);

	let mut loaded_session = Session::from_key(backend.clone(), new_session_key.to_string())
		.await
		.expect("Failed to load new session");
	let loaded_user_id: String = loaded_session.get("_auth_user_id").unwrap().unwrap();
	let loaded_token: String = loaded_session.get("_remember_me_token").unwrap().unwrap();
	let auto_login_timestamp: i64 = loaded_session
		.get("_auto_login_timestamp")
		.unwrap()
		.unwrap();

	assert_eq!(loaded_user_id, user_id.to_string());
	assert_eq!(loaded_token, remember_me_token);
	assert!(auto_login_timestamp > 0);
}

// ============================================================================
// MFA (Multi-Factor Authentication) Integration Tests
// ============================================================================

/// Test session state after MFA verification completion
///
/// **Test Intent**: Verify session transitions from partial authentication
/// to full authentication after successful MFA verification
///
/// **Integration Point**: Login → Partial session → MFA verification → Full session
///
/// **Not Intent**: Password-only login, no MFA flow
#[rstest]
#[serial(sessions)]
#[tokio::test]
async fn test_session_after_mfa_verification(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, database_url) = postgres_container.await;

	// Initialize global ORM connection for Session::objects() calls
	reinitialize_database(&database_url)
		.await
		.expect("Failed to initialize ORM database");

	// Clear table before test to ensure isolation
	let conn = get_connection()
		.await
		.expect("Failed to get ORM connection");
	let _ = conn.execute("DROP TABLE IF EXISTS sessions", vec![]).await;

	// Create database session backend
	let backend = DatabaseSessionBackend::new(&database_url)
		.await
		.expect("Failed to create session backend");

	backend
		.create_table()
		.await
		.expect("Failed to create sessions table");

	// Step 1: Create partial session after password verification
	let user_id = Uuid::new_v4();
	let mut partial_session = Session::new(backend.clone());
	partial_session
		.set("_auth_user_id", user_id.to_string())
		.unwrap();
	partial_session
		.set("_auth_user_name", "bob".to_string())
		.unwrap();
	partial_session
		.set("_mfa_pending", true) // MFA not yet verified
		.unwrap();
	partial_session
		.set("_mfa_method", "totp".to_string())
		.unwrap();
	partial_session
		.set("_password_verified_at", chrono::Utc::now().timestamp())
		.unwrap();
	partial_session
		.save()
		.await
		.expect("Failed to save partial session");
	let session_key = partial_session.session_key().unwrap();

	// Verify partial session state
	let result_partial = sqlx::query("SELECT session_data FROM sessions WHERE session_key = $1")
		.bind(session_key)
		.fetch_one(pool.as_ref())
		.await
		.expect("Failed to query partial session");
	let data_partial_str: String = result_partial.get("session_data");
	let data_partial: serde_json::Value =
		serde_json::from_str(&data_partial_str).expect("Failed to parse session data");
	assert!(data_partial["_mfa_pending"].as_bool().unwrap());
	assert_eq!(data_partial["_mfa_method"], "totp");
	assert!(data_partial["_mfa_verified_at"].is_null());

	// Step 2: Simulate MFA verification success
	let mut full_session = Session::from_key(backend.clone(), session_key.to_string())
		.await
		.expect("Failed to load partial session");
	full_session.set("_mfa_pending", false).unwrap();
	full_session
		.set("_mfa_verified_at", chrono::Utc::now().timestamp())
		.unwrap();
	full_session.set("_fully_authenticated", true).unwrap();
	full_session
		.save()
		.await
		.expect("Failed to save full session");

	// Verify full session state
	let result_full = sqlx::query("SELECT session_data FROM sessions WHERE session_key = $1")
		.bind(session_key)
		.fetch_one(pool.as_ref())
		.await
		.expect("Failed to query full session");
	let data_full_str: String = result_full.get("session_data");
	let data_full: serde_json::Value =
		serde_json::from_str(&data_full_str).expect("Failed to parse session data");
	assert!(!data_full["_mfa_pending"].as_bool().unwrap());
	assert!(data_full["_fully_authenticated"].as_bool().unwrap());
	assert!(data_full["_mfa_verified_at"].as_i64().is_some());

	// Verify session key remains the same (no regeneration)
	let mut loaded_full = Session::from_key(backend.clone(), session_key.to_string())
		.await
		.expect("Failed to load full session");
	let fully_authenticated: bool = loaded_full.get("_fully_authenticated").unwrap().unwrap();
	let mfa_verified_at: i64 = loaded_full.get("_mfa_verified_at").unwrap().unwrap();
	assert!(fully_authenticated);
	assert!(mfa_verified_at > 0);
}

// ============================================================================
// Password Change Session Invalidation Tests
// ============================================================================

/// Test all user sessions invalidated on password change
///
/// **Test Intent**: Verify when user changes password, all existing
/// sessions (across all devices) are invalidated for security
///
/// **Integration Point**: Password change → Invalidate all sessions → Database cleanup
///
/// **Not Intent**: Single session logout, selective invalidation
#[rstest]
#[serial(sessions)]
#[tokio::test]
async fn test_invalidate_all_sessions_on_password_change(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, database_url) = postgres_container.await;

	// Initialize global ORM connection for Session::objects() calls
	reinitialize_database(&database_url)
		.await
		.expect("Failed to initialize ORM database");

	// Clear table before test to ensure isolation
	let conn = get_connection()
		.await
		.expect("Failed to get ORM connection");
	let _ = conn.execute("DROP TABLE IF EXISTS sessions", vec![]).await;

	// Create database session backend
	let backend = DatabaseSessionBackend::new(&database_url)
		.await
		.expect("Failed to create session backend");

	backend
		.create_table()
		.await
		.expect("Failed to create sessions table");

	// Create multiple sessions for the same user (desktop, mobile, tablet)
	let user_id = Uuid::new_v4();
	let user_id_str = user_id.to_string();

	// Session 1: Desktop
	let mut session_desktop = Session::new(backend.clone());
	session_desktop.set("_auth_user_id", &user_id_str).unwrap();
	session_desktop
		.set("_device_type", "desktop".to_string())
		.unwrap();
	session_desktop
		.save()
		.await
		.expect("Failed to save desktop session");
	let session_key_desktop = session_desktop.session_key().unwrap();

	// Session 2: Mobile
	let mut session_mobile = Session::new(backend.clone());
	session_mobile.set("_auth_user_id", &user_id_str).unwrap();
	session_mobile
		.set("_device_type", "mobile".to_string())
		.unwrap();
	session_mobile
		.save()
		.await
		.expect("Failed to save mobile session");
	let session_key_mobile = session_mobile.session_key().unwrap();

	// Session 3: Tablet
	let mut session_tablet = Session::new(backend.clone());
	session_tablet.set("_auth_user_id", &user_id_str).unwrap();
	session_tablet
		.set("_device_type", "tablet".to_string())
		.unwrap();
	session_tablet
		.save()
		.await
		.expect("Failed to save tablet session");
	let session_key_tablet = session_tablet.session_key().unwrap();

	// Verify all sessions exist
	let count_before: i64 = sqlx::query_scalar(
		r#"
		SELECT COUNT(*)
		FROM sessions
		WHERE session_data::text LIKE $1
		"#,
	)
	.bind(format!("%{}%", user_id_str))
	.fetch_one(pool.as_ref())
	.await
	.expect("Failed to count sessions");
	assert_eq!(
		count_before, 3,
		"Should have 3 sessions before password change"
	);

	// Simulate password change - invalidate all sessions for this user
	// In real implementation, this would be triggered by password change event
	let sessions_to_delete = sqlx::query_scalar::<_, String>(
		"SELECT session_key FROM sessions WHERE session_data::text LIKE $1",
	)
	.bind(format!("%{}%", user_id_str))
	.fetch_all(pool.as_ref())
	.await
	.expect("Failed to fetch session keys");

	for session_key in sessions_to_delete {
		backend
			.delete(&session_key)
			.await
			.expect("Failed to delete session");
	}

	// Verify all sessions deleted
	let count_after: i64 = sqlx::query_scalar(
		r#"
		SELECT COUNT(*)
		FROM sessions
		WHERE session_data::text LIKE $1
		"#,
	)
	.bind(format!("%{}%", user_id_str))
	.fetch_one(pool.as_ref())
	.await
	.expect("Failed to count sessions after deletion");
	assert_eq!(
		count_after, 0,
		"All sessions should be deleted after password change"
	);

	// Verify each session no longer exists
	let desktop_exists = backend
		.exists(session_key_desktop)
		.await
		.expect("Failed to check desktop session");
	let mobile_exists = backend
		.exists(session_key_mobile)
		.await
		.expect("Failed to check mobile session");
	let tablet_exists = backend
		.exists(session_key_tablet)
		.await
		.expect("Failed to check tablet session");

	assert!(!desktop_exists, "Desktop session should be deleted");
	assert!(!mobile_exists, "Mobile session should be deleted");
	assert!(!tablet_exists, "Tablet session should be deleted");
}

// ============================================================================
// Complete Session Data Removal Tests
// ============================================================================

/// Test logout completely removes all session data
///
/// **Test Intent**: Verify logout deletes all session data including
/// CSRF tokens, IP address, User-Agent, and custom metadata
///
/// **Integration Point**: Logout → Complete data cleanup → Database verification
///
/// **Not Intent**: Partial cleanup, soft deletion
#[rstest]
#[serial(sessions)]
#[tokio::test]
async fn test_logout_complete_data_removal(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, database_url) = postgres_container.await;

	// Initialize global ORM connection for Session::objects() calls
	reinitialize_database(&database_url)
		.await
		.expect("Failed to initialize ORM database");

	// Clear table before test to ensure isolation
	let conn = get_connection()
		.await
		.expect("Failed to get ORM connection");
	let _ = conn.execute("DROP TABLE IF EXISTS sessions", vec![]).await;

	// Create database session backend
	let backend = DatabaseSessionBackend::new(&database_url)
		.await
		.expect("Failed to create session backend");

	backend
		.create_table()
		.await
		.expect("Failed to create sessions table");

	// Create comprehensive session with all types of data
	let user_id = Uuid::new_v4();
	let csrf_token = Uuid::new_v4().to_string();
	let client_ip = "192.168.1.100";
	let user_agent = "Mozilla/5.0 (Windows NT 10.0; Win64; x64)";

	let mut session = Session::new(backend.clone());
	// Authentication data
	session.set("_auth_user_id", user_id.to_string()).unwrap();
	session
		.set("_auth_user_name", "charlie".to_string())
		.unwrap();
	session
		.set("_auth_user_email", "charlie@example.com".to_string())
		.unwrap();
	session.set("_auth_user_is_admin", true).unwrap();
	// Security data
	session.set("_csrf_token", &csrf_token).unwrap();
	session.set("_client_ip", client_ip.to_string()).unwrap();
	session.set("_user_agent", user_agent.to_string()).unwrap();
	// Session metadata
	session
		.set("_login_timestamp", chrono::Utc::now().timestamp())
		.unwrap();
	session
		.set("_last_activity", chrono::Utc::now().timestamp())
		.unwrap();
	// User preferences
	session.set("_user_language", "en-US".to_string()).unwrap();
	session.set("_user_timezone", "UTC".to_string()).unwrap();
	session.set("_user_theme", "dark".to_string()).unwrap();
	// Application data
	session.set("_cart_items", vec![1, 2, 3, 4, 5]).unwrap();
	session
		.set(
			"_user_preferences",
			serde_json::json!({"notifications": true, "emails": "daily"}),
		)
		.unwrap();

	session.save().await.expect("Failed to save session");
	let session_key = session.session_key().unwrap();

	// Verify all data exists before logout
	let result_before = sqlx::query("SELECT session_data FROM sessions WHERE session_key = $1")
		.bind(session_key)
		.fetch_one(pool.as_ref())
		.await
		.expect("Failed to query session before logout");
	let data_before_str: String = result_before.get("session_data");
	let data_before: serde_json::Value =
		serde_json::from_str(&data_before_str).expect("Failed to parse session data");

	// Verify comprehensive data exists
	assert_eq!(
		data_before["_auth_user_id"].as_str().unwrap(),
		user_id.to_string()
	);
	assert_eq!(data_before["_auth_user_name"], "charlie");
	assert_eq!(data_before["_auth_user_email"], "charlie@example.com");
	assert!(data_before["_auth_user_is_admin"].as_bool().unwrap());
	assert_eq!(data_before["_csrf_token"].as_str().unwrap(), csrf_token);
	assert_eq!(data_before["_client_ip"], client_ip);
	assert_eq!(data_before["_user_agent"], user_agent);
	assert!(data_before["_login_timestamp"].as_i64().is_some());
	assert!(data_before["_last_activity"].as_i64().is_some());
	assert_eq!(data_before["_user_language"], "en-US");
	assert_eq!(data_before["_user_timezone"], "UTC");
	assert_eq!(data_before["_user_theme"], "dark");
	assert_eq!(data_before["_cart_items"].as_array().unwrap().len(), 5);
	assert!(data_before["_user_preferences"].is_object());

	// Simulate logout - delete session
	backend
		.delete(session_key)
		.await
		.expect("Failed to delete session");

	// Verify complete data removal from database
	let result_after = sqlx::query("SELECT session_data FROM sessions WHERE session_key = $1")
		.bind(session_key)
		.fetch_optional(pool.as_ref())
		.await
		.expect("Failed to query session after logout");

	assert!(
		result_after.is_none(),
		"Session should be completely removed from database"
	);

	// Verify session doesn't exist in backend
	let exists = backend
		.exists(session_key)
		.await
		.expect("Failed to check session existence");
	assert!(!exists, "Session should not exist in backend after logout");

	// Verify all session keys count is 0 for this session
	let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sessions WHERE session_key = $1")
		.bind(session_key)
		.fetch_one(pool.as_ref())
		.await
		.expect("Failed to count sessions");
	assert_eq!(count, 0, "No session data should remain");
}
