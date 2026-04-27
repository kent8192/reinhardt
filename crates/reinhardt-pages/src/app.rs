//! WASM client application launcher.

use crate::router::Router;
use std::cell::RefCell;

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
/// scheduler, DOM mounting, reactive `Effect` for route changes, and
/// history listener.
///
/// # Example
///
/// ```ignore
/// use reinhardt::pages::ClientLauncher;
/// use wasm_bindgen::prelude::*;
///
/// #[wasm_bindgen(start)]
/// pub fn main() -> Result<(), JsValue> {
///     ClientLauncher::new("#root")
///         .router(router::init_router)
///         .launch()
/// }
/// ```
pub struct ClientLauncher {
	#[cfg_attr(not(wasm), allow(dead_code))]
	root_selector: &'static str,
	router_init: Option<Box<dyn FnOnce() -> Router>>,
	#[cfg_attr(not(wasm), allow(dead_code))]
	intercept_links: bool,
}

impl ClientLauncher {
	/// Create a new launcher targeting the given CSS selector (e.g. `"#root"`).
	pub fn new(root_selector: &'static str) -> Self {
		Self {
			root_selector,
			router_init: None,
			intercept_links: true,
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
}

#[cfg(wasm)]
impl ClientLauncher {
	/// Start the WASM client application.
	///
	/// Performs in order:
	/// 1. Sets up the panic hook for readable console errors
	/// 2. Configures the reactive scheduler for async contexts
	/// 3. Initialises the router and stores it in the global thread-local
	/// 4. Registers the `popstate` history listener (browser back/forward)
	/// 5. Queries the DOM for `root_selector`; returns `Err` if not found
	/// 6. Installs the SPA link-interception listener on `document`
	///    (when `intercept_links` is `true`, the default)
	/// 7. Initial render: `render_current()` → `cleanup_reactive_nodes()` → `mount()`
	/// 8. Creates a reactive `Effect` that re-renders on route changes; leaks it
	///    intentionally so it persists for the application lifetime
	pub fn launch(self) -> Result<(), wasm_bindgen::JsValue> {
		#[cfg(feature = "console_error_panic_hook")]
		console_error_panic_hook::set_once();

		crate::reactive::runtime::set_scheduler(|task| {
			wasm_bindgen_futures::spawn_local(async move { task() });
		});

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
}
