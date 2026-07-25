//! Unified Router with hierarchical routing support
//!
//! This module provides a unified router that supports:
//! - **High-performance O(m) route matching** using matchit Radix Tree (m = path length)
//! - Nested routers with automatic prefix inheritance
//! - Namespace-based URL reversal
//! - Middleware and DI context propagation
//! - Integration with ViewSets, HTTP macro endpoints, and class-based views
//!
//! # Performance Characteristics
//!
//! The router uses [matchit](https://docs.rs/matchit) for O(m) route matching where m is the path length:
//! - Route lookup: O(m) - Independent of the number of registered routes
//! - Route compilation: O(n) - Done once at startup where n is the number of routes
//! - Memory: Efficient through Radix Tree's prefix sharing
//!
//! With 1000+ routes, matchit provides 3-5x better performance compared to naive O(n×m) linear search.
//!
//! # Implementation Details
//!
//! Each HTTP method has its own matchit router for optimal performance:
//! - `GET`, `POST`, `PUT`, `DELETE`, `PATCH`, `HEAD`, `OPTIONS`
//! - Routes are compiled lazily on first access into an immutable route table
//! - Parameters are extracted directly from matchit's Params
//!
//! # Module Layout
//!
//! The implementation is split across focused submodules to keep each file
//! small and reviewable:
//!
//! - `types`    — `MiddlewareInfo`, `RouteInfo`, `FunctionRoute`, `ViewRoute`,
//!   `RouteHandler`, `RouteMatch`, and the `join_path` helper
//! - `builder` — constructors and builder-style configuration
//!   (`new`, `with_prefix`, `with_namespace`, `with_di_context`,
//!   `with_middleware`, `exclude`, `mount`, `group`)
//! - `registration` — route registration entry points
//!   (`endpoint`, `handler`, `viewset`, `view`, `with_route_middleware`)
//! - `compile` — matchit compilation and `validate_*` helpers
//! - `introspection` — read-only accessors, `get_all_routes`,
//!   `register_all_routes`, `reverse`
//! - `dispatch` — `resolve`, `match_own_routes_*`, `path_exists_for_any_method`
//! - `router_impls` — `Debug`, `Default`, `Handler`, `RegisterViewSet`
//! - `handlers` — ViewSet handler adapters
//! - `matching` — `path_matches` and `extract_params` utilities
//! - `global`   — global router registry used by `showurls`

use crate::routers::UrlReverser;
use reinhardt_di::InjectionContext;
use reinhardt_middleware::Middleware;
#[cfg(feature = "viewsets")]
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

pub mod global;
mod handlers;
mod matching;

mod builder;
mod compile;
mod dispatch;
mod introspection;
mod registration;
mod router_impls;
mod types;

#[cfg(test)]
mod tests;

use self::types::{CompiledRoutes, FunctionRoute, ViewRoute};

pub use self::global::{
	clear_router, get_router, get_router_di_context, is_router_registered,
	register_di_registrations, register_router, register_router_arc, take_di_registrations,
};
pub use self::matching::{extract_params, path_matches};
pub use self::types::{MiddlewareInfo, RegisteredEndpoint, RouteInfo, RouteOrigin};

/// Unified router with hierarchical routing support
///
/// Supports multiple API styles:
/// - HTTP method macro endpoints
/// - DRF-style: ViewSets with automatic CRUD
/// - Django-style: Class-based views
///
/// # Examples
///
/// ```
/// use reinhardt_urls::routers::ServerRouter;
/// # use hyper::Method;
/// # use reinhardt_core::endpoint::EndpointInfo;
/// # use reinhardt_http::{Handler, Request, Response, Result};
///
/// # struct ExportUsers;
/// # struct Health;
/// # impl EndpointInfo for ExportUsers {
/// #     fn path() -> &'static str { "/export/" }
/// #     fn method() -> Method { Method::GET }
/// #     fn name() -> &'static str { "users-export" }
/// # }
/// # impl EndpointInfo for Health {
/// #     fn path() -> &'static str { "/health/" }
/// #     fn method() -> Method { Method::GET }
/// #     fn name() -> &'static str { "health" }
/// # }
/// # #[async_trait::async_trait]
/// # impl Handler for ExportUsers {
/// #     async fn handle(&self, _req: Request) -> Result<Response> { Ok(Response::ok()) }
/// # }
/// # #[async_trait::async_trait]
/// # impl Handler for Health {
/// #     async fn handle(&self, _req: Request) -> Result<Response> { Ok(Response::ok()) }
/// # }
///
/// # async fn example() -> Result<()> {
/// // Create a users sub-router
/// let users_router = ServerRouter::new()
///     .with_namespace("users")
///     .endpoint(|| ExportUsers);
///
/// // Verify users router has namespace
/// assert_eq!(users_router.namespace(), Some("users"));
///
/// // Create root router
/// let router = ServerRouter::new()
///     .with_prefix("/api/v1/")
///     .with_namespace("v1")
///     .endpoint(|| Health)
///     .mount("/users/", users_router);
///
/// // Verify root router configuration
/// assert_eq!(router.prefix(), "/api/v1/");
/// assert_eq!(router.namespace(), Some("v1"));
///
/// // Generated URLs:
/// // /api/v1/health/
/// // /api/v1/users/export/
/// # Ok(())
/// # }
/// # tokio::runtime::Runtime::new().unwrap().block_on(example()).unwrap();
/// ```
pub struct ServerRouter {
	/// Router's prefix path
	pub(crate) prefix: String,

	/// Namespace for URL reversal
	pub(crate) namespace: Option<String>,

	/// Routes defined in this router
	pub(crate) routes: Vec<crate::routers::Route>,

	/// ViewSet registrations
	#[cfg(feature = "viewsets")]
	pub(crate) viewsets: HashMap<String, Arc<dyn reinhardt_views::viewsets::ViewSet>>,

	/// Function-based routes
	pub(crate) functions: Vec<FunctionRoute>,

	/// Configuration errors reported by router extensions.
	pub(crate) configuration_errors: Vec<String>,

	/// Class-based view routes
	pub(crate) views: Vec<ViewRoute>,

	/// Child routers
	pub(crate) children: Vec<ServerRouter>,

	/// DI context
	pub(crate) di_context: Option<Arc<InjectionContext>>,

	/// Middleware-contributed DI singleton registrations that have been
	/// harvested by [`Self::with_middleware`] but not yet applied. Filled
	/// when [`Self::with_middleware`] runs before [`Self::with_di_context`];
	/// drained either by a later [`Self::with_di_context`] call (into that
	/// context's `SingletonScope`) or, if no context is ever attached, by
	/// [`Self::register_all_routes`] (into the global deferred-registration
	/// list). This avoids both the silent drop and the global-list leak
	/// described in #4426.
	pub(crate) pending_middleware_di: reinhardt_di::DiRegistrationList,

	/// Middleware stack
	pub(crate) middleware: Vec<Arc<dyn Middleware>>,

	/// Middleware type information for runtime introspection
	pub(crate) middleware_names: Vec<MiddlewareInfo>,

	/// Per-middleware exclusion patterns, indexed parallel to `middleware` vec.
	/// Each entry contains the exclusion path patterns for the corresponding middleware.
	pub(crate) middleware_exclusions: Vec<Vec<String>>,

	/// URL reverser
	pub(crate) reverser: UrlReverser,

	/// Cached immutable route table and route compilation errors.
	pub(crate) compiled_routes: OnceLock<CompiledRoutes>,
}
