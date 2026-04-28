//! WASM client application launcher.

use crate::router::{PathPattern, Router};
use std::cell::RefCell;
use std::collections::HashMap;

#[cfg(wasm)]
use crate::component::PageExt as _;

thread_local! {
	static APP_ROUTER: RefCell<Option<Router>> = const { RefCell::new(None) };
}

/// Access the globally registered client router.
///
/// # Panics
///
/// Panics if `ClientLauncher::launch` has not been called yet.
pub fn with_router<F, R>(f: F) -> R
where
	F: FnOnce(&Router) -> R,
{
	APP_ROUTER.with(|r| {
		f(r.borrow()
			.as_ref()
			.expect("Router not initialized. Call ClientLauncher::launch() first."))
	})
}

#[cfg(wasm)]
fn store_router(router: Router) {
	APP_ROUTER.with(|r| {
		*r.borrow_mut() = Some(router);
	});
}

/// WASM client application launcher.
///
/// Encapsulates all client-side startup boilerplate: panic hook, reactive
/// scheduler, DOM mounting, reactive `Effect` for route changes,
/// history listener, and built-in SPA link interception. Optional
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
	root_selector: &'static str,
	router_init: Option<Box<dyn FnOnce() -> Router>>,
	#[cfg_attr(not(wasm), allow(dead_code))]
	intercept_links: bool,
	#[cfg_attr(not(wasm), allow(dead_code))]
	before_launch_hooks: Vec<BeforeLaunchHook>,
	#[cfg_attr(not(wasm), allow(dead_code))]
	after_launch_hooks: Vec<AfterLaunchHook>,
	#[cfg_attr(not(wasm), allow(dead_code))]
	path_subscriptions: Vec<PathSubscription>,
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

	/// The currently active path (e.g. `"/orgs/foo/"`).
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
/// `last_params` tracks the previous match state so the Effect can
/// detect transitions (entering a match, or a parameter-set change
/// inside the same pattern) without re-firing on every render.
struct PathSubscription {
	#[cfg_attr(not(wasm), allow(dead_code))]
	pattern: PathPattern,
	#[cfg_attr(not(wasm), allow(dead_code))]
	callback: Box<dyn Fn(&PathCtx<'_>) + 'static>,
	#[cfg_attr(not(wasm), allow(dead_code))]
	last_params: RefCell<Option<HashMap<String, String>>>,
}

impl ClientLauncher {
	/// Create a new launcher targeting the given CSS selector (e.g. `"#root"`).
	pub fn new(root_selector: &'static str) -> Self {
		Self {
			root_selector,
			router_init: None,
			intercept_links: true,
			before_launch_hooks: Vec::new(),
			after_launch_hooks: Vec::new(),
			path_subscriptions: Vec::new(),
		}
	}

	/// Register the router initializer function.
	///
	/// The function is called once during `launch()` before the first render.
	pub fn router<F: FnOnce() -> Router + 'static>(mut self, f: F) -> Self {
		self.router_init = Some(Box::new(f));
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
	/// point, so [`with_router`] is safe to call.
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
	/// The callback receives a [`PathCtx`] with the current document and
	/// path; for exact-match registrations, `params()` is always empty.
	///
	/// Internally each registration becomes a leaked reactive `Effect`,
	/// so callers never write `std::mem::forget` themselves. The Effect
	/// fires when the application enters the matching path; it does
	/// **not** fire on every render of the same path. Leaving the path
	/// does not fire the callback either — register a separate
	/// `on_path` for the destination if you need exit cleanup.
	pub fn on_path<F>(mut self, path: &'static str, callback: F) -> Self
	where
		F: Fn(&PathCtx<'_>) + 'static,
	{
		self.path_subscriptions.push(PathSubscription {
			pattern: PathPattern::new(path),
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
			pattern: PathPattern::new(pattern),
			callback: Box::new(callback),
			last_params: RefCell::new(None),
		});
		self
	}
}

#[cfg(wasm)]
impl ClientLauncher {
	/// Start the WASM client application.
	///
	/// Performs in order:
	/// 1. Sets up the panic hook for readable console errors
	/// 2. Configures the reactive scheduler for async contexts
	/// 3. Runs registered `before_launch` callbacks in registration order
	/// 4. Initialises the router and stores it in the global thread-local
	/// 5. Registers the `popstate` history listener (browser back/forward)
	/// 6. Queries the DOM for `root_selector`; returns `Err` if not found
	/// 7. Installs the SPA link-interception listener on `document`
	///    (when `intercept_links` is `true`, the default)
	/// 8. Initial render: `render_current()` → `cleanup_reactive_nodes()` → `mount()`
	/// 9. Runs registered `after_launch` callbacks in registration order,
	///    each receiving a [`LaunchCtx`] borrowing `window`, `document`,
	///    and the root element
	/// 10. Creates a reactive `Effect` that re-renders on route changes;
	///    leaks it intentionally so it persists for the application lifetime
	/// 11. Registers one leaked reactive `Effect` per `on_path` /
	///    `on_path_pattern` subscription; each fires only on transitions
	///    into a matching path (not on every render)
	pub fn launch(mut self) -> Result<(), wasm_bindgen::JsValue> {
		#[cfg(feature = "console_error_panic_hook")]
		console_error_panic_hook::set_once();

		crate::reactive::runtime::set_scheduler(|task| {
			wasm_bindgen_futures::spawn_local(async move { task() });
		});

		// Step 3: drain before_launch callbacks before any router or DOM work.
		for hook in self.before_launch_hooks.drain(..) {
			hook();
		}

		let router = self
			.router_init
			.expect("ClientLauncher::router() must be called before launch()")();
		store_router(router);

		with_router(|r| r.setup_history_listener());

		let window = web_sys::window()
			.ok_or_else(|| wasm_bindgen::JsValue::from_str("no global `window`"))?;
		let document = window
			.document()
			.ok_or_else(|| wasm_bindgen::JsValue::from_str("no document on window"))?;

		if self.intercept_links {
			install_link_interceptor(&document)?;
		}

		let root_el = document
			.query_selector(self.root_selector)
			.map_err(|e| e)?
			.ok_or_else(|| {
				wasm_bindgen::JsValue::from_str(&format!(
					"element '{}' not found",
					self.root_selector
				))
			})?;

		let view = with_router(|r| {
			let _ = r.current_path().get();
			let _ = r.current_params().get();
			r.render_current()
		});
		crate::component::cleanup_reactive_nodes();
		root_el.set_inner_html("");
		let wrapper = crate::dom::Element::new(root_el.clone());
		view.mount(&wrapper)
			.map_err(|e| wasm_bindgen::JsValue::from_str(&format!("mount failed: {e:?}")))?;

		// Step 9: drain after_launch callbacks now that the router is live and
		// the first DOM mount has completed.
		if !self.after_launch_hooks.is_empty() {
			let ctx = LaunchCtx {
				window: &window,
				document: &document,
				root_element: &root_el,
			};
			for hook in self.after_launch_hooks.drain(..) {
				hook(&ctx);
			}
		}

		let root_clone = root_el.clone();
		let _effect = crate::reactive::Effect::new(move || {
			let view = with_router(|r| {
				let _ = r.current_path().get();
				let _ = r.current_params().get();
				r.render_current()
			});
			crate::component::cleanup_reactive_nodes();
			root_clone.set_inner_html("");
			let wrapper = crate::dom::Element::new(root_clone.clone());
			if let Err(e) = view.mount(&wrapper) {
				web_sys::console::error_1(&format!("re-render failed: {e:?}").into());
			}
		});
		// Intentional leak: Effect must persist for the entire application lifetime.
		// WASM modules never terminate, so there is no destructor to run.
		std::mem::forget(_effect);

		// Step 11: register one leaked Effect per path subscription. Each
		// Effect re-reads the router's `current_path` Signal, so the
		// reactive system wakes them on navigation.
		for sub in self.path_subscriptions.into_iter() {
			let PathSubscription {
				pattern,
				callback,
				last_params,
			} = sub;
			let document_for_effect = document.clone();

			let sub_effect = crate::reactive::Effect::new(move || {
				// Subscribe to the path Signal — Signal::get() registers
				// this Effect as a dependent.
				let path_string: String = with_router(|r| r.current_path().get());

				let new_match: Option<HashMap<String, String>> =
					pattern.matches(&path_string).map(|(p, _)| p);

				// Compare against the previous match state to detect
				// transitions; release the borrow before invoking user code.
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

				if should_fire && let Some(params) = new_match {
					let ctx = PathCtx {
						document: &document_for_effect,
						path: &path_string,
						params: &params,
					};
					callback(&ctx);
				}
			});
			std::mem::forget(sub_effect);
		}

		Ok(())
	}
}

/// Anchor attributes relevant to the link interceptor decision.
///
/// Extracted into a plain struct so the decision logic in
/// [`should_intercept`] stays a pure function and can be unit-tested on
/// the host without a real DOM.
#[cfg_attr(not(any(wasm, test)), allow(dead_code))]
struct AnchorAttrs<'a> {
	has_modifier_key: bool,
	href: Option<&'a str>,
	target: Option<&'a str>,
	has_download: bool,
	rel: Option<&'a str>,
}

/// Decide whether the link interceptor should hijack a click.
///
/// Returns `Some(href)` if the click should be turned into a SPA push,
/// or `None` to let the browser handle the click normally.
#[cfg_attr(not(any(wasm, test)), allow(dead_code))]
fn should_intercept<'a>(attrs: &AnchorAttrs<'a>) -> Option<&'a str> {
	if attrs.has_modifier_key {
		return None;
	}
	let href = attrs.href?;
	// Internal link: starts with `/` but not `//` (protocol-relative URLs are
	// treated as external by the browser).
	if !href.starts_with('/') || href.starts_with("//") {
		return None;
	}
	if attrs.target == Some("_blank") {
		return None;
	}
	if attrs.has_download {
		return None;
	}
	if let Some(rel) = attrs.rel
		&& rel
			.split_ascii_whitespace()
			.any(|w| w.eq_ignore_ascii_case("external"))
	{
		return None;
	}
	Some(href)
}

/// Install a document-level click listener that converts clicks on internal
/// `<a href="/...">` anchors into `Router::push` navigations.
///
/// Skips external links, `target="_blank"`, `download`, `rel="external"`,
/// and modifier-key clicks (so the user can still open in a new tab).
///
/// The closure is leaked via `closure.forget()` so the listener lives for
/// the entire WASM module lifetime — same posture as `setup_popstate_listener`.
#[cfg(wasm)]
fn install_link_interceptor(document: &web_sys::Document) -> Result<(), wasm_bindgen::JsValue> {
	use wasm_bindgen::JsCast;
	use wasm_bindgen::closure::Closure;

	let closure = Closure::wrap(Box::new(move |event: web_sys::MouseEvent| {
		// Walk up the DOM looking for the closest <a> ancestor.
		let Some(target) = event.target() else {
			return;
		};
		let mut el: Option<web_sys::Element> = target.dyn_ref::<web_sys::Element>().cloned();
		while let Some(ref e) = el {
			if e.tag_name().eq_ignore_ascii_case("A") {
				break;
			}
			el = e.parent_element();
		}
		let Some(anchor) = el else {
			return;
		};

		let href = anchor.get_attribute("href");
		let target_attr = anchor.get_attribute("target");
		let rel_attr = anchor.get_attribute("rel");
		let attrs = AnchorAttrs {
			has_modifier_key: event.ctrl_key() || event.meta_key() || event.shift_key(),
			href: href.as_deref(),
			target: target_attr.as_deref(),
			has_download: anchor.has_attribute("download"),
			rel: rel_attr.as_deref(),
		};

		let Some(href) = should_intercept(&attrs) else {
			return;
		};

		event.prevent_default();
		with_router(|r| {
			let _ = r.push(href);
		});
	}) as Box<dyn FnMut(_)>);

	document.add_event_listener_with_callback("click", closure.as_ref().unchecked_ref())?;
	closure.forget();
	Ok(())
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::*;

	#[rstest]
	fn test_client_launcher_new_stores_selector() {
		let launcher = ClientLauncher::new("#root");

		assert_eq!(launcher.root_selector, "#root");
		assert!(launcher.router_init.is_none());
	}

	#[rstest]
	fn test_client_launcher_router_stores_init_fn() {
		let launcher = ClientLauncher::new("#root");

		let launcher = launcher.router(Router::new);

		assert!(launcher.router_init.is_some());
	}

	#[rstest]
	fn test_with_router_panics_before_init() {
		let result = std::panic::catch_unwind(|| with_router(|_r| ()));

		assert!(result.is_err());
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

	// --- should_intercept pure-function tests ---

	fn attrs(href: Option<&str>) -> AnchorAttrs<'_> {
		AnchorAttrs {
			has_modifier_key: false,
			href,
			target: None,
			has_download: false,
			rel: None,
		}
	}

	#[rstest]
	fn test_should_intercept_internal_root_relative_link() {
		// Arrange
		let a = attrs(Some("/users/"));
		// Act
		let result = should_intercept(&a);
		// Assert
		assert_eq!(result, Some("/users/"));
	}

	#[rstest]
	fn test_should_intercept_skips_external_url() {
		// Arrange
		let a = attrs(Some("https://example.com/page"));
		// Act / Assert
		assert_eq!(should_intercept(&a), None);
	}

	#[rstest]
	fn test_should_intercept_skips_protocol_relative_url() {
		// Arrange
		let a = attrs(Some("//example.com/page"));
		// Act / Assert
		assert_eq!(should_intercept(&a), None);
	}

	#[rstest]
	fn test_should_intercept_skips_anchor_without_href() {
		// Arrange
		let a = attrs(None);
		// Act / Assert
		assert_eq!(should_intercept(&a), None);
	}

	#[rstest]
	fn test_should_intercept_skips_relative_link() {
		// Arrange
		let a = attrs(Some("relative/path"));
		// Act / Assert
		assert_eq!(should_intercept(&a), None);
	}

	#[rstest]
	fn test_should_intercept_skips_target_blank() {
		// Arrange
		let mut a = attrs(Some("/users/"));
		a.target = Some("_blank");
		// Act / Assert
		assert_eq!(should_intercept(&a), None);
	}

	#[rstest]
	fn test_should_intercept_allows_target_self() {
		// Arrange
		let mut a = attrs(Some("/users/"));
		a.target = Some("_self");
		// Act / Assert
		assert_eq!(should_intercept(&a), Some("/users/"));
	}

	#[rstest]
	fn test_should_intercept_skips_download_attribute() {
		// Arrange
		let mut a = attrs(Some("/files/report.pdf"));
		a.has_download = true;
		// Act / Assert
		assert_eq!(should_intercept(&a), None);
	}

	#[rstest]
	fn test_should_intercept_skips_rel_external() {
		// Arrange
		let mut a = attrs(Some("/users/"));
		a.rel = Some("external");
		// Act / Assert
		assert_eq!(should_intercept(&a), None);
	}

	#[rstest]
	fn test_should_intercept_skips_compound_rel_with_external() {
		// Arrange
		let mut a = attrs(Some("/users/"));
		a.rel = Some("noopener external");
		// Act / Assert
		assert_eq!(should_intercept(&a), None);
	}

	#[rstest]
	fn test_should_intercept_is_case_insensitive_for_rel() {
		// Arrange
		let mut a = attrs(Some("/users/"));
		a.rel = Some("EXTERNAL");
		// Act / Assert
		assert_eq!(should_intercept(&a), None);
	}

	#[rstest]
	fn test_should_intercept_allows_other_rel_values() {
		// Arrange
		let mut a = attrs(Some("/users/"));
		a.rel = Some("noopener noreferrer");
		// Act / Assert
		assert_eq!(should_intercept(&a), Some("/users/"));
	}

	#[rstest]
	fn test_should_intercept_skips_modifier_key_click() {
		// Arrange
		let mut a = attrs(Some("/users/"));
		a.has_modifier_key = true;
		// Act / Assert
		assert_eq!(should_intercept(&a), None);
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
		// first Effect run will be detected as a `None -> Some(_)` transition.
		assert!(
			launcher.path_subscriptions[0]
				.last_params
				.borrow()
				.is_none()
		);
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
