//! Social authentication error types

use thiserror::Error;

/// Social authentication errors
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SocialAuthError {
	/// Network error during HTTP requests
	#[error("Network error: {0}")]
	Network(String),

	/// Invalid response from provider
	#[error("Invalid response: {0}")]
	InvalidResponse(String),

	/// Token validation failed
	#[error("Token validation failed: {0}")]
	TokenValidation(String),

	/// JWKS (JSON Web Key Set) error
	#[error("JWKS error: {0}")]
	Jwks(String),

	/// OIDC Discovery error
	#[error("Discovery error: {0}")]
	Discovery(String),

	/// State parameter validation failed (CSRF protection)
	#[error("State validation failed: {0}")]
	StateValidation(String),

	/// PKCE validation failed
	#[error("PKCE validation failed: {0}")]
	PkceValidation(String),

	/// Provider-specific error
	#[error("Provider error: {0}")]
	Provider(String),

	/// Configuration error
	#[error("Configuration error: {0}")]
	Configuration(String),

	/// User mapping error
	#[error("User mapping error: {0}")]
	UserMapping(String),

	/// Storage error
	#[error("Storage error: {0}")]
	Storage(String),

	/// Feature not supported
	#[error("Not supported: {0}")]
	NotSupported(String),

	/// Invalid OAuth2 state parameter
	#[error("Invalid state")]
	InvalidState,

	/// Token exchange error
	#[error("Token exchange error: {0}")]
	TokenExchangeError(String),

	/// Token refresh error
	#[error("Token refresh error: {0}")]
	TokenRefreshError(String),

	/// Invalid JWK (JSON Web Key)
	#[error("Invalid JWK: {0}")]
	InvalidJwk(String),

	/// Invalid ID token
	#[error("Invalid ID token: {0}")]
	InvalidIdToken(String),

	/// UserInfo endpoint error
	#[error("UserInfo error: {0}")]
	UserInfoError(String),

	/// Invalid configuration
	#[error("Invalid configuration: {0}")]
	InvalidConfiguration(String),

	/// Unknown error
	#[error("Unknown error: {0}")]
	Unknown(String),
}

/// Conversion from reqwest::Error
impl From<reqwest::Error> for SocialAuthError {
	fn from(error: reqwest::Error) -> Self {
		SocialAuthError::Network(error.to_string())
	}
}

/// Conversion from serde_json::Error
impl From<serde_json::Error> for SocialAuthError {
	fn from(error: serde_json::Error) -> Self {
		SocialAuthError::InvalidResponse(error.to_string())
	}
}

/// Conversion from jsonwebtoken::errors::Error
impl From<jsonwebtoken::errors::Error> for SocialAuthError {
	fn from(error: jsonwebtoken::errors::Error) -> Self {
		SocialAuthError::TokenValidation(error.to_string())
	}
}

/// Conversion to reinhardt_exception::Error
impl From<SocialAuthError> for crate::AuthenticationError {
	fn from(error: SocialAuthError) -> Self {
		match error {
			SocialAuthError::Network(msg) => crate::AuthenticationError::Unknown(msg),
			SocialAuthError::InvalidResponse(msg) => crate::AuthenticationError::Unknown(msg),
			SocialAuthError::TokenValidation(_) => crate::AuthenticationError::InvalidToken,
			SocialAuthError::StateValidation(_) => crate::AuthenticationError::InvalidToken,
			_ => crate::AuthenticationError::Unknown(error.to_string()),
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_error_display() {
		let error = SocialAuthError::Network("Connection timeout".to_string());
		assert_eq!(error.to_string(), "Network error: Connection timeout");

		let error = SocialAuthError::TokenValidation("Invalid signature".to_string());
		assert_eq!(
			error.to_string(),
			"Token validation failed: Invalid signature"
		);

		let error = SocialAuthError::Configuration("Missing client_id".to_string());
		assert_eq!(error.to_string(), "Configuration error: Missing client_id");
	}

	// Note: Testing reqwest::Error conversion is difficult because reqwest::Error
	// cannot be easily constructed in tests. The conversion implementation itself
	// is trivial (a direct wrapper), so we skip this test.

	#[test]
	fn test_error_from_serde_json() {
		// Simulate a serde_json error
		let json_error = serde_json::from_str::<serde_json::Value>("{invalid json}").unwrap_err();
		let social_error: SocialAuthError = json_error.into();

		assert!(matches!(social_error, SocialAuthError::InvalidResponse(_)));
	}

	#[test]
	fn test_error_to_authentication_error() {
		let social_error = SocialAuthError::TokenValidation("Bad token".to_string());
		let auth_error: crate::AuthenticationError = social_error.into();

		assert!(matches!(
			auth_error,
			crate::AuthenticationError::InvalidToken
		));
	}
}
