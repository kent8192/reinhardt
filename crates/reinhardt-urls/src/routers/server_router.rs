//! Unified Router with hierarchical routing support
//!
//! This module provides a unified router that supports:
//! - **High-performance O(m) route matching** using matchit Radix Tree (m = path length)
//! - Nested routers with automatic prefix inheritance
//! - Namespace-based URL reversal
//! - Middleware and DI context propagation
//! - Integration with ViewSets, functions, and class-based views
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
//! - Routes are compiled lazily on first access (thread-safe with RwLock)
//! - Parameters are extracted directly from matchit's Params

use super::{Route, UrlReverser};
use async_trait::async_trait;
use hyper::Method;
use matchit::Router as MatchitRouter;
use reinhardt_core::endpoint::EndpointInfo;
use reinhardt_di::InjectionContext;
use reinhardt_http::{
	Error, ExcludeMiddleware, Handler, MiddlewareChain, Request, Response, Result,
};
use reinhardt_middleware::Middleware;
use reinhardt_views::viewsets::{Action, ViewSet};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::{Arc, PoisonError, RwLock};

pub use self::global::{
	clear_router, get_router, get_router_di_context, is_router_registered,
	register_di_registrations, register_router, register_router_arc, take_di_registrations,
};
pub use self::handlers::FunctionHandler;
pub use self::matching::{extract_params, path_matches};

pub(crate) use self::handlers::ViewSetHandler;

pub mod global;
mod handlers;
mod matching;

/// Information about a registered middleware
///
/// Captures the short name and full type path of a middleware added via
/// [`ServerRouter::with_middleware()`]. This enables runtime introspection
/// of the middleware stack without requiring `Middleware` to be `Debug`.
///
/// # Examples
///
/// ```
/// use reinhardt_urls::routers::server_router::MiddlewareInfo;
///
/// let info = MiddlewareInfo {
///     name: "LoggingMiddleware".to_string(),
///     type_name: "reinhardt_middleware::LoggingMiddleware".to_string(),
/// };
/// assert_eq!(info.name, "LoggingMiddleware");
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MiddlewareInfo {
	/// Short name of the middleware (last segment of the type path)
	pub name: String,

	/// Full type path (e.g., `"reinhardt_middleware::logging::LoggingMiddleware"`)
	pub type_name: String,
}

/// Route information tuple: (path, name, namespace, methods)
pub type RouteInfo = Vec<(String, Option<String>, Option<String>, Vec<Method>)>;

/// Join two path segments, normalizing any double slashes.
///
/// Concatenates `prefix` and `suffix`, then collapses consecutive `/` characters
/// into a single `/`. The leading slash is always preserved.
///
/// # Examples
///
/// ```ignore
/// // crate-internal usage only
/// assert_eq!(join_path("/api/", "/users"), "/api/users");
/// assert_eq!(join_path("/api", "/users"), "/api/users");
/// assert_eq!(join_path("/api", "users"), "/apiusers");
/// assert_eq!(join_path("", "/users"), "/users");
/// ```
pub(crate) fn join_path(prefix: &str, suffix: &str) -> String {
	let raw = format!("{}{}", prefix, suffix);
	let mut result = String::with_capacity(raw.len());
	let mut prev_slash = false;
	for ch in raw.chars() {
		if ch == '/' {
			if !prev_slash {
				result.push(ch);
			}
			prev_slash = true;
		} else {
			result.push(ch);
			prev_slash = false;
		}
	}
	result
}

/// Handler information stored in matchit router
#[derive(Clone)]
struct RouteHandler {
	/// The actual handler
	handler: Arc<dyn Handler>,

	/// Route-level middleware
	middleware: Vec<Arc<dyn Middleware>>,
}

/// Route match result with metadata
#[derive(Clone)]
pub(crate) struct RouteMatch {
	/// Matched handler
	pub handler: Arc<dyn Handler>,

	/// Extracted path parameters
	pub params: HashMap<String, String>,

	/// Middleware stack to apply (parent → child order)
	pub middleware_stack: Vec<Arc<dyn Middleware>>,

	/// DI context
	pub di_context: Option<Arc<InjectionContext>>,
}

/// Unified router with hierarchical routing support
///
/// Supports multiple API styles:
/// - FastAPI-style: Function-based routes
/// - DRF-style: ViewSets with automatic CRUD
/// - Django-style: Class-based views
///
/// # Examples
///
/// ```
/// use reinhardt_urls::routers::ServerRouter;
/// use hyper::Method;
/// # use reinhardt_http::{Request, Response, Result};
///
/// # async fn example() -> Result<()> {
/// // Create a users sub-router
/// let users_router = ServerRouter::new()
///     .with_namespace("users")
///     .function("/export/", Method::GET, |_req| async { Ok(Response::ok()) });
///
/// // Verify users router has namespace
/// assert_eq!(users_router.namespace(), Some("users"));
///
/// // Create root router
/// let router = ServerRouter::new()
///     .with_prefix("/api/v1/")
///     .with_namespace("v1")
///     .function("/health/", Method::GET, |_req| async { Ok(Response::ok()) })
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
	prefix: String,

	/// Namespace for URL reversal
	namespace: Option<String>,

	/// Routes defined in this router
	routes: Vec<Route>,

	/// ViewSet registrations
	viewsets: HashMap<String, Arc<dyn ViewSet>>,

	/// Function-based routes
	functions: Vec<FunctionRoute>,

	/// Class-based view routes
	views: Vec<ViewRoute>,

	/// Child routers
	children: Vec<ServerRouter>,

	/// DI context
	di_context: Option<Arc<InjectionContext>>,

	/// Middleware stack
	middleware: Vec<Arc<dyn Middleware>>,

	/// Middleware type information for runtime introspection
	middleware_names: Vec<MiddlewareInfo>,

	/// Per-middleware exclusion patterns, indexed parallel to `middleware` vec.
	/// Each entry contains the exclusion path patterns for the corresponding middleware.
	middleware_exclusions: Vec<Vec<String>>,

	/// URL reverser
	reverser: UrlReverser,

	/// Matchit router for GET requests (uses RwLock for thread-safe lazy compilation)
	get_router: RwLock<MatchitRouter<RouteHandler>>,

	/// Matchit router for POST requests
	post_router: RwLock<MatchitRouter<RouteHandler>>,

	/// Matchit router for PUT requests
	put_router: RwLock<MatchitRouter<RouteHandler>>,

	/// Matchit router for DELETE requests
	delete_router: RwLock<MatchitRouter<RouteHandler>>,

	/// Matchit router for PATCH requests
	patch_router: RwLock<MatchitRouter<RouteHandler>>,

	/// Matchit router for HEAD requests
	head_router: RwLock<MatchitRouter<RouteHandler>>,

	/// Matchit router for OPTIONS requests
	options_router: RwLock<MatchitRouter<RouteHandler>>,

	/// Flag indicating if routes have been compiled (uses RwLock for thread-safety)
	routes_compiled: RwLock<bool>,
}

/// Function-based route
pub(crate) struct FunctionRoute {
	pub path: String,
	pub method: Method,
	pub handler: Arc<dyn Handler>,
	pub name: Option<String>,
	/// Middleware stack for this route
	pub middleware: Vec<Arc<dyn Middleware>>,
}

/// Class-based view route
pub(crate) struct ViewRoute {
	pub path: String,
	pub handler: Arc<dyn Handler>,
	pub name: Option<String>,
	/// Middleware stack for this route
	pub middleware: Vec<Arc<dyn Middleware>>,
}

impl ServerRouter {
	/// Validate that a prefix for `mount`/`include` follows Django URL conventions.
	///
	/// # Panics
	///
	/// Panics if the prefix doesn't end with "/".
	/// This matches Django's behavior where URL patterns must end with a trailing slash.
	/// Use "/" for root mounting instead of an empty string "".
	///
	/// # Examples
	///
	/// ```should_panic
	/// use reinhardt_urls::routers::ServerRouter;
	///
	/// // This will panic because "api" doesn't end with "/"
	/// let router = ServerRouter::new()
	///     .mount("api", ServerRouter::new());
	/// ```
	///
	/// ```should_panic
	/// use reinhardt_urls::routers::ServerRouter;
	///
	/// // This will panic because "" is not allowed, use "/" instead
	/// let router = ServerRouter::new()
	///     .mount("", ServerRouter::new());
	/// ```
	fn validate_prefix(prefix: &str) {
		// Prefix must end with "/"
		if !prefix.ends_with('/') {
			if prefix.is_empty() {
				panic!(
					"URL route prefix cannot be an empty string. \
					 Use '/' instead of ''. \
					 This follows Django URL configuration conventions."
				);
			} else {
				panic!(
					"URL route '{}' must end with a trailing slash '/'. \
					 Use '{}/' instead of '{}'. \
					 This follows Django URL configuration conventions.",
					prefix, prefix, prefix,
				);
			}
		}
	}

	/// Create a new ServerRouter
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::routers::ServerRouter;
	///
	/// let router = ServerRouter::new();
	/// ```
	pub fn new() -> Self {
		Self {
			prefix: String::new(),
			namespace: None,
			routes: Vec::new(),
			viewsets: HashMap::new(),
			functions: Vec::new(),
			views: Vec::new(),
			children: Vec::new(),
			di_context: None,
			middleware: Vec::new(),
			middleware_names: Vec::new(),
			middleware_exclusions: Vec::new(),
			reverser: UrlReverser::new(),
			get_router: RwLock::new(MatchitRouter::new()),
			post_router: RwLock::new(MatchitRouter::new()),
			put_router: RwLock::new(MatchitRouter::new()),
			delete_router: RwLock::new(MatchitRouter::new()),
			patch_router: RwLock::new(MatchitRouter::new()),
			head_router: RwLock::new(MatchitRouter::new()),
			options_router: RwLock::new(MatchitRouter::new()),
			routes_compiled: RwLock::new(false),
		}
	}

	/// Set the prefix for this router
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::routers::ServerRouter;
	///
	/// let router = ServerRouter::new()
	///     .with_prefix("/api/v1");
	/// ```
	pub fn with_prefix(mut self, prefix: impl Into<String>) -> Self {
		self.prefix = prefix.into();
		self
	}

	/// Set the namespace for this router
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::routers::ServerRouter;
	///
	/// let router = ServerRouter::new()
	///     .with_namespace("v1");
	/// ```
	pub fn with_namespace(mut self, namespace: impl Into<String>) -> Self {
		self.namespace = Some(namespace.into());
		self
	}

	/// Set the DI context for this router
	///
	/// # Examples
	///
	/// ```rust,no_run
	/// use reinhardt_urls::routers::ServerRouter;
	/// use reinhardt_di::{InjectionContext, SingletonScope};
	/// use std::sync::Arc;
	///
	/// let singleton_scope = Arc::new(SingletonScope::new());
	/// let di_ctx = Arc::new(InjectionContext::builder(singleton_scope).build());
	/// let router = ServerRouter::new()
	///     .with_di_context(di_ctx);
	/// ```
	pub fn with_di_context(mut self, ctx: Arc<InjectionContext>) -> Self {
		self.di_context = Some(ctx);
		self
	}

	/// Returns a reference to the DI context, if set.
	pub(crate) fn di_context(&self) -> Option<&Arc<InjectionContext>> {
		self.di_context.as_ref()
	}

	/// Add middleware to this router
	///
	/// # Examples
	///
	/// ```rust,no_run
	/// use reinhardt_urls::routers::ServerRouter;
	/// use reinhardt_middleware::LoggingMiddleware;
	///
	/// let router = ServerRouter::new()
	///     .with_middleware(LoggingMiddleware::new());
	/// ```
	pub fn with_middleware<M: Middleware + 'static>(mut self, mw: M) -> Self {
		let full_type_name = std::any::type_name::<M>().to_string();
		let short_name = full_type_name
			.rsplit("::")
			.next()
			.unwrap_or(&full_type_name)
			.to_string();
		self.middleware_names.push(MiddlewareInfo {
			name: short_name,
			type_name: full_type_name,
		});
		self.middleware.push(Arc::new(mw));
		self.middleware_exclusions.push(Vec::new());
		self
	}

	/// Exclude a URL path from the most recently added middleware.
	///
	/// Paths ending with `/` are treated as prefix matches: any request
	/// path starting with the given prefix will skip the middleware.
	/// Paths without trailing `/` require an exact match.
	///
	/// This method operates on the **last middleware** added via
	/// [`with_middleware()`](Self::with_middleware). Multiple `.exclude()`
	/// calls accumulate exclusions on the same middleware.
	///
	/// # Panics
	///
	/// Panics if no middleware has been added yet.
	///
	/// # Examples
	///
	/// ```rust,no_run
	/// use reinhardt_urls::routers::ServerRouter;
	/// use reinhardt_middleware::LoggingMiddleware;
	///
	/// let router = ServerRouter::new()
	///     .with_middleware(LoggingMiddleware::new())
	///         .exclude("/api/auth/")    // prefix: skips /api/auth/*
	///         .exclude("/health");      // exact: skips only /health
	/// ```
	pub fn exclude(mut self, pattern: &str) -> Self {
		assert!(
			!self.middleware_exclusions.is_empty(),
			"exclude() called with no middleware. Call with_middleware() first."
		);
		self.middleware_exclusions
			.last_mut()
			.unwrap()
			.push(pattern.to_string());
		self
	}

	/// Build middleware list, wrapping any with exclusions in `ExcludeMiddleware`.
	fn build_middleware_with_exclusions(&self) -> Vec<Arc<dyn Middleware>> {
		let mut result: Vec<Arc<dyn Middleware>> = Vec::with_capacity(self.middleware.len());

		for (mw, exclusions) in self
			.middleware
			.iter()
			.zip(self.middleware_exclusions.iter())
		{
			if exclusions.is_empty() {
				result.push(mw.clone());
			} else {
				let mut exclude_mw = ExcludeMiddleware::new(mw.clone());
				for pattern in exclusions {
					exclude_mw.add_exclusion_mut(pattern);
				}
				result.push(Arc::new(exclude_mw) as Arc<dyn Middleware>);
			}
		}

		result
	}

	/// Mount a child router at the given prefix
	///
	/// # Panics
	///
	/// Panics if the prefix is non-empty, not "/" and doesn't end with "/".
	/// This follows Django's URL configuration conventions.
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_urls::routers::ServerRouter;
	///
	/// let users_router = ServerRouter::new()
	///     .with_namespace("users");
	///
	/// let router = ServerRouter::new()
	///     .with_prefix("/api")
	///     .mount("/users/", users_router);  // Note: trailing slash required
	///
	/// // Verify the router was created successfully
	/// assert_eq!(router.prefix(), "/api");
	/// ```
	///
	/// Using "/" for root mounting is also valid:
	///
	/// ```rust
	/// use reinhardt_urls::routers::ServerRouter;
	///
	/// let app_router = ServerRouter::new();
	/// let router = ServerRouter::new().mount("/", app_router);
	/// ```
	pub fn mount(mut self, prefix: &str, mut child: ServerRouter) -> Self {
		// Validate prefix follows Django URL conventions
		Self::validate_prefix(prefix);

		// Set prefix if not already set
		if child.prefix.is_empty() {
			child.prefix = prefix.to_string();
		}

		// Inherit DI context if child doesn't have one
		if child.di_context.is_none() {
			child.di_context = self.di_context.clone();
		}

		self.children.push(child);
		self
	}

	/// Mount a child router (mutable version)
	///
	/// # Examples
	///
	/// ```rust,no_run
	/// use reinhardt_urls::routers::ServerRouter;
	///
	/// let mut router = ServerRouter::new();
	/// let users_router = ServerRouter::new();
	///
	/// router.mount_mut("/users", users_router);
	/// ```
	pub fn mount_mut(&mut self, prefix: &str, mut child: ServerRouter) {
		// Validate prefix follows Django URL conventions
		Self::validate_prefix(prefix);

		if child.prefix.is_empty() {
			child.prefix = prefix.to_string();
		}
		if child.di_context.is_none() {
			child.di_context = self.di_context.clone();
		}
		self.children.push(child);
	}

	/// Add multiple child routers at once
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_urls::routers::ServerRouter;
	///
	/// let users = ServerRouter::new().with_prefix("/users");
	/// let posts = ServerRouter::new().with_prefix("/posts");
	///
	/// let router = ServerRouter::new()
	///     .group(vec![users, posts]);
	///
	/// // Verify the router was created successfully
	/// assert_eq!(router.prefix(), "");
	/// ```
	pub fn group(mut self, routers: Vec<ServerRouter>) -> Self {
		for router in routers {
			self.children.push(router);
		}
		self
	}

	/// Register a function-based route (FastAPI-style)
	///
	/// # Examples
	///
	/// ```rust,no_run
	/// use reinhardt_urls::routers::ServerRouter;
	/// use hyper::Method;
	/// # use reinhardt_http::{Request, Response, Result};
	///
	/// async fn health_check(_req: Request) -> Result<Response> {
	///     Ok(Response::ok())
	/// }
	///
	/// let router = ServerRouter::new()
	///     .function("/health", Method::GET, health_check);
	/// ```
	pub fn function<F, Fut>(mut self, path: &str, method: Method, func: F) -> Self
	where
		F: Fn(Request) -> Fut + Send + Sync + 'static,
		Fut: std::future::Future<Output = Result<Response>> + Send + 'static,
	{
		let handler = Arc::new(FunctionHandler { func });
		self.functions.push(FunctionRoute {
			path: path.to_string(),
			method,
			handler,
			name: None,
			middleware: Vec::new(),
		});
		self
	}

	/// Register a route with a Handler trait implementation and HTTP method
	///
	/// This method accepts a type that implements the `Handler` trait,
	/// allowing for stateful handlers and a more object-oriented approach.
	/// Unlike `handler()`, this method requires specifying an HTTP method.
	///
	/// # Examples
	///
	/// ```rust,no_run
	/// use reinhardt_urls::routers::ServerRouter;
	/// use hyper::Method;
	/// use reinhardt_http::{Request, Response, Result};
	/// use reinhardt_http::Handler;
	/// use async_trait::async_trait;
	///
	/// #[derive(Clone)]
	/// struct ArticleHandler;
	///
	/// #[async_trait]
	/// impl Handler for ArticleHandler {
	///     async fn handle(&self, _request: Request) -> Result<Response> {
	///         Ok(Response::ok())
	///     }
	/// }
	///
	/// let router = ServerRouter::new()
	///     .handler_with_method("/articles", Method::GET, ArticleHandler);
	/// ```
	pub fn handler_with_method<H: Handler + 'static>(
		mut self,
		path: &str,
		method: Method,
		handler: H,
	) -> Self {
		self.functions.push(FunctionRoute {
			path: path.to_string(),
			method,
			handler: Arc::new(handler),
			name: None,
			middleware: Vec::new(),
		});
		self
	}

	/// Register a route (alias for `function`)
	///
	/// This method is an alias for `function` and provides the same functionality.
	/// Use it when you prefer the `route` naming convention.
	///
	/// # Examples
	///
	/// ```rust,no_run
	/// use reinhardt_urls::routers::ServerRouter;
	/// use hyper::Method;
	/// # use reinhardt_http::{Request, Response, Result};
	///
	/// async fn health_check(_req: Request) -> Result<Response> {
	///     Ok(Response::ok())
	/// }
	///
	/// let router = ServerRouter::new()
	///     .route("/health", Method::GET, health_check);
	/// ```
	#[inline]
	pub fn route<F, Fut>(self, path: &str, method: Method, func: F) -> Self
	where
		F: Fn(Request) -> Fut + Send + Sync + 'static,
		Fut: std::future::Future<Output = Result<Response>> + Send + 'static,
	{
		self.function(path, method, func)
	}

	/// Register a named function-based route (FastAPI-style with URL reversal)
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_urls::routers::ServerRouter;
	/// use hyper::Method;
	/// # use reinhardt_http::{Request, Response, Result};
	///
	/// # async fn health_check(_req: Request) -> Result<Response> {
	/// #     Ok(Response::ok())
	/// # }
	/// let mut router = ServerRouter::new()
	///     .with_namespace("api")
	///     .function_named("/health", Method::GET, "health", health_check);
	///
	/// router.register_all_routes();
	/// let url = router.reverse("api:health", &[]).unwrap();
	/// assert_eq!(url, "/health");
	/// ```
	#[deprecated(
		since = "0.2.0",
		note = "Use `#[get(\"/path\", name = \"name\")]` + `.endpoint()` instead"
	)]
	pub fn function_named<F, Fut>(mut self, path: &str, method: Method, name: &str, func: F) -> Self
	where
		F: Fn(Request) -> Fut + Send + Sync + 'static,
		Fut: std::future::Future<Output = Result<Response>> + Send + 'static,
	{
		let handler = Arc::new(FunctionHandler { func });
		self.functions.push(FunctionRoute {
			path: path.to_string(),
			method,
			handler,
			name: Some(name.to_string()),
			middleware: Vec::new(),
		});
		self
	}

	/// Register a named route with a Handler trait implementation and HTTP method
	///
	/// This method accepts a type that implements the `Handler` trait,
	/// allowing for stateful handlers with URL reversal support.
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_urls::routers::ServerRouter;
	/// use hyper::Method;
	/// use reinhardt_http::{Request, Response, Result};
	/// use reinhardt_http::Handler;
	/// use async_trait::async_trait;
	///
	/// #[derive(Clone)]
	/// struct ArticleHandler;
	///
	/// #[async_trait]
	/// impl Handler for ArticleHandler {
	///     async fn handle(&self, _request: Request) -> Result<Response> {
	///         Ok(Response::ok())
	///     }
	/// }
	///
	/// let mut router = ServerRouter::new()
	///     .with_namespace("api")
	///     .handler_with_method_named("/articles", Method::GET, "list_articles", ArticleHandler);
	///
	/// router.register_all_routes();
	/// let url = router.reverse("api:list_articles", &[]).unwrap();
	/// assert_eq!(url, "/articles");
	/// ```
	#[deprecated(
		since = "0.2.0",
		note = "Use `#[get(\"/path\", name = \"name\")]` + `.endpoint()` instead"
	)]
	pub fn handler_with_method_named<H: Handler + 'static>(
		mut self,
		path: &str,
		method: Method,
		name: &str,
		handler: H,
	) -> Self {
		self.functions.push(FunctionRoute {
			path: path.to_string(),
			method,
			handler: Arc::new(handler),
			name: Some(name.to_string()),
			middleware: Vec::new(),
		});
		self
	}

	/// Register a named route (alias for `function_named`)
	///
	/// This method is an alias for `function_named` and provides the same functionality.
	/// Use it when you prefer the `route` naming convention.
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_urls::routers::ServerRouter;
	/// use hyper::Method;
	/// # use reinhardt_http::{Request, Response, Result};
	///
	/// # async fn health_check(_req: Request) -> Result<Response> {
	/// #     Ok(Response::ok())
	/// # }
	/// let mut router = ServerRouter::new()
	///     .with_namespace("api")
	///     .route_named("/health", Method::GET, "health", health_check);
	///
	/// router.register_all_routes();
	/// let url = router.reverse("api:health", &[]).unwrap();
	/// assert_eq!(url, "/health");
	/// ```
	#[deprecated(
		since = "0.2.0",
		note = "Use `#[get(\"/path\", name = \"name\")]` + `.endpoint()` instead"
	)]
	#[inline]
	pub fn route_named<F, Fut>(self, path: &str, method: Method, name: &str, func: F) -> Self
	where
		F: Fn(Request) -> Fut + Send + Sync + 'static,
		Fut: std::future::Future<Output = Result<Response>> + Send + 'static,
	{
		#[allow(deprecated)]
		self.function_named(path, method, name, func)
	}

	/// Register a ViewSet (DRF-style)
	///
	/// # Examples
	///
	/// ```rust,no_run
	/// use reinhardt_urls::routers::ServerRouter;
	/// # use reinhardt_views::viewsets::ViewSet;
	/// # use async_trait::async_trait;
	/// # struct UserViewSet;
	/// # #[async_trait]
	/// # impl ViewSet for UserViewSet {
	/// #     fn get_basename(&self) -> &str { "users" }
	/// #     async fn dispatch(&self, _req: reinhardt_http::Request, _action: reinhardt_views::viewsets::Action)
	/// #         -> reinhardt_core::exception::Result<reinhardt_http::Response> {
	/// #         Ok(reinhardt_http::Response::ok())
	/// #     }
	/// # }
	///
	/// let router = ServerRouter::new()
	///     .viewset("/users", UserViewSet);
	/// ```
	pub fn viewset<V: ViewSet + 'static>(mut self, prefix: &str, viewset: V) -> Self {
		self.viewsets.insert(prefix.to_string(), Arc::new(viewset));
		self
	}

	/// Register an endpoint using EndpointInfo trait
	///
	/// This method accepts a factory function that returns a View type implementing
	/// both `EndpointInfo` and `Handler` traits. The path, HTTP method, and name
	/// are automatically extracted from the `EndpointInfo` implementation.
	///
	/// # Examples
	///
	/// ```rust,no_run
	/// use reinhardt_urls::routers::ServerRouter;
	/// # use reinhardt_core::endpoint::EndpointInfo;
	/// # use reinhardt_http::{Handler, Request, Response};
	/// # use hyper::Method;
	/// # struct ListUsers;
	/// # impl EndpointInfo for ListUsers {
	/// #     fn path() -> &'static str { "/users" }
	/// #     fn method() -> Method { Method::GET }
	/// #     fn name() -> &'static str { "list_users" }
	/// # }
	/// # #[async_trait::async_trait]
	/// # impl Handler for ListUsers {
	/// #     async fn handle(&self, _req: Request) -> Result<Response, reinhardt_http::Error> {
	/// #         Ok(Response::ok())
	/// #     }
	/// # }
	/// # fn list_users() -> ListUsers { ListUsers }
	///
	/// // Pass the function directly (no () needed)
	/// let router = ServerRouter::new()
	///     .endpoint(list_users);
	/// ```
	pub fn endpoint<F, E>(mut self, f: F) -> Self
	where
		F: FnOnce() -> E,
		E: EndpointInfo + Handler + 'static,
	{
		let view = f();
		let path = E::path().to_string();
		let method = E::method();
		let name = E::name().to_string();

		self.functions.push(FunctionRoute {
			path,
			method,
			handler: Arc::new(view),
			name: Some(name),
			middleware: Vec::new(),
		});
		self
	}

	/// Register a class-based view (Django-style)
	///
	/// # Examples
	///
	/// ```rust,no_run
	/// use reinhardt_urls::routers::ServerRouter;
	/// # use reinhardt_http::{Handler, {Request, Response, Result}};
	/// # use async_trait::async_trait;
	/// # struct ArticleListView;
	/// # #[async_trait]
	/// # impl Handler for ArticleListView {
	/// #     async fn handle(&self, _req: Request) -> Result<Response> {
	/// #         Ok(Response::ok())
	/// #     }
	/// # }
	///
	/// let view = ArticleListView;
	/// let router = ServerRouter::new()
	///     .view("/articles", view);
	/// ```
	pub fn view<V>(mut self, path: &str, view: V) -> Self
	where
		V: Handler + 'static,
	{
		self.views.push(ViewRoute {
			path: path.to_string(),
			handler: Arc::new(view),
			name: None,
			middleware: Vec::new(),
		});
		self
	}

	/// Register a named class-based view (Django-style with URL reversal)
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_urls::routers::ServerRouter;
	/// # use reinhardt_http::{Handler, {Request, Response, Result}};
	/// # use async_trait::async_trait;
	/// # struct ArticleListView;
	/// # #[async_trait]
	/// # impl Handler for ArticleListView {
	/// #     async fn handle(&self, _req: Request) -> Result<Response> {
	/// #         Ok(Response::ok())
	/// #     }
	/// # }
	///
	/// let view = ArticleListView;
	/// let mut router = ServerRouter::new()
	///     .with_namespace("articles")
	///     .view_named("/articles", "list", view);
	///
	/// router.register_all_routes();
	/// let url = router.reverse("articles:list", &[]).unwrap();
	/// assert_eq!(url, "/articles");
	/// ```
	#[deprecated(
		since = "0.2.0",
		note = "Use `#[get(\"/path\", name = \"name\")]` + `.endpoint()` instead"
	)]
	pub fn view_named<V>(mut self, path: &str, name: &str, view: V) -> Self
	where
		V: Handler + 'static,
	{
		self.views.push(ViewRoute {
			path: path.to_string(),
			handler: Arc::new(view),
			name: Some(name.to_string()),
			middleware: Vec::new(),
		});
		self
	}

	/// Register a handler directly (recommended method)
	///
	/// This method allows you to pass a handler directly without wrapping it in `Arc`.
	/// The `Arc` wrapping is handled internally for you.
	///
	/// # Examples
	///
	/// ```rust,no_run
	/// use reinhardt_urls::routers::ServerRouter;
	/// # use reinhardt_http::{Handler, {Request, Response, Result}};
	/// # use async_trait::async_trait;
	/// # struct CustomHandler;
	/// # #[async_trait]
	/// # impl Handler for CustomHandler {
	/// #     async fn handle(&self, _req: Request) -> Result<Response> {
	/// #         Ok(Response::ok())
	/// #     }
	/// # }
	///
	/// // No Arc::new() needed!
	/// let router = ServerRouter::new()
	///     .handler("/custom", CustomHandler);
	/// ```
	pub fn handler<H>(mut self, path: &str, handler: H) -> Self
	where
		H: Handler + 'static,
	{
		let route = Route::from_handler(path, handler);
		self.routes.push(route);
		self
	}

	/// Register a handler that is already wrapped in Arc (low-level API)
	///
	/// This is provided for cases where you already have an `Arc<dyn Handler>`.
	/// In most cases, you should use `handler()` instead.
	///
	/// # Examples
	///
	/// ```rust,no_run
	/// use reinhardt_urls::routers::ServerRouter;
	/// # use reinhardt_http::{Handler, {Request, Response, Result}};
	/// # use async_trait::async_trait;
	/// # use std::sync::Arc;
	/// # struct CustomHandler;
	/// # #[async_trait]
	/// # impl Handler for CustomHandler {
	/// #     async fn handle(&self, _req: Request) -> Result<Response> {
	/// #         Ok(Response::ok())
	/// #     }
	/// # }
	///
	/// let handler = Arc::new(CustomHandler);
	/// let router = ServerRouter::new()
	///     .handler_arc("/custom", handler);
	/// ```
	pub fn handler_arc(mut self, path: &str, handler: Arc<dyn Handler>) -> Self {
		let route = Route::new(path, handler);
		self.routes.push(route);
		self
	}

	/// Add middleware to the last registered function route
	///
	/// # Examples
	///
	/// ```rust,no_run
	/// use reinhardt_urls::routers::ServerRouter;
	/// use reinhardt_middleware::LoggingMiddleware;
	/// use hyper::Method;
	/// # use reinhardt_http::{Request, Response, Result};
	///
	/// # async fn health(_req: Request) -> Result<Response> {
	/// #     Ok(Response::ok())
	/// # }
	/// let router = ServerRouter::new()
	///     .function("/health", Method::GET, health)
	///     .with_route_middleware(LoggingMiddleware::new());
	/// ```
	pub fn with_route_middleware<M: Middleware + 'static>(mut self, middleware: M) -> Self {
		let middleware = Arc::new(middleware);
		if let Some(route) = self.functions.last_mut() {
			route.middleware.push(middleware.clone());
		} else if let Some(route) = self.views.last_mut() {
			route.middleware.push(middleware.clone());
		} else if let Some(route) = self.routes.last_mut() {
			route.middleware.push(middleware);
		}
		self
	}

	/// Compile all routes into matchit routers.
	///
	/// This should be called after all routes have been registered.
	/// It converts patterns like "/users/{id}" to matchit format.
	///
	/// Returns a list of route compilation errors (if any). Empty list means
	/// all routes compiled successfully. RwLock poisoning is recovered from
	/// via `PoisonError::into_inner` to prevent cascade failures.
	fn compile_routes(&self) -> Vec<String> {
		// Check if already compiled (read lock, recovers from poisoning)
		if *self
			.routes_compiled
			.read()
			.unwrap_or_else(PoisonError::into_inner)
		{
			return Vec::new();
		}

		let mut errors = Vec::new();

		// Compile function routes
		for func_route in &self.functions {
			let route_handler = RouteHandler {
				handler: func_route.handler.clone(),
				middleware: func_route.middleware.clone(),
			};

			// Strip prefix from route path to avoid double-prefix matching.
			// Routes may be registered with absolute paths that already include the prefix
			// (e.g., server functions register as "/api/server_fn/login"). Since resolve()
			// strips the prefix from incoming request paths before matching against matchit,
			// we must also strip the prefix here during compilation.
			let route_path_owned = Self::strip_prefix_normalized(&self.prefix, &func_route.path)
				.unwrap_or_else(|| Cow::Borrowed(&func_route.path));
			let route_path: &str = &route_path_owned;

			// matchit uses {name} format which matches our pattern
			let router_lock = match func_route.method {
				Method::GET => &self.get_router,
				Method::POST => &self.post_router,
				Method::PUT => &self.put_router,
				Method::DELETE => &self.delete_router,
				Method::PATCH => &self.patch_router,
				Method::HEAD => &self.head_router,
				Method::OPTIONS => &self.options_router,
				_ => &self.get_router,
			};
			if let Err(e) = router_lock
				.write()
				.unwrap_or_else(PoisonError::into_inner)
				.insert(route_path, route_handler)
			{
				errors.push(format!(
					"Failed to compile route '{}' ({}): {}",
					func_route.path, func_route.method, e
				));
			}
		}

		// Compile view routes (views handle all methods internally)
		for view_route in &self.views {
			let route_handler = RouteHandler {
				handler: view_route.handler.clone(),
				middleware: view_route.middleware.clone(),
			};

			// Strip prefix from route path (same reason as function routes above)
			let route_path_owned = Self::strip_prefix_normalized(&self.prefix, &view_route.path)
				.unwrap_or_else(|| Cow::Borrowed(&view_route.path));
			let route_path: &str = &route_path_owned;

			// Register view for all common HTTP methods
			for router_lock in &[
				&self.get_router,
				&self.post_router,
				&self.put_router,
				&self.delete_router,
				&self.patch_router,
			] {
				if let Err(e) = router_lock
					.write()
					.unwrap_or_else(PoisonError::into_inner)
					.insert(route_path, route_handler.clone())
				{
					errors.push(format!(
						"Failed to compile view route '{}': {}",
						view_route.path, e
					));
				}
			}
		}

		// Compile raw routes (routes handle all methods internally)
		for route in &self.routes {
			let route_handler = RouteHandler {
				handler: route.handler_arc(),
				middleware: route.middleware.clone(),
			};

			// Strip prefix from route path (same reason as function routes above)
			let route_path_owned = Self::strip_prefix_normalized(&self.prefix, &route.path)
				.unwrap_or_else(|| Cow::Borrowed(&route.path));
			let route_path: &str = &route_path_owned;

			// Register raw route for all common HTTP methods
			for router_lock in &[
				&self.get_router,
				&self.post_router,
				&self.put_router,
				&self.delete_router,
				&self.patch_router,
			] {
				if let Err(e) = router_lock
					.write()
					.unwrap_or_else(PoisonError::into_inner)
					.insert(route_path, route_handler.clone())
				{
					errors.push(format!(
						"Failed to compile raw route '{}': {}",
						route.path, e
					));
				}
			}
		}

		// Compile ViewSet routes
		// ViewSet base_path must NOT include self.prefix because resolve() strips
		// the prefix from incoming request paths before matching against matchit.
		for (prefix, viewset) in &self.viewsets {
			let base_path = format!("/{}", prefix.trim_start_matches('/'));

			// Collection route: GET /prefix/ (list), POST /prefix/ (create)
			let collection_path = format!("{}/", base_path.trim_end_matches('/'));

			// List action (GET)
			let list_handler = RouteHandler {
				handler: Arc::new(ViewSetHandler {
					viewset: viewset.clone(),
					action: Action::list(),
				}),
				middleware: Vec::new(),
			};
			if let Err(e) = self
				.get_router
				.write()
				.unwrap_or_else(PoisonError::into_inner)
				.insert(&collection_path, list_handler)
			{
				errors.push(format!(
					"Failed to compile ViewSet list route '{}': {}",
					collection_path, e
				));
			}

			// Create action (POST)
			let create_handler = RouteHandler {
				handler: Arc::new(ViewSetHandler {
					viewset: viewset.clone(),
					action: Action::create(),
				}),
				middleware: Vec::new(),
			};
			if let Err(e) = self
				.post_router
				.write()
				.unwrap_or_else(PoisonError::into_inner)
				.insert(&collection_path, create_handler)
			{
				errors.push(format!(
					"Failed to compile ViewSet create route '{}': {}",
					collection_path, e
				));
			}

			// Detail routes: GET/PUT/DELETE /prefix/{id}/
			let lookup_field = viewset.get_lookup_field();
			let detail_path = format!("{}/{{{}}}/", base_path.trim_end_matches('/'), lookup_field);

			// Retrieve action (GET)
			let retrieve_handler = RouteHandler {
				handler: Arc::new(ViewSetHandler {
					viewset: viewset.clone(),
					action: Action::retrieve(),
				}),
				middleware: Vec::new(),
			};
			if let Err(e) = self
				.get_router
				.write()
				.unwrap_or_else(PoisonError::into_inner)
				.insert(&detail_path, retrieve_handler)
			{
				errors.push(format!(
					"Failed to compile ViewSet retrieve route '{}': {}",
					detail_path, e
				));
			}

			// Update action (PUT)
			let update_handler = RouteHandler {
				handler: Arc::new(ViewSetHandler {
					viewset: viewset.clone(),
					action: Action::update(),
				}),
				middleware: Vec::new(),
			};
			if let Err(e) = self
				.put_router
				.write()
				.unwrap_or_else(PoisonError::into_inner)
				.insert(&detail_path, update_handler)
			{
				errors.push(format!(
					"Failed to compile ViewSet update route '{}': {}",
					detail_path, e
				));
			}

			// Destroy action (DELETE)
			let destroy_handler = RouteHandler {
				handler: Arc::new(ViewSetHandler {
					viewset: viewset.clone(),
					action: Action::destroy(),
				}),
				middleware: Vec::new(),
			};
			if let Err(e) = self
				.delete_router
				.write()
				.unwrap_or_else(PoisonError::into_inner)
				.insert(&detail_path, destroy_handler)
			{
				errors.push(format!(
					"Failed to compile ViewSet destroy route '{}': {}",
					detail_path, e
				));
			}
		}

		// Mark routes as compiled
		*self
			.routes_compiled
			.write()
			.unwrap_or_else(PoisonError::into_inner) = true;

		errors
	}

	/// Validate all routes by compiling them and returning any errors.
	///
	/// Call this at application startup to detect invalid route patterns early.
	/// Returns `Ok(())` if all routes compiled successfully, or `Err` with
	/// a list of compilation error messages.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::routers::ServerRouter;
	/// use hyper::Method;
	/// # use reinhardt_http::{Request, Response, Result};
	///
	/// # async fn handler(_req: Request) -> Result<Response> { Ok(Response::ok()) }
	/// let router = ServerRouter::new()
	///     .function("/users/{id}", Method::GET, handler);
	///
	/// // Validate routes at startup
	/// assert!(router.validate_routes().is_ok());
	/// ```
	pub fn validate_routes(&self) -> std::result::Result<(), Vec<String>> {
		let mut errors = self.compile_routes();
		if let Err(name_errors) = self.validate_route_names() {
			errors.extend(name_errors);
		}
		if errors.is_empty() {
			Ok(())
		} else {
			Err(errors)
		}
	}

	/// Validate that no duplicate route names exist among all collected routes.
	///
	/// Returns `Ok(())` if all names are unique, or `Err(errors)` with details
	/// about each duplicate.
	pub fn validate_route_names(&self) -> std::result::Result<(), Vec<String>> {
		let registrations = self.collect_routes_recursive(None, "");
		let mut seen: std::collections::HashMap<String, String> = std::collections::HashMap::new();
		let mut errors = Vec::new();
		for (name, path) in registrations {
			if let Some(existing_path) = seen.insert(name.clone(), path.clone()) {
				errors.push(format!(
					"Duplicate route name '{}': path '{}' conflicts with existing path '{}'",
					name, path, existing_path
				));
			}
		}
		if errors.is_empty() {
			Ok(())
		} else {
			Err(errors)
		}
	}

	/// Get the prefix of this router
	pub fn prefix(&self) -> &str {
		&self.prefix
	}

	/// Get the namespace of this router
	pub fn namespace(&self) -> Option<&str> {
		self.namespace.as_deref()
	}

	/// Get the number of child routers
	pub fn children_count(&self) -> usize {
		self.children.len()
	}

	/// Get all registered middleware information
	///
	/// Returns a deduplicated list of middleware registered on this router
	/// and all child routers. The order reflects registration order.
	pub fn get_registered_middleware(&self) -> Vec<MiddlewareInfo> {
		let mut all = self.middleware_names.clone();

		// Collect from children recursively
		for child in &self.children {
			all.extend(child.get_registered_middleware());
		}

		// Deduplicate while preserving order
		let mut seen = std::collections::HashSet::new();
		all.retain(|info| seen.insert(info.type_name.clone()));
		all
	}

	/// Get all routes from this router and its children
	///
	/// Returns a vector of tuples containing (full_path, name, namespace, methods).
	/// This recursively collects routes from all child routers.
	///
	/// # Examples
	///
	/// ```ignore
	/// let router = ServerRouter::new()
	///     .with_prefix("/api/v1")
	///     .function("/users", Method::GET, handler);
	///
	/// let routes = router.get_all_routes();
	/// // Returns: [("/api/v1/users", None, None, vec![Method::GET])]
	/// ```
	pub fn get_all_routes(&self) -> RouteInfo {
		let mut routes = Vec::new();

		// Collect routes from this router
		for route in &self.routes {
			let full_path = if self.prefix.is_empty() {
				route.path.clone()
			} else {
				join_path(&self.prefix, &route.path)
			};

			routes.push((
				full_path,
				route.name.clone(),
				route.namespace.clone().or_else(|| self.namespace.clone()),
				vec![], // Method-agnostic handlers accept all HTTP methods (shown as "ALL" in showurls)
			));
		}

		// Collect function-based routes
		for func_route in &self.functions {
			let full_path = if self.prefix.is_empty() {
				func_route.path.clone()
			} else {
				join_path(&self.prefix, &func_route.path)
			};

			routes.push((
				full_path,
				None,                   // Function routes don't have names
				self.namespace.clone(), // Use router's namespace
				vec![func_route.method.clone()],
			));
		}

		// Collect view routes
		for view_route in &self.views {
			let full_path = if self.prefix.is_empty() {
				view_route.path.clone()
			} else {
				join_path(&self.prefix, &view_route.path)
			};

			routes.push((
				full_path,
				None,                   // View routes don't have names
				self.namespace.clone(), // Use router's namespace
				vec![], // Class-based views handle method dispatch internally (accepts all methods)
			));
		}

		// Collect ViewSet routes
		for prefix in self.viewsets.keys() {
			let base_path = if self.prefix.is_empty() {
				format!("/{}", prefix)
			} else {
				format!("{}/{}", self.prefix, prefix)
			};

			// ViewSets generate standard CRUD routes
			let viewset_routes = vec![
				(format!("{}/", base_path), vec![Method::GET, Method::POST]),
				(
					format!("{}/<id>/", base_path),
					vec![Method::GET, Method::PUT, Method::DELETE],
				),
			];

			for (path, methods) in viewset_routes {
				routes.push((
					path,
					None,                   // ViewSet routes don't have individual names
					self.namespace.clone(), // Use router's namespace
					methods,
				));
			}
		}

		// Recursively collect from child routers
		for child in &self.children {
			let child_prefix = if self.prefix.is_empty() {
				child.prefix.clone()
			} else if child.prefix.is_empty() {
				self.prefix.clone()
			} else {
				join_path(&self.prefix, &child.prefix)
			};

			for (path, name, namespace, methods) in child.get_all_routes() {
				// Adjust path if child has no prefix (already included)
				let full_path = if path.starts_with(&child.prefix) || child.prefix.is_empty() {
					path
				} else {
					join_path(&child_prefix, &path)
				};

				// Combine namespaces (parent:child)
				let combined_namespace = match (self.namespace.as_ref(), namespace.as_ref()) {
					(Some(parent), Some(child)) => Some(format!("{}:{}", parent, child)),
					(Some(parent), None) => Some(parent.clone()),
					(None, Some(child)) => Some(child.clone()),
					(None, None) => None,
				};

				routes.push((full_path, name, combined_namespace, methods));
			}
		}

		routes
	}

	/// Get the fully qualified namespace for this router
	///
	/// Returns the complete namespace chain from root to this router.
	/// For example, if this router has namespace "users" and its parent has "v1",
	/// this returns "v1:users".
	///
	/// # Arguments
	///
	/// * `parent_namespace` - The parent router's namespace (if any)
	///
	/// # Examples
	///
	/// ```ignore
	/// let router = ServerRouter::new().with_namespace("users");
	/// assert_eq!(router.get_full_namespace(Some("v1")), Some("v1:users".to_string()));
	/// assert_eq!(router.get_full_namespace(None), Some("users".to_string()));
	/// ```
	pub fn get_full_namespace(&self, parent_namespace: Option<&str>) -> Option<String> {
		match (parent_namespace, self.namespace.as_deref()) {
			(Some(parent), Some(child)) => Some(format!("{}:{}", parent, child)),
			(Some(parent), None) => Some(parent.to_string()),
			(None, Some(child)) => Some(child.to_string()),
			(None, None) => None,
		}
	}

	/// Register all routes with the URL reverser
	///
	/// This recursively registers all routes from this router and its children
	/// with their fully qualified names (namespace:name format).
	///
	/// # Examples
	///
	/// ```ignore
	/// let mut router = ServerRouter::new()
	///     .with_namespace("v1");
	///
	/// // After registering routes, you can reverse them:
	/// router.register_all_routes();
	/// let url = router.reverse("v1:users:detail", &[("id", "123")]);
	/// ```
	#[must_use]
	pub fn register_all_routes(&mut self) -> Vec<String> {
		let registrations = self.collect_routes_recursive(None, "");
		let mut errors = Vec::new();
		for (name, path) in registrations {
			if let Err(e) = self.reverser.register_path(&name, &path) {
				errors.push(e);
			}
		}
		errors
	}

	/// Recursively collect all routes with accumulated prefixes and namespaces.
	///
	/// Returns a list of `(qualified_name, full_path)` pairs to be registered
	/// in the root router's reverser.
	fn collect_routes_recursive(
		&self,
		parent_namespace: Option<&str>,
		parent_prefix: &str,
	) -> Vec<(String, String)> {
		let full_namespace = self.get_full_namespace(parent_namespace);
		let current_prefix = super::path_utils::join_prefix_path(parent_prefix, &self.prefix);
		let mut registrations = Vec::new();

		// Collect routes from this router
		for route in &self.routes {
			if let Some(name) = &route.name {
				let qualified_name = if let Some(ref ns) = full_namespace {
					format!("{}:{}", ns, name)
				} else {
					name.clone()
				};

				let full_path = super::path_utils::join_prefix_path(&current_prefix, &route.path);
				registrations.push((qualified_name, full_path));
			}
		}

		// Collect function routes
		for func_route in &self.functions {
			if let Some(ref name) = func_route.name {
				let qualified_name = if let Some(ref ns) = full_namespace {
					format!("{}:{}", ns, name)
				} else {
					name.clone()
				};

				let full_path =
					super::path_utils::join_prefix_path(&current_prefix, &func_route.path);
				registrations.push((qualified_name, full_path));
			}
		}

		// Collect view routes
		for view_route in &self.views {
			if let Some(ref name) = view_route.name {
				let qualified_name = if let Some(ref ns) = full_namespace {
					format!("{}:{}", ns, name)
				} else {
					name.clone()
				};

				let full_path =
					super::path_utils::join_prefix_path(&current_prefix, &view_route.path);
				registrations.push((qualified_name, full_path));
			}
		}

		// Collect ViewSet routes with standard names (Django convention: basename, not prefix)
		for (prefix, viewset) in &self.viewsets {
			let base_path = if current_prefix.is_empty() {
				format!("/{}", prefix)
			} else {
				format!("{}/{}", current_prefix, prefix)
			};

			let basename = viewset.get_basename();
			let lookup_field = viewset.get_lookup_field();
			let viewset_routes = [
				(format!("{}-list", basename), format!("{}/", base_path)),
				(
					format!("{}-detail", basename),
					format!("{}/<{}>/", base_path, lookup_field),
				),
			];

			for (name, path) in viewset_routes {
				let qualified_name = if let Some(ref ns) = full_namespace {
					format!("{}:{}", ns, name)
				} else {
					name
				};

				registrations.push((qualified_name, path));
			}
		}

		// Recursively collect child routes
		for child in &self.children {
			registrations
				.extend(child.collect_routes_recursive(full_namespace.as_deref(), &current_prefix));
		}

		registrations
	}

	/// Reverse a URL by route name
	///
	/// Supports hierarchical namespace notation (e.g., "v1:users:detail").
	///
	/// # Arguments
	///
	/// * `name` - The route name, optionally with namespace (e.g., "users-detail" or "v1:users-detail")
	/// * `params` - URL parameters as key-value pairs
	///
	/// # Examples
	///
	/// ```ignore
	/// let router = ServerRouter::new()
	///     .with_namespace("v1");
	///
	/// // Reverse with namespace
	/// let url = router.reverse("v1:users:detail", &[("id", "123")]).unwrap();
	/// assert_eq!(url, "/users/123/");
	///
	/// // Reverse without namespace (searches all routes)
	/// let url = router.reverse("users-detail", &[("id", "123")]).unwrap();
	/// ```
	pub fn reverse(&self, name: &str, params: &[(&str, &str)]) -> Option<String> {
		// Try own reverser first
		if let Ok(url) = self.reverser.reverse_with(name, params) {
			return Some(url);
		}

		// Try child routers
		for child in &self.children {
			if let Some(url) = child.reverse(name, params) {
				return Some(url);
			}
		}

		None
	}

	/// Strip `prefix` from `path` and ensure the result always has a leading `/`.
	///
	/// When a prefix ends with `/` (e.g., `/api/`), `str::strip_prefix` consumes
	/// the trailing slash, leaving the remainder without a leading `/`. This breaks
	/// child router matching because child prefixes expect paths starting with `/`.
	///
	/// Returns `None` if `path` does not start with `prefix`.
	fn strip_prefix_normalized<'a>(prefix: &str, path: &'a str) -> Option<Cow<'a, str>> {
		if prefix.is_empty() {
			return Some(Cow::Borrowed(path));
		}
		let stripped = path.strip_prefix(prefix)?;
		Some(if stripped.is_empty() {
			Cow::Borrowed("/")
		} else if stripped.starts_with('/') {
			Cow::Borrowed(stripped)
		} else {
			Cow::Owned(format!("/{stripped}"))
		})
	}

	/// Resolve a request path to a route match
	///
	/// This performs hierarchical route resolution:
	/// 1. Check prefix match
	/// 2. Try child routers first (depth-first search)
	/// 3. Try own routes
	fn resolve(&self, path: &str, method: &Method) -> Option<RouteMatch> {
		// 1. Check prefix and normalize remaining path (ensures leading `/`)
		let remaining_path = Self::strip_prefix_normalized(&self.prefix, path)?;

		// 2. Try child routers first
		let own_middleware = self.build_middleware_with_exclusions();
		for child in &self.children {
			if let Some(route_match) =
				child.resolve_internal(&remaining_path, method, &own_middleware, &self.di_context)
			{
				return Some(route_match);
			}
		}

		// 3. Try own routes
		self.match_own_routes_with_context(
			&remaining_path,
			method,
			own_middleware,
			self.di_context.clone(),
		)
	}

	/// Internal route resolution with middleware and DI context inheritance
	fn resolve_internal(
		&self,
		path: &str,
		method: &Method,
		parent_middleware: &[Arc<dyn Middleware>],
		parent_di: &Option<Arc<InjectionContext>>,
	) -> Option<RouteMatch> {
		// Check prefix and normalize remaining path (ensures leading `/`)
		let remaining_path = Self::strip_prefix_normalized(&self.prefix, path)?;

		// Build middleware stack (parent → child order)
		let mut middleware_stack = parent_middleware.to_vec();
		middleware_stack.extend(self.build_middleware_with_exclusions());

		// Inherit DI context
		let di_context = self.di_context.clone().or_else(|| parent_di.clone());

		// Try child routers
		for child in &self.children {
			if let Some(route_match) =
				child.resolve_internal(&remaining_path, method, &middleware_stack, &di_context)
			{
				return Some(route_match);
			}
		}

		// Try own routes
		self.match_own_routes_with_context(&remaining_path, method, middleware_stack, di_context)
	}

	/// Match routes in this router (without context)
	#[cfg(test)]
	fn match_own_routes(&self, path: &str, method: &Method) -> Option<RouteMatch> {
		self.match_own_routes_with_context(
			path,
			method,
			self.build_middleware_with_exclusions(),
			self.di_context.clone(),
		)
	}

	/// Match routes in this router with provided context
	///
	/// This method uses matchit for O(m) route matching where m = path length.
	/// Routes must be compiled before matching (automatically done on first match).
	fn match_own_routes_with_context(
		&self,
		path: &str,
		method: &Method,
		middleware_stack: Vec<Arc<dyn Middleware>>,
		di_context: Option<Arc<InjectionContext>>,
	) -> Option<RouteMatch> {
		// Compile routes on first use (lazy compilation with interior mutability)
		self.compile_routes();

		// Normalize path for matchit lookup - routes are registered with leading slash
		// When prefix is "/" and path is "/health", strip_prefix yields "health" but
		// the route was registered as "/health". We need to ensure we search with "/health".
		let search_path = if path.starts_with('/') {
			path.to_string()
		} else {
			format!("/{}", path)
		};

		// Use matchit to find matching route - O(m) complexity
		let router_lock = match *method {
			Method::GET => &self.get_router,
			Method::POST => &self.post_router,
			Method::PUT => &self.put_router,
			Method::DELETE => &self.delete_router,
			Method::PATCH => &self.patch_router,
			Method::HEAD => &self.head_router,
			Method::OPTIONS => &self.options_router,
			_ => &self.get_router,
		};

		let router = router_lock.read().unwrap_or_else(PoisonError::into_inner);

		// Try matching with the original path first
		// If that fails, try with trailing slash toggled (Django-style APPEND_SLASH behavior)
		let paths_to_try = if search_path.ends_with('/') {
			// Path has trailing slash, try without if not found
			let without_slash = search_path.trim_end_matches('/').to_string();
			let without_slash = if without_slash.is_empty() {
				"/".to_string()
			} else {
				without_slash
			};
			vec![search_path.clone(), without_slash]
		} else {
			// Path has no trailing slash, try with if not found
			vec![search_path.clone(), format!("{}/", search_path)]
		};

		for try_path in paths_to_try {
			if let Ok(matched) = router.at(&try_path) {
				let route_handler = matched.value;

				// Extract parameters from matchit
				let params: HashMap<String, String> = matched
					.params
					.iter()
					.map(|(k, v)| (k.to_string(), v.to_string()))
					.collect();

				// Combine router-level and route-level middleware
				let mut combined_middleware = middleware_stack.clone();
				combined_middleware.extend(route_handler.middleware.iter().cloned());

				return Some(RouteMatch {
					handler: route_handler.handler.clone(),
					params,
					middleware_stack: combined_middleware,
					di_context,
				});
			}
		}

		None
	}

	/// Check if a path exists in any HTTP method's router
	///
	/// This is used to determine whether to return 404 (path not found)
	/// or 405 (method not allowed) when a route doesn't match.
	fn path_exists_for_any_method(&self, path: &str) -> bool {
		self.compile_routes();

		// Apply prefix stripping logic (same as resolve method, ensures leading `/`)
		let search_path = match Self::strip_prefix_normalized(&self.prefix, path) {
			Some(p) => p.into_owned(),
			None => return false,
		};

		// Build paths to try with trailing slash toggled (Django-style APPEND_SLASH)
		let paths_to_try = if search_path.ends_with('/') {
			let without_slash = search_path.trim_end_matches('/').to_string();
			let without_slash = if without_slash.is_empty() {
				"/".to_string()
			} else {
				without_slash
			};
			vec![search_path.clone(), without_slash]
		} else {
			vec![search_path.clone(), format!("{}/", search_path)]
		};

		let method_routers = [
			&self.get_router,
			&self.post_router,
			&self.put_router,
			&self.delete_router,
			&self.patch_router,
			&self.head_router,
			&self.options_router,
		];

		for router_lock in method_routers {
			let router = router_lock.read().unwrap_or_else(PoisonError::into_inner);
			for try_path in &paths_to_try {
				if router.at(try_path).is_ok() {
					return true;
				}
			}
		}

		// Also check children routers with remaining path
		for child in &self.children {
			for try_path in &paths_to_try {
				if child.path_exists_for_any_method(try_path) {
					return true;
				}
			}
		}

		false
	}
}

impl std::fmt::Debug for ServerRouter {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("ServerRouter")
			.field("prefix", &self.prefix)
			.field("namespace", &self.namespace)
			.field("routes", &self.routes.len())
			.field("viewsets", &self.viewsets.len())
			.field("functions", &self.functions.len())
			.field("views", &self.views.len())
			.field("children", &self.children.len())
			.field("middleware", &self.middleware.len())
			.finish_non_exhaustive()
	}
}

impl Default for ServerRouter {
	fn default() -> Self {
		Self::new()
	}
}

/// Handler that always returns a pre-built response.
///
/// Used internally to route framework-level error responses (404/405)
/// through the middleware chain for post-processing. (#3234)
struct FixedResponseHandler(Response);

#[async_trait]
impl Handler for FixedResponseHandler {
	async fn handle(&self, _request: Request) -> Result<Response> {
		Ok(self.0.clone())
	}
}

/// Handler implementation for ServerRouter
#[async_trait]
impl Handler for ServerRouter {
	async fn handle(&self, mut req: Request) -> Result<Response> {
		let path = req.uri.path();
		let method = &req.method;

		// Resolve route with HTTP method for matchit routing
		let route_match = match self.resolve(path, method) {
			Some(m) => m,
			None => {
				// Route not found for this method
				// Check if path exists for any other method to determine 404 vs 405
				let error = if self.path_exists_for_any_method(path) {
					Error::MethodNotAllowed(format!("Method {} not allowed for {}", method, path))
				} else {
					Error::NotFound(format!("No route for {} {}", method, path))
				};

				// If router has middleware, route the error response through the
				// middleware chain so post-processing (e.g., security headers) is
				// applied to framework-level 404/405 responses. (#3234)
				let own_middleware = self.build_middleware_with_exclusions();
				if own_middleware.is_empty() {
					return Err(error);
				}

				let response = Response::from(error);
				let handler: Arc<dyn Handler> = Arc::new(FixedResponseHandler(response));
				let chain = own_middleware
					.iter()
					.fold(MiddlewareChain::new(handler), |chain, mw| {
						chain.with_middleware(mw.clone())
					});
				return chain.handle(req).await;
			}
		};

		// Set path parameters in request
		for (key, value) in route_match.params {
			req.set_path_param(key, value);
		}

		// Set DI context if available
		if let Some(di_ctx) = &route_match.di_context {
			req.set_di_context(di_ctx.clone());
		}

		// Apply middleware stack using MiddlewareChain
		if route_match.middleware_stack.is_empty() {
			// No middleware, execute handler directly
			route_match.handler.handle(req).await
		} else {
			// Build middleware chain
			let chain = route_match.middleware_stack.iter().fold(
				MiddlewareChain::new(route_match.handler.clone()),
				|chain, mw| chain.with_middleware(mw.clone()),
			);

			// Execute chain
			chain.handle(req).await
		}
	}
}

/// Implement RegisterViewSet trait for ServerRouter
///
/// This allows ViewSetBuilder to directly register handlers to the router.
impl reinhardt_views::viewsets::RegisterViewSet for ServerRouter {
	fn register_handler(&mut self, path: &str, handler: Arc<dyn Handler>) {
		self.views.push(ViewRoute {
			path: path.to_string(),
			handler,
			name: None,
			middleware: Vec::new(),
		});
	}
}

#[cfg(test)]
#[allow(deprecated)]
mod tests {
	use super::*;
	use rstest::rstest;

	#[rstest]
	fn test_new_router() {
		// Arrange & Act
		let router = ServerRouter::new();

		// Assert
		assert_eq!(router.prefix(), "");
		assert_eq!(router.namespace(), None);
		assert_eq!(router.children_count(), 0);
	}

	#[rstest]
	fn test_with_prefix() {
		// Arrange & Act
		let router = ServerRouter::new().with_prefix("/api/v1");

		// Assert
		assert_eq!(router.prefix(), "/api/v1");
	}

	#[rstest]
	fn test_with_namespace() {
		// Arrange & Act
		let router = ServerRouter::new().with_namespace("v1");

		// Assert
		assert_eq!(router.namespace(), Some("v1"));
	}

	#[rstest]
	fn test_mount() {
		// Arrange
		let child = ServerRouter::new();

		// Act
		let router = ServerRouter::new().mount("/users/", child);

		// Assert
		assert_eq!(router.children_count(), 1);
	}

	#[rstest]
	fn test_mount_inherits_di_context() {
		// Arrange
		let di_ctx = Arc::new(
			InjectionContext::builder(Arc::new(reinhardt_di::SingletonScope::new())).build(),
		);
		let child = ServerRouter::new();

		// Act
		let router = ServerRouter::new()
			.with_di_context(di_ctx.clone())
			.mount("/users/", child);

		// Assert
		assert!(router.di_context.is_some());
		assert_eq!(router.children_count(), 1);
	}

	#[rstest]
	fn test_group() {
		// Arrange
		let users = ServerRouter::new().with_prefix("/users");
		let posts = ServerRouter::new().with_prefix("/posts");

		// Act
		let router = ServerRouter::new().group(vec![users, posts]);

		// Assert
		assert_eq!(router.children_count(), 2);
	}

	#[rstest]
	fn test_get_all_routes() {
		// Arrange
		let router = ServerRouter::new()
			.with_prefix("/api")
			.with_namespace("api");

		// Act
		let routes = router.get_all_routes();

		// Assert
		assert_eq!(routes.len(), 0);
	}

	#[rstest]
	fn test_get_full_namespace_no_parent() {
		// Arrange
		let router = ServerRouter::new().with_namespace("users");

		// Act & Assert
		assert_eq!(router.get_full_namespace(None), Some("users".to_string()));
	}

	#[rstest]
	fn test_get_full_namespace_with_parent() {
		// Arrange
		let router = ServerRouter::new().with_namespace("users");

		// Act & Assert
		assert_eq!(
			router.get_full_namespace(Some("v1")),
			Some("v1:users".to_string())
		);
	}

	#[rstest]
	fn test_get_full_namespace_no_namespace() {
		// Arrange
		let router = ServerRouter::new();

		// Act & Assert
		assert_eq!(
			router.get_full_namespace(Some("v1")),
			Some("v1".to_string())
		);
		assert_eq!(router.get_full_namespace(None), None);
	}

	#[rstest]
	fn test_hierarchical_namespace() {
		// Arrange
		let child = ServerRouter::new().with_namespace("users");

		// Act
		let parent = ServerRouter::new()
			.with_namespace("v1")
			.mount("/users/", child);

		// Assert
		assert_eq!(parent.namespace(), Some("v1"));
		assert_eq!(parent.children_count(), 1);
	}

	#[rstest]
	fn test_register_all_routes_with_namespace() {
		use hyper::Method;

		async fn dummy_handler(_req: Request) -> Result<Response> {
			Ok(Response::ok())
		}

		// Arrange
		let mut router = ServerRouter::new().with_namespace("api").function_named(
			"/health",
			Method::GET,
			"health",
			dummy_handler,
		);

		// Act
		let errors = router.register_all_routes();
		assert!(errors.is_empty());

		// Assert
		let url = router.reverse("api:health", &[]);
		assert!(url.is_some());
		assert_eq!(url.unwrap(), "/health");
	}

	#[rstest]
	fn test_nested_namespace_registration() {
		use hyper::Method;

		async fn dummy_handler(_req: Request) -> Result<Response> {
			Ok(Response::ok())
		}

		// Arrange
		let users = ServerRouter::new().with_namespace("users").function_named(
			"/list",
			Method::GET,
			"list",
			dummy_handler,
		);

		let mut api = ServerRouter::new()
			.with_namespace("v1")
			.with_prefix("/api/v1")
			.mount("/users/", users);

		// Act
		let errors = api.register_all_routes();
		assert!(errors.is_empty());

		// Assert
		let url = api.reverse("v1:users:list", &[]);
		assert!(url.is_some());
		assert_eq!(url.unwrap(), "/api/v1/users/list");
	}

	#[rstest]
	fn test_mount_prefix_inheritance() {
		// Arrange
		let child = ServerRouter::new();

		// Act
		let parent = ServerRouter::new().with_prefix("/api").mount("/v1/", child);

		// Assert
		assert_eq!(parent.children_count(), 1);
	}

	#[rstest]
	fn test_multiple_child_routers() {
		// Arrange
		let users = ServerRouter::new().with_namespace("users");
		let posts = ServerRouter::new().with_namespace("posts");
		let comments = ServerRouter::new().with_namespace("comments");

		// Act
		let router = ServerRouter::new()
			.mount("/users/", users)
			.mount("/posts/", posts)
			.mount("/comments/", comments);

		// Assert
		assert_eq!(router.children_count(), 3);
	}

	#[rstest]
	fn test_deep_nesting() {
		// Arrange
		let resource = ServerRouter::new().with_namespace("resource");
		let v2 = ServerRouter::new()
			.with_namespace("v2")
			.mount("/resource/", resource);
		let v1 = ServerRouter::new().with_namespace("v1").mount("/v2/", v2);

		// Act
		let api = ServerRouter::new().with_namespace("api").mount("/v1/", v1);

		// Assert
		assert_eq!(api.children_count(), 1);
	}

	#[tokio::test]
	async fn test_route_matching_performance_many_routes() {
		use hyper::Method;
		use std::time::Instant;

		async fn dummy_handler(_req: Request) -> Result<Response> {
			Ok(Response::ok())
		}

		// Arrange
		let mut router = ServerRouter::new();
		for i in 0..1000 {
			router = router.function(
				&format!("/api/resource{}/action", i),
				Method::GET,
				dummy_handler,
			);
		}

		// Act
		router.compile_routes();
		let start = Instant::now();
		for _ in 0..10000 {
			let result = router.match_own_routes("/api/resource500/action", &Method::GET);
			assert!(result.is_some());
		}
		let elapsed = start.elapsed();

		// Assert
		assert!(
			elapsed.as_millis() < 100,
			"Route matching too slow: {:?}",
			elapsed
		);
	}

	#[tokio::test]
	async fn test_route_matching_correctness() {
		use hyper::Method;

		async fn dummy_handler(_req: Request) -> Result<Response> {
			Ok(Response::ok())
		}

		// Arrange
		let router = ServerRouter::new()
			.function("/users/{id}", Method::GET, dummy_handler)
			.function("/users/{id}/posts", Method::GET, dummy_handler)
			.function(
				"/posts/{post_id}/comments/{comment_id}",
				Method::GET,
				dummy_handler,
			);
		router.compile_routes();

		// Act & Assert - exact path matching
		let result = router.match_own_routes("/users/123", &Method::GET);
		assert!(result.is_some());
		assert_eq!(result.unwrap().params.get("id"), Some(&"123".to_string()));

		// Act & Assert - nested path matching
		let result = router.match_own_routes("/users/456/posts", &Method::GET);
		assert!(result.is_some());
		assert_eq!(result.unwrap().params.get("id"), Some(&"456".to_string()));

		// Act & Assert - multiple parameters
		let result = router.match_own_routes("/posts/789/comments/101", &Method::GET);
		let params = result.unwrap().params;
		assert_eq!(params.get("post_id"), Some(&"789".to_string()));
		assert_eq!(params.get("comment_id"), Some(&"101".to_string()));

		// Act & Assert - non-matching route
		let result = router.match_own_routes("/nonexistent", &Method::GET);
		assert!(result.is_none());
	}

	#[tokio::test]
	async fn test_route_matching_different_methods() {
		use hyper::Method;

		async fn get_handler(_req: Request) -> Result<Response> {
			Ok(Response::ok())
		}

		async fn post_handler(_req: Request) -> Result<Response> {
			Ok(Response::ok())
		}

		// Arrange
		let router = ServerRouter::new()
			.function("/users", Method::GET, get_handler)
			.function("/users", Method::POST, post_handler);
		router.compile_routes();

		// Act & Assert - GET method
		let result = router.match_own_routes("/users", &Method::GET);
		assert!(result.is_some());

		// Act & Assert - POST method
		let result = router.match_own_routes("/users", &Method::POST);
		assert!(result.is_some());

		// Act & Assert - unsupported method
		let result = router.match_own_routes("/users", &Method::DELETE);
		assert!(result.is_none());
	}

	#[rstest]
	fn test_validate_routes_success() {
		use hyper::Method;

		async fn dummy_handler(_req: Request) -> Result<Response> {
			Ok(Response::ok())
		}

		// Arrange
		let router = ServerRouter::new()
			.function("/users/{id}", Method::GET, dummy_handler)
			.function("/posts", Method::POST, dummy_handler);

		// Act
		let result = router.validate_routes();

		// Assert
		assert!(result.is_ok());
	}

	#[rstest]
	fn test_compile_routes_returns_errors_for_duplicate_routes() {
		use hyper::Method;

		async fn handler_a(_req: Request) -> Result<Response> {
			Ok(Response::ok())
		}
		async fn handler_b(_req: Request) -> Result<Response> {
			Ok(Response::ok())
		}

		// Arrange - register duplicate paths for the same method
		let router = ServerRouter::new()
			.function("/users", Method::GET, handler_a)
			.function("/users", Method::GET, handler_b);

		// Act
		let errors = router.compile_routes();

		// Assert - matchit should report a conflict for duplicate routes
		assert!(!errors.is_empty());
		assert!(errors[0].contains("Failed to compile route"));
	}

	#[rstest]
	fn test_validate_routes_returns_errors_for_invalid_patterns() {
		use hyper::Method;

		async fn handler_a(_req: Request) -> Result<Response> {
			Ok(Response::ok())
		}
		async fn handler_b(_req: Request) -> Result<Response> {
			Ok(Response::ok())
		}

		// Arrange - duplicate routes cause matchit compilation errors
		let router = ServerRouter::new()
			.function("/items", Method::GET, handler_a)
			.function("/items", Method::GET, handler_b);

		// Act
		let result = router.validate_routes();

		// Assert
		assert!(result.is_err());
		let errors = result.unwrap_err();
		assert!(!errors.is_empty());
	}

	#[rstest]
	fn test_router_recovers_from_poisoned_rwlock() {
		use hyper::Method;

		async fn dummy_handler(_req: Request) -> Result<Response> {
			Ok(Response::ok())
		}

		// Arrange
		let router = ServerRouter::new().function("/health", Method::GET, dummy_handler);

		// Poison the routes_compiled RwLock by panicking while holding write guard
		let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
			let _guard = router.routes_compiled.write().unwrap();
			panic!("intentional panic to poison lock");
		}));

		// Act - compile_routes should recover from poisoned lock
		let errors = router.compile_routes();

		// Assert
		assert!(errors.is_empty());
		let result = router.match_own_routes("/health", &Method::GET);
		assert!(result.is_some());
	}

	#[rstest]
	fn test_route_matching_recovers_from_poisoned_method_router() {
		use hyper::Method;

		async fn dummy_handler(_req: Request) -> Result<Response> {
			Ok(Response::ok())
		}

		// Arrange
		let router = ServerRouter::new().function("/health", Method::GET, dummy_handler);
		router.compile_routes();

		// Poison the get_router RwLock
		let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
			let _guard = router.get_router.write().unwrap();
			panic!("intentional panic to poison lock");
		}));

		// Act - match_own_routes should recover from poisoned lock
		let result = router.match_own_routes("/health", &Method::GET);

		// Assert - route matching should still work
		assert!(result.is_some());
	}

	// --- ServerRouter::exclude() tests ---

	// Simple no-op middleware for testing exclude()
	struct NoopMiddleware;

	#[async_trait::async_trait]
	impl Middleware for NoopMiddleware {
		async fn process(
			&self,
			request: reinhardt_http::Request,
			next: std::sync::Arc<dyn reinhardt_http::Handler>,
		) -> reinhardt_http::Result<reinhardt_http::Response> {
			next.handle(request).await
		}
	}

	fn create_test_request(path: &str) -> reinhardt_http::Request {
		reinhardt_http::Request::builder()
			.method(Method::GET)
			.uri(path)
			.version(hyper::Version::HTTP_11)
			.headers(hyper::HeaderMap::new())
			.body(bytes::Bytes::new())
			.build()
			.unwrap()
	}

	#[rstest]
	fn test_server_router_exclude_stores_exclusion() {
		// Arrange & Act
		let router = ServerRouter::new()
			.with_middleware(NoopMiddleware)
			.exclude("/api/auth/")
			.exclude("/health");

		// Assert
		assert_eq!(router.middleware_exclusions.len(), 1);
		assert_eq!(router.middleware_exclusions[0].len(), 2);
		assert_eq!(router.middleware_exclusions[0][0], "/api/auth/");
		assert_eq!(router.middleware_exclusions[0][1], "/health");
	}

	#[rstest]
	fn test_server_router_exclude_only_affects_last_middleware() {
		// Arrange & Act
		let router = ServerRouter::new()
			.with_middleware(NoopMiddleware)
			.exclude("/admin/")
			.with_middleware(NoopMiddleware)
			.exclude("/api/auth/");

		// Assert
		assert_eq!(router.middleware_exclusions.len(), 2);
		assert_eq!(router.middleware_exclusions[0], vec!["/admin/"]);
		assert_eq!(router.middleware_exclusions[1], vec!["/api/auth/"]);
	}

	#[rstest]
	#[should_panic(expected = "exclude() called with no middleware")]
	fn test_server_router_exclude_panics_without_middleware() {
		// Arrange & Act & Assert
		let _router = ServerRouter::new().exclude("/api/auth/");
	}

	#[rstest]
	fn test_server_router_build_middleware_with_exclusions() {
		// Arrange
		let router = ServerRouter::new()
			.with_middleware(NoopMiddleware)
			.exclude("/admin/")
			.with_middleware(NoopMiddleware);

		// Act
		let built = router.build_middleware_with_exclusions();

		// Assert
		assert_eq!(built.len(), 2);

		let request_admin = create_test_request("/admin/dashboard");
		let request_public = create_test_request("/public");

		// First middleware (with exclusion) skips /admin/
		assert!(!built[0].should_continue(&request_admin));
		assert!(built[0].should_continue(&request_public));
		// Second middleware (no exclusion) runs for all
		assert!(built[1].should_continue(&request_admin));
		assert!(built[1].should_continue(&request_public));
	}

	// --- Framework-level 404/405 middleware tests (#3234) ---

	// Middleware that adds a security header to responses
	struct SecurityHeaderTestMiddleware;

	#[async_trait::async_trait]
	impl Middleware for SecurityHeaderTestMiddleware {
		async fn process(
			&self,
			request: reinhardt_http::Request,
			next: std::sync::Arc<dyn reinhardt_http::Handler>,
		) -> reinhardt_http::Result<reinhardt_http::Response> {
			let mut response = next.handle(request).await?;
			response.headers.insert(
				hyper::header::HeaderName::from_static("x-security-test"),
				hyper::header::HeaderValue::from_static("applied"),
			);
			Ok(response)
		}
	}

	async fn dummy_handler(_req: reinhardt_http::Request) -> reinhardt_http::Result<Response> {
		Ok(Response::ok())
	}

	#[rstest]
	#[tokio::test]
	async fn test_404_response_gets_middleware_headers() {
		// Arrange: router with middleware and a registered route
		let router = ServerRouter::new()
			.with_middleware(SecurityHeaderTestMiddleware)
			.route("/api/users/", Method::GET, dummy_handler);

		// Act: request a non-existent path
		let request = create_test_request("/nonexistent");
		let response = Handler::handle(&router, request).await.unwrap();

		// Assert: 404 response has security header from middleware
		assert_eq!(response.status, hyper::StatusCode::NOT_FOUND);
		assert_eq!(
			response
				.headers
				.get("x-security-test")
				.map(|v| v.to_str().unwrap()),
			Some("applied"),
			"Framework-level 404 response should have middleware security header"
		);
	}

	#[rstest]
	#[tokio::test]
	async fn test_405_response_gets_middleware_headers() {
		// Arrange: router with middleware and a GET-only route
		let router = ServerRouter::new()
			.with_middleware(SecurityHeaderTestMiddleware)
			.route("/api/users/", Method::GET, dummy_handler);

		// Act: send POST to a GET-only route
		let request = reinhardt_http::Request::builder()
			.method(Method::POST)
			.uri("/api/users/")
			.version(hyper::Version::HTTP_11)
			.headers(hyper::HeaderMap::new())
			.body(bytes::Bytes::new())
			.build()
			.unwrap();
		let response = Handler::handle(&router, request).await.unwrap();

		// Assert: 405 response has security header from middleware
		assert_eq!(response.status, hyper::StatusCode::METHOD_NOT_ALLOWED);
		assert_eq!(
			response
				.headers
				.get("x-security-test")
				.map(|v| v.to_str().unwrap()),
			Some("applied"),
			"Framework-level 405 response should have middleware security header"
		);
	}

	#[rstest]
	#[tokio::test]
	async fn test_404_without_middleware_returns_error() {
		// Arrange: router with no middleware
		let router = ServerRouter::new().route("/api/users/", Method::GET, dummy_handler);

		// Act: request a non-existent path
		let request = create_test_request("/nonexistent");
		let result = Handler::handle(&router, request).await;

		// Assert: returns Err (not wrapped in middleware chain)
		assert!(result.is_err(), "404 without middleware should return Err");
	}

	#[rstest]
	#[tokio::test]
	async fn test_404_respects_middleware_exclusions() {
		// Arrange: router with middleware excluded for /admin/
		let router = ServerRouter::new()
			.with_middleware(SecurityHeaderTestMiddleware)
			.exclude("/admin/")
			.route("/api/users/", Method::GET, dummy_handler);

		// Act: request non-existent path under excluded prefix
		let request = create_test_request("/admin/nonexistent");
		let response = Handler::handle(&router, request).await.unwrap();

		// Assert: 404 response but security header absent (middleware excluded)
		assert_eq!(response.status, hyper::StatusCode::NOT_FOUND);
		assert!(
			response.headers.get("x-security-test").is_none(),
			"404 under excluded path should NOT have middleware security header"
		);
	}

	// --- Prefix double-application fix tests (#3407, #3408) ---

	#[rstest]
	#[tokio::test]
	async fn test_function_route_with_prefix_strips_prefix_during_compilation() {
		// Arrange: register a route whose path already contains the prefix,
		// simulating server function registration (e.g., ServerFnRegistration::PATH)
		let router = ServerRouter::new().with_prefix("/api").function(
			"/api/server_fn/test",
			Method::POST,
			dummy_handler,
		);

		// Act: resolve the full path (resolve() strips "/api" before matchit lookup)
		let result = router.resolve("/api/server_fn/test", &Method::POST);

		// Assert: route matches without double-prefix issue
		assert!(
			result.is_some(),
			"POST /api/server_fn/test should match when router has prefix /api"
		);
	}

	#[rstest]
	#[tokio::test]
	async fn test_function_route_post_with_prefix_no_405() {
		// Arrange: register a POST route with a path that includes the prefix
		let router = ServerRouter::new().with_prefix("/api").function(
			"/api/users",
			Method::POST,
			dummy_handler,
		);

		// Act: resolve POST request (verifies no 405 Method Not Allowed)
		let result = router.resolve("/api/users", &Method::POST);

		// Assert: POST route is reachable
		assert!(
			result.is_some(),
			"POST /api/users should match when router has prefix /api (no 405)"
		);

		// Also verify GET returns None (route is POST-only)
		let get_result = router.resolve("/api/users", &Method::GET);
		assert!(
			get_result.is_none(),
			"GET /api/users should not match a POST-only route"
		);
	}

	#[rstest]
	#[tokio::test]
	async fn test_function_route_without_prefix_overlap_still_works() {
		// Arrange: route path does not start with the prefix
		let router =
			ServerRouter::new()
				.with_prefix("/api")
				.function("/health", Method::GET, dummy_handler);

		// Act: resolve a path under the prefix
		let result = router.resolve("/api/health", &Method::GET);

		// Assert: route matches (path kept as-is since it does not start with prefix)
		assert!(
			result.is_some(),
			"/api/health should match /health route under /api prefix"
		);
	}

	// --- Leading slash normalization fix tests (#3419) ---
	//
	// strip_prefix_normalized: unit tests (normal / edge / error)

	#[rstest]
	// Normal: trailing-slash prefix strips correctly
	#[case("/api/", "/api/auth/register/", "/auth/register/")]
	// Normal: non-trailing-slash prefix strips correctly
	#[case("/api", "/api/auth/register/", "/auth/register/")]
	// Normal: prefix equals full path → root "/"
	#[case("/api/", "/api/", "/")]
	#[case("/api", "/api", "/")]
	// Normal: single-segment after strip
	#[case("/api/", "/api/health", "/health")]
	#[case("/v1/", "/v1/users/", "/users/")]
	// Edge: empty prefix returns path as-is
	#[case("", "/anything", "/anything")]
	#[case("", "/", "/")]
	#[case("", "/a/b/c", "/a/b/c")]
	// Edge: prefix is "/" — remainder loses leading slash, must be restored
	#[case("/", "/health", "/health")]
	#[case("/", "/a/b/c", "/a/b/c")]
	// Edge: long multi-segment prefix
	#[case("/api/v2/internal/", "/api/v2/internal/metrics", "/metrics")]
	// Edge: path with URL-encoded segments
	#[case("/api/", "/api/users%2F123/", "/users%2F123/")]
	// Edge: path with hyphens and underscores
	#[case("/api/", "/api/my-resource/sub_path/", "/my-resource/sub_path/")]
	fn test_strip_prefix_normalized(
		#[case] prefix: &str,
		#[case] path: &str,
		#[case] expected: &str,
	) {
		// Act
		let result = ServerRouter::strip_prefix_normalized(prefix, path);

		// Assert
		assert!(
			result.is_some(),
			"strip_prefix_normalized({prefix:?}, {path:?}) should return Some"
		);
		let normalized = result.unwrap();
		assert_eq!(
			normalized.as_ref(),
			expected,
			"strip_prefix_normalized({prefix:?}, {path:?})"
		);
	}

	#[rstest]
	// Error: path doesn't start with prefix at all
	#[case("/api/", "/web/page")]
	#[case("/api", "/web/page")]
	// Error: partial prefix match (not a real prefix)
	#[case("/api/", "/ap")]
	#[case("/api", "/ap")]
	// Error: path is empty
	#[case("/api/", "")]
	#[case("/", "")]
	// Error: prefix longer than path
	#[case("/api/v2/", "/api/")]
	fn test_strip_prefix_normalized_returns_none(#[case] prefix: &str, #[case] path: &str) {
		// Act
		let result = ServerRouter::strip_prefix_normalized(prefix, path);

		// Assert
		assert!(
			result.is_none(),
			"strip_prefix_normalized({prefix:?}, {path:?}) should return None"
		);
	}

	#[rstest]
	fn test_strip_prefix_normalized_result_always_starts_with_slash() {
		// Arrange: various prefix/path combos that should succeed
		let cases = [
			("/api/", "/api/x"),
			("/a/b/c/", "/a/b/c/d"),
			("/", "/x"),
			("", "/x"),
			("/prefix/", "/prefix/rest/of/path"),
		];

		for (prefix, path) in cases {
			// Act
			let result = ServerRouter::strip_prefix_normalized(prefix, path);

			// Assert
			let normalized = result.unwrap();
			assert!(
				normalized.starts_with('/'),
				"result for ({prefix:?}, {path:?}) should start with '/' but got {normalized:?}"
			);
		}
	}

	// resolve(): normal cases with child routers

	#[rstest]
	#[tokio::test]
	async fn test_resolve_trailing_slash_prefix_child_router_matches() {
		// Arrange: parent with trailing-slash prefix, child with its own prefix
		let child = ServerRouter::new().with_prefix("/auth/").function(
			"/auth/register/",
			Method::POST,
			dummy_handler,
		);
		let parent = ServerRouter::new()
			.with_prefix("/api/")
			.mount("/auth/", child);

		// Act
		let result = parent.resolve("/api/auth/register/", &Method::POST);

		// Assert
		assert!(
			result.is_some(),
			"POST /api/auth/register/ should match child route through trailing-slash prefix"
		);
	}

	#[rstest]
	#[tokio::test]
	async fn test_resolve_no_trailing_slash_with_prefix_child_router_matches() {
		// Arrange: parent with_prefix (no trailing slash) + child mounted with trailing slash
		// Note: mount() requires trailing-slash prefix (Django convention),
		// but with_prefix() allows non-trailing-slash prefix
		let child = ServerRouter::new().with_prefix("/auth/").function(
			"/auth/login/",
			Method::POST,
			dummy_handler,
		);
		let parent = ServerRouter::new()
			.with_prefix("/api")
			.mount("/auth/", child);

		// Act
		let result = parent.resolve("/api/auth/login/", &Method::POST);

		// Assert
		assert!(
			result.is_some(),
			"POST /api/auth/login/ should match child route with non-trailing-slash parent prefix"
		);
	}

	#[rstest]
	#[tokio::test]
	async fn test_resolve_multiple_children_with_trailing_slash_prefix() {
		// Arrange: parent with trailing-slash prefix, multiple children
		let auth = ServerRouter::new().with_prefix("/auth/").function(
			"/auth/login/",
			Method::POST,
			dummy_handler,
		);
		let users = ServerRouter::new().with_prefix("/users/").function(
			"/users/",
			Method::GET,
			dummy_handler,
		);
		let parent = ServerRouter::new()
			.with_prefix("/api/")
			.mount("/auth/", auth)
			.mount("/users/", users);

		// Act & Assert: both children should be reachable
		assert!(
			parent.resolve("/api/auth/login/", &Method::POST).is_some(),
			"POST /api/auth/login/ should match auth child"
		);
		assert!(
			parent.resolve("/api/users/", &Method::GET).is_some(),
			"GET /api/users/ should match users child"
		);
	}

	#[rstest]
	#[tokio::test]
	async fn test_resolve_child_root_route_with_trailing_slash_prefix() {
		// Arrange: child's own root route (prefix stripped → "/")
		let child = ServerRouter::new().with_prefix("/dashboard/").function(
			"/dashboard/",
			Method::GET,
			dummy_handler,
		);
		let parent = ServerRouter::new()
			.with_prefix("/app/")
			.mount("/dashboard/", child);

		// Act
		let result = parent.resolve("/app/dashboard/", &Method::GET);

		// Assert
		assert!(
			result.is_some(),
			"GET /app/dashboard/ should match child root route"
		);
	}

	#[rstest]
	#[tokio::test]
	async fn test_resolve_parent_own_route_still_works_with_trailing_slash_prefix() {
		// Arrange: parent has both own routes and children
		let child = ServerRouter::new().with_prefix("/sub/").function(
			"/sub/action/",
			Method::POST,
			dummy_handler,
		);
		let parent = ServerRouter::new()
			.with_prefix("/api/")
			.function("/api/health", Method::GET, dummy_handler)
			.mount("/sub/", child);

		// Act & Assert
		assert!(
			parent.resolve("/api/health", &Method::GET).is_some(),
			"Parent's own route should still work"
		);
		assert!(
			parent.resolve("/api/sub/action/", &Method::POST).is_some(),
			"Child route should also work"
		);
	}

	// resolve(): deep nesting

	#[rstest]
	#[tokio::test]
	async fn test_resolve_deeply_nested_trailing_slash_prefixes() {
		// Arrange: 3 levels of trailing-slash prefixes
		let grandchild = ServerRouter::new().with_prefix("/profile/").function(
			"/profile/",
			Method::GET,
			dummy_handler,
		);
		let child = ServerRouter::new()
			.with_prefix("/users/")
			.mount("/profile/", grandchild);
		let parent = ServerRouter::new()
			.with_prefix("/api/")
			.mount("/users/", child);

		// Act
		let result = parent.resolve("/api/users/profile/", &Method::GET);

		// Assert
		assert!(
			result.is_some(),
			"GET /api/users/profile/ should match through 3 levels of trailing-slash prefix stripping"
		);
	}

	#[rstest]
	#[tokio::test]
	async fn test_resolve_mixed_trailing_and_non_trailing_slash_nesting() {
		// Arrange: with_prefix uses non-trailing slash, mount uses trailing slash
		let grandchild = ServerRouter::new().with_prefix("/detail").function(
			"/detail/",
			Method::GET,
			dummy_handler,
		);
		let child = ServerRouter::new()
			.with_prefix("/items/")
			.mount("/detail/", grandchild);
		let parent = ServerRouter::new()
			.with_prefix("/api/")
			.mount("/items/", child);

		// Act
		let result = parent.resolve("/api/items/detail/", &Method::GET);

		// Assert
		assert!(
			result.is_some(),
			"Mixed trailing/non-trailing prefix nesting should resolve correctly"
		);
	}

	// resolve(): error cases (should return None)

	#[rstest]
	#[tokio::test]
	async fn test_resolve_path_not_matching_parent_prefix() {
		// Arrange
		let child = ServerRouter::new().with_prefix("/auth/").function(
			"/auth/login/",
			Method::POST,
			dummy_handler,
		);
		let parent = ServerRouter::new()
			.with_prefix("/api/")
			.mount("/auth/", child);

		// Act
		let result = parent.resolve("/web/auth/login/", &Method::POST);

		// Assert
		assert!(
			result.is_none(),
			"Path not matching parent prefix should return None"
		);
	}

	#[rstest]
	#[tokio::test]
	async fn test_resolve_path_matches_parent_but_not_child() {
		// Arrange
		let child = ServerRouter::new().with_prefix("/auth/").function(
			"/auth/login/",
			Method::POST,
			dummy_handler,
		);
		let parent = ServerRouter::new()
			.with_prefix("/api/")
			.mount("/auth/", child);

		// Act: path under parent prefix but doesn't match any child
		let result = parent.resolve("/api/unknown/path/", &Method::GET);

		// Assert
		assert!(
			result.is_none(),
			"Path matching parent but not child should return None"
		);
	}

	#[rstest]
	#[tokio::test]
	async fn test_resolve_wrong_method_through_child_with_trailing_slash_prefix() {
		// Arrange: child only has POST route
		let child = ServerRouter::new().with_prefix("/auth/").function(
			"/auth/login/",
			Method::POST,
			dummy_handler,
		);
		let parent = ServerRouter::new()
			.with_prefix("/api/")
			.mount("/auth/", child);

		// Act: try GET instead of POST
		let result = parent.resolve("/api/auth/login/", &Method::GET);

		// Assert
		assert!(
			result.is_none(),
			"Wrong HTTP method through child router should return None"
		);
	}

	// path_exists_for_any_method(): normal / error / edge

	#[rstest]
	#[tokio::test]
	async fn test_path_exists_with_trailing_slash_prefix_and_child() {
		// Arrange
		let child = ServerRouter::new().with_prefix("/users/").function(
			"/users/",
			Method::GET,
			dummy_handler,
		);
		let parent = ServerRouter::new()
			.with_prefix("/api/")
			.mount("/users/", child);

		// Act
		let exists = parent.path_exists_for_any_method("/api/users/");

		// Assert
		assert!(
			exists,
			"path_exists_for_any_method should find path in child router after prefix normalization"
		);
	}

	#[rstest]
	#[tokio::test]
	async fn test_path_exists_nonexistent_path_with_trailing_slash_prefix() {
		// Arrange
		let child = ServerRouter::new().with_prefix("/users/").function(
			"/users/",
			Method::GET,
			dummy_handler,
		);
		let parent = ServerRouter::new()
			.with_prefix("/api/")
			.mount("/users/", child);

		// Act
		let exists = parent.path_exists_for_any_method("/api/nonexistent/");

		// Assert
		assert!(
			!exists,
			"path_exists_for_any_method should return false for nonexistent path"
		);
	}

	#[rstest]
	#[tokio::test]
	async fn test_path_exists_wrong_prefix_returns_false() {
		// Arrange
		let child = ServerRouter::new().with_prefix("/users/").function(
			"/users/",
			Method::GET,
			dummy_handler,
		);
		let parent = ServerRouter::new()
			.with_prefix("/api/")
			.mount("/users/", child);

		// Act
		let exists = parent.path_exists_for_any_method("/web/users/");

		// Assert
		assert!(
			!exists,
			"path_exists_for_any_method with wrong parent prefix should return false"
		);
	}

	#[rstest]
	#[tokio::test]
	async fn test_path_exists_deeply_nested_with_trailing_slash_prefix() {
		// Arrange: 3-level nesting
		let grandchild = ServerRouter::new().with_prefix("/edit/").function(
			"/edit/",
			Method::PUT,
			dummy_handler,
		);
		let child = ServerRouter::new()
			.with_prefix("/items/")
			.mount("/edit/", grandchild);
		let parent = ServerRouter::new()
			.with_prefix("/api/")
			.mount("/items/", child);

		// Act
		let exists = parent.path_exists_for_any_method("/api/items/edit/");

		// Assert
		assert!(
			exists,
			"path_exists_for_any_method should find deeply nested path through trailing-slash prefixes"
		);
	}

	// Edge cases: compile_routes with trailing-slash prefix

	#[rstest]
	#[tokio::test]
	async fn test_function_route_with_trailing_slash_prefix_compiles_correctly() {
		// Arrange: route path includes prefix with trailing slash
		let router = ServerRouter::new().with_prefix("/api/").function(
			"/api/server_fn/test",
			Method::POST,
			dummy_handler,
		);

		// Act
		let result = router.resolve("/api/server_fn/test", &Method::POST);

		// Assert
		assert!(
			result.is_some(),
			"Route with trailing-slash prefix should compile and resolve correctly"
		);
	}

	#[rstest]
	#[should_panic(expected = "URL route prefix cannot be an empty string")]
	fn test_mount_with_empty_prefix_panics() {
		// Arrange & Act: mounting with empty prefix should panic
		let child = ServerRouter::new().function("/catch/", Method::GET, dummy_handler);
		let _parent = ServerRouter::new().with_prefix("/api/").mount("", child);
	}

	#[rstest]
	#[tokio::test]
	async fn test_resolve_child_with_slash_prefix_under_trailing_slash_parent() {
		// Arrange: child router with "/" prefix under parent with trailing-slash prefix
		let child =
			ServerRouter::new()
				.with_prefix("/")
				.function("/catch/", Method::GET, dummy_handler);
		let parent = ServerRouter::new().with_prefix("/api/").mount("/", child);

		// Act
		let result = parent.resolve("/api/catch/", &Method::GET);

		// Assert
		assert!(
			result.is_some(),
			"Child with '/' prefix under trailing-slash parent should match"
		);
	}

	// ===================================================================
	// Duplicate route name detection tests (Issue #3462)
	// ===================================================================

	#[rstest]
	fn test_register_all_routes_detects_duplicate_names() {
		async fn handler_a(_req: Request) -> Result<Response> {
			Ok(Response::ok())
		}
		async fn handler_b(_req: Request) -> Result<Response> {
			Ok(Response::ok())
		}

		// Arrange — two routes with the same name in the same router
		let mut router = ServerRouter::new()
			.with_namespace("api")
			.function_named("/users", Method::GET, "list", handler_a)
			.function_named("/items", Method::GET, "list", handler_b);

		// Act
		let errors = router.register_all_routes();

		// Assert
		assert_eq!(errors.len(), 1);
		assert!(errors[0].contains("Duplicate route name 'api:list'"));
	}

	#[rstest]
	fn test_validate_route_names_succeeds_with_unique_names() {
		async fn handler_a(_req: Request) -> Result<Response> {
			Ok(Response::ok())
		}
		async fn handler_b(_req: Request) -> Result<Response> {
			Ok(Response::ok())
		}

		// Arrange
		let router = ServerRouter::new()
			.with_namespace("api")
			.function_named("/users", Method::GET, "users-list", handler_a)
			.function_named("/items", Method::GET, "items-list", handler_b);

		// Act
		let result = router.validate_route_names();

		// Assert
		assert!(result.is_ok());
	}

	#[rstest]
	fn test_validate_routes_includes_name_errors() {
		async fn handler_a(_req: Request) -> Result<Response> {
			Ok(Response::ok())
		}
		async fn handler_b(_req: Request) -> Result<Response> {
			Ok(Response::ok())
		}

		// Arrange — duplicate name
		let router = ServerRouter::new()
			.with_namespace("api")
			.function_named("/users", Method::GET, "list", handler_a)
			.function_named("/items", Method::GET, "list", handler_b);

		// Act
		let result = router.validate_routes();

		// Assert
		assert!(result.is_err());
		let errors = result.unwrap_err();
		assert!(errors.iter().any(|e| e.contains("Duplicate route name")));
	}
}
