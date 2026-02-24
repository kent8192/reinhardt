//! OIDC discovery integration tests

use helpers::mock_server::MockOAuth2Server;
use reinhardt_auth::social::core::OAuth2Client;
use reinhardt_auth::social::oidc::{DiscoveryClient, OIDCDiscovery};
use rstest::*;

#[path = "../helpers.rs"]
mod helpers;

#[tokio::test]
async fn test_discovery_fetch_from_mock_server() {
	// Arrange
	let server = MockOAuth2Server::new().await;
	let oauth2_client = OAuth2Client::new();
	let client = DiscoveryClient::new(oauth2_client);

	// Act
	let result = client.discover(&server.base_url()).await;

	// Assert
	assert!(result.is_ok(), "Discovery should succeed with mock server");
	let discovery = result.unwrap();
	assert!(!discovery.issuer.is_empty());
	assert!(!discovery.authorization_endpoint.is_empty());
	assert!(!discovery.token_endpoint.is_empty());
	assert!(!discovery.jwks_uri.is_empty());
}

#[tokio::test]
async fn test_discovery_caching() {
	// Arrange
	let server = MockOAuth2Server::new().await;
	let oauth2_client = OAuth2Client::new();
	let client = DiscoveryClient::new(oauth2_client);

	// Act - Fetch twice (second should be cached)
	let result1 = client.discover(&server.base_url()).await;
	let result2 = client.discover(&server.base_url()).await;

	// Assert
	assert!(result1.is_ok());
	assert!(result2.is_ok());
	assert_eq!(result1.unwrap().issuer, result2.unwrap().issuer);
}

#[tokio::test]
async fn test_discovery_server_error() {
	// Arrange
	let mut server = MockOAuth2Server::new().await;
	server.set_error_mode(helpers::mock_server::ErrorMode::ServerError);
	let oauth2_client = OAuth2Client::new();
	let client = DiscoveryClient::new(oauth2_client);

	// Act
	let result = client.discover(&server.base_url()).await;

	// Assert
	assert!(result.is_err(), "Discovery should fail on server error");
}

#[tokio::test]
async fn test_discovery_oidc_disabled() {
	// Arrange
	let server = MockOAuth2Server::new().await.without_oidc();
	let oauth2_client = OAuth2Client::new();
	let client = DiscoveryClient::new(oauth2_client);

	// Act
	let result = client.discover(&server.base_url()).await;

	// Assert
	assert!(
		result.is_err(),
		"Discovery should fail when OIDC is disabled"
	);
}

#[tokio::test]
async fn test_discovery_document_structure() {
	// Arrange
	let discovery = OIDCDiscovery {
		issuer: "https://accounts.google.com".to_string(),
		authorization_endpoint: "https://accounts.google.com/o/oauth2/v2/auth".to_string(),
		token_endpoint: "https://oauth2.googleapis.com/token".to_string(),
		jwks_uri: "https://www.googleapis.com/oauth2/v3/certs".to_string(),
		userinfo_endpoint: Some("https://www.googleapis.com/oauth2/v3/userinfo".to_string()),
		scopes_supported: Some(vec![
			"openid".to_string(),
			"email".to_string(),
			"profile".to_string(),
		]),
		response_types_supported: Some(vec!["code".to_string()]),
		grant_types_supported: Some(vec!["authorization_code".to_string()]),
		subject_types_supported: Some(vec!["public".to_string()]),
		id_token_signing_alg_values_supported: Some(vec!["RS256".to_string()]),
		claims_supported: None,
	};

	// Assert
	assert!(!discovery.issuer.is_empty());
	assert!(!discovery.authorization_endpoint.is_empty());
	assert!(!discovery.token_endpoint.is_empty());
	assert!(!discovery.jwks_uri.is_empty());
	assert!(discovery.scopes_supported.is_some());
}

#[test]
fn test_discovery_serialization() {
	// Arrange
	let discovery = OIDCDiscovery {
		issuer: "https://accounts.google.com".to_string(),
		authorization_endpoint: "https://accounts.google.com/o/oauth2/v2/auth".to_string(),
		token_endpoint: "https://oauth2.googleapis.com/token".to_string(),
		jwks_uri: "https://www.googleapis.com/oauth2/v3/certs".to_string(),
		userinfo_endpoint: Some("https://www.googleapis.com/oauth2/v3/userinfo".to_string()),
		scopes_supported: Some(vec![
			"openid".to_string(),
			"email".to_string(),
			"profile".to_string(),
		]),
		response_types_supported: Some(vec!["code".to_string()]),
		grant_types_supported: Some(vec!["authorization_code".to_string()]),
		subject_types_supported: Some(vec!["public".to_string()]),
		id_token_signing_alg_values_supported: Some(vec!["RS256".to_string()]),
		claims_supported: None,
	};

	// Act
	let json = serde_json::to_string(&discovery).unwrap();

	// Assert
	assert!(json.contains("\"issuer\":\"https://accounts.google.com\""));
	assert!(json.contains("\"jwks_uri\":\"https://www.googleapis.com/oauth2/v3/certs\""));
}

#[test]
fn test_discovery_deserialization() {
	// Arrange
	let json = r#"{
		"issuer": "https://accounts.google.com",
		"authorization_endpoint": "https://accounts.google.com/o/oauth2/v2/auth",
		"token_endpoint": "https://oauth2.googleapis.com/token",
		"jwks_uri": "https://www.googleapis.com/oauth2/v3/certs",
		"userinfo_endpoint": "https://www.googleapis.com/oauth2/v3/userinfo",
		"scopes_supported": ["openid", "email", "profile"]
	}"#;

	// Act
	let discovery: OIDCDiscovery = serde_json::from_str(json).unwrap();

	// Assert
	assert_eq!(discovery.issuer, "https://accounts.google.com");
	assert_eq!(
		discovery.authorization_endpoint,
		"https://accounts.google.com/o/oauth2/v2/auth"
	);
	assert!(discovery.scopes_supported.is_some());
}
