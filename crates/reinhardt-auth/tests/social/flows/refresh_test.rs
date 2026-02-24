//! Token refresh flow tests

use helpers::mock_server::MockOAuth2Server;
use reinhardt_auth::social::core::OAuth2Client;
use reinhardt_auth::social::core::config::ProviderConfig;
use reinhardt_auth::social::flow::RefreshFlow;
use rstest::*;

#[path = "../../helpers.rs"]
mod helpers;

#[tokio::test]
async fn test_token_refresh_with_mock_server() {
	// Arrange
	let server = MockOAuth2Server::new().await;
	let config = ProviderConfig::github(
		"test_client_id".into(),
		"test_client_secret".into(),
		"http://localhost:8080/callback".into(),
	);
	let flow = RefreshFlow::new(OAuth2Client::new(), config);

	// Act
	let result = flow
		.refresh(&server.token_url(), "test_refresh_token")
		.await;

	// Assert
	assert!(
		result.is_ok(),
		"Token refresh should succeed with mock server"
	);
	let response = result.unwrap();
	assert!(!response.access_token.is_empty());
	assert_eq!(response.token_type, "Bearer");
}

#[tokio::test]
async fn test_token_refresh_server_error() {
	// Arrange
	let mut server = MockOAuth2Server::new().await;
	server.set_error_mode(helpers::mock_server::ErrorMode::ServerError);
	let config = ProviderConfig::github(
		"test_client_id".into(),
		"test_client_secret".into(),
		"http://localhost:8080/callback".into(),
	);
	let flow = RefreshFlow::new(OAuth2Client::new(), config);

	// Act
	let result = flow
		.refresh(&server.token_url(), "test_refresh_token")
		.await;

	// Assert
	assert!(result.is_err(), "Token refresh should fail on server error");
}
