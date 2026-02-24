//! Microsoft OIDC provider tests

use reinhardt_auth::social::core::OAuthProvider;
use reinhardt_auth::social::core::config::ProviderConfig;
use reinhardt_auth::social::providers::MicrosoftProvider;
use rstest::*;

#[tokio::test]
async fn test_microsoft_provider_config() {
	// Arrange
	let config = ProviderConfig::microsoft(
		"test_client_id".into(),
		"test_client_secret".into(),
		"http://localhost:8080/callback".into(),
		"common".into(),
	);

	// Assert
	assert_eq!(config.name, "microsoft");
	assert!(config.oidc.is_some());
	assert!(config.oauth2.is_none());
}

#[tokio::test]
async fn test_microsoft_provider_scopes() {
	// Arrange
	let config = ProviderConfig::microsoft(
		"test_client_id".into(),
		"test_client_secret".into(),
		"http://localhost:8080/callback".into(),
		"common".into(),
	);

	// Assert
	assert!(config.scopes.contains(&"openid".to_string()));
	assert!(config.scopes.contains(&"email".to_string()));
	assert!(config.scopes.contains(&"profile".to_string()));
}

#[tokio::test]
async fn test_microsoft_tenant_specific_discovery() {
	// Arrange
	let config = ProviderConfig::microsoft(
		"test_client_id".into(),
		"test_client_secret".into(),
		"http://localhost:8080/callback".into(),
		"mytenant".into(),
	);
	let oidc = config.oidc.unwrap();

	// Assert
	assert!(oidc.discovery_url.contains("mytenant"));
	assert!(oidc.discovery_url.contains("login.microsoftonline.com"));
}

#[tokio::test]
async fn test_microsoft_provider_create_succeeds() {
	// Arrange
	let config = ProviderConfig::microsoft(
		"test_client_id".into(),
		"test_client_secret".into(),
		"http://localhost:8080/callback".into(),
		"common".into(),
	);

	// Act
	let result = MicrosoftProvider::new(config).await;

	// Assert
	assert!(
		result.is_ok(),
		"Microsoft provider should be created successfully"
	);
	let provider = result.unwrap();
	assert_eq!(provider.name(), "microsoft");
	assert!(provider.is_oidc());
}

#[tokio::test]
async fn test_microsoft_provider_requires_oidc_config() {
	// Arrange - Create config without OIDC
	let config = ProviderConfig::github(
		"test_client_id".into(),
		"test_client_secret".into(),
		"http://localhost:8080/callback".into(),
	);

	// Act
	let result = MicrosoftProvider::new(config).await;

	// Assert
	assert!(
		result.is_err(),
		"Microsoft provider should fail without OIDC config"
	);
}

#[tokio::test]
async fn test_microsoft_is_oidc() {
	// Arrange
	let config = ProviderConfig::microsoft(
		"test_client_id".into(),
		"test_client_secret".into(),
		"http://localhost:8080/callback".into(),
		"common".into(),
	);

	// Assert
	assert!(config.oidc.is_some(), "Microsoft uses OIDC");
	assert!(config.oauth2.is_none(), "Microsoft does not use OAuth2");
}
