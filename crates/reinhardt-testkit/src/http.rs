//! HTTP test utilities for Reinhardt framework
//!
//! Provides helper functions for creating and manipulating HTTP requests and responses in tests.

use bytes::Bytes;
use hyper::header::{HeaderName, HeaderValue};
use hyper::{HeaderMap, Method, StatusCode, Uri, Version};
use serde::de::DeserializeOwned;
use std::str::FromStr;

// Re-export types from reinhardt-apps for convenience
pub use reinhardt_http::{Error, Request, Response, Result};

/// Create a test HTTP request
///
/// This is a convenience function for creating HTTP requests in tests.
/// Supports both simple request creation and header-based request creation.
///
/// # Examples
///
/// ## Basic usage
///
/// ```
/// use reinhardt_testkit::http::create_request;
/// use hyper::Method;
///
/// let request = create_request(Method::GET, "/api/users", None, vec![]);
/// assert_eq!(request.method, Method::GET);
/// assert_eq!(request.uri.path(), "/api/users");
/// ```
///
/// ## With body
///
/// ```
/// use reinhardt_testkit::http::create_request;
/// use hyper::Method;
///
/// let body = r#"{"name": "Alice"}"#;
/// let request = create_request(Method::POST, "/api/users", Some(body.to_string()), vec![]);
/// assert_eq!(request.method, Method::POST);
/// assert_eq!(request.body().len(), body.len());
/// ```
///
/// ## With headers
///
/// ```
/// use reinhardt_testkit::http::create_request;
/// use hyper::Method;
///
/// let headers = vec![
///     ("Content-Type", "application/json"),
///     ("X-API-Key", "secret"),
/// ];
/// let request = create_request(Method::GET, "/api/users", None, headers);
/// assert_eq!(request.method, Method::GET);
/// assert!(request.headers.contains_key("content-type"));
/// assert!(request.headers.contains_key("x-api-key"));
/// ```
pub fn create_request(
	method: Method,
	path: &str,
	body: Option<String>,
	headers: Vec<(&str, &str)>,
) -> Request {
	let uri = path.parse::<Uri>().expect("Invalid URI");
	let body_bytes = body.map(Bytes::from).unwrap_or_default();

	let mut header_map = HeaderMap::new();
	for (key, value) in headers {
		let header_name: hyper::header::HeaderName = key.parse().expect("Invalid header name");
		let header_value: hyper::header::HeaderValue = value.parse().expect("Invalid header value");
		header_map.insert(header_name, header_value);
	}

	Request::builder()
		.method(method)
		.uri(uri)
		.version(Version::HTTP_11)
		.headers(header_map)
		.body(body_bytes)
		.build()
		.expect("Failed to build request")
}

/// Extract and deserialize JSON from a response
///
/// Returns the deserialized data or an error if deserialization fails.
///
/// # Examples
///
/// ```
/// use reinhardt_testkit::http::{extract_json, create_request};
/// use reinhardt_http::Response;
/// use serde::{Deserialize, Serialize};
///
/// #[derive(Serialize, Deserialize, PartialEq, Debug)]
/// struct User {
///     id: i64,
///     name: String,
/// }
///
/// let user = User { id: 1, name: "Alice".to_string() };
/// let json = serde_json::to_string(&user).unwrap();
/// let response = Response::ok()
///     .with_header("Content-Type", "application/json")
///     .with_body(json);
///
/// let extracted: User = extract_json(response).unwrap();
/// assert_eq!(extracted.id, 1);
/// assert_eq!(extracted.name, "Alice");
/// ```
///
/// # Invalid JSON
///
/// ```
/// use reinhardt_testkit::http::extract_json;
/// use reinhardt_http::Response;
/// use serde::Deserialize;
///
/// #[derive(Deserialize)]
/// struct User {
///     id: i64,
///     name: String,
/// }
///
/// let response = Response::ok()
///     .with_header("Content-Type", "application/json")
///     .with_body("invalid json");
///
/// let result: Result<User, _> = extract_json(response);
/// assert!(result.is_err());
/// ```
pub fn extract_json<T: DeserializeOwned>(response: Response) -> Result<T> {
	serde_json::from_slice(&response.body)
		.map_err(|e| Error::Serialization(format!("Failed to deserialize response: {}", e)))
}

// ============================================================================
// Request Creation Helpers
// ============================================================================

/// Create a mock HTTP request for testing with secure/insecure mode
///
/// This function provides more control over request creation, including
/// the ability to specify whether the request is secure (HTTPS).
///
/// # Arguments
///
/// * `method` - HTTP method as string (e.g., "GET", "POST")
/// * `uri` - Request URI as string
/// * `secure` - Whether this is an HTTPS request
///
/// # Examples
///
/// ```
/// use reinhardt_testkit::http::create_test_request;
///
/// let request = create_test_request("GET", "/api/users", false);
/// assert_eq!(request.method.as_str(), "GET");
/// assert!(!request.is_secure);
/// ```
///
/// ## Secure request
///
/// ```
/// use reinhardt_testkit::http::create_test_request;
///
/// let request = create_test_request("POST", "/api/login", true);
/// assert!(request.is_secure);
/// assert!(request.headers.contains_key("x-forwarded-proto"));
/// ```
pub fn create_test_request(method: &str, uri: &str, secure: bool) -> Request {
	let method = Method::from_str(method).unwrap_or(Method::GET);
	let uri = Uri::from_str(uri).unwrap_or_else(|_| Uri::from_static("/"));

	let mut headers = HeaderMap::new();

	// Add X-Forwarded-Proto header if secure
	if secure {
		headers.insert(
			HeaderName::from_static("x-forwarded-proto"),
			HeaderValue::from_static("https"),
		);
	}

	let mut request = Request::builder()
		.method(method)
		.uri(uri)
		.version(Version::HTTP_11)
		.headers(headers)
		.body(Bytes::new())
		.build()
		.expect("Failed to build request");
	request.is_secure = secure;
	request
}

/// Create a mock HTTPS request
///
/// Convenience wrapper around [`create_test_request`] for creating secure requests.
///
/// # Examples
///
/// ```
/// use reinhardt_testkit::http::create_secure_request;
///
/// let request = create_secure_request("GET", "/api/users");
/// assert!(request.is_secure);
/// assert_eq!(request.method.as_str(), "GET");
/// ```
pub fn create_secure_request(method: &str, uri: &str) -> Request {
	create_test_request(method, uri, true)
}

/// Create a mock HTTP request (non-secure)
///
/// Convenience wrapper around [`create_test_request`] for creating insecure requests.
///
/// # Examples
///
/// ```
/// use reinhardt_testkit::http::create_insecure_request;
///
/// let request = create_insecure_request("GET", "/api/users");
/// assert!(!request.is_secure);
/// assert_eq!(request.method.as_str(), "GET");
/// ```
pub fn create_insecure_request(method: &str, uri: &str) -> Request {
	create_test_request(method, uri, false)
}

// ============================================================================
// Response Creation Helpers
// ============================================================================

/// Create a mock response for testing
///
/// Returns a default 200 OK response.
///
/// # Examples
///
/// ```
/// use reinhardt_testkit::http::create_test_response;
/// use hyper::StatusCode;
///
/// let response = create_test_response();
/// assert_eq!(response.status, StatusCode::OK);
/// ```
pub fn create_test_response() -> Response {
	Response::ok()
}

/// Create a response with custom status code
///
/// # Examples
///
/// ```
/// use reinhardt_testkit::http::create_response_with_status;
/// use hyper::StatusCode;
///
/// let response = create_response_with_status(StatusCode::NOT_FOUND);
/// assert_eq!(response.status, StatusCode::NOT_FOUND);
/// ```
pub fn create_response_with_status(status: StatusCode) -> Response {
	Response::new(status)
}

/// Create a response with custom headers
///
/// # Examples
///
/// ```
/// use reinhardt_testkit::http::create_response_with_headers;
/// use hyper::{HeaderMap, header::{HeaderName, HeaderValue}};
///
/// let mut headers = HeaderMap::new();
/// headers.insert(
///     HeaderName::from_static("x-custom-header"),
///     HeaderValue::from_static("custom-value"),
/// );
/// let response = create_response_with_headers(headers);
/// assert!(response.headers.contains_key("x-custom-header"));
/// ```
pub fn create_response_with_headers(headers: HeaderMap) -> Response {
	let mut response = Response::ok();
	response.headers = headers;
	response
}

// ============================================================================
// Header Inspection Helpers
// ============================================================================

/// Check if response has a specific header
///
/// # Examples
///
/// ```
/// use reinhardt_testkit::http::{create_test_response, has_header};
///
/// let response = create_test_response().with_header("x-api-version", "v1");
/// assert!(has_header(&response, "x-api-version"));
/// assert!(!has_header(&response, "x-missing-header"));
/// ```
pub fn has_header(response: &Response, header_name: &str) -> bool {
	response.headers.contains_key(header_name)
}

/// Get header value from response
///
/// Returns `None` if the header is not present or cannot be converted to a string.
///
/// # Examples
///
/// ```
/// use reinhardt_testkit::http::{create_test_response, get_header};
///
/// let response = create_test_response().with_header("x-api-version", "v1");
/// assert_eq!(get_header(&response, "x-api-version"), Some("v1"));
/// assert_eq!(get_header(&response, "x-missing"), None);
/// ```
pub fn get_header<'a>(response: &'a Response, header_name: &str) -> Option<&'a str> {
	response
		.headers
		.get(header_name)
		.and_then(|v| v.to_str().ok())
}

/// Check if header has specific value
///
/// # Examples
///
/// ```
/// use reinhardt_testkit::http::{create_test_response, header_equals};
///
/// let response = create_test_response().with_header("content-type", "application/json");
/// assert!(header_equals(&response, "content-type", "application/json"));
/// assert!(!header_equals(&response, "content-type", "text/html"));
/// ```
pub fn header_equals(response: &Response, header_name: &str, expected_value: &str) -> bool {
	get_header(response, header_name)
		.map(|v| v == expected_value)
		.unwrap_or(false)
}

/// Check if header contains substring
///
/// # Examples
///
/// ```
/// use reinhardt_testkit::http::{create_test_response, header_contains};
///
/// let response = create_test_response().with_header("content-type", "application/json; charset=utf-8");
/// assert!(header_contains(&response, "content-type", "application/json"));
/// assert!(header_contains(&response, "content-type", "charset"));
/// assert!(!header_contains(&response, "content-type", "text/html"));
/// ```
pub fn header_contains(response: &Response, header_name: &str, substring: &str) -> bool {
	get_header(response, header_name)
		.map(|v| v.contains(substring))
		.unwrap_or(false)
}

// ============================================================================
// Response Assertions
// ============================================================================

/// Assert response status code
///
/// Panics if the status code doesn't match the expected value.
///
/// # Examples
///
/// ```
/// use reinhardt_testkit::http::{create_test_response, assert_status};
/// use hyper::StatusCode;
///
/// let response = create_test_response();
/// assert_status(&response, StatusCode::OK); // Passes
/// ```
///
/// ```should_panic
/// use reinhardt_testkit::http::{create_test_response, assert_status};
/// use hyper::StatusCode;
///
/// let response = create_test_response();
/// assert_status(&response, StatusCode::NOT_FOUND); // Panics
/// ```
pub fn assert_status(response: &Response, expected: StatusCode) {
	assert_eq!(
		response.status, expected,
		"Expected status {}, got {}",
		expected, response.status
	);
}

/// Assert response has header
///
/// Panics if the header is not present.
///
/// # Examples
///
/// ```
/// use reinhardt_testkit::http::{create_test_response, assert_has_header};
///
/// let response = create_test_response().with_header("x-api-version", "v1");
/// assert_has_header(&response, "x-api-version"); // Passes
/// ```
///
/// ```should_panic
/// use reinhardt_testkit::http::{create_test_response, assert_has_header};
///
/// let response = create_test_response();
/// assert_has_header(&response, "x-missing-header"); // Panics
/// ```
pub fn assert_has_header(response: &Response, header_name: &str) {
	assert!(
		has_header(response, header_name),
		"Expected response to have header '{}'",
		header_name
	);
}

/// Assert response doesn't have header
///
/// Panics if the header is present.
///
/// # Examples
///
/// ```
/// use reinhardt_testkit::http::{create_test_response, assert_no_header};
///
/// let response = create_test_response();
/// assert_no_header(&response, "x-missing-header"); // Passes
/// ```
///
/// ```should_panic
/// use reinhardt_testkit::http::{create_test_response, assert_no_header};
///
/// let response = create_test_response().with_header("x-api-version", "v1");
/// assert_no_header(&response, "x-api-version"); // Panics
/// ```
pub fn assert_no_header(response: &Response, header_name: &str) {
	assert!(
		!has_header(response, header_name),
		"Expected response to NOT have header '{}'",
		header_name
	);
}

/// Assert header value equals expected
///
/// Panics if the header is not present or has a different value.
///
/// # Examples
///
/// ```
/// use reinhardt_testkit::http::{create_test_response, assert_header_equals};
///
/// let response = create_test_response().with_header("content-type", "application/json");
/// assert_header_equals(&response, "content-type", "application/json"); // Passes
/// ```
///
/// ```should_panic
/// use reinhardt_testkit::http::{create_test_response, assert_header_equals};
///
/// let response = create_test_response().with_header("content-type", "application/json");
/// assert_header_equals(&response, "content-type", "text/html"); // Panics
/// ```
pub fn assert_header_equals(response: &Response, header_name: &str, expected_value: &str) {
	let actual = get_header(response, header_name)
		.unwrap_or_else(|| panic!("Header '{}' not found", header_name));
	assert_eq!(
		actual, expected_value,
		"Expected header '{}' to be '{}', got '{}'",
		header_name, expected_value, actual
	);
}

/// Assert header contains substring
///
/// Panics if the header is not present or doesn't contain the expected substring.
///
/// # Examples
///
/// ```
/// use reinhardt_testkit::http::{create_test_response, assert_header_contains};
///
/// let response = create_test_response().with_header("content-type", "application/json; charset=utf-8");
/// assert_header_contains(&response, "content-type", "application/json"); // Passes
/// ```
///
/// ```should_panic
/// use reinhardt_testkit::http::{create_test_response, assert_header_contains};
///
/// let response = create_test_response().with_header("content-type", "application/json");
/// assert_header_contains(&response, "content-type", "text/html"); // Panics
/// ```
pub fn assert_header_contains(response: &Response, header_name: &str, substring: &str) {
	let actual = get_header(response, header_name)
		.unwrap_or_else(|| panic!("Header '{}' not found", header_name));
	assert!(
		actual.contains(substring),
		"Expected header '{}' to contain '{}', got '{}'",
		header_name,
		substring,
		actual
	);
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	// ========================================================================
	// create_request
	// ========================================================================

	#[rstest]
	fn test_create_request_get_basic() {
		// Arrange / Act
		let request = create_request(Method::GET, "/api/users", None, vec![]);

		// Assert
		assert_eq!(request.method, Method::GET);
		assert_eq!(request.uri.path(), "/api/users");
		assert!(request.body().is_empty());
	}

	#[rstest]
	fn test_create_request_post_with_body() {
		// Arrange
		let body = r#"{"name": "Alice"}"#;

		// Act
		let request = create_request(Method::POST, "/api/users", Some(body.to_string()), vec![]);

		// Assert
		assert_eq!(request.method, Method::POST);
		assert_eq!(request.body().len(), body.len());
	}

	#[rstest]
	fn test_create_request_with_headers() {
		// Arrange
		let headers = vec![
			("Content-Type", "application/json"),
			("X-API-Key", "secret"),
		];

		// Act
		let request = create_request(Method::GET, "/api/users", None, headers);

		// Assert
		assert!(request.headers.contains_key("content-type"));
		assert!(request.headers.contains_key("x-api-key"));
	}

	// ========================================================================
	// create_test_request
	// ========================================================================

	#[rstest]
	fn test_create_test_request_secure() {
		// Arrange / Act
		let request = create_test_request("POST", "/api/login", true);

		// Assert
		assert!(request.is_secure);
		assert!(request.headers.contains_key("x-forwarded-proto"));
		assert_eq!(request.method, Method::POST);
	}

	#[rstest]
	fn test_create_test_request_insecure() {
		// Arrange / Act
		let request = create_test_request("GET", "/api/users", false);

		// Assert
		assert!(!request.is_secure);
		assert!(!request.headers.contains_key("x-forwarded-proto"));
	}

	// ========================================================================
	// create_secure_request / create_insecure_request
	// ========================================================================

	#[rstest]
	fn test_create_secure_request() {
		// Arrange / Act
		let request = create_secure_request("GET", "/api/users");

		// Assert
		assert!(request.is_secure);
		assert_eq!(request.method, Method::GET);
	}

	#[rstest]
	fn test_create_insecure_request() {
		// Arrange / Act
		let request = create_insecure_request("GET", "/api/users");

		// Assert
		assert!(!request.is_secure);
		assert_eq!(request.method, Method::GET);
	}

	// ========================================================================
	// Response creation helpers
	// ========================================================================

	#[rstest]
	fn test_create_test_response() {
		// Arrange / Act
		let response = create_test_response();

		// Assert
		assert_eq!(response.status, StatusCode::OK);
	}

	#[rstest]
	fn test_create_response_with_status() {
		// Arrange / Act
		let response = create_response_with_status(StatusCode::NOT_FOUND);

		// Assert
		assert_eq!(response.status, StatusCode::NOT_FOUND);
	}

	#[rstest]
	fn test_create_response_with_headers() {
		// Arrange
		let mut headers = HeaderMap::new();
		headers.insert(
			HeaderName::from_static("x-custom-header"),
			HeaderValue::from_static("custom-value"),
		);

		// Act
		let response = create_response_with_headers(headers);

		// Assert
		assert!(response.headers.contains_key("x-custom-header"));
	}

	// ========================================================================
	// extract_json
	// ========================================================================

	#[rstest]
	fn test_extract_json_valid() {
		// Arrange
		#[derive(serde::Deserialize, PartialEq, Debug)]
		struct User {
			id: i64,
			name: String,
		}
		let response = Response::ok()
			.with_header("Content-Type", "application/json")
			.with_body(r#"{"id": 1, "name": "Alice"}"#);

		// Act
		let user: User = extract_json(response).unwrap();

		// Assert
		assert_eq!(user.id, 1);
		assert_eq!(user.name, "Alice");
	}

	#[rstest]
	fn test_extract_json_invalid() {
		// Arrange
		#[derive(serde::Deserialize)]
		struct User {
			#[allow(dead_code)] // Field used for deserialization target verification
			id: i64,
		}
		let response = Response::ok().with_body("not json");

		// Act
		let result: Result<User> = extract_json(response);

		// Assert
		assert!(result.is_err());
	}

	// ========================================================================
	// Header inspection helpers
	// ========================================================================

	#[rstest]
	fn test_has_header_present() {
		// Arrange
		let response = create_test_response().with_header("x-api-version", "v1");

		// Act / Assert
		assert!(has_header(&response, "x-api-version"));
	}

	#[rstest]
	fn test_has_header_absent() {
		// Arrange
		let response = create_test_response();

		// Act / Assert
		assert!(!has_header(&response, "x-missing"));
	}

	#[rstest]
	fn test_get_header_present() {
		// Arrange
		let response = create_test_response().with_header("x-api-version", "v1");

		// Act
		let value = get_header(&response, "x-api-version");

		// Assert
		assert_eq!(value, Some("v1"));
	}

	#[rstest]
	fn test_get_header_absent() {
		// Arrange
		let response = create_test_response();

		// Act
		let value = get_header(&response, "x-missing");

		// Assert
		assert_eq!(value, None);
	}

	#[rstest]
	fn test_header_equals_match() {
		// Arrange
		let response = create_test_response().with_header("content-type", "application/json");

		// Act / Assert
		assert!(header_equals(&response, "content-type", "application/json"));
	}

	#[rstest]
	fn test_header_equals_mismatch() {
		// Arrange
		let response = create_test_response().with_header("content-type", "application/json");

		// Act / Assert
		assert!(!header_equals(&response, "content-type", "text/html"));
	}

	#[rstest]
	fn test_header_contains_substring() {
		// Arrange
		let response =
			create_test_response().with_header("content-type", "application/json; charset=utf-8");

		// Act / Assert
		assert!(header_contains(
			&response,
			"content-type",
			"application/json"
		));
		assert!(header_contains(&response, "content-type", "charset"));
	}

	#[rstest]
	fn test_header_contains_no_match() {
		// Arrange
		let response = create_test_response().with_header("content-type", "application/json");

		// Act / Assert
		assert!(!header_contains(&response, "content-type", "text/html"));
	}

	// ========================================================================
	// Response assertions
	// ========================================================================

	#[rstest]
	fn test_assert_status_pass() {
		// Arrange
		let response = create_test_response();

		// Act / Assert (should not panic)
		assert_status(&response, StatusCode::OK);
	}

	#[rstest]
	#[should_panic(expected = "Expected status")]
	fn test_assert_status_fail() {
		// Arrange
		let response = create_test_response();

		// Act (should panic)
		assert_status(&response, StatusCode::NOT_FOUND);
	}

	#[rstest]
	fn test_assert_has_header_pass() {
		// Arrange
		let response = create_test_response().with_header("x-api-version", "v1");

		// Act / Assert (should not panic)
		assert_has_header(&response, "x-api-version");
	}

	#[rstest]
	#[should_panic(expected = "Expected response to have header")]
	fn test_assert_has_header_fail() {
		// Arrange
		let response = create_test_response();

		// Act (should panic)
		assert_has_header(&response, "x-missing");
	}

	#[rstest]
	fn test_assert_no_header_pass() {
		// Arrange
		let response = create_test_response();

		// Act / Assert (should not panic)
		assert_no_header(&response, "x-missing");
	}

	#[rstest]
	#[should_panic(expected = "Expected response to NOT have header")]
	fn test_assert_no_header_fail() {
		// Arrange
		let response = create_test_response().with_header("x-api-version", "v1");

		// Act (should panic)
		assert_no_header(&response, "x-api-version");
	}
}
