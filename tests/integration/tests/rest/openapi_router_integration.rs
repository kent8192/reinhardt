//! End-to-end HTTP integration tests for OpenApiRouter
//!
//! Tests that the OpenAPI documentation endpoints are correctly served over a real TCP stack.
//! These tests start an actual HTTP server on a random port and send real HTTP requests.

use reinhardt::OpenApiRouter;
use reinhardt_http::Handler;
use reinhardt_server::{HttpServer, ShutdownCoordinator};
use reinhardt_urls::routers::ServerRouter;
use rstest::rstest;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::task::JoinHandle;

// ============================================================================
// Test server helper for OpenApiRouter
// ============================================================================

/// Guard struct that shuts down the test server when dropped.
struct OpenApiServerGuard {
	/// Base URL of the running server (e.g., "http://127.0.0.1:PORT")
	pub url: String,
	coordinator: Arc<ShutdownCoordinator>,
	server_task: Option<JoinHandle<()>>,
}

impl Drop for OpenApiServerGuard {
	fn drop(&mut self) {
		self.coordinator.shutdown();
		if let Some(task) = self.server_task.take() {
			task.abort();
		}
	}
}

/// Spawn a test server wrapping an `OpenApiRouter` around an empty `ServerRouter`.
///
/// Binds to an OS-assigned random port (port 0) to avoid conflicts between
/// parallel test runs.
async fn openapi_server_guard() -> OpenApiServerGuard {
	// Bind early to obtain a fixed address before spawning the task
	let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
	let addr = listener.local_addr().unwrap();
	let url = format!("http://{}", addr);

	let coordinator = Arc::new(ShutdownCoordinator::new(Duration::from_secs(5)));

	// Build: OpenApiRouter wrapping an empty ServerRouter
	let router = ServerRouter::new();
	let openapi_router = OpenApiRouter::wrap(router).expect("OpenApiRouter::wrap should succeed");
	let handler: Arc<dyn Handler> = Arc::new(openapi_router);
	let server = HttpServer::new(handler);

	let server_coordinator = (*coordinator).clone();
	let server_task = tokio::spawn(async move {
		let mut shutdown_rx = server_coordinator.subscribe();
		loop {
			tokio::select! {
				result = listener.accept() => {
					match result {
						Ok((stream, socket_addr)) => {
							let handler_clone = server.handler();
							tokio::spawn(async move {
								if let Err(e) = HttpServer::handle_connection(
									stream,
									socket_addr,
									handler_clone,
									None,
								)
								.await
								{
									eprintln!("Connection error: {:?}", e);
								}
							});
						}
						Err(e) => {
							eprintln!("Accept error: {:?}", e);
							break;
						}
					}
				}
				_ = shutdown_rx.recv() => {
					break;
				}
			}
		}
	});

	// Allow time for the server to be ready
	tokio::time::sleep(Duration::from_millis(100)).await;

	OpenApiServerGuard {
		url,
		coordinator,
		server_task: Some(server_task),
	}
}

// ============================================================================
// End-to-end HTTP tests
// ============================================================================

/// Test 14: GET /api/openapi.json returns HTTP 200 over a real TCP connection
#[rstest]
#[tokio::test]
async fn test_openapi_json_returns_200_via_http() {
	// Arrange
	let server = openapi_server_guard().await;
	let client = reqwest::Client::new();

	// Act
	let response = client
		.get(format!("{}/api/openapi.json", server.url))
		.send()
		.await
		.expect("HTTP request should succeed");

	// Assert
	assert_eq!(
		response.status(),
		200,
		"GET /api/openapi.json should return 200"
	);
}

/// Test 15: GET /api/openapi.json response has Content-Type: application/json
#[rstest]
#[tokio::test]
async fn test_openapi_json_content_type_via_http() {
	// Arrange
	let server = openapi_server_guard().await;
	let client = reqwest::Client::new();

	// Act
	let response = client
		.get(format!("{}/api/openapi.json", server.url))
		.send()
		.await
		.expect("HTTP request should succeed");

	// Assert
	let content_type = response
		.headers()
		.get("Content-Type")
		.and_then(|v| v.to_str().ok())
		.unwrap_or("");
	assert!(
		content_type.contains("application/json"),
		"Content-Type should contain 'application/json', got: {}",
		content_type
	);
}

/// Test 16: GET /api/openapi.json body has an `openapi` field starting with "3."
#[rstest]
#[tokio::test]
async fn test_openapi_json_body_has_openapi_field_via_http() {
	// Arrange
	let server = openapi_server_guard().await;
	let client = reqwest::Client::new();

	// Act
	let response = client
		.get(format!("{}/api/openapi.json", server.url))
		.send()
		.await
		.expect("HTTP request should succeed");
	let body: serde_json::Value = response.json().await.expect("Body should be valid JSON");

	// Assert
	let openapi_version = body["openapi"]
		.as_str()
		.expect("JSON body should have an 'openapi' string field");
	assert!(
		openapi_version.starts_with("3."),
		"openapi field should start with '3.', got: {}",
		openapi_version
	);
}

/// Test 17: GET /api/docs returns HTTP 200 (Swagger UI)
#[rstest]
#[tokio::test]
async fn test_swagger_docs_returns_200_via_http() {
	// Arrange
	let server = openapi_server_guard().await;
	let client = reqwest::Client::new();

	// Act
	let response = client
		.get(format!("{}/api/docs", server.url))
		.send()
		.await
		.expect("HTTP request should succeed");

	// Assert
	assert_eq!(response.status(), 200, "GET /api/docs should return 200");
}

/// Test 18: GET /api/redoc returns HTTP 200 (Redoc UI)
#[rstest]
#[tokio::test]
async fn test_redoc_docs_returns_200_via_http() {
	// Arrange
	let server = openapi_server_guard().await;
	let client = reqwest::Client::new();

	// Act
	let response = client
		.get(format!("{}/api/redoc", server.url))
		.send()
		.await
		.expect("HTTP request should succeed");

	// Assert
	assert_eq!(response.status(), 200, "GET /api/redoc should return 200");
}

/// Test 19: Disabled OpenApiRouter returns 404 for all documentation paths via real HTTP
#[rstest]
#[case("/api/openapi.json")]
#[case("/api/docs")]
#[case("/api/redoc")]
#[tokio::test]
async fn test_disabled_endpoints_return_404_via_http(#[case] path: &str) {
	// Arrange: bind early
	let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
	let addr = listener.local_addr().unwrap();
	let url = format!("http://{}", addr);

	let coordinator = Arc::new(ShutdownCoordinator::new(Duration::from_secs(5)));

	// Build router with enabled(false)
	let router = ServerRouter::new();
	let openapi_router = OpenApiRouter::wrap(router)
		.expect("OpenApiRouter::wrap should succeed")
		.enabled(false);
	let handler: Arc<dyn Handler> = Arc::new(openapi_router);
	let server = HttpServer::new(handler);

	let server_coordinator = (*coordinator).clone();
	let server_task: JoinHandle<()> = tokio::spawn(async move {
		let mut shutdown_rx = server_coordinator.subscribe();
		loop {
			tokio::select! {
				result = listener.accept() => {
					match result {
						Ok((stream, socket_addr)) => {
							let handler_clone = server.handler();
							tokio::spawn(async move {
								let _ = HttpServer::handle_connection(
									stream,
									socket_addr,
									handler_clone,
									None,
								)
								.await;
							});
						}
						Err(_) => break,
					}
				}
				_ = shutdown_rx.recv() => {
					break;
				}
			}
		}
	});

	tokio::time::sleep(Duration::from_millis(100)).await;

	// Act
	let client = reqwest::Client::new();
	let response = client
		.get(format!("{}{}", url, path))
		.send()
		.await
		.expect("HTTP request should succeed");

	// Assert
	assert_eq!(
		response.status(),
		404,
		"Disabled endpoint {} should return 404",
		path
	);

	// Cleanup
	coordinator.shutdown();
	server_task.abort();
}
