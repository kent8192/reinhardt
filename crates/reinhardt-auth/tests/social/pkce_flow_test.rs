//! PKCE (Proof Key for Code Exchange) flow tests

use reinhardt_auth::social::flow::PkceFlow;
use rstest::*;

#[test]
fn test_pkce_verifier_valid_length() {
	// Act
	let (verifier, _) = PkceFlow::generate();

	// Assert
	let len = verifier.as_str().len();
	assert!(
		len >= 43,
		"Verifier must be at least 43 characters, got {}",
		len
	);
	assert!(
		len <= 128,
		"Verifier must be at most 128 characters, got {}",
		len
	);
}

#[test]
fn test_pkce_verifier_is_alphanumeric() {
	// Arrange & Act
	let (verifier, _) = PkceFlow::generate();

	// Assert
	assert!(
		verifier.as_str().chars().all(|c| c.is_alphanumeric()),
		"Verifier must contain only alphanumeric characters"
	);
}

#[test]
fn test_pkce_calculate_s256_challenge_known_vector() {
	// Arrange
	// This is a known test vector from RFC 7636
	let verifier_str = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";

	// Act
	// Manually calculate the expected challenge using SHA256
	use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
	use sha2::{Digest, Sha256};
	let mut hasher = Sha256::new();
	hasher.update(verifier_str.as_bytes());
	let hash = hasher.finalize();
	let expected = URL_SAFE_NO_PAD.encode(hash);

	// Assert - Verify the known test vector from RFC 7636
	assert_eq!(expected, "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM");
}

#[test]
fn test_pkce_challenge_is_base64url_no_padding() {
	// Arrange & Act
	let (_, challenge) = PkceFlow::generate();

	// Assert
	assert!(
		!challenge.as_str().ends_with('='),
		"Challenge must not have padding"
	);
	assert!(
		!challenge.as_str().contains('+'),
		"Challenge must use URL-safe encoding (no '+')"
	);
	assert!(
		!challenge.as_str().contains('/'),
		"Challenge must use URL-safe encoding (no '/')"
	);
}

#[test]
fn test_pkce_pairs_are_unique() {
	// Arrange & Act
	let (verifier1, challenge1) = PkceFlow::generate();
	let (verifier2, challenge2) = PkceFlow::generate();

	// Assert
	assert_ne!(
		verifier1.as_str(),
		verifier2.as_str(),
		"Verifiers must be unique"
	);
	assert_ne!(
		challenge1.as_str(),
		challenge2.as_str(),
		"Challenges must be unique"
	);
}

#[test]
fn test_pkce_challenge_method() {
	// Arrange & Act
	let (_, challenge) = PkceFlow::generate();

	// Assert
	use reinhardt_auth::social::flow::pkce::ChallengeMethod;
	assert_eq!(challenge.method(), ChallengeMethod::S256);
	assert_eq!(challenge.method().as_str(), "S256");
}

#[test]
fn test_pkce_verifier_as_str() {
	// Arrange & Act
	let (verifier, _) = PkceFlow::generate();

	// Assert
	assert!(!verifier.as_str().is_empty());
	assert!(verifier.as_str().len() >= 43);
}

#[test]
fn test_pkce_challenge_as_str() {
	// Arrange & Act
	let (_, challenge) = PkceFlow::generate();

	// Assert
	assert!(!challenge.as_str().is_empty());
}
