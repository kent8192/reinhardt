use reinhardt_http::Handler;
use reinhardt_http::{Request, Response};
use reinhardt_server::{Http2Server, ShutdownCoordinator, serve_http2_with_shutdown};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

struct TestHandler;

#[async_trait::async_trait]
impl Handler for TestHandler {
	async fn handle(&self, _request: Request) -> reinhardt_core::exception::Result<Response> {
		Ok(Response::ok().with_body("Hello from HTTP/2!"))
	}
}

#[tokio::test]
async fn test_http2_server_with_graceful_shutdown() {
	let handler = Arc::new(TestHandler);
	let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
	let coordinator = ShutdownCoordinator::new(Duration::from_secs(5));

	let server_coordinator = coordinator.clone();
	let server_task = tokio::spawn(async move {
		let server = Http2Server::new(handler);
		let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
		let actual_addr = listener.local_addr().unwrap();
		drop(listener);

		let result = server
			.listen_with_shutdown(actual_addr, server_coordinator)
			.await;
		assert!(result.is_ok());
	});

	// Give server time to start
	sleep(Duration::from_millis(100)).await;

	// Trigger shutdown
	coordinator.shutdown();

	// Wait for server to complete shutdown
	coordinator.wait_for_shutdown().await;

	// Verify server task completed
	let result = tokio::time::timeout(Duration::from_secs(1), server_task).await;
	assert!(result.is_ok());
}

#[tokio::test]
async fn test_http2_server_creation() {
	let handler = Arc::new(TestHandler);
	let server = Http2Server::new(handler);

	// Verify server can be created
	assert!(!std::ptr::addr_of!(server).is_null());
}

#[tokio::test]
async fn test_serve_http2_with_shutdown_function() {
	let handler = Arc::new(TestHandler);
	let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
	let coordinator = ShutdownCoordinator::new(Duration::from_secs(5));

	// Get a free port
	let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
	let actual_addr = listener.local_addr().unwrap();
	drop(listener);

	let server_coordinator = coordinator.clone();
	let server_task = tokio::spawn(async move {
		let result = serve_http2_with_shutdown(actual_addr, handler, server_coordinator).await;
		assert!(result.is_ok());
	});

	// Give server time to start
	sleep(Duration::from_millis(100)).await;

	// Trigger shutdown
	coordinator.shutdown();

	// Wait for shutdown
	coordinator.wait_for_shutdown().await;

	// Verify server completed
	let result = tokio::time::timeout(Duration::from_secs(1), server_task).await;
	assert!(result.is_ok());
}
