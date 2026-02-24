//! Custom assertions for social authentication tests

use chrono::Utc;
use reinhardt_auth::social::core::{
	claims::{IdToken, StandardClaims},
	token::TokenResponse,
};
use reinhardt_auth::social::flow::StateData;

/// Asserts that a token response contains expected fields
pub fn assert_token_response_valid(response: &TokenResponse, expected_scopes: &[&str]) {
	assert!(
		!response.access_token.is_empty(),
		"access_token must not be empty"
	);
	assert_eq!(response.token_type, "Bearer", "token_type must be Bearer");

	if let Some(scope) = &response.scope {
		let scopes: Vec<&str> = scope.split_whitespace().collect();
		for expected in expected_scopes {
			assert!(
				scopes.contains(expected),
				"Expected scope '{}' not found in {:?}",
				expected,
				scopes
			);
		}
	}
}

/// Asserts that an ID token is valid
pub fn assert_id_token_valid(token: &IdToken, expected_issuer: &str, expected_audience: &str) {
	assert_eq!(token.iss, expected_issuer, "Issuer mismatch");
	assert_eq!(token.aud, expected_audience, "Audience mismatch");
	assert!(token.exp > token.iat, "Expiration must be after issued-at");

	let now = Utc::now().timestamp();
	assert!(token.exp > now, "Token must not be expired");
}

/// Asserts that PKCE challenge is correctly calculated
pub fn assert_pkce_challenge_valid(verifier: &str, challenge: &str) {
	use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
	use sha2::{Digest, Sha256};

	let mut hasher = Sha256::new();
	hasher.update(verifier.as_bytes());
	let hash = hasher.finalize();
	let expected = URL_SAFE_NO_PAD.encode(hash);

	assert_eq!(
		challenge, expected,
		"PKCE challenge mismatch (expected: {}, got: {})",
		expected, challenge
	);
}

/// Asserts that state data is not expired
pub fn assert_state_not_expired(data: &StateData) {
	assert!(
		!data.is_expired(),
		"State data must not be expired (expires_at: {:?}, now: {:?})",
		data.expires_at,
		Utc::now()
	);
}

/// Asserts that claims contain expected email
pub fn assert_claims_has_email(claims: &StandardClaims, email: &str) {
	assert_eq!(
		claims.email.as_ref().unwrap(),
		email,
		"Email mismatch (expected: {}, got: {:?})",
		email,
		claims.email
	);
}

/// Asserts that authorization URL contains required parameters
pub fn assert_authorization_url_valid(url: &str, expected_params: &[(&str, &str)]) {
	let parsed: url::Url = url.parse().expect("Invalid URL");
	let query_pairs: std::collections::HashMap<_, _> = parsed.query_pairs().into_iter().collect();

	for (key, value) in expected_params {
		assert_eq!(
			query_pairs.get(*key).map(|s| s.as_ref()),
			Some(*value),
			"Parameter '{}' mismatch (expected: {}, got: {:?})",
			key,
			value,
			query_pairs.get(*key)
		);
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_assert_token_response_valid() {
		let response = TokenResponse {
			access_token: "test_token".into(),
			token_type: "Bearer".into(),
			expires_in: Some(3600),
			refresh_token: None,
			scope: Some("openid email profile".into()),
			id_token: None,
		};

		assert_token_response_valid(&response, &["openid", "email", "profile"]);
	}

	#[test]
	fn test_assert_pkce_challenge_valid() {
		let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
		let challenge = "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM";

		assert_pkce_challenge_valid(verifier, challenge);
	}

	#[test]
	fn test_assert_authorization_url_valid() {
		let url = "https://accounts.google.com/o/oauth2/v2/auth?client_id=test&response_type=code&state=test_state";

		assert_authorization_url_valid(
			url,
			&[
				("client_id", "test"),
				("response_type", "code"),
				("state", "test_state"),
			],
		);
	}
}
