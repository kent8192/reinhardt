//! DOM Reconciliation for Hydration
//!
//! This module verifies that SSR-rendered DOM matches the expected
//! component structure during hydration.

use crate::component::Page;

#[cfg(wasm)]
use crate::component::ControlKind;
#[cfg(wasm)]
use crate::dom::Element;
#[cfg(wasm)]
use reinhardt_core::types::page::{PageElement, is_boolean_attr, is_boolean_attr_truthy};
#[cfg(wasm)]
use wasm_bindgen::JsCast;

/// Breadcrumb context for hydration reconciliation diagnostics.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ReconcilePath {
	component: Option<String>,
	segments: Vec<String>,
}

impl ReconcilePath {
	/// Creates a root reconciliation path.
	pub fn root() -> Self {
		Self::default()
	}

	/// Adds or replaces the component name for this path.
	pub fn with_component(mut self, component: impl Into<String>) -> Self {
		self.component = Some(component.into());
		self
	}

	/// Appends a child-index segment to this path.
	pub fn with_child(mut self, index: usize) -> Self {
		self.segments.push(format!("child[{}]", index));
		self
	}

	/// Appends an element-tag segment to this path.
	pub fn with_element(mut self, tag: impl Into<String>) -> Self {
		self.segments.push(tag.into());
		self
	}

	fn describe(&self) -> String {
		let path = if self.segments.is_empty() {
			"root".to_string()
		} else {
			self.segments.join(" > ")
		};

		if let Some(component) = &self.component {
			format!("in {} at {}", component, path)
		} else {
			format!("at {}", path)
		}
	}
}

/// Errors that can occur during reconciliation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReconcileError {
	/// Tag name mismatch.
	TagMismatch {
		/// Path to the mismatched node.
		path: ReconcilePath,
		/// Expected tag name.
		expected: String,
		/// Actual tag name.
		actual: String,
	},
	/// Child count mismatch.
	ChildCountMismatch {
		/// Path to the mismatched node.
		path: ReconcilePath,
		/// Expected count.
		expected: usize,
		/// Actual count.
		actual: usize,
	},
	/// Text content mismatch.
	TextMismatch {
		/// Path to the mismatched node.
		path: ReconcilePath,
		/// Expected text.
		expected: String,
		/// Actual text.
		actual: String,
	},
	/// Attribute mismatch.
	AttributeMismatch {
		/// Path to the mismatched node.
		path: ReconcilePath,
		/// Attribute name.
		name: String,
		/// Expected value.
		expected: Option<String>,
		/// Actual value.
		actual: Option<String>,
	},
	/// Element not found at expected position.
	ElementNotFound {
		/// Path to the parent node.
		path: ReconcilePath,
		/// Position index.
		index: usize,
	},
}

impl std::fmt::Display for ReconcileError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::TagMismatch {
				path,
				expected,
				actual,
			} => {
				write!(
					f,
					"Hydration mismatch {}: tag expected '{}', found '{}'",
					path.describe(),
					expected,
					actual
				)
			}
			Self::ChildCountMismatch {
				path,
				expected,
				actual,
			} => {
				write!(
					f,
					"Hydration mismatch {}: child count expected {}, found {}",
					path.describe(),
					expected,
					actual
				)
			}
			Self::TextMismatch {
				path,
				expected,
				actual,
			} => {
				write!(
					f,
					"Hydration mismatch {}: text expected '{}', found '{}'",
					path.describe(),
					expected,
					actual
				)
			}
			Self::AttributeMismatch {
				path,
				name,
				expected,
				actual,
			} => {
				write!(
					f,
					"Hydration mismatch {}: attribute '{}' expected {:?}, found {:?}",
					path.describe(),
					name,
					expected,
					actual
				)
			}
			Self::ElementNotFound { path, index } => {
				write!(
					f,
					"Hydration mismatch {}: element not found at index {}",
					path.describe(),
					index
				)
			}
		}
	}
}

impl std::error::Error for ReconcileError {}

/// Options for reconciliation (Phase 2-B).
///
/// Controls how reconciliation is performed during hydration,
/// enabling selective reconciliation for Island Architecture.
#[derive(Debug, Clone)]
pub struct ReconcileOptions {
	/// If true, only reconcile islands (interactive components).
	/// Static content and full-hydration components are skipped.
	pub island_only: bool,

	/// If true, skip elements marked with `data-rh-static="true"`.
	/// This is useful for preserving server-rendered static content.
	pub skip_static: bool,

	/// If true, log warnings for mismatches instead of failing.
	/// Useful for graceful degradation.
	pub warn_on_mismatch: bool,
}

impl Default for ReconcileOptions {
	fn default() -> Self {
		Self {
			island_only: false,
			skip_static: false,
			warn_on_mismatch: true,
		}
	}
}

impl ReconcileOptions {
	/// Creates options for island-only reconciliation.
	pub fn island_only() -> Self {
		Self {
			island_only: true,
			skip_static: true,
			warn_on_mismatch: true,
		}
	}

	/// Creates options for full reconciliation (default).
	pub fn full_reconciliation() -> Self {
		Self::default()
	}

	/// Sets the warn_on_mismatch option.
	pub fn warn_on_mismatch(mut self, warn: bool) -> Self {
		self.warn_on_mismatch = warn;
		self
	}
}

/// Reconciles the existing DOM with the expected Page structure.
///
/// This function verifies that the SSR-rendered HTML matches what
/// the component would render, returning detailed mismatch diagnostics
/// so callers can decide whether to fail or warn.
#[cfg(wasm)]
pub fn reconcile(element: &Element, view: &Page) -> Result<(), ReconcileError> {
	reconcile_at_path(element, view, ReconcilePath::root(), false)
}

#[cfg(wasm)]
fn reconcile_at_path(
	element: &Element,
	view: &Page,
	path: ReconcilePath,
	inside_controlled_select: bool,
) -> Result<(), ReconcileError> {
	match view {
		Page::Element(el_view) => {
			reconcile_element_at_path(element, el_view, path, inside_controlled_select)
		}
		Page::Text(expected_text) => reconcile_text_at_path(
			element.text_content().unwrap_or_default(),
			expected_text,
			path,
		),
		Page::Fragment(views) => {
			reconcile_children_at_path(element, views, path, inside_controlled_select)
		}
		Page::KeyedFragment(views) => {
			let child_views: Vec<Page> = views.iter().map(|(_, view)| view.clone()).collect();
			reconcile_children_at_path(element, &child_views, path, inside_controlled_select)
		}
		Page::Outlet(outlet) => {
			if let Some(child) = outlet.child() {
				reconcile_at_path(element, child, path, inside_controlled_select)
			} else {
				Ok(())
			}
		}
		Page::Empty => Ok(()),
		Page::WithHead { view, .. } => {
			// Head section is handled separately during SSR
			// For hydration, just reconcile the inner view
			reconcile_at_path(element, view, path, inside_controlled_select)
		}
		#[cfg(feature = "hmr")]
		Page::DevTemplate { view, .. } | Page::DevSlot { view, .. } => {
			reconcile_at_path(element, view, path, inside_controlled_select)
		}
		Page::ReactiveIf(reactive_if) => {
			// For hydration, evaluate the condition and reconcile the rendered branch.
			// SSR rendered one branch based on the initial condition value.
			let branch_view = if reactive_if.condition() {
				reactive_if.then_view()
			} else {
				reactive_if.else_view()
			};
			reconcile_at_path(element, &branch_view, path, inside_controlled_select)
		}
		Page::Reactive(reactive) => {
			// For hydration, evaluate the render closure and reconcile the resulting view.
			// SSR rendered the initial view from the closure.
			let rendered_view = reactive.render();
			reconcile_at_path(element, &rendered_view, path, inside_controlled_select)
		}
		Page::Suspense(node) => {
			let branch_view = node.render_branch();
			reconcile_at_path(element, &branch_view, path, inside_controlled_select)
		}
		Page::Deferred(node) => {
			let content_view = node.content();
			reconcile_at_path(element, &content_view, path, inside_controlled_select)
		}
	}
}

#[cfg(wasm)]
fn reconcile_element_at_path(
	element: &Element,
	el_view: &PageElement,
	path: ReconcilePath,
	inside_controlled_select: bool,
) -> Result<(), ReconcileError> {
	let actual_tag = element.tag_name().to_lowercase();
	let expected_tag = el_view.tag_name().to_lowercase();
	let element_path = path_with_dom_component(element, path).with_element(expected_tag.clone());

	if actual_tag != expected_tag {
		return Err(ReconcileError::TagMismatch {
			path: element_path,
			expected: expected_tag,
			actual: actual_tag,
		});
	}

	reconcile_attrs_at_path(
		element,
		el_view,
		element_path.clone(),
		inside_controlled_select,
	)?;
	if el_view.tag_name().eq_ignore_ascii_case("textarea") && el_view.bound_control().is_some() {
		return Ok(());
	}
	reconcile_children_at_path(
		element,
		el_view.child_views(),
		element_path,
		inside_controlled_select || is_controlled_select(el_view),
	)
}

#[cfg(wasm)]
fn reconcile_dom_node_at_path(
	node: &web_sys::Node,
	view: &Page,
	path: ReconcilePath,
	inside_controlled_select: bool,
) -> Result<(), ReconcileError> {
	match view {
		Page::Element(el_view) => {
			if let Some(element) = node.dyn_ref::<web_sys::Element>() {
				reconcile_element_at_path(
					&Element::new(element.clone()),
					el_view,
					path,
					inside_controlled_select,
				)
			} else {
				Err(ReconcileError::TagMismatch {
					path,
					expected: el_view.tag_name().to_lowercase(),
					actual: node.node_name().to_lowercase(),
				})
			}
		}
		Page::Text(expected_text) => {
			reconcile_text_at_path(node.text_content().unwrap_or_default(), expected_text, path)
		}
		Page::Fragment(views) => {
			if let Some(element) = node.dyn_ref::<web_sys::Element>() {
				reconcile_children_at_path(
					&Element::new(element.clone()),
					views,
					path,
					inside_controlled_select,
				)
			} else {
				let mut expected_children = Vec::new();
				collect_expected_children(views, &path, &mut expected_children);
				if expected_children.len() == 1 {
					reconcile_dom_node_at_path(
						node,
						&expected_children[0].1,
						expected_children[0].0.clone(),
						inside_controlled_select,
					)
				} else {
					Err(ReconcileError::ChildCountMismatch {
						path,
						expected: expected_children.len(),
						actual: 1,
					})
				}
			}
		}
		Page::KeyedFragment(views) => {
			let child_views: Vec<Page> = views.iter().map(|(_, view)| view.clone()).collect();
			if let Some(element) = node.dyn_ref::<web_sys::Element>() {
				reconcile_children_at_path(
					&Element::new(element.clone()),
					&child_views,
					path,
					inside_controlled_select,
				)
			} else {
				let mut expected_children = Vec::new();
				collect_expected_children(&child_views, &path, &mut expected_children);
				if expected_children.len() == 1 {
					reconcile_dom_node_at_path(
						node,
						&expected_children[0].1,
						expected_children[0].0.clone(),
						inside_controlled_select,
					)
				} else {
					Err(ReconcileError::ChildCountMismatch {
						path,
						expected: expected_children.len(),
						actual: 1,
					})
				}
			}
		}
		Page::Outlet(outlet) => {
			if let Some(child) = outlet.child() {
				reconcile_dom_node_at_path(node, child, path, inside_controlled_select)
			} else {
				Ok(())
			}
		}
		Page::Empty => Ok(()),
		Page::WithHead { view, .. } => {
			reconcile_dom_node_at_path(node, view, path, inside_controlled_select)
		}
		#[cfg(feature = "hmr")]
		Page::DevTemplate { view, .. } | Page::DevSlot { view, .. } => {
			reconcile_dom_node_at_path(node, view, path, inside_controlled_select)
		}
		Page::ReactiveIf(reactive_if) => {
			let branch_view = if reactive_if.condition() {
				reactive_if.then_view()
			} else {
				reactive_if.else_view()
			};
			reconcile_dom_node_at_path(node, &branch_view, path, inside_controlled_select)
		}
		Page::Reactive(reactive) => {
			let rendered_view = reactive.render();
			reconcile_dom_node_at_path(node, &rendered_view, path, inside_controlled_select)
		}
		Page::Suspense(suspense_node) => {
			let branch_view = suspense_node.render_branch();
			reconcile_dom_node_at_path(node, &branch_view, path, inside_controlled_select)
		}
		Page::Deferred(deferred_node) => {
			let content_view = deferred_node.content();
			reconcile_dom_node_at_path(node, &content_view, path, inside_controlled_select)
		}
	}
}

#[cfg(wasm)]
fn reconcile_attrs_at_path(
	element: &Element,
	el_view: &PageElement,
	path: ReconcilePath,
	inside_controlled_select: bool,
) -> Result<(), ReconcileError> {
	for (index, (name, value)) in el_view.attrs().iter().enumerate() {
		if !crate::component::into_page::static_attribute_is_effective(el_view.attrs(), index) {
			continue;
		}
		let name_str = name.as_ref();
		let has_reactive_override = el_view
			.reactive_attrs()
			.iter()
			.any(|attribute| attribute.name().eq_ignore_ascii_case(name_str));
		if inside_controlled_select
			&& el_view.tag_name().eq_ignore_ascii_case("option")
			&& name_str.eq_ignore_ascii_case("selected")
		{
			continue;
		}
		if has_reactive_override {
			continue;
		}
		if crate::component::into_page::controlled_attribute_is_overridden(
			el_view.bound_control(),
			name_str,
		) {
			continue;
		}
		let expected = expected_dom_attr_value(name_str, value.as_ref());
		let actual = element.get_attribute(name_str);

		if actual != expected {
			return Err(ReconcileError::AttributeMismatch {
				path,
				name: name_str.to_string(),
				expected,
				actual,
			});
		}
	}

	Ok(())
}

#[cfg(wasm)]
fn reconcile_children_at_path(
	element: &Element,
	child_views: &[Page],
	path: ReconcilePath,
	inside_controlled_select: bool,
) -> Result<(), ReconcileError> {
	let actual_nodes = relevant_child_nodes(element);
	let mut expected_children = Vec::new();
	collect_expected_children(child_views, &path, &mut expected_children);

	for (index, (child_path, child_view)) in expected_children.iter().enumerate() {
		let Some(actual_node) = actual_nodes.get(index) else {
			return Err(ReconcileError::ElementNotFound { path, index });
		};
		reconcile_dom_node_at_path(
			actual_node,
			child_view,
			child_path.clone(),
			inside_controlled_select,
		)?;
	}

	if actual_nodes.len() != expected_children.len() {
		return Err(ReconcileError::ChildCountMismatch {
			path,
			expected: expected_children.len(),
			actual: actual_nodes.len(),
		});
	}

	Ok(())
}

#[cfg(wasm)]
fn is_controlled_select(element: &PageElement) -> bool {
	element.tag_name().eq_ignore_ascii_case("select")
		&& matches!(
			element.bound_control().map(|binding| binding.kind()),
			Some(ControlKind::SelectOne | ControlKind::SelectMany)
		)
}

#[cfg(wasm)]
fn reconcile_text_at_path(
	actual_text: String,
	expected_text: &str,
	path: ReconcilePath,
) -> Result<(), ReconcileError> {
	let expected_normalized = normalize_whitespace(expected_text);
	let actual_normalized = normalize_whitespace(&actual_text);

	if expected_normalized != actual_normalized {
		return Err(ReconcileError::TextMismatch {
			path,
			expected: expected_text.to_string(),
			actual: actual_text,
		});
	}

	Ok(())
}

// Allow dead_code: native library builds do not execute wasm reconciliation, but
// this helper is used by wasm runtime reconciliation and native regression tests.
#[allow(dead_code)]
fn collect_expected_children(
	views: &[Page],
	parent_path: &ReconcilePath,
	children: &mut Vec<(ReconcilePath, Page)>,
) {
	for (index, view) in views.iter().enumerate() {
		let child_path = parent_path.clone().with_child(index);
		collect_expected_child(view, child_path, children);
	}
}

// Allow dead_code: native library builds do not execute wasm reconciliation, but
// this helper is used by wasm runtime reconciliation and native regression tests.
#[allow(dead_code)]
fn collect_expected_child(
	view: &Page,
	path: ReconcilePath,
	children: &mut Vec<(ReconcilePath, Page)>,
) {
	match view {
		Page::Empty => {}
		Page::Fragment(fragment_children) => {
			collect_expected_children(fragment_children, &path, children);
		}
		Page::WithHead { view, .. } => {
			collect_expected_child(view, path, children);
		}
		Page::ReactiveIf(reactive_if) => {
			let branch_view = if reactive_if.condition() {
				reactive_if.then_view()
			} else {
				reactive_if.else_view()
			};
			collect_expected_child(&branch_view, path, children);
		}
		Page::Reactive(reactive) => {
			let rendered_view = reactive.render();
			collect_expected_child(&rendered_view, path, children);
		}
		Page::Suspense(node) => {
			let branch_view = node.render_branch();
			collect_expected_child(&branch_view, path, children);
		}
		Page::Deferred(node) => {
			let content_view = node.content();
			collect_expected_child(&content_view, path, children);
		}
		Page::Text(text) => {
			if let Some((_, Page::Text(previous_text))) = children.last_mut() {
				*previous_text = format!("{}{}", previous_text.as_ref(), text.as_ref()).into();
			} else {
				children.push((path, Page::Text(text.clone())));
			}
		}
		Page::KeyedFragment(keyed_children) => {
			let child_views: Vec<Page> = keyed_children
				.iter()
				.map(|(_, view)| view.clone())
				.collect();
			collect_expected_children(&child_views, &path, children);
		}
		_ => children.push((path, view.clone())),
	}
}

#[cfg(wasm)]
fn relevant_child_nodes(element: &Element) -> Vec<web_sys::Node> {
	let child_nodes = element.as_web_sys().child_nodes();
	(0..child_nodes.length())
		.filter_map(|index| child_nodes.item(index))
		.filter(is_relevant_child_node)
		.collect()
}

#[cfg(wasm)]
fn is_relevant_child_node(node: &web_sys::Node) -> bool {
	match node.node_type() {
		web_sys::Node::ELEMENT_NODE => true,
		web_sys::Node::TEXT_NODE => {
			!normalize_whitespace(&node.text_content().unwrap_or_default()).is_empty()
		}
		_ => false,
	}
}

#[cfg(wasm)]
fn path_with_dom_component(element: &Element, path: ReconcilePath) -> ReconcilePath {
	match element.get_attribute("data-rh-component") {
		Some(component) if !component.is_empty() => path.with_component(component),
		_ => path,
	}
}

#[cfg(wasm)]
fn expected_dom_attr_value(name: &str, value: &str) -> Option<String> {
	if is_boolean_attr(name) && !is_boolean_attr_truthy(value) {
		None
	} else {
		Some(value.to_string())
	}
}

/// Reconciles the existing DOM with the expected Page structure with options (Phase 2-B).
///
/// This function performs selective reconciliation based on the provided options,
/// enabling Island Architecture by reconciling only interactive components.
///
/// # Arguments
///
/// * `element` - The DOM element to reconcile
/// * `view` - The expected view structure
/// * `options` - Reconciliation options
///
/// # Returns
///
/// `Ok(())` if reconciliation succeeds, or a `ReconcileError` if a mismatch is found.
///
/// # Behavior
///
/// - If `options.island_only` is true, only elements with `data-rh-island="true"` are reconciled
/// - If `options.skip_static` is true, elements with `data-rh-static="true"` are skipped
/// - If `options.warn_on_mismatch` is true, mismatches are logged as warnings instead of errors
#[cfg(wasm)]
pub fn reconcile_with_options(
	element: &Element,
	view: &Page,
	options: &ReconcileOptions,
) -> Result<(), ReconcileError> {
	reconcile_with_options_at_path(element, view, options, ReconcilePath::root(), false)
}

#[cfg(wasm)]
fn reconcile_with_options_at_path(
	element: &Element,
	view: &Page,
	options: &ReconcileOptions,
	path: ReconcilePath,
	inside_controlled_select: bool,
) -> Result<(), ReconcileError> {
	// Check if this element should be skipped
	let should_skip = if options.skip_static {
		element.get_attribute("data-rh-static").as_deref() == Some("true")
	} else {
		false
	};

	if should_skip {
		return Ok(());
	}

	// Check if this is an island element
	let is_island = element.get_attribute("data-rh-island").as_deref() == Some("true");

	// Determine if we should reconcile this element
	let should_reconcile = if options.island_only {
		// In island-only mode, only reconcile island elements
		is_island
	} else {
		// In full reconciliation mode, reconcile all non-static elements
		true
	};

	// Perform reconciliation if applicable
	if should_reconcile
		&& let Err(err) = reconcile_at_path(element, view, path.clone(), inside_controlled_select)
	{
		handle_reconcile_error(err, options)?;
	}

	// Recursively process children, unless this is an island boundary
	let should_recurse = if options.island_only && is_island {
		// If we're in island-only mode and this is an island,
		// don't recurse into children (they belong to this island's internal reconciliation)
		false
	} else {
		// Otherwise, recurse into children
		true
	};

	if should_recurse {
		reconcile_options_children_at_path(element, view, options, path, inside_controlled_select)?;
	}

	Ok(())
}

#[cfg(wasm)]
fn reconcile_options_children_at_path(
	element: &Element,
	view: &Page,
	options: &ReconcileOptions,
	path: ReconcilePath,
	inside_controlled_select: bool,
) -> Result<(), ReconcileError> {
	let keyed_child_views;
	let child_views: &[Page] = match view {
		Page::Element(el_view) => el_view.child_views(),
		Page::Fragment(views) => views,
		Page::KeyedFragment(views) => {
			keyed_child_views = views
				.iter()
				.map(|(_, view)| view.clone())
				.collect::<Vec<_>>();
			&keyed_child_views
		}
		Page::WithHead { view, .. } => {
			return reconcile_options_children_at_path(
				element,
				view,
				options,
				path,
				inside_controlled_select,
			);
		}
		#[cfg(feature = "hmr")]
		Page::DevTemplate { view, .. } | Page::DevSlot { view, .. } => {
			return reconcile_options_children_at_path(
				element,
				view,
				options,
				path,
				inside_controlled_select,
			);
		}
		Page::ReactiveIf(reactive_if) => {
			let branch_view = if reactive_if.condition() {
				reactive_if.then_view()
			} else {
				reactive_if.else_view()
			};
			return reconcile_options_children_at_path(
				element,
				&branch_view,
				options,
				path,
				inside_controlled_select,
			);
		}
		Page::Reactive(reactive) => {
			let rendered_view = reactive.render();
			return reconcile_options_children_at_path(
				element,
				&rendered_view,
				options,
				path,
				inside_controlled_select,
			);
		}
		Page::Suspense(node) => {
			let branch_view = node.render_branch();
			return reconcile_options_children_at_path(
				element,
				&branch_view,
				options,
				path,
				inside_controlled_select,
			);
		}
		Page::Deferred(node) => {
			let content_view = node.content();
			return reconcile_options_children_at_path(
				element,
				&content_view,
				options,
				path,
				inside_controlled_select,
			);
		}
		Page::Outlet(outlet) => {
			if let Some(child) = outlet.child() {
				return reconcile_options_children_at_path(
					element,
					child,
					options,
					path,
					inside_controlled_select,
				);
			}
			return Ok(());
		}
		Page::Text(_) | Page::Empty => return Ok(()),
	};

	let actual_nodes = relevant_child_nodes(element);
	let mut expected_children = Vec::new();
	let path = path_with_dom_component(element, path);
	let parent_path = match view {
		Page::Element(el_view) => path.with_element(el_view.tag_name().to_lowercase()),
		_ => path,
	};
	let children_inside_controlled_select = inside_controlled_select
		|| matches!(view, Page::Element(element) if is_controlled_select(element));
	collect_expected_children(child_views, &parent_path, &mut expected_children);

	for (index, (child_path, child_view)) in expected_children.iter().enumerate() {
		let Some(actual_node) = actual_nodes.get(index) else {
			return handle_reconcile_error(
				ReconcileError::ElementNotFound {
					path: parent_path,
					index,
				},
				options,
			);
		};

		if let Some(child_element) = actual_node.dyn_ref::<web_sys::Element>() {
			reconcile_with_options_at_path(
				&Element::new(child_element.clone()),
				child_view,
				options,
				child_path.clone(),
				children_inside_controlled_select,
			)?;
		} else if matches!(child_view, Page::Element(_))
			&& let Err(err) = reconcile_dom_node_at_path(
				actual_node,
				child_view,
				child_path.clone(),
				children_inside_controlled_select,
			) {
			handle_reconcile_error(err, options)?;
		}
	}

	if actual_nodes.len() > expected_children.len() {
		handle_reconcile_error(
			ReconcileError::ChildCountMismatch {
				path: parent_path,
				expected: expected_children.len(),
				actual: actual_nodes.len(),
			},
			options,
		)?;
	}

	Ok(())
}

#[cfg(wasm)]
fn handle_reconcile_error(
	err: ReconcileError,
	options: &ReconcileOptions,
) -> Result<(), ReconcileError> {
	if options.warn_on_mismatch {
		#[cfg(debug_assertions)]
		web_sys::console::warn_1(&format!("Reconciliation warning: {}", err).into());
		Ok(())
	} else {
		Err(err)
	}
}

/// Non-WASM version for testing.
#[cfg(native)]
pub fn reconcile(_element: &str, _view: &Page) -> Result<(), ReconcileError> {
	Ok(())
}

/// Non-WASM version for testing (Phase 2-B).
#[cfg(native)]
pub fn reconcile_with_options(
	_element: &str,
	_view: &Page,
	_options: &ReconcileOptions,
) -> Result<(), ReconcileError> {
	Ok(())
}

/// Normalizes whitespace for comparison.
// Allow dead_code: utility function used by reconciliation comparison logic
#[allow(dead_code)]
fn normalize_whitespace(s: &str) -> String {
	s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Checks if an element's structure matches the view.
#[cfg(wasm)]
// Allow dead_code: WASM structure comparison reserved for future reconciliation
#[allow(dead_code)]
pub(super) fn structure_matches(element: &Element, view: &Page) -> bool {
	reconcile(element, view).is_ok()
}

/// Non-WASM version for testing.
#[cfg(native)]
// Allow dead_code: non-WASM stub for structure comparison
#[allow(dead_code)]
pub(super) fn structure_matches(_element: &str, _view: &Page) -> bool {
	true
}

/// Detailed comparison result.
#[derive(Debug, Clone)]
// Allow dead_code: result type for DOM/view structure comparison
#[allow(dead_code)]
pub(super) struct CompareResult {
	/// Whether the structures match.
	pub matches: bool,
	/// List of differences found.
	pub differences: Vec<String>,
}

// Allow dead_code: impl block for CompareResult utility methods
#[allow(dead_code)]
impl CompareResult {
	/// Creates a successful match result.
	pub(super) fn success() -> Self {
		Self {
			matches: true,
			differences: Vec::new(),
		}
	}

	/// Creates a failed match result with differences.
	pub(super) fn failure(differences: Vec<String>) -> Self {
		Self {
			matches: false,
			differences,
		}
	}
}

/// Compares DOM structure with view and returns detailed results.
#[cfg(wasm)]
// Allow dead_code: WASM structure comparison reserved for future reconciliation
#[allow(dead_code)]
pub(super) fn compare_structure(element: &Element, view: &Page) -> CompareResult {
	let mut differences = Vec::new();
	compare_recursive(element, view, "", &mut differences);

	if differences.is_empty() {
		CompareResult::success()
	} else {
		CompareResult::failure(differences)
	}
}

#[cfg(wasm)]
fn compare_recursive(element: &Element, view: &Page, path: &str, differences: &mut Vec<String>) {
	match view {
		Page::Element(el_view) => {
			let actual_tag = element.tag_name().to_lowercase();
			let expected_tag = el_view.tag_name().to_lowercase();

			if actual_tag != expected_tag {
				differences.push(format!(
					"{}: tag mismatch - expected '{}', found '{}'",
					path, expected_tag, actual_tag
				));
				return;
			}

			// Compare children
			let children = element.children();
			let view_children = el_view.child_views();

			if children.len() != view_children.len() {
				differences.push(format!(
					"{}: child count mismatch - expected {}, found {}",
					path,
					view_children.len(),
					children.len()
				));
			}

			for (i, child_view) in view_children.iter().enumerate() {
				let child_path = format!("{}/{}", path, i);
				if i < children.len() {
					compare_recursive(&children[i], child_view, &child_path, differences);
				} else {
					differences.push(format!("{}: missing child at index {}", path, i));
				}
			}
		}
		Page::Text(_) | Page::Empty => {}
		Page::Fragment(views) => {
			let children = element.children();
			for (i, child_view) in views.iter().enumerate() {
				let child_path = format!("{}/{}", path, i);
				if i < children.len() {
					compare_recursive(&children[i], child_view, &child_path, differences);
				}
			}
		}
		Page::KeyedFragment(views) => {
			let children = element.children();
			for (i, (_, child_view)) in views.iter().enumerate() {
				let child_path = format!("{}/{}", path, i);
				if i < children.len() {
					compare_recursive(&children[i], child_view, &child_path, differences);
				}
			}
		}
		Page::Outlet(outlet) => {
			if let Some(child) = outlet.child() {
				compare_recursive(element, child, path, differences);
			}
		}
		Page::WithHead { view, .. } => {
			// Head section is handled separately during SSR
			// For comparison, just compare the inner view
			compare_recursive(element, view, path, differences);
		}
		#[cfg(feature = "hmr")]
		Page::DevTemplate { view, .. } | Page::DevSlot { view, .. } => {
			compare_recursive(element, view, path, differences);
		}
		Page::ReactiveIf(reactive_if) => {
			// For comparison, evaluate the condition and compare the rendered branch
			let branch_view = if reactive_if.condition() {
				reactive_if.then_view()
			} else {
				reactive_if.else_view()
			};
			compare_recursive(element, &branch_view, path, differences);
		}
		Page::Reactive(reactive) => {
			// For comparison, evaluate the render closure and compare the resulting view
			let rendered_view = reactive.render();
			compare_recursive(element, &rendered_view, path, differences);
		}
		Page::Suspense(node) => {
			let branch_view = node.render_branch();
			compare_recursive(element, &branch_view, path, differences);
		}
		Page::Deferred(node) => {
			let content_view = node.content();
			compare_recursive(element, &content_view, path, differences);
		}
	}
}

/// Non-WASM version for testing.
#[cfg(native)]
// Allow dead_code: non-WASM stub for structure comparison
#[allow(dead_code)]
pub(super) fn compare_structure(_element: &str, _view: &Page) -> CompareResult {
	CompareResult::success()
}

// Phase 2-B Tests: Selective Reconciliation

#[test]
fn test_reconcile_options_default() {
	let options = ReconcileOptions::default();
	assert!(!options.island_only);
	assert!(!options.skip_static);
	assert!(options.warn_on_mismatch);
}

#[test]
fn test_reconcile_options_island_only() {
	let options = ReconcileOptions::island_only();
	assert!(options.island_only);
	assert!(options.skip_static);
	assert!(options.warn_on_mismatch);
}

#[test]
fn test_reconcile_options_full_reconciliation() {
	let options = ReconcileOptions::full_reconciliation();
	assert!(!options.island_only);
	assert!(!options.skip_static);
	assert!(options.warn_on_mismatch);
}

#[test]
fn test_reconcile_options_warn_on_mismatch() {
	let options = ReconcileOptions::default().warn_on_mismatch(false);
	assert!(!options.warn_on_mismatch);
}

#[test]
#[cfg(native)]
fn test_reconcile_with_options_non_wasm() {
	// Non-WASM version always succeeds
	let view = Page::Empty;
	let options = ReconcileOptions::default();
	assert!(reconcile_with_options("", &view, &options).is_ok());
}
#[cfg(test)]
mod tests {
	use crate::component::{Head, PageElement};

	use super::*;

	fn assert_text_child(child: &(ReconcilePath, Page), expected_text: &str) {
		match &child.1 {
			Page::Text(text) => assert_eq!(text.as_ref(), expected_text),
			other => panic!("expected text child, found {other:?}"),
		}
	}

	#[test]
	fn test_reconcile_error_display() {
		let err = ReconcileError::TagMismatch {
			path: ReconcilePath::root(),
			expected: "div".to_string(),
			actual: "span".to_string(),
		};
		assert_eq!(
			err.to_string(),
			"Hydration mismatch at root: tag expected 'div', found 'span'"
		);
	}

	#[test]
	fn test_reconcile_error_display_includes_path_context() {
		let err = ReconcileError::AttributeMismatch {
			path: ReconcilePath::root()
				.with_component("TodoList")
				.with_child(3)
				.with_element("li")
				.with_element("input"),
			name: "checked".to_string(),
			expected: Some("true".to_string()),
			actual: None,
		};

		assert_eq!(
			err.to_string(),
			"Hydration mismatch in TodoList at child[3] > li > input: attribute 'checked' expected Some(\"true\"), found None"
		);
	}

	#[test]
	fn test_child_count_mismatch_display() {
		let err = ReconcileError::ChildCountMismatch {
			path: ReconcilePath::root()
				.with_component("TodoList")
				.with_element("ul"),
			expected: 3,
			actual: 2,
		};
		assert_eq!(
			err.to_string(),
			"Hydration mismatch in TodoList at ul: child count expected 3, found 2"
		);
	}

	#[test]
	fn test_text_mismatch_display_includes_path_context() {
		let err = ReconcileError::TextMismatch {
			path: ReconcilePath::root()
				.with_component("TodoItem")
				.with_child(1),
			expected: "Buy milk".to_string(),
			actual: "Buy bread".to_string(),
		};
		assert_eq!(
			err.to_string(),
			"Hydration mismatch in TodoItem at child[1]: text expected 'Buy milk', found 'Buy bread'"
		);
	}

	#[test]
	fn test_element_not_found_display_includes_path_context() {
		let err = ReconcileError::ElementNotFound {
			path: ReconcilePath::root()
				.with_component("TodoList")
				.with_element("ul"),
			index: 4,
		};
		assert_eq!(
			err.to_string(),
			"Hydration mismatch in TodoList at ul: element not found at index 4"
		);
	}

	#[test]
	fn test_collect_expected_children_skips_reactive_empty_branch() {
		let views = vec![
			Page::reactive_if(|| false, || Page::text("visible"), Page::empty),
			Page::text("after"),
		];
		let mut children = Vec::new();

		collect_expected_children(&views, &ReconcilePath::root(), &mut children);

		assert_eq!(children.len(), 1);
		assert_eq!(children[0].0, ReconcilePath::root().with_child(1));
		assert_text_child(&children[0], "after");
	}

	#[test]
	fn test_collect_expected_children_flattens_wrappers_and_coalesces_adjacent_text() {
		let views = vec![
			Page::fragment([Page::text("a"), Page::text("b")]).with_head(Head::new()),
			Page::reactive(|| Page::fragment([Page::text("c"), Page::text("d")])),
		];
		let mut children = Vec::new();

		collect_expected_children(&views, &ReconcilePath::root(), &mut children);

		assert_eq!(children.len(), 1);
		assert_eq!(
			children[0].0,
			ReconcilePath::root().with_child(0).with_child(0)
		);
		assert_text_child(&children[0], "abcd");
	}

	#[test]
	fn test_collect_expected_children_does_not_coalesce_text_across_elements() {
		let views = vec![
			Page::text("before"),
			Page::Element(PageElement::new("span")),
			Page::text("after"),
		];
		let mut children = Vec::new();

		collect_expected_children(&views, &ReconcilePath::root(), &mut children);

		assert_eq!(children.len(), 3);
		assert_text_child(&children[0], "before");
		assert!(matches!(children[1].1, Page::Element(_)));
		assert_text_child(&children[2], "after");
	}

	#[test]
	fn test_normalize_whitespace() {
		assert_eq!(normalize_whitespace("hello  world"), "hello world");
		assert_eq!(normalize_whitespace("  foo   bar  "), "foo bar");
		assert_eq!(normalize_whitespace("single"), "single");
	}

	#[test]
	fn test_compare_result_success() {
		let result = CompareResult::success();
		assert!(result.matches);
		assert!(result.differences.is_empty());
	}

	#[test]
	fn test_compare_result_failure() {
		let result = CompareResult::failure(vec!["diff1".to_string(), "diff2".to_string()]);
		assert!(!result.matches);
		assert_eq!(result.differences.len(), 2);
	}

	#[cfg(native)]
	#[test]
	fn test_structure_matches_non_wasm() {
		// Non-WASM version always returns true
		let view = Page::Empty;
		assert!(structure_matches("", &view));
	}

	#[cfg(native)]
	#[test]
	fn test_reconcile_non_wasm() {
		// Non-WASM version always succeeds
		let view = Page::Empty;
		assert!(reconcile("", &view).is_ok());
	}
}
