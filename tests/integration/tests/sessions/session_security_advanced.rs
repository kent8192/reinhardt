//! Advanced Session Security Integration Tests
//!
//! Tests advanced security features for session management using reinhardt_sessions APIs:
//! - CSRF token rotation on authentication events
//! - Session ID entropy validation (predictability resistance)
//! - Session expiration enforcement
//! - Session model validation
//!
//! ## Test Coverage
//!
//! This test file covers:
//! - **CSRF Token Rotation**: CSRF tokens are regenerated on login using CsrfSessionManager
//! - **Entropy Validation**: Session IDs have sufficient randomness using Session::generate_key
//! - **Expiration Enforcement**: Expired sessions are rejected using SessionModel::is_expired
//! - **Session Validation**: Session validity using SessionModel::is_valid
//!
//! ## Reinhardt Components Used
//!
//! - `reinhardt_auth::sessions::Session` - High-level session API
//! - `reinhardt_auth::sessions::SessionModel` - Session data model with expiration
//! - `reinhardt_auth::sessions::CsrfSessionManager` - CSRF token management
//! - `reinhardt_auth::sessions::backends::InMemorySessionBackend` - In-memory session storage
//!
//! ## Security Standards Verified
//!
//! - CSRF tokens rotate on authentication events (prevents CSRF after login)
//! - Session IDs have high entropy (Shannon entropy > 4.0 bits/char)
//! - Expired sessions are rejected (enforces TTL)
//! - Session validation correctly identifies valid/invalid sessions

use reinhardt_auth::sessions::Session;
use reinhardt_auth::sessions::backends::InMemorySessionBackend;
use reinhardt_auth::sessions::csrf::CsrfSessionManager;
use reinhardt_auth::sessions::models::SessionModel;
use rstest::*;
use serde_json::json;
use serial_test::serial;
use std::collections::{HashMap, HashSet};
use std::time::Duration;
use tokio::time::sleep;

// ============ Helper Functions ============

/// Calculate Shannon entropy of a string
///
/// Shannon entropy measures the unpredictability of information content.
/// For session IDs, higher entropy means better resistance to prediction attacks.
///
/// Formula: H = -Σ(p(x) * log2(p(x))) where p(x) is probability of character x
///
/// Expected values:
/// - Random UUID (hex): ~4.0 bits/char
/// - Base64 random: ~6.0 bits/char
/// - Sequential numbers: ~0.5 bits/char (predictable)
fn calculate_shannon_entropy(s: &str) -> f64 {
	if s.is_empty() {
		return 0.0;
	}

	// Count character frequencies
	let mut freq_map: HashMap<char, usize> = HashMap::new();
	for c in s.chars() {
		*freq_map.entry(c).or_insert(0) += 1;
	}

	// Calculate entropy
	let len = s.len() as f64;
	let mut entropy = 0.0;
	for count in freq_map.values() {
		let probability = *count as f64 / len;
		entropy -= probability * probability.log2();
	}

	entropy
}

// ============ CSRF Token Rotation Tests ============

/// Test CSRF token rotation using CsrfSessionManager
///
/// Verifies:
/// - CSRF token is generated before login using generate_token()
/// - CSRF token is regenerated after login using rotate_token()
/// - Old CSRF token becomes invalid
/// - New CSRF token is valid for subsequent requests
///
/// **Security Rationale:**
/// CSRF token rotation prevents CSRF attacks that could occur if an attacker
/// obtained a pre-login CSRF token and attempts to use it after the user logs in.
#[rstest]
#[serial(sessions)]
#[tokio::test]
async fn test_csrf_token_rotation_on_login() {
	// Create session with in-memory backend
	let backend = InMemorySessionBackend::new();
	let mut session = Session::new(backend);

	// Create CSRF manager
	let csrf_manager = CsrfSessionManager::new();

	// Generate pre-login CSRF token
	let pre_login_csrf = csrf_manager
		.generate_token(&mut session)
		.expect("Failed to generate CSRF token");

	// Verify pre-login token is valid
	assert!(
		csrf_manager
			.validate_token(&mut session, &pre_login_csrf)
			.expect("Failed to validate token"),
		"Pre-login CSRF token should be valid"
	);

	// Simulate login by rotating the CSRF token
	let post_login_csrf = csrf_manager
		.rotate_token(&mut session)
		.expect("Failed to rotate CSRF token");

	// Verify token was rotated
	assert_ne!(
		pre_login_csrf, post_login_csrf,
		"CSRF token should change after rotation (login)"
	);

	// Verify old token is now invalid
	assert!(
		!csrf_manager
			.validate_token(&mut session, &pre_login_csrf)
			.expect("Failed to validate token"),
		"Pre-login CSRF token should be invalid after rotation"
	);

	// Verify new token is valid
	assert!(
		csrf_manager
			.validate_token(&mut session, &post_login_csrf)
			.expect("Failed to validate token"),
		"Post-login CSRF token should be valid"
	);
}

/// Test CSRF token generation and validation
///
/// Verifies:
/// - get_or_create_token() creates token if missing
/// - Multiple calls return the same token
/// - Token validation works correctly
#[rstest]
#[serial(sessions)]
#[tokio::test]
async fn test_csrf_token_get_or_create() {
	let backend = InMemorySessionBackend::new();
	let mut session = Session::new(backend);

	let csrf_manager = CsrfSessionManager::new();

	// First call should create token
	let token1 = csrf_manager
		.get_or_create_token(&mut session)
		.expect("Failed to get or create token");

	// Second call should return same token
	let token2 = csrf_manager
		.get_or_create_token(&mut session)
		.expect("Failed to get or create token");

	assert_eq!(
		token1, token2,
		"get_or_create_token should return same token"
	);

	// Token should be valid
	assert!(
		csrf_manager
			.validate_token(&mut session, &token1)
			.expect("Failed to validate token"),
		"Created token should be valid"
	);
}

// ============ Session ID Entropy Tests ============

/// Test session ID entropy validation using Session::generate_key
///
/// Verifies:
/// - Session IDs have sufficient randomness (Shannon entropy)
/// - Session IDs are unique across multiple generations
/// - Entropy meets security standards (> 4.0 bits/char for UUID)
///
/// **Security Rationale:**
/// High entropy session IDs prevent brute-force and prediction attacks.
/// UUIDs should have ~4.0 bits/char entropy. Sequential or predictable
/// IDs would have much lower entropy (<1.0 bits/char).
#[rstest]
#[serial(sessions)]
#[tokio::test]
async fn test_session_id_entropy() {
	// Generate multiple session IDs using Session::generate_key
	let sample_size = 100;
	let mut session_ids = Vec::new();
	let mut entropies = Vec::new();

	for _ in 0..sample_size {
		let session_id = Session::<InMemorySessionBackend>::generate_key();

		// Calculate entropy
		let entropy = calculate_shannon_entropy(&session_id);
		entropies.push(entropy);

		session_ids.push(session_id);
	}

	// Verify uniqueness
	let unique_ids: HashSet<_> = session_ids.iter().collect();
	assert_eq!(
		unique_ids.len(),
		sample_size,
		"All session IDs should be unique"
	);

	// Verify entropy meets security standards
	let avg_entropy: f64 = entropies.iter().sum::<f64>() / entropies.len() as f64;

	// UUID v4 should have ~4.0 bits/char entropy (32 hex chars + 4 hyphens)
	// We expect average entropy > 3.5 to account for hyphen characters
	assert!(
		avg_entropy > 3.5,
		"Average entropy ({:.2}) should be > 3.5 bits/char for secure session IDs",
		avg_entropy
	);

	// Verify individual entropies
	for (i, entropy) in entropies.iter().enumerate() {
		assert!(
			*entropy > 3.0,
			"Session ID {} has low entropy ({:.2}), indicating predictability",
			i,
			entropy
		);
	}
}

/// Test session key generation produces valid format
#[rstest]
#[serial(sessions)]
#[tokio::test]
async fn test_session_key_format() {
	let key = Session::<InMemorySessionBackend>::generate_key();

	// Key should be a valid UUID format (36 characters with hyphens)
	assert_eq!(
		key.len(),
		36,
		"Session key should be 36 characters (UUID format)"
	);
	assert!(
		key.chars().all(|c| c.is_ascii_hexdigit() || c == '-'),
		"Session key should only contain hex digits and hyphens"
	);
}

// ============ Session Model Expiration Tests ============

/// Test SessionModel expiration detection
///
/// Verifies:
/// - SessionModel::is_expired() correctly identifies expired sessions
/// - SessionModel::is_valid() correctly identifies valid sessions
/// - Expiration is based on TTL
///
/// **Security Rationale:**
/// Enforcing session expiration limits the window of opportunity for
/// session hijacking attacks and ensures users must re-authenticate
/// periodically for sensitive operations.
#[rstest]
#[serial(sessions)]
#[tokio::test]
async fn test_session_model_expiration() {
	// Create session with short TTL (1 second)
	let session = SessionModel::new("test_session_key".to_string(), json!({"user_id": 42}), 1);

	// Session should be valid initially
	assert!(session.is_valid(), "Session should be valid initially");
	assert!(
		!session.is_expired(),
		"Session should not be expired initially"
	);

	// Wait for session to expire
	sleep(Duration::from_secs(2)).await;

	// Session should now be expired
	assert!(session.is_expired(), "Session should be expired after TTL");
	assert!(
		!session.is_valid(),
		"Session should not be valid after expiration"
	);
}

/// Test SessionModel with custom expiration date
#[rstest]
#[serial(sessions)]
#[tokio::test]
async fn test_session_model_with_custom_expiration() {
	use chrono::{Duration as ChronoDuration, Utc};

	// Create session with expiration in the past
	let past_expire = Utc::now() - ChronoDuration::hours(1);
	let expired_session = SessionModel::with_expire_date(
		"expired_key".to_string(),
		json!({"data": "test"}),
		past_expire,
	);

	assert!(
		expired_session.is_expired(),
		"Session with past expiration should be expired"
	);
	assert!(
		!expired_session.is_valid(),
		"Session with past expiration should not be valid"
	);

	// Create session with expiration in the future
	let future_expire = Utc::now() + ChronoDuration::hours(1);
	let valid_session = SessionModel::with_expire_date(
		"valid_key".to_string(),
		json!({"data": "test"}),
		future_expire,
	);

	assert!(
		!valid_session.is_expired(),
		"Session with future expiration should not be expired"
	);
	assert!(
		valid_session.is_valid(),
		"Session with future expiration should be valid"
	);
}

/// Test SessionModel extend functionality
#[rstest]
#[serial(sessions)]
#[tokio::test]
async fn test_session_model_extend() {
	// Create session with short TTL
	let mut session = SessionModel::new("extend_test".to_string(), json!({"user_id": 1}), 1);

	let original_expire = session.expire_date().clone();

	// Extend session by 1 hour
	session.extend(3600);

	let new_expire = session.expire_date().clone();

	assert!(
		new_expire > original_expire,
		"Extended session should have later expiration"
	);
	assert!(session.is_valid(), "Extended session should be valid");
}

// ============ Session Data Management Tests ============

/// Test Session set/get operations
#[rstest]
#[serial(sessions)]
#[tokio::test]
async fn test_session_set_get_operations() {
	let backend = InMemorySessionBackend::new();
	let mut session = Session::new(backend);

	// Set session data
	session
		.set("user_id", &42i32)
		.expect("Failed to set user_id");
	session
		.set("username", &"alice")
		.expect("Failed to set username");
	session
		.set("roles", &vec!["admin", "user"])
		.expect("Failed to set roles");

	// Get session data
	let user_id: Option<i32> = session.get("user_id").expect("Failed to get user_id");
	let username: Option<String> = session.get("username").expect("Failed to get username");
	let roles: Option<Vec<String>> = session.get("roles").expect("Failed to get roles");

	assert_eq!(user_id, Some(42), "user_id should match");
	assert_eq!(username, Some("alice".to_string()), "username should match");
	assert_eq!(
		roles,
		Some(vec!["admin".to_string(), "user".to_string()]),
		"roles should match"
	);
}

/// Test Session flush operation
#[rstest]
#[serial(sessions)]
#[tokio::test]
async fn test_session_flush() {
	let backend = InMemorySessionBackend::new();
	let mut session = Session::new(backend);

	// Set some data
	session.set("key1", &"value1").expect("Failed to set key1");
	session.set("key2", &"value2").expect("Failed to set key2");

	// Verify data exists
	assert!(
		session.contains_key("key1"),
		"key1 should exist before flush"
	);
	assert!(
		session.contains_key("key2"),
		"key2 should exist before flush"
	);

	// Flush session
	session.flush().await.expect("Failed to flush session");

	// Verify data is cleared
	assert!(
		!session.contains_key("key1"),
		"key1 should not exist after flush"
	);
	assert!(
		!session.contains_key("key2"),
		"key2 should not exist after flush"
	);
}

/// Test Session cycle_key operation
///
/// This is important for security as it generates a new session ID
/// while preserving session data (useful after login to prevent fixation)
#[rstest]
#[serial(sessions)]
#[tokio::test]
async fn test_session_cycle_key() {
	let backend = InMemorySessionBackend::new();
	let mut session = Session::new(backend);

	// Set initial data
	session
		.set("user_id", &123i32)
		.expect("Failed to set user_id");

	// Get original session key
	let original_key = session.session_key().map(|s| s.to_string());

	// Cycle the session key (simulates post-login security measure)
	session.cycle_key().await.expect("Failed to cycle key");

	// Get new session key after cycling
	let new_key = session.session_key().map(|s| s.to_string());

	// Verify key changed
	assert_ne!(
		original_key.as_deref(),
		new_key.as_deref(),
		"Session key should change after cycle"
	);

	// Verify data is preserved
	let user_id: Option<i32> = session.get("user_id").expect("Failed to get user_id");
	assert_eq!(
		user_id,
		Some(123),
		"Data should be preserved after key cycle"
	);
}

// ============ Session Timeout Tests ============

/// Test session timeout functionality
#[rstest]
#[serial(sessions)]
#[tokio::test]
async fn test_session_timeout() {
	let backend = InMemorySessionBackend::new();
	let mut session = Session::new(backend);

	// Set a short timeout (1 second)
	session.set_timeout(1);

	// Update activity
	session.update_activity();

	// Session should not be timed out immediately
	assert!(
		!session.is_timed_out(),
		"Session should not be timed out immediately"
	);

	// Wait for timeout
	sleep(Duration::from_secs(2)).await;

	// Session should now be timed out
	assert!(
		session.is_timed_out(),
		"Session should be timed out after timeout period"
	);
}

/// Test session timeout validation
#[rstest]
#[serial(sessions)]
#[tokio::test]
async fn test_session_timeout_validation() {
	let backend = InMemorySessionBackend::new();
	let mut session = Session::new(backend);

	// Set a short timeout
	session.set_timeout(1);
	session.update_activity();

	// Validate timeout should pass initially
	assert!(
		session.validate_timeout().is_ok(),
		"Timeout validation should pass initially"
	);

	// Wait for timeout
	sleep(Duration::from_secs(2)).await;

	// Validate timeout should fail after timeout
	assert!(
		session.validate_timeout().is_err(),
		"Timeout validation should fail after timeout"
	);
}
