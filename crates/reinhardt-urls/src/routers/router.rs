use super::{PathMatcher, PathPattern, Route};
use async_trait::async_trait;
use reinhardt_http::{Handler, Request, Response, Result};
#[cfg(feature = "viewsets")]
use reinhardt_views::viewsets::ViewSet;
#[cfg(feature = "viewsets")]
use reinhardt_views::viewsets::middleware::process_viewset_request;
#[cfg(feature = "viewsets")]
use std::collections::HashMap;
#[cfg(feature = "viewsets")]
use std::sync::Arc;

/// Router trait - composes routes together
pub trait Router: Send + Sync {
	/// Add a route to this router.
	fn add_route(&mut self, route: Route);

	/// Mount routes from another source with a prefix
	fn mount(&mut self, prefix: &str, routes: Vec<Route>, namespace: Option<String>);

	/// Handle a request (similar to Handler::handle)
	fn route(&self, request: Request)
	-> impl std::future::Future<Output = Result<Response>> + Send;
}

/// Default router implementation
/// Similar to Django REST Framework's DefaultRouter and Django's URLResolver
pub struct DefaultRouter {
	routes: Vec<Route>,
	matcher: PathMatcher,
	/// URL reverser for name-to-URL resolution
	reverser: super::reverse::UrlReverser,
}

impl DefaultRouter {
	/// Create a new DefaultRouter
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::routers::DefaultRouter;
	///
	/// let router = DefaultRouter::new();
	/// assert_eq!(router.get_routes().len(), 0);
	/// ```
	pub fn new() -> Self {
		Self {
			routes: Vec::new(),
			matcher: PathMatcher::new(),
			reverser: super::reverse::UrlReverser::new(),
		}
	}
	/// Get a reference to the URL reverser
	/// This allows for URL name resolution (reverse routing)
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::routers::DefaultRouter;
	///
	/// let router = DefaultRouter::new();
	/// let reverser = router.reverser();
	/// assert_eq!(reverser.route_names().len(), 0);
	/// ```
	pub fn reverser(&self) -> &super::reverse::UrlReverser {
		&self.reverser
	}

	/// Reverse a URL name to a path
	/// Similar to Django's reverse() function
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::routers::{DefaultRouter, Router, path};
	/// use reinhardt_http::Handler;
	/// use std::sync::Arc;
	/// use std::collections::HashMap;
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
	/// let mut router = DefaultRouter::new();
	/// let mut route = path("/users/{id}/", handler)
	///     .with_namespace("users");
	/// route.name = Some("detail".to_string());
	/// router.add_route(route);
	///
	/// let mut params = HashMap::new();
	/// params.insert("id".to_string(), "123".to_string());
	///
	/// let url = router.reverse("users:detail", &params).unwrap();
	/// assert_eq!(url, "/users/123/");
	/// ```
	pub fn reverse(
		&self,
		name: &str,
		params: &std::collections::HashMap<String, String>,
	) -> super::reverse::ReverseResult<String> {
		self.reverser.reverse(name, params)
	}
	/// Reverse a URL name with positional parameters
	/// Convenience method for simple parameter passing
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::routers::{DefaultRouter, Router, path};
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
	/// let mut router = DefaultRouter::new();
	/// let mut route = path("/users/{id}/", handler);
	/// route.name = Some("detail".to_string());
	/// router.add_route(route);
	///
	/// let url = router.reverse_with("detail", &[("id", "123")]).unwrap();
	/// assert_eq!(url, "/users/123/");
	/// ```
	pub fn reverse_with<S: AsRef<str>>(
		&self,
		name: &str,
		params: &[(S, S)],
	) -> super::reverse::ReverseResult<String> {
		self.reverser.reverse_with(name, params)
	}
	/// Register a ViewSet with automatic URL pattern generation
	/// Similar to DRF's router.register()
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::routers::DefaultRouter;
	/// use reinhardt_views::viewsets::ViewSet;
	/// use std::sync::Arc;
	///
	/// # use async_trait::async_trait;
	/// # use reinhardt_http::{Request, Response, Result};
	/// # use reinhardt_views::viewsets::Action;
	/// # struct DummyViewSet;
	/// # #[async_trait]
	/// # impl ViewSet for DummyViewSet {
	/// #     fn get_basename(&self) -> &str { "users" }
	/// #     async fn dispatch(&self, _req: Request, _action: Action) -> Result<Response> {
	/// #         Ok(Response::ok())
	/// #     }
	/// # }
	/// let viewset = Arc::new(DummyViewSet);
	/// let mut router = DefaultRouter::new();
	/// router.register_viewset("users", viewset);
	///
	// This creates two routes:
	// - /users/ for list action
	// - /users/{id}/ for detail actions
	/// assert_eq!(router.get_routes().len(), 2);
	/// ```
	#[cfg(feature = "viewsets")]
	pub fn register_viewset<V: ViewSet + 'static>(&mut self, prefix: &str, viewset: Arc<V>) {
		let basename = viewset.get_basename();
		let lookup_field = viewset.get_lookup_field();

		// List/Create endpoint: /prefix/
		let list_path = format!("/{}/", prefix.trim_matches('/'));
		let list_handler = ViewSetListHandler::new(viewset.clone());
		let mut list_route = Route::new(list_path.clone(), Arc::new(list_handler));
		list_route.name = Some(format!("{}-list", basename));
		self.add_route(list_route);

		// Detail endpoint: /prefix/{lookup_field}/
		let detail_path = format!("/{}/{{{}}}/", prefix.trim_matches('/'), lookup_field);
		let detail_handler = ViewSetDetailHandler::new(viewset.clone());
		let mut detail_route = Route::new(detail_path, Arc::new(detail_handler));
		detail_route.name = Some(format!("{}-detail", basename));
		self.add_route(detail_route);

		// Register custom actions from ViewSet
		let extra_actions = viewset.get_extra_actions();
		for action in extra_actions {
			// `#[action]` enforces that `url_path` starts with `/`
			// (see `crates/reinhardt-core/macros/src/action.rs`). When this
			// value flows into the `format!("/{}/.../{}/")` template below,
			// a stored `/children` would yield `.../{id}//children/` (double
			// slash). Strip the leading `/` so the segment join is uniform
			// regardless of whether the value came from
			// `#[action(url_path = "/...")]` or the `action.name` fallback
			// (which has no slash).
			let action_url_path = action
				.url_path
				.as_deref()
				.unwrap_or(action.name.as_str())
				.trim_start_matches('/')
				.to_string();

			let action_path = if action.detail {
				// Detail action: /prefix/{lookup_field}/action_name/
				format!(
					"/{}/{{{}}}/{}/",
					prefix.trim_matches('/'),
					lookup_field,
					action_url_path
				)
			} else {
				// List action: /prefix/action_name/
				format!("/{}/{}/", prefix.trim_matches('/'), action_url_path)
			};

			let action_url_name = if let Some(ref url_name) = action.url_name {
				format!("{}-{}", basename, url_name)
			} else {
				format!("{}-{}", basename, action.name)
			};

			let action_handler =
				ActionHandlerWrapper::new(viewset.clone(), action.handler.clone(), action.methods);
			let mut action_route = Route::new(action_path, Arc::new(action_handler));
			action_route.name = Some(action_url_name);
			self.add_route(action_route);
		}
	}
	/// Get URL map for a ViewSet's extra actions
	/// Returns a map of action names to their full URLs
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::routers::DefaultRouter;
	/// use reinhardt_views::viewsets::{ViewSet, register_action, ActionMetadata, FunctionActionHandler};
	/// use std::sync::Arc;
	///
	/// # use async_trait::async_trait;
	/// # use reinhardt_http::{Request, Response, Result};
	/// # use reinhardt_views::viewsets::Action;
	/// # #[derive(Debug, Clone)]
	/// # struct DummyViewSet;
	/// # #[async_trait]
	/// # impl ViewSet for DummyViewSet {
	/// #     fn get_basename(&self) -> &str { "test" }
	/// #     async fn dispatch(&self, _req: Request, _action: Action) -> Result<Response> {
	/// #         Ok(Response::ok())
	/// #     }
	/// # }
	/// let viewset = Arc::new(DummyViewSet);
	/// let mut router = DefaultRouter::new();
	/// router.register_viewset("test", viewset.clone());
	///
	/// let url_map = router.get_action_url_map(viewset.as_ref(), "http://testserver");
	/// // The ViewSet has no extra actions, so the map will be empty
	/// assert!(url_map.is_empty());
	/// ```
	#[cfg(feature = "viewsets")]
	pub fn get_action_url_map<V: ViewSet>(
		&self,
		viewset: &V,
		base_url: &str,
	) -> HashMap<String, String> {
		let basename = viewset.get_basename();
		let lookup_field = viewset.get_lookup_field();
		let mut url_map = HashMap::new();

		// Get extra actions
		let extra_actions = viewset.get_extra_actions();

		for action in extra_actions {
			let action_url_name = if let Some(ref url_name) = action.url_name {
				format!("{}-{}", basename, url_name)
			} else {
				format!("{}-{}", basename, action.name)
			};

			// For detail actions, we need to provide a lookup_field parameter
			let mut params = HashMap::new();
			if action.detail {
				params.insert(lookup_field.to_string(), "1".to_string());
			}

			// Try to reverse the URL
			match self.reverse(&action_url_name, &params) {
				Ok(path) => {
					let full_url = format!("{}{}", base_url.trim_end_matches('/'), path);
					// Use the original action name as the key, not the URL name
					url_map.insert(action.name.clone(), full_url);
				}
				Err(_) => {
					// If reverse fails, we can't include this action in the URL map
					// This is expected for some actions that require specific parameters
				}
			}
		}

		url_map
	}

	/// Get all registered routes
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::routers::{DefaultRouter, Router, path};
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
	/// let mut router = DefaultRouter::new();
	/// router.add_route(path("/users/", handler));
	///
	/// assert_eq!(router.get_routes().len(), 1);
	/// assert_eq!(router.get_routes()[0].path, "/users/");
	/// ```
	pub fn get_routes(&self) -> &[Route] {
		&self.routes
	}

	/// Find routes that match a namespace pattern
	/// Used for namespace-based versioning
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::routers::{DefaultRouter, Router, path};
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
	/// let mut router = DefaultRouter::new();
	/// router.add_route(path("/v1/users/", handler.clone()).with_namespace("v1"));
	/// router.add_route(path("/v2/users/", handler).with_namespace("v2"));
	///
	/// let v1_routes = router.find_routes_by_namespace_pattern("/v{version}/");
	/// assert_eq!(v1_routes.len(), 2);
	/// ```
	pub fn find_routes_by_namespace_pattern(&self, pattern: &str) -> Vec<&Route> {
		self.routes
			.iter()
			.filter(|route| route.matches_namespace_pattern(pattern))
			.collect()
	}

	/// Extract version from a path using namespace pattern
	/// Returns the version string if the path matches the pattern
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::routers::DefaultRouter;
	///
	/// let router = DefaultRouter::new();
	/// assert_eq!(router.extract_version_from_path("/v1/users/", "/v{version}/"), Some("1"));
	/// assert_eq!(router.extract_version_from_path("/v2/api/", "/v{version}/"), Some("2"));
	/// assert_eq!(router.extract_version_from_path("/users/", "/v{version}/"), None);
	/// ```
	pub fn extract_version_from_path<'a>(&self, path: &'a str, pattern: &str) -> Option<&'a str> {
		// Convert pattern like "/v{version}/" to regex with capture group
		let regex_pattern = pattern.replace("{version}", r"([^/]+)").replace("/", r"\/");
		let full_pattern = format!("^{}", regex_pattern);

		if let Ok(regex) = regex::Regex::new(&full_pattern)
			&& let Some(captures) = regex.captures(path)
			&& let Some(version_match) = captures.get(1)
		{
			return Some(version_match.as_str());
		}
		None
	}

	/// Get all unique versions found in registered routes for a given pattern
	/// Used for namespace-based versioning to discover available versions
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::routers::{DefaultRouter, Router, path};
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
	/// let mut router = DefaultRouter::new();
	/// router.add_route(path("/v1/users/", handler.clone()).with_namespace("v1"));
	/// router.add_route(path("/v2/users/", handler.clone()).with_namespace("v2"));
	/// router.add_route(path("/v1/posts/", handler).with_namespace("v1"));
	///
	/// let versions = router.get_available_versions("/v{version}/");
	/// assert!(versions.contains(&"1".to_string()));
	/// assert!(versions.contains(&"2".to_string()));
	/// assert_eq!(versions.len(), 2);
	/// ```
	pub fn get_available_versions(&self, pattern: &str) -> Vec<String> {
		let mut versions = std::collections::HashSet::new();

		for route in &self.routes {
			if let Some(version) = route.extract_version_from_pattern(pattern) {
				versions.insert(version.to_string());
			}
		}

		let mut version_vec: Vec<String> = versions.into_iter().collect();
		version_vec.sort();
		version_vec
	}
}

impl Default for DefaultRouter {
	fn default() -> Self {
		Self::new()
	}
}

impl Router for DefaultRouter {
	fn add_route(&mut self, route: Route) {
		let pattern = PathPattern::new(&route.path).expect("Invalid path pattern");
		let handler_id = route
			.full_name()
			.or_else(|| route.name.clone())
			.unwrap_or_else(|| format!("route_{}", self.routes.len()));

		// The `Router::add_route` trait signature is infallible, so surface
		// pattern-insertion failures via tracing rather than panicking. If
		// insertion fails we skip recording the route to keep the matcher's
		// linear/radix views consistent with the registered route list.
		if let Err(e) = self.matcher.add_pattern(pattern, handler_id.clone()) {
			tracing::warn!(
				"DefaultRouter: failed to register pattern for path '{}': {}",
				route.path,
				e
			);
			return;
		}

		// Register route for reverse lookup if it has a name
		if (route.full_name().is_some() || route.name.is_some())
			&& let Err(e) = self.reverser.register(route.clone())
		{
			tracing::warn!("{}", e);
		}

		self.routes.push(route);
	}

	/// Mount routes at the given prefix
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::routers::{DefaultRouter, Router, path};
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
	/// let mut list_route = path("/", handler.clone());
	/// list_route.name = Some("list".to_string());
	/// let mut detail_route = path("/{id}/", handler);
	/// detail_route.name = Some("detail".to_string());
	/// let users_routes = vec![list_route, detail_route];
	///
	/// let mut router = DefaultRouter::new();
	/// router.mount("/users", users_routes, Some("users".to_string()));
	///
	/// // Routes are prefixed: /users/ and /users/{id}/
	/// assert_eq!(router.get_routes().len(), 2);
	/// ```
	fn mount(&mut self, prefix: &str, routes: Vec<Route>, namespace: Option<String>) {
		let prefix = prefix.trim_end_matches('/');

		for mut route in routes {
			// Prepend the prefix to the route path
			let new_path = if route.path.starts_with('/') {
				format!("{}{}", prefix, route.path)
			} else {
				format!("{}/{}", prefix, route.path)
			};
			route.path = new_path;

			// Set namespace if provided
			if let Some(ref ns) = namespace {
				route.namespace = Some(ns.clone());
			}

			self.add_route(route);
		}
	}

	async fn route(&self, mut request: Request) -> Result<Response> {
		let path = request.path().to_string();

		if let Some((handler_id, params)) = self.matcher.match_path(&path) {
			// Find the route by name or full_name
			let route = self.routes.iter().find(|r| {
				// Check if route name matches
				if let Some(name) = &r.name
					&& name == &handler_id
				{
					return true;
				}
				// Check if full_name matches
				if let Some(full_name) = r.full_name()
					&& full_name == handler_id
				{
					return true;
				}
				false
			});

			if let Some(route) = route {
				// Add path parameters to request
				request.path_params = params;
				return route.handler().handle(request).await;
			}

			// If handler_id is in format "route_N", try to get route by index
			if handler_id.starts_with("route_")
				&& let Ok(index) = handler_id.strip_prefix("route_").unwrap().parse::<usize>()
				&& let Some(route) = self.routes.get(index)
			{
				request.path_params = params;
				return route.handler().handle(request).await;
			}
		}

		Err(reinhardt_core::exception::Error::NotFound(format!(
			"No route found for {}",
			path
		)))
	}
}

#[async_trait]
impl Handler for DefaultRouter {
	async fn handle(&self, request: Request) -> Result<Response> {
		self.route(request).await
	}
}

/// Handler wrapper for ViewSet list actions
#[cfg(feature = "viewsets")]
struct ViewSetListHandler<V> {
	viewset: Arc<V>,
}

#[cfg(feature = "viewsets")]
impl<V> ViewSetListHandler<V> {
	fn new(viewset: Arc<V>) -> Self {
		Self { viewset }
	}
}

#[async_trait]
#[cfg(feature = "viewsets")]
impl<V: ViewSet + 'static> Handler for ViewSetListHandler<V> {
	async fn handle(&self, request: Request) -> Result<Response> {
		process_viewset_request(self.viewset.as_ref(), request, |request| {
			self.viewset
				.dispatch(request, reinhardt_views::viewsets::Action::list())
		})
		.await
	}
}

/// Handler wrapper for ViewSet detail actions
#[cfg(feature = "viewsets")]
struct ViewSetDetailHandler<V> {
	viewset: Arc<V>,
}

#[cfg(feature = "viewsets")]
impl<V> ViewSetDetailHandler<V> {
	fn new(viewset: Arc<V>) -> Self {
		Self { viewset }
	}
}

#[async_trait]
#[cfg(feature = "viewsets")]
impl<V: ViewSet + 'static> Handler for ViewSetDetailHandler<V> {
	async fn handle(&self, request: Request) -> Result<Response> {
		process_viewset_request(self.viewset.as_ref(), request, |request| {
			let action = match request.method.as_str() {
				"GET" => Ok(reinhardt_views::viewsets::Action::retrieve()),
				"PUT" | "PATCH" => Ok(reinhardt_views::viewsets::Action::update()),
				"DELETE" => Ok(reinhardt_views::viewsets::Action::destroy()),
				_ => Err(reinhardt_core::exception::Error::Http(
					"Method not allowed".to_string(),
				)),
			};
			async move {
				let action = action?;
				self.viewset.dispatch(request, action).await
			}
		})
		.await
	}
}

/// Handler wrapper for custom ViewSet actions
#[cfg(feature = "viewsets")]
struct ActionHandlerWrapper<V> {
	viewset: Arc<V>,
	handler: Arc<dyn reinhardt_views::viewsets::ActionHandler>,
	methods: Vec<hyper::Method>,
}

#[cfg(feature = "viewsets")]
impl<V> ActionHandlerWrapper<V> {
	fn new(
		viewset: Arc<V>,
		handler: Arc<dyn reinhardt_views::viewsets::ActionHandler>,
		methods: Vec<hyper::Method>,
	) -> Self {
		Self {
			viewset,
			handler,
			methods,
		}
	}
}

#[async_trait]
#[cfg(feature = "viewsets")]
impl<V: ViewSet + 'static> Handler for ActionHandlerWrapper<V> {
	async fn handle(&self, request: Request) -> Result<Response> {
		process_viewset_request(self.viewset.as_ref(), request, |request| async {
			if !self.methods.contains(&request.method) {
				let mut response = Response::new(hyper::StatusCode::METHOD_NOT_ALLOWED);
				let allow = self
					.methods
					.iter()
					.map(hyper::Method::as_str)
					.collect::<Vec<_>>()
					.join(", ");
				response.headers.insert(
					hyper::header::ALLOW,
					allow
						.parse()
						.expect("HTTP methods form a valid Allow header"),
				);
				return Ok(response);
			}
			self.handler.handle(request).await
		})
		.await
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	#[cfg(feature = "viewsets")]
	use crate::routers::ServerRouter;
	use crate::routers::helpers::path;
	use crate::routers_macros::path as path_macro;
	use async_trait::async_trait;
	#[cfg(feature = "viewsets")]
	use hyper::{HeaderMap, Method, Version};
	use reinhardt_http::{Request, Response, Result};
	#[cfg(feature = "viewsets")]
	use reinhardt_views::viewsets::{
		Action, ActionMetadata, FunctionActionHandler, ViewSet, ViewSetMiddleware,
	};
	#[cfg(feature = "viewsets")]
	use std::sync::atomic::{AtomicUsize, Ordering};

	struct DummyHandler;

	#[async_trait]
	impl Handler for DummyHandler {
		async fn handle(&self, _req: Request) -> Result<Response> {
			Ok(Response::ok())
		}
	}

	#[cfg(feature = "viewsets")]
	#[derive(Clone)]
	struct ProtectedViewSet {
		dispatch_calls: Arc<AtomicUsize>,
		action_calls: Arc<AtomicUsize>,
		login_required: bool,
		required_permissions: Vec<String>,
	}

	#[cfg(feature = "viewsets")]
	#[async_trait]
	impl ViewSet for ProtectedViewSet {
		fn get_basename(&self) -> &str {
			"protected"
		}

		async fn dispatch(&self, _request: Request, _action: Action) -> Result<Response> {
			self.dispatch_calls.fetch_add(1, Ordering::SeqCst);
			Ok(Response::ok())
		}

		fn get_extra_actions(&self) -> Vec<ActionMetadata> {
			let action_calls = self.action_calls.clone();
			vec![
				ActionMetadata::new("extra")
					.with_detail(true)
					.with_methods(vec![Method::GET])
					.with_handler(FunctionActionHandler::new(move |_request| {
						let action_calls = action_calls.clone();
						Box::pin(async move {
							action_calls.fetch_add(1, Ordering::SeqCst);
							Ok(Response::ok())
						})
					})),
			]
		}

		fn requires_login(&self) -> bool {
			self.login_required
		}

		fn get_required_permissions(&self) -> Vec<String> {
			self.required_permissions.clone()
		}
	}

	#[cfg(feature = "viewsets")]
	struct ResponseMiddleware;

	#[cfg(feature = "viewsets")]
	#[async_trait]
	impl ViewSetMiddleware for ResponseMiddleware {
		async fn process_request(&self, _request: &mut Request) -> Result<Option<Response>> {
			Ok(None)
		}

		async fn process_response(
			&self,
			_request: &Request,
			mut response: Response,
		) -> Result<Response> {
			response.headers.insert(
				"x-viewset-response",
				hyper::header::HeaderValue::from_static("processed"),
			);
			Ok(response)
		}
	}

	#[cfg(feature = "viewsets")]
	struct ResponseMiddlewareViewSet;

	#[cfg(feature = "viewsets")]
	#[async_trait]
	impl ViewSet for ResponseMiddlewareViewSet {
		fn get_basename(&self) -> &str {
			"middleware"
		}

		async fn dispatch(&self, _request: Request, _action: Action) -> Result<Response> {
			Ok(Response::ok())
		}

		fn get_extra_actions(&self) -> Vec<ActionMetadata> {
			vec![
				ActionMetadata::new("extra")
					.with_detail(true)
					.with_methods(vec![Method::GET])
					.with_handler(FunctionActionHandler::new(|_request| {
						Box::pin(async { Ok(Response::ok()) })
					})),
			]
		}

		fn get_middleware(&self) -> Option<Arc<dyn ViewSetMiddleware>> {
			Some(Arc::new(ResponseMiddleware))
		}
	}

	#[cfg(feature = "viewsets")]
	fn viewset_request(method: Method, uri: &str) -> Request {
		Request::builder()
			.method(method)
			.uri(uri)
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(bytes::Bytes::new())
			.build()
			.unwrap()
	}

	#[cfg(feature = "viewsets")]
	fn authenticated_viewset_request(method: Method, uri: &str) -> Request {
		let mut request = viewset_request(method, uri);
		request
			.headers
			.insert(hyper::header::AUTHORIZATION, "Bearer test".parse().unwrap());
		request
	}

	#[test]
	fn test_extract_version_from_path() {
		let router = DefaultRouter::new();

		assert_eq!(
			router.extract_version_from_path(path_macro!("/v1/users/"), "/v{version}/"),
			Some("1")
		);
		assert_eq!(
			router.extract_version_from_path(path_macro!("/v2/api/"), "/v{version}/"),
			Some("2")
		);
		assert_eq!(
			router.extract_version_from_path(path_macro!("/users/"), "/v{version}/"),
			None
		);
		assert_eq!(
			router.extract_version_from_path(path_macro!("/api/v1/users/"), "/api/v{version}/"),
			Some("1")
		);
	}

	#[test]
	fn test_find_routes_by_namespace_pattern() {
		let mut router = DefaultRouter::new();
		let handler = std::sync::Arc::new(DummyHandler);

		router.add_route(path(path_macro!("/v1/users/"), handler.clone()).with_namespace("v1"));
		router.add_route(path(path_macro!("/v2/users/"), handler.clone()).with_namespace("v2"));
		router.add_route(path(path_macro!("/users/"), handler).with_namespace("no-version"));

		let v_routes = router.find_routes_by_namespace_pattern("/v{version}/");
		assert_eq!(v_routes.len(), 2);

		let no_version_routes = router.find_routes_by_namespace_pattern(path_macro!("/users/"));
		assert_eq!(no_version_routes.len(), 1);
	}

	#[test]
	fn test_get_available_versions() {
		let mut router = DefaultRouter::new();
		let handler = std::sync::Arc::new(DummyHandler);

		router.add_route(path(path_macro!("/v1/users/"), handler.clone()).with_namespace("v1"));
		router.add_route(path(path_macro!("/v2/users/"), handler.clone()).with_namespace("v2"));
		router.add_route(path(path_macro!("/v1/posts/"), handler).with_namespace("v1"));

		let versions = router.get_available_versions("/v{version}/");
		assert!(versions.contains(&"1".to_string()));
		assert!(versions.contains(&"2".to_string()));
		assert_eq!(versions.len(), 2);
	}

	#[cfg(feature = "viewsets")]
	#[tokio::test]
	async fn default_router_applies_viewset_middleware_before_every_sink() {
		let dispatch_calls = Arc::new(AtomicUsize::new(0));
		let action_calls = Arc::new(AtomicUsize::new(0));
		let viewset = Arc::new(ProtectedViewSet {
			dispatch_calls: dispatch_calls.clone(),
			action_calls: action_calls.clone(),
			login_required: true,
			required_permissions: Vec::new(),
		});
		let mut router = DefaultRouter::new();
		router.register_viewset("protected", viewset);

		for uri in ["/protected/", "/protected/1/", "/protected/1/extra/"] {
			let response = router
				.route(viewset_request(Method::GET, uri))
				.await
				.unwrap();
			assert_eq!(response.status, hyper::StatusCode::UNAUTHORIZED);
		}
		assert_eq!(dispatch_calls.load(Ordering::SeqCst), 0);
		assert_eq!(action_calls.load(Ordering::SeqCst), 0);

		let response = router
			.route(authenticated_viewset_request(Method::GET, "/protected/"))
			.await
			.unwrap();
		assert_eq!(response.status, hyper::StatusCode::OK);
		assert_eq!(dispatch_calls.load(Ordering::SeqCst), 1);
	}

	#[cfg(feature = "viewsets")]
	#[tokio::test]
	async fn default_router_applies_response_middleware_after_every_sink() {
		let mut router = DefaultRouter::new();
		router.register_viewset("middleware", Arc::new(ResponseMiddlewareViewSet));

		for uri in ["/middleware/", "/middleware/1/", "/middleware/1/extra/"] {
			let response = router
				.route(viewset_request(Method::GET, uri))
				.await
				.unwrap();
			assert_eq!(
				response.headers.get("x-viewset-response"),
				Some(&hyper::header::HeaderValue::from_static("processed"))
			);
		}
	}

	#[cfg(feature = "viewsets")]
	#[tokio::test]
	async fn server_router_applies_viewset_middleware_before_dispatch() {
		let dispatch_calls = Arc::new(AtomicUsize::new(0));
		let viewset = ProtectedViewSet {
			dispatch_calls: dispatch_calls.clone(),
			action_calls: Arc::new(AtomicUsize::new(0)),
			login_required: true,
			required_permissions: Vec::new(),
		};
		let router = ServerRouter::new().viewset("protected", viewset);

		for uri in ["/protected/", "/protected/1/"] {
			let response = router
				.handle(viewset_request(Method::GET, uri))
				.await
				.unwrap();
			assert_eq!(response.status, hyper::StatusCode::UNAUTHORIZED);
		}
		assert_eq!(dispatch_calls.load(Ordering::SeqCst), 0);

		let response = router
			.handle(authenticated_viewset_request(Method::GET, "/protected/"))
			.await
			.unwrap();
		assert_eq!(response.status, hyper::StatusCode::OK);
		assert_eq!(dispatch_calls.load(Ordering::SeqCst), 1);
	}

	#[cfg(feature = "viewsets")]
	#[tokio::test]
	async fn default_router_enforces_declared_permissions_before_dispatch() {
		let dispatch_calls = Arc::new(AtomicUsize::new(0));
		let viewset = Arc::new(ProtectedViewSet {
			dispatch_calls: dispatch_calls.clone(),
			action_calls: Arc::new(AtomicUsize::new(0)),
			login_required: false,
			required_permissions: vec!["items.read".to_string()],
		});
		let mut router = DefaultRouter::new();
		router.register_viewset("protected", viewset);

		let response = router
			.route(viewset_request(Method::GET, "/protected/"))
			.await
			.unwrap();

		assert_eq!(response.status, hyper::StatusCode::FORBIDDEN);
		assert_eq!(dispatch_calls.load(Ordering::SeqCst), 0);
	}

	#[cfg(feature = "viewsets")]
	#[tokio::test]
	async fn default_router_enforces_custom_action_methods() {
		let action_calls = Arc::new(AtomicUsize::new(0));
		let viewset = Arc::new(ProtectedViewSet {
			dispatch_calls: Arc::new(AtomicUsize::new(0)),
			action_calls: action_calls.clone(),
			login_required: true,
			required_permissions: Vec::new(),
		});
		let mut router = DefaultRouter::new();
		router.register_viewset("protected", viewset);
		for method in [Method::POST, Method::PATCH, Method::DELETE] {
			let response = router
				.route(authenticated_viewset_request(method, "/protected/1/extra/"))
				.await
				.unwrap();
			assert_eq!(response.status, hyper::StatusCode::METHOD_NOT_ALLOWED);
			assert_eq!(response.headers.get(hyper::header::ALLOW).unwrap(), "GET");
		}
		assert_eq!(action_calls.load(Ordering::SeqCst), 0);

		let response = router
			.route(authenticated_viewset_request(
				Method::GET,
				"/protected/1/extra/",
			))
			.await
			.unwrap();
		assert_eq!(response.status, hyper::StatusCode::OK);
		assert_eq!(action_calls.load(Ordering::SeqCst), 1);
	}
}
