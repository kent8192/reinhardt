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
//! use hyper::Method;
//! # use reinhardt_http::{Request, Response, Result};
//!
//! # async fn handler(_req: Request) -> Result<Response> {
//! #     Ok(Response::ok())
//! # }
//! let router = UnifiedRouter::new()
//!     .function("/api/users", Method::GET, handler)
//!     .with_middleware(LoggingMiddleware::new());
//! ```
//!
//! ## Route Group Middleware
//!
//! ```rust,no_run
//! use reinhardt_urls::routers::RouteGroup;
//! use reinhardt_middleware::LoggingMiddleware;
//! use hyper::Method;
//! # use reinhardt_http::{Request, Response, Result};
//!
//! # async fn users_list(_req: Request) -> Result<Response> {
//! #     Ok(Response::ok())
//! # }
//! # async fn users_detail(_req: Request) -> Result<Response> {
//! #     Ok(Response::ok())
//! # }
//! // Create a group with middleware
//! let group = RouteGroup::new()
//!     .with_prefix("/api/v1")
//!     .with_middleware(LoggingMiddleware::new())
//!     .function("/users", Method::GET, users_list)
//!     .function("/users/{id}", Method::GET, users_detail);
//!
//! let router = group.build();
//! ```

// Client router (WASM-compatible)
#[cfg(feature = "client-router")]
pub mod client_router;

// Server-only modules (not available on WASM)
/// Route matching result cache for repeated lookups.
#[cfg(not(target_arch = "wasm32"))]
pub mod cache;
/// Path parameter type converters (integer, UUID, slug, date, etc.).
#[cfg(not(target_arch = "wasm32"))]
pub mod converters;
/// Helper functions for building routes (similar to Django's `path()` and `re_path()`).
#[cfg(not(target_arch = "wasm32"))]
pub mod helpers;
/// Route introspection and analysis utilities.
#[cfg(not(target_arch = "wasm32"))]
pub mod introspection;
/// Hierarchical namespace management for URL resolution.
#[cfg(not(target_arch = "wasm32"))]
pub mod namespace;
/// OpenAPI specification generation from registered routes.
#[cfg(not(target_arch = "wasm32"))]
pub mod openapi_integration;
/// Path pattern matching and radix tree routing.
#[cfg(not(target_arch = "wasm32"))]
pub mod pattern;
/// Compile-time URL pattern registration via `inventory`.
#[cfg(not(target_arch = "wasm32"))]
pub mod registration;
/// URL reverse resolution (name-to-URL mapping).
#[cfg(not(target_arch = "wasm32"))]
pub mod reverse;
/// Route definition combining path patterns with handlers.
#[cfg(not(target_arch = "wasm32"))]
pub mod route;
/// Route grouping with shared prefix and middleware.
#[cfg(not(target_arch = "wasm32"))]
pub mod route_group;
/// Router trait and default implementation.
#[cfg(not(target_arch = "wasm32"))]
pub mod router;
/// SCRIPT_NAME prefix management for reverse proxy deployments.
#[cfg(not(target_arch = "wasm32"))]
pub mod script_prefix;
/// Full HTTP routing implementation with global router management.
#[cfg(not(target_arch = "wasm32"))]
pub mod server_router;
/// Minimal router for simple routing use cases.
#[cfg(not(target_arch = "wasm32"))]
pub mod simple;
/// Unified router combining server and client routing.
#[cfg(not(target_arch = "wasm32"))]
pub mod unified_router;
/// Route map visualization in multiple formats (tree, DOT, Markdown).
#[cfg(not(target_arch = "wasm32"))]
pub mod visualization;

// Re-export the path! macro for compile-time path validation
#[cfg(not(target_arch = "wasm32"))]
pub use reinhardt_routers_macros::path;

#[cfg(not(target_arch = "wasm32"))]
pub use cache::RouteCache;
#[cfg(not(target_arch = "wasm32"))]
pub use converters::{
	Converter, ConverterError, ConverterResult, DateConverter, FloatConverter, IntegerConverter,
	PathConverter, SlugConverter, UuidConverter,
};
#[cfg(not(target_arch = "wasm32"))]
pub use helpers::{IncludedRouter, include_routes, path, re_path};
#[cfg(not(target_arch = "wasm32"))]
pub use pattern::{MatchingMode, PathMatcher, PathPattern, RadixRouter, RadixRouterError};
#[cfg(not(target_arch = "wasm32"))]
pub use registration::UrlPatternsRegistration;
#[cfg(not(target_arch = "wasm32"))]
pub use reverse::{
	ReverseError,
	ReverseResult,
	UrlParams,
	// Type-safe reversal
	UrlPattern,
	UrlPatternWithParams,
	UrlReverser,
	extract_param_names,
	reverse,
	reverse_single_pass,
	reverse_typed,
	reverse_typed_with_params,
	reverse_with_aho_corasick,
};
#[cfg(not(target_arch = "wasm32"))]
pub use route::Route;
#[cfg(not(target_arch = "wasm32"))]
pub use route_group::{RouteGroup, RouteInfo};
#[cfg(not(target_arch = "wasm32"))]
pub use router::{DefaultRouter, Router};
#[cfg(not(target_arch = "wasm32"))]
pub use script_prefix::{clear_script_prefix, get_script_prefix, set_script_prefix};
#[cfg(not(target_arch = "wasm32"))]
pub use simple::SimpleRouter;
// Server router (full HTTP routing implementation)
#[cfg(not(target_arch = "wasm32"))]
pub use server_router::{
	FunctionHandler, ServerRouter, clear_router, get_router, is_router_registered, register_router,
	register_router_arc,
};

// Unified router (closure-based API combining server and client routers)
#[cfg(not(target_arch = "wasm32"))]
pub use unified_router::UnifiedRouter;

// Client router re-exports
#[cfg(feature = "client-router")]
pub use client_router::{
	ClientPathPattern, ClientRoute, ClientRouteMatch, ClientRouter, FromPath, HistoryState,
	NavigationType, ParamContext, Path, RouteHandler, SingleFromPath,
};
