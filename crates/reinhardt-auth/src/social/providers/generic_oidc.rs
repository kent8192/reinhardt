//! Generic OIDC provider
//!
//! Implements [`OAuthProvider`] for any OpenID Connect-compliant identity
//! provider (self-hosted GitLab, Keycloak, Authentik, …) using only a
//! discovery URL plus client credentials.
//!
//! Unlike the bundled [`GoogleProvider`] / [`MicrosoftProvider`] / etc.,
//! this provider performs no IdP-specific normalization. All endpoints are
//! resolved through the discovery document published at
//! `<discovery_url>` (typically `<issuer>/.well-known/openid-configuration`),
//! the JWKS is fetched from the discovery document's `jwks_uri`, and the
//! ID token is signature-verified against that JWKS before any claim is
//! trusted.
//!
//! # Security
//!
//! - ID token JWS verification is mandatory: signature, `iss`, `aud`,
//!   `exp`, and (with skew) `iat` are all checked. The `alg: none` JWT
//!   "algorithm" and any symmetric `HS*` algorithm are rejected.
//! - Allowed signing algorithms: `RS256` / `RS384` / `RS512`,
//!   `PS256` / `PS384` / `PS512`. Asymmetric ECDSA (`ES256`, `ES384`)
//!   appears in the spec but is not yet supported by the bundled
//!   [`JwksCache`] (RSA-only); EC support is tracked as a follow-up.
//! - The discovery document and the JWKS are cached in-memory with
//!   configurable TTLs (defaults: 1 hour each).
//! - All endpoint URLs returned by discovery are required to use HTTPS
//!   (HTTP is permitted only for loopback addresses for local development).
//!
//! # Example
//!
//! ```ignore
//! use std::sync::Arc;
//! use reinhardt_auth::social::providers::{GenericOidcConfig, GenericOidcProvider};
//! use reinhardt_auth::social::backend::SocialAuthBackend;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let provider = GenericOidcProvider::new(GenericOidcConfig {
//!     name: "gitlab".to_string(),
//!     discovery_url: "https://gitlab.com/.well-known/openid-configuration".into(),
//!     client_id: "client-id".into(),
//!     client_secret: "client-secret".into(),
//!     redirect_uri: "https://app.example.com/auth/callback".into(),
//!     scopes: vec!["openid".into(), "email".into(), "profile".into()],
//!     discovery_ttl: None,
//!     jwks_ttl: None,
//!     extra_token_params: None,
//! }).await?;
//!
//! let mut backend = SocialAuthBackend::new();
//! backend.register_provider(Arc::new(provider));
//! # Ok(())
//! # }
//! ```
//!
//! [`GoogleProvider`]: crate::social::providers::GoogleProvider
//! [`MicrosoftProvider`]: crate::social::providers::MicrosoftProvider

use std::sync::Arc;
use std::time::Duration as StdDuration;

use async_trait::async_trait;
use chrono::Duration as ChronoDuration;
use jsonwebtoken::Algorithm;
use serde_json::Value;
use tokio::sync::OnceCell;

use crate::social::core::{
	IdToken, OAuth2Client, OAuthProvider, ProviderConfig, SocialAuthError, StandardClaims,
	TokenResponse,
};
use crate::social::flow::pkce::{CodeChallenge, CodeVerifier};
use crate::social::flow::{AuthorizationFlow, RefreshFlow, TokenExchangeFlow};
use crate::social::oidc::id_token::ValidationConfig;
use crate::social::oidc::{
	DiscoveryClient, IdTokenValidator, JwksCache, OIDCDiscovery, UserInfoClient,
};
use crate::social::url_validation::validate_endpoint_url;

/// Default TTL for discovery and JWKS caches when the caller does not override.
const DEFAULT_CACHE_TTL_SECS: i64 = 3600;

/// Asymmetric algorithms that the bundled [`JwksCache`] supports today.
///
/// `HS*` is intentionally absent (symmetric secrets must never be accepted
/// for OIDC ID tokens). `ES*` is also absent because `Jwk::to_decoding_key`
/// does not yet support EC keys.
const SUPPORTED_ASYMMETRIC_ALGORITHMS: &[Algorithm] = &[
	Algorithm::RS256,
	Algorithm::RS384,
	Algorithm::RS512,
	Algorithm::PS256,
	Algorithm::PS384,
	Algorithm::PS512,
];

/// Function type for transforming a raw UserInfo JSON document into
/// [`StandardClaims`].
///
/// Use [`GenericOidcProvider::with_userinfo_mapper`] to override the default
/// (which deserializes the response directly into [`StandardClaims`]).
pub type UserInfoMapper =
	Box<dyn Fn(&Value) -> Result<StandardClaims, SocialAuthError> + Send + Sync>;

/// Configuration for [`GenericOidcProvider`].
///
/// All fields are required except the optional caching/extension fields.
/// The `client_secret` is omitted from the [`std::fmt::Debug`] output to
/// reduce the risk of logs containing secret material.
#[derive(Clone)]
pub struct GenericOidcConfig {
	/// Provider name used to register this provider with
	/// [`SocialAuthBackend`](crate::social::backend::SocialAuthBackend).
	///
	/// This becomes the `provider_name` argument passed to `begin_auth` /
	/// `handle_callback`. It is also returned by [`OAuthProvider::name`].
	pub name: String,

	/// OIDC discovery document URL. Typically ends in
	/// `/.well-known/openid-configuration`.
	pub discovery_url: String,

	/// OAuth2 / OIDC client identifier issued by the IdP.
	pub client_id: String,

	/// OAuth2 / OIDC client secret issued by the IdP.
	pub client_secret: String,

	/// Redirect URI registered with the IdP.
	pub redirect_uri: String,

	/// Requested OAuth2 scopes. SHOULD include `"openid"` for OIDC.
	pub scopes: Vec<String>,

	/// Override for the discovery document cache TTL (default: 1 hour).
	pub discovery_ttl: Option<StdDuration>,

	/// Override for the JWKS cache TTL (default: 1 hour).
	pub jwks_ttl: Option<StdDuration>,

	/// Additional `application/x-www-form-urlencoded` parameters appended
	/// to the token endpoint request. Useful for non-standard IdP extensions
	/// (e.g., audience parameters required by some Auth0 / Okta tenants).
	///
	/// The values are sent verbatim; the caller is responsible for ensuring
	/// they are URL-safe.
	pub extra_token_params: Option<Vec<(String, String)>>,
}

impl std::fmt::Debug for GenericOidcConfig {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("GenericOidcConfig")
			.field("name", &self.name)
			.field("discovery_url", &self.discovery_url)
			.field("client_id", &self.client_id)
			.field("client_secret", &"<redacted>")
			.field("redirect_uri", &self.redirect_uri)
			.field("scopes", &self.scopes)
			.field("discovery_ttl", &self.discovery_ttl)
			.field("jwks_ttl", &self.jwks_ttl)
			.field(
				"extra_token_params",
				&self
					.extra_token_params
					.as_ref()
					.map(|v| v.iter().map(|(k, _)| k.as_str()).collect::<Vec<_>>()),
			)
			.finish()
	}
}

/// Generic OIDC provider that resolves all endpoints via OIDC discovery.
///
/// See the [module-level documentation](self) for security guarantees and
/// usage examples.
pub struct GenericOidcProvider {
	name: String,
	config: GenericOidcConfig,
	client: OAuth2Client,
	auth_flow: AuthorizationFlow,
	token_exchange: TokenExchangeFlow,
	refresh_flow: RefreshFlow,
	userinfo_client: UserInfoClient,
	discovery_client: DiscoveryClient,
	jwks_cache: Arc<JwksCache>,
	/// Validator is constructed lazily on the first call that needs it,
	/// because the issuer comes from the discovery document.
	id_token_validator: OnceCell<IdTokenValidator>,
	userinfo_mapper: Option<UserInfoMapper>,
}

impl GenericOidcProvider {
	/// Creates a new generic OIDC provider.
	///
	/// No network requests are issued here; discovery and JWKS are fetched
	/// lazily on first use and then cached.
	///
	/// # Errors
	///
	/// Returns [`SocialAuthError::InvalidConfiguration`] if `name`,
	/// `discovery_url`, `client_id`, `client_secret`, or `redirect_uri` are
	/// empty, or if `discovery_url` cannot be parsed / does not use HTTPS
	/// (loopback HTTP is allowed for local development).
	pub async fn new(config: GenericOidcConfig) -> Result<Self, SocialAuthError> {
		Self::validate_config(&config)?;

		let provider_config = build_provider_config(&config);
		let client = OAuth2Client::new();
		let auth_flow = AuthorizationFlow::new(provider_config.clone());
		let token_exchange = TokenExchangeFlow::new(client.clone(), provider_config.clone());
		let refresh_flow = RefreshFlow::new(client.clone(), provider_config.clone());
		let userinfo_client = UserInfoClient::new(client.clone());

		let discovery_ttl = chrono_duration_or_default(config.discovery_ttl);
		let discovery_client = DiscoveryClient::with_ttl(client.clone(), discovery_ttl);

		let jwks_ttl = chrono_duration_or_default(config.jwks_ttl);
		let jwks_cache = Arc::new(JwksCache::with_ttl(client.clone(), jwks_ttl));

		// `provider_config` is consumed by the flow components above and
		// is no longer needed once they are constructed.
		let _ = provider_config;

		Ok(Self {
			name: config.name.clone(),
			config,
			client,
			auth_flow,
			token_exchange,
			refresh_flow,
			userinfo_client,
			discovery_client,
			jwks_cache,
			id_token_validator: OnceCell::new(),
			userinfo_mapper: None,
		})
	}

	/// Overrides the default UserInfo mapping with a caller-supplied closure.
	///
	/// The default mapping uses serde to deserialize the JSON body returned
	/// by the UserInfo endpoint directly into [`StandardClaims`]. Override
	/// this when the IdP returns non-standard claim names (e.g., GitLab's
	/// `groups` array, Keycloak's `realm_access`).
	///
	/// The mapper is invoked with the raw `serde_json::Value` produced by
	/// the UserInfo endpoint after a successful HTTP response.
	pub fn with_userinfo_mapper<F>(mut self, mapper: F) -> Self
	where
		F: Fn(&Value) -> Result<StandardClaims, SocialAuthError> + Send + Sync + 'static,
	{
		self.userinfo_mapper = Some(Box::new(mapper));
		self
	}

	/// Returns a reference to the cached configuration. Useful for tests.
	pub fn config(&self) -> &GenericOidcConfig {
		&self.config
	}

	fn validate_config(config: &GenericOidcConfig) -> Result<(), SocialAuthError> {
		if config.name.trim().is_empty() {
			return Err(SocialAuthError::InvalidConfiguration(
				"GenericOidcProvider requires a non-empty name".into(),
			));
		}
		if config.discovery_url.trim().is_empty() {
			return Err(SocialAuthError::InvalidConfiguration(
				"GenericOidcProvider requires a non-empty discovery_url".into(),
			));
		}
		if config.client_id.trim().is_empty() {
			return Err(SocialAuthError::InvalidConfiguration(
				"GenericOidcProvider requires a non-empty client_id".into(),
			));
		}
		if config.client_secret.is_empty() {
			return Err(SocialAuthError::InvalidConfiguration(
				"GenericOidcProvider requires a non-empty client_secret".into(),
			));
		}
		if config.redirect_uri.trim().is_empty() {
			return Err(SocialAuthError::InvalidConfiguration(
				"GenericOidcProvider requires a non-empty redirect_uri".into(),
			));
		}

		// Reject http:// (except loopback) up-front — discovery, token,
		// and userinfo URLs all flow through validate_endpoint_url at use
		// time, but we want to surface configuration errors during
		// `new()` rather than waiting for the first network call.
		validate_endpoint_url(&config.discovery_url)?;

		Ok(())
	}

	/// Fetches (and caches) the OIDC discovery document.
	async fn discover(&self) -> Result<OIDCDiscovery, SocialAuthError> {
		let issuer = issuer_from_discovery_url(&self.config.discovery_url);
		self.discovery_client.discover(&issuer).await
	}

	/// Returns the [`IdTokenValidator`], constructing it on first use from
	/// the issuer published in the discovery document.
	async fn id_token_validator(&self) -> Result<&IdTokenValidator, SocialAuthError> {
		self.id_token_validator
			.get_or_try_init(|| async {
				let discovery = self.discover().await?;

				// Honor the IdP's advertised algorithm list, intersected
				// with what we actually support. Always exclude HS* /
				// "none" — those are unsafe for OIDC.
				let allowed_algorithms = compute_allowed_algorithms(
					discovery.id_token_signing_alg_values_supported.as_deref(),
				);
				if allowed_algorithms.is_empty() {
					return Err(SocialAuthError::InvalidConfiguration(format!(
						"Provider '{}' advertises no asymmetric ID token signing algorithms supported by reinhardt-auth",
						self.name
					)));
				}

				let validation_config =
					ValidationConfig::new(discovery.issuer.clone(), self.config.client_id.clone())
						.with_allowed_algorithms(allowed_algorithms);

				Ok(IdTokenValidator::new(
					self.jwks_cache.clone(),
					validation_config,
				))
			})
			.await
	}

	/// Performs the authorization-code → token exchange. When
	/// `extra_token_params` is set on the config, a custom POST is issued
	/// directly so the additional fields are included; otherwise the
	/// shared [`TokenExchangeFlow`] is used.
	async fn exchange_with_extras(
		&self,
		token_endpoint: &str,
		code: &str,
		code_verifier: Option<&CodeVerifier>,
	) -> Result<TokenResponse, SocialAuthError> {
		let extras = match self.config.extra_token_params.as_ref() {
			Some(extras) if !extras.is_empty() => extras,
			_ => {
				return self
					.token_exchange
					.exchange(token_endpoint, code, code_verifier)
					.await;
			}
		};

		validate_endpoint_url(token_endpoint)?;

		let mut params: Vec<(String, String)> = vec![
			("grant_type".to_string(), "authorization_code".to_string()),
			("code".to_string(), code.to_string()),
			("redirect_uri".to_string(), self.config.redirect_uri.clone()),
			("client_id".to_string(), self.config.client_id.clone()),
			(
				"client_secret".to_string(),
				self.config.client_secret.clone(),
			),
		];

		if let Some(verifier) = code_verifier {
			params.push(("code_verifier".to_string(), verifier.as_str().to_string()));
		}

		for (k, v) in extras {
			params.push((k.clone(), v.clone()));
		}

		let response = self
			.client
			.client()
			.post(token_endpoint)
			.header("Accept", "application/json")
			.form(&params)
			.send()
			.await
			.map_err(|e| SocialAuthError::Network(e.to_string()))?;

		if !response.status().is_success() {
			let status = response.status();
			let error_body = response
				.text()
				.await
				.unwrap_or_else(|_| "Unknown error".to_string());
			return Err(SocialAuthError::TokenExchangeError(format!(
				"Token exchange failed ({}): {}",
				status, error_body
			)));
		}

		let token: TokenResponse = response
			.json()
			.await
			.map_err(|e| SocialAuthError::TokenExchangeError(e.to_string()))?;
		Ok(token)
	}

	/// Default UserInfo mapping: deserialize the raw JSON directly into
	/// [`StandardClaims`] via serde. Exposed only for unit tests; in
	/// production this is the implicit behavior of [`UserInfoClient`]
	/// when no custom mapper is registered.
	#[cfg(test)]
	fn default_map_userinfo(raw: &Value) -> Result<StandardClaims, SocialAuthError> {
		serde_json::from_value(raw.clone())
			.map_err(|e| SocialAuthError::UserInfoError(format!("Failed to map UserInfo: {}", e)))
	}
}

#[async_trait]
impl OAuthProvider for GenericOidcProvider {
	fn name(&self) -> &str {
		&self.name
	}

	fn is_oidc(&self) -> bool {
		true
	}

	async fn authorization_url(
		&self,
		state: &str,
		nonce: Option<&str>,
		code_challenge: Option<&str>,
	) -> Result<String, SocialAuthError> {
		let discovery = self.discover().await?;
		let challenge = code_challenge.map(|c| CodeChallenge::from_raw(c.to_string()));

		self.auth_flow.build_url(
			&discovery.authorization_endpoint,
			state,
			nonce,
			challenge.as_ref(),
		)
	}

	async fn exchange_code(
		&self,
		code: &str,
		code_verifier: Option<&str>,
	) -> Result<TokenResponse, SocialAuthError> {
		let discovery = self.discover().await?;
		let verifier = code_verifier.map(|v| CodeVerifier::from_raw(v.to_string()));

		self.exchange_with_extras(&discovery.token_endpoint, code, verifier.as_ref())
			.await
	}

	async fn refresh_token(&self, refresh_token: &str) -> Result<TokenResponse, SocialAuthError> {
		let discovery = self.discover().await?;
		self.refresh_flow
			.refresh(&discovery.token_endpoint, refresh_token)
			.await
	}

	async fn validate_id_token(
		&self,
		id_token: &str,
		nonce: Option<&str>,
	) -> Result<IdToken, SocialAuthError> {
		let discovery = self.discover().await?;
		let validator = self.id_token_validator().await?;
		validator
			.validate(id_token, &discovery.jwks_uri, nonce)
			.await
	}

	async fn get_user_info(&self, access_token: &str) -> Result<StandardClaims, SocialAuthError> {
		let discovery = self.discover().await?;
		let userinfo_endpoint = discovery.userinfo_endpoint.as_ref().ok_or_else(|| {
			SocialAuthError::InvalidConfiguration(
				"Provider's discovery document does not advertise a userinfo_endpoint".into(),
			)
		})?;

		// When a custom mapper is registered, fetch the raw JSON ourselves
		// and run it through the mapper. Otherwise, defer to the shared
		// UserInfoClient which deserializes directly into StandardClaims.
		match self.userinfo_mapper.as_ref() {
			None => {
				self.userinfo_client
					.get_user_info(userinfo_endpoint, access_token)
					.await
			}
			Some(mapper) => {
				validate_endpoint_url(userinfo_endpoint)?;
				let response = self
					.client
					.client()
					.get(userinfo_endpoint)
					.header("User-Agent", "reinhardt-auth")
					.bearer_auth(access_token)
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
						"UserInfo request failed ({}): {}",
						status, error_body
					)));
				}

				let raw: Value = response
					.json()
					.await
					.map_err(|e| SocialAuthError::UserInfoError(e.to_string()))?;
				mapper(&raw)
			}
		}
	}
}

/// Derives the issuer URL from a discovery URL by stripping the standard
/// `/.well-known/openid-configuration` suffix. This matches the convention
/// used by `GoogleProvider` / `MicrosoftProvider` / `AppleProvider`.
fn issuer_from_discovery_url(discovery_url: &str) -> String {
	discovery_url
		.trim_end_matches("/.well-known/openid-configuration")
		.to_string()
}

/// Translates an optional `std::time::Duration` into a `chrono::Duration`,
/// falling back to the default cache TTL when not provided. Negative or
/// zero overrides also fall back to the default to prevent immediate cache
/// expiration loops.
fn chrono_duration_or_default(ttl: Option<StdDuration>) -> ChronoDuration {
	match ttl {
		Some(d) if d.as_secs() > 0 => ChronoDuration::seconds(d.as_secs() as i64),
		_ => ChronoDuration::seconds(DEFAULT_CACHE_TTL_SECS),
	}
}

/// Computes the intersection of [`SUPPORTED_ASYMMETRIC_ALGORITHMS`] and the
/// provider's advertised algorithms. When the provider advertises none,
/// all supported asymmetric algorithms are allowed (matches OIDC's default
/// of RS256-only, but we are permissive so e.g. Keycloak's PS256-only
/// installs work out of the box).
fn compute_allowed_algorithms(advertised: Option<&[String]>) -> Vec<Algorithm> {
	let advertised = match advertised {
		Some(a) if !a.is_empty() => a,
		// Spec default per OIDC discovery 1.0 §3 is RS256.
		// We still restrict to the supported asymmetric set.
		_ => return SUPPORTED_ASYMMETRIC_ALGORITHMS.to_vec(),
	};

	advertised
		.iter()
		.filter_map(|alg| match alg.as_str() {
			"RS256" => Some(Algorithm::RS256),
			"RS384" => Some(Algorithm::RS384),
			"RS512" => Some(Algorithm::RS512),
			"PS256" => Some(Algorithm::PS256),
			"PS384" => Some(Algorithm::PS384),
			"PS512" => Some(Algorithm::PS512),
			// HS* and "none" are intentionally not mapped — they must
			// never be accepted for OIDC ID tokens.
			_ => None,
		})
		.collect()
}

/// Adapts a [`GenericOidcConfig`] into the existing [`ProviderConfig`] so
/// the shared flow components ([`AuthorizationFlow`], [`TokenExchangeFlow`],
/// [`RefreshFlow`]) can be reused unchanged.
fn build_provider_config(config: &GenericOidcConfig) -> ProviderConfig {
	use crate::social::core::config::OIDCConfig;

	ProviderConfig {
		name: config.name.clone(),
		client_id: config.client_id.clone(),
		client_secret: config.client_secret.clone(),
		redirect_uri: config.redirect_uri.clone(),
		scopes: config.scopes.clone(),
		oidc: Some(OIDCConfig {
			discovery_url: config.discovery_url.clone(),
			use_nonce: true,
		}),
		oauth2: None,
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;
	use std::collections::HashMap;
	use std::time::Duration as StdDuration;

	fn valid_config() -> GenericOidcConfig {
		GenericOidcConfig {
			name: "gitlab".to_string(),
			discovery_url: "https://gitlab.example.com/.well-known/openid-configuration".into(),
			client_id: "client-id".into(),
			client_secret: "client-secret".into(),
			redirect_uri: "https://app.example.com/auth/callback".into(),
			scopes: vec!["openid".into(), "email".into(), "profile".into()],
			discovery_ttl: None,
			jwks_ttl: None,
			extra_token_params: None,
		}
	}

	#[rstest]
	#[tokio::test]
	async fn new_succeeds_with_valid_config() {
		// Arrange
		let config = valid_config();

		// Act
		let provider = GenericOidcProvider::new(config).await;

		// Assert
		let provider = provider.expect("provider should construct");
		assert_eq!(provider.name(), "gitlab");
		assert!(provider.is_oidc());
	}

	#[rstest]
	#[case::empty_name("name")]
	#[case::empty_discovery_url("discovery_url")]
	#[case::empty_client_id("client_id")]
	#[case::empty_client_secret("client_secret")]
	#[case::empty_redirect_uri("redirect_uri")]
	#[tokio::test]
	async fn new_rejects_missing_required_field(#[case] field: &str) {
		// Arrange
		let mut config = valid_config();
		match field {
			"name" => config.name.clear(),
			"discovery_url" => config.discovery_url.clear(),
			"client_id" => config.client_id.clear(),
			"client_secret" => config.client_secret.clear(),
			"redirect_uri" => config.redirect_uri.clear(),
			other => panic!("unhandled field {}", other),
		}

		// Act
		let result = GenericOidcProvider::new(config).await;

		// Assert
		let err = result.err().expect("missing field must reject config");
		assert!(matches!(err, SocialAuthError::InvalidConfiguration(_)));
	}

	#[rstest]
	#[case::http_non_loopback("http://gitlab.example.com/.well-known/openid-configuration")]
	#[case::ftp_scheme("ftp://gitlab.example.com/.well-known/openid-configuration")]
	#[tokio::test]
	async fn new_rejects_insecure_discovery_url(#[case] url: &str) {
		// Arrange
		let mut config = valid_config();
		config.discovery_url = url.to_string();

		// Act
		let result = GenericOidcProvider::new(config).await;

		// Assert
		let err = result.err().expect("insecure URL must be rejected");
		assert!(matches!(err, SocialAuthError::InsecureEndpoint(_)));
	}

	#[rstest]
	#[tokio::test]
	async fn new_accepts_localhost_http_discovery_url_for_dev() {
		// Arrange
		let mut config = valid_config();
		config.discovery_url = "http://localhost:8080/.well-known/openid-configuration".to_string();

		// Act
		let result = GenericOidcProvider::new(config).await;

		// Assert
		assert!(
			result.is_ok(),
			"loopback HTTP must be allowed for local development"
		);
	}

	#[rstest]
	#[case::standard(
		"https://gitlab.com/.well-known/openid-configuration",
		"https://gitlab.com"
	)]
	#[case::with_subpath(
		"https://example.com/auth/.well-known/openid-configuration",
		"https://example.com/auth"
	)]
	#[case::no_well_known("https://example.com/issuer", "https://example.com/issuer")]
	fn issuer_from_discovery_url_strips_well_known_suffix(
		#[case] discovery_url: &str,
		#[case] expected: &str,
	) {
		// Arrange / Act
		let issuer = issuer_from_discovery_url(discovery_url);

		// Assert
		assert_eq!(issuer, expected);
	}

	#[rstest]
	fn chrono_duration_or_default_uses_default_when_none() {
		// Arrange / Act
		let result = chrono_duration_or_default(None);

		// Assert
		assert_eq!(result.num_seconds(), DEFAULT_CACHE_TTL_SECS);
	}

	#[rstest]
	fn chrono_duration_or_default_uses_default_when_zero() {
		// Arrange / Act
		let result = chrono_duration_or_default(Some(StdDuration::from_secs(0)));

		// Assert
		assert_eq!(result.num_seconds(), DEFAULT_CACHE_TTL_SECS);
	}

	#[rstest]
	fn chrono_duration_or_default_honors_explicit_value() {
		// Arrange / Act
		let result = chrono_duration_or_default(Some(StdDuration::from_secs(900)));

		// Assert
		assert_eq!(result.num_seconds(), 900);
	}

	#[rstest]
	fn compute_allowed_algorithms_falls_back_to_supported_set_when_none() {
		// Arrange / Act
		let allowed = compute_allowed_algorithms(None);

		// Assert
		assert_eq!(allowed, SUPPORTED_ASYMMETRIC_ALGORITHMS.to_vec());
	}

	#[rstest]
	fn compute_allowed_algorithms_falls_back_when_empty() {
		// Arrange
		let advertised: Vec<String> = vec![];

		// Act
		let allowed = compute_allowed_algorithms(Some(&advertised));

		// Assert
		assert_eq!(allowed, SUPPORTED_ASYMMETRIC_ALGORITHMS.to_vec());
	}

	#[rstest]
	fn compute_allowed_algorithms_intersects_with_advertised_set() {
		// Arrange
		let advertised = vec![
			"RS256".to_string(),
			"PS256".to_string(),
			// Unsupported entries must be filtered out, not error.
			"ES256".to_string(),
			"none".to_string(),
			"HS256".to_string(),
		];

		// Act
		let allowed = compute_allowed_algorithms(Some(&advertised));

		// Assert
		assert_eq!(allowed, vec![Algorithm::RS256, Algorithm::PS256]);
		assert!(
			!allowed.contains(&Algorithm::HS256),
			"HS* must never be allowed for OIDC ID tokens"
		);
	}

	#[rstest]
	fn compute_allowed_algorithms_rejects_only_unsafe_algs() {
		// Arrange — provider claims to support only HS / none.
		let advertised = vec!["HS256".to_string(), "none".to_string()];

		// Act
		let allowed = compute_allowed_algorithms(Some(&advertised));

		// Assert
		assert!(
			allowed.is_empty(),
			"all symmetric / none algorithms must be rejected"
		);
	}

	#[rstest]
	fn build_provider_config_propagates_oidc_settings() {
		// Arrange
		let cfg = valid_config();

		// Act
		let provider_config = build_provider_config(&cfg);

		// Assert
		assert_eq!(provider_config.name, cfg.name);
		assert_eq!(provider_config.client_id, cfg.client_id);
		assert_eq!(provider_config.redirect_uri, cfg.redirect_uri);
		assert!(provider_config.oidc.is_some(), "OIDC config required");
		assert!(provider_config.oauth2.is_none());
		let oidc = provider_config.oidc.unwrap();
		assert_eq!(oidc.discovery_url, cfg.discovery_url);
		assert!(oidc.use_nonce);
	}

	#[rstest]
	fn debug_redacts_client_secret() {
		// Arrange
		let config = valid_config();

		// Act
		let formatted = format!("{:?}", config);

		// Assert
		assert!(
			!formatted.contains("client-secret"),
			"client_secret must not appear in Debug output"
		);
		assert!(formatted.contains("<redacted>"));
	}

	#[rstest]
	#[tokio::test]
	async fn default_userinfo_mapping_handles_standard_claims() {
		// Arrange
		let raw = serde_json::json!({
			"sub": "user-1",
			"email": "user@example.com",
			"email_verified": true,
			"name": "User One",
			"given_name": "User",
			"family_name": "One",
			"picture": "https://example.com/u1.png",
			"locale": "en-US",
			"groups": ["admins"],
		});

		// Act
		let claims =
			GenericOidcProvider::default_map_userinfo(&raw).expect("default mapper succeeds");

		// Assert
		assert_eq!(claims.sub, "user-1");
		assert_eq!(claims.email.as_deref(), Some("user@example.com"));
		assert_eq!(claims.email_verified, Some(true));
		assert_eq!(claims.name.as_deref(), Some("User One"));
		assert_eq!(claims.given_name.as_deref(), Some("User"));
		assert_eq!(claims.family_name.as_deref(), Some("One"));
		assert_eq!(
			claims.picture.as_deref(),
			Some("https://example.com/u1.png")
		);
		assert_eq!(claims.locale.as_deref(), Some("en-US"));
		// Non-standard claims surface via additional_claims.
		assert!(claims.additional_claims.contains_key("groups"));
	}

	#[rstest]
	#[tokio::test]
	async fn default_userinfo_mapping_rejects_missing_subject() {
		// Arrange — `sub` is required by the OIDC spec.
		let raw = serde_json::json!({
			"email": "user@example.com",
		});

		// Act
		let result = GenericOidcProvider::default_map_userinfo(&raw);

		// Assert
		let err = result.err().expect("missing sub must error");
		assert!(matches!(err, SocialAuthError::UserInfoError(_)));
	}

	#[rstest]
	#[tokio::test]
	async fn with_userinfo_mapper_overrides_default_logic() {
		// Arrange — a custom mapper that pulls a non-standard "uid" field
		// into `sub` and stuffs additional_claims with the original payload.
		let provider = GenericOidcProvider::new(valid_config())
			.await
			.expect("provider construction must succeed")
			.with_userinfo_mapper(|raw: &Value| {
				let sub = raw
					.get("uid")
					.and_then(|v| v.as_str())
					.ok_or_else(|| SocialAuthError::UserMapping("missing uid".into()))?
					.to_string();
				let mut additional = HashMap::new();
				additional.insert("source".to_string(), Value::String("custom".into()));
				Ok(StandardClaims {
					sub,
					email: raw
						.get("mail")
						.and_then(|v| v.as_str())
						.map(|s| s.to_string()),
					email_verified: Some(true),
					name: None,
					given_name: None,
					family_name: None,
					picture: None,
					locale: None,
					additional_claims: additional,
				})
			});

		assert!(provider.userinfo_mapper.is_some(), "mapper must be set");

		// Act — invoke the mapper directly to verify wiring (full
		// HTTP round-trip is exercised in tests/generic_oidc_integration.rs).
		let raw = serde_json::json!({ "uid": "u-42", "mail": "u42@example.com" });
		let claims = (provider.userinfo_mapper.as_ref().expect("mapper installed"))(&raw)
			.expect("custom mapper succeeds");

		// Assert
		assert_eq!(claims.sub, "u-42");
		assert_eq!(claims.email.as_deref(), Some("u42@example.com"));
		assert_eq!(claims.email_verified, Some(true));
		assert_eq!(
			claims.additional_claims.get("source"),
			Some(&Value::String("custom".into()))
		);
	}

	#[rstest]
	#[tokio::test]
	async fn with_userinfo_mapper_can_propagate_errors() {
		// Arrange
		let provider = GenericOidcProvider::new(valid_config())
			.await
			.unwrap()
			.with_userinfo_mapper(|_raw: &Value| {
				Err(SocialAuthError::UserMapping("forced failure".into()))
			});

		// Act
		let raw = serde_json::json!({});
		let result = (provider.userinfo_mapper.as_ref().unwrap())(&raw);

		// Assert
		let err = result.err().expect("mapper must propagate errors");
		assert!(matches!(err, SocialAuthError::UserMapping(_)));
	}
}
