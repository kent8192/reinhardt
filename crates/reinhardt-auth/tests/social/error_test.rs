//! Error handling tests

use reinhardt_auth::social::core::SocialAuthError;
use rstest::*;

#[test]
fn test_error_network() {
	// Arrange & Act
	let error = SocialAuthError::Network("Connection timeout".to_string());

	// Assert
	assert_eq!(error.to_string(), "Network error: Connection timeout");
}

#[test]
fn test_error_invalid_response() {
	// Arrange & Act
	let error = SocialAuthError::InvalidResponse("Malformed JSON".to_string());

	// Assert
	assert_eq!(error.to_string(), "Invalid response: Malformed JSON");
}

#[test]
fn test_error_token_validation() {
	// Arrange & Act
	let error = SocialAuthError::TokenValidation("Invalid signature".to_string());

	// Assert
	assert_eq!(
		error.to_string(),
		"Token validation failed: Invalid signature"
	);
}

#[test]
fn test_error_jwks() {
	// Arrange & Act
	let error = SocialAuthError::Jwks("Key not found".to_string());

	// Assert
	assert_eq!(error.to_string(), "JWKS error: Key not found");
}

#[test]
fn test_error_discovery() {
	// Arrange & Act
	let error = SocialAuthError::Discovery("Discovery document not found".to_string());

	// Assert
	assert_eq!(
		error.to_string(),
		"Discovery error: Discovery document not found"
	);
}

#[test]
fn test_error_state_validation() {
	// Arrange & Act
	let error = SocialAuthError::StateValidation("Invalid state parameter".to_string());

	// Assert
	assert_eq!(
		error.to_string(),
		"State validation failed: Invalid state parameter"
	);
}

#[test]
fn test_error_pkce_validation() {
	// Arrange & Act
	let error = SocialAuthError::PkceValidation("Invalid code verifier".to_string());

	// Assert
	assert_eq!(
		error.to_string(),
		"PKCE validation failed: Invalid code verifier"
	);
}

#[test]
fn test_error_provider() {
	// Arrange & Act
	let error = SocialAuthError::Provider("Provider unavailable".to_string());

	// Assert
	assert_eq!(error.to_string(), "Provider error: Provider unavailable");
}

#[test]
fn test_error_configuration() {
	// Arrange & Act
	let error = SocialAuthError::Configuration("Missing client_id".to_string());

	// Assert
	assert_eq!(error.to_string(), "Configuration error: Missing client_id");
}

#[test]
fn test_error_invalid_state() {
	// Arrange & Act
	let error = SocialAuthError::InvalidState;

	// Assert
	assert_eq!(error.to_string(), "Invalid state");
}

#[test]
fn test_error_token_exchange_error() {
	// Arrange & Act
	let error = SocialAuthError::TokenExchangeError("Invalid authorization code".to_string());

	// Assert
	assert_eq!(
		error.to_string(),
		"Token exchange error: Invalid authorization code"
	);
}

#[test]
fn test_error_token_refresh_error() {
	// Arrange & Act
	let error = SocialAuthError::TokenRefreshError("Refresh token expired".to_string());

	// Assert
	assert_eq!(
		error.to_string(),
		"Token refresh error: Refresh token expired"
	);
}

#[test]
fn test_error_invalid_jwk() {
	// Arrange & Act
	let error = SocialAuthError::InvalidJwk("Invalid key format".to_string());

	// Assert
	assert_eq!(error.to_string(), "Invalid JWK: Invalid key format");
}

#[test]
fn test_error_invalid_id_token() {
	// Arrange & Act
	let error = SocialAuthError::InvalidIdToken("Expired token".to_string());

	// Assert
	assert_eq!(error.to_string(), "Invalid ID token: Expired token");
}

#[test]
fn test_error_from_serde_json() {
	// Arrange
	let json_error = serde_json::from_str::<serde_json::Value>("{invalid json}").unwrap_err();

	// Act
	let social_error: SocialAuthError = json_error.into();

	// Assert
	assert!(matches!(social_error, SocialAuthError::InvalidResponse(_)));
}

#[test]
fn test_error_to_authentication_error() {
	// Arrange
	let social_error = SocialAuthError::TokenValidation("Bad token".to_string());

	// Act
	let auth_error = reinhardt_auth::AuthenticationError::from(social_error);

	// Assert
	assert!(matches!(
		auth_error,
		reinhardt_auth::AuthenticationError::InvalidToken
	));
}

#[test]
fn test_error_serialization() {
	// Arrange
	let error = SocialAuthError::Configuration("Missing client_id".to_string());

	// Act - Some error types can be serialized
	let display = error.to_string();

	// Assert
	assert!(display.contains("Configuration"));
	assert!(display.contains("Missing client_id"));
}

#[test]
fn test_error_unknown() {
	// Arrange & Act
	let error = SocialAuthError::Unknown("Unknown error occurred".to_string());

	// Assert
	assert_eq!(error.to_string(), "Unknown error: Unknown error occurred");
}

#[test]
fn test_error_not_supported() {
	// Arrange & Act
	let error = SocialAuthError::NotSupported("UserInfo endpoint not supported".to_string());

	// Assert
	assert_eq!(
		error.to_string(),
		"Not supported: UserInfo endpoint not supported"
	);
}

#[test]
fn test_error_user_mapping() {
	// Arrange & Act
	let error = SocialAuthError::UserMapping("Cannot map user".to_string());

	// Assert
	assert_eq!(error.to_string(), "User mapping error: Cannot map user");
}

#[test]
fn test_error_storage() {
	// Arrange & Act
	let error = SocialAuthError::Storage("Database error".to_string());

	// Assert
	assert_eq!(error.to_string(), "Storage error: Database error");
}

#[test]
fn test_error_user_info() {
	// Arrange & Act
	let error = SocialAuthError::UserInfoError("UserInfo endpoint error".to_string());

	// Assert
	assert_eq!(error.to_string(), "UserInfo error: UserInfo endpoint error");
}

#[test]
fn test_error_invalid_configuration() {
	// Arrange & Act
	let error = SocialAuthError::InvalidConfiguration("Invalid redirect URI".to_string());

	// Assert
	assert_eq!(
		error.to_string(),
		"Invalid configuration: Invalid redirect URI"
	);
}
