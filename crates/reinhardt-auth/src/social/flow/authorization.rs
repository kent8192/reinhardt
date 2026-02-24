//! OAuth2/OIDC authorization URL generation
//!
//! Constructs authorization endpoint URLs with proper query parameters.

use url::Url;

use super::pkce::CodeChallenge;
use crate::social::core::{ProviderConfig, SocialAuthError};

/// Authorization flow handler
pub struct AuthorizationFlow {
	config: ProviderConfig,
}

impl AuthorizationFlow {
	/// Creates a new authorization flow from configuration
	pub fn new(config: ProviderConfig) -> Self {
		Self { config }
	}

	/// Builds the authorization URL with all required parameters
	///
	/// # Arguments
	///
	/// * `endpoint` - The authorization endpoint URL
	/// * `state` - CSRF protection state parameter
	/// * `nonce` - Optional OIDC nonce parameter
	/// * `code_challenge` - Optional PKCE code challenge
	///
	/// # Returns
	///
	/// The complete authorization URL as a string
	pub fn build_url(
		&self,
		endpoint: &str,
		state: &str,
		nonce: Option<&str>,
		code_challenge: Option<&CodeChallenge>,
	) -> Result<String, SocialAuthError> {
		let mut url = Url::parse(endpoint).map_err(|e| {
			SocialAuthError::InvalidConfiguration(format!("Invalid endpoint URL: {}", e))
		})?;

		{
			let mut query = url.query_pairs_mut();

			// Required OAuth2 parameters
			query.append_pair("response_type", "code");
			query.append_pair("client_id", &self.config.client_id);
			query.append_pair("redirect_uri", &self.config.redirect_uri);
			query.append_pair("state", state);

			// Scopes (space-separated)
			if !self.config.scopes.is_empty() {
				let scope = self.config.scopes.join(" ");
				query.append_pair("scope", &scope);
			}

			// OIDC nonce parameter
			if let Some(nonce_value) = nonce {
				query.append_pair("nonce", nonce_value);
			}

			// PKCE parameters
			if let Some(challenge) = code_challenge {
				query.append_pair("code_challenge", challenge.as_str());
				query.append_pair("code_challenge_method", challenge.method().as_str());
			}
		}

		Ok(url.to_string())
	}

	/// Builds authorization URL for OIDC flow
	///
	/// Automatically includes nonce if configured in provider settings.
	pub fn build_oidc_url(
		&self,
		endpoint: &str,
		state: &str,
		nonce: &str,
		code_challenge: Option<&CodeChallenge>,
	) -> Result<String, SocialAuthError> {
		self.build_url(endpoint, state, Some(nonce), code_challenge)
	}

	/// Builds authorization URL for plain OAuth2 flow
	///
	/// Does not include nonce parameter.
	pub fn build_oauth2_url(
		&self,
		endpoint: &str,
		state: &str,
		code_challenge: Option<&CodeChallenge>,
	) -> Result<String, SocialAuthError> {
		self.build_url(endpoint, state, None, code_challenge)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::social::flow::pkce::PkceFlow;

	#[test]
	fn test_build_basic_oauth2_url() {
		let config = ProviderConfig::github(
			"test_client".to_string(),
			"test_secret".to_string(),
			"https://example.com/callback".to_string(),
		);

		let flow = AuthorizationFlow::new(config);
		let url = flow
			.build_oauth2_url("https://example.com/authorize", "test_state", None)
			.unwrap();

		assert!(url.contains("response_type=code"));
		assert!(url.contains("client_id=test_client"));
		assert!(url.contains("redirect_uri=https%3A%2F%2Fexample.com%2Fcallback"));
		assert!(url.contains("state=test_state"));
		assert!(url.contains("scope=user"));
	}

	#[test]
	fn test_build_oidc_url_with_nonce() {
		let config = ProviderConfig::google(
			"test_client".to_string(),
			"test_secret".to_string(),
			"https://example.com/callback".to_string(),
		);

		let flow = AuthorizationFlow::new(config);
		let url = flow
			.build_oidc_url(
				"https://example.com/authorize",
				"test_state",
				"test_nonce",
				None,
			)
			.unwrap();

		assert!(url.contains("nonce=test_nonce"));
		assert!(url.contains("scope=openid"));
	}

	#[test]
	fn test_build_url_with_pkce() {
		let config = ProviderConfig::google(
			"test_client".to_string(),
			"test_secret".to_string(),
			"https://example.com/callback".to_string(),
		);

		let flow = AuthorizationFlow::new(config);
		let (_, challenge) = PkceFlow::generate();

		let url = flow
			.build_oauth2_url(
				"https://example.com/authorize",
				"test_state",
				Some(&challenge),
			)
			.unwrap();

		assert!(url.contains("code_challenge="));
		assert!(url.contains("code_challenge_method=S256"));
	}

	#[test]
	fn test_build_url_with_all_parameters() {
		let config = ProviderConfig::google(
			"test_client".to_string(),
			"test_secret".to_string(),
			"https://example.com/callback".to_string(),
		);

		let flow = AuthorizationFlow::new(config);
		let (_, challenge) = PkceFlow::generate();

		let url = flow
			.build_url(
				"https://example.com/authorize",
				"test_state",
				Some("test_nonce"),
				Some(&challenge),
			)
			.unwrap();

		assert!(url.contains("response_type=code"));
		assert!(url.contains("client_id=test_client"));
		assert!(url.contains("state=test_state"));
		assert!(url.contains("nonce=test_nonce"));
		assert!(url.contains("code_challenge="));
		assert!(url.contains("code_challenge_method=S256"));
	}

	#[test]
	fn test_scope_joining() {
		let mut config = ProviderConfig::google(
			"test_client".to_string(),
			"test_secret".to_string(),
			"https://example.com/callback".to_string(),
		);

		config.scopes = vec![
			"openid".to_string(),
			"email".to_string(),
			"profile".to_string(),
		];

		let flow = AuthorizationFlow::new(config);
		let url = flow
			.build_oauth2_url("https://example.com/authorize", "test_state", None)
			.unwrap();

		// URL encoding: "openid email profile" -> "openid+email+profile"
		assert!(
			url.contains("scope=openid+email+profile")
				|| url.contains("scope=openid%20email%20profile")
		);
	}

	#[test]
	fn test_invalid_endpoint_url() {
		let config = ProviderConfig::google(
			"test_client".to_string(),
			"test_secret".to_string(),
			"https://example.com/callback".to_string(),
		);

		let flow = AuthorizationFlow::new(config);
		let result = flow.build_oauth2_url("not a valid url", "test_state", None);

		assert!(result.is_err());
	}
}
