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
//! - **Scope-owned**: Automatically removes itself from the dependency graph when its scope is disposed
//!
//! ## Example
//!
//! ```rust
//! use reinhardt_core::reactive::{Effect, ReactiveScope, Signal};
//!
//! ReactiveScope::run(|| {
//!     let count = Signal::new(0);
//!
//!     // Create an effect that logs the count
//!     let count_for_effect = count;
//!     let _effect = Effect::new(move || {
//!         // This get() call automatically creates a dependency
//!         println!("Count is: {}", count_for_effect.get());
//!     });
//!
//!     // This will trigger the effect to re-run
//!     count.set(42); // Prints: "Count is: 42"
//! });
//! ```

use core::cell::RefCell;
use core::marker::PhantomData;

extern crate alloc;
use alloc::boxed::Box;
use alloc::rc::Rc;

use super::deps::ReactiveDeps;
use super::runtime::{EffectTiming, NodeId, NodeType, Observer, try_with_runtime, with_runtime};
use super::scope::{
	NodeKey, NodeKind, ScopeId, allocate_node, enter_scope, find_node_key, mark_node_disposed,
	require_active_scope, with_node, with_node_mut,
};

/// Type alias for effect functions
type EffectFn = Box<dyn FnMut() + 'static>;

/// Type alias for the cleanup slot shared between Effect instances and closures
type CleanupSlot = Rc<RefCell<Option<Box<dyn FnOnce()>>>>;

struct EffectSlot {
	f: Option<EffectFn>,
	timing: EffectTiming,
	cleanup_slot: CleanupSlot,
	scope: ScopeId,
	run_scope: Option<super::scope::ReactiveScope>,
}

/// Get the timing for an effect by its ID.
///
/// Returns `None` if the effect doesn't exist or is not an Effect node.
pub(crate) fn get_effect_timing(effect_id: NodeId) -> Option<EffectTiming> {
	let key = find_node_key(effect_id, NodeKind::Effect)?;
	with_node::<EffectSlot, _>(key, |slot| slot.timing).ok()
}

/// A reactive effect that automatically re-runs when its dependencies change
///
/// Effects are the bridge between the reactive system and the outside world (DOM, console, etc.).
/// They run immediately when created, and automatically re-run whenever any Signal they access changes.
///
/// ## Cleanup
///
/// Effects can optionally provide a cleanup function that runs before the effect is re-executed or its owning scope is disposed.
/// This is useful for cleaning up event listeners, timers, etc.
///
/// ## Example
///
/// ```no_run
/// use reinhardt_core::reactive::{Effect, ReactiveScope, Signal};
///
/// ReactiveScope::run(|| {
///     let count = Signal::new(0);
///     let doubled = Signal::new(0);
///
///     // Effect that keeps doubled in sync with count
///     let count_for_effect = count;
///     let doubled_for_effect = doubled;
///     Effect::new(move || {
///         doubled_for_effect.set(count_for_effect.get() * 2);
///     });
///
///     // After async update, doubled would be 10
///     count.set(5);
/// });
/// ```
///
/// Effect handles are bound to the thread that owns their reactive scope.
///
/// ```compile_fail
/// use reinhardt_core::reactive::{Effect, ReactiveScope};
///
/// let effect = ReactiveScope::run(|| Effect::new(|| {}));
/// std::thread::spawn(move || effect.dispose());
/// ```
pub struct Effect {
	key: NodeKey,
	_thread_bound: PhantomData<Rc<()>>,
}

impl Clone for Effect {
	fn clone(&self) -> Self {
		*self
	}
}

impl Copy for Effect {}

impl Drop for EffectSlot {
	fn drop(&mut self) {
		if let Some(cleanup) = self.cleanup_slot.borrow_mut().take() {
			super::runtime::run_without_observer(|| {
				let _ = enter_scope(self.scope, cleanup);
			});
		}
	}
}

impl Effect {
	/// Create an effect in the active reactive scope.
	pub fn new<F>(f: F) -> Self
	where
		F: FnMut() + 'static,
	{
		Self::new_with_timing(f, EffectTiming::Passive)
	}

	/// Create an effect with explicit execution timing.
	pub fn new_with_timing<F>(f: F, timing: EffectTiming) -> Self
	where
		F: FnMut() + 'static,
	{
		let scope = require_active_scope("Effect::new");
		let key = allocate_node(
			NodeKind::Effect,
			EffectSlot {
				f: Some(Box::new(f)),
				timing,
				cleanup_slot: Rc::new(RefCell::new(None)),
				scope,
				run_scope: None,
			},
		);
		let effect = Self {
			key,
			_thread_bound: PhantomData,
		};
		Self::execute_effect(effect.id());
		effect
	}

	/// Create an effect subscribed only to the listed dependencies.
	#[allow(dead_code)]
	pub fn new_with_deps<F, C>(f: F, deps: super::deps::Deps) -> Self
	where
		F: FnMut() -> Option<C> + 'static,
		C: FnOnce() + 'static,
	{
		Self::new_with_deps_internal(f, Some(deps), EffectTiming::Passive)
	}

	#[doc(hidden)]
	pub fn new_with_mode<F, C>(f: F, deps: ReactiveDeps) -> Self
	where
		F: FnMut() -> Option<C> + 'static,
		C: FnOnce() + 'static,
	{
		Self::new_with_mode_and_timing(f, deps, EffectTiming::Passive)
	}

	#[doc(hidden)]
	pub fn new_with_mode_and_timing<F, C>(f: F, deps: ReactiveDeps, timing: EffectTiming) -> Self
	where
		F: FnMut() -> Option<C> + 'static,
		C: FnOnce() + 'static,
	{
		let deps = match deps {
			ReactiveDeps::Explicit(deps) => Some(deps.into_deps()),
			ReactiveDeps::Auto => None,
		};
		Self::new_with_deps_internal(f, deps, timing)
	}

	fn new_with_deps_internal<F, C>(
		mut f: F,
		deps: Option<super::deps::Deps>,
		timing: EffectTiming,
	) -> Self
	where
		F: FnMut() -> Option<C> + 'static,
		C: FnOnce() + 'static,
	{
		let scope = require_active_scope("Effect::new");
		let cleanup_slot: CleanupSlot = Rc::new(RefCell::new(None));
		let key = allocate_node(
			NodeKind::Effect,
			EffectSlot {
				f: None,
				timing,
				cleanup_slot: Rc::clone(&cleanup_slot),
				scope,
				run_scope: None,
			},
		);
		let effect_id = key.node_id();
		let deps = deps.map(super::deps::Deps::into_inner);
		let cleanup_for_closure = Rc::clone(&cleanup_slot);
		let wrapped = move || {
			let previous_cleanup = { cleanup_for_closure.borrow_mut().take() };
			if let Some(cleanup) = previous_cleanup {
				cleanup();
			}
			let next = if deps.is_some() {
				super::runtime::run_without_observer(&mut f)
			} else {
				f()
			};
			if let Some(cleanup) = next {
				*cleanup_for_closure.borrow_mut() = Some(Box::new(cleanup));
			}
			if find_node_key(effect_id, NodeKind::Effect).is_some()
				&& let Some(deps) = &deps
			{
				for &dep in deps {
					super::runtime::subscribe_node_to_observer(dep, effect_id);
				}
			}
		};
		with_node_mut::<EffectSlot, _>(key, |slot| {
			slot.f = Some(Box::new(wrapped));
		})
		.unwrap_or_else(|err| panic!("{err}"));
		let effect = Self {
			key,
			_thread_bound: PhantomData,
		};
		Self::execute_effect(effect_id);
		effect
	}

	/// Create an explicitly-timed effect subscribed to listed dependencies.
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
		Self::new_with_deps_internal(f, Some(deps), timing)
	}

	pub(crate) fn execute_effect(effect_id: NodeId) {
		let Some(key) = find_node_key(effect_id, NodeKind::Effect) else {
			return;
		};
		let effect_fn = with_node_mut::<EffectSlot, _>(key, |slot| slot.f.take())
			.unwrap_or_else(|err| panic!("{err}"));
		let Some(effect_fn) = effect_fn else {
			return;
		};
		let (previous_run_scope, previous_cleanup) = with_node_mut::<EffectSlot, _>(key, |slot| {
			(slot.run_scope.take(), slot.cleanup_slot.borrow_mut().take())
		})
		.unwrap_or_else(|err| panic!("{err}"));

		struct EffectFnGuard {
			key: NodeKey,
			f: Option<EffectFn>,
		}
		impl Drop for EffectFnGuard {
			fn drop(&mut self) {
				if let Some(f) = self.f.take()
					&& find_node_key(self.key.node_id(), NodeKind::Effect).is_some()
				{
					let _ = with_node_mut::<EffectSlot, _>(self.key, |slot| {
						if slot.f.is_none() {
							slot.f = Some(f);
						}
					});
				}
			}
		}
		let mut guard = EffectFnGuard {
			key,
			f: Some(effect_fn),
		};
		if let Some(cleanup) = previous_cleanup {
			super::runtime::run_without_observer(|| {
				let _ = enter_scope(key.scope(), cleanup);
			});
		}
		drop(previous_run_scope);
		with_runtime(|rt| {
			rt.clear_dependencies(effect_id);
			rt.push_observer(Observer {
				id: effect_id,
				node_type: NodeType::Effect,
				timing: get_effect_timing(effect_id).unwrap_or_default(),
				cleanup: None,
			});
		});

		struct ObserverGuard;
		impl Drop for ObserverGuard {
			fn drop(&mut self) {
				let _ = try_with_runtime(|rt| rt.pop_observer());
			}
		}
		let _observer_guard = ObserverGuard;

		let run_scope = super::scope::ReactiveScope::new();
		let run_scope_id = run_scope.id();
		if let Some(f) = guard.f.as_mut() {
			enter_scope(run_scope_id, f).unwrap_or_else(|err| panic!("{err}"));
		}
		let _ = with_node_mut::<EffectSlot, _>(key, |slot| slot.run_scope = Some(run_scope));
	}

	/// Return the runtime node identifier for this effect.
	pub fn id(&self) -> NodeId {
		self.key.node_id()
	}

	/// Dispose this effect and run its latest cleanup.
	pub fn dispose(&self) {
		let Ok((f, cleanup, run_scope)) = with_node_mut::<EffectSlot, _>(self.key, |slot| {
			(
				slot.f.take(),
				slot.cleanup_slot.borrow_mut().take(),
				slot.run_scope.take(),
			)
		}) else {
			return;
		};
		let _ = mark_node_disposed(self.key);
		drop(f);
		if let Some(cleanup) = cleanup {
			super::runtime::run_without_observer(|| {
				let _ = enter_scope(self.key.scope(), cleanup);
			});
		}
		drop(run_scope);
		let _ = try_with_runtime(|rt| rt.remove_node(self.id()));
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::reactive::Signal;
	use rstest::rstest;
	use serial_test::serial;
	use std::cell::Cell;

	#[rstest]
	#[serial(reactive_runtime)]
	fn effect_is_copy() {
		fn assert_copy<T: Copy>() {}

		assert_copy::<Effect>();
	}

	#[rstest]
	#[serial(reactive_runtime)]
	#[should_panic(expected = "Effect::new requires an active ReactiveScope")]
	fn effect_new_requires_scope() {
		let _ = Effect::new(|| {});
	}

	#[rstest]
	#[serial(reactive_runtime)]
	fn scope_drop_runs_effect_cleanup() {
		let cleaned = Rc::new(RefCell::new(false));
		let cleaned_for_effect = Rc::clone(&cleaned);

		crate::reactive::ReactiveScope::run(|| {
			let _effect = Effect::new_with_deps(
				move || {
					let cleaned_for_cleanup = Rc::clone(&cleaned_for_effect);
					Some(move || {
						*cleaned_for_cleanup.borrow_mut() = true;
					})
				},
				crate::reactive::Deps::from_signals(&[]),
			);
		});

		assert!(*cleaned.borrow(), "scope disposal must run effect cleanup");
	}

	#[rstest]
	#[serial(reactive_runtime)]
	fn scope_drop_runs_effect_cleanup_inside_the_owner_scope() {
		let scope = crate::reactive::ReactiveScope::new();
		let cleaned = Rc::new(Cell::new(false));
		let cleaned_for_effect = Rc::clone(&cleaned);

		scope.enter(|| {
			let _effect = Effect::new_with_deps(
				move || {
					let cleaned_for_cleanup = Rc::clone(&cleaned_for_effect);
					Some(move || {
						let signal = Signal::new(42_i32);
						assert_eq!(signal.get(), 42);
						cleaned_for_cleanup.set(true);
					})
				},
				crate::reactive::Deps::from_signals(&[]),
			);
		});

		scope.dispose();

		assert!(cleaned.get(), "cleanup must run inside its owner scope");
	}

	#[test]
	#[serial]
	fn test_effect_runs_immediately() {
		crate::reactive::ReactiveScope::run(|| {
			let run_count = Rc::new(RefCell::new(0));
			let run_count_clone = run_count.clone();

			let _effect = Effect::new(move || {
				*run_count_clone.borrow_mut() += 1;
			});

			assert_eq!(*run_count.borrow(), 1);
		});
	}

	#[test]
	#[serial]
	fn test_effect_tracks_dependency() {
		crate::reactive::ReactiveScope::run(|| {
			let signal = Signal::new(0);
			let run_count = Rc::new(RefCell::new(0));
			let run_count_clone = run_count.clone();

			let signal_for_effect = signal;
			let _effect = Effect::new(move || {
				let _ = signal_for_effect.get();
				*run_count_clone.borrow_mut() += 1;
			});

			assert_eq!(*run_count.borrow(), 1);

			with_runtime(|rt| {
				let graph = rt.dependency_graph.borrow();
				let signal_node = graph.get(&signal.id()).unwrap();
				assert_eq!(signal_node.subscribers.len(), 1);
			});
		});
	}

	#[test]
	#[serial]
	fn test_effect_reruns_on_signal_change() {
		crate::reactive::ReactiveScope::run(|| {
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
		});
	}

	#[test]
	#[serial(reactive_runtime)]
	fn effect_rerun_disposes_nodes_created_by_its_previous_run() {
		crate::reactive::ReactiveScope::run(|| {
			let trigger = Signal::new(0);
			let source = Signal::new(0);
			let nested_runs = Rc::new(Cell::new(0));
			let trigger_for_effect = trigger;
			let source_for_effect = source;
			let nested_runs_for_effect = Rc::clone(&nested_runs);
			let _effect = Effect::new(move || {
				let _ = trigger_for_effect.get();
				let source_for_nested = source_for_effect;
				let nested_runs = Rc::clone(&nested_runs_for_effect);
				let _nested = Effect::new(move || {
					let _ = source_for_nested.get();
					nested_runs.set(nested_runs.get() + 1);
				});
			});

			assert_eq!(nested_runs.get(), 1);
			trigger.set(1);
			with_runtime(|rt| rt.flush_updates());
			assert_eq!(nested_runs.get(), 2);
			source.set(1);
			with_runtime(|rt| rt.flush_updates());
			assert_eq!(nested_runs.get(), 3);
		});
	}

	#[test]
	#[serial]
	fn test_effect_with_multiple_signals() {
		crate::reactive::ReactiveScope::run(|| {
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
		});
	}

	#[test]
	#[serial]
	fn test_effect_dispose() {
		crate::reactive::ReactiveScope::run(|| {
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
			assert_eq!(*run_count.borrow(), 1);
		});
	}

	#[test]
	#[serial]
	fn copied_effect_handle_drop_does_not_dispose_scope_owned_effect() {
		crate::reactive::ReactiveScope::run(|| {
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
			}

			signal.set(10);
			with_runtime(|rt| rt.flush_updates());
			assert_eq!(*run_count.borrow(), 2);
		});
	}

	// Nested effect creation is supported by temporarily taking the closure from
	// its scope slot before execution and reinserting it through an RAII guard.

	#[rstest::rstest]
	#[serial]
	fn test_nested_effect_creation() {
		crate::reactive::ReactiveScope::run(|| {
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
		});
	}

	#[rstest::rstest]
	#[serial]
	fn test_effect_creates_signal_and_effect() {
		crate::reactive::ReactiveScope::run(|| {
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
		});
	}

	#[rstest::rstest]
	#[serial]
	fn test_effect_dispose_during_execution() {
		crate::reactive::ReactiveScope::run(|| {
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
		});
	}

	#[rstest::rstest]
	#[serial]
	fn self_disposing_effect_keeps_its_current_run_scope_alive() {
		crate::reactive::ReactiveScope::run(|| {
			let source = Signal::new(0_i32);
			let holder: Rc<RefCell<Option<Effect>>> = Rc::new(RefCell::new(None));
			let source_for_effect = source;
			let holder_for_effect = Rc::clone(&holder);
			let effect = Effect::new(move || {
				let _ = source_for_effect.get();
				if let Some(effect) = *holder_for_effect.borrow() {
					effect.dispose();
					let nested = Signal::new(1_i32);
					assert_eq!(nested.get(), 1);
				}
			});
			*holder.borrow_mut() = Some(effect);

			source.set(1);
			with_runtime(|rt| rt.flush_updates());
		});
	}

	#[rstest::rstest]
	#[serial]
	fn scope_disposal_cleanup_does_not_track_the_current_observer() {
		crate::reactive::ReactiveScope::run(|| {
			let cleanup_signal = Signal::new(0_i32);
			let child_scope = Rc::new(crate::reactive::ReactiveScope::new());
			child_scope.enter(|| {
				let cleanup_signal = cleanup_signal;
				let _ = Effect::new_with_deps(
					move || {
						Some(move || {
							let _ = cleanup_signal.get();
						})
					},
					crate::reactive::Deps::from_signals(&[]),
				);
			});

			let runs = Rc::new(RefCell::new(0_i32));
			let runs_for_effect = Rc::clone(&runs);
			let child_scope_for_effect = Rc::clone(&child_scope);
			let _effect = Effect::new(move || {
				*runs_for_effect.borrow_mut() += 1;
				child_scope_for_effect.dispose();
			});

			cleanup_signal.set(1);
			with_runtime(|rt| rt.flush_updates());

			assert_eq!(*runs.borrow(), 1);
		});
	}

	/// Verify that `flush_updates()` actually executes pending passive effects.
	///
	/// This is a regression test for the bug where `flush_updates()` dropped
	/// pending updates without executing them (Fixes #3348).
	#[test]
	#[serial]
	fn test_flush_updates_executes_pending_effects() {
		crate::reactive::ReactiveScope::run(|| {
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
		});
	}

	#[test]
	#[serial(reactive_runtime)]
	fn scheduled_effect_reenters_its_owning_scope() {
		let scope = crate::reactive::ReactiveScope::new();
		let trigger = scope.enter(|| Signal::new(false));
		let nested_value = Rc::new(Cell::new(0_i32));
		let nested_value_for_effect = Rc::clone(&nested_value);
		let trigger_for_effect = trigger;

		let _effect = scope.enter(|| {
			Effect::new(move || {
				if trigger_for_effect.get() {
					let nested = Signal::new(42_i32);
					nested_value_for_effect.set(nested.get());
				}
			})
		});

		trigger.set(true);
		with_runtime(|runtime| runtime.flush_updates());

		assert_eq!(nested_value.get(), 42);
	}

	#[test]
	#[serial]
	fn new_with_deps_listed_dep_triggers_rerun() {
		crate::reactive::ReactiveScope::run(|| {
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
		});
	}

	#[test]
	#[serial]
	fn new_with_deps_unlisted_signal_no_rerun() {
		crate::reactive::ReactiveScope::run(|| {
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
		});
	}

	#[test]
	#[serial]
	fn new_with_deps_cleanup_runs_before_rerun() {
		crate::reactive::ReactiveScope::run(|| {
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
		});
	}

	#[test]
	#[serial(reactive_runtime)]
	fn new_with_deps_cleanup_can_dispose_same_effect_without_reentrant_borrow() {
		crate::reactive::ReactiveScope::run(|| {
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

			let _effect_to_release = effect_holder.borrow_mut().take();
		});
	}

	#[test]
	#[serial(reactive_runtime)]
	fn dispose_can_drop_nested_effect_function_without_reentrant_storage_borrow() {
		crate::reactive::ReactiveScope::run(|| {
			let inner = Effect::new(|| {});
			let outer = Effect::new(move || {
				let _ = inner.id();
			});

			outer.dispose();
		});
	}

	#[test]
	#[serial(reactive_runtime)]
	fn copied_effect_remains_live_until_scope_disposal() {
		crate::reactive::ReactiveScope::run(|| {
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

			drop(guard);
			s.set(3);
			with_runtime(|rt| rt.flush_updates());
			assert_eq!(*runs.borrow(), 4, "scope ownership keeps the effect live");
		});
	}
}
