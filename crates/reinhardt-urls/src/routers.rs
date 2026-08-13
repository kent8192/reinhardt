//! # Reinhardt Routers
//!
//! URL routing for Reinhardt framework with advanced features:
//!
//! - **Namespace-based URL reversal**: Hierarchical route naming (`"api:v1:users:detail"`)
//! - **Nested namespace resolution**: Parent-child namespace relationships
//! - **Route introspection**: Runtime route analysis and debugging
//! - **OpenAPI integration**: Automatic OpenAPI schema generation from routes
//! - **Route visualization**: Generate route maps for documentation (ASCII, DOT, Markdown)
//! - **Per-route middleware**: Apply middleware to specific routes
//! - **Route group middleware**: Apply middleware to groups of routes
//!
//! # Examples
//!
//! ## Basic Routing
//!
//! ```
//! use reinhardt_urls::routers::{UnifiedRouter, Route};
//! use hyper::Method;
//!
//! let router = UnifiedRouter::new()
//!     .with_prefix("/api/v1")
//!     .with_namespace("v1");
//! ```
//!
//! ## Namespace-based URL Reversal
//!
//! ```
//! use reinhardt_urls::routers::namespace::{NamespaceResolver, Namespace};
//!
//! let mut resolver = NamespaceResolver::new();
//! resolver.register("api:v1:users:detail", "/api/v1/users/{id}/");
//!
//! let url = resolver.resolve("api:v1:users:detail", &[("id", "123")]).unwrap();
//! assert_eq!(url, "/api/v1/users/123/");
//! ```
//!
//! ## Route Introspection
//!
//! ```
//! use reinhardt_urls::routers::introspection::RouteInspector;
//! use hyper::Method;
//!
//! let mut inspector = RouteInspector::new();
//! inspector.add_route("/api/users/", vec![Method::GET], Some("api:users:list"), None);
//!
//! let routes = inspector.find_by_namespace("api");
//! assert_eq!(routes.len(), 1);
//! ```
//!
//! ## Route Visualization
//!
//! ```
//! use reinhardt_urls::routers::visualization::{RouteVisualizer, VisualizationFormat};
//! use reinhardt_urls::routers::introspection::RouteInspector;
//! use hyper::Method;
//!
//! let mut inspector = RouteInspector::new();
//! inspector.add_route("/users/", vec![Method::GET], Some("users:list"), None);
//!
//! let visualizer = RouteVisualizer::from_inspector(&inspector);
//! let tree = visualizer.render(VisualizationFormat::Tree);
//! println!("{}", tree);
//! ```
//!
//! ## Per-Route Middleware
//!
//! ```rust,no_run
//! use reinhardt_urls::routers::UnifiedRouter;
//! use reinhardt_middleware::LoggingMiddleware;
//! # use hyper::Method;
//! # use reinhardt_core::endpoint::EndpointInfo;
//! # use reinhardt_http::{Handler, Request, Response, Result};
//!
//! # struct Users;
//! # impl EndpointInfo for Users {
//! #     fn path() -> &'static str { "/api/users" }
//! #     fn method() -> Method { Method::GET }
//! #     fn name() -> &'static str { "api-users" }
//! # }
//! # #[async_trait::async_trait]
//! # impl Handler for Users {
//! #     async fn handle(&self, _req: Request) -> Result<Response> { Ok(Response::ok()) }
//! # }
//! let router = UnifiedRouter::new()
//!     .endpoint(|| Users)
//!     .with_middleware(LoggingMiddleware::new());
//! ```
//!
//! ## Route Group Middleware
//!
//! ```rust,no_run
//! use reinhardt_urls::routers::RouteGroup;
//! use reinhardt_middleware::LoggingMiddleware;
//! # use hyper::Method;
//! # use reinhardt_core::endpoint::EndpointInfo;
//! # use reinhardt_http::{Handler, Request, Response, Result};
//!
//! # struct UsersList;
//! # struct UsersDetail;
//! # impl EndpointInfo for UsersList {
//! #     fn path() -> &'static str { "/users" }
//! #     fn method() -> Method { Method::GET }
//! #     fn name() -> &'static str { "users-list" }
//! # }
//! # impl EndpointInfo for UsersDetail {
//! #     fn path() -> &'static str { "/users/{id}" }
//! #     fn method() -> Method { Method::GET }
//! #     fn name() -> &'static str { "users-detail" }
//! # }
//! # #[async_trait::async_trait]
//! # impl Handler for UsersList {
//! #     async fn handle(&self, _req: Request) -> Result<Response> { Ok(Response::ok()) }
//! # }
//! # #[async_trait::async_trait]
//! # impl Handler for UsersDetail {
//! #     async fn handle(&self, _req: Request) -> Result<Response> { Ok(Response::ok()) }
//! # }
//! // Create a group with middleware
//! let group = RouteGroup::new()
//!     .with_prefix("/api/v1")
//!     .with_middleware(LoggingMiddleware::new())
//!     .endpoint(|| UsersList)
//!     .endpoint(|| UsersDetail);
//!
//! let router = group.build();
//! ```

// Client router (WASM-compatible)
#[cfg(feature = "client-router")]
pub mod client_router;

// Server-only modules (not available on WASM)
/// Route matching result cache for repeated lookups.
#[cfg(native)]
pub mod cache;
/// Path parameter type converters (integer, UUID, slug, date, etc.).
#[cfg(native)]
pub mod converters;
/// Helper functions for building routes (similar to Django's `path()` and `re_path()`).
#[cfg(native)]
pub mod helpers;
/// Route introspection and analysis utilities.
#[cfg(native)]
pub mod introspection;
/// Hierarchical namespace management for URL resolution.
#[cfg(native)]
pub mod namespace;
/// Complete native route payload used by registration and server startup.
#[cfg(native)]
pub mod native_routes;
/// OpenAPI specification generation from registered routes.
#[cfg(native)]
pub mod openapi_integration;
/// URL path joining and normalization utilities.
#[cfg(native)]
pub(crate) mod path_utils;
/// Path pattern matching and radix tree routing.
#[cfg(native)]
pub mod pattern;
/// Compile-time URL pattern registration via `inventory`.
///
/// On native targets exposes `UrlPatternsRegistration`; on
/// `wasm32-unknown-unknown` exposes `ClientRouterRegistration` and
/// `collect_client_router_from_inventory` (refs #4453).
#[cfg(any(native, all(target_family = "wasm", target_os = "unknown")))]
pub mod registration;
/// URL resolver trait for type-safe URL generation.
pub mod resolver;
/// URL reverse resolution (name-to-URL mapping).
#[cfg(native)]
pub mod reverse;
/// Route definition combining path patterns with handlers.
#[cfg(native)]
pub mod route;
/// Route grouping with shared prefix and middleware.
#[cfg(native)]
pub mod route_group;
/// Router trait and default implementation.
#[cfg(native)]
pub mod router;
/// SCRIPT_NAME prefix management for reverse proxy deployments.
#[cfg(native)]
pub mod script_prefix;
/// Full HTTP routing implementation with global router management.
#[cfg(native)]
pub mod server_router;
/// Minimal router for simple routing use cases.
#[cfg(native)]
pub mod simple;
/// Unified router combining server and client routing.
#[cfg(any(native, feature = "client-router"))]
pub mod unified_router;
/// `VersionedRouter` trait impls bridging concrete routers to the
/// `reinhardt-router` crate (issue #4321).
#[cfg(native)]
pub mod versioned;
/// Route map visualization in multiple formats (tree, DOT, Markdown).
#[cfg(native)]
pub mod visualization;

// Re-export the path! macro for compile-time path validation.
#[cfg(all(native, feature = "routers-macros"))]
pub use reinhardt_routers_macros::path;

#[cfg(native)]
pub use cache::RouteCache;
#[cfg(native)]
pub use converters::{
	Converter, ConverterError, ConverterResult, DateConverter, FloatConverter, IntegerConverter,
	PathConverter, SlugConverter, UuidConverter,
};
#[cfg(native)]
pub use helpers::{IncludedRouter, include_routes, path, re_path};
#[cfg(native)]
pub use native_routes::{NativeHttpRoutes, NativeRouteError, NativeRoutes};
#[cfg(native)]
pub use pattern::{MatchingMode, PathMatcher, PathPattern, RadixRouter, RadixRouterError};
#[cfg(native)]
pub use registration::{
	AsyncNativeRouterFactoryFn, NativeRouterFactoryFn, RouterFactory, UrlPatternsRegistration,
};
#[cfg(all(
	target_family = "wasm",
	target_os = "unknown",
	feature = "client-router"
))]
pub use registration::{
	ClientRouterRegistration, collect_client_router_from_inventory, iter_registered_client_routers,
};
#[cfg(native)]
pub use reverse::{
	ReverseError, ReverseResult, UrlReverser, extract_param_names, reverse,
	try_reverse_single_pass, try_reverse_with_aho_corasick,
};
#[cfg(native)]
pub use route::Route;
#[cfg(native)]
pub use route_group::{RouteGroup, RouteInfo};
#[cfg(native)]
pub use router::{DefaultRouter, Router};
#[cfg(native)]
pub use script_prefix::{clear_script_prefix, get_script_prefix, set_script_prefix};
#[cfg(native)]
pub use simple::SimpleRouter;
// Server router (full HTTP routing implementation)
#[cfg(native)]
pub use server_router::{
	MiddlewareInfo, ServerRouter, clear_router, get_router, get_router_di_context,
	is_router_registered, register_di_registrations, register_router, register_router_arc,
	take_di_registrations,
};

// On WASM with `client-router`, `ServerRouter` is the no-op builder defined in
// `unified_router`. Re-exported here so `reinhardt_urls::routers::ServerRouter`
// resolves uniformly on both targets (issue #4569). The native re-export above
// and this one are gated on mutually exclusive cfgs, so exactly one is active.
#[cfg(all(wasm, feature = "client-router"))]
pub use unified_router::ServerRouter;

// Unified router (closure-based API combining server and client routers)
#[cfg(any(native, feature = "client-router"))]
pub use unified_router::UnifiedRouter;

// Client router re-exports
#[cfg(feature = "client-router")]
pub use client_router::{
	ClientPathPattern, ClientRoute, ClientRouteMatch, ClientRouter, FromPath, HistoryState,
	MergeError, NavigationSubscription, NavigationType, ParamContext, Path, RouteHandler,
	RouteLoaderId, RouteMetadata, RouteRegistrationError, RouteScope, ScopeKind, SingleFromPath,
};
pub use resolver::{ClientUrlResolver, WebSocketUrlResolver};

// Re-export the canonical `StreamingTopicResolver` trait from
// `reinhardt-streaming` so downstream users can keep importing it from
// `reinhardt_urls::routers`.
#[cfg(feature = "streaming")]
pub use reinhardt_streaming::StreamingTopicResolver;
