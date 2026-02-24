//! OAuth2 token types

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// OAuth2 access token and associated metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthToken {
	/// Access token value
	pub access_token: String,

	/// Token type (typically "Bearer")
	pub token_type: String,

	/// Expiration timestamp
	pub expires_at: DateTime<Utc>,

	/// Refresh token (if provided)
	pub refresh_token: Option<String>,

	/// Granted scopes
	pub scopes: Vec<String>,

	/// ID token (OIDC only)
	pub id_token: Option<String>,
}

/// Token response from provider
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenResponse {
	/// Access token
	pub access_token: String,

	/// Token type
	pub token_type: String,

	/// Expires in seconds (optional)
	#[serde(skip_serializing_if = "Option::is_none")]
	pub expires_in: Option<u64>,

	/// Refresh token (optional)
	#[serde(skip_serializing_if = "Option::is_none")]
	pub refresh_token: Option<String>,

	/// Scope (space-separated string, optional)
	#[serde(skip_serializing_if = "Option::is_none")]
	pub scope: Option<String>,

	/// ID token (OIDC only, optional)
	#[serde(skip_serializing_if = "Option::is_none")]
	pub id_token: Option<String>,
}

impl TokenResponse {
	/// Convert to OAuthToken with expiration calculation
	pub fn to_oauth_token(&self) -> OAuthToken {
		let expires_at = if let Some(expires_in) = self.expires_in {
			Utc::now() + chrono::Duration::seconds(expires_in as i64)
		} else {
			// Default to 1 hour if not specified
			Utc::now() + chrono::Duration::hours(1)
		};

		let scopes = self
			.scope
			.as_ref()
			.map(|s| s.split_whitespace().map(String::from).collect())
			.unwrap_or_default();

		OAuthToken {
			access_token: self.access_token.clone(),
			token_type: self.token_type.clone(),
			expires_at,
			refresh_token: self.refresh_token.clone(),
			scopes,
			id_token: self.id_token.clone(),
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_token_response_to_oauth_token() {
		let response = TokenResponse {
			access_token: "access_123".to_string(),
			token_type: "Bearer".to_string(),
			expires_in: Some(3600),
			refresh_token: Some("refresh_456".to_string()),
			scope: Some("openid email profile".to_string()),
			id_token: Some("id_token_789".to_string()),
		};

		let token = response.to_oauth_token();

		assert_eq!(token.access_token, "access_123");
		assert_eq!(token.token_type, "Bearer");
		assert_eq!(token.refresh_token, Some("refresh_456".to_string()));
		assert_eq!(token.scopes, vec!["openid", "email", "profile"]);
		assert_eq!(token.id_token, Some("id_token_789".to_string()));
		assert!(token.expires_at > Utc::now());
	}

	#[test]
	fn test_token_response_default_expiration() {
		let response = TokenResponse {
			access_token: "access_123".to_string(),
			token_type: "Bearer".to_string(),
			expires_in: None,
			refresh_token: None,
			scope: None,
			id_token: None,
		};

		let token = response.to_oauth_token();

		// Should default to 1 hour
		let expected_expiry = Utc::now() + chrono::Duration::hours(1);
		assert!(token.expires_at > Utc::now());
		assert!(token.expires_at <= expected_expiry + chrono::Duration::seconds(10));
	}

	#[test]
	fn test_token_serde() {
		let token = OAuthToken {
			access_token: "test_access".to_string(),
			token_type: "Bearer".to_string(),
			expires_at: Utc::now(),
			refresh_token: Some("test_refresh".to_string()),
			scopes: vec!["read".to_string(), "write".to_string()],
			id_token: None,
		};

		// Serialize
		let json = serde_json::to_string(&token).unwrap();
		assert!(json.contains("test_access"));

		// Deserialize
		let deserialized: OAuthToken = serde_json::from_str(&json).unwrap();
		assert_eq!(deserialized.access_token, token.access_token);
		assert_eq!(deserialized.scopes, token.scopes);
	}
}
