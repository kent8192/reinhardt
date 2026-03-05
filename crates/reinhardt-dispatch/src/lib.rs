//! # Reinhardt Dispatch
//!
//! HTTP request dispatching and handler system for Reinhardt framework.
//!
//! This module provides the core request handling functionality,
//! equivalent to Django's `django.core.handlers` and `django.dispatch`.
//!
//! ## Overview
//!
//! The dispatch system handles:
//! - HTTP request handling and routing
//! - Middleware chain execution
//! - View dispatching
//! - Exception handling
//! - Signal emission for request lifecycle events
//!
//! ## Architecture
//!
//! ```text
//! Request → BaseHandler → Middleware Chain → URL Resolver → View → Response
//!                ↓                                            ↓
//!           Signals                                      Signals
//!       (request_started)                          (request_finished)
//! ```
//!
//! ## Examples
//!
//! ### Basic Request Handling with URL Routing
//!
//! ```rust
//! use reinhardt_dispatch::BaseHandler;
//! use reinhardt_urls::routers::{DefaultRouter, Router, path};
//! use reinhardt_http::{Request, Response};
//! use reinhardt_http::Handler;
//! use std::sync::Arc;
//! use hyper::{Method, Version, HeaderMap, StatusCode};
//! use bytes::Bytes;
//! use async_trait::async_trait;
//!
//! // Define a simple handler
//! struct HelloHandler;
//!
//! #[async_trait]
//! impl Handler for HelloHandler {
//!     async fn handle(&self, _req: Request) -> reinhardt_core::exception::Result<Response> {
//!         Ok(Response::ok().with_body("Hello, World!"))
//!     }
//! }
//!
//! # tokio_test::block_on(async {
//! // Create a router and register routes
//! let mut router = DefaultRouter::new();
//! let route = path("/", Arc::new(HelloHandler)).with_name("index");
//! router.add_route(route);
//!
//! // Create handler with router
//! let handler = BaseHandler::with_router(Arc::new(router));
//!
//! // Create a request
//! let request = Request::builder()
//!     .method(Method::GET)
//!     .uri("/")
//!     .version(Version::HTTP_11)
//!     .headers(HeaderMap::new())
//!     .body(Bytes::new())
//!     .build()
//!     .unwrap();
//!
//! // Handle request
//! let response = handler.handle_request(request).await.unwrap();
//! assert_eq!(response.status, StatusCode::OK);
//! # });
//! ```
//!
//! ### With Middleware
//!
//! ```rust
//! use reinhardt_dispatch::{BaseHandler, MiddlewareChain};
//! use reinhardt_urls::routers::{DefaultRouter, Router, path};
//! use reinhardt_http::{Handler, Middleware};
//! use reinhardt_http::{Request, Response};
//! use std::sync::Arc;
//! use async_trait::async_trait;
//!
//! // Example middleware
//! struct LoggingMiddleware;
//!
//! #[async_trait]
//! impl Middleware for LoggingMiddleware {
//!     async fn process(&self, request: Request, next: Arc<dyn Handler>) -> reinhardt_core::exception::Result<Response> {
//!         println!("Request: {} {}", request.method, request.path());
//!         let response = next.handle(request).await?;
//!         println!("Response: {}", response.status);
//!         Ok(response)
//!     }
//! }
//!
//! // Example handler
//! struct ApiHandler;
//!
//! #[async_trait]
//! impl Handler for ApiHandler {
//!     async fn handle(&self, _req: Request) -> reinhardt_core::exception::Result<Response> {
//!         Ok(Response::ok().with_json(&serde_json::json!({"status": "ok"})).unwrap())
//!     }
//! }
//!
//! # tokio_test::block_on(async {
//! // Setup router
//! let mut router = DefaultRouter::new();
//! let api_route = path("/api", Arc::new(ApiHandler)).with_name("api");
//! router.add_route(api_route);
//!
//! // Create base handler with router
//! let base_handler: Arc<dyn Handler> = Arc::new(BaseHandler::with_router(Arc::new(router)));
//!
//! // Wrap with middleware
//! let handler = MiddlewareChain::new(base_handler)
//!     .add_middleware(Arc::new(LoggingMiddleware))
//!     .expect("Failed to add middleware")
//!     .build();
//!
//! // Use the handler
//! let request = Request::builder()
//!     .method(hyper::Method::GET)
//!     .uri("/api")
//!     .version(hyper::Version::HTTP_11)
//!     .headers(hyper::HeaderMap::new())
//!     .body(bytes::Bytes::new())
//!     .build()
//!     .unwrap();
//!
//! let response = handler.handle(request).await.unwrap();
//! assert_eq!(response.status, hyper::StatusCode::OK);
//! # });
//! ```

pub mod dispatcher;
pub mod exception;
pub mod handler;
pub mod middleware;

// Re-exports
pub use dispatcher::Dispatcher;
pub use exception::{ExceptionHandler, convert_exception_to_response};
pub use handler::BaseHandler;
pub use middleware::MiddlewareChain;

use thiserror::Error;

/// Errors that can occur during request dispatching
#[derive(Debug, Error)]
pub enum DispatchError {
	/// Middleware configuration error
	#[error("Middleware error: {0}")]
	Middleware(String),

	/// View execution error
	#[error("View error: {0}")]
	View(String),

	/// URL resolution error
	#[error("URL resolution error: {0}")]
	UrlResolution(String),

	/// HTTP error
	#[error("HTTP error: {0}")]
	Http(String),

	/// Internal error
	#[error("Internal error: {0}")]
	Internal(String),
}

/// Build a plain-text error response with security headers.
///
/// Sets `Content-Type: text/plain; charset=utf-8` and
/// `X-Content-Type-Options: nosniff` to prevent browsers from MIME-sniffing
/// the error body into an executable content type.
pub(crate) fn build_error_response(
	status: hyper::StatusCode,
	message: &str,
) -> reinhardt_http::Response {
	let mut response = reinhardt_http::Response::new(status);
	response.body = bytes::Bytes::from(message.to_owned());
	response.headers.insert(
		hyper::header::CONTENT_TYPE,
		hyper::header::HeaderValue::from_static("text/plain; charset=utf-8"),
	);
	response.headers.insert(
		hyper::header::HeaderName::from_static("x-content-type-options"),
		hyper::header::HeaderValue::from_static("nosniff"),
	);
	response
}
