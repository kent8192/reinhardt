//! X-Frame-Options Middleware
//!
//! Provides clickjacking protection by setting the X-Frame-Options header.

use async_trait::async_trait;
use hyper::header::HeaderName;
use reinhardt_http::{Handler, Middleware, Request, Response, Result};
use std::sync::Arc;

/// X-Frame-Options values
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum XFrameOptions {
	/// DENY - The page cannot be displayed in a frame
	Deny,
	/// SAMEORIGIN - The page can only be displayed in a frame on the same origin
	SameOrigin,
}

impl XFrameOptions {
	/// Convert to header value string
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_middleware::XFrameOptions;
	///
	/// let deny = XFrameOptions::Deny;
	/// assert_eq!(deny.as_str(), "DENY");
	///
	/// let same_origin = XFrameOptions::SameOrigin;
	/// assert_eq!(same_origin.as_str(), "SAMEORIGIN");
	/// ```
	pub fn as_str(&self) -> &'static str {
		match self {
			XFrameOptions::Deny => "DENY",
			XFrameOptions::SameOrigin => "SAMEORIGIN",
		}
	}
}

/// X-Frame-Options middleware for clickjacking protection
pub struct XFrameOptionsMiddleware {
	option: XFrameOptions,
}

impl XFrameOptionsMiddleware {
	/// Create middleware with DENY option
	///
	/// Prevents the page from being displayed in any frame, providing maximum clickjacking protection.
	///
	/// # Examples
	///
	/// ```
	/// use std::sync::Arc;
	/// use reinhardt_middleware::XFrameOptionsMiddleware;
	/// use reinhardt_http::{Handler, Middleware, Request, Response};
	/// use hyper::{StatusCode, Method, Version, HeaderMap};
	/// use bytes::Bytes;
	///
	/// struct TestHandler;
	///
	/// #[async_trait::async_trait]
	/// impl Handler for TestHandler {
	///     async fn handle(&self, _request: Request) -> reinhardt_core::exception::Result<Response> {
	///         Ok(Response::new(StatusCode::OK))
	///     }
	/// }
	///
	/// # tokio_test::block_on(async {
	/// let middleware = XFrameOptionsMiddleware::deny();
	/// let handler = Arc::new(TestHandler);
	///
	/// let request = Request::builder()
	///     .method(Method::GET)
	///     .uri("/secure-page")
	///     .version(Version::HTTP_11)
	///     .headers(HeaderMap::new())
	///     .body(Bytes::new())
	///     .build()
	///     .unwrap();
	///
	/// let response = middleware.process(request, handler).await.unwrap();
	/// assert_eq!(response.headers.get("X-Frame-Options").unwrap(), "DENY");
	/// # });
	/// ```
	pub fn deny() -> Self {
		Self {
			option: XFrameOptions::Deny,
		}
	}
	/// Create middleware with SAMEORIGIN option
	///
	/// Allows the page to be framed only by pages from the same origin.
	///
	/// # Examples
	///
	/// ```
	/// use std::sync::Arc;
	/// use reinhardt_middleware::XFrameOptionsMiddleware;
	/// use reinhardt_http::{Handler, Middleware, Request, Response};
	/// use hyper::{StatusCode, Method, Version, HeaderMap};
	/// use bytes::Bytes;
	///
	/// struct TestHandler;
	///
	/// #[async_trait::async_trait]
	/// impl Handler for TestHandler {
	///     async fn handle(&self, _request: Request) -> reinhardt_core::exception::Result<Response> {
	///         Ok(Response::new(StatusCode::OK))
	///     }
	/// }
	///
	/// # tokio_test::block_on(async {
	/// let middleware = XFrameOptionsMiddleware::same_origin();
	/// let handler = Arc::new(TestHandler);
	///
	/// let request = Request::builder()
	///     .method(Method::GET)
	///     .uri("/dashboard")
	///     .version(Version::HTTP_11)
	///     .headers(HeaderMap::new())
	///     .body(Bytes::new())
	///     .build()
	///     .unwrap();
	///
	/// let response = middleware.process(request, handler).await.unwrap();
	/// assert_eq!(response.headers.get("X-Frame-Options").unwrap(), "SAMEORIGIN");
	/// # });
	/// ```
	pub fn same_origin() -> Self {
		Self {
			option: XFrameOptions::SameOrigin,
		}
	}
	/// Create middleware with custom option
	///
	/// # Arguments
	///
	/// * `option` - The X-Frame-Options value to use
	///
	/// # Examples
	///
	/// ```
	/// use std::sync::Arc;
	/// use reinhardt_middleware::{XFrameOptionsMiddleware, XFrameOptions};
	/// use reinhardt_http::{Handler, Middleware, Request, Response};
	/// use hyper::{StatusCode, Method, Version, HeaderMap};
	/// use bytes::Bytes;
	///
	/// struct TestHandler;
	///
	/// #[async_trait::async_trait]
	/// impl Handler for TestHandler {
	///     async fn handle(&self, _request: Request) -> reinhardt_core::exception::Result<Response> {
	///         Ok(Response::new(StatusCode::OK))
	///     }
	/// }
	///
	/// # tokio_test::block_on(async {
	/// let middleware = XFrameOptionsMiddleware::new(XFrameOptions::Deny);
	/// let handler = Arc::new(TestHandler);
	///
	/// let request = Request::builder()
	///     .method(Method::GET)
	///     .uri("/admin")
	///     .version(Version::HTTP_11)
	///     .headers(HeaderMap::new())
	///     .body(Bytes::new())
	///     .build()
	///     .unwrap();
	///
	/// let response = middleware.process(request, handler).await.unwrap();
	/// assert_eq!(response.headers.get("X-Frame-Options").unwrap(), "DENY");
	/// # });
	/// ```
	pub fn new(option: XFrameOptions) -> Self {
		Self { option }
	}
}

impl Default for XFrameOptionsMiddleware {
	fn default() -> Self {
		Self::same_origin()
	}
}

const X_FRAME_OPTIONS: HeaderName = HeaderName::from_static("x-frame-options");

#[async_trait]
impl Middleware for XFrameOptionsMiddleware {
	async fn process(&self, request: Request, handler: Arc<dyn Handler>) -> Result<Response> {
		// Convert errors to responses so post-processing (e.g., security headers)
		// always runs, even when invoked outside MiddlewareChain. (#3244)
		let mut response = match handler.handle(request).await {
			Ok(resp) => resp,
			Err(e) => Response::from(e),
		};

		// Only add header if not already present
		if !response.headers.contains_key(&X_FRAME_OPTIONS) {
			let header_value = match self.option {
				XFrameOptions::Deny => hyper::header::HeaderValue::from_static("DENY"),
				XFrameOptions::SameOrigin => hyper::header::HeaderValue::from_static("SAMEORIGIN"),
			};
			response.headers.insert(X_FRAME_OPTIONS, header_value);
		}

		Ok(response)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use bytes::Bytes;
	use hyper::{HeaderMap, Method, StatusCode, Version};
	use reinhardt_http::Error;
	use rstest::rstest;

	struct TestHandler;

	#[async_trait]
	impl Handler for TestHandler {
		async fn handle(&self, _request: Request) -> Result<Response> {
			Ok(Response::new(StatusCode::OK).with_body(Bytes::from(&b"test"[..])))
		}
	}

	#[tokio::test]
	async fn test_deny_option() {
		let middleware = XFrameOptionsMiddleware::deny();
		let handler = Arc::new(TestHandler);
		let request = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();

		assert_eq!(response.headers.get(&X_FRAME_OPTIONS).unwrap(), "DENY");
	}

	#[tokio::test]
	async fn test_same_origin_option() {
		let middleware = XFrameOptionsMiddleware::same_origin();
		let handler = Arc::new(TestHandler);
		let request = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();

		assert_eq!(
			response.headers.get(&X_FRAME_OPTIONS).unwrap(),
			"SAMEORIGIN"
		);
	}

	#[tokio::test]
	async fn test_default_is_same_origin() {
		let middleware = XFrameOptionsMiddleware::default();
		let handler = Arc::new(TestHandler);
		let request = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();

		assert_eq!(
			response.headers.get(&X_FRAME_OPTIONS).unwrap(),
			"SAMEORIGIN"
		);
	}

	#[tokio::test]
	async fn test_does_not_override_existing_header() {
		struct TestHandlerWithHeader;

		#[async_trait]
		impl Handler for TestHandlerWithHeader {
			async fn handle(&self, _request: Request) -> Result<Response> {
				let mut response =
					Response::new(StatusCode::OK).with_body(Bytes::from(&b"test"[..]));
				response
					.headers
					.insert(X_FRAME_OPTIONS, "DENY".parse().unwrap());
				Ok(response)
			}
		}

		let middleware = XFrameOptionsMiddleware::same_origin();
		let handler = Arc::new(TestHandlerWithHeader);
		let request = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();

		// Should keep the original DENY value
		assert_eq!(response.headers.get(&X_FRAME_OPTIONS).unwrap(), "DENY");
	}

	#[tokio::test]
	async fn test_new_constructor_with_deny() {
		let middleware = XFrameOptionsMiddleware::new(XFrameOptions::Deny);
		let handler = Arc::new(TestHandler);
		let request = Request::builder()
			.method(Method::GET)
			.uri("/secure")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();
		assert_eq!(response.headers.get(&X_FRAME_OPTIONS).unwrap(), "DENY");
	}

	#[tokio::test]
	async fn test_new_constructor_with_same_origin() {
		let middleware = XFrameOptionsMiddleware::new(XFrameOptions::SameOrigin);
		let handler = Arc::new(TestHandler);
		let request = Request::builder()
			.method(Method::GET)
			.uri("/dashboard")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();
		assert_eq!(
			response.headers.get(&X_FRAME_OPTIONS).unwrap(),
			"SAMEORIGIN"
		);
	}

	#[tokio::test]
	async fn test_response_body_preserved() {
		struct TestHandlerWithBody;

		#[async_trait]
		impl Handler for TestHandlerWithBody {
			async fn handle(&self, _request: Request) -> Result<Response> {
				Ok(Response::new(StatusCode::OK)
					.with_body(Bytes::from(&b"custom response body"[..])))
			}
		}

		let middleware = XFrameOptionsMiddleware::deny();
		let handler = Arc::new(TestHandlerWithBody);
		let request = Request::builder()
			.method(Method::GET)
			.uri("/content")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();

		// Header should be added
		assert_eq!(response.headers.get(&X_FRAME_OPTIONS).unwrap(), "DENY");
		// Body should be preserved
		assert_eq!(response.body, Bytes::from(&b"custom response body"[..]));
	}

	#[tokio::test]
	async fn test_middleware_reusable_across_requests() {
		let middleware = XFrameOptionsMiddleware::deny();
		let handler = Arc::new(TestHandler);

		// First request
		let request1 = Request::builder()
			.method(Method::GET)
			.uri("/page1")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();
		let response1 = middleware.process(request1, handler.clone()).await.unwrap();
		assert_eq!(response1.headers.get(&X_FRAME_OPTIONS).unwrap(), "DENY");

		// Second request
		let request2 = Request::builder()
			.method(Method::POST)
			.uri("/page2")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();
		let response2 = middleware.process(request2, handler.clone()).await.unwrap();
		assert_eq!(response2.headers.get(&X_FRAME_OPTIONS).unwrap(), "DENY");

		// Third request
		let request3 = Request::builder()
			.method(Method::PUT)
			.uri("/page3")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();
		let response3 = middleware.process(request3, handler).await.unwrap();
		assert_eq!(response3.headers.get(&X_FRAME_OPTIONS).unwrap(), "DENY");
	}

	/// Handler that always returns an error to simulate inner handler failure.
	struct ErrorHandler;

	#[async_trait]
	impl Handler for ErrorHandler {
		async fn handle(&self, _request: Request) -> Result<Response> {
			Err(Error::Http("handler error".to_string()))
		}
	}

	#[rstest]
	#[tokio::test]
	async fn test_xframe_header_applied_on_handler_error() {
		// Arrange
		let middleware = XFrameOptionsMiddleware::new(XFrameOptions::Deny);
		let handler: Arc<dyn Handler> = Arc::new(ErrorHandler);

		let request = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		// Act
		let response = middleware.process(request, handler).await.unwrap();

		// Assert — error is converted to response with X-Frame-Options applied
		assert!(response.status.is_client_error() || response.status.is_server_error());
		assert_eq!(response.headers.get(&X_FRAME_OPTIONS).unwrap(), "DENY");
	}
}
