//! Authentication handlers for login, logout, and user management
//!
//! Provides ready-to-use handlers for common authentication workflows

use crate::AuthenticationBackend;
use crate::User;
use crate::session::{SESSION_KEY_USER_ID, Session, SessionId, SessionStore};
use async_trait::async_trait;
use reinhardt_core::exception::Result;
use reinhardt_http::Handler;
use reinhardt_http::{Request, Response};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Login credentials
///
/// # Examples
///
/// ```
/// use reinhardt_auth::handlers::LoginCredentials;
///
/// let credentials = LoginCredentials {
///     username: "user".to_string(),
///     password: "password".to_string(),
/// };
///
/// assert_eq!(credentials.username, "user");
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginCredentials {
	pub username: String,
	pub password: String,
}

/// Session cookie name
pub const SESSION_COOKIE_NAME: &str = "sessionid";

/// Login handler
///
/// Handles user login with username/password authentication
///
/// # Examples
///
/// ```rust,ignore
/// use reinhardt_auth::handlers::LoginHandler;
/// use reinhardt_auth::session::InMemorySessionStore;
/// use reinhardt_auth::core::backend::AuthenticationBackend;
/// use std::sync::Arc;
///
/// let session_store = Arc::new(InMemorySessionStore::new());
/// // AuthenticationBackend is a trait - use a concrete implementation
/// let auth_backend = Arc::new(YourAuthBackendImpl::new());
/// let handler = LoginHandler::new(session_store, auth_backend);
/// ```
pub struct LoginHandler<S: SessionStore, A: AuthenticationBackend> {
	session_store: Arc<S>,
	auth_backend: Arc<A>,
}

impl<S: SessionStore, A: AuthenticationBackend> LoginHandler<S, A> {
	/// Create a new login handler
	///
	/// # Examples
	///
	/// ```ignore
	/// use reinhardt_auth::handlers::LoginHandler;
	/// use reinhardt_auth::session::InMemorySessionStore;
	/// use reinhardt_auth::backend::AuthBackend;
	/// use std::sync::Arc;
	///
	/// let session_store = Arc::new(InMemorySessionStore::new());
	/// // AuthBackend is a trait - use a concrete implementation
	/// let auth_backend = Arc::new(YourAuthBackendImpl::new());
	/// let handler = LoginHandler::new(session_store, auth_backend);
	/// ```
	pub fn new(session_store: Arc<S>, auth_backend: Arc<A>) -> Self {
		Self {
			session_store,
			auth_backend,
		}
	}

	async fn perform_login(&self, user: Box<dyn User>) -> Result<(SessionId, String)> {
		let session_id = self.session_store.create_session_id();
		let mut session = Session::new();
		session.set(SESSION_KEY_USER_ID, serde_json::json!(user.id()));

		self.session_store.save(&session_id, &session).await;

		let cookie_str = format!(
			"{}={}; HttpOnly; Secure; Path=/; SameSite=Lax",
			SESSION_COOKIE_NAME, session_id
		);

		Ok((session_id, cookie_str))
	}
}

#[async_trait]
impl<S: SessionStore + 'static, A: AuthenticationBackend + 'static> Handler for LoginHandler<S, A> {
	async fn handle(&self, request: Request) -> Result<Response> {
		if let Some(user) = self
			.auth_backend
			.authenticate(&request)
			.await
			.ok()
			.flatten()
		{
			let (_session_id, cookie_str) = self.perform_login(user).await?;

			Ok(Response::ok()
				.with_header("Set-Cookie", &cookie_str)
				.with_json(&serde_json::json!({
					"success": true,
					"message": "Login successful"
				}))?)
		} else {
			Ok(
				Response::new(Response::unauthorized().status).with_json(&serde_json::json!({
					"success": false,
					"message": "Invalid credentials"
				}))?,
			)
		}
	}
}

/// Logout handler
///
/// Handles user logout by clearing the session
///
/// # Examples
///
/// ```
/// use reinhardt_auth::handlers::LogoutHandler;
/// use reinhardt_auth::session::InMemorySessionStore;
/// use std::sync::Arc;
///
/// let session_store = Arc::new(InMemorySessionStore::new());
/// let handler = LogoutHandler::new(session_store);
/// ```
pub struct LogoutHandler<S: SessionStore> {
	session_store: Arc<S>,
}

impl<S: SessionStore> LogoutHandler<S> {
	/// Create a new logout handler
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_auth::handlers::LogoutHandler;
	/// use reinhardt_auth::session::InMemorySessionStore;
	/// use std::sync::Arc;
	///
	/// let session_store = Arc::new(InMemorySessionStore::new());
	/// let handler = LogoutHandler::new(session_store);
	/// ```
	pub fn new(session_store: Arc<S>) -> Self {
		Self { session_store }
	}

	fn extract_session_id(&self, request: &Request) -> Option<SessionId> {
		request
			.headers
			.get("cookie")
			.and_then(|v| v.to_str().ok())
			.and_then(|cookies| {
				cookies.split(';').find_map(|cookie| {
					let mut parts = cookie.trim().split('=');
					if parts.next()? == SESSION_COOKIE_NAME {
						Some(parts.next()?.to_string())
					} else {
						None
					}
				})
			})
	}
}

#[async_trait]
impl<S: SessionStore + 'static> Handler for LogoutHandler<S> {
	async fn handle(&self, request: Request) -> Result<Response> {
		if let Some(session_id) = self.extract_session_id(&request) {
			self.session_store.delete(&session_id).await;
		}

		let cookie_str = format!("{}=; HttpOnly; Path=/; Max-Age=0", SESSION_COOKIE_NAME);

		Ok(Response::ok()
			.with_header("Set-Cookie", &cookie_str)
			.with_json(&serde_json::json!({
				"success": true,
				"message": "Logout successful"
			}))?)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::AuthenticationError;
	use crate::SimpleUser;
	use crate::session::InMemorySessionStore;
	use bytes::Bytes;
	use hyper::{HeaderMap, Method};
	use uuid::Uuid;

	struct TestAuthBackend {
		test_user: Option<SimpleUser>,
	}

	#[async_trait]
	impl AuthenticationBackend for TestAuthBackend {
		async fn authenticate(
			&self,
			_request: &Request,
		) -> std::result::Result<Option<Box<dyn User>>, AuthenticationError> {
			if let Some(user) = &self.test_user {
				Ok(Some(Box::new(user.clone())))
			} else {
				Ok(None)
			}
		}

		async fn get_user(
			&self,
			_user_id: &str,
		) -> std::result::Result<Option<Box<dyn User>>, AuthenticationError> {
			if let Some(user) = &self.test_user {
				Ok(Some(Box::new(user.clone())))
			} else {
				Ok(None)
			}
		}
	}

	#[tokio::test]
	async fn test_login_handler_success() {
		let session_store = Arc::new(InMemorySessionStore::new());
		let test_user = SimpleUser {
			id: Uuid::new_v4(),
			username: "testuser".to_string(),
			email: "test@example.com".to_string(),
			is_active: true,
			is_admin: false,
			is_staff: false,
			is_superuser: false,
		};
		let auth_backend = Arc::new(TestAuthBackend {
			test_user: Some(test_user),
		});

		let handler = LoginHandler::new(session_store, auth_backend);
		let request = Request::builder()
			.method(Method::POST)
			.uri("/login")
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = handler.handle(request).await.unwrap();
		assert_eq!(response.status, reinhardt_http::Response::ok().status);
		assert!(response.headers.contains_key("set-cookie"));

		// Verify Secure flag is present in session cookie
		let cookie_value = response
			.headers
			.get("set-cookie")
			.unwrap()
			.to_str()
			.unwrap();
		assert!(
			cookie_value.contains("Secure"),
			"Session cookie must include Secure flag"
		);
	}

	#[tokio::test]
	async fn test_login_handler_failure() {
		let session_store = Arc::new(InMemorySessionStore::new());
		let auth_backend = Arc::new(TestAuthBackend { test_user: None });

		let handler = LoginHandler::new(session_store, auth_backend);
		let request = Request::builder()
			.method(Method::POST)
			.uri("/login")
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = handler.handle(request).await.unwrap();
		assert_eq!(
			response.status,
			reinhardt_http::Response::unauthorized().status
		);
	}

	#[tokio::test]
	async fn test_logout_handler() {
		let session_store = Arc::new(InMemorySessionStore::new());
		let session_id = session_store.create_session_id();
		let mut session = Session::new();
		session.set(SESSION_KEY_USER_ID, serde_json::json!("user123"));
		session_store.save(&session_id, &session).await;

		let handler = LogoutHandler::new(session_store.clone());

		let mut headers = HeaderMap::new();
		headers.insert(
			"cookie",
			format!("{}={}", SESSION_COOKIE_NAME, session_id)
				.parse()
				.unwrap(),
		);

		let request = Request::builder()
			.method(Method::POST)
			.uri("/logout")
			.headers(headers)
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = handler.handle(request).await.unwrap();
		assert_eq!(response.status, reinhardt_http::Response::ok().status);
		assert!(response.headers.contains_key("set-cookie"));

		assert!(session_store.load(&session_id).await.is_none());
	}

	#[tokio::test]
	async fn test_logout_handler_no_session() {
		let session_store = Arc::new(InMemorySessionStore::new());
		let handler = LogoutHandler::new(session_store);

		let request = Request::builder()
			.method(Method::POST)
			.uri("/logout")
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = handler.handle(request).await.unwrap();
		assert_eq!(response.status, reinhardt_http::Response::ok().status);
	}
}
