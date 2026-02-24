//! Google OIDC provider tests

use reinhardt_auth::social::core::OAuthProvider;
use reinhardt_auth::social::core::config::ProviderConfig;
use reinhardt_auth::social::providers::GoogleProvider;
use rstest::*;

#[tokio::test]
async fn test_google_provider_config() {
	// Arrange
	let config = ProviderConfig::google(
		"test_client_id".into(),
		"test_client_secret".into(),
		"http://localhost:8080/callback".into(),
	);

	// Assert
	assert_eq!(config.name, "google");
	assert!(config.oidc.is_some());
	assert!(config.oauth2.is_none());
}

#[tokio::test]
async fn test_google_provider_scopes() {
	// Arrange
	let config = ProviderConfig::google(
		"test_client_id".into(),
		"test_client_secret".into(),
		"http://localhost:8080/callback".into(),
	);

	// Assert
	assert!(config.scopes.contains(&"openid".to_string()));
	assert!(config.scopes.contains(&"email".to_string()));
	assert!(config.scopes.contains(&"profile".to_string()));
}

#[tokio::test]
async fn test_google_oidc_discovery_url() {
	// Arrange
	let config = ProviderConfig::google(
		"test_client_id".into(),
		"test_client_secret".into(),
		"http://localhost:8080/callback".into(),
	);
	let oidc = config.oidc.unwrap();

	// Assert
	assert!(oidc.discovery_url.contains("accounts.google.com"));
	assert!(
		oidc.discovery_url
			.contains(".well-known/openid-configuration")
	);
}

#[tokio::test]
async fn test_google_provider_create_succeeds() {
	// Arrange
	let config = ProviderConfig::google(
		"test_client_id".into(),
		"test_client_secret".into(),
		"http://localhost:8080/callback".into(),
	);

	// Act
	let result = GoogleProvider::new(config).await;

	// Assert
	assert!(
		result.is_ok(),
		"Google provider should be created successfully"
	);
	let provider = result.unwrap();
	assert_eq!(provider.name(), "google");
	assert!(provider.is_oidc());
}

#[tokio::test]
async fn test_google_provider_requires_oidc_config() {
	// Arrange - Create config without OIDC
	let config = ProviderConfig::github(
		"test_client_id".into(),
		"test_client_secret".into(),
		"http://localhost:8080/callback".into(),
	);

	// Act
	let result = GoogleProvider::new(config).await;

	// Assert
	assert!(
		result.is_err(),
		"Google provider should fail without OIDC config"
	);
}

#[tokio::test]
async fn test_google_is_oidc() {
	// Arrange
	let config = ProviderConfig::google(
		"test_client_id".into(),
		"test_client_secret".into(),
		"http://localhost:8080/callback".into(),
	);

	// Assert
	assert!(config.oidc.is_some(), "Google uses OIDC");
	assert!(config.oauth2.is_none(), "Google does not use OAuth2");
}
