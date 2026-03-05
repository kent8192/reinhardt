//! REST API Authentication
//!
//! Provides REST API-compatible authentication wrappers and combinators.

#[cfg(feature = "argon2-hasher")]
use crate::DefaultUser;
use crate::sessions::{Session, backends::SessionBackend};
use crate::{AuthenticationBackend, AuthenticationError, SimpleUser, User};
use reinhardt_http::Request;
use std::sync::Arc;

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
	async fn authenticate(
		&self,
		request: &Request,
	) -> Result<Option<Box<dyn User>>, AuthenticationError> {
		// Try each backend in order
		for backend in &self.backends {
			match backend.authenticate(request).await {
				Ok(Some(user)) => return Ok(Some(user)),
				Ok(None) => continue,
				Err(e) => {
					// Log error but continue to next backend
					tracing::warn!("Authentication backend error occurred");
					tracing::debug!(error = %e, "Authentication backend error details");
					continue;
				}
			}
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
		// Try each backend in order until one succeeds
		// This is a fallback approach since we don't track which backend authenticated the user
		for backend in &self.backends {
			match backend.get_user(user_id).await {
				Ok(Some(user)) => return Ok(Some(user)),
				Ok(None) => continue,
				Err(e) => {
					// Log error but continue to next backend
					tracing::warn!("get_user backend error occurred");
					tracing::debug!(error = %e, "get_user backend error details");
					continue;
				}
			}
		}
		Ok(None)
	}
}

/// Token authentication using custom tokens
pub struct TokenAuthentication {
	/// Token store (token -> user_id)
	tokens: std::collections::HashMap<String, String>,
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
			config: TokenAuthConfig::default(),
		}
	}

	/// Create with custom configuration
	pub fn with_config(config: TokenAuthConfig) -> Self {
		Self {
			tokens: std::collections::HashMap::new(),
			config,
		}
	}

	/// Add a token for a user
	pub fn add_token(&mut self, token: impl Into<String>, user_id: impl Into<String>) {
		self.tokens.insert(token.into(), user_id.into());
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
				&& let Some(user_id) = self.tokens.get(token)
			{
				// Try to parse user_id as UUID, or generate a new one if it fails
				let id = uuid::Uuid::parse_str(user_id).unwrap_or_else(|_| {
					uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_OID, user_id.as_bytes())
				});
				return Ok(Some(Box::new(SimpleUser {
					id,
					username: user_id.clone(),
					email: format!("{}@example.com", user_id),
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
				uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_OID, user_id.as_bytes())
			});
			Ok(Some(Box::new(SimpleUser {
				id,
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
				id: uuid::Uuid::new_v4(),
				username: username.to_string(),
				email: format!("{}@example.com", username),
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
	#[cfg(feature = "jwt")]
	use crate::basic::BasicAuthentication;
	use bytes::Bytes;
	use hyper::{HeaderMap, Method};

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
}
