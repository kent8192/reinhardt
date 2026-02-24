//! End-to-end flow tests

use helpers::mock_server::MockOAuth2Server;
use reinhardt_auth::social::core::OAuthProvider;
use reinhardt_auth::social::core::config::{OAuth2Config, OIDCConfig, ProviderConfig};
use reinhardt_auth::social::flow::{InMemoryStateStore, PkceFlow, StateData, StateStore};
use reinhardt_auth::social::providers::{GitHubProvider, GoogleProvider};
use rstest::*;

#[path = "../../helpers.rs"]
mod helpers;

#[tokio::test]
async fn test_complete_github_oauth2_flow() {
	// Arrange
	let server = MockOAuth2Server::new().await;
	let config = ProviderConfig {
		name: "github".to_string(),
		client_id: "test_client_id".to_string(),
		client_secret: "test_client_secret".to_string(),
		redirect_uri: "http://localhost:8080/callback".to_string(),
		scopes: vec!["user".to_string(), "user:email".to_string()],
		oidc: None,
		oauth2: Some(OAuth2Config {
			authorization_endpoint: server.authorization_url(),
			token_endpoint: server.token_url(),
			userinfo_endpoint: server.userinfo_url(),
		}),
	};
	let state_store = InMemoryStateStore::new();
	let provider = GitHubProvider::new(config).await.unwrap();

	// Step 1: Generate authorization URL
	let state = "test_state_123";
	let auth_url = provider.authorization_url(state, None, None).await.unwrap();
	assert!(auth_url.contains(&server.authorization_url()));
	assert!(auth_url.contains("state=test_state_123"));
	assert!(auth_url.contains("response_type=code"));

	// Step 2: Store state
	let state_data = StateData::new(state.to_string(), None, None);
	state_store.store(state_data).await.unwrap();

	// Step 3: Exchange authorization code
	let token_response = provider
		.exchange_code("test_authorization_code", None)
		.await
		.unwrap();
	assert_eq!(token_response.access_token, "test_access_token");
	assert_eq!(token_response.token_type, "Bearer");

	// Step 4: Retrieve user info
	let claims = provider
		.get_user_info(&token_response.access_token)
		.await
		.unwrap();
	assert_eq!(claims.sub, "test_user");
	assert_eq!(claims.email, Some("test@example.com".to_string()));

	// Step 5: Verify state retrieval
	let retrieved_state = state_store.retrieve(state).await.unwrap();
	assert_eq!(retrieved_state.state, state);
}

#[tokio::test]
async fn test_complete_google_oidc_flow() {
	// Arrange
	let server = MockOAuth2Server::new().await;
	let config = ProviderConfig {
		name: "google".to_string(),
		client_id: "test_client_id".to_string(),
		client_secret: "test_client_secret".to_string(),
		redirect_uri: "http://localhost:8080/callback".to_string(),
		scopes: vec![
			"openid".to_string(),
			"email".to_string(),
			"profile".to_string(),
		],
		oidc: Some(OIDCConfig {
			discovery_url: format!("{}/.well-known/openid-configuration", server.base_url()),
			use_nonce: true,
		}),
		oauth2: None,
	};
	let state_store = InMemoryStateStore::new();
	let provider = GoogleProvider::new(config).await.unwrap();

	// Step 1: Generate authorization URL with PKCE and nonce
	let state = "test_state_456";
	let nonce = "test_nonce_789";
	let (verifier, challenge) = PkceFlow::generate();

	let auth_url = provider
		.authorization_url(state, Some(nonce), Some(challenge.as_str()))
		.await
		.unwrap();
	assert!(auth_url.contains("state=test_state_456"));
	assert!(auth_url.contains("code_challenge"));
	assert!(auth_url.contains("code_challenge_method=S256"));
	assert!(auth_url.contains("nonce=test_nonce_789"));

	// Step 2: Store state with nonce and verifier
	let state_data = StateData::new(
		state.to_string(),
		Some(nonce.to_string()),
		Some(verifier.as_str().to_string()),
	);
	state_store.store(state_data).await.unwrap();

	// Step 3: Exchange authorization code with PKCE verifier
	let token_response = provider
		.exchange_code("test_authorization_code", Some(verifier.as_str()))
		.await
		.unwrap();
	assert_eq!(token_response.access_token, "test_access_token");
	assert_eq!(token_response.token_type, "Bearer");

	// Step 4: Retrieve user info
	let claims = provider
		.get_user_info(&token_response.access_token)
		.await
		.unwrap();
	assert_eq!(claims.sub, "test_user");
	assert_eq!(claims.email, Some("test@example.com".to_string()));

	// Step 5: Verify state retrieval
	let retrieved_state = state_store.retrieve(state).await.unwrap();
	assert_eq!(retrieved_state.state, state);
	assert_eq!(retrieved_state.nonce, Some(nonce.to_string()));
}

#[tokio::test]
async fn test_complete_flow_with_pkce_enabled() {
	// Arrange
	let server = MockOAuth2Server::new().await;
	let config = ProviderConfig {
		name: "github".to_string(),
		client_id: "test_client_id".to_string(),
		client_secret: "test_client_secret".to_string(),
		redirect_uri: "http://localhost:8080/callback".to_string(),
		scopes: vec!["user".to_string(), "user:email".to_string()],
		oidc: None,
		oauth2: Some(OAuth2Config {
			authorization_endpoint: server.authorization_url(),
			token_endpoint: server.token_url(),
			userinfo_endpoint: server.userinfo_url(),
		}),
	};
	let provider = GitHubProvider::new(config).await.unwrap();
	let (verifier, challenge) = PkceFlow::generate();

	// Act
	let auth_url = provider
		.authorization_url("test_state", None, Some(challenge.as_str()))
		.await
		.unwrap();

	// Assert - Authorization URL contains PKCE parameters
	assert!(auth_url.contains("code_challenge"));
	assert!(auth_url.contains("code_challenge_method=S256"));

	// Act - Exchange code with PKCE verifier
	let token_response = provider
		.exchange_code("test_code", Some(verifier.as_str()))
		.await
		.unwrap();

	// Assert
	assert_eq!(token_response.access_token, "test_access_token");
	assert_eq!(token_response.token_type, "Bearer");
}

#[tokio::test]
async fn test_complete_flow_with_pkce_disabled() {
	// Arrange
	let server = MockOAuth2Server::new().await;
	let config = ProviderConfig {
		name: "github".to_string(),
		client_id: "test_client_id".to_string(),
		client_secret: "test_client_secret".to_string(),
		redirect_uri: "http://localhost:8080/callback".to_string(),
		scopes: vec!["user".to_string(), "user:email".to_string()],
		oidc: None,
		oauth2: Some(OAuth2Config {
			authorization_endpoint: server.authorization_url(),
			token_endpoint: server.token_url(),
			userinfo_endpoint: server.userinfo_url(),
		}),
	};
	let provider = GitHubProvider::new(config).await.unwrap();

	// Act
	let auth_url = provider
		.authorization_url("test_state", None, None)
		.await
		.unwrap();

	// Assert - No PKCE parameters present
	assert!(!auth_url.contains("code_challenge"));
	assert!(!auth_url.contains("code_verifier"));
	assert!(auth_url.contains("response_type=code"));
	assert!(auth_url.contains("state=test_state"));
}
