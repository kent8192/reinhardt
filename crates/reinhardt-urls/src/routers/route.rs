use crate::routers::server_router::RouteContractMetadata;
use reinhardt_http::{
	Handler, RequestlessSyncHandler, RequestlessSyncHandlerAdapter, SyncHandler, SyncHandlerAdapter,
};
use reinhardt_middleware::Middleware;
use std::sync::Arc;

/// Route definition
/// Uses composition to combine path patterns with handlers
/// Similar to Django's URLPattern
#[derive(Clone)]
pub struct Route {
	/// URL path pattern (e.g., `"/users/{id}/"`)
	pub path: String,
	handler: Arc<dyn Handler>,
	sync_handler: Option<Arc<dyn SyncHandler>>,
	requestless_sync_handler: Option<Arc<dyn RequestlessSyncHandler>>,
	/// Optional route name for reverse URL lookup.
	pub name: Option<String>,
	/// Namespace for this route (e.g., "users", "api")
	/// When combined with name, forms "namespace:name"
	pub namespace: Option<String>,
	/// Middleware stack for this route
	/// Applied in addition to router-level middleware
	pub middleware: Vec<Arc<dyn Middleware>>,
	pub(crate) contract_metadata: Option<RouteContractMetadata>,
}

impl Route {
	/// Create a new route
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::routers::Route;
	/// use reinhardt_http::Handler;
	/// use std::sync::Arc;
	///
	/// // Create a simple route (using a dummy handler for demonstration)
	/// # use async_trait::async_trait;
	/// # use reinhardt_http::{Request, Response, Result};
	/// # struct DummyHandler;
	/// # #[async_trait]
	/// # impl Handler for DummyHandler {
	/// #     async fn handle(&self, _req: Request) -> Result<Response> {
	/// #         Ok(Response::ok())
	/// #     }
	/// # }
	/// let handler = Arc::new(DummyHandler);
	/// let route = Route::new("/users/", handler);
	/// assert_eq!(route.path, "/users/");
	/// ```
	pub fn new(path: impl Into<String>, handler: Arc<dyn Handler>) -> Self {
		Self {
			path: path.into(),
			handler,
			sync_handler: None,
			requestless_sync_handler: None,
			name: None,
			namespace: None,
			middleware: Vec::new(),
			contract_metadata: None,
		}
	}

	/// Create a new route from a concrete handler (preferred method)
	///
	/// This method allows you to pass a handler directly without wrapping it in `Arc`.
	/// The `Arc` wrapping is handled internally for you.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::routers::Route;
	/// use reinhardt_http::Handler;
	///
	/// # use async_trait::async_trait;
	/// # use reinhardt_http::{Request, Response, Result};
	/// # struct DummyHandler;
	/// # #[async_trait]
	/// # impl Handler for DummyHandler {
	/// #     async fn handle(&self, _req: Request) -> Result<Response> {
	/// #         Ok(Response::ok())
	/// #     }
	/// # }
	/// // No Arc::new() needed!
	/// let route = Route::from_handler("/users/", DummyHandler);
	/// assert_eq!(route.path, "/users/");
	/// ```
	pub fn from_handler<H>(path: impl Into<String>, handler: H) -> Self
	where
		H: Handler + 'static,
	{
		let path = path.into();
		Self {
			contract_metadata: Some(RouteContractMetadata {
				handler: format!("route:{path}"),
				authentication: reinhardt_core::endpoint::AuthProtection::None,
				guard: None,
			}),
			path,
			handler: Arc::new(handler),
			sync_handler: None,
			requestless_sync_handler: None,
			name: None,
			namespace: None,
			middleware: Vec::new(),
		}
	}

	/// Create a new route from a synchronous handler.
	///
	/// The route keeps both the synchronous handler and an async adapter. Routers
	/// can call the synchronous handler directly when no middleware is present,
	/// while middleware and legacy APIs continue to use the async handler trait.
	pub fn from_sync_handler<H>(path: impl Into<String>, handler: H) -> Self
	where
		H: SyncHandler + 'static,
	{
		let path = path.into();
		let sync_handler: Arc<dyn SyncHandler> = Arc::new(handler);
		Self::from_sync_handler_arc(path.clone(), sync_handler).with_contract_metadata(
			RouteContractMetadata {
				handler: format!("route:{path}"),
				authentication: reinhardt_core::endpoint::AuthProtection::None,
				guard: None,
			},
		)
	}

	/// Create a new route from an already shared synchronous handler.
	pub fn from_sync_handler_arc(
		path: impl Into<String>,
		sync_handler: Arc<dyn SyncHandler>,
	) -> Self {
		let handler: Arc<dyn Handler> = Arc::new(SyncHandlerAdapter::new(sync_handler.clone()));
		Self {
			path: path.into(),
			handler,
			sync_handler: Some(sync_handler),
			requestless_sync_handler: None,
			name: None,
			namespace: None,
			middleware: Vec::new(),
			contract_metadata: None,
		}
	}

	/// Create a new route from a requestless synchronous handler.
	///
	/// The route keeps a requestless handler for server adapters that can
	/// produce the response before constructing a full request. It also keeps
	/// request-shaped adapters so middleware and legacy APIs remain usable.
	pub fn from_requestless_sync_handler<H>(path: impl Into<String>, handler: H) -> Self
	where
		H: RequestlessSyncHandler + 'static,
	{
		let path = path.into();
		let requestless_handler: Arc<dyn RequestlessSyncHandler> = Arc::new(handler);
		Self::from_requestless_sync_handler_arc(path.clone(), requestless_handler)
			.with_contract_metadata(RouteContractMetadata {
				handler: format!("route:{path}"),
				authentication: reinhardt_core::endpoint::AuthProtection::None,
				guard: None,
			})
	}

	/// Create a new route from an already shared requestless synchronous handler.
	pub fn from_requestless_sync_handler_arc(
		path: impl Into<String>,
		requestless_handler: Arc<dyn RequestlessSyncHandler>,
	) -> Self {
		let adapter = Arc::new(RequestlessSyncHandlerAdapter::new(
			requestless_handler.clone(),
		));
		let handler: Arc<dyn Handler> = adapter.clone();
		let sync_handler: Arc<dyn SyncHandler> = adapter;
		Self {
			path: path.into(),
			handler,
			sync_handler: Some(sync_handler),
			requestless_sync_handler: Some(requestless_handler),
			name: None,
			namespace: None,
			middleware: Vec::new(),
			contract_metadata: None,
		}
	}

	pub(crate) fn with_contract_metadata(mut self, metadata: RouteContractMetadata) -> Self {
		self.contract_metadata = Some(metadata);
		self
	}

	/// Set the namespace of the route
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::routers::Route;
	/// use reinhardt_http::Handler;
	/// use std::sync::Arc;
	///
	/// # use async_trait::async_trait;
	/// # use reinhardt_http::{Request, Response, Result};
	/// # struct DummyHandler;
	/// # #[async_trait]
	/// # impl Handler for DummyHandler {
	/// #     async fn handle(&self, _req: Request) -> Result<Response> {
	/// #         Ok(Response::ok())
	/// #     }
	/// # }
	/// let handler = Arc::new(DummyHandler);
	/// let route = Route::new("/users/", handler)
	///     .with_namespace("api");
	/// assert_eq!(route.namespace, Some("api".to_string()));
	/// ```
	pub fn with_namespace(mut self, namespace: impl Into<String>) -> Self {
		self.namespace = Some(namespace.into());
		self
	}

	/// Add middleware to this route
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::routers::Route;
	/// use reinhardt_http::Handler;
	/// use reinhardt_middleware::LoggingMiddleware;
	/// use std::sync::Arc;
	///
	/// # use async_trait::async_trait;
	/// # use reinhardt_http::{Request, Response, Result};
	/// # struct DummyHandler;
	/// # #[async_trait]
	/// # impl Handler for DummyHandler {
	/// #     async fn handle(&self, _req: Request) -> Result<Response> {
	/// #         Ok(Response::ok())
	/// #     }
	/// # }
	/// let handler = Arc::new(DummyHandler);
	/// let route = Route::new("/users/", handler)
	///     .with_middleware(Arc::new(LoggingMiddleware::new()));
	/// assert_eq!(route.middleware.len(), 1);
	/// ```
	pub fn with_middleware(mut self, middleware: Arc<dyn Middleware>) -> Self {
		self.middleware.push(middleware);
		self
	}

	/// Get the full name including namespace (e.g., "users:list")
	/// Similar to Django's view_name in ResolverMatch
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::routers::Route;
	/// use reinhardt_http::Handler;
	/// use std::sync::Arc;
	///
	/// # use async_trait::async_trait;
	/// # use reinhardt_http::{Request, Response, Result};
	/// # struct DummyHandler;
	/// # #[async_trait]
	/// # impl Handler for DummyHandler {
	/// #     async fn handle(&self, _req: Request) -> Result<Response> {
	/// #         Ok(Response::ok())
	/// #     }
	/// # }
	/// let handler = Arc::new(DummyHandler);
	///
	/// // With namespace and name
	/// let mut route = Route::new("/users/", handler.clone())
	///     .with_namespace("api");
	/// route.name = Some("list".to_string());
	/// assert_eq!(route.full_name(), Some("api:list".to_string()));
	///
	/// // With only name
	/// let mut route = Route::new("/users/", handler.clone());
	/// route.name = Some("list".to_string());
	/// assert_eq!(route.full_name(), Some("list".to_string()));
	///
	/// // Without name
	/// let route = Route::new("/users/", handler);
	/// assert_eq!(route.full_name(), None);
	/// ```
	pub fn full_name(&self) -> Option<String> {
		match (&self.namespace, &self.name) {
			(Some(ns), Some(name)) => Some(format!("{}:{}", ns, name)),
			(None, Some(name)) => Some(name.clone()),
			_ => None,
		}
	}

	/// Check if this route matches a namespace pattern
	/// Used for namespace-based versioning
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::routers::Route;
	/// use reinhardt_http::Handler;
	/// use std::sync::Arc;
	///
	/// # use async_trait::async_trait;
	/// # use reinhardt_http::{Request, Response, Result};
	/// # struct DummyHandler;
	/// # #[async_trait]
	/// # impl Handler for DummyHandler {
	/// #     async fn handle(&self, _req: Request) -> Result<Response> {
	/// #         Ok(Response::ok())
	/// #     }
	/// # }
	/// let handler = Arc::new(DummyHandler);
	/// let route = Route::new("/v1/users/", handler)
	///     .with_namespace("v1");
	///
	/// assert!(route.matches_namespace_pattern("/v{version}/"));
	/// assert!(!route.matches_namespace_pattern("/api/{version}/"));
	/// ```
	pub fn matches_namespace_pattern(&self, pattern: &str) -> bool {
		// Convert pattern like "/v{version}/" to regex
		let regex_pattern = pattern.replace("{version}", r"([^/]+)").replace("/", r"\/");
		let full_pattern = format!("^{}", regex_pattern);

		if let Ok(regex) = regex::Regex::new(&full_pattern) {
			regex.is_match(&self.path)
		} else {
			false
		}
	}

	/// Extract version from namespace pattern
	/// Returns the version string if the route matches the pattern
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::routers::Route;
	/// use reinhardt_http::Handler;
	/// use std::sync::Arc;
	///
	/// # use async_trait::async_trait;
	/// # use reinhardt_http::{Request, Response, Result};
	/// # struct DummyHandler;
	/// # #[async_trait]
	/// # impl Handler for DummyHandler {
	/// #     async fn handle(&self, _req: Request) -> Result<Response> {
	/// #         Ok(Response::ok())
	/// #     }
	/// # }
	/// let handler = Arc::new(DummyHandler);
	/// let route = Route::new("/v1/users/", handler)
	///     .with_namespace("v1");
	///
	/// assert_eq!(route.extract_version_from_pattern("/v{version}/"), Some("1"));
	/// assert_eq!(route.extract_version_from_pattern("/api/{version}/"), None);
	/// ```
	pub fn extract_version_from_pattern(&self, pattern: &str) -> Option<&str> {
		// Convert pattern like "/v{version}/" to regex with capture group
		let regex_pattern = pattern.replace("{version}", r"([^/]+)").replace("/", r"\/");
		let full_pattern = format!("^{}", regex_pattern);

		if let Ok(regex) = regex::Regex::new(&full_pattern)
			&& let Some(captures) = regex.captures(&self.path)
			&& let Some(version_match) = captures.get(1)
		{
			return Some(version_match.as_str());
		}
		None
	}

	/// Get a reference to the route's handler
	///
	/// This method provides access to the handler without exposing the `Arc` wrapper.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::routers::Route;
	/// use reinhardt_http::Handler;
	///
	/// # use async_trait::async_trait;
	/// # use reinhardt_http::{Request, Response, Result};
	/// # struct DummyHandler;
	/// # #[async_trait]
	/// # impl Handler for DummyHandler {
	/// #     async fn handle(&self, _req: Request) -> Result<Response> {
	/// #         Ok(Response::ok())
	/// #     }
	/// # }
	/// let route = Route::from_handler("/users/", DummyHandler);
	/// let handler = route.handler();
	/// // Use handler as &dyn Handler
	/// ```
	pub fn handler(&self) -> &dyn Handler {
		&*self.handler
	}

	/// Get a cloned Arc of the handler (for cases where you need ownership)
	///
	/// In most cases, you should use `handler()` instead to get a reference.
	pub fn handler_arc(&self) -> Arc<dyn Handler> {
		Arc::clone(&self.handler)
	}

	pub(crate) fn sync_handler_arc(&self) -> Option<Arc<dyn SyncHandler>> {
		self.sync_handler.clone()
	}

	pub(crate) fn requestless_sync_handler_arc(&self) -> Option<Arc<dyn RequestlessSyncHandler>> {
		self.requestless_sync_handler.clone()
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use async_trait::async_trait;
	use reinhardt_http::{Request, Response, Result};

	struct DummyHandler;

	#[async_trait]
	impl Handler for DummyHandler {
		async fn handle(&self, _req: Request) -> Result<Response> {
			Ok(Response::ok())
		}
	}

	#[test]
	fn test_matches_namespace_pattern() {
		let handler = std::sync::Arc::new(DummyHandler);
		let route = Route::new("/v1/users/", handler).with_namespace("v1");

		assert!(route.matches_namespace_pattern("/v{version}/"));
		assert!(!route.matches_namespace_pattern("/api/{version}/"));
		assert!(!route.matches_namespace_pattern("/users/"));
	}

	#[test]
	fn test_extract_version_from_pattern() {
		let handler = std::sync::Arc::new(DummyHandler);
		let route = Route::new("/v1/users/", handler).with_namespace("v1");

		assert_eq!(
			route.extract_version_from_pattern("/v{version}/"),
			Some("1")
		);
		assert_eq!(route.extract_version_from_pattern("/api/{version}/"), None);
		assert_eq!(route.extract_version_from_pattern("/users/"), None);
	}

	#[test]
	fn test_extract_version_with_custom_pattern() {
		let handler = std::sync::Arc::new(DummyHandler);
		let route = Route::new("/api/v2/users/", handler).with_namespace("v2");

		assert_eq!(
			route.extract_version_from_pattern("/api/v{version}/"),
			Some("2")
		);
		assert_eq!(route.extract_version_from_pattern("/v{version}/"), None);
	}
}
