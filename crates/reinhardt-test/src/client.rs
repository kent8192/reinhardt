//! API Client for testing
//!
//! Similar to DRF's APIClient, provides methods for making test requests
//! with authentication, cookies, and headers support.

use bytes::Bytes;
use http::{HeaderMap, HeaderValue, Method, Request, Response};
use http_body_util::{BodyExt, Full};
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use tokio::sync::RwLock;

use crate::response::TestResponse;

/// HTTP version configuration for APIClient
#[derive(Debug, Clone, Copy, Default)]
pub enum HttpVersion {
	/// Use HTTP/1.1 only
	Http1Only,
	/// Use HTTP/2 with prior knowledge (no upgrade negotiation)
	Http2PriorKnowledge,
	/// Auto-negotiate (default)
	#[default]
	Auto,
}

#[derive(Debug, Error)]
pub enum ClientError {
	#[error("HTTP error: {0}")]
	Http(#[from] http::Error),

	#[error("Hyper error: {0}")]
	Hyper(#[from] hyper::Error),

	#[error("Serialization error: {0}")]
	Serialization(#[from] serde_json::Error),

	#[error("Invalid header value: {0}")]
	InvalidHeaderValue(#[from] http::header::InvalidHeaderValue),

	#[error("Reqwest error: {0}")]
	Reqwest(#[from] reqwest::Error),

	#[error("Request failed: {0}")]
	RequestFailed(String),
}

impl ClientError {
	/// Returns true if the error is a timeout error
	pub fn is_timeout(&self) -> bool {
		match self {
			ClientError::Reqwest(e) => e.is_timeout(),
			_ => false,
		}
	}

	/// Returns true if the error is a connection error
	pub fn is_connect(&self) -> bool {
		match self {
			ClientError::Reqwest(e) => e.is_connect(),
			_ => false,
		}
	}

	/// Returns true if the error occurred during request building
	pub fn is_request(&self) -> bool {
		match self {
			ClientError::Reqwest(e) => e.is_request(),
			ClientError::Http(_) => true,
			ClientError::InvalidHeaderValue(_) => true,
			ClientError::Serialization(_) => true,
			ClientError::RequestFailed(_) => true,
			_ => false,
		}
	}
}

pub type ClientResult<T> = Result<T, ClientError>;

/// Type alias for request handler function
pub type RequestHandler = Arc<dyn Fn(Request<Full<Bytes>>) -> Response<Full<Bytes>> + Send + Sync>;

/// Builder for creating APIClient with custom configuration
///
/// # Example
/// ```rust,no_run
/// use reinhardt_test::client::{APIClientBuilder, HttpVersion};
/// use std::time::Duration;
///
/// let client = APIClientBuilder::new()
///     .base_url("http://localhost:8080")
///     .timeout(Duration::from_secs(30))
///     .http_version(HttpVersion::Http2PriorKnowledge)
///     .cookie_store(true)
///     .build();
/// ```
pub struct APIClientBuilder {
	base_url: String,
	timeout: Option<Duration>,
	http_version: HttpVersion,
	cookie_store: bool,
}

impl APIClientBuilder {
	/// Create a new builder with default configuration
	pub fn new() -> Self {
		Self {
			base_url: "http://testserver".to_string(),
			timeout: None,
			http_version: HttpVersion::Auto,
			cookie_store: false,
		}
	}

	/// Set the base URL for requests
	pub fn base_url(mut self, url: impl Into<String>) -> Self {
		self.base_url = url.into();
		self
	}

	/// Set the request timeout
	pub fn timeout(mut self, duration: Duration) -> Self {
		self.timeout = Some(duration);
		self
	}

	/// Set the HTTP version
	pub fn http_version(mut self, version: HttpVersion) -> Self {
		self.http_version = version;
		self
	}

	/// Use HTTP/1.1 only (convenience method)
	pub fn http1_only(mut self) -> Self {
		self.http_version = HttpVersion::Http1Only;
		self
	}

	/// Use HTTP/2 with prior knowledge (convenience method)
	pub fn http2_prior_knowledge(mut self) -> Self {
		self.http_version = HttpVersion::Http2PriorKnowledge;
		self
	}

	/// Enable or disable automatic cookie storage
	pub fn cookie_store(mut self, enabled: bool) -> Self {
		self.cookie_store = enabled;
		self
	}

	/// Build the APIClient
	pub fn build(self) -> APIClient {
		let mut client_builder = reqwest::Client::builder();

		// Configure timeout
		if let Some(timeout) = self.timeout {
			client_builder = client_builder.timeout(timeout);
		}

		// Configure HTTP version
		match self.http_version {
			HttpVersion::Http1Only => {
				client_builder = client_builder.http1_only();
			}
			HttpVersion::Http2PriorKnowledge => {
				client_builder = client_builder.http2_prior_knowledge();
			}
			HttpVersion::Auto => {
				// Default behavior, no special configuration needed
			}
		}

		// Configure cookie store
		if self.cookie_store {
			client_builder = client_builder.cookie_store(true);
		}

		let http_client = client_builder
			.build()
			.expect("Failed to build reqwest client");

		APIClient {
			base_url: self.base_url,
			default_headers: Arc::new(RwLock::new(HeaderMap::new())),
			cookies: Arc::new(RwLock::new(HashMap::new())),
			user: Arc::new(RwLock::new(None)),
			handler: None,
			http_client,
			use_cookie_store: self.cookie_store,
		}
	}
}

impl Default for APIClientBuilder {
	fn default() -> Self {
		Self::new()
	}
}

/// Test client for making API requests
///
/// # Example
/// ```rust,no_run
/// use reinhardt_test::APIClient;
/// use http::StatusCode;
/// use serde_json::json;
///
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let client = APIClient::with_base_url("http://localhost:8080");
/// let credentials = json!({"username": "user", "password": "pass"});
/// client.post("/auth/login", &credentials, "json").await?;
/// let response = client.get("/api/users/").await?;
/// assert_eq!(response.status(), StatusCode::OK);
/// # Ok(())
/// # }
/// ```
pub struct APIClient {
	/// Base URL for requests (e.g., "http://testserver")
	base_url: String,

	/// Default headers to include in all requests
	default_headers: Arc<RwLock<HeaderMap>>,

	/// Cookies to include in requests (manual management)
	cookies: Arc<RwLock<HashMap<String, String>>>,

	/// Current authenticated user (if any)
	user: Arc<RwLock<Option<Value>>>,

	/// Handler function for processing requests
	handler: Option<RequestHandler>,

	/// Reusable HTTP client with connection pooling
	http_client: reqwest::Client,

	/// Whether automatic cookie storage is enabled
	use_cookie_store: bool,
}

impl APIClient {
	/// Create a new API client
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_test::client::APIClient;
	///
	/// let client = APIClient::new();
	/// assert_eq!(client.base_url(), "http://testserver");
	/// ```
	pub fn new() -> Self {
		APIClientBuilder::new().build()
	}

	/// Create a client with a custom base URL
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_test::client::APIClient;
	///
	/// let client = APIClient::with_base_url("https://api.example.com");
	/// assert_eq!(client.base_url(), "https://api.example.com");
	/// ```
	pub fn with_base_url(base_url: impl Into<String>) -> Self {
		APIClientBuilder::new().base_url(base_url).build()
	}

	/// Create a builder for customizing the client configuration
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_test::client::APIClient;
	/// use std::time::Duration;
	///
	/// let client = APIClient::builder()
	///     .base_url("http://localhost:8080")
	///     .timeout(Duration::from_secs(30))
	///     .build();
	/// ```
	pub fn builder() -> APIClientBuilder {
		APIClientBuilder::new()
	}
	pub fn base_url(&self) -> &str {
		&self.base_url
	}
	/// Set a request handler for testing
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_test::client::APIClient;
	/// use http::{Request, Response, StatusCode};
	/// use http_body_util::Full;
	/// use bytes::Bytes;
	///
	/// let mut client = APIClient::new();
	/// client.set_handler(|_req| {
	///     Response::builder()
	///         .status(StatusCode::OK)
	///         .body(Full::new(Bytes::from("test")))
	///         .unwrap()
	/// });
	/// ```
	pub fn set_handler<F>(&mut self, handler: F)
	where
		F: Fn(Request<Full<Bytes>>) -> Response<Full<Bytes>> + Send + Sync + 'static,
	{
		self.handler = Some(Arc::new(handler));
	}
	/// Set a default header for all requests
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_test::client::APIClient;
	///
	/// # tokio_test::block_on(async {
	/// let client = APIClient::new();
	/// client.set_header("User-Agent", "TestClient/1.0").await.unwrap();
	/// # });
	/// ```
	pub async fn set_header(
		&self,
		name: impl AsRef<str>,
		value: impl AsRef<str>,
	) -> ClientResult<()> {
		let mut headers = self.default_headers.write().await;
		let header_name: http::header::HeaderName = name.as_ref().parse().map_err(|_| {
			ClientError::RequestFailed(format!("Invalid header name: {}", name.as_ref()))
		})?;
		headers.insert(header_name, HeaderValue::from_str(value.as_ref())?);
		Ok(())
	}
	/// Force authenticate as a user (for testing)
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_test::client::APIClient;
	/// use serde_json::json;
	///
	/// # tokio_test::block_on(async {
	/// let client = APIClient::new();
	/// let user = json!({"id": 1, "username": "testuser"});
	/// client.force_authenticate(Some(user)).await;
	/// # });
	/// ```
	pub async fn force_authenticate(&self, user: Option<Value>) {
		let mut current_user = self.user.write().await;
		*current_user = user;
	}
	/// Set credentials for Basic Authentication
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_test::client::APIClient;
	///
	/// # tokio_test::block_on(async {
	/// let client = APIClient::new();
	/// client.credentials("username", "password").await.unwrap();
	/// # });
	/// ```
	pub async fn credentials(&self, username: &str, password: &str) -> ClientResult<()> {
		let encoded = base64::encode(format!("{}:{}", username, password));
		self.set_header("Authorization", format!("Basic {}", encoded))
			.await
	}
	/// Clear authentication and cookies
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_test::client::APIClient;
	///
	/// # tokio_test::block_on(async {
	/// let client = APIClient::new();
	/// client.clear_auth().await.unwrap();
	/// # });
	/// ```
	pub async fn clear_auth(&self) -> ClientResult<()> {
		self.force_authenticate(None).await;
		let mut cookies = self.cookies.write().await;
		cookies.clear();
		Ok(())
	}

	/// Clean up all client state for teardown
	///
	/// This method performs a complete cleanup of the client state including:
	/// - Clearing authentication
	/// - Clearing cookies
	/// - Clearing default headers
	///
	/// This is typically called during test teardown to ensure clean state
	/// between tests.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_test::client::APIClient;
	///
	/// # tokio_test::block_on(async {
	/// let client = APIClient::new();
	/// client.set_header("X-Custom", "value").await.unwrap();
	/// client.cleanup().await;
	/// // All state is now cleared
	/// # });
	/// ```
	pub async fn cleanup(&self) {
		// Clear authentication
		self.force_authenticate(None).await;

		// Clear cookies
		{
			let mut cookies = self.cookies.write().await;
			cookies.clear();
		}

		// Clear default headers
		{
			let mut headers = self.default_headers.write().await;
			headers.clear();
		}
	}
	/// Make a GET request
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_test::client::APIClient;
	///
	/// # tokio_test::block_on(async {
	/// let client = APIClient::new();
	// Note: get() requires a working handler
	// let response = client.get("/api/users/").await;
	/// # });
	/// ```
	pub async fn get(&self, path: &str) -> ClientResult<TestResponse> {
		self.request(Method::GET, path, None, None).await
	}
	/// Make a POST request
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_test::client::APIClient;
	/// use serde_json::json;
	///
	/// # tokio_test::block_on(async {
	/// let client = APIClient::new();
	/// let data = json!({"name": "test"});
	// Note: post() requires a working handler
	// let response = client.post("/api/users/", &data, "json").await;
	/// # });
	/// ```
	pub async fn post<T: Serialize>(
		&self,
		path: &str,
		data: &T,
		format: &str,
	) -> ClientResult<TestResponse> {
		let body = self.serialize_data(data, format)?;
		let content_type = self.get_content_type(format);
		self.request(Method::POST, path, Some(body), Some(content_type))
			.await
	}
	/// Make a PUT request
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_test::client::APIClient;
	/// use serde_json::json;
	///
	/// # tokio_test::block_on(async {
	/// let client = APIClient::new();
	/// let data = json!({"name": "updated"});
	// Note: put() requires a working handler
	// let response = client.put("/api/users/1/", &data, "json").await;
	/// # });
	/// ```
	pub async fn put<T: Serialize>(
		&self,
		path: &str,
		data: &T,
		format: &str,
	) -> ClientResult<TestResponse> {
		let body = self.serialize_data(data, format)?;
		let content_type = self.get_content_type(format);
		self.request(Method::PUT, path, Some(body), Some(content_type))
			.await
	}
	/// Make a PATCH request
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_test::client::APIClient;
	/// use serde_json::json;
	///
	/// # tokio_test::block_on(async {
	/// let client = APIClient::new();
	/// let data = json!({"name": "partial_update"});
	// Note: patch() requires a working handler
	// let response = client.patch("/api/users/1/", &data, "json").await;
	/// # });
	/// ```
	pub async fn patch<T: Serialize>(
		&self,
		path: &str,
		data: &T,
		format: &str,
	) -> ClientResult<TestResponse> {
		let body = self.serialize_data(data, format)?;
		let content_type = self.get_content_type(format);
		self.request(Method::PATCH, path, Some(body), Some(content_type))
			.await
	}
	/// Make a DELETE request
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_test::client::APIClient;
	///
	/// # tokio_test::block_on(async {
	/// let client = APIClient::new();
	// Note: delete() requires a working handler
	// let response = client.delete("/api/users/1/").await;
	/// # });
	/// ```
	pub async fn delete(&self, path: &str) -> ClientResult<TestResponse> {
		self.request(Method::DELETE, path, None, None).await
	}
	/// Make a HEAD request
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_test::client::APIClient;
	///
	/// # tokio_test::block_on(async {
	/// let client = APIClient::new();
	// Note: head() requires a working handler
	// let response = client.head("/api/users/").await;
	/// # });
	/// ```
	pub async fn head(&self, path: &str) -> ClientResult<TestResponse> {
		self.request(Method::HEAD, path, None, None).await
	}
	/// Make an OPTIONS request
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_test::client::APIClient;
	///
	/// # tokio_test::block_on(async {
	/// let client = APIClient::new();
	// Note: options() requires a working handler
	// let response = client.options("/api/users/").await;
	/// # });
	/// ```
	pub async fn options(&self, path: &str) -> ClientResult<TestResponse> {
		self.request(Method::OPTIONS, path, None, None).await
	}

	/// Make a GET request with additional per-request headers
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_test::client::APIClient;
	///
	/// # tokio_test::block_on(async {
	/// let client = APIClient::with_base_url("http://localhost:8080");
	/// // let response = client.get_with_headers("/api/data", &[("Accept", "application/json")]).await;
	/// # });
	/// ```
	pub async fn get_with_headers(
		&self,
		path: &str,
		headers: &[(&str, &str)],
	) -> ClientResult<TestResponse> {
		self.request_with_extra_headers(Method::GET, path, None, None, headers)
			.await
	}

	/// Make a POST request with raw body and additional per-request headers
	///
	/// Unlike `post()`, this method allows setting a raw body without automatic serialization.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_test::client::APIClient;
	///
	/// # tokio_test::block_on(async {
	/// let client = APIClient::with_base_url("http://localhost:8080");
	/// // let response = client.post_raw_with_headers(
	/// //     "/api/echo",
	/// //     b"{\"test\":\"data\"}",
	/// //     "application/json",
	/// //     &[("X-Custom-Header", "value")]
	/// // ).await;
	/// # });
	/// ```
	pub async fn post_raw_with_headers(
		&self,
		path: &str,
		body: &[u8],
		content_type: &str,
		headers: &[(&str, &str)],
	) -> ClientResult<TestResponse> {
		self.request_with_extra_headers(
			Method::POST,
			path,
			Some(Bytes::copy_from_slice(body)),
			Some(content_type),
			headers,
		)
		.await
	}

	/// Make a POST request with raw body
	///
	/// Unlike `post()`, this method allows setting a raw body without automatic serialization.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_test::client::APIClient;
	///
	/// # tokio_test::block_on(async {
	/// let client = APIClient::with_base_url("http://localhost:8080");
	/// // let response = client.post_raw("/api/echo", b"{\"test\":\"data\"}", "application/json").await;
	/// # });
	/// ```
	pub async fn post_raw(
		&self,
		path: &str,
		body: &[u8],
		content_type: &str,
	) -> ClientResult<TestResponse> {
		self.request(
			Method::POST,
			path,
			Some(Bytes::copy_from_slice(body)),
			Some(content_type),
		)
		.await
	}

	/// Generic request method
	async fn request(
		&self,
		method: Method,
		path: &str,
		body: Option<Bytes>,
		content_type: Option<&str>,
	) -> ClientResult<TestResponse> {
		self.request_with_extra_headers(method, path, body, content_type, &[])
			.await
	}

	/// Generic request method with additional per-request headers
	///
	/// This method is similar to `request()` but allows adding extra headers
	/// that are specific to this request only, without modifying the default headers.
	async fn request_with_extra_headers(
		&self,
		method: Method,
		path: &str,
		body: Option<Bytes>,
		content_type: Option<&str>,
		extra_headers: &[(&str, &str)],
	) -> ClientResult<TestResponse> {
		let url = if path.starts_with("http://") || path.starts_with("https://") {
			path.to_string()
		} else {
			format!("{}{}", self.base_url, path)
		};

		let mut req_builder = Request::builder().method(method).uri(url);

		// Add default headers
		let default_headers = self.default_headers.read().await;
		for (name, value) in default_headers.iter() {
			req_builder = req_builder.header(name, value);
		}

		// Add extra per-request headers (these override default headers if same name)
		for (name, value) in extra_headers {
			req_builder = req_builder.header(*name, *value);
		}

		// Add content type if provided
		if let Some(ct) = content_type {
			req_builder = req_builder.header("Content-Type", ct);
		}

		// Add cookies (with validation to prevent header injection)
		let cookies = self.cookies.read().await;
		if !cookies.is_empty() {
			let cookie_header = cookies
				.iter()
				.map(|(k, v)| {
					validate_cookie_key(k);
					validate_cookie_value(v);
					format!("{}={}", k, v)
				})
				.collect::<Vec<_>>()
				.join("; ");
			req_builder = req_builder.header("Cookie", cookie_header);
		}

		// Add authentication if user is set
		let user = self.user.read().await;
		if user.is_some() {
			// Add custom header to indicate forced authentication
			req_builder = req_builder.header("X-Test-User", "authenticated");
		}

		// Build request with body
		let request = if let Some(body_bytes) = body {
			req_builder.body(Full::new(body_bytes))?
		} else {
			req_builder.body(Full::new(Bytes::new()))?
		};

		// Execute request
		let response = if let Some(handler) = &self.handler {
			// Use custom handler if set
			handler(request)
		} else {
			// Use reqwest for real HTTP requests when no handler is set
			let (parts, body) = request.into_parts();

			// Build reqwest request
			let url = if parts.uri.scheme_str().is_some() {
				// Absolute URL
				parts.uri.to_string()
			} else {
				// Relative path - use base_url
				format!(
					"{}{}",
					self.base_url.trim_end_matches('/'),
					parts.uri.path()
				)
			};

			// Use the stored http_client (connection pooling enabled)
			let mut reqwest_request = self.http_client.request(
				reqwest::Method::from_bytes(parts.method.as_str().as_bytes()).unwrap(),
				&url,
			);

			// Copy headers (skip Cookie if using cookie_store, as reqwest manages it automatically)
			for (name, value) in parts.headers.iter() {
				if self.use_cookie_store && name.as_str().eq_ignore_ascii_case("cookie") {
					continue;
				}
				reqwest_request = reqwest_request.header(name.as_str(), value.as_bytes());
			}

			// Copy body
			let body_bytes = body
				.collect()
				.await
				.map(|c| c.to_bytes())
				.unwrap_or_else(|_| Bytes::new());
			if !body_bytes.is_empty() {
				reqwest_request = reqwest_request.body(body_bytes.to_vec());
			}

			// Execute reqwest request
			let reqwest_response = reqwest_request.send().await?;

			// Convert reqwest response to http::Response
			let status = reqwest_response.status();
			let version = reqwest_response.version();
			let headers = reqwest_response.headers().clone();
			let body_bytes = reqwest_response.bytes().await?;

			let mut response_builder = Response::builder().status(status).version(version);
			for (name, value) in headers.iter() {
				response_builder = response_builder.header(name, value);
			}

			response_builder.body(Full::new(body_bytes))?
		};

		// Extract body from response using async collection
		let (parts, response_body) = response.into_parts();
		let body_data = response_body
			.collect()
			.await
			.map(|collected| collected.to_bytes())
			.unwrap_or_else(|_| Bytes::new());

		Ok(TestResponse::with_body_and_version(
			parts.status,
			parts.headers,
			body_data,
			parts.version,
		))
	}

	/// Serialize data based on format
	fn serialize_data<T: Serialize>(&self, data: &T, format: &str) -> ClientResult<Bytes> {
		match format {
			"json" => {
				let json = serde_json::to_vec(data)?;
				Ok(Bytes::from(json))
			}
			"form" => {
				// URL-encoded form data
				let json_value = serde_json::to_value(data)?;
				if let Value::Object(map) = json_value {
					let form_data = map
						.iter()
						.map(|(k, v)| {
							let value_str = match v {
								Value::String(s) => s.clone(),
								_ => v.to_string(),
							};
							format!(
								"{}={}",
								urlencoding::encode(k),
								urlencoding::encode(&value_str)
							)
						})
						.collect::<Vec<_>>()
						.join("&");
					Ok(Bytes::from(form_data))
				} else {
					Err(ClientError::RequestFailed(
						"Expected object for form data".to_string(),
					))
				}
			}
			_ => Err(ClientError::RequestFailed(format!(
				"Unsupported format: {}",
				format
			))),
		}
	}

	/// Get content type for format
	fn get_content_type(&self, format: &str) -> &str {
		match format {
			"json" => "application/json",
			"form" => "application/x-www-form-urlencoded",
			_ => "application/octet-stream",
		}
	}
}

/// Validate a cookie key to prevent header injection attacks.
///
/// Cookie keys must not contain `=`, `;`, whitespace, or control characters.
///
/// # Panics
///
/// Panics if the cookie key contains invalid characters.
fn validate_cookie_key(key: &str) {
	assert!(!key.is_empty(), "cookie key must not be empty");
	assert!(
		!key.contains('='),
		"cookie key must not contain '=' (found in key: {:?})",
		key
	);
	assert!(
		!key.contains(';'),
		"cookie key must not contain ';' (found in key: {:?})",
		key
	);
	assert!(
		!key.chars().any(|c| c.is_ascii_whitespace()),
		"cookie key must not contain whitespace (found in key: {:?})",
		key
	);
	assert!(
		!key.chars().any(|c| c.is_control()),
		"cookie key must not contain control characters (found in key: {:?})",
		key
	);
}

/// Validate a cookie value to prevent header injection attacks.
///
/// Cookie values must not contain `;`, newlines (`\r`, `\n`), or control characters.
///
/// # Panics
///
/// Panics if the cookie value contains invalid characters.
fn validate_cookie_value(value: &str) {
	assert!(
		!value.contains(';'),
		"cookie value must not contain ';' (found in value: {:?})",
		value
	);
	assert!(
		!value.contains('\r') && !value.contains('\n'),
		"cookie value must not contain newlines (found in value: {:?})",
		value
	);
	assert!(
		!value.chars().any(|c| c.is_control()),
		"cookie value must not contain control characters (found in value: {:?})",
		value
	);
}

impl Default for APIClient {
	fn default() -> Self {
		Self::new()
	}
}

// Need to add base64 dependency
mod base64 {
	pub(super) fn encode(input: String) -> String {
		// Simple base64 encoding (in production, use a proper library)
		use base64_simd::STANDARD;
		STANDARD.encode_to_string(input.as_bytes())
	}
}

// Need to add urlencoding
mod urlencoding {
	pub(super) fn encode(input: &str) -> String {
		url::form_urlencoded::byte_serialize(input.as_bytes()).collect()
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	#[rstest]
	fn test_validate_cookie_key_accepts_valid_key() {
		// Arrange
		let key = "session_id";

		// Act & Assert (should not panic)
		validate_cookie_key(key);
	}

	#[rstest]
	#[should_panic(expected = "must not be empty")]
	fn test_validate_cookie_key_rejects_empty() {
		// Arrange
		let key = "";

		// Act
		validate_cookie_key(key);
	}

	#[rstest]
	#[should_panic(expected = "must not contain '='")]
	fn test_validate_cookie_key_rejects_equals_sign() {
		// Arrange
		let key = "key=value";

		// Act
		validate_cookie_key(key);
	}

	#[rstest]
	#[should_panic(expected = "must not contain ';'")]
	fn test_validate_cookie_key_rejects_semicolon() {
		// Arrange
		let key = "key;injection";

		// Act
		validate_cookie_key(key);
	}

	#[rstest]
	#[should_panic(expected = "must not contain whitespace")]
	fn test_validate_cookie_key_rejects_whitespace() {
		// Arrange
		let key = "key name";

		// Act
		validate_cookie_key(key);
	}

	#[rstest]
	#[should_panic(expected = "must not contain control characters")]
	fn test_validate_cookie_key_rejects_control_chars() {
		// Arrange
		let key = "key\x00name";

		// Act
		validate_cookie_key(key);
	}

	#[rstest]
	fn test_validate_cookie_value_accepts_valid_value() {
		// Arrange
		let value = "abc123-token";

		// Act & Assert (should not panic)
		validate_cookie_value(value);
	}

	#[rstest]
	fn test_validate_cookie_value_accepts_empty() {
		// Arrange
		let value = "";

		// Act & Assert (should not panic)
		validate_cookie_value(value);
	}

	#[rstest]
	#[should_panic(expected = "must not contain ';'")]
	fn test_validate_cookie_value_rejects_semicolon() {
		// Arrange
		let value = "value; extra=injected";

		// Act
		validate_cookie_value(value);
	}

	#[rstest]
	#[should_panic(expected = "must not contain newlines")]
	fn test_validate_cookie_value_rejects_newline() {
		// Arrange
		let value = "value\r\nInjected-Header: malicious";

		// Act
		validate_cookie_value(value);
	}

	#[rstest]
	#[should_panic(expected = "must not contain control characters")]
	fn test_validate_cookie_value_rejects_control_chars() {
		// Arrange
		let value = "value\x01hidden";

		// Act
		validate_cookie_value(value);
	}

	#[rstest]
	#[should_panic(expected = "must not contain newlines")]
	fn test_validate_cookie_value_rejects_lf_only() {
		// Arrange
		let value = "value\nInjected-Header: evil";

		// Act
		validate_cookie_value(value);
	}
}
