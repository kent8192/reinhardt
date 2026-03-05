use super::{PathMatcher, PathPattern, Route};
use async_trait::async_trait;
use reinhardt_http::{Handler, Request, Response, Result};
use reinhardt_views::viewsets::ViewSet;
use std::collections::HashMap;
use std::sync::Arc;

/// Router trait - composes routes together
pub trait Router: Send + Sync {
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
	/// router.add_route(
	///     path("/users/{id}/", handler)
	///         .with_name("detail")
	///         .with_namespace("users")
	/// );
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
	/// router.add_route(
	///     path("/users/{id}/", handler)
	///         .with_name("detail")
	/// );
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
	pub fn register_viewset<V: ViewSet + 'static>(&mut self, prefix: &str, viewset: Arc<V>) {
		let basename = viewset.get_basename();
		let lookup_field = viewset.get_lookup_field();

		// List/Create endpoint: /prefix/
		let list_path = format!("/{}/", prefix.trim_matches('/'));
		let list_handler = ViewSetListHandler::new(viewset.clone());
		self.add_route(
			Route::new(list_path.clone(), Arc::new(list_handler))
				.with_name(format!("{}-list", basename)),
		);

		// Detail endpoint: /prefix/{lookup_field}/
		let detail_path = format!("/{}/{{{}}}/", prefix.trim_matches('/'), lookup_field);
		let detail_handler = ViewSetDetailHandler::new(viewset.clone());
		self.add_route(
			Route::new(detail_path, Arc::new(detail_handler))
				.with_name(format!("{}-detail", basename)),
		);

		// Register custom actions from ViewSet
		let extra_actions = viewset.get_extra_actions();
		for action in extra_actions {
			let action_url_path = if let Some(ref url_path) = action.url_path {
				url_path.clone()
			} else {
				action.name.clone()
			};

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

			let action_handler = ActionHandlerWrapper::new(action.handler.clone());
			self.add_route(
				Route::new(action_path, Arc::new(action_handler)).with_name(action_url_name),
			);
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

		self.matcher.add_pattern(pattern, handler_id.clone());

		// Register route for reverse lookup if it has a name
		if route.full_name().is_some() || route.name.is_some() {
			self.reverser.register(route.clone());
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
	/// let users_routes = vec![
	///     path("/", handler.clone()).with_name("list"),
	///     path("/{id}/", handler).with_name("detail"),
	/// ];
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
struct ViewSetListHandler<V> {
	viewset: Arc<V>,
}

impl<V> ViewSetListHandler<V> {
	fn new(viewset: Arc<V>) -> Self {
		Self { viewset }
	}
}

#[async_trait]
impl<V: ViewSet + 'static> Handler for ViewSetListHandler<V> {
	async fn handle(&self, request: Request) -> Result<Response> {
		let action = reinhardt_views::viewsets::Action::list();
		self.viewset.dispatch(request, action).await
	}
}

/// Handler wrapper for ViewSet detail actions
struct ViewSetDetailHandler<V> {
	viewset: Arc<V>,
}

impl<V> ViewSetDetailHandler<V> {
	fn new(viewset: Arc<V>) -> Self {
		Self { viewset }
	}
}

#[async_trait]
impl<V: ViewSet + 'static> Handler for ViewSetDetailHandler<V> {
	async fn handle(&self, request: Request) -> Result<Response> {
		// Determine action based on HTTP method
		let action = match request.method.as_str() {
			"GET" => reinhardt_views::viewsets::Action::retrieve(),
			"PUT" | "PATCH" => reinhardt_views::viewsets::Action::update(),
			"DELETE" => reinhardt_views::viewsets::Action::destroy(),
			_ => {
				return Err(reinhardt_core::exception::Error::Http(
					"Method not allowed".to_string(),
				));
			}
		};

		self.viewset.dispatch(request, action).await
	}
}

/// Handler wrapper for custom ViewSet actions
struct ActionHandlerWrapper {
	handler: Arc<dyn reinhardt_views::viewsets::ActionHandler>,
}

impl ActionHandlerWrapper {
	fn new(handler: Arc<dyn reinhardt_views::viewsets::ActionHandler>) -> Self {
		Self { handler }
	}
}

#[async_trait]
impl Handler for ActionHandlerWrapper {
	async fn handle(&self, request: Request) -> Result<Response> {
		self.handler.handle(request).await
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::routers::helpers::path;
	use crate::routers_macros::path as path_macro;
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
}
