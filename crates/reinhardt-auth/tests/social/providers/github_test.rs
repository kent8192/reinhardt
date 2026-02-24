//! GitHub OAuth2 provider tests

use reinhardt_auth::social::core::OAuthProvider;
use reinhardt_auth::social::core::config::ProviderConfig;
use reinhardt_auth::social::providers::GitHubProvider;
use rstest::*;

#[tokio::test]
async fn test_github_provider_config() {
	// Arrange
	let config = ProviderConfig::github(
		"test_client_id".into(),
		"test_client_secret".into(),
		"http://localhost:8080/callback".into(),
	);

	// Assert
	assert_eq!(config.name, "github");
	assert!(config.oauth2.is_some());
	assert!(config.oidc.is_none());
}

#[tokio::test]
async fn test_github_provider_scopes() {
	// Arrange
	let config = ProviderConfig::github(
		"test_client_id".into(),
		"test_client_secret".into(),
		"http://localhost:8080/callback".into(),
	);

	// Assert
	assert!(config.scopes.contains(&"user".to_string()));
	assert!(config.scopes.contains(&"user:email".to_string()));
}

#[tokio::test]
async fn test_github_oauth2_endpoints() {
	// Arrange
	let config = ProviderConfig::github(
		"test_client_id".into(),
		"test_client_secret".into(),
		"http://localhost:8080/callback".into(),
	);
	let oauth2 = config.oauth2.unwrap();

	// Assert
	assert_eq!(
		oauth2.authorization_endpoint,
		"https://github.com/login/oauth/authorize"
	);
	assert_eq!(
		oauth2.token_endpoint,
		"https://github.com/login/oauth/access_token"
	);
	assert_eq!(
		oauth2.userinfo_endpoint,
		Some("https://api.github.com/user".into())
	);
}

#[tokio::test]
async fn test_github_provider_create_succeeds() {
	// Arrange
	let config = ProviderConfig::github(
		"test_client_id".into(),
		"test_client_secret".into(),
		"http://localhost:8080/callback".into(),
	);

	// Act
	let result = GitHubProvider::new(config).await;

	// Assert
	assert!(
		result.is_ok(),
		"GitHub provider should be created successfully"
	);
	let provider = result.unwrap();
	assert_eq!(provider.name(), "github");
	assert!(!provider.is_oidc());
}

#[tokio::test]
async fn test_github_provider_requires_oauth2_config() {
	// Arrange - Create config without OAuth2
	let config = ProviderConfig::google(
		"test_client_id".into(),
		"test_client_secret".into(),
		"http://localhost:8080/callback".into(),
	);

	// Act
	let result = GitHubProvider::new(config).await;

	// Assert
	assert!(
		result.is_err(),
		"GitHub provider should fail without OAuth2 config"
	);
}

#[tokio::test]
async fn test_github_is_oauth2_not_oidc() {
	// Arrange
	let config = ProviderConfig::github(
		"test_client_id".into(),
		"test_client_secret".into(),
		"http://localhost:8080/callback".into(),
	);

	// Assert
	assert!(config.oauth2.is_some(), "GitHub uses OAuth2");
	assert!(config.oidc.is_none(), "GitHub does not use OIDC");
}
