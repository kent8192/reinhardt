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
//! 2. Macro generates an `inventory::submit!` call with function pointers for both routers
//! 3. Framework code retrieves registrations via `inventory::iter::<UrlPatternsRegistration>()`
//! 4. Framework calls the registered functions to get [`ServerRouter`] and [`ClientRouter`]
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
//! [`ClientRouter`]: crate::routers::ClientRouter

#[cfg(feature = "client-router")]
use crate::routers::client_router::ClientRouter;
use crate::routers::server_router::ServerRouter;
use std::sync::Arc;

/// URL patterns registration for compile-time discovery
///
/// This type is used with the `inventory` crate to register URL pattern
/// functions at compile time, allowing the framework to automatically
/// discover and register routers without manual boilerplate in management
/// commands like `runserver` or `check`.
///
/// # Fields
///
/// * `get_server_router` - Function pointer to get the server router
/// * `get_client_router` - Function pointer to get the client router (when `client-router` feature is enabled)
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
	/// Function to get the server router
	///
	/// This function returns an `Arc<ServerRouter>` with all server-side routes.
	/// The `#[routes]` macro extracts the server router from [`UnifiedRouter`]
	/// using `into_server()` and wraps it in `Arc::new()` automatically.
	///
	/// [`UnifiedRouter`]: crate::routers::UnifiedRouter
	pub get_server_router: fn() -> Arc<ServerRouter>,

	/// Function to get the client router
	///
	/// This function returns an `Arc<ClientRouter>` with all client-side routes.
	/// The `#[routes]` macro extracts the client router from [`UnifiedRouter`]
	/// using `into_client()` and wraps it in `Arc::new()` automatically.
	///
	/// [`UnifiedRouter`]: crate::routers::UnifiedRouter
	#[cfg(feature = "client-router")]
	pub get_client_router: fn() -> Arc<ClientRouter>,
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
	///     || Arc::new(routes().into_client()),
	/// );
	/// ```
	///
	/// # Note
	///
	/// You typically don't call this directly. Use the `#[routes]` macro instead.
	#[cfg(feature = "client-router")]
	pub const fn new(
		get_server_router: fn() -> Arc<ServerRouter>,
		get_client_router: fn() -> Arc<ClientRouter>,
	) -> Self {
		Self {
			get_server_router,
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
		Self { get_server_router }
	}

	/// Get the server router from the registration
	pub fn server_router(&self) -> Arc<ServerRouter> {
		(self.get_server_router)()
	}

	/// Get the client router from the registration
	#[cfg(feature = "client-router")]
	pub fn client_router(&self) -> Arc<ClientRouter> {
		(self.get_client_router)()
	}
}

// Collect registrations for runtime iteration
inventory::collect!(UrlPatternsRegistration);
