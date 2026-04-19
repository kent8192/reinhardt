//! DOM Abstraction Layer
//!
//! This module provides a thin, safe wrapper around web-sys DOM APIs.
//! The design emphasizes RAII patterns for automatic resource cleanup
//! and integration with the reactive system.
//!
//! ## Architecture
//!
//! - **Element**: Wrapper around `web_sys::Element` with type-safe operations
//! - **Document**: Wrapper around `web_sys::Document` for DOM creation
//! - **EventHandle**: RAII wrapper for event listeners (Drop-based cleanup)
//!
//! ## RAII Pattern
//!
//! All resources (event listeners, etc.) are automatically cleaned up when dropped:
//!
//! ```ignore
//! {
//!     let element = document().create_element("div")?;
//!     let handle = element.add_event_listener("click", || {
//!         console::log_1(&"Clicked!".into());
//!     });
//!     // Event listener automatically removed when `handle` is dropped
//! }
//! ```
//!
//! ## Integration with Reactive System
//!
//! Elements can bind to Signals for automatic updates:
//!
//! ```ignore
//! use reinhardt_pages::{reactive::Signal, dom::Element};
//!
//! let count = Signal::new(0);
//! let element = document().create_element("div")?;
//!
//! // Reactive attribute: updates automatically when count changes
//! element.set_reactive_attribute("data-count", count.clone());
//! ```

pub mod document;
pub mod element;
pub mod event;

// Re-exports for convenience
pub use document::Document;
pub use element::{Element, EventHandle};
pub use event::EventType;

/// Get the global Document instance
///
/// This is a convenience function that returns a cached Document wrapper.
///
/// # Example
///
/// ```ignore
/// use reinhardt_pages::dom::document;
///
/// let doc = document();
/// let div = doc.create_element("div")?;
/// ```
pub fn document() -> Document {
	Document::global()
}

/// Submit a form element by its metadata ID.
///
/// Called by `form!` macro-generated code for URL-action forms on WASM targets.
/// Locates the form by its HTML `id` attribute from
/// [`StaticFormMetadata`](crate::form_generated::StaticFormMetadata)
/// and triggers native browser form submission.
///
/// # Panics
///
/// - No global `window` exists
/// - `window` has no `document`
/// - No element with `metadata.id` found in the document
/// - Element is not an `HtmlFormElement`
/// - `request_submit()` fails (JS exception)
#[cfg(wasm)]
pub fn submit_form(metadata: &crate::form_generated::StaticFormMetadata) {
	use wasm_bindgen::JsCast;

	let window = web_sys::window().expect("No global window exists");
	let document = window.document().expect("Window should have a document");
	let element = document
		.get_element_by_id(&metadata.id)
		.unwrap_or_else(|| panic!("Form element with id '{}' not found", metadata.id));
	let form: web_sys::HtmlFormElement = element
		.dyn_into()
		.expect("Element is not an HtmlFormElement");
	form.request_submit().expect("Failed to submit form");
}
