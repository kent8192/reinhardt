//! End-to-end OAuth2/OIDC integration tests using `wiremock` and a
//! freshly generated RSA keypair.
//!
//! These tests exercise the network-facing code paths of
//! `TokenExchangeFlow`, `RefreshFlow`, `UserInfoClient`, and
//! `IdTokenValidator` by spinning up real HTTP servers with wiremock
//! and signing ID tokens with an in-memory RSA key that is exposed via
//! a mock JWKS endpoint.
//!
//! All keys, tokens and identifiers are generated at test setup; no
//! real credentials are ever used.

#![cfg(feature = "social")]

use std::collections::HashMap;
use std::sync::Arc;

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use chrono::{Duration, Utc};
use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
use rsa::pkcs8::EncodePrivateKey;
use rsa::traits::PublicKeyParts;
use rsa::{RsaPrivateKey, RsaPublicKey};
use rstest::*;
use serde_json::json;
use wiremock::matchers::{bearer_token, body_string_contains, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use reinhardt_auth::social::core::OAuth2Client;
use reinhardt_auth::social::core::claims::IdToken;
use reinhardt_auth::social::core::config::ProviderConfig;
use reinhardt_auth::social::flow::{RefreshFlow, TokenExchangeFlow};
use reinhardt_auth::social::oidc::id_token::ValidationConfig;
use reinhardt_auth::social::oidc::{IdTokenValidator, JwksCache, UserInfoClient};

// ---------------------------------------------------------------------------
// Fixture
// ---------------------------------------------------------------------------

/// Test environment shared by every integration test: a running wiremock
/// HTTP server plus an RSA keypair whose public half is exposed as JWKS.
struct MockEnv {
	server: MockServer,
	private_pem: Vec<u8>,
	kid: String,
	n_b64: String,
	e_b64: String,
}

impl MockEnv {
	/// Starts a mock server, generates a fresh 2048-bit RSA keypair, and
	/// computes JWK parameters (`n`, `e`) for JWKS publishing.
	async fn new() -> Self {
		let server = MockServer::start().await;

		// Generate fresh RSA keypair for this test run.
		let mut rng = rsa::rand_core::OsRng;
		let private_key = RsaPrivateKey::new(&mut rng, 2048).expect("failed to generate RSA key");
		let public_key = RsaPublicKey::from(&private_key);

		let private_pem = private_key
			.to_pkcs8_pem(rsa::pkcs8::LineEnding::LF)
			.expect("failed to encode private key")
			.as_bytes()
			.to_vec();

		let n_bytes = public_key.n().to_bytes_be();
		let e_bytes = public_key.e().to_bytes_be();
		let n_b64 = URL_SAFE_NO_PAD.encode(&n_bytes);
		let e_b64 = URL_SAFE_NO_PAD.encode(&e_bytes);
		let kid = "test-key-1".to_string();

		Self {
			server,
			private_pem,
			kid,
			n_b64,
			e_b64,
		}
	}

	fn base_url(&self) -> String {
		self.server.uri()
	}

	fn token_url(&self) -> String {
		format!("{}/token", self.base_url())
	}

	fn userinfo_url(&self) -> String {
		format!("{}/userinfo", self.base_url())
	}

	fn jwks_url(&self) -> String {
		format!("{}/jwks.json", self.base_url())
	}

	fn issuer(&self) -> String {
		self.base_url()
	}

	/// JSON body of the JWKS document, referencing the current keypair.
	fn jwks_json(&self) -> serde_json::Value {
		json!({
			"keys": [{
				"kty": "RSA",
				"kid": self.kid,
				"use": "sig",
				"alg": "RS256",
				"n": self.n_b64,
				"e": self.e_b64,
			}]
		})
	}

	/// JSON body of a JWKS document whose key id matches `self.kid` but
	/// whose modulus belongs to a *different* RSA keypair (used to
	/// simulate signature validation failure).
	fn jwks_with_wrong_key(&self) -> serde_json::Value {
		let mut rng = rsa::rand_core::OsRng;
		let other = RsaPrivateKey::new(&mut rng, 2048).expect("failed to generate other key");
		let pub_other = RsaPublicKey::from(&other);
		let n_b64 = URL_SAFE_NO_PAD.encode(pub_other.n().to_bytes_be());
		let e_b64 = URL_SAFE_NO_PAD.encode(pub_other.e().to_bytes_be());
		json!({
			"keys": [{
				"kty": "RSA",
				"kid": self.kid,
				"use": "sig",
				"alg": "RS256",
				"n": n_b64,
				"e": e_b64,
			}]
		})
	}

	/// Sign an `IdToken` claim set into a compact JWS string using the
	/// RSA private key with RS256 and the JWK's `kid` header.
	fn sign_id_token(&self, claims: &IdToken) -> String {
		let mut header = Header::new(Algorithm::RS256);
		header.kid = Some(self.kid.clone());
		let key = EncodingKey::from_rsa_pem(&self.private_pem)
			.expect("failed to parse generated private PEM");
		encode(&header, claims, &key).expect("failed to sign JWT")
	}
}

#[fixture]
async fn mock_env() -> MockEnv {
	MockEnv::new().await
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn test_provider(redirect_uri: String) -> ProviderConfig {
	ProviderConfig::github(
		"test_client_id".to_string(),
		"test_client_secret".to_string(),
		redirect_uri,
	)
}

fn base_id_token(iss: &str, aud: &str) -> IdToken {
	let now = Utc::now();
	IdToken {
		sub: "user-42".to_string(),
		iss: iss.to_string(),
		aud: aud.to_string(),
		exp: (now + Duration::hours(1)).timestamp(),
		iat: now.timestamp(),
		nonce: None,
		email: Some("user@example.com".to_string()),
		email_verified: Some(true),
		name: Some("Test User".to_string()),
		given_name: Some("Test".to_string()),
		family_name: Some("User".to_string()),
		picture: None,
		locale: None,
		additional_claims: HashMap::new(),
	}
}

// ---------------------------------------------------------------------------
// Token exchange
// ---------------------------------------------------------------------------

#[rstest]
#[tokio::test]
async fn token_exchange_parses_full_token_response(#[future] mock_env: MockEnv) {
	// Arrange
	let env = mock_env.await;
	Mock::given(method("POST"))
		.and(path("/token"))
		.and(body_string_contains("grant_type=authorization_code"))
		.and(body_string_contains("code=auth_code_123"))
		.respond_with(ResponseTemplate::new(200).set_body_json(json!({
			"access_token": "access-abc",
			"token_type": "Bearer",
			"expires_in": 3600,
			"refresh_token": "refresh-xyz",
			"scope": "openid email profile",
			"id_token": "dummy.jwt.token",
		})))
		.mount(&env.server)
		.await;

	let flow = TokenExchangeFlow::new(
		OAuth2Client::new(),
		test_provider("http://localhost/callback".into()),
	);

	// Act
	let response = flow
		.exchange(&env.token_url(), "auth_code_123", None)
		.await
		.expect("token exchange should succeed");

	// Assert
	assert_eq!(response.access_token, "access-abc");
	assert_eq!(response.token_type, "Bearer");
	assert_eq!(response.expires_in, Some(3600));
	assert_eq!(response.refresh_token.as_deref(), Some("refresh-xyz"));
	assert_eq!(response.id_token.as_deref(), Some("dummy.jwt.token"));
}

#[rstest]
#[tokio::test]
async fn token_exchange_reports_provider_error(#[future] mock_env: MockEnv) {
	// Arrange
	let env = mock_env.await;
	Mock::given(method("POST"))
		.and(path("/token"))
		.respond_with(ResponseTemplate::new(400).set_body_string("invalid_grant"))
		.mount(&env.server)
		.await;

	let flow = TokenExchangeFlow::new(
		OAuth2Client::new(),
		test_provider("http://localhost/callback".into()),
	);

	// Act
	let result = flow.exchange(&env.token_url(), "bad_code", None).await;

	// Assert
	assert!(result.is_err(), "400 response should surface as error");
}

// ---------------------------------------------------------------------------
// Refresh
// ---------------------------------------------------------------------------

#[rstest]
#[tokio::test]
async fn refresh_flow_returns_new_access_token(#[future] mock_env: MockEnv) {
	// Arrange
	let env = mock_env.await;
	Mock::given(method("POST"))
		.and(path("/token"))
		.and(body_string_contains("grant_type=refresh_token"))
		.and(body_string_contains("refresh_token=rt-old"))
		.respond_with(ResponseTemplate::new(200).set_body_json(json!({
			"access_token": "access-refreshed",
			"token_type": "Bearer",
			"expires_in": 7200,
			"scope": "openid",
		})))
		.mount(&env.server)
		.await;

	let flow = RefreshFlow::new(
		OAuth2Client::new(),
		test_provider("http://localhost/callback".into()),
	);

	// Act
	let response = flow
		.refresh(&env.token_url(), "rt-old")
		.await
		.expect("refresh should succeed");

	// Assert
	assert_eq!(response.access_token, "access-refreshed");
	assert_eq!(response.token_type, "Bearer");
	assert_eq!(response.expires_in, Some(7200));
}

// ---------------------------------------------------------------------------
// UserInfo
// ---------------------------------------------------------------------------

#[rstest]
#[tokio::test]
async fn userinfo_parses_standard_claims(#[future] mock_env: MockEnv) {
	// Arrange
	let env = mock_env.await;
	Mock::given(method("GET"))
		.and(path("/userinfo"))
		.and(bearer_token("access-abc"))
		.respond_with(ResponseTemplate::new(200).set_body_json(json!({
			"sub": "user-42",
			"email": "user@example.com",
			"email_verified": true,
			"name": "Test User",
			"given_name": "Test",
			"family_name": "User",
		})))
		.mount(&env.server)
		.await;

	let client = UserInfoClient::new(OAuth2Client::new());

	// Act
	let claims = client
		.get_user_info(&env.userinfo_url(), "access-abc")
		.await
		.expect("userinfo should succeed");

	// Assert
	assert_eq!(claims.sub, "user-42");
	assert_eq!(claims.email.as_deref(), Some("user@example.com"));
	assert_eq!(claims.email_verified, Some(true));
	assert_eq!(claims.name.as_deref(), Some("Test User"));
}

#[rstest]
#[tokio::test]
async fn userinfo_surfaces_unauthorized(#[future] mock_env: MockEnv) {
	// Arrange
	let env = mock_env.await;
	Mock::given(method("GET"))
		.and(path("/userinfo"))
		.respond_with(ResponseTemplate::new(401).set_body_string("invalid_token"))
		.mount(&env.server)
		.await;

	let client = UserInfoClient::new(OAuth2Client::new());

	// Act
	let result = client
		.get_user_info(&env.userinfo_url(), "stale-token")
		.await;

	// Assert
	assert!(result.is_err(), "401 should propagate as UserInfoError");
}

// ---------------------------------------------------------------------------
// ID token validation via JWKS
// ---------------------------------------------------------------------------

#[rstest]
#[tokio::test]
async fn id_token_validates_with_matching_jwks(#[future] mock_env: MockEnv) {
	// Arrange
	let env = mock_env.await;
	let audience = "test_client_id";
	let claims = base_id_token(&env.issuer(), audience);
	let jwt = env.sign_id_token(&claims);

	Mock::given(method("GET"))
		.and(path("/jwks.json"))
		.respond_with(ResponseTemplate::new(200).set_body_json(env.jwks_json()))
		.mount(&env.server)
		.await;

	let cache = Arc::new(JwksCache::new(OAuth2Client::new()));
	let validator = IdTokenValidator::new(
		cache,
		ValidationConfig::new(env.issuer(), audience.to_string()),
	);

	// Act
	let validated = validator
		.validate(&jwt, &env.jwks_url(), None)
		.await
		.expect("ID token should validate");

	// Assert
	assert_eq!(validated.sub, "user-42");
	assert_eq!(validated.iss, env.issuer());
	assert_eq!(validated.aud, audience);
}

#[rstest]
#[tokio::test]
async fn id_token_rejects_expired_token(#[future] mock_env: MockEnv) {
	// Arrange
	let env = mock_env.await;
	let audience = "test_client_id";
	let mut claims = base_id_token(&env.issuer(), audience);
	claims.iat = (Utc::now() - Duration::hours(3)).timestamp();
	claims.exp = (Utc::now() - Duration::hours(2)).timestamp();
	let jwt = env.sign_id_token(&claims);

	Mock::given(method("GET"))
		.and(path("/jwks.json"))
		.respond_with(ResponseTemplate::new(200).set_body_json(env.jwks_json()))
		.mount(&env.server)
		.await;

	let cache = Arc::new(JwksCache::new(OAuth2Client::new()));
	let validator = IdTokenValidator::new(
		cache,
		ValidationConfig::new(env.issuer(), audience.to_string()),
	);

	// Act
	let result = validator.validate(&jwt, &env.jwks_url(), None).await;

	// Assert
	assert!(result.is_err(), "expired JWT must be rejected");
}

#[rstest]
#[tokio::test]
async fn id_token_rejects_signature_when_jwks_key_mismatches(#[future] mock_env: MockEnv) {
	// Arrange: sign with env's real private key, but JWKS publishes a
	// different public key under the same kid.
	let env = mock_env.await;
	let audience = "test_client_id";
	let claims = base_id_token(&env.issuer(), audience);
	let jwt = env.sign_id_token(&claims);
	let wrong_jwks = env.jwks_with_wrong_key();

	Mock::given(method("GET"))
		.and(path("/jwks.json"))
		.respond_with(ResponseTemplate::new(200).set_body_json(wrong_jwks))
		.mount(&env.server)
		.await;

	let cache = Arc::new(JwksCache::new(OAuth2Client::new()));
	let validator = IdTokenValidator::new(
		cache,
		ValidationConfig::new(env.issuer(), audience.to_string()),
	);

	// Act
	let result = validator.validate(&jwt, &env.jwks_url(), None).await;

	// Assert
	assert!(
		result.is_err(),
		"signature check must fail when JWKS key does not match signer"
	);
}

#[rstest]
#[tokio::test]
async fn id_token_rejects_wrong_audience(#[future] mock_env: MockEnv) {
	// Arrange
	let env = mock_env.await;
	let claims = base_id_token(&env.issuer(), "other_client");
	let jwt = env.sign_id_token(&claims);

	Mock::given(method("GET"))
		.and(path("/jwks.json"))
		.respond_with(ResponseTemplate::new(200).set_body_json(env.jwks_json()))
		.mount(&env.server)
		.await;

	let cache = Arc::new(JwksCache::new(OAuth2Client::new()));
	let validator = IdTokenValidator::new(
		cache,
		ValidationConfig::new(env.issuer(), "test_client_id".to_string()),
	);

	// Act
	let result = validator.validate(&jwt, &env.jwks_url(), None).await;

	// Assert
	assert!(result.is_err(), "audience mismatch must be rejected");
}
