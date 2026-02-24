//! GitHub OAuth2 provider

use crate::social::core::{
	OAuth2Client, OAuthProvider, ProviderConfig, SocialAuthError, StandardClaims, TokenResponse,
};
use crate::social::flow::pkce::{CodeChallenge, CodeVerifier};
use crate::social::flow::{AuthorizationFlow, RefreshFlow, TokenExchangeFlow};
use crate::social::oidc::UserInfoClient;
use async_trait::async_trait;

/// GitHub OAuth2 provider
///
/// Implements OAuth2-only authentication flow using static endpoints
/// configured via `ProviderConfig::github()`.
pub struct GitHubProvider {
	config: ProviderConfig,
	auth_flow: AuthorizationFlow,
	token_exchange: TokenExchangeFlow,
	refresh_flow: RefreshFlow,
	userinfo_client: UserInfoClient,
}

impl GitHubProvider {
	/// Create a new GitHub provider
	///
	/// Validates that the configuration contains OAuth2 endpoints
	/// and constructs all sub-components. No network calls are made.
	pub async fn new(config: ProviderConfig) -> Result<Self, SocialAuthError> {
		if config.oauth2.is_none() {
			return Err(SocialAuthError::InvalidConfiguration(
				"GitHub provider requires OAuth2 configuration".into(),
			));
		}

		let client = OAuth2Client::new();
		let auth_flow = AuthorizationFlow::new(config.clone());
		let token_exchange = TokenExchangeFlow::new(client.clone(), config.clone());
		let refresh_flow = RefreshFlow::new(client.clone(), config.clone());
		let userinfo_client = UserInfoClient::new(client);

		Ok(Self {
			config,
			auth_flow,
			token_exchange,
			refresh_flow,
			userinfo_client,
		})
	}
}

#[async_trait]
impl OAuthProvider for GitHubProvider {
	fn name(&self) -> &str {
		"github"
	}

	fn is_oidc(&self) -> bool {
		false
	}

	async fn authorization_url(
		&self,
		state: &str,
		_nonce: Option<&str>,
		code_challenge: Option<&str>,
	) -> Result<String, SocialAuthError> {
		let oauth2_config =
			self.config.oauth2.as_ref().ok_or_else(|| {
				SocialAuthError::InvalidConfiguration("Missing OAuth2 config".into())
			})?;

		let challenge = code_challenge.map(|c| CodeChallenge::from_raw(c.to_string()));

		self.auth_flow.build_oauth2_url(
			&oauth2_config.authorization_endpoint,
			state,
			challenge.as_ref(),
		)
	}

	async fn exchange_code(
		&self,
		code: &str,
		code_verifier: Option<&str>,
	) -> Result<TokenResponse, SocialAuthError> {
		let oauth2_config =
			self.config.oauth2.as_ref().ok_or_else(|| {
				SocialAuthError::InvalidConfiguration("Missing OAuth2 config".into())
			})?;

		let verifier = code_verifier.map(|v| CodeVerifier::from_raw(v.to_string()));

		self.token_exchange
			.exchange(&oauth2_config.token_endpoint, code, verifier.as_ref())
			.await
	}

	async fn refresh_token(&self, refresh_token: &str) -> Result<TokenResponse, SocialAuthError> {
		let oauth2_config =
			self.config.oauth2.as_ref().ok_or_else(|| {
				SocialAuthError::InvalidConfiguration("Missing OAuth2 config".into())
			})?;

		self.refresh_flow
			.refresh(&oauth2_config.token_endpoint, refresh_token)
			.await
	}

	async fn get_user_info(&self, access_token: &str) -> Result<StandardClaims, SocialAuthError> {
		let oauth2_config =
			self.config.oauth2.as_ref().ok_or_else(|| {
				SocialAuthError::InvalidConfiguration("Missing OAuth2 config".into())
			})?;

		let userinfo_endpoint = oauth2_config.userinfo_endpoint.as_ref().ok_or_else(|| {
			SocialAuthError::InvalidConfiguration("Missing UserInfo endpoint".into())
		})?;

		self.userinfo_client
			.get_user_info(userinfo_endpoint, access_token)
			.await
	}
}
