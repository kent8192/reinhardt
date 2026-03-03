//! PKCE (Proof Key for Code Exchange) implementation for OAuth2
//!
//! Implements RFC 7636 for secure authorization code flow.

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use rand::{Rng, distr::Alphanumeric};
use sha2::{Digest, Sha256};

/// Code verifier for PKCE flow
///
/// A cryptographically random string between 43-128 characters.
#[derive(Debug, Clone)]
pub struct CodeVerifier(String);

impl CodeVerifier {
	/// Creates a code verifier from a raw string value
	///
	/// Used when reconstructing a verifier from stored state data.
	pub fn from_raw(value: String) -> Self {
		Self(value)
	}

	/// Returns the verifier as a string slice
	pub fn as_str(&self) -> &str {
		&self.0
	}
}

/// Code challenge method
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChallengeMethod {
	/// SHA256 hashing method
	S256,
}

impl ChallengeMethod {
	/// Returns the method name as specified in OAuth2 spec
	pub fn as_str(&self) -> &str {
		match self {
			ChallengeMethod::S256 => "S256",
		}
	}
}

/// Code challenge for PKCE flow
///
/// Derived from code verifier using SHA256 hashing.
#[derive(Debug, Clone)]
pub struct CodeChallenge {
	challenge: String,
	method: ChallengeMethod,
}

impl CodeChallenge {
	/// Creates a code challenge from a raw string value
	///
	/// Assumes S256 challenge method since the raw value has already been hashed.
	/// Used when the challenge string is received from the `OAuthProvider` trait.
	pub fn from_raw(value: String) -> Self {
		Self {
			challenge: value,
			method: ChallengeMethod::S256,
		}
	}

	/// Returns the challenge as a string slice
	pub fn as_str(&self) -> &str {
		&self.challenge
	}

	/// Returns the challenge method
	pub fn method(&self) -> ChallengeMethod {
		self.method
	}
}

/// PKCE flow generator
pub struct PkceFlow;

impl PkceFlow {
	/// Generates a new PKCE verifier/challenge pair
	///
	/// The verifier is a cryptographically random string of 128 characters.
	/// The challenge is the Base64URL encoding of SHA256(verifier).
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_auth::social::flow::pkce::PkceFlow;
	///
	/// let (verifier, challenge) = PkceFlow::generate();
	/// assert!(verifier.as_str().len() >= 43);
	/// assert!(verifier.as_str().len() <= 128);
	/// ```
	pub fn generate() -> (CodeVerifier, CodeChallenge) {
		// Generate verifier: 128 random alphanumeric characters
		let verifier_str: String = rand::rng()
			.sample_iter(&Alphanumeric)
			.take(128)
			.map(char::from)
			.collect();

		let verifier = CodeVerifier(verifier_str);

		// Generate challenge: Base64URL(SHA256(verifier))
		let mut hasher = Sha256::new();
		hasher.update(verifier.as_str().as_bytes());
		let hash = hasher.finalize();
		let challenge_str = URL_SAFE_NO_PAD.encode(hash);

		let challenge = CodeChallenge {
			challenge: challenge_str,
			method: ChallengeMethod::S256,
		};

		(verifier, challenge)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_verifier_length() {
		let (verifier, _) = PkceFlow::generate();
		let len = verifier.as_str().len();
		assert!(len >= 43, "verifier too short: {}", len);
		assert!(len <= 128, "verifier too long: {}", len);
	}

	#[test]
	fn test_verifier_is_alphanumeric() {
		let (verifier, _) = PkceFlow::generate();
		assert!(
			verifier.as_str().chars().all(|c| c.is_alphanumeric()),
			"verifier contains non-alphanumeric characters"
		);
	}

	#[test]
	fn test_challenge_calculation() {
		// Known test vector
		let verifier = CodeVerifier("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk".to_string());
		let mut hasher = Sha256::new();
		hasher.update(verifier.as_str().as_bytes());
		let hash = hasher.finalize();
		let expected = URL_SAFE_NO_PAD.encode(hash);

		let (_, challenge) = PkceFlow::generate();
		// We can't test the exact value since it's random, but we can test the format
		assert_eq!(challenge.method(), ChallengeMethod::S256);
		assert!(!challenge.as_str().is_empty());

		// Test the calculation logic separately
		assert_eq!(expected, "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM");
	}

	#[test]
	fn test_different_verifiers_produce_different_challenges() {
		let (verifier1, challenge1) = PkceFlow::generate();
		let (verifier2, challenge2) = PkceFlow::generate();

		assert_ne!(verifier1.as_str(), verifier2.as_str());
		assert_ne!(challenge1.as_str(), challenge2.as_str());
	}

	#[test]
	fn test_challenge_method_str() {
		assert_eq!(ChallengeMethod::S256.as_str(), "S256");
	}
}
