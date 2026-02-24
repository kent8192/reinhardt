//! Mock HTTP request and response utilities for server function testing.
//!
//! This module provides mock HTTP request/response types that can be used
//! to simulate HTTP interactions in server function tests.
//!
//! # Example
//!
//! ```rust,ignore
//! use reinhardt_test::server_fn::mock_request::{MockHttpRequest, MockHttpResponse};
//!
//! let request = MockHttpRequest::post("/api/users")
//!     .with_json(&CreateUserInput { name: "Alice".to_string() })
//!     .with_header("Authorization", "Bearer token");
//!
//! // Use request in server function testing
//! ```

#![cfg(not(target_arch = "wasm32"))]

use std::collections::HashMap;

use bytes::Bytes;
use http::{HeaderMap, HeaderValue, Method, StatusCode, Uri, header::HeaderName};
use serde::{Deserialize, Serialize};

/// Mock HTTP request for testing server functions.
///
/// This provides a fluent API for building HTTP requests without an actual
/// HTTP layer, useful for testing server functions directly.
#[derive(Debug, Clone)]
pub struct MockHttpRequest {
	/// The HTTP method.
	pub method: Method,
	/// The request URI.
	pub uri: Uri,
	/// Request headers.
	pub headers: HeaderMap,
	/// Request body as bytes.
	pub body: Bytes,
	/// Cookies extracted from headers.
	pub cookies: HashMap<String, String>,
	/// Query parameters.
	pub query_params: HashMap<String, String>,
}

impl Default for MockHttpRequest {
	fn default() -> Self {
		Self {
			method: Method::GET,
			uri: "/".parse().unwrap(),
			headers: HeaderMap::new(),
			body: Bytes::new(),
			cookies: HashMap::new(),
			query_params: HashMap::new(),
		}
	}
}

impl MockHttpRequest {
	/// Create a new mock request with the given method and URI.
	pub fn new(method: Method, uri: &str) -> Self {
		let parsed_uri: Uri = uri.parse().unwrap_or_else(|_| "/".parse().unwrap());

		// Extract query parameters
		let query_params = parsed_uri
			.query()
			.map(|q| {
				url::form_urlencoded::parse(q.as_bytes())
					.map(|(k, v)| (k.to_string(), v.to_string()))
					.collect()
			})
			.unwrap_or_default();

		Self {
			method,
			uri: parsed_uri,
			query_params,
			..Default::default()
		}
	}

	/// Create a GET request.
	pub fn get(uri: &str) -> Self {
		Self::new(Method::GET, uri)
	}

	/// Create a POST request.
	pub fn post(uri: &str) -> Self {
		Self::new(Method::POST, uri)
	}

	/// Create a PUT request.
	pub fn put(uri: &str) -> Self {
		Self::new(Method::PUT, uri)
	}

	/// Create a PATCH request.
	pub fn patch(uri: &str) -> Self {
		Self::new(Method::PATCH, uri)
	}

	/// Create a DELETE request.
	pub fn delete(uri: &str) -> Self {
		Self::new(Method::DELETE, uri)
	}

	/// Set the request body as JSON.
	///
	/// This serializes the value to JSON and sets the appropriate Content-Type header.
	///
	/// # Panics
	///
	/// Panics if the value cannot be serialized to JSON. Since this is a test
	/// utility, invalid input indicates a test setup error that should be
	/// caught immediately.
	// Fixes #876
	pub fn with_json<T: Serialize>(mut self, body: &T) -> Self {
		let bytes = serde_json::to_vec(body).unwrap_or_else(|err| {
			panic!("MockHttpRequest::with_json: failed to serialize body to JSON: {err}")
		});
		self.body = Bytes::from(bytes);
		self.headers.insert(
			http::header::CONTENT_TYPE,
			HeaderValue::from_static("application/json"),
		);
		self
	}

	/// Set the request body as form data.
	///
	/// This serializes the value as URL-encoded form data.
	///
	/// # Panics
	///
	/// Panics if the value cannot be serialized as form data. Since this is a
	/// test utility, invalid input indicates a test setup error that should be
	/// caught immediately.
	// Fixes #876
	pub fn with_form<T: Serialize>(mut self, body: &T) -> Self {
		let encoded = serde_urlencoded::to_string(body).unwrap_or_else(|err| {
			panic!("MockHttpRequest::with_form: failed to serialize body as form data: {err}")
		});
		self.body = Bytes::from(encoded);
		self.headers.insert(
			http::header::CONTENT_TYPE,
			HeaderValue::from_static("application/x-www-form-urlencoded"),
		);
		self
	}

	/// Set the request body as raw bytes.
	pub fn with_body(mut self, body: impl Into<Bytes>) -> Self {
		self.body = body.into();
		self
	}

	/// Set the request body as a string.
	pub fn with_text(mut self, body: impl Into<String>) -> Self {
		self.body = Bytes::from(body.into());
		self.headers.insert(
			http::header::CONTENT_TYPE,
			HeaderValue::from_static("text/plain"),
		);
		self
	}

	/// Add a request header.
	pub fn with_header(mut self, name: &str, value: &str) -> Self {
		if let (Ok(header_name), Ok(header_value)) = (
			HeaderName::from_bytes(name.as_bytes()),
			HeaderValue::from_str(value),
		) {
			self.headers.insert(header_name, header_value);
		}
		self
	}

	/// Add multiple headers.
	pub fn with_headers<'a>(
		mut self,
		headers: impl IntoIterator<Item = (&'a str, &'a str)>,
	) -> Self {
		for (name, value) in headers {
			if let (Ok(header_name), Ok(header_value)) = (
				HeaderName::from_bytes(name.as_bytes()),
				HeaderValue::from_str(value),
			) {
				self.headers.insert(header_name, header_value);
			}
		}
		self
	}

	/// Add a cookie.
	pub fn with_cookie(mut self, name: &str, value: &str) -> Self {
		self.cookies.insert(name.to_string(), value.to_string());
		self.update_cookie_header();
		self
	}

	/// Add multiple cookies.
	pub fn with_cookies<'a>(
		mut self,
		cookies: impl IntoIterator<Item = (&'a str, &'a str)>,
	) -> Self {
		for (name, value) in cookies {
			self.cookies.insert(name.to_string(), value.to_string());
		}
		self.update_cookie_header();
		self
	}

	/// Add a query parameter.
	pub fn with_query(mut self, name: &str, value: &str) -> Self {
		self.query_params
			.insert(name.to_string(), value.to_string());
		self.update_uri_query();
		self
	}

	/// Add multiple query parameters.
	pub fn with_query_params<'a>(
		mut self,
		params: impl IntoIterator<Item = (&'a str, &'a str)>,
	) -> Self {
		for (name, value) in params {
			self.query_params
				.insert(name.to_string(), value.to_string());
		}
		self.update_uri_query();
		self
	}

	/// Set the Authorization header with a Bearer token.
	pub fn with_bearer_token(self, token: &str) -> Self {
		self.with_header("Authorization", &format!("Bearer {}", token))
	}

	/// Set the Authorization header with Basic auth.
	pub fn with_basic_auth(self, username: &str, password: &str) -> Self {
		let credentials =
			base64_simd::STANDARD.encode_to_string(format!("{}:{}", username, password));
		self.with_header("Authorization", &format!("Basic {}", credentials))
	}

	/// Set the Content-Type header.
	pub fn with_content_type(self, content_type: &str) -> Self {
		self.with_header("Content-Type", content_type)
	}

	/// Set the Accept header.
	pub fn with_accept(self, accept: &str) -> Self {
		self.with_header("Accept", accept)
	}

	/// Get the request path (without query string).
	pub fn path(&self) -> &str {
		self.uri.path()
	}

	/// Get the full URI as a string.
	pub fn uri_string(&self) -> String {
		self.uri.to_string()
	}

	/// Get a header value.
	pub fn get_header(&self, name: &str) -> Option<&str> {
		self.headers.get(name).and_then(|v| v.to_str().ok())
	}

	/// Get a cookie value.
	pub fn get_cookie(&self, name: &str) -> Option<&str> {
		self.cookies.get(name).map(|s| s.as_str())
	}

	/// Get a query parameter value.
	pub fn get_query(&self, name: &str) -> Option<&str> {
		self.query_params.get(name).map(|s| s.as_str())
	}

	/// Parse the body as JSON.
	pub fn json<T: for<'de> Deserialize<'de>>(&self) -> Result<T, serde_json::Error> {
		serde_json::from_slice(&self.body)
	}

	/// Parse the body as form data.
	pub fn form<T: for<'de> Deserialize<'de>>(&self) -> Result<T, serde_urlencoded::de::Error> {
		serde_urlencoded::from_bytes(&self.body)
	}

	/// Get the body as a string.
	pub fn text(&self) -> Result<String, std::string::FromUtf8Error> {
		String::from_utf8(self.body.to_vec())
	}

	fn update_cookie_header(&mut self) {
		if self.cookies.is_empty() {
			self.headers.remove(http::header::COOKIE);
		} else {
			let cookie_str: String = self
				.cookies
				.iter()
				.map(|(k, v)| format!("{}={}", k, v))
				.collect::<Vec<_>>()
				.join("; ");

			if let Ok(value) = HeaderValue::from_str(&cookie_str) {
				self.headers.insert(http::header::COOKIE, value);
			}
		}
	}

	fn update_uri_query(&mut self) {
		let path = self.uri.path().to_string();
		if self.query_params.is_empty() {
			if let Ok(uri) = path.parse() {
				self.uri = uri;
			}
		} else {
			let query: String = self
				.query_params
				.iter()
				.map(|(k, v)| format!("{}={}", urlencoding::encode(k), urlencoding::encode(v)))
				.collect::<Vec<_>>()
				.join("&");

			if let Ok(uri) = format!("{}?{}", path, query).parse() {
				self.uri = uri;
			}
		}
	}
}

/// Mock HTTP response for testing.
///
/// This represents a response that can be returned from a server function
/// or used for assertions.
#[derive(Debug, Clone)]
pub struct MockHttpResponse {
	/// HTTP status code.
	pub status: StatusCode,
	/// Response headers.
	pub headers: HeaderMap,
	/// Response body.
	pub body: Bytes,
}

impl Default for MockHttpResponse {
	fn default() -> Self {
		Self {
			status: StatusCode::OK,
			headers: HeaderMap::new(),
			body: Bytes::new(),
		}
	}
}

impl MockHttpResponse {
	/// Create a new mock response with the given status.
	pub fn new(status: StatusCode) -> Self {
		Self {
			status,
			..Default::default()
		}
	}

	/// Create a successful (200 OK) response.
	pub fn ok() -> Self {
		Self::new(StatusCode::OK)
	}

	/// Create a created (201 Created) response.
	pub fn created() -> Self {
		Self::new(StatusCode::CREATED)
	}

	/// Create a no content (204 No Content) response.
	pub fn no_content() -> Self {
		Self::new(StatusCode::NO_CONTENT)
	}

	/// Create a bad request (400) response.
	pub fn bad_request() -> Self {
		Self::new(StatusCode::BAD_REQUEST)
	}

	/// Create an unauthorized (401) response.
	pub fn unauthorized() -> Self {
		Self::new(StatusCode::UNAUTHORIZED)
	}

	/// Create a forbidden (403) response.
	pub fn forbidden() -> Self {
		Self::new(StatusCode::FORBIDDEN)
	}

	/// Create a not found (404) response.
	pub fn not_found() -> Self {
		Self::new(StatusCode::NOT_FOUND)
	}

	/// Create an internal server error (500) response.
	pub fn internal_error() -> Self {
		Self::new(StatusCode::INTERNAL_SERVER_ERROR)
	}

	/// Create a JSON response.
	pub fn json<T: Serialize>(body: &T) -> Self {
		let mut response = Self::ok();
		if let Ok(bytes) = serde_json::to_vec(body) {
			response.body = Bytes::from(bytes);
			response.headers.insert(
				http::header::CONTENT_TYPE,
				HeaderValue::from_static("application/json"),
			);
		}
		response
	}

	/// Create a text response.
	pub fn text(body: impl Into<String>) -> Self {
		let mut response = Self::ok();
		response.body = Bytes::from(body.into());
		response.headers.insert(
			http::header::CONTENT_TYPE,
			HeaderValue::from_static("text/plain"),
		);
		response
	}

	/// Set the response body as JSON.
	pub fn with_json<T: Serialize>(mut self, body: &T) -> Self {
		if let Ok(bytes) = serde_json::to_vec(body) {
			self.body = Bytes::from(bytes);
			self.headers.insert(
				http::header::CONTENT_TYPE,
				HeaderValue::from_static("application/json"),
			);
		}
		self
	}

	/// Set the response body.
	pub fn with_body(mut self, body: impl Into<Bytes>) -> Self {
		self.body = body.into();
		self
	}

	/// Set the status code.
	pub fn with_status(mut self, status: StatusCode) -> Self {
		self.status = status;
		self
	}

	/// Add a header.
	pub fn with_header(mut self, name: &str, value: &str) -> Self {
		if let (Ok(header_name), Ok(header_value)) = (
			HeaderName::from_bytes(name.as_bytes()),
			HeaderValue::from_str(value),
		) {
			self.headers.insert(header_name, header_value);
		}
		self
	}

	/// Add a Set-Cookie header.
	pub fn with_cookie(mut self, name: &str, value: &str, options: Option<CookieOptions>) -> Self {
		let opts = options.unwrap_or_default();
		let mut cookie = format!("{}={}", name, value);

		if let Some(max_age) = opts.max_age {
			cookie.push_str(&format!("; Max-Age={}", max_age));
		}
		if let Some(ref path) = opts.path {
			cookie.push_str(&format!("; Path={}", path));
		}
		if let Some(ref domain) = opts.domain {
			cookie.push_str(&format!("; Domain={}", domain));
		}
		if opts.secure {
			cookie.push_str("; Secure");
		}
		if opts.http_only {
			cookie.push_str("; HttpOnly");
		}
		if let Some(ref same_site) = opts.same_site {
			cookie.push_str(&format!("; SameSite={}", same_site));
		}

		if let Ok(value) = HeaderValue::from_str(&cookie) {
			self.headers.append(http::header::SET_COOKIE, value);
		}
		self
	}

	/// Check if the response is successful (2xx).
	pub fn is_success(&self) -> bool {
		self.status.is_success()
	}

	/// Check if the response is a client error (4xx).
	pub fn is_client_error(&self) -> bool {
		self.status.is_client_error()
	}

	/// Check if the response is a server error (5xx).
	pub fn is_server_error(&self) -> bool {
		self.status.is_server_error()
	}

	/// Get a header value.
	pub fn get_header(&self, name: &str) -> Option<&str> {
		self.headers.get(name).and_then(|v| v.to_str().ok())
	}

	/// Parse the body as JSON.
	pub fn json_body<T: for<'de> Deserialize<'de>>(&self) -> Result<T, serde_json::Error> {
		serde_json::from_slice(&self.body)
	}

	/// Get the body as a string.
	pub fn text_body(&self) -> Result<String, std::string::FromUtf8Error> {
		String::from_utf8(self.body.to_vec())
	}
}

/// Cookie options for Set-Cookie header.
#[derive(Debug, Clone, Default)]
pub struct CookieOptions {
	/// Max-Age in seconds.
	pub max_age: Option<i64>,
	/// Cookie path.
	pub path: Option<String>,
	/// Cookie domain.
	pub domain: Option<String>,
	/// Secure flag.
	pub secure: bool,
	/// HttpOnly flag.
	pub http_only: bool,
	/// SameSite attribute.
	pub same_site: Option<String>,
}

impl CookieOptions {
	/// Create new default cookie options.
	pub fn new() -> Self {
		Self::default()
	}

	/// Set max age in seconds.
	pub fn max_age(mut self, seconds: i64) -> Self {
		self.max_age = Some(seconds);
		self
	}

	/// Set the path.
	pub fn path(mut self, path: impl Into<String>) -> Self {
		self.path = Some(path.into());
		self
	}

	/// Set the domain.
	pub fn domain(mut self, domain: impl Into<String>) -> Self {
		self.domain = Some(domain.into());
		self
	}

	/// Enable secure flag.
	pub fn secure(mut self) -> Self {
		self.secure = true;
		self
	}

	/// Enable HTTP-only flag.
	pub fn http_only(mut self) -> Self {
		self.http_only = true;
		self
	}

	/// Set SameSite to Strict.
	pub fn same_site_strict(mut self) -> Self {
		self.same_site = Some("Strict".to_string());
		self
	}

	/// Set SameSite to Lax.
	pub fn same_site_lax(mut self) -> Self {
		self.same_site = Some("Lax".to_string());
		self
	}

	/// Set SameSite to None.
	pub fn same_site_none(mut self) -> Self {
		self.same_site = Some("None".to_string());
		self.secure = true; // None requires Secure
		self
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_mock_request_get() {
		let request = MockHttpRequest::get("/api/users");
		assert_eq!(request.method, Method::GET);
		assert_eq!(request.path(), "/api/users");
	}

	#[test]
	fn test_mock_request_post_json() {
		#[derive(Serialize)]
		struct Input {
			name: String,
		}

		let request = MockHttpRequest::post("/api/users").with_json(&Input {
			name: "Alice".to_string(),
		});

		assert_eq!(request.method, Method::POST);
		assert_eq!(request.get_header("content-type"), Some("application/json"));
		assert!(request.text().unwrap().contains("Alice"));
	}

	#[test]
	fn test_mock_request_with_headers() {
		let request = MockHttpRequest::get("/api")
			.with_header("X-Custom", "value")
			.with_bearer_token("token123");

		assert_eq!(request.get_header("x-custom"), Some("value"));
		assert_eq!(request.get_header("authorization"), Some("Bearer token123"));
	}

	#[test]
	fn test_mock_request_with_cookies() {
		let request = MockHttpRequest::get("/api")
			.with_cookie("session", "abc")
			.with_cookie("user", "123");

		assert_eq!(request.get_cookie("session"), Some("abc"));
		assert_eq!(request.get_cookie("user"), Some("123"));
	}

	#[test]
	fn test_mock_request_with_query() {
		let request = MockHttpRequest::get("/api/search")
			.with_query("q", "test")
			.with_query("page", "1");

		assert_eq!(request.get_query("q"), Some("test"));
		assert_eq!(request.get_query("page"), Some("1"));
	}

	#[test]
	fn test_mock_response_json() {
		#[derive(Serialize, Deserialize, PartialEq, Debug)]
		struct Output {
			id: i32,
		}

		let response = MockHttpResponse::json(&Output { id: 1 });

		assert!(response.is_success());
		assert_eq!(
			response.get_header("content-type"),
			Some("application/json")
		);

		let body: Output = response.json_body().unwrap();
		assert_eq!(body.id, 1);
	}

	#[test]
	fn test_mock_response_with_cookie() {
		let response = MockHttpResponse::ok().with_cookie(
			"session",
			"xyz",
			Some(
				CookieOptions::new()
					.max_age(3600)
					.path("/")
					.secure()
					.http_only(),
			),
		);

		let cookie = response.get_header("set-cookie").unwrap();
		assert!(cookie.contains("session=xyz"));
		assert!(cookie.contains("Max-Age=3600"));
		assert!(cookie.contains("Path=/"));
		assert!(cookie.contains("Secure"));
		assert!(cookie.contains("HttpOnly"));
	}
}
