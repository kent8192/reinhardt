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
//!         .client(|c| c.route("/", home_page))
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
//!         .client(|c| c.route("/", home_page))
//! }
//! ```
//!
//! The `#[routes]` macro automatically handles `inventory` registration,
//! so you don't need any additional boilerplate code.
//!
//! [`UnifiedRouter`]: crate::routers::UnifiedRouter
//! [`ServerRouter`]: crate::routers::ServerRouter

#[cfg(feature = "client-router")]
use crate::routers::client_router::ClientRouter;
use crate::routers::server_router::ServerRouter;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

/// Function pointer type for async router factories.
///
/// Returns a pinned, boxed future that produces a server router or an error.
/// Used by `RouterFactory::Async` and `UrlPatternsRegistration::__macro_new_async`.
pub type AsyncRouterFactoryFn = fn() -> Pin<
	Box<
		dyn Future<Output = Result<Arc<ServerRouter>, Box<dyn std::error::Error + Send + Sync>>>
			+ Send,
	>,
>;

/// Factory for creating server routers, supporting both sync and async creation.
///
/// The sync variant is used by existing `#[routes]` functions that return
/// `UnifiedRouter` synchronously. The async variant is used when `#[routes]`
/// is applied to an `async fn`, enabling DI resolution via `#[inject]` parameters.
#[derive(Clone)]
pub enum RouterFactory {
	/// Synchronous factory (existing behavior for `fn routes() -> UnifiedRouter`)
	Sync(fn() -> Arc<ServerRouter>),
	/// Async factory for `async fn routes()` with optional `#[inject]` DI resolution
	Async(AsyncRouterFactoryFn),
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
#[derive(Clone)]
pub struct UrlPatternsRegistration {
	/// Router factory (sync or async)
	///
	/// The `#[routes]` macro extracts the server router from [`UnifiedRouter`]
	/// using `into_server()` and wraps it in `Arc::new()` automatically.
	/// Sync factories are used for `fn routes()`, async factories for
	/// `async fn routes()` (with optional `#[inject]` DI parameters).
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
			RouterFactory::Async(_) => {
				panic!(
					"Cannot call server_router() on an async #[routes] registration. \
					 Use server_router_async() instead."
				)
			}
		}
	}

	/// Get the server router from the registration, supporting both sync and async factories.
	pub async fn server_router_async(
		&self,
	) -> Result<Arc<ServerRouter>, Box<dyn std::error::Error + Send + Sync>> {
		match &self.factory {
			RouterFactory::Sync(f) => Ok(f()),
			RouterFactory::Async(f) => f().await,
		}
	}

	/// Get the client router from the registration, if available
	#[cfg(feature = "client-router")]
	pub fn client_router(&self) -> Option<Arc<ClientRouter>> {
		self.get_client_router.map(|f| f())
	}
}

// Collect registrations for runtime iteration
inventory::collect!(UrlPatternsRegistration);
