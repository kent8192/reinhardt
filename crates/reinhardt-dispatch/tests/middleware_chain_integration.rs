//! Integration tests for Middleware Chain
//!
//! Tests the integration between middleware chain and handler:
//! - Multiple middleware chaining and execution order
//! - Request/response modification through middleware
//! - Exception propagation through middleware chain

use async_trait::async_trait;
use bytes::Bytes;
use http::{Method, StatusCode};
use reinhardt_dispatch::middleware::MiddlewareChain;
use reinhardt_http::Handler;
use reinhardt_http::{Request, Response};
use reinhardt_middleware::Middleware;
use reinhardt_urls::prelude::Router;
use reinhardt_urls::routers::{DefaultRouter, Route};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Simple test handler
struct TestHandler;

#[async_trait]
impl Handler for TestHandler {
	async fn handle(&self, _request: Request) -> reinhardt_core::exception::Result<Response> {
		Ok(Response::new(StatusCode::OK).with_body(Bytes::from("OK")))
	}
}

/// Test middleware that adds a header
struct HeaderMiddleware {
	header_name: String,
	header_value: String,
}

#[async_trait]
impl Middleware for HeaderMiddleware {
	async fn process(
		&self,
		mut request: Request,
		handler: Arc<dyn Handler>,
	) -> reinhardt_core::exception::Result<Response> {
		// Add header to request
		request.headers.insert(
			http::HeaderName::from_bytes(self.header_name.as_bytes()).unwrap(),
			http::HeaderValue::from_str(&self.header_value).unwrap(),
		);

		// Call next handler
		let response = handler.handle(request).await?;
		Ok(response)
	}
}

/// Test middleware that modifies response
struct ResponseModifierMiddleware {
	status: StatusCode,
}

#[async_trait]
impl Middleware for ResponseModifierMiddleware {
	async fn process(
		&self,
		request: Request,
		handler: Arc<dyn Handler>,
	) -> reinhardt_core::exception::Result<Response> {
		// Call handler
		let mut response = handler.handle(request).await?;

		// Modify response status
		response.status = self.status;
		Ok(response)
	}
}

/// Test middleware that counts invocations
struct CounterMiddleware {
	counter: Arc<AtomicUsize>,
}

#[async_trait]
impl Middleware for CounterMiddleware {
	async fn process(
		&self,
		request: Request,
		handler: Arc<dyn Handler>,
	) -> reinhardt_core::exception::Result<Response> {
		self.counter.fetch_add(1, Ordering::SeqCst);
		handler.handle(request).await
	}
}

/// Test middleware that simulates error
struct ErrorMiddleware;

#[async_trait]
impl Middleware for ErrorMiddleware {
	async fn process(
		&self,
		_request: Request,
		_handler: Arc<dyn Handler>,
	) -> reinhardt_core::exception::Result<Response> {
		Err(reinhardt_core::exception::Error::Internal(
			"Middleware error".to_string(),
		))
	}
}

#[tokio::test]
async fn test_single_middleware_execution() {
	// Setup router and handler
	let mut router = DefaultRouter::new();
	let route = Route::from_handler("/test", Arc::new(TestHandler));
	router.add_route(route);
	let base_handler = Arc::new(router);

	// Create middleware chain with counter
	let counter = Arc::new(AtomicUsize::new(0));
	let middleware = Arc::new(CounterMiddleware {
		counter: counter.clone(),
	});

	let handler = MiddlewareChain::new(base_handler)
		.add_middleware(middleware)
		.expect("Failed to add middleware")
		.build();

	// Execute request
	let request = Request::builder()
		.method(Method::GET)
		.uri("/test")
		.body(Bytes::new())
		.build()
		.expect("Failed to build request");

	let response = handler.handle(request).await;

	// Verify middleware was executed
	assert!(response.is_ok());
	assert_eq!(
		counter.load(Ordering::SeqCst),
		1,
		"Middleware should be executed once"
	);
}

#[tokio::test]
async fn test_multiple_middleware_execution_order() {
	// Setup router and handler
	let mut router = DefaultRouter::new();
	let route = Route::from_handler("/test", Arc::new(TestHandler));
	router.add_route(route);
	let base_handler = Arc::new(router);

	// Create counters for each middleware
	let counter1 = Arc::new(AtomicUsize::new(0));
	let counter2 = Arc::new(AtomicUsize::new(0));
	let counter3 = Arc::new(AtomicUsize::new(0));

	// Create middleware chain
	let handler = MiddlewareChain::new(base_handler)
		.add_middleware(Arc::new(CounterMiddleware {
			counter: counter1.clone(),
		}))
		.expect("Failed to add middleware")
		.add_middleware(Arc::new(CounterMiddleware {
			counter: counter2.clone(),
		}))
		.expect("Failed to add middleware")
		.add_middleware(Arc::new(CounterMiddleware {
			counter: counter3.clone(),
		}))
		.expect("Failed to add middleware")
		.build();

	// Execute request
	let request = Request::builder()
		.method(Method::GET)
		.uri("/test")
		.body(Bytes::new())
		.build()
		.expect("Failed to build request");

	let response = handler.handle(request).await;

	// Verify all middleware were executed
	assert!(response.is_ok());
	assert_eq!(
		counter1.load(Ordering::SeqCst),
		1,
		"First middleware should be executed"
	);
	assert_eq!(
		counter2.load(Ordering::SeqCst),
		1,
		"Second middleware should be executed"
	);
	assert_eq!(
		counter3.load(Ordering::SeqCst),
		1,
		"Third middleware should be executed"
	);
}

#[tokio::test]
async fn test_middleware_can_modify_request() {
	// Setup router and view that checks for header
	struct HeaderCheckHandler;

	#[async_trait]
	impl Handler for HeaderCheckHandler {
		async fn handle(&self, request: Request) -> reinhardt_core::exception::Result<Response> {
			let has_header = request.headers.contains_key("X-Test-Header");
			let body = if has_header { "FOUND" } else { "NOT_FOUND" };
			Ok(Response::new(StatusCode::OK).with_body(Bytes::from(body)))
		}
	}

	let mut router = DefaultRouter::new();
	let route = Route::from_handler("/test", Arc::new(HeaderCheckHandler));
	router.add_route(route);
	let base_handler = Arc::new(router);

	// Create middleware chain with header middleware
	let middleware = Arc::new(HeaderMiddleware {
		header_name: "X-Test-Header".to_string(),
		header_value: "test-value".to_string(),
	});

	let handler = MiddlewareChain::new(base_handler)
		.add_middleware(middleware)
		.expect("Failed to add middleware")
		.build();

	// Execute request
	let request = Request::builder()
		.method(Method::GET)
		.uri("/test")
		.body(Bytes::new())
		.build()
		.expect("Failed to build request");

	let response = handler
		.handle(request)
		.await
		.expect("Failed to build request");

	// Verify middleware added header
	assert_eq!(response.status, StatusCode::OK);
	assert_eq!(&response.body, &Bytes::from("FOUND"));
}

#[tokio::test]
async fn test_middleware_can_modify_response() {
	// Setup router and handler
	let mut router = DefaultRouter::new();
	let route = Route::from_handler("/test", Arc::new(TestHandler));
	router.add_route(route);
	let base_handler = Arc::new(router);

	// Create middleware chain with response modifier
	let middleware = Arc::new(ResponseModifierMiddleware {
		status: StatusCode::ACCEPTED,
	});

	let handler = MiddlewareChain::new(base_handler)
		.add_middleware(middleware)
		.expect("Failed to add middleware")
		.build();

	// Execute request
	let request = Request::builder()
		.method(Method::GET)
		.uri("/test")
		.body(Bytes::new())
		.build()
		.expect("Failed to build request");

	let response = handler
		.handle(request)
		.await
		.expect("Failed to build request");

	// Verify middleware modified response
	assert_eq!(
		response.status,
		StatusCode::ACCEPTED,
		"Middleware should modify response status"
	);
}

#[tokio::test]
async fn test_middleware_error_propagates() {
	// Setup router and handler
	let mut router = DefaultRouter::new();
	let route = Route::from_handler("/test", Arc::new(TestHandler));
	router.add_route(route);
	let base_handler = Arc::new(router);

	// Create middleware chain with error middleware
	let middleware = Arc::new(ErrorMiddleware);

	let handler = MiddlewareChain::new(base_handler)
		.add_middleware(middleware)
		.expect("Failed to add middleware")
		.build();

	// Execute request
	let request = Request::builder()
		.method(Method::GET)
		.uri("/test")
		.body(Bytes::new())
		.build()
		.expect("Failed to build request");

	let response = handler.handle(request).await;

	// Verify error propagates
	assert!(response.is_err(), "Middleware error should propagate");
	assert_eq!(
		response.unwrap_err().to_string(),
		"Internal server error: Middleware error",
		"Error message should match"
	);
}

#[tokio::test]
async fn test_middleware_chain_stops_on_error() {
	// Setup router and handler
	let mut router = DefaultRouter::new();
	let route = Route::from_handler("/test", Arc::new(TestHandler));
	router.add_route(route);
	let base_handler = Arc::new(router);

	// Create counters
	let counter1 = Arc::new(AtomicUsize::new(0));
	let counter2 = Arc::new(AtomicUsize::new(0));

	// Create middleware chain with error in the middle
	let handler = MiddlewareChain::new(base_handler)
		.add_middleware(Arc::new(CounterMiddleware {
			counter: counter1.clone(),
		}))
		.expect("Failed to add middleware")
		.add_middleware(Arc::new(ErrorMiddleware))
		.expect("Failed to add middleware")
		.add_middleware(Arc::new(CounterMiddleware {
			counter: counter2.clone(),
		}))
		.expect("Failed to add middleware")
		.build();

	// Execute request
	let request = Request::builder()
		.method(Method::GET)
		.uri("/test")
		.body(Bytes::new())
		.build()
		.expect("Failed to build request");

	let response = handler.handle(request).await;

	// Verify error stops chain
	assert!(response.is_err());
	// Middleware execution order (due to rev() in build()):
	// CounterMiddleware(counter1) -> ErrorMiddleware -> CounterMiddleware(counter2) -> BaseHandler
	// counter1 increments, then ErrorMiddleware returns error, stopping the chain
	assert_eq!(
		counter1.load(Ordering::SeqCst),
		1,
		"First middleware executes before error"
	);
	assert_eq!(
		counter2.load(Ordering::SeqCst),
		0,
		"Second middleware should not execute after error"
	);
}

#[tokio::test]
async fn test_empty_middleware_chain() {
	// Setup router and handler
	let mut router = DefaultRouter::new();
	let route = Route::from_handler("/test", Arc::new(TestHandler));
	router.add_route(route);
	let base_handler = Arc::new(router);

	// Create empty middleware chain
	let handler = MiddlewareChain::new(base_handler).build();

	// Execute request
	let request = Request::builder()
		.method(Method::GET)
		.uri("/test")
		.body(Bytes::new())
		.build()
		.expect("Failed to build request");

	let response = handler.handle(request).await;

	// Verify handler works without middleware
	assert!(response.is_ok());
	let response = response.expect("Failed to build request");
	assert_eq!(response.status, StatusCode::OK);
	assert_eq!(&response.body, &Bytes::from("OK"));
}

#[tokio::test]
async fn test_middleware_chain_depth_limit() {
	// Setup router and handler
	let mut router = DefaultRouter::new();
	let route = Route::from_handler("/test", Arc::new(TestHandler));
	router.add_route(route);
	let base_handler = Arc::new(router);

	// Create a chain with max depth of 2
	let chain = MiddlewareChain::new(base_handler).with_max_depth(2);

	// Adding 2 middleware should succeed
	let counter1 = Arc::new(AtomicUsize::new(0));
	let counter2 = Arc::new(AtomicUsize::new(0));

	let chain = chain
		.add_middleware(Arc::new(CounterMiddleware {
			counter: counter1.clone(),
		}))
		.expect("First middleware should be accepted");

	let chain = chain
		.add_middleware(Arc::new(CounterMiddleware {
			counter: counter2.clone(),
		}))
		.expect("Second middleware should be accepted");

	// Adding a 3rd middleware should fail
	let counter3 = Arc::new(AtomicUsize::new(0));
	let result = chain.add_middleware(Arc::new(CounterMiddleware {
		counter: counter3.clone(),
	}));

	match result {
		Err(err) => {
			assert!(
				err.to_string().contains("depth limit exceeded"),
				"Error should mention depth limit, got: {}",
				err
			);
		}
		Ok(_) => panic!("Adding middleware beyond max depth should fail"),
	}
}

#[tokio::test]
async fn test_middleware_chain_default_depth_limit() {
	// Setup router and handler
	let mut router = DefaultRouter::new();
	let route = Route::from_handler("/test", Arc::new(TestHandler));
	router.add_route(route);
	let base_handler = Arc::new(router);

	// Default limit should allow at least 256 middleware
	let mut chain = MiddlewareChain::new(base_handler);
	for _ in 0..256 {
		chain = chain
			.add_middleware(Arc::new(CounterMiddleware {
				counter: Arc::new(AtomicUsize::new(0)),
			}))
			.expect("Should be within default limit");
	}

	// The 257th should fail
	let result = chain.add_middleware(Arc::new(CounterMiddleware {
		counter: Arc::new(AtomicUsize::new(0)),
	}));
	assert!(
		result.is_err(),
		"257th middleware should exceed default limit of 256"
	);
}

#[tokio::test]
async fn test_middleware_chain_custom_depth_limit() {
	// Setup router and handler
	let mut router = DefaultRouter::new();
	let route = Route::from_handler("/test", Arc::new(TestHandler));
	router.add_route(route);
	let base_handler = Arc::new(router);

	// Create chain with custom depth limit of 1
	let chain = MiddlewareChain::new(base_handler).with_max_depth(1);

	// First middleware should succeed
	let chain = chain
		.add_middleware(Arc::new(CounterMiddleware {
			counter: Arc::new(AtomicUsize::new(0)),
		}))
		.expect("First middleware within limit");

	// Second should fail
	let result = chain.add_middleware(Arc::new(CounterMiddleware {
		counter: Arc::new(AtomicUsize::new(0)),
	}));
	assert!(
		result.is_err(),
		"Should reject middleware beyond limit of 1"
	);
}
