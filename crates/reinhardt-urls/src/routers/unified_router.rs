//! Unified Router with closure-based server and client configuration.
//!
//! This module provides [`UnifiedRouter`], a unified entry point for configuring
//! both server-side HTTP routing and client-side SPA routing.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────┐
//! │             UnifiedRouter               │
//! │  ┌─────────────┐  ┌─────────────────┐   │
//! │  │ClientRouter │  │  ServerRouter   │   │
//! │  │ (WASM/SPA)  │  │ (HTTP/Backend)  │   │
//! │  └─────────────┘  └─────────────────┘   │
//! └─────────────────────────────────────────┘
//! ```
//!
//! # Example
//!
//! ```rust,ignore
//! use reinhardt_urls::routers::UnifiedRouter;
//! use reinhardt_core::page::Page;
//! use hyper::Method;
//!
//! let router = UnifiedRouter::new()
//!     .server(|s| s
//!         .with_prefix("/api/v1")
//!         .function("/users", Method::GET, list_users)
//!         .function("/users", Method::POST, create_user))
//!     .client(|c| c
//!         .route("/", || home_page())
//!         .route_path("/users/{id}", |Path(id): Path<i64>| user_page(id)));
//! ```
//!
//! # Feature Flags
//!
//! - When `client-router` feature is **enabled**: Full [`UnifiedRouter`] with both
//!   `.server()` and `.client()` methods available.
//! - When `client-router` feature is **disabled**: Server-only [`UnifiedRouter`] with
//!   only `.server()` method available.

#[cfg(native)]
use crate::routers::server_router::ServerRouter;

#[cfg(feature = "client-router")]
use crate::routers::client_router::ClientRouter;

#[cfg(native)]
use hyper::Method;
#[cfg(native)]
use reinhardt_core::exception::Result;
#[cfg(native)]
use reinhardt_di::InjectionContext;
#[cfg(native)]
use reinhardt_http::{Request, Response};
#[cfg(native)]
use reinhardt_middleware::Middleware;
#[cfg(native)]
use std::future::Future;
#[cfg(native)]
use std::sync::Arc;

// ============================================================================
// client-router feature ENABLED
// ============================================================================

/// Unified router combining server and client routing capabilities.
///
/// This struct provides a unified interface for configuring both:
/// - **Server-side routes**: HTTP methods, middleware, DI, ViewSets
/// - **Client-side routes**: SPA navigation, history API, [`Page`] rendering
///
/// # Example
///
/// ```rust,ignore
/// use reinhardt_urls::routers::UnifiedRouter;
/// use reinhardt_core::page::Page;
///
/// let router = UnifiedRouter::new()
///     .server(|s| s.function("/api/health", Method::GET, health_handler))
///     .client(|c| c.route("/", || home_page()));
/// ```
///
/// [`Page`]: reinhardt_core::page::Page
#[cfg(all(feature = "client-router", native))]
pub struct UnifiedRouter {
	server: ServerRouter,
	client: ClientRouter,
	/// WebSocket router for `urls.ws().<app>().<handler>()` URL resolution.
	pub websocket: reinhardt_core::ws::WebSocketRouter,
	di_registrations: reinhardt_di::DiRegistrationList,
	#[cfg(feature = "streaming")]
	streaming_handlers: Vec<reinhardt_streaming::StreamingHandlerRegistration>,
}

#[cfg(all(feature = "client-router", native))]
impl UnifiedRouter {
	/// Creates a new `UnifiedRouter` with default server and client routers.
	pub fn new() -> Self {
		Self {
			server: ServerRouter::new(),
			client: ClientRouter::new(),
			websocket: reinhardt_core::ws::WebSocketRouter::new(),
			di_registrations: reinhardt_di::DiRegistrationList::new(),
			#[cfg(feature = "streaming")]
			streaming_handlers: Vec::new(),
		}
	}

	/// Configure server-side routing with a closure.
	///
	/// The closure receives a [`ServerRouter`] and should return a configured router.
	///
	/// # Example
	///
	/// ```rust,ignore
	/// let router = UnifiedRouter::new()
	///     .server(|s| s
	///         .with_prefix("/api")
	///         .function("/users", Method::GET, list_users));
	/// ```
	pub fn server<F>(mut self, f: F) -> Self
	where
		F: FnOnce(ServerRouter) -> ServerRouter,
	{
		self.server = f(self.server);
		self
	}

	/// Configure client-side routing with a closure.
	///
	/// The closure receives a [`ClientRouter`] and should return a configured router.
	///
	/// # Example
	///
	/// ```rust,ignore
	/// let router = UnifiedRouter::new()
	///     .client(|c| c
	///         .route("/", || home_page())
	///         .route_path("/users/{id}", |Path(id): Path<i64>| user_page(id)));
	/// ```
	pub fn client<F>(mut self, f: F) -> Self
	where
		F: FnOnce(ClientRouter) -> ClientRouter,
	{
		self.client = f(self.client);
		self
	}

	/// Returns a reference to the server router.
	pub fn server_ref(&self) -> &ServerRouter {
		&self.server
	}

	/// Returns a mutable reference to the server router.
	pub fn server_mut(&mut self) -> &mut ServerRouter {
		&mut self.server
	}

	/// Returns a reference to the client router.
	pub fn client_ref(&self) -> &ClientRouter {
		&self.client
	}

	/// Returns a mutable reference to the client router.
	pub fn client_mut(&mut self) -> &mut ClientRouter {
		&mut self.client
	}

	/// Configure WebSocket routing with a closure.
	///
	/// Parallel to `server()` and `client()`. The registered consumers are
	/// available via `urls.ws().<app>().<handler>()` in `ResolvedUrls`.
	///
	/// # Example
	///
	/// ```rust,ignore
	/// let router = UnifiedRouter::new()
	///     .websocket(|ws| ws
	///         .consumer(chat_ws)
	///         .consumer(notif_ws));
	/// ```
	pub fn websocket<F>(mut self, f: F) -> Self
	where
		F: FnOnce(reinhardt_core::ws::WebSocketRouter) -> reinhardt_core::ws::WebSocketRouter,
	{
		self.websocket = f(self.websocket);
		self
	}

	/// Returns a reference to the WebSocket router.
	pub fn websocket_ref(&self) -> &reinhardt_core::ws::WebSocketRouter {
		&self.websocket
	}

	/// Apply or stash deferred DI registrations.
	///
	/// If the server router already has a DI context, registrations are applied
	/// directly to its singleton scope. Otherwise they are stashed globally
	/// for later application (e.g., by the `runall` command).
	fn flush_di_registrations(&mut self) {
		if self.di_registrations.is_empty() {
			return;
		}
		let registrations = std::mem::take(&mut self.di_registrations);
		match self
			.server
			.di_context()
			.map(|ctx| Arc::clone(ctx.singleton_scope()))
		{
			Some(scope) => registrations.apply_to(&scope),
			None => crate::routers::register_di_registrations(registrations),
		}
	}

	/// Consumes the router and returns the server router.
	///
	/// If this router has a DI context, deferred DI registrations are applied
	/// directly to its singleton scope. Otherwise they are stashed in the
	/// global registry for later application by the server.
	pub fn into_server(mut self) -> ServerRouter {
		self.flush_di_registrations();
		let errors = self.server.register_all_routes();
		for error in &errors {
			tracing::warn!("{}", error);
		}
		self.server
	}

	/// Consumes the router and returns the client router.
	pub fn into_client(mut self) -> ClientRouter {
		self.flush_di_registrations();
		self.client
	}

	/// Consumes the router and returns both parts.
	pub fn into_parts(mut self) -> (ServerRouter, ClientRouter) {
		self.flush_di_registrations();
		let errors = self.server.register_all_routes();
		for error in &errors {
			tracing::warn!("{}", error);
		}
		(self.server, self.client)
	}

	/// Registers server router globally and returns client router.
	///
	/// This is a convenience method for full-stack applications that need to:
	/// 1. Register the server router globally for HTTP request handling
	/// 2. Keep the client router for SPA navigation
	///
	/// # Example
	///
	/// ```rust,ignore
	/// let client = UnifiedRouter::new()
	///     .server(|s| s.function("/api/data", Method::GET, handler))
	///     .client(|c| c.route("/", || home_page()))
	///     .register_globally();
	///
	/// // Server router is now globally registered
	/// // Client router is returned for SPA use
	/// ```
	pub fn register_globally(self) -> ClientRouter {
		let (server, client) = self.into_parts();
		let reverser = client.to_reverser();
		crate::routers::register_router(server);
		crate::routers::client_router::register_client_reverser(reverser);
		client
	}

	/// Attach deferred DI registrations to this router.
	///
	/// When the router is consumed, these registrations are applied directly
	/// to the DI context's singleton scope if one has been set via
	/// [`with_di_context`](Self::with_di_context). Otherwise they are stashed
	/// globally for later application (e.g., by the `runall` command).
	pub fn with_di_registrations(mut self, list: reinhardt_di::DiRegistrationList) -> Self {
		self.di_registrations.merge(list);
		self
	}

	// ========================================================================
	// Convenience delegations to ServerRouter
	// ========================================================================

	/// Set URL prefix for server router.
	///
	/// This is a convenience method that delegates to [`ServerRouter::with_prefix`].
	pub fn with_prefix(mut self, prefix: impl Into<String>) -> Self {
		self.server = self.server.with_prefix(prefix);
		self
	}

	/// Set namespace for both server and client routers.
	///
	/// Delegates to [`ServerRouter::with_namespace`] and
	/// [`ClientRouter::with_namespace`] so that server-side URL resolvers
	/// and client-side named route keys are both prefixed consistently
	/// with `"<namespace>:"`. Fixes #3726.
	pub fn with_namespace(mut self, namespace: impl Into<String>) -> Self {
		let ns: String = namespace.into();
		// Borrow for the client (accepts `&str`) first, then move the owned
		// `String` into the server (accepts `impl Into<String>`) to avoid a
		// redundant `String` allocation.
		self.client = self.client.with_namespace(&ns);
		self.server = self.server.with_namespace(ns);
		self
	}

	/// Set DI context for server router.
	///
	/// This is a convenience method that delegates to [`ServerRouter::with_di_context`].
	pub fn with_di_context(mut self, ctx: Arc<InjectionContext>) -> Self {
		self.server = self.server.with_di_context(ctx);
		self
	}

	/// Add middleware to server router.
	///
	/// This is a convenience method that delegates to [`ServerRouter::with_middleware`].
	pub fn with_middleware<M: Middleware + 'static>(mut self, middleware: M) -> Self {
		self.server = self.server.with_middleware(middleware);
		self
	}

	/// Exclude a URL path from the most recently added server middleware.
	///
	/// This is a convenience method that delegates to [`ServerRouter::exclude`].
	pub fn exclude(mut self, pattern: &str) -> Self {
		self.server = self.server.exclude(pattern);
		self
	}

	/// Mount a child server router on this router.
	///
	/// This is a convenience method that delegates to [`ServerRouter::mount`].
	pub fn mount(mut self, prefix: &str, child: ServerRouter) -> Self {
		self.server = self.server.mount(prefix, child);
		self
	}

	/// Mount a child UnifiedRouter on this router.
	///
	/// Extracts the server router from the child and mounts it.
	pub fn mount_unified(self, prefix: &str, child: UnifiedRouter) -> Self {
		self.mount(prefix, child.server)
	}

	/// Mount streaming handlers (producers and consumers) on this router.
	///
	/// Registrations are stored on the router for Phase 3 worker startup.
	/// Consumer worker startup is deferred to a later server startup phase.
	#[cfg(feature = "streaming")]
	pub fn mount_streaming(
		mut self,
		router: reinhardt_streaming::StreamingRouter,
	) -> Self {
		self.streaming_handlers.extend(router.into_handlers());
		self
	}

	/// Register an endpoint on server router.
	///
	/// This is a convenience method that delegates to [`ServerRouter::endpoint`].
	pub fn endpoint<F, E>(mut self, f: F) -> Self
	where
		F: FnOnce() -> E,
		E: reinhardt_core::endpoint::EndpointInfo + reinhardt_http::Handler + 'static,
	{
		self.server = self.server.endpoint(f);
		self
	}

	/// Register a function-based route on server router.
	///
	/// This is a convenience method that delegates to [`ServerRouter::function`].
	pub fn function<F, Fut>(mut self, path: &str, method: Method, func: F) -> Self
	where
		F: Fn(Request) -> Fut + Send + Sync + 'static,
		Fut: Future<Output = Result<Response>> + Send + 'static,
	{
		self.server = self.server.function(path, method, func);
		self
	}

	/// Register a named function-based route on server router.
	///
	/// This is a convenience method that delegates to [`ServerRouter::function_named`].
	#[deprecated(
		since = "0.2.0",
		note = "Use `#[get(\"/path\", name = \"name\")]` + `.endpoint()` instead"
	)]
	pub fn function_named<F, Fut>(mut self, path: &str, method: Method, name: &str, func: F) -> Self
	where
		F: Fn(Request) -> Fut + Send + Sync + 'static,
		Fut: Future<Output = Result<Response>> + Send + 'static,
	{
		#[allow(deprecated)]
		{
			self.server = self.server.function_named(path, method, name, func);
		}
		self
	}
}

#[cfg(all(feature = "client-router", native))]
impl std::fmt::Debug for UnifiedRouter {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("UnifiedRouter")
			.field("server", &self.server)
			.field("client", &self.client)
			.field("di_registrations", &self.di_registrations)
			.finish()
	}
}

#[cfg(all(feature = "client-router", native))]
impl Default for UnifiedRouter {
	fn default() -> Self {
		Self::new()
	}
}

// Note: Handler is NOT implemented for UnifiedRouter when client-router is enabled
// because ClientRouter contains non-Sync types (Rc<RefCell>).
// For server-side HTTP handling, use ServerRouter directly or extract it via into_parts().

// ============================================================================
// client-router feature DISABLED
// ============================================================================

/// Unified router for server-side routing only.
///
/// When the `client-router` feature is disabled, this struct provides
/// server-side routing configuration only.
///
/// # Example
///
/// ```rust,ignore
/// use reinhardt_urls::routers::UnifiedRouter;
///
/// let router = UnifiedRouter::new()
///     .server(|s| s.function("/api/health", Method::GET, health_handler));
/// ```
#[cfg(not(feature = "client-router"))]
pub struct UnifiedRouter {
	server: ServerRouter,
	di_registrations: reinhardt_di::DiRegistrationList,
	#[cfg(feature = "streaming")]
	streaming_handlers: Vec<reinhardt_streaming::StreamingHandlerRegistration>,
}

#[cfg(not(feature = "client-router"))]
impl UnifiedRouter {
	/// Creates a new `UnifiedRouter` with default server router.
	pub fn new() -> Self {
		Self {
			server: ServerRouter::new(),
			di_registrations: reinhardt_di::DiRegistrationList::new(),
			#[cfg(feature = "streaming")]
			streaming_handlers: Vec::new(),
		}
	}

	/// Configure server-side routing with a closure.
	pub fn server<F>(mut self, f: F) -> Self
	where
		F: FnOnce(ServerRouter) -> ServerRouter,
	{
		self.server = f(self.server);
		self
	}

	/// Returns a reference to the server router.
	pub fn server_ref(&self) -> &ServerRouter {
		&self.server
	}

	/// Returns a mutable reference to the server router.
	pub fn server_mut(&mut self) -> &mut ServerRouter {
		&mut self.server
	}

	/// Apply or stash deferred DI registrations.
	///
	/// If the server router already has a DI context, registrations are applied
	/// directly to its singleton scope. Otherwise they are stashed globally
	/// for later application (e.g., by the `runall` command).
	fn flush_di_registrations(&mut self) {
		if self.di_registrations.is_empty() {
			return;
		}
		let registrations = std::mem::take(&mut self.di_registrations);
		match self
			.server
			.di_context()
			.map(|ctx| Arc::clone(ctx.singleton_scope()))
		{
			Some(scope) => registrations.apply_to(&scope),
			None => crate::routers::register_di_registrations(registrations),
		}
	}

	/// Consumes the router and returns the server router.
	///
	/// If this router has a DI context, deferred DI registrations are applied
	/// directly to its singleton scope. Otherwise they are stashed in the
	/// global registry for later application by the server.
	pub fn into_server(mut self) -> ServerRouter {
		self.flush_di_registrations();
		let errors = self.server.register_all_routes();
		for error in &errors {
			tracing::warn!("{}", error);
		}
		self.server
	}

	/// Registers server router globally.
	pub fn register_globally(mut self) {
		self.flush_di_registrations();
		crate::routers::register_router(self.server);
	}

	/// Attach deferred DI registrations to this router.
	///
	/// When the router is consumed, these registrations are applied directly
	/// to the DI context's singleton scope if one has been set via
	/// [`with_di_context`](Self::with_di_context). Otherwise they are stashed
	/// globally for later application (e.g., by the `runall` command).
	pub fn with_di_registrations(mut self, list: reinhardt_di::DiRegistrationList) -> Self {
		self.di_registrations.merge(list);
		self
	}

	// ========================================================================
	// Convenience delegations to ServerRouter
	// ========================================================================

	/// Set URL prefix for server router.
	pub fn with_prefix(mut self, prefix: impl Into<String>) -> Self {
		self.server = self.server.with_prefix(prefix);
		self
	}

	/// Set namespace for server router.
	pub fn with_namespace(mut self, namespace: impl Into<String>) -> Self {
		self.server = self.server.with_namespace(namespace);
		self
	}

	/// Set DI context for server router.
	pub fn with_di_context(mut self, ctx: Arc<InjectionContext>) -> Self {
		self.server = self.server.with_di_context(ctx);
		self
	}

	/// Add middleware to server router.
	pub fn with_middleware<M: Middleware + 'static>(mut self, middleware: M) -> Self {
		self.server = self.server.with_middleware(middleware);
		self
	}

	/// Mount a child server router on this router.
	pub fn mount(mut self, prefix: &str, child: ServerRouter) -> Self {
		self.server = self.server.mount(prefix, child);
		self
	}

	/// Mount a child UnifiedRouter on this router.
	pub fn mount_unified(self, prefix: &str, child: UnifiedRouter) -> Self {
		self.mount(prefix, child.server)
	}

	/// Mount streaming handlers on this router.
	#[cfg(feature = "streaming")]
	pub fn mount_streaming(
		mut self,
		router: reinhardt_streaming::StreamingRouter,
	) -> Self {
		self.streaming_handlers.extend(router.into_handlers());
		self
	}

	/// Register an endpoint on server router.
	pub fn endpoint<F, E>(mut self, f: F) -> Self
	where
		F: FnOnce() -> E,
		E: reinhardt_core::endpoint::EndpointInfo + reinhardt_http::Handler + 'static,
	{
		self.server = self.server.endpoint(f);
		self
	}

	/// Register a function-based route on server router.
	pub fn function<F, Fut>(mut self, path: &str, method: Method, func: F) -> Self
	where
		F: Fn(Request) -> Fut + Send + Sync + 'static,
		Fut: Future<Output = Result<Response>> + Send + 'static,
	{
		self.server = self.server.function(path, method, func);
		self
	}

	/// Register a named function-based route on server router.
	#[deprecated(
		since = "0.2.0",
		note = "Use `#[get(\"/path\", name = \"name\")]` + `.endpoint()` instead"
	)]
	pub fn function_named<F, Fut>(mut self, path: &str, method: Method, name: &str, func: F) -> Self
	where
		F: Fn(Request) -> Fut + Send + Sync + 'static,
		Fut: Future<Output = Result<Response>> + Send + 'static,
	{
		#[allow(deprecated)]
		{
			self.server = self.server.function_named(path, method, name, func);
		}
		self
	}
}

#[cfg(not(feature = "client-router"))]
impl std::fmt::Debug for UnifiedRouter {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("UnifiedRouter")
			.field("server", &self.server)
			.field("di_registrations", &self.di_registrations)
			.finish()
	}
}

#[cfg(not(feature = "client-router"))]
impl Default for UnifiedRouter {
	fn default() -> Self {
		Self::new()
	}
}

/// Handler implementation delegates to the inner ServerRouter.
#[cfg(not(feature = "client-router"))]
#[async_trait::async_trait]
impl reinhardt_http::Handler for UnifiedRouter {
	async fn handle(&self, request: Request) -> Result<Response> {
		self.server.handle(request).await
	}
}

// ============================================================================
// WASM target with client-router feature
// ============================================================================

/// Stub type used in place of `ServerRouter` on WASM targets.
///
/// On WASM, server-side routing is not available. This stub allows
/// `UnifiedRouter::server()` closures to compile by accepting and
/// ignoring the server configuration closure.
#[cfg(wasm)]
pub struct ServerRouterStub;

/// Unified router for WASM targets with client-side routing.
///
/// On WASM, only client-side routing is available. The `.server()` method
/// accepts a closure but discards its result, allowing shared route
/// definitions to compile on both server and client.
#[cfg(all(wasm, feature = "client-router"))]
pub struct UnifiedRouter {
	client: ClientRouter,
}

#[cfg(all(wasm, feature = "client-router"))]
impl UnifiedRouter {
	/// Creates a new `UnifiedRouter` with a default client router.
	pub fn new() -> Self {
		Self {
			client: ClientRouter::new(),
		}
	}

	/// Accept and discard server-side routing configuration.
	///
	/// On WASM, server routing is not available. The closure is called
	/// with a [`ServerRouterStub`] but its result is discarded.
	pub fn server<F>(self, _f: F) -> Self
	where
		F: FnOnce(ServerRouterStub) -> ServerRouterStub,
	{
		self
	}

	/// Configure client-side routing with a closure.
	pub fn client<F>(mut self, f: F) -> Self
	where
		F: FnOnce(ClientRouter) -> ClientRouter,
	{
		self.client = f(self.client);
		self
	}

	/// Returns a reference to the client router.
	pub fn client_ref(&self) -> &ClientRouter {
		&self.client
	}

	/// Returns a mutable reference to the client router.
	pub fn client_mut(&mut self) -> &mut ClientRouter {
		&mut self.client
	}

	/// Consumes the router and returns the client router.
	pub fn into_client(self) -> ClientRouter {
		self.client
	}

	/// Registers client reverser globally and returns client router.
	pub fn register_globally(self) -> ClientRouter {
		let reverser = self.client.to_reverser();
		crate::routers::client_router::register_client_reverser(reverser);
		self.client
	}

	/// Mount a child UnifiedRouter on this router (client routes only).
	pub fn mount_unified(mut self, _prefix: &str, child: UnifiedRouter) -> Self {
		// Merge child client routes into parent
		self.client = self.client.merge(child.client);
		self
	}

	/// No-op on WASM: streaming is only available on native targets.
	#[cfg(feature = "streaming")]
	pub fn mount_streaming(
		self,
		_router: reinhardt_streaming::StreamingRouter,
	) -> Self {
		self
	}

	/// No-op on WASM - server prefix is not applicable.
	pub fn with_prefix(self, _prefix: impl Into<String>) -> Self {
		self
	}

	/// Set namespace for the client router.
	///
	/// On WASM, only the client router is present; server-side namespacing
	/// does not apply. Propagates to [`ClientRouter::with_namespace`] so
	/// that named route keys are prefixed with `"<namespace>:"`. Fixes #3726.
	pub fn with_namespace(mut self, namespace: impl Into<String>) -> Self {
		let ns: String = namespace.into();
		self.client = self.client.with_namespace(&ns);
		self
	}
}

#[cfg(all(wasm, feature = "client-router"))]
impl std::fmt::Debug for UnifiedRouter {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("UnifiedRouter")
			.field("client", &self.client)
			.finish()
	}
}

#[cfg(all(wasm, feature = "client-router"))]
impl Default for UnifiedRouter {
	fn default() -> Self {
		Self::new()
	}
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
#[allow(deprecated)]
mod tests {
	use super::*;
	#[cfg(feature = "client-router")]
	use reinhardt_core::page::Page;

	#[test]
	fn test_unified_router_new() {
		let router = UnifiedRouter::new();
		// Should have default server router
		assert_eq!(router.server_ref().prefix(), "");
	}

	#[test]
	fn test_unified_router_server_closure() {
		let router = UnifiedRouter::new().server(|s| s.with_prefix("/api").with_namespace("v1"));

		assert_eq!(router.server_ref().prefix(), "/api");
		assert_eq!(router.server_ref().namespace(), Some("v1"));
	}

	#[test]
	fn test_unified_router_convenience_methods() {
		let router = UnifiedRouter::new()
			.with_prefix("/api")
			.with_namespace("v1");

		assert_eq!(router.server_ref().prefix(), "/api");
		assert_eq!(router.server_ref().namespace(), Some("v1"));
	}

	#[cfg(feature = "client-router")]
	#[test]
	fn test_unified_router_client_closure() {
		let router = UnifiedRouter::new().client(|c| c.route("/", || Page::Empty));

		assert_eq!(router.client_ref().route_count(), 1);
	}

	#[cfg(all(feature = "client-router", native))]
	#[test]
	fn unified_with_namespace_propagates_to_client() {
		// Arrange: routes are added first, namespace applied after (matches
		// the call pattern generated by `#[url_patterns]`).
		let router = UnifiedRouter::new()
			.client(|c| c.named_route("login", "/login/", || Page::Empty))
			.with_namespace("app");

		// Act & Assert
		assert!(
			router.client_ref().has_route("app:login"),
			"client-side named route should be namespaced by UnifiedRouter::with_namespace"
		);
		assert!(
			!router.client_ref().has_route("login"),
			"unprefixed name should no longer resolve after with_namespace"
		);
	}

	#[cfg(all(wasm, feature = "client-router"))]
	#[test]
	fn unified_wasm_with_namespace_propagates_to_client() {
		// Arrange
		let router = UnifiedRouter::new()
			.client(|c| c.named_route("login", "/login/", || Page::Empty))
			.with_namespace("app");

		// Act & Assert
		assert!(
			router.client_ref().has_route("app:login"),
			"WASM UnifiedRouter::with_namespace must propagate to ClientRouter"
		);
		assert!(
			!router.client_ref().has_route("login"),
			"unprefixed name should no longer resolve after with_namespace on WASM"
		);
	}

	#[cfg(feature = "client-router")]
	#[test]
	fn test_unified_router_into_parts() {
		let router = UnifiedRouter::new()
			.server(|s| s.with_prefix("/api"))
			.client(|c| c.route("/", || Page::Empty));

		let (server, client) = router.into_parts();
		assert_eq!(server.prefix(), "/api");
		assert_eq!(client.route_count(), 1);
	}

	#[cfg(feature = "client-router")]
	#[test]
	fn test_unified_router_into_server() {
		let router = UnifiedRouter::new().server(|s| s.with_prefix("/api"));

		let server = router.into_server();
		assert_eq!(server.prefix(), "/api");
	}

	#[cfg(feature = "client-router")]
	#[test]
	fn test_unified_router_into_client() {
		let router = UnifiedRouter::new().client(|c| c.route("/", || Page::Empty));

		let client = router.into_client();
		assert_eq!(client.route_count(), 1);
	}

	mod flush_di_registrations {
		use super::*;
		use reinhardt_di::{DiRegistrationList, InjectionContext, SingletonScope};
		use rstest::rstest;
		use std::sync::Arc;

		#[rstest]
		fn applies_registrations_to_di_context_singleton_scope() {
			// Arrange
			let singleton_scope = Arc::new(SingletonScope::new());
			let di_ctx = Arc::new(InjectionContext::builder(Arc::clone(&singleton_scope)).build());

			let mut registrations = DiRegistrationList::new();
			registrations.register(42i32);

			// Act
			let _server = UnifiedRouter::new()
				.with_di_registrations(registrations)
				.with_di_context(di_ctx)
				.into_server();

			// Assert
			let value = singleton_scope
				.get::<i32>()
				.expect("i32 should be registered");
			assert_eq!(*value, 42);
		}

		#[rstest]
		fn applies_registrations_regardless_of_builder_order() {
			// Arrange
			let singleton_scope = Arc::new(SingletonScope::new());
			let di_ctx = Arc::new(InjectionContext::builder(Arc::clone(&singleton_scope)).build());

			let mut registrations = DiRegistrationList::new();
			registrations.register(99u64);

			// Act: with_di_context BEFORE with_di_registrations
			let _server = UnifiedRouter::new()
				.with_di_context(di_ctx)
				.with_di_registrations(registrations)
				.into_server();

			// Assert
			let value = singleton_scope
				.get::<u64>()
				.expect("u64 should be registered");
			assert_eq!(*value, 99);
		}

		#[rstest]
		#[serial_test::serial(global_di)]
		fn stashes_globally_when_no_di_context() {
			// Arrange
			let mut registrations = DiRegistrationList::new();
			registrations.register(7u8);

			// Act
			let _server = UnifiedRouter::new()
				.with_di_registrations(registrations)
				.into_server();

			// Assert: registrations stashed globally
			let taken = crate::routers::take_di_registrations();
			assert!(taken.is_some(), "registrations should be stashed globally");
		}
	}

	mod debug_impl {
		use super::*;
		use rstest::rstest;
		use std::sync::Arc;

		#[rstest]
		fn unified_router_implements_debug() {
			let router = UnifiedRouter::new().with_prefix("/api");
			let debug_output = format!("{:?}", router);
			assert!(debug_output.contains("UnifiedRouter"));
			assert!(debug_output.contains("ServerRouter"));
		}

		#[rstest]
		fn arc_try_unwrap_with_expect() {
			// This is the primary use case from #3391:
			// Arc::try_unwrap().expect() requires Debug on the error type
			let router = Arc::new(UnifiedRouter::new());
			let unwrapped = Arc::try_unwrap(router).expect("should have single ref");
			assert_eq!(unwrapped.server_ref().prefix(), "");
		}
	}

	mod route_registration {
		use super::*;
		use hyper::Method;
		use reinhardt_http::{Request, Response, Result};
		use rstest::rstest;

		async fn dummy_handler(_req: Request) -> Result<Response> {
			Ok(Response::ok())
		}

		#[rstest]
		fn into_server_registers_routes_for_reverse() {
			// Arrange
			let router = UnifiedRouter::new().server(|s| {
				s.with_namespace("api").function_named(
					"/health",
					Method::GET,
					"health",
					dummy_handler,
				)
			});

			// Act
			let server = router.into_server();

			// Assert
			let url = server.reverse("api:health", &[]);
			assert_eq!(url, Some("/health".to_string()));
		}

		#[cfg(feature = "client-router")]
		#[rstest]
		fn into_parts_registers_routes_for_reverse() {
			// Arrange
			let router = UnifiedRouter::new().server(|s| {
				s.with_namespace("api").function_named(
					"/health",
					Method::GET,
					"health",
					dummy_handler,
				)
			});

			// Act
			let (server, _client) = router.into_parts();

			// Assert
			let url = server.reverse("api:health", &[]);
			assert_eq!(url, Some("/health".to_string()));
		}
	}
}
