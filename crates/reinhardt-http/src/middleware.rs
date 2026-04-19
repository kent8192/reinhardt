//! Middleware and handler traits for HTTP request processing.
//!
//! This module provides the core abstractions for handling HTTP requests
//! and composing middleware chains.
//!
//! ## Handler
//!
//! The `Handler` trait is the core abstraction for processing requests:
//!
//! ```rust
//! use reinhardt_http::{Handler, Request, Response};
//! use async_trait::async_trait;
//!
//! struct MyHandler;
//!
//! #[async_trait]
//! impl Handler for MyHandler {
//!     async fn handle(&self, request: Request) -> reinhardt_core::exception::Result<Response> {
//!         Ok(Response::ok().with_body("Hello!"))
//!     }
//! }
//! ```
//!
//! ## Middleware
//!
//! Middleware wraps handlers to add cross-cutting concerns:
//!
//! ```rust
//! use reinhardt_http::{Handler, Middleware, Request, Response};
//! use async_trait::async_trait;
//! use std::sync::Arc;
//!
//! struct LoggingMiddleware;
//!
//! #[async_trait]
//! impl Middleware for LoggingMiddleware {
//!     async fn process(&self, request: Request, next: Arc<dyn Handler>) -> reinhardt_core::exception::Result<Response> {
//!         println!("Request: {} {}", request.method, request.uri);
//!         next.handle(request).await
//!     }
//! }
//! ```

use async_trait::async_trait;
use reinhardt_core::exception::Result;
use std::sync::Arc;

use crate::{Request, Response};

/// Handler trait for processing requests.
///
/// This is the core abstraction - all request handlers implement this trait.
/// Handlers receive a request and produce a response or an error.
#[async_trait]
pub trait Handler: Send + Sync {
	/// Handles an HTTP request and produces a response.
	///
	/// # Errors
	///
	/// Returns an error if the request cannot be processed.
	async fn handle(&self, request: Request) -> Result<Response>;
}

/// Blanket implementation for `Arc<T>` where T: Handler.
///
/// This allows `Arc<dyn Handler>` to be used as a Handler,
/// enabling shared ownership of handlers across threads.
#[async_trait]
impl<T: Handler + ?Sized> Handler for Arc<T> {
	async fn handle(&self, request: Request) -> Result<Response> {
		(**self).handle(request).await
	}
}

/// Middleware trait for request/response processing.
///
/// Uses composition pattern instead of inheritance.
/// Middleware can modify requests before passing to the next handler,
/// or modify responses after the handler processes the request.
#[async_trait]
pub trait Middleware: Send + Sync {
	/// Processes a request through this middleware.
	///
	/// # Arguments
	///
	/// * `request` - The incoming HTTP request
	/// * `next` - The next handler in the chain to call
	///
	/// # Errors
	///
	/// Returns an error if the middleware or next handler fails.
	async fn process(&self, request: Request, next: Arc<dyn Handler>) -> Result<Response>;

	/// Determines whether this middleware should be executed for the given request.
	///
	/// This method enables conditional execution of middleware, allowing the middleware
	/// chain to skip unnecessary middleware based on request properties.
	///
	/// # Performance Benefits
	///
	/// By implementing this method, middleware chains can achieve O(k) complexity
	/// instead of O(n), where k is the number of middleware that should run,
	/// and k <= n (total middleware count).
	///
	/// # Common Use Cases
	///
	/// - Skip authentication middleware for public endpoints
	/// - Skip compression middleware for already compressed responses
	/// - Skip CORS middleware for same-origin requests
	/// - Skip rate limiting for internal/admin requests
	///
	/// # Default Implementation
	///
	/// By default, returns `true` (always execute), maintaining backward compatibility.
	fn should_continue(&self, _request: &Request) -> bool {
		true
	}
}

/// Middleware chain - composes multiple middleware into a single handler.
///
/// The chain processes requests through middleware in the order they were added,
/// with optimizations for conditional execution and early termination.
pub struct MiddlewareChain {
	middlewares: Vec<Arc<dyn Middleware>>,
	handler: Arc<dyn Handler>,
}

impl MiddlewareChain {
	/// Creates a new middleware chain with the given handler.
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_http::{MiddlewareChain, Handler, Request, Response};
	/// use std::sync::Arc;
	///
	/// struct MyHandler;
	///
	/// #[async_trait::async_trait]
	/// impl Handler for MyHandler {
	///     async fn handle(&self, _request: Request) -> reinhardt_core::exception::Result<Response> {
	///         Ok(Response::ok())
	///     }
	/// }
	///
	/// let handler = Arc::new(MyHandler);
	/// let chain = MiddlewareChain::new(handler);
	/// ```
	pub fn new(handler: Arc<dyn Handler>) -> Self {
		Self {
			middlewares: Vec::new(),
			handler,
		}
	}

	/// Adds a middleware to the chain using builder pattern.
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_http::{MiddlewareChain, Handler, Middleware, Request, Response};
	/// use std::sync::Arc;
	///
	/// # struct MyHandler;
	/// # struct MyMiddleware;
	/// # #[async_trait::async_trait]
	/// # impl Handler for MyHandler {
	/// #     async fn handle(&self, _request: Request) -> reinhardt_core::exception::Result<Response> {
	/// #         Ok(Response::ok())
	/// #     }
	/// # }
	/// # #[async_trait::async_trait]
	/// # impl Middleware for MyMiddleware {
	/// #     async fn process(&self, request: Request, next: Arc<dyn Handler>) -> reinhardt_core::exception::Result<Response> {
	/// #         next.handle(request).await
	/// #     }
	/// # }
	/// let handler = Arc::new(MyHandler);
	/// let middleware = Arc::new(MyMiddleware);
	/// let chain = MiddlewareChain::new(handler)
	///     .with_middleware(middleware);
	/// ```
	pub fn with_middleware(mut self, middleware: Arc<dyn Middleware>) -> Self {
		self.middlewares.push(middleware);
		self
	}

	/// Adds a middleware to the chain.
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_http::{MiddlewareChain, Handler, Middleware, Request, Response};
	/// use std::sync::Arc;
	///
	/// # struct MyHandler;
	/// # struct MyMiddleware;
	/// # #[async_trait::async_trait]
	/// # impl Handler for MyHandler {
	/// #     async fn handle(&self, _request: Request) -> reinhardt_core::exception::Result<Response> {
	/// #         Ok(Response::ok())
	/// #     }
	/// # }
	/// # #[async_trait::async_trait]
	/// # impl Middleware for MyMiddleware {
	/// #     async fn process(&self, request: Request, next: Arc<dyn Handler>) -> reinhardt_core::exception::Result<Response> {
	/// #         next.handle(request).await
	/// #     }
	/// # }
	/// let handler = Arc::new(MyHandler);
	/// let middleware = Arc::new(MyMiddleware);
	/// let mut chain = MiddlewareChain::new(handler);
	/// chain.add_middleware(middleware);
	/// ```
	pub fn add_middleware(&mut self, middleware: Arc<dyn Middleware>) {
		self.middlewares.push(middleware);
	}
}

#[async_trait]
impl Handler for MiddlewareChain {
	async fn handle(&self, request: Request) -> Result<Response> {
		if self.middlewares.is_empty() {
			return self.handler.handle(request).await;
		}

		// Build nested handler chain using composition with optimizations:
		// 1. Conditional execution (skip middleware based on should_continue)
		// 2. Short-circuiting (early return if response.should_stop_chain() is true)
		//
		// Performance improvements:
		// - Condition check: O(1) per middleware
		// - Skip unnecessary middleware: achieves O(k) where k <= n
		// - Early return: stops processing on first stop_chain=true response
		// Wrap the base handler to convert errors to responses, ensuring
		// all middleware post-processing runs even for error responses.
		let mut current_handler: Arc<dyn Handler> = Arc::new(ErrorToResponseHandler {
			inner: self.handler.clone(),
		});

		// Filter middleware based on should_continue condition
		// This achieves the O(k) optimization where k is the number of middleware that should run
		let active_middlewares: Vec<_> = self
			.middlewares
			.iter()
			.rev()
			.filter(|mw| mw.should_continue(&request))
			.collect();

		for middleware in active_middlewares {
			let mw = middleware.clone();
			let handler = current_handler.clone();

			current_handler = Arc::new(ConditionalComposedHandler {
				middleware: mw,
				next: handler,
			});
		}

		current_handler.handle(request).await
	}
}

/// Middleware wrapper that excludes specific URL paths from execution.
///
/// When a request matches an excluded path, the middleware is skipped
/// and the request passes directly to the next handler in the chain.
///
/// Path matching follows Django URL conventions:
/// - Paths ending with `/` are treated as **prefix matches**
///   (e.g., `"/api/auth/"` excludes `"/api/auth/login"`, `"/api/auth/register"`)
/// - Paths without trailing `/` require an **exact match**
///   (e.g., `"/health"` excludes only `"/health"`, not `"/health/check"`)
///
/// This struct is typically not used directly. Instead, use the
/// `exclude` methods on the `ServerRouter` or `UnifiedRouter` types
/// from the `reinhardt_urls::routers` module for declarative
/// route exclusion at the router level.
///
/// # Examples
///
/// ```rust
/// use reinhardt_http::middleware::ExcludeMiddleware;
/// use reinhardt_http::{Middleware, Request};
/// use std::sync::Arc;
///
/// # struct MyMiddleware;
/// # #[async_trait::async_trait]
/// # impl Middleware for MyMiddleware {
/// #     async fn process(
/// #         &self,
/// #         request: Request,
/// #         next: Arc<dyn reinhardt_http::Handler>,
/// #     ) -> reinhardt_core::exception::Result<reinhardt_http::Response> {
/// #         next.handle(request).await
/// #     }
/// # }
/// let inner: Arc<dyn Middleware> = Arc::new(MyMiddleware);
/// let excluded = ExcludeMiddleware::new(inner)
///     .add_exclusion("/api/auth/")   // prefix match
///     .add_exclusion("/health");     // exact match
/// ```
pub struct ExcludeMiddleware {
	inner: Arc<dyn Middleware>,
	exclusions: Vec<String>,
}

impl ExcludeMiddleware {
	/// Creates a new `ExcludeMiddleware` wrapping the given middleware.
	pub fn new(inner: Arc<dyn Middleware>) -> Self {
		Self {
			inner,
			exclusions: Vec::new(),
		}
	}

	/// Adds an exclusion pattern (builder pattern, consumes self).
	///
	/// Paths ending with `/` are prefix matches; others are exact matches.
	pub fn add_exclusion(mut self, pattern: &str) -> Self {
		self.exclusions.push(pattern.to_string());
		self
	}

	/// Adds an exclusion pattern (mutable reference).
	///
	/// Paths ending with `/` are prefix matches; others are exact matches.
	pub fn add_exclusion_mut(&mut self, pattern: &str) {
		self.exclusions.push(pattern.to_string());
	}

	/// Checks whether the given path matches any exclusion pattern.
	fn is_excluded(&self, path: &str) -> bool {
		self.exclusions.iter().any(|pattern| {
			if pattern.ends_with('/') {
				// Prefix match: excluded if path starts with the pattern
				path.starts_with(pattern.as_str())
			} else {
				// Exact match: excluded only if path equals the pattern
				path == pattern
			}
		})
	}
}

#[async_trait]
impl Middleware for ExcludeMiddleware {
	async fn process(&self, request: Request, next: Arc<dyn Handler>) -> Result<Response> {
		self.inner.process(request, next).await
	}

	fn should_continue(&self, request: &Request) -> bool {
		if self.is_excluded(request.uri.path()) {
			return false;
		}
		self.inner.should_continue(request)
	}
}

/// Internal handler wrapper that converts errors to HTTP responses.
///
/// Wraps the base handler so that middleware always receives `Ok(Response)`
/// from `next.handle()`, even when the handler returns an error. This ensures
/// middleware post-processing (e.g., adding security headers) runs for all
/// responses, matching Django's `process_response` semantics.
struct ErrorToResponseHandler {
	inner: Arc<dyn Handler>,
}

#[async_trait]
impl Handler for ErrorToResponseHandler {
	async fn handle(&self, request: Request) -> Result<Response> {
		match self.inner.handle(request).await {
			Ok(response) => Ok(response),
			Err(e) => Ok(Response::from(e)),
		}
	}
}

/// Internal handler that composes a single middleware with the next handler.
///
/// Converts middleware errors to HTTP responses so that outer middleware
/// post-processing (e.g., adding security headers) always runs.
struct ConditionalComposedHandler {
	middleware: Arc<dyn Middleware>,
	next: Arc<dyn Handler>,
}

#[async_trait]
impl Handler for ConditionalComposedHandler {
	async fn handle(&self, request: Request) -> Result<Response> {
		// Process the request through this middleware.
		// Convert errors to responses so that outer middleware post-processing
		// (e.g., security headers) always runs — matching Django's process_response
		// semantics where the response hook executes for both success and error cases.
		let response = match self.middleware.process(request, self.next.clone()).await {
			Ok(response) => response,
			Err(e) => Response::from(e),
		};

		Ok(response)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use bytes::Bytes;
	use hyper::{HeaderMap, Method, Version};

	// Mock handler for testing
	struct MockHandler {
		response_body: String,
	}

	#[async_trait]
	impl Handler for MockHandler {
		async fn handle(&self, _request: Request) -> Result<Response> {
			Ok(Response::ok().with_body(self.response_body.clone()))
		}
	}

	// Mock middleware for testing
	struct MockMiddleware {
		prefix: String,
	}

	#[async_trait]
	impl Middleware for MockMiddleware {
		async fn process(&self, request: Request, next: Arc<dyn Handler>) -> Result<Response> {
			// Call the next handler
			let response = next.handle(request).await?;

			// Modify the response
			let current_body = String::from_utf8(response.body.to_vec()).unwrap_or_default();
			let new_body = format!("{}{}", self.prefix, current_body);

			Ok(Response::ok().with_body(new_body))
		}
	}

	fn create_test_request() -> Request {
		Request::builder()
			.method(Method::GET)
			.uri("/")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap()
	}

	#[tokio::test]
	async fn test_handler_basic() {
		let handler = MockHandler {
			response_body: "Hello".to_string(),
		};

		let request = create_test_request();
		let response = handler.handle(request).await.unwrap();

		let body = String::from_utf8(response.body.to_vec()).unwrap();
		assert_eq!(body, "Hello");
	}

	#[tokio::test]
	async fn test_middleware_basic() {
		let handler = Arc::new(MockHandler {
			response_body: "World".to_string(),
		});

		let middleware = MockMiddleware {
			prefix: "Hello, ".to_string(),
		};

		let request = create_test_request();
		let response = middleware.process(request, handler).await.unwrap();

		let body = String::from_utf8(response.body.to_vec()).unwrap();
		assert_eq!(body, "Hello, World");
	}

	#[tokio::test]
	async fn test_middleware_chain_empty() {
		let handler = Arc::new(MockHandler {
			response_body: "Test".to_string(),
		});

		let chain = MiddlewareChain::new(handler);

		let request = create_test_request();
		let response = chain.handle(request).await.unwrap();

		let body = String::from_utf8(response.body.to_vec()).unwrap();
		assert_eq!(body, "Test");
	}

	#[tokio::test]
	async fn test_middleware_chain_single() {
		let handler = Arc::new(MockHandler {
			response_body: "Handler".to_string(),
		});

		let middleware1 = Arc::new(MockMiddleware {
			prefix: "MW1:".to_string(),
		});

		let chain = MiddlewareChain::new(handler).with_middleware(middleware1);

		let request = create_test_request();
		let response = chain.handle(request).await.unwrap();

		let body = String::from_utf8(response.body.to_vec()).unwrap();
		assert_eq!(body, "MW1:Handler");
	}

	#[tokio::test]
	async fn test_middleware_chain_multiple() {
		let handler = Arc::new(MockHandler {
			response_body: "Data".to_string(),
		});

		let middleware1 = Arc::new(MockMiddleware {
			prefix: "M1:".to_string(),
		});

		let middleware2 = Arc::new(MockMiddleware {
			prefix: "M2:".to_string(),
		});

		let chain = MiddlewareChain::new(handler)
			.with_middleware(middleware1)
			.with_middleware(middleware2);

		let request = create_test_request();
		let response = chain.handle(request).await.unwrap();

		let body = String::from_utf8(response.body.to_vec()).unwrap();
		// Middleware are applied in the order they were added
		assert_eq!(body, "M1:M2:Data");
	}

	#[tokio::test]
	async fn test_middleware_chain_add_middleware() {
		let handler = Arc::new(MockHandler {
			response_body: "Result".to_string(),
		});

		let middleware = Arc::new(MockMiddleware {
			prefix: "Prefix:".to_string(),
		});

		let mut chain = MiddlewareChain::new(handler);
		chain.add_middleware(middleware);

		let request = create_test_request();
		let response = chain.handle(request).await.unwrap();

		let body = String::from_utf8(response.body.to_vec()).unwrap();
		assert_eq!(body, "Prefix:Result");
	}

	// Conditional middleware that only runs for /api/* paths
	struct ConditionalMiddleware {
		prefix: String,
	}

	#[async_trait]
	impl Middleware for ConditionalMiddleware {
		async fn process(&self, request: Request, next: Arc<dyn Handler>) -> Result<Response> {
			let response = next.handle(request).await?;
			let current_body = String::from_utf8(response.body.to_vec()).unwrap_or_default();
			let new_body = format!("{}{}", self.prefix, current_body);
			Ok(Response::ok().with_body(new_body))
		}

		fn should_continue(&self, request: &Request) -> bool {
			request.uri.path().starts_with("/api/")
		}
	}

	#[tokio::test]
	async fn test_middleware_conditional_skip() {
		let handler = Arc::new(MockHandler {
			response_body: "Response".to_string(),
		});

		let conditional_mw = Arc::new(ConditionalMiddleware {
			prefix: "API:".to_string(),
		});

		let chain = MiddlewareChain::new(handler).with_middleware(conditional_mw);

		// Test with /api/ path - middleware should run
		let api_request = Request::builder()
			.method(Method::GET)
			.uri("/api/users")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();
		let response = chain.handle(api_request).await.unwrap();
		let body = String::from_utf8(response.body.to_vec()).unwrap();
		assert_eq!(body, "API:Response");

		// Test with non-/api/ path - middleware should be skipped
		let non_api_request = Request::builder()
			.method(Method::GET)
			.uri("/public")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();
		let response = chain.handle(non_api_request).await.unwrap();
		let body = String::from_utf8(response.body.to_vec()).unwrap();
		assert_eq!(body, "Response"); // No prefix because middleware was skipped
	}

	// Middleware that returns early with stop_chain=true
	struct ShortCircuitMiddleware {
		should_stop: bool,
	}

	#[async_trait]
	impl Middleware for ShortCircuitMiddleware {
		async fn process(&self, request: Request, next: Arc<dyn Handler>) -> Result<Response> {
			if self.should_stop {
				// Return early without calling next
				return Ok(Response::unauthorized()
					.with_body("Auth required")
					.with_stop_chain(true));
			}
			next.handle(request).await
		}
	}

	#[tokio::test]
	async fn test_middleware_short_circuit() {
		let handler = Arc::new(MockHandler {
			response_body: "Handler Response".to_string(),
		});

		let short_circuit_mw = Arc::new(ShortCircuitMiddleware { should_stop: true });
		let normal_mw = Arc::new(MockMiddleware {
			prefix: "Normal:".to_string(),
		});

		let chain = MiddlewareChain::new(handler)
			.with_middleware(short_circuit_mw)
			.with_middleware(normal_mw);

		let request = create_test_request();
		let response = chain.handle(request).await.unwrap();

		// Should get unauthorized response, not the handler response
		assert_eq!(response.status, hyper::StatusCode::UNAUTHORIZED);
		let body = String::from_utf8(response.body.to_vec()).unwrap();
		assert_eq!(body, "Auth required");
	}

	#[tokio::test]
	async fn test_middleware_no_short_circuit() {
		let handler = Arc::new(MockHandler {
			response_body: "Handler Response".to_string(),
		});

		let short_circuit_mw = Arc::new(ShortCircuitMiddleware { should_stop: false });
		let normal_mw = Arc::new(MockMiddleware {
			prefix: "Normal:".to_string(),
		});

		let chain = MiddlewareChain::new(handler)
			.with_middleware(short_circuit_mw)
			.with_middleware(normal_mw);

		let request = create_test_request();
		let response = chain.handle(request).await.unwrap();

		// Should pass through to handler and apply normal middleware
		assert_eq!(response.status, hyper::StatusCode::OK);
		let body = String::from_utf8(response.body.to_vec()).unwrap();
		assert_eq!(body, "Normal:Handler Response");
	}

	#[tokio::test]
	async fn test_middleware_multiple_conditions() {
		let handler = Arc::new(MockHandler {
			response_body: "Base".to_string(),
		});

		// Only runs for /api/* paths
		let api_mw = Arc::new(ConditionalMiddleware {
			prefix: "API:".to_string(),
		});

		// Always runs
		let always_mw = Arc::new(MockMiddleware {
			prefix: "Always:".to_string(),
		});

		let chain = MiddlewareChain::new(handler)
			.with_middleware(api_mw)
			.with_middleware(always_mw);

		// Test with /api/ path - both middleware should run
		let api_request = Request::builder()
			.method(Method::GET)
			.uri("/api/test")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();
		let response = chain.handle(api_request).await.unwrap();
		let body = String::from_utf8(response.body.to_vec()).unwrap();
		assert_eq!(body, "API:Always:Base");

		// Test with non-/api/ path - only always_mw should run
		let non_api_request = Request::builder()
			.method(Method::GET)
			.uri("/public")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();
		let response = chain.handle(non_api_request).await.unwrap();
		let body = String::from_utf8(response.body.to_vec()).unwrap();
		assert_eq!(body, "Always:Base"); // Only always_mw prefix
	}

	#[tokio::test]
	async fn test_response_should_stop_chain() {
		let response = Response::ok();
		assert!(!response.should_stop_chain());

		let stopping_response = Response::unauthorized().with_stop_chain(true);
		assert!(stopping_response.should_stop_chain());
	}

	// --- ExcludeMiddleware tests ---

	fn create_request_with_path(path: &str) -> Request {
		Request::builder()
			.method(Method::GET)
			.uri(path)
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap()
	}

	#[rstest::rstest]
	#[case("/api/auth/login", true)]
	#[case("/api/auth/register", true)]
	#[case("/api/auth/", true)]
	#[case("/api/users", false)]
	#[case("/public", false)]
	fn test_exclude_middleware_prefix_match(#[case] path: &str, #[case] should_exclude: bool) {
		// Arrange
		let inner: Arc<dyn Middleware> = Arc::new(MockMiddleware {
			prefix: "MW:".to_string(),
		});
		let exclude_mw = ExcludeMiddleware::new(inner).add_exclusion("/api/auth/");

		// Act
		let request = create_request_with_path(path);
		let result = exclude_mw.should_continue(&request);

		// Assert
		assert_eq!(result, !should_exclude);
	}

	#[rstest::rstest]
	#[case("/health", true)]
	#[case("/health/check", false)]
	#[case("/healthz", false)]
	#[case("/api/health", false)]
	fn test_exclude_middleware_exact_match(#[case] path: &str, #[case] should_exclude: bool) {
		// Arrange
		let inner: Arc<dyn Middleware> = Arc::new(MockMiddleware {
			prefix: "MW:".to_string(),
		});
		let exclude_mw = ExcludeMiddleware::new(inner).add_exclusion("/health");

		// Act
		let request = create_request_with_path(path);
		let result = exclude_mw.should_continue(&request);

		// Assert
		assert_eq!(result, !should_exclude);
	}

	#[rstest::rstest]
	fn test_exclude_middleware_no_match_passes_through() {
		// Arrange
		let inner: Arc<dyn Middleware> = Arc::new(MockMiddleware {
			prefix: "MW:".to_string(),
		});
		let exclude_mw = ExcludeMiddleware::new(inner)
			.add_exclusion("/api/auth/")
			.add_exclusion("/health");

		// Act
		let request = create_request_with_path("/api/users");
		let result = exclude_mw.should_continue(&request);

		// Assert
		assert!(result);
	}

	#[rstest::rstest]
	#[tokio::test]
	async fn test_exclude_middleware_delegates_process() {
		// Arrange
		let inner: Arc<dyn Middleware> = Arc::new(MockMiddleware {
			prefix: "INNER:".to_string(),
		});
		let exclude_mw = ExcludeMiddleware::new(inner).add_exclusion("/excluded/");

		let handler = Arc::new(MockHandler {
			response_body: "Response".to_string(),
		});

		// Act
		let request = create_request_with_path("/api/test");
		let response = exclude_mw.process(request, handler).await.unwrap();

		// Assert
		let body = String::from_utf8(response.body.to_vec()).unwrap();
		assert_eq!(body, "INNER:Response");
	}

	#[rstest::rstest]
	fn test_exclude_middleware_multiple_exclusions() {
		// Arrange
		let inner: Arc<dyn Middleware> = Arc::new(MockMiddleware {
			prefix: "MW:".to_string(),
		});
		let mut exclude_mw = ExcludeMiddleware::new(inner);
		exclude_mw.add_exclusion_mut("/api/auth/");
		exclude_mw.add_exclusion_mut("/admin/");
		exclude_mw.add_exclusion_mut("/health");

		// Act & Assert
		assert!(!exclude_mw.should_continue(&create_request_with_path("/api/auth/login")));
		assert!(!exclude_mw.should_continue(&create_request_with_path("/admin/dashboard")));
		assert!(!exclude_mw.should_continue(&create_request_with_path("/health")));
		assert!(exclude_mw.should_continue(&create_request_with_path("/api/users")));
	}

	#[rstest::rstest]
	fn test_exclude_middleware_respects_inner_should_continue() {
		// Arrange - inner middleware that rejects non-/api/ paths
		let inner: Arc<dyn Middleware> = Arc::new(ConditionalMiddleware {
			prefix: "API:".to_string(),
		});
		let exclude_mw = ExcludeMiddleware::new(inner).add_exclusion("/api/auth/");

		// Act & Assert
		// Excluded path -> false (excluded by wrapper)
		assert!(!exclude_mw.should_continue(&create_request_with_path("/api/auth/login")));
		// Non-excluded, but inner rejects non-/api/ -> false (inner's should_continue)
		assert!(!exclude_mw.should_continue(&create_request_with_path("/public")));
		// Non-excluded, inner accepts /api/ -> true
		assert!(exclude_mw.should_continue(&create_request_with_path("/api/users")));
	}

	// ========================================================================
	// Error-to-response conversion tests (issue #3230)
	// ========================================================================

	/// Handler that always returns an error.
	struct NotFoundHandler;

	#[async_trait]
	impl Handler for NotFoundHandler {
		async fn handle(&self, _request: Request) -> Result<Response> {
			Err(reinhardt_core::exception::Error::NotFound(
				"not found".into(),
			))
		}
	}

	struct UnauthorizedHandler;

	#[async_trait]
	impl Handler for UnauthorizedHandler {
		async fn handle(&self, _request: Request) -> Result<Response> {
			Err(reinhardt_core::exception::Error::Authentication(
				"unauthorized".into(),
			))
		}
	}

	/// Middleware that adds a custom header to the response after calling next.
	struct HeaderAddingMiddleware {
		header_name: &'static str,
		header_value: &'static str,
	}

	#[async_trait]
	impl Middleware for HeaderAddingMiddleware {
		async fn process(&self, request: Request, next: Arc<dyn Handler>) -> Result<Response> {
			let response = next.handle(request).await?;
			Ok(response.with_header(self.header_name, self.header_value))
		}
	}

	/// Middleware that always returns an error (simulates CSRF rejection).
	struct RejectingMiddleware;

	#[async_trait]
	impl Middleware for RejectingMiddleware {
		async fn process(&self, _request: Request, _next: Arc<dyn Handler>) -> Result<Response> {
			Err(reinhardt_core::exception::Error::Authorization(
				"CSRF check failed".into(),
			))
		}
	}

	#[rstest::rstest]
	#[tokio::test]
	async fn test_chain_post_processing_runs_on_handler_error() {
		// Arrange: handler returns 404 error, outer middleware adds header
		let handler: Arc<dyn Handler> = Arc::new(NotFoundHandler);
		let mut chain = MiddlewareChain::new(handler);
		chain.add_middleware(Arc::new(HeaderAddingMiddleware {
			header_name: "X-Custom-Security",
			header_value: "applied",
		}));

		// Act
		let request = create_test_request();
		let response = chain.handle(request).await.unwrap();

		// Assert: error converted to 404 response AND header is present
		assert_eq!(response.status, hyper::StatusCode::NOT_FOUND);
		assert_eq!(
			response
				.headers
				.get("X-Custom-Security")
				.map(|v| v.to_str().unwrap()),
			Some("applied")
		);
	}

	#[rstest::rstest]
	#[tokio::test]
	async fn test_chain_post_processing_runs_on_middleware_error() {
		// Arrange: outer middleware adds header, inner middleware rejects.
		// First add = outermost in this framework's chain ordering.
		let handler = Arc::new(MockHandler {
			response_body: "OK".into(),
		});
		let mut chain = MiddlewareChain::new(handler);
		// Outer middleware adds a security header (post-processing)
		chain.add_middleware(Arc::new(HeaderAddingMiddleware {
			header_name: "X-Frame-Options",
			header_value: "DENY",
		}));
		// Inner middleware rejects the request
		chain.add_middleware(Arc::new(RejectingMiddleware));

		// Act
		let request = create_test_request();
		let response = chain.handle(request).await.unwrap();

		// Assert: inner middleware error converted to 403, outer middleware header present
		assert_eq!(response.status, hyper::StatusCode::FORBIDDEN);
		assert_eq!(
			response
				.headers
				.get("X-Frame-Options")
				.map(|v| v.to_str().unwrap()),
			Some("DENY")
		);
	}

	/// Passthrough middleware that does not modify the response.
	struct PassthroughMiddleware;

	#[async_trait]
	impl Middleware for PassthroughMiddleware {
		async fn process(&self, request: Request, next: Arc<dyn Handler>) -> Result<Response> {
			next.handle(request).await
		}
	}

	#[rstest::rstest]
	#[tokio::test]
	async fn test_chain_error_preserves_correct_status_code() {
		// Arrange: handler returns 401 Unauthorized, with at least one middleware
		// so that ConditionalComposedHandler is used (empty chain bypasses it)
		let handler: Arc<dyn Handler> = Arc::new(UnauthorizedHandler);
		let mut chain = MiddlewareChain::new(handler);
		chain.add_middleware(Arc::new(PassthroughMiddleware));

		// Act
		let request = create_test_request();
		let response = chain.handle(request).await.unwrap();

		// Assert: status code correctly reflects the error
		assert_eq!(response.status, hyper::StatusCode::UNAUTHORIZED);
	}
}
