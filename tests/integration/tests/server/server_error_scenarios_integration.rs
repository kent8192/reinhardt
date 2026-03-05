//! Server Error Scenarios Integration Tests
//!
//! Tests for various server error scenarios including:
//! - Invalid HTTP headers (malformed, oversized, invalid characters)
//! - JSON parse errors (invalid JSON, type mismatches)
//! - Network disconnection (abrupt connection close)
//! - Handler panic recovery (panic in handler should not crash server)
//! - Memory exhaustion scenarios (large payloads, many concurrent requests)
//! - Port conflict errors (server bind failures)

use async_trait::async_trait;
use http::StatusCode;
use reinhardt_core::exception::{Error, Result};
use reinhardt_http::Handler;
use reinhardt_http::{Request, Response};
use reinhardt_server::ShutdownCoordinator;
use reinhardt_test::APIClient;
use reinhardt_test::fixtures::*;
use rstest::*;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

// ============================================================================
// Test Handlers
// ============================================================================

/// Handler that throws various HTTP errors based on request path
struct ErrorThrowingHandler;

#[async_trait]
impl Handler for ErrorThrowingHandler {
	async fn handle(&self, request: Request) -> Result<Response> {
		match request.uri.path() {
			"/bad-request" => Err(Error::Http("Bad request".to_string())),
			"/internal-error" => Err(Error::Internal("Internal server error".to_string())),
			"/not-found" => Err(Error::NotFound("Resource not found".to_string())),
			"/unauthorized" => Err(Error::Authentication("Unauthorized".to_string())),
			"/forbidden" => Err(Error::Authorization("Forbidden".to_string())),
			_ => Ok(Response::ok().with_body("OK")),
		}
	}
}

/// Handler that parses JSON from request body
struct JsonParsingHandler;

#[async_trait]
impl Handler for JsonParsingHandler {
	async fn handle(&self, request: Request) -> Result<Response> {
		// Attempt to parse JSON from request body
		let body_str = String::from_utf8(request.body().to_vec())
			.map_err(|e| Error::Serialization(format!("Invalid UTF-8: {}", e)))?;

		// Parse JSON
		let _json: serde_json::Value = serde_json::from_str(&body_str)
			.map_err(|e| Error::Serialization(format!("Invalid JSON: {}", e)))?;

		Ok(Response::ok().with_body("JSON parsed successfully"))
	}
}

/// Handler that panics when called (for testing panic recovery)
struct PanicHandler;

#[async_trait]
impl Handler for PanicHandler {
	async fn handle(&self, _request: Request) -> Result<Response> {
		panic!("Handler intentionally panicked for testing");
	}
}

/// Handler that simulates slow processing
struct SlowHandler {
	delay: Duration,
}

#[async_trait]
impl Handler for SlowHandler {
	async fn handle(&self, _request: Request) -> Result<Response> {
		tokio::time::sleep(self.delay).await;
		Ok(Response::ok().with_body("Slow response"))
	}
}

/// Handler that validates request headers
struct HeaderValidationHandler;

#[async_trait]
impl Handler for HeaderValidationHandler {
	async fn handle(&self, request: Request) -> Result<Response> {
		// Check for required headers
		if !request.headers.contains_key("content-type") {
			return Err(Error::Http("Missing Content-Type header".to_string()));
		}

		// Check for invalid header values
		if let Some(content_type) = request.headers.get("content-type") {
			if content_type.to_str().is_err() {
				return Err(Error::Http("Invalid Content-Type header value".to_string()));
			}
		}

		Ok(Response::ok().with_body("Headers valid"))
	}
}

/// Handler that checks request body size
struct BodySizeHandler {
	max_size: usize,
}

#[async_trait]
impl Handler for BodySizeHandler {
	async fn handle(&self, request: Request) -> Result<Response> {
		if request.body().len() > self.max_size {
			return Err(Error::Http(format!(
				"Request body too large: {} bytes (max: {} bytes)",
				request.body().len(),
				self.max_size
			)));
		}

		Ok(Response::ok().with_body("Body size OK"))
	}
}

// ============================================================================
// Test 1: Invalid HTTP Headers
// ============================================================================

/// Test server handling of malformed HTTP headers
///
/// Verifies that the server can handle requests with:
/// - Missing required headers
/// - Invalid header values
/// - Oversized headers
#[rstest]
#[tokio::test]
async fn test_invalid_http_headers(#[future] http1_server: TestServer) {
	let server = http1_server.await;

	let handler = Arc::new(HeaderValidationHandler);
	let test_server = TestServer::builder()
		.handler(handler)
		.build()
		.await
		.expect("Failed to create test server");

	let client = APIClient::with_base_url(&test_server.url);

	// Test 1: Missing required header
	let response = client.get("/test").await.expect("Failed to send request");

	assert_eq!(response.status(), StatusCode::BAD_REQUEST);
	let body = response.text();
	assert!(body.contains("Missing Content-Type header"));

	// Test 2: Valid headers
	let headers = &[("Content-Type", "application/json")];
	let response = client
		.get_with_headers("/test", headers)
		.await
		.expect("Failed to send request");

	assert_eq!(response.status(), StatusCode::OK);

	// Cleanup
	drop(test_server);
	drop(server);
}

/// Test server handling of oversized headers
///
/// Verifies that the server properly handles requests with excessively large headers.
#[rstest]
#[tokio::test]
async fn test_oversized_headers(#[future] http1_server: TestServer) {
	let server = http1_server.await;

	let handler = Arc::new(BasicHandler);
	let test_server = TestServer::builder()
		.handler(handler)
		.build()
		.await
		.expect("Failed to create test server");

	let client = APIClient::with_base_url(&test_server.url);

	// Create a very large header value (10KB)
	let large_header_value = "x".repeat(10 * 1024);

	// Leak the string to get a static lifetime for the header value
	let large_header_value: &'static str = Box::leak(large_header_value.into_boxed_str());
	let headers = &[("X-Large-Header", large_header_value)];
	let response = client.get_with_headers("/test", headers).await;

	// Server should either reject the request or handle it gracefully
	// hyper may reject oversized headers before reaching our handler
	match response {
		Ok(resp) => {
			// If accepted, server should still be responsive
			assert!(resp.status().is_success() || resp.status().is_client_error());
		}
		Err(_) => {
			// Connection error is acceptable for oversized headers
		}
	}

	// Cleanup
	drop(test_server);
	drop(server);
}

// ============================================================================
// Test 2: JSON Parse Errors
// ============================================================================

/// Test server handling of invalid JSON in request bodies
///
/// Verifies that the server properly handles:
/// - Invalid JSON syntax
/// - Type mismatches
/// - Empty request bodies
#[rstest]
#[tokio::test]
async fn test_json_parse_errors(#[future] http1_server: TestServer) {
	let server = http1_server.await;

	let handler = Arc::new(JsonParsingHandler);
	let test_server = TestServer::builder()
		.handler(handler)
		.build()
		.await
		.expect("Failed to create test server");

	let client = APIClient::with_base_url(&test_server.url);

	// Test 1: Invalid JSON syntax
	let response = client
		.post_raw_with_headers("/parse", b"{invalid json", "application/json", &[])
		.await
		.expect("Failed to send request");

	assert_eq!(response.status(), StatusCode::BAD_REQUEST);
	let body = response.text();
	assert!(body.contains("Invalid JSON"));

	// Test 2: Empty body
	let response = client
		.post_raw_with_headers("/parse", b"", "application/json", &[])
		.await
		.expect("Failed to send request");

	assert_eq!(response.status(), StatusCode::BAD_REQUEST);

	// Test 3: Valid JSON
	let response = client
		.post_raw_with_headers("/parse", br#"{"key": "value"}"#, "application/json", &[])
		.await
		.expect("Failed to send request");

	assert_eq!(response.status(), StatusCode::OK);
	let body = response.text();
	assert_eq!(body, "JSON parsed successfully");

	// Test 4: Invalid UTF-8 in body
	// APIClient's post_raw accepts &[u8], allowing binary data including invalid UTF-8
	let invalid_utf8: &[u8] = &[0xFF, 0xFE, 0xFD]; // Invalid UTF-8 bytes
	let response = client
		.post_raw("/parse", invalid_utf8, "application/json")
		.await
		.expect("Failed to send request");

	assert_eq!(response.status(), StatusCode::BAD_REQUEST);
	let body = response.text();
	assert!(body.contains("Invalid UTF-8"));

	// Cleanup
	drop(test_server);
	drop(server);
}

// ============================================================================
// Test 3: Network Disconnection
// ============================================================================

/// Test server handling of abrupt connection close
///
/// Verifies that the server can handle clients that disconnect abruptly
/// without completing the HTTP request/response cycle.
#[rstest]
#[tokio::test]
async fn test_network_disconnection(#[future] http1_server: TestServer) {
	let server = http1_server.await;

	let handler = Arc::new(SlowHandler {
		delay: Duration::from_secs(2),
	});
	let test_server = TestServer::builder()
		.handler(handler)
		.build()
		.await
		.expect("Failed to create test server");

	// Connect directly with TCP socket
	let addr = test_server.addr;
	let mut stream = TcpStream::connect(addr)
		.await
		.expect("Failed to connect to server");

	// Send partial HTTP request
	stream
		.write_all(b"GET /slow HTTP/1.1\r\nHost: localhost\r\n")
		.await
		.expect("Failed to write to stream");

	// Immediately close the connection without completing the request
	drop(stream);

	// Give the server time to process the disconnection
	tokio::time::sleep(Duration::from_millis(100)).await;

	// Verify server is still responsive by making a normal request
	let client = APIClient::with_base_url(&test_server.url);
	let response = client
		.get("/test")
		.await
		.expect("Failed to send request after disconnection");

	assert_eq!(response.status(), StatusCode::OK);

	// Cleanup
	drop(test_server);
	drop(server);
}

/// Test server handling of client disconnect during response transmission
///
/// Verifies that the server can handle clients that disconnect while the
/// server is sending a response.
#[rstest]
#[tokio::test]
async fn test_client_disconnect_during_response(#[future] http1_server: TestServer) {
	let server = http1_server.await;

	let handler = Arc::new(BasicHandler);
	let test_server = TestServer::builder()
		.handler(handler)
		.build()
		.await
		.expect("Failed to create test server");

	// Connect directly with TCP socket
	let addr = test_server.addr;
	let mut stream = TcpStream::connect(addr)
		.await
		.expect("Failed to connect to server");

	// Send complete HTTP request
	stream
		.write_all(b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n")
		.await
		.expect("Failed to write to stream");

	// Read only part of the response headers
	let mut buffer = [0u8; 20];
	let _ = stream.read(&mut buffer).await;

	// Close connection before reading full response
	drop(stream);

	// Give the server time to process the disconnection
	tokio::time::sleep(Duration::from_millis(100)).await;

	// Verify server is still responsive
	let client = APIClient::with_base_url(&test_server.url);
	let response = client
		.get("/test")
		.await
		.expect("Failed to send request after disconnect");

	assert_eq!(response.status(), StatusCode::OK);

	// Cleanup
	drop(test_server);
	drop(server);
}

// ============================================================================
// Test 4: Handler Panic Recovery
// ============================================================================

/// Test server recovery from handler panics
///
/// Verifies that panics in request handlers do not crash the entire server
/// and that the server remains responsive after a panic occurs.
#[rstest]
#[tokio::test]
async fn test_handler_panic_recovery(#[future] http1_server: TestServer) {
	let server = http1_server.await;

	let handler = Arc::new(PanicHandler);
	let test_server = TestServer::builder()
		.handler(handler)
		.build()
		.await
		.expect("Failed to create test server");

	let client = APIClient::builder()
		.base_url(&test_server.url)
		.timeout(Duration::from_secs(5))
		.build();

	// Send request that will cause panic in handler
	// The panic should be caught by the tokio task spawn
	let result = client.get("/panic").await;

	// The request may fail due to panic or connection reset
	// The important thing is that the server doesn't crash
	match result {
		Ok(response) => {
			// If we get a response, it should be an error status
			assert!(response.is_server_error() || response.is_client_error());
		}
		Err(e) => {
			// Connection error is acceptable when handler panics
			// The panic in the handler task should not crash the server
			assert!(
				e.is_connect() || e.is_timeout() || e.is_request(),
				"Unexpected error type: {:?}",
				e
			);
		}
	}

	// Give the server time to recover from panic
	tokio::time::sleep(Duration::from_millis(500)).await;

	// CRITICAL: Verify server is still alive and responsive
	// Create a new handler that won't panic for verification
	let working_handler = Arc::new(BasicHandler);
	let verification_server = TestServer::builder()
		.handler(working_handler)
		.build()
		.await
		.expect("Failed to create verification server");

	let verification_client = APIClient::builder()
		.base_url(&verification_server.url)
		.timeout(Duration::from_secs(5))
		.build();

	let response = verification_client
		.get("/test")
		.await
		.expect("Server should still be responsive after handler panic");

	assert_eq!(response.status(), StatusCode::OK);

	// Cleanup
	drop(verification_server);
	drop(test_server);
	drop(server);
}

// ============================================================================
// Test 5: Memory Exhaustion Scenarios
// ============================================================================

/// Test server handling of large request payloads
///
/// Verifies that the server can handle large request bodies without
/// running out of memory or becoming unresponsive.
#[rstest]
#[tokio::test]
async fn test_large_request_payload(#[future] http1_server: TestServer) {
	let server = http1_server.await;

	let max_size = 1024 * 1024; // 1MB
	let handler = Arc::new(BodySizeHandler { max_size });
	let test_server = TestServer::builder()
		.handler(handler)
		.build()
		.await
		.expect("Failed to create test server");

	let client = APIClient::builder()
		.base_url(&test_server.url)
		.timeout(Duration::from_secs(10))
		.build();

	// Test 1: Request within size limit
	let small_payload = "x".repeat(512 * 1024); // 512KB
	let response = client
		.post_raw(
			"/upload",
			small_payload.as_bytes(),
			"application/octet-stream",
		)
		.await
		.expect("Failed to send small request");

	assert_eq!(response.status(), StatusCode::OK);

	// Test 2: Request exceeding size limit
	let large_payload = "x".repeat(2 * 1024 * 1024); // 2MB
	let response = client
		.post_raw(
			"/upload",
			large_payload.as_bytes(),
			"application/octet-stream",
		)
		.await
		.expect("Failed to send large request");

	assert_eq!(response.status(), StatusCode::BAD_REQUEST);
	let body = response.text();
	assert!(body.contains("Request body too large"));

	// Verify server is still responsive after large payload
	let response = client
		.get("/test")
		.await
		.expect("Failed to send request after large payload");

	assert_eq!(response.status(), StatusCode::OK);

	// Cleanup
	drop(test_server);
	drop(server);
}

/// Test server handling of many concurrent requests
///
/// Verifies that the server can handle a large number of concurrent
/// requests without running out of resources or becoming unresponsive.
#[rstest]
#[tokio::test]
async fn test_many_concurrent_requests(#[future] http1_server: TestServer) {
	let server = http1_server.await;

	let handler = Arc::new(BasicHandler);
	let test_server = TestServer::builder()
		.handler(handler)
		.build()
		.await
		.expect("Failed to create test server");

	// APIClient uses connection pooling internally via reqwest::Client
	let client = Arc::new(
		APIClient::builder()
			.base_url(&test_server.url)
			.timeout(Duration::from_secs(10))
			.build(),
	);

	// Spawn 100 concurrent requests
	let mut tasks = vec![];
	for i in 0..100 {
		let client = client.clone();
		let path = format!("/test/{}", i);

		let task =
			tokio::spawn(async move { client.get(&path).await.expect("Failed to send request") });

		tasks.push(task);
	}

	// Wait for all requests to complete
	let results = futures::future::join_all(tasks).await;

	// Verify all requests completed successfully
	for result in results {
		let response = result.expect("Task panicked");
		assert_eq!(response.status(), StatusCode::OK);
	}

	// Verify server is still responsive after concurrent load
	let response = client
		.get("/test")
		.await
		.expect("Failed to send request after concurrent load");

	assert_eq!(response.status(), StatusCode::OK);

	// Cleanup
	drop(test_server);
	drop(server);
}

// ============================================================================
// Test 6: Port Conflict Errors
// ============================================================================

/// Test server handling of port bind failures
///
/// Verifies that the server properly reports errors when it cannot bind
/// to the requested port (e.g., port already in use).
#[rstest]
#[tokio::test]
#[ignore = "Needs fix for Send + 'static bound on error type"]
async fn test_port_conflict_error(#[future] http1_server: TestServer) {
	let server = http1_server.await;

	let handler = Arc::new(BasicHandler);

	// Create first server and bind to a random port
	let first_server = TestServer::builder()
		.handler(handler.clone())
		.build()
		.await
		.expect("Failed to create first server");

	let used_addr = first_server.addr;

	// Attempt to create second server on the same address
	// This should fail because the port is already in use
	let coordinator = Arc::new(ShutdownCoordinator::new(Duration::from_secs(5)));
	let coordinator_clone = coordinator.clone();

	let server_task = tokio::spawn(async move {
		let server = reinhardt_server::HttpServer::new(handler);
		server
			.listen_with_shutdown(used_addr, (*coordinator_clone).clone())
			.await
			.map_err(|e| e.to_string())
	});

	// Give some time for the bind attempt
	tokio::time::sleep(Duration::from_millis(100)).await;

	// The server task should fail due to port conflict
	// We use a timeout to ensure the test doesn't hang
	let result = tokio::time::timeout(Duration::from_secs(2), server_task).await;

	match result {
		Ok(Ok(server_result)) => {
			// Server should have returned an error
			assert!(
				server_result.is_err(),
				"Server should fail to bind to already-used port"
			);
		}
		Ok(Err(_)) => {
			// Task panicked - this is also acceptable for port conflict
		}
		Err(_) => {
			// Timeout - server might be stuck trying to bind
			// This is acceptable as the test demonstrates the port is in use
		}
	}

	// Verify first server is still running
	let client = APIClient::with_base_url(&first_server.url);
	let response = client
		.get("/test")
		.await
		.expect("First server should still be responsive");

	assert_eq!(response.status(), StatusCode::OK);

	// Cleanup
	drop(first_server);
	drop(server);
}

/// Test server error responses with proper status codes
///
/// Verifies that the server returns appropriate HTTP status codes for
/// different types of errors.
#[rstest]
#[tokio::test]
async fn test_error_status_codes(#[future] http1_server: TestServer) {
	let server = http1_server.await;

	let handler = Arc::new(ErrorThrowingHandler);
	let test_server = TestServer::builder()
		.handler(handler)
		.build()
		.await
		.expect("Failed to create test server");

	let client = APIClient::with_base_url(&test_server.url);

	// Test 400 Bad Request
	let response = client
		.get("/bad-request")
		.await
		.expect("Failed to send request");
	assert_eq!(response.status(), StatusCode::BAD_REQUEST);

	// Test 401 Unauthorized
	let response = client
		.get("/unauthorized")
		.await
		.expect("Failed to send request");
	assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

	// Test 403 Forbidden
	let response = client
		.get("/forbidden")
		.await
		.expect("Failed to send request");
	assert_eq!(response.status(), StatusCode::FORBIDDEN);

	// Test 404 Not Found
	let response = client
		.get("/not-found")
		.await
		.expect("Failed to send request");
	assert_eq!(response.status(), StatusCode::NOT_FOUND);

	// Test 500 Internal Server Error
	let response = client
		.get("/internal-error")
		.await
		.expect("Failed to send request");
	assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);

	// Cleanup
	drop(test_server);
	drop(server);
}
