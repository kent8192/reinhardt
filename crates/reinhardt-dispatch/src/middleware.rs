//! Middleware system for request/response processing pipeline.

use reinhardt_core::exception::{Error, Result};
use reinhardt_http::{Handler, Request, Response};
use reinhardt_middleware::Middleware;
use std::sync::Arc;

/// Default maximum number of middleware components in a chain.
const DEFAULT_MAX_MIDDLEWARE_DEPTH: usize = 256;

/// A middleware chain that composes multiple middleware components with a handler.
pub struct MiddlewareChain {
	handler: Arc<dyn Handler>,
	middlewares: Vec<Arc<dyn Middleware>>,
	/// Maximum number of middleware components allowed in the chain.
	max_depth: usize,
}

impl MiddlewareChain {
	/// Creates a new middleware chain with the given handler.
	pub fn new(handler: Arc<dyn Handler>) -> Self {
		Self {
			handler,
			middlewares: Vec::new(),
			max_depth: DEFAULT_MAX_MIDDLEWARE_DEPTH,
		}
	}

	/// Sets the maximum number of middleware components allowed in the chain.
	pub fn with_max_depth(mut self, max_depth: usize) -> Self {
		self.max_depth = max_depth;
		self
	}

	/// Adds a middleware to the chain.
	///
	/// Returns an error if adding the middleware would exceed the maximum depth.
	pub fn add_middleware(mut self, middleware: Arc<dyn Middleware>) -> Result<Self> {
		if self.middlewares.len() >= self.max_depth {
			return Err(Error::ImproperlyConfigured(format!(
				"middleware chain depth limit exceeded (max: {})",
				self.max_depth
			)));
		}
		self.middlewares.push(middleware);
		Ok(self)
	}

	/// Builds the final handler by composing all middleware.
	pub fn build(self) -> Arc<dyn Handler> {
		let mut handler = self.handler;

		for middleware in self.middlewares.into_iter().rev() {
			handler = Arc::new(MiddlewareHandler {
				middleware,
				next: handler,
			});
		}

		handler
	}
}

/// Internal handler that wraps a middleware with its next handler.
struct MiddlewareHandler {
	middleware: Arc<dyn Middleware>,
	next: Arc<dyn Handler>,
}

#[async_trait::async_trait]
impl Handler for MiddlewareHandler {
	async fn handle(&self, request: Request) -> Result<Response> {
		self.middleware.process(request, self.next.clone()).await
	}
}
