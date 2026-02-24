//! Exception handling and conversion for HTTP requests
//!
//! This module provides functionality to convert exceptions into HTTP responses,
//! similar to Django's `django.core.handlers.exception`.

use async_trait::async_trait;
use bytes::Bytes;
use hyper::StatusCode;
use reinhardt_http::{Request, Response};
use std::fmt;
use std::future::Future;
use tracing::{error, warn};

use crate::DispatchError;
use crate::build_error_response;

/// Result type for exception handlers
pub type ExceptionResult = Result<Response, DispatchError>;

/// A trait for handling exceptions during request processing
#[async_trait]
pub trait ExceptionHandler: Send + Sync {
	/// Handle an exception and convert it to a response
	async fn handle_exception(&self, request: &Request, error: DispatchError) -> Response;
}

/// Default exception handler implementation
///
/// Converts exceptions to appropriate HTTP error responses.
pub struct DefaultExceptionHandler;

#[async_trait]
impl ExceptionHandler for DefaultExceptionHandler {
	async fn handle_exception(&self, _request: &Request, error: DispatchError) -> Response {
		// Internal error details are logged server-side but never exposed
		// in HTTP response bodies to prevent information disclosure.
		let (status, client_message) = match &error {
			DispatchError::View(msg) => {
				warn!("View error: {}", msg);
				(StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error")
			}
			DispatchError::UrlResolution(msg) => {
				warn!("URL resolution error: {}", msg);
				(StatusCode::NOT_FOUND, "Not Found")
			}
			DispatchError::Middleware(msg) => {
				error!("Middleware error: {}", msg);
				(StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error")
			}
			DispatchError::Http(msg) => {
				warn!("HTTP error: {}", msg);
				(StatusCode::BAD_REQUEST, "Bad Request")
			}
			DispatchError::Internal(msg) => {
				error!("Internal error: {}", msg);
				(StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error")
			}
		};

		build_error_response(status, client_message)
	}
}

/// Convert an exception to an HTTP response
///
/// This function wraps a handler that returns `Result<Response, DispatchError>`
/// and converts any errors into proper HTTP responses using the default exception handler.
///
/// The request's method, URI, version, and headers are preserved before
/// passing ownership to the handler, so that the exception handler retains
/// the original request context (headers, auth info) for context-aware error
/// responses.
pub async fn convert_exception_to_response<F, Fut>(handler: F, request: Request) -> Response
where
	F: FnOnce(Request) -> Fut,
	Fut: Future<Output = Result<Response, DispatchError>>,
{
	// Capture the request context before consuming the request,
	// so the exception handler has access to headers and auth info.
	let method = request.method.clone();
	let uri = request.uri.clone();
	let version = request.version;
	let headers = request.headers.clone();

	match handler(request).await {
		Ok(response) => response,
		Err(error) => {
			let exception_handler = DefaultExceptionHandler;
			// Reconstruct a request with the original context for error handling
			match Request::builder()
				.method(method)
				.uri(uri.to_string())
				.version(version)
				.headers(headers)
				.body(Bytes::new())
				.build()
			{
				Ok(context_request) => {
					exception_handler
						.handle_exception(&context_request, error)
						.await
				}
				Err(_) => {
					let mut response = Response::new(hyper::StatusCode::INTERNAL_SERVER_ERROR);
					response.body = Bytes::from("Internal Server Error");
					response
				}
			}
		}
	}
}

/// Trait for types that can be converted into HTTP responses
pub trait IntoResponse {
	/// Convert self into an HTTP response
	fn into_response(self) -> Response;
}

impl IntoResponse for Response {
	fn into_response(self) -> Response {
		self
	}
}

impl IntoResponse for String {
	fn into_response(self) -> Response {
		let mut response = Response::new(StatusCode::OK);
		response.body = Bytes::from(self.into_bytes());
		response
	}
}

impl IntoResponse for &str {
	fn into_response(self) -> Response {
		let mut response = Response::new(StatusCode::OK);
		response.body = Bytes::from(self.as_bytes().to_vec());
		response
	}
}

impl IntoResponse for Vec<u8> {
	fn into_response(self) -> Response {
		let mut response = Response::new(StatusCode::OK);
		response.body = Bytes::from(self);
		response
	}
}

impl IntoResponse for StatusCode {
	fn into_response(self) -> Response {
		Response::new(self)
	}
}

impl<T: IntoResponse, E: fmt::Display> IntoResponse for Result<T, E> {
	fn into_response(self) -> Response {
		match self {
			Ok(value) => value.into_response(),
			Err(error) => {
				// Log the error details server-side only; never expose in response body
				error!("Error converting to response: {}", error);
				build_error_response(StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error")
			}
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	fn build_request() -> Request {
		Request::builder()
			.method(hyper::Method::GET)
			.uri("/")
			.version(hyper::Version::HTTP_11)
			.headers(hyper::HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap()
	}

	// ==========================================================================
	// Information Disclosure Prevention Tests (#439)
	// ==========================================================================

	#[tokio::test]
	async fn test_internal_error_does_not_expose_details() {
		// Arrange
		let handler = DefaultExceptionHandler;
		let request = build_request();
		let error =
			DispatchError::Internal("database pool exhausted at /src/db/pool.rs:99".to_string());

		// Act
		let response = handler.handle_exception(&request, error).await;

		// Assert: generic message only, no internal details
		let body = String::from_utf8(response.body.to_vec()).unwrap();
		assert_eq!(response.status, StatusCode::INTERNAL_SERVER_ERROR);
		assert_eq!(body, "Internal Server Error");
		assert!(!body.contains("database"));
		assert!(!body.contains(".rs:"));
	}

	#[tokio::test]
	async fn test_middleware_error_does_not_expose_details() {
		// Arrange
		let handler = DefaultExceptionHandler;
		let request = build_request();
		let error = DispatchError::Middleware(
			"JWT decode failed: invalid signature for key abc123".to_string(),
		);

		// Act
		let response = handler.handle_exception(&request, error).await;

		// Assert: generic message only, no internal details
		let body = String::from_utf8(response.body.to_vec()).unwrap();
		assert_eq!(response.status, StatusCode::INTERNAL_SERVER_ERROR);
		assert_eq!(body, "Internal Server Error");
		assert!(!body.contains("JWT"));
		assert!(!body.contains("abc123"));
	}

	#[tokio::test]
	async fn test_view_error_does_not_expose_details() {
		// Arrange
		let handler = DefaultExceptionHandler;
		let request = build_request();
		let error = DispatchError::View(
			"template rendering panicked at /src/views/admin.rs:42".to_string(),
		);

		// Act
		let response = handler.handle_exception(&request, error).await;

		// Assert: generic message only, no internal details
		let body = String::from_utf8(response.body.to_vec()).unwrap();
		assert_eq!(response.status, StatusCode::INTERNAL_SERVER_ERROR);
		assert_eq!(body, "Internal Server Error");
		assert!(!body.contains("panicked"));
		assert!(!body.contains(".rs:"));
	}

	#[tokio::test]
	async fn test_url_resolution_returns_not_found() {
		// Arrange
		let handler = DefaultExceptionHandler;
		let request = build_request();
		let error = DispatchError::UrlResolution("no route matched".to_string());

		// Act
		let response = handler.handle_exception(&request, error).await;

		// Assert
		let body = String::from_utf8(response.body.to_vec()).unwrap();
		assert_eq!(response.status, StatusCode::NOT_FOUND);
		assert_eq!(body, "Not Found");
	}

	#[tokio::test]
	async fn test_http_error_returns_bad_request() {
		// Arrange
		let handler = DefaultExceptionHandler;
		let request = build_request();
		let error = DispatchError::Http("malformed header".to_string());

		// Act
		let response = handler.handle_exception(&request, error).await;

		// Assert
		let body = String::from_utf8(response.body.to_vec()).unwrap();
		assert_eq!(response.status, StatusCode::BAD_REQUEST);
		assert_eq!(body, "Bad Request");
	}

	#[test]
	fn test_into_response_for_result_err_does_not_expose_error() {
		// Arrange
		let result: Result<String, String> =
			Err("connection string: postgres://admin:pass@host/db".to_string());

		// Act
		let response = result.into_response();

		// Assert
		let body = String::from_utf8(response.body.to_vec()).unwrap();
		assert_eq!(response.status, StatusCode::INTERNAL_SERVER_ERROR);
		assert!(!body.contains("postgres"));
		assert!(!body.contains("admin"));
		assert_eq!(body, "Internal Server Error");
	}
}
