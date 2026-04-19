//! Admin Cookie-based JWT Authentication Middleware
//!
//! Extracts JWT tokens from the `reinhardt_admin_token` HTTP-Only cookie
//! and populates the request's [`AuthState`](reinhardt_http::AuthState) extension. This replaces the
//! `Authorization: Bearer` header approach for the admin panel, providing
//! XSS protection since JavaScript cannot access HTTP-Only cookies.
//!
//! # Extraction Order
//!
//! 1. `reinhardt_admin_token` cookie (primary — set by admin login)
//! 2. `Authorization: Bearer` header (fallback — for API testing / migration)
//!
//! # Security Properties
//!
//! - The cookie is `HttpOnly`, so XSS attacks cannot steal the token.
//! - `SameSite=Strict` prevents cross-origin cookie sending (CSRF protection).
//! - `Path=/admin` limits the cookie scope to admin routes.
//! - `Secure` flag ensures HTTPS-only transmission in production.

use async_trait::async_trait;
use reinhardt_auth::JwtAuth;
use reinhardt_http::{
	AuthState, Handler, IsActive, IsAdmin, IsAuthenticated, Middleware, Request, Response, Result,
};
use std::sync::Arc;

use super::security::extract_admin_auth_cookie;

/// Admin-specific JWT authentication middleware.
///
/// Unlike the general `JwtAuthMiddleware` from reinhardt-middleware, this
/// middleware extracts JWT tokens from the `reinhardt_admin_token` cookie
/// first, falling back to the `Authorization: Bearer` header.
///
/// # Example
///
/// ```ignore
/// use reinhardt_admin::server::cookie_auth::AdminCookieAuthMiddleware;
///
/// let middleware = AdminCookieAuthMiddleware::new(b"jwt-secret");
/// let router = ServerRouter::new()
///     .with_namespace("admin")
///     .with_middleware(middleware);
/// ```
pub struct AdminCookieAuthMiddleware {
	jwt_auth: JwtAuth,
}

impl AdminCookieAuthMiddleware {
	/// Creates a new admin cookie auth middleware from a JWT secret.
	pub fn new(secret: &[u8]) -> Self {
		Self {
			jwt_auth: JwtAuth::new(secret),
		}
	}

	/// Creates a new admin cookie auth middleware from a pre-built `JwtAuth`.
	pub fn from_jwt_auth(jwt_auth: JwtAuth) -> Self {
		Self { jwt_auth }
	}

	/// Extracts the JWT token from the admin auth cookie or Authorization header.
	fn extract_token(request: &Request) -> Option<String> {
		// 1. Try cookie first (primary for admin panel)
		if let Some(token) = extract_admin_auth_cookie(&request.headers) {
			return Some(token);
		}

		// 2. Fall back to Authorization: Bearer header (API testing / migration)
		request
			.headers
			.get("Authorization")
			.and_then(|v| v.to_str().ok())
			.and_then(|s| s.strip_prefix("Bearer "))
			.map(|s| s.to_string())
	}
}

#[async_trait]
impl Middleware for AdminCookieAuthMiddleware {
	async fn process(&self, request: Request, next: Arc<dyn Handler>) -> Result<Response> {
		let auth_state = if let Some(token) = Self::extract_token(&request)
			&& let Ok(claims) = self.jwt_auth.verify_token(&token)
		{
			let user_id = claims.sub;
			let is_admin = claims.is_staff || claims.is_superuser;
			let is_active = true;

			// Insert individual values for backward compatibility with
			// existing code that reads from extensions directly.
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

#[cfg(test)]
mod tests {
	use super::*;
	use crate::server::security::ADMIN_AUTH_COOKIE_NAME;
	use bytes::Bytes;
	use hyper::{HeaderMap, Method, StatusCode, Version};
	use reinhardt_auth::JwtAuth;

	struct AuthCheckHandler;

	#[async_trait]
	impl Handler for AuthCheckHandler {
		async fn handle(&self, request: Request) -> Result<Response> {
			let auth_state: AuthState = request
				.extensions
				.get()
				.unwrap_or_else(AuthState::anonymous);
			let body = if auth_state.is_authenticated() {
				format!("authenticated:{}", auth_state.user_id())
			} else {
				"anonymous".to_string()
			};
			Ok(Response::new(StatusCode::OK).with_body(body))
		}
	}

	fn make_request(headers: HeaderMap) -> Request {
		Request::builder()
			.method(Method::POST)
			.uri("/admin/api/server_fn/get_list")
			.version(Version::HTTP_11)
			.headers(headers)
			.body(Bytes::new())
			.build()
			.unwrap()
	}

	fn test_secret() -> &'static [u8] {
		b"test-secret-key-for-jwt-testing-purposes"
	}

	fn generate_test_token(user_id: &str) -> String {
		let jwt_auth = JwtAuth::new(test_secret());
		jwt_auth
			.generate_token(user_id.to_string(), "admin".to_string(), true, false)
			.unwrap()
	}

	#[tokio::test]
	async fn test_no_token_returns_anonymous() {
		let mw = AdminCookieAuthMiddleware::new(test_secret());
		let next = Arc::new(AuthCheckHandler);
		let req = make_request(HeaderMap::new());
		let resp = mw.process(req, next).await.unwrap();
		assert_eq!(resp.body, "anonymous");
	}

	#[tokio::test]
	async fn test_cookie_token_authenticates() {
		let token = generate_test_token("user-123");
		let mw = AdminCookieAuthMiddleware::new(test_secret());
		let next = Arc::new(AuthCheckHandler);
		let mut headers = HeaderMap::new();
		headers.insert(
			"cookie",
			format!("{}={}", ADMIN_AUTH_COOKIE_NAME, token)
				.parse()
				.unwrap(),
		);
		let req = make_request(headers);
		let resp = mw.process(req, next).await.unwrap();
		assert_eq!(resp.body, "authenticated:user-123");
	}

	#[tokio::test]
	async fn test_bearer_token_authenticates_as_fallback() {
		let token = generate_test_token("user-456");
		let mw = AdminCookieAuthMiddleware::new(test_secret());
		let next = Arc::new(AuthCheckHandler);
		let mut headers = HeaderMap::new();
		headers.insert(
			"Authorization",
			format!("Bearer {}", token).parse().unwrap(),
		);
		let req = make_request(headers);
		let resp = mw.process(req, next).await.unwrap();
		assert_eq!(resp.body, "authenticated:user-456");
	}

	#[tokio::test]
	async fn test_cookie_takes_precedence_over_bearer() {
		let cookie_token = generate_test_token("cookie-user");
		let bearer_token = generate_test_token("bearer-user");
		let mw = AdminCookieAuthMiddleware::new(test_secret());
		let next = Arc::new(AuthCheckHandler);
		let mut headers = HeaderMap::new();
		headers.insert(
			"cookie",
			format!("{}={}", ADMIN_AUTH_COOKIE_NAME, cookie_token)
				.parse()
				.unwrap(),
		);
		headers.insert(
			"Authorization",
			format!("Bearer {}", bearer_token).parse().unwrap(),
		);
		let req = make_request(headers);
		let resp = mw.process(req, next).await.unwrap();
		assert_eq!(resp.body, "authenticated:cookie-user");
	}

	#[tokio::test]
	async fn test_invalid_token_returns_anonymous() {
		let mw = AdminCookieAuthMiddleware::new(test_secret());
		let next = Arc::new(AuthCheckHandler);
		let mut headers = HeaderMap::new();
		headers.insert(
			"cookie",
			format!("{}=invalid.token.here", ADMIN_AUTH_COOKIE_NAME)
				.parse()
				.unwrap(),
		);
		let req = make_request(headers);
		let resp = mw.process(req, next).await.unwrap();
		assert_eq!(resp.body, "anonymous");
	}
}
