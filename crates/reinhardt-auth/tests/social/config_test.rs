//! Provider configuration tests

use reinhardt_auth::social::core::config::{OAuth2Config, OIDCConfig, ProviderConfig};
use rstest::*;

#[rstest]
#[case("google")]
#[case("github")]
#[case("microsoft")]
#[case("apple")]
fn test_provider_config_create(#[case] provider: &str) {
	// Act
	let config = match provider {
		"google" => ProviderConfig::google(
			"test_client_id".into(),
			"test_client_secret".into(),
			"http://localhost:8080/callback".into(),
		),
		"github" => ProviderConfig::github(
			"test_client_id".into(),
			"test_client_secret".into(),
			"http://localhost:8080/callback".into(),
		),
		"microsoft" => ProviderConfig::microsoft(
			"test_client_id".into(),
			"test_client_secret".into(),
			"http://localhost:8080/callback".into(),
			"common".into(),
		),
		"apple" => ProviderConfig::apple(
			"test_client_id".into(),
			"test_client_secret_jwt".into(),
			"http://localhost:8080/callback".into(),
		),
		_ => panic!("Unknown provider"),
	};

	// Assert
	assert_eq!(config.name, provider);
	assert_eq!(config.client_id, "test_client_id");
	assert!(!config.scopes.is_empty());
}

#[test]
fn test_google_config_is_oidc() {
	// Act
	let config = ProviderConfig::google(
		"test_client_id".into(),
		"test_client_secret".into(),
		"http://localhost:8080/callback".into(),
	);

	// Assert
	assert!(config.oidc.is_some(), "Google should use OIDC");
	assert!(config.oauth2.is_none(), "Google should not use OAuth2");
	assert!(
		config.oidc.unwrap().use_nonce,
		"OIDC should use nonce by default"
	);
}

#[test]
fn test_github_config_is_oauth2() {
	// Act
	let config = ProviderConfig::github(
		"test_client_id".into(),
		"test_client_secret".into(),
		"http://localhost:8080/callback".into(),
	);

	// Assert
	assert!(config.oauth2.is_some(), "GitHub should use OAuth2");
	assert!(config.oidc.is_none(), "GitHub should not use OIDC");
}

#[test]
fn test_microsoft_config_is_oidc() {
	// Act
	let config = ProviderConfig::microsoft(
		"test_client_id".into(),
		"test_client_secret".into(),
		"http://localhost:8080/callback".into(),
		"common".into(),
	);

	// Assert
	assert!(config.oidc.is_some(), "Microsoft should use OIDC");
	assert!(config.oauth2.is_none(), "Microsoft should not use OAuth2");
	assert!(
		config.oidc.unwrap().discovery_url.contains("common"),
		"Discovery URL should contain tenant"
	);
}

#[test]
fn test_apple_config_is_oidc() {
	// Act
	let config = ProviderConfig::apple(
		"test_client_id".into(),
		"test_client_secret_jwt".into(),
		"http://localhost:8080/callback".into(),
	);

	// Assert
	assert!(config.oidc.is_some(), "Apple should use OIDC");
	assert!(config.oauth2.is_none(), "Apple should not use OAuth2");
}

#[test]
fn test_config_serialization() {
	// Arrange
	let config = ProviderConfig::google(
		"test_client_id".into(),
		"test_client_secret".into(),
		"http://localhost:8080/callback".into(),
	);

	// Act
	let json = serde_json::to_string(&config).unwrap();
	let deserialized: ProviderConfig = serde_json::from_str(&json).unwrap();

	// Assert
	assert_eq!(deserialized.name, config.name);
	assert_eq!(deserialized.client_id, config.client_id);
	assert_eq!(deserialized.redirect_uri, config.redirect_uri);
}

#[test]
fn test_config_scopes() {
	// Arrange & Act
	let google_config = ProviderConfig::google(
		"test_client_id".into(),
		"test_client_secret".into(),
		"http://localhost:8080/callback".into(),
	);

	let github_config = ProviderConfig::github(
		"test_client_id".into(),
		"test_client_secret".into(),
		"http://localhost:8080/callback".into(),
	);

	// Assert
	assert!(google_config.scopes.contains(&"openid".to_string()));
	assert!(google_config.scopes.contains(&"email".to_string()));
	assert!(github_config.scopes.contains(&"user".to_string()));
}

#[test]
fn test_oauth2_config_endpoints() {
	// Arrange
	let oauth2_config = OAuth2Config {
		authorization_endpoint: "https://example.com/auth".into(),
		token_endpoint: "https://example.com/token".into(),
		userinfo_endpoint: Some("https://example.com/userinfo".into()),
	};

	// Assert
	assert_eq!(
		oauth2_config.authorization_endpoint,
		"https://example.com/auth"
	);
	assert_eq!(oauth2_config.token_endpoint, "https://example.com/token");
	assert_eq!(
		oauth2_config.userinfo_endpoint,
		Some("https://example.com/userinfo".into())
	);
}

#[test]
fn test_oidc_config_discovery_url() {
	// Arrange
	let oidc_config = OIDCConfig {
		discovery_url: "https://accounts.google.com/.well-known/openid-configuration".to_string(),
		use_nonce: true,
	};

	// Assert
	assert!(
		oidc_config
			.discovery_url
			.contains(".well-known/openid-configuration")
	);
	assert!(oidc_config.use_nonce);
}
