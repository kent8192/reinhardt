//! HTTP Basic Authentication
//!
//! Passwords are hashed with Argon2id on storage and verified using
//! constant-time comparison provided by the `argon2` crate.

use crate::core::hasher::PasswordHasher;
use crate::rest_authentication::RestAuthentication;
use crate::{AuthenticationBackend, AuthenticationError, SimpleUser, User};
use base64::{Engine, engine::general_purpose::STANDARD};
use reinhardt_http::Request;
use std::collections::HashMap;
use uuid::Uuid;

/// Argon2-based password hasher used internally by `BasicAuthentication`.
///
/// This is intentionally a thin wrapper so the module stays self-contained
/// without requiring the `argon2-hasher` feature flag.
struct InternalArgon2Hasher;

impl PasswordHasher for InternalArgon2Hasher {
	fn hash(&self, password: &str) -> Result<String, reinhardt_core::exception::Error> {
		use argon2::Argon2;
		use password_hash::{PasswordHasher as _, SaltString, rand_core::OsRng};

		let salt = SaltString::generate(&mut OsRng);
		let argon2 = Argon2::default();

		argon2
			.hash_password(password.as_bytes(), &salt)
			.map(|hash| hash.to_string())
			.map_err(|e| reinhardt_core::exception::Error::Authentication(e.to_string()))
	}

	fn verify(&self, password: &str, hash: &str) -> Result<bool, reinhardt_core::exception::Error> {
		use argon2::Argon2;
		use password_hash::{PasswordHash, PasswordVerifier};

		let parsed_hash = PasswordHash::new(hash)
			.map_err(|e| reinhardt_core::exception::Error::Authentication(e.to_string()))?;

		// Argon2 verify_password uses constant-time comparison internally
		Ok(Argon2::default()
			.verify_password(password.as_bytes(), &parsed_hash)
			.is_ok())
	}
}

/// Basic Authentication backend
///
/// Passwords are hashed with Argon2id before storage.
/// Verification uses the constant-time comparison built into Argon2.
pub struct BasicAuthentication {
	/// username -> argon2 password hash
	users: HashMap<String, String>,
	hasher: InternalArgon2Hasher,
}

impl BasicAuthentication {
	/// Creates a new BasicAuthentication backend with no users.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_auth::{HttpBasicAuth, AuthenticationBackend};
	/// use bytes::Bytes;
	/// use hyper::{HeaderMap, Method, Uri, Version};
	/// use reinhardt_http::Request;
	///
	/// # async fn example() {
	/// let auth = HttpBasicAuth::new();
	///
	/// // Create a request without authentication header
	/// let request = Request::builder()
	///     .method(Method::GET)
	///     .uri("/")
	///     .body(Bytes::new())
	///     .build()
	///     .unwrap();
	///
	/// // Since no users are registered, authentication should return None
	/// let result = auth.authenticate(&request).await.unwrap();
	/// assert!(result.is_none());
	/// # }
	/// # tokio::runtime::Runtime::new().unwrap().block_on(example());
	/// ```
	pub fn new() -> Self {
		Self {
			users: HashMap::new(),
			hasher: InternalArgon2Hasher,
		}
	}

	/// Adds a user with the given username and password.
	///
	/// The password is hashed with Argon2id before storage.
	///
	/// # Panics
	///
	/// Panics if password hashing fails (should not happen in practice).
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_auth::{HttpBasicAuth, AuthenticationBackend};
	/// use bytes::Bytes;
	/// use hyper::{HeaderMap, Method, Uri, Version};
	/// use reinhardt_http::Request;
	///
	/// # async fn example() {
	/// let mut auth = HttpBasicAuth::new();
	/// auth.add_user("alice", "secret123");
	/// auth.add_user("bob", "password456");
	///
	/// // Create a request with valid Basic auth credentials
	/// // "alice:secret123" in base64 is "YWxpY2U6c2VjcmV0MTIz"
	/// let mut headers = HeaderMap::new();
	/// headers.insert("Authorization", "Basic YWxpY2U6c2VjcmV0MTIz".parse().unwrap());
	/// let request = Request::builder()
	///     .method(Method::GET)
	///     .uri("/")
	///     .headers(headers)
	///     .body(Bytes::new())
	///     .build()
	///     .unwrap();
	///
	/// // Authentication should succeed
	/// let result = auth.authenticate(&request).await.unwrap();
	/// assert!(result.is_some());
	/// assert_eq!(result.unwrap().get_username(), "alice");
	/// # }
	/// # tokio::runtime::Runtime::new().unwrap().block_on(example());
	/// ```
	pub fn add_user(&mut self, username: impl Into<String>, password: impl Into<String>) {
		let hash = self
			.hasher
			.hash(&password.into())
			.expect("Argon2 hashing should not fail");
		self.users.insert(username.into(), hash);
	}

	/// Parse Authorization header
	fn parse_auth_header(&self, header: &str) -> Option<(String, String)> {
		if !header.starts_with("Basic ") {
			return None;
		}

		let encoded = header.strip_prefix("Basic ")?;
		let decoded = STANDARD.decode(encoded).ok()?;
		let decoded_str = String::from_utf8(decoded).ok()?;

		let parts: Vec<&str> = decoded_str.splitn(2, ':').collect();
		if parts.len() != 2 {
			return None;
		}

		Some((parts[0].to_string(), parts[1].to_string()))
	}
}

impl Default for BasicAuthentication {
	fn default() -> Self {
		Self::new()
	}
}

#[async_trait::async_trait]
impl AuthenticationBackend for BasicAuthentication {
	async fn authenticate(
		&self,
		request: &Request,
	) -> Result<Option<Box<dyn User>>, AuthenticationError> {
		let auth_header = request
			.headers
			.get("Authorization")
			.and_then(|h| h.to_str().ok());

		if let Some(header) = auth_header
			&& let Some((username, password)) = self.parse_auth_header(header)
		{
			if let Some(stored_hash) = self.users.get(&username) {
				// Argon2 verify uses constant-time comparison internally
				let is_valid = self.hasher.verify(&password, stored_hash).unwrap_or(false);
				if is_valid {
					return Ok(Some(Box::new(SimpleUser {
						id: Uuid::new_v4(),
						username: username.clone(),
						email: format!("{}@example.com", username),
						is_active: true,
						is_admin: false,
						is_staff: false,
						is_superuser: false,
					})));
				}
			}
			return Err(AuthenticationError::InvalidCredentials);
		}

		Ok(None)
	}

	async fn get_user(&self, user_id: &str) -> Result<Option<Box<dyn User>>, AuthenticationError> {
		if self.users.contains_key(user_id) {
			Ok(Some(Box::new(SimpleUser {
				id: Uuid::new_v4(),
				username: user_id.to_string(),
				email: format!("{}@example.com", user_id),
				is_active: true,
				is_admin: false,
				is_staff: false,
				is_superuser: false,
			})))
		} else {
			Ok(None)
		}
	}
}

// Implement REST API Authentication trait by forwarding to AuthenticationBackend
#[async_trait::async_trait]
impl RestAuthentication for BasicAuthentication {
	async fn authenticate(
		&self,
		request: &Request,
	) -> Result<Option<Box<dyn User>>, AuthenticationError> {
		// Forward to AuthenticationBackend implementation
		AuthenticationBackend::authenticate(self, request).await
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use bytes::Bytes;
	use hyper::{HeaderMap, Method};
	use rstest::rstest;

	fn create_request_with_auth(auth: &str) -> Request {
		let mut headers = HeaderMap::new();
		headers.insert("Authorization", auth.parse().unwrap());
		Request::builder()
			.method(Method::GET)
			.uri("/")
			.headers(headers)
			.body(Bytes::new())
			.build()
			.unwrap()
	}

	#[rstest]
	#[tokio::test]
	async fn test_basic_auth_success() {
		// Arrange
		let mut backend = BasicAuthentication::new();
		backend.add_user("testuser", "testpass");

		// Base64 encode "testuser:testpass"
		let auth = "Basic dGVzdHVzZXI6dGVzdHBhc3M=";
		let request = create_request_with_auth(auth);

		// Act
		let result = AuthenticationBackend::authenticate(&backend, &request)
			.await
			.unwrap();

		// Assert
		assert!(result.is_some());
		assert_eq!(result.unwrap().get_username(), "testuser");
	}

	#[rstest]
	#[tokio::test]
	async fn test_basic_auth_invalid_password() {
		// Arrange
		let mut backend = BasicAuthentication::new();
		backend.add_user("testuser", "correctpass");

		// Base64 encode "testuser:wrongpass"
		let auth = "Basic dGVzdHVzZXI6d3JvbmdwYXNz";
		let request = create_request_with_auth(auth);

		// Act
		let result = AuthenticationBackend::authenticate(&backend, &request).await;

		// Assert
		assert!(result.is_err());
	}

	#[rstest]
	#[tokio::test]
	async fn test_basic_auth_no_header() {
		// Arrange
		let backend = BasicAuthentication::new();
		let request = Request::builder()
			.method(Method::GET)
			.uri("/")
			.body(Bytes::new())
			.build()
			.unwrap();

		// Act
		let result = AuthenticationBackend::authenticate(&backend, &request)
			.await
			.unwrap();

		// Assert
		assert!(result.is_none());
	}

	#[rstest]
	fn test_parse_auth_header() {
		// Arrange
		let backend = BasicAuthentication::new();

		// Act
		let (user, pass) = backend.parse_auth_header("Basic dGVzdDpwYXNz").unwrap();

		// Assert
		assert_eq!(user, "test");
		assert_eq!(pass, "pass");
	}

	#[rstest]
	#[tokio::test]
	async fn test_get_user() {
		// Arrange
		let mut backend = BasicAuthentication::new();
		backend.add_user("testuser", "testpass");

		// Act
		let user = backend.get_user("testuser").await.unwrap();
		let no_user = backend.get_user("nonexistent").await.unwrap();

		// Assert
		assert!(user.is_some());
		assert_eq!(user.unwrap().get_username(), "testuser");
		assert!(no_user.is_none());
	}

	#[rstest]
	fn test_password_is_hashed_on_storage() {
		// Arrange
		let mut backend = BasicAuthentication::new();

		// Act
		backend.add_user("testuser", "plaintext_password");

		// Assert
		let stored = backend.users.get("testuser").unwrap();
		// Argon2 hashes start with "$argon2"
		assert!(
			stored.starts_with("$argon2"),
			"Password should be stored as Argon2 hash, got: {}",
			stored
		);
		assert_ne!(stored, "plaintext_password");
	}

	#[rstest]
	fn test_argon2_verification_works() {
		// Arrange
		let hasher = InternalArgon2Hasher;
		let password = "test_password_123";

		// Act
		let hash = hasher.hash(password).unwrap();
		let valid = hasher.verify(password, &hash).unwrap();
		let invalid = hasher.verify("wrong_password", &hash).unwrap();

		// Assert
		assert!(valid);
		assert!(!invalid);
	}
}
