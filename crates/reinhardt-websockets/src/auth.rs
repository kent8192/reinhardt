//! WebSocket authentication and authorization
//!
//! This module provides authentication and authorization hooks for WebSocket connections,
//! integrating with Reinhardt's auth system.

use crate::connection::{WebSocketConnection, WebSocketError, WebSocketResult};
use async_trait::async_trait;
use std::sync::Arc;

/// Authentication result for WebSocket connections
pub type AuthResult<T> = Result<T, AuthError>;

/// Authentication errors
#[derive(Debug, thiserror::Error)]
pub enum AuthError {
	#[error("Authentication failed: {0}")]
	AuthenticationFailed(String),
	#[error("Authorization denied: {0}")]
	AuthorizationDenied(String),
	#[error("Invalid credentials")]
	InvalidCredentials,
	#[error("Token expired")]
	TokenExpired,
	#[error("Missing authentication")]
	MissingAuthentication,
}

/// Authenticated user information
pub trait AuthUser: Send + Sync + std::fmt::Debug {
	/// Get user identifier
	fn id(&self) -> &str;
	/// Get username
	fn username(&self) -> &str;
	/// Check if user is authenticated
	fn is_authenticated(&self) -> bool;
	/// Check if user has specific permission
	fn has_permission(&self, permission: &str) -> bool;
}

/// Simple user implementation for WebSocket authentication
///
/// # Examples
///
/// ```
/// use reinhardt_websockets::auth::{SimpleAuthUser, AuthUser};
///
/// let user = SimpleAuthUser::new(
///     "user_123".to_string(),
///     "alice".to_string(),
///     vec!["chat.read".to_string(), "chat.write".to_string()],
/// );
///
/// assert_eq!(user.id(), "user_123");
/// assert_eq!(user.username(), "alice");
/// assert!(user.is_authenticated());
/// assert!(user.has_permission("chat.read"));
/// assert!(!user.has_permission("admin.access"));
/// ```
#[derive(Debug, Clone)]
pub struct SimpleAuthUser {
	id: String,
	username: String,
	permissions: Vec<String>,
}

impl SimpleAuthUser {
	/// Create a new authenticated user
	pub fn new(id: String, username: String, permissions: Vec<String>) -> Self {
		Self {
			id,
			username,
			permissions,
		}
	}
}

impl AuthUser for SimpleAuthUser {
	fn id(&self) -> &str {
		&self.id
	}

	fn username(&self) -> &str {
		&self.username
	}

	fn is_authenticated(&self) -> bool {
		!self.id.is_empty()
	}

	fn has_permission(&self, permission: &str) -> bool {
		self.permissions.contains(&permission.to_string())
	}
}

/// WebSocket authenticator trait
///
/// Implementors define how to authenticate WebSocket connections.
#[async_trait]
pub trait WebSocketAuthenticator: Send + Sync {
	/// Authenticate a WebSocket connection
	///
	/// # Arguments
	///
	/// * `connection` - The WebSocket connection to authenticate
	/// * `credentials` - Authentication credentials (e.g., token, cookie)
	///
	/// # Returns
	///
	/// Returns the authenticated user on success, or an error on failure.
	async fn authenticate(
		&self,
		connection: &Arc<WebSocketConnection>,
		credentials: &str,
	) -> AuthResult<Box<dyn AuthUser>>;
}

/// Token-based WebSocket authenticator
///
/// # Examples
///
/// ```
/// use reinhardt_websockets::auth::{TokenAuthenticator, WebSocketAuthenticator, SimpleAuthUser};
/// use reinhardt_websockets::WebSocketConnection;
/// use tokio::sync::mpsc;
/// use std::sync::Arc;
///
/// # tokio_test::block_on(async {
/// let authenticator = TokenAuthenticator::new(vec![
///     ("valid_token".to_string(), SimpleAuthUser::new(
///         "user_1".to_string(),
///         "alice".to_string(),
///         vec!["chat.read".to_string()],
///     )),
/// ]);
///
/// let (tx, _rx) = mpsc::unbounded_channel();
/// let conn = Arc::new(WebSocketConnection::new("conn_1".to_string(), tx));
///
/// let user = authenticator.authenticate(&conn, "valid_token").await.unwrap();
/// assert_eq!(user.username(), "alice");
/// # });
/// ```
pub struct TokenAuthenticator {
	tokens: std::collections::HashMap<String, SimpleAuthUser>,
}

impl TokenAuthenticator {
	/// Create a new token authenticator with predefined tokens
	pub fn new(tokens: Vec<(String, SimpleAuthUser)>) -> Self {
		Self {
			tokens: tokens.into_iter().collect(),
		}
	}

	/// Add a token to the authenticator
	pub fn add_token(&mut self, token: String, user: SimpleAuthUser) {
		self.tokens.insert(token, user);
	}

	/// Remove a token from the authenticator
	pub fn remove_token(&mut self, token: &str) -> Option<SimpleAuthUser> {
		self.tokens.remove(token)
	}
}

#[async_trait]
impl WebSocketAuthenticator for TokenAuthenticator {
	async fn authenticate(
		&self,
		_connection: &Arc<WebSocketConnection>,
		credentials: &str,
	) -> AuthResult<Box<dyn AuthUser>> {
		self.tokens
			.get(credentials)
			.map(|user| Box::new(user.clone()) as Box<dyn AuthUser>)
			.ok_or(AuthError::InvalidCredentials)
	}
}

/// Authorization policy for WebSocket messages
#[async_trait]
pub trait AuthorizationPolicy: Send + Sync {
	/// Check if a user is authorized to perform an action
	///
	/// # Arguments
	///
	/// * `user` - The authenticated user
	/// * `action` - The action to authorize (e.g., "send_message", "join_room")
	/// * `resource` - Optional resource identifier (e.g., room ID)
	///
	/// # Returns
	///
	/// Returns `Ok(())` if authorized, or an error if denied.
	async fn authorize(
		&self,
		user: &dyn AuthUser,
		action: &str,
		resource: Option<&str>,
	) -> AuthResult<()>;
}

/// Permission-based authorization policy
///
/// # Examples
///
/// ```
/// use reinhardt_websockets::auth::{
///     PermissionBasedPolicy, AuthorizationPolicy, SimpleAuthUser
/// };
///
/// # tokio_test::block_on(async {
/// let policy = PermissionBasedPolicy::new(vec![
///     ("send_message".to_string(), "chat.write".to_string()),
///     ("delete_message".to_string(), "chat.admin".to_string()),
/// ]);
///
/// let user = SimpleAuthUser::new(
///     "user_1".to_string(),
///     "alice".to_string(),
///     vec!["chat.write".to_string()],
/// );
///
/// // User can send messages
/// assert!(policy.authorize(&user, "send_message", None).await.is_ok());
///
/// // User cannot delete messages (lacks chat.admin permission)
/// assert!(policy.authorize(&user, "delete_message", None).await.is_err());
/// # });
/// ```
pub struct PermissionBasedPolicy {
	action_permissions: std::collections::HashMap<String, String>,
}

impl PermissionBasedPolicy {
	/// Create a new permission-based policy
	pub fn new(action_permissions: Vec<(String, String)>) -> Self {
		Self {
			action_permissions: action_permissions.into_iter().collect(),
		}
	}

	/// Add an action-permission mapping
	pub fn add_permission(&mut self, action: String, permission: String) {
		self.action_permissions.insert(action, permission);
	}
}

#[async_trait]
impl AuthorizationPolicy for PermissionBasedPolicy {
	async fn authorize(
		&self,
		user: &dyn AuthUser,
		action: &str,
		_resource: Option<&str>,
	) -> AuthResult<()> {
		let required_permission = self
			.action_permissions
			.get(action)
			.ok_or_else(|| AuthError::AuthorizationDenied(format!("Unknown action: {}", action)))?;

		if user.has_permission(required_permission) {
			Ok(())
		} else {
			Err(AuthError::AuthorizationDenied(format!(
				"Missing permission: {}",
				required_permission
			)))
		}
	}
}

/// Authenticated WebSocket connection wrapper
///
/// # Examples
///
/// ```
/// use reinhardt_websockets::auth::{AuthenticatedConnection, SimpleAuthUser};
/// use reinhardt_websockets::WebSocketConnection;
/// use tokio::sync::mpsc;
/// use std::sync::Arc;
///
/// let (tx, _rx) = mpsc::unbounded_channel();
/// let conn = Arc::new(WebSocketConnection::new("conn_1".to_string(), tx));
/// let user = SimpleAuthUser::new(
///     "user_1".to_string(),
///     "alice".to_string(),
///     vec!["chat.read".to_string()],
/// );
///
/// let auth_conn = AuthenticatedConnection::new(conn, Box::new(user));
/// assert_eq!(auth_conn.user().username(), "alice");
/// ```
pub struct AuthenticatedConnection {
	connection: Arc<WebSocketConnection>,
	user: Box<dyn AuthUser>,
}

impl AuthenticatedConnection {
	/// Create a new authenticated connection
	pub fn new(connection: Arc<WebSocketConnection>, user: Box<dyn AuthUser>) -> Self {
		Self { connection, user }
	}

	/// Get the underlying WebSocket connection
	pub fn connection(&self) -> &Arc<WebSocketConnection> {
		&self.connection
	}

	/// Get the authenticated user
	pub fn user(&self) -> &dyn AuthUser {
		self.user.as_ref()
	}

	/// Send a message with authorization check
	pub async fn send_with_auth<P: AuthorizationPolicy>(
		&self,
		message: crate::connection::Message,
		policy: &P,
	) -> WebSocketResult<()> {
		policy
			.authorize(self.user.as_ref(), "send_message", None)
			.await
			.map_err(|_| WebSocketError::Protocol("authorization failed".to_string()))?;

		self.connection.send(message).await
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::connection::Message;
	use tokio::sync::mpsc;

	#[test]
	fn test_simple_auth_user() {
		let user = SimpleAuthUser::new(
			"user_123".to_string(),
			"alice".to_string(),
			vec!["read".to_string(), "write".to_string()],
		);

		assert_eq!(user.id(), "user_123");
		assert_eq!(user.username(), "alice");
		assert!(user.is_authenticated());
		assert!(user.has_permission("read"));
		assert!(user.has_permission("write"));
		assert!(!user.has_permission("admin"));
	}

	#[tokio::test]
	async fn test_token_authenticator_valid() {
		let user = SimpleAuthUser::new(
			"user_1".to_string(),
			"alice".to_string(),
			vec!["chat.read".to_string()],
		);

		let authenticator = TokenAuthenticator::new(vec![("token123".to_string(), user)]);

		let (tx, _rx) = mpsc::unbounded_channel();
		let conn = Arc::new(WebSocketConnection::new("conn_1".to_string(), tx));

		let auth_user = authenticator.authenticate(&conn, "token123").await.unwrap();
		assert_eq!(auth_user.username(), "alice");
	}

	#[tokio::test]
	async fn test_token_authenticator_invalid() {
		let authenticator = TokenAuthenticator::new(vec![]);

		let (tx, _rx) = mpsc::unbounded_channel();
		let conn = Arc::new(WebSocketConnection::new("conn_1".to_string(), tx));

		let result = authenticator.authenticate(&conn, "invalid_token").await;
		assert!(result.is_err());
		assert!(matches!(result.unwrap_err(), AuthError::InvalidCredentials));
	}

	#[tokio::test]
	async fn test_permission_based_policy_authorized() {
		let policy = PermissionBasedPolicy::new(vec![(
			"send_message".to_string(),
			"chat.write".to_string(),
		)]);

		let user = SimpleAuthUser::new(
			"user_1".to_string(),
			"alice".to_string(),
			vec!["chat.write".to_string()],
		);

		let result = policy.authorize(&user, "send_message", None).await;
		assert!(result.is_ok());
	}

	#[tokio::test]
	async fn test_permission_based_policy_denied() {
		let policy = PermissionBasedPolicy::new(vec![(
			"delete_message".to_string(),
			"chat.admin".to_string(),
		)]);

		let user = SimpleAuthUser::new(
			"user_1".to_string(),
			"alice".to_string(),
			vec!["chat.write".to_string()],
		);

		let result = policy.authorize(&user, "delete_message", None).await;
		assert!(result.is_err());
		assert!(matches!(
			result.unwrap_err(),
			AuthError::AuthorizationDenied(_)
		));
	}

	#[tokio::test]
	async fn test_authenticated_connection_send_with_auth() {
		let policy = PermissionBasedPolicy::new(vec![(
			"send_message".to_string(),
			"chat.write".to_string(),
		)]);

		let user = SimpleAuthUser::new(
			"user_1".to_string(),
			"alice".to_string(),
			vec!["chat.write".to_string()],
		);

		let (tx, mut rx) = mpsc::unbounded_channel();
		let conn = Arc::new(WebSocketConnection::new("conn_1".to_string(), tx));
		let auth_conn = AuthenticatedConnection::new(conn, Box::new(user));

		let msg = Message::text("Hello".to_string());
		auth_conn.send_with_auth(msg, &policy).await.unwrap();

		assert!(matches!(rx.try_recv(), Ok(Message::Text { .. })));
	}

	#[tokio::test]
	async fn test_authenticated_connection_send_with_auth_denied() {
		let policy = PermissionBasedPolicy::new(vec![(
			"send_message".to_string(),
			"chat.admin".to_string(),
		)]);

		let user = SimpleAuthUser::new(
			"user_1".to_string(),
			"alice".to_string(),
			vec!["chat.write".to_string()],
		);

		let (tx, _rx) = mpsc::unbounded_channel();
		let conn = Arc::new(WebSocketConnection::new("conn_1".to_string(), tx));
		let auth_conn = AuthenticatedConnection::new(conn, Box::new(user));

		let msg = Message::text("Hello".to_string());
		let result = auth_conn.send_with_auth(msg, &policy).await;

		assert!(result.is_err());
	}
}
