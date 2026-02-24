//! Base HTTP request handler
//!
//! This module provides the base handler for processing HTTP requests,
//! similar to Django's `django.core.handlers.base.BaseHandler`.

use hyper::StatusCode;
use reinhardt_core::signals::{
	RequestFinishedEvent, RequestStartedEvent, request_finished, request_started,
};
use reinhardt_http::Handler;
use reinhardt_http::{Request, Response};
use reinhardt_urls::routers::DefaultRouter;
use std::sync::Arc;
use tracing::{debug, error, trace, warn};

use crate::DispatchError;

/// Base HTTP request handler
///
/// Handles the complete request lifecycle including URL resolution,
/// view execution, and signal emission.
pub struct BaseHandler {
	/// Whether the handler operates in async mode.
	///
	/// This flag mirrors Django's `BaseHandler._is_async` and is read by
	/// `Dispatcher` to choose between sync and async code paths. When
	/// `false`, async dispatch still works but callers may opt for a
	/// blocking wrapper.
	#[allow(dead_code)] // read via is_async() accessor; behavioral branching planned
	is_async: bool,
	router: Option<Arc<DefaultRouter>>,
}

impl BaseHandler {
	/// Create a new base handler
	pub fn new() -> Self {
		Self {
			is_async: true,
			router: None,
		}
	}

	/// Create a handler with a router
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_dispatch::BaseHandler;
	/// use reinhardt_urls::routers::DefaultRouter;
	/// use std::sync::Arc;
	///
	/// let router = DefaultRouter::new();
	/// let handler = BaseHandler::with_router(Arc::new(router));
	/// assert!(handler.is_async());
	/// ```
	pub fn with_router(router: Arc<DefaultRouter>) -> Self {
		Self {
			is_async: true,
			router: Some(router),
		}
	}

	/// Handle an HTTP request
	///
	/// This is the main entry point for request processing. It:
	/// 1. Emits `request_started` signal
	/// 2. Resolves URL and dispatches to view
	/// 3. Emits `request_finished` signal
	pub async fn handle_request(
		&self,
		request: Request,
	) -> std::result::Result<Response, DispatchError> {
		trace!("Handling request: {:?}", request.uri);

		// Emit request_started signal
		let event = RequestStartedEvent::new();
		if let Err(e) = request_started().send(event).await {
			warn!("Failed to send request_started signal: {}", e);
		}

		// Get response with router
		let response = Self::get_response_async(request, self.router.as_ref()).await;

		// Emit request_finished signal
		let event = RequestFinishedEvent::new();
		if let Err(e) = request_finished().send(event).await {
			warn!("Failed to send request_finished signal: {}", e);
		}

		response
	}

	/// Get response for a request (async version) with URL resolution
	///
	/// This is the core request processing logic that:
	/// - Resolves the URL using the router
	/// - Dispatches to the matched handler
	/// - Returns a 404 response if no route matches
	/// - Returns error for handler errors (will be converted to 500 by Handler trait)
	async fn get_response_async(
		request: Request,
		router: Option<&Arc<DefaultRouter>>,
	) -> std::result::Result<Response, DispatchError> {
		debug!("Getting response for: {}", request.uri.path());

		// URL resolution with router
		if let Some(router) = router {
			trace!("Attempting to route request through router");

			// Use the router to handle the request
			match router.handle(request).await {
				Ok(response) => {
					trace!("Route handled successfully");
					return Ok(response);
				}
				Err(reinhardt_core::exception::Error::NotFound(msg)) => {
					debug!("No route matched: {}", msg);
					return Ok(Response::new(StatusCode::NOT_FOUND));
				}
				Err(e) => {
					error!("Handler error: {}", e);
					// Return error to allow middleware chain to handle it
					return Err(DispatchError::View(e.to_string()));
				}
			}
		}

		// Fallback: router not configured, return 404 since no routes can match
		debug!("No router configured, returning 404 Not Found");
		Ok(Response::new(StatusCode::NOT_FOUND))
	}

	/// Process an exception and convert it to a response.
	///
	/// Error details are logged server-side but not included in the response
	/// body to prevent information disclosure.
	pub async fn handle_exception(&self, _request: &Request, error: DispatchError) -> Response {
		error!("Handling exception: {}", error);

		crate::build_error_response(StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error")
	}

	/// Check if handler is configured for async mode
	pub fn is_async(&self) -> bool {
		self.is_async
	}

	/// Set async mode for the handler
	pub fn set_async(&mut self, is_async: bool) {
		self.is_async = is_async;
	}
}

impl Default for BaseHandler {
	fn default() -> Self {
		Self::new()
	}
}

#[async_trait::async_trait]
impl Handler for BaseHandler {
	async fn handle(&self, request: Request) -> reinhardt_core::exception::Result<Response> {
		match self.handle_request(request).await {
			Ok(response) => Ok(response),
			Err(e) => {
				// Log the detailed error server-side; return generic message to client
				error!("Handler error in BaseHandler::handle: {}", e);
				Ok(crate::build_error_response(
					StatusCode::INTERNAL_SERVER_ERROR,
					"Internal Server Error",
				))
			}
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use async_trait::async_trait;
	use bytes::Bytes;
	use hyper::{HeaderMap, Method, Version};
	use reinhardt_urls::routers::{DefaultRouter, Router, path};

	// Test handler for routing tests
	struct TestHandler {
		response_body: String,
	}

	#[async_trait]
	impl Handler for TestHandler {
		async fn handle(&self, _req: Request) -> reinhardt_core::exception::Result<Response> {
			Ok(Response::ok().with_body(self.response_body.clone()))
		}
	}

	#[tokio::test]
	async fn test_base_handler_new() {
		let handler = BaseHandler::new();
		assert!(handler.is_async());
	}

	#[tokio::test]
	async fn test_base_handler_handle_request() {
		let handler = BaseHandler::new();
		let request = Request::builder()
			.method(Method::GET)
			.uri("/")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = handler.handle_request(request).await;
		let resp = response.unwrap();
		// Handler without router should return 404 Not Found
		assert_eq!(resp.status, StatusCode::NOT_FOUND);
	}

	#[tokio::test]
	async fn test_base_handler_handle_exception() {
		let handler = BaseHandler::new();
		let request = Request::builder()
			.method(Method::GET)
			.uri("/")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();
		let error = DispatchError::View("Test error".to_string());

		let response = handler.handle_exception(&request, error).await;
		assert_eq!(response.status, StatusCode::INTERNAL_SERVER_ERROR);
	}

	// ==========================================================================
	// Information Disclosure Prevention Tests (#439)
	// ==========================================================================

	#[tokio::test]
	async fn test_handle_exception_does_not_expose_internal_details() {
		// Arrange
		let handler = BaseHandler::new();
		let request = Request::builder()
			.method(Method::GET)
			.uri("/")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();
		let sensitive_detail = "database connection refused at postgres://admin:secret@db:5432";
		let error = DispatchError::Internal(sensitive_detail.to_string());

		// Act
		let response = handler.handle_exception(&request, error).await;

		// Assert: response must not contain the sensitive detail
		let body = String::from_utf8(response.body.to_vec()).unwrap();
		assert_eq!(response.status, StatusCode::INTERNAL_SERVER_ERROR);
		assert!(!body.contains("database"));
		assert!(!body.contains("postgres"));
		assert!(!body.contains("secret"));
		assert_eq!(body, "Internal Server Error");
	}

	#[tokio::test]
	async fn test_handler_impl_does_not_expose_error_in_body() {
		// Arrange: create a handler that returns a view error with internal paths
		struct FailingHandler;

		#[async_trait]
		impl Handler for FailingHandler {
			async fn handle(&self, _req: Request) -> reinhardt_core::exception::Result<Response> {
				Err(reinhardt_core::exception::Error::Internal(
					"module::secret_handler panicked at /src/app/handlers.rs:42".to_string(),
				))
			}
		}

		let mut router = DefaultRouter::new();
		let failing = Arc::new(FailingHandler);
		let route = path("/fail", failing).with_name("fail");
		router.add_route(route);
		let handler = BaseHandler::with_router(Arc::new(router));

		let request = Request::builder()
			.method(Method::GET)
			.uri("/fail")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		// Act
		let response = handler.handle(request).await.unwrap();

		// Assert: internal details must not leak
		let body = String::from_utf8(response.body.to_vec()).unwrap();
		assert_eq!(response.status, StatusCode::INTERNAL_SERVER_ERROR);
		assert!(!body.contains("panicked"));
		assert!(!body.contains("handlers.rs"));
		assert!(!body.contains("secret_handler"));
		assert_eq!(body, "Internal Server Error");
	}

	#[test]
	fn test_base_handler_async_mode() {
		let mut handler = BaseHandler::new();
		assert!(handler.is_async());

		handler.set_async(false);
		assert!(!handler.is_async());
	}

	#[tokio::test]
	async fn test_base_handler_different_methods() {
		let handler = BaseHandler::new();

		for method in [Method::GET, Method::POST, Method::PUT, Method::DELETE] {
			let request = Request::builder()
				.method(method)
				.uri("/")
				.version(Version::HTTP_11)
				.headers(HeaderMap::new())
				.body(Bytes::new())
				.build()
				.unwrap();

			let response = handler.handle_request(request).await;
			assert!(response.is_ok());
		}
	}

	#[tokio::test]
	async fn test_base_handler_different_uris() {
		let handler = BaseHandler::new();

		for path in ["/", "/test", "/api/v1/users", "/admin/login"] {
			let request = Request::builder()
				.method(Method::GET)
				.uri(path)
				.version(Version::HTTP_11)
				.headers(HeaderMap::new())
				.body(Bytes::new())
				.build()
				.unwrap();

			let response = handler.handle_request(request).await;
			assert!(response.is_ok());
		}
	}

	#[tokio::test]
	async fn test_handler_with_router() {
		// Create a router with a test route
		let mut router = DefaultRouter::new();
		let test_handler = Arc::new(TestHandler {
			response_body: "Test response".to_string(),
		});
		let route = path("/test", test_handler).with_name("test");
		router.add_route(route);

		// Create BaseHandler with router
		let handler = BaseHandler::with_router(Arc::new(router));

		// Test matching route
		let request = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = handler.handle_request(request).await;
		let resp = response.unwrap();
		assert_eq!(resp.status, StatusCode::OK);

		let body = String::from_utf8(resp.body.to_vec()).unwrap();
		assert_eq!(body, "Test response");
	}

	#[tokio::test]
	async fn test_handler_404_not_found() {
		// Create empty router
		let router = DefaultRouter::new();

		// Create BaseHandler with router
		let handler = BaseHandler::with_router(Arc::new(router));

		// Test non-existent route
		let request = Request::builder()
			.method(Method::GET)
			.uri("/nonexistent")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = handler.handle_request(request).await;
		let resp = response.unwrap();
		assert_eq!(resp.status, StatusCode::NOT_FOUND);
	}

	#[tokio::test]
	async fn test_handler_multiple_routes() {
		// Create router with multiple routes
		let mut router = DefaultRouter::new();

		let hello_handler = Arc::new(TestHandler {
			response_body: "Hello".to_string(),
		});
		let hello_route = path("/hello", hello_handler).with_name("hello");
		router.add_route(hello_route);

		let world_handler = Arc::new(TestHandler {
			response_body: "World".to_string(),
		});
		let world_route = path("/world", world_handler).with_name("world");
		router.add_route(world_route);

		let handler = BaseHandler::with_router(Arc::new(router));

		// Test first route
		let request = Request::builder()
			.method(Method::GET)
			.uri("/hello")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();
		let response = handler.handle_request(request).await.unwrap();
		assert_eq!(response.status, StatusCode::OK);
		assert_eq!(String::from_utf8(response.body.to_vec()).unwrap(), "Hello");

		// Test second route
		let request = Request::builder()
			.method(Method::GET)
			.uri("/world")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();
		let response = handler.handle_request(request).await.unwrap();
		assert_eq!(response.status, StatusCode::OK);
		assert_eq!(String::from_utf8(response.body.to_vec()).unwrap(), "World");
	}
}
