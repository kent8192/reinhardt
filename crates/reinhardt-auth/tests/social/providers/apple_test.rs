//! Apple OIDC provider tests

use reinhardt_auth::social::core::OAuthProvider;
use reinhardt_auth::social::core::config::ProviderConfig;
use reinhardt_auth::social::providers::AppleProvider;
use rstest::*;

#[tokio::test]
async fn test_apple_provider_config() {
	// Arrange
	let config = ProviderConfig::apple(
		"test_client_id".into(),
		"test_client_secret_jwt".into(),
		"http://localhost:8080/callback".into(),
	);

	// Assert
	assert_eq!(config.name, "apple");
	assert!(config.oidc.is_some());
	assert!(config.oauth2.is_none());
}

#[tokio::test]
async fn test_apple_provider_scopes() {
	// Arrange
	let config = ProviderConfig::apple(
		"test_client_id".into(),
		"test_client_secret_jwt".into(),
		"http://localhost:8080/callback".into(),
	);

	// Assert
	assert!(config.scopes.contains(&"openid".to_string()));
	assert!(config.scopes.contains(&"email".to_string()));
	assert!(config.scopes.contains(&"name".to_string()));
}

#[tokio::test]
async fn test_apple_oidc_discovery_url() {
	// Arrange
	let config = ProviderConfig::apple(
		"test_client_id".into(),
		"test_client_secret_jwt".into(),
		"http://localhost:8080/callback".into(),
	);
	let oidc = config.oidc.unwrap();

	// Assert
	assert!(oidc.discovery_url.contains("appleid.apple.com"));
	assert!(
		oidc.discovery_url
			.contains(".well-known/openid-configuration")
	);
}

#[tokio::test]
async fn test_apple_provider_create_succeeds() {
	// Arrange
	let config = ProviderConfig::apple(
		"test_client_id".into(),
		"test_client_secret_jwt".into(),
		"http://localhost:8080/callback".into(),
	);

	// Act
	let result = AppleProvider::new(config).await;

	// Assert
	assert!(
		result.is_ok(),
		"Apple provider should be created successfully"
	);
	let provider = result.unwrap();
	assert_eq!(provider.name(), "apple");
	assert!(provider.is_oidc());
}

#[tokio::test]
async fn test_apple_provider_rejects_empty_client_secret() {
	// Arrange - Create config with empty client_secret
	let config = ProviderConfig {
		name: "apple".to_string(),
		client_id: "test_client_id".to_string(),
		client_secret: String::new(),
		redirect_uri: "http://localhost:8080/callback".to_string(),
		scopes: vec!["openid".to_string()],
		oidc: Some(reinhardt_auth::social::core::config::OIDCConfig {
			discovery_url: "https://appleid.apple.com/.well-known/openid-configuration".to_string(),
			use_nonce: true,
		}),
		oauth2: None,
	};

	// Act
	let result = AppleProvider::new(config).await;

	// Assert
	assert!(
		result.is_err(),
		"Apple provider should reject empty client_secret"
	);
}

#[tokio::test]
async fn test_apple_no_userinfo_endpoint() {
	// Arrange
	let config = ProviderConfig::apple(
		"test_client_id".into(),
		"test_client_secret_jwt".into(),
		"http://localhost:8080/callback".into(),
	);

	// Act
	let provider = AppleProvider::new(config).await.unwrap();
	let result = provider.get_user_info("test_token").await;

	// Assert - Apple does not support UserInfo endpoint
	assert!(result.is_err());
}

#[tokio::test]
async fn test_apple_is_oidc() {
	// Arrange
	let config = ProviderConfig::apple(
		"test_client_id".into(),
		"test_client_secret_jwt".into(),
		"http://localhost:8080/callback".into(),
	);

	// Assert
	assert!(config.oidc.is_some(), "Apple uses OIDC");
	assert!(config.oauth2.is_none(), "Apple does not use OAuth2");
}
