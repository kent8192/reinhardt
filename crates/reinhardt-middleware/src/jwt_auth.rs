#[cfg(feature = "auth-jwt")]
use async_trait::async_trait;
#[cfg(feature = "auth-jwt")]
use std::sync::Arc;

#[cfg(feature = "auth-jwt")]
use reinhardt_auth::jwt::JwtAuth;
#[cfg(feature = "auth-jwt")]
use reinhardt_http::{
	AuthState, Handler, IsActive, IsAdmin, IsAuthenticated, Middleware, Request, Response, Result,
};

/// JWT authentication middleware for stateless token-based auth.
///
/// Extracts Bearer tokens from the `Authorization` header, verifies them
/// using [`reinhardt_auth::jwt::JwtAuth`], and inserts an `AuthState`
/// into request extensions.
///
/// This middleware uses **best-effort authentication**: valid tokens produce
/// `AuthState::authenticated()`, while missing or invalid tokens produce
/// `AuthState::anonymous()`. Requests are never rejected by this middleware —
/// authorization is delegated to endpoint-level guards
/// (`Guard<P>`, `Public`).
///
/// This is the JWT counterpart to the session-based [`AuthenticationMiddleware`](crate::AuthenticationMiddleware).
///
/// # Examples
///
/// ```rust,no_run
/// use std::sync::Arc;
/// use reinhardt_middleware::JwtAuthMiddleware;
/// use reinhardt_http::MiddlewareChain;
/// # use reinhardt_http::{Handler, Request, Response, Result};
/// # use async_trait::async_trait;
/// # struct MyHandler;
/// # #[async_trait]
/// # impl Handler for MyHandler {
/// #     async fn handle(&self, _request: Request) -> Result<Response> {
/// #         Ok(Response::ok())
/// #     }
/// # }
/// # let handler = Arc::new(MyHandler);
///
/// // Create middleware with a secret key
/// let middleware = JwtAuthMiddleware::from_secret(b"my-secret-key");
///
/// // Build middleware chain
/// let app = MiddlewareChain::new(handler)
///     .with_middleware(Arc::new(middleware));
/// ```
///
/// Using a pre-built `JwtAuth` instance:
///
/// ```rust,no_run
/// use reinhardt_auth::jwt::JwtAuth;
/// use reinhardt_middleware::JwtAuthMiddleware;
///
/// let jwt_auth = JwtAuth::new(b"my-secret-key");
/// let middleware = JwtAuthMiddleware::new(jwt_auth);
/// ```
#[cfg(feature = "auth-jwt")]
pub struct JwtAuthMiddleware {
	jwt_auth: JwtAuth,
}

#[cfg(feature = "auth-jwt")]
impl JwtAuthMiddleware {
	/// Creates a new JWT authentication middleware with a pre-built
	/// `JwtAuth` instance.
	///
	/// Use this constructor when you need to share a `JwtAuth` instance
	/// or want to configure custom validation settings.
	///
	/// # Arguments
	///
	/// * `jwt_auth` - A configured JWT authentication handler
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_auth::jwt::JwtAuth;
	/// use reinhardt_middleware::JwtAuthMiddleware;
	///
	/// let jwt_auth = JwtAuth::new(b"my-secret-key");
	/// let middleware = JwtAuthMiddleware::new(jwt_auth);
	/// ```
	pub fn new(jwt_auth: JwtAuth) -> Self {
		Self { jwt_auth }
	}

	/// Creates a new JWT authentication middleware from a secret key.
	///
	/// This is a convenience constructor that creates a `JwtAuth`
	/// instance internally. Secret management (environment variables,
	/// config files) is the application's concern.
	///
	/// # Arguments
	///
	/// * `secret` - The secret key used to verify JWT signatures
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_middleware::JwtAuthMiddleware;
	///
	/// let middleware = JwtAuthMiddleware::from_secret(b"my-secret-key");
	/// ```
	pub fn from_secret(secret: &[u8]) -> Self {
		Self {
			jwt_auth: JwtAuth::new(secret),
		}
	}

	/// Extracts the Bearer token from the Authorization header.
	fn extract_bearer_token(request: &Request) -> Option<&str> {
		request
			.headers
			.get("Authorization")
			.and_then(|v| v.to_str().ok())
			.and_then(|s| s.strip_prefix("Bearer "))
	}
}

#[cfg(feature = "auth-jwt")]
#[async_trait]
impl Middleware for JwtAuthMiddleware {
	async fn process(&self, request: Request, next: Arc<dyn Handler>) -> Result<Response> {
		let auth_state = if let Some(token) = Self::extract_bearer_token(&request)
			&& let Ok(claims) = self.jwt_auth.verify_token(token)
		{
			let user_id = claims.sub;
			let is_admin = claims.is_staff || claims.is_superuser;
			let is_active = true;

			// Insert individual values for backward compatibility
			request.extensions.insert(user_id.clone());
			request.extensions.insert(IsAuthenticated(true));
			request.extensions.insert(IsAdmin(is_admin));
			request.extensions.insert(IsActive(is_active));

			AuthState::authenticated(user_id, is_admin, is_active)
		} else {
			AuthState::anonymous()
		};

		request.extensions.insert(auth_state);
		next.handle(request).await
	}
}

#[cfg(all(test, feature = "auth-jwt"))]
mod tests {
	use super::*;
	use bytes::Bytes;
	use hyper::{HeaderMap, Method, Version};
	use reinhardt_auth::jwt::{Claims, JwtAuth};
	use reinhardt_http::{AuthState, Handler, Middleware, Request, Response};
	use rstest::rstest;

	struct TestHandler;

	#[async_trait::async_trait]
	impl Handler for TestHandler {
		async fn handle(&self, request: Request) -> Result<Response> {
			let auth_state = request.extensions.get::<AuthState>();
			Ok(Response::ok().with_json(&serde_json::json!({
				"has_auth_state": auth_state.is_some(),
				"is_authenticated": auth_state.as_ref().map(|s| s.is_authenticated()).unwrap_or(false),
				"user_id": auth_state.as_ref().map(|s| s.user_id().to_string()).unwrap_or_default(),
				"is_admin": auth_state.as_ref().map(|s| s.is_admin()).unwrap_or(false),
				"is_active": auth_state.as_ref().map(|s| s.is_active()).unwrap_or(false),
			}))?)
		}
	}

	fn create_request_with_header(name: &'static str, value: &str) -> Request {
		let mut headers = HeaderMap::new();
		headers.insert(name, value.parse().unwrap());
		Request::builder()
			.method(Method::GET)
			.uri("/api/resource")
			.version(Version::HTTP_11)
			.headers(headers)
			.body(Bytes::new())
			.build()
			.unwrap()
	}

	fn create_request_without_auth() -> Request {
		Request::builder()
			.method(Method::GET)
			.uri("/api/resource")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap()
	}

	#[rstest]
	#[tokio::test]
	async fn test_valid_token_produces_authenticated_state() {
		// Arrange
		let secret = b"test-secret-key-256bit!!";
		let jwt_auth = JwtAuth::new(secret);
		let token = jwt_auth
			.generate_token(
				"550e8400-e29b-41d4-a716-446655440000".to_string(),
				"alice".to_string(),
				false,
				false,
			)
			.unwrap();
		let middleware = JwtAuthMiddleware::from_secret(secret);
		let handler = Arc::new(TestHandler);
		let request = create_request_with_header("Authorization", &format!("Bearer {}", token));

		// Act
		let response = middleware.process(request, handler).await.unwrap();

		// Assert
		let body_str = String::from_utf8(response.body.to_vec()).unwrap();
		let body: serde_json::Value = serde_json::from_str(&body_str).unwrap();
		assert_eq!(body["is_authenticated"], true);
		assert_eq!(body["user_id"], "550e8400-e29b-41d4-a716-446655440000");
		assert_eq!(body["is_admin"], false);
		assert_eq!(body["is_active"], true);
	}

	#[rstest]
	#[tokio::test]
	async fn test_missing_authorization_header_produces_anonymous_state() {
		// Arrange
		let middleware = JwtAuthMiddleware::from_secret(b"test-secret");
		let handler = Arc::new(TestHandler);
		let request = create_request_without_auth();

		// Act
		let response = middleware.process(request, handler).await.unwrap();

		// Assert
		let body_str = String::from_utf8(response.body.to_vec()).unwrap();
		let body: serde_json::Value = serde_json::from_str(&body_str).unwrap();
		assert_eq!(body["is_authenticated"], false);
	}

	#[rstest]
	#[tokio::test]
	async fn test_invalid_token_produces_anonymous_state() {
		// Arrange
		let middleware = JwtAuthMiddleware::from_secret(b"test-secret");
		let handler = Arc::new(TestHandler);
		let request = create_request_with_header("Authorization", "Bearer invalid.token.here");

		// Act
		let response = middleware.process(request, handler).await.unwrap();

		// Assert
		let body_str = String::from_utf8(response.body.to_vec()).unwrap();
		let body: serde_json::Value = serde_json::from_str(&body_str).unwrap();
		assert_eq!(body["is_authenticated"], false);
	}

	#[rstest]
	#[tokio::test]
	async fn test_expired_token_produces_anonymous_state() {
		// Arrange
		let secret = b"test-secret-key-256bit!!";
		let jwt_auth = JwtAuth::new(secret);
		let claims = Claims {
			sub: "user123".to_string(),
			exp: chrono::Utc::now().timestamp() - 3600,
			iat: chrono::Utc::now().timestamp() - 7200,
			username: "alice".to_string(),
			is_staff: false,
			is_superuser: false,
		};
		let token = jwt_auth.encode(&claims).unwrap();
		let middleware = JwtAuthMiddleware::from_secret(secret);
		let handler = Arc::new(TestHandler);
		let request = create_request_with_header("Authorization", &format!("Bearer {}", token));

		// Act
		let response = middleware.process(request, handler).await.unwrap();

		// Assert
		let body_str = String::from_utf8(response.body.to_vec()).unwrap();
		let body: serde_json::Value = serde_json::from_str(&body_str).unwrap();
		assert_eq!(body["is_authenticated"], false);
	}

	#[rstest]
	#[tokio::test]
	async fn test_non_bearer_scheme_produces_anonymous_state() {
		// Arrange
		let middleware = JwtAuthMiddleware::from_secret(b"test-secret");
		let handler = Arc::new(TestHandler);
		let request = create_request_with_header("Authorization", "Basic dXNlcjpwYXNz");

		// Act
		let response = middleware.process(request, handler).await.unwrap();

		// Assert
		let body_str = String::from_utf8(response.body.to_vec()).unwrap();
		let body: serde_json::Value = serde_json::from_str(&body_str).unwrap();
		assert_eq!(body["is_authenticated"], false);
	}

	#[rstest]
	#[tokio::test]
	async fn test_new_constructor_works_with_jwt_auth() {
		// Arrange
		let secret = b"test-secret-key-256bit!!";
		let jwt_auth = JwtAuth::new(secret);
		let token = jwt_auth
			.generate_token("user-42".to_string(), "bob".to_string(), false, false)
			.unwrap();
		let middleware = JwtAuthMiddleware::new(jwt_auth);
		let handler = Arc::new(TestHandler);
		let request = create_request_with_header("Authorization", &format!("Bearer {}", token));

		// Act
		let response = middleware.process(request, handler).await.unwrap();

		// Assert
		let body_str = String::from_utf8(response.body.to_vec()).unwrap();
		let body: serde_json::Value = serde_json::from_str(&body_str).unwrap();
		assert_eq!(body["is_authenticated"], true);
		assert_eq!(body["user_id"], "user-42");
	}

	#[rstest]
	#[tokio::test]
	async fn test_wrong_secret_produces_anonymous_state() {
		// Arrange
		let jwt_auth = JwtAuth::new(b"encoding-secret!!");
		let token = jwt_auth
			.generate_token("user-1".to_string(), "charlie".to_string(), false, false)
			.unwrap();
		let middleware = JwtAuthMiddleware::from_secret(b"different-secret!");
		let handler = Arc::new(TestHandler);
		let request = create_request_with_header("Authorization", &format!("Bearer {}", token));

		// Act
		let response = middleware.process(request, handler).await.unwrap();

		// Assert
		let body_str = String::from_utf8(response.body.to_vec()).unwrap();
		let body: serde_json::Value = serde_json::from_str(&body_str).unwrap();
		assert_eq!(body["is_authenticated"], false);
	}

	#[rstest]
	#[tokio::test]
	async fn test_staff_user_produces_admin_state() {
		// Arrange
		let secret = b"test-secret-key-256bit!!";
		let jwt_auth = JwtAuth::new(secret);
		let token = jwt_auth
			.generate_token(
				"staff-user".to_string(),
				"admin_alice".to_string(),
				true,
				false,
			)
			.unwrap();
		let middleware = JwtAuthMiddleware::from_secret(secret);
		let handler = Arc::new(TestHandler);
		let request = create_request_with_header("Authorization", &format!("Bearer {}", token));

		// Act
		let response = middleware.process(request, handler).await.unwrap();

		// Assert
		let body_str = String::from_utf8(response.body.to_vec()).unwrap();
		let body: serde_json::Value = serde_json::from_str(&body_str).unwrap();
		assert_eq!(body["is_authenticated"], true);
		assert_eq!(body["user_id"], "staff-user");
		assert_eq!(body["is_admin"], true);
		assert_eq!(body["is_active"], true);
	}

	#[rstest]
	#[tokio::test]
	async fn test_superuser_produces_admin_state() {
		// Arrange
		let secret = b"test-secret-key-256bit!!";
		let jwt_auth = JwtAuth::new(secret);
		let token = jwt_auth
			.generate_token(
				"super-user".to_string(),
				"superadmin".to_string(),
				false,
				true,
			)
			.unwrap();
		let middleware = JwtAuthMiddleware::from_secret(secret);
		let handler = Arc::new(TestHandler);
		let request = create_request_with_header("Authorization", &format!("Bearer {}", token));

		// Act
		let response = middleware.process(request, handler).await.unwrap();

		// Assert
		let body_str = String::from_utf8(response.body.to_vec()).unwrap();
		let body: serde_json::Value = serde_json::from_str(&body_str).unwrap();
		assert_eq!(body["is_authenticated"], true);
		assert_eq!(body["user_id"], "super-user");
		assert_eq!(body["is_admin"], true);
		assert_eq!(body["is_active"], true);
	}

	#[rstest]
	#[tokio::test]
	async fn test_non_staff_non_superuser_produces_non_admin_state() {
		// Arrange
		let secret = b"test-secret-key-256bit!!";
		let jwt_auth = JwtAuth::new(secret);
		let token = jwt_auth
			.generate_token("regular-user".to_string(), "bob".to_string(), false, false)
			.unwrap();
		let middleware = JwtAuthMiddleware::from_secret(secret);
		let handler = Arc::new(TestHandler);
		let request = create_request_with_header("Authorization", &format!("Bearer {}", token));

		// Act
		let response = middleware.process(request, handler).await.unwrap();

		// Assert
		let body_str = String::from_utf8(response.body.to_vec()).unwrap();
		let body: serde_json::Value = serde_json::from_str(&body_str).unwrap();
		assert_eq!(body["is_authenticated"], true);
		assert_eq!(body["user_id"], "regular-user");
		assert_eq!(body["is_admin"], false);
		assert_eq!(body["is_active"], true);
	}
}
