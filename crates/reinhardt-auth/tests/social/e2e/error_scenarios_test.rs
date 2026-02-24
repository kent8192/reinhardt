//! Error scenario tests

use helpers::mock_server::MockOAuth2Server;
use reinhardt_auth::social::core::OAuthProvider;
use reinhardt_auth::social::core::config::{OAuth2Config, ProviderConfig};
use reinhardt_auth::social::flow::{InMemoryStateStore, StateData, StateStore};
use reinhardt_auth::social::providers::GitHubProvider;
use rstest::*;

#[path = "../../helpers.rs"]
mod helpers;

#[tokio::test]
async fn test_error_server_error_on_token_exchange() {
	// Arrange
	let mut server = MockOAuth2Server::new().await;
	server.set_error_mode(helpers::mock_server::ErrorMode::ServerError);
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
	let result = provider.exchange_code("test_code", None).await;

	// Assert
	assert!(
		result.is_err(),
		"Token exchange should fail on server error"
	);
}

#[tokio::test]
async fn test_error_invalid_json_response() {
	// Arrange
	let mut server = MockOAuth2Server::new().await;
	server.set_error_mode(helpers::mock_server::ErrorMode::InvalidResponse);
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
	let result = provider.exchange_code("test_code", None).await;

	// Assert
	assert!(
		result.is_err(),
		"Token exchange should fail on invalid JSON response"
	);
}

#[tokio::test]
async fn test_error_unauthorized_access() {
	// Arrange
	let mut server = MockOAuth2Server::new().await;
	server.set_error_mode(helpers::mock_server::ErrorMode::Unauthorized);
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
	let result = provider.exchange_code("test_code", None).await;

	// Assert
	assert!(
		result.is_err(),
		"Token exchange should fail on unauthorized access"
	);
}

#[tokio::test]
async fn test_error_server_error_on_userinfo() {
	// Arrange - Start with success mode, exchange token, then switch to error
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

	// Act - Exchange code successfully first
	let token_response = provider.exchange_code("test_code", None).await.unwrap();
	assert_eq!(token_response.access_token, "test_access_token");

	// Disable userinfo endpoint
	let server = server.without_userinfo();
	let _ = server; // keep server alive

	// Get user info should fail since endpoint is disabled
	let userinfo_result = provider.get_user_info(&token_response.access_token).await;
	assert!(
		userinfo_result.is_err(),
		"UserInfo should fail when endpoint returns 404"
	);
}

#[tokio::test]
async fn test_error_invalid_state_parameter() {
	// Arrange
	let state_store = InMemoryStateStore::new();

	// Act - Try to retrieve non-existent state
	let result = state_store.retrieve("invalid_state").await;

	// Assert
	assert!(result.is_err(), "Retrieving non-existent state should fail");
}

#[tokio::test]
async fn test_error_recovery_flow() {
	// Arrange
	let state_store = InMemoryStateStore::new();

	// Act - Try to retrieve state, handle error
	let result = state_store.retrieve("nonexistent").await;
	assert!(result.is_err(), "Non-existent state should return error");

	// Recovery - Create new state and proceed
	let new_state = "recovery_state";
	let state_data = StateData::new(new_state.to_string(), None, None);
	let store_result = state_store.store(state_data).await;

	// Assert - Recovery successful
	assert!(store_result.is_ok(), "Storing new state should succeed");
	let retrieve_result = state_store.retrieve(new_state).await;
	assert!(
		retrieve_result.is_ok(),
		"Retrieving stored state should succeed"
	);
	assert_eq!(retrieve_result.unwrap().state, new_state);
}
