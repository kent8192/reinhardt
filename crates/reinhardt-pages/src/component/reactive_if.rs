//! Reactive conditional rendering DOM management.
//!
//! This module provides the `ReactiveIfNode` which manages DOM updates
//! for conditional rendering based on Signal changes.

#[cfg(wasm)]
use crate::component::into_page::PageExt;
#[cfg(wasm)]
use crate::reactive::effect::Effect;
#[cfg(wasm)]
use crate::reactive::runtime::EffectTiming;
#[cfg(wasm)]
use reinhardt_core::types::page::{BOOLEAN_ATTRS, Page, is_boolean_attr_truthy};
#[cfg(wasm)]
use std::cell::RefCell;
#[cfg(wasm)]
use std::rc::Rc;

// Thread-local storage for reactive nodes to prevent them from being dropped.
//
// When a ReactiveIfNode is created during view mounting, it must be kept alive
// for the lifetime of the DOM element. This storage prevents premature cleanup.
#[cfg(wasm)]
thread_local! {
	static REACTIVE_NODES: RefCell<Vec<Box<dyn std::any::Any>>> = RefCell::new(Vec::new());
}

/// Stores a reactive node to keep it alive.
#[cfg(wasm)]
pub fn store_reactive_node<T: 'static>(node: T) {
	REACTIVE_NODES.with(|nodes| {
		nodes.borrow_mut().push(Box::new(node));
	});
}

/// Cleanup function to release all reactive nodes.
///
/// This should be called when the application is being torn down or
/// when a complete re-render is needed.
#[cfg(wasm)]
pub fn cleanup_reactive_nodes() {
	REACTIVE_NODES.with(|nodes| {
		nodes.borrow_mut().clear();
	});
}

/// Manages DOM updates for reactive conditional rendering.
///
/// ReactiveIfNode creates a comment node as a marker in the DOM, and uses
/// an Effect to monitor condition changes. When the condition changes,
/// it removes the old DOM nodes and mounts new ones.
#[cfg(wasm)]
pub struct ReactiveIfNode {
	/// Marker comment node in DOM (used as insertion point reference)
	#[allow(dead_code)] // Kept for potential future use
	marker: web_sys::Comment,
	/// Currently mounted DOM nodes
	#[allow(dead_code)] // Kept for potential future use
	current_nodes: Rc<RefCell<Vec<web_sys::Node>>>,
	/// Last evaluated condition value (for change detection)
	#[allow(dead_code)] // Kept for potential future use
	last_condition: Rc<RefCell<Option<bool>>>,
	/// Effect handle (kept alive to maintain reactivity)
	#[allow(dead_code)] // Effect is kept alive for its side effects
	effect: Effect,
}

#[cfg(wasm)]
impl ReactiveIfNode {
	/// Creates a new ReactiveIfNode and mounts it with reactive updates.
	///
	/// # Arguments
	///
	/// * `parent` - The parent DOM element to mount the conditional content into
	/// * `condition` - Closure that returns the condition value
	/// * `then_view` - Closure that returns the view when condition is true
	/// * `else_view` - Closure that returns the view when condition is false
	pub fn new<C, T, E>(
		parent: &crate::dom::Element,
		condition: C,
		then_view: T,
		else_view: E,
	) -> Self
	where
		C: Fn() -> bool + 'static,
		T: Fn() -> Page + 'static,
		E: Fn() -> Page + 'static,
	{
		// Create a comment node as a marker/anchor point
		let document = web_sys::window()
			.expect("window should be available")
			.document()
			.expect("document should be available");
		let marker = document.create_comment("reactive-if");

		// Append marker to parent
		parent
			.inner()
			.append_child(&marker)
			.expect("should append marker");

		// Shared state for the Effect
		let current_nodes: Rc<RefCell<Vec<web_sys::Node>>> = Rc::new(RefCell::new(Vec::new()));
		let last_condition: Rc<RefCell<Option<bool>>> = Rc::new(RefCell::new(None));

		// Clone references for the Effect closure
		let current_nodes_clone = current_nodes.clone();
		let last_condition_clone = last_condition.clone();
		let marker_clone = marker.clone();

		// Create the Effect that will re-run when condition dependencies change
		let effect = Effect::new_with_timing(
			move || {
				// Evaluate the condition (this tracks Signal dependencies)
				let new_condition = condition();

				// Check if condition has changed
				let mut last = last_condition_clone.borrow_mut();
				if *last == Some(new_condition) {
					// Condition hasn't changed, skip DOM update
					return;
				}
				*last = Some(new_condition);
				drop(last);

				// Remove old nodes
				let mut nodes = current_nodes_clone.borrow_mut();
				for node in nodes.drain(..) {
					if let Some(parent_node) = node.parent_node() {
						let _ = parent_node.remove_child(&node);
					}
				}

				// Generate the appropriate view
				let view = if new_condition {
					then_view()
				} else {
					else_view()
				};

				// Mount new nodes before the marker
				let new_nodes = mount_before_marker(&marker_clone, view);
				*nodes = new_nodes;
			},
			EffectTiming::Layout, // Use Layout timing for synchronous DOM updates
		);

		Self {
			marker,
			current_nodes,
			last_condition,
			effect,
		}
	}
}

/// Manages DOM updates for reactive view rendering.
///
/// ReactiveNode is similar to ReactiveIfNode but handles a single render
/// closure rather than conditional branches. It creates a comment node as
/// a marker and uses an Effect to monitor Signal changes.
#[cfg(wasm)]
pub struct ReactiveNode {
	/// Marker comment node in DOM (used as insertion point reference)
	#[allow(dead_code)] // Kept for potential future use
	marker: web_sys::Comment,
	/// Currently mounted DOM nodes
	#[allow(dead_code)] // Kept for potential future use
	current_nodes: Rc<RefCell<Vec<web_sys::Node>>>,
	/// Effect handle (kept alive to maintain reactivity)
	#[allow(dead_code)] // Effect is kept alive for its side effects
	effect: Effect,
}

#[cfg(wasm)]
impl ReactiveNode {
	/// Creates a new ReactiveNode and mounts it with reactive updates.
	///
	/// # Arguments
	///
	/// * `parent` - The parent DOM element to mount the reactive content into
	/// * `render` - Closure that returns the view to render
	pub fn new<F>(parent: &crate::dom::Element, render: F) -> Self
	where
		F: Fn() -> Page + 'static,
	{
		// Create a comment node as a marker/anchor point
		let document = web_sys::window()
			.expect("window should be available")
			.document()
			.expect("document should be available");
		let marker = document.create_comment("reactive");

		// Append marker to parent
		parent
			.inner()
			.append_child(&marker)
			.expect("should append marker");

		// Shared state for the Effect
		let current_nodes: Rc<RefCell<Vec<web_sys::Node>>> = Rc::new(RefCell::new(Vec::new()));

		// Clone references for the Effect closure
		let current_nodes_clone = current_nodes.clone();
		let marker_clone = marker.clone();

		// Create the Effect that will re-run when dependencies change
		let effect = Effect::new_with_timing(
			move || {
				// Render the view (this tracks Signal dependencies)
				let view = render();

				// Remove old nodes
				let mut nodes = current_nodes_clone.borrow_mut();
				for node in nodes.drain(..) {
					if let Some(parent_node) = node.parent_node() {
						let _ = parent_node.remove_child(&node);
					}
				}

				// Mount new nodes before the marker
				let new_nodes = mount_before_marker(&marker_clone, view);
				*nodes = new_nodes;
			},
			EffectTiming::Layout, // Use Layout timing for synchronous DOM updates
		);

		Self {
			marker,
			current_nodes,
			effect,
		}
	}
}

/// Mounts a Page before a marker node and returns the created DOM nodes.
///
/// This function recursively mounts the view tree and inserts all created
/// nodes before the marker comment node.
#[cfg(wasm)]
fn mount_before_marker(marker: &web_sys::Comment, view: Page) -> Vec<web_sys::Node> {
	use wasm_bindgen::JsCast;

	let document = web_sys::window()
		.expect("window should be available")
		.document()
		.expect("document should be available");

	let parent = marker.parent_node().expect("marker should have a parent");

	let mut nodes = Vec::new();

	match view {
		Page::Element(el) => {
			// Decompose the element to avoid ownership issues
			let (tag, attrs, children, _is_void, event_handlers) = el.into_parts();

			let element = document
				.create_element(&tag)
				.expect("should create element");

			// Set attributes
			for (name, value) in attrs {
				// Skip falsy boolean attributes
				let name_str: &str = name.as_ref();
				if BOOLEAN_ATTRS.contains(&name_str) && !is_boolean_attr_truthy(&value) {
					continue;
				}
				let _ = element.set_attribute(&name, &value);
			}

			// Mount children
			let element_wrapper = crate::dom::Element::new(element.clone());
			for child in children {
				let _ = child.mount(&element_wrapper);
			}

			// Attach event handlers
			for (event_type, handler) in event_handlers {
				use wasm_bindgen::closure::Closure;

				let handler_clone = handler.clone();
				let closure = Closure::wrap(Box::new(move |event: web_sys::Event| {
					handler_clone(event);
				}) as Box<dyn FnMut(web_sys::Event)>);

				let _ = element.add_event_listener_with_callback(
					event_type.as_str(),
					closure.as_ref().unchecked_ref(),
				);
				closure.forget();
			}

			// Insert before marker
			let _ = parent.insert_before(&element, Some(marker));
			nodes.push(element.unchecked_into());
		}
		Page::Text(text) => {
			let text_node = document.create_text_node(&text);
			let _ = parent.insert_before(&text_node, Some(marker));
			nodes.push(text_node.unchecked_into());
		}
		Page::Fragment(children) => {
			for child in children {
				nodes.extend(mount_before_marker(marker, child));
			}
		}
		Page::Empty => {}
		Page::WithHead { view, .. } => {
			// Head is handled separately; just mount the content
			nodes.extend(mount_before_marker(marker, *view));
		}
		Page::ReactiveIf(reactive_if) => {
			// Decompose the ReactiveIf to get the closures
			let (condition, then_view, else_view) = reactive_if.into_parts();

			// Create a nested ReactiveIfNode
			// First, create a new marker for this nested reactive if
			let nested_marker = document.create_comment("reactive-if-nested");
			let _ = parent.insert_before(&nested_marker, Some(marker));
			nodes.push(nested_marker.clone().unchecked_into());

			// Create a temporary parent wrapper to use ReactiveIfNode
			let temp_parent =
				crate::dom::Element::new(parent.clone().unchecked_into::<web_sys::Element>());

			// Use the nested marker as the anchor point
			let nested_node = ReactiveIfNode::new(
				&temp_parent,
				move || condition(),
				move || then_view(),
				move || else_view(),
			);

			// Store the nested node to keep it alive
			store_reactive_node(nested_node);
		}
		Page::Reactive(reactive) => {
			// Create a nested ReactiveNode
			// First, create a new marker for this nested reactive
			let nested_marker = document.create_comment("reactive-nested");
			let _ = parent.insert_before(&nested_marker, Some(marker));
			nodes.push(nested_marker.clone().unchecked_into());

			// Create a temporary parent wrapper to use ReactiveNode
			let temp_parent =
				crate::dom::Element::new(parent.clone().unchecked_into::<web_sys::Element>());

			// Get the render closure
			let render = reactive.into_render();

			// Create the nested ReactiveNode
			let nested_node = ReactiveNode::new(&temp_parent, move || render());

			// Store the nested node to keep it alive
			store_reactive_node(nested_node);
		}
	}

	nodes
}

// Note: is_boolean_attr_truthy and BOOLEAN_ATTRS are imported from reinhardt_core::types::page
