use bytes::Bytes;
use futures::stream::Stream;
use hyper::{HeaderMap, StatusCode};
use serde::Serialize;
use std::pin::Pin;

/// Returns a safe, client-facing error message based on the HTTP status code.
///
/// For 5xx errors, always returns a generic message to prevent information leakage.
/// For 4xx errors, returns a descriptive but safe category message.
/// Internal details are never exposed to clients.
fn safe_error_message(status: StatusCode) -> &'static str {
	match status.as_u16() {
		400 => "Bad Request",
		401 => "Unauthorized",
		403 => "Forbidden",
		404 => "Not Found",
		405 => "Method Not Allowed",
		406 => "Not Acceptable",
		408 => "Request Timeout",
		409 => "Conflict",
		410 => "Gone",
		413 => "Payload Too Large",
		415 => "Unsupported Media Type",
		422 => "Unprocessable Entity",
		429 => "Too Many Requests",
		// All 5xx errors get generic messages
		500 => "Internal Server Error",
		502 => "Bad Gateway",
		503 => "Service Unavailable",
		504 => "Gateway Timeout",
		_ if status.is_client_error() => "Client Error",
		_ if status.is_server_error() => "Server Error",
		_ => "Error",
	}
}

/// Extract a safe, client-facing detail message from an error.
///
/// Returns `None` if no safe detail can be extracted.
/// Only extracts details from error variants where the message is
/// controlled by application code and safe for client exposure.
fn safe_client_error_detail(error: &crate::Error) -> Option<String> {
	use crate::Error;
	match error {
		Error::Validation(msg) => Some(msg.clone()),
		Error::ParseError(_) => Some("Invalid request format".to_string()),
		Error::BodyAlreadyConsumed => Some("Request body has already been consumed".to_string()),
		Error::MissingContentType => Some("Missing Content-Type header".to_string()),
		Error::InvalidPage(msg) => Some(format!("Invalid page: {}", msg)),
		Error::InvalidCursor(_) => Some("Invalid cursor value".to_string()),
		Error::InvalidLimit(msg) => Some(format!("Invalid limit: {}", msg)),
		Error::MissingParameter(name) => Some(format!("Missing parameter: {}", name)),
		Error::ParamValidation(ctx) => {
			Some(format!("{} parameter extraction failed", ctx.param_type))
		}
		// For other client errors, return generic message
		_ => None,
	}
}

/// Builder for creating error responses that prevent information leakage.
///
/// In production mode (default), error responses contain only safe,
/// generic messages. In debug mode, full error details are included.
///
/// # Examples
///
/// ```
/// use reinhardt_http::response::SafeErrorResponse;
/// use hyper::StatusCode;
///
/// // Production-safe response
/// let response = SafeErrorResponse::new(StatusCode::INTERNAL_SERVER_ERROR)
///     .build();
///
/// // Debug response with details
/// let response = SafeErrorResponse::new(StatusCode::BAD_REQUEST)
///     .with_detail("Missing required field: name")
///     .build();
/// ```
pub struct SafeErrorResponse {
	status: StatusCode,
	detail: Option<String>,
	debug_info: Option<String>,
	debug_mode: bool,
}

impl SafeErrorResponse {
	/// Create a new `SafeErrorResponse` with the given HTTP status code.
	pub fn new(status: StatusCode) -> Self {
		Self {
			status,
			detail: None,
			debug_info: None,
			debug_mode: false,
		}
	}

	/// Add a safe, client-facing detail message.
	///
	/// Only included for 4xx errors. Ignored for 5xx errors to prevent
	/// accidental information leakage.
	pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
		self.detail = Some(detail.into());
		self
	}

	/// Add debug information (only included when debug_mode is true).
	///
	/// WARNING: Only use in development environments.
	pub fn with_debug_info(mut self, info: impl Into<String>) -> Self {
		self.debug_info = Some(info.into());
		self
	}

	/// Enable debug mode to include full error details in responses.
	///
	/// WARNING: Only use in development environments. Debug mode exposes
	/// internal error details that may leak sensitive information.
	pub fn with_debug_mode(mut self, debug: bool) -> Self {
		self.debug_mode = debug;
		self
	}

	/// Build the `Response` with safe error content.
	pub fn build(self) -> Response {
		let message = safe_error_message(self.status);
		let mut body = serde_json::json!({
			"error": message,
		});

		// Only include detail for 4xx errors to prevent info leakage on 5xx
		if self.status.is_client_error()
			&& let Some(detail) = &self.detail
		{
			body["detail"] = serde_json::Value::String(detail.clone());
		}

		// Include debug info only when explicitly enabled
		if self.debug_mode {
			if let Some(debug_info) = &self.debug_info {
				body["debug"] = serde_json::Value::String(debug_info.clone());
			}
			// In debug mode, include detail even for 5xx
			if self.status.is_server_error()
				&& let Some(detail) = &self.detail
			{
				body["detail"] = serde_json::Value::String(detail.clone());
			}
		}

		Response::new(self.status)
			.with_json(&body)
			.unwrap_or_else(|_| Response::internal_server_error())
	}
}

/// Truncate a string for safe inclusion in log messages.
///
/// Prevents oversized values from consuming log storage and
/// limits exposure of sensitive data in error contexts.
///
/// # Examples
///
/// ```
/// use reinhardt_http::response::truncate_for_log;
///
/// let short = truncate_for_log("hello", 10);
/// assert_eq!(short, "hello");
///
/// let long = truncate_for_log("a]".repeat(100).as_str(), 10);
/// assert!(long.contains("...[truncated"));
/// ```
pub fn truncate_for_log(input: &str, max_length: usize) -> String {
	if input.len() <= max_length {
		input.to_string()
	} else {
		// Find valid UTF-8 boundary at or before max_length
		let truncate_at = input
			.char_indices()
			.take_while(|&(i, _)| i <= max_length)
			.last()
			.map(|(i, _)| i)
			.unwrap_or(0);
		format!(
			"{}...[truncated, {} total bytes]",
			&input[..truncate_at],
			input.len()
		)
	}
}

/// HTTP Response representation
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Response {
	pub status: StatusCode,
	pub headers: HeaderMap,
	pub body: Bytes,
	/// Indicates whether the middleware chain should stop processing
	/// When true, no further middleware or handlers will be executed
	stop_chain: bool,
}

/// Streaming HTTP Response
pub struct StreamingResponse<S> {
	pub status: StatusCode,
	pub headers: HeaderMap,
	pub stream: S,
}

/// Type alias for streaming body
pub type StreamBody =
	Pin<Box<dyn Stream<Item = Result<Bytes, Box<dyn std::error::Error + Send + Sync>>> + Send>>;

impl Response {
	/// Create a new Response with the given status code
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_http::Response;
	/// use hyper::StatusCode;
	///
	/// let response = Response::new(StatusCode::OK);
	/// assert_eq!(response.status, StatusCode::OK);
	/// assert!(response.body.is_empty());
	/// ```
	pub fn new(status: StatusCode) -> Self {
		Self {
			status,
			headers: HeaderMap::new(),
			body: Bytes::new(),
			stop_chain: false,
		}
	}
	/// Create a Response with HTTP 200 OK status
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_http::Response;
	/// use hyper::StatusCode;
	///
	/// let response = Response::ok();
	/// assert_eq!(response.status, StatusCode::OK);
	/// ```
	pub fn ok() -> Self {
		Self::new(StatusCode::OK)
	}
	/// Create a Response with HTTP 201 Created status
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_http::Response;
	/// use hyper::StatusCode;
	///
	/// let response = Response::created();
	/// assert_eq!(response.status, StatusCode::CREATED);
	/// ```
	pub fn created() -> Self {
		Self::new(StatusCode::CREATED)
	}
	/// Create a Response with HTTP 204 No Content status
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_http::Response;
	/// use hyper::StatusCode;
	///
	/// let response = Response::no_content();
	/// assert_eq!(response.status, StatusCode::NO_CONTENT);
	/// ```
	pub fn no_content() -> Self {
		Self::new(StatusCode::NO_CONTENT)
	}
	/// Create a Response with HTTP 400 Bad Request status
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_http::Response;
	/// use hyper::StatusCode;
	///
	/// let response = Response::bad_request();
	/// assert_eq!(response.status, StatusCode::BAD_REQUEST);
	/// ```
	pub fn bad_request() -> Self {
		Self::new(StatusCode::BAD_REQUEST)
	}
	/// Create a Response with HTTP 401 Unauthorized status
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_http::Response;
	/// use hyper::StatusCode;
	///
	/// let response = Response::unauthorized();
	/// assert_eq!(response.status, StatusCode::UNAUTHORIZED);
	/// ```
	pub fn unauthorized() -> Self {
		Self::new(StatusCode::UNAUTHORIZED)
	}
	/// Create a Response with HTTP 403 Forbidden status
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_http::Response;
	/// use hyper::StatusCode;
	///
	/// let response = Response::forbidden();
	/// assert_eq!(response.status, StatusCode::FORBIDDEN);
	/// ```
	pub fn forbidden() -> Self {
		Self::new(StatusCode::FORBIDDEN)
	}
	/// Create a Response with HTTP 404 Not Found status
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_http::Response;
	/// use hyper::StatusCode;
	///
	/// let response = Response::not_found();
	/// assert_eq!(response.status, StatusCode::NOT_FOUND);
	/// ```
	pub fn not_found() -> Self {
		Self::new(StatusCode::NOT_FOUND)
	}
	/// Create a Response with HTTP 500 Internal Server Error status
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_http::Response;
	/// use hyper::StatusCode;
	///
	/// let response = Response::internal_server_error();
	/// assert_eq!(response.status, StatusCode::INTERNAL_SERVER_ERROR);
	/// ```
	pub fn internal_server_error() -> Self {
		Self::new(StatusCode::INTERNAL_SERVER_ERROR)
	}
	/// Create a Response with HTTP 410 Gone status
	///
	/// Used when a resource has been permanently removed.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_http::Response;
	/// use hyper::StatusCode;
	///
	/// let response = Response::gone();
	/// assert_eq!(response.status, StatusCode::GONE);
	/// ```
	pub fn gone() -> Self {
		Self::new(StatusCode::GONE)
	}
	/// Create a Response with HTTP 301 Moved Permanently (permanent redirect)
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_http::Response;
	/// use hyper::StatusCode;
	///
	/// let response = Response::permanent_redirect("/new-location");
	/// assert_eq!(response.status, StatusCode::MOVED_PERMANENTLY);
	/// assert_eq!(
	///     response.headers.get("location").unwrap().to_str().unwrap(),
	///     "/new-location"
	/// );
	/// ```
	pub fn permanent_redirect(location: impl AsRef<str>) -> Self {
		Self::new(StatusCode::MOVED_PERMANENTLY).with_location(location.as_ref())
	}
	/// Create a Response with HTTP 302 Found (temporary redirect)
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_http::Response;
	/// use hyper::StatusCode;
	///
	/// let response = Response::temporary_redirect("/temp-location");
	/// assert_eq!(response.status, StatusCode::FOUND);
	/// assert_eq!(
	///     response.headers.get("location").unwrap().to_str().unwrap(),
	///     "/temp-location"
	/// );
	/// ```
	pub fn temporary_redirect(location: impl AsRef<str>) -> Self {
		Self::new(StatusCode::FOUND).with_location(location.as_ref())
	}
	/// Create a Response with HTTP 307 Temporary Redirect (preserves HTTP method)
	///
	/// Unlike 302, this guarantees the request method is preserved during redirect.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_http::Response;
	/// use hyper::StatusCode;
	///
	/// let response = Response::temporary_redirect_preserve_method("/temp-location");
	/// assert_eq!(response.status, StatusCode::TEMPORARY_REDIRECT);
	/// assert_eq!(
	///     response.headers.get("location").unwrap().to_str().unwrap(),
	///     "/temp-location"
	/// );
	/// ```
	pub fn temporary_redirect_preserve_method(location: impl AsRef<str>) -> Self {
		Self::new(StatusCode::TEMPORARY_REDIRECT).with_location(location.as_ref())
	}
	/// Set the response body
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_http::Response;
	/// use bytes::Bytes;
	///
	/// let response = Response::ok().with_body("Hello, World!");
	/// assert_eq!(response.body, Bytes::from("Hello, World!"));
	/// ```
	pub fn with_body(mut self, body: impl Into<Bytes>) -> Self {
		self.body = body.into();
		self
	}
	/// Try to add a custom header to the response, returning an error on invalid inputs.
	///
	/// # Errors
	///
	/// Returns `Err` if the header name or value is invalid according to HTTP specifications.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_http::Response;
	///
	/// let response = Response::ok().try_with_header("X-Custom-Header", "custom-value").unwrap();
	/// assert_eq!(
	///     response.headers.get("X-Custom-Header").unwrap().to_str().unwrap(),
	///     "custom-value"
	/// );
	/// ```
	///
	/// ```
	/// use reinhardt_http::Response;
	///
	/// // Invalid header names return an error instead of panicking
	/// let result = Response::ok().try_with_header("Invalid Header", "value");
	/// assert!(result.is_err());
	/// ```
	pub fn try_with_header(mut self, name: &str, value: &str) -> crate::Result<Self> {
		let header_name = hyper::header::HeaderName::from_bytes(name.as_bytes())
			.map_err(|e| crate::Error::Http(format!("Invalid header name '{}': {}", name, e)))?;
		let header_value = hyper::header::HeaderValue::from_str(value).map_err(|e| {
			crate::Error::Http(format!("Invalid header value for '{}': {}", name, e))
		})?;
		self.headers.insert(header_name, header_value);
		Ok(self)
	}

	/// Add a custom header to the response.
	///
	/// Invalid header names or values are silently ignored.
	/// Use `try_with_header` if you need error reporting.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_http::Response;
	///
	/// let response = Response::ok().with_header("X-Custom-Header", "custom-value");
	/// assert_eq!(
	///     response.headers.get("X-Custom-Header").unwrap().to_str().unwrap(),
	///     "custom-value"
	/// );
	/// ```
	///
	/// ```
	/// use reinhardt_http::Response;
	///
	/// // Invalid header names are silently ignored (no panic)
	/// let response = Response::ok().with_header("Invalid Header", "value");
	/// assert!(response.headers.is_empty());
	/// ```
	pub fn with_header(mut self, name: &str, value: &str) -> Self {
		if let Ok(header_name) = hyper::header::HeaderName::from_bytes(name.as_bytes())
			&& let Ok(header_value) = hyper::header::HeaderValue::from_str(value)
		{
			self.headers.insert(header_name, header_value);
		}
		self
	}
	/// Add a Location header to the response (typically used for redirects)
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_http::Response;
	/// use hyper::StatusCode;
	///
	/// let response = Response::new(StatusCode::FOUND).with_location("/redirect-target");
	/// assert_eq!(
	///     response.headers.get("location").unwrap().to_str().unwrap(),
	///     "/redirect-target"
	/// );
	/// ```
	pub fn with_location(mut self, location: &str) -> Self {
		if let Ok(value) = hyper::header::HeaderValue::from_str(location) {
			self.headers.insert(hyper::header::LOCATION, value);
		}
		self
	}
	/// Set the response body to JSON and add appropriate Content-Type header
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_http::Response;
	/// use serde_json::json;
	///
	/// let data = json!({"message": "Hello, World!"});
	/// let response = Response::ok().with_json(&data).unwrap();
	///
	/// assert_eq!(
	///     response.headers.get("content-type").unwrap().to_str().unwrap(),
	///     "application/json"
	/// );
	/// ```
	pub fn with_json<T: Serialize>(mut self, data: &T) -> crate::Result<Self> {
		use crate::Error;
		let json = serde_json::to_vec(data).map_err(|e| Error::Serialization(e.to_string()))?;
		self.body = Bytes::from(json);
		self.headers.insert(
			hyper::header::CONTENT_TYPE,
			hyper::header::HeaderValue::from_static("application/json"),
		);
		Ok(self)
	}
	/// Add a custom header using typed HeaderName and HeaderValue
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_http::Response;
	/// use hyper::header::{HeaderName, HeaderValue};
	///
	/// let header_name = HeaderName::from_static("x-custom-header");
	/// let header_value = HeaderValue::from_static("custom-value");
	/// let response = Response::ok().with_typed_header(header_name, header_value);
	///
	/// assert_eq!(
	///     response.headers.get("x-custom-header").unwrap().to_str().unwrap(),
	///     "custom-value"
	/// );
	/// ```
	pub fn with_typed_header(
		mut self,
		key: hyper::header::HeaderName,
		value: hyper::header::HeaderValue,
	) -> Self {
		self.headers.insert(key, value);
		self
	}

	/// Check if this response should stop the middleware chain
	///
	/// When true, no further middleware or handlers will be executed.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_http::Response;
	///
	/// let response = Response::ok();
	/// assert!(!response.should_stop_chain());
	///
	/// let stopping_response = Response::ok().with_stop_chain(true);
	/// assert!(stopping_response.should_stop_chain());
	/// ```
	pub fn should_stop_chain(&self) -> bool {
		self.stop_chain
	}

	/// Set whether this response should stop the middleware chain
	///
	/// When set to true, the middleware chain will stop processing and return
	/// this response immediately, skipping any remaining middleware and handlers.
	///
	/// This is useful for early returns in middleware, such as:
	/// - Authentication failures (401 Unauthorized)
	/// - CORS preflight responses (204 No Content)
	/// - Rate limiting rejections (429 Too Many Requests)
	/// - Cache hits (304 Not Modified)
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_http::Response;
	/// use hyper::StatusCode;
	///
	/// // Early return for authentication failure
	/// let auth_failure = Response::unauthorized()
	///     .with_body("Authentication required")
	///     .with_stop_chain(true);
	/// assert!(auth_failure.should_stop_chain());
	///
	/// // CORS preflight response
	/// let preflight = Response::no_content()
	///     .with_header("Access-Control-Allow-Origin", "*")
	///     .with_stop_chain(true);
	/// assert!(preflight.should_stop_chain());
	/// ```
	pub fn with_stop_chain(mut self, stop: bool) -> Self {
		self.stop_chain = stop;
		self
	}
}

impl From<crate::Error> for Response {
	fn from(error: crate::Error) -> Self {
		let status =
			StatusCode::from_u16(error.status_code()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);

		// Log the full error for server-side debugging
		tracing::error!(
			status = status.as_u16(),
			error = %error,
			"Request error"
		);

		let mut response = SafeErrorResponse::new(status);

		// For 4xx client errors, include a safe detail message
		// that doesn't expose internal implementation details
		if status.is_client_error()
			&& let Some(detail) = safe_client_error_detail(&error)
		{
			response = response.with_detail(detail);
		}

		response.build()
	}
}

impl<S> StreamingResponse<S>
where
	S: Stream<Item = Result<Bytes, Box<dyn std::error::Error + Send + Sync>>> + Send + 'static,
{
	/// Create a new streaming response with OK status
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_http::StreamingResponse;
	/// use hyper::StatusCode;
	/// use futures::stream;
	/// use bytes::Bytes;
	///
	/// let data = vec![Ok(Bytes::from("chunk1")), Ok(Bytes::from("chunk2"))];
	/// let stream = stream::iter(data);
	/// let response = StreamingResponse::new(stream);
	///
	/// assert_eq!(response.status, StatusCode::OK);
	/// ```
	pub fn new(stream: S) -> Self {
		Self {
			status: StatusCode::OK,
			headers: HeaderMap::new(),
			stream,
		}
	}
	/// Create a streaming response with a specific status code
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_http::StreamingResponse;
	/// use hyper::StatusCode;
	/// use futures::stream;
	/// use bytes::Bytes;
	///
	/// let data = vec![Ok(Bytes::from("data"))];
	/// let stream = stream::iter(data);
	/// let response = StreamingResponse::with_status(stream, StatusCode::PARTIAL_CONTENT);
	///
	/// assert_eq!(response.status, StatusCode::PARTIAL_CONTENT);
	/// ```
	pub fn with_status(stream: S, status: StatusCode) -> Self {
		Self {
			status,
			headers: HeaderMap::new(),
			stream,
		}
	}
	/// Set the status code
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_http::StreamingResponse;
	/// use hyper::StatusCode;
	/// use futures::stream;
	/// use bytes::Bytes;
	///
	/// let data = vec![Ok(Bytes::from("data"))];
	/// let stream = stream::iter(data);
	/// let response = StreamingResponse::new(stream).status(StatusCode::ACCEPTED);
	///
	/// assert_eq!(response.status, StatusCode::ACCEPTED);
	/// ```
	pub fn status(mut self, status: StatusCode) -> Self {
		self.status = status;
		self
	}
	/// Add a header to the streaming response
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_http::StreamingResponse;
	/// use hyper::header::{HeaderName, HeaderValue, CACHE_CONTROL};
	/// use futures::stream;
	/// use bytes::Bytes;
	///
	/// let data = vec![Ok(Bytes::from("data"))];
	/// let stream = stream::iter(data);
	/// let response = StreamingResponse::new(stream)
	///     .header(CACHE_CONTROL, HeaderValue::from_static("no-cache"));
	///
	/// assert_eq!(
	///     response.headers.get(CACHE_CONTROL).unwrap().to_str().unwrap(),
	///     "no-cache"
	/// );
	/// ```
	pub fn header(
		mut self,
		key: hyper::header::HeaderName,
		value: hyper::header::HeaderValue,
	) -> Self {
		self.headers.insert(key, value);
		self
	}
	/// Set the Content-Type header (media type)
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_http::StreamingResponse;
	/// use hyper::header::CONTENT_TYPE;
	/// use futures::stream;
	/// use bytes::Bytes;
	///
	/// let data = vec![Ok(Bytes::from("data"))];
	/// let stream = stream::iter(data);
	/// let response = StreamingResponse::new(stream).media_type("video/mp4");
	///
	/// assert_eq!(
	///     response.headers.get(CONTENT_TYPE).unwrap().to_str().unwrap(),
	///     "video/mp4"
	/// );
	/// ```
	pub fn media_type(self, media_type: &str) -> Self {
		self.header(
			hyper::header::CONTENT_TYPE,
			hyper::header::HeaderValue::from_str(media_type).unwrap_or_else(|_| {
				hyper::header::HeaderValue::from_static("application/octet-stream")
			}),
		)
	}
}

impl<S> StreamingResponse<S> {
	/// Consume the response and return the underlying stream
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_http::StreamingResponse;
	/// use futures::stream::{self, StreamExt};
	/// use bytes::Bytes;
	///
	/// # futures::executor::block_on(async {
	/// let data = vec![Ok(Bytes::from("chunk1")), Ok(Bytes::from("chunk2"))];
	/// let stream = stream::iter(data);
	/// let response = StreamingResponse::new(stream);
	///
	/// let mut extracted_stream = response.into_stream();
	/// let first_chunk = extracted_stream.next().await.unwrap().unwrap();
	/// assert_eq!(first_chunk, Bytes::from("chunk1"));
	/// # });
	/// ```
	pub fn into_stream(self) -> S {
		self.stream
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	#[rstest]
	#[case(StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error")]
	#[case(StatusCode::BAD_GATEWAY, "Bad Gateway")]
	#[case(StatusCode::SERVICE_UNAVAILABLE, "Service Unavailable")]
	#[case(StatusCode::GATEWAY_TIMEOUT, "Gateway Timeout")]
	fn test_5xx_errors_never_include_internal_details(
		#[case] status: StatusCode,
		#[case] expected_message: &str,
	) {
		// Arrange
		let sensitive_detail = "Internal path /src/db/connection.rs:42 failed";

		// Act
		let response = SafeErrorResponse::new(status)
			.with_detail(sensitive_detail)
			.build();

		// Assert
		let body: serde_json::Value = serde_json::from_slice(&response.body).unwrap();
		assert_eq!(body["error"], expected_message);
		// Detail must NOT be included for 5xx errors
		assert!(body.get("detail").is_none());
		assert_eq!(response.status, status);
	}

	#[rstest]
	#[case(StatusCode::BAD_REQUEST, "Bad Request")]
	#[case(StatusCode::UNAUTHORIZED, "Unauthorized")]
	#[case(StatusCode::FORBIDDEN, "Forbidden")]
	#[case(StatusCode::NOT_FOUND, "Not Found")]
	#[case(StatusCode::METHOD_NOT_ALLOWED, "Method Not Allowed")]
	#[case(StatusCode::CONFLICT, "Conflict")]
	fn test_4xx_errors_include_safe_detail(
		#[case] status: StatusCode,
		#[case] expected_message: &str,
	) {
		// Arrange
		let detail = "Missing required field: name";

		// Act
		let response = SafeErrorResponse::new(status).with_detail(detail).build();

		// Assert
		let body: serde_json::Value = serde_json::from_slice(&response.body).unwrap();
		assert_eq!(body["error"], expected_message);
		assert_eq!(body["detail"], detail);
		assert_eq!(response.status, status);
	}

	#[rstest]
	fn test_debug_mode_includes_full_error_info() {
		// Arrange
		let debug_info = "Error at src/handlers/user.rs:42: column 'email' not found";

		// Act
		let response = SafeErrorResponse::new(StatusCode::INTERNAL_SERVER_ERROR)
			.with_detail("Database query failed")
			.with_debug_info(debug_info)
			.with_debug_mode(true)
			.build();

		// Assert
		let body: serde_json::Value = serde_json::from_slice(&response.body).unwrap();
		assert_eq!(body["error"], "Internal Server Error");
		// In debug mode, detail is included even for 5xx
		assert_eq!(body["detail"], "Database query failed");
		assert_eq!(body["debug"], debug_info);
	}

	#[rstest]
	fn test_debug_mode_disabled_excludes_debug_info() {
		// Arrange
		let debug_info = "Sensitive internal detail";

		// Act
		let response = SafeErrorResponse::new(StatusCode::INTERNAL_SERVER_ERROR)
			.with_debug_info(debug_info)
			.with_debug_mode(false)
			.build();

		// Assert
		let body: serde_json::Value = serde_json::from_slice(&response.body).unwrap();
		assert!(body.get("debug").is_none());
	}

	#[rstest]
	#[case(StatusCode::BAD_REQUEST, "Bad Request")]
	#[case(StatusCode::UNAUTHORIZED, "Unauthorized")]
	#[case(StatusCode::FORBIDDEN, "Forbidden")]
	#[case(StatusCode::NOT_FOUND, "Not Found")]
	#[case(StatusCode::METHOD_NOT_ALLOWED, "Method Not Allowed")]
	#[case(StatusCode::NOT_ACCEPTABLE, "Not Acceptable")]
	#[case(StatusCode::REQUEST_TIMEOUT, "Request Timeout")]
	#[case(StatusCode::CONFLICT, "Conflict")]
	#[case(StatusCode::GONE, "Gone")]
	#[case(StatusCode::PAYLOAD_TOO_LARGE, "Payload Too Large")]
	#[case(StatusCode::UNSUPPORTED_MEDIA_TYPE, "Unsupported Media Type")]
	#[case(StatusCode::UNPROCESSABLE_ENTITY, "Unprocessable Entity")]
	#[case(StatusCode::TOO_MANY_REQUESTS, "Too Many Requests")]
	#[case(StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error")]
	#[case(StatusCode::BAD_GATEWAY, "Bad Gateway")]
	#[case(StatusCode::SERVICE_UNAVAILABLE, "Service Unavailable")]
	#[case(StatusCode::GATEWAY_TIMEOUT, "Gateway Timeout")]
	fn test_safe_error_message_returns_correct_messages(
		#[case] status: StatusCode,
		#[case] expected: &str,
	) {
		// Arrange / Act
		let message = safe_error_message(status);

		// Assert
		assert_eq!(message, expected);
	}

	#[rstest]
	fn test_safe_error_message_fallback_client_error() {
		// Arrange
		// 418 I'm a Teapot (not explicitly mapped)
		let status = StatusCode::IM_A_TEAPOT;

		// Act
		let message = safe_error_message(status);

		// Assert
		assert_eq!(message, "Client Error");
	}

	#[rstest]
	fn test_safe_error_message_fallback_server_error() {
		// Arrange
		// 505 HTTP Version Not Supported (not explicitly mapped)
		let status = StatusCode::HTTP_VERSION_NOT_SUPPORTED;

		// Act
		let message = safe_error_message(status);

		// Assert
		assert_eq!(message, "Server Error");
	}

	#[rstest]
	fn test_truncate_for_log_short_string() {
		// Arrange
		let input = "hello";

		// Act
		let result = truncate_for_log(input, 10);

		// Assert
		assert_eq!(result, "hello");
	}

	#[rstest]
	fn test_truncate_for_log_long_string() {
		// Arrange
		let input = "a".repeat(100);

		// Act
		let result = truncate_for_log(&input, 10);

		// Assert
		assert!(result.starts_with("aaaaaaaaaa"));
		assert!(result.contains("...[truncated, 100 total bytes]"));
	}

	#[rstest]
	fn test_truncate_for_log_exact_length() {
		// Arrange
		let input = "abcde";

		// Act
		let result = truncate_for_log(input, 5);

		// Assert
		assert_eq!(result, "abcde");
	}

	#[rstest]
	fn test_truncate_for_log_multi_byte_utf8_does_not_panic() {
		// Arrange
		// Each CJK character is 3 bytes in UTF-8
		let input = "日本語テスト文字列";

		// Act - max_length falls in the middle of a multi-byte char
		let result = truncate_for_log(input, 4);

		// Assert - should truncate at valid char boundary (3 bytes = 1 char)
		assert!(result.starts_with("日"));
		assert!(result.contains("...[truncated"));
	}

	#[rstest]
	fn test_truncate_for_log_emoji_boundary() {
		// Arrange
		// Each emoji is 4 bytes in UTF-8
		let input = "🦀🐍🐹🐿️";

		// Act - max_length 5 falls between first emoji (4 bytes) and second
		let result = truncate_for_log(input, 5);

		// Assert - should include only the first emoji (4 bytes)
		assert!(result.starts_with("🦀"));
		assert!(result.contains("...[truncated"));
	}

	#[rstest]
	fn test_truncate_for_log_mixed_ascii_and_multibyte() {
		// Arrange
		let input = "abc日本語def";

		// Act - max_length 5 falls inside second CJK char
		let result = truncate_for_log(input, 5);

		// Assert - "abc" (3 bytes) + "日" (3 bytes) = 6 bytes > 5, so only "abc"
		assert!(result.starts_with("abc"));
		assert!(result.contains("...[truncated"));
	}

	#[rstest]
	fn test_truncate_for_log_zero_max_length() {
		// Arrange
		let input = "hello";

		// Act
		let result = truncate_for_log(input, 0);

		// Assert - should produce empty prefix with truncation notice
		assert!(result.starts_with("...[truncated"));
	}

	#[rstest]
	fn test_from_error_produces_safe_output_for_5xx() {
		// Arrange
		let error = crate::Error::Database(
			"Connection to postgres://user:pass@db:5432/mydb failed".to_string(),
		);

		// Act
		let response: Response = error.into();

		// Assert
		assert_eq!(response.status, StatusCode::INTERNAL_SERVER_ERROR);
		let body: serde_json::Value = serde_json::from_slice(&response.body).unwrap();
		assert_eq!(body["error"], "Internal Server Error");
		// Must NOT contain internal connection details
		let body_str = String::from_utf8_lossy(&response.body);
		assert!(!body_str.contains("postgres://"));
		assert!(!body_str.contains("user:pass"));
		assert!(body.get("detail").is_none());
	}

	#[rstest]
	fn test_from_error_produces_safe_output_for_4xx_validation() {
		// Arrange
		let error = crate::Error::Validation("Email format is invalid".to_string());

		// Act
		let response: Response = error.into();

		// Assert
		assert_eq!(response.status, StatusCode::BAD_REQUEST);
		let body: serde_json::Value = serde_json::from_slice(&response.body).unwrap();
		assert_eq!(body["error"], "Bad Request");
		assert_eq!(body["detail"], "Email format is invalid");
	}

	#[rstest]
	fn test_from_error_produces_safe_output_for_4xx_parse() {
		// Arrange
		let error = crate::Error::ParseError(
			"invalid digit found in string at src/parser.rs:42".to_string(),
		);

		// Act
		let response: Response = error.into();

		// Assert
		assert_eq!(response.status, StatusCode::BAD_REQUEST);
		let body: serde_json::Value = serde_json::from_slice(&response.body).unwrap();
		assert_eq!(body["error"], "Bad Request");
		// Must NOT expose the internal path from the original error
		assert_eq!(body["detail"], "Invalid request format");
		let body_str = String::from_utf8_lossy(&response.body);
		assert!(!body_str.contains("src/parser.rs"));
	}

	#[rstest]
	fn test_from_error_body_already_consumed() {
		// Arrange
		let error = crate::Error::BodyAlreadyConsumed;

		// Act
		let response: Response = error.into();

		// Assert
		assert_eq!(response.status, StatusCode::BAD_REQUEST);
		let body: serde_json::Value = serde_json::from_slice(&response.body).unwrap();
		assert_eq!(body["detail"], "Request body has already been consumed");
	}

	#[rstest]
	fn test_from_error_internal_error_hides_details() {
		// Arrange
		let error =
			crate::Error::Internal("panic at /Users/dev/projects/app/src/main.rs:10".to_string());

		// Act
		let response: Response = error.into();

		// Assert
		assert_eq!(response.status, StatusCode::INTERNAL_SERVER_ERROR);
		let body_str = String::from_utf8_lossy(&response.body);
		assert!(!body_str.contains("/Users/dev"));
		assert!(!body_str.contains("main.rs"));
	}

	#[rstest]
	fn test_safe_error_response_no_detail_set() {
		// Arrange / Act
		let response = SafeErrorResponse::new(StatusCode::BAD_REQUEST).build();

		// Assert
		let body: serde_json::Value = serde_json::from_slice(&response.body).unwrap();
		assert_eq!(body["error"], "Bad Request");
		assert!(body.get("detail").is_none());
	}

	#[rstest]
	fn test_safe_error_response_content_type_is_json() {
		// Arrange / Act
		let response = SafeErrorResponse::new(StatusCode::NOT_FOUND).build();

		// Assert
		let content_type = response
			.headers
			.get("content-type")
			.unwrap()
			.to_str()
			.unwrap();
		assert_eq!(content_type, "application/json");
	}

	// =================================================================
	// with_header panic prevention tests (Issue #357)
	// =================================================================

	#[rstest]
	fn test_with_header_invalid_name_does_not_panic() {
		// Arrange
		let response = Response::ok();

		// Act - invalid header name with space (previously panicked)
		let response = response.with_header("Invalid Header", "value");

		// Assert - header is silently ignored, no panic
		assert!(response.headers.is_empty());
	}

	#[rstest]
	fn test_with_header_invalid_value_does_not_panic() {
		// Arrange
		let response = Response::ok();

		// Act - header value with non-visible ASCII (previously panicked)
		let response = response.with_header("X-Test", "value\x00with\x01control");

		// Assert - header is silently ignored, no panic
		assert!(response.headers.get("X-Test").is_none());
	}

	#[rstest]
	fn test_with_header_valid_header_works() {
		// Arrange
		let response = Response::ok();

		// Act
		let response = response.with_header("X-Custom", "custom-value");

		// Assert
		assert_eq!(
			response.headers.get("X-Custom").unwrap().to_str().unwrap(),
			"custom-value"
		);
	}

	#[rstest]
	fn test_try_with_header_invalid_name_returns_error() {
		// Arrange
		let response = Response::ok();

		// Act
		let result = response.try_with_header("Invalid Header", "value");

		// Assert
		assert!(result.is_err());
	}

	#[rstest]
	fn test_try_with_header_valid_header_returns_ok() {
		// Arrange
		let response = Response::ok();

		// Act
		let result = response.try_with_header("X-Custom", "valid-value");

		// Assert
		assert!(result.is_ok());
		let response = result.unwrap();
		assert_eq!(
			response.headers.get("X-Custom").unwrap().to_str().unwrap(),
			"valid-value"
		);
	}
}
