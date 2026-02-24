use reinhardt_http::Handler;
use reinhardt_http::{Request, Response};
use reinhardt_server::{HttpServer, ShutdownCoordinator, serve_with_shutdown};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

struct TestHandler;

#[async_trait::async_trait]
impl Handler for TestHandler {
	async fn handle(&self, _request: Request) -> reinhardt_core::exception::Result<Response> {
		Ok(Response::ok().with_body("Hello, World!"))
	}
}

#[tokio::test]
async fn test_server_with_graceful_shutdown() {
	let handler = Arc::new(TestHandler);
	let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
	let coordinator = ShutdownCoordinator::new(Duration::from_secs(5));

	let server_coordinator = coordinator.clone();
	let server_task = tokio::spawn(async move {
		let server = HttpServer::new(handler);
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
async fn test_shutdown_coordinator_timeout() {
	let coordinator = ShutdownCoordinator::new(Duration::from_millis(100));

	// Don't notify completion - should timeout
	let start = std::time::Instant::now();
	coordinator.wait_for_shutdown().await;
	let elapsed = start.elapsed();

	assert!(elapsed >= Duration::from_millis(100));
	assert!(elapsed < Duration::from_millis(200));
}

#[tokio::test]
async fn test_shutdown_signal_broadcast() {
	let coordinator = ShutdownCoordinator::new(Duration::from_secs(1));
	let mut rx1 = coordinator.subscribe();
	let mut rx2 = coordinator.subscribe();
	let mut rx3 = coordinator.subscribe();

	coordinator.shutdown();

	// All receivers should get the signal
	assert!(rx1.recv().await.is_ok());
	assert!(rx2.recv().await.is_ok());
	assert!(rx3.recv().await.is_ok());
}

#[tokio::test]
async fn test_multiple_shutdown_calls() {
	let coordinator = ShutdownCoordinator::new(Duration::from_secs(1));

	// Subscribe before sending
	let mut rx1 = coordinator.subscribe();
	let mut rx2 = coordinator.subscribe();

	// First shutdown call sends the signal
	coordinator.shutdown();

	// Both receivers should get the signal
	assert!(rx1.recv().await.is_ok());
	assert!(rx2.recv().await.is_ok());

	// Subsequent shutdown calls are safe (no-op since channel is closed after first send)
	coordinator.shutdown();
	coordinator.shutdown();
}

#[tokio::test]
async fn test_shutdown_coordinator_clone() {
	let coordinator = ShutdownCoordinator::new(Duration::from_secs(1));
	let coordinator_clone = coordinator.clone();

	let mut rx = coordinator.subscribe();

	// Shutdown via clone
	coordinator_clone.shutdown();

	// Should receive signal from original
	assert!(rx.recv().await.is_ok());
}

#[tokio::test]
async fn test_serve_with_shutdown_function() {
	let handler = Arc::new(TestHandler);
	let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
	let coordinator = ShutdownCoordinator::new(Duration::from_secs(5));

	// Get a free port
	let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
	let actual_addr = listener.local_addr().unwrap();
	drop(listener);

	let server_coordinator = coordinator.clone();
	let server_task = tokio::spawn(async move {
		let result = serve_with_shutdown(actual_addr, handler, server_coordinator).await;
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
