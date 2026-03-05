//! Server Middleware Integration Tests
//!
//! Integration tests for reinhardt-server working with reinhardt-middleware.
//! These tests verify that the server properly integrates with middleware
//! chain functionality, including real HTTP server tests.

use http::StatusCode;
use reinhardt_http::{Handler, Middleware};
use reinhardt_middleware::{LoggingMiddleware, MiddlewareChain};
use reinhardt_test::APIClient;
use reinhardt_test::server::{shutdown_test_server, spawn_test_server};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use reinhardt_http::{Request, Response};

/// Simple test handler for middleware chain tests
struct TestHandler {
	response_body: String,
}

impl TestHandler {
	fn new(body: &str) -> Self {
		Self {
			response_body: body.to_string(),
		}
	}
}

#[async_trait]
impl Handler for TestHandler {
	async fn handle(&self, _request: Request) -> reinhardt_core::exception::Result<Response> {
		use hyper::StatusCode;
		let mut response = Response::new(StatusCode::OK);
		response.body = self.response_body.clone().into();
		Ok(response)
	}
}

/// Middleware that tracks execution order
struct OrderTrackingMiddleware {
	name: String,
	log: Arc<Mutex<Vec<String>>>,
}

impl OrderTrackingMiddleware {
	fn new(name: &str, log: Arc<Mutex<Vec<String>>>) -> Self {
		Self {
			name: name.to_string(),
			log,
		}
	}
}

#[async_trait]
impl Middleware for OrderTrackingMiddleware {
	async fn process(
		&self,
		request: Request,
		next: Arc<dyn Handler>,
	) -> reinhardt_core::exception::Result<Response> {
		// Log before processing
		{
			let mut log = self.log.lock().unwrap();
			log.push(format!("{}:before", self.name));
		}

		// Call next handler
		let response = next.handle(request).await?;

		// Log after processing
		{
			let mut log = self.log.lock().unwrap();
			log.push(format!("{}:after", self.name));
		}

		Ok(response)
	}
}

#[tokio::test]
async fn test_middleware_integration_chain() {
	// Create a simple handler
	let base_handler = Arc::new(TestHandler::new("Response through middleware"));

	// Build middleware chain with logging middleware
	let chain = MiddlewareChain::new(base_handler)
		.with_middleware(Arc::new(LoggingMiddleware::new()) as Arc<dyn Middleware>);

	// Create a test request
	use hyper::{HeaderMap, Method, Version};
	let request = Request::builder()
		.method(Method::GET)
		.uri("/test")
		.version(Version::HTTP_11)
		.headers(HeaderMap::new())
		.body(bytes::Bytes::new())
		.build()
		.unwrap();

	// Execute through the chain
	let response = chain.handle(request).await;

	let response = response.unwrap();
	assert_eq!(response.status, hyper::StatusCode::OK);

	let body = String::from_utf8(response.body.to_vec()).unwrap();
	assert_eq!(body, "Response through middleware");
}

#[tokio::test]
async fn test_server_middleware_integration_multiple() {
	// Test that multiple middlewares can be chained
	let base_handler = Arc::new(TestHandler::new("Multi-middleware response"));

	let chain = MiddlewareChain::new(base_handler)
		.with_middleware(Arc::new(LoggingMiddleware::new()) as Arc<dyn Middleware>)
		.with_middleware(Arc::new(LoggingMiddleware::new()) as Arc<dyn Middleware>);

	use hyper::{HeaderMap, Method, Version};
	let request = Request::builder()
		.method(Method::GET)
		.uri("/test")
		.version(Version::HTTP_11)
		.headers(HeaderMap::new())
		.body(bytes::Bytes::new())
		.build()
		.unwrap();

	let response = chain.handle(request).await;

	let response = response.unwrap();
	assert_eq!(response.status, hyper::StatusCode::OK);
}

/// Test middleware with a real HTTP server
#[tokio::test]
async fn test_middleware_with_real_http_server() {
	// Build middleware chain
	let base_handler = Arc::new(TestHandler::new("Real server response"));
	let chain = MiddlewareChain::new(base_handler)
		.with_middleware(Arc::new(LoggingMiddleware::new()) as Arc<dyn Middleware>);

	// Spawn real HTTP server
	let (url, handle) = spawn_test_server(Arc::new(chain)).await;

	// Give the server a moment to start
	tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

	// Send real HTTP request using APIClient
	let client = APIClient::with_base_url(&url);
	let response = client.get("/test").await.unwrap();

	assert_eq!(response.status(), StatusCode::OK);
	assert_eq!(response.text(), "Real server response");

	// Shutdown server
	shutdown_test_server(handle).await;
}

/// Test middleware execution order with real HTTP server
#[tokio::test]
async fn test_middleware_order_with_real_server() {
	let execution_log = Arc::new(Mutex::new(Vec::<String>::new()));

	// Create order-tracking middlewares
	let middleware1 = OrderTrackingMiddleware::new("first", execution_log.clone());
	let middleware2 = OrderTrackingMiddleware::new("second", execution_log.clone());

	let base_handler = Arc::new(TestHandler::new("OK"));
	let chain = MiddlewareChain::new(base_handler)
		.with_middleware(Arc::new(middleware1) as Arc<dyn Middleware>)
		.with_middleware(Arc::new(middleware2) as Arc<dyn Middleware>);

	// Spawn server
	let (url, handle) = spawn_test_server(Arc::new(chain)).await;

	// Give the server time to start
	tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

	// Send request
	let client = APIClient::with_base_url(&url);
	let response = client.get("/").await.unwrap();

	assert_eq!(response.status(), StatusCode::OK);

	// Verify execution order
	// Middlewares are executed in FIFO order (first added runs first):
	// - first:before -> second:before -> handler -> second:after -> first:after
	// This follows the "onion" pattern where first middleware wraps the second
	let log = execution_log.lock().unwrap();
	assert_eq!(
		*log,
		vec![
			"first:before",
			"second:before",
			"second:after",
			"first:after"
		],
		"Middleware should execute in onion pattern (first added wraps subsequent)"
	);

	shutdown_test_server(handle).await;
}

/// Test multiple concurrent requests through middleware chain
#[tokio::test]
async fn test_concurrent_requests_through_middleware() {
	let base_handler = Arc::new(TestHandler::new("Concurrent response"));
	let chain = MiddlewareChain::new(base_handler)
		.with_middleware(Arc::new(LoggingMiddleware::new()) as Arc<dyn Middleware>);

	let (url, handle) = spawn_test_server(Arc::new(chain)).await;

	// Give server time to start
	tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

	// Send multiple concurrent requests
	let mut handles = Vec::new();

	for i in 0..5 {
		let url = url.clone();
		handles.push(tokio::spawn(async move {
			let client = APIClient::with_base_url(&url);
			let response = client.get(&format!("/request-{}", i)).await.unwrap();
			assert_eq!(response.status(), StatusCode::OK);
			assert_eq!(response.text(), "Concurrent response");
		}));
	}

	// Wait for all requests to complete
	for h in handles {
		h.await.unwrap();
	}

	shutdown_test_server(handle).await;
}

/// Test error handling in middleware chain with real server
#[tokio::test]
async fn test_middleware_error_handling() {
	/// Middleware that can fail based on request path
	struct FailingMiddleware;

	#[async_trait]
	impl Middleware for FailingMiddleware {
		async fn process(
			&self,
			request: Request,
			next: Arc<dyn Handler>,
		) -> reinhardt_core::exception::Result<Response> {
			if request.uri.path().contains("fail") {
				return Err(reinhardt_core::exception::Error::Internal(
					"Intentional failure".to_string(),
				));
			}
			next.handle(request).await
		}
	}

	let base_handler = Arc::new(TestHandler::new("Success"));
	let chain = MiddlewareChain::new(base_handler)
		.with_middleware(Arc::new(FailingMiddleware) as Arc<dyn Middleware>);

	let (url, handle) = spawn_test_server(Arc::new(chain)).await;
	tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

	let client = APIClient::with_base_url(&url);

	// Normal request should succeed
	let response = client.get("/success").await.unwrap();
	assert_eq!(response.status(), StatusCode::OK);

	// Request with "fail" in path should return error (500)
	let response = client.get("/fail").await.unwrap();
	assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);

	shutdown_test_server(handle).await;
}
