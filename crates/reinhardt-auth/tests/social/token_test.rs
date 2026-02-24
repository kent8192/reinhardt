//! Token handling tests

use chrono::{Duration, Utc};
use reinhardt_auth::social::core::token::{OAuthToken, TokenResponse};
use rstest::*;

#[test]
fn test_token_response_from_response() {
	// Arrange
	let response = TokenResponse {
		access_token: "test_access_token".to_string(),
		token_type: "Bearer".to_string(),
		expires_in: Some(3600),
		refresh_token: Some("test_refresh_token".to_string()),
		scope: Some("openid email profile".to_string()),
		id_token: Some("test_id_token".to_string()),
	};

	// Assert
	assert_eq!(response.access_token, "test_access_token");
	assert_eq!(response.token_type, "Bearer");
	assert_eq!(response.expires_in, Some(3600));
	assert_eq!(
		response.refresh_token,
		Some("test_refresh_token".to_string())
	);
}

#[test]
fn test_token_response_minimal() {
	// Arrange
	let response = TokenResponse {
		access_token: "test_access_token".to_string(),
		token_type: "Bearer".to_string(),
		expires_in: None,
		refresh_token: None,
		scope: None,
		id_token: None,
	};

	// Assert
	assert_eq!(response.access_token, "test_access_token");
	assert_eq!(response.token_type, "Bearer");
	assert!(response.expires_in.is_none());
}

#[test]
fn test_token_response_serialization() {
	// Arrange
	let response = TokenResponse {
		access_token: "test_token".to_string(),
		token_type: "Bearer".to_string(),
		expires_in: Some(3600),
		refresh_token: Some("refresh".to_string()),
		scope: Some("openid email".to_string()),
		id_token: None,
	};

	// Act
	let json = serde_json::to_string(&response).unwrap();
	let parsed: TokenResponse = serde_json::from_str(&json).unwrap();

	// Assert
	assert_eq!(parsed.access_token, response.access_token);
	assert_eq!(parsed.token_type, response.token_type);
}

#[test]
fn test_oauth_token_expiration_calculation() {
	// Arrange
	let response = TokenResponse {
		access_token: "test_token".to_string(),
		token_type: "Bearer".to_string(),
		expires_in: Some(3600), // 1 hour
		refresh_token: None,
		scope: None,
		id_token: None,
	};

	// Act
	let expires_at = if let Some(seconds) = response.expires_in {
		Utc::now() + Duration::seconds(seconds as i64)
	} else {
		Utc::now() + Duration::hours(1) // Default
	};

	// Assert
	let time_diff = expires_at - Utc::now();
	assert!(time_diff.num_seconds() >= 3590 && time_diff.num_seconds() <= 3610);
}

#[test]
fn test_oauth_token_from_response_with_id_token() {
	// Arrange
	let response = TokenResponse {
		access_token: "access_token".to_string(),
		token_type: "Bearer".to_string(),
		expires_in: Some(3600),
		refresh_token: Some("refresh_token".to_string()),
		scope: Some("openid email".to_string()),
		id_token: Some("id_token_string".to_string()),
	};

	// Assert
	assert!(response.id_token.is_some());
	assert_eq!(response.id_token.unwrap(), "id_token_string");
}

#[test]
fn test_token_response_parse_scopes() {
	// Arrange
	let response = TokenResponse {
		access_token: "test_token".to_string(),
		token_type: "Bearer".to_string(),
		expires_in: None,
		refresh_token: None,
		scope: Some("openid email profile".to_string()),
		id_token: None,
	};

	// Act
	let scopes: Vec<&str> = response
		.scope
		.as_ref()
		.unwrap()
		.split_whitespace()
		.collect();

	// Assert
	assert_eq!(scopes.len(), 3);
	assert!(scopes.contains(&"openid"));
	assert!(scopes.contains(&"email"));
	assert!(scopes.contains(&"profile"));
}

#[test]
fn test_token_response_empty_scope() {
	// Arrange
	let response = TokenResponse {
		access_token: "test_token".to_string(),
		token_type: "Bearer".to_string(),
		expires_in: None,
		refresh_token: None,
		scope: Some("".to_string()),
		id_token: None,
	};

	// Act
	let scopes: Vec<&str> = response
		.scope
		.as_ref()
		.unwrap()
		.split_whitespace()
		.collect();

	// Assert
	assert!(scopes.is_empty());
}
