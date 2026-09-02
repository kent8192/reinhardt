//! Unified Router with endpoint-based server and client configuration.
//!
//! This module provides [`UnifiedRouter`], a unified entry point for configuring
//! server-side HTTP, WebSocket, and gRPC routing together with client-side SPA
//! routing.
//!
//! # Architecture
//!
//! | Target | Active private storage | Active builder | Access and extraction |
//! |---|---|---|---|
//! | Native | Server, WebSocket, gRPC, DI, and streaming routes | `.server(...)` | Server access and extraction |
//! | WASM | Client routes and navigation state | `.client(...)` | Client access and extraction |
//!
//! `UnifiedRouter` is non-generic on both targets. The inactive builder is P1:
//! it type-checks its closure and returns the router unchanged without invoking
//! the closure or storing the inactive route representation.
//!
//! # Example
//!
//! ```rust,ignore
//! use reinhardt_urls::routers::UnifiedRouter;
//! use reinhardt_core::page::Page;
//!
//! let router = UnifiedRouter::new()
//!     .server(|s| s
//!         .with_prefix("/api/v1")
//!         .endpoint(list_users)
//!         .endpoint(create_user))
//!     // Native-only protocol builders are available when enabled.
//!     // .websocket(|ws| ws.mount("/ws/", app_ws_routes()))
//!     // .grpc(|grpc| grpc.merge(app_grpc_services()))
//!     .client(|c| c
//!         .route("home", "/", || home_page())
//!         .route_path("user_detail", "/users/{id}", |Path(id): Path<i64>| user_page(id)));
//! ```
//!
//! # Feature Flags
//!
//! - When `client-router` feature is **enabled**: Non-generic [`UnifiedRouter`]
//!   with both declaration builders. Each target stores and executes only its
//!   applicable route representation.
//! - When `client-router` feature is **disabled**: Server-only [`UnifiedRouter`] with
//!   only `.server()` method available.
//!
//! Cross-target behavior follows the P0/P1/P2 contract documented in
//! `instructions/API_PARITY.md`.

#[cfg(native)]
use crate::routers::server_router::ServerRouter;
#[cfg(native)]
use crate::routers::{NativeHttpRoutes, NativeRoutes};

#[cfg(feature = "client-router")]
use crate::routers::client_router::ClientRouter;

#[cfg(all(native, not(feature = "client-router")))]
use reinhardt_core::exception::Result;
#[cfg(native)]
use reinhardt_di::InjectionContext;
#[cfg(all(native, not(feature = "client-router")))]
use reinhardt_http::{Request, Response};
#[cfg(native)]
use reinhardt_middleware::Middleware;
#[cfg(native)]
use std::sync::Arc;

#[cfg(native)]
fn attach_child_di_registrations(
	parent: &mut reinhardt_di::DiRegistrationList,
	child_server: ServerRouter,
	registrations: reinhardt_di::DiRegistrationList,
) -> ServerRouter {
	if let Some(context) = child_server.di_context() {
		registrations.apply_to(context.singleton_scope());
	} else {
		parent.merge(registrations);
	}
	child_server
}

#[cfg(native)]
fn retain_websocket_context(
	contexts: &mut Vec<(
		reinhardt_core::ws::WebSocketConsumerKey,
		Arc<InjectionContext>,
	)>,
	router: &reinhardt_core::ws::WebSocketRouter,
	context: Option<Arc<InjectionContext>>,
) {
	let Some(context) = context else { return };
	for route in router.routes() {
		if !contexts.iter().any(|(key, _)| *key == route.consumer_key()) {
			contexts.push((route.consumer_key(), Arc::clone(&context)));
		}
	}
}

// ============================================================================
// client-router feature ENABLED
// ============================================================================

/// Target-neutral unified router for native route declarations.
///
/// This non-generic public type stores only native server, WebSocket, gRPC,
/// DI, and streaming state. It exposes server access and extraction APIs on
/// native. Client access and extraction APIs are available only on the WASM
/// `UnifiedRouter`, which stores [`ClientRouter`] instead.
///
/// The `.client(...)` builder is retained as a P1 declaration API: it
/// type-checks its closure and drops it without invocation or registration.
/// Do not put required side effects in that closure.
///
/// # Example
///
/// ```rust,ignore
/// use reinhardt_urls::routers::UnifiedRouter;
/// use reinhardt_core::page::Page;
///
/// let router = UnifiedRouter::new()
///     .server(|s| s.endpoint(health_handler))
///     .client(|c| c.route("home", "/", || home_page()));
/// ```
///
/// [`Page`]: reinhardt_core::page::Page
#[cfg(all(feature = "client-router", native))]
pub struct UnifiedRouter {
	server: ServerRouter,
	/// WebSocket router for `urls.ws().<app>().<handler>()` URL resolution.
	pub websocket: reinhardt_core::ws::WebSocketRouter,
	websocket_contexts: Vec<(
		reinhardt_core::ws::WebSocketConsumerKey,
		Arc<InjectionContext>,
	)>,
	#[cfg(feature = "grpc")]
	grpc: reinhardt_grpc::GrpcRouter,
	di_registrations: reinhardt_di::DiRegistrationList,
	#[cfg(feature = "streaming")]
	streaming_handlers: Vec<reinhardt_streaming::StreamingHandlerRegistration>,
}

#[cfg(all(feature = "client-router", native))]
impl UnifiedRouter {
	/// Creates a new `UnifiedRouter` with a default server router.
	pub fn new() -> Self {
		Self {
			server: ServerRouter::new(),
			websocket: reinhardt_core::ws::WebSocketRouter::new(),
			websocket_contexts: Vec::new(),
			#[cfg(feature = "grpc")]
			grpc: reinhardt_grpc::GrpcRouter::new(),
			di_registrations: reinhardt_di::DiRegistrationList::new(),
			#[cfg(feature = "streaming")]
			streaming_handlers: Vec::new(),
		}
	}

	/// Declare client-side routing configuration on native targets.
	///
	/// Parity: P1.
	///
	/// This method type-checks `f` as `FnOnce(ClientRouter) -> ClientRouter`, then
	/// drops it and returns the unchanged native router. It does not construct a
	/// `ClientRouter`, invoke the closure, register client routes, or retain client
	/// state. Captured values are dropped normally. Required client-route work must
	/// run on WASM or construct [`ClientRouter`] directly in a reactive scope.
	///
	/// The WASM counterpart invokes and stores this closure's result.
	pub fn client<F>(self, _f: F) -> Self
	where
		F: FnOnce(ClientRouter) -> ClientRouter,
	{
		self
	}

	/// Configure server-side routing with a closure.
	///
	/// The closure receives a [`ServerRouter`] and should return a configured router.
	///
	/// Parity: P1.
	///
	/// Native builds execute the closure and store the configured `ServerRouter`.
	/// The WASM counterpart type-checks and drops the closure without invoking it.
	///
	/// # Example
	///
	/// ```rust,ignore
	/// let router = UnifiedRouter::new()
	///     .server(|s| s
	///         .with_prefix("/api")
	///         .endpoint(list_users));
	/// ```
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

	/// Configure WebSocket routing with a closure.
	///
	/// Parallel to `server()` and `client()`. The registered consumers are
	/// available via `UrlReverser::from_global().reverse("websocket:<app>:<handler>")`.
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

	/// Configure gRPC routing with a closure.
	#[cfg(feature = "grpc")]
	pub fn grpc<F>(mut self, configure: F) -> Self
	where
		F: FnOnce(reinhardt_grpc::GrpcRouter) -> reinhardt_grpc::GrpcRouter,
	{
		self.grpc = configure(self.grpc);
		self
	}

	/// Consumes the router and returns every native protocol component.
	#[doc(hidden)]
	pub fn __into_native_routes(mut self) -> NativeRoutes {
		let di_context = self.server.di_context().cloned();
		retain_websocket_context(
			&mut self.websocket_contexts,
			&self.websocket,
			di_context.clone(),
		);
		NativeRoutes {
			server: NativeHttpRoutes::Owned(Box::new(self.server)),
			websocket: self.websocket,
			websocket_contexts: self.websocket_contexts,
			#[cfg(feature = "grpc")]
			grpc: self.grpc,
			di_context,
			di_registrations: self.di_registrations,
			#[cfg(feature = "streaming")]
			streaming_handlers: self.streaming_handlers,
		}
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
	///
	/// Any DI singleton registrations contributed by the middleware (via
	/// [`Middleware::di_registrations`]) are merged into this router's
	/// deferred DI registration list so that handlers resolved through
	/// `#[inject]` can see middleware-owned state without a parallel
	/// `with_di_registrations(...)` call. See #4426.
	pub fn with_middleware<M: Middleware + 'static>(mut self, middleware: M) -> Self {
		for (type_id, value) in middleware.di_registrations() {
			self.di_registrations.register_arc_any(type_id, value);
		}
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

	/// Mount streaming handlers (producers and consumers) on this router.
	///
	/// Registrations are stored on the router for Phase 3 worker startup.
	/// Consumer worker startup is deferred to a later server startup phase.
	#[cfg(feature = "streaming")]
	pub fn mount_streaming(mut self, router: reinhardt_streaming::StreamingRouter) -> Self {
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

	/// Set namespace for the native server, WebSocket, and gRPC routers.
	pub fn with_namespace(mut self, namespace: impl Into<String>) -> Self {
		let namespace = namespace.into();
		self.server = self.server.with_namespace(&namespace);
		self.websocket = self.websocket.with_namespace(&namespace);
		#[cfg(feature = "grpc")]
		{
			self.grpc = self.grpc.with_namespace(namespace);
		}
		self
	}

	/// Merge another unified router without applying a prefix.
	pub fn merge(mut self, child: Self) -> Self {
		self.websocket_contexts
			.extend(child.websocket_contexts.iter().cloned());
		retain_websocket_context(
			&mut self.websocket_contexts,
			&child.websocket,
			child.server.di_context().cloned(),
		);
		let child_server = attach_child_di_registrations(
			&mut self.di_registrations,
			child.server,
			child.di_registrations,
		);
		self.server = self.server.group(vec![child_server]);
		self.websocket = self.websocket.merge(child.websocket);
		#[cfg(feature = "grpc")]
		{
			self.grpc = self.grpc.merge(child.grpc);
		}
		#[cfg(feature = "streaming")]
		self.streaming_handlers.extend(child.streaming_handlers);
		self
	}

	/// Registers the server router globally.
	pub fn register_globally(mut self) {
		self.flush_di_registrations();
		crate::routers::register_router(self.server);
	}

	/// Mount a child unified router on this router.
	pub fn mount_unified(mut self, prefix: &str, child: Self) -> Self {
		self.websocket_contexts
			.extend(child.websocket_contexts.iter().cloned());
		retain_websocket_context(
			&mut self.websocket_contexts,
			&child.websocket,
			child.server.di_context().cloned(),
		);
		let child_server = attach_child_di_registrations(
			&mut self.di_registrations,
			child.server,
			child.di_registrations,
		);
		self.server = self.server.mount(prefix, child_server);
		self.websocket = self.websocket.mount(prefix, child.websocket);
		#[cfg(feature = "grpc")]
		{
			self.grpc = self.grpc.mount(prefix, child.grpc);
		}
		#[cfg(feature = "streaming")]
		self.streaming_handlers.extend(child.streaming_handlers);
		self
	}
}

#[cfg(all(feature = "client-router", native))]
impl std::fmt::Debug for UnifiedRouter {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("UnifiedRouter")
			.field("server", &self.server)
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

// Cross-target signature unification assertion (issue #4569).
//
// Proves that a free function `fn(ServerRouter) -> ServerRouter` is accepted
// by `UnifiedRouter::server` on the native target, mirroring the WASM arm in
// the `#[cfg(all(wasm, feature = "client-router"))]` block. Because the
// closure parameter type is now `ServerRouter` on both targets, the same
// delegate can be passed cross-target without `#[cfg]` at the call site.
#[cfg(all(native, feature = "client-router"))]
#[doc(hidden)]
const _: fn() = || {
	fn delegate(s: ServerRouter) -> ServerRouter {
		s
	}
	let _ = UnifiedRouter::new().server(delegate).client(|c| c);
};

// Note: Handler is not yet implemented for UnifiedRouter when client-router is enabled.
// As of #4065 / #4067, ClientRouter is Send + Sync on native targets (Signal<T> is backed
// by Arc<RwLock<T>>), so the previous Sync blocker no longer applies. A Handler impl can
// be added in a follow-up PR.
// For server-side HTTP handling, use `into_server()` or `__into_native_routes()`.

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
///     .server(|s| s.endpoint(health_handler));
/// ```
#[cfg(not(feature = "client-router"))]
pub struct UnifiedRouter {
	server: ServerRouter,
	/// WebSocket router for native consumer routes.
	pub websocket: reinhardt_core::ws::WebSocketRouter,
	websocket_contexts: Vec<(
		reinhardt_core::ws::WebSocketConsumerKey,
		Arc<InjectionContext>,
	)>,
	#[cfg(feature = "grpc")]
	grpc: reinhardt_grpc::GrpcRouter,
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
			websocket: reinhardt_core::ws::WebSocketRouter::new(),
			websocket_contexts: Vec::new(),
			#[cfg(feature = "grpc")]
			grpc: reinhardt_grpc::GrpcRouter::new(),
			di_registrations: reinhardt_di::DiRegistrationList::new(),
			#[cfg(feature = "streaming")]
			streaming_handlers: Vec::new(),
		}
	}

	/// Configure server-side routing with a closure.
	///
	/// Parity: P0 native-only.
	///
	/// This arm only exists when `client-router` is disabled, and `routers.rs`
	/// only re-exports it on native targets. WASM builds require the
	/// `client-router` feature for the P1 no-op `server` closure shape.
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

	/// Configure WebSocket routing with a closure.
	pub fn websocket<F>(mut self, configure: F) -> Self
	where
		F: FnOnce(reinhardt_core::ws::WebSocketRouter) -> reinhardt_core::ws::WebSocketRouter,
	{
		self.websocket = configure(self.websocket);
		self
	}

	/// Returns a reference to the WebSocket router.
	pub fn websocket_ref(&self) -> &reinhardt_core::ws::WebSocketRouter {
		&self.websocket
	}

	/// Configure gRPC routing with a closure.
	#[cfg(feature = "grpc")]
	pub fn grpc<F>(mut self, configure: F) -> Self
	where
		F: FnOnce(reinhardt_grpc::GrpcRouter) -> reinhardt_grpc::GrpcRouter,
	{
		self.grpc = configure(self.grpc);
		self
	}

	/// Merge another unified router without applying a prefix.
	pub fn merge(mut self, child: Self) -> Self {
		self.websocket_contexts
			.extend(child.websocket_contexts.iter().cloned());
		retain_websocket_context(
			&mut self.websocket_contexts,
			&child.websocket,
			child.server.di_context().cloned(),
		);
		let child_server = attach_child_di_registrations(
			&mut self.di_registrations,
			child.server,
			child.di_registrations,
		);
		self.server = self.server.group(vec![child_server]);
		self.websocket = self.websocket.merge(child.websocket);
		#[cfg(feature = "grpc")]
		{
			self.grpc = self.grpc.merge(child.grpc);
		}
		#[cfg(feature = "streaming")]
		self.streaming_handlers.extend(child.streaming_handlers);
		self
	}

	/// Consumes the router and returns every native protocol component.
	#[doc(hidden)]
	pub fn __into_native_routes(mut self) -> NativeRoutes {
		let di_context = self.server.di_context().cloned();
		retain_websocket_context(
			&mut self.websocket_contexts,
			&self.websocket,
			di_context.clone(),
		);
		NativeRoutes {
			server: NativeHttpRoutes::Owned(Box::new(self.server)),
			websocket: self.websocket,
			websocket_contexts: self.websocket_contexts,
			#[cfg(feature = "grpc")]
			grpc: self.grpc,
			di_context,
			di_registrations: self.di_registrations,
			#[cfg(feature = "streaming")]
			streaming_handlers: self.streaming_handlers,
		}
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
		let namespace = namespace.into();
		self.server = self.server.with_namespace(&namespace);
		self.websocket = self.websocket.with_namespace(&namespace);
		#[cfg(feature = "grpc")]
		{
			self.grpc = self.grpc.with_namespace(namespace);
		}
		self
	}

	/// Set DI context for server router.
	pub fn with_di_context(mut self, ctx: Arc<InjectionContext>) -> Self {
		self.server = self.server.with_di_context(ctx);
		self
	}

	/// Add middleware to server router.
	///
	/// Any DI singleton registrations contributed by the middleware (via
	/// [`Middleware::di_registrations`]) are merged into this router's
	/// deferred DI registration list so that handlers resolved through
	/// `#[inject]` can see middleware-owned state without a parallel
	/// `with_di_registrations(...)` call. See #4426.
	pub fn with_middleware<M: Middleware + 'static>(mut self, middleware: M) -> Self {
		for (type_id, value) in middleware.di_registrations() {
			self.di_registrations.register_arc_any(type_id, value);
		}
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
		let mut mounted = self;
		let child_server = attach_child_di_registrations(
			&mut mounted.di_registrations,
			child.server,
			child.di_registrations,
		);
		mounted.server = mounted.server.mount(prefix, child_server);
		mounted.websocket = mounted.websocket.mount(prefix, child.websocket);
		#[cfg(feature = "grpc")]
		{
			mounted.grpc = mounted.grpc.mount(prefix, child.grpc);
		}
		#[cfg(feature = "streaming")]
		mounted.streaming_handlers.extend(child.streaming_handlers);
		mounted
	}

	/// Mount streaming handlers on this router.
	#[cfg(feature = "streaming")]
	pub fn mount_streaming(mut self, router: reinhardt_streaming::StreamingRouter) -> Self {
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

/// No-op `ServerRouter` used on WASM targets.
///
/// On WASM, server-side routing is not available, so this is the WASM
/// counterpart of the native [`ServerRouter`](crate::routers::ServerRouter):
/// it presents the same builder surface, but every method is a no-op that
/// consumes `self`, drops its arguments, and returns `Self`. The result is
/// ultimately discarded by [`UnifiedRouter::server`] on WASM.
///
/// Because the type and its builder methods share their names with the
/// native `ServerRouter`, the same
/// `.server(|s| s.with_prefix(...).endpoint(...))` closure body
/// compiles uniformly on both native and WASM targets (issue #4569).
///
/// Generic bounds from the native `ServerRouter` are intentionally relaxed
/// here: many upstream bound types (`Handler`, `Middleware`, `ViewSet`,
/// `InjectionContext`, `Method`, …) live behind `#[cfg(native)]` and would
/// not resolve on WASM, so the no-op methods only reproduce the call-site
/// arity.
#[cfg(all(wasm, feature = "client-router"))]
pub struct ServerRouter;

#[cfg(all(wasm, feature = "client-router"))]
impl ServerRouter {
	/// Construct a new no-op `ServerRouter` (WASM).
	pub fn new() -> Self {
		Self
	}

	/// No-op for `ServerRouter::with_prefix`.
	pub fn with_prefix(self, _prefix: impl Into<String>) -> Self {
		self
	}

	/// No-op for `ServerRouter::with_namespace`.
	pub fn with_namespace(self, _namespace: impl Into<String>) -> Self {
		self
	}

	/// No-op for `ServerRouter::with_di_context`.
	pub fn with_di_context<C>(self, _ctx: C) -> Self {
		self
	}

	/// No-op for `ServerRouter::with_middleware`.
	pub fn with_middleware<M>(self, _middleware: M) -> Self {
		self
	}

	/// No-op for `ServerRouter::with_route_middleware`.
	pub fn with_route_middleware<M>(self, _middleware: M) -> Self {
		self
	}

	/// No-op for `ServerRouter::exclude`.
	pub fn exclude(self, _pattern: &str) -> Self {
		self
	}

	/// No-op for `ServerRouter::mount`.
	pub fn mount<R>(self, _prefix: &str, _child: R) -> Self {
		self
	}

	/// No-op for `ServerRouter::group`.
	pub fn group<R>(self, _routers: Vec<R>) -> Self {
		self
	}

	/// No-op for `ServerRouter::handler`.
	pub fn handler<H>(self, _path: &str, _handler: H) -> Self {
		self
	}

	/// No-op for `ServerRouter::handler_arc`.
	pub fn handler_arc<H>(self, _path: &str, _handler: H) -> Self {
		self
	}

	/// No-op for `ServerRouter::view`.
	pub fn view<V>(self, _path: &str, _view: V) -> Self {
		self
	}

	/// No-op for `ServerRouter::view_named`.
	pub fn view_named<V>(self, _path: &str, _name: &str, _view: V) -> Self {
		self
	}

	/// No-op for `ServerRouter::viewset`.
	pub fn viewset<V>(self, _prefix: &str, _viewset: V) -> Self {
		self
	}

	/// No-op for `ServerRouter::endpoint`.
	pub fn endpoint<F>(self, _f: F) -> Self {
		self
	}

	/// No-op for `ServerFnRouterExt::server_fn` on the WASM side.
	///
	/// `server_fn` is provided to native `ServerRouter` by the
	/// `ServerFnRouterExt` extension trait in `reinhardt-pages`. That trait
	/// (and its `ServerFnRegistration` bound) is a native-only concern, so
	/// the no-op `ServerRouter` absorbs the call as a free-standing inherent
	/// method without any trait bound. This lets cross-target builder chains
	/// such as `UnifiedRouter::new().server(|s| s.server_fn(marker))` compile
	/// on `wasm32-unknown-unknown` without `#[cfg(native)]` gates at the call
	/// site.
	pub fn server_fn<S>(self, _marker: S) -> Self {
		self
	}
}

#[cfg(all(wasm, feature = "client-router"))]
impl Default for ServerRouter {
	fn default() -> Self {
		Self::new()
	}
}

// Drift detection (Fixes #4185).
//
// Unified/server route declarations re-emit the user-supplied
// `.server(|s| ...)` closure body verbatim on WASM, so every builder
// method called inside that closure must exist on the no-op WASM
// `ServerRouter`. The `const _` below type-checks the canonical builder
// chain on every `cargo check --target wasm32-unknown-unknown`; if a future
// change drops a no-op method while the corresponding native `ServerRouter`
// builder still exists, this assertion fails to compile and CI catches the
// drift.
#[cfg(all(wasm, feature = "client-router"))]
#[doc(hidden)]
const _: fn() = || {
	let _ = UnifiedRouter::new()
		.server(|s| {
			s.with_prefix("/api")
				.with_namespace("api")
				.with_di_context(())
				.with_middleware(())
				.with_route_middleware(())
				.exclude("/internal")
				.mount("/v1/", ServerRouter::new())
				.group(Vec::<ServerRouter>::new())
				.handler("/h", ())
				.handler_arc("/h", ())
				.view("/v", ())
				.view_named("/v", "v", ())
				.viewset("/vs/", ())
				.endpoint(|| ())
				.server_fn(())
		})
		.client(|c| c);
};

// Cross-target signature unification assertion (issue #4569).
//
// Proves that a free function `fn(ServerRouter) -> ServerRouter` is accepted
// by `UnifiedRouter::server` on the WASM target, mirroring the native arm in
// the `#[cfg(all(native, feature = "client-router"))]` block. The closure
// parameter type is now uniform across targets, so the same delegate compiles
// on both.
#[cfg(all(wasm, feature = "client-router"))]
#[doc(hidden)]
const _: fn() = || {
	fn delegate(s: ServerRouter) -> ServerRouter {
		s
	}
	let _ = UnifiedRouter::new().server(delegate).client(|c| c);
};

/// Target-neutral unified router for WASM client routing.
///
/// This non-generic public type stores only [`ClientRouter`] state. It exposes
/// client access and extraction APIs on WASM. Server access and extraction APIs
/// are available only on native `UnifiedRouter`, which stores native protocol
/// state instead.
///
/// The `.server(...)` builder is retained as a P1 declaration API: it
/// type-checks its closure and drops it without invocation or registration.
/// Do not put required side effects in that closure.
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
	/// Parity: P1.
	///
	/// Native builds execute the closure and store the configured `ServerRouter`.
	/// WASM builds type-check and drop the closure without invoking it.
	///
	/// On WASM, server routing is not available. The closure is accepted for
	/// cross-target type-checking — its `ServerRouter` parameter type is unified
	/// with the native arm — and then discarded without being invoked, so the
	/// shared route definitions compile on both targets at zero WASM runtime cost.
	pub fn server<F>(self, _f: F) -> Self
	where
		F: FnOnce(ServerRouter) -> ServerRouter,
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

	/// Merge another client-only router without applying a prefix.
	pub fn merge(mut self, child: Self) -> Self {
		self.client = self.client.merge(child.client);
		self
	}

	/// Consumes the router and returns the client router.
	pub fn into_client(self) -> ClientRouter {
		self.client
	}

	/// Returns the client router on WASM.
	/// Global registration is caller-managed.
	pub fn register_globally(self) -> ClientRouter {
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
	pub fn mount_streaming(self, _router: reinhardt_streaming::StreamingRouter) -> Self {
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
	#[cfg(all(wasm, feature = "client-router"))]
	use reinhardt_core::page::Page;
	use reinhardt_core::reactive::ReactiveScope;

	#[test]
	fn test_unified_router_new() {
		ReactiveScope::run(|| {
			let router = UnifiedRouter::new();
			// Should have default server router
			assert_eq!(router.server_ref().prefix(), "");
		});
	}

	#[cfg(all(feature = "client-router", native))]
	#[test]
	fn server_only_router_does_not_require_a_reactive_scope() {
		let router = UnifiedRouter::new()
			.server(|server| server.with_prefix("/api"))
			.into_server();

		assert_eq!(router.prefix(), "/api");
	}

	#[cfg(all(feature = "client-router", native))]
	#[test]
	fn native_client_builder_is_inert_and_preserves_send_and_sync() {
		fn assert_send_and_sync<T: Send + Sync>(_: &T) {}

		let router = UnifiedRouter::new().client(|_| {
			panic!("native client builder must not be invoked");
		});

		assert_send_and_sync(&router);
		assert_eq!(router.server_ref().prefix(), "");
	}

	#[test]
	fn test_unified_router_server_closure() {
		ReactiveScope::run(|| {
			let router =
				UnifiedRouter::new().server(|s| s.with_prefix("/api").with_namespace("v1"));

			assert_eq!(router.server_ref().prefix(), "/api");
			assert_eq!(router.server_ref().namespace(), Some("v1"));
		});
	}

	#[test]
	fn test_unified_router_convenience_methods() {
		ReactiveScope::run(|| {
			let router = UnifiedRouter::new()
				.with_prefix("/api")
				.with_namespace("v1");

			assert_eq!(router.server_ref().prefix(), "/api");
			assert_eq!(router.server_ref().namespace(), Some("v1"));
		});
	}

	#[cfg(all(wasm, feature = "client-router"))]
	#[test]
	fn test_unified_router_client_closure() {
		ReactiveScope::run(|| {
			let router = UnifiedRouter::new().client(|c| c.route("home", "/", || Page::Empty));

			assert_eq!(router.client_ref().route_count(), 1);
		});
	}

	#[cfg(all(wasm, feature = "client-router"))]
	#[test]
	fn unified_wasm_with_namespace_propagates_to_client() {
		ReactiveScope::run(|| {
			// Arrange
			let router = UnifiedRouter::new()
				.client(|c| c.route("login", "/login/", || Page::Empty))
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
		});
	}

	#[cfg(feature = "client-router")]
	#[test]
	fn test_unified_router_into_server() {
		ReactiveScope::run(|| {
			let router = UnifiedRouter::new().server(|s| s.with_prefix("/api"));

			let server = router.into_server();
			assert_eq!(server.prefix(), "/api");
		});
	}

	mod flush_di_registrations {
		use super::*;
		use reinhardt_di::{DiRegistrationList, InjectionContext, SingletonScope};
		use rstest::rstest;
		use std::sync::Arc;

		struct ChildWebSocket;

		impl reinhardt_core::ws::WebSocketEndpointInfo for ChildWebSocket {
			fn path() -> &'static str {
				"/ws/child"
			}
			fn name() -> Option<&'static str> {
				Some("child")
			}
		}

		#[rstest]
		fn applies_registrations_to_di_context_singleton_scope() {
			ReactiveScope::run(|| {
				// Arrange
				let singleton_scope = Arc::new(SingletonScope::new());
				let di_ctx =
					Arc::new(InjectionContext::builder(Arc::clone(&singleton_scope)).build());

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
			});
		}

		#[rstest]
		fn applies_registrations_regardless_of_builder_order() {
			ReactiveScope::run(|| {
				// Arrange
				let singleton_scope = Arc::new(SingletonScope::new());
				let di_ctx =
					Arc::new(InjectionContext::builder(Arc::clone(&singleton_scope)).build());

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
			});
		}

		#[rstest]
		fn merge_applies_child_registrations_to_the_child_context() {
			ReactiveScope::run(|| {
				// Arrange
				let parent_scope = Arc::new(SingletonScope::new());
				let child_scope = Arc::new(SingletonScope::new());
				let parent_context =
					Arc::new(InjectionContext::builder(Arc::clone(&parent_scope)).build());
				let child_context =
					Arc::new(InjectionContext::builder(Arc::clone(&child_scope)).build());
				let mut registrations = DiRegistrationList::new();
				registrations.register(123usize);

				// Act
				let _merged = UnifiedRouter::new().with_di_context(parent_context).merge(
					UnifiedRouter::new()
						.with_di_context(child_context)
						.with_di_registrations(registrations),
				);

				// Assert
				assert!(parent_scope.get::<usize>().is_none());
				assert_eq!(
					*child_scope.get::<usize>().expect("child registration"),
					123
				);
			});
		}

		#[rstest]
		fn merge_retains_child_context_for_websocket_routes() {
			ReactiveScope::run(|| {
				let parent_context =
					Arc::new(InjectionContext::builder(Arc::new(SingletonScope::new())).build());
				let child_context =
					Arc::new(InjectionContext::builder(Arc::new(SingletonScope::new())).build());
				let routes = UnifiedRouter::new()
					.with_di_context(parent_context)
					.merge(
						UnifiedRouter::new()
							.with_di_context(Arc::clone(&child_context))
							.websocket(|router| router.consumer(|| ChildWebSocket)),
					)
					.__into_native_routes();

				assert!(routes.websocket_contexts.iter().any(|(key, context)| {
					*key == reinhardt_core::ws::WebSocketConsumerKey::of::<ChildWebSocket>()
						&& Arc::ptr_eq(context, &child_context)
				}));
			});
		}

		#[rstest]
		#[serial_test::serial(global_di)]
		fn stashes_globally_when_no_di_context() {
			ReactiveScope::run(|| {
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
			});
		}
	}

	mod debug_impl {
		use super::*;
		use rstest::rstest;
		use std::sync::Arc;

		#[rstest]
		fn unified_router_implements_debug() {
			ReactiveScope::run(|| {
				let router = UnifiedRouter::new().with_prefix("/api");
				let debug_output = format!("{:?}", router);
				assert!(debug_output.contains("UnifiedRouter"));
				assert!(debug_output.contains("ServerRouter"));
			});
		}

		#[rstest]
		fn arc_try_unwrap_with_expect() {
			ReactiveScope::run(|| {
				// This is the primary use case from #3391:
				// Arc::try_unwrap().expect() requires Debug on the error type
				let router = Arc::new(UnifiedRouter::new());
				let unwrapped = Arc::try_unwrap(router).expect("should have single ref");
				assert_eq!(unwrapped.server_ref().prefix(), "");
			});
		}
	}

	mod route_registration {
		use super::*;
		use hyper::Method;
		use reinhardt_core::endpoint::EndpointInfo;
		use reinhardt_http::{Request, Response, Result};
		use rstest::rstest;

		struct HealthEndpoint;

		impl EndpointInfo for HealthEndpoint {
			fn path() -> &'static str {
				"/health"
			}

			fn method() -> Method {
				Method::GET
			}

			fn name() -> &'static str {
				"health"
			}
		}

		#[async_trait::async_trait]
		impl reinhardt_http::Handler for HealthEndpoint {
			async fn handle(&self, _req: Request) -> Result<Response> {
				Ok(Response::ok())
			}
		}

		#[rstest]
		fn into_server_registers_routes_for_reverse() {
			ReactiveScope::run(|| {
				// Arrange
				let router = UnifiedRouter::new()
					.server(|s| s.with_namespace("api").endpoint(|| HealthEndpoint));

				// Act
				let server = router.into_server();

				// Assert
				let url = server.reverse("api:health", &[]);
				assert_eq!(url, Some("/health".to_string()));
			});
		}
	}
}
