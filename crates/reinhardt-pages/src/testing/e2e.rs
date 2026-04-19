//! End-to-End Test Infrastructure
//!
//! This module provides utilities for running end-to-end tests with
//! real server and WASM frontend (Layer 3 testing).
//!
//! # Architecture
//!
//! E2E tests consist of:
//! 1. A real server running on a random port
//! 2. WASM tests running in a browser that make actual HTTP requests
//!
//! # Server-Side Usage
//!
//! ```rust,ignore
//! use reinhardt_pages::testing::e2e::{E2ETestEnv, E2ETestConfig};
//! use reinhardt_urls::routers::ServerRouter;
//!
//! #[tokio::test]
//! async fn test_e2e_flow() {
//!     let router = create_app_router();
//!     let config = E2ETestConfig::default();
//!     let env = E2ETestEnv::new(router, config).await.unwrap();
//!
//!     // Server is now running at env.url()
//!     // Make HTTP requests using reqwest or similar
//!
//!     // Automatic cleanup on drop
//! }
//! ```
//!
//! # WASM-Side Usage
//!
//! ```rust,ignore
//! use reinhardt_pages::testing::e2e::get_e2e_server_url;
//! use reqwest::Client;
//!
//! #[wasm_bindgen_test]
//! async fn test_wasm_e2e() {
//!     let server_url = get_e2e_server_url()
//!         .expect("E2E test server URL not set");
//!
//!     let response = Request::get(&format!("{}/api/health", server_url))
//!         .send()
//!         .await
//!         .unwrap();
//!
//!     assert_eq!(response.status(), 200);
//! }
//! ```

use std::time::Duration;

#[cfg(native)]
use std::net::SocketAddr;
#[cfg(native)]
use std::sync::Arc;
#[cfg(native)]
use tokio::net::TcpListener;
#[cfg(native)]
use tokio::task::JoinHandle;

/// Configuration for E2E test environment
#[derive(Debug, Clone)]
pub struct E2ETestConfig {
	/// Port for the test server (0 for random port)
	pub port: u16,
	/// Enable CORS for browser-based tests
	pub enable_cors: bool,
	/// Server startup wait time
	pub startup_delay: Duration,
	/// Shutdown timeout
	pub shutdown_timeout: Duration,
}

impl Default for E2ETestConfig {
	fn default() -> Self {
		Self {
			port: 0, // Random port
			enable_cors: true,
			startup_delay: Duration::from_millis(100),
			shutdown_timeout: Duration::from_secs(5),
		}
	}
}

impl E2ETestConfig {
	/// Create a new E2E test configuration
	pub fn new() -> Self {
		Self::default()
	}

	/// Set the server port (0 for random)
	pub fn with_port(mut self, port: u16) -> Self {
		self.port = port;
		self
	}

	/// Enable or disable CORS
	pub fn with_cors(mut self, enable: bool) -> Self {
		self.enable_cors = enable;
		self
	}

	/// Set server startup delay
	pub fn with_startup_delay(mut self, delay: Duration) -> Self {
		self.startup_delay = delay;
		self
	}

	/// Set shutdown timeout
	pub fn with_shutdown_timeout(mut self, timeout: Duration) -> Self {
		self.shutdown_timeout = timeout;
		self
	}
}

/// E2E test environment that manages server lifecycle
#[cfg(native)]
pub struct E2ETestEnv {
	/// Server URL (e.g., "http://127.0.0.1:12345")
	server_url: String,
	/// Server address
	addr: SocketAddr,
	/// Shutdown coordinator
	coordinator: Arc<reinhardt_server::ShutdownCoordinator>,
	/// Server task handle
	server_task: Option<JoinHandle<()>>,
}

#[cfg(native)]
impl E2ETestEnv {
	/// Create a new E2E test environment with the given router
	///
	/// # Arguments
	///
	/// * `router` - The application router
	/// * `config` - E2E test configuration
	///
	/// # Returns
	///
	/// An E2ETestEnv with a running server
	pub async fn new(
		router: reinhardt_urls::routers::ServerRouter,
		config: E2ETestConfig,
	) -> Result<Self, E2ETestError> {
		// Bind to specified port (0 = random)
		let addr: SocketAddr = format!("127.0.0.1:{}", config.port)
			.parse()
			.map_err(|e| E2ETestError::Initialization(format!("Invalid address: {}", e)))?;

		let listener = TcpListener::bind(addr)
			.await
			.map_err(|e| E2ETestError::Initialization(format!("Failed to bind: {}", e)))?;

		let actual_addr = listener
			.local_addr()
			.map_err(|e| E2ETestError::Initialization(format!("Failed to get address: {}", e)))?;

		let server_url = format!("http://{}", actual_addr);
		drop(listener);

		// Create shutdown coordinator
		let coordinator = Arc::new(reinhardt_server::ShutdownCoordinator::new(
			config.shutdown_timeout,
		));

		// Wrap router with CORS if enabled
		let router = if config.enable_cors {
			Self::add_cors_middleware(router)
		} else {
			router
		};

		// Spawn server
		let server_coordinator = (*coordinator).clone();
		let router = Arc::new(router);
		let server_task = tokio::spawn(async move {
			let server = reinhardt_server::HttpServer::new(router);
			let _ = server
				.listen_with_shutdown(actual_addr, server_coordinator)
				.await;
		});

		// Wait for server to start
		tokio::time::sleep(config.startup_delay).await;

		Ok(Self {
			server_url,
			addr: actual_addr,
			coordinator,
			server_task: Some(server_task),
		})
	}

	/// Get the server URL
	pub fn url(&self) -> &str {
		&self.server_url
	}

	/// Get the server address
	pub fn addr(&self) -> SocketAddr {
		self.addr
	}

	/// Add CORS middleware to the router for browser-based tests
	fn add_cors_middleware(
		router: reinhardt_urls::routers::ServerRouter,
	) -> reinhardt_urls::routers::ServerRouter {
		// CORS is typically handled by middleware, but for simplicity
		// we return the router as-is. Users should add CORS middleware
		// to their router if needed.
		//
		// In a real implementation, you would wrap the router with
		// a CORS middleware layer.
		router
	}

	/// Trigger graceful shutdown
	pub fn shutdown(&self) {
		self.coordinator.shutdown();
	}
}

#[cfg(native)]
impl Drop for E2ETestEnv {
	fn drop(&mut self) {
		// Trigger shutdown signal
		self.coordinator.shutdown();

		// Abort the server task
		if let Some(task) = self.server_task.take() {
			task.abort();
		}
	}
}

/// Error type for E2E test operations
#[derive(Debug, Clone)]
pub enum E2ETestError {
	/// Initialization error
	Initialization(String),
	/// Server error
	Server(String),
	/// Client error
	Client(String),
}

impl std::fmt::Display for E2ETestError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			E2ETestError::Initialization(msg) => write!(f, "E2E initialization error: {}", msg),
			E2ETestError::Server(msg) => write!(f, "E2E server error: {}", msg),
			E2ETestError::Client(msg) => write!(f, "E2E client error: {}", msg),
		}
	}
}

impl std::error::Error for E2ETestError {}

// ============================================================================
// WASM-Side Utilities
// ============================================================================

/// Environment variable name for E2E test server URL
pub const E2E_SERVER_URL_KEY: &str = "__E2E_TEST_SERVER_URL__";

/// Get the E2E test server URL from the browser window object.
///
/// This is used by WASM tests to discover the server URL set by
/// the test orchestrator.
///
/// # Returns
///
/// The server URL if set, or None if not running in E2E test mode.
#[cfg(wasm)]
pub fn get_e2e_server_url() -> Option<String> {
	let window = web_sys::window()?;
	let url = js_sys::Reflect::get(&window, &E2E_SERVER_URL_KEY.into()).ok()?;
	url.as_string()
}

/// Set the E2E test server URL in the browser window object.
///
/// This is used by the test orchestrator to inject the server URL
/// into the browser environment.
///
/// # Arguments
///
/// * `url` - The server URL to set
#[cfg(wasm)]
pub fn set_e2e_server_url(url: &str) {
	if let Some(window) = web_sys::window() {
		let _ = js_sys::Reflect::set(&window, &E2E_SERVER_URL_KEY.into(), &url.into());
	}
}

/// Check if running in E2E test mode.
///
/// Returns true if the E2E test server URL has been set.
#[cfg(wasm)]
pub fn is_e2e_test_mode() -> bool {
	get_e2e_server_url().is_some()
}

// Non-WASM stubs

/// Get the E2E test server URL from environment variable.
///
/// On native (non-WASM) targets, this reads from the environment variable
/// instead of the browser window object.
///
/// # Returns
///
/// The server URL if set, or None if not running in E2E test mode.
#[cfg(native)]
pub fn get_e2e_server_url() -> Option<String> {
	std::env::var(E2E_SERVER_URL_KEY).ok()
}

/// Set the E2E test server URL in environment variable.
///
/// On native (non-WASM) targets, this sets an environment variable
/// instead of the browser window object.
///
/// # Arguments
///
/// * `url` - The server URL to set
///
/// # Safety
///
/// This function modifies environment variables. It should only be called
/// during test setup, before parallel test execution begins.
#[cfg(native)]
pub fn set_e2e_server_url(url: &str) {
	// SAFETY: This function is only called during test setup, before parallel test execution.
	// The E2E test framework ensures single-threaded access during URL configuration.
	unsafe {
		std::env::set_var(E2E_SERVER_URL_KEY, url);
	}
}

/// Check if running in E2E test mode.
///
/// On native (non-WASM) targets, this checks if the E2E server URL
/// environment variable has been set.
///
/// # Returns
///
/// `true` if the E2E test server URL has been set, `false` otherwise.
#[cfg(native)]
pub fn is_e2e_test_mode() -> bool {
	get_e2e_server_url().is_some()
}

// ============================================================================
// Test Helpers
// ============================================================================

/// Helper to make an E2E HTTP request.
///
/// This is a convenience function for making HTTP requests in E2E tests.
/// It automatically uses the E2E server URL.
///
/// # Arguments
///
/// * `path` - The path to request (e.g., "/api/users")
/// * `method` - HTTP method
/// * `body` - Optional request body
///
/// # Returns
///
/// The response status and body, or an error.
#[cfg(wasm)]
pub async fn e2e_fetch(
	path: &str,
	method: &str,
	body: Option<&str>,
) -> Result<(u16, String), E2ETestError> {
	let client = reqwest::Client::new();

	let server_url = get_e2e_server_url()
		.ok_or_else(|| E2ETestError::Client("E2E server URL not set".into()))?;

	let url = format!("{}{}", server_url, path);

	let mut request = match method.to_uppercase().as_str() {
		"GET" => client.get(&url),
		"POST" => client.post(&url),
		"PUT" => client.put(&url),
		"DELETE" => client.delete(&url),
		"PATCH" => client.patch(&url),
		_ => {
			return Err(E2ETestError::Client(format!(
				"Unsupported method: {}",
				method
			)));
		}
	};

	request = request.header("Content-Type", "application/json");

	if let Some(body) = body {
		request = request.body(body.to_string());
	}

	let response = request
		.send()
		.await
		.map_err(|e| E2ETestError::Client(e.to_string()))?;

	let status = response.status().as_u16();
	let text = response
		.text()
		.await
		.map_err(|e| E2ETestError::Client(e.to_string()))?;

	Ok((status, text))
}

#[cfg(test)]
#[cfg(native)]
mod tests {
	use super::*;

	#[test]
	fn test_e2e_test_config_default() {
		let config = E2ETestConfig::default();
		assert_eq!(config.port, 0);
		assert!(config.enable_cors);
	}

	#[test]
	fn test_e2e_test_config_builder() {
		let config = E2ETestConfig::new()
			.with_port(8080)
			.with_cors(false)
			.with_startup_delay(Duration::from_millis(200));

		assert_eq!(config.port, 8080);
		assert!(!config.enable_cors);
		assert_eq!(config.startup_delay, Duration::from_millis(200));
	}

	#[test]
	fn test_e2e_server_url_env() {
		// SAFETY: This test manipulates environment variables in a single-threaded
		// context during testing. No other code runs concurrently that depends on
		// this environment variable.
		unsafe {
			std::env::remove_var(E2E_SERVER_URL_KEY);
		}
		assert!(get_e2e_server_url().is_none());
		assert!(!is_e2e_test_mode());

		set_e2e_server_url("http://localhost:8080");
		assert_eq!(
			get_e2e_server_url(),
			Some("http://localhost:8080".to_string())
		);
		assert!(is_e2e_test_mode());

		// SAFETY: Same as above - test cleanup in single-threaded test context.
		unsafe {
			std::env::remove_var(E2E_SERVER_URL_KEY);
		}
	}

	#[test]
	fn test_e2e_test_error_display() {
		let init_err = E2ETestError::Initialization("test".to_string());
		assert!(init_err.to_string().contains("initialization"));

		let server_err = E2ETestError::Server("test".to_string());
		assert!(server_err.to_string().contains("server"));

		let client_err = E2ETestError::Client("test".to_string());
		assert!(client_err.to_string().contains("client"));
	}
}
