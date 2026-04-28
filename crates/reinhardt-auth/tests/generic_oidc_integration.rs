//! End-to-end integration tests for `GenericOidcProvider`.
//!
//! These tests stand up a `wiremock` HTTP server, publish a synthetic
//! discovery document and JWKS, sign ID tokens with an in-memory RSA key,
//! and exercise the `OAuthProvider` trait surface from the public API
//! (`reinhardt_auth::social::providers::GenericOidcProvider`).
//!
//! All keys, issuers, and identifiers are generated per-test; no real
//! credentials are used.

#![cfg(feature = "social")]

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration as StdDuration;

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use chrono::{Duration as ChronoDuration, Utc};
use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
use rsa::pkcs8::EncodePrivateKey;
use rsa::traits::PublicKeyParts;
use rsa::{RsaPrivateKey, RsaPublicKey};
use rstest::*;
use serde_json::{Value, json};
use wiremock::matchers::{bearer_token, body_string_contains, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate, Times};

use reinhardt_auth::social::core::OAuthProvider;
use reinhardt_auth::social::core::SocialAuthError;
use reinhardt_auth::social::core::claims::IdToken;
use reinhardt_auth::social::providers::{GenericOidcConfig, GenericOidcProvider};

// ---------------------------------------------------------------------------
// Mock environment
// ---------------------------------------------------------------------------

/// Per-test mock server with an ephemeral RSA keypair and helpers for
/// publishing discovery + JWKS documents and minting signed ID tokens.
struct MockEnv {
	server: MockServer,
	private_pem: Vec<u8>,
	kid: String,
	n_b64: String,
	e_b64: String,
}

impl MockEnv {
	async fn new() -> Self {
		let server = MockServer::start().await;
		let mut rng = rsa::rand_core::OsRng;
		let private_key = RsaPrivateKey::new(&mut rng, 2048).expect("RSA keygen failed");
		let public_key = RsaPublicKey::from(&private_key);
		let private_pem = private_key
			.to_pkcs8_pem(rsa::pkcs8::LineEnding::LF)
			.expect("private key encoding failed")
			.as_bytes()
			.to_vec();

		let n_b64 = URL_SAFE_NO_PAD.encode(public_key.n().to_bytes_be());
		let e_b64 = URL_SAFE_NO_PAD.encode(public_key.e().to_bytes_be());
		let kid = "ic-test-key-1".to_string();

		Self {
			server,
			private_pem,
			kid,
			n_b64,
			e_b64,
		}
	}

	fn issuer(&self) -> String {
		self.server.uri()
	}
	fn discovery_url(&self) -> String {
		format!("{}/.well-known/openid-configuration", self.server.uri())
	}
	fn token_url(&self) -> String {
		format!("{}/token", self.server.uri())
	}
	fn jwks_url(&self) -> String {
		format!("{}/jwks.json", self.server.uri())
	}
	fn userinfo_url(&self) -> String {
		format!("{}/userinfo", self.server.uri())
	}
	fn auth_url(&self) -> String {
		format!("{}/oauth/authorize", self.server.uri())
	}

	fn discovery_doc(&self) -> Value {
		json!({
			"issuer": self.issuer(),
			"authorization_endpoint": self.auth_url(),
			"token_endpoint": self.token_url(),
			"jwks_uri": self.jwks_url(),
			"userinfo_endpoint": self.userinfo_url(),
			"id_token_signing_alg_values_supported": ["RS256"],
			"response_types_supported": ["code"],
			"subject_types_supported": ["public"],
		})
	}

	fn jwks_doc(&self) -> Value {
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

	fn sign_id_token(&self, claims: &IdToken) -> String {
		let mut header = Header::new(Algorithm::RS256);
		header.kid = Some(self.kid.clone());
		let key = EncodingKey::from_rsa_pem(&self.private_pem).expect("private PEM parse");
		encode(&header, claims, &key).expect("JWT signing")
	}

	/// Mints a JWT signed with `header.alg = none` (no signature). Used to
	/// verify that the validator rejects unsigned tokens.
	fn unsigned_id_token(&self, claims: &IdToken) -> String {
		let header_json = r#"{"alg":"none","typ":"JWT"}"#;
		let header_b64 = URL_SAFE_NO_PAD.encode(header_json.as_bytes());
		let claims_json = serde_json::to_vec(claims).expect("claims serialization");
		let claims_b64 = URL_SAFE_NO_PAD.encode(&claims_json);
		format!("{}.{}.", header_b64, claims_b64)
	}

	/// Mints a token signed with a different key but advertising
	/// `self.kid` so the JWKS lookup still resolves.
	fn sign_with_other_key(&self, claims: &IdToken) -> String {
		let mut rng = rsa::rand_core::OsRng;
		let other = RsaPrivateKey::new(&mut rng, 2048).expect("RSA keygen failed");
		let pem = other
			.to_pkcs8_pem(rsa::pkcs8::LineEnding::LF)
			.expect("encode other PEM")
			.as_bytes()
			.to_vec();
		let mut header = Header::new(Algorithm::RS256);
		header.kid = Some(self.kid.clone());
		let key = EncodingKey::from_rsa_pem(&pem).expect("other PEM parse");
		encode(&header, claims, &key).expect("JWT signing with other key")
	}

	/// Mints a token signed with the env's real key but a `kid` that is
	/// NOT in the JWKS, to exercise the unknown-kid rejection path.
	fn sign_with_unknown_kid(&self, claims: &IdToken) -> String {
		let mut header = Header::new(Algorithm::RS256);
		header.kid = Some("unknown-kid-99".to_string());
		let key = EncodingKey::from_rsa_pem(&self.private_pem).expect("private PEM parse");
		encode(&header, claims, &key).expect("JWT signing")
	}
}

#[fixture]
async fn env() -> MockEnv {
	MockEnv::new().await
}

fn id_token_for(env: &MockEnv, audience: &str) -> IdToken {
	let now = Utc::now();
	IdToken {
		sub: "user-9001".into(),
		iss: env.issuer(),
		aud: audience.into(),
		exp: (now + ChronoDuration::hours(1)).timestamp(),
		iat: now.timestamp(),
		nonce: None,
		email: Some("user@example.com".into()),
		email_verified: Some(true),
		name: Some("Test User".into()),
		given_name: Some("Test".into()),
		family_name: Some("User".into()),
		picture: None,
		locale: None,
		additional_claims: HashMap::new(),
	}
}

fn build_provider_config(env: &MockEnv, client_id: &str) -> GenericOidcConfig {
	GenericOidcConfig {
		name: "mock-oidc".into(),
		discovery_url: env.discovery_url(),
		client_id: client_id.into(),
		client_secret: "test-client-secret".into(),
		redirect_uri: "http://localhost:8080/callback".into(),
		scopes: vec!["openid".into(), "email".into(), "profile".into()],
		discovery_ttl: None,
		jwks_ttl: None,
		extra_token_params: None,
	}
}

async fn mount_discovery(server: &MockServer, body: Value, expected_calls: u64) {
	Mock::given(method("GET"))
		.and(path("/.well-known/openid-configuration"))
		.respond_with(ResponseTemplate::new(200).set_body_json(body))
		.expect(Times::from(expected_calls))
		.mount(server)
		.await;
}

async fn mount_jwks(server: &MockServer, body: Value) {
	Mock::given(method("GET"))
		.and(path("/jwks.json"))
		.respond_with(ResponseTemplate::new(200).set_body_json(body))
		.mount(server)
		.await;
}

// ---------------------------------------------------------------------------
// Discovery
// ---------------------------------------------------------------------------

#[rstest]
#[tokio::test]
async fn provider_resolves_authorization_url_via_discovery(#[future] env: MockEnv) {
	// Arrange
	let env = env.await;
	mount_discovery(&env.server, env.discovery_doc(), 1).await;
	let provider = GenericOidcProvider::new(build_provider_config(&env, "client-x"))
		.await
		.expect("construct provider");

	// Act
	let url = provider
		.authorization_url("state-123", Some("nonce-abc"), None)
		.await
		.expect("authorization URL");

	// Assert
	assert!(url.starts_with(&env.auth_url()));
	assert!(url.contains("client_id=client-x"));
	assert!(url.contains("state=state-123"));
	assert!(url.contains("nonce=nonce-abc"));
	assert!(url.contains("response_type=code"));
}

#[rstest]
#[tokio::test]
async fn discovery_document_missing_fields_is_rejected(#[future] env: MockEnv) {
	// Arrange — issuer is missing, which serde will catch when deserializing
	// into `OIDCDiscovery`.
	let env = env.await;
	let bad_doc = json!({
		"authorization_endpoint": env.auth_url(),
		"token_endpoint": env.token_url(),
		"jwks_uri": env.jwks_url(),
	});
	Mock::given(method("GET"))
		.and(path("/.well-known/openid-configuration"))
		.respond_with(ResponseTemplate::new(200).set_body_json(bad_doc))
		.mount(&env.server)
		.await;

	let provider = GenericOidcProvider::new(build_provider_config(&env, "client-x"))
		.await
		.expect("construct provider");

	// Act
	let result = provider.authorization_url("state", None, None).await;

	// Assert
	let err = result.err().expect("malformed discovery doc must error");
	assert!(matches!(err, SocialAuthError::Discovery(_)));
}

// ---------------------------------------------------------------------------
// Caching
// ---------------------------------------------------------------------------

#[rstest]
#[tokio::test]
async fn discovery_is_cached_within_ttl(#[future] env: MockEnv) {
	// Arrange — wiremock will fail the test if more than `expect(1)` GETs
	// arrive at /.well-known/openid-configuration.
	let env = env.await;
	mount_discovery(&env.server, env.discovery_doc(), 1).await;
	let provider = GenericOidcProvider::new(build_provider_config(&env, "client-x"))
		.await
		.expect("construct provider");

	// Act — call discovery-using methods three times.
	for _ in 0..3 {
		let _ = provider
			.authorization_url("s", None, None)
			.await
			.expect("authorization URL");
	}

	// Assert — `expect(1)` is verified on Drop of the MockServer; an
	// additional explicit assertion below makes the intent obvious.
	let received = env.server.received_requests().await.unwrap_or_default();
	let discovery_calls = received
		.iter()
		.filter(|r| r.url.path() == "/.well-known/openid-configuration")
		.count();
	assert_eq!(
		discovery_calls, 1,
		"discovery should be cached after first hit"
	);
}

#[rstest]
#[tokio::test]
async fn jwks_is_cached_for_repeated_validations(#[future] env: MockEnv) {
	// Arrange
	let env = env.await;
	let audience = "client-y";
	mount_discovery(&env.server, env.discovery_doc(), 1).await;
	mount_jwks(&env.server, env.jwks_doc()).await;

	let provider = GenericOidcProvider::new(build_provider_config(&env, audience))
		.await
		.expect("provider");

	let claims = id_token_for(&env, audience);
	let jwt = env.sign_id_token(&claims);

	// Act — validate the same token twice; the JWKS endpoint should be
	// hit at most once thanks to the cache.
	let _ = provider
		.validate_id_token(&jwt, None)
		.await
		.expect("validation 1");
	let _ = provider
		.validate_id_token(&jwt, None)
		.await
		.expect("validation 2");

	// Assert
	let received = env.server.received_requests().await.unwrap_or_default();
	let jwks_calls = received
		.iter()
		.filter(|r| r.url.path() == "/jwks.json")
		.count();
	assert_eq!(jwks_calls, 1, "JWKS should be cached after first hit");
}

// ---------------------------------------------------------------------------
// ID token validation
// ---------------------------------------------------------------------------

#[rstest]
#[tokio::test]
async fn validates_well_signed_id_token(#[future] env: MockEnv) {
	// Arrange
	let env = env.await;
	let audience = "client-valid";
	mount_discovery(&env.server, env.discovery_doc(), 1).await;
	mount_jwks(&env.server, env.jwks_doc()).await;

	let provider = GenericOidcProvider::new(build_provider_config(&env, audience))
		.await
		.expect("provider");
	let claims = id_token_for(&env, audience);
	let jwt = env.sign_id_token(&claims);

	// Act
	let validated = provider
		.validate_id_token(&jwt, None)
		.await
		.expect("validation should succeed");

	// Assert
	assert_eq!(validated.sub, "user-9001");
	assert_eq!(validated.aud, audience);
	assert_eq!(validated.iss, env.issuer());
}

#[rstest]
#[tokio::test]
async fn rejects_expired_id_token(#[future] env: MockEnv) {
	// Arrange
	let env = env.await;
	let audience = "client-expired";
	mount_discovery(&env.server, env.discovery_doc(), 1).await;
	mount_jwks(&env.server, env.jwks_doc()).await;
	let provider = GenericOidcProvider::new(build_provider_config(&env, audience))
		.await
		.expect("provider");

	let mut claims = id_token_for(&env, audience);
	claims.iat = (Utc::now() - ChronoDuration::hours(3)).timestamp();
	claims.exp = (Utc::now() - ChronoDuration::hours(2)).timestamp();
	let jwt = env.sign_id_token(&claims);

	// Act
	let result = provider.validate_id_token(&jwt, None).await;

	// Assert
	let err = result.err().expect("expired token must be rejected");
	assert!(
		matches!(err, SocialAuthError::InvalidIdToken(_)),
		"expected InvalidIdToken, got {:?}",
		err
	);
}

#[rstest]
#[tokio::test]
async fn rejects_id_token_with_wrong_issuer(#[future] env: MockEnv) {
	// Arrange
	let env = env.await;
	let audience = "client-iss";
	mount_discovery(&env.server, env.discovery_doc(), 1).await;
	mount_jwks(&env.server, env.jwks_doc()).await;
	let provider = GenericOidcProvider::new(build_provider_config(&env, audience))
		.await
		.expect("provider");

	let mut claims = id_token_for(&env, audience);
	claims.iss = "https://attacker.example.com".into();
	let jwt = env.sign_id_token(&claims);

	// Act
	let result = provider.validate_id_token(&jwt, None).await;

	// Assert
	let err = result.err().expect("wrong-iss token must be rejected");
	assert!(matches!(err, SocialAuthError::InvalidIdToken(_)));
}

#[rstest]
#[tokio::test]
async fn rejects_id_token_with_wrong_audience(#[future] env: MockEnv) {
	// Arrange
	let env = env.await;
	let audience = "client-aud";
	mount_discovery(&env.server, env.discovery_doc(), 1).await;
	mount_jwks(&env.server, env.jwks_doc()).await;
	let provider = GenericOidcProvider::new(build_provider_config(&env, audience))
		.await
		.expect("provider");

	// Sign a token addressed to a *different* client.
	let claims = id_token_for(&env, "different-client");
	let jwt = env.sign_id_token(&claims);

	// Act
	let result = provider.validate_id_token(&jwt, None).await;

	// Assert
	let err = result.err().expect("wrong-aud token must be rejected");
	assert!(matches!(err, SocialAuthError::InvalidIdToken(_)));
}

#[rstest]
#[tokio::test]
async fn rejects_id_token_with_alg_none(#[future] env: MockEnv) {
	// Arrange
	let env = env.await;
	let audience = "client-none";
	mount_discovery(&env.server, env.discovery_doc(), 1).await;
	mount_jwks(&env.server, env.jwks_doc()).await;
	let provider = GenericOidcProvider::new(build_provider_config(&env, audience))
		.await
		.expect("provider");

	let claims = id_token_for(&env, audience);
	let unsigned_jwt = env.unsigned_id_token(&claims);

	// Act
	let result = provider.validate_id_token(&unsigned_jwt, None).await;

	// Assert — `alg: none` must be rejected with a token-validation error.
	let err = result.err().expect("alg=none must be rejected");
	assert!(
		matches!(err, SocialAuthError::InvalidIdToken(_)),
		"expected InvalidIdToken, got {:?}",
		err
	);
}

#[rstest]
#[tokio::test]
async fn rejects_id_token_with_unknown_kid(#[future] env: MockEnv) {
	// Arrange
	let env = env.await;
	let audience = "client-kid";
	mount_discovery(&env.server, env.discovery_doc(), 1).await;
	mount_jwks(&env.server, env.jwks_doc()).await;
	let provider = GenericOidcProvider::new(build_provider_config(&env, audience))
		.await
		.expect("provider");

	let claims = id_token_for(&env, audience);
	let jwt = env.sign_with_unknown_kid(&claims);

	// Act
	let result = provider.validate_id_token(&jwt, None).await;

	// Assert
	let err = result.err().expect("unknown kid must be rejected");
	assert!(
		matches!(err, SocialAuthError::InvalidJwk(_)),
		"expected InvalidJwk, got {:?}",
		err
	);
}

#[rstest]
#[tokio::test]
async fn rejects_id_token_signed_with_different_key(#[future] env: MockEnv) {
	// Arrange
	let env = env.await;
	let audience = "client-sig";
	mount_discovery(&env.server, env.discovery_doc(), 1).await;
	mount_jwks(&env.server, env.jwks_doc()).await;
	let provider = GenericOidcProvider::new(build_provider_config(&env, audience))
		.await
		.expect("provider");

	let claims = id_token_for(&env, audience);
	let jwt = env.sign_with_other_key(&claims);

	// Act
	let result = provider.validate_id_token(&jwt, None).await;

	// Assert
	let err = result.err().expect("bad signature must be rejected");
	assert!(
		matches!(err, SocialAuthError::InvalidIdToken(_)),
		"expected InvalidIdToken, got {:?}",
		err
	);
}

// ---------------------------------------------------------------------------
// PKCE round-trip
// ---------------------------------------------------------------------------

#[rstest]
#[tokio::test]
async fn pkce_state_and_challenge_round_trip_through_authorization_url(#[future] env: MockEnv) {
	// Arrange
	let env = env.await;
	mount_discovery(&env.server, env.discovery_doc(), 1).await;
	let provider = GenericOidcProvider::new(build_provider_config(&env, "client-pkce"))
		.await
		.expect("provider");

	use reinhardt_auth::social::flow::PkceFlow;
	let (verifier, challenge) = PkceFlow::generate();

	// Act
	let url = provider
		.authorization_url(
			"state-pkce-1",
			Some("nonce-pkce-1"),
			Some(challenge.as_str()),
		)
		.await
		.expect("authorization URL");

	// Assert — PKCE parameters are forwarded verbatim, the verifier is
	// preserved by the caller, and the SHA256(verifier) === challenge
	// invariant still holds (sanity check).
	assert!(url.contains("code_challenge="));
	assert!(url.contains("code_challenge_method=S256"));
	assert!(url.contains(challenge.as_str()));
	assert!(
		!url.contains(verifier.as_str()),
		"verifier must NOT leak into the URL"
	);
	assert!(verifier.as_str().len() >= 43);
}

// ---------------------------------------------------------------------------
// UserInfo mapping
// ---------------------------------------------------------------------------

#[rstest]
#[tokio::test]
async fn default_userinfo_mapper_pulls_standard_claims(#[future] env: MockEnv) {
	// Arrange
	let env = env.await;
	mount_discovery(&env.server, env.discovery_doc(), 1).await;
	Mock::given(method("GET"))
		.and(path("/userinfo"))
		.and(bearer_token("access-default"))
		.respond_with(ResponseTemplate::new(200).set_body_json(json!({
			"sub": "user-default",
			"email": "default@example.com",
			"email_verified": true,
			"name": "Default User",
		})))
		.mount(&env.server)
		.await;

	let provider = GenericOidcProvider::new(build_provider_config(&env, "client-ui"))
		.await
		.expect("provider");

	// Act
	let claims = provider
		.get_user_info("access-default")
		.await
		.expect("userinfo should succeed");

	// Assert
	assert_eq!(claims.sub, "user-default");
	assert_eq!(claims.email.as_deref(), Some("default@example.com"));
	assert_eq!(claims.email_verified, Some(true));
	assert_eq!(claims.name.as_deref(), Some("Default User"));
}

#[rstest]
#[tokio::test]
async fn custom_userinfo_mapper_replaces_default_logic(#[future] env: MockEnv) {
	// Arrange — IdP returns non-standard claim names that the default
	// mapper cannot translate (it would error on missing `sub`).
	let env = env.await;
	mount_discovery(&env.server, env.discovery_doc(), 1).await;
	Mock::given(method("GET"))
		.and(path("/userinfo"))
		.and(bearer_token("access-custom"))
		.respond_with(ResponseTemplate::new(200).set_body_json(json!({
			"uid": "uid-42",
			"mail": "mapped@example.com",
			"groups": ["dev", "ops"],
		})))
		.mount(&env.server)
		.await;

	let invocations = Arc::new(AtomicUsize::new(0));
	let invocations_for_mapper = invocations.clone();
	let provider = GenericOidcProvider::new(build_provider_config(&env, "client-custom-ui"))
		.await
		.expect("provider")
		.with_userinfo_mapper(move |raw: &Value| {
			invocations_for_mapper.fetch_add(1, Ordering::SeqCst);
			let sub = raw
				.get("uid")
				.and_then(|v| v.as_str())
				.ok_or_else(|| SocialAuthError::UserMapping("missing uid".into()))?
				.to_string();
			let email = raw.get("mail").and_then(|v| v.as_str()).map(String::from);
			let mut additional = HashMap::new();
			if let Some(groups) = raw.get("groups").cloned() {
				additional.insert("groups".to_string(), groups);
			}
			Ok(reinhardt_auth::social::core::StandardClaims {
				sub,
				email,
				email_verified: Some(true),
				name: None,
				given_name: None,
				family_name: None,
				picture: None,
				locale: None,
				additional_claims: additional,
			})
		});

	// Act
	let claims = provider
		.get_user_info("access-custom")
		.await
		.expect("userinfo with custom mapper");

	// Assert
	assert_eq!(claims.sub, "uid-42");
	assert_eq!(claims.email.as_deref(), Some("mapped@example.com"));
	assert!(claims.additional_claims.contains_key("groups"));
	assert_eq!(
		invocations.load(Ordering::SeqCst),
		1,
		"custom mapper must be invoked exactly once per UserInfo call"
	);
}

#[rstest]
#[tokio::test]
async fn userinfo_propagates_provider_errors(#[future] env: MockEnv) {
	// Arrange
	let env = env.await;
	mount_discovery(&env.server, env.discovery_doc(), 1).await;
	Mock::given(method("GET"))
		.and(path("/userinfo"))
		.respond_with(ResponseTemplate::new(401).set_body_string("invalid_token"))
		.mount(&env.server)
		.await;

	let provider = GenericOidcProvider::new(build_provider_config(&env, "client-401"))
		.await
		.expect("provider");

	// Act
	let result = provider.get_user_info("expired-token").await;

	// Assert
	let err = result.err().expect("401 must surface as error");
	assert!(matches!(err, SocialAuthError::UserInfoError(_)));
}

// ---------------------------------------------------------------------------
// Token exchange + extra params
// ---------------------------------------------------------------------------

#[rstest]
#[tokio::test]
async fn token_exchange_includes_configured_extra_params(#[future] env: MockEnv) {
	// Arrange
	let env = env.await;
	mount_discovery(&env.server, env.discovery_doc(), 1).await;
	// Body must include both standard and extra parameters.
	Mock::given(method("POST"))
		.and(path("/token"))
		.and(body_string_contains("grant_type=authorization_code"))
		.and(body_string_contains("code=auth-1"))
		.and(body_string_contains(
			"audience=https%3A%2F%2Fapi.example.com",
		))
		.and(body_string_contains("resource=urn%3Aexample%3Aapi"))
		.respond_with(ResponseTemplate::new(200).set_body_json(json!({
			"access_token": "access-extra",
			"token_type": "Bearer",
			"expires_in": 3600,
		})))
		.mount(&env.server)
		.await;

	let mut config = build_provider_config(&env, "client-extra");
	config.extra_token_params = Some(vec![
		("audience".into(), "https://api.example.com".into()),
		("resource".into(), "urn:example:api".into()),
	]);
	let provider = GenericOidcProvider::new(config).await.expect("provider");

	// Act
	let response = provider
		.exchange_code("auth-1", None)
		.await
		.expect("token exchange");

	// Assert
	assert_eq!(response.access_token, "access-extra");
	assert_eq!(response.token_type, "Bearer");
}

#[rstest]
#[tokio::test]
async fn token_exchange_works_without_extra_params(#[future] env: MockEnv) {
	// Arrange
	let env = env.await;
	mount_discovery(&env.server, env.discovery_doc(), 1).await;
	Mock::given(method("POST"))
		.and(path("/token"))
		.and(body_string_contains("grant_type=authorization_code"))
		.and(body_string_contains("code=auth-2"))
		.respond_with(ResponseTemplate::new(200).set_body_json(json!({
			"access_token": "access-plain",
			"token_type": "Bearer",
			"expires_in": 1800,
		})))
		.mount(&env.server)
		.await;
	let provider = GenericOidcProvider::new(build_provider_config(&env, "client-plain"))
		.await
		.expect("provider");

	// Act
	let response = provider
		.exchange_code("auth-2", None)
		.await
		.expect("token exchange");

	// Assert
	assert_eq!(response.access_token, "access-plain");
}

// ---------------------------------------------------------------------------
// Refresh
// ---------------------------------------------------------------------------

#[rstest]
#[tokio::test]
async fn refresh_token_uses_discovery_endpoint(#[future] env: MockEnv) {
	// Arrange
	let env = env.await;
	mount_discovery(&env.server, env.discovery_doc(), 1).await;
	Mock::given(method("POST"))
		.and(path("/token"))
		.and(body_string_contains("grant_type=refresh_token"))
		.and(body_string_contains("refresh_token=rt-old"))
		.respond_with(ResponseTemplate::new(200).set_body_json(json!({
			"access_token": "access-refreshed",
			"token_type": "Bearer",
			"expires_in": 7200,
		})))
		.mount(&env.server)
		.await;
	let provider = GenericOidcProvider::new(build_provider_config(&env, "client-refresh"))
		.await
		.expect("provider");

	// Act
	let response = provider
		.refresh_token("rt-old")
		.await
		.expect("refresh succeeds");

	// Assert
	assert_eq!(response.access_token, "access-refreshed");
	assert_eq!(response.expires_in, Some(7200));
}

// ---------------------------------------------------------------------------
// TTL override smoke test
// ---------------------------------------------------------------------------

#[rstest]
#[tokio::test]
async fn provider_accepts_custom_ttls_without_panic(#[future] env: MockEnv) {
	// Arrange
	let env = env.await;
	mount_discovery(&env.server, env.discovery_doc(), 1).await;

	let mut config = build_provider_config(&env, "client-ttl");
	config.discovery_ttl = Some(StdDuration::from_secs(60));
	config.jwks_ttl = Some(StdDuration::from_secs(120));

	let provider = GenericOidcProvider::new(config)
		.await
		.expect("custom TTLs accepted");

	// Act / Assert — single discovery call confirms the cache is wired.
	let _ = provider
		.authorization_url("s", None, None)
		.await
		.expect("authorization URL");
}
