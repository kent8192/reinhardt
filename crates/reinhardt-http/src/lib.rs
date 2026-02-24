//! HTTP request and response handling for Reinhardt framework.
//!
//! This crate provides core HTTP abstractions including request and response types,
//! header handling, and content negotiation.
//!
//! ## Request Construction
//!
//! Requests are constructed using the builder pattern for type-safe configuration:
//!
//! ```rust
//! use reinhardt_http::Request;
//! use hyper::{Method, HeaderMap};
//!
//! let request = Request::builder()
//!     .method(Method::POST)
//!     .uri("/api/users")
//!     .version(hyper::Version::HTTP_11)
//!     .headers(HeaderMap::new())
//!     .body(bytes::Bytes::from("request body"))
//!     .build()
//!     .unwrap();
//! ```
//!
//! ## Response Creation
//!
//! Responses use helper methods and builder pattern:
//!
//! ```rust
//! use reinhardt_http::Response;
//!
//! // Using helpers
//! let response = Response::ok().with_body("Hello, World!");
//!
//! // With JSON
//! let json_response = Response::ok()
//!     .with_json(&serde_json::json!({"status": "success"}))
//!     .unwrap();
//! ```

pub mod auth_state;
pub mod chunked_upload;
pub mod extensions;
#[cfg(feature = "messages")]
pub mod messages_middleware;
pub mod middleware;
pub mod request;
pub mod response;
pub mod upload;

pub use auth_state::AuthState;
pub use chunked_upload::{
	ChunkedUploadError, ChunkedUploadManager, ChunkedUploadSession, UploadProgress,
};
pub use extensions::Extensions;
#[cfg(feature = "messages")]
pub use messages_middleware::MessagesMiddleware;
pub use middleware::{Handler, Middleware, MiddlewareChain};
pub use request::{Request, RequestBuilder, TrustedProxies};
pub use response::{Response, SafeErrorResponse, StreamBody, StreamingResponse};
pub use upload::{FileUploadError, FileUploadHandler, MemoryFileUpload, TemporaryFileUpload};

// Re-export error types from reinhardt-exception for consistency across the framework
pub use reinhardt_core::exception::{Error, Result};

/// A convenient type alias for view/endpoint function return types.
///
/// This type alias is commonly used in endpoint handlers to simplify function signatures.
/// It wraps any type `T` (typically `Response`) with a dynamic error type that can
/// represent various kinds of errors that might occur during request processing.
///
/// The `Send + Sync` bounds ensure this type is safe to use across thread boundaries,
/// which is essential for async runtime environments.
///
/// # Examples
///
/// ```
/// use reinhardt_http::{Response, ViewResult};
///
/// async fn hello_world() -> ViewResult<Response> {
///     Ok(Response::ok().with_body("Hello, World!"))
/// }
///
/// #[tokio::main]
/// # async fn main() {
/// let response = hello_world().await.unwrap();
/// assert_eq!(response.status, hyper::StatusCode::OK);
/// # }
/// ```
/// Result type for view handlers using reinhardt's unified Error type.
///
/// This type alias ensures compatibility with `UnifiedRouter::function` which requires
/// `Future<Output = Result<Response>>` where `Result` is `reinhardt_core::exception::Result`.
pub type ViewResult<T> = reinhardt_core::exception::Result<T>;
