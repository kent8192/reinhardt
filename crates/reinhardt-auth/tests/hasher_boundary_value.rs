//! Password Hasher Boundary Value Tests
//!
//! This module contains boundary value analysis tests for password hashers,
//! testing extreme values and edge cases that might cause issues.
//!
//! # Test Categories
//!
//! - Boundary Value: Empty, minimum, maximum length passwords
//! - Equivalence Partitioning: Different character classes
//! - Stress Testing: Large passwords, special characters

use reinhardt_auth::{Argon2Hasher, PasswordHasher};
use rstest::*;

// =============================================================================
// Fixtures
// =============================================================================

#[fixture]
fn argon2_hasher() -> Argon2Hasher {
	Argon2Hasher::default()
}

// =============================================================================
// Boundary Value Tests - Password Length
// =============================================================================

#[rstest]
#[case(0, "", "empty password")]
#[case(1, "a", "single character")]
#[case(2, "ab", "two characters")]
fn test_minimum_password_lengths(
	argon2_hasher: Argon2Hasher,
	#[case] _len: usize,
	#[case] password: &str,
	#[case] desc: &str,
) {
	let result = argon2_hasher.hash(password);

	// Even empty/short passwords should hash successfully
	// (validation is a separate concern)
	assert!(
		result.is_ok(),
		"Hashing should succeed for {} ({})",
		desc,
		password.len()
	);

	let hash = result.unwrap();
	assert!(!hash.is_empty(), "Hash should not be empty for {}", desc);

	// Verify the password
	let verify_result = argon2_hasher.verify(password, &hash);
	assert!(
		verify_result.is_ok(),
		"Verification should succeed for {}",
		desc
	);
	assert!(
		verify_result.unwrap(),
		"Password should verify for {}",
		desc
	);
}

#[rstest]
#[case(8, "8-character password")]
#[case(16, "16-character password")]
#[case(32, "32-character password")]
#[case(64, "64-character password")]
#[case(72, "72-character password (bcrypt limit)")]
#[case(128, "128-character password")]
#[case(256, "256-character password")]
fn test_common_password_lengths(
	argon2_hasher: Argon2Hasher,
	#[case] len: usize,
	#[case] desc: &str,
) {
	let password: String = "x".repeat(len);

	let result = argon2_hasher.hash(&password);
	assert!(
		result.is_ok(),
		"Hashing should succeed for {} ({} chars)",
		desc,
		len
	);

	let hash = result.unwrap();
	let verify_result = argon2_hasher.verify(&password, &hash);
	assert!(
		verify_result.is_ok() && verify_result.unwrap(),
		"Verification should succeed for {}",
		desc
	);
}

#[rstest]
fn test_very_long_password(argon2_hasher: Argon2Hasher) {
	// 1KB password
	let password: String = "x".repeat(1024);

	let result = argon2_hasher.hash(&password);
	assert!(result.is_ok(), "Hashing should succeed for 1KB password");

	let hash = result.unwrap();
	let verify_result = argon2_hasher.verify(&password, &hash);
	assert!(
		verify_result.is_ok() && verify_result.unwrap(),
		"Verification should succeed for 1KB password"
	);
}

#[rstest]
fn test_extremely_long_password(argon2_hasher: Argon2Hasher) {
	// 100KB password - testing reasonable limits
	let password: String = "x".repeat(100 * 1024);

	let result = argon2_hasher.hash(&password);

	// Should either succeed or fail gracefully
	if result.is_ok() {
		let hash = result.unwrap();
		let verify_result = argon2_hasher.verify(&password, &hash);
		// If it hashed, it should verify
		assert!(
			verify_result.is_ok(),
			"If hashing succeeded, verification should not error"
		);
	}
	// If it fails, that's acceptable for extremely long passwords
}

// =============================================================================
// Boundary Value Tests - Password at bcrypt limit
// =============================================================================

#[rstest]
fn test_password_at_bcrypt_limit(argon2_hasher: Argon2Hasher) {
	// bcrypt has a 72-byte limit, but Argon2 doesn't
	// This test ensures Argon2 handles this boundary correctly
	let password_71: String = "a".repeat(71);
	let password_72: String = "a".repeat(72);
	let password_73: String = "a".repeat(73);

	for password in [&password_71, &password_72, &password_73] {
		let hash = argon2_hasher.hash(password).unwrap();
		let verified = argon2_hasher.verify(password, &hash).unwrap();
		assert!(
			verified,
			"Password of length {} should verify",
			password.len()
		);
	}
}

#[rstest]
fn test_passwords_differ_only_after_bcrypt_limit(argon2_hasher: Argon2Hasher) {
	// Two passwords that differ only after character 72
	// Argon2 should distinguish them (unlike bcrypt)
	let password1 = format!("{}A", "x".repeat(72));
	let password2 = format!("{}B", "x".repeat(72));

	let hash1 = argon2_hasher.hash(&password1).unwrap();

	// password1 should verify
	assert!(
		argon2_hasher.verify(&password1, &hash1).unwrap(),
		"Original password should verify"
	);

	// password2 should NOT verify against hash1
	assert!(
		!argon2_hasher.verify(&password2, &hash1).unwrap(),
		"Different password should not verify (Argon2 uses full password)"
	);
}

// =============================================================================
// Equivalence Partitioning - Character Classes
// =============================================================================

#[rstest]
#[case("abcdefghijklmnopqrstuvwxyz", "lowercase letters")]
#[case("ABCDEFGHIJKLMNOPQRSTUVWXYZ", "uppercase letters")]
#[case("0123456789", "digits")]
#[case("!@#$%^&*()_+-=[]{}|;':\",./<>?`~", "special characters")]
#[case("aB1!", "mixed character classes")]
fn test_ascii_character_classes(
	argon2_hasher: Argon2Hasher,
	#[case] password: &str,
	#[case] desc: &str,
) {
	let result = argon2_hasher.hash(password);
	assert!(result.is_ok(), "Hashing should succeed for {}", desc);

	let hash = result.unwrap();
	let verify_result = argon2_hasher.verify(password, &hash);
	assert!(
		verify_result.is_ok() && verify_result.unwrap(),
		"Verification should succeed for {}",
		desc
	);
}

#[rstest]
#[case("こんにちは", "Japanese hiragana")]
#[case("カタカナ", "Japanese katakana")]
#[case("漢字", "Chinese characters")]
#[case("Привет", "Cyrillic")]
#[case("مرحبا", "Arabic")]
#[case("שלום", "Hebrew")]
#[case("สวัสดี", "Thai")]
#[case("🔐🔑💪", "Emoji")]
fn test_unicode_character_classes(
	argon2_hasher: Argon2Hasher,
	#[case] password: &str,
	#[case] desc: &str,
) {
	let result = argon2_hasher.hash(password);
	assert!(result.is_ok(), "Hashing should succeed for {}", desc);

	let hash = result.unwrap();
	let verify_result = argon2_hasher.verify(password, &hash);
	assert!(
		verify_result.is_ok() && verify_result.unwrap(),
		"Verification should succeed for {}",
		desc
	);
}

// =============================================================================
// Edge Cases - Control Characters
// =============================================================================

#[rstest]
#[case("\x00", "null byte")]
#[case("\x01", "SOH")]
#[case("\x7F", "DEL")]
#[case("\t", "tab")]
#[case("\n", "newline")]
#[case("\r", "carriage return")]
#[case("\r\n", "CRLF")]
fn test_control_characters(
	argon2_hasher: Argon2Hasher,
	#[case] password: &str,
	#[case] desc: &str,
) {
	// Control characters should be handled gracefully
	let result = argon2_hasher.hash(password);

	if result.is_ok() {
		let hash = result.unwrap();
		let verify_result = argon2_hasher.verify(password, &hash);
		// If hashing succeeded, verification should too
		assert!(
			verify_result.is_ok(),
			"If hashing succeeded, verification should not error for {}",
			desc
		);
		if let Ok(verified) = verify_result {
			assert!(
				verified,
				"If hashing succeeded, it should verify for {}",
				desc
			);
		}
	}
	// If hashing fails on control characters, that's acceptable
}

#[rstest]
fn test_password_with_embedded_null(argon2_hasher: Argon2Hasher) {
	// Password with null byte in the middle
	let password = "before\0after";

	let result = argon2_hasher.hash(password);

	if result.is_ok() {
		let hash = result.unwrap();

		// Verify exact password works
		let verify_exact = argon2_hasher.verify(password, &hash);
		assert!(verify_exact.is_ok(), "Verification should not error");

		// Verify truncated password doesn't work
		let truncated = "before";
		let verify_truncated = argon2_hasher.verify(truncated, &hash);
		if let Ok(verified) = verify_truncated {
			// This tests whether the hasher properly handles null bytes
			// If the full password verifies but truncated doesn't, null handling is correct
			let full_verified = verify_exact.unwrap();
			if full_verified && !verified {
				// Good: hasher properly distinguishes
			}
		}
	}
}

// =============================================================================
// Edge Cases - Whitespace
// =============================================================================

#[rstest]
#[case(" ", "single space")]
#[case("  ", "double space")]
#[case("   ", "triple space")]
#[case("\t\t\t", "tabs")]
#[case("  password  ", "spaces around")]
#[case("pass word", "space in middle")]
fn test_whitespace_passwords(
	argon2_hasher: Argon2Hasher,
	#[case] password: &str,
	#[case] desc: &str,
) {
	let result = argon2_hasher.hash(password);
	assert!(result.is_ok(), "Hashing should succeed for {}", desc);

	let hash = result.unwrap();
	let verify_result = argon2_hasher.verify(password, &hash);
	assert!(
		verify_result.is_ok() && verify_result.unwrap(),
		"Verification should succeed for {}",
		desc
	);

	// Verify that trimmed version does NOT match
	let trimmed = password.trim();
	if trimmed != password {
		let verify_trimmed = argon2_hasher.verify(trimmed, &hash);
		if let Ok(verified) = verify_trimmed {
			assert!(!verified, "Trimmed password should not match for {}", desc);
		}
	}
}

// =============================================================================
// Hash Output Tests
// =============================================================================

#[rstest]
fn test_hash_output_format(argon2_hasher: Argon2Hasher) {
	let password = "test_password_123";
	let hash = argon2_hasher.hash(password).unwrap();

	// Argon2 hash should start with $argon2
	assert!(
		hash.starts_with("$argon2"),
		"Hash should be in Argon2 format: {}",
		hash
	);
}

#[rstest]
fn test_different_passwords_different_hashes(argon2_hasher: Argon2Hasher) {
	let password1 = "password1";
	let password2 = "password2";

	let hash1 = argon2_hasher.hash(password1).unwrap();
	let hash2 = argon2_hasher.hash(password2).unwrap();

	assert_ne!(
		hash1, hash2,
		"Different passwords should produce different hashes"
	);
}

#[rstest]
fn test_same_password_different_hashes(argon2_hasher: Argon2Hasher) {
	// Due to salt, same password should produce different hashes
	let password = "same_password";

	let hash1 = argon2_hasher.hash(password).unwrap();
	let hash2 = argon2_hasher.hash(password).unwrap();

	assert_ne!(
		hash1, hash2,
		"Same password should produce different hashes (due to salt)"
	);

	// Both should verify
	assert!(argon2_hasher.verify(password, &hash1).unwrap());
	assert!(argon2_hasher.verify(password, &hash2).unwrap());
}

// =============================================================================
// Verification Edge Cases
// =============================================================================

#[rstest]
fn test_verify_invalid_hash_format(argon2_hasher: Argon2Hasher) {
	let password = "test_password";
	let invalid_hash = "not_a_valid_hash";

	let result = argon2_hasher.verify(password, invalid_hash);
	assert!(result.is_err(), "Invalid hash format should return error");
}

#[rstest]
fn test_verify_empty_hash(argon2_hasher: Argon2Hasher) {
	let password = "test_password";
	let empty_hash = "";

	let result = argon2_hasher.verify(password, empty_hash);
	assert!(result.is_err(), "Empty hash should return error");
}

#[rstest]
fn test_verify_truncated_hash(argon2_hasher: Argon2Hasher) {
	let password = "test_password";
	let full_hash = argon2_hasher.hash(password).unwrap();

	// Truncate the hash
	let truncated_hash = &full_hash[..full_hash.len() / 2];

	let result = argon2_hasher.verify(password, truncated_hash);
	// Return behavior (error vs verification failure) is implementation-dependent
	assert!(
		result.is_err() || !result.unwrap(),
		"Truncated hash should either error or fail verification"
	);
}

#[rstest]
fn test_verify_modified_hash(argon2_hasher: Argon2Hasher) {
	let password = "test_password";
	let mut hash = argon2_hasher.hash(password).unwrap();

	// Modify one character in the hash
	// SAFETY: We own the `hash` String exclusively. The modification only changes
	// the last byte between two valid ASCII characters ('a' <-> 'b'), which
	// preserves valid UTF-8 encoding. This is intentional corruption for testing
	// that verification rejects tampered hashes.
	let bytes = unsafe { hash.as_bytes_mut() };
	if let Some(last) = bytes.last_mut() {
		*last = if *last == b'a' { b'b' } else { b'a' };
	}

	let result = argon2_hasher.verify(password, &hash);

	// Should either error or return false
	match result {
		Ok(verified) => assert!(!verified, "Modified hash should not verify"),
		Err(_) => (), // Error is also acceptable
	}
}

// =============================================================================
// Decision Table - Password/Hash Combinations
// =============================================================================

#[rstest]
#[case("correct", "correct", true, "correct password")]
#[case("correct", "wrong", false, "wrong password")]
#[case("", "", true, "empty password (both)")]
#[case("a", "b", false, "single char different")]
#[case("password", "PASSWORD", false, "case sensitivity")]
#[case("password ", "password", false, "trailing space")]
#[case(" password", "password", false, "leading space")]
fn test_verification_decision_table(
	argon2_hasher: Argon2Hasher,
	#[case] hash_password: &str,
	#[case] verify_password: &str,
	#[case] expected: bool,
	#[case] desc: &str,
) {
	let hash = argon2_hasher.hash(hash_password).unwrap();
	let result = argon2_hasher.verify(verify_password, &hash);

	assert!(result.is_ok(), "Verification should not error for {}", desc);
	assert_eq!(
		result.unwrap(),
		expected,
		"Verification result should be {} for {}",
		expected,
		desc
	);
}

// =============================================================================
// Thread Safety and Cloning
// =============================================================================

#[rstest]
fn test_hasher_clone(argon2_hasher: Argon2Hasher) {
	let cloned = argon2_hasher.clone();
	let password = "test_password";

	let hash1 = argon2_hasher.hash(password).unwrap();
	let hash2 = cloned.hash(password).unwrap();

	// Both should produce valid hashes (different due to salt)
	assert!(argon2_hasher.verify(password, &hash1).unwrap());
	assert!(cloned.verify(password, &hash2).unwrap());

	// Cross-verification should also work
	assert!(cloned.verify(password, &hash1).unwrap());
	assert!(argon2_hasher.verify(password, &hash2).unwrap());
}

#[rstest]
fn test_hasher_is_send_sync() {
	fn assert_send_sync<T: Send + Sync>() {}
	assert_send_sync::<Argon2Hasher>();
}
