use reinhardt_auth::sessions::backends::cache::SessionBackend;
use reinhardt_auth::sessions::backends::database::DatabaseSessionBackend;
use reinhardt_db::orm::manager::{get_connection, reinitialize_database};
use rstest::*;
use serde_json::json;
use serial_test::serial;
use std::sync::atomic::{AtomicU32, Ordering};

/// Counter for unique database file names
static TEST_COUNTER: AtomicU32 = AtomicU32::new(0);

/// Fixture providing a test database session backend
#[fixture]
async fn backend() -> DatabaseSessionBackend {
	// Use a unique file-based SQLite database for each test
	// This works with nextest which runs each test in a separate process
	let test_id = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
	let pid = std::process::id();
	let db_file = format!("/tmp/reinhardt_session_test_{}_{}.db", pid, test_id);
	let database_url = format!("sqlite:{}", db_file);

	// Clean up any existing database file
	let _ = std::fs::remove_file(&db_file);

	// Initialize global ORM connection for Session::objects() calls
	reinitialize_database(&database_url)
		.await
		.expect("Failed to initialize ORM database");

	// Clear table before each test to ensure isolation
	// DROP TABLE ensures clean state even if previous test failed
	let conn = get_connection()
		.await
		.expect("Failed to get ORM connection");
	let _ = conn.execute("DROP TABLE IF EXISTS sessions", vec![]).await;

	let backend = DatabaseSessionBackend::new(&database_url)
		.await
		.expect("Failed to create test backend");
	backend
		.create_table()
		.await
		.expect("Failed to create table");
	backend
}

#[rstest]
#[tokio::test]
#[serial(sessions_db)]
async fn test_save_and_load_session(#[future] backend: DatabaseSessionBackend) {
	let backend = backend.await;
	let session_key = "test_session_1";
	let data = json!({
		"user_id": 123,
		"username": "testuser",
	});

	// Save session with 3600 second TTL
	backend
		.save(session_key, &data, Some(3600))
		.await
		.expect("Failed to save session");

	// Load session
	let loaded: Option<serde_json::Value> = backend
		.load(session_key)
		.await
		.expect("Failed to load session");

	assert_eq!(loaded, Some(data));
}

#[rstest]
#[tokio::test]
#[serial(sessions_db)]
async fn test_session_exists(#[future] backend: DatabaseSessionBackend) {
	let backend = backend.await;
	let session_key = "test_session_2";
	let data = json!({"test": "data"});

	// Session should not exist initially
	let exists = backend
		.exists(session_key)
		.await
		.expect("Failed to check existence");
	assert!(!exists);

	// Save session
	backend
		.save(session_key, &data, Some(3600))
		.await
		.expect("Failed to save session");

	// Session should now exist
	let exists = backend
		.exists(session_key)
		.await
		.expect("Failed to check existence");
	assert!(exists);
}

#[rstest]
#[tokio::test]
#[serial(sessions_db)]
async fn test_delete_session(#[future] backend: DatabaseSessionBackend) {
	let backend = backend.await;
	let session_key = "test_session_3";
	let data = json!({"test": "data"});

	// Save session
	backend
		.save(session_key, &data, Some(3600))
		.await
		.expect("Failed to save session");

	// Verify session exists
	assert!(
		backend
			.exists(session_key)
			.await
			.expect("Failed to check existence")
	);

	// Delete session
	backend
		.delete(session_key)
		.await
		.expect("Failed to delete session");

	// Verify session no longer exists
	assert!(
		!backend
			.exists(session_key)
			.await
			.expect("Failed to check existence")
	);
}

#[rstest]
#[tokio::test]
#[serial(sessions_db)]
async fn test_expired_session(#[future] backend: DatabaseSessionBackend) {
	let backend = backend.await;
	let session_key = "test_session_4";
	let data = json!({"test": "data"});

	// Save session with 0 second TTL (immediately expired)
	backend
		.save(session_key, &data, Some(0))
		.await
		.expect("Failed to save session");

	// Wait a moment to ensure expiration
	tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

	// Try to load expired session
	let loaded: Option<serde_json::Value> = backend
		.load(session_key)
		.await
		.expect("Failed to load session");

	assert_eq!(loaded, None);
}

#[rstest]
#[tokio::test]
#[serial(sessions_db)]
async fn test_cleanup_expired(#[future] backend: DatabaseSessionBackend) {
	let backend = backend.await;

	// Save some expired sessions
	for i in 0..5 {
		let key = format!("expired_{}", i);
		backend
			.save(&key, &json!({ "test": i }), Some(0))
			.await
			.expect("Failed to save session");
	}

	// Save some active sessions
	for i in 0..3 {
		let key = format!("active_{}", i);
		backend
			.save(&key, &json!({ "test": i }), Some(3600))
			.await
			.expect("Failed to save session");
	}

	// Wait for expiration
	tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

	// Clean up expired sessions
	// Note: rows_affected() may not return accurate count on all SQLite configurations
	// (especially with shared cache mode), so we don't assert on the returned value
	let _deleted = backend.cleanup_expired().await.expect("Failed to cleanup");

	// Verify expired sessions no longer exist
	for i in 0..5 {
		let key = format!("expired_{}", i);
		assert!(
			!backend
				.exists(&key)
				.await
				.expect("Failed to check existence"),
			"Expired session {} should be deleted",
			key
		);
	}

	// Verify active sessions still exist
	for i in 0..3 {
		let key = format!("active_{}", i);
		assert!(
			backend
				.exists(&key)
				.await
				.expect("Failed to check existence")
		);
	}
}
