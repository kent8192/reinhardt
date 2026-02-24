//! Apple OIDC provider

use std::sync::Arc;

use crate::social::core::{
	IdToken, OAuth2Client, OAuthProvider, ProviderConfig, SocialAuthError, StandardClaims,
	TokenResponse,
};
use crate::social::flow::pkce::{CodeChallenge, CodeVerifier};
use crate::social::flow::{AuthorizationFlow, RefreshFlow, TokenExchangeFlow};
use crate::social::oidc::id_token::ValidationConfig;
use crate::social::oidc::{DiscoveryClient, IdTokenValidator, JwksCache, OIDCDiscovery};
use async_trait::async_trait;

/// Apple OIDC provider
///
/// Implements OIDC authentication flow with dynamic endpoint discovery.
/// Apple provides user information exclusively through the ID token;
/// the UserInfo endpoint is not supported.
pub struct AppleProvider {
	config: ProviderConfig,
	auth_flow: AuthorizationFlow,
	token_exchange: TokenExchangeFlow,
	refresh_flow: RefreshFlow,
	discovery_client: DiscoveryClient,
	id_token_validator: IdTokenValidator,
}

impl AppleProvider {
	/// Create a new Apple provider
	///
	/// Validates that the configuration contains OIDC settings and a non-empty
	/// client_secret, then constructs all sub-components. No network calls are made.
	///
	/// # Note
	///
	/// Apple Sign In requires a dynamically generated JWT as the client_secret.
	/// The caller is responsible for generating this JWT using the team_id, key_id,
	/// and private key, then passing it as client_secret in the `ProviderConfig`.
	pub async fn new(config: ProviderConfig) -> Result<Self, SocialAuthError> {
		if config.oidc.is_none() {
			return Err(SocialAuthError::InvalidConfiguration(
				"Apple provider requires OIDC configuration".into(),
			));
		}

		if config.client_secret.is_empty() {
			return Err(SocialAuthError::InvalidConfiguration(
				"Apple provider requires a client_secret (JWT). Generate it using team_id, key_id, and private key before creating the provider".into(),
			));
		}

		let client = OAuth2Client::new();
		let auth_flow = AuthorizationFlow::new(config.clone());
		let token_exchange = TokenExchangeFlow::new(client.clone(), config.clone());
		let refresh_flow = RefreshFlow::new(client.clone(), config.clone());
		let discovery_client = DiscoveryClient::new(client.clone());

		let jwks_cache = Arc::new(JwksCache::new(client));
		let validation_config = ValidationConfig::new(
			"https://appleid.apple.com".to_string(),
			config.client_id.clone(),
		);
		let id_token_validator = IdTokenValidator::new(jwks_cache, validation_config);

		Ok(Self {
			config,
			auth_flow,
			token_exchange,
			refresh_flow,
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
impl OAuthProvider for AppleProvider {
	fn name(&self) -> &str {
		"apple"
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

	async fn get_user_info(&self, _access_token: &str) -> Result<StandardClaims, SocialAuthError> {
		// Apple does not provide a UserInfo endpoint.
		// All user information is returned in the ID token.
		Err(SocialAuthError::NotSupported(
			"Apple does not support UserInfo endpoint; use ID token claims instead".into(),
		))
	}
}
