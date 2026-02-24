//! Claims parsing tests

use reinhardt_auth::social::core::claims::{IdToken, StandardClaims};
use rstest::*;
use serde_json::json;
use std::collections::HashMap;

#[test]
fn test_id_token_serialize() {
	// Arrange
	let token = IdToken {
		sub: "user123".to_string(),
		iss: "https://accounts.google.com".to_string(),
		aud: "client_id".to_string(),
		exp: 1735636800,
		iat: 1735633200,
		nonce: Some("nonce123".to_string()),
		email: Some("user@example.com".to_string()),
		email_verified: Some(true),
		name: Some("Test User".to_string()),
		given_name: Some("Test".to_string()),
		family_name: Some("User".to_string()),
		picture: Some("https://example.com/photo.jpg".to_string()),
		locale: Some("en".to_string()),
		additional_claims: HashMap::new(),
	};

	// Act
	let json = serde_json::to_string(&token).unwrap();

	// Assert
	assert!(json.contains("\"sub\":\"user123\""));
	assert!(json.contains("\"iss\":\"https://accounts.google.com\""));
}

#[test]
fn test_id_token_deserialize() {
	// Arrange
	let json = json!({
		"sub": "user123",
		"iss": "https://accounts.google.com",
		"aud": "client_id",
		"exp": 1735636800,
		"iat": 1735633200,
		"nonce": "nonce123",
		"email": "user@example.com",
		"email_verified": true,
		"name": "Test User",
		"given_name": "Test",
		"family_name": "User"
	});

	// Act
	let token: IdToken = serde_json::from_value(json).unwrap();

	// Assert
	assert_eq!(token.sub, "user123");
	assert_eq!(token.iss, "https://accounts.google.com");
	assert_eq!(token.aud, "client_id");
	assert_eq!(token.email, Some("user@example.com".to_string()));
	assert_eq!(token.email_verified, Some(true));
}

#[test]
fn test_id_token_to_standard_claims() {
	// Arrange
	let id_token = IdToken {
		sub: "user123".to_string(),
		iss: "https://accounts.google.com".to_string(),
		aud: "client_id".to_string(),
		exp: 1735636800,
		iat: 1735633200,
		nonce: Some("nonce123".to_string()),
		email: Some("user@example.com".to_string()),
		email_verified: Some(true),
		name: Some("Test User".to_string()),
		given_name: Some("Test".to_string()),
		family_name: Some("User".to_string()),
		picture: Some("https://example.com/photo.jpg".to_string()),
		locale: Some("en".to_string()),
		additional_claims: HashMap::new(),
	};

	// Act - Convert to StandardClaims
	let standard = StandardClaims {
		sub: id_token.sub.clone(),
		email: id_token.email.clone(),
		email_verified: id_token.email_verified,
		name: id_token.name.clone(),
		given_name: id_token.given_name.clone(),
		family_name: id_token.family_name.clone(),
		picture: id_token.picture.clone(),
		locale: id_token.locale.clone(),
		additional_claims: HashMap::new(),
	};

	// Assert
	assert_eq!(standard.sub, "user123");
	assert_eq!(standard.email, Some("user@example.com".to_string()));
	assert_eq!(standard.name, Some("Test User".to_string()));
}

#[test]
fn test_standard_claims_optional_fields() {
	// Arrange
	let claims = StandardClaims {
		sub: "user123".to_string(),
		email: None,
		email_verified: None,
		name: None,
		given_name: None,
		family_name: None,
		picture: None,
		locale: None,
		additional_claims: HashMap::new(),
	};

	// Act
	let json = serde_json::to_string(&claims).unwrap();

	// Assert
	assert!(json.contains("\"sub\":\"user123\""));
	// Optional fields should not be in JSON when None
	assert!(!json.contains("\"email\""));
	assert!(!json.contains("\"name\""));
}

#[test]
fn test_standard_claims_with_additional_claims() {
	// Arrange
	let mut additional = HashMap::new();
	additional.insert(
		"custom_field".to_string(),
		serde_json::json!("custom_value"),
	);

	let claims = StandardClaims {
		sub: "user123".to_string(),
		email: Some("user@example.com".to_string()),
		email_verified: Some(true),
		name: Some("Test User".to_string()),
		given_name: Some("Test".to_string()),
		family_name: Some("User".to_string()),
		picture: None,
		locale: None,
		additional_claims: additional,
	};

	// Act
	let json = serde_json::to_string(&claims).unwrap();

	// Assert
	assert!(json.contains("\"custom_field\":\"custom_value\""));
}

#[test]
fn test_id_token_validation_timestamps() {
	// Arrange
	let now = chrono::Utc::now().timestamp();

	let token = IdToken {
		sub: "user123".to_string(),
		iss: "https://accounts.google.com".to_string(),
		aud: "client_id".to_string(),
		exp: now + 3600, // Expires in 1 hour
		iat: now,        // Issued now
		nonce: None,
		email: None,
		email_verified: None,
		name: None,
		given_name: None,
		family_name: None,
		picture: None,
		locale: None,
		additional_claims: HashMap::new(),
	};

	// Assert
	assert!(token.exp > token.iat, "Expiration must be after issued-at");
	assert!(token.exp > now, "Token must not be expired");
}

#[test]
fn test_standard_claims_parse_from_response() {
	// Arrange
	let json = json!({
		"sub": "user123",
		"email": "user@example.com",
		"email_verified": true,
		"name": "Test User",
		"picture": "https://example.com/photo.jpg"
	});

	// Act
	let claims: StandardClaims = serde_json::from_value(json).unwrap();

	// Assert
	assert_eq!(claims.sub, "user123");
	assert_eq!(claims.email, Some("user@example.com".to_string()));
	assert_eq!(claims.email_verified, Some(true));
	assert_eq!(claims.name, Some("Test User".to_string()));
}
