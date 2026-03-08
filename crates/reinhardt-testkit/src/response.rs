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
