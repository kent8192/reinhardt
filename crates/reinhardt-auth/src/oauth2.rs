//! OAuth2 Authentication
//!
//! Provides OAuth2 authorization flow support for third-party authentication.

use crate::{AuthenticationBackend, AuthenticationError, SimpleUser, User};
use async_trait::async_trait;
use reinhardt_http::Request;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use subtle::ConstantTimeEq;
use tokio::sync::Mutex;
use uuid::Uuid;

/// OAuth2 grant type
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum GrantType {
	/// Authorization code grant
	AuthorizationCode,
	/// Client credentials grant
	ClientCredentials,
	/// Refresh token grant
	RefreshToken,
	/// Implicit grant (deprecated, not recommended)
	Implicit,
}

/// OAuth2 access token
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessToken {
	/// Token value
	pub token: String,
	/// Token type (typically "Bearer")
	pub token_type: String,
	/// Expires in seconds
	pub expires_in: u64,
	/// Refresh token
	pub refresh_token: Option<String>,
	/// Scope granted
	pub scope: Option<String>,
}

/// Maximum lifetime of an authorization code per RFC 6749 Section 4.1.2
const AUTHORIZATION_CODE_TTL: Duration = Duration::from_secs(600);

/// OAuth2 authorization code
#[derive(Debug, Clone)]
pub struct AuthorizationCode {
	/// Code value
	pub code: String,
	/// Client ID
	pub client_id: String,
	/// Redirect URI
	pub redirect_uri: String,
	/// User ID
	pub user_id: String,
	/// Scope
	pub scope: Option<String>,
	/// Timestamp when the code was created
	pub created_at: Instant,
}

/// OAuth2 application/client
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuth2Application {
	/// Client ID
	pub client_id: String,
	/// Client secret
	pub client_secret: String,
	/// Allowed redirect URIs
	pub redirect_uris: Vec<String>,
	/// Grant types allowed
	pub grant_types: Vec<GrantType>,
}

/// OAuth2 token storage trait
#[async_trait]
pub trait OAuth2TokenStore: Send + Sync {
	/// Store an authorization code
	async fn store_code(&self, code: AuthorizationCode) -> Result<(), String>;

	/// Get and consume an authorization code
	async fn consume_code(&self, code: &str) -> Result<Option<AuthorizationCode>, String>;

	/// Store an access token
	async fn store_token(&self, user_id: &str, token: AccessToken) -> Result<(), String>;

	/// Get an access token
	async fn get_token(&self, token: &str) -> Result<Option<String>, String>;

	/// Revoke a token
	async fn revoke_token(&self, token: &str) -> Result<(), String>;
}

/// User repository trait for OAuth2 authentication
///
/// Provides an abstraction for retrieving user data from various storage backends.
///
/// # Examples
///
/// ```
/// use reinhardt_auth::{UserRepository, User};
/// use async_trait::async_trait;
///
/// struct MyUserRepository;
///
/// #[async_trait]
/// impl UserRepository for MyUserRepository {
///     async fn get_user_by_id(&self, user_id: &str) -> Result<Option<Box<dyn User>>, String> {
///         // Custom implementation
///         Ok(None)
///     }
/// }
/// ```
#[async_trait]
pub trait UserRepository: Send + Sync {
	/// Get user by ID
	///
	/// Returns `Ok(Some(user))` if found, `Ok(None)` if not found, or `Err` on error.
	async fn get_user_by_id(&self, user_id: &str) -> Result<Option<Box<dyn User>>, String>;
}

/// In-memory OAuth2 token store
///
/// # Examples
///
/// ```
/// use reinhardt_auth::{InMemoryOAuth2Store, AuthorizationCode, OAuth2TokenStore};
///
/// #[tokio::main]
/// async fn main() {
///     let store = InMemoryOAuth2Store::new();
///
///     let code = AuthorizationCode {
///         code: "auth_code_123".to_string(),
///         client_id: "client_1".to_string(),
///         redirect_uri: "https://example.com/callback".to_string(),
///         user_id: "user_456".to_string(),
///         scope: Some("read write".to_string()),
///         created_at: std::time::Instant::now(),
///     };
///
///     store.store_code(code).await.unwrap();
/// }
/// ```
pub struct InMemoryOAuth2Store {
	codes: Arc<Mutex<HashMap<String, AuthorizationCode>>>,
	tokens: Arc<Mutex<HashMap<String, String>>>, // token -> user_id
}

impl InMemoryOAuth2Store {
	/// Create a new in-memory OAuth2 store
	pub fn new() -> Self {
		Self {
			codes: Arc::new(Mutex::new(HashMap::new())),
			tokens: Arc::new(Mutex::new(HashMap::new())),
		}
	}
}

impl Default for InMemoryOAuth2Store {
	fn default() -> Self {
		Self::new()
	}
}

#[async_trait]
impl OAuth2TokenStore for InMemoryOAuth2Store {
	async fn store_code(&self, code: AuthorizationCode) -> Result<(), String> {
		let mut codes = self.codes.lock().await;
		codes.insert(code.code.clone(), code);
		Ok(())
	}

	async fn consume_code(&self, code: &str) -> Result<Option<AuthorizationCode>, String> {
		let mut codes = self.codes.lock().await;
		match codes.remove(code) {
			Some(auth_code) if auth_code.created_at.elapsed() > AUTHORIZATION_CODE_TTL => {
				Err("authorization code has expired".to_string())
			}
			other => Ok(other),
		}
	}

	async fn store_token(&self, user_id: &str, token: AccessToken) -> Result<(), String> {
		let mut tokens = self.tokens.lock().await;
		tokens.insert(token.token.clone(), user_id.to_string());
		Ok(())
	}

	async fn get_token(&self, token: &str) -> Result<Option<String>, String> {
		let tokens = self.tokens.lock().await;
		Ok(tokens.get(token).cloned())
	}

	async fn revoke_token(&self, token: &str) -> Result<(), String> {
		let mut tokens = self.tokens.lock().await;
		tokens.remove(token);
		Ok(())
	}
}

/// Simple in-memory user repository
///
/// Creates SimpleUser instances on-the-fly without database access.
/// Suitable for testing and development environments.
///
/// # Examples
///
/// ```
/// use reinhardt_auth::{SimpleUserRepository, UserRepository};
///
/// #[tokio::main]
/// async fn main() {
///     let repo = SimpleUserRepository;
///     let user = repo.get_user_by_id("user_123").await.unwrap();
///     assert!(user.is_some());
/// }
/// ```
pub struct SimpleUserRepository;

#[async_trait]
impl UserRepository for SimpleUserRepository {
	async fn get_user_by_id(&self, user_id: &str) -> Result<Option<Box<dyn User>>, String> {
		// Create a simple user object for development/testing
		Ok(Some(Box::new(SimpleUser {
			id: Uuid::new_v4(),
			username: user_id.to_string(),
			email: format!("{}@example.com", user_id),
			is_active: true,
			is_admin: false,
			is_staff: false,
			is_superuser: false,
		})))
	}
}

/// OAuth2 authentication backend
///
/// Provides OAuth2 authorization flow support with customizable user storage.
///
/// # User Repository
///
/// By default, uses `SimpleUserRepository` which creates user objects on-the-fly.
/// For production use, provide a custom `UserRepository` implementation that
/// queries your user database.
///
/// # Examples
///
/// Basic usage with default repository:
///
/// ```
/// use reinhardt_auth::{OAuth2Authentication, OAuth2Application, GrantType};
///
/// let app = OAuth2Application {
///     client_id: "my_client".to_string(),
///     client_secret: "secret".to_string(),
///     redirect_uris: vec!["https://example.com/callback".to_string()],
///     grant_types: vec![GrantType::AuthorizationCode],
/// };
///
/// let auth = OAuth2Authentication::new();
/// auth.register_application(app);
/// ```
///
/// With custom user repository:
///
/// ```ignore
/// use reinhardt_auth::{OAuth2Authentication, SimpleUserRepository};
/// use std::sync::Arc;
///
/// let repo = Arc::new(SimpleUserRepository);
/// let auth = OAuth2Authentication::with_repository(repo);
/// ```
pub struct OAuth2Authentication {
	applications: Arc<Mutex<HashMap<String, OAuth2Application>>>,
	token_store: Arc<dyn OAuth2TokenStore>,
	user_repository: Arc<dyn UserRepository>,
}

impl OAuth2Authentication {
	/// Create a new OAuth2 authentication backend
	///
	/// Uses default implementations:
	/// - Token store: InMemoryOAuth2Store
	/// - User repository: SimpleUserRepository
	pub fn new() -> Self {
		Self {
			applications: Arc::new(Mutex::new(HashMap::new())),
			token_store: Arc::new(InMemoryOAuth2Store::new()),
			user_repository: Arc::new(SimpleUserRepository),
		}
	}

	/// Create with custom token store and default user repository
	pub fn with_store(token_store: Arc<dyn OAuth2TokenStore>) -> Self {
		Self {
			applications: Arc::new(Mutex::new(HashMap::new())),
			token_store,
			user_repository: Arc::new(SimpleUserRepository),
		}
	}

	/// Create with custom user repository and default token store
	pub fn with_repository(user_repository: Arc<dyn UserRepository>) -> Self {
		Self {
			applications: Arc::new(Mutex::new(HashMap::new())),
			token_store: Arc::new(InMemoryOAuth2Store::new()),
			user_repository,
		}
	}

	/// Create with custom token store and user repository
	pub fn with_store_and_repository(
		token_store: Arc<dyn OAuth2TokenStore>,
		user_repository: Arc<dyn UserRepository>,
	) -> Self {
		Self {
			applications: Arc::new(Mutex::new(HashMap::new())),
			token_store,
			user_repository,
		}
	}

	/// Register an OAuth2 application
	pub async fn register_application(&self, app: OAuth2Application) {
		let mut applications = self.applications.lock().await;
		applications.insert(app.client_id.clone(), app);
	}

	/// Validate client credentials
	///
	/// Uses constant-time comparison to prevent timing attacks on client secrets.
	pub async fn validate_client(&self, client_id: &str, client_secret: &str) -> bool {
		let applications = self.applications.lock().await;
		if let Some(app) = applications.get(client_id) {
			app.client_secret
				.as_bytes()
				.ct_eq(client_secret.as_bytes())
				.into()
		} else {
			false
		}
	}

	/// Generate authorization code
	pub async fn generate_authorization_code(
		&self,
		client_id: &str,
		redirect_uri: &str,
		user_id: &str,
		scope: Option<String>,
	) -> Result<String, String> {
		let code = format!("code_{}", Uuid::new_v4());

		let auth_code = AuthorizationCode {
			code: code.clone(),
			client_id: client_id.to_string(),
			redirect_uri: redirect_uri.to_string(),
			user_id: user_id.to_string(),
			scope,
			created_at: Instant::now(),
		};

		self.token_store.store_code(auth_code).await?;
		Ok(code)
	}

	/// Exchange authorization code for access token
	pub async fn exchange_code(
		&self,
		code: &str,
		client_id: &str,
		client_secret: &str,
	) -> Result<AccessToken, String> {
		// Validate client
		if !self.validate_client(client_id, client_secret).await {
			return Err("Invalid client credentials".to_string());
		}

		// Consume authorization code
		let auth_code = self
			.token_store
			.consume_code(code)
			.await?
			.ok_or_else(|| "Invalid or expired authorization code".to_string())?;

		// Verify the authorization code was issued to the requesting client
		if auth_code.client_id != client_id {
			return Err("Authorization code was not issued to this client".to_string());
		}

		// Generate access token
		let token = AccessToken {
			token: format!("access_{}", Uuid::new_v4()),
			token_type: "Bearer".to_string(),
			expires_in: 3600,
			refresh_token: Some(format!("refresh_{}", Uuid::new_v4())),
			scope: auth_code.scope.clone(),
		};

		// Store token
		self.token_store
			.store_token(&auth_code.user_id, token.clone())
			.await?;

		Ok(token)
	}
}

impl Default for OAuth2Authentication {
	fn default() -> Self {
		Self::new()
	}
}

#[async_trait]
impl AuthenticationBackend for OAuth2Authentication {
	async fn authenticate(
		&self,
		request: &Request,
	) -> Result<Option<Box<dyn User>>, AuthenticationError> {
		// Extract Bearer token from Authorization header
		let auth_header = request
			.headers
			.get("Authorization")
			.and_then(|h| h.to_str().ok());

		if let Some(header) = auth_header
			&& let Some(token) = header.strip_prefix("Bearer ")
		{
			// Query the token store asynchronously
			match self.token_store.get_token(token).await {
				Ok(Some(user_id)) => {
					// Token is valid, get the user
					return self.get_user(&user_id).await;
				}
				Ok(None) => {
					// Token not found or expired
					return Ok(None);
				}
				Err(e) => {
					// Error querying token store
					return Err(AuthenticationError::Unknown(format!(
						"Token store error: {}",
						e
					)));
				}
			}
		}

		Ok(None)
	}

	async fn get_user(&self, user_id: &str) -> Result<Option<Box<dyn User>>, AuthenticationError> {
		self.user_repository
			.get_user_by_id(user_id)
			.await
			.map_err(|e| AuthenticationError::Unknown(format!("User repository error: {}", e)))
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[tokio::test]
	async fn test_oauth2_application() {
		let app = OAuth2Application {
			client_id: "test_client".to_string(),
			client_secret: "test_secret".to_string(),
			redirect_uris: vec!["https://example.com/callback".to_string()],
			grant_types: vec![GrantType::AuthorizationCode],
		};

		let auth = OAuth2Authentication::new();
		auth.register_application(app).await;

		assert!(auth.validate_client("test_client", "test_secret").await);
		assert!(!auth.validate_client("test_client", "wrong_secret").await);
	}

	#[tokio::test]
	async fn test_authorization_code_flow() {
		let app = OAuth2Application {
			client_id: "test_client".to_string(),
			client_secret: "test_secret".to_string(),
			redirect_uris: vec!["https://example.com/callback".to_string()],
			grant_types: vec![GrantType::AuthorizationCode],
		};

		let auth = OAuth2Authentication::new();
		auth.register_application(app).await;

		// Generate authorization code
		let code = auth
			.generate_authorization_code(
				"test_client",
				"https://example.com/callback",
				"user_123",
				Some("read write".to_string()),
			)
			.await
			.unwrap();

		assert!(code.starts_with("code_"));

		// Exchange code for token
		let token = auth
			.exchange_code(&code, "test_client", "test_secret")
			.await
			.unwrap();

		assert_eq!(token.token_type, "Bearer");
		assert_eq!(token.expires_in, 3600);
		assert!(token.refresh_token.is_some());
	}

	#[tokio::test]
	async fn test_token_store() {
		let store = InMemoryOAuth2Store::new();

		let code = AuthorizationCode {
			code: "test_code".to_string(),
			client_id: "client_1".to_string(),
			redirect_uri: "https://example.com/callback".to_string(),
			user_id: "user_123".to_string(),
			scope: Some("read".to_string()),
			created_at: Instant::now(),
		};

		store.store_code(code.clone()).await.unwrap();

		let retrieved = store.consume_code("test_code").await.unwrap();
		assert!(retrieved.is_some());
		assert_eq!(retrieved.unwrap().user_id, "user_123");

		// Code should be consumed
		let consumed = store.consume_code("test_code").await.unwrap();
		assert!(consumed.is_none());
	}

	#[tokio::test]
	async fn test_exchange_code_rejects_mismatched_client_id() {
		// Arrange - register two clients
		let app_a = OAuth2Application {
			client_id: "client_a".to_string(),
			client_secret: "secret_a".to_string(),
			redirect_uris: vec!["https://a.example.com/callback".to_string()],
			grant_types: vec![GrantType::AuthorizationCode],
		};
		let app_b = OAuth2Application {
			client_id: "client_b".to_string(),
			client_secret: "secret_b".to_string(),
			redirect_uris: vec!["https://b.example.com/callback".to_string()],
			grant_types: vec![GrantType::AuthorizationCode],
		};

		let auth = OAuth2Authentication::new();
		auth.register_application(app_a).await;
		auth.register_application(app_b).await;

		// Generate code for client_a
		let code = auth
			.generate_authorization_code(
				"client_a",
				"https://a.example.com/callback",
				"user_123",
				None,
			)
			.await
			.unwrap();

		// Act - try to exchange code using client_b's credentials
		let result = auth.exchange_code(&code, "client_b", "secret_b").await;

		// Assert - should reject because code was issued to client_a
		assert!(result.is_err());
		assert_eq!(
			result.unwrap_err(),
			"Authorization code was not issued to this client"
		);
	}

	#[tokio::test]
	async fn test_invalid_client_credentials() {
		let app = OAuth2Application {
			client_id: "test_client".to_string(),
			client_secret: "test_secret".to_string(),
			redirect_uris: vec!["https://example.com/callback".to_string()],
			grant_types: vec![GrantType::AuthorizationCode],
		};

		let auth = OAuth2Authentication::new();
		auth.register_application(app).await;

		let code = auth
			.generate_authorization_code(
				"test_client",
				"https://example.com/callback",
				"user_123",
				None,
			)
			.await
			.unwrap();

		let result = auth
			.exchange_code(&code, "test_client", "wrong_secret")
			.await;

		assert!(result.is_err());
	}

	#[tokio::test]
	async fn test_simple_user_repository() {
		let repo = SimpleUserRepository;

		// Get user by ID
		let user = repo.get_user_by_id("test_user").await.unwrap();
		assert!(user.is_some());

		let user = user.unwrap();
		assert_eq!(user.get_username(), "test_user");
		assert!(user.is_authenticated());
		assert!(user.is_active());
	}

	#[tokio::test]
	async fn test_oauth2_with_default_repository() {
		let auth = OAuth2Authentication::new();

		// Get user via default SimpleUserRepository
		let user = auth.get_user("user_456").await.unwrap();
		assert!(user.is_some());

		let user = user.unwrap();
		assert_eq!(user.get_username(), "user_456");
	}

	#[tokio::test]
	async fn test_oauth2_with_custom_repository() {
		// Custom repository for testing
		struct MockUserRepository {
			username: String,
		}

		#[async_trait]
		impl UserRepository for MockUserRepository {
			async fn get_user_by_id(&self, user_id: &str) -> Result<Option<Box<dyn User>>, String> {
				if user_id == "mock_user" {
					Ok(Some(Box::new(SimpleUser {
						id: Uuid::from_u128(999),
						username: self.username.clone(),
						email: "mock@example.com".to_string(),
						is_active: true,
						is_admin: true,
						is_staff: true,
						is_superuser: true,
					})))
				} else {
					Ok(None)
				}
			}
		}

		let custom_repo = Arc::new(MockUserRepository {
			username: "custom_mock_user".to_string(),
		});

		let auth = OAuth2Authentication::with_repository(custom_repo);

		// Get user via custom repository
		let user = auth.get_user("mock_user").await.unwrap();
		assert!(user.is_some());

		let user = user.unwrap();
		assert_eq!(user.get_username(), "custom_mock_user");
		assert!(user.is_admin());

		// Non-existent user
		let user = auth.get_user("nonexistent").await.unwrap();
		assert!(user.is_none());
	}

	#[tokio::test]
	async fn test_oauth2_with_store_and_repository() {
		struct CustomRepository;

		#[async_trait]
		impl UserRepository for CustomRepository {
			async fn get_user_by_id(&self, user_id: &str) -> Result<Option<Box<dyn User>>, String> {
				Ok(Some(Box::new(SimpleUser {
					id: Uuid::from_u128(777),
					username: format!("custom_{}", user_id),
					email: format!("{}@custom.com", user_id),
					is_active: true,
					is_admin: false,
					is_staff: true,
					is_superuser: false,
				})))
			}
		}

		let token_store = Arc::new(InMemoryOAuth2Store::new());
		let user_repo = Arc::new(CustomRepository);

		let auth = OAuth2Authentication::with_store_and_repository(token_store, user_repo);

		// Verify custom repository is used
		let user = auth.get_user("test").await.unwrap().unwrap();
		assert_eq!(user.get_username(), "custom_test");
	}
}
