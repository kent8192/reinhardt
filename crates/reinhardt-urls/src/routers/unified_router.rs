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

#[cfg(not(target_arch = "wasm32"))]
use crate::routers::server_router::ServerRouter;

#[cfg(feature = "client-router")]
use crate::routers::client_router::ClientRouter;

#[cfg(not(target_arch = "wasm32"))]
use hyper::Method;
#[cfg(not(target_arch = "wasm32"))]
use reinhardt_core::exception::Result;
#[cfg(not(target_arch = "wasm32"))]
use reinhardt_di::InjectionContext;
#[cfg(not(target_arch = "wasm32"))]
use reinhardt_http::{Request, Response};
#[cfg(not(target_arch = "wasm32"))]
use reinhardt_middleware::Middleware;
#[cfg(not(target_arch = "wasm32"))]
use std::future::Future;
#[cfg(not(target_arch = "wasm32"))]
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
#[cfg(all(feature = "client-router", not(target_arch = "wasm32")))]
pub struct UnifiedRouter {
	server: ServerRouter,
	client: ClientRouter,
	di_registrations: reinhardt_di::DiRegistrationList,
}

#[cfg(all(feature = "client-router", not(target_arch = "wasm32")))]
impl UnifiedRouter {
	/// Creates a new `UnifiedRouter` with default server and client routers.
	pub fn new() -> Self {
		Self {
			server: ServerRouter::new(),
			client: ClientRouter::new(),
			di_registrations: reinhardt_di::DiRegistrationList::new(),
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

	/// Consumes the router and returns the server router.
	///
	/// If this router has deferred DI registrations, they are stashed
	/// in the global registry for later application by the server.
	pub fn into_server(self) -> ServerRouter {
		if !self.di_registrations.is_empty() {
			crate::routers::register_di_registrations(self.di_registrations);
		}
		self.server
	}

	/// Consumes the router and returns the client router.
	pub fn into_client(self) -> ClientRouter {
		if !self.di_registrations.is_empty() {
			crate::routers::register_di_registrations(self.di_registrations);
		}
		self.client
	}

	/// Consumes the router and returns both parts.
	pub fn into_parts(self) -> (ServerRouter, ClientRouter) {
		if !self.di_registrations.is_empty() {
			crate::routers::register_di_registrations(self.di_registrations);
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
		crate::routers::register_router(server);
		client
	}

	/// Attach deferred DI registrations to this router.
	///
	/// These registrations will be stashed globally when the router is
	/// consumed (via [`into_server`](Self::into_server),
	/// [`into_parts`](Self::into_parts), or
	/// [`register_globally`](Self::register_globally)),
	/// and applied to the server's singleton scope during startup.
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

	/// Set namespace for server router.
	///
	/// This is a convenience method that delegates to [`ServerRouter::with_namespace`].
	pub fn with_namespace(mut self, namespace: impl Into<String>) -> Self {
		self.server = self.server.with_namespace(namespace);
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
	pub fn function_named<F, Fut>(mut self, path: &str, method: Method, name: &str, func: F) -> Self
	where
		F: Fn(Request) -> Fut + Send + Sync + 'static,
		Fut: Future<Output = Result<Response>> + Send + 'static,
	{
		self.server = self.server.function_named(path, method, name, func);
		self
	}
}

#[cfg(all(feature = "client-router", not(target_arch = "wasm32")))]
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
}

#[cfg(not(feature = "client-router"))]
impl UnifiedRouter {
	/// Creates a new `UnifiedRouter` with default server router.
	pub fn new() -> Self {
		Self {
			server: ServerRouter::new(),
			di_registrations: reinhardt_di::DiRegistrationList::new(),
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

	/// Consumes the router and returns the server router.
	///
	/// If this router has deferred DI registrations, they are stashed
	/// in the global registry for later application by the server.
	pub fn into_server(self) -> ServerRouter {
		if !self.di_registrations.is_empty() {
			crate::routers::register_di_registrations(self.di_registrations);
		}
		self.server
	}

	/// Registers server router globally.
	pub fn register_globally(self) {
		let di = self.di_registrations;
		if !di.is_empty() {
			crate::routers::register_di_registrations(di);
		}
		crate::routers::register_router(self.server);
	}

	/// Attach deferred DI registrations to this router.
	///
	/// These registrations will be stashed globally when the router is
	/// consumed (via [`into_server`](Self::into_server) or
	/// [`register_globally`](Self::register_globally)),
	/// and applied to the server's singleton scope during startup.
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
	pub fn function_named<F, Fut>(mut self, path: &str, method: Method, name: &str, func: F) -> Self
	where
		F: Fn(Request) -> Fut + Send + Sync + 'static,
		Fut: Future<Output = Result<Response>> + Send + 'static,
	{
		self.server = self.server.function_named(path, method, name, func);
		self
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
#[cfg(target_arch = "wasm32")]
pub struct ServerRouterStub;

/// Unified router for WASM targets with client-side routing.
///
/// On WASM, only client-side routing is available. The `.server()` method
/// accepts a closure but discards its result, allowing shared route
/// definitions to compile on both server and client.
#[cfg(all(target_arch = "wasm32", feature = "client-router"))]
pub struct UnifiedRouter {
	client: ClientRouter,
}

#[cfg(all(target_arch = "wasm32", feature = "client-router"))]
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

	/// Registers (no-op for server) and returns client router.
	pub fn register_globally(self) -> ClientRouter {
		self.client
	}

	/// Mount a child UnifiedRouter on this router (client routes only).
	pub fn mount_unified(mut self, _prefix: &str, child: UnifiedRouter) -> Self {
		// Merge child client routes into parent
		self.client = self.client.merge(child.client);
		self
	}

	/// No-op on WASM - server prefix is not applicable.
	pub fn with_prefix(self, _prefix: impl Into<String>) -> Self {
		self
	}

	/// No-op on WASM - server namespace is not applicable.
	pub fn with_namespace(self, _namespace: impl Into<String>) -> Self {
		self
	}
}

#[cfg(all(target_arch = "wasm32", feature = "client-router"))]
impl Default for UnifiedRouter {
	fn default() -> Self {
		Self::new()
	}
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
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
}
