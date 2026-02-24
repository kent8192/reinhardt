//! OAuth2 token refresh flow
//!
//! Refreshes access tokens using refresh_token grant type.

use std::collections::HashMap;

use crate::social::core::{OAuth2Client, ProviderConfig, SocialAuthError, TokenResponse};

/// Token refresh flow handler
pub struct RefreshFlow {
	client: OAuth2Client,
	config: ProviderConfig,
}

impl RefreshFlow {
	/// Creates a new token refresh flow
	pub fn new(client: OAuth2Client, config: ProviderConfig) -> Self {
		Self { client, config }
	}

	/// Refreshes an access token using a refresh token
	///
	/// # Arguments
	///
	/// * `token_endpoint` - The token endpoint URL
	/// * `refresh_token` - The refresh token to use
	///
	/// # Returns
	///
	/// The new token response containing a fresh access_token
	pub async fn refresh(
		&self,
		token_endpoint: &str,
		refresh_token: &str,
	) -> Result<TokenResponse, SocialAuthError> {
		let mut params = HashMap::new();
		params.insert("grant_type", "refresh_token");
		params.insert("refresh_token", refresh_token);
		params.insert("client_id", &self.config.client_id);
		params.insert("client_secret", &self.config.client_secret);

		let response = self
			.client
			.client()
			.post(token_endpoint)
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

			return Err(SocialAuthError::TokenRefreshError(format!(
				"Token refresh failed ({}): {}",
				status, error_body
			)));
		}

		let token_response: TokenResponse = response
			.json()
			.await
			.map_err(|e| SocialAuthError::TokenRefreshError(e.to_string()))?;

		Ok(token_response)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_refresh_flow_creation() {
		let client = OAuth2Client::new();
		let config = ProviderConfig::google(
			"test_client".to_string(),
			"test_secret".to_string(),
			"https://example.com/callback".to_string(),
		);

		let flow = RefreshFlow::new(client, config);
		// Just verify it constructs without panic
		assert!(std::mem::size_of_val(&flow) > 0);
	}

	// Integration tests with mock server would go here
	// For now, we rely on manual testing with real providers
}
