//! Social authentication backend
//!
//! Orchestrates OAuth2/OIDC flows and integrates with reinhardt-auth.

use std::collections::HashMap;
use std::sync::Arc;

use crate::social::core::{OAuthProvider, SocialAuthError, StandardClaims, TokenResponse};
use crate::social::flow::{ContextualStateData, InMemoryStateStore, StateData, StateStore};

/// Result of beginning an authorization flow
pub struct AuthorizationResult {
	/// The URL to redirect the user to
	pub authorization_url: String,
	/// The state parameter for CSRF verification
	pub state: String,
	/// The nonce parameter for replay attack prevention (OIDC only)
	pub nonce: Option<String>,
	/// The PKCE code verifier (if PKCE is used)
	pub code_verifier: Option<String>,
}

/// Result of handling an authorization callback
pub struct CallbackResult {
	/// The token response from the provider
	pub token_response: TokenResponse,
	/// The user's claims (from ID token or UserInfo endpoint)
	pub claims: Option<StandardClaims>,
}

/// Result of handling a contextual authorization callback.
pub struct ContextualCallbackResult {
	/// The callback result from the provider.
	pub callback: CallbackResult,
	/// Opaque application context stored when authorization began.
	pub context: Vec<u8>,
}

/// Social authentication backend
pub struct SocialAuthBackend {
	providers: HashMap<String, Arc<dyn OAuthProvider>>,
	state_store: Arc<dyn StateStore>,
}

impl SocialAuthBackend {
	/// Create a new social authentication backend with in-memory state store
	pub fn new() -> Self {
		Self {
			providers: HashMap::new(),
			state_store: Arc::new(InMemoryStateStore::new()),
		}
	}

	/// Create a new social authentication backend with custom state store
	pub fn with_state_store(state_store: Arc<dyn StateStore>) -> Self {
		Self {
			providers: HashMap::new(),
			state_store,
		}
	}

	/// Register a provider
	pub fn register_provider(&mut self, provider: Arc<dyn OAuthProvider>) {
		self.providers.insert(provider.name().to_string(), provider);
	}

	/// Get a registered provider by name
	pub fn get_provider(&self, name: &str) -> Option<&Arc<dyn OAuthProvider>> {
		self.providers.get(name)
	}

	/// List registered provider names
	pub fn provider_names(&self) -> Vec<&str> {
		self.providers.keys().map(|s| s.as_str()).collect()
	}

	/// Begin an authorization flow for a provider
	pub async fn begin_auth(
		&self,
		provider_name: &str,
		code_challenge: Option<&str>,
		code_verifier: Option<String>,
	) -> Result<AuthorizationResult, SocialAuthError> {
		let (authorization, state_data) = self
			.prepare_authorization(provider_name, code_challenge, code_verifier)
			.await?;
		self.state_store.store(state_data).await?;
		Ok(authorization)
	}

	/// Begin an authorization flow with browser or session binding and opaque context.
	pub async fn begin_auth_with_context(
		&self,
		provider_name: &str,
		code_challenge: Option<&str>,
		code_verifier: Option<String>,
		binding: &[u8],
		context: Vec<u8>,
	) -> Result<AuthorizationResult, SocialAuthError> {
		if binding.is_empty() {
			return Err(SocialAuthError::StateValidation(
				"OAuth state binding must not be empty".to_string(),
			));
		}

		let (authorization, state_data) = self
			.prepare_authorization(provider_name, code_challenge, code_verifier)
			.await?;
		let contextual =
			ContextualStateData::new(state_data, provider_name.to_string(), binding, context)?;
		self.state_store.store_contextual(contextual).await?;
		Ok(authorization)
	}

	/// Prepare the provider authorization URL and state record shared by both flows.
	async fn prepare_authorization(
		&self,
		provider_name: &str,
		code_challenge: Option<&str>,
		code_verifier: Option<String>,
	) -> Result<(AuthorizationResult, StateData), SocialAuthError> {
		let provider = self.providers.get(provider_name).ok_or_else(|| {
			SocialAuthError::Provider(format!("Provider not registered: {}", provider_name))
		})?;

		// Generate state for CSRF protection
		let state = generate_random_string(32);

		// Generate nonce for OIDC providers
		let nonce = if provider.is_oidc() {
			Some(generate_random_string(32))
		} else {
			None
		};

		// Build authorization URL
		let authorization_url = provider
			.authorization_url(&state, nonce.as_deref(), code_challenge)
			.await?;

		// Store state data for callback verification
		let state_data = StateData::new(state.clone(), nonce.clone(), code_verifier.clone());
		Ok((
			AuthorizationResult {
				authorization_url,
				state,
				nonce,
				code_verifier,
			},
			state_data,
		))
	}

	/// Handle an authorization callback
	pub async fn handle_callback(
		&self,
		provider_name: &str,
		code: &str,
		state: &str,
	) -> Result<CallbackResult, SocialAuthError> {
		let provider = self.providers.get(provider_name).ok_or_else(|| {
			SocialAuthError::Provider(format!("Provider not registered: {}", provider_name))
		})?;

		let state_data = self.state_store.consume(state).await?;
		self.complete_callback(provider.as_ref(), provider_name, code, &state_data)
			.await
	}

	/// Handle a contextual authorization callback with binding verification.
	pub async fn handle_callback_with_context(
		&self,
		provider_name: &str,
		code: &str,
		state: &str,
		binding: &[u8],
	) -> Result<ContextualCallbackResult, SocialAuthError> {
		if binding.is_empty() {
			return Err(SocialAuthError::StateValidation(
				"OAuth state binding must not be empty".to_string(),
			));
		}

		let provider = self.providers.get(provider_name).ok_or_else(|| {
			SocialAuthError::Provider(format!("Provider not registered: {}", provider_name))
		})?;
		let contextual = self.state_store.consume_contextual(state).await?;
		if contextual.state_data().is_expired()
			|| contextual.provider_name() != provider_name
			|| !contextual.binding_matches(binding)
		{
			return Err(SocialAuthError::InvalidState);
		}

		let (state_data, context) = contextual.into_parts();
		let callback = self
			.complete_callback(provider.as_ref(), provider_name, code, &state_data)
			.await?;
		Ok(ContextualCallbackResult { callback, context })
	}

	/// Complete provider token exchange and claims retrieval for a validated state.
	async fn complete_callback(
		&self,
		provider: &dyn OAuthProvider,
		provider_name: &str,
		code: &str,
		state_data: &StateData,
	) -> Result<CallbackResult, SocialAuthError> {
		// Exchange code for tokens
		let token_response = provider
			.exchange_code(code, state_data.code_verifier.as_deref())
			.await?;

		// Try to get user claims
		let claims = if provider.is_oidc() {
			// For OIDC providers, validate ID token if present
			if let Some(id_token_str) = &token_response.id_token {
				let id_token = provider
					.validate_id_token(id_token_str, state_data.nonce.as_deref())
					.await?;
				Some(StandardClaims::from(id_token))
			} else {
				// Fall back to UserInfo endpoint. UserInfo failures are non-fatal:
				// log them so operators can diagnose silent claim losses (issue #4001).
				provider
					.get_user_info(&token_response.access_token)
					.await
					.inspect_err(|e| {
						tracing::warn!(
							provider = %provider_name,
							error = %e,
							"Failed to fetch user info from OIDC UserInfo fallback; claims will be None",
						)
					})
					.ok()
			}
		} else {
			// For OAuth2-only providers, use UserInfo endpoint. UserInfo failures
			// are non-fatal: log them so operators can diagnose silent claim
			// losses (issue #4001).
			provider
				.get_user_info(&token_response.access_token)
				.await
				.inspect_err(|e| {
					tracing::warn!(
						provider = %provider_name,
						error = %e,
						"Failed to fetch user info from OAuth2 provider; claims will be None",
					)
				})
				.ok()
		};

		Ok(CallbackResult {
			token_response,
			claims,
		})
	}
}

impl Default for SocialAuthBackend {
	fn default() -> Self {
		Self::new()
	}
}

/// Generates a random alphanumeric string of the specified length
fn generate_random_string(length: usize) -> String {
	use rand::Rng;
	rand::rng()
		.sample_iter(&rand::distr::Alphanumeric)
		.take(length)
		.map(char::from)
		.collect()
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_backend_creation() {
		// Arrange & Act
		let backend = SocialAuthBackend::new();

		// Assert
		assert!(backend.provider_names().is_empty());
	}

	#[test]
	fn test_backend_default() {
		// Arrange & Act
		let backend = SocialAuthBackend::default();

		// Assert
		assert!(backend.provider_names().is_empty());
	}

	#[test]
	fn test_get_nonexistent_provider() {
		// Arrange
		let backend = SocialAuthBackend::new();

		// Act
		let provider = backend.get_provider("nonexistent");

		// Assert
		assert!(provider.is_none());
	}
}
