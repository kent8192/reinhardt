//! Effect - Reactive Side Effects
//!
//! `Effect` represents a side effect that automatically re-runs when its dependencies change.
//! Dependencies are tracked automatically - any Signal accessed inside the effect closure
//! becomes a dependency.
//!
//! ## Key Features
//!
//! - **Automatic Dependency Tracking**: Signal::get() calls inside the effect are automatically tracked
//! - **Automatic Re-execution**: When a dependent Signal changes, the effect is scheduled for re-run
//! - **Cleanup Support**: Optional cleanup function that runs before re-execution
//! - **Memory Safe**: Automatically removes itself from the dependency graph when dropped
//!
//! ## Example
//!
//! ```rust
//! use reinhardt_core::reactive::{Signal, Effect};
//!
//! let count = Signal::new(0);
//!
//! // Create an effect that logs the count
//! let count_for_effect = count.clone();
//! let _effect = Effect::new(move || {
//!     // This get() call automatically creates a dependency
//!     println!("Count is: {}", count_for_effect.get());
//! });
//!
//! // This will trigger the effect to re-run
//! count.set(42); // Prints: "Count is: 42"
//! ```

use core::cell::RefCell;

extern crate alloc;
use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::rc::Rc;

use super::runtime::{EffectTiming, NodeId, NodeType, Observer, try_with_runtime, with_runtime};

/// Type alias for effect functions
type EffectFn = Box<dyn FnMut() + 'static>;

/// Type alias for stored effect slots.
type EffectSlot = Option<EffectFn>;

/// Type alias for the cleanup slot shared between Effect instances and closures
type CleanupSlot = Rc<RefCell<Option<Box<dyn FnOnce()>>>>;

// Global storage for Effect functions
//
// This stores the closures for all Effects so they can be re-executed when dependencies change.
thread_local! {
	static EFFECT_FUNCTIONS: RefCell<BTreeMap<NodeId, EffectSlot>> = RefCell::new(BTreeMap::new());
}

// Global storage for Effect timing information
//
// This stores the execution timing (Layout vs Passive) for each Effect.
thread_local! {
	static EFFECT_TIMING: RefCell<BTreeMap<NodeId, EffectTiming>> = const { RefCell::new(BTreeMap::new()) };
}

/// Get the timing for an effect by its ID.
///
/// Returns `None` if the effect doesn't exist or is not an Effect node.
pub(crate) fn get_effect_timing(effect_id: NodeId) -> Option<EffectTiming> {
	EFFECT_TIMING.with(|storage| storage.borrow().get(&effect_id).copied())
}

/// A reactive effect that automatically re-runs when its dependencies change
///
/// Effects are the bridge between the reactive system and the outside world (DOM, console, etc.).
/// They run immediately when created, and automatically re-run whenever any Signal they access changes.
///
/// ## Cleanup
///
/// Effects can optionally provide a cleanup function that runs before the effect is re-executed or dropped.
/// This is useful for cleaning up event listeners, timers, etc.
///
/// ## Example
///
/// ```no_run
/// use reinhardt_core::reactive::{Signal, Effect};
///
/// let count = Signal::new(0);
/// let doubled = Signal::new(0);
///
/// // Effect that keeps doubled in sync with count
/// let count_clone = count.clone();
/// let doubled_clone = doubled.clone();
/// Effect::new(move || {
///     doubled_clone.set(count_clone.get() * 2);
/// });
///
/// // After async update, doubled would be 10
/// count.set(5);
/// ```
pub struct Effect {
	/// Unique identifier for this effect
	id: NodeId,
	/// Whether this effect has been disposed
	disposed: Rc<RefCell<bool>>,
	/// Pending cleanup function from the most recent run (Option A hooks only).
	///
	/// Populated by `new_with_deps` when the user's closure returns `Some(c)`;
	/// flushed before each re-run and on dispose. `Effect::new` and
	/// `Effect::new_with_timing` leave this slot empty.
	cleanup_slot: CleanupSlot,
}

impl Effect {
	/// Create a new Effect that runs the given function
	///
	/// The function runs immediately, and will automatically re-run whenever any
	/// Signal it accesses changes.
	///
	/// # Arguments
	///
	/// * `f` - The effect function. Must be `FnMut() + 'static`.
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_core::reactive::{Signal, Effect};
	///
	/// let count = Signal::new(0);
	///
	/// let count_clone = count.clone();
	/// Effect::new(move || {
	///     println!("Count: {}", count_clone.get());
	/// });
	/// ```
	pub fn new<F>(mut f: F) -> Self
	where
		F: FnMut() + 'static,
	{
		let id = NodeId::new();
		let disposed = Rc::new(RefCell::new(false));

		// Store the effect function
		let disposed_clone = disposed.clone();
		EFFECT_FUNCTIONS.with(|storage| {
			storage.borrow_mut().insert(
				id,
				Some(Box::new(move || {
					if !*disposed_clone.borrow() {
						f();
					}
				})),
			);
		});

		// Store the timing information (default: Passive)
		EFFECT_TIMING.with(|storage| {
			storage.borrow_mut().insert(id, EffectTiming::Passive);
		});

		// Run the effect for the first time
		Self::execute_effect(id);

		Self {
			id,
			disposed,
			cleanup_slot: Rc::new(RefCell::new(None)),
		}
	}

	/// Create a new Effect with specified execution timing
	///
	/// This is the low-level constructor that allows specifying whether the effect
	/// should run synchronously (Layout) or asynchronously (Passive).
	///
	/// # Arguments
	///
	/// * `f` - The effect function. Must be `FnMut() + 'static`.
	/// * `timing` - The execution timing (Layout or Passive).
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_core::reactive::{Signal, Effect, EffectTiming};
	///
	/// let count = Signal::new(0);
	///
	/// let count_clone = count.clone();
	/// Effect::new_with_timing(move || {
	///     println!("Count: {}", count_clone.get());
	/// }, EffectTiming::Layout);
	/// ```
	pub fn new_with_timing<F>(mut f: F, timing: EffectTiming) -> Self
	where
		F: FnMut() + 'static,
	{
		let id = NodeId::new();
		let disposed = Rc::new(RefCell::new(false));

		// Store the effect function
		let disposed_clone = disposed.clone();
		EFFECT_FUNCTIONS.with(|storage| {
			storage.borrow_mut().insert(
				id,
				Some(Box::new(move || {
					if !*disposed_clone.borrow() {
						f();
					}
				})),
			);
		});

		// Store the timing information
		EFFECT_TIMING.with(|storage| {
			storage.borrow_mut().insert(id, timing);
		});

		// Run the effect for the first time
		Self::execute_effect(id);

		Self {
			id,
			disposed,
			cleanup_slot: Rc::new(RefCell::new(None)),
		}
	}

	/// Create a new Effect that subscribes only to the listed `deps` (Option A).
	///
	/// Unlike `Effect::new`, the closure runs with **no active reactive
	/// Observer**, so `Signal::get` calls inside the closure do not auto-
	/// subscribe. Only the `NodeId`s carried by `deps` register as
	/// dependencies; changes to other Signals do not trigger re-execution.
	///
	/// The closure may return `Some(cleanup)` to register a one-shot
	/// cleanup function that is invoked before the next re-run and on
	/// `dispose`, matching React's `useEffect(() => () => cleanup())`.
	///
	/// This is the lower-layer primitive used by `use_effect(f, deps)` in
	/// `reinhardt-pages`. See the design spec at
	/// `docs/superpowers/specs/2026-05-22-issue-4195-hooks-deps-array-design.md`.
	#[allow(dead_code)]
	pub fn new_with_deps<F, C>(f: F, deps: super::deps::Deps) -> Self
	where
		F: FnMut() -> Option<C> + 'static,
		C: FnOnce() + 'static,
	{
		Self::new_with_deps_internal(f, deps, EffectTiming::Passive)
	}

	/// Internal helper: constructs an Effect with deps and a specified timing,
	/// inserting the timing entry *before* the initial execution so the first
	/// run respects the requested timing.
	fn new_with_deps_internal<F, C>(mut f: F, deps: super::deps::Deps, timing: EffectTiming) -> Self
	where
		F: FnMut() -> Option<C> + 'static,
		C: FnOnce() + 'static,
	{
		let id = NodeId::new();
		let disposed = Rc::new(RefCell::new(false));
		let cleanup_slot: CleanupSlot = Rc::new(RefCell::new(None));

		// Capture deps so the wrapped closure can re-subscribe after every
		// run: `execute_effect` calls `clear_dependencies(id)` before each
		// invocation, which would otherwise erase our explicit subscriptions.
		let deps_for_closure = deps.into_inner();
		let cleanup_for_closure = cleanup_slot.clone();
		let disposed_for_closure = disposed.clone();

		let wrapped = move || {
			if *disposed_for_closure.borrow() {
				return;
			}
			// Flush any cleanup from the previous run before re-executing.
			let previous_cleanup = { cleanup_for_closure.borrow_mut().take() };
			if let Some(prev) = previous_cleanup {
				prev();
			}
			// Run the user's closure with the Observer stack detached so
			// in-closure Signal reads do not auto-subscribe.
			let next = super::runtime::run_without_observer(&mut f);
			if let Some(c) = next {
				*cleanup_for_closure.borrow_mut() = Some(Box::new(c));
			}
			// Re-subscribe the listed deps (cleared by `clear_dependencies`).
			for &dep in &deps_for_closure {
				super::runtime::subscribe_node_to_observer(dep, id);
			}
		};

		EFFECT_FUNCTIONS.with(|storage| {
			storage.borrow_mut().insert(id, Some(Box::new(wrapped)));
		});

		// Insert timing *before* initial execution so the first
		// `execute_effect` dispatches with the requested timing.
		EFFECT_TIMING.with(|storage| {
			storage.borrow_mut().insert(id, timing);
		});

		// Initial run.
		Self::execute_effect(id);

		Self {
			id,
			disposed,
			cleanup_slot,
		}
	}

	/// Create a new Effect with deps and an explicit `EffectTiming`.
	///
	/// Combines the deps-driven semantics of [`Self::new_with_deps`] with
	/// the synchronous-vs-asynchronous control of [`Self::new_with_timing`].
	/// Used by `use_layout_effect(f, deps)`.
	#[allow(dead_code)]
	pub fn new_with_deps_and_timing<F, C>(
		f: F,
		deps: super::deps::Deps,
		timing: EffectTiming,
	) -> Self
	where
		F: FnMut() -> Option<C> + 'static,
		C: FnOnce() + 'static,
	{
		Self::new_with_deps_internal(f, deps, timing)
	}

	/// Execute an effect by its ID
	///
	/// This is called internally by the runtime when an effect needs to re-run.
	pub(crate) fn execute_effect(effect_id: NodeId) {
		// Get the timing for this effect (default to Passive if not found)
		let timing = EFFECT_TIMING.with(|storage| {
			storage
				.borrow()
				.get(&effect_id)
				.copied()
				.unwrap_or(EffectTiming::Passive)
		});

		with_runtime(|rt| {
			// Clear old dependencies before re-running
			rt.clear_dependencies(effect_id);

			// Push observer onto stack with timing information
			rt.push_observer(Observer {
				id: effect_id,
				node_type: NodeType::Effect,
				timing,
				cleanup: None,
			});
		});

		// Execute the effect function using Remove-Execute-Reinsert pattern
		// to avoid RefCell reentrant borrow panics when the closure creates nested effects.
		// An RAII guard ensures the closure is reinserted even if the user closure panics,
		// and skips reinsertion if the effect was disposed during execution.
		struct EffectFnGuard {
			effect_id: NodeId,
			effect_fn: Option<EffectFn>,
		}

		impl Drop for EffectFnGuard {
			fn drop(&mut self) {
				// Only reinsert if the effect is still registered (not disposed during execution)
				let still_alive =
					EFFECT_TIMING.with(|storage| storage.borrow().contains_key(&self.effect_id));
				if still_alive && let Some(f) = self.effect_fn.take() {
					EFFECT_FUNCTIONS.with(|storage| {
						storage.borrow_mut().insert(self.effect_id, Some(f));
					});
				}
			}
		}

		let mut guard = EffectFnGuard {
			effect_id,
			effect_fn: EFFECT_FUNCTIONS.with(|storage| {
				storage
					.borrow_mut()
					.get_mut(&effect_id)
					.and_then(Option::take)
			}),
		};
		if let Some(ref mut f) = guard.effect_fn {
			f();
		}

		// Pop observer from stack
		with_runtime(|rt| {
			rt.pop_observer();
		});
	}

	/// Get the NodeId of this effect (for testing)
	pub fn id(&self) -> NodeId {
		self.id
	}

	/// Dispose this effect
	///
	/// After calling this, the effect will no longer run and its resources will be cleaned up.
	pub fn dispose(&self) {
		*self.disposed.borrow_mut() = true;

		// Flush any pending cleanup from new_with_deps before tearing down
		// runtime state, matching React's "cleanup runs on unmount" semantics.
		let cleanup = { self.cleanup_slot.borrow_mut().take() };
		if let Some(c) = cleanup {
			c();
		}

		// Remove from runtime's dependency graph (ignore if TLS is destroyed)
		let _ = try_with_runtime(|rt| rt.remove_node(self.id));

		// Remove from storage (ignore if TLS is destroyed)
		let mut effect_fn = None;
		let _ = EFFECT_FUNCTIONS.try_with(|storage| {
			let mut functions = storage.borrow_mut();
			effect_fn = functions.get_mut(&self.id).and_then(Option::take);
			functions.remove(&self.id);
		});
		drop(effect_fn);

		// Remove timing entry so the RAII guard in execute_effect() knows not to reinsert
		let _ = EFFECT_TIMING.try_with(|storage| {
			storage.borrow_mut().remove(&self.id);
		});
	}
}

impl Drop for Effect {
	fn drop(&mut self) {
		self.dispose();
	}
}

// Update Runtime to support effect execution
//
// This extends the Runtime with the ability to execute effects when they're scheduled.
impl super::runtime::Runtime {
	/// Execute a scheduled effect
	///
	/// This is called internally when flushing pending updates.
	fn execute_scheduled_effect(&self, effect_id: NodeId) {
		Effect::execute_effect(effect_id);
	}

	/// Flush all pending updates
	///
	/// This executes all Effects that have been scheduled for update.
	/// Layout effects run before passive effects, preserving queue order within each timing.
	/// Writes raised by callbacks remain batched and share the pending queue until it is empty.
	/// Skips effects that were disposed between scheduling and execution.
	pub fn flush_updates(&self) {
		if *self.batch_depth.borrow() > 0 {
			return;
		}

		struct FlushGuard<'a>(&'a super::runtime::Runtime);

		impl Drop for FlushGuard<'_> {
			fn drop(&mut self) {
				*self.0.batch_depth.borrow_mut() -= 1;
				*self.0.update_scheduled.borrow_mut() = false;
			}
		}

		*self.batch_depth.borrow_mut() += 1;
		let _guard = FlushGuard(self);

		loop {
			let node_id = {
				let mut pending = self.pending_updates.borrow_mut();
				if pending.is_empty() {
					break;
				}
				// Keep not-yet-run IDs queued so callback writes deduplicate against them.
				// New layout work also takes priority over remaining passive consumers.
				let index = pending
					.iter()
					.position(|&id| get_effect_timing(id) == Some(EffectTiming::Layout))
					.unwrap_or(0);
				pending.remove(index)
			};
			let still_registered =
				EFFECT_TIMING.with(|storage| storage.borrow().contains_key(&node_id));
			if still_registered {
				self.execute_scheduled_effect(node_id);
			}
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::reactive::Signal;
	use serial_test::serial;

	#[test]
	#[serial]
	fn test_effect_runs_immediately() {
		let run_count = Rc::new(RefCell::new(0));
		let run_count_clone = run_count.clone();

		let _effect = Effect::new(move || {
			*run_count_clone.borrow_mut() += 1;
		});

		assert_eq!(*run_count.borrow(), 1);
	}

	#[test]
	#[serial]
	fn test_effect_tracks_dependency() {
		let signal = Signal::new(0);
		let run_count = Rc::new(RefCell::new(0));
		let run_count_clone = run_count.clone();

		let signal_clone = signal.clone();
		let _effect = Effect::new(move || {
			let _ = signal_clone.get(); // Track dependency
			*run_count_clone.borrow_mut() += 1;
		});

		// Effect should have run once
		assert_eq!(*run_count.borrow(), 1);

		// Verify dependency was tracked
		with_runtime(|rt| {
			let graph = rt.dependency_graph.borrow();
			let signal_node = graph.get(&signal.id()).unwrap();
			assert_eq!(signal_node.subscribers.len(), 1);
		});
	}

	#[test]
	#[serial]
	fn test_effect_reruns_on_signal_change() {
		let signal = Signal::new(0);
		let values = Rc::new(RefCell::new(alloc::vec::Vec::new()));
		let values_clone = values.clone();

		let signal_clone = signal.clone();
		let _effect = Effect::new(move || {
			values_clone.borrow_mut().push(signal_clone.get());
		});

		// Initial run
		assert_eq!(*values.borrow(), alloc::vec![0]);

		// Change signal and flush updates
		signal.set(10);
		with_runtime(|rt| rt.flush_updates());
		assert_eq!(*values.borrow(), alloc::vec![0, 10]);

		// Change again
		signal.set(20);
		with_runtime(|rt| rt.flush_updates());
		assert_eq!(*values.borrow(), alloc::vec![0, 10, 20]);
	}

	#[test]
	#[serial]
	fn test_effect_with_multiple_signals() {
		let signal1 = Signal::new(1);
		let signal2 = Signal::new(2);
		let sum = Rc::new(RefCell::new(0));
		let sum_clone = sum.clone();

		let s1 = signal1.clone();
		let s2 = signal2.clone();
		let _effect = Effect::new(move || {
			*sum_clone.borrow_mut() = s1.get() + s2.get();
		});

		// Initial run
		assert_eq!(*sum.borrow(), 3);

		// Change first signal
		signal1.set(10);
		with_runtime(|rt| rt.flush_updates());
		assert_eq!(*sum.borrow(), 12);

		// Change second signal
		signal2.set(20);
		with_runtime(|rt| rt.flush_updates());
		assert_eq!(*sum.borrow(), 30);
	}

	#[test]
	#[serial]
	fn test_effect_dispose() {
		let signal = Signal::new(0);
		let run_count = Rc::new(RefCell::new(0));
		let run_count_clone = run_count.clone();

		let signal_clone = signal.clone();
		let effect = Effect::new(move || {
			let _ = signal_clone.get();
			*run_count_clone.borrow_mut() += 1;
		});

		assert_eq!(*run_count.borrow(), 1);

		// Dispose the effect
		effect.dispose();

		// Signal change should not trigger the effect
		signal.set(10);
		with_runtime(|rt| rt.flush_updates());
		assert_eq!(*run_count.borrow(), 1); // Still 1, not 2
	}

	#[test]
	#[serial]
	fn test_effect_drop_cleans_up() {
		let signal = Signal::new(0);
		let run_count = Rc::new(RefCell::new(0));
		let run_count_clone = run_count.clone();

		{
			let signal_clone = signal.clone();
			let _effect = Effect::new(move || {
				let _ = signal_clone.get();
				*run_count_clone.borrow_mut() += 1;
			});

			assert_eq!(*run_count.borrow(), 1);
		} // Effect dropped here

		// Signal change should not trigger the dropped effect
		signal.set(10);
		with_runtime(|rt| rt.flush_updates());
		assert_eq!(*run_count.borrow(), 1); // Still 1
	}

	// Nested effect creation is supported via the Remove-Execute-Reinsert pattern in
	// execute_effect(). The closure is temporarily removed from EFFECT_FUNCTIONS storage
	// before execution, preventing RefCell reentrant borrow panics when nested Effects
	// are created inside the closure.

	#[rstest::rstest]
	#[serial]
	fn test_nested_effect_creation() {
		// Arrange
		let outer_ran = Rc::new(RefCell::new(false));
		let inner_ran = Rc::new(RefCell::new(false));
		let outer_ran_clone = outer_ran.clone();
		let inner_ran_clone = inner_ran.clone();

		// Act - create an effect whose closure creates another effect
		let _outer = Effect::new(move || {
			*outer_ran_clone.borrow_mut() = true;
			let inner_ran_inner = inner_ran_clone.clone();
			let _inner = Effect::new(move || {
				*inner_ran_inner.borrow_mut() = true;
			});
		});

		// Assert - both effects should have executed without panic
		assert!(*outer_ran.borrow());
		assert!(*inner_ran.borrow());
	}

	#[rstest::rstest]
	#[serial]
	fn test_effect_creates_signal_and_effect() {
		// Arrange
		let outer_ran = Rc::new(RefCell::new(false));
		let inner_value = Rc::new(RefCell::new(0));
		let outer_ran_clone = outer_ran.clone();
		let inner_value_clone = inner_value.clone();

		// Act - create an effect whose closure creates a signal and another effect
		let _outer = Effect::new(move || {
			*outer_ran_clone.borrow_mut() = true;
			let new_signal = Signal::new(42);
			let signal_for_inner = new_signal.clone();
			let value_capture = inner_value_clone.clone();
			let _inner = Effect::new(move || {
				*value_capture.borrow_mut() = signal_for_inner.get();
			});
		});

		// Assert - outer ran and inner captured the signal value
		assert!(*outer_ran.borrow());
		assert_eq!(*inner_value.borrow(), 42);
	}

	#[rstest::rstest]
	#[serial]
	fn test_effect_dispose_during_execution() {
		// Arrange
		let signal = Signal::new(0);
		let run_count = Rc::new(RefCell::new(0));
		let run_count_clone = run_count.clone();
		let effect_holder: Rc<RefCell<Option<Effect>>> = Rc::new(RefCell::new(None));
		let holder_clone = effect_holder.clone();
		let signal_clone = signal.clone();

		// Act - create an effect that reads a signal and disposes itself on re-execution
		let effect = Effect::new(move || {
			let _val = signal_clone.get(); // Track signal dependency
			*run_count_clone.borrow_mut() += 1;
			// On re-execution, the holder has the effect so dispose is called
			if let Some(e) = holder_clone.borrow().as_ref() {
				e.dispose();
			}
		});

		// Store the effect so it can be disposed during next execution
		*effect_holder.borrow_mut() = Some(effect);

		// Assert - effect ran once during creation
		assert_eq!(*run_count.borrow(), 1);

		// Act - trigger re-execution via signal change; effect disposes itself
		signal.set(1);
		with_runtime(|rt| rt.flush_updates());

		// Assert - effect ran a second time (during which it disposed itself)
		assert_eq!(*run_count.borrow(), 2);

		// Act - trigger another change; disposed effect should NOT run
		signal.set(2);
		with_runtime(|rt| rt.flush_updates());

		// Assert - still 2, effect did not run again
		assert_eq!(*run_count.borrow(), 2);
	}

	/// Verify that `flush_updates()` actually executes pending passive effects.
	///
	/// This is a regression test for the bug where `flush_updates()` dropped
	/// pending updates without executing them (Fixes #3348).
	#[test]
	#[serial]
	fn test_flush_updates_executes_pending_effects() {
		use crate::reactive::runtime::set_scheduler;
		use std::sync::{Arc, Mutex};

		// Collect tasks scheduled via set_scheduler
		type ScheduledTasks = Arc<Mutex<Vec<Box<dyn FnOnce() + Send>>>>;
		let scheduled_tasks: ScheduledTasks = Arc::new(Mutex::new(Vec::new()));
		let tasks_clone = scheduled_tasks.clone();

		// Install a scheduler that captures tasks instead of executing them
		// Note: OnceLock means this only works once per process, but serial
		// test ordering ensures no conflict
		set_scheduler(move |task| {
			tasks_clone.lock().unwrap().push(task);
		});

		let signal = Signal::new(0);
		let values = Rc::new(RefCell::new(alloc::vec::Vec::new()));
		let values_clone = values.clone();

		let signal_clone = signal.clone();
		// Default timing is Passive, so signal changes go through scheduler
		let _effect = Effect::new(move || {
			values_clone.borrow_mut().push(signal_clone.get());
		});

		// Effect ran once immediately during creation
		assert_eq!(*values.borrow(), alloc::vec![0]);

		// Change signal — passive effect should be scheduled, not executed immediately
		signal.set(42);

		// The scheduler captured the flush task
		let tasks = std::mem::take(&mut *scheduled_tasks.lock().unwrap());
		assert!(!tasks.is_empty(), "scheduler should have captured a task");

		// Execute the captured tasks (simulating what spawn_local would do)
		for task in tasks {
			task();
		}

		// Effect should have re-executed with the new value
		assert_eq!(*values.borrow(), alloc::vec![0, 42]);
	}

	#[test]
	#[serial]
	fn new_with_deps_listed_dep_triggers_rerun() {
		// Arrange
		let s = Signal::new(0_i32);
		let runs = Rc::new(RefCell::new(0_i32));
		let runs_for_effect = runs.clone();
		let s_for_effect = s.clone();
		let deps = crate::reactive::deps::Deps::from_signals(&[s.id()]);

		// Act — explicit cleanup type since closure returns None.
		let _eff = Effect::new_with_deps::<_, fn()>(
			move || {
				let _ = s_for_effect.get();
				*runs_for_effect.borrow_mut() += 1;
				None
			},
			deps,
		);
		let initial = *runs.borrow();
		s.set(1);
		with_runtime(|rt| rt.flush_updates());

		// Assert
		assert_eq!(
			*runs.borrow(),
			initial + 1,
			"listed dep change must trigger re-run"
		);
	}

	#[test]
	#[serial]
	fn new_with_deps_unlisted_signal_no_rerun() {
		// Arrange
		let listed = Signal::new(0_i32);
		let unlisted = Signal::new(0_i32);
		let runs = Rc::new(RefCell::new(0_i32));
		let runs_for_effect = runs.clone();
		let unlisted_for_effect = unlisted.clone();
		let deps = crate::reactive::deps::Deps::from_signals(&[listed.id()]);

		// Act — reading `unlisted` MUST NOT subscribe under Option A.
		let _eff = Effect::new_with_deps::<_, fn()>(
			move || {
				let _ = unlisted_for_effect.get();
				*runs_for_effect.borrow_mut() += 1;
				None
			},
			deps,
		);
		let initial = *runs.borrow();
		unlisted.set(99);
		with_runtime(|rt| rt.flush_updates());

		// Assert
		assert_eq!(
			*runs.borrow(),
			initial,
			"unlisted Signal read must not subscribe (Option A core)"
		);
	}

	#[test]
	#[serial]
	fn new_with_deps_cleanup_runs_before_rerun() {
		// Arrange
		let s = Signal::new(0_i32);
		let log: Rc<RefCell<alloc::vec::Vec<&'static str>>> =
			Rc::new(RefCell::new(alloc::vec::Vec::new()));
		let log_for_effect = log.clone();
		let s_for_effect = s.clone();
		let deps = crate::reactive::deps::Deps::from_signals(&[s.id()]);

		// Act
		let _eff = Effect::new_with_deps(
			move || {
				let _ = s_for_effect.get();
				log_for_effect.borrow_mut().push("run");
				let log_inner = log_for_effect.clone();
				Some(move || log_inner.borrow_mut().push("cleanup"))
			},
			deps,
		);
		s.set(1);
		with_runtime(|rt| rt.flush_updates());

		// Assert — sequence should be: run, cleanup, run.
		let recorded = log.borrow().clone();
		assert_eq!(recorded, alloc::vec!["run", "cleanup", "run"]);
	}

	#[test]
	#[serial(reactive_runtime)]
	fn new_with_deps_cleanup_can_dispose_same_effect_without_reentrant_borrow() {
		let s = Signal::new(0_i32);
		let runs = Rc::new(RefCell::new(0_i32));
		let effect_holder: Rc<RefCell<Option<Effect>>> = Rc::new(RefCell::new(None));
		let runs_for_effect = runs.clone();
		let holder_for_effect = effect_holder.clone();
		let s_for_effect = s.clone();
		let deps = crate::reactive::deps::Deps::from_signals(&[s.id()]);

		let effect = Effect::new_with_deps(
			move || {
				let _ = s_for_effect.get();
				*runs_for_effect.borrow_mut() += 1;
				let holder_for_cleanup = holder_for_effect.clone();
				Some(move || {
					if let Some(effect) = holder_for_cleanup.borrow().as_ref() {
						effect.dispose();
					}
				})
			},
			deps,
		);
		*effect_holder.borrow_mut() = Some(effect);

		s.set(1);
		with_runtime(|rt| rt.flush_updates());
		assert_eq!(
			*runs.borrow(),
			2,
			"Refs #5104: cleanup-triggered self-dispose must not panic before the rerun body"
		);

		s.set(2);
		with_runtime(|rt| rt.flush_updates());
		assert_eq!(
			*runs.borrow(),
			2,
			"effect disposed during cleanup must not be reinserted"
		);

		let effect_to_drop = { effect_holder.borrow_mut().take() };
		drop(effect_to_drop);
	}

	#[test]
	#[serial(reactive_runtime)]
	fn dispose_can_drop_nested_effect_function_without_reentrant_storage_borrow() {
		let inner = Effect::new(|| {});
		let outer = Effect::new(move || {
			let _ = inner.id();
		});

		outer.dispose();
	}

	#[test]
	#[serial(reactive_runtime)]
	fn new_with_deps_effect_in_rc_survives_until_rc_dropped() {
		// Regression for the resource dependency-tracking lifetime invariant.
		// `use_resource` stores its `new_with_deps` Effect inside an `Rc<Effect>`
		// (`Resource::effect_guard`) so dependency-change refetch keeps firing for
		// the Resource's lifetime. An Effect disposes on drop, so a handle that is
		// created and immediately dropped would stop tracking right after its
		// first run.
		let s = Signal::new(0_i32);
		let runs = Rc::new(RefCell::new(0_i32));
		let runs_for_effect = runs.clone();
		let s_for_effect = s.clone();
		let deps = crate::reactive::deps::Deps::from_signals(&[s.id()]);

		let effect = Effect::new_with_deps::<_, fn()>(
			move || {
				let _ = s_for_effect.get();
				*runs_for_effect.borrow_mut() += 1;
				None
			},
			deps,
		);
		// Mirror `Resource::effect_guard`: move the Effect into an Rc keep-alive anchor.
		let guard = Rc::new(effect);
		assert_eq!(*runs.borrow(), 1, "effect runs once on creation");

		// While the Rc is held, listed-dep changes keep re-running the effect.
		s.set(1);
		with_runtime(|rt| rt.flush_updates());
		assert_eq!(
			*runs.borrow(),
			2,
			"effect held via Rc must re-run on dependency change"
		);

		s.set(2);
		with_runtime(|rt| rt.flush_updates());
		assert_eq!(*runs.borrow(), 3, "effect held via Rc must keep re-running");

		// Dropping the only Rc disposes the Effect; further dep changes do nothing.
		drop(guard);
		s.set(3);
		with_runtime(|rt| rt.flush_updates());
		assert_eq!(
			*runs.borrow(),
			3,
			"disposed effect must not re-run after its Rc anchor is dropped"
		);
	}
}
