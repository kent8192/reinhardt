//! WASM entry point for Reinhardt Admin Panel

use crate::pages::router;
use reinhardt_pages::Effect;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use web_sys::{Event, HtmlElement, window};

/// WASM entry point
///
/// This function is called when the WASM module is loaded.
/// It initializes the application and mounts it to the DOM.
#[allow(clippy::main_recursion)]
#[wasm_bindgen(start)]
pub fn main() -> Result<(), JsValue> {
	// Set up panic hook for better error messages in console
	#[cfg(feature = "console_error_panic_hook")]
	console_error_panic_hook::set_once();

	// Get DOM elements
	let window = window().ok_or_else(|| JsValue::from_str("No window object"))?;
	let document = window
		.document()
		.ok_or_else(|| JsValue::from_str("No document object"))?;
	let app_element = document
		.get_element_by_id("app")
		.ok_or_else(|| JsValue::from_str("No #app element found"))?;

	// Initialize global router
	router::init_global_router();

	// Initial render
	// SAFETY(XSS): render_to_string() HTML-escapes all dynamic text content
	// and attribute values via html_escape(), preventing XSS injection.
	let view = router::with_router(|r| r.render_current());
	app_element.set_inner_html(&view.render_to_string());

	// Set up reactive effect for route changes
	// Effect will automatically re-render when route changes
	let app_clone = app_element.clone();
	let _effect = Effect::new(move || {
		// SAFETY(XSS): render_to_string() HTML-escapes all dynamic text content
		// and attribute values via html_escape(), preventing XSS injection.
		let view = router::with_router(|r| {
			// Subscribe to current_params to trigger re-render on route change
			let _ = r.current_params().get();
			r.render_current()
		});
		app_clone.set_inner_html(&view.render_to_string());
	});
	// Intentional memory leak: WASM entry points run for the entire application
	// lifetime and never terminate. The Effect must persist to keep reactive
	// re-renders working. Dropping it would disable route-change rendering.
	// See: https://rustwasm.github.io/wasm-bindgen/reference/weak-references.html
	std::mem::forget(_effect);

	// Set up navigation event listeners

	// 1. Link click handler (event delegation)
	// Intercept all link clicks and use client-side routing
	let link_handler = Closure::wrap(Box::new(move |event: Event| {
		// Check if the clicked element is a link
		if let Some(target) = event.target()
			&& let Ok(element) = target.dyn_into::<HtmlElement>()
		{
			// Find the closest <a> tag (in case user clicked inside the link)
			let mut current = Some(element);
			while let Some(el) = current {
				if el.tag_name().to_lowercase() == "a" {
					// Get href attribute
					if let Some(href) = el.get_attribute("href") {
						// Only handle internal links (starting with /)
						if href.starts_with('/') {
							// Prevent default browser navigation
							event.prevent_default();

							// Navigate using router
							router::with_router(|r| {
								let _ = r.push(&href);
							});
							return;
						}
					}
					break;
				}
				// Move up to parent
				current = el
					.parent_element()
					.and_then(|p| p.dyn_into::<HtmlElement>().ok());
			}
		}
	}) as Box<dyn FnMut(_)>);

	document.add_event_listener_with_callback("click", link_handler.as_ref().unchecked_ref())?;
	link_handler.forget();

	// 2. Browser back/forward button handler (popstate)
	let popstate_handler = Closure::wrap(Box::new(move |_event: Event| {
		// Re-render when user navigates with browser buttons
		// Use replace() since browser has already changed the history
		router::with_router(|r| {
			let current_path = web_sys::window()
				.and_then(|w| w.location().pathname().ok())
				.unwrap_or_else(|| "/".to_string());
			let _ = r.replace(&current_path);
		});
	}) as Box<dyn FnMut(_)>);

	window
		.add_event_listener_with_callback("popstate", popstate_handler.as_ref().unchecked_ref())?;
	popstate_handler.forget();

	Ok(())
}
