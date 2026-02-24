//! Server test fixtures with automatic graceful shutdown.
//!
//! This module provides rstest fixtures for testing HTTP servers with automatic
//! cleanup via RAII pattern.

use reinhardt_di::InjectionContext;
use reinhardt_http::Handler;
use reinhardt_http::{Request, Response};
use reinhardt_server::{
	HttpServer, RateLimitConfig, RateLimitHandler, ShutdownCoordinator, TimeoutHandler,
};
use reinhardt_urls::routers::ServerRouter as Router;
use rstest::fixture;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::task::JoinHandle;

#[cfg(feature = "websockets")]
use reinhardt_server::WebSocketServer;

#[cfg(feature = "graphql")]
use reinhardt_server::GraphQLHandler;

/// Test server guard with automatic graceful shutdown.
///
/// This guard automatically performs graceful shutdown when dropped, ensuring
/// proper cleanup of server resources even if the test panics.
///
/// # Examples
///
/// ```no_run
/// use reinhardt_test::fixtures::*;
/// use reinhardt_urls::routers::ServerRouter as Router;
///
/// #[tokio::test]
/// async fn test_example() {
///     let router = Router::new();
///     let server = test_server_guard(router).await;
///     let response = reqwest::get(&format!("{}/test", server.url))
///         .await
///         .unwrap();
///     assert_eq!(response.status(), 200);
///     // Automatic graceful shutdown when server goes out of scope
/// }
/// ```
pub struct TestServerGuard {
	/// Server URL (e.g., "http://127.0.0.1:12345")
	pub url: String,
	/// Shutdown coordinator for graceful shutdown
	pub coordinator: Arc<ShutdownCoordinator>,
	/// Server task handle
	server_task: Option<JoinHandle<()>>,
}

impl TestServerGuard {
	/// Create a new test server guard.
	///
	/// This function:
	/// 1. Binds to a random port (127.0.0.1:0)
	/// 2. Creates a ShutdownCoordinator
	/// 3. Spawns the server task
	/// 4. Waits 100ms for the server to start
	///
	/// # Arguments
	///
	/// * `router` - Router to use for handling requests
	async fn new(router: Router) -> Self {
		let shutdown_timeout = Duration::from_secs(5);
		// Bind to random port and keep the listener to avoid TOCTOU race
		let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
		let actual_addr = listener.local_addr().unwrap();
		let url = format!("http://{}", actual_addr);

		// Create shutdown coordinator
		let coordinator = Arc::new(ShutdownCoordinator::new(shutdown_timeout));

		// Spawn server using the already-bound listener to avoid port race
		let server_coordinator = (*coordinator).clone();
		let handler: Arc<dyn Handler> = Arc::new(router);
		let server = HttpServer::new(handler);
		let mut shutdown_rx = server_coordinator.subscribe();
		let server_task = tokio::spawn(async move {
			loop {
				tokio::select! {
					result = listener.accept() => {
						match result {
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
					_ = shutdown_rx.recv() => {
						break;
					}
				}
			}
		});

		// Wait for server to start
		tokio::time::sleep(Duration::from_millis(100)).await;

		Self {
			url,
			coordinator,
			server_task: Some(server_task),
		}
	}
}

impl Drop for TestServerGuard {
	fn drop(&mut self) {
		// Trigger shutdown signal
		self.coordinator.shutdown();

		// Abort the server task
		// The ShutdownCoordinator will handle graceful shutdown,
		// but we need to ensure the task is terminated
		if let Some(task) = self.server_task.take() {
			task.abort();
		}
	}
}

/// Create a test server guard with the given router.
///
/// This is a helper function (not an rstest fixture) that creates a test server
/// with automatic graceful shutdown. Use it directly in your tests.
///
/// # Examples
///
/// ```no_run
/// use reinhardt_test::fixtures::*;
/// use reinhardt_urls::routers::ServerRouter as Router;
///
/// #[tokio::test]
/// async fn test_server() {
///     let router = Router::new();
///     let server = test_server_guard(router).await;
///     let response = reqwest::get(&format!("{}/hello", server.url))
///         .await
///         .unwrap();
///     assert_eq!(response.status(), 200);
///     // Automatic cleanup on drop
/// }
/// ```
pub async fn test_server_guard(router: Router) -> TestServerGuard {
	TestServerGuard::new(router).await
}

// ============================================================================
// Basic Test Handlers
// ============================================================================

/// Basic handler for testing purposes that returns "OK"
#[derive(Clone)]
pub struct BasicHandler;

#[async_trait::async_trait]
impl Handler for BasicHandler {
	async fn handle(&self, _request: Request) -> reinhardt_core::exception::Result<Response> {
		Ok(Response::ok().with_body("OK"))
	}
}

// ============================================================================
// Client Fixtures
// ============================================================================

/// HTTP client fixture for testing HTTP requests
///
/// # Examples
///
/// ```no_run
/// use reinhardt_test::fixtures::*;
/// use rstest::*;
///
/// #[rstest]
/// #[tokio::test]
/// async fn test_with_client(http_client: reqwest::Client) {
///     let response = http_client
///         .get("http://localhost:8080/api/test")
///         .send()
///         .await
///         .unwrap();
///     assert_eq!(response.status(), 200);
/// }
/// ```
#[fixture]
pub fn http_client() -> reqwest::Client {
	reqwest::Client::builder()
		.timeout(Duration::from_secs(10))
		.build()
		.expect("Failed to create HTTP client")
}
// ============================================================================
// HTTP/1.1 Server Fixtures
// ============================================================================

/// HTTP/1.1 test server fixture
///
/// # Examples
///
/// ```no_run
/// use reinhardt_test::fixtures::*;
/// use rstest::*;
///
/// #[rstest]
/// #[tokio::test]
/// async fn test_http1_server(#[future] http1_server: TestServer) {
///     let server = http1_server.await;
///     let client = reqwest::Client::new();
///     let response = client.get(&server.url).send().await.unwrap();
///     assert_eq!(response.status(), 200);
/// }
/// ```
#[fixture]
pub async fn http1_server() -> TestServer {
	let handler = Arc::new(BasicHandler);
	TestServer::builder()
		.handler(handler)
		.build()
		.await
		.expect("Failed to create HTTP/1.1 server")
}

// ============================================================================
// HTTP/2 Server Fixtures
// ============================================================================

/// HTTP/2 test server fixture
///
/// # Examples
///
/// ```no_run
/// use reinhardt_test::fixtures::*;
/// use rstest::*;
///
/// #[rstest]
/// #[tokio::test]
/// async fn test_http2_server(#[future] http2_server: TestServer) {
///     let server = http2_server.await;
///     // Test with HTTP/2 client
/// }
/// ```
#[fixture]
pub async fn http2_server() -> TestServer {
	let handler = Arc::new(BasicHandler);
	TestServer::builder()
		.handler(handler)
		.http2(true)
		.build()
		.await
		.expect("Failed to create HTTP/2 server")
}

// ============================================================================
// Middleware Server Fixtures
// ============================================================================

/// Server fixture with timeout middleware
///
/// Default timeout: 5 seconds
///
/// # Examples
///
/// ```no_run
/// use reinhardt_test::fixtures::*;
/// use rstest::*;
///
/// #[rstest]
/// #[tokio::test]
/// async fn test_timeout(#[future] server_with_timeout: TestServer) {
///     let server = server_with_timeout.await;
///     // Timeout test
/// }
/// ```
#[fixture]
pub async fn server_with_timeout(
	#[default(Duration::from_secs(5))] timeout: Duration,
) -> TestServer {
	let handler = Arc::new(BasicHandler);
	let timeout_handler = Arc::new(TimeoutHandler::new(handler, timeout));
	TestServer::builder()
		.handler(timeout_handler)
		.build()
		.await
		.expect("Failed to create server with timeout")
}

/// Server fixture with rate limit middleware
///
/// Default rate limit: 100 requests/minute
///
/// # Examples
///
/// ```no_run
/// use reinhardt_test::fixtures::*;
/// use rstest::*;
///
/// #[rstest]
/// #[tokio::test]
/// async fn test_rate_limit(#[future] server_with_rate_limit: TestServer) {
///     let server = server_with_rate_limit.await;
///     // Rate limit test
/// }
/// ```
#[fixture]
pub async fn server_with_rate_limit(#[default(100)] limit: u32) -> TestServer {
	let handler = Arc::new(BasicHandler);
	let config = RateLimitConfig::per_minute(limit as usize);
	let rate_limit_handler = Arc::new(RateLimitHandler::new(handler, config));
	TestServer::builder()
		.handler(rate_limit_handler)
		.build()
		.await
		.expect("Failed to create server with rate limit")
}

/// Server fixture with middleware chain (Timeout + RateLimit)
///
/// # Examples
///
/// ```no_run
/// use reinhardt_test::fixtures::*;
/// use rstest::*;
///
/// #[rstest]
/// #[tokio::test]
/// async fn test_middleware_chain(#[future] server_with_middleware_chain: TestServer) {
///     let server = server_with_middleware_chain.await;
///     // Middleware chain test
/// }
/// ```
#[fixture]
pub async fn server_with_middleware_chain() -> TestServer {
	let handler = Arc::new(BasicHandler);
	let timeout_handler = Arc::new(TimeoutHandler::new(handler, Duration::from_secs(5)));
	let config = RateLimitConfig::per_minute(100);
	let rate_limit_handler = Arc::new(RateLimitHandler::new(timeout_handler, config));

	TestServer::builder()
		.handler(rate_limit_handler)
		.build()
		.await
		.expect("Failed to create server with middleware chain")
}

/// Server fixture with DI context
///
/// # Examples
///
/// ```no_run
/// use reinhardt_test::fixtures::*;
/// use rstest::*;
///
/// #[rstest]
/// #[tokio::test]
/// async fn test_di_context(#[future] server_with_di: (TestServer, Arc<InjectionContext>)) {
///     let (server, di_context) = server_with_di.await;
///     // DI context test
/// }
/// ```
#[fixture]
pub async fn server_with_di() -> (TestServer, Arc<InjectionContext>) {
	use reinhardt_di::SingletonScope;

	let handler = Arc::new(BasicHandler);
	let di_context = Arc::new(InjectionContext::builder(Arc::new(SingletonScope::new())).build());

	let server = TestServer::builder()
		.handler(handler)
		.di_context(di_context.clone())
		.build()
		.await
		.expect("Failed to create server with DI context");

	(server, di_context)
}

// ============================================================================
// WebSocket Server Fixtures (feature: websocket)
// ============================================================================

#[cfg(feature = "websockets")]
/// WebSocket-enabled server fixture
///
/// # Examples
///
/// ```no_run
/// use reinhardt_test::fixtures::*;
/// use rstest::*;
///
/// #[rstest]
/// #[tokio::test]
/// async fn test_websocket_server(#[future] websocket_server: TestServer) {
///     let server = websocket_server.await;
///     // WebSocket test
/// }
/// ```
#[fixture]
pub async fn websocket_server() -> TestServer {
	use reinhardt_server::WebSocketHandler;

	#[derive(Clone)]
	struct EchoHandler;

	#[async_trait::async_trait]
	impl WebSocketHandler for EchoHandler {
		async fn handle_message(&self, message: String) -> Result<String, String> {
			Ok(message) // Echo back
		}

		async fn on_connect(&self) {}
		async fn on_disconnect(&self) {}
	}

	let ws_handler = Arc::new(EchoHandler);
	TestServer::builder()
		.websocket_handler(ws_handler)
		.build()
		.await
		.expect("Failed to create WebSocket server")
}

#[cfg(feature = "websockets")]
/// WebSocket client fixture
///
/// # Examples
///
/// ```no_run
/// use reinhardt_test::fixtures::*;
/// use rstest::*;
///
/// #[rstest]
/// #[tokio::test]
/// async fn test_websocket_client(websocket_client: tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>) {
///     // WebSocket client test
/// }
/// ```
#[fixture]
pub async fn websocket_client(
	#[from(websocket_server)]
	#[future]
	server: TestServer,
) -> tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>> {
	let server = server.await;
	let ws_url = server.url.replace("http://", "ws://");
	let (ws_stream, _) = tokio_tungstenite::connect_async(&ws_url)
		.await
		.expect("Failed to connect WebSocket");
	ws_stream
}

// ============================================================================
// GraphQL Server Fixtures (feature: graphql)
// ============================================================================

#[cfg(feature = "graphql")]
/// GraphQL-enabled server fixture
///
/// # Examples
///
/// ```no_run
/// use reinhardt_test::fixtures::*;
/// use rstest::*;
///
/// #[rstest]
/// #[tokio::test]
/// async fn test_graphql_server(#[future] graphql_server: TestServer) {
///     let server = graphql_server.await;
///     // GraphQL test
/// }
/// ```
#[cfg(feature = "graphql")]
#[fixture]
pub async fn graphql_server() -> TestServer {
	use async_graphql::{EmptyMutation, EmptySubscription, Object, Schema};

	struct Query;

	#[Object]
	impl Query {
		async fn hello(&self) -> &'static str {
			"Hello, GraphQL!"
		}
	}

	let schema = Schema::build(Query, EmptyMutation, EmptySubscription).finish();
	let graphql_handler = Arc::new(GraphQLHandler::new(schema));

	TestServer::builder()
		.handler(graphql_handler)
		.build()
		.await
		.expect("Failed to create GraphQL server")
}

// ============================================================================
// TestServer Structure with Builder Pattern
// ============================================================================

/// Test server with automatic graceful shutdown
pub struct TestServer {
	/// Server URL (e.g., "http://127.0.0.1:12345")
	pub url: String,
	/// Server address
	pub addr: SocketAddr,
	/// Shutdown coordinator
	pub coordinator: Arc<ShutdownCoordinator>,
	/// Server task handle
	server_task: Option<JoinHandle<()>>,
}

impl TestServer {
	/// Create a new TestServerBuilder
	pub fn builder() -> TestServerBuilder {
		TestServerBuilder::new()
	}
}

impl Drop for TestServer {
	fn drop(&mut self) {
		// Trigger shutdown signal
		self.coordinator.shutdown();

		// Abort the server task
		if let Some(task) = self.server_task.take() {
			task.abort();
		}
	}
}

/// Builder for TestServer
pub struct TestServerBuilder {
	handler: Option<Arc<dyn Handler>>,
	#[cfg(feature = "websockets")]
	websocket_handler: Option<Arc<dyn reinhardt_server::WebSocketHandler>>,
	di_context: Option<Arc<InjectionContext>>,
	http2: bool,
	shutdown_timeout: Duration,
}

impl TestServerBuilder {
	fn new() -> Self {
		Self {
			handler: None,
			#[cfg(feature = "websockets")]
			websocket_handler: None,
			di_context: None,
			http2: false,
			shutdown_timeout: Duration::from_secs(5),
		}
	}

	/// Set the handler for HTTP requests
	pub fn handler(mut self, handler: Arc<dyn Handler>) -> Self {
		self.handler = Some(handler);
		self
	}

	#[cfg(feature = "websockets")]
	/// Set the WebSocket handler
	pub fn websocket_handler(
		mut self,
		handler: Arc<dyn reinhardt_server::WebSocketHandler>,
	) -> Self {
		self.websocket_handler = Some(handler);
		self
	}

	/// Set the DI context
	pub fn di_context(mut self, context: Arc<InjectionContext>) -> Self {
		self.di_context = Some(context);
		self
	}

	/// Enable HTTP/2
	pub fn http2(mut self, enabled: bool) -> Self {
		self.http2 = enabled;
		self
	}

	/// Set shutdown timeout
	pub fn shutdown_timeout(mut self, timeout: Duration) -> Self {
		self.shutdown_timeout = timeout;
		self
	}

	/// Build the TestServer
	pub async fn build(self) -> Result<TestServer, Box<dyn std::error::Error>> {
		// Bind to random port and keep the listener to avoid TOCTOU race
		let listener = TcpListener::bind("127.0.0.1:0").await?;
		let actual_addr = listener.local_addr()?;
		let url = format!("http://{}", actual_addr);

		// Create shutdown coordinator
		let coordinator = Arc::new(ShutdownCoordinator::new(self.shutdown_timeout));

		// Spawn server based on configuration
		let server_coordinator = (*coordinator).clone();

		#[cfg(feature = "websockets")]
		let websocket_handler = self.websocket_handler;

		let handler = self.handler;
		let di_context = self.di_context;
		let http2 = self.http2;

		let server_task = tokio::spawn(async move {
			// For WebSocket and HTTP/2 servers, we must drop the listener and re-bind
			// because their APIs only accept SocketAddr. This has a small TOCTOU window
			// but these server types are rarely used in parallel tests.
			#[cfg(feature = "websockets")]
			if let Some(ws_handler) = websocket_handler {
				drop(listener);
				let server = WebSocketServer::from_arc(ws_handler);
				let _ = server
					.listen_with_shutdown(actual_addr, server_coordinator)
					.await;
				return;
			}

			if let Some(h) = handler {
				if http2 {
					drop(listener);
					let server = reinhardt_server::Http2Server::new(h);
					let _ = server
						.listen_with_shutdown(actual_addr, server_coordinator)
						.await;
				} else {
					// Use the already-bound listener directly to avoid TOCTOU race
					let server = HttpServer::new(h);
					let mut shutdown_rx = server_coordinator.subscribe();
					loop {
						tokio::select! {
							result = listener.accept() => {
								match result {
									Ok((stream, socket_addr)) => {
										let handler_clone = server.handler();
										let di_ctx = di_context.clone();
										tokio::spawn(async move {
											if let Err(e) =
												HttpServer::handle_connection(stream, socket_addr, handler_clone, di_ctx)
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
							_ = shutdown_rx.recv() => {
								break;
							}
						}
					}
				}
			}
		});

		// Wait for server to start
		tokio::time::sleep(Duration::from_millis(100)).await;

		Ok(TestServer {
			url,
			addr: actual_addr,
			coordinator,
			server_task: Some(server_task),
		})
	}
}
