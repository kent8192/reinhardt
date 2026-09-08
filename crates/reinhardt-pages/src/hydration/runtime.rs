//! Hydration Runtime
//!
//! This module provides the main entry point for client-side hydration,
//! connecting reactive state with SSR-rendered DOM elements.

use crate::component::Component;
use crate::ssr::SsrState;
use std::collections::HashMap;

#[cfg(wasm)]
use crate::dom::{Element, document};

#[cfg(wasm)]
use crate::ssr::HYDRATION_ATTR_ID;

/// Errors that can occur during hydration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HydrationError {
	/// The hydration root element was not found.
	RootNotFound(String),
	/// SSR state could not be parsed.
	StateParseError(String),
	/// A hydration marker was not found.
	MarkerNotFound(String),
	/// DOM structure doesn't match expected structure.
	StructureMismatch {
		/// The hydration ID.
		id: String,
		/// Expected element.
		expected: String,
		/// Actual element.
		actual: String,
	},
	/// Event attachment failed.
	EventAttachmentFailed(String),
}

impl std::fmt::Display for HydrationError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::RootNotFound(id) => write!(f, "Hydration root element not found: {}", id),
			Self::StateParseError(msg) => write!(f, "Failed to parse SSR state: {}", msg),
			Self::MarkerNotFound(id) => write!(f, "Hydration marker not found: {}", id),
			Self::StructureMismatch {
				id,
				expected,
				actual,
			} => {
				write!(
					f,
					"DOM structure mismatch at {}: expected {}, found {}",
					id, expected, actual
				)
			}
			Self::EventAttachmentFailed(msg) => write!(f, "Event attachment failed: {}", msg),
		}
	}
}

impl std::error::Error for HydrationError {}

/// Context for hydration operations.
#[derive(Debug)]
pub struct HydrationContext {
	/// The restored SSR state.
	state: SsrState,
	/// Mapping of hydration IDs to signal values.
	signals: HashMap<String, serde_json::Value>,
	/// Mapping of hydration IDs to component props.
	props: HashMap<String, serde_json::Value>,
	/// Whether hydration has been completed.
	hydrated: bool,
}

impl Default for HydrationContext {
	fn default() -> Self {
		Self::new()
	}
}

impl HydrationContext {
	/// Creates a new hydration context.
	pub fn new() -> Self {
		Self {
			state: SsrState::new(),
			signals: HashMap::new(),
			props: HashMap::new(),
			hydrated: false,
		}
	}

	/// Creates a context from SSR state.
	pub fn from_state(state: SsrState) -> Self {
		Self {
			state,
			signals: HashMap::new(),
			props: HashMap::new(),
			hydrated: false,
		}
	}

	/// Restores state from the `<script id="ssr-state">` element's JSON content.
	#[cfg(wasm)]
	pub fn from_window() -> Result<Self, HydrationError> {
		let window = web_sys::window()
			.ok_or_else(|| HydrationError::StateParseError("Window not available".to_string()))?;

		let document = window
			.document()
			.ok_or_else(|| HydrationError::StateParseError("Document not available".to_string()))?;

		let element = document.get_element_by_id("ssr-state").ok_or_else(|| {
			HydrationError::StateParseError("SSR state element not found".to_string())
		})?;

		let json = element.text_content().ok_or_else(|| {
			HydrationError::StateParseError("SSR state element is empty".to_string())
		})?;

		if json.trim().is_empty() {
			return Ok(Self::new());
		}

		let state = SsrState::from_json(&json)
			.map_err(|e| HydrationError::StateParseError(e.to_string()))?;

		Ok(Self::from_state(state))
	}

	/// Non-WASM version that returns an empty context.
	#[cfg(native)]
	pub fn from_window() -> Result<Self, HydrationError> {
		Ok(Self::new())
	}

	/// Gets a signal value by its hydration ID.
	pub fn get_signal(&self, id: &str) -> Option<&serde_json::Value> {
		self.signals.get(id).or_else(|| self.state.get_signal(id))
	}

	/// Gets component props by its hydration ID.
	pub fn get_props(&self, id: &str) -> Option<&serde_json::Value> {
		self.props.get(id).or_else(|| self.state.get_props(id))
	}

	/// Marks hydration as complete.
	pub fn mark_hydrated(&mut self) {
		self.hydrated = true;
	}

	/// Checks if hydration is complete.
	pub fn is_hydrated(&self) -> bool {
		self.hydrated
	}
}

/// Hydrates a component into the specified root element.
#[cfg(wasm)]
pub fn hydrate<C: Component>(component: &C, root: &Element) -> Result<(), HydrationError> {
	use super::events::EventRegistry;
	use super::reconcile::reconcile;

	web_sys::console::log_1(&"[Hydration] Starting...".into());

	// 1. Restore SSR state
	let mut context = HydrationContext::from_window()?;

	// 2. Render the component to get expected structure
	let view = component.render();
	web_sys::console::log_1(&"[Hydration] View rendered".into());

	// 3. Reconcile DOM structure
	reconcile(root, &view)
		.map_err(|e| HydrationError::StateParseError(format!("Reconciliation failed: {}", e)))?;
	web_sys::console::log_1(&"[Hydration] Reconciliation complete".into());

	// 4. Attach event handlers
	let mut registry = EventRegistry::new();
	attach_events_recursive(root, &view, &mut registry)?;
	crate::component::reactive_if::store_reactive_node(registry);
	web_sys::console::log_1(&"[Hydration] Events attached".into());

	// 5. Mark hydration complete
	context.mark_hydrated();
	mark_hydration_complete_internal();
	web_sys::console::log_1(&"[Hydration] Complete!".into());

	Ok(())
}

/// Non-WASM version for testing.
#[cfg(native)]
pub fn hydrate<C: Component>(_component: &C, _root: &str) -> Result<(), HydrationError> {
	Ok(())
}

/// Hydrates a component at the default root element (#app).
#[cfg(wasm)]
pub fn hydrate_root<C: Component + Default>() -> Result<(), HydrationError> {
	let component = C::default();
	let doc = document();
	let root = doc
		.query_selector("#app")
		.map_err(|e| HydrationError::StateParseError(format!("Query selector failed: {}", e)))?
		.ok_or_else(|| HydrationError::RootNotFound("#app".to_string()))?;

	hydrate(&component, &root)
}

/// Non-WASM version for testing.
#[cfg(native)]
pub fn hydrate_root<C: Component + Default>() -> Result<(), HydrationError> {
	Ok(())
}

/// Attaches event handlers to a mounted view (CSR mode).
///
/// This is a convenience function for client-side rendering (CSR) applications.
/// After mounting a view with `view.mount()`, call this function to attach event handlers.
///
/// # Example
///
/// ```ignore
/// use reinhardt_pages::hydration::attach_events_to_mounted_view;
/// use reinhardt_pages::dom::Element;
///
/// // Mount the view
/// let view = my_component();
/// let root = Element::new(root_element);
/// view.mount(&root)?;
///
/// // Attach event handlers
/// attach_events_to_mounted_view(&root, &view)?;
/// ```
#[cfg(wasm)]
pub fn attach_events_to_mounted_view(
	element: &Element,
	view: &crate::component::Page,
) -> Result<(), HydrationError> {
	use super::events::EventRegistry;

	web_sys::console::log_1(&"[CSR] Attaching events to mounted view...".into());

	let mut registry = EventRegistry::new();
	attach_events_recursive(element, view, &mut registry)?;
	crate::component::reactive_if::store_reactive_node(registry);

	web_sys::console::log_1(&"[CSR] Events attached successfully!".into());

	Ok(())
}

/// Attaches events and retained bindings to the existing DOM without replacing controls.
#[cfg(wasm)]
pub(crate) fn attach_events_recursive(
	element: &Element,
	view: &crate::component::Page,
	registry: &mut super::events::EventRegistry,
) -> Result<(), HydrationError> {
	let mut controls = Vec::new();
	collect_events_recursive(element, view, registry, &mut controls)?;
	let controllers = crate::dom::control_binding::hydrate_controls(controls);
	crate::component::reactive_if::store_reactive_node(controllers);
	Ok(())
}

#[cfg(wasm)]
fn collect_events_recursive(
	element: &Element,
	view: &crate::component::Page,
	registry: &mut super::events::EventRegistry,
	controls: &mut Vec<(Element, crate::component::ControlBinding)>,
) -> Result<(), HydrationError> {
	use crate::component::Page;

	match view {
		Page::Element(el_view) => {
			for (event_type, handler) in el_view.event_handlers() {
				super::events::attach_event(element, event_type, handler.clone(), registry)
					.map_err(|error| HydrationError::EventAttachmentFailed(error.to_string()))?;
			}
			if let Some(binding) = el_view.bound_control() {
				controls.push((element.clone(), binding.clone()));
			}
			collect_child_events(element, el_view.child_views(), registry, controls)?;
		}
		Page::Fragment(children) => {
			collect_child_events(element, children, registry, controls)?;
		}
		Page::KeyedFragment(children) => {
			let children = children
				.iter()
				.map(|(_, child)| child.clone())
				.collect::<Vec<_>>();
			collect_child_events(element, &children, registry, controls)?;
		}
		Page::WithHead { view, .. } => {
			collect_events_recursive(element, view, registry, controls)?;
		}
		Page::ReactiveIf(reactive) => {
			let branch = if reactive.condition() {
				reactive.then_view()
			} else {
				reactive.else_view()
			};
			collect_events_recursive(element, &branch, registry, controls)?;
		}
		Page::Reactive(reactive) => {
			let view = reactive.render();
			collect_events_recursive(element, &view, registry, controls)?;
		}
		Page::Text(_) | Page::Empty => {}
	}
	Ok(())
}

#[cfg(wasm)]
fn collect_child_events(
	element: &Element,
	children: &[crate::component::Page],
	registry: &mut super::events::EventRegistry,
	controls: &mut Vec<(Element, crate::component::ControlBinding)>,
) -> Result<(), HydrationError> {
	let mut views = Vec::new();
	for child in children {
		collect_element_views(child, &mut views);
	}
	let elements = element.children();
	for (index, view) in views.into_iter().enumerate() {
		let child = elements
			.get(index)
			.ok_or_else(|| HydrationError::StructureMismatch {
				id: element.get_attribute(HYDRATION_ATTR_ID).unwrap_or_default(),
				expected: view.tag_name().to_owned(),
				actual: "missing child".to_owned(),
			})?;
		collect_events_recursive(
			child,
			&crate::component::Page::Element(view),
			registry,
			controls,
		)?;
	}
	Ok(())
}

#[cfg(wasm)]
fn collect_element_views(
	view: &crate::component::Page,
	elements: &mut Vec<crate::component::PageElement>,
) {
	use crate::component::Page;

	match view {
		Page::Element(element) => elements.push(element.clone()),
		Page::Fragment(children) => {
			for child in children {
				collect_element_views(child, elements);
			}
		}
		Page::KeyedFragment(children) => {
			for (_, child) in children {
				collect_element_views(child, elements);
			}
		}
		Page::WithHead { view, .. } => collect_element_views(view, elements),
		Page::ReactiveIf(reactive) => {
			let branch = if reactive.condition() {
				reactive.then_view()
			} else {
				reactive.else_view()
			};
			collect_element_views(&branch, elements);
		}
		Page::Reactive(reactive) => {
			let view = reactive.render();
			collect_element_views(&view, elements);
		}
		Page::Text(_) | Page::Empty => {}
	}
}

/// Finds all elements with hydration markers in the given root.
#[cfg(wasm)]
// Allow dead_code: WASM hydration helper reserved for future hydration runtime
#[allow(dead_code)]
pub(super) fn find_hydration_markers(root: &Element) -> Vec<(String, Element)> {
	let mut markers = Vec::new();
	find_markers_recursive(root, &mut markers);
	markers
}

#[cfg(wasm)]
fn find_markers_recursive(element: &Element, markers: &mut Vec<(String, Element)>) {
	if let Some(id) = element.get_attribute(HYDRATION_ATTR_ID) {
		markers.push((id, element.clone()));
	}

	for child in element.children() {
		find_markers_recursive(&child, markers);
	}
}

/// Non-WASM version for testing.
#[cfg(native)]
// Allow dead_code: non-WASM stub for hydration marker scanning
#[allow(dead_code)]
pub(super) fn find_hydration_markers(_root: &str) -> Vec<(String, String)> {
	Vec::new()
}

// Global hydration state management
type HydrationListener = Box<dyn Fn(bool) + 'static>;
type HydrationListeners = Vec<HydrationListener>;

thread_local! {
	static HYDRATION_COMPLETE: std::cell::RefCell<bool> = const { std::cell::RefCell::new(false) };
	static HYDRATION_LISTENERS: std::cell::RefCell<HydrationListeners> = const { std::cell::RefCell::new(Vec::new()) };
}

/// Initialize hydration state (called before hydration starts)
pub fn init_hydration_state() {
	HYDRATION_COMPLETE.with(|state| {
		*state.borrow_mut() = false;
	});
}

/// Check if hydration is complete
pub fn is_hydration_complete() -> bool {
	HYDRATION_COMPLETE.with(|state| *state.borrow())
}

/// Register a callback to be called when hydration completes
pub fn on_hydration_complete<F>(callback: F)
where
	F: Fn(bool) + 'static,
{
	HYDRATION_LISTENERS.with(|listeners| {
		listeners.borrow_mut().push(Box::new(callback));
	});
}

/// Mark hydration as complete and notify all listeners (internal)
#[cfg(wasm)]
fn mark_hydration_complete_internal() {
	HYDRATION_COMPLETE.with(|state| {
		*state.borrow_mut() = true;
	});

	// Notify all listeners
	HYDRATION_LISTENERS.with(|listeners| {
		for listener in listeners.borrow().iter() {
			listener(true);
		}
	});
}

/// Manually mark hydration as complete (public API)
///
/// This function can be called explicitly to mark hydration as complete
/// when not using the automatic hydration process (e.g., when using mount() instead of hydrate()).
#[cfg(wasm)]
pub fn mark_hydration_complete() {
	mark_hydration_complete_internal();
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_hydration_context_new() {
		let ctx = HydrationContext::new();
		assert!(!ctx.is_hydrated());
	}

	#[test]
	fn test_hydration_context_mark_hydrated() {
		let mut ctx = HydrationContext::new();
		assert!(!ctx.is_hydrated());
		ctx.mark_hydrated();
		assert!(ctx.is_hydrated());
	}

	#[test]
	fn test_hydration_context_from_state() {
		let mut state = SsrState::new();
		state.add_signal("count", 42);
		let ctx = HydrationContext::from_state(state);
		assert_eq!(ctx.get_signal("count"), Some(&serde_json::json!(42)));
	}

	#[test]
	fn test_hydration_error_display() {
		let err = HydrationError::RootNotFound("#app".to_string());
		assert_eq!(err.to_string(), "Hydration root element not found: #app");

		let err = HydrationError::StructureMismatch {
			id: "rh-0".to_string(),
			expected: "div".to_string(),
			actual: "span".to_string(),
		};
		assert!(err.to_string().contains("DOM structure mismatch"));
	}

	#[test]
	fn test_hydration_context_from_window_non_wasm() {
		// Non-WASM version should return empty context
		let ctx = HydrationContext::from_window().unwrap();
		assert!(!ctx.is_hydrated());
	}
}
