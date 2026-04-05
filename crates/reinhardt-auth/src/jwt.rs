// This module uses the deprecated User trait for backward compatibility.
// JwtAuth returns Box<dyn User> to preserve existing authentication APIs.
#![allow(deprecated)]
use crate::rest_authentication::RestAuthentication;
use crate::{AuthenticationBackend, AuthenticationError, SimpleUser, User};
use chrono::{Duration, Utc};
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation, decode, encode};
use reinhardt_http::Request;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

/// JWT-specific errors with distinct variants for each failure mode.
///
/// This enum allows callers to programmatically distinguish between
/// token expiration, signature failures, and other token issues.
///
/// # Examples
///
/// ```
/// use reinhardt_auth::jwt::{JwtAuth, JwtError};
///
/// let jwt_auth = JwtAuth::new(b"secret");
/// let result = jwt_auth.verify_token("invalid.token.here");
///
/// match result {
///     Ok(claims) => println!("Valid: {}", claims.sub),
///     Err(JwtError::TokenExpired) => println!("Token has expired"),
///     Err(JwtError::InvalidSignature(_)) => println!("Signature mismatch"),
///     Err(e) => println!("Other error: {}", e),
/// }
/// ```
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum JwtError {
	/// The token has expired.
	#[error("Token expired")]
	TokenExpired,
	/// The token signature is invalid (wrong secret or tampered).
	#[error("Invalid signature: {0}")]
	InvalidSignature(String),
	/// The token is malformed or cannot be decoded.
	#[error("Invalid token: {0}")]
	InvalidToken(String),
	/// An error occurred during token encoding.
	#[error("Encoding error: {0}")]
	EncodingError(String),
}

impl From<jsonwebtoken::errors::Error> for JwtError {
	fn from(err: jsonwebtoken::errors::Error) -> Self {
		match err.kind() {
			jsonwebtoken::errors::ErrorKind::ExpiredSignature => JwtError::TokenExpired,
			jsonwebtoken::errors::ErrorKind::InvalidSignature
			| jsonwebtoken::errors::ErrorKind::InvalidRsaKey(_)
			| jsonwebtoken::errors::ErrorKind::InvalidEcdsaKey => {
				JwtError::InvalidSignature(err.to_string())
			}
			_ => JwtError::InvalidToken(err.to_string()),
		}
	}
}

/// JWT Claims
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
	/// Subject (user ID).
	pub sub: String,
	/// Expiration time (Unix timestamp).
	pub exp: i64,
	/// Issued at time (Unix timestamp).
	pub iat: i64,
	/// The username associated with this token.
	pub username: String,
	/// Whether the user has staff access.
	#[serde(default)]
	pub is_staff: bool,
	/// Whether the user has superuser access.
	#[serde(default)]
	pub is_superuser: bool,
}

impl Claims {
	/// Creates a new JWT Claims with user information and expiration time.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_auth::jwt::Claims;
	/// use chrono::Duration;
	///
	/// let claims = Claims::new(
	///     "user123".to_string(),
	///     "john_doe".to_string(),
	///     Duration::hours(24),
	///     false,
	///     false,
	/// );
	///
	/// assert_eq!(claims.sub, "user123");
	/// assert_eq!(claims.username, "john_doe");
	/// assert!(claims.exp > claims.iat);
	/// ```
	pub fn new(
		user_id: String,
		username: String,
		expires_in: Duration,
		is_staff: bool,
		is_superuser: bool,
	) -> Self {
		let now = Utc::now();
		Self {
			sub: user_id,
			username,
			iat: now.timestamp(),
			exp: (now + expires_in).timestamp(),
			is_staff,
			is_superuser,
		}
	}
	/// Checks if the JWT claims have expired.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_auth::jwt::Claims;
	/// use chrono::Duration;
	///
	/// let claims = Claims::new(
	///     "user123".to_string(),
	///     "john_doe".to_string(),
	///     Duration::hours(24),
	///     false,
	///     false,
	/// );
	///
	/// assert!(!claims.is_expired());
	/// ```
	pub fn is_expired(&self) -> bool {
		Utc::now().timestamp() > self.exp
	}
}

/// JWT Authentication handler
#[derive(Clone)]
pub struct JwtAuth {
	encoding_key: EncodingKey,
	decoding_key: DecodingKey,
	validation: Validation,
	validation_allow_expired: Validation,
}

impl JwtAuth {
	/// Creates a new JWT authentication handler with the given secret key.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_auth::jwt::JwtAuth;
	///
	/// let secret = b"my-secret-key-12345";
	/// let jwt_auth = JwtAuth::new(secret);
	/// ```
	pub fn new(secret: &[u8]) -> Self {
		let mut validation_allow_expired = Validation::default();
		validation_allow_expired.validate_exp = false;
		Self {
			encoding_key: EncodingKey::from_secret(secret),
			decoding_key: DecodingKey::from_secret(secret),
			validation: Validation::default(),
			validation_allow_expired,
		}
	}
	/// Encodes JWT claims into a token string.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_auth::jwt::{JwtAuth, Claims};
	/// use chrono::Duration;
	///
	/// let jwt_auth = JwtAuth::new(b"secret");
	/// let claims = Claims::new(
	///     "user123".to_string(),
	///     "john".to_string(),
	///     Duration::hours(1),
	///     false,
	///     false,
	/// );
	///
	/// let token = jwt_auth.encode(&claims).unwrap();
	/// assert!(!token.is_empty());
	/// ```
	pub fn encode(&self, claims: &Claims) -> Result<String, JwtError> {
		encode(&Header::default(), claims, &self.encoding_key)
			.map_err(|e| JwtError::EncodingError(e.to_string()))
	}
	/// Decodes a JWT token string into claims.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_auth::jwt::{JwtAuth, Claims};
	/// use chrono::Duration;
	///
	/// let jwt_auth = JwtAuth::new(b"secret");
	/// let claims = Claims::new(
	///     "user123".to_string(),
	///     "john".to_string(),
	///     Duration::hours(1),
	///     false,
	///     false,
	/// );
	///
	/// let token = jwt_auth.encode(&claims).unwrap();
	/// let decoded = jwt_auth.decode(&token).unwrap();
	/// assert_eq!(decoded.sub, "user123");
	/// ```
	pub fn decode(&self, token: &str) -> Result<Claims, JwtError> {
		decode::<Claims>(token, &self.decoding_key, &self.validation)
			.map(|data| data.claims)
			.map_err(JwtError::from)
	}
	/// Generates a JWT token for the given user with 24-hour expiration.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_auth::jwt::JwtAuth;
	///
	/// let jwt_auth = JwtAuth::new(b"secret");
	/// let token = jwt_auth.generate_token(
	///     "user123".to_string(),
	///     "john_doe".to_string(),
	///     false,
	///     false,
	/// ).unwrap();
	///
	/// assert!(!token.is_empty());
	/// assert!(token.contains('.'));
	/// ```
	pub fn generate_token(
		&self,
		user_id: String,
		username: String,
		is_staff: bool,
		is_superuser: bool,
	) -> Result<String, JwtError> {
		let claims = Claims::new(
			user_id,
			username,
			Duration::hours(24),
			is_staff,
			is_superuser,
		);
		self.encode(&claims)
	}
	/// Verifies a JWT token and returns the claims if valid and not expired.
	///
	/// Returns [`JwtError::TokenExpired`] if the token has expired.
	/// This method applies a strict zero-leeway expiration check.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_auth::jwt::{JwtAuth, JwtError};
	///
	/// let jwt_auth = JwtAuth::new(b"secret");
	/// let token = jwt_auth.generate_token(
	///     "user123".to_string(),
	///     "john_doe".to_string(),
	///     false,
	///     false,
	/// ).unwrap();
	///
	/// let claims = jwt_auth.verify_token(&token).unwrap();
	/// assert_eq!(claims.sub, "user123");
	/// assert_eq!(claims.username, "john_doe");
	/// ```
	pub fn verify_token(&self, token: &str) -> Result<Claims, JwtError> {
		let claims = self.decode(token)?;

		if claims.is_expired() {
			return Err(JwtError::TokenExpired);
		}

		Ok(claims)
	}
	/// Verifies a JWT token signature without checking expiration.
	///
	/// Useful for token refresh flows where the caller needs to read
	/// claims from an expired token to issue a new one.
	/// The token's signature and structure are still validated.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_auth::jwt::JwtAuth;
	///
	/// let jwt_auth = JwtAuth::new(b"secret");
	/// let token = jwt_auth.generate_token(
	///     "user123".to_string(),
	///     "john_doe".to_string(),
	///     false,
	///     false,
	/// ).unwrap();
	///
	/// // Even after token expires, claims can be read for refresh
	/// let claims = jwt_auth.verify_token_allow_expired(&token).unwrap();
	/// assert_eq!(claims.sub, "user123");
	/// ```
	pub fn verify_token_allow_expired(&self, token: &str) -> Result<Claims, JwtError> {
		decode::<Claims>(token, &self.decoding_key, &self.validation_allow_expired)
			.map(|data| data.claims)
			.map_err(JwtError::from)
	}
}

// Implement REST API Authentication trait
#[async_trait::async_trait]
impl RestAuthentication for JwtAuth {
	async fn authenticate(
		&self,
		request: &Request,
	) -> Result<Option<Box<dyn User>>, AuthenticationError> {
		// Get Authorization header
		let auth_header = request
			.headers
			.get("Authorization")
			.and_then(|h| h.to_str().ok());

		if let Some(header) = auth_header {
			// Check for Bearer token
			if let Some(token) = header.strip_prefix("Bearer ") {
				// Verify and decode token
				match self.verify_token(token) {
					Ok(claims) => {
						// Parse user ID from claims, returning an error for malformed values
						let id = Uuid::parse_str(&claims.sub)
							.map_err(|_| AuthenticationError::InvalidToken)?;
						// Create user from claims
						return Ok(Some(Box::new(SimpleUser {
							id,
							username: claims.username.clone(),
							email: String::new(),
							// Security defaults: privilege flags are set to restrictive values
							// since JWT claims alone cannot determine user privileges.
							// Use UserRepository integration for accurate privilege data.
							is_active: true,
							is_admin: false,
							is_staff: false,
							is_superuser: false,
						})));
					}
					Err(err) => {
						return Err(AuthenticationError::from(err));
					}
				}
			}
		}

		Ok(None)
	}
}

// Implement AuthenticationBackend trait
#[async_trait::async_trait]
impl AuthenticationBackend for JwtAuth {
	async fn authenticate(
		&self,
		request: &Request,
	) -> Result<Option<Box<dyn User>>, AuthenticationError> {
		// Delegate to REST API Authentication trait implementation
		<Self as RestAuthentication>::authenticate(self, request).await
	}

	async fn get_user(&self, _user_id: &str) -> Result<Option<Box<dyn User>>, AuthenticationError> {
		// JWT authentication doesn't support get_user by ID
		// It only authenticates via token validation
		// Return None to indicate this backend doesn't support user retrieval
		Ok(None)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use bytes::Bytes;
	use hyper::{HeaderMap, Method};
	use reinhardt_http::Request;
	use rstest::rstest;
	/// Helper to create a request with a given Authorization header value.
	fn create_request_with_bearer(token: &str) -> Request {
		let mut headers = HeaderMap::new();
		headers.insert(
			"Authorization",
			format!("Bearer {}", token).parse().unwrap(),
		);
		Request::builder()
			.method(Method::GET)
			.uri("/api/resource")
			.headers(headers)
			.body(Bytes::new())
			.build()
			.unwrap()
	}

	#[rstest]
	#[tokio::test]
	async fn test_authenticate_with_valid_uuid_sub_claim() {
		// Arrange
		let jwt_auth = JwtAuth::new(b"test-secret-key-256bit!");
		let user_id = "550e8400-e29b-41d4-a716-446655440000";
		let username = "alice";
		let token = jwt_auth
			.generate_token(user_id.to_string(), username.to_string(), false, false)
			.unwrap();
		let request = create_request_with_bearer(&token);

		// Act
		let result = RestAuthentication::authenticate(&jwt_auth, &request).await;

		// Assert
		let user = result.unwrap().unwrap();
		assert_eq!(user.id(), user_id);
		assert_eq!(user.username(), username);
		assert!(user.is_authenticated());
		assert!(user.is_active());
	}

	#[rstest]
	#[tokio::test]
	async fn test_authenticate_with_non_uuid_sub_claim_returns_invalid_token() {
		// Arrange
		let jwt_auth = JwtAuth::new(b"test-secret-key-256bit!");
		// Encode a token with a non-UUID sub claim
		let claims = Claims::new(
			"not-a-valid-uuid".to_string(),
			"bob".to_string(),
			Duration::hours(1),
			false,
			false,
		);
		let token = jwt_auth.encode(&claims).unwrap();
		let request = create_request_with_bearer(&token);

		// Act
		let result = RestAuthentication::authenticate(&jwt_auth, &request).await;

		// Assert
		assert!(
			matches!(&result, Err(AuthenticationError::InvalidToken)),
			"expected InvalidToken error for non-UUID sub claim"
		);
	}

	#[rstest]
	#[tokio::test]
	async fn test_authenticate_with_empty_sub_claim_returns_invalid_token() {
		// Arrange
		let jwt_auth = JwtAuth::new(b"test-secret-key-256bit!");
		let claims = Claims::new(
			String::new(),
			"charlie".to_string(),
			Duration::hours(1),
			false,
			false,
		);
		let token = jwt_auth.encode(&claims).unwrap();
		let request = create_request_with_bearer(&token);

		// Act
		let result = RestAuthentication::authenticate(&jwt_auth, &request).await;

		// Assert
		assert!(
			matches!(&result, Err(AuthenticationError::InvalidToken)),
			"expected InvalidToken error for empty sub claim"
		);
	}

	#[rstest]
	#[tokio::test]
	async fn test_authenticate_with_tampered_token_returns_invalid_token() {
		// Arrange
		let jwt_auth = JwtAuth::new(b"test-secret-key-256bit!");
		let token = jwt_auth
			.generate_token(
				"550e8400-e29b-41d4-a716-446655440000".to_string(),
				"dave".to_string(),
				false,
				false,
			)
			.unwrap();
		// Tamper with the token by modifying characters in the signature
		let tampered_token = format!("{}tampered", token);
		let request = create_request_with_bearer(&tampered_token);

		// Act
		let result = RestAuthentication::authenticate(&jwt_auth, &request).await;

		// Assert
		assert!(matches!(&result, Err(AuthenticationError::InvalidToken)));
	}

	#[rstest]
	#[tokio::test]
	async fn test_authenticate_without_authorization_header_returns_none() {
		// Arrange
		let jwt_auth = JwtAuth::new(b"test-secret-key-256bit!");
		let request = Request::builder()
			.method(Method::GET)
			.uri("/api/resource")
			.body(Bytes::new())
			.build()
			.unwrap();

		// Act
		let result = RestAuthentication::authenticate(&jwt_auth, &request).await;

		// Assert
		assert!(result.unwrap().is_none());
	}

	#[rstest]
	#[tokio::test]
	async fn test_authenticate_with_non_bearer_prefix_returns_none() {
		// Arrange
		let jwt_auth = JwtAuth::new(b"test-secret-key-256bit!");
		let mut headers = HeaderMap::new();
		headers.insert("Authorization", "Token some-token-value".parse().unwrap());
		let request = Request::builder()
			.method(Method::GET)
			.uri("/api/resource")
			.headers(headers)
			.body(Bytes::new())
			.build()
			.unwrap();

		// Act
		let result = RestAuthentication::authenticate(&jwt_auth, &request).await;

		// Assert
		assert!(result.unwrap().is_none());
	}

	#[rstest]
	#[tokio::test]
	async fn test_authenticate_with_wrong_secret_returns_invalid_token() {
		// Arrange
		let jwt_auth_encode = JwtAuth::new(b"encoding-secret-key!!!");
		let jwt_auth_decode = JwtAuth::new(b"different-secret-key!!");
		let token = jwt_auth_encode
			.generate_token(
				"550e8400-e29b-41d4-a716-446655440000".to_string(),
				"eve".to_string(),
				false,
				false,
			)
			.unwrap();
		let request = create_request_with_bearer(&token);

		// Act
		let result = RestAuthentication::authenticate(&jwt_auth_decode, &request).await;

		// Assert
		assert!(matches!(&result, Err(AuthenticationError::InvalidToken)));
	}

	#[rstest]
	#[tokio::test]
	async fn test_authenticate_does_not_fabricate_privilege_flags() {
		// Arrange
		let jwt_auth = JwtAuth::new(b"test-secret-key-256bit!");
		let user_id = "550e8400-e29b-41d4-a716-446655440000";
		let username = "alice";
		let token = jwt_auth
			.generate_token(user_id.to_string(), username.to_string(), false, false)
			.unwrap();
		let request = create_request_with_bearer(&token);

		// Act
		let result = RestAuthentication::authenticate(&jwt_auth, &request).await;

		// Assert - JWT claims contain only sub and username; all other fields
		// must use security defaults (not fabricated values)
		let user = result.unwrap().unwrap();
		assert_eq!(user.id(), user_id);
		assert_eq!(user.username(), username);
		assert!(user.is_active());
		assert!(!user.is_admin(), "admin flag should default to false");
		assert!(!user.is_staff(), "staff flag should default to false");
		assert!(
			!user.is_superuser(),
			"superuser flag should default to false"
		);
		// Email emptiness is verified in test_claims_struct_has_no_email_field.
		// The User trait does not expose email, so direct assertion is not
		// possible through the trait object returned by authenticate().
	}

	/// Verifies that JWT authentication does not fabricate email data.
	/// JWT claims carry only `sub` (user ID) and `username`; the authenticated
	/// user must not have email information injected from outside the token.
	#[rstest]
	#[tokio::test]
	async fn test_jwt_authenticated_user_has_no_email_in_claims() {
		// Arrange
		let jwt_auth = JwtAuth::new(b"test-secret-key-256bit!");
		let token = jwt_auth
			.generate_token(
				"550e8400-e29b-41d4-a716-446655440000".to_string(),
				"alice".to_string(),
				false,
				false,
			)
			.unwrap();
		let request = create_request_with_bearer(&token);

		// Act
		let result = RestAuthentication::authenticate(&jwt_auth, &request).await;
		let user = result.unwrap().unwrap();

		// Assert - JWT claims schema has no email field, so the authenticated
		// user cannot carry email data from the token
		let claims = Claims::new(
			"550e8400-e29b-41d4-a716-446655440000".to_string(),
			"alice".to_string(),
			Duration::hours(1),
			false,
			false,
		);
		let serialized = serde_json::to_value(&claims).unwrap();
		assert!(
			serialized.get("email").is_none(),
			"JWT Claims must not contain an email field"
		);
		// Verify the user was actually authenticated successfully
		assert_eq!(user.username(), "alice");
		assert_eq!(user.id(), "550e8400-e29b-41d4-a716-446655440000");
	}

	// === JwtError variant tests ===

	#[rstest]
	fn test_verify_expired_token_returns_token_expired_error() {
		// Arrange
		let jwt_auth = JwtAuth::new(b"test-secret-key-256bit!");
		let claims = Claims {
			sub: "user123".to_string(),
			exp: Utc::now().timestamp() - 3600, // expired 1 hour ago
			iat: Utc::now().timestamp() - 7200,
			username: "alice".to_string(),
			is_staff: false,
			is_superuser: false,
		};
		// encode() does not validate expiration, so this succeeds
		let token = jwt_auth.encode(&claims).unwrap();

		// Act
		let result = jwt_auth.verify_token(&token);

		// Assert
		assert_eq!(result.unwrap_err(), JwtError::TokenExpired);
	}

	#[rstest]
	fn test_verify_tampered_token_returns_invalid_token() {
		// Arrange
		let jwt_auth = JwtAuth::new(b"test-secret-key-256bit!");
		let token = jwt_auth
			.generate_token("user123".to_string(), "alice".to_string(), false, false)
			.unwrap();
		let tampered = format!("{}tampered", token);

		// Act
		let result = jwt_auth.verify_token(&tampered);

		// Assert
		let err = result.unwrap_err();
		assert!(
			matches!(err, JwtError::InvalidToken(_)),
			"expected InvalidToken, got: {:?}",
			err
		);
	}

	#[rstest]
	fn test_verify_malformed_token_returns_invalid_token() {
		// Arrange
		let jwt_auth = JwtAuth::new(b"test-secret-key-256bit!");

		// Act
		let result = jwt_auth.verify_token("not-a-jwt");

		// Assert
		let err = result.unwrap_err();
		assert!(
			matches!(err, JwtError::InvalidToken(_)),
			"expected InvalidToken, got: {:?}",
			err
		);
	}

	// === verify_token_allow_expired tests ===

	#[rstest]
	fn test_verify_allow_expired_returns_claims_for_expired_token() {
		// Arrange
		let jwt_auth = JwtAuth::new(b"test-secret-key-256bit!");
		let claims = Claims {
			sub: "user123".to_string(),
			exp: Utc::now().timestamp() - 3600, // expired 1 hour ago
			iat: Utc::now().timestamp() - 7200,
			username: "alice".to_string(),
			is_staff: false,
			is_superuser: false,
		};
		let token = jwt_auth.encode(&claims).unwrap();

		// Act
		let result = jwt_auth.verify_token_allow_expired(&token);

		// Assert
		let decoded = result.unwrap();
		assert_eq!(decoded.sub, "user123");
		assert_eq!(decoded.username, "alice");
		assert!(decoded.is_expired());
	}

	#[rstest]
	fn test_verify_allow_expired_rejects_tampered_token() {
		// Arrange
		let jwt_auth = JwtAuth::new(b"test-secret-key-256bit!");
		let token = jwt_auth
			.generate_token("user123".to_string(), "alice".to_string(), false, false)
			.unwrap();
		let tampered = format!("{}tampered", token);

		// Act
		let result = jwt_auth.verify_token_allow_expired(&tampered);

		// Assert
		assert!(result.is_err());
	}

	#[rstest]
	fn test_verify_allow_expired_rejects_wrong_secret() {
		// Arrange
		let jwt_auth_encode = JwtAuth::new(b"encoding-secret-key!!!");
		let jwt_auth_decode = JwtAuth::new(b"different-secret-key!!");
		let token = jwt_auth_encode
			.generate_token("user123".to_string(), "alice".to_string(), false, false)
			.unwrap();

		// Act
		let result = jwt_auth_decode.verify_token_allow_expired(&token);

		// Assert
		assert!(result.is_err());
	}

	#[rstest]
	fn test_verify_allow_expired_works_for_valid_token() {
		// Arrange
		let jwt_auth = JwtAuth::new(b"test-secret-key-256bit!");
		let token = jwt_auth
			.generate_token("user123".to_string(), "alice".to_string(), false, false)
			.unwrap();

		// Act
		let result = jwt_auth.verify_token_allow_expired(&token);

		// Assert
		let claims = result.unwrap();
		assert_eq!(claims.sub, "user123");
		assert!(!claims.is_expired());
	}

	// === AuthenticationError mapping tests ===

	#[rstest]
	#[tokio::test]
	async fn test_authenticate_expired_token_returns_token_expired() {
		// Arrange
		let jwt_auth = JwtAuth::new(b"test-secret-key-256bit!");
		let claims = Claims {
			sub: "550e8400-e29b-41d4-a716-446655440000".to_string(),
			exp: Utc::now().timestamp() - 3600,
			iat: Utc::now().timestamp() - 7200,
			username: "alice".to_string(),
			is_staff: false,
			is_superuser: false,
		};
		let token = jwt_auth.encode(&claims).unwrap();
		let request = create_request_with_bearer(&token);

		// Act
		let result = RestAuthentication::authenticate(&jwt_auth, &request).await;

		// Assert
		assert!(
			matches!(&result, Err(AuthenticationError::TokenExpired)),
			"expected TokenExpired"
		);
	}

	#[rstest]
	fn test_jwt_error_to_auth_error_mapping() {
		// Arrange & Act & Assert
		assert_eq!(
			AuthenticationError::from(JwtError::TokenExpired),
			AuthenticationError::TokenExpired
		);
		assert_eq!(
			AuthenticationError::from(JwtError::InvalidSignature("bad sig".to_string())),
			AuthenticationError::InvalidToken
		);
		assert_eq!(
			AuthenticationError::from(JwtError::InvalidToken("bad token".to_string())),
			AuthenticationError::InvalidToken
		);
		assert!(matches!(
			AuthenticationError::from(JwtError::EncodingError("enc err".to_string())),
			AuthenticationError::Unknown(_)
		));
	}

	// === Backward compatibility tests ===

	#[rstest]
	fn test_serde_default_backward_compatibility_for_staff_fields() {
		// Arrange
		// Simulate a token created before is_staff/is_superuser fields existed
		let jwt_auth = JwtAuth::new(b"test-secret-key-256bit!");
		let legacy_payload = serde_json::json!({
			"sub": "user123",
			"exp": chrono::Utc::now().timestamp() + 3600,
			"iat": chrono::Utc::now().timestamp(),
			"username": "alice"
		});
		// Manually encode a token without staff fields
		let header = jsonwebtoken::Header::default();
		let token = jsonwebtoken::encode(
			&header,
			&legacy_payload,
			&jsonwebtoken::EncodingKey::from_secret(b"test-secret-key-256bit!"),
		)
		.unwrap();

		// Act
		let claims = jwt_auth.decode(&token).unwrap();

		// Assert - missing fields should default to false via #[serde(default)]
		assert_eq!(claims.sub, "user123");
		assert_eq!(claims.username, "alice");
		assert!(
			!claims.is_staff,
			"is_staff should default to false for legacy tokens"
		);
		assert!(
			!claims.is_superuser,
			"is_superuser should default to false for legacy tokens"
		);
	}
}
