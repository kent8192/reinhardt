//! HTTP Mock Infrastructure for WASM Tests
//!
//! This module provides utilities for mocking HTTP requests in WASM tests,
//! enabling Layer 2 testing (component tests with mocked server functions).
//!
//! # Overview
//!
//! When testing WASM components that call server functions, you often want to
//! mock the HTTP responses instead of making actual network requests. This module
//! provides a registry-based approach to intercept and mock these calls.
//!
//! # Example
//!
//! ```rust,ignore
//! use reinhardt_pages::testing::{mock_server_fn, clear_mocks, assert_server_fn_called};
//!
//! #[wasm_bindgen_test]
//! async fn test_login_component() {
//!     // Setup mock response
//!     let user = UserInfo { username: "test".to_string(), ... };
//!     mock_server_fn("/api/server_fn/login", &user);
//!
//!     // Render and interact with component
//!     // ...
//!
//!     // Verify the server function was called
//!     assert_server_fn_called("/api/server_fn/login");
//!
//!     // Cleanup
//!     clear_mocks();
//! }
//! ```

use std::cell::RefCell;
use std::collections::HashMap;

/// Mock response configuration for HTTP requests.
///
/// This struct defines what response should be returned when a
/// mocked endpoint is called.
#[derive(Clone, Debug)]
pub struct MockResponse {
	/// HTTP status code (e.g., 200, 401, 500)
	pub status: u16,
	/// Response body as a string
	pub body: String,
	/// Optional response headers
	pub headers: HashMap<String, String>,
}

impl MockResponse {
	/// Create a successful (200 OK) mock response with JSON body.
	///
	/// # Arguments
	///
	/// * `data` - Any serializable data to be returned as JSON
	///
	/// # Example
	///
	/// ```rust,ignore
	/// let response = MockResponse::ok(&UserInfo { username: "test".to_string() });
	/// ```
	pub fn ok<T: serde::Serialize>(data: &T) -> Self {
		Self {
			status: 200,
			body: serde_json::to_string(data).unwrap_or_default(),
			headers: HashMap::new(),
		}
	}

	/// Create an error mock response.
	///
	/// # Arguments
	///
	/// * `status` - HTTP status code (e.g., 401, 403, 500)
	/// * `message` - Error message
	///
	/// # Example
	///
	/// ```rust,ignore
	/// let response = MockResponse::error(401, "Invalid credentials");
	/// ```
	pub fn error(status: u16, message: impl Into<String>) -> Self {
		Self {
			status,
			body: message.into(),
			headers: HashMap::new(),
		}
	}

	/// Create a mock response with custom status and JSON body.
	///
	/// # Arguments
	///
	/// * `status` - HTTP status code
	/// * `data` - Data to serialize as JSON body
	pub fn with_status<T: serde::Serialize>(status: u16, data: &T) -> Self {
		Self {
			status,
			body: serde_json::to_string(data).unwrap_or_default(),
			headers: HashMap::new(),
		}
	}

	/// Add a header to the mock response.
	pub fn with_header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
		self.headers.insert(name.into(), value.into());
		self
	}
}

/// Record of a mock HTTP call for assertion purposes.
#[derive(Clone, Debug)]
pub struct MockCall {
	/// The path that was called
	pub path: String,
	/// The request body
	pub body: String,
	/// HTTP method used
	pub method: String,
	/// Timestamp of the call (in milliseconds since epoch)
	pub timestamp: f64,
}

/// Internal mock registry storage
struct MockRegistry {
	/// Registered mock responses keyed by path
	responses: HashMap<String, MockResponse>,
	/// Call history for assertions
	call_history: Vec<MockCall>,
}

impl MockRegistry {
	fn new() -> Self {
		Self {
			responses: HashMap::new(),
			call_history: Vec::new(),
		}
	}
}

thread_local! {
	/// Thread-local mock registry
	static MOCK_REGISTRY: RefCell<MockRegistry> = RefCell::new(MockRegistry::new());
}

/// Register a mock response for a server function endpoint.
///
/// When the specified endpoint is called, the provided data will be
/// serialized to JSON and returned as a successful response.
///
/// # Arguments
///
/// * `path` - The server function endpoint path (e.g., "/api/server_fn/login")
/// * `response` - Any serializable data to return
///
/// # Example
///
/// ```rust,ignore
/// let user = UserInfo { username: "test".to_string(), ... };
/// mock_server_fn("/api/server_fn/login", &user);
/// ```
#[deprecated(
	since = "0.1.0-rc.16",
	note = "Use `MockServiceWorker::handle_server_fn` from `reinhardt_test::msw` instead. See issue #3283."
)]
pub fn mock_server_fn<T: serde::Serialize>(path: &str, response: &T) {
	MOCK_REGISTRY.with(|r| {
		r.borrow_mut()
			.responses
			.insert(path.to_string(), MockResponse::ok(response));
	});
}

/// Register an error mock response for a server function endpoint.
///
/// # Arguments
///
/// * `path` - The server function endpoint path
/// * `status` - HTTP status code for the error
/// * `message` - Error message
///
/// # Example
///
/// ```rust,ignore
/// mock_server_fn_error("/api/server_fn/login", 401, "Invalid credentials");
/// ```
#[deprecated(
	since = "0.1.0-rc.16",
	note = "Use `MockServiceWorker` with `rest::post(...).respond(...)` from `reinhardt_test::msw` instead. See issue #3283."
)]
pub fn mock_server_fn_error(path: &str, status: u16, message: &str) {
	MOCK_REGISTRY.with(|r| {
		r.borrow_mut()
			.responses
			.insert(path.to_string(), MockResponse::error(status, message));
	});
}

/// Register a custom mock response for more control.
///
/// # Arguments
///
/// * `path` - The server function endpoint path
/// * `response` - The custom MockResponse
#[deprecated(
	since = "0.1.0-rc.16",
	note = "Use `MockServiceWorker::handle` from `reinhardt_test::msw` instead. See issue #3283."
)]
pub fn mock_server_fn_custom(path: &str, response: MockResponse) {
	MOCK_REGISTRY.with(|r| {
		r.borrow_mut().responses.insert(path.to_string(), response);
	});
}

/// Clear all mock responses and call history.
///
/// Should be called at the end of each test to ensure a clean state.
///
/// # Example
///
/// ```rust,ignore
/// #[wasm_bindgen_test]
/// async fn test_example() {
///     mock_server_fn("/api/endpoint", &data);
///     // ... test code ...
///     clear_mocks(); // Cleanup
/// }
/// ```
pub fn clear_mocks() {
	MOCK_REGISTRY.with(|r| {
		let mut registry = r.borrow_mut();
		registry.responses.clear();
		registry.call_history.clear();
	});
}

/// Get the call history for a specific endpoint.
///
/// # Arguments
///
/// * `path` - The endpoint path to filter by (or None for all calls)
///
/// # Returns
///
/// A vector of MockCall records
pub fn get_call_history_for(path: &str) -> Vec<MockCall> {
	MOCK_REGISTRY.with(|r| {
		r.borrow()
			.call_history
			.iter()
			.filter(|c| c.path == path)
			.cloned()
			.collect()
	})
}

/// Get all mock call history.
///
/// # Returns
///
/// A vector of all MockCall records
pub fn get_call_history() -> Vec<MockCall> {
	MOCK_REGISTRY.with(|r| r.borrow().call_history.clone())
}

/// Assert that a server function was called at least once.
///
/// # Panics
///
/// Panics if the server function was not called.
///
/// # Example
///
/// ```rust,ignore
/// mock_server_fn("/api/server_fn/login", &response);
/// // ... trigger the call ...
/// assert_server_fn_called("/api/server_fn/login");
/// ```
pub fn assert_server_fn_called(path: &str) {
	let history = get_call_history();
	assert!(
		history.iter().any(|c| c.path == path),
		"Expected server function '{}' to be called, but it wasn't.\nActual calls: {:?}",
		path,
		history.iter().map(|c| &c.path).collect::<Vec<_>>()
	);
}

/// Assert that a server function was called with specific data.
///
/// # Panics
///
/// Panics if the server function was not called with the expected data.
///
/// # Example
///
/// ```rust,ignore
/// let request = LoginRequest { email: "test@example.com".to_string(), password: "pass".to_string() };
/// // ... trigger the call ...
/// assert_server_fn_called_with("/api/server_fn/login", &request);
/// ```
pub fn assert_server_fn_called_with<T: serde::Serialize>(path: &str, expected: &T) {
	let expected_body = serde_json::to_string(expected).unwrap_or_default();
	let history = get_call_history();
	assert!(
		history
			.iter()
			.any(|c| c.path == path && c.body == expected_body),
		"Expected server function '{}' to be called with body '{}'.\nActual calls: {:?}",
		path,
		expected_body,
		history
	);
}

/// Assert that a server function was called exactly N times.
///
/// # Panics
///
/// Panics if the call count doesn't match.
pub fn assert_server_fn_call_count(path: &str, expected_count: usize) {
	let count = get_call_history_for(path).len();
	assert_eq!(
		count, expected_count,
		"Expected server function '{}' to be called {} times, but was called {} times",
		path, expected_count, count
	);
}

/// Assert that a server function was NOT called.
///
/// # Panics
///
/// Panics if the server function was called.
pub fn assert_server_fn_not_called(path: &str) {
	let history = get_call_history_for(path);
	assert!(
		history.is_empty(),
		"Expected server function '{}' to NOT be called, but it was called {} times",
		path,
		history.len()
	);
}

/// Record a mock call (internal use by mock fetch implementation).
///
/// This is called by the mock fetch implementation to record
/// that a request was made.
pub(crate) fn record_mock_call(path: &str, body: &str, method: &str, timestamp: f64) {
	MOCK_REGISTRY.with(|r| {
		r.borrow_mut().call_history.push(MockCall {
			path: path.to_string(),
			body: body.to_string(),
			method: method.to_string(),
			timestamp,
		});
	});
}

/// Get mock response for a path (internal use by mock fetch implementation).
///
/// # Returns
///
/// The registered mock response, or None if no mock is registered.
pub(crate) fn get_mock_response(path: &str) -> Option<MockResponse> {
	MOCK_REGISTRY.with(|r| r.borrow().responses.get(path).cloned())
}

#[cfg(test)]
#[allow(deprecated)]
mod tests {
	use super::*;
	use serde::{Deserialize, Serialize};

	#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
	struct TestData {
		name: String,
		value: i32,
	}

	#[test]
	fn test_mock_response_ok() {
		let data = TestData {
			name: "test".to_string(),
			value: 42,
		};
		let response = MockResponse::ok(&data);

		assert_eq!(response.status, 200);
		assert!(response.body.contains("test"));
		assert!(response.body.contains("42"));
	}

	#[test]
	fn test_mock_response_error() {
		let response = MockResponse::error(401, "Unauthorized");

		assert_eq!(response.status, 401);
		assert_eq!(response.body, "Unauthorized");
	}

	#[test]
	fn test_mock_server_fn_registration() {
		clear_mocks();

		let data = TestData {
			name: "test".to_string(),
			value: 42,
		};
		mock_server_fn("/api/test", &data);

		let response = get_mock_response("/api/test");
		assert!(response.is_some());
		assert_eq!(response.unwrap().status, 200);

		clear_mocks();
	}

	#[test]
	fn test_mock_server_fn_error_registration() {
		clear_mocks();

		mock_server_fn_error("/api/test", 500, "Internal Error");

		let response = get_mock_response("/api/test");
		assert!(response.is_some());
		let r = response.unwrap();
		assert_eq!(r.status, 500);
		assert_eq!(r.body, "Internal Error");

		clear_mocks();
	}

	#[test]
	fn test_call_history_recording() {
		clear_mocks();

		record_mock_call("/api/test1", r#"{"key":"value"}"#, "POST", 1000.0);
		record_mock_call("/api/test2", "", "GET", 2000.0);
		record_mock_call("/api/test1", r#"{"key":"other"}"#, "POST", 3000.0);

		let history = get_call_history();
		assert_eq!(history.len(), 3);

		let test1_history = get_call_history_for("/api/test1");
		assert_eq!(test1_history.len(), 2);

		let test2_history = get_call_history_for("/api/test2");
		assert_eq!(test2_history.len(), 1);

		clear_mocks();
	}

	#[test]
	fn test_clear_mocks() {
		mock_server_fn("/api/test", &"data");
		record_mock_call("/api/test", "", "GET", 1000.0);

		assert!(get_mock_response("/api/test").is_some());
		assert_eq!(get_call_history().len(), 1);

		clear_mocks();

		assert!(get_mock_response("/api/test").is_none());
		assert!(get_call_history().is_empty());
	}

	#[test]
	fn test_assert_server_fn_called() {
		clear_mocks();
		record_mock_call("/api/login", "", "POST", 1000.0);

		assert_server_fn_called("/api/login");

		clear_mocks();
	}

	#[test]
	#[should_panic(expected = "Expected server function '/api/missing' to be called")]
	fn test_assert_server_fn_called_panics_when_not_called() {
		clear_mocks();
		assert_server_fn_called("/api/missing");
	}

	#[test]
	fn test_assert_server_fn_not_called() {
		clear_mocks();
		assert_server_fn_not_called("/api/test");
		clear_mocks();
	}

	#[test]
	fn test_assert_server_fn_call_count() {
		clear_mocks();
		record_mock_call("/api/test", "", "GET", 1000.0);
		record_mock_call("/api/test", "", "GET", 2000.0);
		record_mock_call("/api/test", "", "GET", 3000.0);

		assert_server_fn_call_count("/api/test", 3);

		clear_mocks();
	}
}
