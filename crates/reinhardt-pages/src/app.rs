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
}

impl ClientLauncher {
	/// Create a new launcher targeting the given CSS selector (e.g. `"#root"`).
	pub fn new(root_selector: &'static str) -> Self {
		Self {
			root_selector,
			router_init: None,
		}
	}

	/// Register the router initializer function.
	///
	/// The function is called once during `launch()` before the first render.
	pub fn router<F: FnOnce() -> Router + 'static>(mut self, f: F) -> Self {
		self.router_init = Some(Box::new(f));
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
	/// 6. Initial render: `render_current()` → `cleanup_reactive_nodes()` → `mount()`
	/// 7. Creates a reactive `Effect` that re-renders on route changes; leaks it
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
}
