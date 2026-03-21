//! Social authentication backend
//!
//! Orchestrates OAuth2/OIDC flows and integrates with reinhardt-auth.

use std::collections::HashMap;
use std::sync::Arc;

use crate::social::core::{OAuthProvider, SocialAuthError, StandardClaims, TokenResponse};
use crate::social::flow::{InMemoryStateStore, StateData, StateStore};

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
		self.state_store.store(state_data).await?;

		Ok(AuthorizationResult {
			authorization_url,
			state,
			nonce,
			code_verifier,
		})
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

		// Verify and consume state
		let state_data = self.state_store.retrieve(state).await?;
		self.state_store.remove(state).await?;

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
				// Fall back to UserInfo endpoint
				provider
					.get_user_info(&token_response.access_token)
					.await
					.ok()
			}
		} else {
			// For OAuth2-only providers, use UserInfo endpoint
			provider
				.get_user_info(&token_response.access_token)
				.await
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
