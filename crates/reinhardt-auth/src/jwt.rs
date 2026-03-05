use crate::rest_authentication::RestAuthentication;
use crate::{AuthenticationBackend, AuthenticationError, SimpleUser, User};
use chrono::{Duration, Utc};
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation, decode, encode};
use reinhardt_http::Request;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// JWT Claims
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
	pub sub: String, // Subject (user ID)
	pub exp: i64,    // Expiration time
	pub iat: i64,    // Issued at
	pub username: String,
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
	///     Duration::hours(24)
	/// );
	///
	/// assert_eq!(claims.sub, "user123");
	/// assert_eq!(claims.username, "john_doe");
	/// assert!(claims.exp > claims.iat);
	/// ```
	pub fn new(user_id: String, username: String, expires_in: Duration) -> Self {
		let now = Utc::now();
		Self {
			sub: user_id,
			username,
			iat: now.timestamp(),
			exp: (now + expires_in).timestamp(),
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
	///     Duration::hours(24)
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
		Self {
			encoding_key: EncodingKey::from_secret(secret),
			decoding_key: DecodingKey::from_secret(secret),
			validation: Validation::default(),
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
	///     Duration::hours(1)
	/// );
	///
	/// let token = jwt_auth.encode(&claims).unwrap();
	/// assert!(!token.is_empty());
	/// ```
	pub fn encode(&self, claims: &Claims) -> reinhardt_core::exception::Result<String> {
		encode(&Header::default(), claims, &self.encoding_key)
			.map_err(|e| reinhardt_core::exception::Error::Authentication(e.to_string()))
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
	///     Duration::hours(1)
	/// );
	///
	/// let token = jwt_auth.encode(&claims).unwrap();
	/// let decoded = jwt_auth.decode(&token).unwrap();
	/// assert_eq!(decoded.sub, "user123");
	/// ```
	pub fn decode(&self, token: &str) -> reinhardt_core::exception::Result<Claims> {
		decode::<Claims>(token, &self.decoding_key, &self.validation)
			.map(|data| data.claims)
			.map_err(|e| reinhardt_core::exception::Error::Authentication(e.to_string()))
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
	///     "john_doe".to_string()
	/// ).unwrap();
	///
	/// assert!(!token.is_empty());
	/// assert!(token.contains('.'));
	/// ```
	pub fn generate_token(
		&self,
		user_id: String,
		username: String,
	) -> reinhardt_core::exception::Result<String> {
		let claims = Claims::new(user_id, username, Duration::hours(24));
		self.encode(&claims)
	}
	/// Verifies a JWT token and returns the claims if valid and not expired.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_auth::jwt::JwtAuth;
	///
	/// let jwt_auth = JwtAuth::new(b"secret");
	/// let token = jwt_auth.generate_token(
	///     "user123".to_string(),
	///     "john_doe".to_string()
	/// ).unwrap();
	///
	/// let claims = jwt_auth.verify_token(&token).unwrap();
	/// assert_eq!(claims.sub, "user123");
	/// assert_eq!(claims.username, "john_doe");
	/// ```
	pub fn verify_token(&self, token: &str) -> reinhardt_core::exception::Result<Claims> {
		let claims = self.decode(token)?;

		if claims.is_expired() {
			return Err(reinhardt_core::exception::Error::Authentication(
				"Token expired".to_string(),
			));
		}

		Ok(claims)
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
							email: format!("{}@example.com", claims.username),
							is_active: true,
							is_admin: false,
							is_staff: false,
							is_superuser: false,
						})));
					}
					Err(_) => {
						return Err(AuthenticationError::InvalidToken);
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
			.generate_token(user_id.to_string(), username.to_string())
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
		let claims = Claims::new(String::new(), "charlie".to_string(), Duration::hours(1));
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
			)
			.unwrap();
		let request = create_request_with_bearer(&token);

		// Act
		let result = RestAuthentication::authenticate(&jwt_auth_decode, &request).await;

		// Assert
		assert!(matches!(&result, Err(AuthenticationError::InvalidToken)));
	}
}
