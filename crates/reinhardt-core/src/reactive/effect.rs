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

// Global storage for Effect functions
//
// This stores the closures for all Effects so they can be re-executed when dependencies change.
thread_local! {
	static EFFECT_FUNCTIONS: RefCell<BTreeMap<NodeId, EffectFn>> = RefCell::new(BTreeMap::new());
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
				Box::new(move || {
					if !*disposed_clone.borrow() {
						f();
					}
				}),
			);
		});

		// Store the timing information (default: Passive)
		EFFECT_TIMING.with(|storage| {
			storage.borrow_mut().insert(id, EffectTiming::Passive);
		});

		// Run the effect for the first time
		Self::execute_effect(id);

		Self { id, disposed }
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
				Box::new(move || {
					if !*disposed_clone.borrow() {
						f();
					}
				}),
			);
		});

		// Store the timing information
		EFFECT_TIMING.with(|storage| {
			storage.borrow_mut().insert(id, timing);
		});

		// Run the effect for the first time
		Self::execute_effect(id);

		Self { id, disposed }
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
						storage.borrow_mut().insert(self.effect_id, f);
					});
				}
			}
		}

		let mut guard = EffectFnGuard {
			effect_id,
			effect_fn: EFFECT_FUNCTIONS.with(|storage| storage.borrow_mut().remove(&effect_id)),
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

		// Remove from runtime's dependency graph (ignore if TLS is destroyed)
		let _ = try_with_runtime(|rt| rt.remove_node(self.id));

		// Remove from storage (ignore if TLS is destroyed)
		let _ = EFFECT_FUNCTIONS.try_with(|storage| {
			storage.borrow_mut().remove(&self.id);
		});

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
	/// Skips effects that were disposed between scheduling and execution.
	pub fn flush_updates(&self) {
		*self.update_scheduled.borrow_mut() = false;

		// Take all pending updates
		let pending = core::mem::take(&mut *self.pending_updates.borrow_mut());

		// Execute each pending effect (skip disposed ones)
		for node_id in pending {
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
}
