//! Remote User Authentication
//!
//! Authentication backend that trusts HTTP headers set by upstream
//! authentication systems (e.g., Apache mod_auth, nginx auth_request).

use crate::{AuthenticationBackend, AuthenticationError, SimpleUser, User};
use reinhardt_http::Request;
use uuid::Uuid;

/// Remote user authentication backend
///
/// Authenticates users based on a trusted HTTP header (typically REMOTE_USER)
/// set by an upstream authentication layer.
///
/// # Security Warning
///
/// This backend trusts the specified header completely. Only use this when
/// your application is behind a properly configured authentication proxy
/// that prevents clients from spoofing this header.
///
/// # Examples
///
/// ```no_run
/// use reinhardt_auth::{AuthenticationBackend, SimpleUser};
/// use bytes::Bytes;
/// use hyper::{HeaderMap, Method};
/// use reinhardt_http::Request;
///
/// # async fn example() {
/// // Create auth backend
/// let auth = reinhardt_auth::RemoteUserAuth::new();
///
/// // Create request with REMOTE_USER header
/// let mut headers = HeaderMap::new();
/// headers.insert("REMOTE_USER", "alice".parse().unwrap());
/// let request = Request::builder()
///     .method(Method::GET)
///     .uri("/")
///     .headers(headers)
///     .body(Bytes::new())
///     .build()
///     .unwrap();
///
/// let result = auth.authenticate(&request).await.unwrap();
/// assert!(result.is_some());
/// assert_eq!(result.unwrap().get_username(), "alice");
/// # }
/// ```
pub struct RemoteUserAuthentication {
	/// Header name to check (default: "REMOTE_USER")
	header_name: String,
	/// Whether to force logout if header is missing
	force_logout: bool,
}

impl RemoteUserAuthentication {
	/// Create a new remote user authentication backend
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_auth::RemoteUserAuth;
	///
	/// let auth = RemoteUserAuth::new();
	/// ```
	pub fn new() -> Self {
		Self {
			header_name: "REMOTE_USER".to_string(),
			force_logout: true,
		}
	}

	/// Set custom header name
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_auth::RemoteUserAuth;
	///
	/// let auth = RemoteUserAuth::new()
	///     .with_header("X-Auth-User");
	/// ```
	pub fn with_header(mut self, header: impl Into<String>) -> Self {
		self.header_name = header.into();
		self
	}

	/// Set whether to force logout when header is missing
	pub fn force_logout(mut self, force: bool) -> Self {
		self.force_logout = force;
		self
	}
}

impl Default for RemoteUserAuthentication {
	fn default() -> Self {
		Self::new()
	}
}

#[async_trait::async_trait]
impl AuthenticationBackend for RemoteUserAuthentication {
	async fn authenticate(
		&self,
		request: &Request,
	) -> Result<Option<Box<dyn User>>, AuthenticationError> {
		// Get header value
		let header_value = request
			.headers
			.get(&self.header_name)
			.and_then(|v| v.to_str().ok());

		match header_value {
			Some(username) if !username.is_empty() => {
				// Create user from header
				Ok(Some(Box::new(SimpleUser {
					id: Uuid::new_v5(&crate::USER_ID_NAMESPACE, username.as_bytes()),
					username: username.to_string(),
					email: String::new(),
					is_active: true,
					is_admin: false,
					is_staff: false,
					is_superuser: false,
				})))
			}
			_ => {
				// No header or empty header
				Ok(None)
			}
		}
	}

	async fn get_user(&self, _user_id: &str) -> Result<Option<Box<dyn User>>, AuthenticationError> {
		// For remote user auth, we can't retrieve users by ID
		// since we only have the username from the header
		Ok(None)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use bytes::Bytes;
	use hyper::{HeaderMap, Method};
	use rstest::rstest;

	#[tokio::test]
	async fn test_remote_user_with_header() {
		let auth = RemoteUserAuthentication::new();
		let mut headers = HeaderMap::new();
		headers.insert("REMOTE_USER", "testuser".parse().unwrap());

		let request = Request::builder()
			.method(Method::GET)
			.uri("/")
			.headers(headers)
			.body(Bytes::new())
			.build()
			.unwrap();

		let result = auth.authenticate(&request).await.unwrap();
		assert!(result.is_some());
		assert_eq!(result.unwrap().get_username(), "testuser");
	}

	#[tokio::test]
	async fn test_remote_user_without_header() {
		let auth = RemoteUserAuthentication::new();
		let request = Request::builder()
			.method(Method::GET)
			.uri("/")
			.body(Bytes::new())
			.build()
			.unwrap();

		let result = auth.authenticate(&request).await.unwrap();
		assert!(result.is_none());
	}

	#[tokio::test]
	async fn test_custom_header() {
		let auth = RemoteUserAuthentication::new().with_header("X-Auth-User");
		let mut headers = HeaderMap::new();
		headers.insert("X-Auth-User", "alice".parse().unwrap());

		let request = Request::builder()
			.method(Method::GET)
			.uri("/")
			.headers(headers)
			.body(Bytes::new())
			.build()
			.unwrap();

		let result = auth.authenticate(&request).await.unwrap();
		assert!(result.is_some());
		assert_eq!(result.unwrap().get_username(), "alice");
	}

	#[rstest]
	#[tokio::test]
	async fn test_same_username_produces_same_id() {
		// Arrange
		let auth = RemoteUserAuthentication::new();

		let mut headers1 = HeaderMap::new();
		headers1.insert("REMOTE_USER", "testuser".parse().unwrap());
		let request1 = Request::builder()
			.method(Method::GET)
			.uri("/")
			.headers(headers1)
			.body(Bytes::new())
			.build()
			.unwrap();

		let mut headers2 = HeaderMap::new();
		headers2.insert("REMOTE_USER", "testuser".parse().unwrap());
		let request2 = Request::builder()
			.method(Method::GET)
			.uri("/")
			.headers(headers2)
			.body(Bytes::new())
			.build()
			.unwrap();

		// Act
		let user1 = auth.authenticate(&request1).await.unwrap().unwrap();
		let user2 = auth.authenticate(&request2).await.unwrap().unwrap();

		// Assert
		assert_eq!(
			user1.id(),
			user2.id(),
			"same username must produce the same UUID"
		);
	}

	#[rstest]
	#[tokio::test]
	async fn test_authenticated_user_has_default_privilege_flags() {
		// Arrange
		let auth = RemoteUserAuthentication::new();
		let mut headers = HeaderMap::new();
		headers.insert("REMOTE_USER", "testuser".parse().unwrap());
		let request = Request::builder()
			.method(Method::GET)
			.uri("/")
			.headers(headers)
			.body(Bytes::new())
			.build()
			.unwrap();

		// Act
		let user = auth.authenticate(&request).await.unwrap().unwrap();

		// Assert
		assert!(user.is_active());
		assert!(!user.is_admin());
	}

	#[tokio::test]
	async fn test_empty_header() {
		let auth = RemoteUserAuthentication::new();
		let mut headers = HeaderMap::new();
		headers.insert("REMOTE_USER", "".parse().unwrap());

		let request = Request::builder()
			.method(Method::GET)
			.uri("/")
			.headers(headers)
			.body(Bytes::new())
			.build()
			.unwrap();

		let result = auth.authenticate(&request).await.unwrap();
		assert!(result.is_none());
	}
}
