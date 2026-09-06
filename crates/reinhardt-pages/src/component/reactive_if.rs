//! Reactive conditional rendering DOM management.
//!
//! This module provides the `ReactiveIfNode` which manages DOM updates
//! for conditional rendering based on Signal changes.

#[cfg(wasm)]
use crate::component::into_page::PageExt;
#[cfg(wasm)]
use crate::reactive::effect::Effect;
#[cfg(wasm)]
use crate::reactive::runtime::{EffectTiming, with_runtime};
#[cfg(wasm)]
use reinhardt_core::reactive::ReactiveScope;
#[cfg(wasm)]
use reinhardt_core::types::page::{
	MountError, Page, is_boolean_attr, is_boolean_attr_truthy, is_safe_html_attribute,
	is_safe_html_element_name,
};
#[cfg(wasm)]
use std::cell::Cell;
use std::cell::RefCell;
#[cfg(native)]
use std::future::Future;
use std::rc::Rc;

pub(crate) type ReactiveNodeStore = Rc<RefCell<Vec<Box<dyn std::any::Any>>>>;

// Thread-local storage for reactive nodes to prevent them from being dropped.
//
// When a reactive node or retained hook effect is created during view mounting,
// it must be kept alive for the lifetime of the current mounted view. This
// storage prevents premature cleanup while still allowing route and portal
// teardown to drop stored values through RAII.
thread_local! {
	static ROOT_REACTIVE_NODES: ReactiveNodeStore = Rc::new(RefCell::new(Vec::new()));
	#[cfg(wasm)]
	static ACTIVE_REACTIVE_NODE_STORE: RefCell<Option<ReactiveNodeStore>> = RefCell::new(None);
	#[cfg(wasm)]
	static HYDRATION_ROLLBACK_DEPTH: Cell<usize> = const { Cell::new(0) };
}

#[cfg(native)]
tokio::task_local! {
	static SSR_REACTIVE_NODE_STORE: ReactiveNodeStore;
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

fn root_reactive_node_store() -> ReactiveNodeStore {
	ROOT_REACTIVE_NODES.with(Clone::clone)
}

#[cfg(wasm)]
fn current_reactive_node_store() -> ReactiveNodeStore {
	ACTIVE_REACTIVE_NODE_STORE
		.with(|active| active.borrow().clone())
		.unwrap_or_else(root_reactive_node_store)
}

#[cfg(native)]
fn current_reactive_node_store() -> ReactiveNodeStore {
	SSR_REACTIVE_NODE_STORE
		.try_with(Clone::clone)
		.unwrap_or_else(|_| root_reactive_node_store())
}

pub(crate) fn new_reactive_node_store() -> ReactiveNodeStore {
	Rc::new(RefCell::new(Vec::new()))
}

pub(crate) fn clear_reactive_node_store(store: &ReactiveNodeStore) {
	let _stored_nodes = {
		let mut stored_nodes = store.borrow_mut();
		std::mem::take(&mut *stored_nodes)
	};
}

/// Drops failed hydration branch owners without removing the server-rendered DOM range.
#[cfg(wasm)]
pub(crate) fn clear_hydration_rollback_reactive_node_store(store: &ReactiveNodeStore) {
	let _guard = HydrationRollbackGuard::enter();
	clear_reactive_node_store(store);
}

#[cfg(wasm)]
struct HydrationRollbackGuard;

#[cfg(wasm)]
impl HydrationRollbackGuard {
	fn enter() -> Self {
		HYDRATION_ROLLBACK_DEPTH.with(|depth| depth.set(depth.get() + 1));
		Self
	}
}

#[cfg(wasm)]
impl Drop for HydrationRollbackGuard {
	fn drop(&mut self) {
		HYDRATION_ROLLBACK_DEPTH.with(|depth| {
			let current_depth = depth.get();
			debug_assert!(current_depth > 0);
			depth.set(current_depth.saturating_sub(1));
		});
	}
}

#[cfg(wasm)]
fn preserves_hydrated_dom_on_drop() -> bool {
	HYDRATION_ROLLBACK_DEPTH.with(|depth| depth.get() > 0)
}

#[cfg(wasm)]
pub(crate) fn with_reactive_node_store<R>(store: &ReactiveNodeStore, f: impl FnOnce() -> R) -> R {
	let previous = ACTIVE_REACTIVE_NODE_STORE.with(|active| active.replace(Some(store.clone())));
	let _guard = ActiveReactiveNodeStoreGuard { previous };
	f()
}

pub(crate) struct ReactiveNodeTransaction {
	destination: ReactiveNodeStore,
	staged: ReactiveNodeStore,
	committed: bool,
}

impl ReactiveNodeTransaction {
	fn new(destination: ReactiveNodeStore) -> Self {
		Self {
			destination,
			staged: new_reactive_node_store(),
			committed: false,
		}
	}

	fn store(&self) -> ReactiveNodeStore {
		self.staged.clone()
	}

	pub(crate) fn commit(&mut self) {
		self.destination
			.borrow_mut()
			.append(&mut self.staged.borrow_mut());
		self.committed = true;
	}
}

impl Drop for ReactiveNodeTransaction {
	fn drop(&mut self) {
		if !self.committed {
			clear_reactive_node_store(&self.staged);
		}
	}
}

#[cfg(wasm)]
pub(crate) fn with_reactive_node_transaction<T, E>(
	f: impl FnOnce() -> Result<T, E>,
) -> Result<T, E> {
	let mut transaction = ReactiveNodeTransaction::new(current_reactive_node_store());
	let result = with_reactive_node_store(&transaction.store(), f);
	if result.is_ok() {
		transaction.commit();
	}
	result
}

#[cfg(native)]
pub(crate) async fn scope_reactive_node_store<R>(future: impl Future<Output = R>) -> R {
	SSR_REACTIVE_NODE_STORE
		.scope(new_reactive_node_store(), future)
		.await
}

#[cfg(native)]
pub(crate) async fn scope_reactive_node_transaction<R>(
	future: impl Future<Output = R>,
) -> (R, ReactiveNodeTransaction) {
	let transaction = ReactiveNodeTransaction::new(current_reactive_node_store());
	let result = SSR_REACTIVE_NODE_STORE
		.scope(transaction.store(), future)
		.await;
	(result, transaction)
}

/// Stores a reactive node to keep it alive.
#[cfg(wasm)]
pub fn store_reactive_node<T: 'static>(node: T) {
	current_reactive_node_store()
		.borrow_mut()
		.push(Box::new(node));
}

/// Stores a reactive scope to keep its arena alive for the mounted view.
#[cfg(wasm)]
pub(crate) fn store_reactive_scope(scope: ReactiveScope) {
	store_reactive_node(scope);
}

/// Stores a reactive node to keep it alive.
#[cfg(native)]
pub(crate) fn store_reactive_node<T: 'static>(node: T) {
	current_reactive_node_store()
		.borrow_mut()
		.push(Box::new(node));
}

/// Owns element attribute effects and disposes their runtime subscriptions on teardown.
#[cfg(wasm)]
pub(crate) struct ReactiveAttributeEffects {
	effects: Vec<Effect>,
}

#[cfg(wasm)]
impl ReactiveAttributeEffects {
	pub(crate) fn new(effects: Vec<Effect>) -> Self {
		Self { effects }
	}
}

#[cfg(wasm)]
impl Drop for ReactiveAttributeEffects {
	fn drop(&mut self) {
		for effect in self.effects.drain(..) {
			effect.dispose();
		}
	}
}

/// Cleanup function to release all reactive nodes.
///
/// This should be called when the application is being torn down or
/// when a complete re-render is needed.
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
	/// Stable start marker for hydrated DOM ranges.
	start_marker: Option<web_sys::Comment>,
	/// Currently mounted DOM nodes
	#[allow(dead_code)] // Kept for potential future use
	current_nodes: Rc<RefCell<Vec<web_sys::Node>>>,
	/// Last evaluated condition value (for change detection)
	#[allow(dead_code)] // Kept for potential future use
	last_condition: Rc<RefCell<Option<bool>>>,
	/// Nested reactive nodes owned by the current branch.
	reactive_nodes: ReactiveNodeStore,
	/// Whether hydration retained the original server-rendered nodes.
	hydrated_nodes_preserved: Rc<Cell<bool>>,
	/// Effect handle (kept alive to maintain reactivity)
	effect: Option<Effect>,
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
		Self::new_with_markers(marker, None, condition, then_view, else_view)
	}

	fn new_before_marker(
		anchor: &web_sys::Comment,
		condition: std::sync::Arc<dyn Fn() -> bool + 'static>,
		then_view: std::sync::Arc<dyn Fn() -> Page + 'static>,
		else_view: std::sync::Arc<dyn Fn() -> Page + 'static>,
	) -> Self {
		let document = web_sys::window()
			.expect("window should be available")
			.document()
			.expect("document should be available");
		let start_marker = document.create_comment("reactive-if-start");
		let marker = document.create_comment("reactive-if");
		let parent = anchor.parent_node().expect("marker should have a parent");
		parent
			.insert_before(&start_marker, Some(anchor))
			.expect("should insert start marker");
		parent
			.insert_before(&marker, Some(anchor))
			.expect("should insert marker");
		Self::new_with_markers(marker, Some(start_marker), condition, then_view, else_view)
	}

	fn new_with_markers(
		marker: web_sys::Comment,
		start_marker: Option<web_sys::Comment>,
		condition: std::sync::Arc<dyn Fn() -> bool + 'static>,
		then_view: std::sync::Arc<dyn Fn() -> Page + 'static>,
		else_view: std::sync::Arc<dyn Fn() -> Page + 'static>,
	) -> Self {
		// Shared state for the Effect
		let current_nodes: Rc<RefCell<Vec<web_sys::Node>>> = Rc::new(RefCell::new(Vec::new()));
		let last_condition: Rc<RefCell<Option<bool>>> = Rc::new(RefCell::new(None));

		// Clone references for the Effect closure
		let current_nodes_clone = current_nodes.clone();
		let last_condition_clone = last_condition.clone();
		let marker_clone = marker.clone();
		let reactive_nodes = new_reactive_node_store();
		let effect_reactive_node_store = current_reactive_node_store();
		let branch_reactive_node_store = reactive_nodes.clone();

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

					clear_reactive_node_store(&branch_reactive_node_store);

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

					let new_nodes = with_reactive_node_store(&branch_reactive_node_store, || {
						// Generate the appropriate view
						let view = if new_condition {
							then_view()
						} else {
							else_view()
						};

						// Mount new nodes before the marker
						mount_before_marker(&marker_clone, view)
					});
					*current_nodes_clone.borrow_mut() = new_nodes;
				});
			},
			EffectTiming::Layout, // Use Layout timing for synchronous DOM updates
		);

		Self {
			marker,
			start_marker,
			current_nodes,
			last_condition,
			reactive_nodes,
			hydrated_nodes_preserved: Rc::new(Cell::new(false)),
			effect: Some(effect),
		}
	}

	// DOM boundary coordinates, the precreated owner store, and the hydrated baseline must stay
	// explicit so hydration adopts the exact server-rendered range without reconstructing state.
	#[allow(clippy::too_many_arguments)]
	pub(crate) fn hydrate_at(
		parent: web_sys::Node,
		next_sibling: Option<web_sys::Node>,
		existing_nodes: Vec<web_sys::Node>,
		hydrated_condition: bool,
		condition: std::sync::Arc<dyn Fn() -> bool + 'static>,
		then_view: std::sync::Arc<dyn Fn() -> Page + 'static>,
		else_view: std::sync::Arc<dyn Fn() -> Page + 'static>,
		reactive_nodes: ReactiveNodeStore,
		refresh_after_control_adoption: bool,
	) -> Option<Self> {
		let document = web_sys::window()
			.expect("window should be available")
			.document()
			.expect("document should be available");
		let start_marker = document.create_comment("reactive-if-start");
		let marker = document.create_comment("reactive-if");
		let start_anchor = existing_nodes.first().or(next_sibling.as_ref());
		let _ = parent.insert_before(&start_marker, start_anchor);
		let _ = parent.insert_before(&marker, next_sibling.as_ref());

		let current_nodes: Rc<RefCell<Vec<web_sys::Node>>> = Rc::new(RefCell::new(existing_nodes));
		let last_condition: Rc<RefCell<Option<bool>>> =
			Rc::new(RefCell::new(Some(hydrated_condition)));
		let current_nodes_clone = current_nodes.clone();
		let last_condition_clone = last_condition.clone();
		let start_marker_clone = Some(start_marker.clone());
		let marker_clone = marker.clone();
		let effect_reactive_node_store = current_reactive_node_store();
		let branch_reactive_node_store = reactive_nodes.clone();
		let first_run = Rc::new(Cell::new(true));
		let first_run_clone = first_run.clone();
		let hydration_mismatch = Rc::new(Cell::new(false));
		let hydration_mismatch_clone = hydration_mismatch.clone();
		let hydrated_nodes_preserved = Rc::new(Cell::new(true));
		let hydrated_nodes_preserved_clone = hydrated_nodes_preserved.clone();
		#[cfg(feature = "i18n")]
		let i18n_context = crate::i18n::current_i18n_callback_context();

		let effect = Effect::new_with_timing(
			move || {
				let update = || {
					with_reactive_node_store(&effect_reactive_node_store, || {
						let new_condition = condition();

						let first_run = first_run_clone.replace(false);
						if first_run {
							hydration_mismatch_clone.set(new_condition != hydrated_condition);
							if !refresh_after_control_adoption {
								return;
							}
						}

						let mut last = last_condition_clone.borrow_mut();
						if *last == Some(new_condition) && !first_run {
							return;
						}
						*last = Some(new_condition);
						drop(last);

						hydrated_nodes_preserved_clone.set(false);
						clear_reactive_node_store(&branch_reactive_node_store);

						refresh_current_nodes_before_marker(
							start_marker_clone.as_ref(),
							&marker_clone,
							&current_nodes_clone,
						);
						let old_nodes = {
							let mut nodes = current_nodes_clone.borrow_mut();
							nodes.drain(..).collect::<Vec<_>>()
						};
						for node in old_nodes {
							if let Some(parent_node) = node.parent_node() {
								let _ = parent_node.remove_child(&node);
							}
						}

						let new_nodes =
							with_reactive_node_store(&branch_reactive_node_store, || {
								let view = if new_condition {
									then_view()
								} else {
									else_view()
								};
								mount_before_marker(&marker_clone, view)
							});
						*current_nodes_clone.borrow_mut() = new_nodes;
					});
				};
				#[cfg(feature = "i18n")]
				crate::i18n::with_optional_i18n_context(i18n_context.as_ref(), update);
				#[cfg(not(feature = "i18n"))]
				update();
			},
			EffectTiming::Layout,
		);
		if hydration_mismatch.get() {
			with_runtime(|runtime| runtime.schedule_update(effect.id()));
		}

		Some(Self {
			marker,
			start_marker: Some(start_marker),
			current_nodes,
			last_condition,
			reactive_nodes,
			hydrated_nodes_preserved,
			effect: Some(effect),
		})
	}

	pub(crate) fn reactive_node_store(&self) -> ReactiveNodeStore {
		self.reactive_nodes.clone()
	}

	pub(crate) fn hydrated_nodes_preserved(&self) -> bool {
		self.hydrated_nodes_preserved.get()
	}

	pub(crate) fn refresh_hydrated_current_nodes(&self) {
		refresh_current_nodes_before_marker(
			self.start_marker.as_ref(),
			&self.marker,
			&self.current_nodes,
		);
	}
}

#[cfg(wasm)]
impl Drop for ReactiveIfNode {
	fn drop(&mut self) {
		let _marker_removal = MarkerRemovalGuard::new(self.start_marker.as_ref(), &self.marker);
		if let Some(effect) = self.effect.take() {
			effect.dispose();
		}
		clear_reactive_node_store(&self.reactive_nodes);
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
	/// Stable start marker for hydrated DOM ranges.
	start_marker: Option<web_sys::Comment>,
	/// Currently mounted DOM nodes
	#[allow(dead_code)] // Kept for potential future use
	current_nodes: Rc<RefCell<Vec<web_sys::Node>>>,
	/// Nested reactive nodes owned by the current render.
	reactive_nodes: ReactiveNodeStore,
	/// Whether hydration retained the original server-rendered nodes.
	hydrated_nodes_preserved: Rc<Cell<bool>>,
	/// Effect handle (kept alive to maintain reactivity)
	effect: Option<Effect>,
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
		Self::new_with_markers(marker, None, render)
	}

	fn new_before_marker(
		anchor: &web_sys::Comment,
		render: std::sync::Arc<dyn Fn() -> Page + 'static>,
	) -> Self {
		let document = web_sys::window()
			.expect("window should be available")
			.document()
			.expect("document should be available");
		let start_marker = document.create_comment("reactive-start");
		let marker = document.create_comment("reactive");
		let parent = anchor.parent_node().expect("marker should have a parent");
		parent
			.insert_before(&start_marker, Some(anchor))
			.expect("should insert start marker");
		parent
			.insert_before(&marker, Some(anchor))
			.expect("should insert marker");
		Self::new_with_markers(marker, Some(start_marker), render)
	}

	fn new_with_markers(
		marker: web_sys::Comment,
		start_marker: Option<web_sys::Comment>,
		render: std::sync::Arc<dyn Fn() -> Page + 'static>,
	) -> Self {
		// Shared state for the Effect
		let current_nodes: Rc<RefCell<Vec<web_sys::Node>>> = Rc::new(RefCell::new(Vec::new()));

		// Clone references for the Effect closure
		let current_nodes_clone = current_nodes.clone();
		let marker_clone = marker.clone();
		let reactive_nodes = new_reactive_node_store();
		let effect_reactive_node_store = current_reactive_node_store();
		let render_reactive_node_store = new_reactive_node_store();
		let mount_reactive_node_store = reactive_nodes.clone();
		let hydrated_nodes_preserved = Rc::new(Cell::new(true));
		let hydrated_nodes_preserved_clone = hydrated_nodes_preserved.clone();
		#[cfg(feature = "i18n")]
		let i18n_context = crate::i18n::current_i18n_callback_context();

		// Create the Effect that will re-run when dependencies change
		let effect = Effect::new_with_timing(
			move || {
				let update = || {
					with_reactive_node_store(&effect_reactive_node_store, || {
						let candidate_render_store = new_reactive_node_store();
						// Render into a candidate store so an Activity attribute-only update
						// retains the owners for the already-mounted content.
						let ScopedRender { view, scope } =
							render_in_reactive_scope(&candidate_render_store, || render());

						if update_activity_boundary_attrs(&current_nodes_clone, &view) {
							clear_reactive_node_store(&candidate_render_store);
							return;
						}
						clear_reactive_node_store(&render_reactive_node_store);
						render_reactive_node_store
							.borrow_mut()
							.append(&mut candidate_render_store.borrow_mut());
						hydrated_nodes_preserved_clone.set(false);

						clear_reactive_node_store(&mount_reactive_node_store);

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
						let new_nodes = scope.enter(|| {
							with_reactive_node_store(&mount_reactive_node_store, || {
								mount_before_marker(&marker_clone, view)
							})
						});
						with_reactive_node_store(&mount_reactive_node_store, || {
							store_reactive_scope(scope)
						});
						*current_nodes_clone.borrow_mut() = new_nodes;
					});
				};
				#[cfg(feature = "i18n")]
				crate::i18n::with_optional_i18n_context(i18n_context.as_ref(), update);
				#[cfg(not(feature = "i18n"))]
				update();
			},
			EffectTiming::Layout, // Use Layout timing for synchronous DOM updates
		);

		Self {
			marker,
			start_marker,
			current_nodes,
			reactive_nodes,
			hydrated_nodes_preserved,
			effect: Some(effect),
		}
	}

	pub(crate) fn hydrate_at(
		parent: web_sys::Node,
		next_sibling: Option<web_sys::Node>,
		existing_nodes: Vec<web_sys::Node>,
		render: std::sync::Arc<dyn Fn() -> Page + 'static>,
		render_reactive_node_store: ReactiveNodeStore,
		reactive_nodes: ReactiveNodeStore,
		refresh_after_control_adoption: bool,
	) -> Option<Self> {
		let document = web_sys::window()
			.expect("window should be available")
			.document()
			.expect("document should be available");
		let start_marker = document.create_comment("reactive-start");
		let marker = document.create_comment("reactive");
		let start_anchor = existing_nodes.first().or(next_sibling.as_ref());
		let _ = parent.insert_before(&start_marker, start_anchor);
		let _ = parent.insert_before(&marker, next_sibling.as_ref());

		let current_nodes: Rc<RefCell<Vec<web_sys::Node>>> = Rc::new(RefCell::new(existing_nodes));
		let current_nodes_clone = current_nodes.clone();
		let start_marker_clone = Some(start_marker.clone());
		let marker_clone = marker.clone();
		let effect_reactive_node_store = current_reactive_node_store();
		let mount_reactive_node_store = reactive_nodes.clone();
		let first_run = Rc::new(Cell::new(true));
		let first_run_clone = first_run.clone();
		let hydrated_nodes_preserved = Rc::new(Cell::new(true));
		let hydrated_nodes_preserved_clone = hydrated_nodes_preserved.clone();
		#[cfg(feature = "i18n")]
		let i18n_context = crate::i18n::current_i18n_callback_context();

		let effect = Effect::new_with_timing(
			move || {
				let update = || {
					with_reactive_node_store(&effect_reactive_node_store, || {
						let candidate_render_store = new_reactive_node_store();
						let first_run_resource_counter =
							crate::reactive::resource::current_client_resource_counter();
						let first_run_id_counter =
							crate::reactive::hooks::id::id_counter_snapshot();
						let ScopedRender { view, scope } =
							render_in_reactive_scope(&candidate_render_store, || render());

						let preserve_adopted_control = refresh_after_control_adoption
							&& single_control_attrs_match(&current_nodes_clone, &view);
						if first_run_clone.replace(false)
							&& (!refresh_after_control_adoption || preserve_adopted_control)
						{
							crate::reactive::resource::set_client_resource_counter(
								first_run_resource_counter,
							);
							crate::reactive::hooks::id::restore_id_counter(first_run_id_counter);
							clear_reactive_node_store(&candidate_render_store);
							return;
						}

						if update_activity_boundary_attrs(&current_nodes_clone, &view) {
							clear_reactive_node_store(&candidate_render_store);
							return;
						}
						clear_reactive_node_store(&render_reactive_node_store);
						render_reactive_node_store
							.borrow_mut()
							.append(&mut candidate_render_store.borrow_mut());
						hydrated_nodes_preserved_clone.set(false);

						clear_reactive_node_store(&mount_reactive_node_store);

						refresh_current_nodes_before_marker(
							start_marker_clone.as_ref(),
							&marker_clone,
							&current_nodes_clone,
						);
						let old_nodes = {
							let mut nodes = current_nodes_clone.borrow_mut();
							nodes.drain(..).collect::<Vec<_>>()
						};
						for node in old_nodes {
							if let Some(parent_node) = node.parent_node() {
								let _ = parent_node.remove_child(&node);
							}
						}

						let new_nodes = scope.enter(|| {
							with_reactive_node_store(&mount_reactive_node_store, || {
								mount_before_marker(&marker_clone, view)
							})
						});
						with_reactive_node_store(&mount_reactive_node_store, || {
							store_reactive_scope(scope)
						});
						*current_nodes_clone.borrow_mut() = new_nodes;
					});
				};
				#[cfg(feature = "i18n")]
				crate::i18n::with_optional_i18n_context(i18n_context.as_ref(), update);
				#[cfg(not(feature = "i18n"))]
				update();
			},
			EffectTiming::Layout,
		);

		Some(Self {
			marker,
			start_marker: Some(start_marker),
			current_nodes,
			reactive_nodes,
			hydrated_nodes_preserved,
			effect: Some(effect),
		})
	}

	pub(crate) fn reactive_node_store(&self) -> ReactiveNodeStore {
		self.reactive_nodes.clone()
	}

	pub(crate) fn hydrated_nodes_preserved(&self) -> bool {
		self.hydrated_nodes_preserved.get()
	}

	pub(crate) fn refresh_hydrated_current_nodes(&self) {
		refresh_current_nodes_before_marker(
			self.start_marker.as_ref(),
			&self.marker,
			&self.current_nodes,
		);
	}
}

#[cfg(wasm)]
struct ScopedRender {
	view: Page,
	scope: ReactiveScope,
}

#[cfg(wasm)]
fn render_in_reactive_scope(
	store: &ReactiveNodeStore,
	render: impl FnOnce() -> Page,
) -> ScopedRender {
	let scope = ReactiveScope::new();
	let view = scope.enter(|| with_reactive_node_store(store, render));
	ScopedRender { view, scope }
}

#[cfg(wasm)]
fn single_control_attrs_match(
	current_nodes: &Rc<RefCell<Vec<web_sys::Node>>>,
	view: &Page,
) -> bool {
	use wasm_bindgen::JsCast;

	match view {
		Page::Element(element)
			if element.bound_control().is_some() && element.child_views().is_empty() =>
		{
			let existing_element = current_nodes
				.borrow()
				.iter()
				.find_map(|node| node.dyn_ref::<web_sys::Element>())
				.cloned();
			let Some(existing_element) = existing_element else {
				return false;
			};
			let has_reactive_override = |name: &str| {
				element
					.reactive_attrs()
					.iter()
					.any(|attribute| attribute.name().eq_ignore_ascii_case(name))
			};
			let expected_attrs_match = element
				.attrs()
				.iter()
				.enumerate()
				.filter(|(index, _)| {
					crate::component::into_page::static_attribute_is_effective(
						element.attrs(),
						*index,
					)
				})
				.all(|(_, (name, value))| {
					let name = name.as_ref();
					if has_reactive_override(name) {
						return true;
					}
					if crate::component::into_page::controlled_attribute_is_overridden(
						element.bound_control(),
						name,
					) {
						return true;
					}
					let expected = if is_boolean_attr(name) && !is_boolean_attr_truthy(value) {
						None
					} else {
						Some(value.as_ref())
					};
					existing_element.get_attribute(name).as_deref() == expected
				}) && element
				.reactive_attrs()
				.iter()
				.enumerate()
				.filter(|(index, attribute)| {
					!element.reactive_attrs()[*index + 1..]
						.iter()
						.any(|later| later.name().eq_ignore_ascii_case(attribute.name()))
				})
				.all(|(_, attribute)| {
					if crate::component::into_page::controlled_attribute_is_overridden(
						element.bound_control(),
						attribute.name(),
					) {
						return true;
					}
					let expected = attribute.value().filter(|value| {
						!is_boolean_attr(attribute.name()) || is_boolean_attr_truthy(value)
					});
					existing_element.get_attribute(attribute.name()).as_deref()
						== expected.as_deref()
				});
			let actual_attrs = existing_element.attributes();
			let actual_attrs_match =
				(0..actual_attrs.length()).all(|index| {
					let Some(attribute) = actual_attrs.item(index) else {
						return true;
					};
					let name = attribute.name();
					if crate::component::into_page::controlled_attribute_is_overridden(
						element.bound_control(),
						&name,
					) {
						return true;
					}
					element.attrs().iter().any(|(expected_name, _)| {
						expected_name.as_ref().eq_ignore_ascii_case(&name)
					}) || has_reactive_override(&name)
				});
			expected_attrs_match && actual_attrs_match
		}
		Page::Fragment(children) => {
			children.len() == 1
				&& children
					.first()
					.is_some_and(|child| single_control_attrs_match(current_nodes, child))
		}
		_ => false,
	}
}

#[cfg(wasm)]
impl Drop for ReactiveNode {
	fn drop(&mut self) {
		let _marker_removal = MarkerRemovalGuard::new(self.start_marker.as_ref(), &self.marker);
		if let Some(effect) = self.effect.take() {
			effect.dispose();
		}
		clear_reactive_node_store(&self.reactive_nodes);
	}
}

#[cfg(wasm)]
struct MarkerRemovalGuard {
	start_marker: Option<web_sys::Comment>,
	marker: web_sys::Comment,
}

#[cfg(wasm)]
impl MarkerRemovalGuard {
	fn new(start_marker: Option<&web_sys::Comment>, marker: &web_sys::Comment) -> Self {
		Self {
			start_marker: start_marker.cloned(),
			marker: marker.clone(),
		}
	}
}

#[cfg(wasm)]
impl Drop for MarkerRemovalGuard {
	fn drop(&mut self) {
		if !preserves_hydrated_dom_on_drop()
			&& let Some(start_marker) = self.start_marker.as_ref()
			&& remove_marker_range_from_dom(start_marker, &self.marker)
		{
			return;
		}
		remove_marker_from_dom(self.start_marker.as_ref());
		remove_marker_from_dom(Some(&self.marker));
	}
}

#[cfg(wasm)]
fn remove_marker_range_from_dom(
	start_marker: &web_sys::Comment,
	marker: &web_sys::Comment,
) -> bool {
	let start_node: web_sys::Node = start_marker.clone().into();
	let marker_node: web_sys::Node = marker.clone().into();
	let Some(parent) = start_node.parent_node() else {
		return false;
	};
	if !marker_node
		.parent_node()
		.is_some_and(|marker_parent| marker_parent.is_same_node(Some(&parent)))
	{
		return false;
	}

	let mut nodes = Vec::new();
	let mut current = Some(start_node);
	while let Some(node) = current {
		let is_marker = node.is_same_node(Some(&marker_node));
		current = node.next_sibling();
		nodes.push(node);
		if is_marker {
			for node in nodes {
				let _ = parent.remove_child(&node);
			}
			return true;
		}
	}

	false
}

#[cfg(wasm)]
fn remove_marker_from_dom(marker: Option<&web_sys::Comment>) {
	let Some(marker) = marker else { return };
	let marker_node: web_sys::Node = marker.clone().into();
	if let Some(parent) = marker_node.parent_node() {
		let _ = parent.remove_child(&marker_node);
	}
}

#[cfg(wasm)]
fn refresh_current_nodes_before_marker(
	start_marker: Option<&web_sys::Comment>,
	marker: &web_sys::Comment,
	current_nodes: &Rc<RefCell<Vec<web_sys::Node>>>,
) {
	let first_node = start_marker
		.and_then(|marker| {
			let marker_node: web_sys::Node = marker.clone().into();
			marker_node.next_sibling()
		})
		.or_else(|| current_nodes.borrow().first().cloned());

	let Some(first_node) = first_node else { return };
	let marker_node: web_sys::Node = marker.clone().into();
	let mut nodes = Vec::new();
	let mut next = Some(first_node);
	while let Some(node) = next {
		if node.is_same_node(Some(&marker_node)) {
			break;
		}
		next = node.next_sibling();
		nodes.push(node);
	}
	*current_nodes.borrow_mut() = nodes;
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
	let Some(existing_element) = nodes
		.iter()
		.filter_map(|node| node.dyn_ref::<web_sys::Element>())
		.find(|element| {
			element.get_attribute("data-rh-state-preserved").as_deref() == Some("true")
				&& element.get_attribute("data-rh-activity").is_some()
		})
	else {
		return false;
	};

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
			if !is_safe_html_element_name(el.tag_name()) {
				let (_, _, _, children, _, _, _) = el.into_parts_with_control_binding();
				for child in children {
					nodes.extend(mount_before_marker(marker, child));
				}
				return nodes;
			}
			let mount_element = || {
				// Decompose the element to avoid ownership issues
				let (
					tag,
					attrs,
					reactive_attrs,
					children,
					_is_void,
					event_handlers,
					control_binding,
				) = el.into_parts_with_control_binding();
				let element = document
					.create_element(&tag)
					.expect("should create element");

				// Set attributes
				for (index, (name, value)) in attrs.iter().enumerate() {
					if !crate::component::into_page::static_attribute_is_effective(&attrs, index) {
						continue;
					}
					if !is_safe_html_attribute(name, value) {
						continue;
					}
					// Skip falsy boolean attributes
					let name_str: &str = name.as_ref();
					if is_boolean_attr(name_str) && !is_boolean_attr_truthy(value) {
						continue;
					}
					if crate::component::into_page::controlled_attribute_is_overridden(
						control_binding.as_ref(),
						name_str,
					) {
						continue;
					}
					let _ = element.set_attribute(name, value);
				}

				let element_wrapper = crate::dom::Element::new(element.clone());
				let mount_children_before_binding = tag.eq_ignore_ascii_case("select");
				let skip_bound_textarea_children =
					control_binding.is_some() && tag.eq_ignore_ascii_case("textarea");
				let mut children = children.into_iter();
				if mount_children_before_binding {
					for child in children.by_ref() {
						child.mount(&element_wrapper)?;
					}
				}

				let initializing_reactive_attributes = std::rc::Rc::new(std::cell::Cell::new(true));
				let reactive_attribute_effects = reactive_attrs
					.iter()
					.enumerate()
					.filter(|(index, attribute)| {
						!reactive_attrs[*index + 1..]
							.iter()
							.any(|later| later.name().eq_ignore_ascii_case(attribute.name()))
					})
					.filter(|(_, attribute)| {
						!crate::component::into_page::controlled_attribute_is_overridden(
							control_binding.as_ref(),
							attribute.name(),
						)
					})
					.map(|(_, attribute)| {
						let attribute = attribute.clone();
						let element = element_wrapper.clone();
						let binding = control_binding.clone();
						let initializing = std::rc::Rc::clone(&initializing_reactive_attributes);
						Effect::new(move || {
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
									if !is_safe_html_attribute(attribute.name(), &value) =>
								{
									let _ = element.remove_attribute(attribute.name());
								}
								Some(value)
									if is_boolean_attr(attribute.name())
										&& !is_boolean_attr_truthy(&value) =>
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
								crate::dom::control_binding::reconcile_control_binding(
									&element, binding,
								) {
								web_sys::console::error_1(
									&format!("controlled input attribute update failed: {error}")
										.into(),
								);
							}
						})
					})
					.collect::<Vec<_>>();
				if let Some(binding) = control_binding.as_ref() {
					crate::component::into_page::initialize_control_default(
						&element_wrapper,
						binding,
					);
				}
				if !reactive_attribute_effects.is_empty() {
					initializing_reactive_attributes.set(false);
					if let Some(binding) = control_binding.as_ref() {
						crate::dom::control_binding::reconcile_control_binding(
							&element_wrapper,
							binding,
						)?;
					}
				}
				let binding_controller = control_binding
					.clone()
					.map(|binding| {
						crate::dom::control_binding::ControlBindingController::mount(
							element_wrapper.clone(),
							binding,
						)
					})
					.transpose()?;
				let mut event_handles = Vec::new();
				for (event_type, handler) in event_handlers {
					let handler_clone = handler.clone();
					#[cfg(feature = "i18n")]
					let i18n_context = crate::i18n::current_i18n_callback_context();
					event_handles.push(element_wrapper.add_event_listener_with_event(
						event_type.as_str(),
						move |event| {
							#[cfg(feature = "i18n")]
							{
								crate::i18n::with_optional_i18n_context(
									i18n_context.as_ref(),
									|| handler_clone(event),
								);
							}
							#[cfg(not(feature = "i18n"))]
							handler_clone(event);
						},
					));
				}

				if !mount_children_before_binding && !skip_bound_textarea_children {
					let child_marker = document.create_comment("reactive-element-children");
					element
						.append_child(&child_marker)
						.map_err(|_| MountError::AppendChildFailed)?;
					for child in children {
						mount_before_marker(&child_marker, child);
					}
					let _ = element.remove_child(&child_marker);
				}

				parent
					.insert_before(&element, Some(marker))
					.map_err(|_| MountError::AppendChildFailed)?;
				store_reactive_node((
					binding_controller,
					event_handles,
					ReactiveAttributeEffects::new(reactive_attribute_effects),
				));
				Ok::<_, MountError>(element.unchecked_into::<web_sys::Node>())
			};
			let Ok(element) = with_reactive_node_transaction(mount_element) else {
				return nodes;
			};
			nodes.push(element);
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
		Page::Outlet(outlet) => {
			let id = outlet.id().map(str::to_string);
			if let Some(child) = outlet.into_child() {
				nodes.extend(mount_before_marker(marker, child));
			} else if let Some(id) = id {
				let element = document
					.create_element("reinhardt-outlet")
					.expect("should create outlet host");
				let _ = element.set_attribute("data-rh-outlet-id", &id);
				let _ = element.set_attribute("style", "display: contents;");
				let _ = parent.insert_before(&element, Some(marker));
				nodes.push(element.unchecked_into());
			}
		}
		Page::Empty => {}
		Page::WithHead { view, head } => {
			match crate::document_head::current_document_head_manager()
				.and_then(|manager| manager.register_static_page(head))
			{
				Ok(registration) => store_reactive_node(registration),
				Err(error) => {
					web_sys::console::error_1(&wasm_bindgen::JsValue::from_str(&error.to_string()));
				}
			}
			nodes.extend(mount_before_marker(marker, *view));
		}
		#[cfg(feature = "hmr")]
		Page::DevTemplate { view, .. } | Page::DevSlot { view, .. } => {
			nodes.extend(mount_before_marker(marker, *view));
		}
		Page::ReactiveIf(reactive_if) => {
			let (condition, then_view, else_view) = reactive_if.into_parts();
			let nested_node =
				ReactiveIfNode::new_before_marker(marker, condition, then_view, else_view);
			store_reactive_node(nested_node);
		}
		Page::Reactive(reactive) => {
			let render = reactive.into_render();
			let nested_node = ReactiveNode::new_before_marker(marker, render);
			store_reactive_node(nested_node);
		}
		Page::Suspense(node) => {
			nodes.extend(mount_before_marker(marker, node.render_branch()));
		}
		Page::Deferred(node) => {
			nodes.extend(mount_before_marker(marker, node.content()));
		}
	}

	nodes
}

// Note: boolean attribute helpers are imported from reinhardt_core::types::page.

#[cfg(all(test, native))]
mod tests {
	use std::cell::Cell;
	use std::panic::{AssertUnwindSafe, catch_unwind};
	use std::rc::Rc;

	use rstest::rstest;

	use super::*;

	struct CyclicDropProbe {
		drops: Rc<Cell<usize>>,
		_store_cycle: ReactiveNodeStore,
	}

	impl Drop for CyclicDropProbe {
		fn drop(&mut self) {
			self.drops.set(self.drops.get() + 1);
		}
	}

	#[rstest]
	fn reactive_node_transaction_rolls_back_cycles_during_unwind() {
		// Arrange
		let destination = new_reactive_node_store();
		let drops = Rc::new(Cell::new(0));
		let transaction_drops = Rc::clone(&drops);

		// Act
		let result = catch_unwind(AssertUnwindSafe(|| {
			let transaction = ReactiveNodeTransaction::new(destination.clone());
			let staged = transaction.store();
			staged.borrow_mut().push(Box::new(CyclicDropProbe {
				drops: transaction_drops,
				_store_cycle: staged.clone(),
			}));
			panic!("transaction rollback");
		}));

		// Assert
		assert!(result.is_err());
		assert_eq!(drops.get(), 1);
		assert!(destination.borrow().is_empty());
	}

	#[rstest]
	fn reactive_node_transaction_transfers_committed_owners() {
		// Arrange
		let destination = new_reactive_node_store();
		let drops = Rc::new(Cell::new(0));
		let mut transaction = ReactiveNodeTransaction::new(destination.clone());
		let staged = transaction.store();
		staged.borrow_mut().push(Box::new(CyclicDropProbe {
			drops: Rc::clone(&drops),
			_store_cycle: staged.clone(),
		}));

		// Act
		transaction.commit();
		drop(transaction);

		// Assert
		assert_eq!(drops.get(), 0);
		assert_eq!(destination.borrow().len(), 1);
		clear_reactive_node_store(&destination);
		assert_eq!(drops.get(), 1);
	}
}
