//! Context-aware social authentication regression tests.

use async_trait::async_trait;
use reinhardt_auth::social::{
	OAuthProvider, SocialAuthBackend, SocialAuthError, StandardClaims, TokenResponse,
};
use std::collections::HashMap;
use std::sync::{
	Arc,
	atomic::{AtomicUsize, Ordering},
};

struct TestProvider {
	name: &'static str,
	exchange_calls: Arc<AtomicUsize>,
}

impl TestProvider {
	fn new(name: &'static str, exchange_calls: Arc<AtomicUsize>) -> Self {
		Self {
			name,
			exchange_calls,
		}
	}
}

#[async_trait]
impl OAuthProvider for TestProvider {
	fn name(&self) -> &str {
		self.name
	}

	fn is_oidc(&self) -> bool {
		false
	}

	async fn authorization_url(
		&self,
		state: &str,
		_nonce: Option<&str>,
		_code_challenge: Option<&str>,
	) -> Result<String, SocialAuthError> {
		Ok(format!("https://provider.example/authorize?state={state}"))
	}

	async fn exchange_code(
		&self,
		_code: &str,
		_code_verifier: Option<&str>,
	) -> Result<TokenResponse, SocialAuthError> {
		self.exchange_calls.fetch_add(1, Ordering::SeqCst);
		Ok(TokenResponse {
			access_token: "access-token".to_string(),
			token_type: "Bearer".to_string(),
			expires_in: Some(3600),
			refresh_token: None,
			scope: None,
			id_token: None,
		})
	}

	async fn refresh_token(&self, _refresh_token: &str) -> Result<TokenResponse, SocialAuthError> {
		Err(SocialAuthError::NotSupported("refresh".to_string()))
	}

	async fn get_user_info(&self, _access_token: &str) -> Result<StandardClaims, SocialAuthError> {
		Ok(StandardClaims {
			sub: "user-42".to_string(),
			email: None,
			email_verified: None,
			name: None,
			given_name: None,
			family_name: None,
			picture: None,
			locale: None,
			additional_claims: HashMap::new(),
		})
	}
}

#[tokio::test]
async fn contextual_callback_returns_bound_context() {
	// Arrange
	let exchange_calls = Arc::new(AtomicUsize::new(0));
	let mut backend = SocialAuthBackend::new();
	backend.register_provider(Arc::new(TestProvider::new(
		"github",
		exchange_calls.clone(),
	)));
	let authorization = backend
		.begin_auth_with_context(
			"github",
			None,
			Some("pkce-verifier".to_string()),
			b"browser-a",
			b"link-user-42".to_vec(),
		)
		.await
		.unwrap();

	// Act
	let result = backend
		.handle_callback_with_context(
			"github",
			"provider-code",
			&authorization.state,
			b"browser-a",
		)
		.await
		.unwrap();

	// Assert
	assert_eq!(result.context, b"link-user-42");
	assert_eq!(result.callback.token_response.access_token, "access-token");
	assert_eq!(exchange_calls.load(Ordering::SeqCst), 1);

	let replay = backend
		.handle_callback_with_context(
			"github",
			"provider-code",
			&authorization.state,
			b"browser-a",
		)
		.await;
	assert!(matches!(replay, Err(SocialAuthError::InvalidState)));
	assert_eq!(exchange_calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn contextual_callback_rejects_login_csrf_and_consumes_state() {
	// Arrange
	let exchange_calls = Arc::new(AtomicUsize::new(0));
	let mut backend = SocialAuthBackend::new();
	backend.register_provider(Arc::new(TestProvider::new(
		"github",
		exchange_calls.clone(),
	)));
	let authorization = backend
		.begin_auth_with_context("github", None, None, b"browser-a", Vec::new())
		.await
		.unwrap();

	// Act
	let mismatched = backend
		.handle_callback_with_context(
			"github",
			"provider-code",
			&authorization.state,
			b"browser-b",
		)
		.await;
	let retry = backend
		.handle_callback_with_context(
			"github",
			"provider-code",
			&authorization.state,
			b"browser-a",
		)
		.await;

	// Assert
	assert!(matches!(mismatched, Err(SocialAuthError::InvalidState)));
	assert!(matches!(retry, Err(SocialAuthError::InvalidState)));
	assert_eq!(exchange_calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn contextual_callback_rejects_session_swap() {
	// Arrange
	let exchange_calls = Arc::new(AtomicUsize::new(0));
	let mut backend = SocialAuthBackend::new();
	backend.register_provider(Arc::new(TestProvider::new(
		"github",
		exchange_calls.clone(),
	)));
	let authorization = backend
		.begin_auth_with_context("github", None, None, b"session-user-a", Vec::new())
		.await
		.unwrap();

	// Act
	let result = backend
		.handle_callback_with_context(
			"github",
			"provider-code",
			&authorization.state,
			b"session-user-b",
		)
		.await;

	// Assert
	assert!(matches!(result, Err(SocialAuthError::InvalidState)));
	assert_eq!(exchange_calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn contextual_callback_rejects_provider_swap_before_exchange() {
	// Arrange
	let github_calls = Arc::new(AtomicUsize::new(0));
	let google_calls = Arc::new(AtomicUsize::new(0));
	let mut backend = SocialAuthBackend::new();
	backend.register_provider(Arc::new(TestProvider::new("github", github_calls.clone())));
	backend.register_provider(Arc::new(TestProvider::new("google", google_calls.clone())));
	let authorization = backend
		.begin_auth_with_context("github", None, None, b"browser-a", Vec::new())
		.await
		.unwrap();

	// Act
	let result = backend
		.handle_callback_with_context(
			"google",
			"provider-code",
			&authorization.state,
			b"browser-a",
		)
		.await;

	// Assert
	assert!(matches!(result, Err(SocialAuthError::InvalidState)));
	assert_eq!(github_calls.load(Ordering::SeqCst), 0);
	assert_eq!(google_calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn contextual_begin_rejects_empty_binding() {
	let backend = SocialAuthBackend::new();

	let result = backend
		.begin_auth_with_context("missing", None, None, b"", Vec::new())
		.await;

	assert!(matches!(result, Err(SocialAuthError::StateValidation(_))));
}

#[tokio::test]
async fn contextual_callback_rejects_empty_binding_without_consuming_state() {
	// Arrange
	let exchange_calls = Arc::new(AtomicUsize::new(0));
	let mut backend = SocialAuthBackend::new();
	backend.register_provider(Arc::new(TestProvider::new(
		"github",
		exchange_calls.clone(),
	)));
	let authorization = backend
		.begin_auth_with_context("github", None, None, b"browser-a", Vec::new())
		.await
		.unwrap();

	// Act
	let empty_binding = backend
		.handle_callback_with_context("github", "code", &authorization.state, b"")
		.await;
	let valid_binding = backend
		.handle_callback_with_context("github", "code", &authorization.state, b"browser-a")
		.await;

	// Assert
	assert!(matches!(
		empty_binding,
		Err(SocialAuthError::StateValidation(_))
	));
	assert!(valid_binding.is_ok());
	assert_eq!(exchange_calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn legacy_callback_still_succeeds() {
	// Arrange
	let exchange_calls = Arc::new(AtomicUsize::new(0));
	let mut backend = SocialAuthBackend::new();
	backend.register_provider(Arc::new(TestProvider::new(
		"github",
		exchange_calls.clone(),
	)));
	let authorization = backend
		.begin_auth("github", None, Some("pkce-verifier".to_string()))
		.await
		.unwrap();

	// Act
	let result = backend
		.handle_callback("github", "provider-code", &authorization.state)
		.await
		.unwrap();

	// Assert
	assert_eq!(result.token_response.access_token, "access-token");
	assert_eq!(exchange_calls.load(Ordering::SeqCst), 1);
}
