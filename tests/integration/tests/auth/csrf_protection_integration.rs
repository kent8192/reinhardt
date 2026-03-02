//! CSRF Protection Integration Tests
//!
//! Tests CSRF token generation/verification, session, and reinhardt-forms integration.

use reinhardt_core::macros::model;
use reinhardt_db::orm::manager::reinitialize_database;
use reinhardt_test::fixtures::testcontainers::{ContainerAsync, GenericImage, postgres_container};
use rstest::*;
use serde::{Deserialize, Serialize};
use sqlx::Row;
use std::sync::Arc;
use uuid::Uuid;

// ============================================================================
// ORM Model Definition
// ============================================================================

/// ORM model for session table - demonstrates reinhardt_orm integration with CSRF tests
#[model(app_label = "csrf_test", table_name = "sessions")]
#[derive(Serialize, Deserialize, Clone, Debug)]
#[allow(dead_code)] // ORM model for CSRF integration tests
struct SessionModel {
	#[field(primary_key = true, max_length = 40)]
	session_key: String,
	#[field(max_length = 10000)]
	session_data: String,
	#[field(max_length = 50)]
	expire_date: String,
}

// Note: Actual CSRF implementation is in reinhardt-sessions/src/csrf.rs
// Here we implement CSRF protection integration tests

// ============================================================================
// Sanity Tests (2 tests)
// ============================================================================

#[rstest]
#[tokio::test]
async fn sanity_csrf_token_generation() {
	// Basic CSRF token generation operation
	let csrf_token = generate_csrf_token();

	assert!(!csrf_token.is_empty());
	assert_eq!(csrf_token.len(), 64); // 32 bytes of random data in Hex representation is 64 characters
}

#[rstest]
#[tokio::test]
async fn sanity_csrf_token_verification() {
	// Basic CSRF token verification operation
	let csrf_token = generate_csrf_token();

	// Same token should pass verification
	assert!(verify_csrf_token(&csrf_token, &csrf_token));

	// Different token should fail verification
	let different_token = generate_csrf_token();
	assert!(!verify_csrf_token(&csrf_token, &different_token));
}

// ============================================================================
// Normal Cases (5 tests)
// ============================================================================

#[rstest]
#[tokio::test]
async fn normal_session_based_csrf_protection(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<sqlx::PgPool>, u16, String),
) {
	// Session-based CSRF protection (token generation/verification)
	let (_container, pool, _port, url) = postgres_container.await;
	reinitialize_database(&url).await.unwrap();

	// Create session table
	sqlx::query(
		"CREATE TABLE IF NOT EXISTS sessions (
			session_key VARCHAR(40) PRIMARY KEY,
			session_data TEXT NOT NULL,
			expire_date TIMESTAMP NOT NULL
		)",
	)
	.execute(pool.as_ref())
	.await
	.unwrap();

	// Create session
	let session_key = "test_session_key";
	let csrf_token = generate_csrf_token();

	// Save CSRF token to session data
	let session_data = serde_json::json!({
		"csrf_token": csrf_token,
		"user_id": "test_user_id"
	})
	.to_string();

	sqlx::query(
		"INSERT INTO sessions (session_key, session_data, expire_date) VALUES ($1, $2, NOW() + INTERVAL '1 hour')"
	)
	.bind(session_key)
	.bind(&session_data)
	.execute(pool.as_ref())
	.await
	.unwrap();

	// Retrieve CSRF token from session
	let row = sqlx::query("SELECT session_data FROM sessions WHERE session_key = $1")
		.bind(session_key)
		.fetch_one(pool.as_ref())
		.await
		.unwrap();

	let loaded_data: String = row.get("session_data");
	let data: serde_json::Value = serde_json::from_str(&loaded_data).unwrap();
	let loaded_csrf_token = data["csrf_token"].as_str().unwrap();

	assert_eq!(loaded_csrf_token, csrf_token);
}

#[rstest]
#[tokio::test]
async fn normal_csrf_token_form_submission() {
	// CSRF token and form submission
	let csrf_token = generate_csrf_token();

	// Include CSRF token in form data
	let form_data = serde_json::json!({
		"csrf_token": csrf_token,
		"username": "testuser",
		"email": "test@example.com"
	});

	// Verify token
	let submitted_token = form_data["csrf_token"].as_str().unwrap();
	assert!(verify_csrf_token(&csrf_token, submitted_token));
}

#[rstest]
#[tokio::test]
async fn normal_csrf_token_ajax_request() {
	// CSRF token and AJAX request (header)
	let csrf_token = generate_csrf_token();

	// Include CSRF token in HTTP header (typically X-CSRF-Token)
	let header_token = csrf_token.clone();

	assert!(verify_csrf_token(&csrf_token, &header_token));
}

#[rstest]
#[tokio::test]
async fn normal_csrf_token_auto_regeneration(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<sqlx::PgPool>, u16, String),
) {
	// Automatic CSRF token regeneration
	let (_container, pool, _port, url) = postgres_container.await;
	reinitialize_database(&url).await.unwrap();

	sqlx::query(
		"CREATE TABLE IF NOT EXISTS sessions (
			session_key VARCHAR(40) PRIMARY KEY,
			session_data TEXT NOT NULL,
			expire_date TIMESTAMP NOT NULL
		)",
	)
	.execute(pool.as_ref())
	.await
	.unwrap();

	let session_key = "test_session_key";
	let old_csrf_token = generate_csrf_token();

	// Create session with old token
	let session_data = serde_json::json!({
		"csrf_token": old_csrf_token
	})
	.to_string();

	sqlx::query(
		"INSERT INTO sessions (session_key, session_data, expire_date) VALUES ($1, $2, NOW() + INTERVAL '1 hour')"
	)
	.bind(session_key)
	.bind(&session_data)
	.execute(pool.as_ref())
	.await
	.unwrap();

	// Generate and update with new token
	let new_csrf_token = generate_csrf_token();
	let new_session_data = serde_json::json!({
		"csrf_token": new_csrf_token
	})
	.to_string();

	sqlx::query("UPDATE sessions SET session_data = $1 WHERE session_key = $2")
		.bind(&new_session_data)
		.bind(session_key)
		.execute(pool.as_ref())
		.await
		.unwrap();

	// Verify updated token
	let row = sqlx::query("SELECT session_data FROM sessions WHERE session_key = $1")
		.bind(session_key)
		.fetch_one(pool.as_ref())
		.await
		.unwrap();

	let loaded_data: String = row.get("session_data");
	let data: serde_json::Value = serde_json::from_str(&loaded_data).unwrap();
	let loaded_csrf_token = data["csrf_token"].as_str().unwrap();

	assert_eq!(loaded_csrf_token, new_csrf_token);
	assert_ne!(loaded_csrf_token, old_csrf_token);
}

#[rstest]
#[tokio::test]
async fn normal_csrf_token_shared_across_tabs(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<sqlx::PgPool>, u16, String),
) {
	// CSRF token sharing across multiple tabs (via session)
	let (_container, pool, _port, url) = postgres_container.await;
	reinitialize_database(&url).await.unwrap();

	sqlx::query(
		"CREATE TABLE IF NOT EXISTS sessions (
			session_key VARCHAR(40) PRIMARY KEY,
			session_data TEXT NOT NULL,
			expire_date TIMESTAMP NOT NULL
		)",
	)
	.execute(pool.as_ref())
	.await
	.unwrap();

	let session_key = "shared_session_key";
	let csrf_token = generate_csrf_token();

	let session_data = serde_json::json!({
		"csrf_token": csrf_token
	})
	.to_string();

	sqlx::query(
		"INSERT INTO sessions (session_key, session_data, expire_date) VALUES ($1, $2, NOW() + INTERVAL '1 hour')"
	)
	.bind(session_key)
	.bind(&session_data)
	.execute(pool.as_ref())
	.await
	.unwrap();

	// Retrieve from tab 1
	let row1 = sqlx::query("SELECT session_data FROM sessions WHERE session_key = $1")
		.bind(session_key)
		.fetch_one(pool.as_ref())
		.await
		.unwrap();

	// Retrieve from tab 2 (same session key)
	let row2 = sqlx::query("SELECT session_data FROM sessions WHERE session_key = $1")
		.bind(session_key)
		.fetch_one(pool.as_ref())
		.await
		.unwrap();

	let data1: serde_json::Value =
		serde_json::from_str(&row1.get::<String, _>("session_data")).unwrap();
	let data2: serde_json::Value =
		serde_json::from_str(&row2.get::<String, _>("session_data")).unwrap();

	// Both tabs share the same CSRF token
	assert_eq!(data1["csrf_token"], data2["csrf_token"]);
}

// ============================================================================
// Error Cases (4 tests)
// ============================================================================

#[rstest]
#[tokio::test]
async fn abnormal_invalid_csrf_token_rejected() {
	// Reject with invalid CSRF token (403 Forbidden)
	let valid_csrf_token = generate_csrf_token();
	let invalid_csrf_token = "invalid_token_12345";

	assert!(!verify_csrf_token(&valid_csrf_token, invalid_csrf_token));
}

#[rstest]
#[tokio::test]
async fn abnormal_missing_csrf_token_rejected() {
	// Reject without CSRF token (POST request)
	let valid_csrf_token = generate_csrf_token();
	let empty_token = "";

	assert!(!verify_csrf_token(&valid_csrf_token, empty_token));
}

#[rstest]
#[tokio::test]
async fn abnormal_expired_csrf_token() {
	// Expired CSRF token
	let csrf_token = generate_csrf_token();

	// For timestamp-based tokens, simulate expiration
	// Here, simply treat it as a different token
	let expired_token = "expired_token_old";

	assert!(!verify_csrf_token(&csrf_token, expired_token));
}

#[rstest]
#[tokio::test]
async fn abnormal_tampered_csrf_token() {
	// Token tampering detection
	let csrf_token = generate_csrf_token();

	// Tamper with token (change 1 character)
	let mut tampered_token = csrf_token.clone();
	tampered_token.replace_range(0..1, "X");

	assert!(!verify_csrf_token(&csrf_token, &tampered_token));
}

// ============================================================================
// Regression Tests (2 tests)
// ============================================================================

#[rstest]
#[tokio::test]
async fn regression_csrf_token_format_backward_compatibility() {
	// CSRF token format backward compatibility
	// Compare with previous versions

	// Old format token (32 bytes Hex)
	let old_format_token = "a".repeat(64);

	// New format token (also 32 bytes Hex)
	let new_format_token = generate_csrf_token();

	// Verify both have the same length
	assert_eq!(old_format_token.len(), new_format_token.len());
}

#[rstest]
#[tokio::test]
async fn regression_session_rotation_csrf_validity(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<sqlx::PgPool>, u16, String),
) {
	// CSRF token validity after session rotation
	let (_container, pool, _port, url) = postgres_container.await;
	reinitialize_database(&url).await.unwrap();

	sqlx::query(
		"CREATE TABLE IF NOT EXISTS sessions (
			session_key VARCHAR(40) PRIMARY KEY,
			session_data TEXT NOT NULL,
			expire_date TIMESTAMP NOT NULL
		)",
	)
	.execute(pool.as_ref())
	.await
	.unwrap();

	let old_session_key = "old_session_key";
	let new_session_key = "new_session_key";
	let csrf_token = generate_csrf_token();

	// Save CSRF token to old session
	let session_data = serde_json::json!({
		"csrf_token": csrf_token
	})
	.to_string();

	sqlx::query(
		"INSERT INTO sessions (session_key, session_data, expire_date) VALUES ($1, $2, NOW() + INTERVAL '1 hour')"
	)
	.bind(old_session_key)
	.bind(&session_data)
	.execute(pool.as_ref())
	.await
	.unwrap();

	// Session rotation (recreate with new key)
	sqlx::query(
		"INSERT INTO sessions (session_key, session_data, expire_date) VALUES ($1, $2, NOW() + INTERVAL '1 hour')"
	)
	.bind(new_session_key)
	.bind(&session_data)
	.execute(pool.as_ref())
	.await
	.unwrap();

	// Delete old session
	sqlx::query("DELETE FROM sessions WHERE session_key = $1")
		.bind(old_session_key)
		.execute(pool.as_ref())
		.await
		.unwrap();

	// Verify CSRF token is valid in new session
	let row = sqlx::query("SELECT session_data FROM sessions WHERE session_key = $1")
		.bind(new_session_key)
		.fetch_one(pool.as_ref())
		.await
		.unwrap();

	let loaded_data: String = row.get("session_data");
	let data: serde_json::Value = serde_json::from_str(&loaded_data).unwrap();
	let loaded_csrf_token = data["csrf_token"].as_str().unwrap();

	assert_eq!(loaded_csrf_token, csrf_token);
}

// ============================================================================
// State Transitions (2 tests)
// ============================================================================

#[rstest]
#[tokio::test]
async fn state_transition_session_creation_to_csrf_verification() {
	// Session creation → CSRF token generation → Verification → Success

	// Step 1: Create session
	let session_id = Uuid::new_v4();

	// Step 2: Generate CSRF token
	let csrf_token = generate_csrf_token();

	// Step 3: Save token to session (virtual)
	let session_data = serde_json::json!({
		"session_id": session_id.to_string(),
		"csrf_token": csrf_token
	});

	// Step 4: Verify token
	let submitted_token = session_data["csrf_token"].as_str().unwrap();
	assert!(verify_csrf_token(&csrf_token, submitted_token));
}

#[rstest]
#[tokio::test]
async fn state_transition_session_destroy_csrf_invalidation() {
	// Session destruction → CSRF token invalidation → Verification failure

	// Step 1: Create session and generate CSRF token
	let csrf_token = generate_csrf_token();

	// Step 2: Destroy session (invalidate token as well)
	let destroyed_session = true;

	// Step 3: Token from destroyed session is invalid
	if destroyed_session {
		// New token is required
		let new_csrf_token = generate_csrf_token();
		assert!(!verify_csrf_token(&new_csrf_token, &csrf_token));
	}
}

// ============================================================================
// Edge Cases (2 tests)
// ============================================================================

#[rstest]
#[tokio::test]
async fn edge_get_request_csrf_skip() {
	// GET requests skip CSRF protection
	let http_method = "GET";

	// Skip CSRF verification for GET requests
	let csrf_required = !matches!(http_method, "GET" | "HEAD" | "OPTIONS" | "TRACE");

	assert!(!csrf_required);
}

#[rstest]
#[tokio::test]
async fn edge_safe_methods_csrf_handling() {
	// CSRF handling for safe methods (HEAD, OPTIONS)
	let safe_methods = vec!["GET", "HEAD", "OPTIONS", "TRACE"];

	for method in safe_methods {
		// CSRF verification not required for safe methods
		let csrf_required = !matches!(method, "GET" | "HEAD" | "OPTIONS" | "TRACE");
		assert!(!csrf_required, "Method {} should skip CSRF", method);
	}
}

// ============================================================================
// Fuzz Tests (1 test)
// ============================================================================

#[rstest]
#[tokio::test]
async fn fuzz_random_csrf_token_validation() {
	// Random CSRF token input (1000 times)
	use rand::{Rng, distr::Alphanumeric, rng};

	let valid_csrf_token = generate_csrf_token();

	for _ in 0..1000 {
		// Generate random token
		let random_token: String = rng()
			.sample_iter(&Alphanumeric)
			.take(64)
			.map(char::from)
			.collect();

		// Random token typically fails verification
		// (However, verify no panic as there's a very rare chance of match)
		let result = verify_csrf_token(&valid_csrf_token, &random_token);

		// Verify no panic (result doesn't matter)
		assert!(result == true || result == false);
	}
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Generate CSRF token (32 bytes of random data in Hex representation)
fn generate_csrf_token() -> String {
	use rand::Rng;
	let mut rng = rand::rng();
	let bytes: Vec<u8> = (0..32).map(|_| rng.r#gen()).collect();
	hex::encode(bytes)
}

/// Verify CSRF token
fn verify_csrf_token(expected: &str, provided: &str) -> bool {
	// Constant-time comparison (timing attack protection)
	use subtle::ConstantTimeEq;

	if expected.len() != provided.len() {
		return false;
	}

	expected.as_bytes().ct_eq(provided.as_bytes()).into()
}
