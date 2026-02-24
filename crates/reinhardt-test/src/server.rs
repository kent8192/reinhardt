//! HTTP server test utilities
//!
//! This module provides utilities for testing HTTP servers, including
//! spawning test servers and various test handler implementations.

use reinhardt_http::Handler;
use reinhardt_http::{Request, Response};
use reinhardt_server::HttpServer;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::task::JoinHandle;

/// Spawns a test server on a random available port
///
/// # Arguments
///
/// * `handler` - The handler to use for the test server
///
/// # Returns
///
/// Returns a tuple containing:
/// * The server URL (e.g., "http://127.0.0.1:12345")
/// * A JoinHandle to the running server task
///
/// # Example
///
/// ```no_run
/// use reinhardt_test::server::{spawn_test_server, EchoPathHandler};
/// use std::sync::Arc;
///
/// # async fn example() {
/// let handler = Arc::new(EchoPathHandler);
/// let (url, handle) = spawn_test_server(handler).await;
/// println!("Server running at: {}", url);
/// # }
/// ```
pub async fn spawn_test_server(handler: Arc<dyn Handler>) -> (String, JoinHandle<()>) {
	// Bind to port 0 to get a random available port
	let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
	let addr = listener.local_addr().unwrap();
	let url = format!("http://{}", addr);

	// Create server
	let server = HttpServer::new(handler);

	// Spawn server in background task
	let handle = tokio::spawn(async move {
		// Accept connections manually since we need to use our existing listener
		loop {
			match listener.accept().await {
				Ok((stream, socket_addr)) => {
					let handler_clone = server.handler();
					tokio::spawn(async move {
						if let Err(e) =
							HttpServer::handle_connection(stream, socket_addr, handler_clone, None)
								.await
						{
							eprintln!("Error handling connection: {:?}", e);
						}
					});
				}
				Err(e) => {
					eprintln!("Error accepting connection: {:?}", e);
					break;
				}
			}
		}
	});

	// Give the server a moment to start

	(url, handle)
}

/// Shuts down a test server gracefully
///
/// # Arguments
///
/// * `handle` - The JoinHandle returned by [`spawn_test_server`]
///
/// # Example
///
/// ```no_run
/// use reinhardt_test::server::{spawn_test_server, shutdown_test_server, EchoPathHandler};
/// use std::sync::Arc;
///
/// # async fn example() {
/// let handler = Arc::new(EchoPathHandler);
/// let (url, handle) = spawn_test_server(handler).await;
/// // ... perform tests ...
/// shutdown_test_server(handle).await;
/// # }
/// ```
pub async fn shutdown_test_server(handle: JoinHandle<()>) {
	handle.abort();
	// Give it a moment to clean up
}

/// Simple test handler that echoes the request path
///
/// Returns the request path as the response body.
///
/// # Example
///
/// ```no_run
/// use reinhardt_test::server::{spawn_test_server, EchoPathHandler};
/// use std::sync::Arc;
///
/// # async fn example() {
/// let handler = Arc::new(EchoPathHandler);
/// let (url, handle) = spawn_test_server(handler).await;
/// // A request to "/test/path" will return "test/path"
/// # }
/// ```
pub struct EchoPathHandler;

#[async_trait::async_trait]
impl Handler for EchoPathHandler {
	async fn handle(&self, request: Request) -> reinhardt_core::exception::Result<Response> {
		let path = request.path().to_string();
		Ok(Response::ok().with_body(path))
	}
}

/// Test handler that returns specific status codes based on path
///
/// Responds with different HTTP status codes based on the request path:
/// * `/200` - Returns 200 OK
/// * `/404` - Returns 404 Not Found
/// * `/500` - Returns 500 Internal Server Error
/// * Other paths - Returns 200 OK with "Default" body
///
/// # Example
///
/// ```no_run
/// use reinhardt_test::server::{spawn_test_server, StatusCodeHandler};
/// use std::sync::Arc;
///
/// # async fn example() {
/// let handler = Arc::new(StatusCodeHandler);
/// let (url, handle) = spawn_test_server(handler).await;
/// // A request to "/404" will return 404 Not Found
/// # }
/// ```
pub struct StatusCodeHandler;

#[async_trait::async_trait]
impl Handler for StatusCodeHandler {
	async fn handle(&self, request: Request) -> reinhardt_core::exception::Result<Response> {
		match request.path() {
			"/200" => Ok(Response::ok().with_body("OK")),
			"/404" => Ok(Response::not_found().with_body("Not Found")),
			"/500" => Ok(Response::internal_server_error().with_body("Internal Server Error")),
			_ => Ok(Response::ok().with_body("Default")),
		}
	}
}

/// Test handler that echoes the request method
///
/// Returns the HTTP method (GET, POST, etc.) as the response body.
///
/// # Example
///
/// ```no_run
/// use reinhardt_test::server::{spawn_test_server, MethodEchoHandler};
/// use std::sync::Arc;
///
/// # async fn example() {
/// let handler = Arc::new(MethodEchoHandler);
/// let (url, handle) = spawn_test_server(handler).await;
/// // A GET request will return "GET" as the response body
/// # }
/// ```
pub struct MethodEchoHandler;

#[async_trait::async_trait]
impl Handler for MethodEchoHandler {
	async fn handle(&self, request: Request) -> reinhardt_core::exception::Result<Response> {
		let method = request.method.as_str().to_string();
		Ok(Response::ok().with_body(method))
	}
}

/// Test handler with configurable delay
///
/// Useful for testing timeouts and async behavior. The handler
/// waits for a specified duration before returning a response.
///
/// # Fields
///
/// * `delay_ms` - Delay in milliseconds before responding
/// * `response_body` - The body to return in the response
///
/// # Example
///
/// ```no_run
/// use reinhardt_test::server::{spawn_test_server, DelayedHandler};
/// use std::sync::Arc;
///
/// # async fn example() {
/// let handler = Arc::new(DelayedHandler {
///     delay_ms: 100,
///     response_body: "Delayed response".to_string(),
/// });
/// let (url, handle) = spawn_test_server(handler).await;
/// // Responses will be delayed by 100ms
/// # }
/// ```
pub struct DelayedHandler {
	pub delay_ms: u64,
	pub response_body: String,
}

#[async_trait::async_trait]
impl Handler for DelayedHandler {
	async fn handle(&self, _request: Request) -> reinhardt_core::exception::Result<Response> {
		tokio::time::sleep(std::time::Duration::from_millis(self.delay_ms)).await;
		Ok(Response::ok().with_body(self.response_body.clone()))
	}
}

/// Test handler that echoes the request body
///
/// Returns the request body as the response body.
///
/// # Example
///
/// ```no_run
/// use reinhardt_test::server::{spawn_test_server, BodyEchoHandler};
/// use std::sync::Arc;
///
/// # async fn example() {
/// let handler = Arc::new(BodyEchoHandler);
/// let (url, handle) = spawn_test_server(handler).await;
/// // A POST request with body "test data" will return "test data"
/// # }
/// ```
pub struct BodyEchoHandler;

#[async_trait::async_trait]
impl Handler for BodyEchoHandler {
	async fn handle(&self, request: Request) -> reinhardt_core::exception::Result<Response> {
		let body = request.read_body()?;
		Ok(Response::ok().with_body(body))
	}
}

/// Test handler that returns a large response
///
/// Useful for testing response size limits and memory handling.
///
/// # Fields
///
/// * `size_kb` - Size of the response in kilobytes
///
/// # Example
///
/// ```no_run
/// use reinhardt_test::server::{spawn_test_server, LargeResponseHandler};
/// use std::sync::Arc;
///
/// # async fn example() {
/// let handler = Arc::new(LargeResponseHandler {
///     size_kb: 1024, // 1MB response
/// });
/// let (url, handle) = spawn_test_server(handler).await;
/// // Responses will be 1MB of repeated 'x' characters
/// # }
/// ```
pub struct LargeResponseHandler {
	pub size_kb: usize,
}

#[async_trait::async_trait]
impl Handler for LargeResponseHandler {
	async fn handle(&self, _request: Request) -> reinhardt_core::exception::Result<Response> {
		let data = "x".repeat(self.size_kb * 1024);
		Ok(Response::ok().with_body(data))
	}
}

/// Test handler that returns different responses based on path
///
/// A simple router-like handler for testing routing behavior:
/// * `/` - Returns "Home"
/// * `/api` - Returns JSON `{"status": "ok"}`
/// * `/notfound` - Returns 404 with "Not Found"
/// * Other paths - Returns 404 with "Unknown path"
///
/// # Example
///
/// ```no_run
/// use reinhardt_test::server::{spawn_test_server, RouterHandler};
/// use std::sync::Arc;
///
/// # async fn example() {
/// let handler = Arc::new(RouterHandler);
/// let (url, handle) = spawn_test_server(handler).await;
/// // A request to "/" will return "Home"
/// // A request to "/api" will return JSON
/// # }
/// ```
pub struct RouterHandler;

#[async_trait::async_trait]
impl Handler for RouterHandler {
	async fn handle(&self, request: Request) -> reinhardt_core::exception::Result<Response> {
		let path = request.uri.path();

		match path {
			"/" => Ok(Response::ok().with_body("Home")),
			"/api" => Ok(Response::ok().with_body(r#"{"status": "ok"}"#)),
			"/notfound" => Ok(Response::not_found().with_body("Not Found")),
			_ => Ok(Response::not_found().with_body("Unknown path")),
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	fn test_request() -> Request {
		Request::builder().uri("/test").build().unwrap()
	}

	#[rstest]
	#[case(100)]
	#[case(200)]
	#[tokio::test]
	async fn delayed_handler_actually_delays(#[case] delay_ms: u64) {
		// Arrange
		let handler = DelayedHandler {
			delay_ms,
			response_body: "delayed".to_string(),
		};

		// Act
		let start = tokio::time::Instant::now();
		let response = handler.handle(test_request()).await.unwrap();
		let elapsed = start.elapsed();

		// Assert
		assert!(
			elapsed.as_millis() >= u128::from(delay_ms),
			"Expected at least {}ms delay, but elapsed was {}ms",
			delay_ms,
			elapsed.as_millis()
		);
		assert_eq!(String::from_utf8_lossy(&response.body), "delayed");
	}

	#[rstest]
	#[tokio::test]
	async fn delayed_handler_zero_delay_returns_immediately() {
		// Arrange
		let handler = DelayedHandler {
			delay_ms: 0,
			response_body: "instant".to_string(),
		};

		// Act
		let start = tokio::time::Instant::now();
		let response = handler.handle(test_request()).await.unwrap();
		let elapsed = start.elapsed();

		// Assert
		assert!(
			elapsed.as_millis() < 50,
			"Zero delay should return almost immediately, but took {}ms",
			elapsed.as_millis()
		);
		assert_eq!(String::from_utf8_lossy(&response.body), "instant");
	}
}
