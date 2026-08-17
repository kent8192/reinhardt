//! Type definitions and small helpers used by [`ServerRouter`].
//!
//! Splitting the data definitions out of the main module keeps the
//! `ServerRouter` definition file focused on the struct itself, while the
//! method bodies live in dedicated submodules (`builder`, `registration`,
//! `compile`, `introspection`, `dispatch`).

use hyper::Method;
use matchit::Router as MatchitRouter;
use reinhardt_core::endpoint::AuthProtection;
use reinhardt_di::InjectionContext;
use reinhardt_http::{Handler, PathParams, RequestlessSyncHandler, SyncHandler};
use reinhardt_middleware::Middleware;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

/// Information about a registered middleware
///
/// Captures the short name and full type path of a middleware added via
/// [`ServerRouter::with_middleware()`](crate::routers::server_router::ServerRouter::with_middleware).
/// This enables runtime introspection of the middleware stack without
/// requiring `Middleware` to be `Debug`.
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

/// Application-contract metadata retained by a registered route.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RouteContractMetadata {
	/// Stable registration identifier for the executable handler.
	pub handler: String,
	/// Module containing the endpoint handler, when the handler is typed.
	pub module_path: Option<String>,
	/// Endpoint handler function name, when the handler is typed.
	pub function_name: Option<String>,
	/// Declared authentication requirement.
	pub authentication: AuthProtection,
	/// Optional human-readable guard description.
	pub guard: Option<String>,
}

/// A method-specific route resolved through all mounted router prefixes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MountedRouteContract {
	/// Fully resolved route path.
	pub path: String,
	/// HTTP method dispatched by the route.
	pub method: Method,
	/// Optional route name without the `!` opt-out sigil.
	pub name: Option<String>,
	/// Application-contract metadata for the executable handler.
	pub metadata: RouteContractMetadata,
}

/// Immutable method-indexed route table built from registered routes.
///
/// `ServerRouter` stores this behind `OnceLock` so route compilation remains
/// lazy while dispatch can read compiled matchit routers without a per-request
/// lock.
pub(crate) struct CompiledRoutes {
	pub(crate) get: MatchitRouter<RouteHandler>,
	pub(crate) post: MatchitRouter<RouteHandler>,
	pub(crate) put: MatchitRouter<RouteHandler>,
	pub(crate) delete: MatchitRouter<RouteHandler>,
	pub(crate) patch: MatchitRouter<RouteHandler>,
	pub(crate) head: MatchitRouter<RouteHandler>,
	pub(crate) options: MatchitRouter<RouteHandler>,
	pub(crate) exact_get: HashMap<String, RouteHandler>,
	pub(crate) exact_post: HashMap<String, RouteHandler>,
	pub(crate) exact_put: HashMap<String, RouteHandler>,
	pub(crate) exact_delete: HashMap<String, RouteHandler>,
	pub(crate) exact_patch: HashMap<String, RouteHandler>,
	pub(crate) exact_head: HashMap<String, RouteHandler>,
	pub(crate) exact_options: HashMap<String, RouteHandler>,
	pub(crate) custom: HashMap<Method, MatchitRouter<RouteHandler>>,
	pub(crate) exact_custom: HashMap<Method, HashMap<String, RouteHandler>>,
	pub(crate) errors: Vec<String>,
}

impl Default for CompiledRoutes {
	fn default() -> Self {
		Self {
			get: MatchitRouter::new(),
			post: MatchitRouter::new(),
			put: MatchitRouter::new(),
			delete: MatchitRouter::new(),
			patch: MatchitRouter::new(),
			head: MatchitRouter::new(),
			options: MatchitRouter::new(),
			exact_get: HashMap::new(),
			exact_post: HashMap::new(),
			exact_put: HashMap::new(),
			exact_delete: HashMap::new(),
			exact_patch: HashMap::new(),
			exact_head: HashMap::new(),
			exact_options: HashMap::new(),
			custom: HashMap::new(),
			exact_custom: HashMap::new(),
			errors: Vec::new(),
		}
	}
}

impl CompiledRoutes {
	pub(crate) fn exact_for_method(
		&self,
		method: &Method,
	) -> Option<&HashMap<String, RouteHandler>> {
		match *method {
			Method::GET => Some(&self.exact_get),
			Method::POST => Some(&self.exact_post),
			Method::PUT => Some(&self.exact_put),
			Method::DELETE => Some(&self.exact_delete),
			Method::PATCH => Some(&self.exact_patch),
			Method::HEAD => Some(&self.exact_head),
			Method::OPTIONS => Some(&self.exact_options),
			_ => self.exact_custom.get(method),
		}
	}

	pub(crate) fn exact_for_method_mut(
		&mut self,
		method: &Method,
	) -> Option<&mut HashMap<String, RouteHandler>> {
		match *method {
			Method::GET => Some(&mut self.exact_get),
			Method::POST => Some(&mut self.exact_post),
			Method::PUT => Some(&mut self.exact_put),
			Method::DELETE => Some(&mut self.exact_delete),
			Method::PATCH => Some(&mut self.exact_patch),
			Method::HEAD => Some(&mut self.exact_head),
			Method::OPTIONS => Some(&mut self.exact_options),
			_ => Some(self.exact_custom.entry(method.clone()).or_default()),
		}
	}

	pub(crate) fn router_for_method(
		&self,
		method: &Method,
	) -> Option<&MatchitRouter<RouteHandler>> {
		match *method {
			Method::GET => Some(&self.get),
			Method::POST => Some(&self.post),
			Method::PUT => Some(&self.put),
			Method::DELETE => Some(&self.delete),
			Method::PATCH => Some(&self.patch),
			Method::HEAD => Some(&self.head),
			Method::OPTIONS => Some(&self.options),
			_ => self.custom.get(method),
		}
	}

	pub(crate) fn router_for_method_mut(
		&mut self,
		method: &Method,
	) -> Option<&mut MatchitRouter<RouteHandler>> {
		match *method {
			Method::GET => Some(&mut self.get),
			Method::POST => Some(&mut self.post),
			Method::PUT => Some(&mut self.put),
			Method::DELETE => Some(&mut self.delete),
			Method::PATCH => Some(&mut self.patch),
			Method::HEAD => Some(&mut self.head),
			Method::OPTIONS => Some(&mut self.options),
			_ => Some(self.custom.entry(method.clone()).or_default()),
		}
	}

	pub(crate) fn method_routers(&self) -> impl Iterator<Item = &MatchitRouter<RouteHandler>> {
		[
			&self.get,
			&self.post,
			&self.put,
			&self.delete,
			&self.patch,
			&self.head,
			&self.options,
		]
		.into_iter()
		.chain(self.custom.values())
	}
}

/// Join two path segments, normalizing any double slashes.
///
/// Concatenates `prefix` and `suffix` and collapses consecutive `/` characters
/// into a single `/`. This helper does **not** insert a separator between
/// `prefix` and `suffix`; the caller is responsible for ensuring that either
/// `prefix` ends with `/` or `suffix` starts with `/` when a separator is
/// desired. The leading slash is always preserved.
///
/// # Contract
///
/// - If `prefix` ends with `/` or `suffix` starts with `/`, the result is a
///   well-formed joined path with a single `/` between segments.
/// - If neither holds, the segments are concatenated verbatim. This is
///   intentional: internal call sites always satisfy the contract above, and
///   collapsing only deduplicates existing slashes rather than inventing one.
///
/// # Examples
///
/// ```ignore
/// // crate-internal usage only
/// assert_eq!(join_path("/api/", "/users"), "/api/users");
/// assert_eq!(join_path("/api", "/users"), "/api/users");
/// // Caller-provided invariant violated: no separator is inserted.
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
pub(crate) struct RouteHandler {
	/// The actual handler
	pub(crate) handler: Arc<dyn Handler>,

	/// Optional synchronous fast-path handler.
	pub(crate) sync_handler: Option<Arc<dyn SyncHandler>>,

	/// Optional requestless synchronous fast-path handler.
	pub(crate) requestless_sync_handler: Option<Arc<dyn RequestlessSyncHandler>>,

	/// Route-level middleware
	pub(crate) middleware: Vec<Arc<dyn Middleware>>,

	/// Path parameter names in URL pattern declaration order.
	pub(crate) param_names: Arc<[String]>,
}

/// Route match result with metadata
pub(crate) struct RouteMatch<'a> {
	/// Matched handler
	pub handler: &'a Arc<dyn Handler>,

	/// Matched synchronous fast-path handler, when available.
	pub sync_handler: Option<&'a Arc<dyn SyncHandler>>,

	/// Matched requestless synchronous fast-path handler, when available.
	pub requestless_sync_handler: Option<&'a Arc<dyn RequestlessSyncHandler>>,

	/// Extracted path parameters in URL pattern declaration order.
	///
	/// Stored as ordered [`PathParams`] so downstream extractors such
	/// as `Path<(T1, T2)>` can rely on URL declaration order. See issue #4013.
	pub params: Option<PathParams>,

	/// Middleware stack to apply (parent → child order)
	pub middleware_stack: Vec<Arc<dyn Middleware>>,

	/// DI context
	pub di_context: Option<Arc<InjectionContext>>,
}

impl RouteMatch<'_> {
	/// Look up a path parameter by name.
	///
	/// `params` is stored in declaration order (see issue #4013) so this helper
	/// performs a linear scan. Path parameter sets are tiny in practice
	/// (typically 1–3 entries), so the cost is negligible compared to a
	/// `HashMap` lookup.
	#[cfg(test)]
	pub(crate) fn param(&self, name: &str) -> Option<&str> {
		self.params
			.as_ref()?
			.iter()
			.find(|(k, _)| *k == name)
			.map(|(_, v)| v)
	}
}

/// Function-based route
pub(crate) struct FunctionRoute {
	pub path: String,
	pub method: Method,
	pub handler: Arc<dyn Handler>,
	pub sync_handler: Option<Arc<dyn SyncHandler>>,
	pub requestless_sync_handler: Option<Arc<dyn RequestlessSyncHandler>>,
	pub name: Option<String>,
	pub metadata: RouteContractMetadata,
	/// Middleware stack for this route
	pub middleware: Vec<Arc<dyn Middleware>>,
}

/// Class-based view route
pub(crate) struct ViewRoute {
	pub path: String,
	pub handler: Arc<dyn Handler>,
	pub sync_handler: Option<Arc<dyn SyncHandler>>,
	pub requestless_sync_handler: Option<Arc<dyn RequestlessSyncHandler>>,
	pub name: Option<String>,
	pub metadata: Option<RouteContractMetadata>,
	/// Middleware stack for this route
	pub middleware: Vec<Arc<dyn Middleware>>,
}
