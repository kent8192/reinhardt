// The `User` trait is deprecated in favour of the new `#[model]`-based user macro system.
// Downstream crates still reference it during the transition period.
#![allow(deprecated)]

#[cfg(feature = "sessions")]
use async_trait::async_trait;
#[cfg(feature = "sessions")]
use std::sync::Arc;

#[cfg(feature = "sessions")]
use reinhardt_auth::{AuthenticationBackend, User};
#[cfg(feature = "sessions")]
use reinhardt_http::{
	AuthState, Handler, IsActive, IsAdmin, IsAuthenticated, Middleware, Request, Response, Result,
};

/// Default HTTP header name for the remote user.
#[cfg(feature = "sessions")]
pub const REMOTE_USER_HEADER: &str = "REMOTE_USER";

/// Remote user authentication middleware.
///
/// Authenticates users based on the `REMOTE_USER` header set by a
/// reverse proxy (Apache, Nginx, etc.). This is the Rust equivalent
/// of Django's [`RemoteUserMiddleware`](https://docs.djangoproject.com/en/5.1/ref/middleware/#django.contrib.auth.middleware.RemoteUserMiddleware).
///
/// When the configured header is present, the middleware uses the
/// provided [`AuthenticationBackend`] to look up the user. When the
/// header is absent, the request proceeds as anonymous, clearing any
/// previously authenticated state.
///
/// # Security Warning
///
/// This middleware should **only** be used behind a trusted reverse
/// proxy that controls the `REMOTE_USER` header. If the proxy does
/// not strip or override this header from client requests, an
/// attacker can impersonate any user by sending a crafted header.
///
/// # Examples
///
/// ```rust,no_run
/// use std::sync::Arc;
/// use reinhardt_middleware::RemoteUserMiddleware;
/// use reinhardt_http::MiddlewareChain;
/// # use reinhardt_http::{Handler, Request, Response, Result};
/// # use reinhardt_auth::{AuthenticationBackend, AuthenticationError, User};
/// # use async_trait::async_trait;
/// # struct MyHandler;
/// # #[async_trait]
/// # impl Handler for MyHandler {
/// #     async fn handle(&self, _request: Request) -> Result<Response> {
/// #         Ok(Response::ok())
/// #     }
/// # }
/// # struct MyAuthBackend;
/// # #[async_trait]
/// # impl AuthenticationBackend for MyAuthBackend {
/// #     async fn authenticate(&self, _req: &Request) -> std::result::Result<Option<Box<dyn User>>, AuthenticationError> { Ok(None) }
/// #     async fn get_user(&self, _uid: &str) -> std::result::Result<Option<Box<dyn User>>, AuthenticationError> { Ok(None) }
/// # }
/// # let handler = Arc::new(MyHandler);
///
/// let auth_backend = Arc::new(MyAuthBackend);
/// let middleware = RemoteUserMiddleware::new(auth_backend);
///
/// let app = MiddlewareChain::new(handler)
///     .with_middleware(Arc::new(middleware));
/// ```
#[cfg(feature = "sessions")]
pub struct RemoteUserMiddleware<A: AuthenticationBackend> {
	auth_backend: Arc<A>,
	header_name: String,
	/// When `true`, absence of the remote user header forces logout
	/// (anonymous state). When `false`, the existing session auth
	/// is preserved even without the header.
	force_logout_if_no_header: bool,
}

#[cfg(feature = "sessions")]
impl<A: AuthenticationBackend> RemoteUserMiddleware<A> {
	/// Creates a new remote user middleware with the default `REMOTE_USER` header.
	///
	/// When the header is absent, the request proceeds as anonymous.
	///
	/// # Arguments
	///
	/// * `auth_backend` - Authentication backend for user lookup
	pub fn new(auth_backend: Arc<A>) -> Self {
		Self {
			auth_backend,
			header_name: REMOTE_USER_HEADER.to_string(),
			force_logout_if_no_header: true,
		}
	}

	/// Sets a custom header name for remote user identification.
	///
	/// # Arguments
	///
	/// * `header_name` - The HTTP header name containing the remote username
	///
	/// # Examples
	///
	/// ```rust,no_run
	/// # use std::sync::Arc;
	/// # use reinhardt_middleware::RemoteUserMiddleware;
	/// # use reinhardt_auth::{AuthenticationBackend, AuthenticationError, User};
	/// # use reinhardt_http::Request;
	/// # use async_trait::async_trait;
	/// # struct MyAuth;
	/// # #[async_trait]
	/// # impl AuthenticationBackend for MyAuth {
	/// #     async fn authenticate(&self, _req: &Request) -> std::result::Result<Option<Box<dyn User>>, AuthenticationError> { Ok(None) }
	/// #     async fn get_user(&self, _uid: &str) -> std::result::Result<Option<Box<dyn User>>, AuthenticationError> { Ok(None) }
	/// # }
	///
	/// let backend = Arc::new(MyAuth);
	/// let middleware = RemoteUserMiddleware::new(backend)
	///     .with_header("X-Forwarded-User");
	/// ```
	pub fn with_header(mut self, header_name: &str) -> Self {
		self.header_name = header_name.to_string();
		self
	}

	/// Looks up a user by username via the authentication backend.
	async fn get_user_by_name(&self, username: &str) -> Option<Box<dyn User>> {
		self.auth_backend.get_user(username).await.ok().flatten()
	}

	/// Inserts user information into request extensions.
	fn insert_user_extensions(request: &Request, user: &dyn User) {
		let is_authenticated = user.is_authenticated();
		let is_admin = user.is_admin();
		let is_active = user.is_active();
		let user_id = user.id();

		// Insert individual values for backward compatibility
		request.extensions.insert(user_id.clone());
		request.extensions.insert(IsAuthenticated(is_authenticated));
		request.extensions.insert(IsAdmin(is_admin));
		request.extensions.insert(IsActive(is_active));

		// Insert AuthState object
		let auth_state = if is_authenticated {
			AuthState::authenticated(user_id, is_admin, is_active)
		} else {
			AuthState::anonymous()
		};
		request.extensions.insert(auth_state);
	}
}

#[cfg(feature = "sessions")]
#[async_trait]
impl<A: AuthenticationBackend + 'static> Middleware for RemoteUserMiddleware<A> {
	async fn process(&self, request: Request, next: Arc<dyn Handler>) -> Result<Response> {
		let remote_user = request
			.headers
			.get(&self.header_name)
			.and_then(|v| v.to_str().ok())
			.map(|s| s.to_string());

		if let Some(username) = remote_user {
			if let Some(user) = self.get_user_by_name(&username).await {
				Self::insert_user_extensions(&request, user.as_ref());
			} else {
				request.extensions.insert(AuthState::anonymous());
			}
		} else if self.force_logout_if_no_header {
			// No header and force logout: set anonymous
			request.extensions.insert(AuthState::anonymous());
		}
		// If !force_logout_if_no_header and no header: don't touch
		// extensions, preserve existing auth state from upstream middleware.

		next.handle(request).await
	}
}

/// Persistent remote user authentication middleware.
///
/// A variant of [`RemoteUserMiddleware`] that preserves the existing
/// session authentication when the remote user header is absent. This
/// is the Rust equivalent of Django's
/// [`PersistentRemoteUserMiddleware`](https://docs.djangoproject.com/en/5.1/ref/middleware/#django.contrib.auth.middleware.PersistentRemoteUserMiddleware).
///
/// Use this middleware when the reverse proxy may not always set the
/// header (e.g., only on initial login pages) and you want to keep
/// the user authenticated via session for subsequent requests.
///
/// # Examples
///
/// ```rust,no_run
/// use std::sync::Arc;
/// use reinhardt_middleware::PersistentRemoteUserMiddleware;
/// use reinhardt_http::MiddlewareChain;
/// # use reinhardt_http::{Handler, Request, Response, Result};
/// # use reinhardt_auth::{AuthenticationBackend, AuthenticationError, User};
/// # use async_trait::async_trait;
/// # struct MyHandler;
/// # #[async_trait]
/// # impl Handler for MyHandler {
/// #     async fn handle(&self, _request: Request) -> Result<Response> {
/// #         Ok(Response::ok())
/// #     }
/// # }
/// # struct MyAuthBackend;
/// # #[async_trait]
/// # impl AuthenticationBackend for MyAuthBackend {
/// #     async fn authenticate(&self, _req: &Request) -> std::result::Result<Option<Box<dyn User>>, AuthenticationError> { Ok(None) }
/// #     async fn get_user(&self, _uid: &str) -> std::result::Result<Option<Box<dyn User>>, AuthenticationError> { Ok(None) }
/// # }
/// # let handler = Arc::new(MyHandler);
///
/// let auth_backend = Arc::new(MyAuthBackend);
/// let middleware = PersistentRemoteUserMiddleware::new(auth_backend);
///
/// let app = MiddlewareChain::new(handler)
///     .with_middleware(Arc::new(middleware));
/// ```
#[cfg(feature = "sessions")]
pub struct PersistentRemoteUserMiddleware<A: AuthenticationBackend> {
	inner: RemoteUserMiddleware<A>,
}

#[cfg(feature = "sessions")]
impl<A: AuthenticationBackend> PersistentRemoteUserMiddleware<A> {
	/// Creates a new persistent remote user middleware.
	///
	/// Unlike [`RemoteUserMiddleware`], this middleware does not clear
	/// authentication when the header is absent.
	///
	/// # Arguments
	///
	/// * `auth_backend` - Authentication backend for user lookup
	pub fn new(auth_backend: Arc<A>) -> Self {
		Self {
			inner: RemoteUserMiddleware {
				auth_backend,
				header_name: REMOTE_USER_HEADER.to_string(),
				force_logout_if_no_header: false,
			},
		}
	}

	/// Sets a custom header name for remote user identification.
	pub fn with_header(mut self, header_name: &str) -> Self {
		self.inner.header_name = header_name.to_string();
		self
	}
}

#[cfg(feature = "sessions")]
#[async_trait]
impl<A: AuthenticationBackend + 'static> Middleware for PersistentRemoteUserMiddleware<A> {
	async fn process(&self, request: Request, next: Arc<dyn Handler>) -> Result<Response> {
		self.inner.process(request, next).await
	}
}

#[cfg(all(test, feature = "sessions"))]
mod tests {
	use super::*;
	use bytes::Bytes;
	use hyper::{HeaderMap, Method, Version};
	use reinhardt_auth::{AuthenticationError, SimpleUser};
	use reinhardt_http::{AuthState, Handler, Middleware, Request, Response};
	use rstest::rstest;
	use uuid::Uuid;

	struct TestHandler;

	#[async_trait::async_trait]
	impl Handler for TestHandler {
		async fn handle(&self, request: Request) -> Result<Response> {
			let auth_state = request.extensions.get::<AuthState>();
			Ok(Response::ok().with_json(&serde_json::json!({
				"is_authenticated": auth_state.as_ref().map(|s| s.is_authenticated()).unwrap_or(false),
				"user_id": auth_state.as_ref().map(|s| s.user_id().to_string()).unwrap_or_default(),
			}))?)
		}
	}

	struct TestAuthBackend {
		user: Option<SimpleUser>,
	}

	#[async_trait::async_trait]
	impl AuthenticationBackend for TestAuthBackend {
		async fn authenticate(
			&self,
			_request: &Request,
		) -> std::result::Result<Option<Box<dyn User>>, AuthenticationError> {
			Ok(self
				.user
				.as_ref()
				.map(|u| Box::new(u.clone()) as Box<dyn User>))
		}

		async fn get_user(
			&self,
			_user_id: &str,
		) -> std::result::Result<Option<Box<dyn User>>, AuthenticationError> {
			Ok(self
				.user
				.as_ref()
				.map(|u| Box::new(u.clone()) as Box<dyn User>))
		}
	}

	fn test_user() -> SimpleUser {
		SimpleUser {
			id: Uuid::now_v7(),
			username: "proxy-user".to_string(),
			email: "proxy@example.com".to_string(),
			is_active: true,
			is_admin: false,
			is_staff: false,
			is_superuser: false,
		}
	}

	fn create_request_with_header(name: &'static str, value: &str) -> Request {
		let mut headers = HeaderMap::new();
		headers.insert(name, value.parse().unwrap());
		Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(headers)
			.body(Bytes::new())
			.build()
			.unwrap()
	}

	fn create_request_without_header() -> Request {
		Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap()
	}

	#[rstest]
	#[tokio::test]
	async fn test_remote_user_header_authenticates_user() {
		// Arrange
		let user = test_user();
		let expected_id = user.id.to_string();
		let auth_backend = Arc::new(TestAuthBackend { user: Some(user) });
		let middleware = RemoteUserMiddleware::new(auth_backend);
		let handler = Arc::new(TestHandler);
		let request = create_request_with_header("REMOTE_USER", "proxy-user");

		// Act
		let response = middleware.process(request, handler).await.unwrap();

		// Assert
		let body_str = String::from_utf8(response.body.to_vec()).unwrap();
		let body: serde_json::Value = serde_json::from_str(&body_str).unwrap();
		assert_eq!(body["is_authenticated"], true);
		assert_eq!(body["user_id"], expected_id);
	}

	#[rstest]
	#[tokio::test]
	async fn test_missing_header_produces_anonymous() {
		// Arrange
		let auth_backend = Arc::new(TestAuthBackend {
			user: Some(test_user()),
		});
		let middleware = RemoteUserMiddleware::new(auth_backend);
		let handler = Arc::new(TestHandler);
		let request = create_request_without_header();

		// Act
		let response = middleware.process(request, handler).await.unwrap();

		// Assert
		let body_str = String::from_utf8(response.body.to_vec()).unwrap();
		let body: serde_json::Value = serde_json::from_str(&body_str).unwrap();
		assert_eq!(body["is_authenticated"], false);
	}

	#[rstest]
	#[tokio::test]
	async fn test_unknown_user_produces_anonymous() {
		// Arrange
		let auth_backend = Arc::new(TestAuthBackend { user: None });
		let middleware = RemoteUserMiddleware::new(auth_backend);
		let handler = Arc::new(TestHandler);
		let request = create_request_with_header("REMOTE_USER", "unknown-user");

		// Act
		let response = middleware.process(request, handler).await.unwrap();

		// Assert
		let body_str = String::from_utf8(response.body.to_vec()).unwrap();
		let body: serde_json::Value = serde_json::from_str(&body_str).unwrap();
		assert_eq!(body["is_authenticated"], false);
	}

	#[rstest]
	#[tokio::test]
	async fn test_custom_header_name() {
		// Arrange
		let user = test_user();
		let expected_id = user.id.to_string();
		let auth_backend = Arc::new(TestAuthBackend { user: Some(user) });
		let middleware = RemoteUserMiddleware::new(auth_backend).with_header("X-Forwarded-User");
		let handler = Arc::new(TestHandler);
		let request = create_request_with_header("X-Forwarded-User", "proxy-user");

		// Act
		let response = middleware.process(request, handler).await.unwrap();

		// Assert
		let body_str = String::from_utf8(response.body.to_vec()).unwrap();
		let body: serde_json::Value = serde_json::from_str(&body_str).unwrap();
		assert_eq!(body["is_authenticated"], true);
		assert_eq!(body["user_id"], expected_id);
	}

	#[rstest]
	#[tokio::test]
	async fn test_persistent_middleware_preserves_auth_when_no_header() {
		// Arrange
		let auth_backend = Arc::new(TestAuthBackend {
			user: Some(test_user()),
		});
		let middleware = PersistentRemoteUserMiddleware::new(auth_backend);
		let handler = Arc::new(TestHandler);

		// Pre-insert an authenticated state to simulate upstream auth
		let request = create_request_without_header();
		request
			.extensions
			.insert(AuthState::authenticated("existing-user", false, true));

		// Act
		let response = middleware.process(request, handler).await.unwrap();

		// Assert - the existing auth state should be preserved
		let body_str = String::from_utf8(response.body.to_vec()).unwrap();
		let body: serde_json::Value = serde_json::from_str(&body_str).unwrap();
		assert_eq!(body["is_authenticated"], true);
		assert_eq!(body["user_id"], "existing-user");
	}

	#[rstest]
	#[tokio::test]
	async fn test_persistent_middleware_authenticates_when_header_present() {
		// Arrange
		let user = test_user();
		let expected_id = user.id.to_string();
		let auth_backend = Arc::new(TestAuthBackend { user: Some(user) });
		let middleware = PersistentRemoteUserMiddleware::new(auth_backend);
		let handler = Arc::new(TestHandler);
		let request = create_request_with_header("REMOTE_USER", "proxy-user");

		// Act
		let response = middleware.process(request, handler).await.unwrap();

		// Assert
		let body_str = String::from_utf8(response.body.to_vec()).unwrap();
		let body: serde_json::Value = serde_json::from_str(&body_str).unwrap();
		assert_eq!(body["is_authenticated"], true);
		assert_eq!(body["user_id"], expected_id);
	}
}
