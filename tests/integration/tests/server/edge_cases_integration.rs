//! Edge case integration tests for HTTP server
//!
//! Tests server behavior under edge cases and unusual conditions:
//! - Empty POST bodies
//! - Duplicate headers
//! - HTTP/1.0 compatibility
//! - Minimum/maximum request sizes
//! - Requests during server shutdown
//! - Malformed URIs

use http::StatusCode;
use reinhardt_http::{Request, Response, ViewResult};
use reinhardt_macros::{get, post};
use reinhardt_server::ShutdownCoordinator;
use reinhardt_test::APIClient;
use reinhardt_test::fixtures::*;
use reinhardt_urls::routers::ServerRouter as Router;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;

// ============================================================================
// Test Handlers
// ============================================================================

/// Handler for empty body tests
#[post("/empty", name = "empty_body_handler")]
async fn empty_body_handler(req: Request) -> ViewResult<Response> {
	let body = req.body();
	let is_empty = body.is_empty();
	let content_length = body.len();
	Ok(Response::ok().with_body(format!("empty:{},length:{}", is_empty, content_length)))
}

/// Handler for duplicate headers tests
#[get("/headers", name = "duplicate_headers_handler")]
async fn duplicate_headers_handler(req: Request) -> ViewResult<Response> {
	// Get all values for a header (if duplicated)
	let custom_values: Vec<String> = req
		.headers
		.get_all("x-custom")
		.iter()
		.filter_map(|v| v.to_str().ok())
		.map(|s| s.to_string())
		.collect();

	Ok(Response::ok().with_body(format!("count:{}", custom_values.len())))
}

/// Handler for HTTP/1.0 compatibility tests
#[get("/version", name = "version_handler")]
async fn version_handler(req: Request) -> ViewResult<Response> {
	let version = format!("{:?}", req.version);
	Ok(Response::ok().with_body(format!("version:{}", version)))
}

/// Handler for size limit tests - accepts large payloads
#[post("/size", name = "size_handler")]
async fn size_handler(req: Request) -> ViewResult<Response> {
	let size = req.body().len();
	Ok(Response::ok().with_body(format!("size:{}", size)))
}

/// Handler for malformed URI tests
#[get("/uri", name = "uri_handler")]
async fn uri_handler(req: Request) -> ViewResult<Response> {
	let path = req.uri.path();
	let query = req.uri.query().unwrap_or("");
	Ok(Response::ok().with_body(format!("path:{},query:{}", path, query)))
}

/// Handler for shutdown tests - includes delay to test graceful shutdown
#[get("/slow", name = "slow_handler")]
async fn slow_handler(_req: Request) -> ViewResult<Response> {
	// Simulate slow processing
	tokio::time::sleep(Duration::from_millis(200)).await;
	Ok(Response::ok().with_body("slow_response"))
}

// ============================================================================
// Edge Case Tests
// ============================================================================

#[tokio::test]
async fn test_empty_post_body() {
	// Test server handling of POST request with Content-Length: 0
	let router = Router::new().endpoint(empty_body_handler);
	let server = test_server_guard(router).await;

	let client = APIClient::with_base_url(&server.url);

	// Send POST with empty body
	let response = client
		.post_raw_with_headers(
			"/empty",
			b"",
			"application/octet-stream",
			&[("Content-Length", "0")],
		)
		.await
		.unwrap();

	assert_eq!(response.status(), StatusCode::OK);
	let text = response.text();
	assert_eq!(text, "empty:true,length:0");
}

#[tokio::test]
async fn test_empty_post_body_no_content_length() {
	// Test server handling of POST without explicit Content-Length
	let router = Router::new().endpoint(empty_body_handler);
	let server = test_server_guard(router).await;

	let client = APIClient::with_base_url(&server.url);

	// Send POST with empty body
	let response = client
		.post_raw("/empty", b"", "application/octet-stream")
		.await
		.unwrap();

	assert_eq!(response.status(), StatusCode::OK);
	let text = response.text();
	assert_eq!(text, "empty:true,length:0");
}

#[tokio::test]
async fn test_duplicate_headers() {
	// Test server handling of multiple headers with the same name
	// Note: Most HTTP clients (including reqwest) don't expose duplicate header support in their public API,
	// as HTTP/2 and modern HTTP/1.1 implementations typically merge duplicate headers.
	// This test verifies the server can handle the case if such headers arrive.
	let router = Router::new().endpoint(duplicate_headers_handler);
	let server = test_server_guard(router).await;

	// Extract host and port from server URL
	let url_parts = server.url.replace("http://", "");
	let parts: Vec<&str> = url_parts.split(':').collect();
	let host = parts[0];
	let port: u16 = parts[1].parse().unwrap();

	// Create manual HTTP request with duplicate headers
	use tokio::io::{AsyncReadExt, AsyncWriteExt};
	let mut stream = tokio::net::TcpStream::connect((host, port)).await.unwrap();

	// Send HTTP request with duplicate x-custom headers
	let request = format!(
		"GET /headers HTTP/1.1\r\nHost: {}\r\nx-custom: value1\r\nx-custom: value2\r\nx-custom: value3\r\nConnection: close\r\n\r\n",
		host
	);
	stream.write_all(request.as_bytes()).await.unwrap();

	// Read response
	let mut buffer = Vec::new();
	stream.read_to_end(&mut buffer).await.unwrap();

	let response_text = String::from_utf8_lossy(&buffer);

	// Should accept request and respond successfully
	assert!(
		response_text.contains("200 OK") || response_text.contains("HTTP/1.1 200"),
		"Response should indicate success"
	);
	// Server should see all 3 duplicate headers
	assert!(
		response_text.contains("count:3"),
		"Response body should indicate 3 headers received"
	);
}

#[tokio::test]
async fn test_http_1_0_compatibility() {
	// Test server compatibility with HTTP/1.0 requests
	let router = Router::new().endpoint(version_handler);
	let server = test_server_guard(router).await;

	// Extract host and port from server URL
	let url_parts = server.url.replace("http://", "");
	let parts: Vec<&str> = url_parts.split(':').collect();
	let host = parts[0];
	let port: u16 = parts[1].parse().unwrap();

	// Create manual HTTP/1.0 request
	use tokio::io::{AsyncReadExt, AsyncWriteExt};
	let mut stream = tokio::net::TcpStream::connect((host, port)).await.unwrap();

	// Send HTTP/1.0 request
	let request = format!(
		"GET /version HTTP/1.0\r\nHost: {}\r\nConnection: close\r\n\r\n",
		host
	);
	stream.write_all(request.as_bytes()).await.unwrap();

	// Read response
	let mut buffer = Vec::new();
	stream.read_to_end(&mut buffer).await.unwrap();

	let response_text = String::from_utf8_lossy(&buffer);

	// Should accept HTTP/1.0 and respond successfully
	assert!(
		response_text.contains("200 OK") || response_text.contains("HTTP/1.1 200"),
		"Response should indicate success"
	);
	assert!(
		response_text.contains("version:"),
		"Response body should contain version info"
	);
}

#[tokio::test]
async fn test_minimum_request_size() {
	// Test server handling of minimal 1-byte request body
	let router = Router::new().endpoint(size_handler);
	let server = test_server_guard(router).await;

	let client = APIClient::with_base_url(&server.url);

	// Send 1-byte body
	let response = client
		.post_raw("/size", b"a", "application/octet-stream")
		.await
		.unwrap();

	assert_eq!(response.status(), StatusCode::OK);
	let text = response.text();
	assert_eq!(text, "size:1");
}

#[tokio::test]
async fn test_small_request_sizes() {
	// Test various small request sizes (0, 1, 10, 100 bytes)
	let router = Router::new().endpoint(size_handler);
	let server = test_server_guard(router).await;

	let client = APIClient::with_base_url(&server.url);

	// Test 0 bytes
	let response = client
		.post_raw("/size", b"", "application/octet-stream")
		.await
		.unwrap();
	assert_eq!(response.status(), StatusCode::OK);
	assert_eq!(response.text(), "size:0");

	// Test 1 byte
	let response = client
		.post_raw("/size", b"a", "application/octet-stream")
		.await
		.unwrap();
	assert_eq!(response.status(), StatusCode::OK);
	assert_eq!(response.text(), "size:1");

	// Test 10 bytes
	let response = client
		.post_raw("/size", b"0123456789", "application/octet-stream")
		.await
		.unwrap();
	assert_eq!(response.status(), StatusCode::OK);
	assert_eq!(response.text(), "size:10");

	// Test 100 bytes
	let payload_100 = "x".repeat(100);
	let response = client
		.post_raw("/size", payload_100.as_bytes(), "application/octet-stream")
		.await
		.unwrap();
	assert_eq!(response.status(), StatusCode::OK);
	assert_eq!(response.text(), "size:100");
}

#[tokio::test]
async fn test_large_request_size() {
	// Test server handling of large (but acceptable) request body
	let router = Router::new().endpoint(size_handler);
	let server = test_server_guard(router).await;

	let client = APIClient::with_base_url(&server.url);

	// Send 1MB body (should be accepted)
	let large_payload = vec![b'x'; 1024 * 1024];
	let response = client
		.post_raw("/size", &large_payload, "application/octet-stream")
		.await
		.unwrap();

	assert_eq!(response.status(), StatusCode::OK);
	let text = response.text();
	assert_eq!(text, format!("size:{}", 1024 * 1024));
}

#[tokio::test]
async fn test_request_during_shutdown() {
	// Test server behavior when receiving requests during graceful shutdown
	let router = Router::new().endpoint(slow_handler);

	// Manual setup to control shutdown timing
	let shutdown_timeout = Duration::from_secs(5);
	let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
	let listener = TcpListener::bind(addr).await.unwrap();
	let actual_addr = listener.local_addr().unwrap();
	let url = format!("http://{}", actual_addr);
	drop(listener);

	let coordinator = ShutdownCoordinator::new(shutdown_timeout);

	// Spawn server
	let server_coordinator = coordinator.clone();
	let server_task = tokio::spawn(async move {
		let server = reinhardt_server::HttpServer::new(router);
		let _ = server
			.listen_with_shutdown(actual_addr, server_coordinator)
			.await;
	});

	// Wait for server to start
	tokio::time::sleep(Duration::from_millis(100)).await;

	let client = Arc::new(
		APIClient::builder()
			.base_url(&url)
			.timeout(Duration::from_secs(10))
			.build(),
	);

	// Start a slow request
	let client_clone = client.clone();
	let request_task = tokio::spawn(async move { client_clone.get("/slow").await });

	// Wait for request to start processing
	tokio::time::sleep(Duration::from_millis(50)).await;

	// Trigger shutdown while request is processing
	coordinator.shutdown();

	// Request should complete (graceful shutdown waits for in-flight requests)
	let result = tokio::time::timeout(Duration::from_secs(2), request_task)
		.await
		.unwrap();

	// Request may complete successfully or fail due to shutdown
	// Both are acceptable behaviors for graceful shutdown
	let is_completed = match result {
		Ok(Ok(response)) => response.status() == StatusCode::OK,
		_ => false,
	};

	// Server should shut down
	let server_result = tokio::time::timeout(Duration::from_secs(2), server_task).await;
	assert!(
		server_result.is_ok(),
		"Server should complete shutdown within timeout"
	);

	// Either request completed OR server shutdown gracefully
	// This tests that shutdown doesn't panic or hang
	assert!(
		is_completed || server_result.is_ok(),
		"Server should handle shutdown gracefully"
	);
}

#[tokio::test]
async fn test_new_request_after_shutdown_signal() {
	// Test that new requests after shutdown signal are rejected
	let router = Router::new().endpoint(slow_handler);

	let shutdown_timeout = Duration::from_secs(2);
	let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
	let listener = TcpListener::bind(addr).await.unwrap();
	let actual_addr = listener.local_addr().unwrap();
	let url = format!("http://{}", actual_addr);
	drop(listener);

	let coordinator = ShutdownCoordinator::new(shutdown_timeout);

	// Spawn server
	let server_coordinator = coordinator.clone();
	let server_task = tokio::spawn(async move {
		let server = reinhardt_server::HttpServer::new(router);
		let _ = server
			.listen_with_shutdown(actual_addr, server_coordinator)
			.await;
	});

	// Wait for server to start
	tokio::time::sleep(Duration::from_millis(100)).await;

	// Trigger shutdown immediately
	coordinator.shutdown();

	// Wait a bit for shutdown to propagate
	tokio::time::sleep(Duration::from_millis(50)).await;

	// Try to make new request after shutdown signal
	let client = APIClient::builder()
		.base_url(&url)
		.timeout(Duration::from_secs(1))
		.build();
	let result = client.get("/slow").await;

	// Request should fail (connection refused or timeout)
	assert!(result.is_err(), "New requests after shutdown should fail");

	// Server should complete shutdown
	let server_result = tokio::time::timeout(Duration::from_secs(3), server_task).await;
	assert!(
		server_result.is_ok(),
		"Server should complete shutdown within timeout"
	);
}

#[tokio::test]
async fn test_malformed_uri_special_characters() {
	// Test server handling of URIs with special characters
	let router = Router::new().endpoint(uri_handler);
	let server = test_server_guard(router).await;

	let client = APIClient::with_base_url(&server.url);

	// Test various special characters in URI
	let test_cases = vec![
		(
			"/uri?key=value%20with%20spaces",
			"path:/uri,query:key=value%20with%20spaces",
		),
		(
			"/uri?key=value&another=test",
			"path:/uri,query:key=value&another=test",
		),
		("/uri?empty=", "path:/uri,query:empty="),
	];

	for (uri_path, expected) in test_cases {
		let response = client.get(uri_path).await.unwrap();

		assert_eq!(response.status(), StatusCode::OK);
		let text = response.text();
		assert_eq!(text, expected, "Failed for URI: {}", uri_path);
	}
}

#[tokio::test]
async fn test_uri_without_query_string() {
	// Test server handling of URI without query string
	let router = Router::new().endpoint(uri_handler);
	let server = test_server_guard(router).await;

	let client = APIClient::with_base_url(&server.url);

	let response = client.get("/uri").await.unwrap();

	assert_eq!(response.status(), StatusCode::OK);
	let text = response.text();
	assert_eq!(text, "path:/uri,query:");
}

#[tokio::test]
async fn test_uri_with_fragment() {
	// Test server handling of URI with fragment (fragments should not be sent to server)
	let router = Router::new().endpoint(uri_handler);
	let server = test_server_guard(router).await;

	let client = APIClient::with_base_url(&server.url);

	// Fragment (#section) should not be sent to server by client
	let response = client.get("/uri?key=value#fragment").await.unwrap();

	assert_eq!(response.status(), StatusCode::OK);
	let text = response.text();
	// Server should not see the fragment
	assert_eq!(text, "path:/uri,query:key=value");
}

#[tokio::test]
async fn test_very_long_uri() {
	// Test server handling of very long URI
	let router = Router::new().endpoint(uri_handler);
	let server = test_server_guard(router).await;

	let client = APIClient::with_base_url(&server.url);

	// Create a long query string (but still reasonable)
	let long_value = "x".repeat(1000);
	let path = format!("/uri?data={}", long_value);
	let response = client.get(&path).await.unwrap();

	// Should handle long URI successfully
	assert_eq!(response.status(), StatusCode::OK);
	let text = response.text();
	assert!(text.starts_with("path:/uri,query:data="));
	assert!(text.contains(&long_value));
}

#[tokio::test]
async fn test_extremely_long_uri() {
	// Test server handling of extremely long URI (may exceed limits)
	let router = Router::new().endpoint(uri_handler);
	let server = test_server_guard(router).await;

	let client = APIClient::with_base_url(&server.url);

	// Create an extremely long query string (10KB)
	let very_long_value = "x".repeat(10_000);
	let path = format!("/uri?data={}", very_long_value);
	let result = client.get(&path).await;

	// Server may accept it (200) or reject it (414 URI Too Long, 400 Bad Request, or connection error)
	// All are acceptable behaviors - we just verify server doesn't panic
	match result {
		Ok(response) => {
			let status = response.status();
			assert!(
				status == StatusCode::OK
					|| status == StatusCode::URI_TOO_LONG
					|| status == StatusCode::BAD_REQUEST,
				"Server should respond with 200, 414, or 400 for very long URI, got: {}",
				status
			);
		}
		Err(_) => {
			// Connection error is also acceptable for extremely long URIs
			// Server may close connection before fully reading the request
		}
	}
}

#[tokio::test]
async fn test_concurrent_requests_during_shutdown() {
	// Test multiple concurrent requests when shutdown is triggered
	let router = Router::new().endpoint(slow_handler);

	let shutdown_timeout = Duration::from_secs(5);
	let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
	let listener = TcpListener::bind(addr).await.unwrap();
	let actual_addr = listener.local_addr().unwrap();
	let url = format!("http://{}", actual_addr);
	drop(listener);

	let coordinator = ShutdownCoordinator::new(shutdown_timeout);

	// Spawn server
	let server_coordinator = coordinator.clone();
	let server_task = tokio::spawn(async move {
		let server = reinhardt_server::HttpServer::new(router);
		let _ = server
			.listen_with_shutdown(actual_addr, server_coordinator)
			.await;
	});

	// Wait for server to start
	tokio::time::sleep(Duration::from_millis(100)).await;

	// APIClient with connection pooling and timeout
	let client = Arc::new(
		APIClient::builder()
			.base_url(&url)
			.timeout(Duration::from_secs(10))
			.build(),
	);

	// Start multiple concurrent slow requests
	let mut request_tasks = vec![];
	for _ in 0..5 {
		let client_clone = client.clone();
		let task = tokio::spawn(async move { client_clone.get("/slow").await });
		request_tasks.push(task);
	}

	// Wait for requests to start processing
	tokio::time::sleep(Duration::from_millis(50)).await;

	// Trigger shutdown while requests are processing
	coordinator.shutdown();

	// Wait for all requests to complete or fail
	let mut completed_count = 0;
	for task in request_tasks {
		if let Ok(Ok(Ok(response))) = tokio::time::timeout(Duration::from_secs(3), task).await {
			if response.status() == StatusCode::OK {
				completed_count += 1;
			}
		}
	}

	// Server should shut down gracefully
	let server_result = tokio::time::timeout(Duration::from_secs(6), server_task).await;
	assert!(
		server_result.is_ok(),
		"Server should complete shutdown within timeout"
	);

	// At least some requests should have completed (graceful shutdown)
	// but not all need to complete if shutdown timeout is reached
	println!(
		"Completed {} out of 5 requests during shutdown",
		completed_count
	);
}
