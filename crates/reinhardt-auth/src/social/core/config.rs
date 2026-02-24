//! Provider configuration types

use serde::{Deserialize, Serialize};

/// Provider configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
	/// Provider name (e.g., "google", "github")
	pub name: String,

	/// OAuth2 client ID
	pub client_id: String,

	/// OAuth2 client secret
	pub client_secret: String,

	/// Redirect URI
	pub redirect_uri: String,

	/// Requested scopes
	pub scopes: Vec<String>,

	/// OIDC-specific configuration
	#[serde(skip_serializing_if = "Option::is_none")]
	pub oidc: Option<OIDCConfig>,

	/// OAuth2-specific configuration
	#[serde(skip_serializing_if = "Option::is_none")]
	pub oauth2: Option<OAuth2Config>,
}

/// OIDC-specific configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OIDCConfig {
	/// Discovery URL
	pub discovery_url: String,

	/// Use nonce for replay attack prevention
	#[serde(default = "default_use_nonce")]
	pub use_nonce: bool,
}

/// OAuth2-specific configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuth2Config {
	/// Authorization endpoint URL
	pub authorization_endpoint: String,

	/// Token endpoint URL
	pub token_endpoint: String,

	/// UserInfo endpoint URL (optional)
	#[serde(skip_serializing_if = "Option::is_none")]
	pub userinfo_endpoint: Option<String>,
}

fn default_use_nonce() -> bool {
	true
}

impl ProviderConfig {
	/// Create a Google OIDC provider configuration
	pub fn google(client_id: String, client_secret: String, redirect_uri: String) -> Self {
		Self {
			name: "google".to_string(),
			client_id,
			client_secret,
			redirect_uri,
			scopes: vec![
				"openid".to_string(),
				"email".to_string(),
				"profile".to_string(),
			],
			oidc: Some(OIDCConfig {
				discovery_url: "https://accounts.google.com/.well-known/openid-configuration"
					.to_string(),
				use_nonce: true,
			}),
			oauth2: None,
		}
	}

	/// Create a GitHub OAuth2 provider configuration
	pub fn github(client_id: String, client_secret: String, redirect_uri: String) -> Self {
		Self {
			name: "github".to_string(),
			client_id,
			client_secret,
			redirect_uri,
			scopes: vec!["user".to_string(), "user:email".to_string()],
			oidc: None,
			oauth2: Some(OAuth2Config {
				authorization_endpoint: "https://github.com/login/oauth/authorize".to_string(),
				token_endpoint: "https://github.com/login/oauth/access_token".to_string(),
				userinfo_endpoint: Some("https://api.github.com/user".to_string()),
			}),
		}
	}

	/// Create an Apple OIDC provider configuration
	///
	/// The `client_secret` must be a pre-generated JWT signed with ES256.
	/// Apple requires this JWT to be generated using your team_id, key_id,
	/// and private key. See Apple's documentation for details.
	pub fn apple(client_id: String, client_secret: String, redirect_uri: String) -> Self {
		Self {
			name: "apple".to_string(),
			client_id,
			client_secret,
			redirect_uri,
			scopes: vec![
				"openid".to_string(),
				"email".to_string(),
				"name".to_string(),
			],
			oidc: Some(OIDCConfig {
				discovery_url: "https://appleid.apple.com/.well-known/openid-configuration"
					.to_string(),
				use_nonce: true,
			}),
			oauth2: None,
		}
	}

	/// Create a Microsoft OIDC provider configuration
	pub fn microsoft(
		client_id: String,
		client_secret: String,
		redirect_uri: String,
		tenant: String,
	) -> Self {
		let discovery_url = format!(
			"https://login.microsoftonline.com/{}/v2.0/.well-known/openid-configuration",
			tenant
		);

		Self {
			name: "microsoft".to_string(),
			client_id,
			client_secret,
			redirect_uri,
			scopes: vec![
				"openid".to_string(),
				"email".to_string(),
				"profile".to_string(),
			],
			oidc: Some(OIDCConfig {
				discovery_url,
				use_nonce: true,
			}),
			oauth2: None,
		}
	}

	/// Check if this is an OIDC provider
	pub fn is_oidc(&self) -> bool {
		self.oidc.is_some()
	}

	/// Check if this is an OAuth2-only provider
	pub fn is_oauth2_only(&self) -> bool {
		self.oauth2.is_some() && self.oidc.is_none()
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_google_config() {
		let config = ProviderConfig::google(
			"client_id".to_string(),
			"client_secret".to_string(),
			"https://example.com/callback".to_string(),
		);

		assert_eq!(config.name, "google");
		assert!(config.is_oidc());
		assert!(!config.is_oauth2_only());
		assert_eq!(config.scopes, vec!["openid", "email", "profile"]);
	}

	#[test]
	fn test_github_config() {
		let config = ProviderConfig::github(
			"client_id".to_string(),
			"client_secret".to_string(),
			"https://example.com/callback".to_string(),
		);

		assert_eq!(config.name, "github");
		assert!(!config.is_oidc());
		assert!(config.is_oauth2_only());
		assert_eq!(config.scopes, vec!["user", "user:email"]);
	}

	#[test]
	fn test_apple_config() {
		let config = ProviderConfig::apple(
			"client_id".to_string(),
			"test_client_secret_jwt".to_string(),
			"https://example.com/callback".to_string(),
		);

		assert_eq!(config.name, "apple");
		assert!(config.is_oidc());
		assert!(!config.is_oauth2_only());
		assert_eq!(config.client_secret, "test_client_secret_jwt");
	}

	#[test]
	fn test_microsoft_config() {
		let config = ProviderConfig::microsoft(
			"client_id".to_string(),
			"client_secret".to_string(),
			"https://example.com/callback".to_string(),
			"common".to_string(),
		);

		assert_eq!(config.name, "microsoft");
		assert!(config.is_oidc());
		assert!(!config.is_oauth2_only());
		assert!(config.oidc.unwrap().discovery_url.contains("common"));
	}

	#[test]
	fn test_config_serde() {
		let config = ProviderConfig::google(
			"test_client".to_string(),
			"test_secret".to_string(),
			"https://test.com/callback".to_string(),
		);

		// Serialize
		let json = serde_json::to_string(&config).unwrap();
		assert!(json.contains("google"));

		// Deserialize
		let deserialized: ProviderConfig = serde_json::from_str(&json).unwrap();
		assert_eq!(deserialized.name, "google");
		assert_eq!(deserialized.client_id, "test_client");
	}
}
