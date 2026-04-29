//! JSON Web Key Set (JWKS) management
//!
//! Fetches and caches public keys for JWT verification.

use std::collections::HashMap;

use chrono::{DateTime, Duration, Utc};
use jsonwebtoken::DecodingKey;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use crate::social::core::{OAuth2Client, SocialAuthError};

/// A single JSON Web Key
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Jwk {
	/// Key type (e.g., "RSA", "EC")
	pub kty: String,

	/// Key ID
	#[serde(skip_serializing_if = "Option::is_none")]
	pub kid: Option<String>,

	/// Public key use (e.g., "sig" for signature)
	#[serde(rename = "use", skip_serializing_if = "Option::is_none")]
	pub use_: Option<String>,

	/// Algorithm (e.g., "RS256")
	#[serde(skip_serializing_if = "Option::is_none")]
	pub alg: Option<String>,

	/// RSA modulus (Base64URL encoded)
	#[serde(skip_serializing_if = "Option::is_none")]
	pub n: Option<String>,

	/// RSA exponent (Base64URL encoded)
	#[serde(skip_serializing_if = "Option::is_none")]
	pub e: Option<String>,

	/// EC curve (e.g., "P-256")
	#[serde(skip_serializing_if = "Option::is_none")]
	pub crv: Option<String>,

	/// EC x coordinate (Base64URL encoded)
	#[serde(skip_serializing_if = "Option::is_none")]
	pub x: Option<String>,

	/// EC y coordinate (Base64URL encoded)
	#[serde(skip_serializing_if = "Option::is_none")]
	pub y: Option<String>,
}

impl Jwk {
	/// Converts this JWK to a DecodingKey for use with jsonwebtoken
	pub fn to_decoding_key(&self) -> Result<DecodingKey, SocialAuthError> {
		match self.kty.as_str() {
			"RSA" => {
				let n = self.n.as_ref().ok_or_else(|| {
					SocialAuthError::InvalidJwk("Missing RSA modulus (n)".to_string())
				})?;
				let e = self.e.as_ref().ok_or_else(|| {
					SocialAuthError::InvalidJwk("Missing RSA exponent (e)".to_string())
				})?;

				DecodingKey::from_rsa_components(n, e)
					.map_err(|e| SocialAuthError::InvalidJwk(e.to_string()))
			}
			"EC" => {
				let crv = self.crv.as_ref().ok_or_else(|| {
					SocialAuthError::InvalidJwk("Missing EC curve (crv)".to_string())
				})?;
				let x = self.x.as_ref().ok_or_else(|| {
					SocialAuthError::InvalidJwk("Missing EC x coordinate".to_string())
				})?;
				let y = self.y.as_ref().ok_or_else(|| {
					SocialAuthError::InvalidJwk("Missing EC y coordinate".to_string())
				})?;
				match crv.as_str() {
					"P-256" | "P-384" | "P-521" => DecodingKey::from_ec_components(x, y)
						.map_err(|e| SocialAuthError::InvalidJwk(e.to_string())),
					other => Err(SocialAuthError::InvalidJwk(format!(
						"Unsupported EC curve: {}",
						other
					))),
				}
			}
			other => Err(SocialAuthError::InvalidJwk(format!(
				"Unsupported key type: {}",
				other
			))),
		}
	}
}

/// A set of JSON Web Keys
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JwkSet {
	/// Array of keys
	pub keys: Vec<Jwk>,
}

impl JwkSet {
	/// Finds a key by its Key ID
	pub fn find_key(&self, kid: &str) -> Option<&Jwk> {
		self.keys.iter().find(|jwk| jwk.kid.as_deref() == Some(kid))
	}
}

/// Cached JWKS with expiration
#[derive(Debug, Clone)]
struct CachedJwks {
	jwks: JwkSet,
	expires_at: DateTime<Utc>,
}

impl CachedJwks {
	fn new(jwks: JwkSet, ttl: Duration) -> Self {
		Self {
			jwks,
			expires_at: Utc::now() + ttl,
		}
	}

	fn is_expired(&self) -> bool {
		Utc::now() > self.expires_at
	}
}

/// JWKS cache with automatic fetching and caching, keyed by jwks_uri
pub struct JwksCache {
	client: OAuth2Client,
	cache: RwLock<HashMap<String, CachedJwks>>,
	cache_ttl: Duration,
}

impl JwksCache {
	/// Creates a new JWKS cache with default TTL (1 hour)
	pub fn new(client: OAuth2Client) -> Self {
		Self {
			client,
			cache: RwLock::new(HashMap::new()),
			cache_ttl: Duration::hours(1),
		}
	}

	/// Creates a new JWKS cache with custom TTL
	pub fn with_ttl(client: OAuth2Client, cache_ttl: Duration) -> Self {
		Self {
			client,
			cache: RwLock::new(HashMap::new()),
			cache_ttl,
		}
	}

	/// Fetches JWKS from the given URI
	async fn fetch_jwks(&self, jwks_uri: &str) -> Result<JwkSet, SocialAuthError> {
		let response = self
			.client
			.client()
			.get(jwks_uri)
			.send()
			.await
			.map_err(|e| SocialAuthError::Network(e.to_string()))?;

		if !response.status().is_success() {
			return Err(SocialAuthError::Jwks(format!(
				"JWKS fetch failed: {}",
				response.status()
			)));
		}

		let jwks: JwkSet = response
			.json()
			.await
			.map_err(|e| SocialAuthError::InvalidJwk(e.to_string()))?;

		Ok(jwks)
	}

	/// Gets a decoding key for the given Key ID
	///
	/// # Arguments
	///
	/// * `jwks_uri` - The JWKS endpoint URI
	/// * `kid` - The Key ID to retrieve
	///
	/// # Returns
	///
	/// A DecodingKey for use with jsonwebtoken
	pub async fn get_key(&self, jwks_uri: &str, kid: &str) -> Result<DecodingKey, SocialAuthError> {
		// Check cache first
		{
			let cache = self.cache.read().await;
			if let Some(cached) = cache.get(jwks_uri)
				&& !cached.is_expired()
				&& let Some(jwk) = cached.jwks.find_key(kid)
			{
				return jwk.to_decoding_key();
			}
		}

		// Fetch from network
		let jwks = self.fetch_jwks(jwks_uri).await?;

		// Update cache keyed by jwks_uri
		{
			let mut cache = self.cache.write().await;
			cache.insert(
				jwks_uri.to_string(),
				CachedJwks::new(jwks.clone(), self.cache_ttl),
			);
		}

		// Find the key
		let jwk = jwks
			.find_key(kid)
			.ok_or_else(|| SocialAuthError::InvalidJwk(format!("Key ID not found: {}", kid)))?;

		jwk.to_decoding_key()
	}

	/// Clears the cache
	pub async fn clear_cache(&self) {
		let mut cache = self.cache.write().await;
		cache.clear();
	}
}

#[cfg(test)]
mod tests {
	use rstest::rstest;

	use super::*;

	/// Builds a minimally-populated EC [`Jwk`] for use in tests.
	fn ec_jwk(crv: Option<&str>, x: Option<&str>, y: Option<&str>) -> Jwk {
		Jwk {
			kty: "EC".to_string(),
			kid: Some("ec-test".to_string()),
			use_: Some("sig".to_string()),
			alg: None,
			n: None,
			e: None,
			crv: crv.map(str::to_string),
			x: x.map(str::to_string),
			y: y.map(str::to_string),
		}
	}

	// Real EC public-key coordinates (Base64URL, no padding) generated via
	// `openssl ecparam -name <curve> -genkey` and the uncompressed point
	// encoding `0x04 || X || Y`. Using genuine on-curve points lets
	// `DecodingKey::from_ec_components` validate the byte layout end-to-end.
	const P256_X: &str = "3El72Z5v8sF_yAFl4X-oBwzdqNo2fSUGgnF9Op3jW_Y";
	const P256_Y: &str = "ShFzmdJhPvr4CTie59tn5yi6TB4CeyQtu52iZ5QiG2Y";
	const P384_X: &str = "biuKLTSSYW309rbLeZq2c1jcH5FG5DOpaabLO5sHZMMt8RmXPpP8kYOkpY85Sc9r";
	const P384_Y: &str = "r3zcUmyzZtfrU8CHuSJFa-NPyLdbSJXJq7HRjpgjHSG6c1MLSDh2UZrFnqTSketK";
	const P521_X: &str =
		"AHRnYq_KUnouQZLJDcZY-e5fhMq1YvkvjQPClW2NdxlQWaRbs9VVahJ9i2jDarxyFb4gPHZfACMiBxh31-hXQ4ca";
	const P521_Y: &str =
		"AUHo3s3341w1Vl8-3qMz1qXm5-G5NrOZWqzXC63naeOZVRVzo6nW5Xa4nwVBA5FCZu8uZIbQYw24AdINRnb7RBM7";

	#[rstest]
	#[case::p256("P-256", P256_X, P256_Y)]
	#[case::p384("P-384", P384_X, P384_Y)]
	#[case::p521("P-521", P521_X, P521_Y)]
	fn ec_jwk_to_decoding_key_succeeds_for_supported_curves(
		#[case] crv: &str,
		#[case] x: &str,
		#[case] y: &str,
	) {
		// Arrange
		let jwk = ec_jwk(Some(crv), Some(x), Some(y));

		// Act
		let result = jwk.to_decoding_key();

		// Assert
		assert!(
			result.is_ok(),
			"expected EC JWK on curve {crv} to convert successfully, got {:?}",
			result.err()
		);
	}

	#[rstest]
	fn ec_jwk_missing_crv_returns_invalid_jwk_error() {
		// Arrange
		let jwk = ec_jwk(None, Some(P256_X), Some(P256_Y));

		// Act
		let err = jwk
			.to_decoding_key()
			.expect_err("expected InvalidJwk error");

		// Assert
		match err {
			SocialAuthError::InvalidJwk(msg) => assert!(
				msg.contains("crv"),
				"expected error message to mention 'crv', got: {msg}"
			),
			other => panic!("expected SocialAuthError::InvalidJwk, got {other:?}"),
		}
	}

	#[rstest]
	fn ec_jwk_missing_x_returns_invalid_jwk_error() {
		// Arrange
		let jwk = ec_jwk(Some("P-256"), None, Some(P256_Y));

		// Act
		let err = jwk
			.to_decoding_key()
			.expect_err("expected InvalidJwk error");

		// Assert
		match err {
			SocialAuthError::InvalidJwk(msg) => assert!(
				msg.contains("x coordinate"),
				"expected error message to mention 'x coordinate', got: {msg}"
			),
			other => panic!("expected SocialAuthError::InvalidJwk, got {other:?}"),
		}
	}

	#[rstest]
	fn ec_jwk_missing_y_returns_invalid_jwk_error() {
		// Arrange
		let jwk = ec_jwk(Some("P-256"), Some(P256_X), None);

		// Act
		let err = jwk
			.to_decoding_key()
			.expect_err("expected InvalidJwk error");

		// Assert
		match err {
			SocialAuthError::InvalidJwk(msg) => assert!(
				msg.contains("y coordinate"),
				"expected error message to mention 'y coordinate', got: {msg}"
			),
			other => panic!("expected SocialAuthError::InvalidJwk, got {other:?}"),
		}
	}

	#[rstest]
	fn ec_jwk_unsupported_curve_returns_invalid_jwk_error() {
		// Arrange
		// secp256k1 is the Bitcoin curve; not part of the JOSE family
		// this provider supports.
		let jwk = ec_jwk(Some("secp256k1"), Some(P256_X), Some(P256_Y));

		// Act
		let err = jwk
			.to_decoding_key()
			.expect_err("expected InvalidJwk error");

		// Assert
		match err {
			SocialAuthError::InvalidJwk(msg) => assert!(
				msg.contains("Unsupported EC curve"),
				"expected error message to mention 'Unsupported EC curve', got: {msg}"
			),
			other => panic!("expected SocialAuthError::InvalidJwk, got {other:?}"),
		}
	}

	#[test]
	fn test_jwk_set_find_key() {
		let jwks = JwkSet {
			keys: vec![
				Jwk {
					kty: "RSA".to_string(),
					kid: Some("key1".to_string()),
					use_: Some("sig".to_string()),
					alg: Some("RS256".to_string()),
					n: Some("test_n".to_string()),
					e: Some("test_e".to_string()),
					crv: None,
					x: None,
					y: None,
				},
				Jwk {
					kty: "RSA".to_string(),
					kid: Some("key2".to_string()),
					use_: Some("sig".to_string()),
					alg: Some("RS256".to_string()),
					n: Some("test_n2".to_string()),
					e: Some("test_e2".to_string()),
					crv: None,
					x: None,
					y: None,
				},
			],
		};

		assert!(jwks.find_key("key1").is_some());
		assert!(jwks.find_key("key2").is_some());
		assert!(jwks.find_key("key3").is_none());
	}

	#[test]
	fn test_cached_jwks_expiration() {
		let jwks = JwkSet { keys: vec![] };

		let cached = CachedJwks::new(jwks.clone(), Duration::seconds(1));
		assert!(!cached.is_expired());

		let expired = CachedJwks::new(jwks, Duration::seconds(-1));
		assert!(expired.is_expired());
	}

	#[tokio::test]
	async fn test_cache_creation() {
		let client = OAuth2Client::new();
		let cache = JwksCache::new(client);
		assert!(cache.cache.read().await.is_empty());
	}

	#[tokio::test]
	async fn test_cache_with_custom_ttl() {
		let client = OAuth2Client::new();
		let cache = JwksCache::with_ttl(client, Duration::minutes(30));
		assert_eq!(cache.cache_ttl, Duration::minutes(30));
	}

	#[tokio::test]
	async fn test_clear_cache() {
		let client = OAuth2Client::new();
		let cache = JwksCache::new(client);

		// Manually set cache with a key
		{
			let jwks = JwkSet { keys: vec![] };
			let mut cache_lock = cache.cache.write().await;
			cache_lock.insert(
				"https://example.com/jwks".to_string(),
				CachedJwks::new(jwks, Duration::hours(1)),
			);
		}

		assert!(!cache.cache.read().await.is_empty());

		cache.clear_cache().await;
		assert!(cache.cache.read().await.is_empty());
	}
}
