//! Test fixtures for social authentication tests

use reinhardt_auth::social::core::{
	claims::{IdToken, StandardClaims},
	config::{OAuth2Config, OIDCConfig, ProviderConfig},
	token::TokenResponse,
};
use std::collections::HashMap;

/// Test fixture builder
pub struct TestFixtures;

impl TestFixtures {
	// ============================================================
	// GitHub OAuth2 Fixtures
	// ============================================================

	/// Create GitHub provider configuration
	pub fn github_config() -> ProviderConfig {
		ProviderConfig::github(
			"test_github_client_id".into(),
			"test_github_client_secret".into(),
			"http://localhost:8080/callback".into(),
		)
	}

	/// Create GitHub token response
	pub fn github_token_response() -> TokenResponse {
		TokenResponse {
			access_token: "gho_test_token".into(),
			token_type: "Bearer".into(),
			expires_in: None,
			refresh_token: None,
			scope: Some("user,user:email".into()),
			id_token: None,
		}
	}

	/// Create GitHub user info
	pub fn github_userinfo() -> StandardClaims {
		StandardClaims {
			sub: "github_user_123".into(),
			email: Some("user@example.com".into()),
			email_verified: None,
			name: Some("Test User".into()),
			given_name: None,
			family_name: None,
			picture: Some("https://avatar.example.com".into()),
			locale: None,
			additional_claims: HashMap::new(),
		}
	}

	// ============================================================
	// Google OIDC Fixtures
	// ============================================================

	/// Create Google provider configuration
	pub fn google_config() -> ProviderConfig {
		ProviderConfig::google(
			"test_google_client_id".into(),
			"test_google_client_secret".into(),
			"http://localhost:8080/callback".into(),
		)
	}

	/// Create Google token response
	pub fn google_token_response() -> TokenResponse {
		TokenResponse {
			access_token: "ya29.test_token".into(),
			token_type: "Bearer".into(),
			expires_in: Some(3600),
			refresh_token: Some("refresh_token".into()),
			scope: Some("openid email profile".into()),
			id_token: Some("test_id_token".into()),
		}
	}

	/// Create Google ID token
	pub fn google_id_token() -> IdToken {
		IdToken {
			sub: "google_user_123".into(),
			iss: "https://accounts.google.com".into(),
			aud: "test_google_client_id".into(),
			exp: 1735636800,
			iat: 1735633200,
			nonce: Some("test_nonce".into()),
			email: Some("user@example.com".into()),
			email_verified: Some(true),
			name: Some("Test User".into()),
			given_name: Some("Test".into()),
			family_name: Some("User".into()),
			picture: Some("https://lh3.googleusercontent.com/photo.jpg".into()),
			locale: Some("en".into()),
			additional_claims: HashMap::new(),
		}
	}

	/// Create Google user info
	pub fn google_userinfo() -> StandardClaims {
		StandardClaims {
			sub: "google_user_123".into(),
			email: Some("user@gmail.com".into()),
			email_verified: Some(true),
			name: Some("Test User".into()),
			given_name: Some("Test".into()),
			family_name: Some("User".into()),
			picture: Some("https://lh3.googleusercontent.com/photo.jpg".into()),
			locale: Some("en".into()),
			additional_claims: HashMap::new(),
		}
	}

	// ============================================================
	// Microsoft OIDC Fixtures
	// ============================================================

	/// Create Microsoft provider configuration
	pub fn microsoft_config(tenant: &str) -> ProviderConfig {
		ProviderConfig::microsoft(
			"test_microsoft_client_id".into(),
			"test_microsoft_client_secret".into(),
			"http://localhost:8080/callback".into(),
			tenant.into(),
		)
	}

	/// Create Microsoft token response
	pub fn microsoft_token_response() -> TokenResponse {
		TokenResponse {
			access_token: "EwAgA8l6BAAU".into(),
			token_type: "Bearer".into(),
			expires_in: Some(3600),
			refresh_token: Some("refresh_token".into()),
			scope: Some("openid email profile".into()),
			id_token: Some("test_id_token".into()),
		}
	}

	/// Create Microsoft ID token
	pub fn microsoft_id_token() -> IdToken {
		IdToken {
			sub: "microsoft_user_123".into(),
			iss: "https://login.microsoftonline.com/common/v2.0".into(),
			aud: "test_microsoft_client_id".into(),
			exp: 1735636800,
			iat: 1735633200,
			nonce: Some("test_nonce".into()),
			email: Some("user@outlook.com".into()),
			email_verified: Some(true),
			name: Some("Test User".into()),
			given_name: Some("Test".into()),
			family_name: Some("User".into()),
			picture: None,
			locale: None,
			additional_claims: HashMap::new(),
		}
	}

	/// Create Microsoft user info
	pub fn microsoft_userinfo() -> StandardClaims {
		StandardClaims {
			sub: "microsoft_user_123".into(),
			email: Some("user@outlook.com".into()),
			email_verified: Some(true),
			name: Some("Test User".into()),
			given_name: Some("Test".into()),
			family_name: Some("User".into()),
			picture: None,
			locale: None,
			additional_claims: HashMap::new(),
		}
	}

	// ============================================================
	// Apple OIDC Fixtures
	// ============================================================

	/// Create Apple provider configuration
	pub fn apple_config() -> ProviderConfig {
		ProviderConfig::apple(
			"test_apple_client_id".into(),
			"test_apple_client_secret_jwt".into(),
			"http://localhost:8080/callback".into(),
		)
	}

	/// Create Apple token response
	pub fn apple_token_response() -> TokenResponse {
		TokenResponse {
			access_token: "apple_access_token".into(),
			token_type: "Bearer".into(),
			expires_in: Some(3600),
			refresh_token: Some("refresh_token".into()),
			scope: Some("openid email name".into()),
			id_token: Some("test_id_token".into()),
		}
	}

	/// Create Apple ID token
	pub fn apple_id_token() -> IdToken {
		IdToken {
			sub: "apple_user_123".into(),
			iss: "https://appleid.apple.com".into(),
			aud: "test_apple_client_id".into(),
			exp: 1735636800,
			iat: 1735633200,
			nonce: Some("test_nonce".into()),
			email: Some("user@icloud.com".into()),
			email_verified: Some(true),
			name: None,
			given_name: None,
			family_name: None,
			picture: None,
			locale: None,
			additional_claims: HashMap::new(),
		}
	}

	// ============================================================
	// Common Fixtures
	// ============================================================

	/// Generate random state string
	pub fn random_state() -> String {
		use rand::Rng;
		rand::rng()
			.sample_iter(&rand::distr::Alphanumeric)
			.take(32)
			.map(char::from)
			.collect()
	}

	/// Generate random nonce string
	pub fn random_nonce() -> String {
		use rand::Rng;
		rand::rng()
			.sample_iter(&rand::distr::Alphanumeric)
			.take(32)
			.map(char::from)
			.collect()
	}
}
