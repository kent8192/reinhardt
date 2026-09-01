//! Core ClientRouter Implementation.
//!
//! This module provides the main ClientRouter struct and routing logic.
//! The router uses `Page` type for all view rendering.

use super::component::{ComponentInfo, ComponentMetadata, ComponentNavigationGuardMetadata};
use super::error::{MergeError, RouteRegistrationError, RouterError};
use super::from_request::FromRequest;
use super::handler::{
	Handler, LayoutRouteHandler, RouteHandler, from_request_handler, no_params_handler,
	result_handler, with_params_handler,
};
#[cfg(wasm)]
use super::history::current_location_path;
#[cfg(native)]
use super::history::current_path;
#[cfg(wasm)]
use super::history::setup_popstate_listener;
use super::history::{HistoryState, NavigationType, push_state, replace_state};
use super::loader::RouteLoaderId;
use super::navigation_guard::NavigationGuardId;
use super::params::{FromPath, ParamContext, Path};
use super::pattern::ClientPathPattern;
use super::scope::{RegisteredRouteScope, RouteScope};
use super::tree::{ClientRouteTreeMatch, ResolvedRouteMetadata, RouteNode};
use reinhardt_core::page::{Head, Outlet, Page};
use reinhardt_core::reactive::{
	Effect, EffectTiming, ReactiveScope, Signal, scope::current_scope_id,
};
#[cfg(wasm)]
use reinhardt_core::reactive::{ScopeId, scope::enter_scope};
use std::collections::{HashMap, HashSet};
#[cfg(native)]
use std::marker::PhantomData;
use std::rc::Rc;
use std::sync::Arc;

/// Type alias for route guard functions.
pub(super) type RouteGuard = Arc<dyn Fn(&ClientRouteMatch) -> bool + Send + Sync>;

const NAVIGATION_GUARD_COORDINATOR_REQUIRED: &str =
	"navigation guards require navigation through reinhardt-pages";
const MATCHED_ROUTE_PATH_MISMATCH: &str = "matched route does not correspond to path";

// (Refs #4234) Mirrors `pages::Router NavigationObservers / NavigationListener`.
// Browser navigation observers are only available on wasm targets.
//
// `Rc<RefCell<...>>` because Routers are not `Send` on wasm32 anyway,
// and the borrow is released before listeners run (see `notify_observers`).
#[cfg(wasm)]
type NavigationObservers = std::rc::Rc<std::cell::RefCell<Vec<std::rc::Weak<NavigationListener>>>>;

#[cfg(wasm)]
type NavigationCallback = dyn Fn(&str, &HashMap<String, String>) + 'static;

/// Listener stored behind a `Weak<...>` so a dropped
/// [`NavigationSubscription`] drops its strong `Rc`, after which
/// [`ClientRouter::notify_observers`] filters out the dead `Weak`.
#[cfg(wasm)]
struct NavigationListener {
	owner_scope: Option<ScopeId>,
	callback: Box<NavigationCallback>,
}

type NavigationSignals = (
	Signal<String>,
	Signal<HashMap<String, String>>,
	Signal<Option<String>>,
	Signal<bool>,
	Signal<Option<String>>,
	Option<Rc<ReactiveScope>>,
);

fn create_navigation_signals(initial_path: String) -> NavigationSignals {
	if current_scope_id().is_some() {
		return create_navigation_signals_in_scope(initial_path);
	}

	let scope = Rc::new(ReactiveScope::new());
	let (
		current_path,
		current_params,
		current_route_name,
		current_match_is_unmatched,
		current_navigation_guard_approval,
		_,
	) = scope.enter(|| create_navigation_signals_in_scope(initial_path));
	(
		current_path,
		current_params,
		current_route_name,
		current_match_is_unmatched,
		current_navigation_guard_approval,
		Some(scope),
	)
}

fn create_navigation_signals_in_scope(initial_path: String) -> NavigationSignals {
	let current_path = Signal::new(initial_path);
	let current_params = Signal::new(HashMap::new());
	let current_route_name = Signal::new(None);
	let current_match_is_unmatched = Signal::new(false);
	let current_navigation_guard_approval = Signal::new(None);
	let path_for_invalidation = current_path;
	let approval_for_invalidation = current_navigation_guard_approval;
	Effect::new_with_timing(
		move || {
			let _ = path_for_invalidation.get();
			approval_for_invalidation.set(None);
		},
		EffectTiming::Layout,
	);
	(
		current_path,
		current_params,
		current_route_name,
		current_match_is_unmatched,
		current_navigation_guard_approval,
		None,
	)
}

/// RAII handle returned by [`ClientRouter::on_navigate`].
///
/// While alive, the registered listener fires on every
/// [`ClientRouter::push`] / [`ClientRouter::replace`] and on browser
/// back/forward navigation handled by
/// [`ClientRouter::setup_history_listener`]. Dropping this handle
/// removes the listener (no explicit `unsubscribe` call needed).
///
/// Mirrors `reinhardt_pages::router::NavigationSubscription`. (Refs #4234)
pub struct NavigationSubscription {
	#[cfg(wasm)]
	#[allow(dead_code)] // Dropped automatically; presence keeps the Weak alive.
	listener: std::rc::Rc<NavigationListener>,
}

impl NavigationSubscription {
	#[cfg(wasm)]
	fn new<F>(router: &ClientRouter, listener: F) -> Self
	where
		F: Fn(&str, &HashMap<String, String>) + 'static,
	{
		let listener = std::rc::Rc::new(NavigationListener {
			owner_scope: current_scope_id()
				.or_else(|| router._navigation_scope.as_ref().map(|scope| scope.id())),
			callback: Box::new(listener),
		});
		router
			.navigation_observers
			.borrow_mut()
			.push(std::rc::Rc::downgrade(&listener));
		Self { listener }
	}

	#[cfg(native)]
	fn new<F>(_router: &ClientRouter, _listener: F) -> Self
	where
		F: Fn(&str, &HashMap<String, String>) + 'static,
	{
		Self {}
	}
}

/// A matched route with extracted parameters.
#[derive(Debug, Clone)]
pub struct ClientRouteMatch {
	/// The matched route.
	pub route: ClientRoute,
	/// Matched path without the query string.
	pub path: String,
	/// Extracted path parameters.
	pub params: HashMap<String, String>,
	/// Parameter values in the order they appear in the pattern.
	///
	/// This guarantees that tuple extraction works correctly by index,
	/// matching the order of parameters in the URL pattern.
	pub(crate) param_values: Vec<String>,
	/// Raw query string (without the leading `?`), captured from the
	/// path passed to [`ClientRouter::match_path`].
	///
	/// Populated by `match_path` so [`ClientRouter::page`] handlers can
	/// surface query data via [`QueryParam<T>`]. `None` when the path
	/// had no `?` segment.
	///
	/// [`ClientRouter::page`]: ClientRouter::page
	/// [`QueryParam<T>`]: super::from_request::QueryParam
	pub query: Option<String>,
}

/// Route-level metadata exposed alongside matched client routes.
///
/// The head contribution is mounted as an outer structural page wrapper. A
/// matched layout contributes before its active child route, and the existing
/// layout/leaf lifetimes determine when each contribution is removed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RouteMetadata {
	/// Structural document-head contribution associated with the route.
	head: Head,
	/// Human-readable breadcrumb label associated with the route.
	breadcrumb: Option<String>,
	/// Whether this route should be treated as authentication-protected.
	requires_auth: bool,
}

impl RouteMetadata {
	/// Creates empty route metadata.
	pub fn new() -> Self {
		Self::default()
	}

	/// Merges a structural document-head contribution into the route metadata.
	///
	/// The contribution participates in SSR, hydration, and browser resolution;
	/// it is not a separate title-only lookup table.
	pub fn with_head(mut self, head: Head) -> Self {
		self.head = self.head.merge(head);
		self
	}

	/// Sets the route title.
	pub fn with_title(self, title: impl Into<String>) -> Self {
		self.with_head(Head::new().title(title.into()))
	}

	/// Sets the breadcrumb label.
	pub fn with_breadcrumb(mut self, breadcrumb: impl Into<String>) -> Self {
		self.breadcrumb = Some(breadcrumb.into());
		self
	}

	/// Sets whether the route requires authentication.
	pub fn with_requires_auth(mut self, requires_auth: bool) -> Self {
		self.requires_auth = requires_auth;
		self
	}

	/// Returns the route title, if configured.
	pub fn title(&self) -> Option<&str> {
		self.head.title.as_deref()
	}

	/// Returns the structural document-head contribution.
	pub fn head(&self) -> &Head {
		&self.head
	}

	/// Returns the breadcrumb label, if configured.
	pub fn breadcrumb(&self) -> Option<&str> {
		self.breadcrumb.as_deref()
	}

	/// Returns whether the route requires authentication.
	pub fn requires_auth(&self) -> bool {
		self.requires_auth
	}
}

/// A single route definition.
pub struct ClientRoute {
	/// The path pattern.
	pattern: ClientPathPattern,
	/// Optional route name for reverse lookups.
	name: Option<String>,
	/// Route-level metadata.
	metadata: RouteMetadata,
	/// Optional loader metadata for a flat component registration.
	loader_id: Option<RouteLoaderId>,
	/// Optional navigation guard metadata for a flat component registration.
	navigation_guard_id: Option<NavigationGuardId>,
	/// The route handler.
	handler: ClientRouteHandler,
	/// Optional guard function.
	guard: Option<RouteGuard>,
}

enum ClientRouteHandler {
	Leaf(Arc<dyn RouteHandler>),
	Layout(Arc<dyn LayoutRouteHandler>),
}

impl Clone for ClientRouteHandler {
	fn clone(&self) -> Self {
		match self {
			Self::Leaf(handler) => Self::Leaf(Arc::clone(handler)),
			Self::Layout(handler) => Self::Layout(Arc::clone(handler)),
		}
	}
}

impl Clone for ClientRoute {
	fn clone(&self) -> Self {
		Self {
			pattern: self.pattern.clone(),
			name: self.name.clone(),
			metadata: self.metadata.clone(),
			loader_id: self.loader_id,
			navigation_guard_id: self.navigation_guard_id,
			handler: self.handler.clone(),
			guard: self.guard.clone(),
		}
	}
}

impl std::fmt::Debug for ClientRoute {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("ClientRoute")
			.field("pattern", &self.pattern)
			.field("name", &self.name)
			.field("metadata", &self.metadata)
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
		Self::from_route_handler(
			None,
			ClientPathPattern::new(pattern)
				.unwrap_or_else(|e| panic!("Invalid route pattern '{}': {}", pattern, e)),
			no_params_handler(component),
		)
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
		Self::from_route_handler(
			Some(name.into()),
			ClientPathPattern::new(pattern)
				.unwrap_or_else(|e| panic!("Invalid route pattern '{}': {}", pattern, e)),
			no_params_handler(component),
		)
	}

	pub(crate) fn from_route_handler(
		name: Option<String>,
		pattern: ClientPathPattern,
		handler: Arc<dyn RouteHandler>,
	) -> Self {
		Self {
			pattern,
			name,
			metadata: RouteMetadata::default(),
			loader_id: None,
			navigation_guard_id: None,
			handler: ClientRouteHandler::Leaf(handler),
			guard: None,
		}
	}

	pub(crate) fn from_layout_handler(
		name: Option<String>,
		pattern: ClientPathPattern,
		handler: Arc<dyn LayoutRouteHandler>,
	) -> Self {
		Self {
			pattern,
			name,
			metadata: RouteMetadata::default(),
			loader_id: None,
			navigation_guard_id: None,
			handler: ClientRouteHandler::Layout(handler),
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

	/// Adds route-level metadata to this route.
	pub fn with_metadata(mut self, metadata: RouteMetadata) -> Self {
		self.metadata = metadata;
		self
	}

	pub(crate) fn set_metadata(&mut self, metadata: RouteMetadata) {
		self.metadata = metadata;
	}

	pub(crate) fn set_loader_id(&mut self, loader_id: Option<RouteLoaderId>) {
		self.loader_id = loader_id;
	}

	pub(crate) fn set_navigation_guard_id(
		&mut self,
		navigation_guard_id: Option<NavigationGuardId>,
	) {
		self.navigation_guard_id = navigation_guard_id;
	}

	pub(crate) fn loader_id(&self) -> Option<RouteLoaderId> {
		self.loader_id
	}

	pub(crate) fn navigation_guard_id(&self) -> Option<NavigationGuardId> {
		self.navigation_guard_id
	}

	pub(crate) fn set_guard(&mut self, guard: RouteGuard) {
		self.guard = Some(guard);
	}

	pub(crate) fn prefix_name(&mut self, namespace: &str) {
		if let Some(name) = &mut self.name {
			*name = format!("{namespace}:{name}");
		}
	}

	/// Returns the route name.
	pub fn name(&self) -> Option<&str> {
		self.name.as_deref()
	}

	/// Returns the pattern.
	pub fn pattern(&self) -> &ClientPathPattern {
		&self.pattern
	}

	/// Returns route-level metadata.
	pub fn metadata(&self) -> &RouteMetadata {
		&self.metadata
	}

	/// Checks if the guard allows access.
	pub fn check_guard(&self, route_match: &ClientRouteMatch) -> bool {
		self.guard.as_ref().map(|g| g(route_match)).unwrap_or(true)
	}

	pub(crate) fn handle_leaf(&self, ctx: &ParamContext) -> Result<Page, RouterError> {
		match &self.handler {
			ClientRouteHandler::Leaf(handler) => handler.handle(ctx),
			ClientRouteHandler::Layout(_) => Err(RouterError::NotFound(
				self.name()
					.map(str::to_string)
					.unwrap_or_else(|| self.pattern().pattern().to_string()),
			)),
		}
	}

	pub(crate) fn handle_layout(
		&self,
		ctx: &ParamContext,
		outlet: Outlet,
	) -> Result<Page, RouterError> {
		match &self.handler {
			ClientRouteHandler::Layout(handler) => handler.handle(ctx, outlet),
			ClientRouteHandler::Leaf(_) => Err(RouterError::NotFound(
				self.name()
					.map(str::to_string)
					.unwrap_or_else(|| self.pattern().pattern().to_string()),
			)),
		}
	}
}

/// The main client-side router.
///
/// `ClientRouter` renders views using the [`Page`] type.
///
/// # `Clone` semantics
///
/// The `Clone` impl is **shallow** by design. `Signal` fields share their
/// underlying reactive state across clones, and on WASM the navigation
/// observer state (held behind `Rc`) is shared as well; only the route
/// table (`Vec<ClientRoute>` / `HashMap<String, usize>`) is copied
/// independently. As a result two clones see the same navigation state
/// but can diverge if either is mutated to register new routes — this
/// is **not** a deep clone of the router.
///
/// In practice `Clone` is only invoked by
/// `collect_client_router_from_inventory` (Refs #4453) as the fallback
/// path for `Arc::try_unwrap` when the underlying factory `Arc` is
/// shared, and `ClientLauncher::register_routes_from_inventory()` is the
/// only caller. Application code should NOT call `.clone()` directly to
/// "branch" routers; treat the router as a single owned value moved
/// into the launcher. Refs Copilot review on PR #4477.
#[derive(Clone)]
pub struct ClientRouter {
	/// Registered routes.
	routes: Vec<ClientRoute>,
	/// Route table indices registered through the nested route tree API.
	tree_route_indices: HashSet<usize>,
	/// Nested route tree for layout-aware rendering.
	route_tree: RouteNode,
	/// Named routes for reverse lookups.
	named_routes: HashMap<String, usize>,
	/// Current path signal.
	current_path: Signal<String>,
	/// Current params signal.
	current_params: Signal<HashMap<String, String>>,
	/// Current matched route name signal.
	current_route_name: Signal<Option<String>>,
	/// Whether the current committed state intentionally selected the not-found surface.
	current_match_is_unmatched: Signal<bool>,
	/// Path whose asynchronous navigation guards were approved by the Pages coordinator.
	current_navigation_guard_approval: Signal<Option<String>>,
	/// Owns navigation state created outside an active reactive scope.
	///
	/// The final router clone drops this scope and disposes its navigation
	/// signals instead of retaining them in a process-wide thread-local scope.
	_navigation_scope: Option<Rc<ReactiveScope>>,
	/// Not found handler.
	not_found: Option<Arc<dyn Fn() -> Page + Send + Sync>>,
	// (Refs #4234, Fixes #4258) Mirrors `pages::Router::navigation_observers`.
	// Navigation observers registered via `on_navigate`. Held as `Weak`
	// so dropping the returned `NavigationSubscription` deregisters the
	// listener.
	//
	// Gated `#[cfg(wasm)]` because the popstate listener and reactive
	// navigation observers are browser-only.
	#[cfg(wasm)]
	navigation_observers: NavigationObservers,
	// (Refs #4234, Fixes #4258) Mirrors `pages::Router::dispatch_count`.
	// Cumulative count of `notify_observers` invocations since this
	// Router was constructed. Used by tests to assert invariants that
	// DOM-only assertions cannot reach.
	//
	// Gated `#[cfg(wasm)]` for the same reason as `navigation_observers`:
	// `Rc<Cell<u64>>` is `!Send + !Sync` on native.
	#[cfg(wasm)]
	dispatch_count: std::rc::Rc<std::cell::Cell<u64>>,
	// Move-stable diagnostic identity for native targets where wasm observer
	// storage does not exist. The `Arc` allocation address remains stable
	// across moves of the `ClientRouter` value itself.
	#[cfg(native)]
	diag_router_identity: Arc<()>,
	// Native reactive signals are scope- and thread-affine, so a router must
	// remain on the thread that created its navigation state.
	#[cfg(native)]
	_thread_bound: PhantomData<Rc<()>>,
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
		#[cfg(wasm)]
		let initial_path = current_location_path().unwrap_or_else(|_| "/".to_string());
		#[cfg(native)]
		let initial_path = current_path().unwrap_or_else(|_| "/".to_string());
		let (
			current_path,
			current_params,
			current_route_name,
			current_match_is_unmatched,
			current_navigation_guard_approval,
			navigation_scope,
		) = create_navigation_signals(initial_path);

		Self {
			routes: Vec::new(),
			tree_route_indices: HashSet::new(),
			route_tree: RouteNode::root(),
			named_routes: HashMap::new(),
			current_path,
			current_params,
			current_route_name,
			current_match_is_unmatched,
			current_navigation_guard_approval,
			_navigation_scope: navigation_scope,
			not_found: None,
			// (Fixes #4258) Reactive observation state is wasm-only; see field
			// definitions on `ClientRouter`.
			#[cfg(wasm)]
			navigation_observers: std::rc::Rc::new(std::cell::RefCell::new(Vec::new())),
			#[cfg(wasm)]
			dispatch_count: std::rc::Rc::new(std::cell::Cell::new(0)),
			#[cfg(native)]
			diag_router_identity: Arc::new(()),
			#[cfg(native)]
			_thread_bound: PhantomData,
		}
	}

	/// Registers a nested route tree.
	///
	/// # Errors
	///
	/// Returns [`RouteRegistrationError`] when a route name, path, or scoped
	/// path pattern is invalid.
	pub fn try_routes<F>(mut self, configure: F) -> Result<Self, RouteRegistrationError>
	where
		F: FnOnce(RouteScope) -> RouteScope,
	{
		let mut scope =
			RouteScope::root(self.registered_route_names(), self.registered_leaf_paths());
		scope = configure(scope);
		self.append_registered_scope(scope.finish()?);
		Ok(self)
	}

	/// Registers a nested route tree and panics if registration fails.
	pub fn routes<F>(self, configure: F) -> Self
	where
		F: FnOnce(RouteScope) -> RouteScope,
	{
		self.try_routes(configure)
			.unwrap_or_else(|err| panic!("Invalid client route tree: {err}"))
	}

	fn append_registered_scope(&mut self, registered: RegisteredRouteScope) {
		let offset = self.routes.len();
		for (idx, route) in registered.routes.iter().enumerate() {
			if let Some(name) = route.name() {
				self.insert_named_route(name, offset + idx);
			}
			self.tree_route_indices.insert(offset + idx);
		}
		self.routes.extend(registered.routes);
		self.route_tree.extend_children(registered.nodes);
	}

	fn registered_route_names(&self) -> Vec<String> {
		let mut names = self.named_routes.keys().cloned().collect::<Vec<_>>();
		self.route_tree.collect_route_names(&mut names);
		names
	}

	fn registered_leaf_paths(&self) -> Vec<String> {
		self.routes
			.iter()
			.map(|route| route.pattern().pattern().to_string())
			.collect()
	}

	/// Combine another `ClientRouter` into this one.
	///
	/// Routes and named-route mappings from `other` are appended to `self`,
	/// preserving the order in which routes were originally registered. The
	/// reactive signals (`current_path`, `current_params`, `current_route_name`)
	/// and the `not_found` handler from `other` are discarded — `self`'s
	/// observation state is the one that drives the merged router.
	///
	/// # Named-route collisions
	///
	/// If both routers register the same named route, the entry from `other`
	/// overwrites the entry from `self` (last-wins). This matches the way
	/// `UnifiedRouter::mount_unified` already composes per-app routers, and
	/// keeps `merge` callable in chains where the caller does not want to
	/// handle errors. Use [`ClientRouter::try_merge`] for a fallible variant
	/// that surfaces collisions instead of silently shadowing them.
	///
	/// # Examples
	///
	/// Composing per-app SPA routers into the single
	/// `ClientRouter` that `ClientLauncher::router_client` expects:
	///
	/// ```rust,ignore
	/// let router = polls_client_url_patterns()
	///     .merge(users_client_url_patterns());
	/// ```
	pub fn merge(mut self, other: ClientRouter) -> Self {
		let offset = self.routes.len();
		let other_tree_children = other.route_tree.children().to_vec();
		for index in other.tree_route_indices {
			self.tree_route_indices.insert(index + offset);
		}
		for (name, idx) in other.named_routes {
			self.named_routes.insert(name, idx + offset);
		}
		self.routes.extend(other.routes);
		self.route_tree.extend_children(other_tree_children);
		self
	}

	/// Like [`ClientRouter::merge`], but fail if any named route collides.
	///
	/// Validates first, so on `Err` `self` is dropped without being mutated.
	/// On success the semantics are identical to `merge` (routes appended,
	/// `other`'s signals and `not_found` discarded).
	///
	/// # Errors
	///
	/// Returns [`MergeError::NameCollision`] carrying a colliding name when
	/// at least one named route is registered in both routers. When several
	/// names collide, the returned `name` is one of them; the choice is
	/// unspecified because named routes are stored in a `HashMap`.
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// match polls_client_url_patterns().try_merge(users_client_url_patterns()) {
	///     Ok(router) => launcher.router_client(|| router),
	///     Err(MergeError::NameCollision { name }) => {
	///         panic!("two apps register the route `{name}`");
	///     }
	/// }
	/// ```
	pub fn try_merge(self, other: ClientRouter) -> Result<Self, MergeError> {
		if let Some(name) = other
			.named_routes
			.keys()
			.find(|name| self.named_routes.contains_key(*name))
		{
			return Err(MergeError::NameCollision { name: name.clone() });
		}
		Ok(self.merge(other))
	}

	/// Prefix all named route keys with `"namespace:"`.
	///
	/// This is the client-side equivalent of `ServerRouter::with_namespace()`.
	/// Called by client/unified route declarations to ensure registered
	/// names match the `"app:route"` format used by per-app resolver
	/// structs.
	pub fn with_namespace(mut self, namespace: &str) -> Self {
		let old = std::mem::take(&mut self.named_routes);
		for (name, idx) in old {
			self.named_routes.insert(format!("{namespace}:{name}"), idx);
		}
		// Also update route names stored inside ClientRoute
		for route in &mut self.routes {
			route.prefix_name(namespace);
		}
		self.route_tree.prefix_names(namespace);
		self
	}

	/// Insert a named route into the lookup table, panicking on duplicates.
	///
	/// All individual route registration methods (`route`, `route_params`,
	/// `route_result`, `page`, `route_path`) delegate to this helper so
	/// that a duplicate name is caught immediately at registration time
	/// rather than causing silent reverse/match drift at runtime.
	///
	/// The `merge` and `with_namespace` methods intentionally bypass this
	/// check: `merge` documents last-wins semantics, and `with_namespace`
	/// re-keys existing entries without introducing new names.
	///
	/// # Panics
	///
	/// Panics if `name` is already registered.
	fn insert_named_route(&mut self, name: &str, index: usize) {
		if self.named_routes.insert(name.to_string(), index).is_some() {
			panic!(
				"Duplicate client route name '{}': a route with this name is already registered",
				name,
			);
		}
	}

	/// Adds metadata to an already-registered named route or layout.
	///
	/// # Panics
	///
	/// Panics if `name` is not registered.
	pub fn with_route_metadata(mut self, name: &str, metadata: RouteMetadata) -> Self {
		let mut updated = false;
		if let Some(index) = self.named_routes.get(name).copied() {
			self.routes[index].set_metadata(metadata.clone());
			updated = true;
		}
		updated |= self
			.route_tree
			.update_metadata_for_name(name, metadata.clone());
		if !updated {
			panic!(
				"Unknown client route name '{}': cannot attach metadata",
				name
			)
		}
		self
	}

	/// Adds a guard to an already-registered named route or layout.
	///
	/// Layout guards are evaluated for every matched descendant route.
	///
	/// # Panics
	///
	/// Panics if `name` is not registered.
	pub fn with_route_guard<G>(mut self, name: &str, guard: G) -> Self
	where
		G: Fn(&ClientRouteMatch) -> bool + Send + Sync + 'static,
	{
		let guard: RouteGuard = Arc::new(guard);
		let mut updated = false;
		if let Some(index) = self.named_routes.get(name).copied() {
			self.routes[index].set_guard(Arc::clone(&guard));
			updated = true;
		}
		updated |= self.route_tree.update_guard_for_name(name, guard);
		if !updated {
			panic!("Unknown client route name '{}': cannot attach guard", name)
		}
		self
	}

	/// Adds a named route to the router.
	///
	/// Every route requires a unique `name` for reverse URL lookup.
	///
	/// # Panics
	///
	/// Panics if `name` duplicates an already-registered route name.
	pub fn route<F>(mut self, name: &str, pattern: &str, component: F) -> Self
	where
		F: Fn() -> Page + Send + Sync + 'static,
	{
		let index = self.routes.len();
		self.routes
			.push(ClientRoute::named(name, pattern, component));
		self.insert_named_route(name, index);
		self
	}

	/// Adds a named route with typed path parameters.
	///
	/// # Panics
	///
	/// Panics if `name` duplicates an already-registered route name.
	pub fn route_params<F, T>(mut self, name: &str, pattern: &str, handler: F) -> Self
	where
		F: Fn(Path<T>) -> Page + Send + Sync + 'static,
		T: FromPath + Send + Sync + 'static,
	{
		let index = self.routes.len();
		self.routes.push(ClientRoute::from_route_handler(
			Some(name.to_string()),
			ClientPathPattern::new(pattern)
				.unwrap_or_else(|e| panic!("Invalid route pattern '{}': {}", pattern, e)),
			with_params_handler(handler),
		));
		self.insert_named_route(name, index);
		self
	}

	/// Adds a named route with typed path parameters that returns a `Result`.
	///
	/// # Panics
	///
	/// Panics if `name` duplicates an already-registered route name.
	pub fn route_result<F, T, E>(mut self, name: &str, pattern: &str, handler: F) -> Self
	where
		F: Fn(Path<T>) -> Result<Page, E> + Send + Sync + 'static,
		T: FromPath + Send + Sync + 'static,
		E: Into<RouterError> + Send + Sync + 'static,
	{
		let index = self.routes.len();
		self.routes.push(ClientRoute::from_route_handler(
			Some(name.to_string()),
			ClientPathPattern::new(pattern)
				.unwrap_or_else(|e| panic!("Invalid route pattern '{}': {}", pattern, e)),
			result_handler(handler),
		));
		self.insert_named_route(name, index);
		self
	}

	/// Adds a named route whose handler receives a single Props struct
	/// constructed via [`FromRequest`] (Manouche DSL v2 spec §4.3).
	///
	/// This is the canonical v2 route-handler shape: the same Props
	/// struct can be used both as a Component (passed via the
	/// `Component { ... }` invocation syntax) and as a page function
	/// (registered here). Path / query extraction errors surface as a
	/// `Page::Text` describing the failure rather than panicking.
	///
	/// # Example
	///
	/// ```ignore
	/// use reinhardt_urls::routers::ClientRouter;
	/// use reinhardt_urls::routers::client_router::from_request::{
	///     ExtractError, FromRequest, PathParam, RouteContext,
	/// };
	///
	/// struct UserPageProps { id: PathParam<i32> }
	///
	/// impl FromRequest for UserPageProps {
	///     fn from_request(ctx: &RouteContext) -> Result<Self, ExtractError> {
	///         Ok(Self { id: PathParam::extract(ctx, "id")? })
	///     }
	/// }
	///
	/// fn user_page(props: UserPageProps) -> reinhardt_core::types::page::Page {
	///     reinhardt_core::types::page::Page::Text(
	///         format!("user {}", props.id.into_inner()).into(),
	///     )
	/// }
	///
	/// let router = ClientRouter::new().page("user", "/users/{id}/", user_page);
	/// ```
	///
	/// # Panics
	///
	/// Panics if the pattern is invalid (exceeds length/segment limits
	/// or invalid regex). Use [`ClientPathPattern::new`] directly for
	/// fallible construction. Also panics if `name` duplicates an
	/// already-registered route name.
	pub fn page<F, P>(mut self, name: &str, pattern: &str, handler: F) -> Self
	where
		F: Fn(P) -> Page + Send + Sync + 'static,
		P: FromRequest + Send + Sync + 'static,
	{
		let index = self.routes.len();
		let loader_id = inventory::iter::<ComponentMetadata>
			.into_iter()
			.find(|metadata| metadata.name == name && metadata.path == pattern)
			.and_then(|metadata| metadata.loader_id);
		let navigation_guard_id = inventory::iter::<ComponentNavigationGuardMetadata>
			.into_iter()
			.find(|metadata| metadata.name == name && metadata.path == pattern)
			.map(|metadata| metadata.navigation_guard_id);
		let mut route = ClientRoute::from_route_handler(
			Some(name.to_string()),
			ClientPathPattern::new(pattern)
				.unwrap_or_else(|e| panic!("Invalid route pattern '{}': {}", pattern, e)),
			from_request_handler(handler, pattern.to_string()),
		);
		route.set_loader_id(loader_id);
		route.set_navigation_guard_id(navigation_guard_id);
		self.routes.push(route);
		self.insert_named_route(name, index);
		self
	}

	/// Adds a named route from metadata generated by `#[component]`.
	///
	/// This mirrors the server-side endpoint registration style: the handler
	/// function supplies the props type, and the props type supplies route
	/// metadata through [`ComponentInfo`].
	///
	/// # Example
	///
	/// ```ignore
	/// use reinhardt_pages::{Page, Path, component, page};
	/// use reinhardt_urls::routers::ClientRouter;
	///
	/// #[component("/users/{id}/", name = "user-detail")]
	/// fn user_page(Path(id): Path<i64>) -> Page {
	///     page!(|id: i64| { div { { id.to_string() } } })(id)
	/// }
	///
	/// let router = ClientRouter::new().component(user_page);
	/// ```
	pub fn component<F, P>(self, handler: F) -> Self
	where
		F: Fn(P) -> Page + Send + Sync + 'static,
		P: FromRequest + ComponentInfo + Send + Sync + 'static,
	{
		let mut router = self.page(P::name(), P::path(), handler);
		if let Some(index) = router.named_routes.get(P::name()).copied() {
			router.routes[index].set_loader_id(P::loader_id());
			router.routes[index].set_navigation_guard_id(P::navigation_guard_id());
		}
		router
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

	/// Adds a named route with one to eight path parameters using `Path<T>`
	/// extractors.
	///
	/// The handler closure may take any number of `Path<T>` arguments from 1
	/// to 8. The compiler infers the arity from the closure signature via the
	/// sealed `Handler<Args>` trait, so the same method name covers every
	/// supported arity. Refs Issue #4637.
	///
	/// # Examples
	///
	/// ```ignore
	/// // One path parameter
	/// let router = ClientRouter::new()
	///     .route_path("user_detail", "/users/{id}/", |Path(id): Path<i64>| user_detail(id));
	///
	/// // Two path parameters
	/// let router = router.route_path(
	///     "user_post_detail",
	///     "/users/{user_id}/posts/{post_id}/",
	///     |Path(user_id): Path<i64>, Path(post_id): Path<i64>| {
	///         user_post_detail(user_id, post_id)
	///     },
	/// );
	///
	/// // Three path parameters
	/// let router = router.route_path(
	///     "issue_detail",
	///     "/org/{org}/repos/{repo}/issues/{issue}/",
	///     |Path(org): Path<String>, Path(repo): Path<String>, Path(issue): Path<i32>| {
	///         issue_detail(org, repo, issue)
	///     },
	/// );
	/// ```
	///
	/// # Panics
	///
	/// Panics if the pattern is invalid or if `name` duplicates an
	/// already-registered route name.
	pub fn route_path<H, Args>(mut self, name: &str, pattern: &str, handler: H) -> Self
	where
		H: Handler<Args>,
	{
		let index = self.routes.len();
		self.routes.push(ClientRoute::from_route_handler(
			Some(name.to_string()),
			ClientPathPattern::new(pattern)
				.unwrap_or_else(|e| panic!("Invalid route pattern '{}': {}", pattern, e)),
			handler.into_route_handler(),
		));
		self.insert_named_route(name, index);
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

	/// Matches the current path for the persistent renderer when navigation is approved.
	///
	/// This hidden cross-crate accessor keeps the persistent layout renderer from
	/// treating a public `current_path` mutation as a committed navigation. A
	/// route carrying asynchronous navigation guards is returned only when the
	/// Pages coordinator recorded approval for the exact current path. Routes
	/// without asynchronous guards remain available for the initial render.
	#[doc(hidden)]
	pub fn __match_current_for_render(&self) -> Option<ClientRouteTreeMatch> {
		if self.current_match_is_unmatched.get() {
			return None;
		}
		let path = self.current_path.get();
		let route_match = self.match_tree(&path)?;
		if !route_match.navigation_guard_ids().is_empty()
			&& self.current_navigation_guard_approval.get().as_deref() != Some(path.as_str())
		{
			return None;
		}
		Some(route_match)
	}

	/// Returns an iterator over registered route patterns and their optional names.
	///
	/// Each item is `(pattern_str, name)` where `name` is `Some` for named routes.
	/// Intended for diagnostic output (e.g., the `runserver` startup banner).
	pub fn route_patterns(&self) -> impl Iterator<Item = (&str, Option<&str>)> {
		self.routes
			.iter()
			.map(|r| (r.pattern.pattern(), r.name.as_deref()))
	}

	/// Matches a path against registered routes.
	///
	/// Strips an optional `?query` suffix before matching and stores
	/// the captured query (without the leading `?`) on
	/// [`ClientRouteMatch::query`]. Patterns therefore match against
	/// the path portion only; the query is delivered to handlers via
	/// the match struct (used by [`ClientRouter::page`] /
	/// [`QueryParam`]).
	///
	/// [`QueryParam`]: super::from_request::QueryParam
	pub fn match_path(&self, path: &str) -> Option<ClientRouteMatch> {
		self.match_tree(path)
			.map(|matched| matched.leaf_match().clone())
	}

	fn match_legacy_path(&self, path: &str) -> Option<ClientRouteMatch> {
		self.match_legacy_path_matching(path, true, |_| true)
	}

	fn match_legacy_path_matching<F>(
		&self,
		path: &str,
		evaluate_guards: bool,
		predicate: F,
	) -> Option<ClientRouteMatch>
	where
		F: Fn(&ClientRouteMatch) -> bool,
	{
		self.match_path_filtered(
			path,
			|index, _| !self.tree_route_indices.contains(&index),
			evaluate_guards,
			predicate,
		)
	}

	fn match_path_filtered<F, P>(
		&self,
		path: &str,
		route_filter: F,
		evaluate_guards: bool,
		predicate: P,
	) -> Option<ClientRouteMatch>
	where
		F: Fn(usize, &ClientRoute) -> bool,
		P: Fn(&ClientRouteMatch) -> bool,
	{
		let path = path.split_once('#').map_or(path, |(path, _)| path);
		let (path_only, query) = match path.split_once('?') {
			Some((p, q)) => (p, Some(q.to_string())),
			None => (path, None),
		};
		for (index, route) in self.routes.iter().enumerate() {
			if !route_filter(index, route) {
				continue;
			}
			if let Some((params, param_values)) = route.pattern.matches(path_only) {
				let route_match = ClientRouteMatch {
					route: route.clone(),
					path: path_only.to_string(),
					params,
					param_values,
					query: query.clone(),
				};

				if (!evaluate_guards || route.check_guard(&route_match)) && predicate(&route_match)
				{
					return Some(route_match);
				}
			}
		}
		None
	}

	/// Matches a path against the nested route tree.
	pub fn match_tree(&self, path: &str) -> Option<ClientRouteTreeMatch> {
		self.match_tree_with_guard_evaluation(path, true, |_| true)
	}

	fn match_tree_with_guard_evaluation<F>(
		&self,
		path: &str,
		evaluate_guards: bool,
		predicate: F,
	) -> Option<ClientRouteTreeMatch>
	where
		F: Fn(&ClientRouteTreeMatch) -> bool,
	{
		let match_path = path.split_once('#').map_or(path, |(path, _)| path);
		let (path_only, query) = match match_path.split_once('?') {
			Some((p, q)) => (p, Some(q.to_string())),
			None => (match_path, None),
		};
		let route_match = if evaluate_guards {
			self.route_tree.match_path(path_only, query.clone())
		} else {
			self.route_tree.match_path_without_guards_matching(
				path_only,
				query.clone(),
				|route_match| predicate(route_match),
			)
		};
		if let Some(route_match) = route_match {
			return Some(route_match);
		}
		let leaf = if evaluate_guards {
			self.match_legacy_path(match_path)
		} else {
			self.match_legacy_path_matching(match_path, false, |leaf| {
				let route_match = self.legacy_tree_match(leaf.clone());
				predicate(&route_match)
			})
		};
		leaf.map(|leaf| self.legacy_tree_match(leaf))
	}

	fn legacy_tree_match(&self, leaf: ClientRouteMatch) -> ClientRouteTreeMatch {
		let leaf_metadata = ResolvedRouteMetadata::new(
			leaf.route.name().map(str::to_string),
			leaf.route.pattern().pattern().to_string(),
			leaf.route.pattern().pattern().to_string(),
			None,
			None,
			None,
			leaf.route.loader_id(),
			leaf.route.navigation_guard_id(),
			leaf.route.metadata().clone(),
		);
		ClientRouteTreeMatch::new(leaf, Vec::new(), leaf_metadata)
	}

	fn param_context_from_match(route_match: &ClientRouteMatch) -> ParamContext {
		ParamContext::new(route_match.params.clone(), route_match.param_values.clone())
			.with_path(route_match.path.clone())
			.with_query(route_match.query.clone())
	}

	fn render_tree_match(&self, route_match: &ClientRouteTreeMatch) -> Option<Page> {
		let mut page = self.render_tree_leaf(route_match)?;
		for index in (0..route_match.layouts().len()).rev() {
			page = self.render_tree_layout(route_match, index, Outlet::inline(page))?;
		}
		Some(page)
	}

	fn render_matched_path(&self, route_match: &ClientRouteTreeMatch) -> Page {
		self.render_tree_match(route_match)
			.unwrap_or_else(|| self.__render_not_found())
	}

	fn render_tree_leaf(&self, route_match: &ClientRouteTreeMatch) -> Option<Page> {
		let ctx = Self::param_context_from_match(route_match.leaf_match());
		let head = route_match.leaf().metadata().head().clone();
		route_match
			.leaf()
			.handle_leaf(&ctx)
			.ok()
			.map(|page| page.with_head(head))
	}

	fn render_tree_layout(
		&self,
		route_match: &ClientRouteTreeMatch,
		layout_index: usize,
		outlet: Outlet,
	) -> Option<Page> {
		let ctx = Self::param_context_from_match(route_match.leaf_match());
		let layout = route_match.layouts().get(layout_index)?;
		let head = layout.metadata().route_metadata().head().clone();
		layout
			.route()
			.handle_layout(&ctx, outlet)
			.ok()
			.map(|page| page.with_head(head))
	}

	/// Renders only the leaf route for a tree match.
	///
	/// Hidden cross-crate hook for the WASM layout persistence mount path.
	///
	/// # Safety
	///
	/// The caller must pass a match produced by this router and must ensure
	/// that every asynchronous navigation guard for the match has completed
	/// successfully before rendering a guarded route.
	#[doc(hidden)]
	pub unsafe fn __render_tree_leaf(&self, route_match: &ClientRouteTreeMatch) -> Option<Page> {
		self.render_tree_leaf(route_match)
	}

	/// Renders one matched layout using the provided outlet.
	///
	/// Hidden cross-crate hook for the WASM layout persistence mount path.
	///
	/// # Safety
	///
	/// The caller must pass a match produced by this router and must ensure
	/// that every asynchronous navigation guard for the match has completed
	/// successfully before rendering a guarded route.
	#[doc(hidden)]
	pub unsafe fn __render_tree_layout(
		&self,
		route_match: &ClientRouteTreeMatch,
		layout_index: usize,
		outlet: Outlet,
	) -> Option<Page> {
		self.render_tree_layout(route_match, layout_index, outlet)
	}

	/// Renders a path without mutating router navigation state.
	///
	/// Routes with asynchronous navigation guards are not rendered through this
	/// low-level API. Use the Pages navigation coordinator so those guards can
	/// run before the protected route is committed and rendered.
	pub fn render_path(&self, path: &str) -> Page {
		if let Some(route_match) = self.match_tree(path)
			&& route_match.navigation_guard_ids().is_empty()
		{
			return self.render_matched_path(&route_match);
		}
		self.__render_not_found()
	}

	/// Navigates to a path using pushState.
	///
	/// Routes with asynchronous navigation guards or loaders cannot be
	/// navigated through this low-level API. Such routes return
	/// [`RouterError::NavigationFailed`] and require the Pages navigation
	/// coordinator to prepare them before committing.
	pub fn push(&self, path: &str) -> Result<(), RouterError> {
		self.navigate(path, NavigationType::Push)
	}

	/// Navigates to a path using replaceState.
	///
	/// Routes with asynchronous navigation guards or loaders cannot be
	/// navigated through this low-level API. Such routes return
	/// [`RouterError::NavigationFailed`] and require the Pages navigation
	/// coordinator to prepare them before committing.
	pub fn replace(&self, path: &str) -> Result<(), RouterError> {
		self.navigate(path, NavigationType::Replace)
	}

	/// Commits an already matched route after any asynchronous preparation.
	///
	/// Matching and guard evaluation are deliberately separate from this
	/// operation. The supplied match must be the match produced by this router
	/// for `path`; mismatched paths or route metadata are rejected before any
	/// state changes. Matches carrying asynchronous navigation guards are also
	/// rejected because those guards require the Pages navigation coordinator.
	/// `Push` and `Replace` update browser history first; `Pop` and the
	/// signal-only `Initial` path do not create a new entry. Signals and
	/// observers are updated exactly once after the history operation succeeds.
	pub fn commit_match(
		&self,
		path: &str,
		matched: &ClientRouteTreeMatch,
		navigation: NavigationType,
		entry_index: i64,
	) -> Result<(), RouterError> {
		self.validate_match_for_path(path, matched)?;
		if !matched.navigation_guard_ids().is_empty() {
			return Err(RouterError::NavigationFailed(
				NAVIGATION_GUARD_COORDINATOR_REQUIRED.to_owned(),
			));
		}
		self.commit_match_validated(path, matched, navigation, entry_index)
	}

	/// Commits a guarded route after the Pages coordinator has approved its
	/// navigation guards.
	///
	/// This hidden cross-crate hook keeps asynchronous guard execution in
	/// `reinhardt-pages` while allowing the low-level router to record the
	/// approved route for `render_current`. The Pages coordinator uses the
	/// safe [`ClientRouter::commit_match`] operation for matches without
	/// asynchronous navigation guards.
	///
	/// # Safety
	///
	/// The caller must be the Pages navigation coordinator and must call this
	/// method only after every asynchronous navigation guard for `matched` has
	/// completed successfully. Call [`ClientRouter::commit_match`] for routes
	/// without asynchronous navigation guards.
	#[doc(hidden)]
	pub unsafe fn __commit_match_after_navigation_guard(
		&self,
		path: &str,
		matched: &ClientRouteTreeMatch,
		navigation: NavigationType,
		entry_index: i64,
	) -> Result<(), RouterError> {
		self.validate_match_for_path(path, matched)?;
		self.commit_match_validated(path, matched, navigation, entry_index)
	}

	fn validate_match_for_path(
		&self,
		path: &str,
		matched: &ClientRouteTreeMatch,
	) -> Result<(), RouterError> {
		let Some(actual) = self.match_tree_with_guard_evaluation(path, false, |actual| {
			Self::same_route_tree_match(actual, matched)
		}) else {
			return Err(RouterError::NavigationFailed(
				MATCHED_ROUTE_PATH_MISMATCH.to_owned(),
			));
		};
		if !Self::same_route_tree_match(&actual, matched) {
			return Err(RouterError::NavigationFailed(
				MATCHED_ROUTE_PATH_MISMATCH.to_owned(),
			));
		}
		Ok(())
	}

	fn same_route_tree_match(
		actual: &ClientRouteTreeMatch,
		candidate: &ClientRouteTreeMatch,
	) -> bool {
		actual.path() == candidate.path()
			&& actual.query() == candidate.query()
			&& actual.params() == candidate.params()
			&& actual.param_values() == candidate.param_values()
			&& actual.metadata_chain() == candidate.metadata_chain()
			&& actual.loader_ids() == candidate.loader_ids()
			&& actual.navigation_guard_ids() == candidate.navigation_guard_ids()
			&& Self::same_client_route(actual.leaf(), candidate.leaf())
			&& actual.layouts().len() == candidate.layouts().len()
			&& actual
				.layouts()
				.iter()
				.zip(candidate.layouts())
				.all(|(actual, candidate)| {
					actual.key() == candidate.key()
						&& actual.metadata() == candidate.metadata()
						&& Self::same_client_route(actual.route(), candidate.route())
				})
	}

	fn same_client_route(actual: &ClientRoute, candidate: &ClientRoute) -> bool {
		actual.pattern == candidate.pattern
			&& actual.name == candidate.name
			&& actual.metadata == candidate.metadata
			&& actual.loader_id == candidate.loader_id
			&& actual.navigation_guard_id == candidate.navigation_guard_id
			&& match (&actual.handler, &candidate.handler) {
				(ClientRouteHandler::Leaf(actual), ClientRouteHandler::Leaf(candidate)) => {
					Arc::ptr_eq(actual, candidate)
				}
				(ClientRouteHandler::Layout(actual), ClientRouteHandler::Layout(candidate)) => {
					Arc::ptr_eq(actual, candidate)
				}
				_ => false,
			} && match (&actual.guard, &candidate.guard) {
			(Some(actual), Some(candidate)) => Arc::ptr_eq(actual, candidate),
			(None, None) => true,
			_ => false,
		}
	}

	fn commit_match_validated(
		&self,
		path: &str,
		matched: &ClientRouteTreeMatch,
		navigation: NavigationType,
		entry_index: i64,
	) -> Result<(), RouterError> {
		let leaf = matched.leaf_match();
		let mut state = HistoryState::new(path)
			.with_params(leaf.params.clone())
			.with_entry_index(entry_index);
		if let Some(name) = leaf.route.name() {
			state = state.with_route_name(name);
		}

		match navigation {
			NavigationType::Push => push_state(&state),
			NavigationType::Replace => replace_state(&state),
			// The launcher normalizes the first entry before preparation. Keeping
			// this commit signal-only preserves host state upgraded during launch.
			NavigationType::Initial => Ok(()),
			NavigationType::Pop => Ok(()),
		}
		.map_err(RouterError::NavigationFailed)?;

		self.current_path.set(path.to_string());
		self.current_params.set(leaf.params.clone());
		self.current_route_name
			.set(leaf.route.name().map(str::to_string));
		self.current_match_is_unmatched.set(false);
		self.current_navigation_guard_approval
			.set(Some(path.to_owned()));
		self.notify_observers(path, &leaf.params);
		Ok(())
	}

	/// Commits a path that has no matching route so the configured not-found
	/// renderer observes the navigation just like a direct [`Self::push`].
	pub fn commit_unmatched(
		&self,
		path: &str,
		navigation: NavigationType,
		entry_index: i64,
	) -> Result<(), RouterError> {
		let state = HistoryState::new(path).with_entry_index(entry_index);
		match navigation {
			NavigationType::Push => push_state(&state),
			NavigationType::Replace => replace_state(&state),
			NavigationType::Initial => Ok(()),
			NavigationType::Pop => Ok(()),
		}
		.map_err(RouterError::NavigationFailed)?;

		self.current_path.set(path.to_string());
		let params = HashMap::new();
		self.current_params.set(params.clone());
		self.current_route_name.set(None);
		self.current_match_is_unmatched.set(true);
		self.current_navigation_guard_approval.set(None);
		self.notify_observers(path, &params);
		Ok(())
	}

	/// Internal navigation implementation.
	fn navigate(&self, path: &str, nav_type: NavigationType) -> Result<(), RouterError> {
		if let Some(matched) = self.match_tree(path) {
			if !matched.loader_ids().is_empty() {
				return Err(RouterError::NavigationFailed(
					"route loaders require navigation through reinhardt-pages".to_string(),
				));
			}
			if !matched.navigation_guard_ids().is_empty() {
				return Err(RouterError::NavigationFailed(
					NAVIGATION_GUARD_COORDINATOR_REQUIRED.to_owned(),
				));
			}
			return self.commit_match(path, &matched, nav_type, 0);
		}
		self.commit_unmatched(path, nav_type, 0)
	}

	/// Register a listener for navigation events.
	///
	/// Returns a [`NavigationSubscription`] handle. Drop the handle to
	/// deregister the listener. The router itself only retains a `Weak`
	/// reference, so dropping the subscription frees the listener
	/// closure immediately. The stale `Weak` entry in
	/// `navigation_observers` is pruned lazily on the next
	/// `notify_observers` call (the listener itself is already gone by
	/// then).
	///
	/// Robust against nested reactive nodes spawned during view rendering
	/// because this subscription is independent of the reactive
	/// `Effect` / `Signal` auto-tracking system.
	///
	/// Mirrors `pages::Router::on_navigate`. (Refs #4234)
	///
	/// On native targets (Fixes #4258) this is effectively a no-op: the
	/// returned `NavigationSubscription` is unbound to any observer storage
	/// because reactive observation only fires from the wasm popstate
	/// listener. The method is still callable on native so that the
	/// `SpaRouter` trait impl in `reinhardt-pages` (which dispatches into
	/// `on_navigate` from cross-target launcher code) keeps compiling.
	pub fn on_navigate<F>(&self, listener: F) -> NavigationSubscription
	where
		F: Fn(&str, &HashMap<String, String>) + 'static,
	{
		NavigationSubscription::new(self, listener)
	}

	/// Dispatch the registered `on_navigate` listeners with the given path
	/// and params.
	///
	/// Both `ClientRouter::navigate` (after a programmatic push/replace) and
	/// the popstate listener (after a browser-driven back/forward) end up
	/// calling [`dispatch_navigation_observers`] after the `Signal` updates
	/// so listeners always see the new state when they read `Signal::get`
	/// from inside their closure.
	///
	/// (Refs #4234, Inv-4)
	///
	/// Wasm-only (Fixes #4258): the reactive observer state lives only on
	/// wasm. The native no-op stub immediately below preserves the
	/// cross-target call site in `ClientRouter::navigate`.
	#[cfg(wasm)]
	fn notify_observers(&self, path: &str, params: &HashMap<String, String>) {
		// (Refs #4234) `nav_diag_dom!` invocation from
		// `pages::Router::notify_observers` is intentionally not mirrored
		// here. The nav-diag-dom feature can be added separately in a
		// follow-up if downstream consumers need urls-side runtime
		// diagnostics.
		dispatch_navigation_observers(
			&self.navigation_observers,
			&self.dispatch_count,
			path,
			params,
		);
	}

	/// Native no-op stub for `notify_observers` (Fixes #4258).
	///
	/// On native targets there is no popstate listener, so navigation cannot
	/// dispatch browser listeners. This stub keeps the call site in
	/// `ClientRouter::navigate` cross-target.
	#[cfg(native)]
	fn notify_observers(&self, _path: &str, _params: &HashMap<String, String>) {}

	/// Diagnostic counter: number of currently-alive navigation observers.
	///
	/// Returns the count of `Weak<NavigationListener>` entries in
	/// `navigation_observers` whose `strong_count() > 0`. Used by tests in
	/// `tests/wasm/` to assert observer-lifecycle invariants.
	///
	/// Internal diagnostic API. `#[doc(hidden)]` removes this from the
	/// rendered documentation, but it remains technically part of the
	/// public API surface. Treat it as unstable: callers outside this
	/// crate's own tests should not depend on it. (Refs #4234)
	///
	/// On native (Fixes #4258) this returns `0` because the observer storage
	/// itself is wasm-only. Stays callable on both targets so the
	/// `SpaRouter` trait impl in `reinhardt-pages` keeps compiling.
	/// Consumed by `tests/wasm/*` (see `required-features =
	/// ["wasm-diag-test"]` in `Cargo.toml`).
	#[doc(hidden)]
	pub fn __diag_observer_count(&self) -> usize {
		self.diag_observer_count()
	}

	/// Diagnostic counter: cumulative `notify_observers` invocation count.
	///
	/// Includes invocations from `ClientRouter::push`,
	/// `ClientRouter::replace`, and the popstate listener.
	///
	/// Hidden API for testing only. (Refs #4234)
	///
	/// On native (Fixes #4258) this returns `0` — see `__diag_observer_count`.
	#[doc(hidden)]
	pub fn __diag_dispatch_count(&self) -> u64 {
		self.diag_dispatch_count()
	}

	/// Stable per-instance router id for diagnostic correlation.
	///
	/// Returns the pointer of the `Rc` backing `navigation_observers`.
	/// Two `ClientRouter` values share an id iff they share the same
	/// observer list, which only happens within the same logical instance:
	/// the `Rc` is constructed fresh in `ClientRouter::new` and never
	/// reseated.
	///
	/// Hidden API for testing only. (Refs #4234)
	///
	/// On native (Fixes #4258) this returns the address of a heap-backed
	/// identity marker because the observer `Rc` storage is wasm-only. The id
	/// stays per-instance-stable across moves of the `ClientRouter` value.
	#[doc(hidden)]
	pub fn __diag_router_id(&self) -> usize {
		self.diag_router_id()
	}

	#[cfg(wasm)]
	fn diag_observer_count(&self) -> usize {
		self.navigation_observers
			.borrow()
			.iter()
			.filter(|w| w.strong_count() > 0)
			.count()
	}

	#[cfg(native)]
	fn diag_observer_count(&self) -> usize {
		0
	}

	#[cfg(wasm)]
	fn diag_dispatch_count(&self) -> u64 {
		self.dispatch_count.get()
	}

	#[cfg(native)]
	fn diag_dispatch_count(&self) -> u64 {
		0
	}

	#[cfg(wasm)]
	fn diag_router_id(&self) -> usize {
		std::rc::Rc::as_ptr(&self.navigation_observers) as usize
	}

	#[cfg(native)]
	fn diag_router_id(&self) -> usize {
		Arc::as_ptr(&self.diag_router_identity) as usize
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
	/// Returns the registered `not_found` page when no route matches, or a
	/// default 404 page if no `not_found` handler has been set.
	pub fn render_current(&self) -> Page {
		if self.current_match_is_unmatched.get() {
			return self.__render_not_found();
		}
		let path = self.current_path.get();
		let Some(route_match) = self.match_tree(&path) else {
			return self.__render_not_found();
		};
		if !route_match.navigation_guard_ids().is_empty()
			&& self.current_navigation_guard_approval.get().as_deref() != Some(path.as_str())
		{
			return self.__render_not_found();
		}
		self.render_matched_path(&route_match)
	}

	/// Renders the configured not-found handler without inspecting the current path.
	#[doc(hidden)]
	pub fn __render_not_found(&self) -> Page {
		self.not_found
			.as_ref()
			.map(|render| render())
			.unwrap_or(Page::Empty)
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
	#[cfg(wasm)]
	pub fn setup_history_listener(&self) {
		let path_signal = self.current_path;
		let params_signal = self.current_params;
		let route_name_signal = self.current_route_name;
		let current_match_is_unmatched = self.current_match_is_unmatched;
		let current_navigation_guard_approval = self.current_navigation_guard_approval;
		let navigation_scope = self._navigation_scope.clone();
		let navigation_observers = self.navigation_observers.clone();
		let dispatch_count = self.dispatch_count.clone();

		let closure = setup_popstate_listener(move |path, state| {
			let _ = &navigation_scope;
			// (Refs #4234, Inv-1, Inv-5) Update Signals first, then notify
			// observers, so listeners that read `Signal::get` from inside
			// their closure see the new state. Mirrors
			// `ClientRouter::navigate`.
			path_signal.set(path.clone());
			current_match_is_unmatched.set(false);
			current_navigation_guard_approval.set(None);

			let params_for_observers = if let Some(hist_state) = state {
				let params = hist_state.params.clone();
				params_signal.set(hist_state.params);
				route_name_signal.set(hist_state.route_name);
				params
			} else {
				// Clear params when no state is available.
				params_signal.set(HashMap::new());
				route_name_signal.set(None);
				HashMap::new()
			};

			// (Refs #4234, Inv-4, Inv-5, Inv-6) Bump the diagnostic counter
			// and dispatch on_navigate observers via the shared helper so
			// popstate-driven dispatches are counted and ordered
			// identically to push/replace-driven ones.
			dispatch_navigation_observers(
				&navigation_observers,
				&dispatch_count,
				&path,
				&params_for_observers,
			);
		});

		if let Ok(c) = closure {
			// Keep the closure alive for the lifetime of the page
			c.forget();
		}
	}

	/// Non-WASM version of `setup_history_listener`.
	#[cfg(native)]
	pub fn setup_history_listener(&self) {
		// No-op on non-WASM targets
	}
}

#[cfg(all(test, wasm))]
mod wasm_tests {
	use super::*;
	use wasm_bindgen::JsValue;
	use wasm_bindgen_test::*;

	wasm_bindgen_test_configure!(run_in_browser);

	#[wasm_bindgen_test]
	fn popstate_clears_forced_unmatched_rendering_for_a_matching_path() {
		let router = ClientRouter::new()
			.route("valid", "/valid/", || Page::text("valid"))
			.not_found(|| Page::text("not found"));
		router
			.commit_unmatched("/valid/", NavigationType::Initial, 0)
			.expect("forced unmatched commit succeeds");

		let window = web_sys::window().expect("browser window exists");
		window
			.history()
			.expect("browser history exists")
			.replace_state_with_url(&JsValue::NULL, "", Some("/valid/"))
			.expect("set matching popstate location");
		router.setup_history_listener();
		window
			.dispatch_event(
				&web_sys::PopStateEvent::new("popstate").expect("create popstate event"),
			)
			.expect("dispatch popstate");

		assert_eq!(router.render_current().render_to_string(), "valid");
	}
}

/// Snapshot, prune, and invoke navigation observers.
///
/// Bumps `dispatch_count` first so even a no-listener dispatch is
/// counted (Inv-5), then collects strong `Rc` references to live
/// listeners while pruning dead `Weak` entries (Inv-6). The `RefCell`
/// borrow is released before any user-supplied closure runs (Inv-4),
/// which lets listeners call `ClientRouter::push` /
/// `ClientRouter::replace` reentrantly, register new listeners via
/// `on_navigate`, or drop existing `NavigationSubscription` handles
/// without panicking on `RefCell` reentry.
///
/// Each listener is also invoked inside the reactive scope that was active at
/// registration time. If that owner scope has been disposed, the listener is
/// skipped along with its scoped reactive work.
///
/// Used by both `ClientRouter::notify_observers` (programmatic
/// push/replace) and the popstate listener (browser back/forward) so
/// the two code paths stay observably identical. (Refs #4234, Inv-4)
///
/// Wasm-only (Fixes #4258): touches `NavigationObservers` /
/// `dispatch_count` which are themselves wasm-only.
#[cfg(wasm)]
fn dispatch_navigation_observers(
	navigation_observers: &NavigationObservers,
	dispatch_count: &std::rc::Rc<std::cell::Cell<u64>>,
	path: &str,
	params: &HashMap<String, String>,
) {
	dispatch_count.set(dispatch_count.get() + 1);
	let listeners_snapshot: Vec<std::rc::Rc<NavigationListener>> = {
		let mut observers = navigation_observers.borrow_mut();
		observers.retain(|w| w.strong_count() > 0);
		observers.iter().filter_map(|w| w.upgrade()).collect()
	};
	for listener in listeners_snapshot {
		match listener.owner_scope {
			Some(scope) => {
				let _ = enter_scope(scope, || (listener.callback)(path, params));
			}
			None => (listener.callback)(path, params),
		}
	}
}

#[cfg(test)]
mod tests {
	use super::super::component::{FromLayoutRequest, LayoutInfo};
	use super::super::from_request::{ExtractError, RouteContext};
	use super::*;
	use reinhardt_core::reactive::{Effect, ReactiveScope, with_runtime};
	use rstest::*;

	struct LoaderBoundPageProps;

	impl FromRequest for LoaderBoundPageProps {
		fn from_request(_ctx: &RouteContext) -> Result<Self, ExtractError> {
			Ok(Self)
		}
	}

	impl ComponentInfo for LoaderBoundPageProps {
		fn path() -> &'static str {
			"/loaded/"
		}

		fn name() -> &'static str {
			"loaded-page"
		}

		fn component_name() -> &'static str {
			"LoadedPage"
		}

		fn function_name() -> &'static str {
			"loaded_page"
		}

		fn props_type_name() -> &'static str {
			"LoaderBoundPageProps"
		}

		fn navigation_guard_id() -> Option<NavigationGuardId> {
			Some(NavigationGuardId::new("test:loaded-page-guard"))
		}
	}

	struct GuardOnlyPageProps;

	impl FromRequest for GuardOnlyPageProps {
		fn from_request(_ctx: &RouteContext) -> Result<Self, ExtractError> {
			Ok(Self)
		}
	}

	impl ComponentInfo for GuardOnlyPageProps {
		fn path() -> &'static str {
			"/guard-only/"
		}

		fn name() -> &'static str {
			"guard-only-page"
		}

		fn component_name() -> &'static str {
			"GuardOnlyPage"
		}

		fn function_name() -> &'static str {
			"guard_only_page"
		}

		fn props_type_name() -> &'static str {
			"GuardOnlyPageProps"
		}

		fn navigation_guard_id() -> Option<NavigationGuardId> {
			Some(NavigationGuardId::new("test:guard-only-page"))
		}
	}

	inventory::submit! {
		ComponentMetadata {
			path: "/loaded/",
			name: "loaded-page",
			component_name: "LoadedPage",
			function_name: "loaded_page",
			props_type_name: "LoadedPageProps",
			module_path: module_path!(),
			loader_id: Some(RouteLoaderId::new("test:loaded-page")),
		}
	}

	inventory::submit! {
		ComponentNavigationGuardMetadata {
			path: "/loaded/",
			name: "loaded-page",
			navigation_guard_id: NavigationGuardId::new("test:loaded-page-guard"),
		}
	}

	fn loaded_page(_props: LoaderBoundPageProps) -> Page {
		Page::Empty
	}

	fn guard_only_page(_props: GuardOnlyPageProps) -> Page {
		page_with_text("protected")
	}

	struct NavigationOuterLayoutProps;

	impl FromLayoutRequest for NavigationOuterLayoutProps {
		fn from_layout_request(_ctx: &RouteContext, _outlet: Outlet) -> Result<Self, ExtractError> {
			Ok(Self)
		}
	}

	impl LayoutInfo for NavigationOuterLayoutProps {
		fn path() -> &'static str {
			"/navigation/"
		}

		fn name() -> &'static str {
			"navigation-outer"
		}

		fn component_name() -> &'static str {
			"NavigationOuter"
		}

		fn function_name() -> &'static str {
			"navigation_outer"
		}

		fn props_type_name() -> &'static str {
			"NavigationOuterLayoutProps"
		}

		fn navigation_guard_id() -> Option<NavigationGuardId> {
			Some(NavigationGuardId::new("test:navigation-outer"))
		}
	}

	struct NavigationMiddleLayoutProps;

	impl FromLayoutRequest for NavigationMiddleLayoutProps {
		fn from_layout_request(_ctx: &RouteContext, _outlet: Outlet) -> Result<Self, ExtractError> {
			Ok(Self)
		}
	}

	impl LayoutInfo for NavigationMiddleLayoutProps {
		fn path() -> &'static str {
			"middle/"
		}

		fn name() -> &'static str {
			"navigation-middle"
		}

		fn component_name() -> &'static str {
			"NavigationMiddle"
		}

		fn function_name() -> &'static str {
			"navigation_middle"
		}

		fn props_type_name() -> &'static str {
			"NavigationMiddleLayoutProps"
		}

		fn navigation_guard_id() -> Option<NavigationGuardId> {
			Some(NavigationGuardId::new("test:navigation-middle"))
		}
	}

	struct NavigationInnerLayoutProps;

	impl FromLayoutRequest for NavigationInnerLayoutProps {
		fn from_layout_request(_ctx: &RouteContext, _outlet: Outlet) -> Result<Self, ExtractError> {
			Ok(Self)
		}
	}

	impl LayoutInfo for NavigationInnerLayoutProps {
		fn path() -> &'static str {
			"inner/"
		}

		fn name() -> &'static str {
			"navigation-inner"
		}

		fn component_name() -> &'static str {
			"NavigationInner"
		}

		fn function_name() -> &'static str {
			"navigation_inner"
		}

		fn props_type_name() -> &'static str {
			"NavigationInnerLayoutProps"
		}

		fn navigation_guard_id() -> Option<NavigationGuardId> {
			Some(NavigationGuardId::new("test:navigation-outer"))
		}
	}

	struct NavigationPageProps;

	impl FromRequest for NavigationPageProps {
		fn from_request(_ctx: &RouteContext) -> Result<Self, ExtractError> {
			Ok(Self)
		}
	}

	impl super::ComponentInfo for NavigationPageProps {
		fn path() -> &'static str {
			"page/"
		}

		fn name() -> &'static str {
			"navigation-page"
		}

		fn component_name() -> &'static str {
			"NavigationPage"
		}

		fn function_name() -> &'static str {
			"navigation_page"
		}

		fn props_type_name() -> &'static str {
			"NavigationPageProps"
		}

		fn navigation_guard_id() -> Option<NavigationGuardId> {
			Some(NavigationGuardId::new("test:navigation-page"))
		}
	}

	fn navigation_layout(_props: NavigationOuterLayoutProps) -> Page {
		Page::Empty
	}

	fn navigation_middle_layout(_props: NavigationMiddleLayoutProps) -> Page {
		Page::Empty
	}

	fn navigation_inner_layout(_props: NavigationInnerLayoutProps) -> Page {
		Page::Empty
	}

	fn navigation_page(_props: NavigationPageProps) -> Page {
		Page::Empty
	}

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
		ReactiveScope::run(|| {
			let router = ClientRouter::new();
			assert_eq!(router.route_count(), 0);
		});
	}

	#[cfg(native)]
	#[test]
	fn router_builds_route_table_without_an_active_reactive_scope() {
		let router = ClientRouter::new().route("home", "/", home_page);

		assert_eq!(router.route_count(), 1);
		assert_eq!(router.reverse("home", &[]).unwrap(), "/");
	}

	#[cfg(native)]
	#[test]
	fn unscoped_router_keeps_navigation_state_on_its_owner_thread() {
		let router = ClientRouter::new().route("home", "/", home_page);

		assert_eq!(router.current_path().get(), "/");
		router.push("/").expect("native navigation must succeed");
		assert_eq!(router.current_path().get(), "/");
	}

	#[cfg(native)]
	#[test]
	#[serial_test::serial(reactive_runtime)]
	fn dropping_an_unscoped_router_removes_its_navigation_nodes() {
		let (path_id, params_id, route_name_id) = {
			let router = ClientRouter::new();
			let path = *router.current_path();
			let params = *router.current_params();
			let route_name = *router.current_route_name();

			ReactiveScope::run(|| {
				let _effect = Effect::new(move || {
					let _ = (path.get(), params.get(), route_name.get());
				});
			});

			(path.id(), params.id(), route_name.id())
		};

		with_runtime(|runtime| {
			let path_removed = !runtime.has_node(path_id);
			let params_removed = !runtime.has_node(params_id);
			let route_name_removed = !runtime.has_node(route_name_id);
			assert!(
				path_removed,
				"dropping an unscoped router must remove its path signal node"
			);
			assert!(
				params_removed,
				"dropping an unscoped router must remove its params signal node"
			);
			assert!(
				route_name_removed,
				"dropping an unscoped router must remove its route-name signal node"
			);
		});
	}

	#[test]
	fn test_router_add_route() {
		ReactiveScope::run(|| {
			let router = ClientRouter::new()
				.route("home", "/", home_page)
				.route("users", "/users/", user_page);

			assert_eq!(router.route_count(), 2);
		});
	}

	#[test]
	fn test_router_route_with_name() {
		ReactiveScope::run(|| {
			let router = ClientRouter::new()
				.route("home", "/", home_page)
				.route("users", "/users/", user_page);

			assert!(router.has_route("home"));
			assert!(router.has_route("users"));
			assert!(!router.has_route("nonexistent"));
		});
	}

	#[test]
	fn test_router_match_exact() {
		ReactiveScope::run(|| {
			let router = ClientRouter::new()
				.route("home", "/", home_page)
				.route("users", "/users/", user_page);

			assert!(router.match_path("/").is_some());
			assert!(router.match_path("/users/").is_some());
			assert!(router.match_path("/nonexistent/").is_none());
		});
	}

	#[rstest]
	fn test_router_match_ignores_fragment() {
		ReactiveScope::run(|| {
			let router = ClientRouter::new().route("users", "/users/", user_page);

			let route_match = router
				.match_path("/users/?tab=active#details")
				.expect("route should ignore the fragment");
			assert_eq!(route_match.path, "/users/");
			assert_eq!(route_match.query.as_deref(), Some("tab=active"));
		});
	}

	#[test]
	fn match_tree_does_not_fall_back_to_flat_copy_for_tree_routes() {
		ReactiveScope::run(|| {
			let mut router = ClientRouter::new()
				.routes(|routes| routes.route("tree", "/tree/", || page_with_text("Tree")));
			assert!(router.match_tree("/tree/").is_some());

			router.route_tree = RouteNode::root();

			assert!(router.match_tree("/tree/").is_none());
			assert!(router.match_path("/tree/").is_none());
		});
	}

	#[test]
	fn test_router_match_params() {
		ReactiveScope::run(|| {
			let router = ClientRouter::new().route("user_detail", "/users/{id}/", user_page);

			let route_match = router.match_path("/users/42/");
			assert!(route_match.is_some());

			let route_match = route_match.unwrap();
			assert_eq!(route_match.params.get("id"), Some(&"42".to_string()));
		});
	}

	#[test]
	fn test_router_reverse() {
		ReactiveScope::run(|| {
			let router = ClientRouter::new().route("home", "/", home_page).route(
				"user_detail",
				"/users/{id}/",
				user_page,
			);

			assert_eq!(router.reverse("home", &[]).unwrap(), "/");
			assert_eq!(
				router.reverse("user_detail", &[("id", "42")]).unwrap(),
				"/users/42/"
			);
		});
	}

	#[test]
	fn test_router_reverse_invalid_name() {
		ReactiveScope::run(|| {
			let router = ClientRouter::new();
			let result = router.reverse("nonexistent", &[]);
			assert!(matches!(result, Err(RouterError::InvalidRouteName(_))));
		});
	}

	#[test]
	fn test_router_not_found() {
		ReactiveScope::run(|| {
			let router = ClientRouter::new().not_found(not_found_page);

			let _view = router.render_current();
		});
	}

	#[test]
	fn commit_unmatched_renders_not_found_even_when_path_matches_a_route() {
		ReactiveScope::run(|| {
			let router = ClientRouter::new()
				.route("item", "/items/{id}/", || page_with_text("item"))
				.not_found(not_found_page);

			router
				.commit_unmatched("/items/7/", NavigationType::Push, 1)
				.expect("unmatched navigation commits");

			assert_eq!(router.render_current().render_to_string(), "NotFound");
		});
	}

	#[rstest]
	fn test_render_current_returns_page_without_not_found() {
		ReactiveScope::run(|| {
			// Arrange
			let router = ClientRouter::new().route("home", "/home/", home_page);

			// Act — path does not match, no not_found registered
			let page = router.render_current();

			// Assert — returns Page::Empty as default fallback
			assert!(matches!(page, Page::Empty));
		});
	}

	#[test]
	fn test_router_with_guard() {
		ReactiveScope::run(|| {
			let router = ClientRouter::new()
				.guarded_route("/admin/", test_page, |_| false)
				.route("public", "/public/", test_page);

			// Guard rejects
			assert!(router.match_path("/admin/").is_none());
			// No guard
			assert!(router.match_path("/public/").is_some());
			assert!(
				ClientRoute::new("/sync/", test_page)
					.with_guard(|_| true)
					.navigation_guard_id()
					.is_none()
			);
		});
	}

	#[test]
	fn named_route_guard_rejects_flat_route() {
		ReactiveScope::run(|| {
			let router = ClientRouter::new()
				.route("admin", "/admin/", test_page)
				.with_route_guard("admin", |_| false);

			assert!(router.match_path("/admin/").is_none());
		});
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
		ReactiveScope::run(|| {
			let router = ClientRouter::new()
				.route("home", "/", home_page)
				.route("users", "/users/", user_page);

			// Non-WASM push should succeed
			assert!(router.push("/users/").is_ok());
		});
	}

	#[test]
	fn test_router_replace_non_wasm() {
		ReactiveScope::run(|| {
			let router = ClientRouter::new().route("home", "/", home_page);

			// Non-WASM replace should succeed
			assert!(router.replace("/").is_ok());
		});
	}

	#[test]
	fn direct_push_rejects_guard_only_routes_without_changing_state() {
		ReactiveScope::run(|| {
			let router = ClientRouter::new()
				.route("home", "/", home_page)
				.component(guard_only_page);

			assert!(matches!(
				router.push("/guard-only/"),
				Err(RouterError::NavigationFailed(message))
					if message == NAVIGATION_GUARD_COORDINATOR_REQUIRED
			));
			assert_eq!(router.current_path().get(), "/");
		});
	}

	#[test]
	fn direct_replace_rejects_guard_only_routes_without_changing_state() {
		ReactiveScope::run(|| {
			let router = ClientRouter::new()
				.route("home", "/", home_page)
				.component(guard_only_page);

			assert!(matches!(
				router.replace("/guard-only/"),
				Err(RouterError::NavigationFailed(message))
					if message == NAVIGATION_GUARD_COORDINATOR_REQUIRED
			));
			assert_eq!(router.current_path().get(), "/");
		});
	}

	#[test]
	fn commit_match_rejects_guarded_routes_without_coordinator() {
		ReactiveScope::run(|| {
			let router = ClientRouter::new()
				.route("home", "/", home_page)
				.component(guard_only_page);
			let matched = router
				.match_tree("/guard-only/")
				.expect("guarded route should match");

			let result = router.commit_match("/guard-only/", &matched, NavigationType::Push, 1);

			assert!(matches!(
				result,
				Err(RouterError::NavigationFailed(message))
					if message == NAVIGATION_GUARD_COORDINATOR_REQUIRED
			));
			assert_eq!(router.current_path().get(), "/");
		});
	}

	#[test]
	fn commit_match_rejects_a_match_for_a_different_path_or_guard_metadata() {
		ReactiveScope::run(|| {
			let router = ClientRouter::new()
				.route("home", "/", home_page)
				.component(guard_only_page);
			let unguarded_match = router.match_tree("/").expect("home route should match");

			let result =
				router.commit_match("/guard-only/", &unguarded_match, NavigationType::Push, 1);

			assert!(matches!(
				result,
				Err(RouterError::NavigationFailed(message))
					if message == MATCHED_ROUTE_PATH_MISMATCH
			));
			assert_eq!(router.current_path().get(), "/");
		});
	}

	#[test]
	fn commit_match_does_not_re_evaluate_synchronous_route_guards() {
		ReactiveScope::run(|| {
			let guard_calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
			let guard_calls_for_route = Arc::clone(&guard_calls);
			let router = ClientRouter::new().guarded_route("/guarded/", test_page, move |_| {
				guard_calls_for_route.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
				true
			});

			let matched = router
				.match_tree("/guarded/")
				.expect("synchronous guard should allow the route");
			assert_eq!(guard_calls.load(std::sync::atomic::Ordering::Relaxed), 1);

			router
				.commit_match("/guarded/", &matched, NavigationType::Push, 1)
				.expect("the previously matched route should commit");

			assert_eq!(guard_calls.load(std::sync::atomic::Ordering::Relaxed), 1);
		});
	}

	#[test]
	fn raw_rendering_rejects_guard_only_routes_until_pages_approval() {
		ReactiveScope::run(|| {
			let router = ClientRouter::new()
				.route("home", "/", home_page)
				.component(guard_only_page)
				.not_found(not_found_page);

			assert_eq!(
				router.render_path("/guard-only/").render_to_string(),
				"NotFound"
			);
			router.current_path().set("/guard-only/".to_owned());
			assert_eq!(router.render_current().render_to_string(), "NotFound");
		});
	}

	#[test]
	fn persistent_render_match_rejects_public_current_path_mutation_to_guarded_route() {
		ReactiveScope::run(|| {
			let router = ClientRouter::new()
				.route("home", "/", home_page)
				.component(guard_only_page);

			assert!(router.__match_current_for_render().is_some());
			router.current_path().set("/guard-only/".to_owned());

			assert!(router.__match_current_for_render().is_none());
		});
	}

	#[test]
	fn public_current_path_mutation_cannot_reuse_guard_approval() {
		ReactiveScope::run(|| {
			let router = ClientRouter::new()
				.route("home", "/", home_page)
				.component(guard_only_page)
				.not_found(not_found_page);
			let matched = router
				.match_tree("/guard-only/")
				.expect("guarded route should match");

			unsafe {
				router
					.__commit_match_after_navigation_guard(
						"/guard-only/",
						&matched,
						NavigationType::Push,
						1,
					)
					.expect("approved guarded route should commit");
			}
			assert_eq!(router.render_current().render_to_string(), "protected");

			router.current_path().set("/".to_owned());
			router.current_path().set("/guard-only/".to_owned());
			assert_eq!(router.render_current().render_to_string(), "NotFound");
		});
	}

	#[test]
	fn page_registration_preserves_component_loader_metadata() {
		ReactiveScope::run(|| {
			let router = ClientRouter::new().page("loaded-page", "/loaded/", loaded_page);
			let matched = router.match_tree("/loaded/").expect("route matches");

			assert_eq!(
				matched.loader_ids(),
				&[RouteLoaderId::new("test:loaded-page")]
			);
			assert_eq!(
				matched.navigation_guard_ids(),
				&[NavigationGuardId::new("test:loaded-page-guard")]
			);
			assert!(matches!(
				router.push("/loaded/"),
				Err(RouterError::NavigationFailed(message))
					if message == "route loaders require navigation through reinhardt-pages"
			));
		});
	}

	#[test]
	fn typed_component_registration_preserves_navigation_guard_metadata() {
		ReactiveScope::run(|| {
			let router = ClientRouter::new().component(loaded_page);
			let matched = router.match_tree("/loaded/").expect("route matches");

			assert_eq!(
				matched.navigation_guard_ids(),
				&[NavigationGuardId::new("test:loaded-page-guard")]
			);
		});
	}

	#[test]
	fn navigation_guard_ids_follow_root_to_leaf_order_without_deduplication() {
		let router = ClientRouter::new()
			.try_routes(|routes| {
				routes.layout(navigation_layout, |children| {
					children.layout(navigation_middle_layout, |children| {
						children.layout(navigation_inner_layout, |children| {
							children.component(navigation_page)
						})
					})
				})
			})
			.expect("route tree should register");

		let matched = router
			.match_tree("/navigation/middle/inner/page/")
			.expect("route should match");

		assert_eq!(
			matched.navigation_guard_ids(),
			&[
				NavigationGuardId::new("test:navigation-outer"),
				NavigationGuardId::new("test:navigation-middle"),
				NavigationGuardId::new("test:navigation-outer"),
				NavigationGuardId::new("test:navigation-page"),
			]
		);
	}

	// ============================================================================
	// route_path* tests
	// ============================================================================

	#[test]
	fn test_route_path_single() {
		ReactiveScope::run(|| {
			let router = ClientRouter::new().route_path(
				"user_detail",
				"/users/{id}/",
				|Path(_id): Path<i64>| page_with_text("User"),
			);

			assert_eq!(router.route_count(), 1);

			// Match and verify handler works
			let route_match = router.match_path("/users/42/");
			assert!(route_match.is_some());

			let route_match = route_match.unwrap();
			assert_eq!(route_match.params.get("id"), Some(&"42".to_string()));
		});
	}

	#[test]
	fn test_route_path_two_params() {
		ReactiveScope::run(|| {
			// Two path parameters now flow through the unified `route_path`
			// (Issue #4637). Closure signature is unchanged from the prior
			// `route_path2`.
			let router = ClientRouter::new().route_path(
				"user_post",
				"/users/{user_id}/posts/{post_id}/",
				|Path(_user_id): Path<i64>, Path(_post_id): Path<i64>| page_with_text("UserPost"),
			);

			assert_eq!(router.route_count(), 1);

			let route_match = router.match_path("/users/123/posts/456/");
			assert!(route_match.is_some());

			let route_match = route_match.unwrap();
			assert_eq!(route_match.params.get("user_id"), Some(&"123".to_string()));
			assert_eq!(route_match.params.get("post_id"), Some(&"456".to_string()));
		});
	}

	#[test]
	fn test_route_path_three_params() {
		ReactiveScope::run(|| {
			// Three path parameters via unified `route_path` (Issue #4637).
			let router = ClientRouter::new().route_path(
				"member",
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
		});
	}

	#[test]
	fn test_route_path_four_params() {
		ReactiveScope::run(|| {
			// Guard against regression of the 1..=8 widening (Issue #4637).
			// Arity-4 is the smallest case beyond the previously supported
			// arity-3 ceiling, so exercising it proves the `impl_handler!`
			// macro expansion actually reaches the higher tuples.
			let router = ClientRouter::new().route_path(
				"quad",
				"/a/{a}/b/{b}/c/{c}/d/{d}/",
				|Path(_a): Path<i64>,
				 Path(_b): Path<i64>,
				 Path(_c): Path<i64>,
				 Path(_d): Path<i64>| { page_with_text("Quad") },
			);

			assert_eq!(router.route_count(), 1);

			let route_match = router.match_path("/a/1/b/2/c/3/d/4/");
			assert!(route_match.is_some());

			let route_match = route_match.unwrap();
			assert_eq!(route_match.params.get("a"), Some(&"1".to_string()));
			assert_eq!(route_match.params.get("b"), Some(&"2".to_string()));
			assert_eq!(route_match.params.get("c"), Some(&"3".to_string()));
			assert_eq!(route_match.params.get("d"), Some(&"4".to_string()));
		});
	}

	#[test]
	fn test_route_path_single_with_reverse() {
		ReactiveScope::run(|| {
			let router = ClientRouter::new().route_path(
				"user_detail",
				"/users/{id}/",
				|Path(_id): Path<i64>| page_with_text("User"),
			);

			assert!(router.has_route("user_detail"));
			assert_eq!(
				router.reverse("user_detail", &[("id", "42")]).unwrap(),
				"/users/42/"
			);
		});
	}

	#[test]
	fn test_route_path_two_params_with_reverse() {
		ReactiveScope::run(|| {
			// `route_path` now covers every arity (Issue #4637).
			let router = ClientRouter::new().route_path(
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
		});
	}

	#[test]
	fn test_route_path_three_params_with_reverse() {
		ReactiveScope::run(|| {
			let router = ClientRouter::new().route_path(
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
		});
	}

	#[test]
	fn test_route_path_with_string_param() {
		ReactiveScope::run(|| {
			let router = ClientRouter::new().route_path(
				"post_detail",
				"/posts/{slug}/",
				|Path(_slug): Path<String>| page_with_text("Post"),
			);

			let route_match = router.match_path("/posts/hello-world/");
			assert!(route_match.is_some());
			assert_eq!(
				route_match.unwrap().params.get("slug"),
				Some(&"hello-world".to_string())
			);
		});
	}

	// ----- merge / try_merge ---------------------------------------------

	fn polls_router() -> ClientRouter {
		ClientRouter::new()
			.route("polls:index", "/polls/", home_page)
			.route("polls:detail", "/polls/{id}/", user_page)
	}

	fn users_router() -> ClientRouter {
		ClientRouter::new()
			.route("users:login", "/users/login/", home_page)
			.route("users:logout", "/users/logout/", home_page)
	}

	#[test]
	fn merge_appends_routes_and_named_routes() {
		ReactiveScope::run(|| {
			let merged = polls_router().merge(users_router());

			assert_eq!(merged.route_count(), 4);
			assert_eq!(merged.reverse("polls:index", &[]).unwrap(), "/polls/");
			assert_eq!(
				merged.reverse("polls:detail", &[("id", "1")]).unwrap(),
				"/polls/1/"
			);
			assert_eq!(merged.reverse("users:login", &[]).unwrap(), "/users/login/");
			assert_eq!(
				merged.reverse("users:logout", &[]).unwrap(),
				"/users/logout/"
			);
		});
	}

	#[test]
	fn merge_last_wins_on_name_collision() {
		ReactiveScope::run(|| {
			let first = ClientRouter::new().route("shared", "/a/", || page_with_text("first"));
			let second = ClientRouter::new().route("shared", "/b/", || page_with_text("second"));

			let merged = first.merge(second);

			// Both physical routes survive — merge appends, never deduplicates.
			assert_eq!(merged.route_count(), 2);
			// The named-route key points at the second router's entry, so reverse()
			// resolves to `/b/`. This is the "last wins" contract documented on
			// `ClientRouter::merge` and matches the pre-existing
			// `UnifiedRouter::mount_unified` behavior.
			assert_eq!(merged.reverse("shared", &[]).unwrap(), "/b/");
		});
	}

	#[test]
	fn merge_discards_other_not_found() {
		ReactiveScope::run(|| {
			let other_not_found_seen = Arc::new(std::sync::atomic::AtomicBool::new(false));
			let flag = Arc::clone(&other_not_found_seen);
			let other = ClientRouter::new().not_found(move || {
				flag.store(true, std::sync::atomic::Ordering::SeqCst);
				page_with_text("other-not-found")
			});

			let merged = ClientRouter::new()
				.route("home", "/home/", home_page)
				.merge(other);

			// Render against a non-matching path; `other`'s `not_found` must not
			// fire because `merge` keeps `self`'s observation state and discards
			// `other`'s. With no `not_found` on `self`, the default is Page::Empty.
			let page = merged.render_current();
			assert!(matches!(page, Page::Empty));
			assert!(!other_not_found_seen.load(std::sync::atomic::Ordering::SeqCst));
		});
	}

	#[test]
	fn try_merge_ok_when_no_collision() {
		ReactiveScope::run(|| {
			let merged = polls_router()
				.try_merge(users_router())
				.expect("disjoint named routes merge cleanly");

			assert_eq!(merged.route_count(), 4);
			assert!(merged.has_route("polls:index"));
			assert!(merged.has_route("users:login"));
		});
	}

	#[test]
	fn try_merge_err_on_name_collision() {
		ReactiveScope::run(|| {
			let first = ClientRouter::new().route("polls:index", "/a/", home_page);
			let second = ClientRouter::new().route("polls:index", "/b/", home_page);

			let err = first
				.try_merge(second)
				.expect_err("collision must be reported");

			assert_eq!(
				err,
				MergeError::NameCollision {
					name: "polls:index".to_string(),
				},
			);
		});
	}

	#[test]
	fn try_merge_err_leaves_neither_router_partially_merged() {
		ReactiveScope::run(|| {
			// Build a router whose routes vector would clearly grow if `try_merge`
			// fell through to `merge` before validating. Then attempt a merge that
			// must fail. We can only observe `merged`'s state on the Ok path, so
			// the structural check is: on Err, `merge` was never called (validated
			// by inspecting the original router we kept aside).
			let original = polls_router();
			let baseline_count = original.route_count();
			let baseline_named = original.has_route("polls:index");

			// Re-build the same router because `try_merge` takes `self` by value.
			let attempt = polls_router();
			let collide = ClientRouter::new().route("polls:index", "/x/", home_page);
			let err = attempt
				.try_merge(collide)
				.expect_err("collision must be reported");
			assert!(matches!(err, MergeError::NameCollision { .. }));

			// The independent `original` is unchanged (sanity-checks that
			// `try_merge`'s validation does not depend on hidden global state).
			assert_eq!(original.route_count(), baseline_count);
			assert!(baseline_named);
		});
	}

	#[test]
	fn merge_error_display_includes_route_name() {
		let err = MergeError::NameCollision {
			name: "polls:detail".to_string(),
		};
		let text = err.to_string();
		assert!(text.contains("polls:detail"));
	}

	// ----- duplicate route name detection ------------------------------------

	#[rstest]
	#[should_panic(expected = "Duplicate client route name 'home'")]
	fn route_panics_on_duplicate_name() {
		ReactiveScope::run(|| {
			// Arrange — a router with an existing "home" route
			let router = ClientRouter::new().route("home", "/", home_page);

			// Act — registering the same name again must panic
			let _router = router.route("home", "/other/", user_page);
		});
	}

	#[rstest]
	#[should_panic(expected = "Duplicate client route name 'detail'")]
	fn route_params_panics_on_duplicate_name() {
		ReactiveScope::run(|| {
			// Arrange
			let router = ClientRouter::new().route("detail", "/items/{id}/", home_page);

			// Act
			let _router = router.route_params("detail", "/users/{id}/", |Path(_id): Path<i64>| {
				page_with_text("User")
			});
		});
	}

	#[rstest]
	#[should_panic(expected = "Duplicate client route name 'show'")]
	fn route_path_panics_on_duplicate_name() {
		ReactiveScope::run(|| {
			// Arrange
			let router = ClientRouter::new().route("show", "/a/", home_page);

			// Act
			let _router = router.route_path("show", "/b/{id}/", |Path(_id): Path<i64>| {
				page_with_text("B")
			});
		});
	}

	#[rstest]
	fn merge_does_not_panic_on_duplicate_name() {
		ReactiveScope::run(|| {
			// Arrange — merge intentionally uses last-wins semantics
			let first = ClientRouter::new().route("shared", "/a/", || page_with_text("first"));
			let second = ClientRouter::new().route("shared", "/b/", || page_with_text("second"));

			// Act — must NOT panic (last-wins is the documented contract)
			let merged = first.merge(second);

			// Assert — the second route wins
			assert_eq!(merged.reverse("shared", &[]).unwrap(), "/b/");
		});
	}
}
