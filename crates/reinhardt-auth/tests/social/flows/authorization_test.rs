//! Authorization flow tests

use reinhardt_auth::social::core::OAuthProvider;
use reinhardt_auth::social::core::config::ProviderConfig;
use reinhardt_auth::social::providers::GitHubProvider;
use rstest::*;

#[tokio::test]
async fn test_authorization_url_generation_github() {
	// Arrange
	let config = ProviderConfig::github(
		"test_client_id".into(),
		"test_client_secret".into(),
		"http://localhost:8080/callback".into(),
	);
	let provider = GitHubProvider::new(config).await.unwrap();

	// Act
	let url = provider
		.authorization_url("test_state", None, None)
		.await
		.unwrap();

	// Assert
	assert!(url.contains("github.com"));
	assert!(url.contains("client_id=test_client_id"));
	assert!(url.contains("state=test_state"));
}

#[tokio::test]
async fn test_authorization_url_with_pkce() {
	// Arrange
	let config = ProviderConfig::github(
		"test_client_id".into(),
		"test_client_secret".into(),
		"http://localhost:8080/callback".into(),
	);
	let provider = GitHubProvider::new(config).await.unwrap();
	let (_verifier, challenge) = reinhardt_auth::social::flow::PkceFlow::generate();

	// Act
	let url = provider
		.authorization_url("test_state", None, Some(challenge.as_str()))
		.await
		.unwrap();

	// Assert
	assert!(url.contains("code_challenge"));
	assert!(url.contains("code_challenge_method=S256"));
}

#[tokio::test]
async fn test_authorization_url_required_parameters() {
	// Arrange
	let config = ProviderConfig::github(
		"test_client_id".into(),
		"test_client_secret".into(),
		"http://localhost:8080/callback".into(),
	);
	let provider = GitHubProvider::new(config).await.unwrap();

	// Act
	let url = provider
		.authorization_url("test_state", None, None)
		.await
		.unwrap();

	// Assert
	assert!(url.contains("response_type=code"));
	assert!(url.contains("state=test_state"));
	assert!(url.contains("redirect_uri=http%3A%2F%2Flocalhost%3A8080%2Fcallback"));
}
