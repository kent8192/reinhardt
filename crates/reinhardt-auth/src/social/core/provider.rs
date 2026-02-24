//! OAuth2/OIDC provider trait

use crate::social::core::{IdToken, SocialAuthError, StandardClaims, TokenResponse};
use async_trait::async_trait;

/// OAuth2/OIDC provider trait
///
/// Unified abstraction for all OAuth2 and OIDC providers.
///
/// # Example
///
/// ```ignore
/// use reinhardt_auth::social::{OAuthProvider, ProviderConfig};
/// use std::sync::Arc;
///
/// #[tokio::main]
/// async fn main() {
///     let config = ProviderConfig::google(
///         "client_id".to_string(),
///         "client_secret".to_string(),
///         "https://example.com/callback".to_string(),
///     );
///
///     let provider: Arc<dyn OAuthProvider> = Arc::new(
///         GoogleProvider::new(config).await.unwrap()
///     );
///
///     // Generate authorization URL
///     let auth_url = provider.authorization_url(
///         "state123",
///         Some("nonce456"),
///         Some("challenge789"),
///     ).await.unwrap();
/// }
/// ```
#[async_trait]
pub trait OAuthProvider: Send + Sync {
	/// Get provider name
	fn name(&self) -> &str;

	/// Check if this is an OIDC provider
	fn is_oidc(&self) -> bool;

	/// Build authorization URL
	///
	/// # Arguments
	///
	/// * `state` - CSRF protection token
	/// * `nonce` - Replay attack prevention token (OIDC only)
	/// * `code_challenge` - PKCE code challenge
	async fn authorization_url(
		&self,
		state: &str,
		nonce: Option<&str>,
		code_challenge: Option<&str>,
	) -> Result<String, SocialAuthError>;

	/// Exchange authorization code for tokens
	///
	/// # Arguments
	///
	/// * `code` - Authorization code from callback
	/// * `code_verifier` - PKCE code verifier
	async fn exchange_code(
		&self,
		code: &str,
		code_verifier: Option<&str>,
	) -> Result<TokenResponse, SocialAuthError>;

	/// Refresh access token
	///
	/// # Arguments
	///
	/// * `refresh_token` - Refresh token
	async fn refresh_token(&self, refresh_token: &str) -> Result<TokenResponse, SocialAuthError>;

	/// Validate ID token (OIDC only)
	///
	/// # Arguments
	///
	/// * `id_token` - JWT ID token
	/// * `nonce` - Expected nonce value
	async fn validate_id_token(
		&self,
		_id_token: &str,
		_nonce: Option<&str>,
	) -> Result<IdToken, SocialAuthError> {
		Err(SocialAuthError::NotSupported(
			"OIDC not supported by this provider".into(),
		))
	}

	/// Get user info
	///
	/// # Arguments
	///
	/// * `access_token` - Access token
	async fn get_user_info(&self, access_token: &str) -> Result<StandardClaims, SocialAuthError>;

	/// Revoke token (optional)
	///
	/// # Arguments
	///
	/// * `token` - Token to revoke
	async fn revoke_token(&self, _token: &str) -> Result<(), SocialAuthError> {
		// Default implementation: no-op
		Ok(())
	}
}
