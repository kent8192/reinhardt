//! Router wrapper that adds OpenAPI documentation endpoints
//!
//! This module provides a wrapper around any `Handler` implementation that
//! automatically serves OpenAPI documentation endpoints without modifying
//! user code.
//!
//! # Example
//!
//! ```rust,ignore
//! use reinhardt_openapi::OpenApiRouter;
//! use reinhardt_urls::routers::BasicRouter;
//!
//! fn main() {
//!     // Create your existing router
//!     let router = BasicRouter::new();
//!
//!     // Wrap with OpenAPI endpoints
//!     let wrapped = OpenApiRouter::wrap(router)?;
//!
//!     // The wrapped router now serves:
//!     // - /api/openapi.json (OpenAPI spec)
//!     // - /api/docs (Swagger UI)
//!     // - /api/redoc (Redoc UI)
//! }
//! ```

use async_trait::async_trait;
use reinhardt_http::Handler;
use reinhardt_http::{Request, Response, Result};
use reinhardt_rest::openapi::endpoints::generate_openapi_schema;
use reinhardt_rest::openapi::{RedocUI, SwaggerUI};
use reinhardt_urls::prelude::Route;
use reinhardt_urls::routers::Router;
use std::sync::Arc;

/// Type alias for the authentication guard callback.
///
/// The guard receives a reference to the incoming request and returns
/// `true` if the request is authorized to access documentation endpoints,
/// or `false` to deny access with HTTP 403 Forbidden.
// Fixes #828
pub type AuthGuard = Arc<dyn Fn(&Request) -> bool + Send + Sync>;

/// Router wrapper that adds OpenAPI documentation endpoints
///
/// This wrapper intercepts requests to OpenAPI documentation paths and
/// serves them from memory, delegating all other requests to the wrapped
/// handler.
///
/// The OpenAPI schema is generated once at wrap time from the global
/// schema registry, ensuring minimal runtime overhead.
///
/// Access control is supported via the `enabled` flag and an optional
/// authentication guard callback. When `enabled` is `false`, all
/// documentation endpoints return HTTP 404. When an auth guard is set
/// and returns `false`, endpoints return HTTP 403.
pub struct OpenApiRouter<H> {
	/// Base handler to delegate to
	inner: H,
	/// Pre-generated OpenAPI JSON schema
	openapi_json: Arc<String>,
	/// Swagger UI HTML
	swagger_html: Arc<String>,
	/// Redoc UI HTML
	redoc_html: Arc<String>,
	/// Whether documentation endpoints are enabled (default: true)
	// Fixes #828
	enabled: bool,
	/// Optional authentication guard for documentation endpoints
	// Fixes #828
	auth_guard: Option<AuthGuard>,
}

impl<H> OpenApiRouter<H> {
	/// Wrap an existing handler with OpenAPI endpoints
	///
	/// This generates the OpenAPI schema from the global registry and
	/// pre-renders the Swagger and Redoc UIs.
	///
	/// # Example
	///
	/// ```rust,ignore
	/// use reinhardt_openapi::OpenApiRouter;
	/// use reinhardt_urls::routers::BasicRouter;
	///
	/// let router = BasicRouter::new();
	/// let wrapped = OpenApiRouter::wrap(router)?;
	/// # Ok::<(), reinhardt_rest::openapi::SchemaError>(())
	/// ```
	pub fn wrap(handler: H) -> std::result::Result<Self, reinhardt_rest::openapi::SchemaError> {
		// Generate OpenAPI schema from global registry
		let schema = generate_openapi_schema();
		let openapi_json = serde_json::to_string_pretty(&schema)?;

		// Generate Swagger UI HTML
		let swagger_ui = SwaggerUI::new(schema.clone());
		let swagger_html = swagger_ui.render_html()?;

		// Generate Redoc UI HTML
		let redoc_ui = RedocUI::new(schema);
		let redoc_html = redoc_ui.render_html()?;

		Ok(Self {
			inner: handler,
			openapi_json: Arc::new(openapi_json),
			swagger_html: Arc::new(swagger_html),
			redoc_html: Arc::new(redoc_html),
			enabled: true,
			auth_guard: None,
		})
	}

	/// Set whether documentation endpoints are enabled
	///
	/// When set to `false`, all documentation endpoints (`/api/openapi.json`,
	/// `/api/docs`, `/api/redoc`) will return HTTP 404 Not Found.
	///
	/// Default is `true`.
	///
	/// # Example
	///
	/// ```rust,ignore
	/// use reinhardt_openapi::OpenApiRouter;
	/// use reinhardt_urls::routers::BasicRouter;
	///
	/// let router = BasicRouter::new();
	/// let wrapped = OpenApiRouter::wrap(router)?.enabled(false);
	/// ```
	// Fixes #828
	pub fn enabled(mut self, enabled: bool) -> Self {
		self.enabled = enabled;
		self
	}

	/// Set an authentication guard for documentation endpoints
	///
	/// The guard function receives a reference to the incoming request and
	/// should return `true` to allow access or `false` to deny with HTTP 403
	/// Forbidden.
	///
	/// The guard is only checked when `enabled` is `true`. When `enabled` is
	/// `false`, endpoints return 404 regardless of the guard.
	///
	/// # Example
	///
	/// ```rust,ignore
	/// use reinhardt_openapi::OpenApiRouter;
	/// use reinhardt_urls::routers::BasicRouter;
	///
	/// let router = BasicRouter::new();
	/// let wrapped = OpenApiRouter::wrap(router)?.auth_guard(|request| {
	///     // Check for API key in header
	///     request.headers().get("X-Api-Key")
	///         .map(|v| v == "secret")
	///         .unwrap_or(false)
	/// });
	/// ```
	// Fixes #828
	pub fn auth_guard(mut self, guard: impl Fn(&Request) -> bool + Send + Sync + 'static) -> Self {
		self.auth_guard = Some(Arc::new(guard));
		self
	}

	/// Get a reference to the wrapped handler
	pub fn inner(&self) -> &H {
		&self.inner
	}

	/// Check access control for documentation endpoints.
	///
	/// Returns `None` if access is allowed, or `Some(Response)` with the
	/// appropriate error status if access is denied.
	// Fixes #828
	fn check_access(&self, request: &Request) -> Option<Response> {
		if !self.enabled {
			return Some(Response::not_found());
		}
		if let Some(ref guard) = self.auth_guard
			&& !guard(request)
		{
			return Some(Response::forbidden());
		}
		None
	}

	/// Try to serve an OpenAPI documentation endpoint.
	///
	/// Returns `Some(Ok(Response))` if the request path matches an OpenAPI
	/// endpoint and access control checks pass, `Some(Ok(denied))` if access
	/// is denied, or `None` if the path does not match any documentation
	/// endpoint.
	///
	/// Fixes #831: Deduplicate route handling between Handler and Router.
	fn try_serve_openapi(&self, request: &Request) -> Option<Result<Response>> {
		match request.uri.path() {
			"/api/openapi.json" | "/api/docs" | "/api/redoc" => {
				if let Some(denied) = self.check_access(request) {
					return Some(Ok(denied));
				}
				let response = match request.uri.path() {
					"/api/openapi.json" => {
						let json = (*self.openapi_json).clone();
						Response::ok()
							.with_header("Content-Type", "application/json; charset=utf-8")
							.with_body(json)
					}
					"/api/docs" => {
						let html = (*self.swagger_html).clone();
						Response::ok()
							.with_header("Content-Type", "text/html; charset=utf-8")
							.with_body(html)
					}
					"/api/redoc" => {
						let html = (*self.redoc_html).clone();
						Response::ok()
							.with_header("Content-Type", "text/html; charset=utf-8")
							.with_body(html)
					}
					_ => unreachable!(),
				};
				Some(Ok(Self::apply_security_headers(response)))
			}
			_ => None,
		}
	}

	/// Apply security headers to documentation endpoint responses.
	///
	/// Adds Content-Security-Policy, X-Frame-Options, X-Content-Type-Options,
	/// and Cache-Control headers to prevent clickjacking, MIME sniffing,
	/// and stale cache attacks on documentation pages.
	// Fixes #830
	fn apply_security_headers(response: Response) -> Response {
		response
			.with_header(
				"Content-Security-Policy",
				"default-src 'none'; \
				 script-src 'unsafe-inline' https://unpkg.com https://cdn.redoc.ly; \
				 style-src 'unsafe-inline' https://unpkg.com; \
				 img-src 'self' data:; \
				 connect-src 'self'; \
				 font-src https://fonts.gstatic.com; \
				 frame-ancestors 'none'",
			)
			.with_header("X-Frame-Options", "DENY")
			.with_header("X-Content-Type-Options", "nosniff")
			.with_header("Cache-Control", "no-store")
	}
}

#[async_trait]
impl<H: Handler> Handler for OpenApiRouter<H> {
	/// Handle requests, intercepting OpenAPI documentation paths
	///
	/// Requests to `/api/openapi.json`, `/api/docs`, or `/api/redoc`
	/// are served from memory if access control checks pass. All other
	/// requests are delegated to the wrapped handler.
	///
	/// Access control is enforced via the `enabled` flag and optional
	/// auth guard. Disabled endpoints return 404, unauthorized requests
	/// return 403.
	async fn handle(&self, request: Request) -> Result<Response> {
		// Fixes #831: Use shared OpenAPI serving logic
		if let Some(response) = self.try_serve_openapi(&request) {
			return response;
		}
		self.inner.handle(request).await
	}
}

/// Router trait implementation for OpenApiRouter
///
/// This implementation allows OpenApiRouter to be used where Router trait
/// is required. However, routes cannot be modified after wrapping - use
/// `add_route()` and `include()` on the base router before wrapping.
impl<H> Router for OpenApiRouter<H>
where
	H: Handler + Router,
{
	/// Add a route to the router
	///
	/// # Panics
	///
	/// This method always panics. Routes must be added to the base router
	/// before wrapping with `OpenApiRouter::wrap()`.
	fn add_route(&mut self, _route: Route) {
		panic!(
			"Cannot add routes to OpenApiRouter after wrapping. \
             Add routes to the base router before calling OpenApiRouter::wrap()."
		);
	}

	/// Include routes with a prefix
	///
	/// # Panics
	///
	/// This method always panics. Routes must be mounted in the base router
	/// before wrapping with `OpenApiRouter::wrap()`.
	fn mount(&mut self, _prefix: &str, _routes: Vec<Route>, _namespace: Option<String>) {
		panic!(
			"Cannot mount routes in OpenApiRouter after wrapping. \
             Mount routes in the base router before calling OpenApiRouter::wrap()."
		);
	}

	/// Route a request through the OpenAPI wrapper
	///
	/// OpenAPI documentation endpoints (`/api/openapi.json`, `/api/docs`,
	/// `/api/redoc`) are handled directly if access control checks pass.
	/// All other requests are delegated to the wrapped router's `route()`
	/// method.
	///
	/// Access control is enforced via the `enabled` flag and optional
	/// auth guard. Disabled endpoints return 404, unauthorized requests
	/// return 403.
	async fn route(&self, request: Request) -> Result<Response> {
		// Fixes #831: Use shared OpenAPI serving logic
		if let Some(response) = self.try_serve_openapi(&request) {
			return response;
		}
		self.inner.route(request).await
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use hyper::StatusCode;
	use rstest::rstest;

	struct DummyHandler;

	#[async_trait]
	impl Handler for DummyHandler {
		async fn handle(&self, _request: Request) -> Result<Response> {
			Ok(Response::new(StatusCode::OK).with_body("Hello from inner handler"))
		}
	}

	#[rstest]
	#[tokio::test]
	async fn test_openapi_json_endpoint() {
		// Arrange
		let handler = DummyHandler;
		let wrapped = OpenApiRouter::wrap(handler).unwrap();

		// Act
		let request = Request::builder().uri("/api/openapi.json").build().unwrap();
		let response = wrapped.handle(request).await.unwrap();

		// Assert
		assert_eq!(response.status, StatusCode::OK);
		let body_str = String::from_utf8(response.body.to_vec()).unwrap();
		assert!(body_str.contains("openapi"));
		assert!(body_str.contains("3.")); // OpenAPI version (3.0 or 3.1)
	}

	#[rstest]
	#[tokio::test]
	async fn test_swagger_docs_endpoint() {
		// Arrange
		let handler = DummyHandler;
		let wrapped = OpenApiRouter::wrap(handler).unwrap();

		// Act
		let request = Request::builder().uri("/api/docs").build().unwrap();
		let response = wrapped.handle(request).await.unwrap();

		// Assert
		assert_eq!(response.status, StatusCode::OK);
		let body_str = String::from_utf8(response.body.to_vec()).unwrap();
		assert!(body_str.contains("swagger-ui"));
	}

	#[rstest]
	#[tokio::test]
	async fn test_redoc_docs_endpoint() {
		// Arrange
		let handler = DummyHandler;
		let wrapped = OpenApiRouter::wrap(handler).unwrap();

		// Act
		let request = Request::builder().uri("/api/redoc").build().unwrap();
		let response = wrapped.handle(request).await.unwrap();

		// Assert
		assert_eq!(response.status, StatusCode::OK);
		let body_str = String::from_utf8(response.body.to_vec()).unwrap();
		assert!(body_str.contains("redoc"));
	}

	#[rstest]
	#[tokio::test]
	async fn test_delegation_to_inner_handler() {
		// Arrange
		let handler = DummyHandler;
		let wrapped = OpenApiRouter::wrap(handler).unwrap();

		// Act
		let request = Request::builder().uri("/some/other/path").build().unwrap();
		let response = wrapped.handle(request).await.unwrap();

		// Assert
		assert_eq!(response.status, StatusCode::OK);
		let body_str = String::from_utf8(response.body.to_vec()).unwrap();
		assert_eq!(body_str, "Hello from inner handler");
	}

	// Fixes #828: Access control tests

	#[rstest]
	#[case("/api/openapi.json")]
	#[case("/api/docs")]
	#[case("/api/redoc")]
	#[tokio::test]
	async fn test_disabled_endpoints_return_404(#[case] path: &str) {
		// Arrange
		let handler = DummyHandler;
		let wrapped = OpenApiRouter::wrap(handler).unwrap().enabled(false);

		// Act
		let request = Request::builder().uri(path).build().unwrap();
		let response = wrapped.handle(request).await.unwrap();

		// Assert
		assert_eq!(response.status, StatusCode::NOT_FOUND);
	}

	#[rstest]
	#[tokio::test]
	async fn test_disabled_does_not_affect_other_routes() {
		// Arrange
		let handler = DummyHandler;
		let wrapped = OpenApiRouter::wrap(handler).unwrap().enabled(false);

		// Act
		let request = Request::builder().uri("/some/other/path").build().unwrap();
		let response = wrapped.handle(request).await.unwrap();

		// Assert
		assert_eq!(response.status, StatusCode::OK);
		let body_str = String::from_utf8(response.body.to_vec()).unwrap();
		assert_eq!(body_str, "Hello from inner handler");
	}

	#[rstest]
	#[case("/api/openapi.json")]
	#[case("/api/docs")]
	#[case("/api/redoc")]
	#[tokio::test]
	async fn test_auth_guard_rejects_unauthorized(#[case] path: &str) {
		// Arrange
		let handler = DummyHandler;
		let wrapped = OpenApiRouter::wrap(handler)
			.unwrap()
			.auth_guard(|_request| false);

		// Act
		let request = Request::builder().uri(path).build().unwrap();
		let response = wrapped.handle(request).await.unwrap();

		// Assert
		assert_eq!(response.status, StatusCode::FORBIDDEN);
	}

	#[rstest]
	#[case("/api/openapi.json")]
	#[case("/api/docs")]
	#[case("/api/redoc")]
	#[tokio::test]
	async fn test_auth_guard_allows_authorized(#[case] path: &str) {
		// Arrange
		let handler = DummyHandler;
		let wrapped = OpenApiRouter::wrap(handler)
			.unwrap()
			.auth_guard(|_request| true);

		// Act
		let request = Request::builder().uri(path).build().unwrap();
		let response = wrapped.handle(request).await.unwrap();

		// Assert
		assert_eq!(response.status, StatusCode::OK);
	}

	#[rstest]
	#[tokio::test]
	async fn test_auth_guard_does_not_affect_other_routes() {
		// Arrange
		let handler = DummyHandler;
		let wrapped = OpenApiRouter::wrap(handler)
			.unwrap()
			.auth_guard(|_request| false);

		// Act
		let request = Request::builder().uri("/some/other/path").build().unwrap();
		let response = wrapped.handle(request).await.unwrap();

		// Assert
		assert_eq!(response.status, StatusCode::OK);
		let body_str = String::from_utf8(response.body.to_vec()).unwrap();
		assert_eq!(body_str, "Hello from inner handler");
	}

	#[rstest]
	#[case("/api/openapi.json")]
	#[case("/api/docs")]
	#[case("/api/redoc")]
	#[tokio::test]
	async fn test_disabled_takes_precedence_over_auth_guard(#[case] path: &str) {
		// Arrange: enabled=false should return 404 even with a passing auth guard
		let handler = DummyHandler;
		let wrapped = OpenApiRouter::wrap(handler)
			.unwrap()
			.enabled(false)
			.auth_guard(|_request| true);

		// Act
		let request = Request::builder().uri(path).build().unwrap();
		let response = wrapped.handle(request).await.unwrap();

		// Assert: Should be 404 (disabled), not 200 (auth passed)
		assert_eq!(response.status, StatusCode::NOT_FOUND);
	}

	#[rstest]
	#[tokio::test]
	async fn test_auth_guard_inspects_request_headers() {
		// Arrange: Guard checks for a specific header value
		let handler = DummyHandler;
		let wrapped = OpenApiRouter::wrap(handler).unwrap().auth_guard(|request| {
			request
				.headers
				.get("X-Docs-Token")
				.and_then(|v| v.to_str().ok())
				.map(|v| v == "valid-token")
				.unwrap_or(false)
		});

		// Act: Request without token
		let request_no_token = Request::builder().uri("/api/docs").build().unwrap();
		let response_no_token = wrapped.handle(request_no_token).await.unwrap();

		// Assert: Should be forbidden
		assert_eq!(response_no_token.status, StatusCode::FORBIDDEN);

		// Act: Request with valid token
		let request_valid = Request::builder()
			.uri("/api/docs")
			.header("X-Docs-Token", "valid-token")
			.build()
			.unwrap();
		let response_valid = wrapped.handle(request_valid).await.unwrap();

		// Assert: Should be OK
		assert_eq!(response_valid.status, StatusCode::OK);

		// Act: Request with invalid token
		let request_invalid = Request::builder()
			.uri("/api/docs")
			.header("X-Docs-Token", "wrong-token")
			.build()
			.unwrap();
		let response_invalid = wrapped.handle(request_invalid).await.unwrap();

		// Assert: Should be forbidden
		assert_eq!(response_invalid.status, StatusCode::FORBIDDEN);
	}
}
