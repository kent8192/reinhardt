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
//! use reinhardt_core::reactive::{Signal, Effect, Runtime};
//!
//! // Create a signal
//! let count = Signal::new(0);
//!
//! // Create an effect that automatically tracks dependencies
//! let count_for_effect = count.clone();
//! Effect::new(move || {
//!     // This get() call automatically registers the dependency
//!     println!("Count is: {}", count_for_effect.get());
//! });
//!
//! // Update the signal - the effect will automatically re-run
//! count.set(42);
//! ```

use core::cell::RefCell;
use core::sync::atomic::{AtomicUsize, Ordering};

extern crate alloc;
use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::vec::Vec;

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
	/// Active explicit batch and callback-flush nesting depth.
	pub(crate) batch_depth: RefCell<usize>,
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
	/// This schedules all subscribers (Effects/Memos that depend on this Signal) for re-execution.
	/// Outside explicit batches, layout effects execute synchronously and passive
	/// effects are scheduled asynchronously. Batches defer both until the outermost exit.
	///
	/// # Arguments
	///
	/// * `signal_id` - ID of the Signal that changed
	pub fn notify_signal_change(&self, signal_id: NodeId) {
		let graph = self.dependency_graph.borrow();
		if let Some(node) = graph.get(&signal_id) {
			// Collect layout effects and passive effects separately
			let mut layout_effects = Vec::new();
			let mut passive_effects = Vec::new();

			for &subscriber_id in &node.subscribers {
				// Check if this is an effect and get its timing
				if let Some(timing) = super::effect::get_effect_timing(subscriber_id) {
					match timing {
						EffectTiming::Layout => layout_effects.push(subscriber_id),
						EffectTiming::Passive => passive_effects.push(subscriber_id),
					}
				} else {
					// Non-effect subscribers (like Memos) are treated as passive
					passive_effects.push(subscriber_id);
				}
			}

			// Drop the borrow before executing effects
			drop(graph);

			// Explicit batches must not expose intermediate values to layout effects.
			let batched = *self.batch_depth.borrow() > 0;
			for effect_id in layout_effects {
				if batched {
					self.schedule_update(effect_id);
				} else {
					super::effect::Effect::execute_effect(effect_id);
				}
			}

			// Schedule passive effects asynchronously
			for effect_id in passive_effects {
				self.schedule_update(effect_id);
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
		let mut pending = self.pending_updates.borrow_mut();
		if !pending.contains(&node_id) {
			pending.push(node_id);
		}
		drop(pending);

		if *self.batch_depth.borrow() > 0 {
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
/// Updates scheduled while the batch is active are queued, then flushed once
/// the outermost batch exits. Layout effects run before passive effects, with
/// write order preserved within each timing. Nested batches and writes raised by
/// callbacks share the pending queue until the flush finishes.
pub fn batch<R>(f: impl FnOnce() -> R) -> R {
	struct BatchGuard;

	impl Drop for BatchGuard {
		fn drop(&mut self) {
			let _ = try_with_runtime(|rt| {
				let should_flush = {
					let mut depth = rt.batch_depth.borrow_mut();
					debug_assert!(*depth > 0, "reactive batch depth underflow");
					*depth -= 1;
					*depth == 0 && !rt.pending_updates.borrow().is_empty()
				};

				if should_flush {
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

	let saved = with_runtime(|rt| core::mem::take(&mut *rt.observer_stack.borrow_mut()));
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
	use serial_test::serial;

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
	#[serial]
	fn test_notify_signal_change() {
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
		}

		// Notify change
		runtime.notify_signal_change(signal_id);

		// Verify update was scheduled
		let pending = runtime.pending_updates.borrow();
		assert!(pending.contains(&effect_id));
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

	#[test]
	#[serial(reactive_batch)]
	fn batch_flush_deduplicates_layout_notifications_until_callbacks_finish() {
		use crate::reactive::{Effect, Signal};
		use std::{cell::Cell, rc::Rc};

		// Arrange: the first queued layout callback also changes the second's input.
		let source = Signal::new(0);
		let trigger = Signal::new(0);
		let forwarded = Signal::new(0);
		let producer_finished = Rc::new(Cell::new(false));
		let observations = Rc::new(RefCell::new(Vec::new()));
		let producer_source = source.clone();
		let producer_forwarded = forwarded.clone();
		let producer_state = Rc::clone(&producer_finished);
		let _producer = Effect::new_with_timing(
			move || {
				producer_forwarded.set(producer_source.get() * 10);
				producer_state.set(true);
			},
			EffectTiming::Layout,
		);
		let consumer_trigger = trigger.clone();
		let consumer_state = Rc::clone(&producer_finished);
		let consumer_observations = Rc::clone(&observations);
		let _consumer = Effect::new_with_timing(
			move || {
				consumer_observations.borrow_mut().push((
					consumer_trigger.get(),
					forwarded.get(),
					consumer_state.get(),
				));
			},
			EffectTiming::Layout,
		);
		observations.borrow_mut().clear();
		producer_finished.set(false);

		// Act: the consumer is already pending when the producer changes its input.
		batch(|| {
			source.set(2);
			trigger.set(3);
		});

		// Assert: it observes the completed producer once, without synchronous reentry.
		assert_eq!(*observations.borrow(), [(3, 20, true)]);
	}

	#[test]
	#[serial(reactive_batch)]
	fn batch_flush_drains_new_layout_work_before_pending_passive_consumers() {
		use crate::reactive::{Effect, Signal};
		use std::{cell::Cell, rc::Rc};

		// Arrange: a layout callback creates more work while passive work is pending.
		let source = Signal::new(0);
		let forwarded = Signal::new(0);
		let completed_value = Rc::new(Cell::new(0));
		let observed = Rc::new(RefCell::new(Vec::new()));
		let effect_source = source.clone();
		let effect_forwarded = forwarded.clone();
		let _producer = Effect::new_with_timing(
			move || effect_forwarded.set(effect_source.get() * 10),
			EffectTiming::Layout,
		);
		let layout_value = Rc::clone(&completed_value);
		let _layout_consumer = Effect::new_with_timing(
			move || layout_value.set(forwarded.get()),
			EffectTiming::Layout,
		);
		let passive_source = source.clone();
		let passive_observed = Rc::clone(&observed);
		let _passive_consumer = Effect::new(move || {
			let _ = passive_source.get();
			passive_observed.borrow_mut().push(completed_value.get());
		});
		observed.borrow_mut().clear();

		// Act.
		batch(|| source.set(4));

		// Assert: newly queued layout work is drained before the passive snapshot.
		assert_eq!(*observed.borrow(), [40]);
		assert_eq!(with_runtime(Runtime::debug_pending_updates), Vec::new());
	}

	#[test]
	#[serial(reactive_batch)]
	fn batches_flush_layout_effects_before_passive_effects_in_write_order() {
		use crate::reactive::{Effect, Signal};
		use std::{cell::Cell, rc::Rc};

		// Arrange: a passive consumer observes the completed layout work.
		let passive = Signal::new(0);
		let first_layout = Signal::new(0);
		let second_layout = Signal::new(0);
		let layout_total = Rc::new(Cell::new(0));
		let observed = Rc::new(RefCell::new(Vec::new()));
		let passive_signal = passive.clone();
		let passive_total = Rc::clone(&layout_total);
		let passive_observed = Rc::clone(&observed);
		let _passive_effect = Effect::new(move || {
			let _ = passive_signal.get();
			passive_observed
				.borrow_mut()
				.push(("passive", passive_total.get()));
		});
		let _layout_effects: Vec<_> = [
			("first", first_layout.clone()),
			("second", second_layout.clone()),
		]
		.into_iter()
		.map(|(name, signal)| {
			let total = Rc::clone(&layout_total);
			let observed = Rc::clone(&observed);
			Effect::new_with_timing(
				move || {
					let value = signal.get();
					total.set(total.get() + value);
					observed.borrow_mut().push((name, value));
				},
				EffectTiming::Layout,
			)
		})
		.collect();
		observed.borrow_mut().clear();

		// Act: passive work is queued first, and layout writes reverse creation order.
		batch(|| {
			passive.set(1);
			second_layout.set(2);
			first_layout.set(1);
			assert_eq!(observed.borrow().len(), 0);
		});

		// Assert: layout work keeps its write order and completes before passive work.
		assert_eq!(
			*observed.borrow(),
			[("second", 2), ("first", 1), ("passive", 3)]
		);
	}

	#[test]
	#[serial(reactive_batch)]
	fn nested_batches_defer_layout_effects_until_all_values_are_ready() {
		use crate::reactive::{Effect, Signal};
		use std::{cell::RefCell, rc::Rc};

		// Arrange
		let first = Signal::new(0);
		let second = Signal::new(0);
		let observed = Rc::new(RefCell::new(Vec::new()));
		let effect_first = first.clone();
		let effect_second = second.clone();
		let effect_observed = Rc::clone(&observed);
		let _effect = Effect::new_with_timing(
			move || {
				effect_observed
					.borrow_mut()
					.push((effect_first.get(), effect_second.get()))
			},
			EffectTiming::Layout,
		);

		// Act and assert: nested batches expose only the final snapshot at outer exit.
		batch(|| {
			first.set(1);
			with_runtime(Runtime::flush_updates);
			batch(|| {
				second.set(2);
				first.set(3);
			});
			assert_eq!(*observed.borrow(), [(0, 0)]);
		});
		assert_eq!(*observed.borrow(), [(0, 0), (3, 2)]);

		// Layout effects remain synchronous outside a batch.
		second.set(4);
		assert_eq!(*observed.borrow(), [(0, 0), (3, 2), (3, 4)]);
	}
}
