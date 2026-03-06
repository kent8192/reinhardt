//! Integration with reinhardt-pages
//!
//! This module provides integration between reinhardt-websockets and reinhardt-pages,
//! enabling WebSocket connections to use the same authentication and session management
//! as the HTTP layer.
//!
//! ## Overview
//!
//! This integration allows WebSocket connections to authenticate using the same
//! Cookie/session-based authentication as reinhardt-pages HTTP requests. Cookies
//! from the user's browser session are automatically included in the WebSocket
//! handshake, allowing the server to authenticate the connection.
//!
//! ## Server-Side Setup
//!
//! On the server side, use [`PagesAuthenticator`] to validate WebSocket connections
//! using session cookies:
//!
//! ```ignore
//! use reinhardt_websockets::integration::pages::PagesAuthenticator;
//! use reinhardt_websockets::{WebSocketConsumer, WebSocketConnection, Message};
//! use reinhardt_auth::sessions::InMemorySessionBackend;
//! use async_trait::async_trait;
//! use std::sync::Arc;
//!
//! pub struct ChatHandler<B: SessionBackend> {
//!     authenticator: Arc<PagesAuthenticator<B>>,
//! }
//!
//! #[async_trait]
//! impl<B: SessionBackend + 'static> WebSocketConsumer for ChatHandler<B> {
//!     async fn on_connect(&self, connection: &Arc<WebSocketConnection>) -> WebSocketResult<()> {
//!         // Extract cookies from handshake headers
//!         let cookies = "sessionid=abc123; csrftoken=xyz789"; // From HTTP headers
//!
//!         // Authenticate using session cookies
//!         let user = self.authenticator
//!             .authenticate_from_cookies(cookies)
//!             .await?;
//!
//!         log::info!("User {} connected", user.username());
//!         Ok(())
//!     }
//!
//!     async fn on_message(
//!         &self,
//!         connection: &Arc<WebSocketConnection>,
//!         message: Message,
//!     ) -> WebSocketResult<()> {
//!         // Handle message
//!         Ok(())
//!     }
//! }
//! ```
//!
//! ## Client-Side Setup
//!
//! On the client side (WASM), use the `use_websocket` hook from reinhardt-pages:
//!
//! ```ignore
//! use reinhardt_pages::reactive::hooks::{use_websocket, use_effect, UseWebSocketOptions};
//! use reinhardt_pages::reactive::hooks::{ConnectionState, WebSocketMessage};
//!
//! fn chat_component(room_id: String) -> View {
//!     let ws = use_websocket(
//!         &format!("ws://localhost:8000/ws/chat/{}", room_id),
//!         UseWebSocketOptions::default()
//!     );
//!
//!     use_effect({
//!         let ws = ws.clone();
//!         move || {
//!             if let Some(WebSocketMessage::Text(text)) = ws.latest_message().get() {
//!                 log!("Received: {}", text);
//!             }
//!             None::<fn()>
//!         }
//!     });
//!
//!     page!(|| {
//!         div {
//!             button {
//!                 @click: move |_| {
//!                     ws.send_text("Hello!".to_string()).ok();
//!                 },
//!                 "Send"
//!             }
//!         }
//!     })()
//! }
//! ```
//!
//! ## Authentication Flow
//!
//! 1. User authenticates via HTTP (Cookie/session created by reinhardt-pages)
//! 2. User's browser stores session cookie
//! 3. Client-side JavaScript initiates WebSocket connection
//! 4. Browser automatically includes cookies in WebSocket handshake
//! 5. Server extracts session ID from cookies using [`PagesAuthenticator::authenticate_from_cookies`]
//! 6. Server validates session and retrieves user information
//! 7. WebSocket connection is authenticated and associated with the user
//!
//! ## Session Keys
//!
//! The following keys are expected in the session data:
//!
//! | Key | Type | Required | Description |
//! |-----|------|----------|-------------|
//! | `_auth_user_id` | String | Yes | User ID |
//! | `_auth_user_name` | String | No | Username (defaults to user_id) |
//! | `_auth_user_is_superuser` | bool | No | Superuser flag (defaults to false) |
//! | `_auth_user_permissions` | `Vec<String>` | No | Permission list (defaults to empty) |

use crate::auth::{AuthError, AuthResult, AuthUser, WebSocketAuthenticator};
use crate::connection::WebSocketConnection;
use async_trait::async_trait;
use reinhardt_auth::sessions::{Session, SessionBackend, SessionError};
use std::sync::Arc;

/// Default session cookie name
const DEFAULT_COOKIE_NAME: &str = "sessionid";

/// Default session timeout in seconds (30 minutes)
const DEFAULT_TIMEOUT: u64 = 1800;

/// User authenticated from reinhardt-pages session
///
/// This struct wraps user information extracted from reinhardt-pages'
/// Cookie/session-based authentication system.
#[derive(Debug, Clone)]
pub struct PagesAuthUser {
	/// User ID
	pub user_id: String,
	/// Username
	pub username: String,
	/// User permissions
	pub permissions: Vec<String>,
	/// Whether the user is a superuser
	pub is_superuser: bool,
}

impl AuthUser for PagesAuthUser {
	fn id(&self) -> &str {
		&self.user_id
	}

	fn username(&self) -> &str {
		&self.username
	}

	fn is_authenticated(&self) -> bool {
		!self.user_id.is_empty()
	}

	fn has_permission(&self, permission: &str) -> bool {
		self.is_superuser || self.permissions.contains(&permission.to_string())
	}
}

/// Authenticator that integrates with reinhardt-pages' Cookie/session authentication
///
/// This authenticator extracts session information from WebSocket handshake cookies
/// and validates them against reinhardt-pages' session store.
///
/// # Type Parameters
///
/// * `B` - The session backend type (e.g., `InMemorySessionBackend`)
///
/// # Example
///
/// ```ignore
/// use reinhardt_websockets::integration::pages::PagesAuthenticator;
/// use reinhardt_auth::sessions::InMemorySessionBackend;
///
/// let backend = InMemorySessionBackend::new();
/// let authenticator = PagesAuthenticator::new(backend);
///
/// // During WebSocket handshake, extract cookies from HTTP headers
/// let cookie_header = "sessionid=abc123; csrftoken=xyz789";
/// let user = authenticator.authenticate_from_cookies(cookie_header).await?;
/// ```
pub struct PagesAuthenticator<B: SessionBackend> {
	session_backend: B,
	cookie_name: String,
	timeout: Option<u64>,
}

impl<B: SessionBackend> PagesAuthenticator<B> {
	/// Create a new PagesAuthenticator with the given session backend
	///
	/// Uses default settings:
	/// - Cookie name: "sessionid"
	/// - Timeout: 30 minutes (1800 seconds)
	///
	/// # Arguments
	///
	/// * `session_backend` - The session backend to use for session validation
	///
	/// # Example
	///
	/// ```ignore
	/// use reinhardt_websockets::integration::pages::PagesAuthenticator;
	/// use reinhardt_auth::sessions::InMemorySessionBackend;
	///
	/// let backend = InMemorySessionBackend::new();
	/// let authenticator = PagesAuthenticator::new(backend);
	/// ```
	pub fn new(session_backend: B) -> Self {
		Self {
			session_backend,
			cookie_name: DEFAULT_COOKIE_NAME.to_string(),
			timeout: Some(DEFAULT_TIMEOUT),
		}
	}

	/// Set a custom cookie name for session ID extraction
	///
	/// # Arguments
	///
	/// * `name` - The cookie name to use (e.g., "my_session_id")
	///
	/// # Example
	///
	/// ```ignore
	/// use reinhardt_websockets::integration::pages::PagesAuthenticator;
	/// use reinhardt_auth::sessions::InMemorySessionBackend;
	///
	/// let backend = InMemorySessionBackend::new();
	/// let authenticator = PagesAuthenticator::new(backend)
	///     .with_cookie_name("my_session_id");
	/// ```
	pub fn with_cookie_name(mut self, name: impl Into<String>) -> Self {
		self.cookie_name = name.into();
		self
	}

	/// Set a custom session timeout
	///
	/// # Arguments
	///
	/// * `timeout` - Timeout in seconds, or `None` to disable timeout validation
	///
	/// # Example
	///
	/// ```ignore
	/// use reinhardt_websockets::integration::pages::PagesAuthenticator;
	/// use reinhardt_auth::sessions::InMemorySessionBackend;
	///
	/// let backend = InMemorySessionBackend::new();
	/// let authenticator = PagesAuthenticator::new(backend)
	///     .with_timeout(Some(3600)); // 1 hour timeout
	/// ```
	pub fn with_timeout(mut self, timeout: Option<u64>) -> Self {
		self.timeout = timeout;
		self
	}

	/// Authenticate a user from Cookie header string
	///
	/// This method extracts the session ID from the Cookie header and validates it
	/// against the session store to retrieve user information.
	///
	/// # Arguments
	///
	/// * `cookies` - Cookie header string (e.g., "sessionid=abc123; csrftoken=xyz789")
	///
	/// # Returns
	///
	/// Returns the authenticated user on success, or an error if:
	/// - Session ID is missing or invalid
	/// - Session has expired
	/// - User ID is not found in session
	///
	/// # Errors
	///
	/// - `AuthError::AuthenticationFailed` - Session ID not found in cookies
	/// - `AuthError::TokenExpired` - Session has timed out
	/// - `AuthError::AuthenticationFailed` - Session store error or missing user ID
	///
	/// # Example
	///
	/// ```ignore
	/// use reinhardt_websockets::integration::pages::PagesAuthenticator;
	/// use reinhardt_auth::sessions::InMemorySessionBackend;
	///
	/// let backend = InMemorySessionBackend::new();
	/// let authenticator = PagesAuthenticator::new(backend);
	///
	/// let cookies = "sessionid=valid_session_id; csrftoken=xyz789";
	/// let user = authenticator.authenticate_from_cookies(cookies).await?;
	/// println!("User {} authenticated", user.username());
	/// ```
	pub async fn authenticate_from_cookies(&self, cookies: &str) -> AuthResult<Box<dyn AuthUser>> {
		// Extract session ID from cookies
		let session_id = self.extract_session_id(cookies)?;

		// Load session from backend
		let mut session = Session::from_key(self.session_backend.clone(), session_id)
			.await
			.map_err(Self::map_session_error)?;

		// Validate timeout if configured
		if let Some(timeout) = self.timeout {
			session.set_timeout(timeout);
		}
		session
			.validate_timeout()
			.map_err(|_| AuthError::TokenExpired)?;

		// Extract user data from session
		let user_id: String = session
			.get("_auth_user_id")
			.map_err(|e| AuthError::AuthenticationFailed(format!("Failed to read user ID: {}", e)))?
			.ok_or_else(|| {
				AuthError::AuthenticationFailed("User ID not found in session".to_string())
			})?;

		let username: String = session
			.get("_auth_user_name")
			.map_err(|e| {
				AuthError::AuthenticationFailed(format!("Failed to read username: {}", e))
			})?
			.unwrap_or_else(|| user_id.clone());

		let is_superuser: bool = session
			.get("_auth_user_is_superuser")
			.map_err(|e| {
				AuthError::AuthenticationFailed(format!("Failed to read superuser flag: {}", e))
			})?
			.unwrap_or(false);

		let permissions: Vec<String> = session
			.get("_auth_user_permissions")
			.map_err(|e| {
				AuthError::AuthenticationFailed(format!("Failed to read permissions: {}", e))
			})?
			.unwrap_or_default();

		// Build and return PagesAuthUser
		Ok(Box::new(PagesAuthUser {
			user_id,
			username,
			permissions,
			is_superuser,
		}))
	}

	/// Extract session ID from Cookie header
	///
	/// Parses the Cookie header string and extracts the session ID.
	///
	/// # Arguments
	///
	/// * `cookies` - Cookie header string
	///
	/// # Returns
	///
	/// Returns the session ID if found, or an error if not found.
	fn extract_session_id(&self, cookies: &str) -> AuthResult<String> {
		// Parse cookies and look for the configured cookie name
		for cookie in cookies.split(';') {
			let cookie = cookie.trim();
			if let Some((name, value)) = cookie.split_once('=')
				&& name.trim() == self.cookie_name
			{
				return Ok(value.trim().to_string());
			}
		}

		Err(AuthError::AuthenticationFailed(format!(
			"Session ID not found in cookies (looking for '{}')",
			self.cookie_name
		)))
	}

	/// Map SessionError to AuthError
	fn map_session_error(error: SessionError) -> AuthError {
		match error {
			SessionError::SessionExpired => AuthError::TokenExpired,
			SessionError::CacheError(msg) => {
				AuthError::AuthenticationFailed(format!("Session store error: {}", msg))
			}
			SessionError::SerializationError(msg) => {
				AuthError::AuthenticationFailed(format!("Session data error: {}", msg))
			}
			_ => AuthError::AuthenticationFailed(format!("Session error: {}", error)),
		}
	}
}

#[async_trait]
impl<B: SessionBackend + 'static> WebSocketAuthenticator for PagesAuthenticator<B> {
	async fn authenticate(
		&self,
		_connection: &Arc<WebSocketConnection>,
		credentials: &str,
	) -> AuthResult<Box<dyn AuthUser>> {
		// credentials parameter contains the Cookie header string
		self.authenticate_from_cookies(credentials).await
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use reinhardt_auth::sessions::InMemorySessionBackend;
	use rstest::{fixture, rstest};

	#[fixture]
	fn backend() -> InMemorySessionBackend {
		InMemorySessionBackend::new()
	}

	#[rstest]
	fn test_pages_auth_user_creation() {
		let user = PagesAuthUser {
			user_id: "user_1".to_string(),
			username: "alice".to_string(),
			permissions: vec!["chat.read".to_string(), "chat.write".to_string()],
			is_superuser: false,
		};

		assert_eq!(user.id(), "user_1");
		assert_eq!(user.username(), "alice");
		assert!(user.is_authenticated());
		assert!(user.has_permission("chat.read"));
		assert!(user.has_permission("chat.write"));
		assert!(!user.has_permission("admin.delete"));
	}

	#[rstest]
	fn test_pages_auth_user_superuser() {
		let user = PagesAuthUser {
			user_id: "admin_1".to_string(),
			username: "admin".to_string(),
			permissions: vec![],
			is_superuser: true,
		};

		// Superuser has all permissions
		assert!(user.has_permission("any.permission"));
		assert!(user.has_permission("admin.delete"));
	}

	#[rstest]
	fn test_extract_session_id_success(backend: InMemorySessionBackend) {
		let auth = PagesAuthenticator::new(backend);
		let cookies = "sessionid=abc123; csrftoken=xyz789";
		let session_id = auth.extract_session_id(cookies).unwrap();
		assert_eq!(session_id, "abc123");
	}

	#[rstest]
	fn test_extract_session_id_with_spaces(backend: InMemorySessionBackend) {
		let auth = PagesAuthenticator::new(backend);
		let cookies = "sessionid = abc123 ; csrftoken = xyz789";
		let session_id = auth.extract_session_id(cookies).unwrap();
		assert_eq!(session_id, "abc123");
	}

	#[rstest]
	fn test_extract_session_id_not_found(backend: InMemorySessionBackend) {
		let auth = PagesAuthenticator::new(backend);
		let cookies = "csrftoken=xyz789; other=value";
		let result = auth.extract_session_id(cookies);
		assert!(result.is_err());
	}

	#[rstest]
	fn test_extract_session_id_empty_cookies(backend: InMemorySessionBackend) {
		let auth = PagesAuthenticator::new(backend);
		let cookies = "";
		let result = auth.extract_session_id(cookies);
		assert!(result.is_err());
	}

	#[rstest]
	fn test_custom_cookie_name(backend: InMemorySessionBackend) {
		let auth = PagesAuthenticator::new(backend).with_cookie_name("my_session");
		let cookies = "my_session=custom123; sessionid=default456";
		let session_id = auth.extract_session_id(cookies).unwrap();
		assert_eq!(session_id, "custom123");
	}

	#[rstest]
	fn test_custom_cookie_name_not_found(backend: InMemorySessionBackend) {
		let auth = PagesAuthenticator::new(backend).with_cookie_name("my_session");
		let cookies = "sessionid=abc123";
		let result = auth.extract_session_id(cookies);
		assert!(result.is_err());
		let err = result.unwrap_err();
		match err {
			AuthError::AuthenticationFailed(msg) => {
				assert!(msg.contains("my_session"));
			}
			_ => panic!("Expected AuthenticationFailed error"),
		}
	}

	#[rstest]
	#[tokio::test]
	async fn test_authenticate_valid_session(backend: InMemorySessionBackend) {
		// Create and save a session with user data
		let mut session = Session::new(backend.clone());
		session.set("_auth_user_id", "user_123").unwrap();
		session.set("_auth_user_name", "testuser").unwrap();
		session.set("_auth_user_is_superuser", false).unwrap();
		session
			.set(
				"_auth_user_permissions",
				vec!["read".to_string(), "write".to_string()],
			)
			.unwrap();
		session.save().await.unwrap();
		let session_key = session.session_key().unwrap().to_string();

		// Authenticate using the session
		let auth = PagesAuthenticator::new(backend);
		let cookies = format!("sessionid={}; csrftoken=xyz789", session_key);
		let user = auth.authenticate_from_cookies(&cookies).await.unwrap();

		assert_eq!(user.id(), "user_123");
		assert_eq!(user.username(), "testuser");
		assert!(user.is_authenticated());
		assert!(user.has_permission("read"));
		assert!(user.has_permission("write"));
		assert!(!user.has_permission("admin"));
	}

	#[rstest]
	#[tokio::test]
	async fn test_authenticate_missing_user_id(backend: InMemorySessionBackend) {
		// Create a session without user ID
		let mut session = Session::new(backend.clone());
		session.set("_auth_user_name", "testuser").unwrap();
		session.save().await.unwrap();
		let session_key = session.session_key().unwrap().to_string();

		// Authenticate using the session
		let auth = PagesAuthenticator::new(backend);
		let cookies = format!("sessionid={}", session_key);
		let result = auth.authenticate_from_cookies(&cookies).await;

		assert!(result.is_err());
		let err = result.unwrap_err();
		match err {
			AuthError::AuthenticationFailed(msg) => {
				assert!(msg.contains("User ID not found"));
			}
			_ => panic!("Expected AuthenticationFailed error"),
		}
	}

	#[rstest]
	#[tokio::test]
	async fn test_authenticate_superuser_permissions(backend: InMemorySessionBackend) {
		// Create a superuser session
		let mut session = Session::new(backend.clone());
		session.set("_auth_user_id", "admin_1").unwrap();
		session.set("_auth_user_name", "admin").unwrap();
		session.set("_auth_user_is_superuser", true).unwrap();
		session.save().await.unwrap();
		let session_key = session.session_key().unwrap().to_string();

		// Authenticate using the session
		let auth = PagesAuthenticator::new(backend);
		let cookies = format!("sessionid={}", session_key);
		let user = auth.authenticate_from_cookies(&cookies).await.unwrap();

		// Superuser has all permissions
		assert!(user.has_permission("any.permission"));
		assert!(user.has_permission("admin.delete"));
		assert!(user.has_permission("chat.write"));
	}

	#[rstest]
	#[tokio::test]
	async fn test_authenticate_default_values(backend: InMemorySessionBackend) {
		// Create a session with only user_id (other fields use defaults)
		let mut session = Session::new(backend.clone());
		session.set("_auth_user_id", "minimal_user").unwrap();
		session.save().await.unwrap();
		let session_key = session.session_key().unwrap().to_string();

		// Authenticate using the session
		let auth = PagesAuthenticator::new(backend);
		let cookies = format!("sessionid={}", session_key);
		let user = auth.authenticate_from_cookies(&cookies).await.unwrap();

		// Username defaults to user_id
		assert_eq!(user.username(), "minimal_user");
		// Not a superuser by default
		assert!(!user.has_permission("admin.access"));
	}

	#[rstest]
	#[tokio::test]
	async fn test_authenticate_missing_session_id(backend: InMemorySessionBackend) {
		let auth = PagesAuthenticator::new(backend);
		let cookies = "csrftoken=xyz789";
		let result = auth.authenticate_from_cookies(cookies).await;

		assert!(result.is_err());
		let err = result.unwrap_err();
		match err {
			AuthError::AuthenticationFailed(msg) => {
				assert!(msg.contains("Session ID not found"));
			}
			_ => panic!("Expected AuthenticationFailed error"),
		}
	}

	#[rstest]
	#[tokio::test]
	async fn test_authenticate_nonexistent_session(backend: InMemorySessionBackend) {
		let auth = PagesAuthenticator::new(backend);
		let cookies = "sessionid=nonexistent_session_id";
		let result = auth.authenticate_from_cookies(cookies).await;

		// Session doesn't exist, so user_id won't be found
		assert!(result.is_err());
	}

	#[rstest]
	#[tokio::test]
	async fn test_custom_timeout(backend: InMemorySessionBackend) {
		// Create a session
		let mut session = Session::new(backend.clone());
		session.set("_auth_user_id", "user_123").unwrap();
		session.save().await.unwrap();
		let session_key = session.session_key().unwrap().to_string();

		// Authenticate with custom timeout (very long to ensure it works)
		let auth = PagesAuthenticator::new(backend).with_timeout(Some(86400)); // 24 hours
		let cookies = format!("sessionid={}", session_key);
		let result = auth.authenticate_from_cookies(&cookies).await;

		assert!(result.is_ok());
	}

	#[rstest]
	#[tokio::test]
	async fn test_disabled_timeout(backend: InMemorySessionBackend) {
		// Create a session
		let mut session = Session::new(backend.clone());
		session.set("_auth_user_id", "user_123").unwrap();
		session.save().await.unwrap();
		let session_key = session.session_key().unwrap().to_string();

		// Authenticate with disabled timeout
		let auth = PagesAuthenticator::new(backend).with_timeout(None);
		let cookies = format!("sessionid={}", session_key);
		let result = auth.authenticate_from_cookies(&cookies).await;

		assert!(result.is_ok());
	}

	#[rstest]
	#[tokio::test]
	async fn test_websocket_authenticator_trait(backend: InMemorySessionBackend) {
		use tokio::sync::mpsc;

		// Create a session with user data
		let mut session = Session::new(backend.clone());
		session.set("_auth_user_id", "ws_user").unwrap();
		session.set("_auth_user_name", "websocket_user").unwrap();
		session.save().await.unwrap();
		let session_key = session.session_key().unwrap().to_string();

		// Create authenticator and connection
		let auth = PagesAuthenticator::new(backend);
		let (tx, _rx) = mpsc::unbounded_channel();
		let conn = Arc::new(WebSocketConnection::new("conn_1".to_string(), tx));

		// Authenticate using WebSocketAuthenticator trait
		let cookies = format!("sessionid={}; csrftoken=abc", session_key);
		let user = auth.authenticate(&conn, &cookies).await.unwrap();

		assert_eq!(user.id(), "ws_user");
		assert_eq!(user.username(), "websocket_user");
	}
}
