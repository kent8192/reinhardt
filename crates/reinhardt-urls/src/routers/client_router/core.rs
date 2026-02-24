//! Core ClientRouter Implementation.
//!
//! This module provides the main ClientRouter struct and routing logic.
//! The router uses `Page` type for all view rendering.

use super::error::RouterError;
use super::handler::{
	RouteHandler, no_params_handler, result_handler, single_path_handler, three_path_handler,
	two_path_handler, with_params_handler,
};
#[cfg(target_arch = "wasm32")]
use super::history::setup_popstate_listener;
use super::history::{HistoryState, NavigationType, current_path, push_state, replace_state};
use super::params::{FromPath, ParamContext, Path, SingleFromPath};
use super::pattern::ClientPathPattern;
use reinhardt_core::page::Page;
use reinhardt_core::reactive::Signal;
use std::collections::HashMap;
use std::sync::Arc;

/// Type alias for route guard functions.
pub(super) type RouteGuard = Arc<dyn Fn(&ClientRouteMatch) -> bool + Send + Sync>;

/// A matched route with extracted parameters.
#[derive(Debug, Clone)]
pub struct ClientRouteMatch {
	/// The matched route.
	pub route: ClientRoute,
	/// Extracted path parameters.
	pub params: HashMap<String, String>,
	/// Parameter values in the order they appear in the pattern.
	///
	/// This guarantees that tuple extraction works correctly by index,
	/// matching the order of parameters in the URL pattern.
	pub(crate) param_values: Vec<String>,
}

/// A single route definition.
pub struct ClientRoute {
	/// The path pattern.
	pattern: ClientPathPattern,
	/// Optional route name for reverse lookups.
	name: Option<String>,
	/// The route handler.
	handler: Arc<dyn RouteHandler>,
	/// Optional guard function.
	guard: Option<RouteGuard>,
}

impl Clone for ClientRoute {
	fn clone(&self) -> Self {
		Self {
			pattern: self.pattern.clone(),
			name: self.name.clone(),
			handler: Arc::clone(&self.handler),
			guard: self.guard.clone(),
		}
	}
}

impl std::fmt::Debug for ClientRoute {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("ClientRoute")
			.field("pattern", &self.pattern)
			.field("name", &self.name)
			.field("has_guard", &self.guard.is_some())
			.finish()
	}
}

impl ClientRoute {
	/// Creates a new route.
	///
	/// # Panics
	///
	/// Panics if the pattern is invalid (exceeds length/segment limits or invalid regex).
	/// Use [`ClientPathPattern::new`] directly for fallible construction.
	pub fn new<F>(pattern: &str, component: F) -> Self
	where
		F: Fn() -> Page + Send + Sync + 'static,
	{
		Self {
			pattern: ClientPathPattern::new(pattern)
				.unwrap_or_else(|e| panic!("Invalid route pattern '{}': {}", pattern, e)),
			name: None,
			handler: no_params_handler(component),
			guard: None,
		}
	}

	/// Creates a named route.
	///
	/// # Panics
	///
	/// Panics if the pattern is invalid (exceeds length/segment limits or invalid regex).
	/// Use [`ClientPathPattern::new`] directly for fallible construction.
	pub fn named<F>(name: impl Into<String>, pattern: &str, component: F) -> Self
	where
		F: Fn() -> Page + Send + Sync + 'static,
	{
		Self {
			pattern: ClientPathPattern::new(pattern)
				.unwrap_or_else(|e| panic!("Invalid route pattern '{}': {}", pattern, e)),
			name: Some(name.into()),
			handler: no_params_handler(component),
			guard: None,
		}
	}

	/// Adds a guard to this route.
	pub fn with_guard<G>(mut self, guard: G) -> Self
	where
		G: Fn(&ClientRouteMatch) -> bool + Send + Sync + 'static,
	{
		self.guard = Some(Arc::new(guard));
		self
	}

	/// Returns the route name.
	pub fn name(&self) -> Option<&str> {
		self.name.as_deref()
	}

	/// Returns the pattern.
	pub fn pattern(&self) -> &ClientPathPattern {
		&self.pattern
	}

	/// Checks if the guard allows access.
	pub fn check_guard(&self, route_match: &ClientRouteMatch) -> bool {
		self.guard.as_ref().map(|g| g(route_match)).unwrap_or(true)
	}
}

/// The main client-side router.
///
/// `ClientRouter` renders views using the [`Page`] type.
pub struct ClientRouter {
	/// Registered routes.
	routes: Vec<ClientRoute>,
	/// Named routes for reverse lookups.
	named_routes: HashMap<String, usize>,
	/// Current path signal.
	current_path: Signal<String>,
	/// Current params signal.
	current_params: Signal<HashMap<String, String>>,
	/// Current matched route name signal.
	current_route_name: Signal<Option<String>>,
	/// Not found handler.
	not_found: Option<Arc<dyn Fn() -> Page + Send + Sync>>,
}

impl std::fmt::Debug for ClientRouter {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("ClientRouter")
			.field("routes_count", &self.routes.len())
			.field(
				"named_routes",
				&self.named_routes.keys().collect::<Vec<_>>(),
			)
			.finish()
	}
}

impl Default for ClientRouter {
	fn default() -> Self {
		Self::new()
	}
}

impl ClientRouter {
	/// Creates a new router.
	pub fn new() -> Self {
		let initial_path = current_path().unwrap_or_else(|_| "/".to_string());

		Self {
			routes: Vec::new(),
			named_routes: HashMap::new(),
			current_path: Signal::new(initial_path),
			current_params: Signal::new(HashMap::new()),
			current_route_name: Signal::new(None),
			not_found: None,
		}
	}

	/// Adds a route to the router.
	pub fn route<F>(mut self, pattern: &str, component: F) -> Self
	where
		F: Fn() -> Page + Send + Sync + 'static,
	{
		self.routes.push(ClientRoute::new(pattern, component));
		self
	}

	/// Adds a named route to the router.
	pub fn named_route<F>(mut self, name: &str, pattern: &str, component: F) -> Self
	where
		F: Fn() -> Page + Send + Sync + 'static,
	{
		let index = self.routes.len();
		self.routes
			.push(ClientRoute::named(name, pattern, component));
		self.named_routes.insert(name.to_string(), index);
		self
	}

	/// Adds a route with typed path parameters.
	pub fn route_params<F, T>(mut self, pattern: &str, handler: F) -> Self
	where
		F: Fn(Path<T>) -> Page + Send + Sync + 'static,
		T: FromPath + Send + Sync + 'static,
	{
		self.routes.push(ClientRoute {
			pattern: ClientPathPattern::new(pattern)
				.unwrap_or_else(|e| panic!("Invalid route pattern '{}': {}", pattern, e)),
			name: None,
			handler: with_params_handler(handler),
			guard: None,
		});
		self
	}

	/// Adds a named route with typed path parameters.
	pub fn named_route_params<F, T>(mut self, name: &str, pattern: &str, handler: F) -> Self
	where
		F: Fn(Path<T>) -> Page + Send + Sync + 'static,
		T: FromPath + Send + Sync + 'static,
	{
		let index = self.routes.len();
		self.routes.push(ClientRoute {
			pattern: ClientPathPattern::new(pattern)
				.unwrap_or_else(|e| panic!("Invalid route pattern '{}': {}", pattern, e)),
			name: Some(name.to_string()),
			handler: with_params_handler(handler),
			guard: None,
		});
		self.named_routes.insert(name.to_string(), index);
		self
	}

	/// Adds a route with typed path parameters that returns a Result.
	pub fn route_result<F, T, E>(mut self, pattern: &str, handler: F) -> Self
	where
		F: Fn(Path<T>) -> Result<Page, E> + Send + Sync + 'static,
		T: FromPath + Send + Sync + 'static,
		E: Into<RouterError> + Send + Sync + 'static,
	{
		self.routes.push(ClientRoute {
			pattern: ClientPathPattern::new(pattern)
				.unwrap_or_else(|e| panic!("Invalid route pattern '{}': {}", pattern, e)),
			name: None,
			handler: result_handler(handler),
			guard: None,
		});
		self
	}

	/// Adds a named route with typed path parameters that returns a Result.
	pub fn named_route_result<F, T, E>(mut self, name: &str, pattern: &str, handler: F) -> Self
	where
		F: Fn(Path<T>) -> Result<Page, E> + Send + Sync + 'static,
		T: FromPath + Send + Sync + 'static,
		E: Into<RouterError> + Send + Sync + 'static,
	{
		let index = self.routes.len();
		self.routes.push(ClientRoute {
			pattern: ClientPathPattern::new(pattern)
				.unwrap_or_else(|e| panic!("Invalid route pattern '{}': {}", pattern, e)),
			name: Some(name.to_string()),
			handler: result_handler(handler),
			guard: None,
		});
		self.named_routes.insert(name.to_string(), index);
		self
	}

	/// Adds a route with a guard.
	pub fn guarded_route<F, G>(mut self, pattern: &str, component: F, guard: G) -> Self
	where
		F: Fn() -> Page + Send + Sync + 'static,
		G: Fn(&ClientRouteMatch) -> bool + Send + Sync + 'static,
	{
		self.routes
			.push(ClientRoute::new(pattern, component).with_guard(guard));
		self
	}

	/// Adds a route with a single path parameter using `Path<T>` extractor.
	///
	/// # Example
	///
	/// ```ignore
	/// let router = ClientRouter::new()
	///     .route_path("/users/{id}/", |Path(id): Path<i64>| {
	///         user_detail(id)
	///     });
	/// ```
	pub fn route_path<F, T>(mut self, pattern: &str, handler: F) -> Self
	where
		F: Fn(Path<T>) -> Page + Send + Sync + 'static,
		T: SingleFromPath + Send + Sync + 'static,
	{
		self.routes.push(ClientRoute {
			pattern: ClientPathPattern::new(pattern)
				.unwrap_or_else(|e| panic!("Invalid route pattern '{}': {}", pattern, e)),
			name: None,
			handler: single_path_handler(handler),
			guard: None,
		});
		self
	}

	/// Adds a named route with a single path parameter using `Path<T>` extractor.
	pub fn named_route_path<F, T>(mut self, name: &str, pattern: &str, handler: F) -> Self
	where
		F: Fn(Path<T>) -> Page + Send + Sync + 'static,
		T: SingleFromPath + Send + Sync + 'static,
	{
		let index = self.routes.len();
		self.routes.push(ClientRoute {
			pattern: ClientPathPattern::new(pattern)
				.unwrap_or_else(|e| panic!("Invalid route pattern '{}': {}", pattern, e)),
			name: Some(name.to_string()),
			handler: single_path_handler(handler),
			guard: None,
		});
		self.named_routes.insert(name.to_string(), index);
		self
	}

	/// Adds a route with two path parameters using multiple `Path<T>` extractors.
	///
	/// # Example
	///
	/// ```ignore
	/// let router = ClientRouter::new()
	///     .route_path2("/users/{user_id}/posts/{post_id}/",
	///         |Path(user_id): Path<i64>, Path(post_id): Path<i64>| {
	///             user_post_detail(user_id, post_id)
	///         });
	/// ```
	pub fn route_path2<F, T1, T2>(mut self, pattern: &str, handler: F) -> Self
	where
		F: Fn(Path<T1>, Path<T2>) -> Page + Send + Sync + 'static,
		T1: SingleFromPath + Send + Sync + 'static,
		T2: SingleFromPath + Send + Sync + 'static,
	{
		self.routes.push(ClientRoute {
			pattern: ClientPathPattern::new(pattern)
				.unwrap_or_else(|e| panic!("Invalid route pattern '{}': {}", pattern, e)),
			name: None,
			handler: two_path_handler(handler),
			guard: None,
		});
		self
	}

	/// Adds a named route with two path parameters.
	pub fn named_route_path2<F, T1, T2>(mut self, name: &str, pattern: &str, handler: F) -> Self
	where
		F: Fn(Path<T1>, Path<T2>) -> Page + Send + Sync + 'static,
		T1: SingleFromPath + Send + Sync + 'static,
		T2: SingleFromPath + Send + Sync + 'static,
	{
		let index = self.routes.len();
		self.routes.push(ClientRoute {
			pattern: ClientPathPattern::new(pattern)
				.unwrap_or_else(|e| panic!("Invalid route pattern '{}': {}", pattern, e)),
			name: Some(name.to_string()),
			handler: two_path_handler(handler),
			guard: None,
		});
		self.named_routes.insert(name.to_string(), index);
		self
	}

	/// Adds a route with three path parameters using multiple `Path<T>` extractors.
	///
	/// # Example
	///
	/// ```ignore
	/// let router = ClientRouter::new()
	///     .route_path3("/org/{org}/repos/{repo}/issues/{issue}/",
	///         |Path(org): Path<String>, Path(repo): Path<String>, Path(issue): Path<i32>| {
	///             issue_detail(org, repo, issue)
	///         });
	/// ```
	pub fn route_path3<F, T1, T2, T3>(mut self, pattern: &str, handler: F) -> Self
	where
		F: Fn(Path<T1>, Path<T2>, Path<T3>) -> Page + Send + Sync + 'static,
		T1: SingleFromPath + Send + Sync + 'static,
		T2: SingleFromPath + Send + Sync + 'static,
		T3: SingleFromPath + Send + Sync + 'static,
	{
		self.routes.push(ClientRoute {
			pattern: ClientPathPattern::new(pattern)
				.unwrap_or_else(|e| panic!("Invalid route pattern '{}': {}", pattern, e)),
			name: None,
			handler: three_path_handler(handler),
			guard: None,
		});
		self
	}

	/// Adds a named route with three path parameters.
	pub fn named_route_path3<F, T1, T2, T3>(mut self, name: &str, pattern: &str, handler: F) -> Self
	where
		F: Fn(Path<T1>, Path<T2>, Path<T3>) -> Page + Send + Sync + 'static,
		T1: SingleFromPath + Send + Sync + 'static,
		T2: SingleFromPath + Send + Sync + 'static,
		T3: SingleFromPath + Send + Sync + 'static,
	{
		let index = self.routes.len();
		self.routes.push(ClientRoute {
			pattern: ClientPathPattern::new(pattern)
				.unwrap_or_else(|e| panic!("Invalid route pattern '{}': {}", pattern, e)),
			name: Some(name.to_string()),
			handler: three_path_handler(handler),
			guard: None,
		});
		self.named_routes.insert(name.to_string(), index);
		self
	}

	/// Sets the not found handler.
	pub fn not_found<F>(mut self, component: F) -> Self
	where
		F: Fn() -> Page + Send + Sync + 'static,
	{
		self.not_found = Some(Arc::new(component));
		self
	}

	/// Returns the current path signal.
	pub fn current_path(&self) -> &Signal<String> {
		&self.current_path
	}

	/// Returns the current params signal.
	pub fn current_params(&self) -> &Signal<HashMap<String, String>> {
		&self.current_params
	}

	/// Returns the current route name signal.
	pub fn current_route_name(&self) -> &Signal<Option<String>> {
		&self.current_route_name
	}

	/// Matches a path against registered routes.
	pub fn match_path(&self, path: &str) -> Option<ClientRouteMatch> {
		for route in &self.routes {
			if let Some((params, param_values)) = route.pattern.matches(path) {
				let route_match = ClientRouteMatch {
					route: route.clone(),
					params,
					param_values,
				};

				// Check guard if present
				if route.check_guard(&route_match) {
					return Some(route_match);
				}
			}
		}
		None
	}

	/// Navigates to a path using pushState.
	pub fn push(&self, path: &str) -> Result<(), RouterError> {
		self.navigate(path, NavigationType::Push)
	}

	/// Navigates to a path using replaceState.
	pub fn replace(&self, path: &str) -> Result<(), RouterError> {
		self.navigate(path, NavigationType::Replace)
	}

	/// Internal navigation implementation.
	fn navigate(&self, path: &str, nav_type: NavigationType) -> Result<(), RouterError> {
		let route_match = self.match_path(path);

		let state = HistoryState::new(path)
			.with_params(
				route_match
					.as_ref()
					.map(|m| m.params.clone())
					.unwrap_or_default(),
			)
			.with_route_name(
				route_match
					.as_ref()
					.and_then(|m| m.route.name())
					.unwrap_or(""),
			);

		let result = match nav_type {
			NavigationType::Push => push_state(&state),
			NavigationType::Replace => replace_state(&state),
			_ => Ok(()),
		};

		result.map_err(RouterError::NavigationFailed)?;

		// Update reactive signals
		self.current_path.set(path.to_string());
		self.current_params.set(
			route_match
				.as_ref()
				.map(|m| m.params.clone())
				.unwrap_or_default(),
		);
		self.current_route_name.set(
			route_match
				.as_ref()
				.and_then(|m| m.route.name().map(|s| s.to_string())),
		);

		Ok(())
	}

	/// Generates a URL by route name with parameters.
	pub fn reverse(&self, name: &str, params: &[(&str, &str)]) -> Result<String, RouterError> {
		let index = self
			.named_routes
			.get(name)
			.ok_or_else(|| RouterError::InvalidRouteName(name.to_string()))?;

		let route = &self.routes[*index];
		let params_map: HashMap<String, String> = params
			.iter()
			.map(|(k, v)| (k.to_string(), v.to_string()))
			.collect();

		route
			.pattern
			.reverse(&params_map)
			.ok_or_else(|| RouterError::MissingParameter("unknown".to_string()))
	}

	/// Renders the current route's component.
	///
	/// This method renders the view for the current path. If no route matches,
	/// it returns `None` if no not_found handler is set.
	pub fn render_current(&self) -> Option<Page> {
		let path = self.current_path.get();

		if let Some(route_match) = self.match_path(&path) {
			let ctx =
				ParamContext::new(route_match.params.clone(), route_match.param_values.clone());

			match route_match.route.handler.handle(&ctx) {
				Ok(view) => Some(view),
				Err(_err) => {
					// Return not_found on error
					self.not_found.as_ref().map(|f| f())
				}
			}
		} else {
			self.not_found.as_ref().map(|f| f())
		}
	}

	/// Returns the number of registered routes.
	pub fn route_count(&self) -> usize {
		self.routes.len()
	}

	/// Checks if a route name exists.
	pub fn has_route(&self, name: &str) -> bool {
		self.named_routes.contains_key(name)
	}

	/// Sets up a popstate event listener for browser back/forward navigation.
	///
	/// This method registers a listener for the browser's `popstate` event,
	/// which fires when the user navigates using the back/forward buttons.
	/// When triggered, it updates the router's reactive signals to reflect
	/// the new URL state.
	///
	/// # WASM Only
	///
	/// This method only has effect on WASM targets. On non-WASM targets,
	/// it's a no-op that always returns `Ok(())`.
	///
	/// # Note
	///
	/// The listener closure is kept alive using `.forget()`, meaning it will
	/// persist for the lifetime of the page. This is intentional for SPA
	/// navigation handling.
	#[cfg(target_arch = "wasm32")]
	pub fn setup_history_listener(&self) {
		let path_signal = self.current_path.clone();
		let params_signal = self.current_params.clone();
		let route_name_signal = self.current_route_name.clone();

		let closure = setup_popstate_listener(move |path, state| {
			// Update path signal
			path_signal.set(path);

			// Update params and route name from history state if available
			if let Some(hist_state) = state {
				params_signal.set(hist_state.params);
				route_name_signal.set(hist_state.route_name);
			} else {
				// Clear params when no state is available
				params_signal.set(HashMap::new());
				route_name_signal.set(None);
			}
		});

		if let Ok(c) = closure {
			// Keep the closure alive for the lifetime of the page
			c.forget();
		}
	}

	/// Non-WASM version of `setup_history_listener`.
	#[cfg(not(target_arch = "wasm32"))]
	pub fn setup_history_listener(&self) {
		// No-op on non-WASM targets
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	fn test_page() -> Page {
		Page::Empty
	}

	fn page_with_text(s: &str) -> Page {
		Page::Text(s.to_string().into())
	}

	fn home_page() -> Page {
		page_with_text("Home")
	}

	fn user_page() -> Page {
		page_with_text("User")
	}

	fn not_found_page() -> Page {
		page_with_text("NotFound")
	}

	#[test]
	fn test_route_new() {
		let route = ClientRoute::new("/", test_page);
		assert!(route.name().is_none());
	}

	#[test]
	fn test_route_named() {
		let route = ClientRoute::named("home", "/", test_page);
		assert_eq!(route.name(), Some("home"));
	}

	#[test]
	fn test_router_new() {
		let router = ClientRouter::new();
		assert_eq!(router.route_count(), 0);
	}

	#[test]
	fn test_router_add_route() {
		let router = ClientRouter::new()
			.route("/", home_page)
			.route("/users/", user_page);

		assert_eq!(router.route_count(), 2);
	}

	#[test]
	fn test_router_named_route() {
		let router = ClientRouter::new()
			.named_route("home", "/", home_page)
			.named_route("users", "/users/", user_page);

		assert!(router.has_route("home"));
		assert!(router.has_route("users"));
		assert!(!router.has_route("nonexistent"));
	}

	#[test]
	fn test_router_match_exact() {
		let router = ClientRouter::new()
			.route("/", home_page)
			.route("/users/", user_page);

		assert!(router.match_path("/").is_some());
		assert!(router.match_path("/users/").is_some());
		assert!(router.match_path("/nonexistent/").is_none());
	}

	#[test]
	fn test_router_match_params() {
		let router = ClientRouter::new().route("/users/{id}/", user_page);

		let route_match = router.match_path("/users/42/");
		assert!(route_match.is_some());

		let route_match = route_match.unwrap();
		assert_eq!(route_match.params.get("id"), Some(&"42".to_string()));
	}

	#[test]
	fn test_router_reverse() {
		let router = ClientRouter::new()
			.named_route("home", "/", home_page)
			.named_route("user_detail", "/users/{id}/", user_page);

		assert_eq!(router.reverse("home", &[]).unwrap(), "/");
		assert_eq!(
			router.reverse("user_detail", &[("id", "42")]).unwrap(),
			"/users/42/"
		);
	}

	#[test]
	fn test_router_reverse_invalid_name() {
		let router = ClientRouter::new();
		let result = router.reverse("nonexistent", &[]);
		assert!(matches!(result, Err(RouterError::InvalidRouteName(_))));
	}

	#[test]
	fn test_router_not_found() {
		let router = ClientRouter::new().not_found(not_found_page);

		let view = router.render_current();
		assert!(view.is_some());
	}

	#[test]
	fn test_router_with_guard() {
		let router = ClientRouter::new()
			.guarded_route("/admin/", test_page, |_| false)
			.route("/public/", test_page);

		// Guard rejects
		assert!(router.match_path("/admin/").is_none());
		// No guard
		assert!(router.match_path("/public/").is_some());
	}

	#[test]
	fn test_router_error_display() {
		assert_eq!(
			RouterError::NotFound("/test/".to_string()).to_string(),
			"Route not found: /test/"
		);
		assert_eq!(
			RouterError::InvalidRouteName("test".to_string()).to_string(),
			"Invalid route name: test"
		);
	}

	#[test]
	fn test_router_push_non_wasm() {
		let router = ClientRouter::new()
			.route("/", home_page)
			.route("/users/", user_page);

		// Non-WASM push should succeed
		assert!(router.push("/users/").is_ok());
	}

	#[test]
	fn test_router_replace_non_wasm() {
		let router = ClientRouter::new().route("/", home_page);

		// Non-WASM replace should succeed
		assert!(router.replace("/").is_ok());
	}

	// ============================================================================
	// route_path* tests
	// ============================================================================

	#[test]
	fn test_route_path_single() {
		let router = ClientRouter::new().route_path("/users/{id}/", |Path(_id): Path<i64>| {
			page_with_text("User")
		});

		assert_eq!(router.route_count(), 1);

		// Match and verify handler works
		let route_match = router.match_path("/users/42/");
		assert!(route_match.is_some());

		let route_match = route_match.unwrap();
		assert_eq!(route_match.params.get("id"), Some(&"42".to_string()));
	}

	#[test]
	fn test_route_path2_two_params() {
		let router = ClientRouter::new().route_path2(
			"/users/{user_id}/posts/{post_id}/",
			|Path(_user_id): Path<i64>, Path(_post_id): Path<i64>| page_with_text("UserPost"),
		);

		assert_eq!(router.route_count(), 1);

		let route_match = router.match_path("/users/123/posts/456/");
		assert!(route_match.is_some());

		let route_match = route_match.unwrap();
		assert_eq!(route_match.params.get("user_id"), Some(&"123".to_string()));
		assert_eq!(route_match.params.get("post_id"), Some(&"456".to_string()));
	}

	#[test]
	fn test_route_path3_three_params() {
		let router = ClientRouter::new().route_path3(
			"/orgs/{org_id}/teams/{team_id}/members/{member_id}/",
			|Path(_org_id): Path<String>,
			 Path(_team_id): Path<i64>,
			 Path(_member_id): Path<i64>| page_with_text("Member"),
		);

		assert_eq!(router.route_count(), 1);

		let route_match = router.match_path("/orgs/acme/teams/10/members/100/");
		assert!(route_match.is_some());

		let route_match = route_match.unwrap();
		assert_eq!(route_match.params.get("org_id"), Some(&"acme".to_string()));
		assert_eq!(route_match.params.get("team_id"), Some(&"10".to_string()));
		assert_eq!(
			route_match.params.get("member_id"),
			Some(&"100".to_string())
		);
	}

	#[test]
	fn test_named_route_path() {
		let router = ClientRouter::new().named_route_path(
			"user_detail",
			"/users/{id}/",
			|Path(_id): Path<i64>| page_with_text("User"),
		);

		assert!(router.has_route("user_detail"));
		assert_eq!(
			router.reverse("user_detail", &[("id", "42")]).unwrap(),
			"/users/42/"
		);
	}

	#[test]
	fn test_named_route_path2() {
		let router = ClientRouter::new().named_route_path2(
			"user_post",
			"/users/{user_id}/posts/{post_id}/",
			|Path(_user_id): Path<i64>, Path(_post_id): Path<i64>| page_with_text("UserPost"),
		);

		assert!(router.has_route("user_post"));
		assert_eq!(
			router
				.reverse("user_post", &[("user_id", "10"), ("post_id", "20")])
				.unwrap(),
			"/users/10/posts/20/"
		);
	}

	#[test]
	fn test_named_route_path3() {
		let router = ClientRouter::new().named_route_path3(
			"org_team_member",
			"/orgs/{org}/teams/{team}/members/{member}/",
			|Path(_org): Path<String>, Path(_team): Path<i64>, Path(_member): Path<i64>| {
				page_with_text("Member")
			},
		);

		assert!(router.has_route("org_team_member"));
		assert_eq!(
			router
				.reverse(
					"org_team_member",
					&[("org", "acme"), ("team", "5"), ("member", "42")]
				)
				.unwrap(),
			"/orgs/acme/teams/5/members/42/"
		);
	}

	#[test]
	fn test_route_path_with_string_param() {
		let router = ClientRouter::new()
			.route_path("/posts/{slug}/", |Path(_slug): Path<String>| {
				page_with_text("Post")
			});

		let route_match = router.match_path("/posts/hello-world/");
		assert!(route_match.is_some());
		assert_eq!(
			route_match.unwrap().params.get("slug"),
			Some(&"hello-world".to_string())
		);
	}
}
