//! MFA Integration Tests
//!
//! Comprehensive integration tests for Multi-Factor Authentication (TOTP).
//! These tests verify the complete MFA workflow including registration,
//! verification, and edge cases.
//!
//! # Test Categories
//!
//! - Happy path: Registration, verification, URL generation
//! - Error path: Invalid codes, unregistered users, malformed secrets
//! - State transition: Registration, deregistration, re-registration
//! - Edge cases: Boundary TOTP codes, special characters in usernames
//! - Decision table: Verification outcomes based on conditions

use reinhardt_auth::AuthenticationError;
use reinhardt_auth::mfa::MFAAuthentication as MfaManager;
use reinhardt_test::fixtures::auth::TestUser;
use rstest::*;
use std::collections::HashSet;

// =============================================================================
// Test Fixtures
// =============================================================================

/// Creates a new MFA manager with default issuer
#[fixture]
fn mfa_manager() -> MfaManager {
	MfaManager::new("ReinhardtTest").time_window(30)
}

/// Creates a test user
#[fixture]
fn test_user() -> TestUser {
	TestUser {
		id: uuid::Uuid::new_v4(),
		username: "testuser".to_string(),
		email: "test@example.com".to_string(),
		is_active: true,
		is_admin: false,
		is_staff: false,
		is_superuser: false,
	}
}

/// Valid base32 secret for testing (RFC 4648 compliant)
const VALID_SECRET: &str = "JBSWY3DPEHPK3PXP";
/// Another valid base32 secret for comparison tests
const VALID_SECRET_2: &str = "GEZDGNBVGY3TQOJQ";

/// Helper function to generate current TOTP code for a secret
fn generate_current_totp(secret: &str, time_window: u64) -> String {
	let secret_bytes = data_encoding::BASE32_NOPAD
		.decode(secret.as_bytes())
		.expect("Valid base32 secret");

	let current_time = std::time::SystemTime::now()
		.duration_since(std::time::UNIX_EPOCH)
		.unwrap()
		.as_secs();
	let time_step = current_time / time_window;

	totp_lite::totp_custom::<totp_lite::Sha256>(time_window, 6, &secret_bytes, time_step)
}

// =============================================================================
// Happy Path Tests
// =============================================================================

#[rstest]
#[tokio::test]
async fn test_mfa_registration_stores_secret(mfa_manager: MfaManager, test_user: TestUser) {
	// Act
	mfa_manager
		.register_user(&test_user.username, VALID_SECRET)
		.await;

	// Assert
	let stored_secret = mfa_manager.get_secret(&test_user.username).await;
	assert!(stored_secret.is_some(), "Secret should be stored");
	assert_eq!(
		stored_secret.unwrap(),
		VALID_SECRET,
		"Stored secret should match registered secret"
	);
}

#[rstest]
#[tokio::test]
async fn test_mfa_registration_multiple_users(mfa_manager: MfaManager) {
	// Arrange
	let users = vec![
		("alice", VALID_SECRET),
		("bob", VALID_SECRET_2),
		("charlie", "MFRGGZDFMY"),
	];

	// Act
	for (username, secret) in &users {
		mfa_manager.register_user(*username, *secret).await;
	}

	// Assert - Each user should have their own secret
	for (username, expected_secret) in &users {
		let stored_secret = mfa_manager
			.get_secret(username)
			.await
			.expect("Secret should exist");
		assert_eq!(
			&stored_secret, *expected_secret,
			"User {} should have correct secret",
			username
		);
	}
}

#[rstest]
#[tokio::test]
async fn test_mfa_verification_with_valid_code(mfa_manager: MfaManager, test_user: TestUser) {
	// Arrange
	mfa_manager
		.register_user(&test_user.username, VALID_SECRET)
		.await;
	let valid_code = generate_current_totp(VALID_SECRET, 30);

	// Act
	let result = mfa_manager
		.verify_totp(&test_user.username, &valid_code)
		.await;

	// Assert
	assert!(result.is_ok(), "Verification should succeed");
	assert!(result.unwrap(), "Valid code should be accepted");
}

#[rstest]
fn test_mfa_totp_url_generation(mfa_manager: MfaManager, test_user: TestUser) {
	// Act
	let url = mfa_manager.generate_totp_url(&test_user.username, VALID_SECRET);

	// Assert - URL format: otpauth://totp/{issuer}:{username}?secret={secret}&issuer={issuer}
	assert!(
		url.starts_with("otpauth://totp/"),
		"URL should start with otpauth://totp/"
	);
	assert!(
		url.contains(&test_user.username),
		"URL should contain username"
	);
	assert!(url.contains(VALID_SECRET), "URL should contain secret");
	assert!(
		url.contains("ReinhardtTest"),
		"URL should contain issuer name"
	);
}

#[rstest]
#[tokio::test]
async fn test_mfa_time_window_configuration() {
	// Act
	let mfa_60sec = MfaManager::new("Test").time_window(60);
	let mfa_30sec = MfaManager::new("Test").time_window(30);

	// Register and get codes
	mfa_60sec.register_user("alice", VALID_SECRET).await;
	mfa_30sec.register_user("alice", VALID_SECRET).await;

	// Generate codes with different time windows
	let code_60 = generate_current_totp(VALID_SECRET, 60);
	let code_30 = generate_current_totp(VALID_SECRET, 30);

	// Assert - Different time windows can produce different codes
	// (depending on current time, they might be same or different)
	// What matters is that each validates with its own time window
	assert!(
		mfa_60sec.verify_totp("alice", &code_60).await.unwrap(),
		"60-second window should validate its own code"
	);
	assert!(
		mfa_30sec.verify_totp("alice", &code_30).await.unwrap(),
		"30-second window should validate its own code"
	);
}

// =============================================================================
// Error Path Tests
// =============================================================================

#[rstest]
#[tokio::test]
async fn test_mfa_verification_fails_with_invalid_code(
	mfa_manager: MfaManager,
	test_user: TestUser,
) {
	// Arrange
	mfa_manager
		.register_user(&test_user.username, VALID_SECRET)
		.await;

	// Act - Try obviously invalid codes
	let invalid_codes = vec!["000000", "111111", "999999", "123456", "abcdef"];

	for invalid_code in invalid_codes {
		// Skip if by chance this is the current valid code
		let current_valid = generate_current_totp(VALID_SECRET, 30);
		if invalid_code == current_valid {
			continue;
		}

		let result = mfa_manager
			.verify_totp(&test_user.username, invalid_code)
			.await;

		// Assert
		assert!(result.is_ok(), "Verification should not error");
		assert!(
			!result.unwrap(),
			"Invalid code '{}' should be rejected",
			invalid_code
		);
	}
}

#[rstest]
#[tokio::test]
async fn test_mfa_verification_fails_for_unregistered_user(mfa_manager: MfaManager) {
	// Act - Try to verify for non-existent user
	let result = mfa_manager.verify_totp("nonexistent_user", "123456").await;

	// Assert
	assert!(result.is_err(), "Verification should fail for unknown user");
	match result.unwrap_err() {
		AuthenticationError::UserNotFound => {}
		other => panic!("Expected UserNotFound error, got: {:?}", other),
	}
}

#[rstest]
#[tokio::test]
async fn test_mfa_verification_fails_with_malformed_secret(mfa_manager: MfaManager) {
	// Arrange - Register with invalid base32 secret
	let invalid_secrets = vec!["not-base32!", "12345678", "!!!@@@###"];

	for (i, invalid_secret) in invalid_secrets.iter().enumerate() {
		let username = format!("user_{}", i);
		mfa_manager.register_user(&username, *invalid_secret).await;

		// Act
		let result = mfa_manager.verify_totp(&username, "123456").await;

		// Assert - Should error due to invalid secret
		assert!(
			result.is_err(),
			"Verification with invalid secret '{}' should fail",
			invalid_secret
		);
		match result.unwrap_err() {
			AuthenticationError::InvalidCredentials => {}
			other => panic!(
				"Expected InvalidCredentials for '{}', got: {:?}",
				invalid_secret, other
			),
		}
	}
}

#[rstest]
#[tokio::test]
async fn test_mfa_verification_empty_code(mfa_manager: MfaManager, test_user: TestUser) {
	// Arrange
	mfa_manager
		.register_user(&test_user.username, VALID_SECRET)
		.await;

	// Act
	let result = mfa_manager.verify_totp(&test_user.username, "").await;

	// Assert
	assert!(result.is_ok(), "Empty code should not cause error");
	assert!(!result.unwrap(), "Empty code should be rejected");
}

#[rstest]
#[tokio::test]
async fn test_mfa_get_secret_returns_none_for_unregistered(mfa_manager: MfaManager) {
	// Act
	let result = mfa_manager.get_secret("unregistered_user").await;

	// Assert
	assert!(result.is_none(), "Unregistered user should have no secret");
}

// =============================================================================
// State Transition Tests
// =============================================================================

#[rstest]
#[tokio::test]
async fn test_mfa_secret_update_overwrites_old_secret(
	mfa_manager: MfaManager,
	test_user: TestUser,
) {
	// Arrange - Register with initial secret
	mfa_manager
		.register_user(&test_user.username, VALID_SECRET)
		.await;
	let initial_secret = mfa_manager.get_secret(&test_user.username).await.unwrap();

	// Act - Register again with different secret
	mfa_manager
		.register_user(&test_user.username, VALID_SECRET_2)
		.await;
	let updated_secret = mfa_manager.get_secret(&test_user.username).await.unwrap();

	// Assert
	assert_ne!(initial_secret, updated_secret, "Secret should be updated");
	assert_eq!(
		updated_secret, VALID_SECRET_2,
		"New secret should be stored"
	);
}

#[rstest]
#[tokio::test]
async fn test_mfa_old_codes_invalid_after_secret_change(
	mfa_manager: MfaManager,
	test_user: TestUser,
) {
	// Arrange - Register and generate code
	mfa_manager
		.register_user(&test_user.username, VALID_SECRET)
		.await;
	let old_code = generate_current_totp(VALID_SECRET, 30);

	// Act - Update secret
	mfa_manager
		.register_user(&test_user.username, VALID_SECRET_2)
		.await;

	// Old code should no longer be valid
	let result = mfa_manager
		.verify_totp(&test_user.username, &old_code)
		.await;

	// Assert (code might still be valid if it happens to match new secret, but unlikely)
	// The key point is the new secret should generate different codes
	let new_code = generate_current_totp(VALID_SECRET_2, 30);
	let new_result = mfa_manager
		.verify_totp(&test_user.username, &new_code)
		.await;

	assert!(
		new_result.is_ok() && new_result.unwrap(),
		"New code should be valid"
	);

	// If old and new codes are different, old code should be invalid
	if old_code != new_code {
		assert!(
			result.is_ok() && !result.unwrap(),
			"Old code should be invalid after secret change"
		);
	}
}

#[rstest]
#[tokio::test]
async fn test_mfa_registration_state_transitions(mfa_manager: MfaManager) {
	let username = "state_test_user";

	// State 1: Unregistered
	assert!(
		mfa_manager.get_secret(username).await.is_none(),
		"Initial state: no secret"
	);
	assert!(
		mfa_manager.verify_totp(username, "123456").await.is_err(),
		"Unregistered: verification should fail"
	);

	// Transition: Register
	mfa_manager.register_user(username, VALID_SECRET).await;

	// State 2: Registered
	assert!(
		mfa_manager.get_secret(username).await.is_some(),
		"Registered state: has secret"
	);
	let valid_code = generate_current_totp(VALID_SECRET, 30);
	assert!(
		mfa_manager
			.verify_totp(username, &valid_code)
			.await
			.unwrap(),
		"Registered: valid code should work"
	);

	// Transition: Update secret
	mfa_manager.register_user(username, VALID_SECRET_2).await;

	// State 3: Re-registered with new secret
	let new_secret = mfa_manager.get_secret(username).await.unwrap();
	assert_eq!(new_secret, VALID_SECRET_2, "Secret should be updated");
}

// =============================================================================
// Edge Cases Tests
// =============================================================================

#[rstest]
#[case("000000")]
#[case("999999")]
#[case("123456")]
#[case("654321")]
#[tokio::test]
async fn test_mfa_verification_with_boundary_codes(
	mfa_manager: MfaManager,
	test_user: TestUser,
	#[case] boundary_code: &str,
) {
	// Arrange
	mfa_manager
		.register_user(&test_user.username, VALID_SECRET)
		.await;

	// Act
	let result = mfa_manager
		.verify_totp(&test_user.username, boundary_code)
		.await;

	// Assert - Should not error, just return valid/invalid
	assert!(
		result.is_ok(),
		"Boundary code '{}' should not cause error",
		boundary_code
	);

	// Check if this boundary code happens to be the current valid code
	let current_valid = generate_current_totp(VALID_SECRET, 30);
	if boundary_code == current_valid {
		assert!(result.unwrap(), "Should accept matching boundary code");
	} else {
		assert!(!result.unwrap(), "Should reject non-matching boundary code");
	}
}

#[rstest]
#[case("user_with_underscore")]
#[case("user-with-dash")]
#[case("user.with.dot")]
#[case("user@example.com")]
#[case("用户名")] // Chinese characters
#[case("ユーザー")] // Japanese characters
#[tokio::test]
async fn test_mfa_registration_with_special_usernames(
	mfa_manager: MfaManager,
	#[case] username: &str,
) {
	// Act
	mfa_manager.register_user(username, VALID_SECRET).await;

	// Assert
	let stored_secret = mfa_manager.get_secret(username).await;
	assert!(
		stored_secret.is_some(),
		"Username '{}' should be registered",
		username
	);
	assert_eq!(
		stored_secret.unwrap(),
		VALID_SECRET,
		"Secret should be stored for '{}'",
		username
	);
}

#[rstest]
fn test_mfa_totp_codes_are_unique_for_different_secrets(_mfa_manager: MfaManager) {
	// Arrange - Different secrets
	let secrets = vec![VALID_SECRET, VALID_SECRET_2, "MFRGGZDFMY"];

	// Act - Generate codes for each secret
	let codes: Vec<String> = secrets
		.iter()
		.map(|s| generate_current_totp(s, 30))
		.collect();

	// Assert - While collisions are possible, different secrets should generally produce different codes
	let unique_codes: HashSet<&String> = codes.iter().collect();

	// At least 2 of 3 codes should be different (allowing for one collision)
	assert!(
		unique_codes.len() >= 2,
		"Different secrets should mostly produce different codes"
	);
}

#[rstest]
#[tokio::test]
async fn test_mfa_concurrent_registrations(mfa_manager: MfaManager) {
	use std::sync::Arc;
	use tokio::task::JoinSet;

	let mfa = Arc::new(mfa_manager);
	let mut join_set = JoinSet::new();

	// Spawn multiple tasks registering different users
	for i in 0..10 {
		let mfa_clone = Arc::clone(&mfa);
		join_set.spawn(async move {
			let username = format!("concurrent_user_{}", i);
			mfa_clone.register_user(&username, VALID_SECRET).await;
			username
		});
	}

	// Wait for all tasks
	let mut usernames = Vec::new();
	while let Some(result) = join_set.join_next().await {
		usernames.push(result.unwrap());
	}

	// Assert - All users should be registered
	for username in usernames {
		let secret = mfa.get_secret(&username).await;
		assert!(
			secret.is_some(),
			"User '{}' should be registered after concurrent registration",
			username
		);
	}
}

// =============================================================================
// Decision Table Tests
// =============================================================================

#[rstest]
#[case(true, true, true)] // Registered + Valid code = Valid
#[case(true, false, false)] // Registered + Invalid code = Invalid
#[tokio::test]
async fn test_mfa_verification_decision_table(
	mfa_manager: MfaManager,
	#[case] is_registered: bool,
	#[case] is_code_valid: bool,
	#[case] expected_valid: bool,
) {
	let username = "decision_table_user";

	// Setup based on conditions
	if is_registered {
		mfa_manager.register_user(username, VALID_SECRET).await;
	}

	let code = if is_code_valid && is_registered {
		generate_current_totp(VALID_SECRET, 30)
	} else {
		"000000".to_string() // Invalid code
	};

	// Act
	let result = mfa_manager.verify_totp(username, &code).await;

	// Assert
	if !is_registered {
		assert!(result.is_err(), "Unregistered user should error");
	} else {
		assert!(result.is_ok(), "Registered user should not error");
		assert_eq!(
			result.unwrap(),
			expected_valid,
			"Verification for (registered={}, valid_code={}) should be {}",
			is_registered,
			is_code_valid,
			expected_valid
		);
	}
}

// =============================================================================
// Use Case Tests
// =============================================================================

#[rstest]
#[tokio::test]
async fn test_use_case_complete_mfa_enrollment_flow(mfa_manager: MfaManager, test_user: TestUser) {
	// Step 1: Generate secret and TOTP URL for QR code
	let secret = VALID_SECRET;
	let totp_url = mfa_manager.generate_totp_url(&test_user.username, secret);

	// Assert URL is well-formed
	assert!(totp_url.contains("otpauth://"));
	assert!(totp_url.contains(&test_user.username));

	// Step 2: Register user with secret
	mfa_manager.register_user(&test_user.username, secret).await;

	// Assert registration succeeded
	assert!(mfa_manager.get_secret(&test_user.username).await.is_some());

	// Step 3: User enters TOTP code from authenticator app
	let totp_code = generate_current_totp(secret, 30);

	// Step 4: Verify the code
	let verification_result = mfa_manager
		.verify_totp(&test_user.username, &totp_code)
		.await;

	// Assert verification succeeded
	assert!(verification_result.is_ok());
	assert!(
		verification_result.unwrap(),
		"MFA enrollment should complete successfully"
	);
}

#[rstest]
#[tokio::test]
async fn test_use_case_failed_verification_lockout_simulation(mfa_manager: MfaManager) {
	// Simulate multiple failed verification attempts
	let username = "locked_user";
	mfa_manager.register_user(username, VALID_SECRET).await;

	let mut failed_attempts = 0;
	let max_attempts = 5;

	// Try invalid codes
	for _ in 0..max_attempts {
		let result = mfa_manager.verify_totp(username, "000000").await;
		if result.is_ok() && !result.unwrap() {
			failed_attempts += 1;
		}
	}

	// Assert - All attempts should have failed
	assert_eq!(
		failed_attempts, max_attempts,
		"All invalid code attempts should fail"
	);

	// Valid code should still work (no actual lockout in current implementation)
	let valid_code = generate_current_totp(VALID_SECRET, 30);
	let result = mfa_manager.verify_totp(username, &valid_code).await;
	assert!(
		result.is_ok() && result.unwrap(),
		"Valid code should work after failed attempts"
	);
}

// =============================================================================
// Sanity Tests
// =============================================================================

#[rstest]
#[tokio::test]
async fn test_mfa_default_configuration() {
	// Act
	let mfa = MfaManager::default();

	// Assert
	assert!(
		mfa.get_secret("nonexistent").await.is_none(),
		"Default MFA should have no registered users"
	);

	// Can register and verify
	mfa.register_user("test", VALID_SECRET).await;
	assert!(mfa.get_secret("test").await.is_some());
}

#[rstest]
fn test_mfa_issuer_in_totp_url() {
	// Arrange
	let custom_issuer = "CustomAppName";
	let mfa = MfaManager::new(custom_issuer);

	// Act
	let url = mfa.generate_totp_url("user", VALID_SECRET);

	// Assert
	assert!(
		url.contains(custom_issuer),
		"TOTP URL should contain custom issuer"
	);
	let issuer_count = url.matches(custom_issuer).count();
	assert_eq!(
		issuer_count, 2,
		"Issuer should appear twice in URL (path and query param)"
	);
}
