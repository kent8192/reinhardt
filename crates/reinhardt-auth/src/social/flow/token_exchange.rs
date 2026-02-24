//! OAuth2 authorization code to token exchange
//!
//! Exchanges authorization code for access token.

use std::collections::HashMap;

use super::pkce::CodeVerifier;
use crate::social::core::{OAuth2Client, ProviderConfig, SocialAuthError, TokenResponse};

/// Token exchange flow handler
pub struct TokenExchangeFlow {
	client: OAuth2Client,
	config: ProviderConfig,
}

impl TokenExchangeFlow {
	/// Creates a new token exchange flow
	pub fn new(client: OAuth2Client, config: ProviderConfig) -> Self {
		Self { client, config }
	}

	/// Exchanges authorization code for access token
	///
	/// # Arguments
	///
	/// * `token_endpoint` - The token endpoint URL
	/// * `code` - The authorization code received from the provider
	/// * `code_verifier` - Optional PKCE code verifier
	///
	/// # Returns
	///
	/// The token response containing access_token and optionally refresh_token
	pub async fn exchange(
		&self,
		token_endpoint: &str,
		code: &str,
		code_verifier: Option<&CodeVerifier>,
	) -> Result<TokenResponse, SocialAuthError> {
		let mut params = HashMap::new();
		params.insert("grant_type", "authorization_code");
		params.insert("code", code);
		params.insert("redirect_uri", &self.config.redirect_uri);
		params.insert("client_id", &self.config.client_id);
		params.insert("client_secret", &self.config.client_secret);

		// Add PKCE code_verifier if present
		if let Some(verifier) = code_verifier {
			params.insert("code_verifier", verifier.as_str());
		}

		let response = self
			.client
			.client()
			.post(token_endpoint)
			.header("Accept", "application/json")
			.form(&params)
			.send()
			.await
			.map_err(|e| SocialAuthError::Network(e.to_string()))?;

		if !response.status().is_success() {
			let status = response.status();
			let error_body = response
				.text()
				.await
				.unwrap_or_else(|_| "Unknown error".to_string());

			return Err(SocialAuthError::TokenExchangeError(format!(
				"Token exchange failed ({}): {}",
				status, error_body
			)));
		}

		let token_response: TokenResponse = response
			.json()
			.await
			.map_err(|e| SocialAuthError::TokenExchangeError(e.to_string()))?;

		Ok(token_response)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_token_exchange_flow_creation() {
		let client = OAuth2Client::new();
		let config = ProviderConfig::google(
			"test_client".to_string(),
			"test_secret".to_string(),
			"https://example.com/callback".to_string(),
		);

		let flow = TokenExchangeFlow::new(client, config);
		// Just verify it constructs without panic
		assert!(std::mem::size_of_val(&flow) > 0);
	}

	// Integration tests with mock server would go here
	// For now, we rely on manual testing with real providers
}
