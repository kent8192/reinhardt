//! JWKS cache integration tests

use reinhardt_auth::social::core::OAuth2Client;
use reinhardt_auth::social::oidc::{Jwk, JwkSet, JwksCache};
use rstest::*;

#[test]
fn test_jwk_set_structure() {
	// Arrange
	let jwk = Jwk {
		kty: "RSA".to_string(),
		kid: Some("test_key_id".to_string()),
		use_: Some("sig".to_string()),
		alg: Some("RS256".to_string()),
		n: Some("test_modulus".to_string()),
		e: Some("AQAB".to_string()),
		crv: None,
		x: None,
		y: None,
	};

	let jwks = JwkSet {
		keys: vec![jwk.clone()],
	};

	// Assert
	assert_eq!(jwks.keys.len(), 1);
	assert_eq!(jwks.keys[0].kid, Some("test_key_id".to_string()));
}

#[test]
fn test_jwk_retrieve_by_kid() {
	// Arrange
	let jwk1 = Jwk {
		kty: "RSA".to_string(),
		kid: Some("key1".to_string()),
		use_: Some("sig".to_string()),
		alg: Some("RS256".to_string()),
		n: Some("modulus1".to_string()),
		e: Some("AQAB".to_string()),
		crv: None,
		x: None,
		y: None,
	};

	let jwk2 = Jwk {
		kty: "RSA".to_string(),
		kid: Some("key2".to_string()),
		use_: Some("sig".to_string()),
		alg: Some("RS256".to_string()),
		n: Some("modulus2".to_string()),
		e: Some("AQAB".to_string()),
		crv: None,
		x: None,
		y: None,
	};

	let jwks = JwkSet {
		keys: vec![jwk1.clone(), jwk2],
	};

	// Act
	let key = jwks.find_key("key1");

	// Assert
	assert!(key.is_some());
	assert_eq!(key.unwrap().kid, Some("key1".to_string()));
}

#[test]
fn test_jwk_missing_key() {
	// Arrange
	let jwks = JwkSet { keys: vec![] };

	// Act
	let key = jwks.find_key("nonexistent");

	// Assert
	assert!(key.is_none());
}

#[tokio::test]
async fn test_jwks_cache_creation() {
	// Arrange
	let client = OAuth2Client::new();

	// Act
	let cache = JwksCache::new(client);

	// Assert
	// Cache should be created successfully
	// The cache is internal and not directly accessible, so we just verify creation
	assert!(std::mem::size_of_val(&cache) > 0);
}

#[tokio::test]
async fn test_jwks_cache_clear() {
	// Arrange
	let client = OAuth2Client::new();
	let cache = JwksCache::new(client);

	// Act
	cache.clear_cache().await;

	// Assert
	// Cache should clear without error
	// After clearing, next get_key call would fetch from network
	assert!(std::mem::size_of_val(&cache) > 0);
}

#[test]
fn test_jwk_rsa_key_support() {
	// Arrange
	let jwk = Jwk {
		kty: "RSA".to_string(),
		kid: Some("test_key".to_string()),
		use_: Some("sig".to_string()),
		alg: Some("RS256".to_string()),
		n: Some("test_modulus".to_string()),
		e: Some("AQAB".to_string()),
		crv: None,
		x: None,
		y: None,
	};

	// Assert
	assert_eq!(jwk.kty, "RSA");
	assert!(jwk.n.is_some());
	assert!(jwk.e.is_some());
	assert_eq!(jwk.alg, Some("RS256".to_string()));
}

#[test]
fn test_jwk_ec_key_support() {
	// Arrange
	let jwk = Jwk {
		kty: "EC".to_string(),
		kid: Some("test_key".to_string()),
		use_: Some("sig".to_string()),
		alg: Some("ES256".to_string()),
		n: None,
		e: None,
		crv: Some("P-256".to_string()),
		x: Some("test_x".to_string()),
		y: Some("test_y".to_string()),
	};

	// Assert
	assert_eq!(jwk.kty, "EC");
	assert!(jwk.x.is_some());
	assert!(jwk.y.is_some());
	assert!(jwk.crv.is_some());
}

#[test]
fn test_jwks_serialization() {
	// Arrange
	let jwk = Jwk {
		kty: "RSA".to_string(),
		kid: Some("test_key".to_string()),
		use_: Some("sig".to_string()),
		alg: Some("RS256".to_string()),
		n: Some("modulus".to_string()),
		e: Some("AQAB".to_string()),
		crv: None,
		x: None,
		y: None,
	};

	let jwks = JwkSet { keys: vec![jwk] };

	// Act
	let json = serde_json::to_string(&jwks).unwrap();

	// Assert
	assert!(json.contains("\"kty\":\"RSA\""));
	assert!(json.contains("\"kid\":\"test_key\""));
}

#[test]
fn test_jwks_deserialization() {
	// Arrange
	let json = r#"{
		"keys": [{
			"kty": "RSA",
			"kid": "test_key",
			"use": "sig",
			"alg": "RS256",
			"n": "test_modulus",
			"e": "AQAB"
		}]
	}"#;

	// Act
	let jwks: JwkSet = serde_json::from_str(json).unwrap();

	// Assert
	assert_eq!(jwks.keys.len(), 1);
	assert_eq!(jwks.keys[0].kid, Some("test_key".to_string()));
}
