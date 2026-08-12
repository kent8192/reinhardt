//! Route-registration methods for [`ServerRouter`].
//!
//! Covers endpoint-trait registration, ViewSets, class-based views, raw
//! method-agnostic handlers, and per-route middleware attachment.

use super::ServerRouter;
use super::types::{FunctionRoute, RouteContractMetadata, ViewRoute};
use crate::routers::Route;
use reinhardt_core::endpoint::EndpointInfo;
use reinhardt_http::{
	Handler, RequestlessSyncHandler, RequestlessSyncHandlerAdapter, SyncHandler, SyncHandlerAdapter,
};
use reinhardt_middleware::Middleware;
#[cfg(feature = "viewsets")]
use reinhardt_views::viewsets::ViewSet;
use std::sync::Arc;

fn typed_contract_metadata(path: &str) -> RouteContractMetadata {
	RouteContractMetadata {
		handler: format!("view:{path}"),
		authentication: reinhardt_core::endpoint::AuthProtection::None,
		guard: None,
	}
}

impl ServerRouter {
	/// Register a ViewSet (DRF-style)
	///
	/// # Examples
	///
	/// ```rust,no_run
	/// use reinhardt_urls::routers::ServerRouter;
	/// # use reinhardt_views::viewsets::ViewSet;
	/// # use async_trait::async_trait;
	/// # struct UserViewSet;
	/// # #[async_trait]
	/// # impl ViewSet for UserViewSet {
	/// #     fn get_basename(&self) -> &str { "users" }
	/// #     async fn dispatch(&self, _req: reinhardt_http::Request, _action: reinhardt_views::viewsets::Action)
	/// #         -> reinhardt_core::exception::Result<reinhardt_http::Response> {
	/// #         Ok(reinhardt_http::Response::ok())
	/// #     }
	/// # }
	///
	/// let router = ServerRouter::new()
	///     .viewset("/users", UserViewSet);
	/// ```
	#[cfg(feature = "viewsets")]
	pub fn viewset<V: ViewSet + 'static>(mut self, prefix: &str, viewset: V) -> Self {
		self.invalidate_compiled_routes();
		self.viewsets.insert(prefix.to_string(), Arc::new(viewset));
		self
	}

	/// Same as [`Self::viewset`] at runtime, but carries a `PhantomData<M>`
	/// marker that the route resolver machinery recovers at expansion time
	/// to discover `#[action]`-decorated methods on the impl block `M`.
	///
	/// `M` is purely a name-bearing token. Users write
	/// `PhantomData::<MyViewSetImpl>` as the third argument. The bound is
	/// `M: 'static` so the marker's `std::any::type_name` is reachable for
	/// the marker→runtime bridge below.
	///
	/// Phase 5.1 of Issue #4507: in addition to delegating to [`Self::viewset`],
	/// this method calls [`reinhardt_views::viewsets::bridge_marker_actions_to_viewset`]
	/// to copy every action submitted under `type_name::<M>()` into the
	/// runtime-keyed `register_action(type_name::<V>(), ...)` slot, so the
	/// dispatcher's [`ViewSet::get_extra_actions`] lookup finds them under
	/// the concrete ViewSet's type name (not the marker's).
	///
	/// The marker-keyed submissions themselves are produced by a
	/// `#[ctor::ctor]` startup function emitted by `#[viewset(basename =
	/// "...")] impl M { #[action(...)] fn ... }` (the `ctor` path is the
	/// production registration mechanism today; the helper additionally
	/// drains an `inventory` collection for forward-compatibility once
	/// `const_type_name` stabilizes and `inventory::submit!` becomes usable
	/// for marker-keyed registrations). Because `#[ctor]` runs at process
	/// startup on non-wasm targets, the marker bridge is a no-op on wasm
	/// (gated by `#[cfg(not(target_family = "wasm"))]` at the emitter site).
	///
	/// Refs Issue #4507.
	#[cfg(feature = "viewsets")]
	pub fn viewset_with_actions<V, M>(
		self,
		prefix: &str,
		viewset: V,
		_marker: std::marker::PhantomData<M>,
	) -> Self
	where
		V: reinhardt_views::viewsets::ViewSet + 'static,
		M: 'static,
	{
		reinhardt_views::viewsets::bridge_marker_actions_to_viewset(
			std::any::type_name::<M>(),
			std::any::type_name::<V>(),
		);
		self.viewset(prefix, viewset)
	}

	/// Register an endpoint using EndpointInfo trait
	///
	/// This method accepts a factory function that returns a View type implementing
	/// both `EndpointInfo` and `Handler` traits. The path, HTTP method, and name
	/// are automatically extracted from the `EndpointInfo` implementation.
	///
	/// # Examples
	///
	/// ```rust,no_run
	/// use reinhardt_urls::routers::ServerRouter;
	/// # use reinhardt_core::endpoint::EndpointInfo;
	/// # use reinhardt_http::{Handler, Request, Response};
	/// # use hyper::Method;
	/// # struct ListUsers;
	/// # impl EndpointInfo for ListUsers {
	/// #     fn path() -> &'static str { "/users" }
	/// #     fn method() -> Method { Method::GET }
	/// #     fn name() -> &'static str { "list_users" }
	/// # }
	/// # #[async_trait::async_trait]
	/// # impl Handler for ListUsers {
	/// #     async fn handle(&self, _req: Request) -> Result<Response, reinhardt_http::Error> {
	/// #         Ok(Response::ok())
	/// #     }
	/// # }
	/// # fn list_users() -> ListUsers { ListUsers }
	///
	/// // Pass the function directly (no () needed)
	/// let router = ServerRouter::new()
	///     .endpoint(list_users);
	/// ```
	pub fn endpoint<F, E>(mut self, f: F) -> Self
	where
		F: FnOnce() -> E,
		E: EndpointInfo + Handler + 'static,
	{
		self.invalidate_compiled_routes();
		let view = f();
		let path = E::path().to_string();
		let method = E::method();
		let name = E::name().to_string();

		self.functions.push(FunctionRoute {
			path,
			method,
			handler: Arc::new(view),
			sync_handler: None,
			requestless_sync_handler: None,
			name: Some(name),
			metadata: RouteContractMetadata {
				handler: E::handler_identity().to_string(),
				authentication: E::auth_protection(),
				guard: E::guard_description().map(str::to_string),
			},
			middleware: Vec::new(),
		});
		self
	}

	/// Register a synchronous endpoint using the `EndpointInfo` trait.
	///
	/// This variant is for endpoints that can complete without awaiting I/O.
	/// Routers call the synchronous handler directly when no middleware is
	/// attached, avoiding the boxed future required by the async handler trait.
	pub fn endpoint_sync<F, E>(mut self, f: F) -> Self
	where
		F: FnOnce() -> E,
		E: EndpointInfo + SyncHandler + 'static,
	{
		self.invalidate_compiled_routes();
		let view = f();
		let path = E::path().to_string();
		let method = E::method();
		let name = E::name().to_string();
		let sync_handler: Arc<dyn SyncHandler> = Arc::new(view);
		let handler: Arc<dyn Handler> = Arc::new(SyncHandlerAdapter::new(sync_handler.clone()));

		self.functions.push(FunctionRoute {
			path,
			method,
			handler,
			sync_handler: Some(sync_handler),
			requestless_sync_handler: None,
			name: Some(name),
			metadata: RouteContractMetadata {
				handler: E::handler_identity().to_string(),
				authentication: E::auth_protection(),
				guard: E::guard_description().map(str::to_string),
			},
			middleware: Vec::new(),
		});
		self
	}

	/// Register a synchronous endpoint that does not inspect request state.
	///
	/// This is the lowest-overhead endpoint registration path. HTTP adapters can
	/// execute these routes before constructing a full request when the incoming
	/// request has no body and the route has no middleware or path parameters.
	pub fn endpoint_requestless_sync<F, E>(mut self, f: F) -> Self
	where
		F: FnOnce() -> E,
		E: EndpointInfo + RequestlessSyncHandler + 'static,
	{
		self.invalidate_compiled_routes();
		let view = f();
		let path = E::path().to_string();
		let method = E::method();
		let name = E::name().to_string();
		let requestless_handler: Arc<dyn RequestlessSyncHandler> = Arc::new(view);
		let adapter = Arc::new(RequestlessSyncHandlerAdapter::new(
			requestless_handler.clone(),
		));
		let handler: Arc<dyn Handler> = adapter.clone();
		let sync_handler: Arc<dyn SyncHandler> = adapter;

		self.functions.push(FunctionRoute {
			path,
			method,
			handler,
			sync_handler: Some(sync_handler),
			requestless_sync_handler: Some(requestless_handler),
			name: Some(name),
			metadata: RouteContractMetadata {
				handler: E::handler_identity().to_string(),
				authentication: E::auth_protection(),
				guard: E::guard_description().map(str::to_string),
			},
			middleware: Vec::new(),
		});
		self
	}

	/// Register a class-based view (Django-style)
	///
	/// # Examples
	///
	/// ```rust,no_run
	/// use reinhardt_urls::routers::ServerRouter;
	/// # use reinhardt_http::{Handler, {Request, Response, Result}};
	/// # use async_trait::async_trait;
	/// # struct ArticleListView;
	/// # #[async_trait]
	/// # impl Handler for ArticleListView {
	/// #     async fn handle(&self, _req: Request) -> Result<Response> {
	/// #         Ok(Response::ok())
	/// #     }
	/// # }
	///
	/// let view = ArticleListView;
	/// let router = ServerRouter::new()
	///     .view("/articles", view);
	/// ```
	pub fn view<V>(mut self, path: &str, view: V) -> Self
	where
		V: Handler + 'static,
	{
		self.invalidate_compiled_routes();
		self.views.push(ViewRoute {
			path: path.to_string(),
			handler: Arc::new(view),
			sync_handler: None,
			requestless_sync_handler: None,
			name: None,
			metadata: Some(typed_contract_metadata(path)),
			middleware: Vec::new(),
		});
		self
	}

	/// Register a named class-based view (Django-style with URL reversal)
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_urls::routers::ServerRouter;
	/// # use reinhardt_http::{Handler, {Request, Response, Result}};
	/// # use async_trait::async_trait;
	/// # struct ArticleListView;
	/// # #[async_trait]
	/// # impl Handler for ArticleListView {
	/// #     async fn handle(&self, _req: Request) -> Result<Response> {
	/// #         Ok(Response::ok())
	/// #     }
	/// # }
	///
	/// let view = ArticleListView;
	/// let mut router = ServerRouter::new()
	///     .with_namespace("articles")
	///     .view_named("/articles", "list", view);
	///
	/// router.register_all_routes();
	/// let url = router.reverse("articles:list", &[]).unwrap();
	/// assert_eq!(url, "/articles");
	/// ```
	#[deprecated(
		since = "0.2.0",
		note = "Use `#[get(\"/path\", name = \"name\")]` + `.endpoint()` instead"
	)]
	pub fn view_named<V>(mut self, path: &str, name: &str, view: V) -> Self
	where
		V: Handler + 'static,
	{
		self.invalidate_compiled_routes();
		self.views.push(ViewRoute {
			path: path.to_string(),
			handler: Arc::new(view),
			sync_handler: None,
			requestless_sync_handler: None,
			name: Some(name.to_string()),
			metadata: Some(typed_contract_metadata(path)),
			middleware: Vec::new(),
		});
		self
	}

	/// Register a handler directly (recommended method)
	///
	/// This method allows you to pass a handler directly without wrapping it in `Arc`.
	/// The `Arc` wrapping is handled internally for you.
	///
	/// # Examples
	///
	/// ```rust,no_run
	/// use reinhardt_urls::routers::ServerRouter;
	/// # use reinhardt_http::{Handler, {Request, Response, Result}};
	/// # use async_trait::async_trait;
	/// # struct CustomHandler;
	/// # #[async_trait]
	/// # impl Handler for CustomHandler {
	/// #     async fn handle(&self, _req: Request) -> Result<Response> {
	/// #         Ok(Response::ok())
	/// #     }
	/// # }
	///
	/// // No Arc::new() needed!
	/// let router = ServerRouter::new()
	///     .handler("/custom", CustomHandler);
	/// ```
	pub fn handler<H>(mut self, path: &str, handler: H) -> Self
	where
		H: Handler + 'static,
	{
		self.invalidate_compiled_routes();
		let route = Route::from_handler(path, handler);
		self.routes.push(route);
		self
	}

	/// Register a synchronous handler directly.
	///
	/// This is the raw-handler counterpart to [`Self::endpoint_sync`].
	pub fn handler_sync<H>(mut self, path: &str, handler: H) -> Self
	where
		H: SyncHandler + 'static,
	{
		self.invalidate_compiled_routes();
		let route = Route::from_sync_handler(path, handler);
		self.routes.push(route);
		self
	}

	/// Register a requestless synchronous handler directly.
	///
	/// This is the raw-handler counterpart to
	/// [`Self::endpoint_requestless_sync`].
	pub fn handler_requestless_sync<H>(mut self, path: &str, handler: H) -> Self
	where
		H: RequestlessSyncHandler + 'static,
	{
		self.invalidate_compiled_routes();
		let route = Route::from_requestless_sync_handler(path, handler);
		self.routes.push(route);
		self
	}

	/// Register a handler that is already wrapped in Arc (low-level API)
	///
	/// This is provided for cases where you already have an `Arc<dyn Handler>`.
	/// In most cases, you should use `handler()` instead.
	///
	/// # Examples
	///
	/// ```rust,no_run
	/// use reinhardt_urls::routers::ServerRouter;
	/// # use reinhardt_http::{Handler, {Request, Response, Result}};
	/// # use async_trait::async_trait;
	/// # use std::sync::Arc;
	/// # struct CustomHandler;
	/// # #[async_trait]
	/// # impl Handler for CustomHandler {
	/// #     async fn handle(&self, _req: Request) -> Result<Response> {
	/// #         Ok(Response::ok())
	/// #     }
	/// # }
	///
	/// let handler = Arc::new(CustomHandler);
	/// let router = ServerRouter::new()
	///     .handler_arc("/custom", handler);
	/// ```
	pub fn handler_arc(mut self, path: &str, handler: Arc<dyn Handler>) -> Self {
		self.invalidate_compiled_routes();
		let route = Route::new(path, handler);
		self.routes.push(route);
		self
	}

	/// Register an erased handler with caller-provided contract metadata.
	pub fn handler_arc_with_contract_metadata(
		mut self,
		path: &str,
		handler: Arc<dyn Handler>,
		metadata: RouteContractMetadata,
	) -> Self {
		self.invalidate_compiled_routes();
		self.routes
			.push(Route::new(path, handler).with_contract_metadata(metadata));
		self
	}

	/// Add middleware to the last registered endpoint, view, or raw handler route.
	///
	/// # Examples
	///
	/// ```rust,no_run
	/// use reinhardt_urls::routers::ServerRouter;
	/// use reinhardt_middleware::LoggingMiddleware;
	/// # use hyper::Method;
	/// # use reinhardt_core::endpoint::EndpointInfo;
	/// # use reinhardt_http::{Handler, Request, Response, Result};
	///
	/// # struct Health;
	/// # impl EndpointInfo for Health {
	/// #     fn path() -> &'static str { "/health" }
	/// #     fn method() -> Method { Method::GET }
	/// #     fn name() -> &'static str { "health" }
	/// # }
	/// # #[async_trait::async_trait]
	/// # impl Handler for Health {
	/// #     async fn handle(&self, _req: Request) -> Result<Response> { Ok(Response::ok()) }
	/// # }
	/// let router = ServerRouter::new()
	///     .endpoint(|| Health)
	///     .with_route_middleware(LoggingMiddleware::new());
	/// ```
	pub fn with_route_middleware<M: Middleware + 'static>(mut self, middleware: M) -> Self {
		self.invalidate_compiled_routes();
		let middleware = Arc::new(middleware);
		if let Some(route) = self.functions.last_mut() {
			route.middleware.push(middleware.clone());
		} else if let Some(route) = self.views.last_mut() {
			route.middleware.push(middleware.clone());
		} else if let Some(route) = self.routes.last_mut() {
			route.middleware.push(middleware);
		}
		self
	}
}

#[cfg(test)]
mod sync_handler_tests {
	use super::*;
	use async_trait::async_trait;
	use hyper::StatusCode;
	use reinhardt_http::{Request, Response, Result};

	struct HealthEndpoint;

	impl EndpointInfo for HealthEndpoint {
		fn path() -> &'static str {
			"/health"
		}

		fn method() -> hyper::Method {
			hyper::Method::GET
		}

		fn name() -> &'static str {
			"health"
		}
	}

	impl SyncHandler for HealthEndpoint {
		fn handle_sync(&self, _request: Request) -> Result<Response> {
			Ok(Response::ok().with_static_body(b"ok"))
		}
	}

	struct RequestlessHealthEndpoint;

	impl EndpointInfo for RequestlessHealthEndpoint {
		fn path() -> &'static str {
			"/requestless-health"
		}

		fn method() -> hyper::Method {
			hyper::Method::GET
		}

		fn name() -> &'static str {
			"requestless_health"
		}
	}

	impl RequestlessSyncHandler for RequestlessHealthEndpoint {
		fn handle_requestless_sync(&self) -> Result<Response> {
			Ok(Response::ok().with_static_body(b"ok"))
		}
	}

	struct PassThroughMiddleware;

	#[async_trait]
	impl Middleware for PassThroughMiddleware {
		async fn process(&self, request: Request, next: Arc<dyn Handler>) -> Result<Response> {
			let mut response = next.handle(request).await?;
			response.status = StatusCode::ACCEPTED;
			Ok(response)
		}
	}

	fn request(path: &str) -> Request {
		Request::builder()
			.method(hyper::Method::GET)
			.uri(path)
			.build()
			.expect("request should build")
	}

	#[tokio::test]
	async fn endpoint_sync_dispatches_synchronous_handler() {
		// Arrange
		let router = ServerRouter::new().endpoint_sync(|| HealthEndpoint);

		// Act
		let response = router
			.dispatch(request("/health"))
			.await
			.expect("route should dispatch");

		// Assert
		assert_eq!(response.status, StatusCode::OK);
		assert_eq!(response.body.as_ref(), b"ok");
	}

	#[test]
	fn endpoint_sync_dispatches_through_synchronous_router_path() {
		// Arrange
		let router = ServerRouter::new().endpoint_sync(|| HealthEndpoint);

		// Act
		let response = router
			.try_dispatch_sync(request("/health"))
			.expect("sync route should use the synchronous dispatch path")
			.expect("route should dispatch");

		// Assert
		assert_eq!(response.status, StatusCode::OK);
		assert_eq!(response.body.as_ref(), b"ok");
	}

	#[test]
	fn endpoint_requestless_sync_dispatches_without_request() {
		// Arrange
		let router = ServerRouter::new().endpoint_requestless_sync(|| RequestlessHealthEndpoint);

		// Act
		let response = router
			.try_dispatch_requestless_sync("/requestless-health", &hyper::Method::GET)
			.expect("requestless route should use the requestless dispatch path")
			.expect("route should dispatch");

		// Assert
		assert_eq!(response.status, StatusCode::OK);
		assert_eq!(response.body.as_ref(), b"ok");
	}

	#[test]
	fn endpoint_requestless_sync_with_middleware_declines_requestless_path() {
		// Arrange
		let router = ServerRouter::new()
			.with_middleware(PassThroughMiddleware)
			.endpoint_requestless_sync(|| RequestlessHealthEndpoint);

		// Act & Assert
		assert!(
			router
				.try_dispatch_requestless_sync("/requestless-health", &hyper::Method::GET)
				.is_none()
		);
	}

	#[test]
	fn endpoint_sync_with_middleware_declines_synchronous_router_path() {
		// Arrange
		let router = ServerRouter::new()
			.with_middleware(PassThroughMiddleware)
			.endpoint_sync(|| HealthEndpoint);

		// Act & Assert
		assert!(router.try_dispatch_sync(request("/health")).is_none());
	}

	#[tokio::test]
	async fn handler_sync_still_runs_through_middleware_chain() {
		// Arrange
		let router = ServerRouter::new()
			.with_middleware(PassThroughMiddleware)
			.handler_sync("/health", HealthEndpoint);

		// Act
		let response = router
			.dispatch(request("/health"))
			.await
			.expect("route should dispatch");

		// Assert
		assert_eq!(response.status, StatusCode::ACCEPTED);
		assert_eq!(response.body.as_ref(), b"ok");
	}
}

#[cfg(all(test, feature = "viewsets"))]
mod viewset_with_actions_tests {
	use super::*;
	use async_trait::async_trait;
	use reinhardt_http::{Request, Response, Result};
	use reinhardt_views::viewsets::{Action, ViewSet};
	use rstest::rstest;
	use std::marker::PhantomData;

	/// Minimal `ViewSet` fixture for parity tests between `viewset` and
	/// `viewset_with_actions`. The dispatch body is irrelevant — these tests
	/// only inspect what routes get registered.
	#[derive(Debug, Clone)]
	struct DummyViewSet {
		basename: String,
	}

	#[async_trait]
	impl ViewSet for DummyViewSet {
		fn get_basename(&self) -> &str {
			&self.basename
		}

		async fn dispatch(&self, _request: Request, _action: Action) -> Result<Response> {
			Ok(Response::ok())
		}
	}

	/// Marker type the route resolver machinery recovers at expansion
	/// time. It carries no runtime state.
	struct DummyImpl;

	#[rstest]
	fn viewset_with_actions_is_equivalent_to_viewset() {
		// Arrange
		let mut router_a = ServerRouter::new().viewset(
			"/users",
			DummyViewSet {
				basename: "users".to_string(),
			},
		);
		let mut router_b = ServerRouter::new().viewset_with_actions(
			"/users",
			DummyViewSet {
				basename: "users".to_string(),
			},
			PhantomData::<DummyImpl>,
		);

		// Act
		let _ = router_a.register_all_routes();
		let _ = router_b.register_all_routes();
		let routes_a = router_a.get_all_routes();
		let routes_b = router_b.get_all_routes();

		// Assert
		assert_eq!(routes_a, routes_b);
	}
}
