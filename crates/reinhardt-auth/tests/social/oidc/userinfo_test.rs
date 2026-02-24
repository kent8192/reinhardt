//! UserInfo endpoint tests

use helpers::mock_server::MockOAuth2Server;
use reinhardt_auth::social::core::OAuth2Client;
use reinhardt_auth::social::core::claims::StandardClaims;
use reinhardt_auth::social::oidc::UserInfoClient;
use rstest::*;
use std::collections::HashMap;

#[path = "../../helpers.rs"]
mod helpers;

#[tokio::test]
async fn test_userinfo_retrieve_claims() {
	// Arrange
	let server = MockOAuth2Server::new().await;
	let oauth2_client = OAuth2Client::new();
	let client = UserInfoClient::new(oauth2_client);
	let userinfo_url = server.userinfo_url().unwrap();

	// Act
	let result = client
		.get_user_info(&userinfo_url, "test_access_token")
		.await;

	// Assert
	assert!(result.is_ok(), "UserInfo should succeed with mock server");
	let claims = result.unwrap();
	assert_eq!(claims.sub, "test_user");
	assert_eq!(claims.email, Some("test@example.com".to_string()));
	assert_eq!(claims.email_verified, Some(true));
	assert_eq!(claims.name, Some("Test User".to_string()));
	assert_eq!(claims.given_name, Some("Test".to_string()));
	assert_eq!(claims.family_name, Some("User".to_string()));
}

#[tokio::test]
async fn test_userinfo_with_custom_claims() {
	// Arrange
	let mut server = MockOAuth2Server::new().await;
	let custom_claims = StandardClaims {
		sub: "custom_user_42".to_string(),
		email: Some("custom@example.com".to_string()),
		email_verified: Some(false),
		name: Some("Custom User".to_string()),
		given_name: None,
		family_name: None,
		picture: Some("https://example.com/photo.jpg".to_string()),
		locale: Some("ja".to_string()),
		additional_claims: HashMap::new(),
	};
	server.set_userinfo_response(custom_claims);

	let oauth2_client = OAuth2Client::new();
	let client = UserInfoClient::new(oauth2_client);
	let userinfo_url = server.userinfo_url().unwrap();

	// Act
	let result = client
		.get_user_info(&userinfo_url, "test_access_token")
		.await;

	// Assert
	assert!(result.is_ok(), "UserInfo should succeed with custom claims");
	let claims = result.unwrap();
	assert_eq!(claims.sub, "custom_user_42");
	assert_eq!(claims.email, Some("custom@example.com".to_string()));
	assert_eq!(claims.email_verified, Some(false));
	assert_eq!(
		claims.picture,
		Some("https://example.com/photo.jpg".to_string())
	);
	assert_eq!(claims.locale, Some("ja".to_string()));
}

#[tokio::test]
async fn test_userinfo_handle_endpoint_errors() {
	// Arrange
	let mut server = MockOAuth2Server::new().await;
	server.set_error_mode(helpers::mock_server::ErrorMode::ServerError);
	let oauth2_client = OAuth2Client::new();
	let client = UserInfoClient::new(oauth2_client);
	let userinfo_url = server.userinfo_url().unwrap();

	// Act
	let result = client
		.get_user_info(&userinfo_url, "test_access_token")
		.await;

	// Assert
	assert!(result.is_err(), "UserInfo should fail on server error");
}

#[tokio::test]
async fn test_userinfo_handle_not_found() {
	// Arrange
	let server = MockOAuth2Server::new().await.without_userinfo();
	let oauth2_client = OAuth2Client::new();
	let client = UserInfoClient::new(oauth2_client);
	let userinfo_url = server.userinfo_url().unwrap();

	// Act
	let result = client
		.get_user_info(&userinfo_url, "test_access_token")
		.await;

	// Assert
	assert!(
		result.is_err(),
		"UserInfo should fail when endpoint returns 404"
	);
}

#[test]
fn test_userinfo_parse_standard_claims() {
	// Arrange
	let json = r#"{
		"sub": "user123",
		"email": "user@example.com",
		"email_verified": true,
		"name": "Test User",
		"given_name": "Test",
		"family_name": "User",
		"picture": "https://example.com/photo.jpg"
	}"#;

	// Act
	let claims: StandardClaims = serde_json::from_str(json).unwrap();

	// Assert
	assert_eq!(claims.sub, "user123");
	assert_eq!(claims.email, Some("user@example.com".to_string()));
	assert_eq!(claims.email_verified, Some(true));
	assert_eq!(claims.name, Some("Test User".to_string()));
	assert_eq!(claims.given_name, Some("Test".to_string()));
	assert_eq!(claims.family_name, Some("User".to_string()));
	assert_eq!(
		claims.picture,
		Some("https://example.com/photo.jpg".to_string())
	);
}

#[test]
fn test_userinfo_extract_email_and_profile() {
	// Arrange
	let claims = StandardClaims {
		sub: "user123".to_string(),
		email: Some("user@example.com".to_string()),
		email_verified: Some(true),
		name: Some("Test User".to_string()),
		given_name: Some("Test".to_string()),
		family_name: Some("User".to_string()),
		picture: Some("https://example.com/photo.jpg".to_string()),
		locale: Some("en".to_string()),
		additional_claims: HashMap::new(),
	};

	// Assert
	assert_eq!(claims.email.as_deref(), Some("user@example.com"));
	assert_eq!(claims.email_verified, Some(true));
	assert_eq!(claims.name.as_deref(), Some("Test User"));
	assert_eq!(
		claims.picture.as_deref(),
		Some("https://example.com/photo.jpg")
	);
}

#[test]
fn test_userinfo_minimal_claims() {
	// Arrange
	let json = r#"{"sub": "user123"}"#;

	// Act
	let claims: StandardClaims = serde_json::from_str(json).unwrap();

	// Assert
	assert_eq!(claims.sub, "user123");
	assert!(claims.email.is_none());
	assert!(claims.name.is_none());
	assert!(claims.given_name.is_none());
	assert!(claims.family_name.is_none());
	assert!(claims.picture.is_none());
	assert!(claims.locale.is_none());
}

#[test]
fn test_userinfo_serialization() {
	// Arrange
	let claims = StandardClaims {
		sub: "user123".to_string(),
		email: Some("user@example.com".to_string()),
		email_verified: Some(true),
		name: Some("Test User".to_string()),
		given_name: None,
		family_name: None,
		picture: None,
		locale: None,
		additional_claims: HashMap::new(),
	};

	// Act
	let json = serde_json::to_string(&claims).unwrap();

	// Assert
	assert!(json.contains("\"sub\":\"user123\""));
	assert!(json.contains("\"email\":\"user@example.com\""));
}
