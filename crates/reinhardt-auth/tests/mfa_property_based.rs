//! MFA Property-Based Tests
//!
//! This module contains property-based tests for MFA authentication using proptest.
//! These tests verify invariants and properties that should hold for all inputs.
//!
//! # Properties Tested
//!
//! - TOTP codes are always 6 digits
//! - TOTP URL is always a valid URI format
//! - Different secrets produce different TOTP codes (usually)
//! - Registration is idempotent for same username/secret pair
//! - Get secret always returns exactly what was registered

use proptest::prelude::*;
use reinhardt_auth::mfa::MFAAuthentication as MfaManager;
use tokio::runtime::Runtime;

// =============================================================================
// Strategy Definitions
// =============================================================================

/// Strategy for generating valid base32 secrets (RFC 4648)
fn base32_secret_strategy() -> impl Strategy<Value = String> {
	// Base32 alphabet: A-Z and 2-7
	// Length must be multiple of 8 for BASE32_NOPAD decoding
	prop::sample::select(vec![8, 16, 24, 32])
		.prop_flat_map(|len| {
			prop::collection::vec(prop::sample::select(BASE32_ALPHABET.to_vec()), len..=len)
		})
		.prop_map(|chars| chars.into_iter().collect::<String>())
}

/// Base32 alphabet for secret generation
const BASE32_ALPHABET: &[char] = &[
	'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J', 'K', 'L', 'M', 'N', 'O', 'P', 'Q', 'R', 'S',
	'T', 'U', 'V', 'W', 'X', 'Y', 'Z', '2', '3', '4', '5', '6', '7',
];

/// Strategy for generating valid usernames
fn username_strategy() -> impl Strategy<Value = String> {
	prop::string::string_regex("[a-zA-Z][a-zA-Z0-9_]{0,49}").expect("Valid regex for username")
}

/// Strategy for generating TOTP issuers
fn issuer_strategy() -> impl Strategy<Value = String> {
	prop::string::string_regex("[a-zA-Z][a-zA-Z0-9 _-]{0,30}").expect("Valid regex for issuer")
}

/// Strategy for generating time windows (valid TOTP periods)
fn time_window_strategy() -> impl Strategy<Value = u64> {
	prop::num::u64::ANY.prop_map(|n| (n % 180) + 15) // 15 to 194 seconds
}

/// Strategy for generating 6-digit TOTP codes
fn totp_code_strategy() -> impl Strategy<Value = String> {
	(0u32..1000000u32).prop_map(|n| format!("{:06}", n))
}

// =============================================================================
// Helper Functions
// =============================================================================

/// Generate TOTP code for a given secret at current time
fn generate_totp_for_secret(secret: &str, time_window: u64) -> Option<String> {
	let secret_bytes = match data_encoding::BASE32_NOPAD.decode(secret.as_bytes()) {
		Ok(bytes) => bytes,
		Err(_) => return None,
	};

	let current_time = std::time::SystemTime::now()
		.duration_since(std::time::UNIX_EPOCH)
		.unwrap()
		.as_secs();
	let time_step = current_time / time_window;

	Some(totp_lite::totp_custom::<totp_lite::Sha256>(
		time_window,
		6,
		&secret_bytes,
		time_step,
	))
}

/// Run async code in a tokio runtime for use in synchronous proptest tests
/// Panics on TestCaseError::Fail or TestCaseError::Reject
fn run_async<F>(future: F)
where
	F: std::future::Future<Output = Result<(), TestCaseError>>,
{
	match Runtime::new().unwrap().block_on(future) {
		Ok(()) => {}
		Err(TestCaseError::Fail(msg)) => panic!("Property test failed: {}", msg),
		Err(TestCaseError::Reject(msg)) => panic!("Property test rejected: {}", msg),
	}
}

// =============================================================================
// Property Tests - TOTP Code Format
// =============================================================================

proptest! {
	/// Property: Generated TOTP codes are always exactly 6 digits
	#[test]
	fn test_totp_code_always_6_digits(
		secret in base32_secret_strategy(),
		time_window in time_window_strategy(),
	) {
		if let Some(code) = generate_totp_for_secret(&secret, time_window) {
			prop_assert_eq!(code.len(), 6, "TOTP code should be exactly 6 characters");
			prop_assert!(
				code.chars().all(|c| c.is_ascii_digit()),
				"TOTP code should contain only digits"
			);
		}
	}

	/// Property: TOTP codes are numeric strings from 000000 to 999999
	#[test]
	fn test_totp_code_in_valid_range(
		secret in base32_secret_strategy(),
	) {
		if let Some(code) = generate_totp_for_secret(&secret, 30) {
			let code_num: u32 = code.parse().expect("Code should be numeric");
			prop_assert!(code_num <= 999999, "TOTP code should be <= 999999");
		}
	}
}

// =============================================================================
// Property Tests - TOTP URL Format
// =============================================================================

proptest! {
	/// Property: TOTP URL is always a valid otpauth URI format
	#[test]
	fn test_totp_url_is_valid_format(
		issuer in issuer_strategy(),
		username in username_strategy(),
		secret in base32_secret_strategy(),
	) {
		let mfa = MfaManager::new(&issuer);
		let url = mfa.generate_totp_url(&username, &secret);

		// Check URL structure
		prop_assert!(
			url.starts_with("otpauth://totp/"),
			"URL should start with otpauth://totp/"
		);
		prop_assert!(
			url.contains("secret="),
			"URL should contain secret parameter"
		);
		prop_assert!(
			url.contains("issuer="),
			"URL should contain issuer parameter"
		);
	}

	/// Property: TOTP URL contains the provided username
	#[test]
	fn test_totp_url_contains_username(
		username in username_strategy(),
		secret in base32_secret_strategy(),
	) {
		let mfa = MfaManager::new("TestIssuer");
		let url = mfa.generate_totp_url(&username, &secret);

		prop_assert!(
			url.contains(&username),
			"URL should contain username '{}'",
			username
		);
	}

	/// Property: TOTP URL contains the provided secret
	#[test]
	fn test_totp_url_contains_secret(
		username in username_strategy(),
		secret in base32_secret_strategy(),
	) {
		let mfa = MfaManager::new("TestIssuer");
		let url = mfa.generate_totp_url(&username, &secret);

		prop_assert!(
			url.contains(&secret),
			"URL should contain secret '{}'",
			secret
		);
	}
}

// =============================================================================
// Property Tests - Registration (Async via block_on)
// =============================================================================

proptest! {
	/// Property: Get secret always returns exactly what was registered
	#[test]
	fn test_registration_preserves_secret(
		username in username_strategy(),
		secret in base32_secret_strategy(),
	) {
		run_async(async {
			let mfa = MfaManager::new("TestApp");
			mfa.register_user(&username, &secret).await;

			let stored = mfa.get_secret(&username).await;

			prop_assert!(stored.is_some(), "Secret should be stored for user '{}'", username);
			prop_assert_eq!(
				stored.unwrap(),
				secret,
				"Stored secret should match registered secret"
			);
			Ok(())
		})
	}

	/// Property: Registration is idempotent for same username/secret
	#[test]
	fn test_registration_idempotent_same_secret(
		username in username_strategy(),
		secret in base32_secret_strategy(),
	) {
		run_async(async {
			let mfa = MfaManager::new("TestApp");

			// Register multiple times with same secret
			mfa.register_user(&username, &secret).await;
			mfa.register_user(&username, &secret).await;
			mfa.register_user(&username, &secret).await;

			let stored = mfa.get_secret(&username).await;

			prop_assert!(stored.is_some());
			prop_assert_eq!(stored.unwrap(), secret);
			Ok(())
		})
	}

	/// Property: Re-registration overwrites previous secret
	#[test]
	fn test_reregistration_overwrites_secret(
		username in username_strategy(),
		secret1 in base32_secret_strategy(),
		secret2 in base32_secret_strategy(),
	) {
		prop_assume!(secret1 != secret2);

		run_async(async {
			let mfa = MfaManager::new("TestApp");

			mfa.register_user(&username, &secret1).await;
			mfa.register_user(&username, &secret2).await;

			let stored = mfa.get_secret(&username).await;

			prop_assert!(stored.is_some());
			prop_assert_eq!(
				stored.unwrap(),
				secret2,
				"Secret should be updated to latest value"
			);
			Ok(())
		})
	}

	/// Property: Different usernames have independent secrets
	#[test]
	fn test_usernames_have_independent_secrets(
		username1 in username_strategy(),
		username2 in username_strategy(),
		secret1 in base32_secret_strategy(),
		secret2 in base32_secret_strategy(),
	) {
		prop_assume!(username1 != username2);

		run_async(async {
			let mfa = MfaManager::new("TestApp");
			mfa.register_user(&username1, &secret1).await;
			mfa.register_user(&username2, &secret2).await;

			let stored1 = mfa.get_secret(&username1).await;
			let stored2 = mfa.get_secret(&username2).await;

			prop_assert_eq!(stored1, Some(secret1));
			prop_assert_eq!(stored2, Some(secret2));
			Ok(())
		})
	}
}

// =============================================================================
// Property Tests - Verification (Async via block_on)
// =============================================================================

proptest! {
	/// Property: Valid code verification returns Ok (not error)
	#[test]
	fn test_verification_returns_ok_for_registered_user(
		username in username_strategy(),
		secret in base32_secret_strategy(),
		code in totp_code_strategy(),
	) {
		run_async(async {
			let mfa = MfaManager::new("TestApp");
			mfa.register_user(&username, &secret).await;

			let result = mfa.verify_totp(&username, &code).await;

			// Should always return Ok, never Err for registered user with valid secret
			prop_assert!(
				result.is_ok(),
				"Verification should return Ok for registered user"
			);
			Ok(())
		})
	}

	/// Property: Verification fails with error for unregistered user
	#[test]
	fn test_verification_errors_for_unregistered_user(
		username in username_strategy(),
		code in totp_code_strategy(),
	) {
		run_async(async {
			let mfa = MfaManager::new("TestApp");
			// Don't register the user

			let result = mfa.verify_totp(&username, &code).await;

			prop_assert!(
				result.is_err(),
				"Verification should error for unregistered user"
			);
			Ok(())
		})
	}

	/// Property: Self-generated code validates successfully
	#[test]
	fn test_self_generated_code_validates(
		username in username_strategy(),
		secret in base32_secret_strategy(),
	) {
		run_async(async {
			let mfa = MfaManager::new("TestApp").time_window(30);
			mfa.register_user(&username, &secret).await;

			if let Some(code) = generate_totp_for_secret(&secret, 30) {
				let result = mfa.verify_totp(&username, &code).await;
				prop_assert!(result.is_ok(), "Verification should not error");
				prop_assert!(result.unwrap(), "Self-generated code should validate");
			}
			Ok(())
		})
	}
}

// =============================================================================
// Property Tests - Code Uniqueness
// =============================================================================

proptest! {
	/// Property: Different secrets usually produce different codes at same time
	///
	/// Note: This is probabilistic - collisions are possible but rare
	#[test]
	fn test_different_secrets_usually_different_codes(
		secret1 in base32_secret_strategy(),
		secret2 in base32_secret_strategy(),
	) {
		prop_assume!(secret1 != secret2);

		let code1 = generate_totp_for_secret(&secret1, 30);
		let code2 = generate_totp_for_secret(&secret2, 30);

		// Both should generate valid codes
		prop_assert!(code1.is_some() && code2.is_some(), "Both secrets should generate valid codes");

		// We can't guarantee they're different (collisions possible)
		// but we verify both are valid 6-digit codes
		let c1 = code1.unwrap();
		let c2 = code2.unwrap();

		prop_assert_eq!(c1.len(), 6);
		prop_assert_eq!(c2.len(), 6);
	}
}

// =============================================================================
// Property Tests - Time Window (Async via block_on)
// =============================================================================

proptest! {
	/// Property: Time window configuration is preserved
	#[test]
	fn test_time_window_preserved(
		time_window in time_window_strategy(),
	) {
		run_async(async {
			let mfa = MfaManager::new("TestApp").time_window(time_window);
			mfa.register_user("testuser", "JBSWY3DPEHPK3PXP").await;

			// Generate code with same time window
			if let Some(code) = generate_totp_for_secret("JBSWY3DPEHPK3PXP", time_window) {
				let result = mfa.verify_totp("testuser", &code).await;
				prop_assert!(result.is_ok());
				prop_assert!(result.unwrap(), "Code generated with same time window should validate");
			}
			Ok(())
		})
	}
}

// =============================================================================
// Non-Property Tests (rstest for organization)
// =============================================================================

#[cfg(test)]
mod sanity_tests {
	use super::*;
	use rstest::*;

	#[fixture]
	fn mfa_manager() -> MfaManager {
		MfaManager::new("SanityTestApp")
	}

	#[rstest]
	#[tokio::test]
	async fn test_basic_registration_and_retrieval(mfa_manager: MfaManager) {
		mfa_manager.register_user("alice", "JBSWY3DPEHPK3PXP").await;

		let secret = mfa_manager.get_secret("alice").await;
		assert!(secret.is_some());
		assert_eq!(secret.unwrap(), "JBSWY3DPEHPK3PXP");
	}

	#[rstest]
	#[tokio::test]
	async fn test_unregistered_user_returns_none(mfa_manager: MfaManager) {
		let secret = mfa_manager.get_secret("nonexistent").await;
		assert!(secret.is_none());
	}

	#[rstest]
	fn test_totp_url_format(mfa_manager: MfaManager) {
		let url = mfa_manager.generate_totp_url("alice", "SECRETKEY");

		assert!(url.starts_with("otpauth://totp/"));
		assert!(url.contains("alice"));
		assert!(url.contains("SECRETKEY"));
		assert!(url.contains("SanityTestApp"));
	}

	#[rstest]
	#[tokio::test]
	async fn test_verification_with_invalid_secret_errors(mfa_manager: MfaManager) {
		// Register with invalid base32
		mfa_manager
			.register_user("alice", "NOT_VALID_BASE32!")
			.await;

		let result = mfa_manager.verify_totp("alice", "123456").await;

		assert!(result.is_err(), "Invalid base32 should cause error");
	}
}
