//! OIDC claims types

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

/// OIDC ID Token claims
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdToken {
	/// Subject (user ID)
	pub sub: String,

	/// Issuer
	pub iss: String,

	/// Audience (client ID)
	pub aud: String,

	/// Expiration time (Unix timestamp)
	pub exp: i64,

	/// Issued at time (Unix timestamp)
	pub iat: i64,

	/// Nonce (replay attack prevention)
	#[serde(skip_serializing_if = "Option::is_none")]
	pub nonce: Option<String>,

	/// Email address
	#[serde(skip_serializing_if = "Option::is_none")]
	pub email: Option<String>,

	/// Email verified flag
	#[serde(skip_serializing_if = "Option::is_none")]
	pub email_verified: Option<bool>,

	/// Full name
	#[serde(skip_serializing_if = "Option::is_none")]
	pub name: Option<String>,

	/// Given name (first name)
	#[serde(skip_serializing_if = "Option::is_none")]
	pub given_name: Option<String>,

	/// Family name (last name)
	#[serde(skip_serializing_if = "Option::is_none")]
	pub family_name: Option<String>,

	/// Profile picture URL
	#[serde(skip_serializing_if = "Option::is_none")]
	pub picture: Option<String>,

	/// Locale
	#[serde(skip_serializing_if = "Option::is_none")]
	pub locale: Option<String>,

	/// Additional claims (provider-specific)
	#[serde(flatten)]
	pub additional_claims: HashMap<String, Value>,
}

/// Standard OIDC claims (from ID token or UserInfo endpoint)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StandardClaims {
	/// Subject (user ID)
	pub sub: String,

	/// Email address
	#[serde(skip_serializing_if = "Option::is_none")]
	pub email: Option<String>,

	/// Email verified flag
	#[serde(skip_serializing_if = "Option::is_none")]
	pub email_verified: Option<bool>,

	/// Full name
	#[serde(skip_serializing_if = "Option::is_none")]
	pub name: Option<String>,

	/// Given name (first name)
	#[serde(skip_serializing_if = "Option::is_none")]
	pub given_name: Option<String>,

	/// Family name (last name)
	#[serde(skip_serializing_if = "Option::is_none")]
	pub family_name: Option<String>,

	/// Profile picture URL
	#[serde(skip_serializing_if = "Option::is_none")]
	pub picture: Option<String>,

	/// Locale
	#[serde(skip_serializing_if = "Option::is_none")]
	pub locale: Option<String>,

	/// Additional claims (provider-specific)
	#[serde(flatten)]
	pub additional_claims: HashMap<String, Value>,
}

impl From<IdToken> for StandardClaims {
	fn from(id_token: IdToken) -> Self {
		StandardClaims {
			sub: id_token.sub,
			email: id_token.email,
			email_verified: id_token.email_verified,
			name: id_token.name,
			given_name: id_token.given_name,
			family_name: id_token.family_name,
			picture: id_token.picture,
			locale: id_token.locale,
			additional_claims: id_token.additional_claims,
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_id_token_serde() {
		let id_token = IdToken {
			sub: "user123".to_string(),
			iss: "https://accounts.google.com".to_string(),
			aud: "client_id".to_string(),
			exp: 1234567890,
			iat: 1234567800,
			nonce: Some("nonce123".to_string()),
			email: Some("user@example.com".to_string()),
			email_verified: Some(true),
			name: Some("Test User".to_string()),
			given_name: Some("Test".to_string()),
			family_name: Some("User".to_string()),
			picture: Some("https://example.com/photo.jpg".to_string()),
			locale: Some("en-US".to_string()),
			additional_claims: HashMap::new(),
		};

		// Serialize
		let json = serde_json::to_string(&id_token).unwrap();
		assert!(json.contains("user123"));

		// Deserialize
		let deserialized: IdToken = serde_json::from_str(&json).unwrap();
		assert_eq!(deserialized.sub, "user123");
		assert_eq!(deserialized.email, Some("user@example.com".to_string()));
	}

	#[test]
	fn test_standard_claims_from_id_token() {
		let id_token = IdToken {
			sub: "user456".to_string(),
			iss: "https://example.com".to_string(),
			aud: "client_id".to_string(),
			exp: 1234567890,
			iat: 1234567800,
			nonce: None,
			email: Some("test@example.com".to_string()),
			email_verified: Some(true),
			name: Some("Test User".to_string()),
			given_name: None,
			family_name: None,
			picture: None,
			locale: None,
			additional_claims: HashMap::new(),
		};

		let claims: StandardClaims = id_token.into();

		assert_eq!(claims.sub, "user456");
		assert_eq!(claims.email, Some("test@example.com".to_string()));
		assert_eq!(claims.email_verified, Some(true));
		assert_eq!(claims.name, Some("Test User".to_string()));
	}

	#[test]
	fn test_additional_claims() {
		let json = r#"{
			"sub": "user789",
			"email": "user@example.com",
			"custom_field": "custom_value",
			"another_field": 123
		}"#;

		let claims: StandardClaims = serde_json::from_str(json).unwrap();

		assert_eq!(claims.sub, "user789");
		assert_eq!(claims.email, Some("user@example.com".to_string()));
		assert!(claims.additional_claims.contains_key("custom_field"));
		assert!(claims.additional_claims.contains_key("another_field"));
	}
}
