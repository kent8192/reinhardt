//! GitHub OAuth2 provider
//!
//! Implements OAuth2 authentication against GitHub's API. Unlike OIDC providers,
//! GitHub's `/user` endpoint returns a non-standard payload (numeric `id` instead
//! of the OIDC `sub` claim, and uses `avatar_url` instead of `picture`). This
//! module fetches that payload and transforms it into [`StandardClaims`] so the
//! rest of the social authentication pipeline can consume it uniformly.

use crate::social::core::{
	OAuth2Client, OAuthProvider, ProviderConfig, SocialAuthError, StandardClaims, TokenResponse,
};
use crate::social::flow::pkce::{CodeChallenge, CodeVerifier};
use crate::social::flow::{AuthorizationFlow, RefreshFlow, TokenExchangeFlow};
use crate::social::url_validation::validate_endpoint_url;
use async_trait::async_trait;
use serde::Deserialize;
use std::collections::HashMap;

/// GitHub `/user` endpoint response shape.
///
/// GitHub's REST API does not follow the OIDC UserInfo schema, so we deserialize
/// into this private intermediate struct and then map to [`StandardClaims`] via
/// [`map_github_user_to_claims`].
#[derive(Debug, Deserialize)]
struct GitHubUserResponse {
	/// Numeric GitHub user ID. Mapped to `StandardClaims::sub` as a string.
	id: u64,
	/// GitHub login (username). Used as a fallback for `name` when null.
	login: String,
	/// Public email address. May be `null` if the user keeps their email private.
	#[serde(default)]
	email: Option<String>,
	/// Display name. May be `null` if the user has not set one.
	#[serde(default)]
	name: Option<String>,
	/// Avatar URL. Mapped to `StandardClaims::picture`.
	#[serde(default)]
	avatar_url: Option<String>,
}

/// Map a GitHub `/user` response into the framework's [`StandardClaims`].
///
/// - `sub` is set to the stringified numeric `id`.
/// - `name` falls back to `login` when GitHub returns `null`.
/// - `email` and `picture` (from `avatar_url`) pass through as-is.
fn map_github_user_to_claims(user: GitHubUserResponse) -> StandardClaims {
	let GitHubUserResponse {
		id,
		login,
		email,
		name,
		avatar_url,
	} = user;

	StandardClaims {
		sub: id.to_string(),
		email,
		email_verified: None,
		name: name.or(Some(login)),
		given_name: None,
		family_name: None,
		picture: avatar_url,
		locale: None,
		additional_claims: HashMap::new(),
	}
}

/// GitHub OAuth2 provider
///
/// Implements OAuth2-only authentication flow using static endpoints
/// configured via `ProviderConfig::github()`. GitHub's `/user` endpoint
/// returns a non-OIDC payload, so this provider issues its own HTTP request
/// instead of delegating to a generic `UserInfoClient`.
pub struct GitHubProvider {
	config: ProviderConfig,
	auth_flow: AuthorizationFlow,
	token_exchange: TokenExchangeFlow,
	refresh_flow: RefreshFlow,
	client: OAuth2Client,
}

impl GitHubProvider {
	/// Create a new GitHub provider
	///
	/// Validates that the configuration contains OAuth2 endpoints
	/// and constructs all sub-components. No network calls are made.
	pub async fn new(config: ProviderConfig) -> Result<Self, SocialAuthError> {
		if config.oauth2.is_none() {
			return Err(SocialAuthError::InvalidConfiguration(
				"GitHub provider requires OAuth2 configuration".into(),
			));
		}

		let client = OAuth2Client::new();
		let auth_flow = AuthorizationFlow::new(config.clone());
		let token_exchange = TokenExchangeFlow::new(client.clone(), config.clone());
		let refresh_flow = RefreshFlow::new(client.clone(), config.clone());

		Ok(Self {
			config,
			auth_flow,
			token_exchange,
			refresh_flow,
			client,
		})
	}
}

#[async_trait]
impl OAuthProvider for GitHubProvider {
	fn name(&self) -> &str {
		"github"
	}

	fn is_oidc(&self) -> bool {
		false
	}

	async fn authorization_url(
		&self,
		state: &str,
		_nonce: Option<&str>,
		code_challenge: Option<&str>,
	) -> Result<String, SocialAuthError> {
		let oauth2_config =
			self.config.oauth2.as_ref().ok_or_else(|| {
				SocialAuthError::InvalidConfiguration("Missing OAuth2 config".into())
			})?;

		let challenge = code_challenge.map(|c| CodeChallenge::from_raw(c.to_string()));

		self.auth_flow.build_oauth2_url(
			&oauth2_config.authorization_endpoint,
			state,
			challenge.as_ref(),
		)
	}

	async fn exchange_code(
		&self,
		code: &str,
		code_verifier: Option<&str>,
	) -> Result<TokenResponse, SocialAuthError> {
		let oauth2_config =
			self.config.oauth2.as_ref().ok_or_else(|| {
				SocialAuthError::InvalidConfiguration("Missing OAuth2 config".into())
			})?;

		let verifier = code_verifier.map(|v| CodeVerifier::from_raw(v.to_string()));

		self.token_exchange
			.exchange(&oauth2_config.token_endpoint, code, verifier.as_ref())
			.await
	}

	async fn refresh_token(&self, refresh_token: &str) -> Result<TokenResponse, SocialAuthError> {
		let oauth2_config =
			self.config.oauth2.as_ref().ok_or_else(|| {
				SocialAuthError::InvalidConfiguration("Missing OAuth2 config".into())
			})?;

		self.refresh_flow
			.refresh(&oauth2_config.token_endpoint, refresh_token)
			.await
	}

	async fn get_user_info(&self, access_token: &str) -> Result<StandardClaims, SocialAuthError> {
		let oauth2_config =
			self.config.oauth2.as_ref().ok_or_else(|| {
				SocialAuthError::InvalidConfiguration("Missing OAuth2 config".into())
			})?;

		let userinfo_endpoint = oauth2_config.userinfo_endpoint.as_ref().ok_or_else(|| {
			SocialAuthError::InvalidConfiguration("Missing UserInfo endpoint".into())
		})?;

		// Enforce HTTPS (or loopback HTTP) before transmitting the bearer token.
		// Mirrors the validation performed by `UserInfoClient` and by
		// `GenericOidcProvider::get_user_info` so the bearer token is never sent
		// to an arbitrary scheme.
		validate_endpoint_url(userinfo_endpoint)?;

		// GitHub requires a User-Agent header on `/user` requests, and returns
		// a non-OIDC payload, so we do not delegate to `UserInfoClient` here.
		let response = self
			.client
			.client()
			.get(userinfo_endpoint)
			.bearer_auth(access_token)
			.header("User-Agent", "reinhardt-auth")
			.header("Accept", "application/vnd.github+json")
			.send()
			.await
			.map_err(|e| SocialAuthError::Network(e.to_string()))?;

		if !response.status().is_success() {
			let status = response.status();
			let error_body = response
				.text()
				.await
				.unwrap_or_else(|_| "Unknown error".to_string());

			return Err(SocialAuthError::UserInfoError(format!(
				"GitHub UserInfo request failed ({}): {}",
				status, error_body
			)));
		}

		let user: GitHubUserResponse = response
			.json()
			.await
			.map_err(|e| SocialAuthError::UserInfoError(e.to_string()))?;

		Ok(map_github_user_to_claims(user))
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	#[rstest]
	fn github_user_response_deserializes_typical_payload() {
		// Arrange
		let json = r#"{
			"id": 12345,
			"login": "octotest",
			"email": "octo@example.com",
			"name": "Octo Test",
			"avatar_url": "https://avatars.githubusercontent.com/u/12345"
		}"#;

		// Act
		let user: GitHubUserResponse =
			serde_json::from_str(json).expect("typical GitHub /user payload must deserialize");

		// Assert
		assert_eq!(user.id, 12345);
		assert_eq!(user.login, "octotest");
		assert_eq!(user.email.as_deref(), Some("octo@example.com"));
		assert_eq!(user.name.as_deref(), Some("Octo Test"));
		assert_eq!(
			user.avatar_url.as_deref(),
			Some("https://avatars.githubusercontent.com/u/12345")
		);
	}

	#[rstest]
	fn github_user_response_deserializes_with_null_optional_fields() {
		// Arrange: GitHub returns `null` for email/name when the user keeps them private.
		let json = r#"{
			"id": 99,
			"login": "ghost",
			"email": null,
			"name": null,
			"avatar_url": null
		}"#;

		// Act
		let user: GitHubUserResponse =
			serde_json::from_str(json).expect("payload with null optionals must deserialize");

		// Assert
		assert_eq!(user.id, 99);
		assert_eq!(user.login, "ghost");
		assert!(user.email.is_none());
		assert!(user.name.is_none());
		assert!(user.avatar_url.is_none());
	}

	#[rstest]
	fn github_user_response_deserializes_with_missing_optional_fields() {
		// Arrange: GitHub may omit optional fields entirely for some account types.
		let json = r#"{
			"id": 7,
			"login": "minimal"
		}"#;

		// Act
		let user: GitHubUserResponse = serde_json::from_str(json)
			.expect("payload with missing optional fields must deserialize");

		// Assert
		assert_eq!(user.id, 7);
		assert_eq!(user.login, "minimal");
		assert!(user.email.is_none());
		assert!(user.name.is_none());
		assert!(user.avatar_url.is_none());
	}

	#[rstest]
	fn map_github_user_to_claims_maps_full_payload() {
		// Arrange
		let user = GitHubUserResponse {
			id: 12345,
			login: "octotest".to_string(),
			email: Some("octo@example.com".to_string()),
			name: Some("Octo Test".to_string()),
			avatar_url: Some("https://avatars.githubusercontent.com/u/12345".to_string()),
		};

		// Act
		let claims = map_github_user_to_claims(user);

		// Assert
		assert_eq!(claims.sub, "12345");
		assert_eq!(claims.email.as_deref(), Some("octo@example.com"));
		assert_eq!(claims.name.as_deref(), Some("Octo Test"));
		assert_eq!(
			claims.picture.as_deref(),
			Some("https://avatars.githubusercontent.com/u/12345")
		);
		assert!(claims.email_verified.is_none());
		assert!(claims.given_name.is_none());
		assert!(claims.family_name.is_none());
		assert!(claims.locale.is_none());
		assert!(claims.additional_claims.is_empty());
	}

	#[rstest]
	fn map_github_user_to_claims_falls_back_to_login_when_name_is_null() {
		// Arrange
		let user = GitHubUserResponse {
			id: 42,
			login: "octotest".to_string(),
			email: Some("octo@example.com".to_string()),
			name: None,
			avatar_url: None,
		};

		// Act
		let claims = map_github_user_to_claims(user);

		// Assert
		assert_eq!(claims.sub, "42");
		assert_eq!(
			claims.name.as_deref(),
			Some("octotest"),
			"name must fall back to login when GitHub returns null"
		);
		assert_eq!(claims.email.as_deref(), Some("octo@example.com"));
		assert!(claims.picture.is_none());
	}

	#[rstest]
	fn map_github_user_to_claims_handles_both_name_and_email_null() {
		// Arrange: edge case where the user has hidden both display name and email.
		let user = GitHubUserResponse {
			id: 1,
			login: "ghost".to_string(),
			email: None,
			name: None,
			avatar_url: None,
		};

		// Act
		let claims = map_github_user_to_claims(user);

		// Assert
		assert_eq!(claims.sub, "1");
		assert_eq!(
			claims.name.as_deref(),
			Some("ghost"),
			"name must fall back to login when GitHub returns null"
		);
		assert!(
			claims.email.is_none(),
			"email must remain None when GitHub returns null"
		);
		assert!(claims.picture.is_none());
	}

	#[rstest]
	#[case::numeric_id_serializes_as_string(0_u64, "0")]
	#[case::large_id_preserved(9_999_999_999_u64, "9999999999")]
	fn map_github_user_to_claims_stringifies_id(#[case] id: u64, #[case] expected_sub: &str) {
		// Arrange
		let user = GitHubUserResponse {
			id,
			login: "any".to_string(),
			email: None,
			name: None,
			avatar_url: None,
		};

		// Act
		let claims = map_github_user_to_claims(user);

		// Assert
		assert_eq!(claims.sub, expected_sub);
	}
}
