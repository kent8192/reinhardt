//! Trait impls for [`ServerRouter`] that don't fit the builder / dispatch split.
//!
//! Includes `Debug`, `Default`, the `Handler` impl that turns the router
//! into an HTTP entry point, and the `RegisterViewSet` adapter used by
//! `ViewSetBuilder`.

use super::ServerRouter;
#[cfg(feature = "viewsets")]
use super::types::{RouteContractMetadata, ViewRoute};
use async_trait::async_trait;
use reinhardt_http::{Error, Handler, MiddlewareChain, Request, Response, Result};
use std::sync::Arc;

impl std::fmt::Debug for ServerRouter {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		let mut debug = f.debug_struct("ServerRouter");
		debug
			.field("prefix", &self.prefix)
			.field("namespace", &self.namespace)
			.field("routes", &self.routes.len());
		#[cfg(feature = "viewsets")]
		debug.field("viewsets", &self.viewsets.len());
		debug
			.field("functions", &self.functions.len())
			.field("views", &self.views.len())
			.field("children", &self.children.len())
			.field("middleware", &self.middleware.len())
			.finish_non_exhaustive()
	}
}

impl Default for ServerRouter {
	fn default() -> Self {
		Self::new()
	}
}

/// Handler that always returns a pre-built response.
///
/// Used internally to route framework-level error responses (404/405)
/// through the middleware chain for post-processing. (#3234)
struct FixedResponseHandler(Response);

#[async_trait]
impl Handler for FixedResponseHandler {
	async fn handle(&self, _request: Request) -> Result<Response> {
		Ok(self.0.clone())
	}
}

macro_rules! dispatch_router_request {
	($router:expr, $req:expr) => {{
		let mut req = $req;
		let path = req.uri.path();
		let method = &req.method;

		// Resolve route with HTTP method for matchit routing
		let route_match = match $router.resolve(path, method) {
			Some(m) => m,
			None => {
				// Route not found for this method
				// Check if path exists for any other method to determine 404 vs 405
				let error = if $router.path_exists_for_any_method(path) {
					Error::MethodNotAllowed(format!("Method {} not allowed for {}", method, path))
				} else {
					Error::NotFound(format!("No route for {} {}", method, path))
				};

				// If router has middleware, route the error response through the
				// middleware chain so post-processing (e.g., security headers) is
				// applied to framework-level 404/405 responses. (#3234)
				let own_middleware = $router.build_middleware_with_exclusions();
				if own_middleware.is_empty() {
					return Err(error);
				}

				let response = Response::from(error);
				let handler: Arc<dyn Handler> = Arc::new(FixedResponseHandler(response));
				let chain = own_middleware
					.iter()
					.fold(MiddlewareChain::new(handler), |chain, mw| {
						chain.with_middleware(mw.clone())
					});
				return chain.handle(req).await;
			}
		};

		if let Some(params) = route_match.params {
			req.path_params = params;
		} else if !req.path_params.is_empty() {
			req.path_params = Default::default();
		}

		// Set DI context if available
		if let Some(di_ctx) = &route_match.di_context {
			req.set_di_context(di_ctx.clone());
		}

		// Apply middleware stack using MiddlewareChain
		if route_match.middleware_stack.is_empty() {
			if let Some(requestless_handler) = route_match.requestless_sync_handler {
				return requestless_handler.handle_requestless_sync();
			}
			if let Some(sync_handler) = route_match.sync_handler {
				return sync_handler.handle_sync(req);
			}

			// No middleware, execute the trait object directly. Calling through
			// `Arc<dyn Handler>` would add the blanket `Arc<T>` async-trait box.
			route_match.handler.as_ref().handle(req).await
		} else {
			let chain = MiddlewareChain::with_middlewares(
				Arc::clone(route_match.handler),
				route_match.middleware_stack,
			);

			// Execute chain
			chain.handle(req).await
		}
	}};
}

impl ServerRouter {
	fn try_dispatch_exact_requestless_sync(
		&self,
		path: &str,
		method: &hyper::Method,
	) -> Option<Result<Response>> {
		if !self.children.is_empty() || !self.middleware.is_empty() || self.di_context.is_some() {
			return None;
		}

		let remaining_path = Self::strip_prefix_normalized(&self.prefix, path)?;
		let route_handler = self
			.compiled_routes()
			.exact_for_method(method)?
			.get(remaining_path.as_ref())?;

		if !route_handler.middleware.is_empty() || !route_handler.param_names.is_empty() {
			return None;
		}

		route_handler
			.requestless_sync_handler
			.as_ref()
			.map(|handler| handler.handle_requestless_sync())
	}

	/// Try to dispatch a request through the synchronous route fast path.
	///
	/// Returns `None` when the matched route requires async handling or a
	/// middleware chain. Callers that need general routing should fall back to
	/// [`Self::dispatch`] in that case.
	pub fn try_dispatch_sync(&self, mut req: Request) -> Option<Result<Response>> {
		if self.children.is_empty()
			&& self.middleware.is_empty()
			&& let Some(remaining_path) =
				Self::strip_prefix_normalized(&self.prefix, req.uri.path())
			&& let Some(exact_routes) = self.compiled_routes().exact_for_method(&req.method)
			&& let Some(route_handler) = exact_routes.get(remaining_path.as_ref())
			&& route_handler.middleware.is_empty()
			&& route_handler.param_names.is_empty()
		{
			if let Some(requestless_handler) = route_handler.requestless_sync_handler.as_ref()
				&& self.di_context.is_none()
			{
				return Some(requestless_handler.handle_requestless_sync());
			}

			if let Some(sync_handler) = route_handler.sync_handler.as_ref() {
				if !req.path_params.is_empty() {
					req.path_params = Default::default();
				}
				if let Some(di_ctx) = &self.di_context {
					req.set_di_context(di_ctx.clone());
				}
				return Some(sync_handler.handle_sync(req));
			}
		}

		let path = req.uri.path();
		let method = &req.method;

		let route_match = match self.resolve(path, method) {
			Some(m) => m,
			None => {
				let error = if self.path_exists_for_any_method(path) {
					Error::MethodNotAllowed(format!("Method {} not allowed for {}", method, path))
				} else {
					Error::NotFound(format!("No route for {} {}", method, path))
				};

				if self.build_middleware_with_exclusions().is_empty() {
					return Some(Err(error));
				}
				return None;
			}
		};

		if !route_match.middleware_stack.is_empty() {
			return None;
		}

		if let Some(requestless_handler) = route_match.requestless_sync_handler {
			return Some(requestless_handler.handle_requestless_sync());
		}

		let sync_handler = route_match.sync_handler?;
		if let Some(params) = route_match.params {
			req.path_params = params;
		} else if !req.path_params.is_empty() {
			req.path_params = Default::default();
		}
		if let Some(di_ctx) = &route_match.di_context {
			req.set_di_context(di_ctx.clone());
		}

		Some(sync_handler.handle_sync(req))
	}

	/// Try to dispatch a requestless synchronous route before building a request.
	///
	/// This only succeeds for routes that need no request state: no middleware,
	/// no path parameters, and no DI context. HTTP adapters can use this after
	/// validating that the incoming request has no body.
	pub fn try_dispatch_requestless_sync(
		&self,
		path: &str,
		method: &hyper::Method,
	) -> Option<Result<Response>> {
		if let Some(response) = self.try_dispatch_exact_requestless_sync(path, method) {
			return Some(response);
		}

		let route_match = self.resolve(path, method)?;
		if !route_match.middleware_stack.is_empty()
			|| route_match.params.is_some()
			|| route_match.di_context.is_some()
		{
			return None;
		}

		let requestless_handler = route_match.requestless_sync_handler?;
		Some(requestless_handler.handle_requestless_sync())
	}

	/// Dispatch a request through this router without a trait-object handler wrapper.
	///
	/// This has the same routing behavior as the [`Handler`] implementation, but
	/// concrete callers can await the router's inherent future directly instead of
	/// going through the boxed future produced by `async_trait`.
	pub async fn dispatch(&self, req: Request) -> Result<Response> {
		dispatch_router_request!(self, req)
	}
}

/// Handler implementation for ServerRouter
#[async_trait]
impl Handler for ServerRouter {
	async fn handle(&self, req: Request) -> Result<Response> {
		dispatch_router_request!(self, req)
	}
}

/// Implement RegisterViewSet trait for ServerRouter
///
/// This allows ViewSetBuilder to directly register handlers to the router.
#[cfg(feature = "viewsets")]
impl reinhardt_views::viewsets::RegisterViewSet for ServerRouter {
	fn register_handler(&mut self, path: &str, handler: Arc<dyn Handler>) {
		self.invalidate_compiled_routes();
		self.views.push(ViewRoute {
			path: path.to_string(),
			handler,
			sync_handler: None,
			requestless_sync_handler: None,
			name: None,
			metadata: RouteContractMetadata {
				handler: "<erased handler>".to_string(),
				authentication: reinhardt_core::endpoint::AuthProtection::None,
				guard: None,
			},
			middleware: Vec::new(),
		});
	}
}
