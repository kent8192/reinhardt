//! WASM entry point
//!
//! This is the main entry point for the WASM application.

use reinhardt::pages::dom::Element;
use wasm_bindgen::prelude::*;

// Use modules from parent `core::client` module via super::
use super::router;
use crate::apps::auth::client::state;

pub use router::{AppRoute, init_global_router, with_router};
pub use state::{get_current_user, init_auth_state, is_authenticated, set_current_user};

#[wasm_bindgen(start)]
pub fn main() -> Result<(), JsValue> {
	// Set panic hook for better error messages in browser console
	console_error_panic_hook::set_once();

	// Initialize hydration state BEFORE any component initialization
	reinhardt::pages::hydration::init_hydration_state();

	// Initialize global state
	state::init_auth_state();

	// Initialize router
	router::init_global_router();

	// Get the root element
	let window = web_sys::window().expect("no global `window` exists");
	let document = window.document().expect("should have a document on window");
	let root = document
		.get_element_by_id("root")
		.expect("should have #root element");

	// Clear loading spinner
	root.set_inner_html("");

	// Mount the router's current view and attach events
	router::with_router(|router| {
		let root_element = Element::new(root.clone());

		// Initialize route params by navigating to current path
		let current_path = window
			.location()
			.pathname()
			.unwrap_or_else(|_| "/".to_string());
		if let Err(e) = router.replace(&current_path) {
			web_sys::console::error_1(&format!("Failed to navigate: {:?}", e).into());
		}

		// Render and mount the view (events are attached during mount)
		let view = router.render_current();
		if let Err(e) = view.mount(&root_element) {
			web_sys::console::error_1(&format!("Failed to mount view: {:?}", e).into());
			return;
		}
	});

	// Mark hydration complete after mounting (since this app doesn't use SSR/hydration)
	// This ensures that form buttons and other hydration-gated UI elements become enabled
	reinhardt::pages::hydration::mark_hydration_complete();

	Ok(())
}
