//! Hydration Runtime
//!
//! This module provides the main entry point for client-side hydration,
//! connecting reactive state with SSR-rendered DOM elements.

use crate::component::Component;
use crate::ssr::SsrState;
use std::collections::HashMap;

#[cfg(target_arch = "wasm32")]
use crate::dom::{Element, document};

#[cfg(target_arch = "wasm32")]
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

	/// Restores state from the window's SSR state object.
	#[cfg(target_arch = "wasm32")]
	pub fn from_window() -> Result<Self, HydrationError> {
		let window = web_sys::window()
			.ok_or_else(|| HydrationError::StateParseError("Window not available".to_string()))?;

		let global = window
			.get("__REINHARDT_SSR_STATE__")
			.ok_or_else(|| HydrationError::StateParseError("SSR state not found".to_string()))?;

		if global.is_undefined() || global.is_null() {
			return Ok(Self::new());
		}

		let json = js_sys::JSON::stringify(&global)
			.map_err(|_| HydrationError::StateParseError("Failed to stringify state".to_string()))?
			.as_string()
			.ok_or_else(|| {
				HydrationError::StateParseError("Failed to convert state to string".to_string())
			})?;

		let state = SsrState::from_json(&json)
			.map_err(|e| HydrationError::StateParseError(e.to_string()))?;

		Ok(Self::from_state(state))
	}

	/// Non-WASM version that returns an empty context.
	#[cfg(not(target_arch = "wasm32"))]
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
#[cfg(target_arch = "wasm32")]
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
		.map_err(|e| HydrationError::StateParseError(format!("Reconciliation failed: {:?}", e)))?;
	web_sys::console::log_1(&"[Hydration] Reconciliation complete".into());

	// 4. Attach event handlers
	let mut registry = EventRegistry::new();
	attach_events_recursive(root, &view, &mut registry)?;
	web_sys::console::log_1(&"[Hydration] Events attached".into());

	// 5. Mark hydration complete
	context.mark_hydrated();
	mark_hydration_complete_internal();
	web_sys::console::log_1(&"[Hydration] Complete!".into());

	Ok(())
}

/// Non-WASM version for testing.
#[cfg(not(target_arch = "wasm32"))]
pub fn hydrate<C: Component>(_component: &C, _root: &str) -> Result<(), HydrationError> {
	Ok(())
}

/// Hydrates a component at the default root element (#app).
#[cfg(target_arch = "wasm32")]
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
#[cfg(not(target_arch = "wasm32"))]
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
#[cfg(target_arch = "wasm32")]
pub fn attach_events_to_mounted_view(
	element: &Element,
	view: &crate::component::Page,
) -> Result<(), HydrationError> {
	use super::events::EventRegistry;

	web_sys::console::log_1(&"[CSR] Attaching events to mounted view...".into());

	let mut registry = EventRegistry::new();
	attach_events_recursive(element, view, &mut registry)?;

	web_sys::console::log_1(&"[CSR] Events attached successfully!".into());

	Ok(())
}

/// Recursively attaches event handlers to DOM elements.
///
/// This function can be used for both SSR+Hydration and CSR (client-side rendering only) scenarios.
/// For CSR, call this after mounting a view to attach event handlers.
#[cfg(target_arch = "wasm32")]
pub(crate) fn attach_events_recursive(
	element: &Element,
	view: &crate::component::Page,
	registry: &mut super::events::EventRegistry,
) -> Result<(), HydrationError> {
	use super::events::attach_event;
	use crate::component::Page;

	match view {
		Page::Element(el_view) => {
			let tag = el_view.tag_name();
			let event_count = el_view.event_handlers().len();

			if event_count > 0 {
				web_sys::console::log_1(
					&format!("[attach_events] {} has {} event handlers", tag, event_count).into(),
				);
			}

			// Attach events from the view's event handlers
			for (event_type, handler) in el_view.event_handlers() {
				web_sys::console::log_1(
					&format!("[attach_events] Attaching {:?} to {}", event_type, tag).into(),
				);

				attach_event(element, event_type, handler.clone(), registry)
					.map_err(|e| HydrationError::EventAttachmentFailed(e.to_string()))?;
			}

			// Recursively process children
			let children = element.children();
			let view_children = el_view.child_views();

			for (i, child_view) in view_children.iter().enumerate() {
				if i < children.len() {
					attach_events_recursive(&children[i], child_view, registry)?;
				}
			}
		}
		Page::Fragment(views) => {
			let children = element.children();
			for (i, child_view) in views.iter().enumerate() {
				if i < children.len() {
					attach_events_recursive(&children[i], child_view, registry)?;
				}
			}
		}
		Page::Text(_) | Page::Empty => {
			// No events to attach
		}
		Page::WithHead { view, .. } => {
			// Head section doesn't have event handlers
			// Attach events to the inner view
			attach_events_recursive(element, view, registry)?;
		}
		Page::ReactiveIf(reactive_if) => {
			// For hydration, evaluate the condition and attach events to the rendered branch
			let branch_view = if reactive_if.condition() {
				reactive_if.then_view()
			} else {
				reactive_if.else_view()
			};
			attach_events_recursive(element, &branch_view, registry)?;
		}
		Page::Reactive(reactive) => {
			// For hydration, evaluate the render closure and attach events to the resulting view
			let rendered_view = reactive.render();
			attach_events_recursive(element, &rendered_view, registry)?;
		}
	}

	Ok(())
}

/// Finds all elements with hydration markers in the given root.
#[cfg(target_arch = "wasm32")]
// Allow dead_code: WASM hydration helper reserved for future hydration runtime
#[allow(dead_code)]
pub(super) fn find_hydration_markers(root: &Element) -> Vec<(String, Element)> {
	let mut markers = Vec::new();
	find_markers_recursive(root, &mut markers);
	markers
}

#[cfg(target_arch = "wasm32")]
fn find_markers_recursive(element: &Element, markers: &mut Vec<(String, Element)>) {
	if let Some(id) = element.get_attribute(HYDRATION_ATTR_ID) {
		markers.push((id, element.clone()));
	}

	for child in element.children() {
		find_markers_recursive(&child, markers);
	}
}

/// Non-WASM version for testing.
#[cfg(not(target_arch = "wasm32"))]
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
#[cfg(target_arch = "wasm32")]
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
#[cfg(target_arch = "wasm32")]
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
