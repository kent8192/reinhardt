//! REST API Authentication
//!
//! Provides REST API-compatible authentication wrappers and combinators.

#[cfg(feature = "argon2-hasher")]
use crate::DefaultUser;
use crate::sessions::{Session, backends::SessionBackend};
use crate::{AuthenticationBackend, AuthenticationError, SimpleUser, User};
use reinhardt_http::Request;
use sha2::{Digest, Sha256};
use std::sync::Arc;
use subtle::ConstantTimeEq;

/// REST API authentication trait wrapper
///
/// Provides a REST API-compatible interface for authentication.
#[async_trait::async_trait]
pub trait RestAuthentication: Send + Sync {
	/// Authenticate a request and return a user if successful
	async fn authenticate(
		&self,
		request: &Request,
	) -> Result<Option<Box<dyn User>>, AuthenticationError>;
}

/// Basic authentication configuration
#[derive(Debug, Clone)]
pub struct BasicAuthConfig {
	/// Realm for WWW-Authenticate header
	pub realm: String,
}

impl Default for BasicAuthConfig {
	fn default() -> Self {
		Self {
			realm: "api".to_string(),
		}
	}
}

/// Session authentication configuration
#[derive(Debug, Clone)]
pub struct SessionAuthConfig {
	/// Session cookie name
	pub cookie_name: String,
	/// Whether to enforce CSRF protection
	pub enforce_csrf: bool,
}

impl Default for SessionAuthConfig {
	fn default() -> Self {
		Self {
			cookie_name: "sessionid".to_string(),
			enforce_csrf: true,
		}
	}
}

/// Token authentication configuration
#[derive(Debug, Clone)]
pub struct TokenAuthConfig {
	/// Token header name (default: "Authorization")
	pub header_name: String,
	/// Token prefix (default: "Token")
	pub prefix: String,
}

impl Default for TokenAuthConfig {
	fn default() -> Self {
		Self {
			header_name: "Authorization".to_string(),
			prefix: "Token".to_string(),
		}
	}
}

/// Composite authentication backend
///
/// Tries multiple authentication methods in sequence, similar to Django REST Framework.
///
/// # Examples
///
/// ```
/// use reinhardt_auth::{CompositeAuthentication, SessionAuthentication, TokenAuthentication};
/// use reinhardt_auth::sessions::backends::InMemorySessionBackend;
///
/// let session_backend = InMemorySessionBackend::new();
/// let auth = CompositeAuthentication::new()
///     .with_backend(SessionAuthentication::new(session_backend))
///     .with_backend(TokenAuthentication::new());
/// ```
pub struct CompositeAuthentication {
	backends: Vec<Arc<dyn AuthenticationBackend>>,
}

impl CompositeAuthentication {
	/// Create a new composite authentication backend
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_auth::CompositeAuthentication;
	///
	/// let auth = CompositeAuthentication::new();
	/// ```
	pub fn new() -> Self {
		Self {
			backends: Vec::new(),
		}
	}

	/// Add an authentication backend (chainable)
	///
	/// Backends are tried in the order they are added.
	/// The backend will be wrapped in an Arc internally.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_auth::{CompositeAuthentication, TokenAuthentication};
	///
	/// let auth = CompositeAuthentication::new()
	///     .with_backend(TokenAuthentication::new());
	/// ```
	pub fn with_backend<B: AuthenticationBackend + 'static>(mut self, backend: B) -> Self {
		self.backends.push(Arc::new(backend));
		self
	}

	/// Add multiple backends at once (chainable)
	pub fn with_backends(mut self, backends: Vec<Arc<dyn AuthenticationBackend>>) -> Self {
		self.backends.extend(backends);
		self
	}
}

impl Default for CompositeAuthentication {
	fn default() -> Self {
		Self::new()
	}
}

#[async_trait::async_trait]
impl RestAuthentication for CompositeAuthentication {
	/// Fallback authentication pattern: backends are tried sequentially.
	///
	/// - `Ok(Some(user))`: authentication succeeded, return immediately
	/// - `Ok(None)`: this backend does not handle this authentication type, try next
	/// - `Err(e)`: backend error (e.g., database failure), log and try next
	///
	/// Errors are only propagated when ALL backends return `Err`, meaning no backend
	/// could attempt authentication. If any backend returns `Ok(None)`, it indicates
	/// that at least one backend processed the request normally, so errors from
	/// other backends are considered irrelevant to this request type.
	async fn authenticate(
		&self,
		request: &Request,
	) -> Result<Option<Box<dyn User>>, AuthenticationError> {
		// Try each backend in order, collecting errors
		let mut errors: Vec<AuthenticationError> = Vec::new();

		for backend in &self.backends {
			match backend.authenticate(request).await {
				Ok(Some(user)) => return Ok(Some(user)),
				Ok(None) => continue,
				Err(e) => {
					tracing::warn!("Authentication backend error occurred");
					tracing::debug!(error = %e, "Authentication backend error details");
					errors.push(e);
				}
			}
		}

		// If all backends failed with errors and none returned Ok(None),
		// propagate the first error to inform the caller
		if !errors.is_empty() && self.backends.len() == errors.len() {
			return Err(errors.into_iter().next().expect("errors is non-empty"));
		}

		Ok(None)
	}
}

#[async_trait::async_trait]
impl AuthenticationBackend for CompositeAuthentication {
	async fn authenticate(
		&self,
		request: &Request,
	) -> Result<Option<Box<dyn User>>, AuthenticationError> {
		<Self as RestAuthentication>::authenticate(self, request).await
	}

	async fn get_user(&self, user_id: &str) -> Result<Option<Box<dyn User>>, AuthenticationError> {
		// Try each backend in order until one succeeds, collecting errors
		let mut errors: Vec<AuthenticationError> = Vec::new();

		for backend in &self.backends {
			match backend.get_user(user_id).await {
				Ok(Some(user)) => return Ok(Some(user)),
				Ok(None) => continue,
				Err(e) => {
					tracing::warn!("get_user backend error occurred");
					tracing::debug!(error = %e, "get_user backend error details");
					errors.push(e);
				}
			}
		}

		// If all backends failed with errors and none returned Ok(None),
		// propagate the first error to inform the caller
		if !errors.is_empty() && self.backends.len() == errors.len() {
			return Err(errors.into_iter().next().expect("errors is non-empty"));
		}

		Ok(None)
	}
}

/// Token authentication using custom tokens
pub struct TokenAuthentication {
	/// Token store (token -> user_id) — retained for `get_user` lookups by user_id
	tokens: std::collections::HashMap<String, String>,
	/// SHA-256 digest index for O(1) token lookup
	/// Key: SHA-256(token), Value: (original_token, user_id)
	token_index: std::collections::HashMap<[u8; 32], (String, String)>,
	/// Configuration
	config: TokenAuthConfig,
}

impl TokenAuthentication {
	/// Create a new token authentication backend
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_auth::TokenAuthentication;
	///
	/// let auth = TokenAuthentication::new();
	/// ```
	pub fn new() -> Self {
		Self {
			tokens: std::collections::HashMap::new(),
			token_index: std::collections::HashMap::new(),
			config: TokenAuthConfig::default(),
		}
	}

	/// Create with custom configuration
	pub fn with_config(config: TokenAuthConfig) -> Self {
		Self {
			tokens: std::collections::HashMap::new(),
			token_index: std::collections::HashMap::new(),
			config,
		}
	}

	/// Add a token for a user
	///
	/// Tokens are stored with their SHA-256 digest for O(1) lookup.
	pub fn add_token(&mut self, token: impl Into<String>, user_id: impl Into<String>) {
		let token = token.into();
		let user_id = user_id.into();
		let digest: [u8; 32] = Sha256::digest(token.as_bytes()).into();
		self.token_index
			.insert(digest, (token.clone(), user_id.clone()));
		self.tokens.insert(token, user_id);
	}

	/// Find a token using SHA-256 digest for O(1) lookup with constant-time
	/// verification to prevent timing attacks.
	///
	/// 1. Compute SHA-256(candidate) and look up in the digest index (O(1))
	/// 2. Verify the match with constant-time comparison on the original token
	fn find_token_constant_time(&self, candidate: &str) -> Option<&String> {
		let digest: [u8; 32] = Sha256::digest(candidate.as_bytes()).into();
		if let Some((stored_token, user_id)) = self.token_index.get(&digest) {
			let candidate_bytes = candidate.as_bytes();
			let stored_bytes = stored_token.as_bytes();
			if candidate_bytes.len() == stored_bytes.len()
				&& bool::from(candidate_bytes.ct_eq(stored_bytes))
			{
				Some(user_id)
			} else {
				None
			}
		} else {
			None
		}
	}
}

impl Default for TokenAuthentication {
	fn default() -> Self {
		Self::new()
	}
}

#[async_trait::async_trait]
impl RestAuthentication for TokenAuthentication {
	async fn authenticate(
		&self,
		request: &Request,
	) -> Result<Option<Box<dyn User>>, AuthenticationError> {
		let auth_header = request
			.headers
			.get(&self.config.header_name)
			.and_then(|h| h.to_str().ok());

		if let Some(header) = auth_header {
			let prefix = format!("{} ", self.config.prefix);
			if let Some(token) = header.strip_prefix(&prefix)
				&& let Some(user_id) = self.find_token_constant_time(token)
			{
				// Try to parse user_id as UUID, or generate a new one if it fails
				let id = uuid::Uuid::parse_str(user_id).unwrap_or_else(|_| {
					uuid::Uuid::new_v5(&crate::USER_ID_NAMESPACE, user_id.as_bytes())
				});
				return Ok(Some(Box::new(SimpleUser {
					id,
					username: user_id.clone(),
					email: String::new(),
					is_active: true,
					is_admin: false,
					is_staff: false,
					is_superuser: false,
				})));
			}
		}

		Ok(None)
	}
}

#[async_trait::async_trait]
impl AuthenticationBackend for TokenAuthentication {
	async fn authenticate(
		&self,
		request: &Request,
	) -> Result<Option<Box<dyn User>>, AuthenticationError> {
		<Self as RestAuthentication>::authenticate(self, request).await
	}

	async fn get_user(&self, user_id: &str) -> Result<Option<Box<dyn User>>, AuthenticationError> {
		if self.tokens.values().any(|id| id == user_id) {
			// Try to parse user_id as UUID, or generate a new one if it fails
			let id = uuid::Uuid::parse_str(user_id).unwrap_or_else(|_| {
				uuid::Uuid::new_v5(&crate::USER_ID_NAMESPACE, user_id.as_bytes())
			});
			Ok(Some(Box::new(SimpleUser {
				id,
				username: user_id.to_string(),
				email: String::new(),
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

/// Remote user authentication (from upstream proxy)
pub struct RemoteUserAuthentication {
	/// Header name to check
	header_name: String,
}

impl RemoteUserAuthentication {
	/// Create a new remote user authentication backend
	pub fn new() -> Self {
		Self {
			header_name: "REMOTE_USER".to_string(),
		}
	}

	/// Set custom header name
	pub fn with_header(mut self, header: impl Into<String>) -> Self {
		self.header_name = header.into();
		self
	}
}

impl Default for RemoteUserAuthentication {
	fn default() -> Self {
		Self::new()
	}
}

#[async_trait::async_trait]
impl RestAuthentication for RemoteUserAuthentication {
	async fn authenticate(
		&self,
		request: &Request,
	) -> Result<Option<Box<dyn User>>, AuthenticationError> {
		let header_value = request
			.headers
			.get(&self.header_name)
			.and_then(|v| v.to_str().ok());

		if let Some(username) = header_value
			&& !username.is_empty()
		{
			return Ok(Some(Box::new(SimpleUser {
				id: uuid::Uuid::new_v5(&crate::USER_ID_NAMESPACE, username.as_bytes()),
				username: username.to_string(),
				email: String::new(),
				is_active: true,
				is_admin: false,
				is_staff: false,
				is_superuser: false,
			})));
		}

		Ok(None)
	}
}

#[async_trait::async_trait]
impl AuthenticationBackend for RemoteUserAuthentication {
	async fn authenticate(
		&self,
		request: &Request,
	) -> Result<Option<Box<dyn User>>, AuthenticationError> {
		<Self as RestAuthentication>::authenticate(self, request).await
	}

	async fn get_user(&self, _user_id: &str) -> Result<Option<Box<dyn User>>, AuthenticationError> {
		Ok(None)
	}
}

/// Session-based authentication
#[derive(Clone)]
pub struct SessionAuthentication<B: SessionBackend> {
	/// Configuration
	config: SessionAuthConfig,
	/// Session backend for loading session data
	session_backend: B,
}

impl<B: SessionBackend> SessionAuthentication<B> {
	/// Create a new session authentication backend
	pub fn new(session_backend: B) -> Self {
		Self {
			config: SessionAuthConfig::default(),
			session_backend,
		}
	}

	/// Create with custom configuration
	pub fn with_config(config: SessionAuthConfig, session_backend: B) -> Self {
		Self {
			config,
			session_backend,
		}
	}
}

impl<B: SessionBackend + Default> Default for SessionAuthentication<B> {
	fn default() -> Self {
		Self::new(B::default())
	}
}

#[async_trait::async_trait]
impl<B: SessionBackend> RestAuthentication for SessionAuthentication<B> {
	async fn authenticate(
		&self,
		request: &Request,
	) -> Result<Option<Box<dyn User>>, AuthenticationError> {
		// Check for session cookie
		let cookie_header = request.headers.get("Cookie").and_then(|h| h.to_str().ok());

		if let Some(cookies) = cookie_header {
			for cookie in cookies.split(';') {
				let parts: Vec<&str> = cookie.trim().splitn(2, '=').collect();
				if parts.len() == 2 && parts[0] == self.config.cookie_name {
					let session_key = parts[1];

					// Load session from backend
					let mut session =
						Session::from_key(self.session_backend.clone(), session_key.to_string())
							.await
							.map_err(|_| AuthenticationError::SessionExpired)?;

					// Get user ID from session
					let user_id: String = match session.get("_auth_user_id") {
						Ok(Some(id)) => id,
						Ok(None) => return Ok(None), // No user in session
						Err(_) => return Err(AuthenticationError::SessionExpired),
					};

					// Get additional user fields from session
					let username: String = session
						.get("_auth_user_name")
						.ok()
						.flatten()
						.unwrap_or_else(|| user_id.clone());
					let email: String = session
						.get("_auth_user_email")
						.ok()
						.flatten()
						.unwrap_or_default();
					let is_active: bool = session
						.get("_auth_user_is_active")
						.ok()
						.flatten()
						.unwrap_or(true);
					let is_admin: bool = session
						.get("_auth_user_is_admin")
						.ok()
						.flatten()
						.unwrap_or(false);
					let is_staff: bool = session
						.get("_auth_user_is_staff")
						.ok()
						.flatten()
						.unwrap_or(false);
					let is_superuser: bool = session
						.get("_auth_user_is_superuser")
						.ok()
						.flatten()
						.unwrap_or(false);

					// Create user from session data
					let user = SimpleUser {
						id: uuid::Uuid::parse_str(&user_id)
							.map_err(|_| AuthenticationError::InvalidCredentials)?,
						username,
						email,
						is_active,
						is_admin,
						is_staff,
						is_superuser,
					};

					return Ok(Some(Box::new(user)));
				}
			}
		}

		Ok(None)
	}
}

#[async_trait::async_trait]
impl<B: SessionBackend> AuthenticationBackend for SessionAuthentication<B> {
	async fn authenticate(
		&self,
		request: &Request,
	) -> Result<Option<Box<dyn User>>, AuthenticationError> {
		<Self as RestAuthentication>::authenticate(self, request).await
	}

	#[cfg(feature = "argon2-hasher")]
	async fn get_user(&self, user_id: &str) -> Result<Option<Box<dyn User>>, AuthenticationError> {
		// Parse user_id as UUID
		let id =
			uuid::Uuid::parse_str(user_id).map_err(|_| AuthenticationError::InvalidCredentials)?;

		// Get database connection
		let conn = reinhardt_db::orm::manager::get_connection()
			.await
			.map_err(|e| AuthenticationError::DatabaseError(e.to_string()))?;

		// Build SQL query using reinhardt-query for type-safe query construction
		use reinhardt_db::orm::{
			Alias, DatabaseBackend, Expr, ExprTrait, Model, MySqlQueryBuilder,
			PostgresQueryBuilder, Query, QueryStatementBuilder, SqliteQueryBuilder,
		};

		let table_name = DefaultUser::table_name();

		// Build SELECT query using reinhardt-query
		let stmt = Query::select()
			.columns([
				Alias::new("id"),
				Alias::new("username"),
				Alias::new("email"),
				Alias::new("first_name"),
				Alias::new("last_name"),
				Alias::new("password_hash"),
				Alias::new("last_login"),
				Alias::new("is_active"),
				Alias::new("is_staff"),
				Alias::new("is_superuser"),
				Alias::new("date_joined"),
				Alias::new("user_permissions"),
				Alias::new("groups"),
			])
			.from(Alias::new(table_name))
			.and_where(Expr::col(Alias::new("id")).eq(Expr::value(id.to_string())))
			.to_owned();

		let sql = match conn.backend() {
			DatabaseBackend::Postgres => stmt.to_string(PostgresQueryBuilder),
			DatabaseBackend::MySql => stmt.to_string(MySqlQueryBuilder),
			DatabaseBackend::Sqlite => stmt.to_string(SqliteQueryBuilder),
		};

		// Execute query
		let row = conn
			.query_one(&sql, vec![])
			.await
			.map_err(|e| AuthenticationError::DatabaseError(e.to_string()))?;

		// Deserialize to DefaultUser
		let user: DefaultUser = serde_json::from_value(row.data).map_err(|e| {
			AuthenticationError::DatabaseError(format!("Deserialization failed: {}", e))
		})?;

		// Return as trait object
		Ok(Some(Box::new(user)))
	}

	#[cfg(not(feature = "argon2-hasher"))]
	async fn get_user(&self, _user_id: &str) -> Result<Option<Box<dyn User>>, AuthenticationError> {
		// When argon2-hasher feature is disabled, DefaultUser is not available
		// Return None to indicate user retrieval is not supported
		Ok(None)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::AuthenticationError;
	#[cfg(feature = "jwt")]
	use crate::basic::BasicAuthentication;
	use bytes::Bytes;
	use hyper::{HeaderMap, Method};
	use rstest::rstest;
	use std::sync::Mutex;

	#[tokio::test]
	#[cfg(feature = "jwt")]
	async fn test_composite_authentication() {
		let mut basic = BasicAuthentication::new();
		basic.add_user("user1", "pass1");

		let composite = CompositeAuthentication::new().with_backend(basic);

		// Test with basic auth
		let mut headers = HeaderMap::new();
		headers.insert(
			"Authorization",
			"Basic dXNlcjE6cGFzczE=".parse().unwrap(), // user1:pass1
		);

		let request = Request::builder()
			.method(Method::GET)
			.uri("/")
			.headers(headers)
			.body(Bytes::new())
			.build()
			.unwrap();

		let result = RestAuthentication::authenticate(&composite, &request)
			.await
			.unwrap();
		assert!(result.is_some());
		assert_eq!(result.unwrap().get_username(), "user1");
	}

	#[tokio::test]
	async fn test_token_authentication() {
		let mut auth = TokenAuthentication::new();
		auth.add_token("secret_token", "alice");

		let mut headers = HeaderMap::new();
		headers.insert("Authorization", "Token secret_token".parse().unwrap());

		let request = Request::builder()
			.method(Method::GET)
			.uri("/")
			.headers(headers)
			.body(Bytes::new())
			.build()
			.unwrap();

		let result = RestAuthentication::authenticate(&auth, &request)
			.await
			.unwrap();
		assert!(result.is_some());
		assert_eq!(result.unwrap().get_username(), "alice");
	}

	#[tokio::test]
	async fn test_remote_user_authentication() {
		let auth = RemoteUserAuthentication::new();

		let mut headers = HeaderMap::new();
		headers.insert("REMOTE_USER", "bob".parse().unwrap());

		let request = Request::builder()
			.method(Method::GET)
			.uri("/")
			.headers(headers)
			.body(Bytes::new())
			.build()
			.unwrap();

		let result = RestAuthentication::authenticate(&auth, &request)
			.await
			.unwrap();
		assert!(result.is_some());
		assert_eq!(result.unwrap().get_username(), "bob");
	}

	#[tokio::test]
	async fn test_session_authentication() {
		use crate::sessions::InMemorySessionBackend;
		use crate::sessions::Session;

		let session_backend = InMemorySessionBackend::new();

		// Create a session with user data
		let mut session = Session::new(session_backend.clone());
		session
			.set("_auth_user_id", "550e8400-e29b-41d4-a716-446655440000")
			.unwrap();
		session.set("_auth_user_name", "testuser").unwrap();
		session.set("_auth_user_email", "test@example.com").unwrap();
		session.set("_auth_user_is_active", true).unwrap();
		session.save().await.unwrap();

		// Get the generated session key
		let session_key = session.get_or_create_key().to_string();

		let auth = SessionAuthentication::new(session_backend);

		let mut headers = HeaderMap::new();
		let cookie_value = format!("sessionid={}", session_key);
		headers.insert("Cookie", cookie_value.parse().unwrap());

		let request = Request::builder()
			.method(Method::GET)
			.uri("/")
			.headers(headers)
			.body(Bytes::new())
			.build()
			.unwrap();

		let result = RestAuthentication::authenticate(&auth, &request)
			.await
			.unwrap();
		assert!(result.is_some());

		// Verify the authenticated user
		let user = result.unwrap();
		assert_eq!(user.get_username(), "testuser");
	}

	#[rstest]
	#[tokio::test]
	async fn test_token_auth_same_username_produces_same_id() {
		// Arrange
		let mut auth = TokenAuthentication::new();
		auth.add_token("token1", "alice");
		auth.add_token("token2", "alice");

		let mut headers1 = HeaderMap::new();
		headers1.insert("Authorization", "Token token1".parse().unwrap());
		let request1 = Request::builder()
			.method(Method::GET)
			.uri("/")
			.headers(headers1)
			.body(Bytes::new())
			.build()
			.unwrap();

		let mut headers2 = HeaderMap::new();
		headers2.insert("Authorization", "Token token2".parse().unwrap());
		let request2 = Request::builder()
			.method(Method::GET)
			.uri("/")
			.headers(headers2)
			.body(Bytes::new())
			.build()
			.unwrap();

		// Act
		let user1 = RestAuthentication::authenticate(&auth, &request1)
			.await
			.unwrap()
			.unwrap();
		let user2 = RestAuthentication::authenticate(&auth, &request2)
			.await
			.unwrap()
			.unwrap();

		// Assert
		assert_eq!(
			user1.id(),
			user2.id(),
			"same username must produce the same UUID"
		);
	}

	#[rstest]
	#[tokio::test]
	async fn test_token_auth_user_has_default_privilege_flags() {
		// Arrange
		let mut auth = TokenAuthentication::new();
		auth.add_token("secret_token", "alice");

		let mut headers = HeaderMap::new();
		headers.insert("Authorization", "Token secret_token".parse().unwrap());
		let request = Request::builder()
			.method(Method::GET)
			.uri("/")
			.headers(headers)
			.body(Bytes::new())
			.build()
			.unwrap();

		// Act
		let user = RestAuthentication::authenticate(&auth, &request)
			.await
			.unwrap()
			.unwrap();

		// Assert
		assert!(user.is_active());
		assert!(!user.is_admin());
	}

	#[rstest]
	#[tokio::test]
	async fn test_remote_user_auth_same_username_produces_same_id() {
		// Arrange
		let auth = RemoteUserAuthentication::new();

		let mut headers1 = HeaderMap::new();
		headers1.insert("REMOTE_USER", "bob".parse().unwrap());
		let request1 = Request::builder()
			.method(Method::GET)
			.uri("/")
			.headers(headers1)
			.body(Bytes::new())
			.build()
			.unwrap();

		let mut headers2 = HeaderMap::new();
		headers2.insert("REMOTE_USER", "bob".parse().unwrap());
		let request2 = Request::builder()
			.method(Method::GET)
			.uri("/")
			.headers(headers2)
			.body(Bytes::new())
			.build()
			.unwrap();

		// Act
		let user1 = RestAuthentication::authenticate(&auth, &request1)
			.await
			.unwrap()
			.unwrap();
		let user2 = RestAuthentication::authenticate(&auth, &request2)
			.await
			.unwrap()
			.unwrap();

		// Assert
		assert_eq!(
			user1.id(),
			user2.id(),
			"same username must produce the same UUID"
		);
	}

	#[rstest]
	#[tokio::test]
	async fn test_token_auth_get_user_same_id_produces_same_uuid() {
		// Arrange
		let mut auth = TokenAuthentication::new();
		auth.add_token("secret_token", "alice");

		// Act
		let user1 = auth.get_user("alice").await.unwrap().unwrap();
		let user2 = auth.get_user("alice").await.unwrap().unwrap();

		// Assert
		assert_eq!(
			user1.id(),
			user2.id(),
			"same user_id must produce the same UUID via get_user"
		);
	}

	#[rstest]
	#[tokio::test]
	async fn test_token_auth_unknown_token_returns_none() {
		// Arrange
		let mut auth = TokenAuthentication::new();
		auth.add_token("known_token", "alice");

		let mut headers = HeaderMap::new();
		headers.insert("Authorization", "Token unknown_token".parse().unwrap());
		let request = Request::builder()
			.method(Method::GET)
			.uri("/")
			.headers(headers)
			.body(Bytes::new())
			.build()
			.unwrap();

		// Act
		let result = RestAuthentication::authenticate(&auth, &request)
			.await
			.unwrap();

		// Assert
		assert!(result.is_none());
	}

	#[rstest]
	#[tokio::test]
	async fn test_token_auth_get_user_unknown_returns_none() {
		// Arrange
		let mut auth = TokenAuthentication::new();
		auth.add_token("secret_token", "alice");

		// Act
		let result = auth.get_user("nonexistent_user").await.unwrap();

		// Assert
		assert!(result.is_none());
	}

	#[tokio::test]
	async fn test_custom_token_config() {
		let config = TokenAuthConfig {
			header_name: "X-API-Key".to_string(),
			prefix: "Bearer".to_string(),
		};

		let mut auth = TokenAuthentication::with_config(config);
		auth.add_token("my_token", "charlie");

		let mut headers = HeaderMap::new();
		headers.insert("X-API-Key", "Bearer my_token".parse().unwrap());

		let request = Request::builder()
			.method(Method::GET)
			.uri("/")
			.headers(headers)
			.body(Bytes::new())
			.build()
			.unwrap();

		let result = RestAuthentication::authenticate(&auth, &request)
			.await
			.unwrap();
		assert!(result.is_some());
		assert_eq!(result.unwrap().get_username(), "charlie");
	}

	struct MockAuthBackend {
		auth_result: Mutex<Option<Result<Option<Box<dyn User>>, AuthenticationError>>>,
		get_user_result: Mutex<Option<Result<Option<Box<dyn User>>, AuthenticationError>>>,
	}

	impl MockAuthBackend {
		fn new(
			auth_result: Result<Option<Box<dyn User>>, AuthenticationError>,
			get_user_result: Result<Option<Box<dyn User>>, AuthenticationError>,
		) -> Self {
			Self {
				auth_result: Mutex::new(Some(auth_result)),
				get_user_result: Mutex::new(Some(get_user_result)),
			}
		}
	}

	#[async_trait::async_trait]
	impl AuthenticationBackend for MockAuthBackend {
		async fn authenticate(
			&self,
			_request: &Request,
		) -> Result<Option<Box<dyn User>>, AuthenticationError> {
			self.auth_result.lock().unwrap().take().unwrap_or(Ok(None))
		}

		async fn get_user(
			&self,
			_user_id: &str,
		) -> Result<Option<Box<dyn User>>, AuthenticationError> {
			self.get_user_result
				.lock()
				.unwrap()
				.take()
				.unwrap_or(Ok(None))
		}
	}

	#[rstest]
	#[tokio::test]
	async fn test_composite_auth_all_backends_error() {
		// Arrange
		let composite = CompositeAuthentication::new()
			.with_backend(MockAuthBackend::new(
				Err(AuthenticationError::DatabaseError("db1 down".to_string())),
				Err(AuthenticationError::DatabaseError("db1 down".to_string())),
			))
			.with_backend(MockAuthBackend::new(
				Err(AuthenticationError::DatabaseError("db2 down".to_string())),
				Err(AuthenticationError::DatabaseError("db2 down".to_string())),
			));

		let request = Request::builder()
			.method(Method::GET)
			.uri("/")
			.body(Bytes::new())
			.build()
			.unwrap();

		// Act
		let result = RestAuthentication::authenticate(&composite, &request).await;

		// Assert - all backends errored, so error is propagated
		assert!(result.is_err());
	}

	#[rstest]
	#[tokio::test]
	async fn test_composite_auth_one_error_one_none() {
		// Arrange - one backend errors, another returns Ok(None)
		// This tests the intentional fallback behavior: Ok(None) means
		// "this backend doesn't handle this auth type", so errors from
		// other backends are irrelevant.
		let composite = CompositeAuthentication::new()
			.with_backend(MockAuthBackend::new(
				Err(AuthenticationError::DatabaseError("db down".to_string())),
				Err(AuthenticationError::DatabaseError("db down".to_string())),
			))
			.with_backend(MockAuthBackend::new(Ok(None), Ok(None)));

		let request = Request::builder()
			.method(Method::GET)
			.uri("/")
			.body(Bytes::new())
			.build()
			.unwrap();

		// Act
		let result = RestAuthentication::authenticate(&composite, &request).await;

		// Assert - one backend returned Ok(None), so errors are not propagated
		assert!(result.is_ok());
		assert!(result.unwrap().is_none());
	}

	#[rstest]
	#[tokio::test]
	async fn test_composite_get_user_all_error() {
		// Arrange
		let composite = CompositeAuthentication::new()
			.with_backend(MockAuthBackend::new(
				Ok(None),
				Err(AuthenticationError::DatabaseError("db1 down".to_string())),
			))
			.with_backend(MockAuthBackend::new(
				Ok(None),
				Err(AuthenticationError::DatabaseError("db2 down".to_string())),
			));

		// Act
		let result = AuthenticationBackend::get_user(&composite, "some_user").await;

		// Assert - all backends errored on get_user, so error is propagated
		assert!(result.is_err());
	}
}
