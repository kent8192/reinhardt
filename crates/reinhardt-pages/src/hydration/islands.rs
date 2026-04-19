//! Island Detection for Partial Hydration (Phase 2-B Step 2)
//!
//! This module provides mechanisms to detect and manage interactive islands
//! in the DOM for selective hydration. Islands are components marked with
//! `data-rh-island="true"` during SSR that require client-side hydration.
//!
//! ## Architecture
//!
//! ```text
//! SSR Output:
//! <div data-rh-island="true" data-rh-id="rh-0">
//!   <button>Click me</button>  <!-- Interactive -->
//! </div>
//! <div data-rh-static="true" data-rh-id="rh-1">
//!   <p>Static content</p>      <!-- No hydration -->
//! </div>
//!
//! Island Detection:
//! IslandDetector → [IslandNode] → Selective Hydration
//! ```

#[cfg(wasm)]
use wasm_bindgen::JsCast;
#[cfg(wasm)]
use web_sys::{Document, Element};

/// Represents an interactive island in the DOM.
///
/// An island is a component that requires client-side hydration,
/// marked with `data-rh-island="true"` during SSR.
#[derive(Debug, Clone)]
pub struct IslandNode {
	/// The DOM element representing this island.
	#[cfg(wasm)]
	pub element: Element,
	/// Hydration ID from data-rh-id attribute.
	pub hydration_id: String,
	/// Component name from data-rh-component attribute (if available).
	pub component_name: Option<String>,
}

#[cfg(wasm)]
impl IslandNode {
	/// Creates a new IslandNode from a DOM element.
	///
	/// # Arguments
	///
	/// * `element` - The DOM element marked as an island
	///
	/// # Returns
	///
	/// `Some(IslandNode)` if the element has a valid hydration ID, `None` otherwise.
	pub fn from_element(element: Element) -> Option<Self> {
		let hydration_id = element.get_attribute("data-rh-id")?;
		let component_name = element.get_attribute("data-rh-component");

		Some(Self {
			element,
			hydration_id,
			component_name,
		})
	}

	/// Checks if this island contains the given element.
	pub fn contains(&self, element: &Element) -> bool {
		self.element.contains(Some(element))
	}
}

/// Island detector for finding interactive islands in the DOM.
///
/// The detector scans the DOM for elements marked with `data-rh-island="true"`
/// and creates `IslandNode` instances for selective hydration.
#[cfg(wasm)]
pub struct IslandDetector {
	document: Document,
}

#[cfg(wasm)]
impl IslandDetector {
	/// Creates a new island detector.
	///
	/// # Arguments
	///
	/// * `document` - The DOM document to scan
	pub fn new(document: Document) -> Self {
		Self { document }
	}

	/// Finds all island elements in the document.
	///
	/// Returns a list of `IslandNode` instances, excluding nested islands
	/// (only top-level islands are returned).
	///
	/// # Example
	///
	/// ```ignore
	/// let detector = IslandDetector::new(window.document());
	/// let islands = detector.find_islands();
	/// for island in islands {
	///     println!("Found island: {:?}", island.hydration_id);
	/// }
	/// ```
	pub fn find_islands(&self) -> Vec<IslandNode> {
		let mut islands = Vec::new();

		// Query all elements with data-rh-island="true"
		if let Ok(node_list) = self.document.query_selector_all("[data-rh-island='true']") {
			for i in 0..node_list.length() {
				if let Some(node) = node_list.item(i) {
					if let Some(element) = node.dyn_ref::<Element>() {
						// Check if this element is nested within another island
						if !self.is_within_island(element, &islands) {
							if let Some(island_node) = IslandNode::from_element(element.clone()) {
								islands.push(island_node);
							}
						}
					}
				}
			}
		}

		islands
	}

	/// Finds all static content elements in the document.
	///
	/// Returns a list of elements marked with `data-rh-static="true"`.
	pub fn find_static_nodes(&self) -> Vec<Element> {
		let mut static_nodes = Vec::new();

		if let Ok(node_list) = self.document.query_selector_all("[data-rh-static='true']") {
			for i in 0..node_list.length() {
				if let Some(node) = node_list.item(i) {
					if let Some(element) = node.dyn_ref::<Element>() {
						static_nodes.push(element.clone());
					}
				}
			}
		}

		static_nodes
	}

	/// Checks if an element is within any of the given islands.
	///
	/// This is used to exclude nested islands from the top-level list.
	fn is_within_island(&self, element: &Element, islands: &[IslandNode]) -> bool {
		for island in islands {
			if island.contains(element) && &island.element != element {
				return true;
			}
		}
		false
	}

	/// Checks if an element is within an island.
	///
	/// This is a public helper method for determining if a given element
	/// is contained within any island boundary.
	pub fn is_element_within_island(&self, element: &Element) -> bool {
		let islands = self.find_islands();
		for island in &islands {
			if island.contains(element) && &island.element != element {
				return true;
			}
		}
		false
	}
}

// Compilation-only implementations for non-WASM targets
#[cfg(native)]
impl IslandNode {
	/// Compilation-only implementation for non-WASM targets.
	pub fn from_element(_element: ()) -> Option<Self> {
		None
	}
}

/// Compilation-only implementation of IslandDetector for non-WASM targets.
///
/// This provides type compatibility for cross-compilation scenarios.
/// Actual island detection requires WASM environment with DOM access.
#[cfg(native)]
pub struct IslandDetector;

#[cfg(native)]
impl IslandDetector {
	/// Compilation-only implementation for non-WASM targets.
	pub fn new(_document: ()) -> Self {
		Self
	}

	/// Compilation-only implementation for non-WASM targets.
	pub fn find_islands(&self) -> Vec<IslandNode> {
		Vec::new()
	}

	/// Compilation-only implementation for non-WASM targets.
	pub fn find_static_nodes(&self) -> Vec<()> {
		Vec::new()
	}

	/// Compilation-only implementation for non-WASM targets.
	pub fn is_element_within_island(&self, _element: &()) -> bool {
		false
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_island_node_struct() {
		// Test that IslandNode can be created (compilation test)
		#[cfg(wasm)]
		{
			use wasm_bindgen_test::*;
			// Actual WASM tests would go here
		}

		// Non-WASM: just ensure the module compiles
		#[cfg(native)]
		{
			let _detector = IslandDetector::new(());
			let islands = _detector.find_islands();
			assert_eq!(islands.len(), 0);
		}
	}
}
