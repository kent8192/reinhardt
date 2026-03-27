//! Test response wrapper with assertion helpers

use bytes::Bytes;
use http::{HeaderMap, Response, StatusCode, Version};
use http_body_util::{BodyExt, Full};
use serde::de::DeserializeOwned;
use serde_json::Value;

/// Test response wrapper
pub struct TestResponse {
	status: StatusCode,
	headers: HeaderMap,
	body: Bytes,
	version: Version,
}

impl TestResponse {
	/// Create a new test response (async version for collecting body)
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_testkit::response::TestResponse;
	/// use http::{Response, StatusCode};
	/// use http_body_util::Full;
	/// use bytes::Bytes;
	///
	/// # tokio_test::block_on(async {
	/// let response = Response::builder()
	///     .status(StatusCode::OK)
	///     .body(Full::new(Bytes::from("Hello World")))
	///     .unwrap();
	/// let test_response = TestResponse::new(response).await;
	/// assert_eq!(test_response.status(), StatusCode::OK);
	/// # });
	/// ```
	pub async fn new(response: Response<Full<Bytes>>) -> Self {
		let (parts, body) = response.into_parts();

		// Collect the body bytes
		let body_bytes = body
			.collect()
			.await
			.map(|collected| collected.to_bytes())
			.unwrap_or_else(|_| Bytes::new());

		Self {
			status: parts.status,
			headers: parts.headers,
			body: body_bytes,
			version: parts.version,
		}
	}

	/// Create a test response with status, headers, and body (defaults to HTTP/1.1)
	pub fn with_body(status: StatusCode, headers: HeaderMap, body: Bytes) -> Self {
		Self {
			status,
			headers,
			body,
			version: Version::HTTP_11,
		}
	}

	/// Create a test response with status, headers, body, and HTTP version
	pub fn with_body_and_version(
		status: StatusCode,
		headers: HeaderMap,
		body: Bytes,
		version: Version,
	) -> Self {
		Self {
			status,
			headers,
			body,
			version,
		}
	}
	/// Get response status
	pub fn status(&self) -> StatusCode {
		self.status
	}

	/// Get response status code as u16
	pub fn status_code(&self) -> u16 {
		self.status.as_u16()
	}

	/// Get HTTP version of the response
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_testkit::response::TestResponse;
	/// use http::{StatusCode, HeaderMap, Version};
	/// use bytes::Bytes;
	///
	/// let response = TestResponse::with_body_and_version(
	///     StatusCode::OK,
	///     HeaderMap::new(),
	///     Bytes::new(),
	///     Version::HTTP_2,
	/// );
	/// assert_eq!(response.version(), Version::HTTP_2);
	/// ```
	pub fn version(&self) -> Version {
		self.version
	}
	/// Get response headers
	pub fn headers(&self) -> &HeaderMap {
		&self.headers
	}
	/// Get response body as bytes
	pub fn body(&self) -> &Bytes {
		&self.body
	}
	/// Get response body as string
	pub fn text(&self) -> String {
		String::from_utf8_lossy(&self.body).to_string()
	}
	/// Parse response body as JSON
	pub fn json<T: DeserializeOwned>(&self) -> Result<T, serde_json::Error> {
		serde_json::from_slice(&self.body)
	}
	/// Parse response body as generic JSON value
	pub fn json_value(&self) -> Result<Value, serde_json::Error> {
		serde_json::from_slice(&self.body)
	}
	/// Check if response is successful (2xx)
	pub fn is_success(&self) -> bool {
		self.status.is_success()
	}
	/// Check if response is client error (4xx)
	pub fn is_client_error(&self) -> bool {
		self.status.is_client_error()
	}
	/// Check if response is server error (5xx)
	pub fn is_server_error(&self) -> bool {
		self.status.is_server_error()
	}
	/// Get content type header
	pub fn content_type(&self) -> Option<&str> {
		self.headers
			.get("content-type")
			.and_then(|v| v.to_str().ok())
	}
	/// Get header value
	pub fn header(&self, name: &str) -> Option<&str> {
		self.headers.get(name).and_then(|v| v.to_str().ok())
	}
}

/// Extension trait for Response assertions
pub trait ResponseExt {
	/// Assert status code
	fn assert_status(&self, expected: StatusCode) -> &Self;

	/// Assert 2xx success
	fn assert_success(&self) -> &Self;

	/// Assert 4xx client error
	fn assert_client_error(&self) -> &Self;

	/// Assert 5xx server error
	fn assert_server_error(&self) -> &Self;

	/// Assert specific status codes
	fn assert_ok(&self) -> &Self;
	/// Assert that the response status is 201 Created.
	fn assert_created(&self) -> &Self;
	/// Assert that the response status is 204 No Content.
	fn assert_no_content(&self) -> &Self;
	/// Assert that the response status is 400 Bad Request.
	fn assert_bad_request(&self) -> &Self;
	/// Assert that the response status is 401 Unauthorized.
	fn assert_unauthorized(&self) -> &Self;
	/// Assert that the response status is 403 Forbidden.
	fn assert_forbidden(&self) -> &Self;
	/// Assert that the response status is 404 Not Found.
	fn assert_not_found(&self) -> &Self;
}

impl ResponseExt for TestResponse {
	fn assert_status(&self, expected: StatusCode) -> &Self {
		assert_eq!(
			self.status,
			expected,
			"Expected status {}, got {}. Body: {}",
			expected,
			self.status,
			self.text()
		);
		self
	}

	fn assert_success(&self) -> &Self {
		assert!(
			self.is_success(),
			"Expected success status (2xx), got {}. Body: {}",
			self.status,
			self.text()
		);
		self
	}

	fn assert_client_error(&self) -> &Self {
		assert!(
			self.is_client_error(),
			"Expected client error status (4xx), got {}. Body: {}",
			self.status,
			self.text()
		);
		self
	}

	fn assert_server_error(&self) -> &Self {
		assert!(
			self.is_server_error(),
			"Expected server error status (5xx), got {}. Body: {}",
			self.status,
			self.text()
		);
		self
	}

	fn assert_ok(&self) -> &Self {
		self.assert_status(StatusCode::OK)
	}

	fn assert_created(&self) -> &Self {
		self.assert_status(StatusCode::CREATED)
	}

	fn assert_no_content(&self) -> &Self {
		self.assert_status(StatusCode::NO_CONTENT)
	}

	fn assert_bad_request(&self) -> &Self {
		self.assert_status(StatusCode::BAD_REQUEST)
	}

	fn assert_unauthorized(&self) -> &Self {
		self.assert_status(StatusCode::UNAUTHORIZED)
	}

	fn assert_forbidden(&self) -> &Self {
		self.assert_status(StatusCode::FORBIDDEN)
	}

	fn assert_not_found(&self) -> &Self {
		self.assert_status(StatusCode::NOT_FOUND)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	// ========================================================================
	// Helper: build a TestResponse from parts
	// ========================================================================

	fn make_response(status: u16, body: &[u8]) -> TestResponse {
		TestResponse::with_body(
			StatusCode::from_u16(status).unwrap(),
			HeaderMap::new(),
			Bytes::from(body.to_vec()),
		)
	}

	// ========================================================================
	// Construction
	// ========================================================================

	#[rstest]
	fn test_with_body() {
		// Arrange
		let body = Bytes::from("hello");

		// Act
		let resp = TestResponse::with_body(StatusCode::OK, HeaderMap::new(), body.clone());

		// Assert
		assert_eq!(resp.status(), StatusCode::OK);
		assert_eq!(resp.body(), &body);
		assert_eq!(resp.version(), Version::HTTP_11);
	}

	#[rstest]
	fn test_with_body_and_version() {
		// Arrange / Act
		let resp = TestResponse::with_body_and_version(
			StatusCode::CREATED,
			HeaderMap::new(),
			Bytes::from("data"),
			Version::HTTP_2,
		);

		// Assert
		assert_eq!(resp.status(), StatusCode::CREATED);
		assert_eq!(resp.version(), Version::HTTP_2);
	}

	#[rstest]
	#[tokio::test]
	async fn test_new_async() {
		// Arrange
		let response = Response::builder()
			.status(StatusCode::OK)
			.body(Full::new(Bytes::from("Hello World")))
			.unwrap();

		// Act
		let test_resp = TestResponse::new(response).await;

		// Assert
		assert_eq!(test_resp.status(), StatusCode::OK);
		assert_eq!(test_resp.text(), "Hello World");
	}

	// ========================================================================
	// Getters
	// ========================================================================

	#[rstest]
	fn test_status() {
		// Arrange
		let resp = make_response(404, b"");

		// Act / Assert
		assert_eq!(resp.status(), StatusCode::NOT_FOUND);
	}

	#[rstest]
	fn test_status_code() {
		// Arrange
		let resp = make_response(201, b"");

		// Act / Assert
		assert_eq!(resp.status_code(), 201);
	}

	#[rstest]
	fn test_version() {
		// Arrange
		let resp = TestResponse::with_body(StatusCode::OK, HeaderMap::new(), Bytes::new());

		// Act / Assert
		assert_eq!(resp.version(), Version::HTTP_11);
	}

	#[rstest]
	fn test_headers() {
		// Arrange
		let mut headers = HeaderMap::new();
		headers.insert("x-custom", "value".parse().unwrap());
		let resp = TestResponse::with_body(StatusCode::OK, headers, Bytes::new());

		// Act / Assert
		assert!(resp.headers().contains_key("x-custom"));
	}

	#[rstest]
	fn test_body() {
		// Arrange
		let resp = make_response(200, b"body-content");

		// Act / Assert
		assert_eq!(resp.body().as_ref(), b"body-content");
	}

	#[rstest]
	fn test_text() {
		// Arrange
		let resp = make_response(200, b"hello text");

		// Act / Assert
		assert_eq!(resp.text(), "hello text");
	}

	#[rstest]
	fn test_text_non_utf8_lossy() {
		// Arrange
		let resp = TestResponse::with_body(
			StatusCode::OK,
			HeaderMap::new(),
			Bytes::from(vec![0xFF, 0xFE, 0x68, 0x69]),
		);

		// Act
		let text = resp.text();

		// Assert - lossy conversion replaces invalid bytes with replacement character
		assert!(text.contains("hi"));
		assert!(text.contains('\u{FFFD}'));
	}

	// ========================================================================
	// JSON parsing
	// ========================================================================

	#[rstest]
	fn test_json_valid() {
		// Arrange
		#[derive(serde::Deserialize, PartialEq, Debug)]
		struct Item {
			id: i32,
		}
		let resp = make_response(200, br#"{"id": 42}"#);

		// Act
		let item: Item = resp.json().unwrap();

		// Assert
		assert_eq!(item.id, 42);
	}

	#[rstest]
	fn test_json_invalid() {
		// Arrange
		#[derive(serde::Deserialize)]
		struct Item {
			#[allow(dead_code)] // Field used for deserialization target verification
			id: i32,
		}
		let resp = make_response(200, b"not json");

		// Act
		let result: Result<Item, _> = resp.json();

		// Assert
		assert!(result.is_err());
	}

	#[rstest]
	fn test_json_value_valid() {
		// Arrange
		let resp = make_response(200, br#"{"key": "value"}"#);

		// Act
		let val = resp.json_value().unwrap();

		// Assert
		assert_eq!(val["key"], "value");
	}

	#[rstest]
	fn test_json_value_invalid() {
		// Arrange
		let resp = make_response(200, b"broken");

		// Act
		let result = resp.json_value();

		// Assert
		assert!(result.is_err());
	}

	// ========================================================================
	// Status category checks
	// ========================================================================

	#[rstest]
	fn test_is_success_200() {
		// Arrange / Act / Assert
		assert!(make_response(200, b"").is_success());
	}

	#[rstest]
	fn test_is_success_299() {
		// Arrange / Act / Assert
		assert!(make_response(299, b"").is_success());
	}

	#[rstest]
	fn test_is_success_boundary_199() {
		// Arrange / Act / Assert
		assert!(!make_response(199, b"").is_success());
	}

	#[rstest]
	fn test_is_success_boundary_300() {
		// Arrange / Act / Assert
		assert!(!make_response(300, b"").is_success());
	}

	#[rstest]
	fn test_is_client_error_400() {
		// Arrange / Act / Assert
		assert!(make_response(400, b"").is_client_error());
	}

	#[rstest]
	fn test_is_client_error_boundary_399() {
		// Arrange / Act / Assert
		assert!(!make_response(399, b"").is_client_error());
	}

	#[rstest]
	fn test_is_client_error_boundary_499() {
		// Arrange / Act / Assert
		assert!(make_response(499, b"").is_client_error());
	}

	#[rstest]
	fn test_is_server_error_500() {
		// Arrange / Act / Assert
		assert!(make_response(500, b"").is_server_error());
	}

	#[rstest]
	fn test_is_server_error_boundary_499() {
		// Arrange / Act / Assert
		assert!(!make_response(499, b"").is_server_error());
	}

	// ========================================================================
	// content_type and header
	// ========================================================================

	#[rstest]
	fn test_content_type() {
		// Arrange
		let mut headers = HeaderMap::new();
		headers.insert("content-type", "application/json".parse().unwrap());
		let resp = TestResponse::with_body(StatusCode::OK, headers, Bytes::new());

		// Act / Assert
		assert_eq!(resp.content_type(), Some("application/json"));
	}

	#[rstest]
	fn test_content_type_absent() {
		// Arrange
		let resp = make_response(200, b"");

		// Act / Assert
		assert_eq!(resp.content_type(), None);
	}

	#[rstest]
	fn test_header_present() {
		// Arrange
		let mut headers = HeaderMap::new();
		headers.insert("x-request-id", "abc123".parse().unwrap());
		let resp = TestResponse::with_body(StatusCode::OK, headers, Bytes::new());

		// Act / Assert
		assert_eq!(resp.header("x-request-id"), Some("abc123"));
	}

	#[rstest]
	fn test_header_absent() {
		// Arrange
		let resp = make_response(200, b"");

		// Act / Assert
		assert_eq!(resp.header("x-missing"), None);
	}

	// ========================================================================
	// ResponseExt assertions
	// ========================================================================

	#[rstest]
	fn test_assert_ok() {
		// Arrange
		let resp = make_response(200, b"");

		// Act / Assert (should not panic)
		resp.assert_ok();
	}

	#[rstest]
	fn test_assert_created() {
		// Arrange
		let resp = make_response(201, b"");

		// Act / Assert
		resp.assert_created();
	}

	#[rstest]
	fn test_assert_no_content() {
		// Arrange
		let resp = make_response(204, b"");

		// Act / Assert
		resp.assert_no_content();
	}

	#[rstest]
	fn test_assert_bad_request() {
		// Arrange
		let resp = make_response(400, b"");

		// Act / Assert
		resp.assert_bad_request();
	}

	#[rstest]
	fn test_assert_unauthorized() {
		// Arrange
		let resp = make_response(401, b"");

		// Act / Assert
		resp.assert_unauthorized();
	}

	#[rstest]
	fn test_assert_forbidden() {
		// Arrange
		let resp = make_response(403, b"");

		// Act / Assert
		resp.assert_forbidden();
	}

	#[rstest]
	fn test_assert_not_found() {
		// Arrange
		let resp = make_response(404, b"");

		// Act / Assert
		resp.assert_not_found();
	}

	#[rstest]
	fn test_assert_success() {
		// Arrange
		let resp = make_response(200, b"");

		// Act / Assert
		resp.assert_success();
	}

	#[rstest]
	fn test_assert_client_error() {
		// Arrange
		let resp = make_response(404, b"");

		// Act / Assert
		resp.assert_client_error();
	}

	#[rstest]
	fn test_assert_server_error() {
		// Arrange
		let resp = make_response(500, b"");

		// Act / Assert
		resp.assert_server_error();
	}

	#[rstest]
	fn test_fluent_chaining() {
		// Arrange
		let resp = make_response(200, b"");

		// Act / Assert - fluent chaining should return &Self
		resp.assert_ok().assert_success();
	}

	#[rstest]
	#[should_panic(expected = "Expected status")]
	fn test_assert_status_mismatch() {
		// Arrange
		let resp = make_response(200, b"");

		// Act (should panic)
		resp.assert_status(StatusCode::NOT_FOUND);
	}
}
