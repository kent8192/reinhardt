//! ID Token validation
//!
//! Validates OIDC ID tokens with signature and claims verification.

use std::sync::Arc;

use chrono::Utc;
use jsonwebtoken::{Algorithm, Validation, decode, decode_header};

use super::jwks::JwksCache;
use crate::social::core::{IdToken, SocialAuthError};

/// Default allowed algorithms for ID token validation
const DEFAULT_ALLOWED_ALGORITHMS: &[Algorithm] =
	&[Algorithm::RS256, Algorithm::RS384, Algorithm::RS512];

/// Configuration for ID token validation
#[derive(Debug, Clone)]
pub struct ValidationConfig {
	/// Expected issuer URL
	pub issuer: String,
	/// Expected audience (client ID)
	pub audience: String,
	/// Clock skew tolerance in seconds (default: 60)
	pub clock_skew: i64,
	/// Allowed algorithms for JWT validation
	pub allowed_algorithms: Vec<Algorithm>,
}

impl ValidationConfig {
	/// Creates a new validation configuration
	pub fn new(issuer: String, audience: String) -> Self {
		Self {
			issuer,
			audience,
			clock_skew: 60,
			allowed_algorithms: DEFAULT_ALLOWED_ALGORITHMS.to_vec(),
		}
	}

	/// Sets clock skew tolerance
	pub fn with_clock_skew(mut self, clock_skew: i64) -> Self {
		self.clock_skew = clock_skew;
		self
	}

	/// Sets allowed algorithms
	pub fn with_allowed_algorithms(mut self, algorithms: Vec<Algorithm>) -> Self {
		self.allowed_algorithms = algorithms;
		self
	}
}

/// ID token validator
pub struct IdTokenValidator {
	jwks_cache: Arc<JwksCache>,
	config: ValidationConfig,
}

impl IdTokenValidator {
	/// Creates a new ID token validator
	pub fn new(jwks_cache: Arc<JwksCache>, config: ValidationConfig) -> Self {
		Self { jwks_cache, config }
	}

	/// Validates an ID token
	///
	/// # Arguments
	///
	/// * `id_token` - The JWT ID token string
	/// * `jwks_uri` - The JWKS endpoint URI
	/// * `nonce` - Optional expected nonce value
	///
	/// # Returns
	///
	/// The validated ID token claims
	pub async fn validate(
		&self,
		id_token: &str,
		jwks_uri: &str,
		nonce: Option<&str>,
	) -> Result<IdToken, SocialAuthError> {
		// Decode header to get kid and algorithm
		let header = decode_header(id_token)
			.map_err(|e| SocialAuthError::InvalidIdToken(format!("Invalid JWT header: {}", e)))?;

		// Validate algorithm against whitelist
		if !self.config.allowed_algorithms.contains(&header.alg) {
			return Err(SocialAuthError::InvalidIdToken(format!(
				"Algorithm {:?} is not in the allowed list: {:?}",
				header.alg, self.config.allowed_algorithms
			)));
		}

		let kid = header.kid.ok_or_else(|| {
			SocialAuthError::InvalidIdToken("Missing kid in JWT header".to_string())
		})?;

		// Get decoding key from JWKS
		let decoding_key = self.jwks_cache.get_key(jwks_uri, &kid).await?;

		// Configure validation
		let mut validation = Validation::new(header.alg);
		validation.set_issuer(&[&self.config.issuer]);
		validation.set_audience(&[&self.config.audience]);
		validation.leeway = self.config.clock_skew as u64;

		// Decode and validate token
		let token_data = decode::<IdToken>(id_token, &decoding_key, &validation).map_err(|e| {
			SocialAuthError::InvalidIdToken(format!("JWT validation failed: {}", e))
		})?;

		let claims = token_data.claims;

		// Validate nonce if provided
		if let Some(expected_nonce) = nonce {
			match &claims.nonce {
				Some(token_nonce) if token_nonce == expected_nonce => {
					// Nonce matches
				}
				Some(token_nonce) => {
					return Err(SocialAuthError::InvalidIdToken(format!(
						"Nonce mismatch: expected {}, got {}",
						expected_nonce, token_nonce
					)));
				}
				None => {
					return Err(SocialAuthError::InvalidIdToken(
						"Nonce expected but not found in token".to_string(),
					));
				}
			}
		}

		// Verify expiration (should already be checked by jsonwebtoken, but double-check)
		let now = Utc::now().timestamp();
		if claims.exp < now - self.config.clock_skew {
			return Err(SocialAuthError::InvalidIdToken(
				"Token has expired".to_string(),
			));
		}

		Ok(claims)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::social::core::OAuth2Client;

	#[test]
	fn test_validation_config_creation() {
		let config =
			ValidationConfig::new("https://example.com".to_string(), "client_id".to_string());

		assert_eq!(config.issuer, "https://example.com");
		assert_eq!(config.audience, "client_id");
		assert_eq!(config.clock_skew, 60);
	}

	#[test]
	fn test_validation_config_with_clock_skew() {
		let config =
			ValidationConfig::new("https://example.com".to_string(), "client_id".to_string())
				.with_clock_skew(120);

		assert_eq!(config.clock_skew, 120);
	}

	#[tokio::test]
	async fn test_validator_creation() {
		let client = OAuth2Client::new();
		let jwks_cache = Arc::new(JwksCache::new(client));
		let config =
			ValidationConfig::new("https://example.com".to_string(), "client_id".to_string());

		let validator = IdTokenValidator::new(jwks_cache, config);
		assert_eq!(validator.config.issuer, "https://example.com");
	}

	// Integration tests with actual JWT tokens would require mock JWKS server
	// For now, we rely on manual testing with real providers
}
