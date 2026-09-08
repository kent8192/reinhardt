//! Reactive Runtime
//!
//! This module provides the core reactive runtime for managing Signal dependencies,
//! Effect execution, and update scheduling.
//!
//! ## Architecture
//!
//! The reactive system is based on a pull-based reactivity model similar to Leptos and Solid.js:
//!
//! 1. **Observer Stack**: Tracks currently executing Effects
//! 2. **Dependency Tracking**: Automatically records dependencies when Signal::get() is called
//! 3. **Update Scheduling**: Batches multiple Signal changes into a single update cycle
//! 4. **Micro-task Execution**: Uses browser micro-tasks for efficient batching
//!
//! ## Example
//!
//! ```rust
//! use reinhardt_core::reactive::{Effect, ReactiveScope, Signal};
//!
//! ReactiveScope::run(|| {
//!     // Create a signal
//!     let count = Signal::new(0);
//!
//!     // Create an effect that automatically tracks dependencies
//!     let count_for_effect = count.clone();
//!     Effect::new(move || {
//!         // This get() call automatically registers the dependency
//!         println!("Count is: {}", count_for_effect.get());
//!     });
//!
//!     // Update the signal - the effect will automatically re-run
//!     count.set(42);
//! });
//! ```

use core::cell::{Cell, RefCell};
use core::sync::atomic::{AtomicUsize, Ordering};

extern crate alloc;
use alloc::boxed::Box;
use alloc::collections::{BTreeMap, BTreeSet};
use alloc::vec::Vec;

#[derive(Clone, Copy, PartialEq, Eq)]
enum NotificationPhase {
	Idle,
	Propagating,
	Consuming,
}

const MAX_NOTIFICATION_EPOCHS: usize = 32;

/// Unique identifier for reactive nodes (Signals, Effects, Memos)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NodeId(usize);

impl NodeId {
	/// Create a new unique NodeId
	pub fn new() -> Self {
		static COUNTER: AtomicUsize = AtomicUsize::new(0);
		Self(COUNTER.fetch_add(1, Ordering::Relaxed))
	}

	/// Returns the underlying counter value as a `u64`.
	///
	/// Useful when an opaque numeric identifier is needed (e.g. by the
	/// `Trackable` trait in `reinhardt-pages`).
	pub fn as_u64(self) -> u64 {
		self.0 as u64
	}
}

impl Default for NodeId {
	fn default() -> Self {
		Self::new()
	}
}

/// Type of reactive node
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeType {
	/// A Signal node (source of reactivity)
	Signal,
	/// An Effect node (side effect that runs when dependencies change)
	Effect,
	/// A Memo node (cached computation)
	Memo,
}

/// Effect execution timing.
///
/// Determines when an effect should be executed:
/// - Layout effects run synchronously before paint (use_layout_effect)
/// - Passive effects run asynchronously via microtask (use_effect)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EffectTiming {
	/// Layout effect - runs synchronously before paint
	Layout,
	/// Passive effect - runs asynchronously via microtask
	#[default]
	Passive,
}

/// Observer represents a currently executing Effect or Memo
pub struct Observer {
	/// Unique identifier for this observer
	pub id: NodeId,
	/// Type of this observer
	pub node_type: NodeType,
	/// Effect execution timing (only used for Effect nodes)
	pub timing: EffectTiming,
	/// Cleanup function to run when dependencies change (not used yet)
	pub cleanup: Option<()>,
}

impl Clone for Observer {
	fn clone(&self) -> Self {
		Self {
			id: self.id,
			node_type: self.node_type,
			timing: self.timing,
			cleanup: None, // Cleanup functions are not cloneable
		}
	}
}

/// Dependency graph node
#[derive(Debug, Default)]
pub(crate) struct DependencyNode {
	/// IDs of nodes that depend on this node
	pub(crate) subscribers: Vec<NodeId>,
	/// IDs of nodes this node depends on
	pub(crate) dependencies: Vec<NodeId>,
}

/// Type for async task scheduler function
type SchedulerFn = Box<dyn Fn(Box<dyn FnOnce() + Send>) + Send + Sync>;

/// Global scheduler function
static SCHEDULER: std::sync::OnceLock<SchedulerFn> = std::sync::OnceLock::new();

/// Set the global scheduler function for async task execution.
///
/// This should be called once at application startup to configure how
/// async updates are scheduled. In WASM environments, this would typically
/// use `wasm_bindgen_futures::spawn_local`.
///
/// # Arguments
///
/// * `scheduler` - A function that takes a boxed closure and schedules it for execution.
///
/// # Example
///
/// ```ignore
/// // In WASM environment
/// reinhardt_core::reactive::runtime::set_scheduler(|task| {
///     wasm_bindgen_futures::spawn_local(async move { task() });
/// });
/// ```
pub fn set_scheduler<F>(scheduler: F)
where
	F: Fn(Box<dyn FnOnce() + Send>) + Send + Sync + 'static,
{
	let _ = SCHEDULER.set(Box::new(scheduler));
}

/// Global reactive runtime
///
/// This struct manages the reactive dependency graph and update scheduling.
/// It uses thread-local storage to maintain separate runtime state per thread.
pub struct Runtime {
	/// Observer stack for tracking currently executing effects
	observer_stack: RefCell<Vec<Observer>>,
	/// Dependency graph: NodeId -> DependencyNode
	pub(crate) dependency_graph: RefCell<BTreeMap<NodeId, DependencyNode>>,
	/// Pending updates (nodes that need to be re-executed)
	pub(crate) pending_updates: RefCell<Vec<NodeId>>,
	/// Whether an update is currently scheduled
	pub(crate) update_scheduled: RefCell<bool>,
	/// Active explicit batch nesting depth.
	pub(crate) batch_depth: RefCell<usize>,
	/// Current notification processing phase.
	notification_phase: Cell<NotificationPhase>,
	/// Reactive nodes whose subscribers still need propagation.
	notification_sources: RefCell<Vec<NodeId>>,
	/// Source notifications raised while consumers execute.
	notification_next_sources: RefCell<Vec<NodeId>>,
	/// Consumer-raised sources retained after a consumer panic.
	notification_recovery_sources: RefCell<Vec<NodeId>>,
	/// Memos already propagated in the current epoch.
	notification_memos_seen: RefCell<BTreeSet<NodeId>>,
	/// Consumers already collected in the current epoch.
	notification_consumers_seen: RefCell<BTreeSet<NodeId>>,
	/// Layout effects collected after propagation completes.
	notification_layout_effects: RefCell<Vec<NodeId>>,
	/// Passive consumers collected after propagation completes.
	notification_passive: RefCell<Vec<NodeId>>,
	/// Number of notifications emitted by each signal.
	signal_revisions: RefCell<BTreeMap<NodeId, usize>>,
}

impl Runtime {
	/// Create a new Runtime instance
	pub fn new() -> Self {
		Self {
			observer_stack: RefCell::new(Vec::new()),
			dependency_graph: RefCell::new(BTreeMap::new()),
			pending_updates: RefCell::new(Vec::new()),
			update_scheduled: RefCell::new(false),
			batch_depth: RefCell::new(0),
			notification_phase: Cell::new(NotificationPhase::Idle),
			notification_sources: RefCell::new(Vec::new()),
			notification_next_sources: RefCell::new(Vec::new()),
			notification_recovery_sources: RefCell::new(Vec::new()),
			notification_memos_seen: RefCell::new(BTreeSet::new()),
			notification_consumers_seen: RefCell::new(BTreeSet::new()),
			notification_layout_effects: RefCell::new(Vec::new()),
			notification_passive: RefCell::new(Vec::new()),
			signal_revisions: RefCell::new(BTreeMap::new()),
		}
	}

	/// Get the current observer (the currently executing Effect or Memo)
	pub fn current_observer(&self) -> Option<NodeId> {
		self.observer_stack
			.borrow()
			.last()
			.map(|observer| observer.id)
	}

	/// Push an observer onto the stack
	///
	/// This should be called when starting to execute an Effect or Memo.
	pub fn push_observer(&self, observer: Observer) {
		self.observer_stack.borrow_mut().push(observer);
	}

	/// Pop an observer from the stack
	///
	/// This should be called when finishing execution of an Effect or Memo.
	pub fn pop_observer(&self) -> Option<Observer> {
		self.observer_stack.borrow_mut().pop()
	}

	/// Track a dependency between the current observer and a signal
	///
	/// This is called automatically when Signal::get() is invoked.
	///
	/// # Arguments
	///
	/// * `signal_id` - ID of the Signal being accessed
	pub fn track_dependency(&self, signal_id: NodeId) {
		if let Some(observer_id) = self.current_observer() {
			let mut graph = self.dependency_graph.borrow_mut();

			// Add signal -> observer edge (signal has a new subscriber)
			let signal_node = graph.entry(signal_id).or_default();
			if !signal_node.subscribers.contains(&observer_id) {
				signal_node.subscribers.push(observer_id);
			}

			// Add observer -> signal edge (observer depends on signal)
			let observer_node = graph.entry(observer_id).or_default();
			if !observer_node.dependencies.contains(&signal_id) {
				observer_node.dependencies.push(signal_id);
			}
		}
	}

	/// Notify that a Signal has changed
	///
	/// Each notification epoch first propagates dirty state through every Memo,
	/// then executes Layout effects and schedules passive consumers. Writes
	/// raised by a consumer are processed in a new epoch after the current
	/// consumers finish.
	///
	/// # Arguments
	///
	/// * `signal_id` - ID of the Signal that changed
	pub fn notify_signal_change(&self, signal_id: NodeId) {
		self.notify_signal_changes(core::slice::from_ref(&signal_id));
	}

	/// Notify multiple signals as one propagation wave.
	///
	/// All source values must already be updated before this method is called.
	/// Keeping the source IDs in the same wave prevents a consumer subscribed to
	/// more than one source from running once per source.
	pub(crate) fn notify_signal_changes(&self, signal_ids: &[NodeId]) {
		if signal_ids.is_empty() {
			return;
		}

		let mut revisions = self.signal_revisions.borrow_mut();
		for &signal_id in signal_ids {
			let revision = revisions.entry(signal_id).or_default();
			*revision = revision.saturating_add(1);
		}
		drop(revisions);
		match self.notification_phase.get() {
			NotificationPhase::Idle => {
				let mut sources = self.notification_sources.borrow_mut();
				sources.extend(signal_ids.iter().copied());
				drop(sources);
				self.notification_phase.set(NotificationPhase::Propagating);
				self.process_notification_epochs();
				self.schedule_pending_flush();
			}
			NotificationPhase::Propagating => {
				self.notification_sources
					.borrow_mut()
					.extend(signal_ids.iter().copied());
			}
			NotificationPhase::Consuming => {
				self.notification_next_sources
					.borrow_mut()
					.extend(signal_ids.iter().copied());
			}
		}
	}

	/// Returns how many times a signal has notified the runtime.
	#[must_use]
	pub fn signal_revision(&self, signal_id: NodeId) -> usize {
		self.signal_revisions
			.borrow()
			.get(&signal_id)
			.copied()
			.unwrap_or_default()
	}

	fn process_notification_epochs(&self) {
		struct NotificationWaveGuard<'a> {
			runtime: &'a Runtime,
			completed: bool,
			discard_pending: bool,
		}

		impl Drop for NotificationWaveGuard<'_> {
			fn drop(&mut self) {
				self.runtime.notification_sources.borrow_mut().clear();
				if self.discard_pending {
					self.runtime.notification_next_sources.borrow_mut().clear();
					self.runtime
						.notification_recovery_sources
						.borrow_mut()
						.clear();
				} else if self.completed {
					self.runtime.notification_next_sources.borrow_mut().clear();
				} else {
					let pending =
						core::mem::take(&mut *self.runtime.notification_next_sources.borrow_mut());
					self.runtime
						.notification_recovery_sources
						.borrow_mut()
						.extend(pending);
					self.runtime.queue_updates(core::mem::take(
						&mut *self.runtime.notification_passive.borrow_mut(),
					));
				}
				self.runtime.notification_memos_seen.borrow_mut().clear();
				self.runtime
					.notification_consumers_seen
					.borrow_mut()
					.clear();
				self.runtime
					.notification_layout_effects
					.borrow_mut()
					.clear();
				self.runtime.notification_passive.borrow_mut().clear();
				self.runtime.notification_phase.set(NotificationPhase::Idle);
			}
		}

		let mut wave_guard = NotificationWaveGuard {
			runtime: self,
			completed: false,
			discard_pending: false,
		};
		let recovery = core::mem::take(&mut *self.notification_recovery_sources.borrow_mut());
		self.notification_sources.borrow_mut().extend(recovery);
		let mut epoch_count = 0_usize;
		loop {
			epoch_count += 1;
			if epoch_count > MAX_NOTIFICATION_EPOCHS {
				wave_guard.discard_pending = true;
				panic!(
					"reactive notification exceeded {MAX_NOTIFICATION_EPOCHS} epochs; possible non-converging layout update loop"
				);
			}
			self.notification_phase.set(NotificationPhase::Propagating);
			self.notification_memos_seen.borrow_mut().clear();
			self.notification_consumers_seen.borrow_mut().clear();
			self.notification_layout_effects.borrow_mut().clear();
			self.notification_passive.borrow_mut().clear();
			if *self.batch_depth.borrow() == 0 {
				// Deferred Layout effects join the same epoch as fresh sources.
				// Retain passive work so new notifications cannot enqueue it twice.
				self.pending_updates.borrow_mut().retain(|node_id| {
					if super::effect::get_effect_timing(*node_id) == Some(EffectTiming::Layout) {
						self.notification_consumers_seen
							.borrow_mut()
							.insert(*node_id);
						self.notification_layout_effects.borrow_mut().push(*node_id);
						false
					} else {
						true
					}
				});
			}

			loop {
				let source_id = { self.notification_sources.borrow_mut().pop() };
				let Some(source_id) = source_id else {
					break;
				};
				self.propagate_notification_source(source_id);
			}

			self.notification_phase.set(NotificationPhase::Consuming);
			let layout_effects =
				core::mem::take(&mut *self.notification_layout_effects.borrow_mut());
			if *self.batch_depth.borrow() > 0 {
				self.queue_updates(layout_effects);
			} else {
				self.execute_pending_effects(layout_effects);
			}
			let passive = core::mem::take(&mut *self.notification_passive.borrow_mut());
			self.queue_updates(passive);

			let next_sources = core::mem::take(&mut *self.notification_next_sources.borrow_mut());
			let has_pending_layout = *self.batch_depth.borrow() == 0
				&& self.pending_updates.borrow().iter().any(|&node_id| {
					super::effect::get_effect_timing(node_id) == Some(EffectTiming::Layout)
				});
			if next_sources.is_empty() && !has_pending_layout {
				break;
			}
			self.notification_sources.borrow_mut().extend(next_sources);
		}
		wave_guard.completed = true;
	}

	fn execute_pending_effects(&self, effects: Vec<NodeId>) {
		struct PendingEffectsGuard<'a> {
			runtime: &'a Runtime,
			remaining: alloc::vec::IntoIter<NodeId>,
		}

		impl Drop for PendingEffectsGuard<'_> {
			fn drop(&mut self) {
				// Keep callbacks that did not run when an earlier callback panicked.
				// Scheduling a flush here could execute user code during unwinding.
				self.runtime.queue_updates(self.remaining.by_ref());
			}
		}

		let mut pending = PendingEffectsGuard {
			runtime: self,
			remaining: effects.into_iter(),
		};
		for node_id in pending.remaining.by_ref() {
			super::effect::Effect::execute_effect(node_id);
		}
	}

	/// Flush pending Layout effects through notification epochs, then passive effects.
	///
	/// Layout writes converge before passive consumers run. A flush inside a batch,
	/// an active notification, or panic unwinding is deferred. Unexecuted effects
	/// survive a callback panic for the next normal batch, notification, or flush.
	pub fn flush_updates(&self) {
		if *self.batch_depth.borrow() > 0
			|| self.notification_phase.get() != NotificationPhase::Idle
			|| std::thread::panicking()
		{
			return;
		}
		*self.update_scheduled.borrow_mut() = false;
		self.process_notification_epochs();
		let pending = core::mem::take(&mut *self.pending_updates.borrow_mut());
		self.execute_pending_effects(pending);
	}

	fn propagate_notification_source(&self, node_id: NodeId) {
		let graph = self.dependency_graph.borrow();
		let Some(node) = graph.get(&node_id) else {
			return;
		};
		let subscribers = node.subscribers.clone();
		drop(graph);

		for subscriber_id in subscribers {
			if let Some(timing) = super::effect::get_effect_timing(subscriber_id) {
				if self
					.notification_consumers_seen
					.borrow_mut()
					.insert(subscriber_id)
				{
					match timing {
						EffectTiming::Layout => self
							.notification_layout_effects
							.borrow_mut()
							.push(subscriber_id),
						EffectTiming::Passive => {
							self.notification_passive.borrow_mut().push(subscriber_id)
						}
					}
				}
			} else if super::memo::is_memo_registered(subscriber_id)
				&& self
					.notification_memos_seen
					.borrow_mut()
					.insert(subscriber_id)
			{
				super::memo::mark_memo_dirty_by_id(subscriber_id);
			}
		}
	}

	/// Schedule a node for update
	///
	/// The actual update will be performed in a batched micro-task.
	///
	/// # Arguments
	///
	/// * `node_id` - ID of the node to update
	pub fn schedule_update(&self, node_id: NodeId) {
		self.queue_updates([node_id]);
		self.schedule_pending_flush();
	}

	fn queue_updates(&self, nodes: impl IntoIterator<Item = NodeId>) {
		let mut pending = self.pending_updates.borrow_mut();
		for node_id in nodes {
			if !pending.contains(&node_id) {
				pending.push(node_id);
			}
		}
	}

	fn schedule_pending_flush(&self) {
		if *self.batch_depth.borrow() > 0
			|| self.notification_phase.get() != NotificationPhase::Idle
			|| self.pending_updates.borrow().is_empty()
		{
			return;
		}

		// Schedule flush if not already scheduled
		if !*self.update_scheduled.borrow() {
			*self.update_scheduled.borrow_mut() = true;

			// If a scheduler is set, use it to schedule the flush
			if let Some(scheduler) = SCHEDULER.get() {
				scheduler(Box::new(|| {
					RUNTIME.with(|rt| rt.flush_updates());
				}));
			}
			// If no scheduler is set, updates must be flushed manually
			// This is the case for non-WASM environments or during testing
		}
	}

	/// Clear dependencies for a node
	///
	/// This should be called before re-executing an Effect/Memo to clear old dependencies.
	///
	/// # Arguments
	///
	/// * `node_id` - ID of the node whose dependencies should be cleared
	pub fn clear_dependencies(&self, node_id: NodeId) {
		let mut graph = self.dependency_graph.borrow_mut();

		// Get the current dependencies
		if let Some(node) = graph.get(&node_id) {
			let dependencies = node.dependencies.clone();

			// Remove this node from all signal subscribers
			for &dep_id in &dependencies {
				if let Some(dep_node) = graph.get_mut(&dep_id) {
					dep_node.subscribers.retain(|&id| id != node_id);
				}
			}
		}

		// Clear the dependencies list
		if let Some(node) = graph.get_mut(&node_id) {
			node.dependencies.clear();
		}
	}

	/// Remove a node from the dependency graph
	///
	/// This should be called when a Signal/Effect/Memo is dropped.
	/// Also removes the node from pending updates to prevent disposed effects
	/// from being re-scheduled, which could cause infinite loops.
	///
	/// # Arguments
	///
	/// * `node_id` - ID of the node to remove
	pub fn remove_node(&self, node_id: NodeId) {
		self.clear_dependencies(node_id);
		self.dependency_graph.borrow_mut().remove(&node_id);
		self.signal_revisions.borrow_mut().remove(&node_id);
		// Remove from pending updates to prevent re-execution of disposed effects
		self.pending_updates
			.borrow_mut()
			.retain(|&id| id != node_id);
	}

	/// Check if a node exists in the dependency graph (for testing)
	pub fn has_node(&self, node_id: NodeId) -> bool {
		self.dependency_graph.borrow().contains_key(&node_id)
	}

	/// Get the number of subscribers for a node (for testing)
	pub fn subscriber_count(&self, node_id: NodeId) -> usize {
		self.dependency_graph
			.borrow()
			.get(&node_id)
			.map(|node| node.subscribers.len())
			.unwrap_or(0)
	}

	/// Returns the list of NodeIds subscribed to the given node.
	///
	/// Diagnostic-only. Used by `reinhardt-pages` WASM tests to verify
	/// dependency-tracking shape (Refs #4088). Analogous to React's
	/// internal subscriber tracking inside `useSyncExternalStore`.
	#[doc(hidden)]
	pub fn debug_subscribers(&self, node_id: NodeId) -> alloc::vec::Vec<NodeId> {
		self.dependency_graph
			.borrow()
			.get(&node_id)
			.map(|n| n.subscribers.clone())
			.unwrap_or_default()
	}

	/// Returns the list of NodeIds the given observer depends on.
	///
	/// Diagnostic-only (Refs #4088).
	#[doc(hidden)]
	pub fn debug_dependencies(&self, node_id: NodeId) -> alloc::vec::Vec<NodeId> {
		self.dependency_graph
			.borrow()
			.get(&node_id)
			.map(|n| n.dependencies.clone())
			.unwrap_or_default()
	}

	/// Returns the current observer stack as a list of NodeIds (bottom to top).
	///
	/// Diagnostic-only (Refs #4088).
	#[doc(hidden)]
	pub fn debug_observer_stack(&self) -> alloc::vec::Vec<NodeId> {
		self.observer_stack.borrow().iter().map(|o| o.id).collect()
	}

	/// Returns the pending updates queue as a snapshot (does not drain).
	///
	/// Diagnostic-only (Refs #4088).
	#[doc(hidden)]
	pub fn debug_pending_updates(&self) -> alloc::vec::Vec<NodeId> {
		self.pending_updates.borrow().clone()
	}
}

impl Default for Runtime {
	fn default() -> Self {
		Self::new()
	}
}

// Thread-local runtime instance
//
// In WASM, there is only one thread, so this effectively provides a global runtime.
// On non-WASM platforms, each thread gets its own runtime instance.
thread_local! {
	static RUNTIME: Runtime = Runtime::new();
}

/// Get a reference to the global runtime
///
/// # Example
///
/// ```rust
/// use reinhardt_core::reactive::runtime::{with_runtime, NodeId};
///
/// let signal_id = NodeId::new();
/// with_runtime(|rt| {
///     rt.track_dependency(signal_id);
/// });
/// ```
pub fn with_runtime<F, R>(f: F) -> R
where
	F: FnOnce(&Runtime) -> R,
{
	RUNTIME.with(f)
}

/// Execute multiple reactive writes as a single update cycle.
///
/// Effect re-executions are queued, then flushed once the outermost batch returns
/// normally, including an `Err` result. Layout effects run before passive effects.
/// Nested batches share the same queue. All Memos are invalidated immediately so
/// reads inside the batch observe the current signal values.
///
/// During panic unwinding, callbacks remain queued to preserve the original panic.
/// The next normal batch or flush drains them; a subsequent signal notification
/// runs pending Layout effects and schedules passive work.
pub fn batch<R>(f: impl FnOnce() -> R) -> R {
	struct BatchGuard;

	impl Drop for BatchGuard {
		fn drop(&mut self) {
			let _ = try_with_runtime(|rt| {
				let should_flush = {
					let mut depth = rt.batch_depth.borrow_mut();
					debug_assert!(*depth > 0, "reactive batch depth underflow");
					*depth -= 1;
					*depth == 0
						&& (!rt.pending_updates.borrow().is_empty()
							|| !rt.notification_recovery_sources.borrow().is_empty())
				};

				if should_flush && !std::thread::panicking() {
					rt.flush_updates();
				}
			});
		}
	}

	with_runtime(|rt| {
		*rt.batch_depth.borrow_mut() += 1;
	});

	let _guard = BatchGuard;
	f()
}

/// Try to access the global runtime (safe version for Drop implementations)
///
/// Returns None if the thread-local storage has been destroyed.
pub(crate) fn try_with_runtime<F, R>(f: F) -> Option<R>
where
	F: FnOnce(&Runtime) -> R,
{
	RUNTIME.try_with(f).ok()
}

/// Execute `f` with the active Observer (if any) temporarily detached.
///
/// `Signal::get` calls inside `f` will not auto-subscribe to the outer
/// reactive context. The Observer stack is restored before this function
/// returns, including on panic.
///
/// Used by `*::new_with_deps` constructors to implement the React-aligned
/// "closure runs without Observer; only listed deps subscribe" semantics
/// (Refs #4195).
//
// Unused in lib builds until Task 5 (`Effect::new_with_deps`) and Task 6
// (`Memo::new_with_deps`) land. Tests in this file already exercise it.
#[allow(dead_code)]
pub(crate) fn run_without_observer<R>(f: impl FnOnce() -> R) -> R {
	struct Restore {
		saved: Vec<Observer>,
		active: bool,
	}
	impl Drop for Restore {
		fn drop(&mut self) {
			if self.active {
				let saved = core::mem::take(&mut self.saved);
				// Best-effort restore on panic. `try_with_runtime` guards
				// against thread-local destruction during shutdown.
				let _ = try_with_runtime(|rt| {
					*rt.observer_stack.borrow_mut() = saved;
				});
			}
		}
	}

	let Some(saved) = try_with_runtime(|rt| core::mem::take(&mut *rt.observer_stack.borrow_mut()))
	else {
		return f();
	};
	let mut guard = Restore {
		saved,
		active: true,
	};
	let result = f();
	// Success path: restore inline and disarm the Drop guard so the panic
	// branch does not double-restore.
	let saved = core::mem::take(&mut guard.saved);
	with_runtime(|rt| {
		*rt.observer_stack.borrow_mut() = saved;
	});
	guard.active = false;
	result
}

/// Executes a closure without subscribing the active reactive observer.
///
/// This is useful for imperative initialization that must read signal-backed
/// state without turning the surrounding render or effect into a subscriber.
pub fn untracked<R>(f: impl FnOnce() -> R) -> R {
	run_without_observer(f)
}

/// Wire an explicit subscription edge from `node` to `observer` in the
/// reactive dependency graph, bypassing the auto-tracking Observer stack.
///
/// Used by `Effect::new_with_deps` and `Memo::new_with_deps` to subscribe
/// the observer to each listed dependency after running the closure with
/// no Observer active (Refs #4195).
//
// Unused in lib builds until Task 5 / Task 6. Tests in this file exercise it.
#[allow(dead_code)]
pub(crate) fn subscribe_node_to_observer(node: NodeId, observer: NodeId) {
	with_runtime(|rt| {
		let mut graph = rt.dependency_graph.borrow_mut();

		// node -> observer: node has a new subscriber
		let node_entry = graph.entry(node).or_default();
		if !node_entry.subscribers.contains(&observer) {
			node_entry.subscribers.push(observer);
		}

		// observer -> node: observer now depends on this node
		let obs_entry = graph.entry(observer).or_default();
		if !obs_entry.dependencies.contains(&node) {
			obs_entry.dependencies.push(node);
		}
	});
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::reactive::{Effect, Memo, ReactiveScope, Signal};
	#[cfg(native)]
	use reinhardt_test::resource::{TeardownGuard, TestResource};
	use serial_test::serial;
	use std::{cell::Cell, rc::Rc};

	#[cfg(native)]
	impl TestResource for ReactiveScope {
		fn setup() -> Self {
			Self::new()
		}

		fn teardown(&mut self) {
			self.dispose();
		}
	}

	// Keep unit tests on this crate's runtime rather than the facade's compiled core.
	#[cfg(native)]
	type ReactiveScopeFixture = TeardownGuard<ReactiveScope>;
	#[cfg(wasm)]
	type ReactiveScopeFixture = ReactiveScope;

	#[rstest::fixture]
	fn reactive_scope() -> ReactiveScopeFixture {
		ReactiveScopeFixture::default()
	}

	#[test]
	#[serial]
	fn test_node_id_uniqueness() {
		let id1 = NodeId::new();
		let id2 = NodeId::new();
		let id3 = NodeId::new();

		assert_ne!(id1, id2);
		assert_ne!(id2, id3);
		assert_ne!(id1, id3);
	}

	#[test]
	#[serial]
	fn test_runtime_observer_stack() {
		let runtime = Runtime::new();

		assert!(runtime.current_observer().is_none());

		let observer1 = Observer {
			id: NodeId::new(),
			node_type: NodeType::Effect,
			timing: EffectTiming::default(),
			cleanup: None,
		};
		let id1 = observer1.id;

		runtime.push_observer(observer1);
		assert_eq!(runtime.current_observer(), Some(id1));

		let observer2 = Observer {
			id: NodeId::new(),
			node_type: NodeType::Effect,
			timing: EffectTiming::default(),
			cleanup: None,
		};
		let id2 = observer2.id;

		runtime.push_observer(observer2);
		assert_eq!(runtime.current_observer(), Some(id2));

		runtime.pop_observer();
		assert_eq!(runtime.current_observer(), Some(id1));

		runtime.pop_observer();
		assert!(runtime.current_observer().is_none());
	}

	#[test]
	#[serial]
	fn test_dependency_tracking() {
		let runtime = Runtime::new();

		let signal_id = NodeId::new();
		let effect_id = NodeId::new();

		// Push effect observer
		runtime.push_observer(Observer {
			id: effect_id,
			node_type: NodeType::Effect,
			timing: EffectTiming::default(),
			cleanup: None,
		});

		// Track dependency
		runtime.track_dependency(signal_id);

		// Verify dependency was recorded
		let graph = runtime.dependency_graph.borrow();
		let signal_node = graph.get(&signal_id).unwrap();
		assert!(signal_node.subscribers.contains(&effect_id));

		let effect_node = graph.get(&effect_id).unwrap();
		assert!(effect_node.dependencies.contains(&signal_id));
	}

	#[test]
	#[serial(reactive_runtime)]
	fn test_notify_signal_change() {
		crate::reactive::ReactiveScope::run(|| {
			let signal = crate::reactive::Signal::new(0_i32);
			let run_count = Rc::new(Cell::new(0));
			let signal_for_effect = signal;
			let run_count_for_effect = Rc::clone(&run_count);
			let effect = crate::reactive::Effect::new(move || {
				let _ = signal_for_effect.get();
				run_count_for_effect.set(run_count_for_effect.get() + 1);
			});
			assert_eq!(run_count.get(), 1);

			with_runtime(|runtime| {
				let graph = runtime.dependency_graph.borrow();
				assert!(graph[&signal.id()].subscribers.contains(&effect.id()));
				assert!(graph[&effect.id()].dependencies.contains(&signal.id()));
				drop(graph);

				runtime.notify_signal_change(signal.id());
				assert!(runtime.pending_updates.borrow().contains(&effect.id()));
				runtime.flush_updates();
			});
			assert_eq!(run_count.get(), 2);
		});
	}

	#[test]
	#[serial]
	fn test_notify_signal_change_ignores_stale_subscribers() {
		let runtime = Runtime::new();
		let signal_id = NodeId::new();
		let stale_effect_id = NodeId::new();

		// Manually add a dependency whose effect node no longer exists.
		{
			let mut graph = runtime.dependency_graph.borrow_mut();
			graph
				.entry(signal_id)
				.or_default()
				.subscribers
				.push(stale_effect_id);
		}

		// Notify change.
		runtime.notify_signal_change(signal_id);

		// Stale scope-owned effects must not be scheduled.
		let pending = runtime.pending_updates.borrow();
		assert!(!pending.contains(&stale_effect_id));
	}

	#[test]
	#[serial]
	fn test_clear_dependencies() {
		let runtime = Runtime::new();

		let signal_id = NodeId::new();
		let effect_id = NodeId::new();

		// Manually add dependency
		{
			let mut graph = runtime.dependency_graph.borrow_mut();
			graph
				.entry(signal_id)
				.or_default()
				.subscribers
				.push(effect_id);
			graph
				.entry(effect_id)
				.or_default()
				.dependencies
				.push(signal_id);
		}

		// Clear dependencies
		runtime.clear_dependencies(effect_id);

		// Verify dependencies were cleared
		let graph = runtime.dependency_graph.borrow();
		let signal_node = graph.get(&signal_id).unwrap();
		assert!(!signal_node.subscribers.contains(&effect_id));

		let effect_node = graph.get(&effect_id).unwrap();
		assert!(effect_node.dependencies.is_empty());
	}

	#[test]
	#[serial]
	fn debug_subscribers_returns_registered_observers_in_insertion_order() {
		// Arrange
		let runtime = Runtime::new();
		let signal_id = NodeId::new();
		let effect_id_a = NodeId::new();
		let effect_id_b = NodeId::new();
		{
			let mut graph = runtime.dependency_graph.borrow_mut();
			let node = graph.entry(signal_id).or_default();
			node.subscribers.push(effect_id_a);
			node.subscribers.push(effect_id_b);
		}

		// Act
		let subs = runtime.debug_subscribers(signal_id);

		// Assert
		assert_eq!(subs, alloc::vec![effect_id_a, effect_id_b]);
	}

	#[test]
	#[serial]
	fn debug_dependencies_returns_observer_dependency_list() {
		// Arrange
		let runtime = Runtime::new();
		let observer_id = NodeId::new();
		let signal_a = NodeId::new();
		let signal_b = NodeId::new();
		{
			let mut graph = runtime.dependency_graph.borrow_mut();
			let node = graph.entry(observer_id).or_default();
			node.dependencies.push(signal_a);
			node.dependencies.push(signal_b);
		}

		// Act
		let deps = runtime.debug_dependencies(observer_id);

		// Assert
		assert_eq!(deps, alloc::vec![signal_a, signal_b]);
	}

	#[test]
	#[serial]
	fn debug_observer_stack_returns_pushed_observers_bottom_to_top() {
		// Arrange
		let runtime = Runtime::new();
		let outer_id = NodeId::new();
		let inner_id = NodeId::new();
		runtime.push_observer(Observer {
			id: outer_id,
			node_type: NodeType::Effect,
			timing: EffectTiming::default(),
			cleanup: None,
		});
		runtime.push_observer(Observer {
			id: inner_id,
			node_type: NodeType::Effect,
			timing: EffectTiming::default(),
			cleanup: None,
		});

		// Act
		let stack = runtime.debug_observer_stack();

		// Assert
		assert_eq!(stack, alloc::vec![outer_id, inner_id]);
	}

	#[test]
	#[serial]
	fn debug_pending_updates_returns_scheduled_node_ids_snapshot() {
		// Arrange
		let runtime = Runtime::new();
		let pending_a = NodeId::new();
		let pending_b = NodeId::new();
		{
			let mut p = runtime.pending_updates.borrow_mut();
			p.push(pending_a);
			p.push(pending_b);
		}

		// Act
		let snapshot = runtime.debug_pending_updates();

		// Assert
		assert_eq!(snapshot, alloc::vec![pending_a, pending_b]);
		// Snapshot must not drain the queue
		assert_eq!(runtime.pending_updates.borrow().len(), 2);
	}

	#[test]
	#[serial]
	fn run_without_observer_isolates_inner_signal_reads() {
		ReactiveScope::run(|| {
			// Arrange
			let outer = crate::reactive::signal::Signal::new(0_i32);
			let inner = crate::reactive::signal::Signal::new(0_i32);
			let counter = std::rc::Rc::new(std::cell::Cell::new(0));
			let counter_for_effect = counter.clone();
			let outer_for_effect = outer.clone();
			let inner_for_effect = inner.clone();

			// Act
			let _eff = crate::reactive::effect::Effect::new(move || {
				let _ = outer_for_effect.get();
				super::run_without_observer(|| {
					let _ = inner_for_effect.get();
				});
				counter_for_effect.set(counter_for_effect.get() + 1);
			});

			let initial = counter.get();
			inner.set(99);
			super::with_runtime(|rt| rt.flush_updates());

			// Assert
			assert_eq!(
				counter.get(),
				initial,
				"run_without_observer must isolate Signal reads from outer Observer"
			);
		});
	}

	#[test]
	#[serial]
	fn subscribe_node_to_observer_wires_edges_both_directions() {
		// Arrange
		let node = NodeId::new();
		let observer = NodeId::new();

		// Act — exercise the public free function so this test covers the
		// same code path used by `*::new_with_deps`.
		super::subscribe_node_to_observer(node, observer);

		// Assert
		let subs = super::with_runtime(|rt| rt.debug_subscribers(node));
		let deps = super::with_runtime(|rt| rt.debug_dependencies(observer));
		assert_eq!(
			subs,
			alloc::vec![observer],
			"node must have observer as subscriber"
		);
		assert_eq!(deps, alloc::vec![node], "observer must depend on node");

		// Calling twice must not duplicate entries.
		super::subscribe_node_to_observer(node, observer);
		let subs2 = super::with_runtime(|rt| rt.debug_subscribers(node));
		assert_eq!(subs2.len(), 1, "subscribe must be idempotent");
	}

	#[rstest::rstest]
	#[case::layout(EffectTiming::Layout)]
	#[case::passive(EffectTiming::Passive)]
	#[serial(reactive_runtime)]
	fn nested_batch_defers_effects_but_keeps_memo_reads_current(
		reactive_scope: ReactiveScopeFixture,
		#[case] timing: EffectTiming,
	) {
		reactive_scope.enter(|| {
			// Arrange
			let source = Signal::new(0);
			let derived = Memo::new(move || source.get() * 2);
			let observed = Rc::new(RefCell::new(Vec::new()));
			let effect_observed = Rc::clone(&observed);
			let _effect = Effect::new_with_timing(
				move || effect_observed.borrow_mut().push(derived.get()),
				timing,
			);

			// Act
			batch(|| {
				source.set(1);
				assert_eq!(derived.get(), 2);
				batch(|| source.set(2));
				assert_eq!(derived.get(), 4);
				with_runtime(|runtime| runtime.flush_updates());
				assert_eq!(*observed.borrow(), vec![0]);
			});

			// Assert
			assert_eq!(*observed.borrow(), vec![0, 4]);
			source.set(3);
			with_runtime(|runtime| runtime.flush_updates());
			assert_eq!(*observed.borrow(), vec![0, 4, 6]);
		});
	}

	#[rstest::rstest]
	#[case::layout_next_batch(EffectTiming::Layout, true)]
	#[case::passive_next_batch(EffectTiming::Passive, true)]
	#[case::layout_next_notification(EffectTiming::Layout, false)]
	#[case::passive_next_notification(EffectTiming::Passive, false)]
	#[serial(reactive_runtime)]
	fn batch_releases_effects_after_error_and_unwind(
		reactive_scope: ReactiveScopeFixture,
		#[case] timing: EffectTiming,
		#[case] recover_with_batch: bool,
	) {
		use std::panic::{AssertUnwindSafe, catch_unwind};

		reactive_scope.enter(|| {
			// Arrange
			let source = Signal::new(0);
			let unrelated = Signal::new(0);
			let observed = Rc::new(RefCell::new(Vec::new()));
			let effect_observed = Rc::clone(&observed);
			let _effect = Effect::new_with_timing(
				move || effect_observed.borrow_mut().push(source.get()),
				timing,
			);

			// Act
			let result: Result<(), &str> = batch(|| {
				source.set(1);
				assert_eq!(*observed.borrow(), vec![0]);
				Err("rejected")
			});
			assert_eq!(result, Err("rejected"));
			assert_eq!(*observed.borrow(), vec![0, 1]);
			let panic = catch_unwind(AssertUnwindSafe(|| {
				batch(|| {
					source.set(2);
					panic!("batch interrupted");
				});
			}));

			// Assert
			assert!(panic.is_err());
			assert_eq!(*observed.borrow(), vec![0, 1]);
			if recover_with_batch {
				batch(|| ());
			} else {
				unrelated.set(1);
				let expected = match timing {
					EffectTiming::Layout => vec![0, 1, 2],
					EffectTiming::Passive => vec![0, 1],
				};
				assert_eq!(*observed.borrow(), expected);
				with_runtime(|runtime| runtime.flush_updates());
			}
			assert_eq!(*observed.borrow(), vec![0, 1, 2]);
			source.set(3);
			with_runtime(|runtime| runtime.flush_updates());
			assert_eq!(*observed.borrow(), vec![0, 1, 2, 3]);
		});
	}

	#[rstest::rstest]
	#[case::layout_first(true, false)]
	#[case::passive_first(false, false)]
	#[case::passive_already_pending(false, true)]
	#[serial(reactive_runtime)]
	fn batch_flushes_layout_before_passive(
		reactive_scope: ReactiveScopeFixture,
		#[case] layout_first: bool,
		#[case] passive_already_pending: bool,
	) {
		reactive_scope.enter(|| {
			// Arrange
			let layout_source = Signal::new(0);
			let passive_source = Signal::new(0);
			let rendered = Rc::new(Cell::new(0));
			let observed = Rc::new(RefCell::new(Vec::new()));
			let passive_rendered = Rc::clone(&rendered);
			let passive_observed = Rc::clone(&observed);
			let _passive = Effect::new(move || {
				let _ = passive_source.get();
				passive_observed
					.borrow_mut()
					.push((EffectTiming::Passive, passive_rendered.get()));
			});
			let layout_observed = Rc::clone(&observed);
			let _layout = Effect::new_with_timing(
				move || {
					let value = layout_source.get();
					rendered.set(value);
					layout_observed
						.borrow_mut()
						.push((EffectTiming::Layout, value));
				},
				EffectTiming::Layout,
			);
			observed.borrow_mut().clear();
			if passive_already_pending {
				passive_source.set(1);
			}

			// Act
			batch(|| {
				if layout_first {
					layout_source.set(1);
				}
				if !passive_already_pending {
					passive_source.set(1);
				}
				if !layout_first {
					layout_source.set(1);
				}
				assert_eq!(*observed.borrow(), Vec::new());
			});

			// Assert
			assert_eq!(
				*observed.borrow(),
				vec![(EffectTiming::Layout, 1), (EffectTiming::Passive, 1)]
			);
			with_runtime(|runtime| runtime.flush_updates());
			assert_eq!(observed.borrow().len(), 2);
		});
	}

	#[rstest::rstest]
	#[case::constructor(false)]
	#[case::hook_mode(true)]
	#[serial(reactive_runtime)]
	fn batch_keeps_explicit_memo_reads_current(
		reactive_scope: ReactiveScopeFixture,
		#[case] use_mode: bool,
		#[values(EffectTiming::Layout, EffectTiming::Passive)] timing: EffectTiming,
	) {
		reactive_scope.enter(|| {
			// Arrange
			let source = Signal::new(0);
			let unlisted = Signal::new(10);
			let computations = Rc::new(Cell::new(0));
			let memo_computations = Rc::clone(&computations);
			let compute = move || {
				memo_computations.set(memo_computations.get() + 1);
				source.get() + unlisted.get()
			};
			let memo = if use_mode {
				Memo::new_with_mode(compute, crate::deps![source].into())
			} else {
				Memo::new_with_deps(compute, crate::deps![source].into_deps())
			};
			let derived = Memo::new(move || memo.get() * 2);
			let observed = Rc::new(RefCell::new(Vec::new()));
			let effect_observed = Rc::clone(&observed);
			let _effect = Effect::new_with_timing(
				move || effect_observed.borrow_mut().push(derived.get()),
				timing,
			);

			// Act
			batch(|| {
				source.set(1);
				assert_eq!(derived.get(), 22);
				unlisted.set(20);
				assert_eq!(memo.get(), 11);
				batch(|| source.set(2));
				assert_eq!(derived.get(), 44);
				assert_eq!(*observed.borrow(), vec![20]);
			});

			// Assert
			assert_eq!(*observed.borrow(), vec![20, 44]);
			source.set(3);
			with_runtime(|runtime| runtime.flush_updates());
			assert_eq!(*observed.borrow(), vec![20, 44, 46]);
			assert_eq!(computations.get(), 4);
		});
	}

	#[rstest::rstest]
	#[case::dispose_memo(true)]
	#[case::drop_owner(false)]
	#[serial(reactive_runtime)]
	fn explicit_memo_dependencies_remain_owned_after_recomputation(
		reactive_scope: ReactiveScopeFixture,
		#[case] dispose_memo: bool,
	) {
		reactive_scope.enter(|| {
			// Arrange
			let source = Signal::new(1);
			let owner = ReactiveScope::new();
			let memo = owner.enter(|| {
				Memo::new_with_deps(move || source.get() * 2, crate::deps![source].into_deps())
			});

			// Act
			for value in [2, 3] {
				source.set(value);
				assert_eq!(memo.get(), value * 2);
				assert_eq!(
					with_runtime(|runtime| runtime.subscriber_count(source.id())),
					1
				);
			}
			if dispose_memo {
				memo.dispose();
				assert_eq!(
					with_runtime(|runtime| runtime.subscriber_count(source.id())),
					0
				);
			}
			drop(owner);
			source.set(4);

			// Assert
			assert_eq!(
				with_runtime(|runtime| runtime.subscriber_count(source.id())),
				0
			);
			assert_eq!(
				with_runtime(|runtime| runtime.debug_pending_updates()),
				Vec::new()
			);
		});
	}

	#[cfg(native)]
	#[rstest::rstest]
	#[serial(reactive_runtime)]
	fn panicking_batch_preserves_body_panic_and_pending_effects(
		reactive_scope: ReactiveScopeFixture,
	) {
		use std::panic::{AssertUnwindSafe, catch_unwind};
		const CHILD_ENV: &str = "REINHARDT_BATCH_PANIC_TEST_CHILD";
		const CHILD_DONE: &str = "reinhardt batch panic recovery completed";

		// Isolate the double-panic regression so an abort cannot kill the test suite.
		if std::env::var_os(CHILD_ENV).is_none() {
			let output =
				std::process::Command::new(std::env::current_exe().expect("test executable"))
					.args([
						"--exact",
						"reactive::runtime::tests::panicking_batch_preserves_body_panic_and_pending_effects",
						"--nocapture",
					])
					.env(CHILD_ENV, "1")
					.env("RUST_BACKTRACE", "0")
					.output()
					.expect("run isolated panic test");
			assert_eq!(
				output.status.code(),
				Some(0),
				"isolated panic test failed:\n{}\n{}",
				String::from_utf8_lossy(&output.stdout),
				String::from_utf8_lossy(&output.stderr),
			);
			assert_eq!(
				String::from_utf8_lossy(&output.stdout)
					.lines()
					.filter(|line| *line == CHILD_DONE)
					.count(),
				1,
				"the child must execute the recovery assertions"
			);
			return;
		}

		reactive_scope.enter(|| {
			// Arrange
			let source = Signal::new(0);
			let secondary = Signal::new(0);
			let observed = Rc::new(RefCell::new(Vec::new()));
			let secondary_observed = Rc::new(RefCell::new(Vec::new()));
			let panic_next = Rc::new(Cell::new(false));
			let effect_panic = Rc::clone(&panic_next);
			let effect_observed = Rc::clone(&observed);
			let _panicking = Effect::new_with_timing(
				move || {
					effect_observed.borrow_mut().push(source.get());
					assert!(!effect_panic.replace(false), "layout callback panic");
				},
				EffectTiming::Layout,
			);
			let effect_secondary = Rc::clone(&secondary_observed);
			let _secondary = Effect::new_with_timing(
				move || effect_secondary.borrow_mut().push(secondary.get()),
				EffectTiming::Layout,
			);
			panic_next.set(true);

			// Act
			let panic = catch_unwind(AssertUnwindSafe(|| {
				batch(|| {
					source.set(1);
					secondary.set(1);
					panic!("batch body panic");
				});
			}));

			// Assert
			let panic = panic.expect_err("the original body panic must escape");
			assert_eq!(panic.downcast_ref::<&str>(), Some(&"batch body panic"));
			assert_eq!(*observed.borrow(), vec![0]);
			assert_eq!(*secondary_observed.borrow(), vec![0]);
			let callback_panic = catch_unwind(AssertUnwindSafe(|| batch(|| ())));
			let callback_panic =
				callback_panic.expect_err("normal flush must surface callback panic");
			assert_eq!(
				callback_panic.downcast_ref::<&str>(),
				Some(&"layout callback panic")
			);
			batch(|| ());
			assert_eq!(*observed.borrow(), vec![0, 1]);
			assert_eq!(*secondary_observed.borrow(), vec![0, 1]);
			source.set(2);
			assert_eq!(*observed.borrow(), vec![0, 1, 2]);
			with_runtime(|runtime| {
				assert_eq!(*runtime.batch_depth.borrow(), 0);
				assert_eq!(runtime.debug_pending_updates(), Vec::new());
				assert_eq!(runtime.current_observer(), None);
			});
		});
		println!("\n{CHILD_DONE}");
	}

	#[rstest::rstest]
	#[case::immediate(false)]
	#[case::batched(true)]
	#[serial(reactive_runtime)]
	fn layout_effect_write_runs_in_next_notification_epoch(
		reactive_scope: ReactiveScopeFixture,
		#[case] batched: bool,
	) {
		reactive_scope.enter(|| {
			let source = Signal::new(0_i32);
			let runs = std::rc::Rc::new(std::cell::Cell::new(0_u8));
			let observed = std::rc::Rc::new(std::cell::Cell::new(-1_i32));
			let _effect = Effect::new_with_timing(
				{
					let source = source.clone();
					let runs = std::rc::Rc::clone(&runs);
					let observed = std::rc::Rc::clone(&observed);
					move || {
						let value = source.get();
						observed.set(value);
						runs.set(runs.get() + 1);
						if value == 1 {
							source.set(2);
						}
					}
				},
				EffectTiming::Layout,
			);

			if batched {
				batch(|| source.set(1));
			} else {
				source.set(1);
			}

			assert_eq!(source.get(), 2);
			assert_eq!(observed.get(), 2);
			assert_eq!(runs.get(), 3);
		});
	}

	#[rstest::rstest]
	#[serial(reactive_runtime)]
	fn notification_panic_recovers_pending_consumer_on_next_change() {
		use std::panic::{AssertUnwindSafe, catch_unwind};

		ReactiveScope::run(|| {
			let source = Signal::new(0_i32);
			let memo = Memo::new({
				let source = source.clone();
				move || source.get() * 2
			});
			let panic_next = std::rc::Rc::new(std::cell::Cell::new(false));
			let _panicking = Effect::new_with_timing(
				{
					let memo = memo.clone();
					let panic_next = std::rc::Rc::clone(&panic_next);
					move || {
						assert!(!panic_next.replace(false), "notification consumer panic");
						let _ = memo.get();
					}
				},
				EffectTiming::Layout,
			);
			let observed = std::rc::Rc::new(std::cell::Cell::new(0_i32));
			let _observer = Effect::new_with_timing(
				{
					let memo = memo.clone();
					let observed = std::rc::Rc::clone(&observed);
					move || observed.set(memo.get())
				},
				EffectTiming::Layout,
			);
			panic_next.set(true);

			let result = catch_unwind(AssertUnwindSafe(|| source.set(1)));
			assert!(result.is_err());
			assert_eq!(observed.get(), 0);

			source.set(2);

			assert_eq!(observed.get(), 4);
		});
	}

	#[rstest::rstest]
	#[case::empty_next_batch(true)]
	#[case::unrelated_notification(false)]
	#[serial(reactive_runtime)]
	fn consumer_write_before_panic_recovers_pending_notification(
		reactive_scope: ReactiveScopeFixture,
		#[case] recover_with_batch: bool,
	) {
		use std::panic::{AssertUnwindSafe, catch_unwind};

		reactive_scope.enter(|| {
			let secondary = Signal::new(0_i32);
			let observed = std::rc::Rc::new(std::cell::Cell::new(0_i32));
			let _secondary_effect = Effect::new_with_timing(
				{
					let secondary = secondary.clone();
					let observed = std::rc::Rc::clone(&observed);
					move || observed.set(secondary.get())
				},
				EffectTiming::Layout,
			);
			let root = Signal::new(0_i32);
			let _panicking = Effect::new_with_timing(
				{
					let root = root.clone();
					let secondary = secondary.clone();
					move || {
						if root.get() == 1 {
							secondary.set(1);
							panic!("consumer panic after write");
						}
					}
				},
				EffectTiming::Layout,
			);
			let unrelated = Signal::new(0_i32);

			let result = catch_unwind(AssertUnwindSafe(|| root.set(1)));
			assert!(result.is_err());
			assert_eq!(secondary.get(), 1);
			assert_eq!(observed.get(), 0);

			if recover_with_batch {
				batch(|| ());
			} else {
				unrelated.set(1);
			}

			assert_eq!(observed.get(), 1);
		});
	}

	#[rstest::rstest]
	#[case::immediate(false)]
	#[case::batched(true)]
	#[serial(reactive_runtime)]
	fn non_converging_layout_updates_panic_and_runtime_remains_reusable(
		reactive_scope: ReactiveScopeFixture,
		#[case] batched: bool,
	) {
		use std::panic::{AssertUnwindSafe, catch_unwind};
		const EXPECTED_MAX_NOTIFICATION_EPOCHS: usize = 32;

		reactive_scope.enter(|| {
			let looping = Signal::new(0_u32);
			let loop_enabled = std::rc::Rc::new(std::cell::Cell::new(false));
			let runs = std::rc::Rc::new(std::cell::Cell::new(0_usize));
			let _looping_effect = Effect::new_with_timing(
				{
					let looping = looping.clone();
					let loop_enabled = std::rc::Rc::clone(&loop_enabled);
					let runs = std::rc::Rc::clone(&runs);
					move || {
						let value = looping.get();
						runs.set(runs.get() + 1);
						if loop_enabled.get() {
							looping.set(value + 1);
						}
					}
				},
				EffectTiming::Layout,
			);
			let unrelated = Signal::new(0_i32);
			let observed = std::rc::Rc::new(std::cell::Cell::new(0_i32));
			let _unrelated_effect = Effect::new_with_timing(
				{
					let unrelated = unrelated.clone();
					let observed = std::rc::Rc::clone(&observed);
					move || observed.set(unrelated.get())
				},
				EffectTiming::Layout,
			);
			loop_enabled.set(true);

			let result = catch_unwind(AssertUnwindSafe(|| {
				if batched {
					batch(|| looping.set(1));
				} else {
					looping.set(1);
				}
			}));
			let panic = result.expect_err("non-converging notification must panic");
			let message = panic
				.downcast_ref::<String>()
				.map(String::as_str)
				.or_else(|| panic.downcast_ref::<&str>().copied())
				.expect("notification limit panic must have a string message");
			assert_eq!(
				message,
				format!(
					"reactive notification exceeded {EXPECTED_MAX_NOTIFICATION_EPOCHS} epochs; possible non-converging layout update loop"
				)
			);
			assert_eq!(runs.get(), EXPECTED_MAX_NOTIFICATION_EPOCHS + 1);
			loop_enabled.set(false);

			unrelated.set(1);

			assert_eq!(observed.get(), 1);
		});
	}
}
