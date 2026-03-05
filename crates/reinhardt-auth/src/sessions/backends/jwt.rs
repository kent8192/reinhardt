//! JWT-based session backend
//!
//! This module provides session storage using JSON Web Tokens (JWT).
//! Sessions are encoded as JWT tokens which can be stored in cookies or headers.
//!
//! ## Features
//!
//! - Stateless session storage (no server-side storage required)
//! - Configurable JWT algorithm (HS256, HS512, RS256, etc.)
//! - Built-in token expiration handling
//! - Secure token signing and verification
//!
//! ## Example
//!
//! ```rust
//! use reinhardt_auth::sessions::backends::{JwtSessionBackend, JwtConfig, SessionBackend};
//! use jsonwebtoken::Algorithm;
//! use serde_json::json;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Create JWT configuration
//! let config = JwtConfig::new("your-secret-key".to_string())
//!     .with_algorithm(Algorithm::HS256)
//!     .with_expiration(3600); // 1 hour
//!
//! // Create JWT session backend
//! let backend = JwtSessionBackend::new(config);
//!
//! // Store user session with login data
//! let session_data = json!({
//!     "user_id": 123,
//!     "username": "alice",
//!     "roles": ["user", "admin"],
//! });
//!
//! // Generate JWT token
//! backend.save("session_key", &session_data, Some(3600)).await?;
//!
//! // Verify and load session data from token
//! let loaded: Option<serde_json::Value> = backend.load("session_key").await?;
//! assert!(loaded.is_some());
//! assert_eq!(loaded.unwrap()["user_id"], 123);
//! # Ok(())
//! # }
//! ```

use async_trait::async_trait;
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use thiserror::Error;

/// JWT-specific session errors
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum JwtSessionError {
	#[error("JWT encoding error: {0}")]
	EncodingError(String),
	#[error("JWT decoding error: {0}")]
	DecodingError(String),
	#[error("Token not found: {0}")]
	TokenNotFound(String),
	#[error("Token expired")]
	TokenExpired,
	#[error("Invalid token")]
	InvalidToken,
}

/// JWT session configuration
///
/// ## Example
///
/// ```rust
/// use reinhardt_auth::sessions::backends::JwtConfig;
/// use jsonwebtoken::Algorithm;
///
/// // Basic configuration with HS256
/// let config = JwtConfig::new("my-secret-key".to_string());
///
/// // Advanced configuration
/// let config = JwtConfig::new("my-secret-key".to_string())
///     .with_algorithm(Algorithm::HS512)
///     .with_expiration(7200)
///     .with_issuer("my-app".to_string())
///     .with_audience("web-users".to_string());
/// ```
#[derive(Clone, PartialEq, Eq)]
pub struct JwtConfig {
	/// Secret key for signing tokens (for HS256, HS512, etc.)
	pub secret: String,
	/// JWT algorithm to use
	pub algorithm: Algorithm,
	/// Default token expiration in seconds
	pub expiration: u64,
	/// Token issuer (optional)
	pub issuer: Option<String>,
	/// Token audience (optional)
	pub audience: Option<String>,
}

impl std::fmt::Debug for JwtConfig {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("JwtConfig")
			.field("secret", &"[REDACTED]")
			.field("algorithm", &self.algorithm)
			.field("expiration", &self.expiration)
			.field("issuer", &self.issuer)
			.field("audience", &self.audience)
			.finish()
	}
}

impl JwtConfig {
	/// Create a new JWT configuration with default settings
	///
	/// Default algorithm: HS256
	/// Default expiration: 3600 seconds (1 hour)
	pub fn new(secret: String) -> Self {
		Self {
			secret,
			algorithm: Algorithm::HS256,
			expiration: 3600,
			issuer: None,
			audience: None,
		}
	}

	/// Set the JWT algorithm
	pub fn with_algorithm(mut self, algorithm: Algorithm) -> Self {
		self.algorithm = algorithm;
		self
	}

	/// Set the token expiration time in seconds
	pub fn with_expiration(mut self, expiration: u64) -> Self {
		self.expiration = expiration;
		self
	}

	/// Set the token issuer
	pub fn with_issuer(mut self, issuer: String) -> Self {
		self.issuer = Some(issuer);
		self
	}

	/// Set the token audience
	pub fn with_audience(mut self, audience: String) -> Self {
		self.audience = Some(audience);
		self
	}
}

/// JWT claims structure for session data
#[derive(Debug, Clone, Serialize, Deserialize)]
struct SessionClaims {
	/// Session data (JSON value)
	data: serde_json::Value,
	/// Token expiration time (UTC timestamp)
	exp: usize,
	/// Token issued at (UTC timestamp)
	iat: usize,
	/// Token issuer (optional)
	#[serde(skip_serializing_if = "Option::is_none")]
	iss: Option<String>,
	/// Token audience (optional)
	#[serde(skip_serializing_if = "Option::is_none")]
	aud: Option<String>,
}

/// JWT-based session backend
///
/// Provides stateless session storage using JSON Web Tokens.
/// Sessions are encoded as JWT tokens and can be verified without server-side storage.
///
/// ## Example
///
/// ```rust
/// use reinhardt_auth::sessions::backends::{JwtSessionBackend, JwtConfig, SessionBackend};
/// use jsonwebtoken::Algorithm;
/// use serde_json::json;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let config = JwtConfig::new("secret".to_string())
///     .with_algorithm(Algorithm::HS256);
///
/// let backend = JwtSessionBackend::new(config);
///
/// // Store session data as JWT token
/// let session_data = json!({
///     "user_id": 42,
///     "permissions": ["read", "write"],
/// });
///
/// backend.save("session_abc", &session_data, Some(3600)).await?;
///
/// // Verify and load session data
/// let loaded: Option<serde_json::Value> = backend.load("session_abc").await?;
/// assert_eq!(loaded.unwrap()["user_id"], 42);
/// # Ok(())
/// # }
/// ```
#[derive(Clone)]
pub struct JwtSessionBackend {
	config: Arc<JwtConfig>,
	// In-memory storage for session_key -> JWT token mapping
	// In real usage, tokens would be stored in cookies/headers
	tokens: Arc<RwLock<HashMap<String, String>>>,
}

impl JwtSessionBackend {
	/// Create a new JWT session backend with the given configuration
	pub fn new(config: JwtConfig) -> Self {
		Self {
			config: Arc::new(config),
			tokens: Arc::new(RwLock::new(HashMap::new())),
		}
	}

	/// Encode session data into a JWT token
	fn encode_token<T>(&self, data: &T, ttl: Option<u64>) -> Result<String, JwtSessionError>
	where
		T: Serialize,
	{
		let now = chrono::Utc::now().timestamp() as usize;
		let expiration = ttl.unwrap_or(self.config.expiration);

		// Serialize data to JSON
		let json_data = serde_json::to_value(data)
			.map_err(|e| JwtSessionError::EncodingError(e.to_string()))?;

		let claims = SessionClaims {
			data: json_data,
			exp: now + expiration as usize,
			iat: now,
			iss: self.config.issuer.clone(),
			aud: self.config.audience.clone(),
		};

		let header = Header::new(self.config.algorithm);
		let encoding_key = EncodingKey::from_secret(self.config.secret.as_bytes());

		encode(&header, &claims, &encoding_key)
			.map_err(|e| JwtSessionError::EncodingError(e.to_string()))
	}

	/// Decode and verify a JWT token, extracting session data
	fn decode_token<T>(&self, token: &str) -> Result<T, JwtSessionError>
	where
		T: for<'de> Deserialize<'de>,
	{
		let decoding_key = DecodingKey::from_secret(self.config.secret.as_bytes());

		let mut validation = Validation::new(self.config.algorithm);

		// Configure validation settings
		if let Some(ref issuer) = self.config.issuer {
			validation.set_issuer(&[issuer]);
		}
		if let Some(ref audience) = self.config.audience {
			validation.set_audience(&[audience]);
		}

		let token_data =
			decode::<SessionClaims>(token, &decoding_key, &validation).map_err(|e| {
				match e.kind() {
					jsonwebtoken::errors::ErrorKind::ExpiredSignature => {
						JwtSessionError::TokenExpired
					}
					_ => JwtSessionError::DecodingError(e.to_string()),
				}
			})?;

		// Deserialize the data field from claims
		serde_json::from_value(token_data.claims.data)
			.map_err(|e| JwtSessionError::DecodingError(e.to_string()))
	}
}

/// Session backend trait implementation for cache backends
use super::cache::{SessionBackend, SessionError};

#[async_trait]
impl SessionBackend for JwtSessionBackend {
	async fn load<T>(&self, session_key: &str) -> Result<Option<T>, SessionError>
	where
		T: for<'de> Deserialize<'de> + Send,
	{
		let tokens = self
			.tokens
			.read()
			.map_err(|e| SessionError::CacheError(format!("Lock error: {}", e)))?;

		if let Some(token) = tokens.get(session_key) {
			match self.decode_token::<T>(token) {
				Ok(data) => Ok(Some(data)),
				Err(JwtSessionError::TokenExpired) => Ok(None),
				Err(e) => Err(SessionError::CacheError(e.to_string())),
			}
		} else {
			Ok(None)
		}
	}

	async fn save<T>(
		&self,
		session_key: &str,
		data: &T,
		ttl: Option<u64>,
	) -> Result<(), SessionError>
	where
		T: Serialize + Send + Sync,
	{
		let token = self
			.encode_token(data, ttl)
			.map_err(|e| SessionError::SerializationError(e.to_string()))?;

		let mut tokens = self
			.tokens
			.write()
			.map_err(|e| SessionError::CacheError(format!("Lock error: {}", e)))?;

		tokens.insert(session_key.to_string(), token);
		Ok(())
	}

	async fn delete(&self, session_key: &str) -> Result<(), SessionError> {
		let mut tokens = self
			.tokens
			.write()
			.map_err(|e| SessionError::CacheError(format!("Lock error: {}", e)))?;

		tokens.remove(session_key);
		Ok(())
	}

	async fn exists(&self, session_key: &str) -> Result<bool, SessionError> {
		let tokens = self
			.tokens
			.read()
			.map_err(|e| SessionError::CacheError(format!("Lock error: {}", e)))?;

		if let Some(token) = tokens.get(session_key) {
			// Check if token is valid and not expired
			match self.decode_token::<serde_json::Value>(token) {
				Ok(_) => Ok(true),
				Err(JwtSessionError::TokenExpired) => Ok(false),
				Err(_) => Ok(false),
			}
		} else {
			Ok(false)
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use jsonwebtoken::{EncodingKey, Header, encode};
	use serde_json::json;

	#[tokio::test]
	async fn test_jwt_session_save_and_load() {
		let config = JwtConfig::new("test-secret".to_string());
		let backend = JwtSessionBackend::new(config);

		let session_data = json!({
			"user_id": 123,
			"username": "test_user",
		});

		// Save session
		backend
			.save("test_session", &session_data, Some(3600))
			.await
			.unwrap();

		// Load session
		let loaded: Option<serde_json::Value> = backend.load("test_session").await.unwrap();
		assert!(loaded.is_some());
		assert_eq!(loaded.unwrap()["user_id"], 123);
	}

	#[tokio::test]
	async fn test_jwt_session_expiration() {
		let config = JwtConfig::new("test-secret".to_string());
		let backend = JwtSessionBackend::new(config.clone());

		// Manually create an expired token by directly manipulating the tokens map
		let now = chrono::Utc::now().timestamp() as usize;
		let expired_claims = SessionClaims {
			data: json!({
				"user_id": 456,
			}),
			exp: now - 3600, // Expired 1 hour ago
			iat: now - 7200, // Issued 2 hours ago
			iss: None,
			aud: None,
		};

		let header = Header::new(config.algorithm);
		let encoding_key = EncodingKey::from_secret(config.secret.as_bytes());
		let expired_token = encode(&header, &expired_claims, &encoding_key).unwrap();

		// Insert expired token directly
		backend
			.tokens
			.write()
			.unwrap()
			.insert("expired_session".to_string(), expired_token);

		// Load should return None for expired token
		let loaded: Option<serde_json::Value> = backend.load("expired_session").await.unwrap();
		assert!(loaded.is_none());
	}

	#[tokio::test]
	async fn test_jwt_session_delete() {
		let config = JwtConfig::new("test-secret".to_string());
		let backend = JwtSessionBackend::new(config);

		let session_data = json!({
			"user_id": 789,
		});

		// Save session
		backend
			.save("delete_test", &session_data, Some(3600))
			.await
			.unwrap();

		// Verify exists
		assert!(backend.exists("delete_test").await.unwrap());

		// Delete session
		backend.delete("delete_test").await.unwrap();

		// Verify deleted
		assert!(!backend.exists("delete_test").await.unwrap());
	}

	#[tokio::test]
	async fn test_jwt_session_exists() {
		let config = JwtConfig::new("test-secret".to_string());
		let backend = JwtSessionBackend::new(config);

		// Non-existent session
		assert!(!backend.exists("non_existent").await.unwrap());

		let session_data = json!({
			"user_id": 999,
		});

		// Save session
		backend
			.save("exists_test", &session_data, Some(3600))
			.await
			.unwrap();

		// Verify exists
		assert!(backend.exists("exists_test").await.unwrap());
	}

	#[tokio::test]
	async fn test_jwt_with_different_algorithms() {
		let config = JwtConfig::new("test-secret".to_string()).with_algorithm(Algorithm::HS512);

		let backend = JwtSessionBackend::new(config);

		let session_data = json!({
			"user_id": 111,
		});

		backend
			.save("hs512_test", &session_data, Some(3600))
			.await
			.unwrap();

		let loaded: Option<serde_json::Value> = backend.load("hs512_test").await.unwrap();
		assert!(loaded.is_some());
		assert_eq!(loaded.unwrap()["user_id"], 111);
	}

	#[tokio::test]
	async fn test_jwt_with_issuer_and_audience() {
		let config = JwtConfig::new("test-secret".to_string())
			.with_issuer("test-app".to_string())
			.with_audience("test-users".to_string());

		let backend = JwtSessionBackend::new(config);

		let session_data = json!({
			"user_id": 222,
		});

		backend
			.save("iss_aud_test", &session_data, Some(3600))
			.await
			.unwrap();

		let loaded: Option<serde_json::Value> = backend.load("iss_aud_test").await.unwrap();
		assert!(loaded.is_some());
		assert_eq!(loaded.unwrap()["user_id"], 222);
	}

	#[tokio::test]
	async fn test_jwt_session_complex_data() {
		let config = JwtConfig::new("test-secret".to_string());
		let backend = JwtSessionBackend::new(config);

		let session_data = json!({
			"user_id": 333,
			"username": "complex_user",
			"roles": ["admin", "editor"],
			"metadata": {
				"last_login": "2024-01-01T00:00:00Z",
				"preferences": {
					"theme": "dark",
					"language": "en"
				}
			}
		});

		backend
			.save("complex_test", &session_data, Some(3600))
			.await
			.unwrap();

		let loaded: Option<serde_json::Value> = backend.load("complex_test").await.unwrap();
		let data = loaded.unwrap();
		assert_eq!(data["user_id"], 333);
		assert_eq!(data["roles"][0], "admin");
		assert_eq!(data["metadata"]["preferences"]["theme"], "dark");
	}
}
