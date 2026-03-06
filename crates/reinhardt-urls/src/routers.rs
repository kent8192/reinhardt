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

pub mod cache;
#[cfg(feature = "client-router")]
pub mod client_router;
pub mod converters;
pub mod helpers;
pub mod introspection;
pub mod namespace;
pub mod openapi_integration;
pub mod pattern;
pub mod registration;
pub mod reverse;
pub mod route;
pub mod route_group;
pub mod router;
pub mod script_prefix;
pub mod server_router;
pub mod simple;
pub mod unified_router;
pub mod visualization;

// Re-export the path! macro for compile-time path validation
pub use reinhardt_routers_macros::path;

pub use cache::RouteCache;
pub use converters::{
	Converter, ConverterError, ConverterResult, DateConverter, FloatConverter, IntegerConverter,
	PathConverter, SlugConverter, UuidConverter,
};
pub use helpers::{IncludedRouter, include_routes, path, re_path};
pub use pattern::{MatchingMode, PathMatcher, PathPattern, RadixRouter, RadixRouterError};
pub use registration::UrlPatternsRegistration;
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
pub use route::Route;
pub use route_group::{RouteGroup, RouteInfo};
pub use router::{DefaultRouter, Router};
pub use script_prefix::{clear_script_prefix, get_script_prefix, set_script_prefix};
pub use simple::SimpleRouter;
// Server router (full HTTP routing implementation)
pub use server_router::{
	FunctionHandler, ServerRouter, clear_router, get_router, is_router_registered, register_router,
	register_router_arc,
};

// Unified router (closure-based API combining server and client routers)
pub use unified_router::UnifiedRouter;

// Client router re-exports
#[cfg(feature = "client-router")]
pub use client_router::{
	ClientPathPattern, ClientRoute, ClientRouteMatch, ClientRouter, FromPath, HistoryState,
	NavigationType, ParamContext, Path, RouteHandler, SingleFromPath,
};
