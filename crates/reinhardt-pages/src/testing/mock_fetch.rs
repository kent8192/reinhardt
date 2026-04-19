//! Mock Fetch Implementation for WASM Tests
//!
//! This module provides a mock-aware fetch implementation that can intercept
//! HTTP requests during WASM tests and return registered mock responses.
//!
//! # Architecture
//!
//! When the `test-mock` feature is enabled (or in test mode), this module
//! provides an alternative fetch function that:
//!
//! 1. Checks if a mock response is registered for the request path
//! 2. If found, records the call and returns the mock response
//! 3. If not found, falls back to the real HTTP request
//!
//! # Usage
//!
//! This is typically used internally by the server_fn macro generated code,
//! but can also be used directly for custom testing scenarios.

use super::mock_http::{MockResponse, get_mock_response, record_mock_call};
use crate::server_fn::ServerFnError;

/// Perform a fetch operation that can be mocked.
///
/// This function checks for registered mocks before making actual HTTP requests.
/// In test mode, it will use mock responses if available.
///
/// # Arguments
///
/// * `url` - The URL to fetch
/// * `method` - HTTP method (GET, POST, PUT, DELETE, etc.)
/// * `body` - Optional request body
/// * `headers` - Request headers as key-value pairs
///
/// # Returns
///
/// A tuple of (status_code, response_body) on success, or a ServerFnError on failure.
#[cfg(wasm)]
pub async fn fetch_with_mock(
	url: &str,
	method: &str,
	body: Option<&str>,
	headers: &[(&str, &str)],
) -> Result<(u16, String), ServerFnError> {
	let body_str = body.unwrap_or("");

	// Get current timestamp for call recording
	let timestamp = js_sys::Date::now();

	// Record the call for assertion purposes
	record_mock_call(url, body_str, method, timestamp);

	// Check if we have a mock for this endpoint
	if let Some(response) = get_mock_response(url) {
		return mock_response_to_result(response);
	}

	// No mock registered - make real HTTP request
	real_fetch(url, method, body, headers).await
}

/// Convert a MockResponse to the expected Result format
fn mock_response_to_result(response: MockResponse) -> Result<(u16, String), ServerFnError> {
	if response.status >= 200 && response.status < 300 {
		Ok((response.status, response.body))
	} else {
		Err(ServerFnError::server(response.status, response.body))
	}
}

/// Perform a real HTTP fetch using reqwest
#[cfg(wasm)]
async fn real_fetch(
	url: &str,
	method: &str,
	body: Option<&str>,
	headers: &[(&str, &str)],
) -> Result<(u16, String), ServerFnError> {
	let client = reqwest::Client::new();

	let mut request = match method.to_uppercase().as_str() {
		"GET" => client.get(url),
		"POST" => client.post(url),
		"PUT" => client.put(url),
		"DELETE" => client.delete(url),
		"PATCH" => client.patch(url),
		_ => {
			return Err(ServerFnError::application(format!(
				"Unsupported HTTP method: {}",
				method
			)));
		}
	};

	// Add headers
	for (name, value) in headers {
		request = request.header(*name, *value);
	}

	// Add body if present
	if let Some(body) = body {
		request = request.body(body.to_string());
	}

	// Send request
	let response = request
		.send()
		.await
		.map_err(|e| ServerFnError::network(e.to_string()))?;

	let status = response.status().as_u16();
	let text = response
		.text()
		.await
		.map_err(|e| ServerFnError::deserialization(e.to_string()))?;

	if status >= 200 && status < 300 {
		Ok((status, text))
	} else {
		Err(ServerFnError::server(status, text))
	}
}

/// Non-WASM implementation for compilation purposes
#[cfg(native)]
pub async fn fetch_with_mock(
	url: &str,
	method: &str,
	body: Option<&str>,
	_headers: &[(&str, &str)],
) -> Result<(u16, String), ServerFnError> {
	let body_str = body.unwrap_or("");

	// Record the call for assertion purposes
	record_mock_call(url, body_str, method, 0.0);

	// Check if we have a mock for this endpoint
	if let Some(response) = get_mock_response(url) {
		return mock_response_to_result(response);
	}

	// In non-WASM context, if no mock is registered, fail
	Err(ServerFnError::network(format!(
		"No mock registered for {} {} and real fetch is not available on non-WASM targets",
		method, url
	)))
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
	use super::*;
	#[allow(deprecated)]
	use crate::testing::mock_http::{
		clear_mocks, get_call_history, mock_server_fn, mock_server_fn_error,
	};

	#[derive(serde::Serialize, serde::Deserialize)]
	struct TestResponse {
		message: String,
	}

	#[tokio::test]
	async fn test_fetch_with_mock_returns_mocked_response() {
		clear_mocks();

		let response = TestResponse {
			message: "Hello".to_string(),
		};
		mock_server_fn("/api/test", &response);

		let result = fetch_with_mock("/api/test", "GET", None, &[]).await;
		assert!(result.is_ok());

		let (status, body) = result.unwrap();
		assert_eq!(status, 200);
		assert!(body.contains("Hello"));

		clear_mocks();
	}

	#[tokio::test]
	async fn test_fetch_with_mock_records_call() {
		clear_mocks();

		mock_server_fn("/api/test", &"ok");

		let _ = fetch_with_mock("/api/test", "POST", Some(r#"{"key":"value"}"#), &[]).await;

		let history = get_call_history();
		assert_eq!(history.len(), 1);
		assert_eq!(history[0].path, "/api/test");
		assert_eq!(history[0].method, "POST");
		assert_eq!(history[0].body, r#"{"key":"value"}"#);

		clear_mocks();
	}

	#[tokio::test]
	async fn test_fetch_with_mock_error_response() {
		clear_mocks();

		mock_server_fn_error("/api/error", 401, "Unauthorized");

		let result = fetch_with_mock("/api/error", "GET", None, &[]).await;
		assert!(result.is_err());

		clear_mocks();
	}

	#[tokio::test]
	async fn test_fetch_without_mock_fails_on_server() {
		clear_mocks();

		let result = fetch_with_mock("/api/unmocked", "GET", None, &[]).await;
		assert!(result.is_err());

		// Call should still be recorded
		let history = get_call_history();
		assert_eq!(history.len(), 1);
		assert_eq!(history[0].path, "/api/unmocked");

		clear_mocks();
	}
}
