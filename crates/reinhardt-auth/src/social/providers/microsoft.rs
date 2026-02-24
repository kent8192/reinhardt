//! Microsoft OIDC provider

use std::sync::Arc;

use crate::social::core::{
	IdToken, OAuth2Client, OAuthProvider, ProviderConfig, SocialAuthError, StandardClaims,
	TokenResponse,
};
use crate::social::flow::pkce::{CodeChallenge, CodeVerifier};
use crate::social::flow::{AuthorizationFlow, RefreshFlow, TokenExchangeFlow};
use crate::social::oidc::id_token::ValidationConfig;
use crate::social::oidc::{
	DiscoveryClient, IdTokenValidator, JwksCache, OIDCDiscovery, UserInfoClient,
};
use async_trait::async_trait;

/// Microsoft OIDC provider
///
/// Implements OIDC authentication flow with dynamic endpoint discovery.
/// Supports tenant-specific discovery URLs configured via
/// `ProviderConfig::microsoft()`.
pub struct MicrosoftProvider {
	config: ProviderConfig,
	auth_flow: AuthorizationFlow,
	token_exchange: TokenExchangeFlow,
	refresh_flow: RefreshFlow,
	userinfo_client: UserInfoClient,
	discovery_client: DiscoveryClient,
	id_token_validator: IdTokenValidator,
}

impl MicrosoftProvider {
	/// Create a new Microsoft provider
	///
	/// Validates that the configuration contains OIDC settings
	/// and constructs all sub-components. No network calls are made.
	pub async fn new(config: ProviderConfig) -> Result<Self, SocialAuthError> {
		let oidc_config = config.oidc.as_ref().ok_or_else(|| {
			SocialAuthError::InvalidConfiguration(
				"Microsoft provider requires OIDC configuration".into(),
			)
		})?;

		let client = OAuth2Client::new();
		let auth_flow = AuthorizationFlow::new(config.clone());
		let token_exchange = TokenExchangeFlow::new(client.clone(), config.clone());
		let refresh_flow = RefreshFlow::new(client.clone(), config.clone());
		let userinfo_client = UserInfoClient::new(client.clone());
		let discovery_client = DiscoveryClient::new(client.clone());

		// Extract issuer from discovery URL for token validation
		let issuer = oidc_config
			.discovery_url
			.trim_end_matches("/.well-known/openid-configuration")
			.to_string();

		let jwks_cache = Arc::new(JwksCache::new(client));
		let validation_config = ValidationConfig::new(issuer, config.client_id.clone());
		let id_token_validator = IdTokenValidator::new(jwks_cache, validation_config);

		Ok(Self {
			config,
			auth_flow,
			token_exchange,
			refresh_flow,
			userinfo_client,
			discovery_client,
			id_token_validator,
		})
	}

	/// Fetches the OIDC discovery document for this provider
	async fn discover(&self) -> Result<OIDCDiscovery, SocialAuthError> {
		let oidc_config =
			self.config.oidc.as_ref().ok_or_else(|| {
				SocialAuthError::InvalidConfiguration("Missing OIDC config".into())
			})?;

		let issuer = oidc_config
			.discovery_url
			.trim_end_matches("/.well-known/openid-configuration")
			.to_string();

		self.discovery_client.discover(&issuer).await
	}
}

#[async_trait]
impl OAuthProvider for MicrosoftProvider {
	fn name(&self) -> &str {
		"microsoft"
	}

	fn is_oidc(&self) -> bool {
		true
	}

	async fn authorization_url(
		&self,
		state: &str,
		nonce: Option<&str>,
		code_challenge: Option<&str>,
	) -> Result<String, SocialAuthError> {
		let discovery = self.discover().await?;
		let challenge = code_challenge.map(|c| CodeChallenge::from_raw(c.to_string()));

		self.auth_flow.build_url(
			&discovery.authorization_endpoint,
			state,
			nonce,
			challenge.as_ref(),
		)
	}

	async fn exchange_code(
		&self,
		code: &str,
		code_verifier: Option<&str>,
	) -> Result<TokenResponse, SocialAuthError> {
		let discovery = self.discover().await?;
		let verifier = code_verifier.map(|v| CodeVerifier::from_raw(v.to_string()));

		self.token_exchange
			.exchange(&discovery.token_endpoint, code, verifier.as_ref())
			.await
	}

	async fn refresh_token(&self, refresh_token: &str) -> Result<TokenResponse, SocialAuthError> {
		let discovery = self.discover().await?;

		self.refresh_flow
			.refresh(&discovery.token_endpoint, refresh_token)
			.await
	}

	async fn validate_id_token(
		&self,
		id_token: &str,
		nonce: Option<&str>,
	) -> Result<IdToken, SocialAuthError> {
		let discovery = self.discover().await?;

		self.id_token_validator
			.validate(id_token, &discovery.jwks_uri, nonce)
			.await
	}

	async fn get_user_info(&self, access_token: &str) -> Result<StandardClaims, SocialAuthError> {
		let discovery = self.discover().await?;

		let userinfo_endpoint = discovery.userinfo_endpoint.as_ref().ok_or_else(|| {
			SocialAuthError::InvalidConfiguration("Missing UserInfo endpoint in discovery".into())
		})?;

		self.userinfo_client
			.get_user_info(userinfo_endpoint, access_token)
			.await
	}
}
