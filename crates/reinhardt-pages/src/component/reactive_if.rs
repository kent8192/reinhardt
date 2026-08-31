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
use reinhardt_core::types::page::{
	BOOLEAN_ATTRS, Page, is_boolean_attr_truthy, is_safe_html_attribute, is_safe_html_element_name,
};
#[cfg(wasm)]
use std::cell::RefCell;
#[cfg(wasm)]
use std::rc::Rc;

#[cfg(wasm)]
pub(crate) type ReactiveNodeStore = Rc<RefCell<Vec<Box<dyn std::any::Any>>>>;

// Thread-local storage for reactive nodes to prevent them from being dropped.
//
// When a ReactiveIfNode is created during view mounting, it must be kept alive
// for the lifetime of the DOM element. This storage prevents premature cleanup.
#[cfg(wasm)]
thread_local! {
	static ROOT_REACTIVE_NODES: ReactiveNodeStore = Rc::new(RefCell::new(Vec::new()));
	static ACTIVE_REACTIVE_NODE_STORE: RefCell<Option<ReactiveNodeStore>> = RefCell::new(None);
}

#[cfg(wasm)]
struct ActiveReactiveNodeStoreGuard {
	previous: Option<ReactiveNodeStore>,
}

#[cfg(wasm)]
impl Drop for ActiveReactiveNodeStoreGuard {
	fn drop(&mut self) {
		ACTIVE_REACTIVE_NODE_STORE.with(|active| {
			active.replace(self.previous.take());
		});
	}
}

#[cfg(wasm)]
fn root_reactive_node_store() -> ReactiveNodeStore {
	ROOT_REACTIVE_NODES.with(Clone::clone)
}

#[cfg(wasm)]
fn current_reactive_node_store() -> ReactiveNodeStore {
	ACTIVE_REACTIVE_NODE_STORE
		.with(|active| active.borrow().clone())
		.unwrap_or_else(root_reactive_node_store)
}

#[cfg(wasm)]
pub(crate) fn new_reactive_node_store() -> ReactiveNodeStore {
	Rc::new(RefCell::new(Vec::new()))
}

#[cfg(wasm)]
pub(crate) fn clear_reactive_node_store(store: &ReactiveNodeStore) {
	store.borrow_mut().clear();
}

#[cfg(wasm)]
pub(crate) fn with_reactive_node_store<R>(store: &ReactiveNodeStore, f: impl FnOnce() -> R) -> R {
	let previous = ACTIVE_REACTIVE_NODE_STORE.with(|active| active.replace(Some(store.clone())));
	let _guard = ActiveReactiveNodeStoreGuard { previous };
	f()
}

/// Stores a reactive node to keep it alive.
#[cfg(wasm)]
pub fn store_reactive_node<T: 'static>(node: T) {
	current_reactive_node_store()
		.borrow_mut()
		.push(Box::new(node));
}

/// Cleanup function to release all reactive nodes.
///
/// This should be called when the application is being torn down or
/// when a complete re-render is needed.
#[cfg(wasm)]
pub fn cleanup_reactive_nodes() {
	clear_reactive_node_store(&root_reactive_node_store());
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
	pub fn new(
		parent: &crate::dom::Element,
		condition: std::sync::Arc<dyn Fn() -> bool + 'static>,
		then_view: std::sync::Arc<dyn Fn() -> Page + 'static>,
		else_view: std::sync::Arc<dyn Fn() -> Page + 'static>,
	) -> Self {
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
		let effect_reactive_node_store = current_reactive_node_store();

		// Create the Effect that will re-run when condition dependencies change
		let effect = Effect::new_with_timing(
			move || {
				with_reactive_node_store(&effect_reactive_node_store, || {
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

					// Refs #5100: remove old nodes before mounting the replacement view. The
					// mount path may synchronously run layout effects, so do not
					// hold this RefCell borrow across `mount_before_marker`.
					let old_nodes = {
						let mut nodes = current_nodes_clone.borrow_mut();
						nodes.drain(..).collect::<Vec<_>>()
					};
					for node in old_nodes {
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
					*current_nodes_clone.borrow_mut() = new_nodes;
				});
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
	pub fn new(
		parent: &crate::dom::Element,
		render: std::sync::Arc<dyn Fn() -> Page + 'static>,
	) -> Self {
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
		let effect_reactive_node_store = current_reactive_node_store();

		// Create the Effect that will re-run when dependencies change
		let effect = Effect::new_with_timing(
			move || {
				with_reactive_node_store(&effect_reactive_node_store, || {
					// Render the view (this tracks Signal dependencies)
					let view = render();

					if update_activity_boundary_attrs(&current_nodes_clone, &view) {
						return;
					}

					// Refs #5100: remove old nodes before mounting the replacement view. The
					// mount path may synchronously run layout effects, so do not
					// hold this RefCell borrow across `mount_before_marker`.
					let old_nodes = {
						let mut nodes = current_nodes_clone.borrow_mut();
						nodes.drain(..).collect::<Vec<_>>()
					};
					for node in old_nodes {
						if let Some(parent_node) = node.parent_node() {
							let _ = parent_node.remove_child(&node);
						}
					}

					// Mount new nodes before the marker
					let new_nodes = mount_before_marker(&marker_clone, view);
					*current_nodes_clone.borrow_mut() = new_nodes;
				});
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

#[cfg(wasm)]
fn update_activity_boundary_attrs(
	current_nodes: &Rc<RefCell<Vec<web_sys::Node>>>,
	view: &Page,
) -> bool {
	use wasm_bindgen::JsCast;

	let Page::Element(element_view) = view else {
		return false;
	};

	let Some(activity_mode) = element_view
		.attrs()
		.iter()
		.find(|(name, _)| name.as_ref() == "data-rh-activity")
		.map(|(_, value)| value.as_ref())
	else {
		return false;
	};

	if !element_view
		.attrs()
		.iter()
		.any(|(name, value)| name.as_ref() == "data-rh-state-preserved" && value.as_ref() == "true")
	{
		return false;
	}

	let nodes = current_nodes.borrow();
	if nodes.len() != 1 {
		return false;
	}

	let Some(existing_element) = nodes[0].dyn_ref::<web_sys::Element>() else {
		return false;
	};

	if existing_element
		.get_attribute("data-rh-state-preserved")
		.as_deref()
		!= Some("true")
		|| existing_element.get_attribute("data-rh-activity").is_none()
	{
		return false;
	}

	let _ = existing_element.set_attribute("data-rh-activity", activity_mode);
	let _ = existing_element.set_attribute("data-rh-state-preserved", "true");

	if activity_mode == "hidden" {
		let _ = existing_element.set_attribute("hidden", "hidden");
		let _ = existing_element.set_attribute("aria-hidden", "true");
	} else {
		let _ = existing_element.remove_attribute("hidden");
		let _ = existing_element.remove_attribute("aria-hidden");
	}

	true
}

#[cfg(wasm)]
fn create_nested_reactive_parent(
	document: &web_sys::Document,
	parent: &web_sys::Node,
	marker: &web_sys::Comment,
) -> web_sys::Element {
	let nested_parent = document
		.create_element("span")
		.expect("should create nested reactive parent");
	let _ = nested_parent.set_attribute("style", "display: contents");
	parent
		.insert_before(&nested_parent, Some(marker))
		.expect("should insert nested reactive parent");
	nested_parent
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
			if !is_safe_html_element_name(&tag) {
				for child in children {
					nodes.extend(mount_before_marker(marker, child));
				}
				return nodes;
			}

			let element = document
				.create_element(&tag)
				.expect("should create element");

			// Set attributes
			for (name, value) in attrs {
				if !is_safe_html_attribute(&name, &value) {
					continue;
				}
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
		Page::KeyedFragment(children) => {
			for (_, child) in children {
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

			let nested_parent = create_nested_reactive_parent(&document, &parent, marker);
			let nested_parent_wrapper = crate::dom::Element::new(nested_parent.clone());
			let nested_node =
				ReactiveIfNode::new(&nested_parent_wrapper, condition, then_view, else_view);

			// Track the wrapper so the outer reactive owner removes the complete
			// nested DOM subtree, including the nested marker and rendered content.
			nodes.push(nested_parent.unchecked_into());
			store_reactive_node(nested_node);
		}
		Page::Reactive(reactive) => {
			let nested_parent = create_nested_reactive_parent(&document, &parent, marker);
			let nested_parent_wrapper = crate::dom::Element::new(nested_parent.clone());
			let render = reactive.into_render();
			let nested_node = ReactiveNode::new(&nested_parent_wrapper, render);

			// Track the wrapper so the outer reactive owner removes the complete
			// nested DOM subtree, including the nested marker and rendered content.
			nodes.push(nested_parent.unchecked_into());
			store_reactive_node(nested_node);
		}
	}

	nodes
}

// Note: is_boolean_attr_truthy and BOOLEAN_ATTRS are imported from reinhardt_core::types::page
