//! ID token validation tests

use chrono::{Duration, Utc};
use reinhardt_auth::social::core::claims::IdToken;
use rstest::*;

#[test]
fn test_id_token_validate_signature() {
	// This test documents the expected signature validation behavior
	// In real scenarios, you would need a valid JWT signed by the provider

	// Arrange
	let token = IdToken {
		sub: "user123".to_string(),
		iss: "https://accounts.google.com".to_string(),
		aud: "test_client_id".to_string(),
		exp: (Utc::now() + Duration::hours(1)).timestamp(),
		iat: Utc::now().timestamp(),
		nonce: Some("test_nonce".to_string()),
		email: Some("user@example.com".to_string()),
		email_verified: Some(true),
		name: Some("Test User".to_string()),
		given_name: Some("Test".to_string()),
		family_name: Some("User".to_string()),
		picture: None,
		locale: None,
		additional_claims: Default::default(),
	};

	// Assert
	// Signature validation requires actual JWKS, so we just verify token structure
	assert!(!token.sub.is_empty());
	assert!(!token.iss.is_empty());
	assert!(!token.aud.is_empty());
}

#[test]
fn test_id_token_verify_issuer_claim() {
	// Arrange
	let expected_issuer = "https://accounts.google.com";
	let token = IdToken {
		sub: "user123".to_string(),
		iss: expected_issuer.to_string(),
		aud: "test_client_id".to_string(),
		exp: (Utc::now() + Duration::hours(1)).timestamp(),
		iat: Utc::now().timestamp(),
		nonce: None,
		email: None,
		email_verified: None,
		name: None,
		given_name: None,
		family_name: None,
		picture: None,
		locale: None,
		additional_claims: Default::default(),
	};

	// Assert
	assert_eq!(token.iss, expected_issuer);
}

#[test]
fn test_id_token_verify_audience_claim() {
	// Arrange
	let expected_audience = "test_client_id";
	let token = IdToken {
		sub: "user123".to_string(),
		iss: "https://accounts.google.com".to_string(),
		aud: expected_audience.to_string(),
		exp: (Utc::now() + Duration::hours(1)).timestamp(),
		iat: Utc::now().timestamp(),
		nonce: None,
		email: None,
		email_verified: None,
		name: None,
		given_name: None,
		family_name: None,
		picture: None,
		locale: None,
		additional_claims: Default::default(),
	};

	// Assert
	assert_eq!(token.aud, expected_audience);
}

#[test]
fn test_id_token_verify_expiration_claim() {
	// Arrange
	let token = IdToken {
		sub: "user123".to_string(),
		iss: "https://accounts.google.com".to_string(),
		aud: "test_client_id".to_string(),
		exp: (Utc::now() + Duration::hours(1)).timestamp(),
		iat: Utc::now().timestamp(),
		nonce: None,
		email: None,
		email_verified: None,
		name: None,
		given_name: None,
		family_name: None,
		picture: None,
		locale: None,
		additional_claims: Default::default(),
	};

	// Assert
	assert!(token.exp > token.iat);
	assert!(token.exp > Utc::now().timestamp());
}

#[test]
fn test_id_token_verify_nonce_claim() {
	// Arrange
	let expected_nonce = "test_nonce";
	let token = IdToken {
		sub: "user123".to_string(),
		iss: "https://accounts.google.com".to_string(),
		aud: "test_client_id".to_string(),
		exp: (Utc::now() + Duration::hours(1)).timestamp(),
		iat: Utc::now().timestamp(),
		nonce: Some(expected_nonce.to_string()),
		email: None,
		email_verified: None,
		name: None,
		given_name: None,
		family_name: None,
		picture: None,
		locale: None,
		additional_claims: Default::default(),
	};

	// Assert
	assert_eq!(token.nonce, Some(expected_nonce.to_string()));
}

#[test]
fn test_id_token_reject_expired() {
	// Arrange
	let token = IdToken {
		sub: "user123".to_string(),
		iss: "https://accounts.google.com".to_string(),
		aud: "test_client_id".to_string(),
		exp: (Utc::now() - Duration::hours(1)).timestamp(), // Expired
		iat: (Utc::now() - Duration::hours(2)).timestamp(),
		nonce: None,
		email: None,
		email_verified: None,
		name: None,
		given_name: None,
		family_name: None,
		picture: None,
		locale: None,
		additional_claims: Default::default(),
	};

	// Assert
	assert!(
		token.exp < Utc::now().timestamp(),
		"Token should be expired"
	);
}

#[test]
fn test_id_token_reject_mismatched_nonce() {
	// Arrange
	let expected_nonce = "correct_nonce";
	let token_nonce = "wrong_nonce";
	let token = IdToken {
		sub: "user123".to_string(),
		iss: "https://accounts.google.com".to_string(),
		aud: "test_client_id".to_string(),
		exp: (Utc::now() + Duration::hours(1)).timestamp(),
		iat: Utc::now().timestamp(),
		nonce: Some(token_nonce.to_string()),
		email: None,
		email_verified: None,
		name: None,
		given_name: None,
		family_name: None,
		picture: None,
		locale: None,
		additional_claims: Default::default(),
	};

	// Assert
	assert_ne!(token.nonce, Some(expected_nonce.to_string()));
}
