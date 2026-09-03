//! Hydration Runtime
//!
//! This module provides the main entry point for client-side hydration,
//! connecting reactive state with SSR-rendered DOM elements.

use crate::component::Component;
#[cfg(test)]
use crate::reactive::QueryKey;
#[cfg(any(wasm, test))]
use crate::reactive::entity::{ENTITY_TABLE_HYDRATION_ID, EntityHydrationEnvelope};
#[cfg(any(wasm, test))]
use crate::reactive::{QueryClient, QueryDescriptor};
use crate::ssr::SsrState;
#[cfg(any(wasm, test))]
use serde::{Serialize, de::DeserializeOwned};
use std::collections::HashMap;

#[cfg(wasm)]
use crate::dom::{Element, document};

#[cfg(wasm)]
use crate::component::{
	Page, ReactiveAttributeEffects, new_reactive_node_store, store_reactive_node,
	with_reactive_node_store,
};

#[cfg(wasm)]
use crate::document_head::{
	DocumentHeadManager, current_document_head_manager, ensure_browser_document_head_manager,
	with_document_head_manager,
};

#[cfg(wasm)]
use crate::ssr::HYDRATION_ATTR_ID;

#[cfg(wasm)]
use reinhardt_core::types::page::{is_boolean_attr, is_boolean_attr_truthy};

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

	/// Gets SSR metadata by key.
	pub fn get_metadata(&self, key: &str) -> Option<&serde_json::Value> {
		self.state.get_metadata(key)
	}

	/// Gets resource state by deterministic resource ID.
	pub fn get_resource_state(&self, id: &str) -> Option<&serde_json::Value> {
		self.state.get_resource_state(id)
	}

	/// Gets a successful route-loader value by its stable loader ID.
	///
	/// Route-loader values are serialized in their own namespace so initial
	/// navigation hydration can restore the typed loader store without relying
	/// on call-order resource identifiers.
	pub fn get_route_loader_state(&self, id: impl AsRef<str>) -> Option<&serde_json::Value> {
		self.state.get_route_loader_state(id)
	}

	#[cfg(test)]
	pub(crate) fn seed_query<T, E>(
		&self,
		client: &QueryClient,
		key: QueryKey<T, E>,
	) -> Result<bool, serde_json::Error>
	where
		T: Clone + Serialize + DeserializeOwned + 'static,
		E: Clone + Serialize + DeserializeOwned + 'static,
	{
		let Some(serialized) = self.get_resource_state(&key.hydration_id()) else {
			return Ok(false);
		};
		client.seed_query_snapshot(key, serialized)?;
		Ok(true)
	}

	#[cfg(any(wasm, test))]
	pub(crate) fn seed_query_descriptor<T, E>(
		&mut self,
		client: &QueryClient,
		descriptor: &QueryDescriptor<T, E>,
	) -> Result<bool, serde_json::Error>
	where
		T: Clone + Serialize + DeserializeOwned + 'static,
		E: Clone + Serialize + DeserializeOwned + 'static,
	{
		self.install_entity_table(client)?;
		let Some(serialized) = self.get_resource_state(&descriptor.key().hydration_id()) else {
			return Ok(false);
		};
		client.seed_query_descriptor(descriptor, serialized)?;
		Ok(true)
	}

	#[cfg(any(wasm, test))]
	pub(crate) fn install_entity_table(
		&mut self,
		client: &QueryClient,
	) -> Result<(), serde_json::Error> {
		let Some(serialized) = self.state.take_resource_state(ENTITY_TABLE_HYDRATION_ID) else {
			return Ok(());
		};
		let envelope: EntityHydrationEnvelope = serde_json::from_value(serialized)?;
		client.install_entity_hydration_envelope(envelope);
		Ok(())
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

	let scope = reinhardt_core::reactive::ReactiveScope::new();
	let result = scope.enter(|| {
		web_sys::console::log_1(&"[Hydration] Starting...".into());

		// 1. Restore SSR state
		let mut context = HydrationContext::from_window()?;
		#[cfg(feature = "i18n")]
		let i18n_guard = crate::i18n::provide_i18n_from_hydration_context(&context).map_err(|e| {
			HydrationError::StateParseError(format!("Failed to hydrate i18n state: {}", e))
		})?;
		let document_head_manager = ensure_browser_document_head_manager().map_err(|error| {
			HydrationError::StateParseError(format!("Document-head initialization failed: {error}"))
		})?;
		document_head_manager.begin_batch();

		let installation_result = with_document_head_manager(&document_head_manager, || {
			crate::component::reactive_if::with_reactive_node_transaction(|| {
				// Render the root component exactly once in the durable hydration store so
				// retained hooks own the hydrated root lifetime.
				let view = component.render();
				let resource_counter_offset =
					crate::reactive::resource::current_client_resource_counter();
				let id_counter_offset = crate::reactive::hooks::id::id_counter_snapshot();
				web_sys::console::log_1(&"[Hydration] View rendered".into());

				// Reconciliation may render lazy branches. Keep that shape prepass fully
				// disposable so it cannot register root-owned hooks or consume browser-head
				// registry slots before the hydrated ownership pass.
				let prepass_head_manager = DocumentHeadManager::new(crate::component::Head::new());
				crate::reactive::resource::set_client_resource_counter(resource_counter_offset);
				crate::reactive::hooks::id::restore_id_counter(id_counter_offset);
				let prepass_result = with_document_head_manager(&prepass_head_manager, || {
					with_hydration_prepass_store(|| -> Result<(), HydrationError> {
						reconcile(root, &view).map_err(|error| {
							HydrationError::StateParseError(format!(
								"Reconciliation failed: {error}"
							))
						})?;
						validate_hydrated_controls(root, &view)
					})
				});
				crate::reactive::resource::set_client_resource_counter(resource_counter_offset);
				crate::reactive::hooks::id::restore_id_counter(id_counter_offset);
				prepass_result?;
				web_sys::console::log_1(&"[Hydration] Reconciliation complete".into());

				// Install hydration guards and reactive DOM owners in the same ownership pass.
				crate::dom::control_binding::with_hydration_snapshot_transaction(|| {
					let mut root_registry = EventRegistry::new_for_hydration();
					install_hydrated_reactive_nodes(root, &view, &mut root_registry)?;
					store_reactive_node(root_registry);
					document_head_manager.reconcile().map_err(|error| {
						HydrationError::StateParseError(format!(
							"Document-head reconciliation failed: {error}"
						))
					})?;
					Ok::<_, HydrationError>(())
				})
			})
		});
		let batch_result = document_head_manager.end_batch(false);
		installation_result.map_err(|error| {
			HydrationError::StateParseError(format!("Document-head installation failed: {error}"))
		})?;
		batch_result.map_err(|error| {
			HydrationError::StateParseError(format!("Document-head reconciliation failed: {error}"))
		})?;
		web_sys::console::log_1(&"[Hydration] Events attached".into());
		web_sys::console::log_1(&"[Hydration] Reactive nodes installed".into());

		// 6. Mark hydration complete
		#[cfg(feature = "i18n")]
		if let Some(i18n_guard) = i18n_guard {
			crate::i18n::retain_hydrated_i18n_context(i18n_guard);
		}
		context.mark_hydrated();
		mark_hydration_complete_internal();
		web_sys::console::log_1(&"[Hydration] Complete!".into());

		Ok(())
	});
	if result.is_ok() {
		crate::component::store_reactive_scope(scope);
	}
	result
}

#[cfg(wasm)]
fn with_hydration_prepass_store<R>(f: impl FnOnce() -> R) -> R {
	let store = new_reactive_node_store();
	with_reactive_node_store(&store, f)
}

#[cfg(wasm)]
fn validate_hydrated_controls(element: &Element, view: &Page) -> Result<(), HydrationError> {
	match view {
		Page::Element(element_view) => {
			if let Some(binding) = element_view.bound_control() {
				crate::dom::control_binding::validate_control(element, binding.kind())
					.map_err(|error| HydrationError::EventAttachmentFailed(error.to_string()))?;
				if element_view.tag_name().eq_ignore_ascii_case("textarea") {
					return Ok(());
				}
			}
			validate_hydrated_element_children(element, element_view.child_views())?;
		}
		Page::WithHead { view, .. } => validate_hydrated_controls(element, view)?,
		#[cfg(feature = "hmr")]
		Page::DevTemplate { view, .. } | Page::DevSlot { view, .. } => {
			validate_hydrated_controls(element, view)?
		}
		Page::Fragment(children) => validate_hydrated_element_children(element, children)?,
		Page::KeyedFragment(children) => {
			let child_views = children
				.iter()
				.map(|(_, child)| child.clone())
				.collect::<Vec<_>>();
			validate_hydrated_element_children(element, &child_views)?;
		}
		Page::Outlet(outlet) => {
			if let Some(child) = outlet.child() {
				validate_hydrated_controls(element, child)?;
			}
		}
		Page::Reactive(reactive) => validate_hydrated_controls(element, &reactive.render())?,
		Page::ReactiveIf(reactive_if) => {
			let branch = if reactive_if.condition() {
				reactive_if.then_view()
			} else {
				reactive_if.else_view()
			};
			validate_hydrated_controls(element, &branch)?;
		}
		Page::Suspense(node) => validate_hydrated_controls(element, &node.render_branch())?,
		Page::Deferred(node) => validate_hydrated_controls(element, &node.content())?,
		Page::Text(_) | Page::Empty => {}
	}
	Ok(())
}

#[cfg(wasm)]
fn validate_hydrated_element_children(
	element: &Element,
	children: &[Page],
) -> Result<(), HydrationError> {
	with_hydration_prepass_store(|| split_coalesced_text_children(element, children));
	validate_hydrated_child_sequence(&relevant_child_nodes(element), children)
}

#[cfg(wasm)]
fn validate_hydrated_child_controls(
	nodes: &[web_sys::Node],
	view: &Page,
) -> Result<(), HydrationError> {
	match view {
		Page::Reactive(reactive) => {
			validate_hydrated_child_controls(nodes, &reactive.render())?;
		}
		Page::ReactiveIf(reactive_if) => {
			let branch = if reactive_if.condition() {
				reactive_if.then_view()
			} else {
				reactive_if.else_view()
			};
			validate_hydrated_child_controls(nodes, &branch)?;
		}
		Page::Element(_) => {
			if let Some(element) = nodes
				.first()
				.and_then(|node| wasm_bindgen::JsCast::dyn_ref::<web_sys::Element>(node))
			{
				validate_hydrated_controls(&Element::new(element.clone()), view)?;
			}
		}
		Page::WithHead { view, .. } => validate_hydrated_child_controls(nodes, view)?,
		#[cfg(feature = "hmr")]
		Page::DevTemplate { view, .. } | Page::DevSlot { view, .. } => {
			validate_hydrated_child_controls(nodes, view)?
		}
		Page::Fragment(children) => validate_hydrated_child_sequence(nodes, children)?,
		Page::KeyedFragment(children) => {
			let child_views = children
				.iter()
				.map(|(_, child)| child.clone())
				.collect::<Vec<_>>();
			validate_hydrated_child_sequence(nodes, &child_views)?;
		}
		Page::Outlet(outlet) => {
			if let Some(child) = outlet.child() {
				validate_hydrated_child_controls(nodes, child)?;
			}
		}
		Page::Suspense(node) => {
			validate_hydrated_child_controls(nodes, &node.render_branch())?;
		}
		Page::Deferred(node) => validate_hydrated_child_controls(nodes, &node.content())?,
		Page::Text(_) | Page::Empty => {}
	}
	Ok(())
}

#[cfg(wasm)]
fn validate_hydrated_child_sequence(
	nodes: &[web_sys::Node],
	children: &[Page],
) -> Result<(), HydrationError> {
	let mut index = 0;
	for child in children {
		let count = with_hydration_prepass_store(|| hydrated_node_count(child));
		let end = (index + count).min(nodes.len());
		validate_hydrated_child_controls(&nodes[index..end], child)?;
		index = end;
	}
	Ok(())
}

#[cfg(wasm)]
struct HydrationBranchTransaction {
	store: crate::component::reactive_if::ReactiveNodeStore,
	committed: bool,
}

#[cfg(wasm)]
impl HydrationBranchTransaction {
	fn new() -> Self {
		Self {
			store: new_reactive_node_store(),
			committed: false,
		}
	}

	fn store(&self) -> crate::component::reactive_if::ReactiveNodeStore {
		self.store.clone()
	}

	fn commit(&mut self) {
		self.committed = true;
	}
}

#[cfg(wasm)]
impl Drop for HydrationBranchTransaction {
	fn drop(&mut self) {
		if !self.committed {
			crate::component::reactive_if::clear_hydration_rollback_reactive_node_store(
				&self.store,
			);
		}
	}
}

#[cfg(wasm)]
fn register_hydrated_static_head(head: &crate::component::Head) -> Result<(), HydrationError> {
	let registration = current_document_head_manager()
		.and_then(|manager| manager.register_static_page(head.clone()))
		.map_err(|error| {
			HydrationError::StateParseError(format!("Document-head registration failed: {error}"))
		})?;
	store_reactive_node(registration);
	Ok(())
}

#[cfg(wasm)]
fn install_hydrated_reactive_nodes(
	element: &Element,
	view: &Page,
	registry: &mut super::events::EventRegistry,
) -> Result<(), HydrationError> {
	match view {
		Page::Element(element_view) => {
			attach_hydrated_element_events(element, element_view, registry)?;
			let suppress_bound_textarea_children =
				element_view.bound_control().is_some_and(|binding| {
					element_view.tag_name().eq_ignore_ascii_case("textarea")
						&& binding.kind() == crate::component::ControlKind::Text
				});
			if !suppress_bound_textarea_children {
				install_hydrated_element_children(element, element_view.child_views(), registry)?;
			}
		}
		Page::WithHead { view, head } => {
			register_hydrated_static_head(head)?;
			install_hydrated_reactive_nodes(element, view, registry)?;
		}
		Page::Fragment(children) => {
			install_hydrated_element_children(element, children, registry)?;
		}
		Page::KeyedFragment(children) => {
			let child_views = children
				.iter()
				.map(|(_, child)| child.clone())
				.collect::<Vec<_>>();
			install_hydrated_element_children(element, &child_views, registry)?;
		}
		Page::Outlet(outlet) => {
			if let Some(child) = outlet.child() {
				install_hydrated_reactive_nodes(element, child, registry)?;
			}
		}
		Page::Reactive(reactive) => {
			let render_store = new_reactive_node_store();
			let mut branch_transaction = HydrationBranchTransaction::new();
			let branch_store = branch_transaction.store();
			let rendered = with_reactive_node_store(&render_store, || reactive.render());
			with_hydration_prepass_store(|| {
				split_coalesced_text_children(element, std::slice::from_ref(&rendered));
			});
			let nodes = relevant_child_nodes(element);
			let mut branch_registry = super::events::EventRegistry::new_for_hydration();
			with_reactive_node_store(&branch_store, || {
				install_hydrated_child_reactive_nodes(
					&element.as_web_sys().clone().into(),
					&nodes,
					None,
					&rendered,
					&mut branch_registry,
				)
			})?;
			let control_binding_adopted = branch_registry.control_binding_adopted();
			let hydrated_node = crate::component::ReactiveNode::hydrate_at(
				element.as_web_sys().clone().into(),
				None,
				nodes.clone(),
				reactive.clone().into_render(),
				render_store,
				branch_store,
				control_binding_adopted,
			)
			.ok_or_else(|| {
				HydrationError::EventAttachmentFailed(
					"failed to install hydrated reactive owner".to_string(),
				)
			})?;
			if hydrated_node.hydrated_nodes_preserved() {
				with_reactive_node_store(&hydrated_node.reactive_node_store(), || {
					store_reactive_node(branch_registry);
				});
			}
			hydrated_node.refresh_hydrated_current_nodes();
			store_reactive_node(hydrated_node);
			branch_transaction.commit();
		}
		Page::ReactiveIf(reactive_if) => {
			let mut branch_transaction = HydrationBranchTransaction::new();
			let branch_store = branch_transaction.store();
			let (hydrated_condition, branch_view) = with_reactive_node_store(&branch_store, || {
				let hydrated_condition = reactive_if.condition();
				let branch_view = if hydrated_condition {
					reactive_if.then_view()
				} else {
					reactive_if.else_view()
				};
				(hydrated_condition, branch_view)
			});
			with_hydration_prepass_store(|| {
				split_coalesced_text_children(element, std::slice::from_ref(&branch_view));
			});
			let nodes = relevant_child_nodes(element);
			let mut branch_registry = super::events::EventRegistry::new_for_hydration();
			with_reactive_node_store(&branch_store, || {
				install_hydrated_child_reactive_nodes(
					&element.as_web_sys().clone().into(),
					&nodes,
					None,
					&branch_view,
					&mut branch_registry,
				)
			})?;
			let control_binding_adopted = branch_registry.control_binding_adopted();
			let (condition, then_view, else_view) = reactive_if.clone().into_parts();
			let hydrated_node = crate::component::ReactiveIfNode::hydrate_at(
				element.as_web_sys().clone().into(),
				None,
				nodes.clone(),
				hydrated_condition,
				condition,
				then_view,
				else_view,
				branch_store,
				control_binding_adopted,
			)
			.ok_or_else(|| {
				HydrationError::EventAttachmentFailed(
					"failed to install hydrated reactive-if owner".to_string(),
				)
			})?;
			if hydrated_node.hydrated_nodes_preserved() {
				with_reactive_node_store(&hydrated_node.reactive_node_store(), || {
					store_reactive_node(branch_registry);
				});
			}
			hydrated_node.refresh_hydrated_current_nodes();
			store_reactive_node(hydrated_node);
			branch_transaction.commit();
		}
		Page::Suspense(node) => {
			let branch_view = node.render_branch();
			install_hydrated_reactive_nodes(element, &branch_view, registry)?;
		}
		Page::Deferred(node) => {
			let content_view = node.content();
			install_hydrated_reactive_nodes(element, &content_view, registry)?;
		}
		_ => {}
	}
	Ok(())
}

#[cfg(wasm)]
fn install_hydrated_element_children(
	element: &Element,
	children: &[Page],
	registry: &mut super::events::EventRegistry,
) -> Result<(), HydrationError> {
	with_hydration_prepass_store(|| split_coalesced_text_children(element, children));
	let actual_nodes = relevant_child_nodes(element);
	install_hydrated_children_reactive_nodes(
		&element.as_web_sys().clone().into(),
		&actual_nodes,
		None,
		children,
		registry,
	)
}

#[cfg(wasm)]
fn install_hydrated_children_reactive_nodes(
	parent: &web_sys::Node,
	nodes: &[web_sys::Node],
	next_sibling: Option<web_sys::Node>,
	children: &[Page],
	registry: &mut super::events::EventRegistry,
) -> Result<(), HydrationError> {
	let mut index = 0;
	for child in children {
		let node_count = with_hydration_prepass_store(|| hydrated_node_count(child));
		let end = (index + node_count).min(nodes.len());
		let child_next_sibling = nodes.get(end).cloned().or_else(|| next_sibling.clone());
		install_hydrated_child_reactive_nodes(
			parent,
			&nodes[index..end],
			child_next_sibling,
			child,
			registry,
		)?;
		index = end;
	}
	Ok(())
}

#[cfg(wasm)]
fn install_hydrated_child_reactive_nodes(
	parent: &web_sys::Node,
	nodes: &[web_sys::Node],
	next_sibling: Option<web_sys::Node>,
	view: &Page,
	registry: &mut super::events::EventRegistry,
) -> Result<(), HydrationError> {
	match view {
		Page::Reactive(reactive) => {
			let render_store = new_reactive_node_store();
			let mut branch_transaction = HydrationBranchTransaction::new();
			let branch_store = branch_transaction.store();
			let rendered = with_reactive_node_store(&render_store, || reactive.render());
			let mut branch_registry = super::events::EventRegistry::new_for_hydration();
			with_reactive_node_store(&branch_store, || {
				install_hydrated_child_reactive_nodes(
					parent,
					nodes,
					next_sibling.clone(),
					&rendered,
					&mut branch_registry,
				)
			})?;
			let control_binding_adopted =
				registry.control_binding_adopted() || branch_registry.control_binding_adopted();
			if branch_registry.control_binding_adopted() {
				registry.mark_control_binding_adopted();
			}
			let hydrated_node = crate::component::ReactiveNode::hydrate_at(
				parent.clone(),
				next_sibling.clone(),
				nodes.to_vec(),
				reactive.clone().into_render(),
				render_store,
				branch_store,
				control_binding_adopted,
			)
			.ok_or_else(|| {
				HydrationError::EventAttachmentFailed(
					"failed to install nested hydrated reactive owner".to_string(),
				)
			})?;
			if hydrated_node.hydrated_nodes_preserved() {
				with_reactive_node_store(&hydrated_node.reactive_node_store(), || {
					store_reactive_node(branch_registry);
				});
			}
			hydrated_node.refresh_hydrated_current_nodes();
			store_reactive_node(hydrated_node);
			branch_transaction.commit();
		}
		Page::ReactiveIf(reactive_if) => {
			let mut branch_transaction = HydrationBranchTransaction::new();
			let branch_store = branch_transaction.store();
			let (hydrated_condition, branch_view) = with_reactive_node_store(&branch_store, || {
				let hydrated_condition = reactive_if.condition();
				let branch_view = if hydrated_condition {
					reactive_if.then_view()
				} else {
					reactive_if.else_view()
				};
				(hydrated_condition, branch_view)
			});
			let mut branch_registry = super::events::EventRegistry::new_for_hydration();
			with_reactive_node_store(&branch_store, || {
				install_hydrated_child_reactive_nodes(
					parent,
					nodes,
					next_sibling.clone(),
					&branch_view,
					&mut branch_registry,
				)
			})?;
			let control_binding_adopted =
				registry.control_binding_adopted() || branch_registry.control_binding_adopted();
			if branch_registry.control_binding_adopted() {
				registry.mark_control_binding_adopted();
			}
			let (condition, then_view, else_view) = reactive_if.clone().into_parts();
			let hydrated_node = crate::component::ReactiveIfNode::hydrate_at(
				parent.clone(),
				next_sibling.clone(),
				nodes.to_vec(),
				hydrated_condition,
				condition,
				then_view,
				else_view,
				branch_store,
				control_binding_adopted,
			)
			.ok_or_else(|| {
				HydrationError::EventAttachmentFailed(
					"failed to install nested hydrated reactive-if owner".to_string(),
				)
			})?;
			if hydrated_node.hydrated_nodes_preserved() {
				with_reactive_node_store(&hydrated_node.reactive_node_store(), || {
					store_reactive_node(branch_registry);
				});
			}
			hydrated_node.refresh_hydrated_current_nodes();
			store_reactive_node(hydrated_node);
			branch_transaction.commit();
		}
		Page::Element(_) => {
			if let Some(element) = nodes
				.first()
				.and_then(|node| wasm_bindgen::JsCast::dyn_ref::<web_sys::Element>(node))
			{
				install_hydrated_reactive_nodes(&Element::new(element.clone()), view, registry)?;
			}
		}
		Page::WithHead { view, head } => {
			register_hydrated_static_head(head)?;
			install_hydrated_child_reactive_nodes(parent, nodes, next_sibling, view, registry)?;
		}
		#[cfg(feature = "hmr")]
		Page::DevTemplate { view, .. } | Page::DevSlot { view, .. } => {
			install_hydrated_child_reactive_nodes(parent, nodes, next_sibling, view, registry)?;
		}
		Page::Fragment(children) => {
			install_hydrated_children_reactive_nodes(
				parent,
				nodes,
				next_sibling,
				children,
				registry,
			)?;
		}
		Page::KeyedFragment(children) => {
			let child_views = children
				.iter()
				.map(|(_, child)| child.clone())
				.collect::<Vec<_>>();
			install_hydrated_children_reactive_nodes(
				parent,
				nodes,
				next_sibling,
				&child_views,
				registry,
			)?;
		}
		Page::Outlet(outlet) => {
			if let Some(child) = outlet.child() {
				install_hydrated_child_reactive_nodes(
					parent,
					nodes,
					next_sibling,
					child,
					registry,
				)?;
			}
		}
		Page::Suspense(node) => {
			let branch_view = node.render_branch();
			install_hydrated_child_reactive_nodes(
				parent,
				nodes,
				next_sibling,
				&branch_view,
				registry,
			)?;
		}
		Page::Deferred(node) => {
			let content_view = node.content();
			install_hydrated_child_reactive_nodes(
				parent,
				nodes,
				next_sibling,
				&content_view,
				registry,
			)?;
		}
		Page::Text(_) | Page::Empty => {}
	}
	Ok(())
}

#[cfg(wasm)]
#[derive(Clone, Debug, PartialEq, Eq)]
enum ExpectedDomChild {
	Text(String),
	Node,
}

#[cfg(wasm)]
fn hydrated_node_count(view: &Page) -> usize {
	match view {
		Page::Text(text) => usize::from(!normalize_whitespace(text.as_ref()).is_empty()),
		Page::Element(_) => 1,
		Page::Fragment(children) => children.iter().map(hydrated_node_count).sum(),
		Page::KeyedFragment(children) => children
			.iter()
			.map(|(_, child)| hydrated_node_count(child))
			.sum(),
		Page::Outlet(outlet) => outlet.child().map(hydrated_node_count).unwrap_or(0),
		Page::WithHead { view, .. } => hydrated_node_count(view),
		#[cfg(feature = "hmr")]
		Page::DevTemplate { view, .. } | Page::DevSlot { view, .. } => hydrated_node_count(view),
		Page::ReactiveIf(reactive_if) => {
			let branch_view = if reactive_if.condition() {
				reactive_if.then_view()
			} else {
				reactive_if.else_view()
			};
			hydrated_node_count(&branch_view)
		}
		Page::Reactive(reactive) => hydrated_node_count(&reactive.render()),
		Page::Suspense(node) => hydrated_node_count(&node.render_branch()),
		Page::Deferred(node) => hydrated_node_count(&node.content()),
		Page::Empty => 0,
	}
}

#[cfg(wasm)]
fn split_coalesced_text_children(element: &Element, children: &[Page]) {
	use wasm_bindgen::JsCast;

	let mut expected = Vec::new();
	for child in children {
		collect_expected_dom_children(child, &mut expected);
	}

	let mut actual_nodes = relevant_child_nodes(element);
	let document = web_sys::window()
		.and_then(|window| window.document())
		.expect("document should be available");
	let mut actual_index = 0;

	for expected_child in expected {
		match expected_child {
			ExpectedDomChild::Node => {
				actual_index += 1;
			}
			ExpectedDomChild::Text(expected_text) => {
				if expected_text.is_empty() {
					continue;
				}
				let Some(node) = actual_nodes.get(actual_index).cloned() else {
					return;
				};
				if node.node_type() != web_sys::Node::TEXT_NODE {
					actual_index += 1;
					continue;
				}

				let actual_text = node.text_content().unwrap_or_default();
				if actual_text == expected_text {
					actual_index += 1;
					continue;
				}
				if !actual_text.starts_with(&expected_text) {
					actual_index += 1;
					continue;
				}

				let remainder = actual_text[expected_text.len()..].to_string();
				node.set_text_content(Some(&expected_text));
				if !remainder.is_empty() {
					let remainder_node = document.create_text_node(&remainder);
					if let Some(parent) = node.parent_node() {
						let next = node.next_sibling();
						let _ = parent.insert_before(&remainder_node, next.as_ref());
						actual_nodes.insert(actual_index + 1, remainder_node.unchecked_into());
					}
				}
				actual_index += 1;
			}
		}
	}
}

#[cfg(wasm)]
fn collect_expected_dom_children(view: &Page, children: &mut Vec<ExpectedDomChild>) {
	match view {
		Page::Empty => {}
		Page::Text(text) => {
			if !text.is_empty() {
				children.push(ExpectedDomChild::Text(text.to_string()));
			}
		}
		Page::Fragment(fragment_children) => {
			for child in fragment_children {
				collect_expected_dom_children(child, children);
			}
		}
		Page::KeyedFragment(keyed_children) => {
			for (_, child) in keyed_children {
				collect_expected_dom_children(child, children);
			}
		}
		Page::Outlet(outlet) => {
			if let Some(child) = outlet.child() {
				collect_expected_dom_children(child, children);
			}
		}
		Page::WithHead { view, .. } => collect_expected_dom_children(view, children),
		#[cfg(feature = "hmr")]
		Page::DevTemplate { view, .. } | Page::DevSlot { view, .. } => {
			collect_expected_dom_children(view, children)
		}
		Page::ReactiveIf(reactive_if) => {
			let branch_view = if reactive_if.condition() {
				reactive_if.then_view()
			} else {
				reactive_if.else_view()
			};
			collect_expected_dom_children(&branch_view, children);
		}
		Page::Reactive(reactive) => {
			let rendered_view = reactive.render();
			collect_expected_dom_children(&rendered_view, children);
		}
		Page::Suspense(node) => {
			let branch_view = node.render_branch();
			collect_expected_dom_children(&branch_view, children);
		}
		Page::Deferred(node) => {
			let content_view = node.content();
			collect_expected_dom_children(&content_view, children);
		}
		Page::Element(_) => children.push(ExpectedDomChild::Node),
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
fn normalize_whitespace(text: &str) -> String {
	text.split_whitespace().collect::<Vec<_>>().join(" ")
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

	crate::dom::control_binding::with_hydration_snapshot_transaction(|| {
		let mut registry = EventRegistry::new_for_hydration();
		attach_events_recursive(element, view, &mut registry)?;
		store_reactive_node(registry);
		Ok::<_, HydrationError>(())
	})?;

	web_sys::console::log_1(&"[CSR] Events attached successfully!".into());

	Ok(())
}

#[cfg(wasm)]
fn attach_events_to_child_views(
	element: &Element,
	view_children: &[Page],
	registry: &mut super::events::EventRegistry,
) -> Result<(), HydrationError> {
	use wasm_bindgen::JsCast;

	let mut expected_children = Vec::new();
	collect_event_child_views(view_children, &mut expected_children);
	let actual_children = relevant_child_nodes(element);

	for (index, child_view) in expected_children.iter().enumerate() {
		let Some(actual_child) = actual_children.get(index) else {
			break;
		};
		let Some(child_element) = actual_child.dyn_ref::<web_sys::Element>() else {
			continue;
		};
		attach_events_recursive(&Element::new(child_element.clone()), child_view, registry)?;
	}

	Ok(())
}

#[cfg(wasm)]
fn collect_event_child_views(views: &[Page], children: &mut Vec<Page>) {
	for view in views {
		match view {
			Page::Empty => {}
			Page::Fragment(fragment_children) => {
				collect_event_child_views(fragment_children, children);
			}
			Page::KeyedFragment(keyed_children) => {
				let child_views = keyed_children
					.iter()
					.map(|(_, child)| child.clone())
					.collect::<Vec<_>>();
				collect_event_child_views(&child_views, children);
			}
			Page::Outlet(outlet) => {
				if let Some(child) = outlet.child() {
					collect_event_child_views(std::slice::from_ref(child), children);
				}
			}
			Page::WithHead { view, .. } => {
				collect_event_child_views(std::slice::from_ref(view), children);
			}
			#[cfg(feature = "hmr")]
			Page::DevTemplate { view, .. } | Page::DevSlot { view, .. } => {
				collect_event_child_views(std::slice::from_ref(view), children);
			}
			Page::ReactiveIf(reactive_if) => {
				let branch_view = if reactive_if.condition() {
					reactive_if.then_view()
				} else {
					reactive_if.else_view()
				};
				collect_event_child_views(std::slice::from_ref(&branch_view), children);
			}
			Page::Reactive(reactive) => {
				let rendered_view = reactive.render();
				collect_event_child_views(std::slice::from_ref(&rendered_view), children);
			}
			Page::Suspense(node) => {
				let branch_view = node.render_branch();
				collect_event_child_views(std::slice::from_ref(&branch_view), children);
			}
			Page::Deferred(node) => {
				let content_view = node.content();
				collect_event_child_views(std::slice::from_ref(&content_view), children);
			}
			Page::Text(text) => {
				if normalize_whitespace(text.as_ref()).is_empty() {
					continue;
				}
				if let Some(Page::Text(previous_text)) = children.last_mut() {
					*previous_text = format!("{}{}", previous_text.as_ref(), text.as_ref()).into();
				} else {
					children.push(Page::Text(text.clone()));
				}
			}
			Page::Element(_) => children.push(view.clone()),
		}
	}
}

/// Recursively attaches event handlers to DOM elements.
///
/// This function can be used for both SSR+Hydration and CSR (client-side rendering only) scenarios.
/// For CSR, call this after mounting a view to attach event handlers.
#[cfg(wasm)]
pub(crate) fn attach_events_recursive(
	element: &Element,
	view: &crate::component::Page,
	registry: &mut super::events::EventRegistry,
) -> Result<(), HydrationError> {
	use crate::component::Page;

	match view {
		Page::Element(el_view) => {
			attach_hydrated_element_events(element, el_view, registry)?;
			attach_events_to_child_views(element, el_view.child_views(), registry)?;
		}
		Page::Fragment(views) => {
			attach_events_to_child_views(element, views, registry)?;
		}
		Page::KeyedFragment(views) => {
			let child_views = views
				.iter()
				.map(|(_, child)| child.clone())
				.collect::<Vec<_>>();
			attach_events_to_child_views(element, &child_views, registry)?;
		}
		Page::Outlet(outlet) => {
			if let Some(child) = outlet.child() {
				attach_events_recursive(element, child, registry)?;
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
		#[cfg(feature = "hmr")]
		Page::DevTemplate { view, .. } | Page::DevSlot { view, .. } => {
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
		Page::Suspense(node) => {
			let branch_view = node.render_branch();
			attach_events_recursive(element, &branch_view, registry)?;
		}
		Page::Deferred(node) => {
			let content_view = node.content();
			attach_events_recursive(element, &content_view, registry)?;
		}
	}

	Ok(())
}

#[cfg(wasm)]
fn attach_hydrated_element_events(
	element: &Element,
	element_view: &crate::component::PageElement,
	registry: &mut super::events::EventRegistry,
) -> Result<(), HydrationError> {
	use super::events::attach_event;

	let tag = element_view.tag_name();
	let event_count = element_view.event_handlers().len();
	if registry.should_hydrate_control_bindings()
		&& let Some(binding) = element_view.bound_control()
	{
		super::events::hydrate_control_binding(element, binding, registry)
			.map_err(|error| HydrationError::EventAttachmentFailed(error.to_string()))?;
	}

	if event_count > 0 {
		web_sys::console::log_1(
			&format!("[attach_events] {} has {} event handlers", tag, event_count).into(),
		);
	}

	for (event_type, handler) in element_view.event_handlers() {
		web_sys::console::log_1(
			&format!("[attach_events] Attaching {:?} to {}", event_type, tag).into(),
		);

		attach_event(element, event_type, handler.clone(), registry)
			.map_err(|error| HydrationError::EventAttachmentFailed(error.to_string()))?;
	}

	let initializing_reactive_attributes = std::rc::Rc::new(std::cell::Cell::new(true));
	let reactive_attribute_effects = element_view
		.reactive_attrs()
		.iter()
		.enumerate()
		.filter(|(index, attribute)| {
			!element_view.reactive_attrs()[*index + 1..]
				.iter()
				.any(|later| later.name().eq_ignore_ascii_case(attribute.name()))
		})
		.filter(|(_, attribute)| {
			!crate::component::into_page::controlled_attribute_is_overridden(
				element_view.bound_control(),
				attribute.name(),
			)
		})
		.map(|(_, attribute)| attribute)
		.cloned()
		.map(|attribute| {
			let element = element.clone();
			let binding = element_view.bound_control().cloned();
			let initializing = std::rc::Rc::clone(&initializing_reactive_attributes);
			crate::reactive::Effect::new(move || {
				let value = attribute.value();
				if binding.as_ref().is_some_and(|binding| {
					!crate::control_binding::controlled_attribute_update_is_supported(
						&element.as_web_sys().tag_name(),
						binding.kind(),
						attribute.name(),
						value.as_deref(),
					)
				}) {
					return;
				}
				match value {
					Some(value)
						if !reinhardt_core::types::page::is_safe_html_attribute(
							attribute.name(),
							&value,
						) =>
					{
						let _ = element.remove_attribute(attribute.name());
					}
					Some(value)
						if is_boolean_attr(attribute.name()) && !is_boolean_attr_truthy(&value) =>
					{
						let _ = element.remove_attribute(attribute.name());
					}
					Some(value) => {
						let _ = element.set_attribute(attribute.name(), &value);
					}
					None => {
						let _ = element.remove_attribute(attribute.name());
					}
				}
				if !initializing.get()
					&& let Some(binding) = binding.as_ref()
					&& crate::component::into_page::controlled_attribute_affects_value(
						&element,
						binding,
						attribute.name(),
					) && let Err(error) =
					crate::dom::control_binding::reconcile_control_binding(&element, binding)
				{
					web_sys::console::error_1(
						&format!("controlled input attribute update failed: {error}").into(),
					);
				}
			})
		})
		.collect::<Vec<_>>();
	initializing_reactive_attributes.set(false);
	if registry.should_hydrate_control_bindings()
		&& let Some(binding) = element_view.bound_control()
		&& (binding.kind() != crate::component::ControlKind::Number
			|| (element
				.as_web_sys()
				.tag_name()
				.eq_ignore_ascii_case("input")
				&& element
					.as_web_sys()
					.get_attribute("type")
					.is_some_and(|input_type| input_type.eq_ignore_ascii_case("range"))))
		&& element_view.reactive_attrs().iter().any(|attribute| {
			crate::component::into_page::controlled_attribute_affects_value(
				element,
				binding,
				attribute.name(),
			)
		}) {
		crate::dom::control_binding::reconcile_control_binding(element, binding)
			.map_err(|error| HydrationError::EventAttachmentFailed(error.to_string()))?;
	}
	store_reactive_node(ReactiveAttributeEffects::new(reactive_attribute_effects));

	Ok(())
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
	#[cfg(wasm)]
	use crate::component::{
		Component, ControlBinding, IntoPage, PageElement, PageExt, cleanup_reactive_nodes,
	};
	#[cfg(wasm)]
	use crate::reactive::hooks::{use_head, use_page_title, use_retained_effect};
	#[cfg(native)]
	use crate::reactive::{QueryClient, QueryDefaults, QueryFamily, QueryOptions, ReactiveScope};
	#[cfg(wasm)]
	use crate::reactive::{ReactiveScope, Signal, with_runtime};
	#[cfg(wasm)]
	use reinhardt_core::deps;
	#[cfg(any(native, wasm))]
	use std::cell::Cell;
	#[cfg(wasm)]
	use std::cell::RefCell;
	#[cfg(any(native, wasm))]
	use std::rc::Rc;
	#[cfg(wasm)]
	use wasm_bindgen::JsCast;
	#[cfg(wasm)]
	use wasm_bindgen_test::*;

	#[cfg(wasm)]
	wasm_bindgen_test_configure!(run_in_browser);

	#[cfg(wasm)]
	struct RootHeadHydrationComponent {
		title: Signal<String>,
		render_count: Rc<Cell<usize>>,
		head_factory_count: Rc<Cell<usize>>,
		title_factory_count: Rc<Cell<usize>>,
	}

	#[cfg(wasm)]
	impl Component for RootHeadHydrationComponent {
		fn render(&self) -> Page {
			self.render_count.set(self.render_count.get() + 1);
			use_head(
				{
					let title = self.title.clone();
					let head_factory_count = Rc::clone(&self.head_factory_count);
					move || {
						head_factory_count.set(head_factory_count.get() + 1);
						crate::component::Head::new().meta_description(title.get())
					}
				},
				deps![self.title.clone()],
			);
			use_page_title(
				{
					let title = self.title.clone();
					let title_factory_count = Rc::clone(&self.title_factory_count);
					move || {
						title_factory_count.set(title_factory_count.get() + 1);
						title.get()
					}
				},
				deps![self.title.clone()],
			);
			Page::text("hydrated root")
		}

		fn name() -> &'static str {
			"RootHeadHydrationComponent"
		}
	}

	#[cfg(wasm)]
	struct HydrationStructuralHeadComponent;

	#[cfg(wasm)]
	impl Component for HydrationStructuralHeadComponent {
		fn render(&self) -> Page {
			Page::fragment([
				Page::reactive(|| {
					PageElement::new("span")
						.child("reactive")
						.into_page()
						.with_head(crate::component::Head::new().title("Reactive title"))
				}),
				Page::reactive_if(
					|| true,
					|| {
						PageElement::new("span")
							.child("conditional")
							.into_page()
							.with_head(crate::component::Head::new().title("Conditional title"))
					},
					|| Page::Empty,
				),
				PageElement::new("span")
					.child("trailing")
					.into_page()
					.with_head(crate::component::Head::new().title("Trailing title")),
			])
		}

		fn name() -> &'static str {
			"HydrationStructuralHeadComponent"
		}
	}

	#[cfg(wasm)]
	struct HydrationHeadFixture {
		document: web_sys::Document,
		root: web_sys::Element,
		state: web_sys::Element,
		original_title: String,
	}

	#[cfg(wasm)]
	impl HydrationHeadFixture {
		fn new(markup: &str) -> Self {
			cleanup_reactive_nodes();
			let document = web_sys::window().unwrap().document().unwrap();
			let original_title = document.title();
			let state = document.create_element("script").unwrap();
			state.set_id("ssr-state");
			state.set_text_content(Some("{}"));
			document.body().unwrap().append_child(&state).unwrap();
			let root = document.create_element("div").unwrap();
			root.set_inner_html(markup);
			document.body().unwrap().append_child(&root).unwrap();

			Self {
				document,
				root,
				state,
				original_title,
			}
		}

		fn root(&self) -> Element {
			Element::new(self.root.clone())
		}
	}

	#[cfg(wasm)]
	impl Drop for HydrationHeadFixture {
		fn drop(&mut self) {
			cleanup_reactive_nodes();
			self.state.remove();
			self.root.remove();
			self.document.set_title(&self.original_title);
		}
	}

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
	fn test_hydration_context_get_resource_state() {
		let mut state = SsrState::new();
		state.add_resource_state("rh-res-0", serde_json::json!({"Success": {"name": "Ada"}}));
		let ctx = HydrationContext::from_state(state);
		assert_eq!(
			ctx.get_resource_state("rh-res-0"),
			Some(&serde_json::json!({"Success": {"name": "Ada"}}))
		);
	}

	#[test]
	fn test_hydration_context_get_route_loader_state() {
		let mut state = SsrState::new();
		state.add_route_loader_state("app::loader", serde_json::json!({"name": "Ada"}));
		let context = HydrationContext::from_state(state);

		assert_eq!(
			context.get_route_loader_state("app::loader"),
			Some(&serde_json::json!({"name": "Ada"}))
		);
	}

	#[cfg(native)]
	#[test]
	fn query_snapshot_seeds_client_before_first_observer_mount() {
		ReactiveScope::run(|| {
			let family = QueryFamily::<(), String, String>::new("tests::hydrated-query");
			let key = family.key(());
			let mut state = SsrState::new();
			state.add_resource_state(
				key.hydration_id(),
				serde_json::json!({
					"state": { "Success": "server-value" },
					"refetch_error": null,
					"is_fetching": false,
					"is_stale": false
				}),
			);
			let context = HydrationContext::from_state(state);
			let client = QueryClient::new(QueryDefaults::default());
			let fetch_count = Rc::new(Cell::new(0));

			context
				.seed_query(&client, key)
				.expect("typed query snapshot should hydrate");
			let hydrated = client.observe(
				family.query((), {
					let fetch_count = Rc::clone(&fetch_count);
					move || {
						fetch_count.set(fetch_count.get() + 1);
						async { Ok::<_, String>("client-value".to_string()) }
					}
				}),
				QueryOptions::default(),
			);

			assert_eq!(hydrated.data(), Some("server-value".to_string()));
			assert_eq!(fetch_count.get(), 0);
		});
	}

	#[cfg(native)]
	fn assert_unsettled_query_snapshot_is_rejected(status: &'static str) {
		ReactiveScope::run(|| {
			let family_id = match status {
				"Idle" => "tests::invalid-idle-hydrated-query",
				"Pending" => "tests::invalid-pending-hydrated-query",
				_ => panic!("unsupported unsettled query status"),
			};
			let family = QueryFamily::<(), String, String>::new(family_id);
			let key = family.key(());
			let mut malformed_state = SsrState::new();
			malformed_state.add_resource_state(
				key.hydration_id(),
				serde_json::json!({
					"state": { status: null },
					"refetch_error": null,
					"is_fetching": false,
					"is_stale": false
				}),
			);
			let client = QueryClient::new(QueryDefaults::default());
			let fetch_count = Rc::new(Cell::new(0));

			let error = HydrationContext::from_state(malformed_state)
				.seed_query(&client, key.clone())
				.expect_err("unsettled SSR query snapshots must be rejected");

			assert!(error.is_data());
			assert_eq!(fetch_count.get(), 0);

			let mut valid_state = SsrState::new();
			valid_state.add_resource_state(
				key.hydration_id(),
				serde_json::json!({
					"state": { "Success": "server-value" },
					"refetch_error": null,
					"is_fetching": false,
					"is_stale": false
				}),
			);
			HydrationContext::from_state(valid_state)
				.seed_query(&client, key)
				.expect("a valid snapshot should seed after malformed input is rejected");
			let hydrated = client.observe(
				family.query((), {
					let fetch_count = Rc::clone(&fetch_count);
					move || {
						fetch_count.set(fetch_count.get() + 1);
						async { Ok::<_, String>("client-value".to_string()) }
					}
				}),
				QueryOptions::default(),
			);

			assert_eq!(hydrated.data(), Some("server-value".to_string()));
			assert_eq!(fetch_count.get(), 0);
		});
	}

	#[cfg(native)]
	#[test]
	fn idle_query_snapshot_does_not_seed_cache_or_start_fetch() {
		assert_unsettled_query_snapshot_is_rejected("Idle");
	}

	#[cfg(native)]
	#[test]
	fn pending_query_snapshot_does_not_seed_cache_or_start_fetch() {
		assert_unsettled_query_snapshot_is_rejected("Pending");
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

	#[cfg(wasm)]
	#[wasm_bindgen_test]
	fn hydration_retains_root_document_head_hook_without_rerendering_component() {
		let scope = ReactiveScope::new();
		scope.enter(|| {
			cleanup_reactive_nodes();
			let document = web_sys::window().unwrap().document().unwrap();
			let original_title = document.title();
			let state = document.create_element("script").unwrap();
			state.set_id("ssr-state");
			state.set_text_content(Some("{}"));
			document.body().unwrap().append_child(&state).unwrap();
			let root = document.create_element("div").unwrap();
			root.set_inner_html("hydrated root");
			document.body().unwrap().append_child(&root).unwrap();

			let title = Signal::new("Initial title".to_string());
			let render_count = Rc::new(Cell::new(0));
			let head_factory_count = Rc::new(Cell::new(0));
			let title_factory_count = Rc::new(Cell::new(0));
			let component = RootHeadHydrationComponent {
				title: title.clone(),
				render_count: Rc::clone(&render_count),
				head_factory_count: Rc::clone(&head_factory_count),
				title_factory_count: Rc::clone(&title_factory_count),
			};

			hydrate(&component, &Element::new(root.clone())).expect("root hydration succeeds");

			assert_eq!(
				render_count.get(),
				1,
				"hydration must not rerender the root component"
			);
			assert_eq!(head_factory_count.get(), 1);
			assert_eq!(title_factory_count.get(), 1);
			assert_eq!(document.title(), "Initial title");
			assert_eq!(
				document
					.query_selector_all(
						"meta[name='description'][content='Initial title'][data-reinhardt-head]"
					)
					.unwrap()
					.length(),
				1
			);
			title.set("Updated title".to_string());
			with_runtime(|runtime| runtime.flush_updates());
			assert_eq!(head_factory_count.get(), 2);
			assert_eq!(title_factory_count.get(), 2);
			assert_eq!(document.title(), "Updated title");
			assert_eq!(
				document
					.query_selector_all(
						"meta[name='description'][content='Updated title'][data-reinhardt-head]"
					)
					.unwrap()
					.length(),
				1
			);
			assert_eq!(
				document
					.query_selector_all("meta[name='description'][data-reinhardt-head]")
					.unwrap()
					.length(),
				1
			);

			cleanup_reactive_nodes();
			state.remove();
			root.remove();
			document.set_title(&original_title);
		});
	}

	#[cfg(wasm)]
	#[wasm_bindgen_test]
	fn hydration_preserves_structural_head_precedence_across_reactive_branches() {
		let fixture = HydrationHeadFixture::new(
			"<span>reactive</span><span>conditional</span><span>trailing</span>",
		);

		hydrate(&HydrationStructuralHeadComponent, &fixture.root()).expect("hydration succeeds");

		assert_eq!(fixture.document.title(), "Trailing title");
	}

	#[cfg(wasm)]
	#[wasm_bindgen_test]
	fn hydration_preview_replaces_retained_effect_in_same_render_store() {
		let scope = ReactiveScope::new();
		scope.enter(|| {
			cleanup_reactive_nodes();
			let document = web_sys::window().unwrap().document().unwrap();
			let root = document.create_element("div").unwrap();
			root.set_inner_html("<span>value:0</span>");
			document.body().unwrap().append_child(&root).unwrap();

			let render_signal = Signal::new(0_i32);
			let effect_signal = Signal::new(0_i32);
			let effect_log = Rc::new(RefCell::new(Vec::new()));
			let view = Page::reactive({
				let render_signal = render_signal.clone();
				let effect_signal = effect_signal.clone();
				let effect_log = Rc::clone(&effect_log);
				move || {
					let render_value = render_signal.get();
					use_retained_effect(
						{
							let effect_signal = effect_signal.clone();
							let effect_log = Rc::clone(&effect_log);
							move || {
								let value = effect_signal.get();
								effect_log.borrow_mut().push(format!("run:{value}"));
								let effect_log = Rc::clone(&effect_log);
								Some(move || effect_log.borrow_mut().push("cleanup".to_string()))
							}
						},
						deps![effect_signal],
					);
					PageElement::new("span")
						.child(format!("value:{render_value}"))
						.into_page()
				}
			});

			let mut registry = crate::hydration::events::EventRegistry::new();
			install_hydrated_reactive_nodes(&Element::new(root.clone()), &view, &mut registry)
				.expect("hydration node installation should succeed");
			store_reactive_node(registry);
			effect_signal.set(1);
			with_runtime(|runtime| runtime.flush_updates());

			let log = effect_log.borrow();
			assert_eq!(
				log.iter().filter(|entry| entry.as_str() == "run:1").count(),
				1,
				"only the tracked hydration render should retain an effect: {log:?}"
			);
			drop(log);
			cleanup_reactive_nodes();
			root.remove();
		});
	}

	#[cfg(wasm)]
	#[wasm_bindgen_test]
	fn hydration_element_child_prepasses_do_not_retain_effects() {
		let scope = ReactiveScope::new();
		scope.enter(|| {
			cleanup_reactive_nodes();
			let document = web_sys::window().unwrap().document().unwrap();
			let root = document.create_element("div").unwrap();
			root.set_inner_html("<span>value:0</span>");
			document.body().unwrap().append_child(&root).unwrap();

			let effect_signal = Signal::new(0_i32);
			let effect_log = Rc::new(RefCell::new(Vec::new()));
			let view = PageElement::new("div")
				.child(Page::reactive({
					let effect_signal = effect_signal.clone();
					let effect_log = Rc::clone(&effect_log);
					move || {
						use_retained_effect(
							{
								let effect_signal = effect_signal.clone();
								let effect_log = Rc::clone(&effect_log);
								move || {
									let value = effect_signal.get();
									effect_log.borrow_mut().push(format!("run:{value}"));
									let effect_log = Rc::clone(&effect_log);
									Some(move || {
										effect_log.borrow_mut().push("cleanup".to_string())
									})
								}
							},
							deps![effect_signal],
						);
						PageElement::new("span").child("value:0").into_page()
					}
				}))
				.into_page();

			let mut registry = crate::hydration::events::EventRegistry::new();
			install_hydrated_reactive_nodes(&Element::new(root.clone()), &view, &mut registry)
				.expect("hydration node installation should succeed");
			store_reactive_node(registry);
			effect_signal.set(1);
			with_runtime(|runtime| runtime.flush_updates());

			let log = effect_log.borrow();
			assert_eq!(
				log.iter().filter(|entry| entry.as_str() == "run:1").count(),
				1,
				"hydration prepasses must not retain duplicate effects: {log:?}"
			);
			drop(log);
			cleanup_reactive_nodes();
			root.remove();
		});
	}

	#[cfg(wasm)]
	fn retained_cleanup_child(
		parent_dependency: Signal<i32>,
		effect_dependency: Signal<i32>,
		cleanup_count: Rc<Cell<usize>>,
	) -> Page {
		Page::reactive(move || {
			use_retained_effect(
				{
					let parent_dependency = parent_dependency.clone();
					let effect_dependency = effect_dependency.clone();
					let cleanup_count = Rc::clone(&cleanup_count);
					move || {
						let _ = effect_dependency.get();
						let parent_dependency = parent_dependency.clone();
						let cleanup_count = Rc::clone(&cleanup_count);
						Some(move || {
							cleanup_count.set(cleanup_count.get() + 1);
							parent_dependency.set(1);
						})
					}
				},
				deps![effect_dependency],
			);
			PageElement::new("span").child("initial").into_page()
		})
	}

	#[cfg(wasm)]
	fn direct_comment_count(root: &web_sys::Element) -> usize {
		(0..root.child_nodes().length())
			.filter_map(|index| root.child_nodes().item(index))
			.filter(|node| node.node_type() == web_sys::Node::COMMENT_NODE)
			.count()
	}

	#[cfg(wasm)]
	#[wasm_bindgen_test]
	fn reactive_drop_disposes_parent_before_child_cleanup_updates_dependency() {
		let scope = ReactiveScope::new();
		scope.enter(|| {
			cleanup_reactive_nodes();
			let document = web_sys::window().unwrap().document().unwrap();
			let root = document.create_element("div").unwrap();
			document.body().unwrap().append_child(&root).unwrap();
			let parent_dependency = Signal::new(0_i32);
			let effect_dependency = Signal::new(0_i32);
			let render_count = Rc::new(Cell::new(0_usize));
			let cleanup_count = Rc::new(Cell::new(0_usize));
			let view = Page::reactive({
				let parent_dependency = parent_dependency.clone();
				let effect_dependency = effect_dependency.clone();
				let render_count = Rc::clone(&render_count);
				let cleanup_count = Rc::clone(&cleanup_count);
				move || {
					let _ = parent_dependency.get();
					render_count.set(render_count.get() + 1);
					retained_cleanup_child(
						parent_dependency.clone(),
						effect_dependency.clone(),
						Rc::clone(&cleanup_count),
					)
				}
			});
			view.mount(&Element::new(root.clone())).unwrap();
			with_runtime(|runtime| runtime.flush_updates());

			assert_eq!(render_count.get(), 1);
			cleanup_reactive_nodes();
			parent_dependency.set(2);
			with_runtime(|runtime| runtime.flush_updates());

			assert_eq!(cleanup_count.get(), 1);
			assert_eq!(render_count.get(), 1, "drop must not re-render the parent");
			assert_eq!(root.text_content().as_deref(), Some(""));
			assert_eq!(direct_comment_count(&root), 0, "{}", root.inner_html());
			assert_eq!(root.inner_html(), "");
			root.remove();
		});
	}

	#[cfg(wasm)]
	#[wasm_bindgen_test]
	fn reactive_if_drop_disposes_parent_before_child_cleanup_updates_condition() {
		let scope = ReactiveScope::new();
		scope.enter(|| {
			cleanup_reactive_nodes();
			let document = web_sys::window().unwrap().document().unwrap();
			let root = document.create_element("div").unwrap();
			document.body().unwrap().append_child(&root).unwrap();
			let parent_dependency = Signal::new(0_i32);
			let effect_dependency = Signal::new(0_i32);
			let render_count = Rc::new(Cell::new(0_usize));
			let cleanup_count = Rc::new(Cell::new(0_usize));
			let view = Page::reactive_if(
				{
					let parent_dependency = parent_dependency.clone();
					let render_count = Rc::clone(&render_count);
					move || {
						render_count.set(render_count.get() + 1);
						parent_dependency.get() == 0
					}
				},
				{
					let parent_dependency = parent_dependency.clone();
					let effect_dependency = effect_dependency.clone();
					let cleanup_count = Rc::clone(&cleanup_count);
					move || {
						retained_cleanup_child(
							parent_dependency.clone(),
							effect_dependency.clone(),
							Rc::clone(&cleanup_count),
						)
					}
				},
				|| PageElement::new("span").child("replacement").into_page(),
			);
			view.mount(&Element::new(root.clone())).unwrap();
			with_runtime(|runtime| runtime.flush_updates());

			assert_eq!(render_count.get(), 1);
			cleanup_reactive_nodes();
			parent_dependency.set(2);
			with_runtime(|runtime| runtime.flush_updates());

			assert_eq!(cleanup_count.get(), 1);
			assert_eq!(
				render_count.get(),
				1,
				"drop must not re-evaluate the condition"
			);
			assert_eq!(root.text_content().as_deref(), Some(""));
			assert_eq!(direct_comment_count(&root), 0, "{}", root.inner_html());
			assert_eq!(root.inner_html(), "");
			root.remove();
		});
	}

	#[cfg(wasm)]
	#[wasm_bindgen_test]
	fn hydrated_reactive_owner_refreshes_after_control_adoption() {
		let scope = ReactiveScope::new();
		scope.enter(|| {
			cleanup_reactive_nodes();
			let document = web_sys::window().unwrap().document().unwrap();
			let root = document.create_element("div").unwrap();
			root.set_inner_html("<span>server</span><input value=\"live\">");
			let value = Signal::new("server".to_owned());
			let view = Page::reactive({
				let value = value.clone();
				move || {
					Page::Fragment(vec![
						PageElement::new("span").child(value.get()).into_page(),
						PageElement::new("input")
							.control_binding(ControlBinding::text(value.clone()))
							.into_page(),
					])
				}
			});
			let mut registry = crate::hydration::events::EventRegistry::new();

			install_hydrated_reactive_nodes(&Element::new(root.clone()), &view, &mut registry)
				.expect("hydrate");

			assert_eq!(value.get(), "live");
			assert_eq!(root.text_content().as_deref(), Some("live"));
			cleanup_reactive_nodes();
			root.remove();
		});
	}

	#[cfg(wasm)]
	#[wasm_bindgen_test]
	fn hydrated_reactive_if_refreshes_same_condition_after_control_adoption() {
		let scope = ReactiveScope::new();
		scope.enter(|| {
			cleanup_reactive_nodes();
			let document = web_sys::window().unwrap().document().unwrap();
			let root = document.create_element("div").unwrap();
			root.set_inner_html("<span>server</span><input value=\"live\">");
			let value = Signal::new("server".to_owned());
			let view = Page::reactive_if(
				{
					let value = value.clone();
					move || !value.get().is_empty()
				},
				{
					let value = value.clone();
					move || {
						Page::Fragment(vec![
							PageElement::new("span").child(value.get()).into_page(),
							PageElement::new("input")
								.control_binding(ControlBinding::text(value.clone()))
								.into_page(),
						])
					}
				},
				|| Page::Empty,
			);
			let mut registry = crate::hydration::events::EventRegistry::new();

			install_hydrated_reactive_nodes(&Element::new(root.clone()), &view, &mut registry)
				.expect("hydrate");

			assert_eq!(value.get(), "live");
			assert_eq!(
				root.query_selector("span")
					.unwrap()
					.expect("span")
					.text_content()
					.as_deref(),
				Some("live")
			);
			cleanup_reactive_nodes();
			root.remove();
		});
	}

	#[cfg(wasm)]
	#[wasm_bindgen_test]
	fn hydrated_reactive_if_drops_replaced_binding_registry() {
		let scope = ReactiveScope::new();
		scope.enter(|| {
			cleanup_reactive_nodes();
			let document = web_sys::window().unwrap().document().unwrap();
			let root = document.create_element("div").unwrap();
			root.set_inner_html("<input value=\"live\">");
			let detached_input: web_sys::HtmlInputElement = root
				.query_selector("input")
				.unwrap()
				.expect("input")
				.unchecked_into();
			let value = Signal::new("server".to_owned());
			let view = Page::reactive_if(
				{
					let value = value.clone();
					move || value.get() == "server"
				},
				{
					let value = value.clone();
					move || {
						PageElement::new("input")
							.control_binding(ControlBinding::text(value.clone()))
							.into_page()
					}
				},
				|| PageElement::new("span").child("replacement").into_page(),
			);
			let mut registry = crate::hydration::events::EventRegistry::new();

			install_hydrated_reactive_nodes(&Element::new(root.clone()), &view, &mut registry)
				.expect("hydrate");

			assert_eq!(value.get(), "live");
			assert_eq!(root.text_content().as_deref(), Some("replacement"));
			value.set("next".to_owned());
			with_runtime(|runtime| runtime.flush_updates());
			assert_eq!(detached_input.value(), "live");
			cleanup_reactive_nodes();
			root.remove();
		});
	}

	#[cfg(wasm)]
	#[wasm_bindgen_test]
	fn hydrated_single_control_refreshes_attrs_after_control_adoption() {
		let scope = ReactiveScope::new();
		scope.enter(|| {
			cleanup_reactive_nodes();
			let document = web_sys::window().unwrap().document().unwrap();
			let root = document.create_element("div").unwrap();
			root.set_inner_html("<input class=\"server\" value=\"live\">");
			let value = Signal::new("server".to_owned());
			let view = Page::reactive({
				let value = value.clone();
				move || {
					PageElement::new("input")
						.attr("class", value.get())
						.control_binding(ControlBinding::text(value.clone()))
						.into_page()
				}
			});
			let mut registry = crate::hydration::events::EventRegistry::new();

			install_hydrated_reactive_nodes(&Element::new(root.clone()), &view, &mut registry)
				.expect("hydrate");

			assert_eq!(value.get(), "live");
			assert_eq!(
				root.query_selector("input")
					.unwrap()
					.expect("input")
					.get_attribute("class")
					.as_deref(),
				Some("live")
			);
			cleanup_reactive_nodes();
			root.remove();
		});
	}

	#[cfg(wasm)]
	#[wasm_bindgen_test]
	fn hydrated_single_control_removes_stale_attrs_after_control_adoption() {
		let scope = ReactiveScope::new();
		scope.enter(|| {
			cleanup_reactive_nodes();
			let document = web_sys::window().unwrap().document().unwrap();
			let root = document.create_element("div").unwrap();
			root.set_inner_html("<input data-server-state=\"true\" value=\"live\">");
			let value = Signal::new("server".to_owned());
			let view = Page::reactive({
				let value = value.clone();
				move || {
					let input = PageElement::new("input")
						.control_binding(ControlBinding::text(value.clone()));
					if value.get() == "server" {
						input.attr("data-server-state", "true").into_page()
					} else {
						input.into_page()
					}
				}
			});
			let mut registry = crate::hydration::events::EventRegistry::new();

			install_hydrated_reactive_nodes(&Element::new(root.clone()), &view, &mut registry)
				.expect("hydrate");

			assert_eq!(value.get(), "live");
			assert_eq!(
				root.query_selector("input")
					.unwrap()
					.expect("input")
					.get_attribute("data-server-state"),
				None
			);
			cleanup_reactive_nodes();
			root.remove();
		});
	}

	#[cfg(wasm)]
	#[wasm_bindgen_test]
	fn failed_branch_hydration_drops_nested_owners_and_markers() {
		let scope = ReactiveScope::new();
		scope.enter(|| {
			cleanup_reactive_nodes();
			let document = web_sys::window().unwrap().document().unwrap();
			let root = document.create_element("div").unwrap();
			root.set_inner_html("<span>nested:0</span><div></div>");
			let nested_value = Signal::new(0_i32);
			let binding_value = Signal::new("server".to_owned());
			let view = PageElement::new("div")
				.child(Page::reactive({
					let nested_value = nested_value.clone();
					let binding_value = binding_value.clone();
					move || {
						let nested_value = nested_value.clone();
						Page::Fragment(vec![
							Page::reactive(move || {
								PageElement::new("span")
									.child(format!("nested:{}", nested_value.get()))
									.into_page()
							}),
							PageElement::new("input")
								.control_binding(ControlBinding::text(binding_value.clone()))
								.into_page(),
						])
					}
				}))
				.into_page();
			let mut registry = crate::hydration::events::EventRegistry::new();

			let error =
				install_hydrated_reactive_nodes(&Element::new(root.clone()), &view, &mut registry)
					.expect_err("the later input binding should reject the actual div");
			assert_eq!(
				error,
				HydrationError::EventAttachmentFailed(
					"Failed to attach 'control-binding' event: text control does not support a <div> element"
						.to_owned(),
				),
			);
			nested_value.set(1);
			with_runtime(|runtime| runtime.flush_updates());
			let marker_count = (0..root.child_nodes().length())
				.filter_map(|index| root.child_nodes().item(index))
				.filter(|node| node.node_type() == web_sys::Node::COMMENT_NODE)
				.count();
			assert_eq!(
				(root.text_content().as_deref(), marker_count),
				(Some("nested:0"), 0),
				"failed hydration must leave the initial DOM inert and marker-free",
			);
			cleanup_reactive_nodes();
			root.remove();
		});
	}

	#[cfg(wasm)]
	#[wasm_bindgen_test]
	fn failed_reactive_if_branch_hydration_rejects_invalid_control_binding() {
		let scope = ReactiveScope::new();
		scope.enter(|| {
			cleanup_reactive_nodes();
			let document = web_sys::window().unwrap().document().unwrap();
			let root = document.create_element("div").unwrap();
			root.set_inner_html("<div></div>");
			let binding_value = Signal::new("server".to_owned());
			let view = PageElement::new("div")
				.child(Page::reactive_if(
					|| true,
					{
						let binding_value = binding_value.clone();
						move || {
							PageElement::new("input")
								.control_binding(ControlBinding::text(binding_value.clone()))
								.into_page()
						}
					},
					|| Page::Empty,
				))
				.into_page();
			let mut registry = crate::hydration::events::EventRegistry::new();

			let error =
				install_hydrated_reactive_nodes(&Element::new(root.clone()), &view, &mut registry)
					.expect_err("the reactive-if branch should reject the actual div");
			assert_eq!(
				error,
				HydrationError::EventAttachmentFailed(
					"Failed to attach 'control-binding' event: text control does not support a <div> element"
						.to_owned(),
				),
			);
			cleanup_reactive_nodes();
			root.remove();
		});
	}
}
