//! Middleware Error Handling Integration Tests
//!
//! Tests for middleware error handling, error propagation, request/response
//! transformation errors, and custom error responses.

use async_trait::async_trait;
use http::StatusCode;
use reinhardt_core::exception::{Error, Result};
use reinhardt_http::{Handler, Middleware, Request, Response};
use reinhardt_test::APIClient;
use reinhardt_test::fixtures::*;
use rstest::*;
use std::sync::Arc;

// ============================================================================
// Test Middlewares
// ============================================================================

/// Middleware that always throws an exception for testing error handling
struct ErrorThrowingMiddleware;

#[async_trait]
impl Middleware for ErrorThrowingMiddleware {
	async fn process(&self, _request: Request, _next: Arc<dyn Handler>) -> Result<Response> {
		Err(Error::Internal("Middleware error occurred".to_string()))
	}
}

/// Middleware that propagates errors from downstream handlers
struct ErrorPropagationMiddleware {
	add_context: bool,
}

#[async_trait]
impl Middleware for ErrorPropagationMiddleware {
	async fn process(&self, request: Request, next: Arc<dyn Handler>) -> Result<Response> {
		match next.handle(request).await {
			Ok(response) => Ok(response),
			Err(e) => {
				if self.add_context {
					// Add context to error
					Err(Error::Internal(format!(
						"ErrorPropagationMiddleware: {}",
						e
					)))
				} else {
					// Propagate error as-is
					Err(e)
				}
			}
		}
	}
}

/// Middleware that fails during request transformation
struct RequestTransformErrorMiddleware;

#[async_trait]
impl Middleware for RequestTransformErrorMiddleware {
	async fn process(&self, request: Request, _next: Arc<dyn Handler>) -> Result<Response> {
		// Simulate request transformation error (e.g., invalid header parsing)
		if request.uri.path().contains("invalid") {
			return Err(Error::Http(
				"Invalid request format during transformation".to_string(),
			));
		}
		Err(Error::Internal(
			"Request transformation not implemented".to_string(),
		))
	}
}

/// Middleware that fails during response transformation
struct ResponseTransformErrorMiddleware;

#[async_trait]
impl Middleware for ResponseTransformErrorMiddleware {
	async fn process(&self, request: Request, next: Arc<dyn Handler>) -> Result<Response> {
		let _response = next.handle(request).await?;

		// Simulate response transformation error (e.g., compression failure)
		Err(Error::Internal(
			"Response transformation failed".to_string(),
		))
	}
}

/// Middleware that transforms errors into custom error responses
struct CustomErrorResponseMiddleware;

#[async_trait]
impl Middleware for CustomErrorResponseMiddleware {
	async fn process(&self, request: Request, next: Arc<dyn Handler>) -> Result<Response> {
		match next.handle(request).await {
			Ok(response) => Ok(response),
			Err(e) => {
				// Transform error into custom JSON response
				let error_json =
					format!(r#"{{"error":"{}","type":"custom_error"}}"#, e.to_string());
				Ok(Response::internal_server_error()
					.with_body(error_json)
					.with_header("Content-Type", "application/json"))
			}
		}
	}
}

/// Handler that always returns an error
struct ErrorHandler;

#[async_trait]
impl Handler for ErrorHandler {
	async fn handle(&self, _request: Request) -> Result<Response> {
		Err(Error::Internal("Handler error".to_string()))
	}
}

// ============================================================================
// Tests
// ============================================================================

/// Test middleware exception handling
///
/// Verifies that errors thrown by middleware are properly handled and
/// returned as error responses to the client.
#[rstest]
#[tokio::test]
async fn test_middleware_exception_handling(#[future] http1_server: TestServer) {
	let server = http1_server.await;

	// Create server with error-throwing middleware
	let handler = Arc::new(BasicHandler);
	let middleware = Arc::new(ErrorThrowingMiddleware);

	let chain = reinhardt_http::MiddlewareChain::new(handler).with_middleware(middleware);

	// Start server with error middleware
	let test_server = TestServer::builder()
		.handler(Arc::new(chain))
		.build()
		.await
		.expect("Failed to create test server");

	let client = APIClient::with_base_url(&test_server.url);
	let response = client.get("/test").await.expect("Failed to send request");

	// Should return 500 Internal Server Error
	assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);

	// Cleanup
	drop(test_server);
	drop(server);
}

/// Test error propagation through middleware chain
///
/// Verifies that errors from handlers propagate correctly through the
/// middleware chain, and that middleware can add context to errors.
#[rstest]
#[tokio::test]
async fn test_error_propagation_through_chain(#[future] http1_server: TestServer) {
	let server = http1_server.await;

	// Create handler that returns error
	let handler = Arc::new(ErrorHandler);

	// Create middleware chain with error propagation
	let middleware1 = Arc::new(ErrorPropagationMiddleware { add_context: true });
	let middleware2 = Arc::new(ErrorPropagationMiddleware { add_context: true });

	let chain = reinhardt_http::MiddlewareChain::new(handler)
		.with_middleware(middleware1)
		.with_middleware(middleware2);

	let test_server = TestServer::builder()
		.handler(Arc::new(chain))
		.build()
		.await
		.expect("Failed to create test server");

	let client = APIClient::with_base_url(&test_server.url);
	let response = client.get("/test").await.expect("Failed to send request");

	// Error should propagate and return 500
	assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);

	// Cleanup
	drop(test_server);
	drop(server);
}

/// Test request transformation error
///
/// Verifies that errors occurring during request transformation (e.g., parsing
/// headers, validating request format) are properly handled.
#[rstest]
#[tokio::test]
async fn test_request_transformation_error(#[future] http1_server: TestServer) {
	let server = http1_server.await;

	let handler = Arc::new(BasicHandler);
	let middleware = Arc::new(RequestTransformErrorMiddleware);

	let chain = reinhardt_http::MiddlewareChain::new(handler).with_middleware(middleware);

	let test_server = TestServer::builder()
		.handler(Arc::new(chain))
		.build()
		.await
		.expect("Failed to create test server");

	let client = APIClient::with_base_url(&test_server.url);

	// Request with "invalid" in path should trigger BadRequest error
	let response = client
		.get("/invalid-request")
		.await
		.expect("Failed to send request");

	// Should return 400 Bad Request
	assert_eq!(response.status(), StatusCode::BAD_REQUEST);

	// Cleanup
	drop(test_server);
	drop(server);
}

/// Test response transformation error
///
/// Verifies that errors occurring during response transformation (e.g., compression
/// failure, serialization error) are properly handled.
#[rstest]
#[tokio::test]
async fn test_response_transformation_error(#[future] http1_server: TestServer) {
	let server = http1_server.await;

	let handler = Arc::new(BasicHandler);
	let middleware = Arc::new(ResponseTransformErrorMiddleware);

	let chain = reinhardt_http::MiddlewareChain::new(handler).with_middleware(middleware);

	let test_server = TestServer::builder()
		.handler(Arc::new(chain))
		.build()
		.await
		.expect("Failed to create test server");

	let client = APIClient::with_base_url(&test_server.url);
	let response = client.get("/test").await.expect("Failed to send request");

	// Response transformation error should return 500
	assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);

	// Cleanup
	drop(test_server);
	drop(server);
}

/// Test custom error response
///
/// Verifies that middleware can transform errors into custom error responses
/// (e.g., JSON error format, custom headers).
#[rstest]
#[tokio::test]
async fn test_custom_error_response(#[future] http1_server: TestServer) {
	let server = http1_server.await;

	// Create handler that returns error
	let handler = Arc::new(ErrorHandler);

	// Wrap with custom error response middleware
	let middleware = Arc::new(CustomErrorResponseMiddleware);

	let chain = reinhardt_http::MiddlewareChain::new(handler).with_middleware(middleware);

	let test_server = TestServer::builder()
		.handler(Arc::new(chain))
		.build()
		.await
		.expect("Failed to create test server");

	let client = APIClient::with_base_url(&test_server.url);
	let response = client.get("/test").await.expect("Failed to send request");

	// Should return 500 with custom error response
	assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);

	// Verify Content-Type is JSON
	let content_type = response
		.headers()
		.get("content-type")
		.expect("Content-Type header missing");
	assert_eq!(content_type, "application/json");

	// Verify custom error format
	let body = response.text();
	assert!(body.contains(r#""type":"custom_error""#));
	assert!(body.contains(r#""error":"#));

	// Cleanup
	drop(test_server);
	drop(server);
}
