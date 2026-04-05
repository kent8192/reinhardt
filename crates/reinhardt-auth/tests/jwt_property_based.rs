//! JWT Property-Based Tests
//!
//! This module contains property-based tests for JWT authentication using proptest.
//! These tests verify invariants and properties that should hold for all inputs.
//!
//! # Properties Tested
//!
//! - Token roundtrip (encode then decode returns original data)
//! - Different secrets produce different tokens
//! - Different claims produce different tokens
//! - Token structure (always has 3 parts separated by dots)
//! - Expiration is always in the future when generated

use proptest::prelude::*;
use reinhardt_auth::jwt::{Claims, JwtAuth};

// =============================================================================
// Strategy Definitions
// =============================================================================

/// Strategy for generating valid secrets (minimum 16 bytes for security)
fn secret_strategy() -> impl Strategy<Value = Vec<u8>> {
	prop::collection::vec(any::<u8>(), 16..64)
}

/// Strategy for generating user IDs
fn user_id_strategy() -> impl Strategy<Value = String> {
	prop::string::string_regex("[a-zA-Z0-9_-]{1,100}").expect("Valid regex for user_id")
}

/// Strategy for generating usernames
fn username_strategy() -> impl Strategy<Value = String> {
	prop::string::string_regex("[a-zA-Z][a-zA-Z0-9_]{0,49}").expect("Valid regex for username")
}

/// Strategy for generating expiration durations in seconds (1 second to 1 year)
fn exp_seconds_strategy() -> impl Strategy<Value = i64> {
	1i64..(365 * 24 * 60 * 60)
}

// =============================================================================
// Property Tests - Token Roundtrip
// =============================================================================

proptest! {
	/// Property: Token generation and verification roundtrip preserves data
	///
	/// For any valid user_id and username, generating a token and then
	/// verifying it should return claims with the original user_id and username.
	#[test]
	fn test_jwt_roundtrip_preserves_user_data(
		secret in secret_strategy(),
		user_id in user_id_strategy(),
		username in username_strategy(),
	) {
		let jwt_auth = JwtAuth::new(&secret);

		let token = jwt_auth
			.generate_token(user_id.clone(), username.clone(), false, false)
			.expect("Token generation should succeed");

		let claims = jwt_auth
			.verify_token(&token)
			.expect("Token verification should succeed");

		prop_assert_eq!(claims.sub, user_id, "Subject should match original user_id");
		prop_assert_eq!(claims.username, username, "Username should match original");
	}

	/// Property: Decode after encode returns original claims data
	#[test]
	fn test_jwt_encode_decode_roundtrip(
		secret in secret_strategy(),
		user_id in user_id_strategy(),
		username in username_strategy(),
		exp_seconds in exp_seconds_strategy(),
	) {
		let jwt_auth = JwtAuth::new(&secret);
		let duration = chrono::Duration::seconds(exp_seconds);
		let claims = Claims::new(user_id.clone(), username.clone(), duration, false, false);

		let token = jwt_auth.encode(&claims).expect("Encoding should succeed");
		let decoded = jwt_auth.decode(&token).expect("Decoding should succeed");

		prop_assert_eq!(decoded.sub, user_id, "Decoded sub should match original");
		prop_assert_eq!(decoded.username, username, "Decoded username should match");
	}
}

// =============================================================================
// Property Tests - Token Uniqueness
// =============================================================================

proptest! {
	/// Property: Different secrets produce tokens that cannot be verified with each other
	///
	/// If we generate a token with secret A, it should fail verification with secret B.
	#[test]
	fn test_different_secrets_fail_verification(
		secret1 in secret_strategy(),
		secret2 in secret_strategy(),
		user_id in user_id_strategy(),
		username in username_strategy(),
	) {
		// Skip if secrets are identical
		prop_assume!(secret1 != secret2);

		let jwt_auth1 = JwtAuth::new(&secret1);
		let jwt_auth2 = JwtAuth::new(&secret2);

		let token = jwt_auth1
			.generate_token(user_id.clone(), username.clone(), false, false)
			.expect("Token generation should succeed");

		// Verification with different secret should fail
		let result = jwt_auth2.verify_token(&token);
		prop_assert!(
			result.is_err(),
			"Token should not verify with different secret"
		);
	}

	/// Property: Different user_ids produce different tokens
	#[test]
	fn test_different_user_ids_produce_different_tokens(
		secret in secret_strategy(),
		user_id1 in user_id_strategy(),
		user_id2 in user_id_strategy(),
		username in username_strategy(),
	) {
		prop_assume!(user_id1 != user_id2);

		let jwt_auth = JwtAuth::new(&secret);

		let token1 = jwt_auth
			.generate_token(user_id1, username.clone(), false, false)
			.expect("Token1 generation should succeed");

		let token2 = jwt_auth
			.generate_token(user_id2, username, false, false)
			.expect("Token2 generation should succeed");

		// Tokens should be different (due to different claims and timestamps)
		prop_assert_ne!(token1, token2, "Different user_ids should produce different tokens");
	}
}

// =============================================================================
// Property Tests - Token Structure
// =============================================================================

proptest! {
	/// Property: JWT tokens always have exactly 3 parts separated by dots
	#[test]
	fn test_jwt_token_has_three_parts(
		secret in secret_strategy(),
		user_id in user_id_strategy(),
		username in username_strategy(),
	) {
		let jwt_auth = JwtAuth::new(&secret);

		let token = jwt_auth
			.generate_token(user_id, username, false, false)
			.expect("Token generation should succeed");

		let parts: Vec<&str> = token.split('.').collect();

		prop_assert_eq!(
			parts.len(),
			3,
			"JWT token should have exactly 3 parts (header.payload.signature)"
		);

		// Each part should be non-empty
		for (i, part) in parts.iter().enumerate() {
			prop_assert!(
				!part.is_empty(),
				"JWT token part {} should not be empty",
				i
			);
		}
	}

	/// Property: JWT tokens are valid Base64 URL-safe encoded
	#[test]
	fn test_jwt_token_parts_are_base64_url_safe(
		secret in secret_strategy(),
		user_id in user_id_strategy(),
		username in username_strategy(),
	) {
		let jwt_auth = JwtAuth::new(&secret);

		let token = jwt_auth
			.generate_token(user_id, username, false, false)
			.expect("Token generation should succeed");

		let parts: Vec<&str> = token.split('.').collect();

		// Check that each part contains only Base64 URL-safe characters
		let base64_url_chars = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_=";

		for (i, part) in parts.iter().enumerate() {
			for ch in part.chars() {
				prop_assert!(
					base64_url_chars.contains(ch),
					"JWT token part {} contains invalid character: {}",
					i,
					ch
				);
			}
		}
	}
}

// =============================================================================
// Property Tests - Expiration
// =============================================================================

proptest! {
	/// Property: Generated claims expiration is always in the future
	#[test]
	fn test_claims_exp_always_in_future(
		user_id in user_id_strategy(),
		username in username_strategy(),
		exp_seconds in 1i64..10000i64,
	) {
		let now = chrono::Utc::now().timestamp();
		let duration = chrono::Duration::seconds(exp_seconds);
		let claims = Claims::new(user_id, username, duration, false, false);

		prop_assert!(
			claims.exp > now,
			"Claims expiration ({}) should be greater than current time ({})",
			claims.exp,
			now
		);

		prop_assert!(
			!claims.is_expired(),
			"Freshly created claims should not be expired"
		);
	}

	/// Property: Issued-at time is always less than or equal to expiration
	#[test]
	fn test_claims_iat_before_exp(
		user_id in user_id_strategy(),
		username in username_strategy(),
		exp_seconds in 1i64..10000i64,
	) {
		let duration = chrono::Duration::seconds(exp_seconds);
		let claims = Claims::new(user_id, username, duration, false, false);

		prop_assert!(
			claims.iat <= claims.exp,
			"Issued-at time ({}) should be <= expiration time ({})",
			claims.iat,
			claims.exp
		);
	}
}

// =============================================================================
// Property Tests - Edge Cases
// =============================================================================

proptest! {
	/// Property: Empty user_id and username still produce valid tokens
	#[test]
	fn test_empty_claims_produce_valid_tokens(
		secret in secret_strategy(),
	) {
		let jwt_auth = JwtAuth::new(&secret);

		let token = jwt_auth
			.generate_token(String::new(), String::new(), false, false)
			.expect("Token generation with empty claims should succeed");

		let claims = jwt_auth
			.verify_token(&token)
			.expect("Token verification should succeed");

		prop_assert_eq!(claims.sub, "", "Empty user_id should be preserved");
		prop_assert_eq!(claims.username, "", "Empty username should be preserved");
	}

	/// Property: Unicode in claims is handled correctly
	#[test]
	fn test_unicode_claims_roundtrip(
		secret in secret_strategy(),
		user_id in "[a-zA-Z0-9\u{4e00}-\u{9fa5}]{1,50}",
		username in "[a-zA-Z\u{3040}-\u{309F}]{1,50}",
	) {
		let jwt_auth = JwtAuth::new(&secret);

		let token = jwt_auth
			.generate_token(user_id.clone(), username.clone(), false, false)
			.expect("Token generation with unicode should succeed");

		let claims = jwt_auth
			.verify_token(&token)
			.expect("Token verification should succeed");

		prop_assert_eq!(claims.sub, user_id, "Unicode user_id should be preserved");
		prop_assert_eq!(claims.username, username, "Unicode username should be preserved");
	}

	/// Property: Long claims produce valid tokens
	#[test]
	fn test_long_claims_roundtrip(
		secret in secret_strategy(),
		user_id in "[a-zA-Z0-9]{100,500}",
		username in "[a-zA-Z0-9_]{100,500}",
	) {
		let jwt_auth = JwtAuth::new(&secret);

		let token = jwt_auth
			.generate_token(user_id.clone(), username.clone(), false, false)
			.expect("Token generation with long claims should succeed");

		let claims = jwt_auth
			.verify_token(&token)
			.expect("Token verification should succeed");

		prop_assert_eq!(claims.sub, user_id, "Long user_id should be preserved");
		prop_assert_eq!(claims.username, username, "Long username should be preserved");
	}
}

// =============================================================================
// Property Tests - Security Properties
// =============================================================================

proptest! {
	/// Property: Token tampering is detected
	#[test]
	fn test_token_tampering_detected(
		secret in secret_strategy(),
		user_id in user_id_strategy(),
		username in username_strategy(),
		tamper_index in 0usize..100usize,
	) {
		let jwt_auth = JwtAuth::new(&secret);

		let token = jwt_auth
			.generate_token(user_id, username, false, false)
			.expect("Token generation should succeed");

		// Only tamper if the index is within bounds
		if tamper_index < token.len() {
			let mut tampered_bytes: Vec<u8> = token.bytes().collect();

			// XOR the byte to change it
			tampered_bytes[tamper_index] ^= 0x01;

			// Try to convert back to string and verify
			if let Ok(tampered_token) = String::from_utf8(tampered_bytes) {
				if tampered_token != token {
					let result = jwt_auth.verify_token(&tampered_token);
					prop_assert!(
						result.is_err(),
						"Tampered token should not verify successfully"
					);
				}
			}
		}
	}

	/// Property: Minimum secret length produces valid tokens
	#[test]
	fn test_minimum_secret_length(
		user_id in user_id_strategy(),
		username in username_strategy(),
	) {
		// Minimum practical secret size (1 byte, though not recommended)
		let secret = vec![0xABu8];
		let jwt_auth = JwtAuth::new(&secret);

		let token = jwt_auth
			.generate_token(user_id.clone(), username.clone(), false, false)
			.expect("Token generation with minimal secret should succeed");

		let claims = jwt_auth
			.verify_token(&token)
			.expect("Token verification should succeed");

		prop_assert_eq!(claims.sub, user_id);
		prop_assert_eq!(claims.username, username);
	}
}

// =============================================================================
// Non-Property Tests (Using rstest for better organization)
// =============================================================================

#[cfg(test)]
mod sanity_tests {
	use super::*;
	use rstest::*;

	/// Fixture for a standard JWT authenticator
	#[fixture]
	fn jwt_auth() -> JwtAuth {
		JwtAuth::new(b"test-secret-for-sanity-tests-32bytes!")
	}

	#[rstest]
	fn test_basic_token_generation_and_verification(jwt_auth: JwtAuth) {
		let token = jwt_auth
			.generate_token("user123".to_string(), "alice".to_string(), false, false)
			.expect("Token generation should succeed");

		let claims = jwt_auth
			.verify_token(&token)
			.expect("Token verification should succeed");

		assert_eq!(claims.sub, "user123");
		assert_eq!(claims.username, "alice");
		assert!(!claims.is_expired());
	}

	#[rstest]
	fn test_token_with_special_characters(jwt_auth: JwtAuth) {
		let user_id = "user@example.com".to_string();
		let username = "John Doe <admin>".to_string();

		let token = jwt_auth
			.generate_token(user_id.clone(), username.clone(), false, false)
			.expect("Token generation should succeed");

		let claims = jwt_auth
			.verify_token(&token)
			.expect("Token verification should succeed");

		assert_eq!(claims.sub, user_id);
		assert_eq!(claims.username, username);
	}

	#[rstest]
	fn test_invalid_token_format_rejected(jwt_auth: JwtAuth) {
		let invalid_tokens = vec![
			"",
			"not.a.valid.token",
			"only-one-part",
			"two.parts",
			"a.b.c.d.e",
			"eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.invalid.signature",
		];

		for invalid_token in invalid_tokens {
			let result = jwt_auth.verify_token(invalid_token);
			assert!(
				result.is_err(),
				"Token '{}' should be rejected",
				invalid_token
			);
		}
	}
}
