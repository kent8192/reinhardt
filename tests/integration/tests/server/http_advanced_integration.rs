// HTTP Advanced Features Integration Tests
// Tests advanced HTTP features: large payloads, streaming, keep-alive, chunked encoding, etc.

use reinhardt_http::Handler;
use reinhardt_http::{Request, Response};
use reinhardt_test::fixtures::*;
use reqwest;
use rstest::*;
use std::sync::Arc;

// ============================================================================
// Test Handlers
// ============================================================================

/// Handler that echoes request body
struct BodyEchoHandler;

#[async_trait::async_trait]
impl Handler for BodyEchoHandler {
	async fn handle(&self, request: Request) -> reinhardt_core::exception::Result<Response> {
		let body = request.read_body()?;
		Ok(Response::ok().with_body(body))
	}
}

/// Handler that generates large response
struct LargeResponseHandler {
	size_bytes: usize,
}

#[async_trait::async_trait]
impl Handler for LargeResponseHandler {
	async fn handle(&self, _request: Request) -> reinhardt_core::exception::Result<Response> {
		// Generate large body (filled with 'x')
		let body = "x".repeat(self.size_bytes);
		Ok(Response::ok().with_body(body))
	}
}

/// Handler that returns streaming response
struct StreamingHandler;

#[async_trait::async_trait]
impl Handler for StreamingHandler {
	async fn handle(&self, _request: Request) -> reinhardt_core::exception::Result<Response> {
		// Return multiple chunks as a single response
		let chunks = vec!["chunk1", "chunk2", "chunk3"];
		let body = chunks.join("");
		Ok(Response::ok().with_body(body))
	}
}

/// Handler that supports keep-alive connections
struct KeepAliveHandler;

#[async_trait::async_trait]
impl Handler for KeepAliveHandler {
	async fn handle(&self, _request: Request) -> reinhardt_core::exception::Result<Response> {
		Ok(Response::ok()
			.with_header("Connection", "keep-alive")
			.with_body("keep-alive response"))
	}
}

/// Handler that uses chunked transfer encoding
struct ChunkedHandler;

#[async_trait::async_trait]
impl Handler for ChunkedHandler {
	async fn handle(&self, _request: Request) -> reinhardt_core::exception::Result<Response> {
		// Return response without Content-Length to trigger chunked encoding
		let body = "chunked response body";
		Ok(Response::ok()
			.with_header("Transfer-Encoding", "chunked")
			.with_body(body))
	}
}

/// Handler that supports Expect: 100-continue
struct ExpectContinueHandler;

#[async_trait::async_trait]
impl Handler for ExpectContinueHandler {
	async fn handle(&self, request: Request) -> reinhardt_core::exception::Result<Response> {
		// Check for Expect header
		let has_expect = request
			.headers
			.get("Expect")
			.and_then(|v| v.to_str().ok())
			.map(|v| v == "100-continue")
			.unwrap_or(false);

		if has_expect {
			// Echo the body to confirm we received it
			let body = request.read_body()?;
			Ok(Response::ok()
				.with_header("X-Expect-Continue", "true")
				.with_body(body))
		} else {
			Ok(Response::ok().with_body("no expect header"))
		}
	}
}

/// Handler that processes multipart/form-data
struct MultipartHandler;

#[async_trait::async_trait]
impl Handler for MultipartHandler {
	async fn handle(&self, request: Request) -> reinhardt_core::exception::Result<Response> {
		// Check Content-Type header
		let content_type = request
			.headers
			.get("Content-Type")
			.and_then(|v| v.to_str().ok())
			.unwrap_or("");

		if content_type.starts_with("multipart/form-data") {
			// This handler simply confirms we received multipart data
			let body = request.read_body()?;
			let response_body = format!(
				"Received multipart data: {} bytes, content-type: {}",
				body.len(),
				content_type
			);
			Ok(Response::ok()
				.with_header("X-Multipart-Processed", "true")
				.with_body(response_body))
		} else {
			Ok(Response::bad_request().with_body("Expected multipart/form-data"))
		}
	}
}

// ============================================================================
// Tests
// ============================================================================

/// Test large request body handling (10MB)
#[rstest]
#[tokio::test]
async fn test_large_request_body(http_client: reqwest::Client) {
	// Create test server with body echo handler
	let handler = Arc::new(BodyEchoHandler);
	let server = TestServer::builder()
		.handler(handler)
		.build()
		.await
		.expect("Failed to create test server");

	// Generate 10MB request body
	let request_size = 10 * 1024 * 1024; // 10MB
	let request_body = "x".repeat(request_size);

	// Send large POST request
	let response = http_client
		.post(&server.url)
		.body(request_body.clone())
		.send()
		.await
		.expect("Failed to send request");

	// Verify response
	assert_eq!(response.status(), 200);

	let response_body = response.text().await.expect("Failed to read response body");
	assert_eq!(
		response_body.len(),
		request_size,
		"Response body size should match request body size"
	);
	assert_eq!(
		response_body, request_body,
		"Response body should echo request body"
	);
}

/// Test large response body handling (10MB)
#[rstest]
#[tokio::test]
async fn test_large_response_body(http_client: reqwest::Client) {
	// Create test server with large response handler
	let response_size = 10 * 1024 * 1024; // 10MB
	let handler = Arc::new(LargeResponseHandler {
		size_bytes: response_size,
	});
	let server = TestServer::builder()
		.handler(handler)
		.build()
		.await
		.expect("Failed to create test server");

	// Send request
	let response = http_client
		.get(&server.url)
		.send()
		.await
		.expect("Failed to send request");

	// Verify response
	assert_eq!(response.status(), 200);

	let response_body = response.text().await.expect("Failed to read response body");
	assert_eq!(
		response_body.len(),
		response_size,
		"Response body size should be 10MB"
	);
	assert!(
		response_body.chars().all(|c| c == 'x'),
		"Response body should contain only 'x' characters"
	);
}

/// Test streaming response
#[rstest]
#[tokio::test]
async fn test_streaming_response(http_client: reqwest::Client) {
	// Create test server with streaming handler
	let handler = Arc::new(StreamingHandler);
	let server = TestServer::builder()
		.handler(handler)
		.build()
		.await
		.expect("Failed to create test server");

	// Send request
	let response = http_client
		.get(&server.url)
		.send()
		.await
		.expect("Failed to send request");

	// Verify response
	assert_eq!(response.status(), 200);

	let response_body = response.text().await.expect("Failed to read response body");
	assert_eq!(
		response_body, "chunk1chunk2chunk3",
		"Response should contain all chunks"
	);
}

/// Test keep-alive connection handling
#[rstest]
#[tokio::test]
async fn test_keep_alive_connection(http_client: reqwest::Client) {
	// Create test server with keep-alive handler
	let handler = Arc::new(KeepAliveHandler);
	let server = TestServer::builder()
		.handler(handler)
		.build()
		.await
		.expect("Failed to create test server");

	// Send multiple requests on the same connection
	for i in 0..3 {
		let response = http_client
			.get(&server.url)
			.send()
			.await
			.expect(&format!("Failed to send request {}", i + 1));

		assert_eq!(response.status(), 200);

		let connection_header = response.headers().get("Connection");
		assert!(
			connection_header.is_some(),
			"Connection header should be present"
		);

		let response_body = response.text().await.expect("Failed to read response body");
		assert_eq!(response_body, "keep-alive response");
	}
}

/// Test chunked transfer encoding
#[rstest]
#[tokio::test]
async fn test_chunked_transfer_encoding(http_client: reqwest::Client) {
	// Create test server with chunked handler
	let handler = Arc::new(ChunkedHandler);
	let server = TestServer::builder()
		.handler(handler)
		.build()
		.await
		.expect("Failed to create test server");

	// Send request
	let response = http_client
		.get(&server.url)
		.send()
		.await
		.expect("Failed to send request");

	// Verify response
	assert_eq!(response.status(), 200);

	let response_body = response.text().await.expect("Failed to read response body");
	assert_eq!(response_body, "chunked response body");
}

/// Test Expect: 100-continue handling
#[rstest]
#[tokio::test]
async fn test_expect_100_continue(http_client: reqwest::Client) {
	// Create test server with expect-continue handler
	let handler = Arc::new(ExpectContinueHandler);
	let server = TestServer::builder()
		.handler(handler)
		.build()
		.await
		.expect("Failed to create test server");

	// Send request with Expect: 100-continue header
	let request_body = "test body for expect continue";
	let response = http_client
		.post(&server.url)
		.header("Expect", "100-continue")
		.body(request_body)
		.send()
		.await
		.expect("Failed to send request");

	// Verify response
	assert_eq!(response.status(), 200);

	let expect_header = response.headers().get("X-Expect-Continue");
	assert!(
		expect_header.is_some(),
		"X-Expect-Continue header should be present"
	);
	assert_eq!(
		expect_header.unwrap().to_str().unwrap(),
		"true",
		"X-Expect-Continue should be true"
	);

	let response_body = response.text().await.expect("Failed to read response body");
	assert_eq!(
		response_body, request_body,
		"Response body should echo request body"
	);
}

/// Test multipart/form-data handling
#[rstest]
#[tokio::test]
async fn test_multipart_form_data(http_client: reqwest::Client) {
	// Create test server with multipart handler
	let handler = Arc::new(MultipartHandler);
	let server = TestServer::builder()
		.handler(handler)
		.build()
		.await
		.expect("Failed to create test server");

	// Create multipart form
	let file_content = b"file content".to_vec();
	let form = reqwest::multipart::Form::new()
		.text("field1", "value1")
		.text("field2", "value2")
		.part(
			"file",
			reqwest::multipart::Part::bytes(file_content)
				.file_name("test.txt")
				.mime_str("text/plain")
				.expect("Failed to set mime type"),
		);

	// Send multipart request
	let response = http_client
		.post(&server.url)
		.multipart(form)
		.send()
		.await
		.expect("Failed to send multipart request");

	// Verify response
	assert_eq!(response.status(), 200);

	let processed_header = response.headers().get("X-Multipart-Processed");
	assert!(
		processed_header.is_some(),
		"X-Multipart-Processed header should be present"
	);
	assert_eq!(
		processed_header.unwrap().to_str().unwrap(),
		"true",
		"Multipart should be processed"
	);

	let response_body = response.text().await.expect("Failed to read response body");
	assert!(
		response_body.contains("Received multipart data"),
		"Response should confirm multipart data received"
	);
	assert!(
		response_body.contains("multipart/form-data"),
		"Response should mention content type"
	);
}
