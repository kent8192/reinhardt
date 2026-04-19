use async_trait::async_trait;
use std::sync::Arc;

use reinhardt_http::{AuthState, Handler, Middleware, Request, Response, Result};

/// Default login URL for unauthenticated users.
pub const DEFAULT_LOGIN_URL: &str = "/accounts/login/";

/// Default query parameter name for the redirect URL.
pub const DEFAULT_REDIRECT_FIELD_NAME: &str = "next";

/// Configuration for [`LoginRequiredMiddleware`].
///
/// # Examples
///
/// ```rust
/// use reinhardt_middleware::LoginRequiredConfig;
///
/// let config = LoginRequiredConfig::new()
///     .with_login_url("/auth/login/")
///     .with_redirect_field_name("return_to")
///     .with_exempt_path("/api/health");
/// ```
#[derive(Clone, Debug)]
pub struct LoginRequiredConfig {
	/// The URL to redirect unauthenticated users to.
	pub login_url: String,
	/// The query parameter name for the original URL.
	pub redirect_field_name: String,
	/// Paths exempt from the login requirement (prefix match).
	///
	/// Paths ending with `/` are prefix-matched (e.g., `/api/` matches
	/// `/api/health`). Paths without a trailing `/` are exact-matched.
	pub exempt_paths: Vec<String>,
}

impl Default for LoginRequiredConfig {
	fn default() -> Self {
		Self {
			login_url: DEFAULT_LOGIN_URL.to_string(),
			redirect_field_name: DEFAULT_REDIRECT_FIELD_NAME.to_string(),
			exempt_paths: Vec::new(),
		}
	}
}

impl LoginRequiredConfig {
	/// Creates a new configuration with default values.
	///
	/// Defaults:
	/// - `login_url`: `/accounts/login/`
	/// - `redirect_field_name`: `next`
	/// - `exempt_paths`: empty
	pub fn new() -> Self {
		Self::default()
	}

	/// Sets the login URL.
	pub fn with_login_url(mut self, url: &str) -> Self {
		self.login_url = url.to_string();
		self
	}

	/// Sets the redirect field name (query parameter).
	pub fn with_redirect_field_name(mut self, name: &str) -> Self {
		self.redirect_field_name = name.to_string();
		self
	}

	/// Adds an exempt path.
	///
	/// Paths ending with `/` are prefix-matched.
	/// Paths without a trailing `/` are exact-matched.
	pub fn with_exempt_path(mut self, path: &str) -> Self {
		self.exempt_paths.push(path.to_string());
		self
	}

	/// Adds multiple exempt paths.
	pub fn with_exempt_paths(mut self, paths: &[&str]) -> Self {
		self.exempt_paths
			.extend(paths.iter().map(|p| p.to_string()));
		self
	}

	/// Checks if a request path is exempt from the login requirement.
	fn is_exempt(&self, path: &str) -> bool {
		// The login URL itself is always exempt to avoid redirect loops
		if path == self.login_url || path.starts_with(&self.login_url) {
			return true;
		}

		self.exempt_paths.iter().any(|exempt| {
			if exempt.ends_with('/') {
				// Prefix match for paths ending with /
				path.starts_with(exempt.as_str())
			} else {
				// Exact match
				path == exempt
			}
		})
	}
}

/// Login required middleware.
///
/// Redirects unauthenticated users to a login page. This is the Rust
/// equivalent of Django 5.1's
/// [`LoginRequiredMiddleware`](https://docs.djangoproject.com/en/5.1/ref/middleware/#django.contrib.auth.middleware.LoginRequiredMiddleware).
///
/// The middleware checks the `AuthState` in request extensions (set by
/// [`AuthenticationMiddleware`](crate::AuthenticationMiddleware) or
/// [`JwtAuthMiddleware`](crate::jwt_auth::JwtAuthMiddleware)). If the
/// user is not authenticated and the path is not exempt, the middleware
/// returns a 302 redirect to the configured login URL with the original
/// path as a query parameter.
///
/// # Path Exemption
///
/// Certain paths can be exempt from the login requirement:
/// - Configure exempt paths via [`LoginRequiredConfig::with_exempt_path`]
/// - The login URL itself is always exempt (prevents redirect loops)
/// - Paths ending with `/` are prefix-matched
/// - Paths without `/` are exact-matched
///
/// # Examples
///
/// ```rust,no_run
/// use std::sync::Arc;
/// use reinhardt_middleware::{LoginRequiredMiddleware, LoginRequiredConfig};
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
/// let config = LoginRequiredConfig::new()
///     .with_login_url("/auth/login/")
///     .with_exempt_path("/api/public/")
///     .with_exempt_path("/health");
///
/// let middleware = LoginRequiredMiddleware::new(config);
///
/// let app = MiddlewareChain::new(handler)
///     .with_middleware(Arc::new(middleware));
/// ```
pub struct LoginRequiredMiddleware {
	config: LoginRequiredConfig,
}

impl LoginRequiredMiddleware {
	/// Creates a new login required middleware with the given configuration.
	pub fn new(config: LoginRequiredConfig) -> Self {
		Self { config }
	}

	/// Builds the redirect URL with the original path as a query parameter.
	fn build_redirect_url(&self, original_path: &str) -> String {
		format!(
			"{}?{}={}",
			self.config.login_url, self.config.redirect_field_name, original_path
		)
	}
}

impl Default for LoginRequiredMiddleware {
	fn default() -> Self {
		Self::new(LoginRequiredConfig::default())
	}
}

#[async_trait]
impl Middleware for LoginRequiredMiddleware {
	async fn process(&self, request: Request, next: Arc<dyn Handler>) -> Result<Response> {
		let path = request.uri.path().to_string();

		// Skip exempt paths
		if self.config.is_exempt(&path) {
			return next.handle(request).await;
		}

		// Check authentication state
		let is_authenticated = request
			.extensions
			.get::<AuthState>()
			.map(|s| s.is_authenticated())
			.unwrap_or(false);

		if !is_authenticated {
			let redirect_url = self.build_redirect_url(&path);
			return Ok(Response::temporary_redirect(&redirect_url));
		}

		next.handle(request).await
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use bytes::Bytes;
	use hyper::{HeaderMap, Method, StatusCode, Version};
	use reinhardt_http::{AuthState, Handler, Middleware, Request, Response};
	use rstest::rstest;

	struct TestHandler;

	#[async_trait::async_trait]
	impl Handler for TestHandler {
		async fn handle(&self, _request: Request) -> Result<Response> {
			Ok(Response::ok())
		}
	}

	fn create_request(path: &str, auth_state: Option<AuthState>) -> Request {
		let request = Request::builder()
			.method(Method::GET)
			.uri(path)
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		if let Some(state) = auth_state {
			request.extensions.insert(state);
		}

		request
	}

	#[rstest]
	#[tokio::test]
	async fn test_authenticated_user_passes_through() {
		// Arrange
		let middleware = LoginRequiredMiddleware::default();
		let handler = Arc::new(TestHandler);
		let request = create_request(
			"/dashboard",
			Some(AuthState::authenticated("user-1", false, true)),
		);

		// Act
		let response = middleware.process(request, handler).await.unwrap();

		// Assert
		assert_eq!(response.status, StatusCode::OK);
	}

	#[rstest]
	#[tokio::test]
	async fn test_unauthenticated_user_gets_redirected() {
		// Arrange
		let middleware = LoginRequiredMiddleware::default();
		let handler = Arc::new(TestHandler);
		let request = create_request("/dashboard", Some(AuthState::anonymous()));

		// Act
		let response = middleware.process(request, handler).await.unwrap();

		// Assert
		assert_eq!(response.status, StatusCode::FOUND);
		let location = response.headers.get("Location").unwrap().to_str().unwrap();
		assert_eq!(location, "/accounts/login/?next=/dashboard");
	}

	#[rstest]
	#[tokio::test]
	async fn test_no_auth_state_gets_redirected() {
		// Arrange
		let middleware = LoginRequiredMiddleware::default();
		let handler = Arc::new(TestHandler);
		let request = create_request("/dashboard", None);

		// Act
		let response = middleware.process(request, handler).await.unwrap();

		// Assert
		assert_eq!(response.status, StatusCode::FOUND);
	}

	#[rstest]
	#[tokio::test]
	async fn test_login_url_is_exempt() {
		// Arrange
		let middleware = LoginRequiredMiddleware::default();
		let handler = Arc::new(TestHandler);
		let request = create_request("/accounts/login/", Some(AuthState::anonymous()));

		// Act
		let response = middleware.process(request, handler).await.unwrap();

		// Assert
		assert_eq!(response.status, StatusCode::OK);
	}

	#[rstest]
	#[tokio::test]
	async fn test_custom_exempt_path_prefix_match() {
		// Arrange
		let config = LoginRequiredConfig::new().with_exempt_path("/api/public/");
		let middleware = LoginRequiredMiddleware::new(config);
		let handler = Arc::new(TestHandler);
		let request = create_request("/api/public/health", Some(AuthState::anonymous()));

		// Act
		let response = middleware.process(request, handler).await.unwrap();

		// Assert
		assert_eq!(response.status, StatusCode::OK);
	}

	#[rstest]
	#[tokio::test]
	async fn test_custom_exempt_path_exact_match() {
		// Arrange
		let config = LoginRequiredConfig::new().with_exempt_path("/health");
		let middleware = LoginRequiredMiddleware::new(config);
		let handler = Arc::new(TestHandler);
		let request = create_request("/health", Some(AuthState::anonymous()));

		// Act
		let response = middleware.process(request, handler).await.unwrap();

		// Assert
		assert_eq!(response.status, StatusCode::OK);
	}

	#[rstest]
	#[tokio::test]
	async fn test_custom_exempt_path_exact_no_prefix() {
		// Arrange
		let config = LoginRequiredConfig::new().with_exempt_path("/health");
		let middleware = LoginRequiredMiddleware::new(config);
		let handler = Arc::new(TestHandler);
		let request = create_request("/health/detail", Some(AuthState::anonymous()));

		// Act
		let response = middleware.process(request, handler).await.unwrap();

		// Assert - /health/detail should NOT be exempt (exact match only)
		assert_eq!(response.status, StatusCode::FOUND);
	}

	#[rstest]
	#[tokio::test]
	async fn test_custom_login_url_and_redirect_field() {
		// Arrange
		let config = LoginRequiredConfig::new()
			.with_login_url("/auth/signin/")
			.with_redirect_field_name("return_to");
		let middleware = LoginRequiredMiddleware::new(config);
		let handler = Arc::new(TestHandler);
		let request = create_request("/dashboard", Some(AuthState::anonymous()));

		// Act
		let response = middleware.process(request, handler).await.unwrap();

		// Assert
		assert_eq!(response.status, StatusCode::FOUND);
		let location = response.headers.get("Location").unwrap().to_str().unwrap();
		assert_eq!(location, "/auth/signin/?return_to=/dashboard");
	}

	#[rstest]
	#[tokio::test]
	async fn test_multiple_exempt_paths() {
		// Arrange
		let config =
			LoginRequiredConfig::new().with_exempt_paths(&["/api/", "/health", "/static/"]);
		let middleware = LoginRequiredMiddleware::new(config);
		let handler = Arc::new(TestHandler);

		// Act & Assert - /api/ prefix
		let request = create_request("/api/users", Some(AuthState::anonymous()));
		let response = middleware.process(request, handler.clone()).await.unwrap();
		assert_eq!(response.status, StatusCode::OK);

		// Act & Assert - /health exact
		let request = create_request("/health", Some(AuthState::anonymous()));
		let response = middleware.process(request, handler.clone()).await.unwrap();
		assert_eq!(response.status, StatusCode::OK);

		// Act & Assert - /static/ prefix
		let request = create_request("/static/css/main.css", Some(AuthState::anonymous()));
		let response = middleware.process(request, handler.clone()).await.unwrap();
		assert_eq!(response.status, StatusCode::OK);

		// Act & Assert - /dashboard should still redirect
		let request = create_request("/dashboard", Some(AuthState::anonymous()));
		let response = middleware.process(request, handler).await.unwrap();
		assert_eq!(response.status, StatusCode::FOUND);
	}
}
