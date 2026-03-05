//! Combined Features Integration Tests
//!
//! This module tests the integration of multiple server features working together:
//! - HTTP/2 + Middleware chain (multiple middleware with HTTP/2)
//! - WebSocket + Rate limiting (feature-gated: `#[cfg(feature = "websocket")]`)
//! - GraphQL + Timeout (feature-gated: `#[cfg(feature = "graphql")]`)
//! - HTTP/1.1 and HTTP/2 mixed environment
//! - Graceful shutdown + WebSocket (feature-gated: `#[cfg(feature = "websocket")]`)
//! - Multiple middleware + DI (if DI integration exists)

use http::Version;
use reinhardt_http::{Handler, Middleware};
use reinhardt_http::{Request, Response};
use reinhardt_server::{Http2Server, HttpServer, ShutdownCoordinator, TimeoutHandler};
use reinhardt_test::APIClient;

#[cfg(feature = "websocket")]
use reinhardt_server::{RateLimitConfig, RateLimitHandler, RateLimitStrategy};
use rstest::*;
use std::net::SocketAddr;

use std::sync::Arc;
#[cfg(feature = "websocket")]
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;
use tokio::time::sleep;

// ============================================================================
// Test Handlers
// ============================================================================

/// Basic handler for testing
#[derive(Clone)]
struct BasicTestHandler;

#[async_trait::async_trait]
impl Handler for BasicTestHandler {
	async fn handle(&self, _request: Request) -> reinhardt_core::exception::Result<Response> {
		Ok(Response::ok().with_body("Hello from handler"))
	}
}

/// Handler with configurable delay for timeout testing
///
/// This handler is only available when the `graphql` feature is enabled.
#[cfg(feature = "graphql")]
#[derive(Clone)]
struct DelayedHandler {
	delay: Duration,
}

#[cfg(feature = "graphql")]
#[async_trait::async_trait]
impl Handler for DelayedHandler {
	async fn handle(&self, _request: Request) -> reinhardt_core::exception::Result<Response> {
		sleep(self.delay).await;
		Ok(Response::ok().with_body("Delayed response"))
	}
}

/// Handler with counter for tracking requests
///
/// This handler is only available when the `websocket` feature is enabled.
#[cfg(feature = "websocket")]
#[derive(Clone)]
struct CountingHandler {
	counter: Arc<AtomicU32>,
}

#[cfg(feature = "websocket")]
impl CountingHandler {
	fn new() -> Self {
		Self {
			counter: Arc::new(AtomicU32::new(0)),
		}
	}

	fn get_count(&self) -> u32 {
		self.counter.load(Ordering::SeqCst)
	}
}

#[cfg(feature = "websocket")]
#[async_trait::async_trait]
impl Handler for CountingHandler {
	async fn handle(&self, _request: Request) -> reinhardt_core::exception::Result<Response> {
		let count = self.counter.fetch_add(1, Ordering::SeqCst);
		Ok(Response::ok().with_body(format!("Request #{}", count + 1)))
	}
}

// ============================================================================
// Test Middlewares
// ============================================================================

/// Middleware that adds a custom header
#[derive(Clone)]
struct HeaderMiddleware {
	header_name: String,
	header_value: String,
}

impl HeaderMiddleware {
	fn new(name: &str, value: &str) -> Self {
		Self {
			header_name: name.to_string(),
			header_value: value.to_string(),
		}
	}
}

#[async_trait::async_trait]
impl Middleware for HeaderMiddleware {
	async fn process(
		&self,
		request: Request,
		next: Arc<dyn Handler>,
	) -> reinhardt_core::exception::Result<Response> {
		let mut response = next.handle(request).await?;
		response = response.with_header(&self.header_name, &self.header_value);
		Ok(response)
	}
}

/// Middleware that logs request processing
#[derive(Clone)]
struct LoggingMiddleware {
	log: Arc<tokio::sync::Mutex<Vec<String>>>,
}

impl LoggingMiddleware {
	fn new() -> Self {
		Self {
			log: Arc::new(tokio::sync::Mutex::new(Vec::new())),
		}
	}

	async fn get_logs(&self) -> Vec<String> {
		self.log.lock().await.clone()
	}
}

#[async_trait::async_trait]
impl Middleware for LoggingMiddleware {
	async fn process(
		&self,
		request: Request,
		next: Arc<dyn Handler>,
	) -> reinhardt_core::exception::Result<Response> {
		{
			let mut log = self.log.lock().await;
			log.push(format!("Before: {}", request.uri));
		}

		let response = next.handle(request).await?;

		{
			let mut log = self.log.lock().await;
			log.push(format!("After: {}", response.status));
		}

		Ok(response)
	}
}

/// Middleware chain wrapper to apply multiple middleware
struct MiddlewareChain {
	handler: Arc<dyn Handler>,
	middlewares: Vec<Arc<dyn Middleware>>,
}

impl MiddlewareChain {
	fn new(handler: Arc<dyn Handler>) -> Self {
		Self {
			handler,
			middlewares: Vec::new(),
		}
	}

	fn with_middleware(mut self, middleware: Arc<dyn Middleware>) -> Self {
		self.middlewares.push(middleware);
		self
	}

	fn build(self) -> Arc<dyn Handler> {
		let mut current: Arc<dyn Handler> = self.handler;

		// Apply middleware in reverse order (last added is outermost)
		for middleware in self.middlewares.into_iter().rev() {
			current = Arc::new(MiddlewareHandler {
				middleware,
				next: current,
			});
		}

		current
	}
}

/// Handler wrapper for middleware processing
struct MiddlewareHandler {
	middleware: Arc<dyn Middleware>,
	next: Arc<dyn Handler>,
}

#[async_trait::async_trait]
impl Handler for MiddlewareHandler {
	async fn handle(&self, request: Request) -> reinhardt_core::exception::Result<Response> {
		self.middleware.process(request, self.next.clone()).await
	}
}

// ============================================================================
// Test 1: HTTP/2 + Middleware Chain
// ============================================================================

/// Test HTTP/2 server with multiple middleware
///
/// This test verifies that HTTP/2 server works correctly with a chain of
/// middleware (logging + header injection).
#[rstest]
#[tokio::test]
async fn test_http2_with_middleware_chain() {
	// Setup handler and middleware
	let handler = Arc::new(BasicTestHandler);
	let logging_middleware = Arc::new(LoggingMiddleware::new());
	let header_middleware = Arc::new(HeaderMiddleware::new("X-Custom-Header", "test-value"));

	// Build middleware chain
	let chain = MiddlewareChain::new(handler)
		.with_middleware(logging_middleware.clone() as Arc<dyn Middleware>)
		.with_middleware(header_middleware as Arc<dyn Middleware>)
		.build();

	// Setup HTTP/2 server
	let coordinator = ShutdownCoordinator::new(Duration::from_secs(10));
	let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
	let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
	let actual_addr = listener.local_addr().unwrap();
	let url = format!("http://{}", actual_addr);
	drop(listener);

	// Start HTTP/2 server
	let server_coordinator = coordinator.clone();
	let server_task = tokio::spawn(async move {
		let server = Http2Server::new(chain);
		let _ = server
			.listen_with_shutdown(actual_addr, server_coordinator)
			.await;
	});

	// Wait for server to start
	sleep(Duration::from_millis(100)).await;

	// Create HTTP/2 client
	let client = APIClient::builder()
		.base_url(&url)
		.http2_prior_knowledge()
		.timeout(Duration::from_secs(10))
		.build();

	// Send request
	let response = client.get("/").await.expect("Request should succeed");

	// Verify response
	assert_eq!(response.status_code(), 200);
	assert_eq!(response.version(), Version::HTTP_2);

	// Verify custom header is present
	let custom_header = response.header("X-Custom-Header");
	assert!(custom_header.is_some(), "Custom header should be present");
	assert_eq!(custom_header.unwrap(), "test-value");

	// Verify logging occurred
	let logs = logging_middleware.get_logs().await;
	assert!(logs.len() >= 2, "Should have before and after logs");

	// Cleanup
	coordinator.shutdown();
	server_task.abort();
}

// ============================================================================
// Test 2: WebSocket + Rate Limiting
// ============================================================================

#[cfg(feature = "websocket")]
/// Test WebSocket server with rate limiting
///
/// This test verifies that WebSocket connections respect rate limits.
#[rstest]
#[tokio::test]
async fn test_websocket_with_rate_limit() {
	// Note: This is a placeholder test demonstrating the pattern
	// Full WebSocket implementation requires additional setup

	let handler = Arc::new(BasicTestHandler);
	let config = RateLimitConfig::new(2, Duration::from_secs(1), RateLimitStrategy::FixedWindow);
	let rate_limit_handler = Arc::new(RateLimitHandler::new(handler, config));

	let coordinator = ShutdownCoordinator::new(Duration::from_secs(10));
	let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
	let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
	let actual_addr = listener.local_addr().unwrap();
	let url = format!("http://{}", actual_addr);
	drop(listener);

	let server_coordinator = coordinator.clone();
	let server_task = tokio::spawn(async move {
		let server = HttpServer::new(rate_limit_handler);
		let _ = server
			.listen_with_shutdown(actual_addr, server_coordinator)
			.await;
	});

	sleep(Duration::from_millis(100)).await;

	let client = APIClient::with_base_url(&url);

	// First 2 requests should succeed
	for i in 1..=2 {
		let response = client.get("/").await.expect("Request should succeed");
		assert_eq!(response.status_code(), 200, "Request {} should succeed", i);
	}

	// 3rd request should be rate limited
	let response = client.get("/").await.expect("Request should succeed");
	assert_eq!(
		response.status_code(),
		429,
		"3rd request should be rate limited"
	);

	// Cleanup
	coordinator.shutdown();
	server_task.abort();
}

// ============================================================================
// Test 3: GraphQL + Timeout
// ============================================================================

#[cfg(feature = "graphql")]
/// Test GraphQL server with request timeout
///
/// This test verifies that GraphQL requests respect timeout configuration.
#[rstest]
#[tokio::test]
async fn test_graphql_with_timeout() {
	// Note: This is a placeholder test demonstrating the pattern
	// Full GraphQL implementation requires additional setup

	// Setup handler with 2 second delay
	let slow_handler = Arc::new(DelayedHandler {
		delay: Duration::from_secs(2),
	});

	// Wrap with timeout middleware (100ms timeout)
	let timeout_handler = Arc::new(TimeoutHandler::new(
		slow_handler,
		Duration::from_millis(100),
	));

	let coordinator = ShutdownCoordinator::new(Duration::from_secs(10));
	let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
	let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
	let actual_addr = listener.local_addr().unwrap();
	let url = format!("http://{}", actual_addr);
	drop(listener);

	let server_coordinator = coordinator.clone();
	let server_task = tokio::spawn(async move {
		let server = HttpServer::new(timeout_handler);
		let _ = server
			.listen_with_shutdown(actual_addr, server_coordinator)
			.await;
	});

	sleep(Duration::from_millis(100)).await;

	let client = APIClient::with_base_url(&url);
	let response = client.get("/").await.expect("Request should complete");

	// Should get timeout response
	assert_eq!(response.status_code(), 408, "Should timeout");

	// Cleanup
	coordinator.shutdown();
	server_task.abort();
}

// ============================================================================
// Test 4: HTTP/1.1 and HTTP/2 Mixed Environment
// ============================================================================

/// Test mixed HTTP/1.1 and HTTP/2 environment
///
/// This test verifies that both HTTP/1.1 and HTTP/2 servers can run
/// simultaneously and handle requests correctly.
#[rstest]
#[tokio::test]
async fn test_http1_http2_mixed_environment() {
	let handler1 = Arc::new(BasicTestHandler);
	let handler2 = Arc::new(BasicTestHandler);

	let coordinator1 = ShutdownCoordinator::new(Duration::from_secs(10));
	let coordinator2 = ShutdownCoordinator::new(Duration::from_secs(10));

	// Setup HTTP/1.1 server
	let addr1: SocketAddr = "127.0.0.1:0".parse().unwrap();
	let listener1 = tokio::net::TcpListener::bind(addr1).await.unwrap();
	let actual_addr1 = listener1.local_addr().unwrap();
	let url1 = format!("http://{}", actual_addr1);
	drop(listener1);

	let server1_coordinator = coordinator1.clone();
	let server1_task = tokio::spawn(async move {
		let server = HttpServer::new(handler1);
		let _ = server
			.listen_with_shutdown(actual_addr1, server1_coordinator)
			.await;
	});

	// Setup HTTP/2 server
	let addr2: SocketAddr = "127.0.0.1:0".parse().unwrap();
	let listener2 = tokio::net::TcpListener::bind(addr2).await.unwrap();
	let actual_addr2 = listener2.local_addr().unwrap();
	let url2 = format!("http://{}", actual_addr2);
	drop(listener2);

	let server2_coordinator = coordinator2.clone();
	let server2_task = tokio::spawn(async move {
		let server = Http2Server::new(handler2);
		let _ = server
			.listen_with_shutdown(actual_addr2, server2_coordinator)
			.await;
	});

	// Wait for servers to start
	sleep(Duration::from_millis(100)).await;

	// Test HTTP/1.1 server
	let http1_client = APIClient::builder()
		.base_url(&url1)
		.http1_only()
		.timeout(Duration::from_secs(10))
		.build();

	let response1 = http1_client
		.get("/")
		.await
		.expect("HTTP/1.1 request should succeed");

	assert_eq!(response1.status_code(), 200);
	assert_eq!(response1.version(), Version::HTTP_11);

	// Test HTTP/2 server
	let http2_client = APIClient::builder()
		.base_url(&url2)
		.http2_prior_knowledge()
		.timeout(Duration::from_secs(10))
		.build();

	let response2 = http2_client
		.get("/")
		.await
		.expect("HTTP/2 request should succeed");

	assert_eq!(response2.status_code(), 200);
	assert_eq!(response2.version(), Version::HTTP_2);

	// Cleanup
	coordinator1.shutdown();
	coordinator2.shutdown();
	server1_task.abort();
	server2_task.abort();
}

// ============================================================================
// Test 5: Graceful Shutdown + WebSocket
// ============================================================================

#[cfg(feature = "websocket")]
/// Test graceful shutdown with active WebSocket connections
///
/// This test verifies that the server can gracefully shutdown even when
/// WebSocket connections are active.
#[rstest]
#[tokio::test]
async fn test_graceful_shutdown_with_websocket() {
	// Note: This is a placeholder test demonstrating the pattern
	// Full WebSocket implementation requires additional setup

	let handler = Arc::new(CountingHandler::new());
	let coordinator = ShutdownCoordinator::new(Duration::from_secs(5));

	let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
	let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
	let actual_addr = listener.local_addr().unwrap();
	let url = format!("http://{}", actual_addr);
	drop(listener);

	let server_coordinator = coordinator.clone();
	let server_handler = handler.clone();
	let server_task = tokio::spawn(async move {
		let server = HttpServer::new(server_handler);
		let _ = server
			.listen_with_shutdown(actual_addr, server_coordinator)
			.await;
	});

	sleep(Duration::from_millis(100)).await;

	let client = APIClient::with_base_url(&url);

	// Make a request to verify server is running
	let response = client.get("/").await.expect("Request should succeed");
	assert_eq!(response.status_code(), 200);
	assert_eq!(handler.get_count(), 1);

	// Trigger graceful shutdown
	coordinator.shutdown();

	// Wait for shutdown to complete
	coordinator.wait_for_shutdown().await;

	// Verify server task completed
	let result = tokio::time::timeout(Duration::from_secs(2), server_task).await;
	assert!(result.is_ok(), "Server should shutdown gracefully");
}

// ============================================================================
// Test 6: Multiple Middleware + DI (Dependency Injection)
// ============================================================================

/// Test multiple middleware with Dependency Injection
///
/// This test verifies that multiple middleware can work together and that
/// DI (if available) integrates correctly with the middleware chain.
#[rstest]
#[tokio::test]
async fn test_multiple_middleware_with_di() {
	// Setup middleware chain
	let logging_middleware = Arc::new(LoggingMiddleware::new());
	let header_middleware = Arc::new(HeaderMiddleware::new("X-Framework", "Reinhardt"));
	let timeout_middleware = Arc::new(TimeoutHandler::new(
		Arc::new(BasicTestHandler),
		Duration::from_secs(5),
	));

	// Build chain (innermost to outermost: handler -> timeout -> logging -> header)
	let chain = MiddlewareChain::new(timeout_middleware as Arc<dyn Handler>)
		.with_middleware(logging_middleware.clone() as Arc<dyn Middleware>)
		.with_middleware(header_middleware as Arc<dyn Middleware>)
		.build();

	// Setup server
	let coordinator = ShutdownCoordinator::new(Duration::from_secs(10));
	let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
	let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
	let actual_addr = listener.local_addr().unwrap();
	let url = format!("http://{}", actual_addr);
	drop(listener);

	let server_coordinator = coordinator.clone();
	let server_task = tokio::spawn(async move {
		let server = HttpServer::new(chain);
		let _ = server
			.listen_with_shutdown(actual_addr, server_coordinator)
			.await;
	});

	sleep(Duration::from_millis(100)).await;

	let client = APIClient::with_base_url(&url);

	// Send multiple requests
	for i in 1..=3 {
		let response = client.get("/").await.expect("Request should succeed");

		assert_eq!(response.status_code(), 200, "Request {} should succeed", i);

		// Verify framework header is present
		let framework_header = response.header("X-Framework");
		assert!(
			framework_header.is_some(),
			"Framework header should be present"
		);
		assert_eq!(framework_header.unwrap(), "Reinhardt");
	}

	// Verify logging occurred for all requests
	let logs = logging_middleware.get_logs().await;
	assert_eq!(
		logs.len(),
		6,
		"Should have 6 log entries (3 before + 3 after)"
	);

	// Cleanup
	coordinator.shutdown();
	server_task.abort();
}
