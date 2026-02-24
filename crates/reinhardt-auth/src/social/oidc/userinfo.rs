//! UserInfo endpoint client
//!
//! Fetches user profile information from OIDC UserInfo endpoint.

use crate::social::core::{OAuth2Client, SocialAuthError, StandardClaims};

/// UserInfo endpoint client
pub struct UserInfoClient {
	client: OAuth2Client,
}

impl UserInfoClient {
	/// Creates a new UserInfo client
	pub fn new(client: OAuth2Client) -> Self {
		Self { client }
	}

	/// Fetches user information from the UserInfo endpoint
	///
	/// # Arguments
	///
	/// * `userinfo_endpoint` - The UserInfo endpoint URL
	/// * `access_token` - The OAuth2 access token
	///
	/// # Returns
	///
	/// The user's standard claims
	pub async fn get_user_info(
		&self,
		userinfo_endpoint: &str,
		access_token: &str,
	) -> Result<StandardClaims, SocialAuthError> {
		let response = self
			.client
			.client()
			.get(userinfo_endpoint)
			.header("User-Agent", "reinhardt-auth")
			.bearer_auth(access_token)
			.send()
			.await
			.map_err(|e| SocialAuthError::Network(e.to_string()))?;

		if !response.status().is_success() {
			let status = response.status();
			let error_body = response
				.text()
				.await
				.unwrap_or_else(|_| "Unknown error".to_string());

			return Err(SocialAuthError::UserInfoError(format!(
				"UserInfo request failed ({}): {}",
				status, error_body
			)));
		}

		let claims: StandardClaims = response
			.json()
			.await
			.map_err(|e| SocialAuthError::UserInfoError(e.to_string()))?;

		Ok(claims)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_userinfo_client_creation() {
		let client = OAuth2Client::new();
		let userinfo_client = UserInfoClient::new(client);
		// Just verify it constructs without panic
		assert!(std::mem::size_of_val(&userinfo_client) > 0);
	}

	// Integration tests with mock server would go here
	// For now, we rely on manual testing with real providers
}
