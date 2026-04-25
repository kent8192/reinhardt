//! WASM entry point for {{ project_name }}.
//!
//! Mounts the client-side router into the `#root` element on page load.

use reinhardt::pages::PageExt;
use reinhardt::pages::dom::Element;
use wasm_bindgen::prelude::*;

use super::router;

pub use router::{init_global_router, with_router};

#[wasm_bindgen(start)]
pub fn main() -> Result<(), JsValue> {
	// Better panic messages in browser console
	console_error_panic_hook::set_once();

	// Initialize the application router
	router::init_global_router();

	// Resolve the root element and clear any loading placeholder
	let window = web_sys::window().expect("no global `window` exists");
	let document = window
		.document()
		.expect("should have a document on window");
	let root = document
		.get_element_by_id("root")
		.expect("should have #root element");
	root.set_inner_html("");

	// Mount the router's current view
	router::with_router(|router| {
		let view = router.render_current();
		let root_element = Element::new(root.clone());
		let _ = view.mount(&root_element);
	});

	Ok(())
}
