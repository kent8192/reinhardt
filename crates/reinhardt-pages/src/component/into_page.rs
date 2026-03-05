//! IntoPage trait and Page enum for component rendering.
//!
//! This module re-exports the core Page types from `reinhardt-types` and provides
//! WASM-specific extensions for DOM mounting.

// Re-export core types from reinhardt-types
pub use reinhardt_core::types::page::{
	Head, IntoPage, LinkTag, MetaTag, MountError, Page, PageElement, PageEventHandler, Reactive,
	ReactiveIf, ScriptTag, StyleTag,
};

// DummyEvent is only available on non-WASM targets
#[cfg(not(target_arch = "wasm32"))]
pub use reinhardt_core::types::page::DummyEvent;
// Re-export boolean attribute utilities (used in WASM mount)
// Note: EventType is re-exported from dom::event module
#[cfg(target_arch = "wasm32")]
pub use reinhardt_core::types::page::{BOOLEAN_ATTRS, is_boolean_attr_truthy};

#[cfg(target_arch = "wasm32")]
use crate::component::reactive_if::{ReactiveIfNode, ReactiveNode, store_reactive_node};
#[cfg(target_arch = "wasm32")]
use crate::dom::Element;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::closure::Closure;

/// Extension trait for mounting Page to DOM (WASM only).
///
/// This trait provides the `mount()` method for `Page` which is only available
/// in WASM environments where actual DOM manipulation is possible.
#[cfg(target_arch = "wasm32")]
pub trait PageExt {
	/// Mounts the view to a DOM element (client-side only).
	fn mount(self, parent: &Element) -> Result<(), MountError>;
}

#[cfg(target_arch = "wasm32")]
impl PageExt for Page {
	fn mount(self, parent: &Element) -> Result<(), MountError> {
		mount_inner(self, parent)
	}
}

#[cfg(target_arch = "wasm32")]
fn mount_inner(page: Page, parent: &Element) -> Result<(), MountError> {
	use crate::dom::document;

	match page {
		Page::Element(el) => {
			let doc = document();
			let (tag, attrs, children, _is_void, event_handlers) = el.into_parts();

			let element = doc
				.create_element(&tag)
				.map_err(|_| MountError::CreateElementFailed)?;

			for (name, value) in attrs {
				// Skip boolean attributes with falsy values (empty, "false", "0")
				// This ensures `disabled: ""` doesn't set the attribute
				let name_str: &str = name.as_ref();
				let value_str: &str = value.as_ref();
				let is_boolean = BOOLEAN_ATTRS.contains(&name_str);
				let is_falsy = !is_boolean_attr_truthy(value_str);

				if is_boolean && is_falsy {
					continue;
				}

				element
					.set_attribute(&name, &value)
					.map_err(|err_str: String| {
						// Log detailed error to browser console
						let msg: wasm_bindgen::JsValue = format!(
							"[SetAttributeFailed] attribute='{}', value='{}'",
							name, value
						)
						.into();
						let label: wasm_bindgen::JsValue = "Error message:".into();
						let err_msg: wasm_bindgen::JsValue = err_str.into();
						web_sys::console::error_3(&msg, &label, &err_msg);
						MountError::SetAttributeFailed
					})?;
			}

			// Attach event handlers before mounting children
			for (event_type, handler) in event_handlers {
				let handler_clone = handler.clone();
				let closure = Closure::wrap(Box::new(move |event: web_sys::Event| {
					handler_clone(event);
				}) as Box<dyn FnMut(web_sys::Event)>);

				element
					.inner()
					.add_event_listener_with_callback(
						event_type.as_str(),
						closure.as_ref().unchecked_ref(),
					)
					.expect("Failed to add event listener");

				// Intentional memory leak: the closure must outlive the element's DOM
				// lifetime. Since mount_inner creates closures in a recursive loop
				// with no parent struct to store them, forget() is the practical
				// choice here. For components with frequent mount/unmount cycles,
				// consider using a lifecycle-managed approach instead.
				closure.forget();
			}

			for child in children {
				mount_inner(child, &element)?;
			}

			parent
				.append_child(element)
				.map_err(|_| MountError::AppendChildFailed)?;
		}
		Page::Text(text) => {
			let window = web_sys::window().ok_or(MountError::NoWindow)?;
			let document = window.document().ok_or(MountError::NoDocument)?;
			let text_node = document.create_text_node(&text);
			parent
				.inner()
				.append_child(&text_node)
				.map_err(|_| MountError::AppendChildFailed)?;
		}
		Page::Fragment(children) => {
			for child in children {
				mount_inner(child, parent)?;
			}
		}
		Page::Empty => {}
		Page::WithHead { view, .. } => {
			// On client-side, head is handled separately; just mount the content
			mount_inner(*view, parent)?;
		}
		Page::ReactiveIf(reactive_if) => {
			// Decompose the ReactiveIf to get the closures
			let (condition, then_view, else_view) = reactive_if.into_parts();

			// Create a ReactiveIfNode that manages DOM updates reactively.
			// The node uses an Effect to monitor condition changes and swaps
			// DOM nodes when the condition value changes.
			let node = ReactiveIfNode::new(
				parent,
				move || condition(),
				move || then_view(),
				move || else_view(),
			);
			// Store the node to keep it alive for the lifetime of the DOM element
			store_reactive_node(node);
		}
		Page::Reactive(reactive) => {
			// Get the render closure from the Reactive
			let render = reactive.into_render();

			// Create a ReactiveNode that manages DOM updates reactively.
			// The node uses an Effect to monitor dependency changes and
			// re-renders when they change.
			let node = ReactiveNode::new(parent, move || render());
			// Store the node to keep it alive for the lifetime of the DOM element
			store_reactive_node(node);
		}
	}

	Ok(())
}

/// Extension trait for mounting Page (non-WASM stub).
///
/// In non-WASM environments, this trait provides a no-op stub for API compatibility.
#[cfg(not(target_arch = "wasm32"))]
pub trait PageExt {
	/// Mounts the view (non-WASM stub).
	///
	/// In non-WASM environments, this function is a no-op stub that always succeeds.
	/// The `parent` parameter is unused and exists only for API compatibility.
	fn mount<T>(self, _parent: &T) -> Result<(), MountError>;
}

#[cfg(not(target_arch = "wasm32"))]
impl PageExt for Page {
	fn mount<T>(self, _parent: &T) -> Result<(), MountError> {
		Ok(())
	}
}
