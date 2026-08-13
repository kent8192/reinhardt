//! URL patterns registration for compile-time discovery
//!
//! This module provides types for registering URL pattern functions
//! at compile time using the `inventory` crate. This allows the framework to
//! automatically discover and register routers without manual boilerplate in
//! management commands.
//!
//! # Important Constraints
//!
//! **Only one `#[routes]` function is allowed per project.** If multiple
//! functions are annotated with `#[routes]`, the linker will fail with a
//! "duplicate symbol" error for `__reinhardt_routes_registration_marker`.
//!
//! If you need to organize routes across multiple files, combine them in
//! a single root function:
//!
//! ```rust,ignore
//! // src/config/urls.rs
//! use reinhardt::prelude::*;
//! use reinhardt::routes;
//!
//! mod api;
//! mod web;
//!
//! #[routes]
//! pub fn routes() -> UnifiedRouter {
//!     UnifiedRouter::new()
//!         .mount("/api/", api::routes())  // Returns ServerRouter, not annotated with #[routes]
//!         .mount("/", web::routes())      // Returns ServerRouter, not annotated with #[routes]
//!         .client(|c| c.route("home", "/", home_page))
//! }
//! ```
//!
//! # Architecture
//!
//! The URL patterns registration system follows the same pattern as other
//! compile-time registration systems in Reinhardt (DI, Signals, OpenAPI, ViewSets):
//!
//! 1. User code uses the `#[routes]` attribute macro on a function returning [`UnifiedRouter`]
//! 2. Macro generates an `inventory::submit!` call with a server router function pointer
//! 3. Framework code retrieves registrations via `inventory::iter::<UrlPatternsRegistration>()`
//! 4. Framework calls the registered functions to get [`ServerRouter`] and optionally `ClientRouter`
//!
//! # Feature Independence
//!
//! The `#[routes]` macro always generates feature-independent code. The macro output
//! only contains `UrlPatternsRegistration::new(__get_server_router)` without any
//! `#[cfg]` attributes. The client router is set via `with_client_router()` within
//! library code that is properly feature-gated, avoiding feature context mismatches
//! between the library and downstream crates.
//!
//! # Examples
//!
//! ```rust,ignore
//! // src/config/urls.rs
//! use reinhardt::prelude::*;
//! use reinhardt::routes;
//!
//! #[routes]
//! pub fn routes() -> UnifiedRouter {
//!     UnifiedRouter::new()
//!         .server(|s| s.endpoint(views::index))
//!         .client(|c| c.route("home", "/", home_page))
//! }
//! ```
//!
//! The `#[routes]` macro automatically handles `inventory` registration,
//! so you don't need any additional boilerplate code.
//!
//! [`UnifiedRouter`]: crate::routers::UnifiedRouter
//! [`ServerRouter`]: crate::routers::ServerRouter

#[cfg(native)]
use crate::routers::NativeRoutes;
#[cfg(all(native, feature = "client-router"))]
use crate::routers::client_router::ClientRouter;
#[cfg(native)]
use crate::routers::native_routes::NativeHttpRoutes;
#[cfg(native)]
use crate::routers::server_router::ServerRouter;
#[cfg(native)]
use reinhardt_core::endpoint::{EndpointMetadata, ResolvedEndpoint};
#[cfg(native)]
use std::future::Future;
#[cfg(native)]
use std::pin::Pin;
#[cfg(native)]
use std::sync::Arc;

/// Error returned when mounted endpoint topology cannot be collected safely.
#[cfg(native)]
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum RouteTopologyError {
	/// An asynchronous route factory could execute arbitrary startup work.
	#[error("dynamic route topology cannot be resolved safely")]
	DynamicFactory,
	/// A mounted route could not be matched to endpoint metadata.
	#[error("mounted route metadata is incomplete")]
	IncompleteMetadata,
}

/// Error type returned by asynchronous route factories.
#[cfg(native)]
pub type BoxError = Box<dyn std::error::Error + Send + Sync>;

/// Function pointer type for async router factories.
///
/// Returns a pinned, boxed future that produces a server router or an error.
/// Used by `RouterFactory::Async` and `UrlPatternsRegistration::__macro_new_async`.
#[cfg(native)]
pub type AsyncRouterFactoryFn =
	fn() -> Pin<Box<dyn Future<Output = Result<Arc<ServerRouter>, BoxError>> + Send>>;

/// Function pointer type for synchronous native aggregate factories.
#[cfg(native)]
pub type NativeRouterFactoryFn = fn() -> NativeRoutes;

/// Function pointer type for asynchronous native aggregate factories.
#[cfg(native)]
pub type AsyncNativeRouterFactoryFn =
	fn() -> Pin<Box<dyn Future<Output = Result<NativeRoutes, BoxError>> + Send>>;

/// Factory for creating server routers, supporting both sync and async creation.
///
/// The sync variant is used by existing `#[routes]` functions that return
/// `UnifiedRouter` synchronously. The async variant is used when `#[routes]`
/// is applied to an `async fn`, enabling DI resolution via `#[inject]` parameters.
#[cfg(native)]
#[derive(Clone)]
pub enum RouterFactory {
	/// Synchronous factory (existing behavior for `fn routes() -> UnifiedRouter`)
	Sync(fn() -> Arc<ServerRouter>),
	/// Async factory for `async fn routes()` with optional `#[inject]` DI resolution
	Async(AsyncRouterFactoryFn),
	/// Synchronous factory preserving every native protocol route.
	NativeSync(NativeRouterFactoryFn),
	/// Asynchronous factory preserving every native protocol route.
	NativeAsync(AsyncNativeRouterFactoryFn),
}

/// URL patterns registration for compile-time discovery
///
/// This type is used with the `inventory` crate to register URL pattern
/// functions at compile time, allowing the framework to automatically
/// discover and register routers without manual boilerplate in management
/// commands like `runserver` or `check`.
///
/// # Fields
///
/// * `factory` - Router factory (sync or async) to create the server router
/// * `get_client_router` - Optional function pointer to get the client router (when `client-router` feature is enabled)
///
/// # Implementation Details
///
/// This struct is collected by `inventory::collect!` and can be iterated
/// at runtime using `inventory::iter::<UrlPatternsRegistration>()`.
///
/// The framework automatically calls these functions in `execute_from_command_line()`
/// to register routers before executing management commands.
///
/// # Note
///
/// You typically don't create this struct directly. Instead, use the `#[routes]`
/// attribute macro which generates the registration code automatically.
#[cfg(native)]
#[derive(Clone)]
pub struct UrlPatternsRegistration {
	/// Router factory (sync or async)
	///
	/// The `#[routes]` macro extracts the complete native aggregate from
	/// [`UnifiedRouter`]. Legacy public constructors continue to store HTTP-only
	/// factories, which are normalized when materialized.
	///
	/// [`UnifiedRouter`]: crate::routers::UnifiedRouter
	pub factory: RouterFactory,

	/// Optional function to get the client router
	///
	/// This function returns an `Arc<ClientRouter>` with all client-side routes.
	/// Set via `with_client_router()` builder method. The field is `Option` to
	/// allow feature-independent construction from macro-generated code, avoiding
	/// feature context mismatches between the library and downstream crates.
	///
	/// [`UnifiedRouter`]: crate::routers::UnifiedRouter
	#[cfg(feature = "client-router")]
	pub get_client_router: Option<fn() -> Arc<ClientRouter>>,
}

#[cfg(native)]
impl UrlPatternsRegistration {
	/// Create a new registration with the router factory functions
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_urls::routers::registration::UrlPatternsRegistration;
	/// use std::sync::Arc;
	///
	/// let registration = UrlPatternsRegistration::new(
	///     || Arc::new(routes().into_server()),
	///     Some(|| Arc::new(routes().into_client())),
	/// );
	/// ```
	///
	/// # Note
	///
	/// You typically don't call this directly. Use the `#[routes]` macro instead.
	#[cfg(feature = "client-router")]
	pub const fn new(
		get_server_router: fn() -> Arc<ServerRouter>,
		get_client_router: Option<fn() -> Arc<ClientRouter>>,
	) -> Self {
		Self {
			factory: RouterFactory::Sync(get_server_router),
			get_client_router,
		}
	}

	/// Create a new registration with the server router factory function (server-only mode)
	///
	/// # Note
	///
	/// You typically don't call this directly. Use the `#[routes]` macro instead.
	#[cfg(not(feature = "client-router"))]
	pub const fn new(get_server_router: fn() -> Arc<ServerRouter>) -> Self {
		Self {
			factory: RouterFactory::Sync(get_server_router),
		}
	}

	/// Internal constructor used by the `#[routes]` macro for sync routes.
	///
	/// Always takes a single argument regardless of feature flags, ensuring
	/// the macro output is feature-independent. This avoids feature context
	/// mismatches between the library and downstream crates.
	#[doc(hidden)]
	pub const fn __macro_new(get_server_router: fn() -> Arc<ServerRouter>) -> Self {
		Self {
			factory: RouterFactory::Sync(get_server_router),
			#[cfg(feature = "client-router")]
			get_client_router: None,
		}
	}

	/// Internal constructor used by the `#[routes]` macro for async routes.
	///
	/// Used when `#[routes]` is applied to an `async fn`, enabling DI
	/// resolution via `#[inject]` parameters.
	#[doc(hidden)]
	pub const fn __macro_new_async(factory: AsyncRouterFactoryFn) -> Self {
		Self {
			factory: RouterFactory::Async(factory),
			#[cfg(feature = "client-router")]
			get_client_router: None,
		}
	}

	/// Internal constructor used by the `#[routes]` macro for native sync routes.
	#[doc(hidden)]
	pub const fn __macro_new_native(factory: NativeRouterFactoryFn) -> Self {
		Self {
			factory: RouterFactory::NativeSync(factory),
			#[cfg(feature = "client-router")]
			get_client_router: None,
		}
	}

	/// Internal constructor used by the `#[routes]` macro for native async routes.
	#[doc(hidden)]
	pub const fn __macro_new_native_async(factory: AsyncNativeRouterFactoryFn) -> Self {
		Self {
			factory: RouterFactory::NativeAsync(factory),
			#[cfg(feature = "client-router")]
			get_client_router: None,
		}
	}

	/// Set the client router factory function (builder pattern)
	///
	/// This method is called within library code that is properly feature-gated,
	/// avoiding the feature context mismatch that would occur if the macro
	/// generated `#[cfg(feature = "client-router")]` code (which would be
	/// evaluated in the downstream crate's feature context).
	///
	/// # Note
	///
	/// You typically don't call this directly. Use the `#[routes]` macro instead.
	#[cfg(feature = "client-router")]
	pub const fn with_client_router(
		mut self,
		get_client_router: fn() -> Arc<ClientRouter>,
	) -> Self {
		self.get_client_router = Some(get_client_router);
		self
	}

	/// Get the server router from the registration (sync only).
	///
	/// # Panics
	///
	/// Panics if the factory is async. Use `server_router_async()` instead.
	pub fn server_router(&self) -> Arc<ServerRouter> {
		match &self.factory {
			RouterFactory::Sync(f) => f(),
			RouterFactory::NativeSync(f) => f().into_legacy_server(),
			RouterFactory::Async(_) | RouterFactory::NativeAsync(_) => {
				panic!(
					"Cannot call server_router() on an async #[routes] registration. \
					 Use server_router_async() instead."
				)
			}
		}
	}

	/// Get the server router from the registration, supporting both sync and async factories.
	pub async fn server_router_async(&self) -> Result<Arc<ServerRouter>, BoxError> {
		match &self.factory {
			RouterFactory::Sync(f) => Ok(f()),
			RouterFactory::Async(f) => f().await,
			RouterFactory::NativeSync(f) => Ok(f().into_legacy_server()),
			RouterFactory::NativeAsync(f) => Ok(f().await?.into_legacy_server()),
		}
	}

	/// Materialize the complete native route aggregate from a sync factory.
	///
	/// # Panics
	///
	/// Panics if the factory is async. Use [`Self::native_routes_async`] instead.
	pub fn native_routes(&self) -> NativeRoutes {
		match &self.factory {
			RouterFactory::Sync(f) => NativeRoutes::from_legacy(f()),
			RouterFactory::NativeSync(f) => f(),
			RouterFactory::Async(_) | RouterFactory::NativeAsync(_) => {
				panic!(
					"Cannot call native_routes() on an async #[routes] registration. \
					 Use native_routes_async() instead."
				)
			}
		}
	}

	/// Materialize the complete native route aggregate from any factory.
	pub async fn native_routes_async(&self) -> Result<NativeRoutes, BoxError> {
		match &self.factory {
			RouterFactory::Sync(f) => Ok(NativeRoutes::from_legacy(f())),
			RouterFactory::Async(f) => Ok(NativeRoutes::from_legacy(f().await?)),
			RouterFactory::NativeSync(f) => Ok(f()),
			RouterFactory::NativeAsync(f) => f().await,
		}
	}

	/// Get the client router from the registration, if available
	#[cfg(feature = "client-router")]
	pub fn client_router(&self) -> Option<Arc<ClientRouter>> {
		self.get_client_router.map(|f| f())
	}
}

// Collect registrations for runtime iteration
#[cfg(native)]
inventory::collect!(UrlPatternsRegistration);

/// Returns an iterator over all registered [`UrlPatternsRegistration`] entries.
///
/// Each registration corresponds to one `#[routes]`-annotated function in the
/// application. Useful for diagnostic commands (e.g., `runserver` startup banner)
/// that enumerate registered routers without executing them.
///
/// # Examples
///
/// ```rust,no_run
/// use reinhardt_urls::routers::registration::iter_registered_url_patterns;
///
/// let count = iter_registered_url_patterns().count();
/// println!("registered routers: {count}");
/// ```
#[cfg(native)]
pub fn iter_registered_url_patterns() -> impl Iterator<Item = &'static UrlPatternsRegistration> {
	inventory::iter::<UrlPatternsRegistration>()
}

/// Collect endpoint metadata after resolving every synchronous mounted router.
///
/// This inspection path never installs a global router or initializes a DI
/// context. Asynchronous factories are rejected before their factory function
/// is called because they may perform dynamic startup work.
#[cfg(native)]
pub fn collect_resolved_endpoints() -> Result<Vec<ResolvedEndpoint>, RouteTopologyError> {
	let mut endpoints = Vec::new();
	for registration in inventory::iter::<UrlPatternsRegistration>() {
		endpoints.extend(collect_resolved_endpoints_from_registration(registration)?);
	}
	sort_resolved_endpoints(&mut endpoints);
	Ok(endpoints)
}

/// Collect endpoints from one already-registered route factory.
///
/// This is public so verification callers that already discovered a
/// registration can avoid consulting global inventory again.
#[cfg(native)]
pub fn collect_resolved_endpoints_from_registration(
	registration: &UrlPatternsRegistration,
) -> Result<Vec<ResolvedEndpoint>, RouteTopologyError> {
	match &registration.factory {
		RouterFactory::Sync(factory) => collect_resolved_endpoints_from_router(&factory()),
		RouterFactory::NativeSync(factory) => {
			let routes = factory();
			let router = match &routes.server {
				NativeHttpRoutes::Owned(router) => router.as_ref(),
				NativeHttpRoutes::LegacyShared(router) => router.as_ref(),
			};
			collect_resolved_endpoints_from_router(router)
		}
		RouterFactory::Async(_) | RouterFactory::NativeAsync(_) => {
			Err(RouteTopologyError::DynamicFactory)
		}
	}
}

#[cfg(native)]
fn collect_resolved_endpoints_from_router(
	router: &ServerRouter,
) -> Result<Vec<ResolvedEndpoint>, RouteTopologyError> {
	let endpoint_metadata: std::collections::HashMap<_, _> = inventory::iter::<EndpointMetadata>()
		.map(|metadata| {
			(
				format!("{}::{}", metadata.module_path, metadata.function_name),
				metadata.clone(),
			)
		})
		.collect();
	let mut endpoints = router
		.get_mounted_route_contracts_unchecked()
		.map_err(|_| RouteTopologyError::IncompleteMetadata)?
		.into_iter()
		.map(|contract| {
			let module_path = contract
				.metadata
				.module_path
				.as_deref()
				.ok_or(RouteTopologyError::IncompleteMetadata)?;
			let function_name = contract
				.metadata
				.function_name
				.as_deref()
				.ok_or(RouteTopologyError::IncompleteMetadata)?;
			let metadata = endpoint_metadata
				.get(&format!("{module_path}::{function_name}"))
				.cloned()
				.ok_or(RouteTopologyError::IncompleteMetadata)?;
			Ok(ResolvedEndpoint {
				handler_identity: contract.metadata.handler,
				method: contract.method.to_string(),
				resolved_path: contract.path,
				metadata,
			})
		})
		.collect::<Result<Vec<_>, _>>()?;
	sort_resolved_endpoints(&mut endpoints);
	Ok(endpoints)
}

#[cfg(native)]
fn sort_resolved_endpoints(endpoints: &mut [ResolvedEndpoint]) {
	endpoints.sort_by(|left, right| {
		(
			&left.method,
			&left.resolved_path,
			left.metadata.module_path,
			left.metadata.function_name,
		)
			.cmp(&(
				&right.method,
				&right.resolved_path,
				right.metadata.module_path,
				right.metadata.function_name,
			))
	});
}

/// Client-router inventory registration (WASM target).
///
/// This module is the WASM-side counterpart of `UrlPatternsRegistration`.
/// The `#[routes]` macro submits one [`ClientRouterRegistration`] per
/// annotated function on `wasm32-unknown-unknown`, and the launcher
/// consumes them via [`collect_client_router_from_inventory`].
///
/// Also gated on the `client-router` feature because the module references
/// `crate::routers::client_router::ClientRouter`, which is itself behind
/// `#[cfg(feature = "client-router")]`. Without this guard, a WASM build
/// of `reinhardt-urls` without `client-router` enabled would fail to
/// resolve the import. Refs #4453, Codex review feedback.
#[cfg(all(
	target_family = "wasm",
	target_os = "unknown",
	feature = "client-router"
))]
mod client_registration {
	use crate::routers::client_router::ClientRouter;
	use std::sync::Arc;

	/// WASM-side counterpart of [`UrlPatternsRegistration`].
	///
	/// Submitted by the `#[routes]` macro on `wasm32-unknown-unknown`.
	///
	/// [`UrlPatternsRegistration`]: super::UrlPatternsRegistration
	#[derive(Clone)]
	pub struct ClientRouterRegistration {
		get_client_router: fn() -> Arc<ClientRouter>,
	}

	impl ClientRouterRegistration {
		/// Internal constructor used by the `#[routes]` macro.
		///
		/// Not part of the public API; do not call directly.
		#[doc(hidden)]
		pub const fn __macro_new(get_client_router: fn() -> Arc<ClientRouter>) -> Self {
			Self { get_client_router }
		}

		/// Materialize the `ClientRouter` from this registration.
		pub fn client_router(&self) -> Arc<ClientRouter> {
			(self.get_client_router)()
		}
	}

	inventory::collect!(ClientRouterRegistration);

	/// Iterate over all `#[routes]`-registered client routers.
	pub fn iter_registered_client_routers()
	-> impl Iterator<Item = &'static ClientRouterRegistration> {
		inventory::iter::<ClientRouterRegistration>()
	}

	/// Iterate inventory, merge every registered `ClientRouter`, and return
	/// the merged router (or `None` if no entries are registered).
	///
	/// `ClientRouter::merge` is `pub(crate)`; this helper lives in the
	/// same crate so the visibility holds. Refs #4442, #4453.
	pub fn collect_client_router_from_inventory() -> Option<ClientRouter> {
		let mut merged: Option<ClientRouter> = None;
		for reg in iter_registered_client_routers() {
			let arc = reg.client_router();
			let r = Arc::try_unwrap(arc).unwrap_or_else(|a| (*a).clone());
			merged = Some(match merged.take() {
				None => r,
				Some(acc) => acc.merge(r),
			});
		}
		merged
	}
}

#[cfg(all(
	target_family = "wasm",
	target_os = "unknown",
	feature = "client-router"
))]
pub use client_registration::{
	ClientRouterRegistration, collect_client_router_from_inventory, iter_registered_client_routers,
};

#[cfg(all(test, native))]
mod native_tests {
	use super::*;
	use crate::routers::{NativeHttpRoutes, NativeRouteError, NativeRoutes, UnifiedRouter};
	use hyper::Method;
	use reinhardt_core::endpoint::EndpointInfo;
	use reinhardt_di::{InjectionContext, SingletonScope};
	use reinhardt_http::{Handler, Request, Response, Result as HttpResult};

	struct FinalizedEndpoint;

	impl EndpointInfo for FinalizedEndpoint {
		fn path() -> &'static str {
			"/finalized/"
		}

		fn method() -> Method {
			Method::GET
		}

		fn name() -> &'static str {
			"native-finalized"
		}
	}

	#[async_trait::async_trait]
	impl Handler for FinalizedEndpoint {
		async fn handle(&self, _request: Request) -> HttpResult<Response> {
			Ok(Response::ok())
		}
	}

	fn legacy_sync_factory() -> Arc<ServerRouter> {
		Arc::new(ServerRouter::new().with_prefix("/legacy-sync"))
	}

	fn legacy_async_factory() -> Pin<
		Box<
			dyn Future<Output = Result<Arc<ServerRouter>, Box<dyn std::error::Error + Send + Sync>>>
				+ Send,
		>,
	> {
		Box::pin(async { Ok(Arc::new(ServerRouter::new().with_prefix("/legacy-async"))) })
	}

	fn native_sync_factory() -> NativeRoutes {
		UnifiedRouter::new().__into_native_routes()
	}

	fn native_async_factory() -> Pin<
		Box<
			dyn Future<Output = Result<NativeRoutes, Box<dyn std::error::Error + Send + Sync>>>
				+ Send,
		>,
	> {
		Box::pin(async { Ok(UnifiedRouter::new().__into_native_routes()) })
	}

	fn native_legacy_adapter_factory() -> NativeRoutes {
		let context = Arc::new(InjectionContext::builder(Arc::new(SingletonScope::new())).build());
		let mut first = reinhardt_di::DiRegistrationList::new();
		first.register(String::from("chat"));
		let mut second = reinhardt_di::DiRegistrationList::new();
		second.register(String::from("notifications"));

		let native = UnifiedRouter::new()
			.endpoint(|| FinalizedEndpoint)
			.with_di_registrations(first)
			.merge(UnifiedRouter::new().with_di_registrations(second))
			.__into_native_routes();
		match native.__with_di_context(context) {
			Ok(native) => native,
			Err(error) => panic!("test aggregate must accept its materialization context: {error}"),
		}
	}

	fn native_legacy_adapter_async_factory() -> Pin<
		Box<
			dyn Future<Output = Result<NativeRoutes, Box<dyn std::error::Error + Send + Sync>>>
				+ Send,
		>,
	> {
		Box::pin(async { Ok(native_legacy_adapter_factory()) })
	}

	fn assert_native_legacy_http_is_materialized(router: &ServerRouter) {
		assert_eq!(
			router.reverse("native-finalized", &[]),
			Some(String::from("/finalized/"))
		);
		let context = router
			.di_context()
			.expect("materialized DI context must be attached to HTTP routes");
		assert_eq!(
			context
				.singleton_scope()
				.get::<String>()
				.expect("deferred registrations must be applied")
				.as_str(),
			"notifications"
		);
	}

	#[test]
	fn legacy_factories_normalize_to_protocol_empty_shared_routes() {
		let sync = UrlPatternsRegistration::__macro_new(legacy_sync_factory).native_routes();
		let async_registration = UrlPatternsRegistration::__macro_new_async(legacy_async_factory);
		let asynchronous = tokio::runtime::Runtime::new()
			.expect("runtime must start")
			.block_on(async_registration.native_routes_async())
			.expect("legacy async factory must succeed");

		assert!(matches!(sync.server, NativeHttpRoutes::LegacyShared(_)));
		assert!(matches!(
			asynchronous.server,
			NativeHttpRoutes::LegacyShared(_)
		));
		assert_eq!(sync.websocket.len(), 0);
		assert_eq!(asynchronous.websocket.len(), 0);
		#[cfg(feature = "grpc")]
		{
			assert_eq!(sync.grpc.len(), 0);
			assert_eq!(asynchronous.grpc.len(), 0);
		}
	}

	#[test]
	fn native_factories_materialize_owned_aggregates() {
		let sync = UrlPatternsRegistration::__macro_new_native(native_sync_factory).native_routes();
		let async_registration =
			UrlPatternsRegistration::__macro_new_native_async(native_async_factory);
		let asynchronous = tokio::runtime::Runtime::new()
			.expect("runtime must start")
			.block_on(async_registration.native_routes_async())
			.expect("native async factory must succeed");

		assert!(matches!(sync.server, NativeHttpRoutes::Owned(_)));
		assert!(matches!(asynchronous.server, NativeHttpRoutes::Owned(_)));
	}

	#[test]
	fn native_routes_reject_a_different_di_context() {
		let first = Arc::new(InjectionContext::builder(Arc::new(SingletonScope::new())).build());
		let second = Arc::new(InjectionContext::builder(Arc::new(SingletonScope::new())).build());

		let exact = UnifiedRouter::new()
			.__into_native_routes()
			.__with_di_context(Arc::clone(&first))
			.expect("empty aggregate must accept the macro context");
		let conflict = UnifiedRouter::new()
			.with_di_context(second)
			.__into_native_routes()
			.__with_di_context(Arc::clone(&first));

		assert!(Arc::ptr_eq(
			exact
				.di_context
				.as_ref()
				.expect("DI context must be preserved"),
			&first,
		));
		assert_eq!(
			conflict.err().expect("different contexts must conflict"),
			NativeRouteError::ConflictingDiContext
		);
	}

	#[test]
	fn native_server_adapters_finalize_http_and_apply_deferred_di_in_order() {
		let sync = UrlPatternsRegistration::__macro_new_native(native_legacy_adapter_factory)
			.server_router();
		let asynchronous = tokio::runtime::Runtime::new()
			.expect("runtime must start")
			.block_on(
				UrlPatternsRegistration::__macro_new_native_async(
					native_legacy_adapter_async_factory,
				)
				.server_router_async(),
			)
			.expect("native async adapter must materialize");

		assert_native_legacy_http_is_materialized(&sync);
		assert_native_legacy_http_is_materialized(&asynchronous);
	}
}
