//! `ClientLauncher` builder, lifecycle contexts, and the `launch()` pipeline.

use reinhardt_urls::routers::ClientPathPattern;
use std::any::Any;
use std::cell::RefCell;
use std::collections::HashMap;

use crate::reactive::query::QueryDefaults;
#[cfg(wasm)]
use crate::reactive::query::{
	QueryClient, provide_query_client, with_query_client, with_query_client_async,
};
use crate::reactive::{Context, ContextGuard};

#[cfg(wasm)]
use super::link_interceptor::install_link_interceptor;
#[cfg(wasm)]
use super::{
	store_link_interceptor_guard, store_navigation_coordinator, store_popstate_subscription,
	store_spa_router, with_spa_router,
};
#[cfg(wasm)]
use crate::component::reactive_if::{
	ReactiveNodeStore, clear_reactive_node_store, new_reactive_node_store, with_reactive_node_store,
};
#[cfg(any(wasm, test))]
use crate::component::{IntoPage as _, Page, PageElement};
#[cfg(wasm)]
use crate::component::{MountError, PageExt as _};
#[cfg(wasm)]
use crate::document_head::{
	DocumentHeadManager, ensure_browser_document_head_manager, with_document_head_manager,
};
#[cfg(any(wasm, test))]
use crate::router::loader::RouteLoaderError;
#[cfg(wasm)]
use crate::router::loader::{
	LoaderStore, active_loader_store, loader_cache_id_with_optional_queries, route_context,
	with_loader_store,
};
#[cfg(wasm)]
use crate::router::loader_registry::LoaderRegistry;
#[cfg(wasm)]
use reinhardt_core::page::Outlet;
#[cfg(wasm)]
use reinhardt_urls::routers::client_router::history::normalize_initial_state;
#[cfg(wasm)]
use reinhardt_urls::routers::client_router::{
	ClientRouteTreeMatch, ClientRouter, HistoryState, LayoutKey, listen_pop_requests,
};

#[cfg(wasm)]
thread_local! {
	/// Cumulative count of `ClientLauncher::render_and_mount` invocations
	/// since the WASM module loaded. Backs `ClientLauncher::__diag_render_count()`.
	/// Hidden diagnostic counter for testing — Refs #4122.
	static RENDER_COUNT: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
	static PERSISTENT_LAYOUT_RENDERER: RefCell<PersistentLayoutRenderer> =
		RefCell::new(PersistentLayoutRenderer::new());
	static ACTIVE_APP_ROOT: RefCell<Option<web_sys::Element>> = const { RefCell::new(None) };
	static ROOT_CONTEXT_GUARDS: RefCell<Vec<Box<dyn Any>>> = const { RefCell::new(Vec::new()) };
}

type RootContextProvider = Box<dyn FnOnce() -> Box<dyn Any>>;

#[cfg(wasm)]
struct PersistentLayoutRenderer {
	layout_keys: Vec<LayoutKey>,
	layout_loader_keys: Vec<Option<String>>,
	layout_stores: Vec<ReactiveNodeStore>,
	layout_loader_stores: Vec<LoaderStore>,
	leaf_store: Option<ReactiveNodeStore>,
	leaf_loader_store: Option<LoaderStore>,
}

#[cfg(wasm)]
impl PersistentLayoutRenderer {
	fn new() -> Self {
		Self {
			layout_keys: Vec::new(),
			layout_loader_keys: Vec::new(),
			layout_stores: Vec::new(),
			layout_loader_stores: Vec::new(),
			leaf_store: None,
			leaf_loader_store: None,
		}
	}

	fn reset(&mut self) {
		for store in self.layout_stores.drain(..) {
			clear_reactive_node_store(&store);
		}
		if let Some(store) = self.leaf_store.take() {
			clear_reactive_node_store(&store);
		}
		self.layout_loader_stores.clear();
		self.leaf_loader_store = None;
		self.layout_keys.clear();
		self.layout_loader_keys.clear();
	}

	fn clear_from_layout_depth(&mut self, depth: usize) {
		for store in self.layout_stores.drain(depth..) {
			clear_reactive_node_store(&store);
		}
		if let Some(store) = self.leaf_store.take() {
			clear_reactive_node_store(&store);
		}
		self.layout_loader_stores.truncate(depth);
		self.leaf_loader_store = None;
		self.layout_keys.truncate(depth);
		self.layout_loader_keys.truncate(depth);
	}

	fn render(
		&mut self,
		root_el: &web_sys::Element,
		router: &ClientRouter,
		document_head_manager: &DocumentHeadManager,
	) -> Result<bool, MountError> {
		let Some(route_match) = router.__match_current_for_render() else {
			self.reset();
			return Ok(false);
		};
		if route_match.layouts().is_empty() {
			self.reset();
			return Ok(false);
		}

		let next_keys = route_match
			.layouts()
			.iter()
			.map(|layout| layout.key().clone())
			.collect::<Vec<_>>();
		let loader_context = route_context(&route_match);
		let registry = LoaderRegistry::global().ok();
		let next_loader_keys = route_match
			.layouts()
			.iter()
			.map(|layout| {
				let query_key = format!(
					"route-query:{}?{}",
					layout.key().full_pattern(),
					route_match.query().unwrap_or_default()
				);
				if let Some(id) = layout.metadata().loader_id() {
					let cache_key = registry.as_ref().and_then(|registry| {
						registry.get(id).ok().and_then(|registration| {
							loader_cache_id_with_optional_queries(
								id,
								&loader_context,
								registration.inputs,
								registry.optional_query_inputs(id),
							)
							.ok()
						})
					});
					let cache_key = cache_key.unwrap_or_else(|| {
						format!(
							"route-loader:{}:{}?{}",
							id.as_str(),
							route_match.path(),
							route_match.query().unwrap_or_default()
						)
					});
					Some(format!("{cache_key}:{query_key}"))
				} else {
					Some(query_key)
				}
			})
			.collect::<Vec<_>>();
		let mut preserved = common_layout_prefix_len(&self.layout_keys, &next_keys);
		preserved = preserved.min(common_loader_prefix_len(
			&self.layout_loader_keys,
			&next_loader_keys,
		));
		if preserved > 0 && Self::find_outlet(root_el, preserved - 1).is_err() {
			preserved = 0;
		}

		if preserved == 0 {
			self.reset();
			crate::component::cleanup_reactive_nodes();
			root_el.set_inner_html("");
		} else {
			self.clear_from_layout_depth(preserved);
			let outlet = Self::find_outlet(root_el, preserved - 1)?;
			outlet.set_inner_html("");
		}

		self.mount_suffix(
			root_el,
			router,
			&route_match,
			preserved,
			document_head_manager,
		)?;
		self.layout_keys = next_keys;
		self.layout_loader_keys = next_loader_keys;
		Ok(true)
	}

	fn mount_suffix(
		&mut self,
		root_el: &web_sys::Element,
		router: &ClientRouter,
		route_match: &ClientRouteTreeMatch,
		start_depth: usize,
		document_head_manager: &DocumentHeadManager,
	) -> Result<(), MountError> {
		let mut parent = if start_depth == 0 {
			root_el.clone()
		} else {
			Self::find_outlet(root_el, start_depth - 1)?
		};
		let loader_store = active_loader_store().unwrap_or_default();

		for depth in start_depth..route_match.layouts().len() {
			let outlet_id = Self::outlet_id(depth);
			let store = new_reactive_node_store();
			let scope = reinhardt_core::reactive::ReactiveScope::new();
			let parent_wrapper = crate::dom::Element::new(parent.clone());
			let mounted = with_document_head_manager(document_head_manager, || {
				with_loader_store(&loader_store, || {
					scope.enter(|| {
						with_reactive_node_store(&store, || {
							// SAFETY: `route_match` was produced by the navigation
							// coordinator for the committed route, after all guards allowed.
							let page = unsafe {
								router.__render_tree_layout(
									route_match,
									depth,
									Outlet::placeholder(outlet_id),
								)
							}
							.ok_or(MountError::CreateElementFailed)?;
							page.mount(&parent_wrapper)
						})
					})
				})
			});
			mounted?;
			with_reactive_node_store(&store, || {
				crate::component::store_reactive_scope(scope);
			});
			self.layout_stores.push(store);
			self.layout_loader_stores.push(loader_store.clone());
			parent = Self::find_outlet(root_el, depth)?;
		}

		let leaf_store = new_reactive_node_store();
		let leaf_scope = reinhardt_core::reactive::ReactiveScope::new();
		let parent_wrapper = crate::dom::Element::new(parent);
		let mounted = with_document_head_manager(document_head_manager, || {
			with_loader_store(&loader_store, || {
				leaf_scope.enter(|| {
					with_reactive_node_store(&leaf_store, || {
						// SAFETY: `route_match` was produced by the navigation
						// coordinator for the committed route, after all guards allowed.
						let leaf = unsafe { router.__render_tree_leaf(route_match) }
							.ok_or(MountError::CreateElementFailed)?;
						leaf.mount(&parent_wrapper)
					})
				})
			})
		});
		mounted?;
		with_reactive_node_store(&leaf_store, || {
			crate::component::store_reactive_scope(leaf_scope);
		});
		self.leaf_store = Some(leaf_store);
		self.leaf_loader_store = Some(loader_store);
		Ok(())
	}

	fn outlet_id(depth: usize) -> String {
		format!("__reinhardt_layout_outlet_{depth}")
	}

	fn find_outlet(
		root_el: &web_sys::Element,
		depth: usize,
	) -> Result<web_sys::Element, MountError> {
		let selector = format!("[data-rh-outlet-id=\"{}\"]", Self::outlet_id(depth));
		root_el
			.query_selector(&selector)
			.map_err(|_| MountError::CreateElementFailed)?
			.ok_or(MountError::AppendChildFailed)
	}
}

#[cfg(wasm)]
pub(super) fn clear_mounted_route_for_authentication_change() {
	PERSISTENT_LAYOUT_RENDERER.with(|renderer| renderer.borrow_mut().reset());
	crate::component::cleanup_reactive_nodes();
	ACTIVE_APP_ROOT.with(|root| {
		if let Some(root) = root.borrow().as_ref() {
			root.set_inner_html("");
		}
	});
}

#[cfg(wasm)]
fn common_layout_prefix_len(previous: &[LayoutKey], next: &[LayoutKey]) -> usize {
	previous
		.iter()
		.zip(next)
		.take_while(|(previous, next)| previous == next)
		.count()
}

#[cfg(wasm)]
fn common_loader_prefix_len(previous: &[Option<String>], next: &[Option<String>]) -> usize {
	previous
		.iter()
		.zip(next)
		.take_while(|(previous, next)| previous == next)
		.count()
}

/// WASM client application launcher.
///
/// Encapsulates all client-side startup boilerplate: panic hook, reactive
/// scheduler, DOM mounting, `Router::on_navigate` listeners for route
/// changes, history listener, and built-in SPA link interception. Optional
/// lifecycle hooks (`before_launch`, `after_launch`) and path-driven
/// side effects (`on_path`, `on_path_pattern`) plug into the builder
/// chain so app-level wiring stays declarative.
///
/// # Example
///
/// ```ignore
/// use reinhardt::pages::{ClientLauncher, LaunchCtx, PathCtx};
/// use wasm_bindgen::prelude::*;
///
/// #[wasm_bindgen(start)]
/// pub fn main() -> Result<(), JsValue> {
///     ClientLauncher::new("#root")
///         .before_launch(|| {
///             // Runs after the panic hook + reactive scheduler are
///             // configured but BEFORE the router is initialised.
///             my_app::state::init_app_state();
///         })
///         .router(router::init_router)
///         .after_launch(|ctx: &LaunchCtx<'_>| {
///             // Runs after the first DOM mount; router is live here.
///             my_app::analytics::report_boot(ctx.document());
///         })
///         .on_path("/", |ctx: &PathCtx<'_>| {
///             // Idempotent body-level mount + side effect on entering "/".
///             ctx.ensure_portal("toast-container", components::toast::container);
///             my_app::ws::connect_notifications();
///         })
///         .on_path_pattern("/orgs/{slug}/", |ctx| {
///             // Re-fires when {slug} changes within the same pattern.
///             my_app::analytics::track_view(ctx.params());
///         })
///         // SPA link interception is enabled by default. Pass false to
///         // opt out if your app installs its own document click handler:
///         // .intercept_links(false)
///         .launch()
/// }
/// ```
pub struct ClientLauncher {
	#[cfg_attr(not(wasm), allow(dead_code))]
	pub(super) root_selector: &'static str,
	#[cfg_attr(not(wasm), allow(dead_code))]
	pub(super) query_defaults: QueryDefaults,
	/// Optional `ClientRouter` initialiser registered via
	/// [`ClientLauncher::router_client`]. Mutually exclusive with
	/// `launch()` rejects the launcher if neither source is set.
	/// (Refs #4234, #4453)
	#[cfg_attr(not(wasm), allow(dead_code))]
	pub(super) client_router_init:
		Option<Box<dyn FnOnce() -> reinhardt_urls::routers::ClientRouter>>,
	#[cfg_attr(not(wasm), allow(dead_code))]
	pub(super) intercept_links: bool,
	#[cfg_attr(not(wasm), allow(dead_code))]
	pub(super) before_launch_hooks: Vec<BeforeLaunchHook>,
	#[cfg_attr(not(wasm), allow(dead_code))]
	pub(super) root_context_providers: Vec<RootContextProvider>,
	#[cfg_attr(not(wasm), allow(dead_code))]
	pub(super) after_launch_hooks: Vec<AfterLaunchHook>,
	#[cfg_attr(not(wasm), allow(dead_code))]
	pub(super) path_subscriptions: Vec<PathSubscription>,
	/// (Refs #4453) `true` when `register_routes_from_inventory()` has been
	/// called. Mutually exclusive with `client_router_init`.
	#[cfg_attr(not(wasm), allow(dead_code))]
	pub(super) use_inventory: bool,
}

/// Context passed to [`ClientLauncher::after_launch`] callbacks.
///
/// Borrows the resources `ClientLauncher::launch` already owns
/// (`window`, `document`, root element); never owns them.
pub struct LaunchCtx<'a> {
	#[cfg_attr(not(wasm), allow(dead_code))]
	window: &'a web_sys::Window,
	#[cfg_attr(not(wasm), allow(dead_code))]
	document: &'a web_sys::Document,
	#[cfg_attr(not(wasm), allow(dead_code))]
	root_element: &'a web_sys::Element,
}

impl<'a> LaunchCtx<'a> {
	/// The browser `window` object.
	pub fn window(&self) -> &web_sys::Window {
		self.window
	}

	/// The current `document`.
	pub fn document(&self) -> &web_sys::Document {
		self.document
	}

	/// The element matched by the launcher's root selector (e.g. `#root`).
	pub fn root_element(&self) -> &web_sys::Element {
		self.root_element
	}
}

/// A one-shot callback invoked before the router is initialised.
type BeforeLaunchHook = Box<dyn FnOnce()>;

/// A one-shot callback invoked after the first DOM mount, receiving a
/// borrow of the launcher's [`LaunchCtx`].
type AfterLaunchHook = Box<dyn FnOnce(&LaunchCtx<'_>)>;

/// Path parameters extracted from a route match.
///
/// Re-exposed as a type alias so the public API does not leak the
/// `HashMap` constructor at call sites; users can write
/// `fn handle(ctx: &PathCtx) { let id = ctx.params().get("id"); }`.
pub type PathParams = HashMap<String, String>;

/// Context passed to [`ClientLauncher::on_path`] /
/// [`ClientLauncher::on_path_pattern`] callbacks.
///
/// Borrows the current `document`, the matched path string, and the
/// extracted path parameters; never owns them.
pub struct PathCtx<'a> {
	#[cfg_attr(not(wasm), allow(dead_code))]
	document: &'a web_sys::Document,
	path: &'a str,
	params: &'a PathParams,
}

impl<'a> PathCtx<'a> {
	/// The current `document`. Only useful in WASM builds; on the host
	/// the underlying type is still defined but no real DOM exists.
	pub fn document(&self) -> &web_sys::Document {
		self.document
	}

	/// The currently active route location (e.g. `"/orgs/foo/?tab=activity"`).
	///
	/// [`ClientLauncher::on_path`] and [`ClientLauncher::on_path_pattern`]
	/// match only the pathname, but callbacks retain the full location so they
	/// can read the active query when needed.
	pub fn path(&self) -> &str {
		self.path
	}

	/// Path parameters extracted by the matched pattern (e.g. `{ "slug": "foo" }`).
	///
	/// For exact-match `on_path` registrations this is always empty.
	pub fn params(&self) -> &PathParams {
		self.params
	}

	/// Idempotent body-level mount.
	///
	/// If `document.getElementById(id)` already returns an element,
	/// this is a no-op. Otherwise renders `factory()` and appends the
	/// resulting root element to `document.body`.
	///
	/// Useful for installing app-level overlays (toast containers,
	/// modals) in `on_path` callbacks without hand-rolling the
	/// "mount once" guard.
	#[cfg(wasm)]
	pub fn ensure_portal<F>(&self, id: &str, factory: F)
	where
		F: FnOnce() -> crate::component::Page,
	{
		if self.document.get_element_by_id(id).is_some() {
			return;
		}

		let page = factory();
		let html = page.render_to_string();

		let Some(body) = self.document.body() else {
			return;
		};
		let Ok(wrapper) = self.document.create_element("div") else {
			return;
		};
		wrapper.set_inner_html(&html);

		// Convention: factory output is a single root element. Append the
		// first element child if present so the caller's `id` lands on the
		// outermost element they wrote; fall back to the wrapper otherwise
		// so something is always inserted.
		if let Some(child) = wrapper.first_element_child() {
			let _ = body.append_child(&child);
		} else {
			let _ = body.append_child(&wrapper);
		}
	}

	/// Host-side stub for `ensure_portal`. No DOM exists, so the call
	/// is a no-op.
	#[cfg(not(wasm))]
	pub fn ensure_portal<F>(&self, _id: &str, _factory: F)
	where
		F: FnOnce() -> crate::component::Page,
	{
	}
}

/// Internal record produced by every `.on_path` / `.on_path_pattern` call.
///
/// `last_params` tracks the previous match state so the `on_navigate`
/// listener can detect transitions (entering a match, or a parameter-set
/// change inside the same pattern) without re-firing on every navigation.
pub(super) struct PathSubscription {
	#[cfg_attr(not(wasm), allow(dead_code))]
	pub(super) pattern: ClientPathPattern,
	#[cfg_attr(not(wasm), allow(dead_code))]
	pub(super) callback: Box<dyn Fn(&PathCtx<'_>) + 'static>,
	#[cfg_attr(not(wasm), allow(dead_code))]
	pub(super) last_params: RefCell<Option<HashMap<String, String>>>,
}

/// Diff state machine shared by `on_path` / `on_path_pattern`
/// subscriptions.
///
/// Evaluates `pattern` against the pathname component of `path`, updates
/// `last_params` with the
/// new match (or `None`) regardless of the transition, and returns
/// `Some(new_params)` only on transitions that should fire the user
/// callback:
///
/// - `None -> Some` (entering a match): fires.
/// - `Some(a) -> Some(b)` where `a != b` (param change inside the
///   same pattern): fires.
/// - `Some(_) -> None` (leaving a match): does not fire.
/// - `None -> None` (still unmatched): does not fire.
///
/// Extracted so the launcher's Phase C path-subscription registration
/// (`launch()`) and its native unit tests share a single diff
/// implementation; the same helper is invoked once at registration to
/// deliver the bootstrap route and again from each `Router::on_navigate`
/// dispatch (Refs #4101).
fn pathname_for_path_subscription(path: &str) -> &str {
	path.split(['?', '#']).next().unwrap_or(path)
}

/// Match a subscription against a route's pathname while preserving the full
/// route location for [`PathCtx::path`].
// Native production builds do not install router observers; native unit tests
// and the WASM launcher both exercise this helper.
#[allow(dead_code)]
fn next_path_subscription_match(
	pattern: &ClientPathPattern,
	path: &str,
	last_params: &RefCell<Option<HashMap<String, String>>>,
) -> Option<HashMap<String, String>> {
	let new_match: Option<HashMap<String, String>> = pattern
		.matches(pathname_for_path_subscription(path))
		.map(|(p, _)| p);
	let should_fire = {
		let mut prev = last_params.borrow_mut();
		let fire = match (&*prev, &new_match) {
			(None, Some(_)) => true,
			(Some(_), None) => false,
			(Some(a), Some(b)) => a != b,
			(None, None) => false,
		};
		*prev = new_match.clone();
		fire
	};
	if should_fire { new_match } else { None }
}

/// The router source `launch()` will use, decided from the three
/// mutually-exclusive launcher methods.
///
/// Used only by `launch()` (WASM-only) and the native test module that
/// exercises the pure decision logic. On non-test native builds the
/// items below are intentionally unreferenced.
///
/// Refs #4453.
#[cfg_attr(not(wasm), allow(dead_code))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RouterSourceChoice {
	/// `router_client(...)` was the only source set.
	Client,
	/// `register_routes_from_inventory()` was the only source set.
	Inventory,
	/// No source was set.
	None,
	/// More than one source was set.
	Conflict,
}

/// Decide which router source `launch()` will use, based on which of the
/// three launcher methods were called. Returns `Conflict` if more than one
/// was called; `None` if none were called.
///
/// Pure function — testable on the native target without the WASM runtime.
///
/// Refs #4453.
#[cfg_attr(not(wasm), allow(dead_code))]
fn select_router_source_counts(client: bool, inventory: bool) -> RouterSourceChoice {
	match (client as u8) + (inventory as u8) {
		0 => RouterSourceChoice::None,
		1 if client => RouterSourceChoice::Client,
		1 if inventory => RouterSourceChoice::Inventory,
		_ => RouterSourceChoice::Conflict,
	}
}

impl ClientLauncher {
	/// Create a new launcher targeting the given CSS selector (e.g. `"#root"`).
	pub fn new(root_selector: &'static str) -> Self {
		Self {
			root_selector,
			query_defaults: QueryDefaults::default(),
			client_router_init: None,
			intercept_links: true,
			before_launch_hooks: Vec::new(),
			root_context_providers: Vec::new(),
			after_launch_hooks: Vec::new(),
			path_subscriptions: Vec::new(),
			use_inventory: false,
		}
	}

	/// Configures the defaults used by the application query client.
	pub fn query_defaults(mut self, defaults: QueryDefaults) -> Self {
		self.query_defaults = defaults;
		self
	}

	/// Provide a root context for the lifetime of the launched application.
	///
	/// The context is installed after the reactive scheduler is configured and
	/// before any `before_launch` callback or router construction. Its RAII guard
	/// is retained for later SPA navigations. If launching fails, the guard is
	/// dropped and the context is removed automatically.
	pub fn provide_context<T>(mut self, context: &Context<T>, value: T) -> Self
	where
		T: Clone + 'static,
	{
		let context = *context;
		self.root_context_providers.push(Box::new(move || {
			Box::new(ContextGuard::new(&context, value))
		}));
		self
	}

	/// Provide an i18n context for the lifetime of the launched application.
	///
	/// This is the fresh-CSR counterpart to hydration's retained i18n context.
	/// The context is available to the initial render, callbacks, and later SPA
	/// route renders without an application-owned global guard.
	#[cfg(feature = "i18n")]
	pub fn i18n_context(mut self, context: crate::i18n::I18nContext) -> Self {
		self.root_context_providers.push(Box::new(move || {
			Box::new(crate::i18n::provide_i18n_context(context))
		}));
		self
	}

	/// Register a [`reinhardt_urls::routers::ClientRouter`] factory function.
	pub fn router_client<F>(mut self, f: F) -> Self
	where
		F: FnOnce() -> reinhardt_urls::routers::ClientRouter + 'static,
	{
		self.client_router_init = Some(Box::new(f));
		self
	}

	/// Pull the SPA route table from all `#[routes]`-annotated functions
	/// registered via `inventory` at compile time.
	///
	/// This is the canonical Reinhardt SPA bootstrap pattern:
	///
	/// ```rust,ignore
	/// // src/config/urls.rs
	/// use reinhardt::routes;
	/// use reinhardt::UnifiedRouter;
	///
	/// #[routes]
	/// pub fn routes() -> UnifiedRouter {
	///     UnifiedRouter::new()
	///         .server(|s| s /* ... */)
	///         .client(|c| c /* ... */)
	/// }
	///
	/// // src/client/lib.rs
	/// #[wasm_bindgen(start)]
	/// pub fn main() -> Result<(), JsValue> {
	///     ClientLauncher::new("#root")
	///         .register_routes_from_inventory()
	///         .launch()
	/// }
	/// ```
	///
	/// # Conflict
	///
	/// Mutually exclusive with [`Self::router_client`]. Calling more
	/// than one on the same launcher causes `launch()` to return
	/// an error. Calling none of them also returns an error.
	/// (`launch` is `#[cfg(wasm)]`, so it cannot be linked from native docs.)
	///
	/// `launch()` returns an error if no `#[routes]` registration is found
	/// at runtime (i.e. the `inventory` iterator is empty).
	pub fn register_routes_from_inventory(mut self) -> Self {
		self.use_inventory = true;
		self
	}

	/// Toggle built-in SPA link interception.
	///
	/// When enabled (the default), `launch()` installs a document-level
	/// `click` listener that converts clicks on internal `<a href="/...">`
	/// anchors into `Router::push` navigations, so a full page reload is
	/// avoided.
	///
	/// The listener intentionally skips:
	/// - external URLs (`href` not starting with `/`)
	/// - `target="_blank"`
	/// - `download` attribute
	/// - `rel="external"` (whitespace-split, case-insensitive)
	/// - clicks with Ctrl/Cmd/Shift modifier keys (so users can still
	///   open links in a new tab/window)
	///
	/// Pass `false` to opt out — applications that already install their
	/// own document-level link handler should disable this to avoid
	/// double-handling.
	pub fn intercept_links(mut self, enabled: bool) -> Self {
		self.intercept_links = enabled;
		self
	}

	/// Register a callback to run **before** the router is initialised.
	///
	/// `before_launch` callbacks fire after the panic hook and reactive
	/// scheduler are configured but before `router_init` is called, so
	/// they are the right place for state initialisation that components
	/// will read during their first render.
	///
	/// Multiple calls accumulate in registration order.
	pub fn before_launch<F>(mut self, hook: F) -> Self
	where
		F: FnOnce() + 'static,
	{
		self.before_launch_hooks.push(Box::new(hook));
		self
	}

	/// Register a callback to run **after** the first DOM mount.
	///
	/// `after_launch` callbacks fire after the initial render has been
	/// mounted to the root element. The callback receives a [`LaunchCtx`]
	/// with borrows of the `window`, `document`, and root element that
	/// `launch()` already owns. The router is fully initialised at this
	/// point, so [`with_spa_router`](crate::app::with_spa_router) is safe to call.
	/// When an unhydrated initial route loader delays that first mount, callbacks
	/// wait until the loader-backed route commits and mounts successfully.
	///
	/// Multiple calls accumulate in registration order.
	pub fn after_launch<F>(mut self, hook: F) -> Self
	where
		F: FnOnce(&LaunchCtx<'_>) + 'static,
	{
		self.after_launch_hooks.push(Box::new(hook));
		self
	}

	/// Register a side effect that fires on transitions into `path` (exact match).
	///
	/// The callback receives a [`PathCtx`] with the current document and full
	/// route location; matching ignores the query string, and for exact-match
	/// registrations `params()` is always empty.
	///
	/// Internally each registration becomes a leaked `Router::on_navigate`
	/// listener; the callback fires when the application enters the matching
	/// path and is independent of the reactive `Effect` / `Signal`
	/// auto-tracking system. It does **not** fire on repeated navigations
	/// to the same path. Leaving the path does not fire the callback either
	/// — register a separate `on_path` for the destination if you need exit
	/// cleanup.
	pub fn on_path<F>(mut self, path: &'static str, callback: F) -> Self
	where
		F: Fn(&PathCtx<'_>) + 'static,
	{
		self.path_subscriptions.push(PathSubscription {
			pattern: ClientPathPattern::new(path).expect("valid path pattern"),
			callback: Box::new(callback),
			last_params: RefCell::new(None),
		});
		self
	}

	/// Register a side effect that fires on transitions into any path
	/// matching `pattern`.
	///
	/// The pattern syntax is the same as `Router::route` (e.g.
	/// `"/orgs/{slug}/"` or `"/static/{path:*}/"`). The callback fires
	/// when:
	/// - the app enters a path that matches the pattern, OR
	/// - the path still matches but the extracted parameters changed
	///   (e.g. `/orgs/foo/` → `/orgs/bar/`).
	///
	/// Re-renders that do not change the path leave the callback dormant.
	pub fn on_path_pattern<F>(mut self, pattern: &'static str, callback: F) -> Self
	where
		F: Fn(&PathCtx<'_>) + 'static,
	{
		self.path_subscriptions.push(PathSubscription {
			pattern: ClientPathPattern::new(pattern).expect("valid path pattern"),
			callback: Box::new(callback),
			last_params: RefCell::new(None),
		});
		self
	}
}

#[cfg(any(wasm, test))]
fn initial_loader_error_page(error: Option<RouteLoaderError>) -> Page {
	let Some(error) = error else {
		return Page::Empty;
	};
	PageElement::new("div")
		.attr("data-route-error", "loader")
		.child(error.public_message().to_owned())
		.into_page()
}

#[cfg(wasm)]
impl ClientLauncher {
	fn mount_initial_loader_error_surface(
		root_el: &web_sys::Element,
		document_head_manager: &DocumentHeadManager,
	) -> Result<(), crate::component::MountError> {
		with_document_head_manager(document_head_manager, || {
			crate::component::cleanup_reactive_nodes();
			let scope = reinhardt_core::reactive::ReactiveScope::new();
			let page = Page::reactive(move || {
				let error = crate::app::try_with_navigation_coordinator(|coordinator| {
					coordinator.error().get()
				})
				.flatten();
				initial_loader_error_page(error)
			});
			root_el.set_inner_html("");
			let root = crate::dom::Element::new(root_el.clone());
			let result = scope.enter(|| page.mount(&root));
			if result.is_ok() {
				crate::component::store_reactive_scope(scope);
			}
			result
		})
	}

	/// Render the current route into the given root element.
	///
	/// Performs `cleanup_reactive_nodes` -> `Router::render_current` ->
	/// clears `innerHTML` -> `view.mount`. Used by `launch()` for both
	/// the initial mount (called inline in Phase B) and every
	/// subsequent re-mount (called from a `Router::on_navigate`
	/// listener registered in Phase C).
	///
	/// Refs #4101.
	fn render_and_mount(
		root_el: &web_sys::Element,
		document_head_manager: &DocumentHeadManager,
	) -> Result<(), crate::component::MountError> {
		with_document_head_manager(document_head_manager, || {
			RENDER_COUNT.with(|c| c.set(c.get() + 1));
			let mounted_loader_store = crate::app::try_with_navigation_coordinator(|coordinator| {
				coordinator.mounted_store()
			})
			.flatten();
			let client_router =
				with_spa_router(|r| r.as_any().downcast_ref::<ClientRouter>().cloned());
			if let Some(router) = client_router {
				let render_layouts = || {
					PERSISTENT_LAYOUT_RENDERER.with(|renderer| {
						renderer
							.borrow_mut()
							.render(root_el, &router, document_head_manager)
					})
				};
				let handled_by_layout_renderer = if let Some(store) = mounted_loader_store.as_ref()
				{
					with_loader_store(store, render_layouts)
				} else {
					render_layouts()
				}?;
				if handled_by_layout_renderer {
					crate::app::observe_viewport_prefetch_links();
					return Ok(());
				}
			}

			// Refs #5104: tear down the previous route's reactive graph before
			// constructing the next route. Route construction can create forms,
			// resources, and reactive blocks that synchronously touch signals; if
			// stale route effects are still alive, those signal notifications can
			// re-enter the runtime and abort the navigation before DOM remount.
			crate::component::cleanup_reactive_nodes();
			let scope = reinhardt_core::reactive::ReactiveScope::new();
			let render_current = || {
				let view = with_spa_router(|r| r.render_current());
				root_el.set_inner_html("");
				let wrapper = crate::dom::Element::new(root_el.clone());
				view.mount(&wrapper)
			};
			let result = if let Some(store) = mounted_loader_store.as_ref() {
				with_loader_store(store, || scope.enter(render_current))
			} else {
				scope.enter(render_current)
			};
			if result.is_ok() {
				crate::component::store_reactive_scope(scope);
				crate::app::observe_viewport_prefetch_links();
			}
			result
		})
	}

	/// Diagnostic counter: cumulative count of `render_and_mount`
	/// invocations since the WASM module loaded. Includes the initial
	/// Phase B mount and every subsequent `Router::on_navigate`-driven
	/// re-mount. Used by tests in
	/// `tests/wasm/spa_navigation_diag_test.rs` to assert Inv-4.
	///
	/// Only available under `cfg(wasm)`: the entire `impl ClientLauncher`
	/// block in this scope is gated, and `RENDER_COUNT` is a `cfg(wasm)`-
	/// only thread_local. Calling from a non-wasm32 build results in a
	/// compile error rather than a no-op zero. Hidden API for testing
	/// only. Refs #4122.
	#[doc(hidden)]
	pub fn __diag_render_count() -> u64 {
		RENDER_COUNT.with(|c| c.get())
	}

	/// Run the lifecycle work that requires a successfully mounted initial route.
	///
	/// Path subscriptions intentionally retain their router observers for the
	/// application lifetime. A WASM module has no application shutdown phase, so
	/// the subscriptions are leaked after registration just as the render
	/// observer is in [`Self::launch`].
	fn activate_post_mount_lifecycle(
		after_launch_hooks: Vec<AfterLaunchHook>,
		path_subscriptions: Vec<PathSubscription>,
		window: &web_sys::Window,
		document: &web_sys::Document,
		root_el: &web_sys::Element,
		scope: &std::rc::Rc<reinhardt_core::reactive::ReactiveScope>,
		query_client: &QueryClient,
	) {
		let ctx = LaunchCtx {
			window,
			document,
			root_element: root_el,
		};
		for hook in after_launch_hooks {
			hook(&ctx);
		}

		for sub in path_subscriptions {
			let PathSubscription {
				pattern,
				callback,
				last_params,
			} = sub;
			let pattern: std::rc::Rc<ClientPathPattern> = std::rc::Rc::new(pattern);
			let callback: std::rc::Rc<dyn Fn(&PathCtx<'_>) + 'static> = std::rc::Rc::from(callback);
			let last_params: std::rc::Rc<RefCell<Option<HashMap<String, String>>>> =
				std::rc::Rc::new(last_params);

			let pattern_for_listener = pattern.clone();
			let callback_for_listener = callback.clone();
			let last_params_for_listener = last_params.clone();
			let document_for_listener = document.clone();
			let scope_for_listener = std::rc::Rc::clone(scope);
			let query_client_for_listener = query_client.clone();
			let listener_subscription = with_spa_router(|r| {
				r.on_navigate_dyn(Box::new(move |path, _params_from_router| {
					with_query_client(&query_client_for_listener, || {
						if let Some(params) = next_path_subscription_match(
							&pattern_for_listener,
							path,
							&last_params_for_listener,
						) {
							let ctx = PathCtx {
								document: &document_for_listener,
								path,
								params: &params,
							};
							scope_for_listener.enter(|| callback_for_listener(&ctx));
						}
					});
				}))
			});
			std::mem::forget(listener_subscription);

			let initial_path = with_spa_router(|r| r.current_path().get());
			if let Some(params) =
				next_path_subscription_match(&pattern, &initial_path, &last_params)
			{
				let ctx = PathCtx {
					document,
					path: &initial_path,
					params: &params,
				};
				scope.enter(|| callback(&ctx));
			}
		}
	}

	/// Start the WASM client application.
	///
	/// The launcher owns an application-lifetime reactive scope for setup,
	/// router state, and persistent subscriptions. Individual route mounts use
	/// separate scopes that are disposed when their rendered nodes are cleaned up.
	///
	/// Performs three phases in order:
	///
	/// 1. **Phase A — Setup.** Sets up the panic hook for readable
	///    console errors, configures the reactive scheduler for async
	///    contexts, runs registered `before_launch` callbacks,
	///    initialises the [`Router`] and stores it in the global
	///    thread-local, registers the `popstate` history listener,
	///    queries the DOM for `root_selector` (returns `Err` if not
	///    found), and installs the SPA link-interception listener on
	///    `document` when `intercept_links` is `true` (the default).
	///
	/// 2. **Phase B — Initial mount.** Inline (no `Effect`):
	///    `cleanup_reactive_nodes()` -> `Router::render_current()` ->
	///    clears `innerHTML` -> `view.mount()`. On mount failure
	///    `launch()` returns `Err` and Phase C is skipped.
	///
	/// 3. **Phase C — Persistent subscriptions.** Registers the
	///    launcher's render listener via [`Router::on_navigate`] (the
	///    returned [`NavigationSubscription`] is leaked via
	///    `mem::forget` so it persists for the WASM module lifetime),
	///    then runs registered `after_launch` callbacks and registers
	///    one `Router::on_navigate` listener per `on_path` /
	///    `on_path_pattern` subscription. Each path-subscription
	///    listener fires only on transitions into or between matching
	///    param sets (de-duplicated through a
	///    `RefCell<Option<HashMap>>` state). If an unhydrated initial
	///    route loader defers Phase B, the post-mount lifecycle waits
	///    for that loader-backed route to commit and mount successfully.
	///
	/// The launcher does **not** create any reactive `Effect`. The
	/// render pipeline is driven entirely by [`Router::on_navigate`]
	/// callbacks, which are independent of the reactive runtime. This
	/// is structurally robust against nested reactive nodes spawned
	/// during view rendering (Refs #3348, #4075, #4088, #4101).
	///
	/// [`Router::on_navigate`] fires for both programmatic navigation
	/// (`Router::push` / `Router::replace`) and browser back/forward
	/// (popstate).
	///
	/// # Router Source Selection
	///
	/// Exactly one of the two router-source builders must be called
	/// before `launch()`:
	///
	/// - [`Self::router_client`] (closure producing a `ClientRouter`)
	/// - [`Self::register_routes_from_inventory`] (pulls a `ClientRouter`
	///   from `inventory`-registered `#[routes]` functions)
	///
	/// Calling none of them or more than one returns `Err`. The
	/// `register_routes_from_inventory` path additionally returns `Err`
	/// when no `#[routes]` registrations are found at runtime.
	/// (Refs #4453)
	pub fn launch(self) -> Result<(), wasm_bindgen::JsValue> {
		let scope = std::rc::Rc::new(reinhardt_core::reactive::ReactiveScope::new());
		let stored_scope = std::rc::Rc::clone(&scope);
		scope.enter(move || self.launch_in_scope(stored_scope))
	}

	fn launch_in_scope(
		mut self,
		scope: std::rc::Rc<reinhardt_core::reactive::ReactiveScope>,
	) -> Result<(), wasm_bindgen::JsValue> {
		#[cfg(feature = "console_error_panic_hook")]
		console_error_panic_hook::set_once();

		let query_client = QueryClient::new(self.query_defaults.clone());
		crate::reactive::runtime::set_scheduler(|task| {
			if let Some(client) = crate::reactive::query::current_query_client() {
				wasm_bindgen_futures::spawn_local(with_query_client_async(client, async move {
					task()
				}));
			} else {
				wasm_bindgen_futures::spawn_local(async move { task() });
			}
		});

		let query_client_activation = provide_query_client(query_client.clone());
		let mut root_context_guards: Vec<Box<dyn Any>> = vec![
			Box::new(query_client.clone()),
			Box::new(query_client_activation),
		];
		root_context_guards.extend(
			self.root_context_providers
				.drain(..)
				.map(|provider| provider()),
		);

		// Step 3: drain before_launch callbacks before any router or DOM work.
		for hook in self.before_launch_hooks.drain(..) {
			hook();
		}

		// (Refs #4453) Pick exactly one router source. The
		// `router_client(...)` builder produces a `ClientRouter` from
		// a user-supplied closure; `register_routes_from_inventory()`
		// pulls a `ClientRouter` from `inventory`-registered
		// `#[routes]` functions at module load time.
		let client_init = self.client_router_init.take();
		let use_inventory = self.use_inventory;

		let spa_router: Box<dyn super::SpaRouter> =
			match select_router_source_counts(client_init.is_some(), use_inventory) {
				RouterSourceChoice::Client => {
					Box::new((client_init.expect("Client variant guarantees Some"))())
				}
				RouterSourceChoice::Inventory => {
					match reinhardt_urls::routers::collect_client_router_from_inventory() {
						Some(router) => Box::new(router),
						None => {
							return Err(wasm_bindgen::JsValue::from_str(
								"ClientLauncher::register_routes_from_inventory: no \
							 `#[routes]` registrations found. Ensure the project has \
							 a `#[routes]`-annotated function returning a \
							 `UnifiedRouter` with client routes.",
							));
						}
					}
				}
				RouterSourceChoice::None => {
					return Err(wasm_bindgen::JsValue::from_str(
						"ClientLauncher: `router_client(...)`, or \
					 `register_routes_from_inventory()` must be called before \
					 `launch()`.",
					));
				}
				RouterSourceChoice::Conflict => {
					return Err(wasm_bindgen::JsValue::from_str(
						"ClientLauncher: `router_client(...)`, and \
					 `register_routes_from_inventory()` are mutually exclusive; \
					 configure exactly one.",
					));
				}
			};
		store_spa_router(spa_router, std::rc::Rc::clone(&scope));
		let window = web_sys::window()
			.ok_or_else(|| wasm_bindgen::JsValue::from_str("no global `window`"))?;
		let document = window
			.document()
			.ok_or_else(|| wasm_bindgen::JsValue::from_str("no document on window"))?;
		let document_head_manager = ensure_browser_document_head_manager().map_err(|error| {
			wasm_bindgen::JsValue::from_str(&format!(
				"document-head manager initialization failed: {error}"
			))
		})?;
		let mut coordinator_installed = false;
		let mut initial_preparation_path = None;
		if let Some(router) =
			with_spa_router(|router| router.as_any().downcast_ref::<ClientRouter>().cloned())
		{
			let coordinator =
				super::navigation::NavigationCoordinator::new(std::rc::Rc::new(router.clone()))
					.map_err(|error| {
						wasm_bindgen::JsValue::from_str(&format!(
							"route-loader coordinator initialization failed: {error}"
						))
					})?;
			let initial_path = with_spa_router(|router| router.current_path().get());
			let proposed_initial_state = router
				.match_tree(&initial_path)
				.map(|matched| {
					let leaf = matched.leaf_match();
					let mut state =
						HistoryState::new(initial_path.clone()).with_params(leaf.params.clone());
					if let Some(name) = leaf.route.name() {
						state = state.with_route_name(name);
					}
					state
				})
				.unwrap_or_else(|| HistoryState::new(initial_path.clone()));
			let initial_state =
				normalize_initial_state(proposed_initial_state).map_err(|error| {
					wasm_bindgen::JsValue::from_str(&format!(
						"initial history state normalization failed: {error}"
					))
				})?;
			coordinator.initialize_committed_index(initial_state.entry_index().unwrap_or(0));
			let initial_store_hydrated = coordinator
				.hydrate_initial_store(&query_client, &initial_path)
				.map_err(|error| {
					wasm_bindgen::JsValue::from_str(&format!(
						"initial route-loader hydration failed: {error}"
					))
				})?;
			if !initial_store_hydrated {
				initial_preparation_path = Some(initial_path.clone());
			}
			let pop_coordinator = std::rc::Rc::clone(&coordinator);
			let pop_query_client = query_client.clone();
			let pop_subscription = listen_pop_requests(move |request| {
				with_query_client(&pop_query_client, || {
					if pop_coordinator.consume_restoration_pop() {
						return;
					}
					let target_index = request.state.entry_index();
					let _ = pop_coordinator
						.navigate(request.path, super::NavigationIntent::Pop { target_index });
				});
			})?;
			store_navigation_coordinator(coordinator);
			store_popstate_subscription(pop_subscription);
			coordinator_installed = true;
		}

		crate::nav_diag!(
			"site=store_router router_id={} route_count={}",
			with_spa_router(|r| r.__diag_router_id()),
			with_spa_router(|r| r.route_count())
		);
		crate::nav_diag_dom!("store_router");

		if !coordinator_installed {
			with_spa_router(|r| r.setup_history_listener());
		}
		#[cfg(feature = "hmr")]
		crate::hmr::HmrBridge::new().install(&document)?;
		if self.intercept_links {
			let guard = install_link_interceptor(&document)?;
			store_link_interceptor_guard(guard);
		}

		let root_el = document
			.query_selector(self.root_selector)?
			.ok_or_else(|| {
				wasm_bindgen::JsValue::from_str(&format!(
					"element '{}' not found",
					self.root_selector
				))
			})?;
		ACTIVE_APP_ROOT.with(|root| {
			root.borrow_mut().replace(root_el.clone());
		});
		with_document_head_manager(&document_head_manager, || {
			PERSISTENT_LAYOUT_RENDERER.with(|renderer| renderer.borrow_mut().reset());
		});
		let pending_post_mount_lifecycle = std::rc::Rc::new(RefCell::new(Some((
			std::mem::take(&mut self.after_launch_hooks),
			std::mem::take(&mut self.path_subscriptions),
		))));

		// Phase B: initial mount runs inline (no Effect). Errors
		// propagate directly because no Effect/Signal indirection
		// captures them. Refs #4101.
		if initial_preparation_path.is_none() {
			Self::render_and_mount(&root_el, &document_head_manager).map_err(|e| {
				wasm_bindgen::JsValue::from_str(&format!("initial mount failed: {e}"))
			})?;
		}

		// Phase C (part 1): register the launcher's render listener
		// via Router::on_navigate before post-mount lifecycle activation.
		// This preserves the previous ordering for navigation triggered from
		// an after_launch hook and defers that lifecycle until an initially
		// unhydrated loader route has committed and mounted. Router::on_navigate
		// fires for both programmatic navigation and popstate (popstate dispatch
		// added in #4108).
		//
		// The subscription is leaked for the entire WASM module
		// lifetime (modules never terminate, so there is no
		// destructor to run). Refs #4101, #4108, #4088.
		let render_root = root_el.clone();
		let lifecycle_for_render = std::rc::Rc::clone(&pending_post_mount_lifecycle);
		let window_for_render = window.clone();
		let document_for_render = document.clone();
		let scope_for_render = std::rc::Rc::clone(&scope);
		let document_head_manager_for_render = document_head_manager.clone();
		let query_client_for_render = query_client.clone();
		let render_subscription = with_spa_router(|r| {
			r.on_navigate_dyn(Box::new(move |_path, _params| {
				with_query_client(&query_client_for_render, || {
					match Self::render_and_mount(&render_root, &document_head_manager_for_render) {
						Ok(()) => {
							if let Some((after_launch_hooks, path_subscriptions)) =
								lifecycle_for_render.borrow_mut().take()
							{
								Self::activate_post_mount_lifecycle(
									after_launch_hooks,
									path_subscriptions,
									&window_for_render,
									&document_for_render,
									&render_root,
									&scope_for_render,
									&query_client_for_render,
								);
							}
						}
						Err(error) => {
							web_sys::console::error_1(&format!("re-render failed: {error}").into());
						}
					}
				});
			}))
		});
		std::mem::forget(render_subscription);

		if let Some(path) = initial_preparation_path {
			Self::mount_initial_loader_error_surface(&root_el, &document_head_manager).map_err(
				|error| {
					wasm_bindgen::JsValue::from_str(&format!(
						"initial loader error surface failed to mount: {error}"
					))
				},
			)?;
			let result = crate::app::try_with_navigation_coordinator(|coordinator| {
				coordinator.navigate(path, super::NavigationIntent::Initial)
			});
			if let Some(Err(error)) = result {
				return Err(wasm_bindgen::JsValue::from_str(&format!(
					"initial route-loader preparation failed to start: {error}"
				)));
			}
		} else if let Some((after_launch_hooks, path_subscriptions)) =
			pending_post_mount_lifecycle.borrow_mut().take()
		{
			Self::activate_post_mount_lifecycle(
				after_launch_hooks,
				path_subscriptions,
				&window,
				&document,
				&root_el,
				&scope,
				&query_client,
			);
		}

		crate::nav_diag!(
			"site=register_render_listener router_id={} observer_count_after={}",
			with_spa_router(|r| r.__diag_router_id()),
			with_spa_router(|r| r.__diag_observer_count())
		);

		ROOT_CONTEXT_GUARDS.with(|guards| guards.borrow_mut().extend(root_context_guards));

		Ok(())
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn initial_loader_error_page_renders_a_safe_failure_surface() {
		assert_eq!(initial_loader_error_page(None).render_to_string(), "");
		let page = initial_loader_error_page(Some(RouteLoaderError::new("initial loader failed")));
		assert_eq!(
			page.render_to_string(),
			"<div data-route-error=\"loader\">initial loader failed</div>"
		);
	}
	use rstest::*;

	#[rstest]
	fn test_client_launcher_new_stores_selector() {
		let launcher = ClientLauncher::new("#root");

		assert_eq!(launcher.root_selector, "#root");
		assert_eq!(launcher.query_defaults, QueryDefaults::default());
		assert!(launcher.client_router_init.is_none());
	}

	#[test]
	fn query_defaults_replaces_the_application_client_defaults() {
		let defaults = QueryDefaults::new()
			.stale_time(std::time::Duration::from_secs(5))
			.gc_time(std::time::Duration::from_secs(60));

		let launcher = ClientLauncher::new("#root").query_defaults(defaults.clone());

		assert_eq!(launcher.query_defaults, defaults);
	}

	// (Refs #4234) Mirrors `test_client_launcher_router_stores_init_fn`
	// for the new `router_client(...)` builder.
	#[rstest]
	fn test_client_launcher_router_client_stores_init_fn() {
		let launcher = ClientLauncher::new("#root");

		let launcher = launcher.router_client(reinhardt_urls::routers::ClientRouter::new);

		assert!(launcher.client_router_init.is_some());
	}

	#[rstest]
	fn test_client_launcher_intercept_links_default_true() {
		// Arrange / Act
		let launcher = ClientLauncher::new("#root");

		// Assert
		assert!(launcher.intercept_links);
	}

	#[rstest]
	fn test_client_launcher_intercept_links_false_overrides_default() {
		// Arrange / Act
		let launcher = ClientLauncher::new("#root").intercept_links(false);

		// Assert
		assert!(!launcher.intercept_links);
	}

	// --- before_launch / after_launch builder tests ---

	#[rstest]
	fn test_before_launch_starts_empty() {
		// Arrange / Act
		let launcher = ClientLauncher::new("#root");
		// Assert
		assert!(launcher.before_launch_hooks.is_empty());
	}

	#[rstest]
	fn root_context_providers_start_empty() {
		// Arrange / Act
		let launcher = ClientLauncher::new("#root");

		// Assert
		assert!(launcher.root_context_providers.is_empty());
	}

	#[rstest]
	fn provide_context_installs_context_when_launch_setup_runs() {
		// Arrange
		let context = crate::reactive::Context::new();
		let mut launcher = ClientLauncher::new("#root").provide_context(&context, 42);

		// Act
		let guards = launcher
			.root_context_providers
			.drain(..)
			.map(|provider| provider())
			.collect::<Vec<_>>();

		// Assert
		assert_eq!(crate::reactive::get_context(&context), Some(42));
		drop(guards);
		assert_eq!(crate::reactive::get_context(&context), None);
	}

	#[rstest]
	#[cfg(feature = "i18n")]
	fn i18n_context_installs_context_when_launch_setup_runs() {
		// Arrange
		let context = crate::i18n::I18nContext::empty("en-US", "en-US");
		let mut launcher = ClientLauncher::new("#root").i18n_context(context.clone());

		// Act
		let guards = launcher
			.root_context_providers
			.drain(..)
			.map(|provider| provider())
			.collect::<Vec<_>>();

		// Assert
		assert_eq!(
			crate::i18n::use_i18n_context().map(|context| context.locale()),
			Some(context.locale())
		);
		drop(guards);
		assert!(crate::i18n::use_i18n_context().is_none());
	}

	#[rstest]
	fn test_before_launch_accumulates_in_registration_order() {
		// Arrange / Act
		let launcher = ClientLauncher::new("#root")
			.before_launch(|| { /* hook 1 */ })
			.before_launch(|| { /* hook 2 */ })
			.before_launch(|| { /* hook 3 */ });
		// Assert
		assert_eq!(launcher.before_launch_hooks.len(), 3);
	}

	#[rstest]
	fn test_after_launch_starts_empty() {
		// Arrange / Act
		let launcher = ClientLauncher::new("#root");
		// Assert
		assert!(launcher.after_launch_hooks.is_empty());
	}

	#[rstest]
	fn test_after_launch_accumulates_in_registration_order() {
		// Arrange / Act
		let launcher = ClientLauncher::new("#root")
			.after_launch(|_ctx: &LaunchCtx<'_>| { /* hook 1 */ })
			.after_launch(|_ctx: &LaunchCtx<'_>| { /* hook 2 */ });
		// Assert
		assert_eq!(launcher.after_launch_hooks.len(), 2);
	}

	// --- on_path / on_path_pattern builder tests ---

	#[rstest]
	fn test_path_subscriptions_start_empty() {
		// Arrange / Act
		let launcher = ClientLauncher::new("#root");
		// Assert
		assert!(launcher.path_subscriptions.is_empty());
	}

	#[rstest]
	fn test_on_path_appends_exact_subscription() {
		// Arrange / Act
		let launcher = ClientLauncher::new("#root")
			.on_path("/", |_ctx: &PathCtx<'_>| {})
			.on_path("/users/", |_ctx: &PathCtx<'_>| {});
		// Assert
		assert_eq!(launcher.path_subscriptions.len(), 2);
		assert!(launcher.path_subscriptions[0].pattern.is_exact());
		assert_eq!(launcher.path_subscriptions[0].pattern.pattern(), "/");
		assert!(launcher.path_subscriptions[1].pattern.is_exact());
		assert_eq!(launcher.path_subscriptions[1].pattern.pattern(), "/users/");
	}

	#[rstest]
	fn test_on_path_pattern_appends_pattern_subscription() {
		// Arrange / Act
		let launcher =
			ClientLauncher::new("#root").on_path_pattern("/orgs/{slug}/", |_ctx: &PathCtx<'_>| {});
		// Assert
		assert_eq!(launcher.path_subscriptions.len(), 1);
		let sub = &launcher.path_subscriptions[0];
		assert!(!sub.pattern.is_exact());
		assert!(sub.pattern.matches("/orgs/foo/").is_some());
		assert!(sub.pattern.matches("/orgs/").is_none());
	}

	#[rstest]
	fn test_on_path_subscriptions_start_with_no_recorded_match() {
		// Arrange / Act
		let launcher = ClientLauncher::new("#root").on_path("/", |_ctx: &PathCtx<'_>| {});
		// Assert: last_params is None at registration time so the very
		// first listener invocation will be detected as a `None -> Some(_)` transition.
		assert!(
			launcher.path_subscriptions[0]
				.last_params
				.borrow()
				.is_none()
		);
	}

	#[rstest]
	fn path_subscriptions_match_the_pathname_when_the_route_has_a_query() {
		// Arrange
		let pattern = ClientPathPattern::new("/query-loaded").expect("valid path pattern");
		let last_params = RefCell::new(None);

		// Act
		let matched =
			next_path_subscription_match(&pattern, "/query-loaded?tab=initial", &last_params);

		// Assert
		assert_eq!(matched, Some(HashMap::new()));
	}

	#[rstest]
	fn path_subscriptions_match_the_pathname_when_the_route_has_a_fragment() {
		let pattern = ClientPathPattern::new("/query-loaded").expect("valid path pattern");
		let last_params = RefCell::new(None);

		let matched = next_path_subscription_match(&pattern, "/query-loaded#details", &last_params);

		assert_eq!(matched, Some(HashMap::new()));
	}

	// --- transition logic regression test ---

	/// Mirrors the per-subscription transition decision used inside
	/// `launch()` so we can exercise the `Cell<bool>` -> `RefCell<HashMap>`
	/// upgrade rationale on the host (where no router exists).
	fn fire_decision(
		prev: &Option<HashMap<String, String>>,
		new: &Option<HashMap<String, String>>,
	) -> bool {
		match (prev, new) {
			(None, Some(_)) => true,
			(Some(_), None) => false,
			(Some(a), Some(b)) => a != b,
			(None, None) => false,
		}
	}

	#[rstest]
	fn test_transition_logic_fires_on_initial_match() {
		// Arrange
		let prev = None;
		let new = Some(HashMap::new());
		// Act / Assert
		assert!(fire_decision(&prev, &new));
	}

	#[rstest]
	fn test_transition_logic_skips_re_render_at_same_path() {
		// Arrange
		let mut params = HashMap::new();
		params.insert("slug".into(), "foo".into());
		let prev = Some(params.clone());
		let new = Some(params);
		// Act / Assert
		assert!(!fire_decision(&prev, &new));
	}

	#[rstest]
	fn test_transition_logic_fires_when_pattern_params_change() {
		// Arrange
		let mut a = HashMap::new();
		a.insert("slug".into(), "foo".into());
		let mut b = HashMap::new();
		b.insert("slug".into(), "bar".into());
		let prev = Some(a);
		let new = Some(b);
		// Act / Assert
		assert!(fire_decision(&prev, &new));
	}

	#[rstest]
	fn test_transition_logic_does_not_fire_when_leaving_match() {
		// Arrange
		let mut params = HashMap::new();
		params.insert("slug".into(), "foo".into());
		let prev = Some(params);
		let new = None;
		// Act / Assert
		assert!(!fire_decision(&prev, &new));
	}

	// --- Phase C path-subscription registration regression tests ---

	/// Mirror of the launcher's Phase C (part 2) path-subscription
	/// registration algorithm: register an on_navigate listener that
	/// runs the diff-and-fire helper, then immediately evaluate the
	/// current path through the same helper so an already-matching
	/// bootstrap route delivers its callback at registration time.
	///
	/// Returns the `NavigationSubscription` so the test can `mem::forget`
	/// it (matching the launcher) or drop it to unregister.

	#[rstest]
	fn test_lifecycle_hooks_observe_registration_order() {
		// Arrange: shared counter that records the call order.
		let trace = std::rc::Rc::new(std::cell::RefCell::new(Vec::<u32>::new()));

		let t = trace.clone();
		let h1 = move || t.borrow_mut().push(1);
		let t = trace.clone();
		let h2 = move || t.borrow_mut().push(2);

		// Act
		let launcher = ClientLauncher::new("#root")
			.before_launch(h1)
			.before_launch(h2);

		// Drain the recorded hooks like `launch()` would, on the host.
		// We can read the field directly because the test lives in the same
		// module.
		for hook in launcher.before_launch_hooks {
			hook();
		}

		// Assert
		assert_eq!(*trace.borrow(), vec![1, 2]);
	}
}

#[cfg(test)]
mod select_router_source_tests {
	use super::*;

	#[rstest::rstest]
	fn returns_none_when_no_source_set() {
		// Arrange
		let client = false;
		let inventory = false;
		// Act
		let result = select_router_source_counts(client, inventory);
		// Assert
		assert_eq!(result, RouterSourceChoice::None);
	}

	#[rstest::rstest]
	fn returns_client_when_only_client_set() {
		// Arrange + Act
		let result = select_router_source_counts(true, false);
		// Assert
		assert_eq!(result, RouterSourceChoice::Client);
	}

	#[rstest::rstest]
	fn returns_inventory_when_only_inventory_set() {
		// Arrange + Act
		let result = select_router_source_counts(false, true);
		// Assert
		assert_eq!(result, RouterSourceChoice::Inventory);
	}

	#[rstest::rstest]
	#[case(true, true)]
	fn returns_conflict_when_multiple_sources_set(#[case] client: bool, #[case] inventory: bool) {
		// Arrange + Act
		let result = select_router_source_counts(client, inventory);
		// Assert
		assert_eq!(result, RouterSourceChoice::Conflict);
	}
}
