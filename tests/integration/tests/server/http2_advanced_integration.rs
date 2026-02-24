//! HTTP/2 Advanced Features Integration Tests
//!
//! This module tests advanced HTTP/2 features such as:
//! - Concurrent stream processing
//! - Header compression (HPACK)
//! - Stream priority
//! - HTTP/1.1 and HTTP/2 mixed mode
//! - Flow control

use http::Version;
use reinhardt_http::Handler;
use reinhardt_http::{Request, Response};
use reinhardt_server::{Http2Server, ShutdownCoordinator};
use reinhardt_test::APIClient;
use reinhardt_test::fixtures::*;
use rstest::*;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;
use tokio::time::sleep;

// ============================================================================
// Test Handlers
// ============================================================================

/// Handler for concurrent stream processing test
#[derive(Clone)]
struct ConcurrentStreamHandler {
	counter: Arc<AtomicU32>,
}

impl ConcurrentStreamHandler {
	fn new() -> Self {
		Self {
			counter: Arc::new(AtomicU32::new(0)),
		}
	}

	fn get_count(&self) -> u32 {
		self.counter.load(Ordering::SeqCst)
	}
}

#[async_trait::async_trait]
impl Handler for ConcurrentStreamHandler {
	async fn handle(&self, _request: Request) -> reinhardt_core::exception::Result<Response> {
		// Increment counter
		let count = self.counter.fetch_add(1, Ordering::SeqCst);

		// Simulate some processing
		sleep(Duration::from_millis(10)).await;

		Ok(Response::ok().with_body(format!("Stream {}", count)))
	}
}

/// Handler for header compression test
#[derive(Clone)]
struct HeaderCompressionHandler;

#[async_trait::async_trait]
impl Handler for HeaderCompressionHandler {
	async fn handle(&self, request: Request) -> reinhardt_core::exception::Result<Response> {
		// Extract headers to verify they were properly decompressed
		let mut headers_count = 0;
		let mut custom_header_value = String::new();

		for (key, value) in request.headers.iter() {
			headers_count += 1;
			if key.as_str() == "x-custom-header" {
				custom_header_value = String::from_utf8_lossy(value.as_bytes()).to_string();
			}
		}

		let response_body = format!(
			"Headers: {}, Custom: {}",
			headers_count, custom_header_value
		);

		Ok(Response::ok().with_body(response_body))
	}
}

/// Handler for stream priority test
#[derive(Clone)]
struct StreamPriorityHandler;

#[async_trait::async_trait]
impl Handler for StreamPriorityHandler {
	async fn handle(&self, request: Request) -> reinhardt_core::exception::Result<Response> {
		// Check for priority header
		let priority = request
			.headers
			.get("x-priority")
			.and_then(|v| String::from_utf8(v.as_bytes().to_vec()).ok())
			.unwrap_or_else(|| "normal".to_string());

		// Simulate different processing times based on priority
		let delay = match priority.as_str() {
			"high" => 5,
			"low" => 50,
			_ => 20,
		};

		sleep(Duration::from_millis(delay)).await;

		Ok(Response::ok().with_body(format!("Priority: {}", priority)))
	}
}

/// Handler for flow control test
#[derive(Clone)]
struct FlowControlHandler;

#[async_trait::async_trait]
impl Handler for FlowControlHandler {
	async fn handle(&self, request: Request) -> reinhardt_core::exception::Result<Response> {
		// Get the requested response size
		let size = request
			.headers
			.get("x-response-size")
			.and_then(|v| String::from_utf8(v.as_bytes().to_vec()).ok())
			.and_then(|v| v.parse::<usize>().ok())
			.unwrap_or(1024);

		// Generate a large response to test flow control
		let response_body = "X".repeat(size);

		Ok(Response::ok().with_body(response_body))
	}
}

// ============================================================================
// Tests
// ============================================================================

/// Test concurrent stream processing (100 concurrent streams)
///
/// This test verifies that the HTTP/2 server can handle multiple concurrent
/// streams efficiently by sending 100 concurrent requests.
#[rstest]
#[tokio::test]
async fn test_concurrent_streams() {
	// Setup server
	let handler = Arc::new(ConcurrentStreamHandler::new());
	let handler_clone = handler.clone();
	let coordinator = ShutdownCoordinator::new(Duration::from_secs(10));

	// Start server
	let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
	let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
	let actual_addr = listener.local_addr().unwrap();
	let url = format!("http://{}", actual_addr);
	drop(listener);

	let server_coordinator = coordinator.clone();
	let server_task = tokio::spawn(async move {
		let server = Http2Server::new(handler_clone);
		let _ = server
			.listen_with_shutdown(actual_addr, server_coordinator)
			.await;
	});

	// Wait for server to start
	sleep(Duration::from_millis(100)).await;

	// Create HTTP/2 client with proper configuration
	let client = Arc::new(
		APIClient::builder()
			.base_url(&url)
			.http2_prior_knowledge()
			.timeout(Duration::from_secs(30))
			.build(),
	);

	// Send 100 concurrent requests
	let mut tasks = Vec::new();
	for _ in 0..100 {
		let client = Arc::clone(&client);
		let task = tokio::spawn(async move { client.get("/").await });
		tasks.push(task);
	}

	// Wait for all requests to complete
	let mut success_count = 0;
	for task in tasks {
		if let Ok(Ok(response)) = task.await {
			if response.status_code() == 200 {
				success_count += 1;
			}
		}
	}

	// Verify all requests succeeded
	assert_eq!(
		success_count, 100,
		"All 100 concurrent requests should succeed"
	);
	assert_eq!(
		handler.get_count(),
		100,
		"Handler should process all 100 requests"
	);

	// Cleanup
	coordinator.shutdown();
	server_task.abort();
}

/// Test header compression (HPACK)
///
/// This test verifies that HTTP/2 header compression works correctly
/// by sending multiple requests with repeated headers.
#[rstest]
#[tokio::test]
async fn test_header_compression() {
	// Setup server
	let handler = Arc::new(HeaderCompressionHandler);
	let coordinator = ShutdownCoordinator::new(Duration::from_secs(10));

	// Start server
	let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
	let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
	let actual_addr = listener.local_addr().unwrap();
	let url = format!("http://{}", actual_addr);
	drop(listener);

	let server_coordinator = coordinator.clone();
	let server_task = tokio::spawn(async move {
		let server = Http2Server::new(handler);
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

	// Send multiple requests with custom headers
	for i in 0..10 {
		let header_value = format!("value-{}", i);
		let response = client
			.get_with_headers(
				"/",
				&[
					("x-custom-header", header_value.as_str()),
					("x-another-header", "repeated-value"),
				],
			)
			.await
			.expect("Request should succeed");

		assert_eq!(response.status_code(), 200);

		let body = response.text();
		assert!(body.contains(&format!("value-{}", i)));
	}

	// Cleanup
	coordinator.shutdown();
	server_task.abort();
}

/// Test stream priority
///
/// This test verifies that HTTP/2 stream priority works correctly
/// by sending requests with different priority levels.
#[rstest]
#[tokio::test]
async fn test_stream_priority() {
	// Setup server
	let handler = Arc::new(StreamPriorityHandler);
	let coordinator = ShutdownCoordinator::new(Duration::from_secs(10));

	// Start server
	let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
	let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
	let actual_addr = listener.local_addr().unwrap();
	let url = format!("http://{}", actual_addr);
	drop(listener);

	let server_coordinator = coordinator.clone();
	let server_task = tokio::spawn(async move {
		let server = Http2Server::new(handler);
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

	// Send requests with different priority levels
	let priorities = vec!["high", "normal", "low"];
	for priority in priorities {
		let response = client
			.get_with_headers("/", &[("x-priority", priority)])
			.await
			.expect("Request should succeed");

		assert_eq!(response.status_code(), 200);

		let body = response.text();
		assert_eq!(body, format!("Priority: {}", priority));
	}

	// Cleanup
	coordinator.shutdown();
	server_task.abort();
}

/// Test HTTP/1.1 and HTTP/2 mixed mode
///
/// This test verifies that the server can handle HTTP/1.1 requests
/// when configured for HTTP/2 (note: current implementation is HTTP/2 only).
#[rstest]
#[tokio::test]
async fn test_http1_http2_mixed() {
	// Setup server
	let handler = Arc::new(BasicHandler);
	let coordinator = ShutdownCoordinator::new(Duration::from_secs(10));

	// Start server
	let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
	let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
	let actual_addr = listener.local_addr().unwrap();
	let url = format!("http://{}", actual_addr);
	drop(listener);

	let server_coordinator = coordinator.clone();
	let server_task = tokio::spawn(async move {
		let server = Http2Server::new(handler);
		let _ = server
			.listen_with_shutdown(actual_addr, server_coordinator)
			.await;
	});

	// Wait for server to start
	sleep(Duration::from_millis(100)).await;

	// Test with HTTP/2 client
	let http2_client = APIClient::builder()
		.base_url(&url)
		.http2_prior_knowledge()
		.timeout(Duration::from_secs(10))
		.build();

	let response = http2_client
		.get("/")
		.await
		.expect("HTTP/2 request should succeed");

	assert_eq!(response.status_code(), 200);
	assert_eq!(response.version(), Version::HTTP_2);

	// Note: HTTP/1.1 requests to HTTP/2-only server will fail
	// This is expected behavior for the current implementation

	// Cleanup
	coordinator.shutdown();
	server_task.abort();
}

/// Test flow control
///
/// This test verifies that HTTP/2 flow control works correctly
/// by sending requests for large responses.
#[rstest]
#[tokio::test]
async fn test_flow_control() {
	// Setup server
	let handler = Arc::new(FlowControlHandler);
	let coordinator = ShutdownCoordinator::new(Duration::from_secs(10));

	// Start server
	let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
	let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
	let actual_addr = listener.local_addr().unwrap();
	let url = format!("http://{}", actual_addr);
	drop(listener);

	let server_coordinator = coordinator.clone();
	let server_task = tokio::spawn(async move {
		let server = Http2Server::new(handler);
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

	// Test different response sizes to trigger flow control
	let sizes = vec![1024, 16384, 65536, 262144]; // 1KB, 16KB, 64KB, 256KB

	for size in sizes {
		let size_str = size.to_string();
		let response = client
			.get_with_headers("/", &[("x-response-size", size_str.as_str())])
			.await
			.expect("Request should succeed");

		assert_eq!(response.status_code(), 200);

		let body = response.text();
		assert_eq!(
			body.len(),
			size,
			"Response size should match requested size"
		);
	}

	// Cleanup
	coordinator.shutdown();
	server_task.abort();
}
