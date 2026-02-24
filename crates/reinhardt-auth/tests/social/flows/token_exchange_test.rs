//! Token exchange flow tests

use helpers::mock_server::MockOAuth2Server;
use reinhardt_auth::social::core::OAuth2Client;
use reinhardt_auth::social::core::config::ProviderConfig;
use reinhardt_auth::social::flow::TokenExchangeFlow;
use rstest::*;

#[path = "../../helpers.rs"]
mod helpers;

#[tokio::test]
async fn test_token_exchange_with_mock_server() {
	// Arrange
	let server = MockOAuth2Server::new().await;
	let config = ProviderConfig::github(
		"test_client_id".into(),
		"test_client_secret".into(),
		"http://localhost:8080/callback".into(),
	);
	let flow = TokenExchangeFlow::new(OAuth2Client::new(), config);

	// Act
	let result = flow
		.exchange(&server.token_url(), "test_authorization_code", None)
		.await;

	// Assert
	assert!(
		result.is_ok(),
		"Token exchange should succeed with mock server"
	);
	let response = result.unwrap();
	assert_eq!(response.access_token, "test_access_token");
	assert_eq!(response.token_type, "Bearer");
}

#[tokio::test]
async fn test_token_exchange_with_pkce() {
	// Arrange
	let server = MockOAuth2Server::new().await;
	let config = ProviderConfig::github(
		"test_client_id".into(),
		"test_client_secret".into(),
		"http://localhost:8080/callback".into(),
	);
	let flow = TokenExchangeFlow::new(OAuth2Client::new(), config);
	let (verifier, _challenge) = reinhardt_auth::social::flow::PkceFlow::generate();

	// Act
	let result = flow
		.exchange(
			&server.token_url(),
			"test_authorization_code",
			Some(&verifier),
		)
		.await;

	// Assert
	assert!(result.is_ok(), "Token exchange with PKCE should succeed");
	let response = result.unwrap();
	assert!(!response.access_token.is_empty());
}

#[tokio::test]
async fn test_token_exchange_server_error() {
	// Arrange
	let mut server = MockOAuth2Server::new().await;
	server.set_error_mode(helpers::mock_server::ErrorMode::ServerError);
	let config = ProviderConfig::github(
		"test_client_id".into(),
		"test_client_secret".into(),
		"http://localhost:8080/callback".into(),
	);
	let flow = TokenExchangeFlow::new(OAuth2Client::new(), config);

	// Act
	let result = flow.exchange(&server.token_url(), "test_code", None).await;

	// Assert
	assert!(
		result.is_err(),
		"Token exchange should fail on server error"
	);
}

#[tokio::test]
async fn test_token_exchange_invalid_response() {
	// Arrange
	let mut server = MockOAuth2Server::new().await;
	server.set_error_mode(helpers::mock_server::ErrorMode::InvalidResponse);
	let config = ProviderConfig::github(
		"test_client_id".into(),
		"test_client_secret".into(),
		"http://localhost:8080/callback".into(),
	);
	let flow = TokenExchangeFlow::new(OAuth2Client::new(), config);

	// Act
	let result = flow.exchange(&server.token_url(), "test_code", None).await;

	// Assert
	assert!(
		result.is_err(),
		"Token exchange should fail on invalid JSON response"
	);
}
