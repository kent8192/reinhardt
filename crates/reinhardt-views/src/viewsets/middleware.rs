//! Middleware support for ViewSets
//!
//! Provides middleware integration capabilities for ViewSets, including
//! authentication, permission checks, and other cross-cutting concerns.

use async_trait::async_trait;
use reinhardt_http::{Request, Response, Result};
use std::sync::Arc;
use tracing;

/// Middleware trait for ViewSet processing
///
/// This trait allows ViewSets to integrate with middleware components
/// for authentication, authorization, logging, and other cross-cutting concerns.
#[async_trait]
pub trait ViewSetMiddleware: Send + Sync {
	/// Process the request before it reaches the ViewSet
	///
	/// This method is called before the ViewSet's dispatch method.
	/// It can modify the request, perform authentication, or return an early response.
	///
	/// # Arguments
	///
	/// * `request` - The incoming HTTP request
	///
	/// # Returns
	///
	/// * `Ok(Some(response))` - Return an early response, bypassing the ViewSet
	/// * `Ok(None)` - Continue processing with the ViewSet
	/// * `Err(error)` - Return an error response
	async fn process_request(&self, request: &mut Request) -> Result<Option<Response>>;

	/// Process the response after it leaves the ViewSet
	///
	/// This method is called after the ViewSet's dispatch method.
	/// It can modify the response, add headers, or perform cleanup.
	///
	/// # Arguments
	///
	/// * `request` - The original HTTP request
	/// * `response` - The response from the ViewSet
	///
	/// # Returns
	///
	/// * `Ok(modified_response)` - The modified response
	/// * `Err(error)` - An error occurred during processing
	async fn process_response(&self, request: &Request, response: Response) -> Result<Response>;
}

/// Authentication middleware for ViewSets
///
/// Provides login_required functionality similar to Django's @login_required decorator.
#[derive(Debug, Clone)]
pub struct AuthenticationMiddleware {
	/// Whether login is required for this ViewSet
	pub login_required: bool,
	/// Login URL to redirect to if authentication is required
	pub login_url: Option<String>,
}

impl AuthenticationMiddleware {
	/// Create a new authentication middleware
	pub fn new(login_required: bool) -> Self {
		Self {
			login_required,
			login_url: None,
		}
	}

	/// Create a new authentication middleware with login URL
	pub fn with_login_url(login_required: bool, login_url: impl Into<String>) -> Self {
		Self {
			login_required,
			login_url: Some(login_url.into()),
		}
	}

	/// Check if the user is authenticated
	///
	/// This is a simplified implementation for demonstration purposes.
	/// In production, integrate with reinhardt-auth for full authentication support.
	fn is_authenticated(&self, request: &Request) -> bool {
		request.headers.get("authorization").is_some()
			|| request.get_language_from_cookie("sessionid").is_some()
	}
}

#[async_trait]
impl ViewSetMiddleware for AuthenticationMiddleware {
	async fn process_request(&self, request: &mut Request) -> Result<Option<Response>> {
		if self.login_required && !self.is_authenticated(request) {
			// Return 401 Unauthorized or redirect to login page
			let response = if let Some(login_url) = &self.login_url {
				// Redirect to login page
				match login_url.parse() {
					Ok(header_value) => {
						let mut response = Response::new(hyper::StatusCode::FOUND);
						response.headers.insert("Location", header_value);
						response.body = "Redirecting to login...".into();
						response
					}
					Err(e) => {
						tracing::warn!(
							login_url = %login_url,
							error = %e,
							"Invalid login_url header value, returning 401 instead of redirect"
						);
						let mut response = Response::new(hyper::StatusCode::UNAUTHORIZED);
						response.body = "Authentication required".into();
						response
					}
				}
			} else {
				// Return 401 Unauthorized
				let mut response = Response::new(hyper::StatusCode::UNAUTHORIZED);
				response.body = "Authentication required".into();
				response
			};

			return Ok(Some(response));
		}

		Ok(None)
	}

	async fn process_response(&self, _request: &Request, response: Response) -> Result<Response> {
		// No response processing needed for authentication middleware
		Ok(response)
	}
}

/// Permission middleware for ViewSets
///
/// Provides permission checking functionality similar to Django's permission system.
#[derive(Debug, Clone)]
pub struct PermissionMiddleware {
	/// Required permissions for this ViewSet
	pub required_permissions: Vec<String>,
}

impl PermissionMiddleware {
	/// Create a new permission middleware
	pub fn new(required_permissions: Vec<String>) -> Self {
		Self {
			required_permissions,
		}
	}

	/// Check if the user has the required permissions
	///
	/// This is a simplified implementation for demonstration purposes.
	/// Returns `false` to demonstrate permission denial behavior.
	/// In production, integrate with reinhardt-auth for full permission checking.
	fn has_permissions(&self, _request: &Request) -> bool {
		false
	}
}

#[async_trait]
impl ViewSetMiddleware for PermissionMiddleware {
	async fn process_request(&self, request: &mut Request) -> Result<Option<Response>> {
		if !self.required_permissions.is_empty() && !self.has_permissions(request) {
			// Return 403 Forbidden
			let mut response = Response::new(hyper::StatusCode::FORBIDDEN);
			response.body = "Permission denied".into();

			return Ok(Some(response));
		}

		Ok(None)
	}

	async fn process_response(&self, _request: &Request, response: Response) -> Result<Response> {
		// No response processing needed for permission middleware
		Ok(response)
	}
}

/// Composite middleware that combines multiple middleware components
pub struct CompositeMiddleware {
	middlewares: Vec<Arc<dyn ViewSetMiddleware>>,
}

impl CompositeMiddleware {
	/// Create a new composite middleware
	pub fn new() -> Self {
		Self {
			middlewares: Vec::new(),
		}
	}

	/// Add a middleware to the composite
	pub fn add_middleware(&mut self, middleware: Arc<dyn ViewSetMiddleware>) {
		self.middlewares.push(middleware);
	}

	/// Add authentication middleware
	pub fn with_authentication(mut self, login_required: bool) -> Self {
		self.middlewares
			.push(Arc::new(AuthenticationMiddleware::new(login_required)));
		self
	}

	/// Add permission middleware
	pub fn with_permissions(mut self, permissions: Vec<String>) -> Self {
		self.middlewares
			.push(Arc::new(PermissionMiddleware::new(permissions)));
		self
	}
}

impl Default for CompositeMiddleware {
	fn default() -> Self {
		Self::new()
	}
}

impl std::fmt::Debug for CompositeMiddleware {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("CompositeMiddleware")
			.field(
				"middlewares",
				&format!("<{} middleware components>", self.middlewares.len()),
			)
			.finish()
	}
}

#[async_trait]
impl ViewSetMiddleware for CompositeMiddleware {
	async fn process_request(&self, request: &mut Request) -> Result<Option<Response>> {
		for middleware in &self.middlewares {
			if let Some(response) = middleware.process_request(request).await? {
				return Ok(Some(response));
			}
		}
		Ok(None)
	}

	async fn process_response(
		&self,
		request: &Request,
		mut response: Response,
	) -> Result<Response> {
		for middleware in &self.middlewares {
			response = middleware.process_response(request, response).await?;
		}
		Ok(response)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use hyper::{HeaderMap, Method, Version};
	use reinhardt_http::Request;

	fn create_test_request() -> Request {
		Request::builder()
			.method(Method::GET)
			.uri("/test/")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(bytes::Bytes::new())
			.build()
			.unwrap()
	}

	#[tokio::test]
	async fn test_authentication_middleware_no_login_required() {
		let middleware = AuthenticationMiddleware::new(false);
		let mut request = create_test_request();

		let result = middleware.process_request(&mut request).await;
		assert!(result.is_ok());
		assert!(result.unwrap().is_none());
	}

	#[tokio::test]
	async fn test_authentication_middleware_login_required_authenticated() {
		let middleware = AuthenticationMiddleware::new(true);
		let mut request = create_test_request();

		// Add authorization header to simulate authenticated user
		request
			.headers
			.insert("authorization", "Bearer token".parse().unwrap());

		let result = middleware.process_request(&mut request).await;
		assert!(result.is_ok());
		assert!(result.unwrap().is_none());
	}

	#[tokio::test]
	async fn test_authentication_middleware_login_required_not_authenticated() {
		let middleware = AuthenticationMiddleware::new(true);
		let mut request = create_test_request();

		let result = middleware.process_request(&mut request).await;
		let response = result.unwrap().unwrap();
		assert_eq!(response.status, hyper::StatusCode::UNAUTHORIZED);
	}

	#[tokio::test]
	async fn test_permission_middleware_no_permissions_required() {
		let middleware = PermissionMiddleware::new(vec![]);
		let mut request = create_test_request();

		let result = middleware.process_request(&mut request).await;
		assert!(result.is_ok());
		assert!(result.unwrap().is_none());
	}

	#[tokio::test]
	async fn test_permission_middleware_permissions_required() {
		let middleware = PermissionMiddleware::new(vec!["read".to_string()]);
		let mut request = create_test_request();

		let result = middleware.process_request(&mut request).await;
		let response = result.unwrap().unwrap();
		assert_eq!(response.status, hyper::StatusCode::FORBIDDEN);
	}

	#[tokio::test]
	async fn test_composite_middleware() {
		let middleware = CompositeMiddleware::new()
			.with_authentication(true)
			.with_permissions(vec!["read".to_string()]);

		let mut request = create_test_request();

		// Should fail authentication first
		let result = middleware.process_request(&mut request).await;
		let response = result.unwrap().unwrap();
		assert_eq!(response.status, hyper::StatusCode::UNAUTHORIZED);
	}
}
