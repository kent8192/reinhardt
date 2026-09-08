//! Memo - Cached Reactive Computations
//!
//! `Memo<T>` represents a memoized computation that automatically updates when its dependencies change.
//! Unlike `Effect`, which runs for side effects, `Memo` caches and returns a computed value.
//!
//! ## Key Features
//!
//! - **Automatic Caching**: Computation result is cached and reused until dependencies change
//! - **Lazy Evaluation**: Only recomputes when `get()` is called and dependencies have changed
//! - **Automatic Dependency Tracking**: Dependencies are tracked automatically, just like Effect
//! - **Can be Depended On**: Other Effects and Memos can depend on a Memo, making it act like a Signal
//!
//! ## Example
//!
//! ```no_run
//! use reinhardt_core::reactive::{Memo, ReactiveScope, Signal};
//!
//! ReactiveScope::run(|| {
//!     let count = Signal::new(5);
//!
//!     // Create a memo that computes count * 2
//!     let count_for_memo = count;
//!     let doubled = Memo::new(move || count_for_memo.get() * 2);
//!
//!     // First access computes the value
//!     assert_eq!(doubled.get(), 10);
//!
//!     // Second access uses cached value (no recomputation)
//!     assert_eq!(doubled.get(), 10);
//!
//!     // When dependency changes, memo is marked dirty
//!     count.set(10);
//!
//!     // Next access recomputes
//!     assert_eq!(doubled.get(), 20);
//! });
//! ```

extern crate alloc;
use alloc::boxed::Box;

use super::deps::ReactiveDeps;
use super::runtime::{EffectTiming, NodeId, NodeType, Observer, try_with_runtime, with_runtime};
use super::scope::{
	NodeKey, NodeKind, allocate_node, enter_scope, find_node_key, mark_node_disposed,
	node_is_dirty, require_active_scope, set_node_dirty, with_node, with_node_mut,
};

/// Computation function for a Memo
type MemoFn<T> = Box<dyn FnMut() -> T + 'static>;

struct MemoSlot<T: Clone + 'static> {
	f: Option<MemoFn<T>>,
	value: Option<T>,
	run_scope: Option<super::scope::ReactiveScope>,
}

pub(crate) fn mark_memo_dirty_by_id(memo_id: NodeId) {
	let Some(key) = find_node_key(memo_id, NodeKind::Memo) else {
		return;
	};
	if set_node_dirty(key, true).is_ok() {
		with_runtime(|rt| rt.notify_signal_change(memo_id));
	}
}

pub(crate) fn is_memo(node_id: NodeId) -> bool {
	find_node_key(node_id, NodeKind::Memo).is_some()
}

pub(crate) fn is_memo_registered(node_id: NodeId) -> bool {
	is_memo(node_id)
}

/// A memoized reactive computation that caches its result
///
/// `Memo<T>` is similar to a Signal in that it can be read with `get()` and can have dependents.
/// However, unlike a Signal, its value is computed from other reactive values, and the
/// computation is automatically cached.
///
/// ## When to Use Memo vs Effect
///
/// - Use `Memo` when you need a **derived value** that should be cached
/// - Use `Effect` when you need to perform **side effects**
///
/// ## Example
///
/// ```rust
/// use reinhardt_core::reactive::{Effect, Memo, ReactiveScope, Signal};
///
/// ReactiveScope::run(|| {
///     let first_name = Signal::new("John".to_string());
///     let last_name = Signal::new("Doe".to_string());
///
///     // Memo caches the full name computation
///     let full_name = Memo::new(move || {
///         format!("{} {}", first_name.get(), last_name.get())
///     });
///
///     // Effect uses the memo
///     Effect::new(move || {
///         println!("Full name: {}", full_name.get());
///     });
/// });
/// ```
///
/// Memos are neither `Send` nor `Sync`, even when `T` is. Their computation
/// and cached value are owned by a thread-local reactive scope.
///
/// ```compile_fail
/// use reinhardt_core::reactive::Memo;
///
/// fn assert_send_sync<T: Send + Sync>() {}
///
/// assert_send_sync::<Memo<i32>>();
/// ```
pub struct Memo<T: Clone + 'static> {
	key: NodeKey,
	_phantom: core::marker::PhantomData<fn() -> T>,
}

impl<T: Clone + 'static> Clone for Memo<T> {
	fn clone(&self) -> Self {
		*self
	}
}

impl<T: Clone + 'static> Copy for Memo<T> {}

impl<T: Clone + 'static> Memo<T> {
	/// Create a memoized computation in the active reactive scope.
	pub fn new<F>(f: F) -> Self
	where
		F: FnMut() -> T + 'static,
	{
		Self::allocate(Box::new(f))
	}

	/// Create a memo that recomputes only when a listed dependency changes.
	#[allow(dead_code)]
	pub fn new_with_deps<F>(mut f: F, deps: super::deps::Deps) -> Self
	where
		F: FnMut() -> T + 'static,
	{
		Self::allocate(Box::new(move || {
			// Register on the Memo itself so propagation invalidates it before
			// consumers run, including reads inside an explicit batch.
			with_runtime(|rt| {
				for &dep in deps.as_slice() {
					rt.track_dependency(dep);
				}
			});
			super::runtime::run_without_observer(&mut f)
		}))
	}

	#[doc(hidden)]
	pub fn new_with_mode<F>(f: F, deps: ReactiveDeps) -> Self
	where
		F: FnMut() -> T + 'static,
	{
		match deps {
			ReactiveDeps::Explicit(deps) => Self::new_with_deps(f, deps.into_deps()),
			ReactiveDeps::Auto => Self::new(f),
		}
	}

	fn allocate(f: MemoFn<T>) -> Self {
		require_active_scope("Memo::new");
		let key = allocate_node(
			NodeKind::Memo,
			MemoSlot {
				f: Some(f),
				value: None,
				run_scope: None,
			},
		);
		let memo = Self {
			key,
			_phantom: core::marker::PhantomData,
		};
		let initial_value = Self::compute_value(key);
		with_node_mut::<MemoSlot<T>, _>(key, |slot| {
			slot.value = Some(initial_value);
		})
		.unwrap_or_else(|err| panic!("{err}"));
		memo
	}

	fn compute_value(key: NodeKey) -> T {
		let memo_id = key.node_id();
		let previous_run_scope = with_node_mut::<MemoSlot<T>, _>(key, |slot| slot.run_scope.take())
			.unwrap_or_else(|err| panic!("{err}"));
		drop(previous_run_scope);
		with_runtime(|rt| {
			rt.clear_dependencies(memo_id);
			rt.push_observer(Observer {
				id: memo_id,
				node_type: NodeType::Memo,
				timing: EffectTiming::default(),
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

		struct MemoFnGuard<T: Clone + 'static> {
			key: NodeKey,
			f: Option<MemoFn<T>>,
		}
		impl<T: Clone + 'static> Drop for MemoFnGuard<T> {
			fn drop(&mut self) {
				if let Some(f) = self.f.take() {
					let _ = with_node_mut::<MemoSlot<T>, _>(self.key, |slot| {
						if slot.f.is_none() {
							slot.f = Some(f);
						}
					});
				}
			}
		}

		let mut guard = MemoFnGuard {
			key,
			f: with_node_mut::<MemoSlot<T>, _>(key, |slot| slot.f.take())
				.unwrap_or_else(|err| panic!("{err}")),
		};
		let run_scope = super::scope::ReactiveScope::new();
		let run_scope_id = run_scope.id();
		let value = enter_scope(run_scope_id, || {
			guard
				.f
				.as_mut()
				.expect("Memo function must exist while the memo is active")()
		})
		.unwrap_or_else(|err| panic!("{err}"));
		let _ = with_node_mut::<MemoSlot<T>, _>(key, |slot| slot.run_scope = Some(run_scope));
		value
	}

	/// Return the cached value, recomputing when dependencies marked it dirty.
	pub fn get(&self) -> T {
		with_runtime(|rt| rt.track_dependency(self.id()));
		self.read_value()
	}

	/// Return the current value without tracking the caller as a dependency.
	pub fn get_untracked(&self) -> T {
		self.read_value()
	}

	fn read_value(&self) -> T {
		if node_is_dirty(self.key).unwrap_or_else(|err| panic!("{err}")) {
			let new_value = Self::compute_value(self.key);
			match with_node_mut::<MemoSlot<T>, _>(self.key, |slot| {
				slot.value = Some(new_value.clone());
			}) {
				Ok(()) => {
					set_node_dirty(self.key, false).unwrap_or_else(|err| panic!("{err}"));
				}
				Err(super::scope::ReactiveScopeError::DisposedNode { .. }) => {}
				Err(err) => panic!("{err}"),
			}
			new_value
		} else {
			with_node::<MemoSlot<T>, _>(self.key, |slot| {
				slot.value
					.as_ref()
					.expect("Memo value must exist after initialization")
					.clone()
			})
			.unwrap_or_else(|err| panic!("{err}"))
		}
	}

	/// Mark this memo dirty and notify downstream subscribers.
	pub fn mark_dirty(&self) {
		set_node_dirty(self.key, true).unwrap_or_else(|err| panic!("{err}"));
		with_runtime(|rt| rt.notify_signal_change(self.id()));
	}

	/// Return the runtime node identifier for this memo.
	pub fn id(&self) -> NodeId {
		self.key.node_id()
	}

	/// Dispose this memo and unsubscribe its dependencies.
	pub fn dispose(&self) {
		let Ok((f, value, run_scope)) = with_node_mut::<MemoSlot<T>, _>(self.key, |slot| {
			(slot.f.take(), slot.value.take(), slot.run_scope.take())
		}) else {
			return;
		};
		let _ = mark_node_disposed(self.key);
		drop(f);
		drop(value);
		drop(run_scope);
		let _ = try_with_runtime(|rt| rt.remove_node(self.id()));
	}
}

#[cfg(test)]
mod tests {
	use core::cell::RefCell;
	use std::rc::Rc;

	use super::*;
	use crate::reactive::Signal;
	use rstest::rstest;
	use serial_test::serial;

	#[rstest]
	#[serial(reactive_runtime)]
	fn memo_is_copy() {
		fn assert_copy<T: Copy>() {}

		assert_copy::<Memo<i32>>();
	}

	#[rstest]
	#[serial(reactive_runtime)]
	#[should_panic(expected = "Memo::new requires an active ReactiveScope")]
	fn memo_new_requires_scope() {
		let _ = Memo::new(|| 1_i32);
	}

	#[rstest]
	#[serial(reactive_runtime)]
	#[should_panic(expected = "disposed reactive node access")]
	fn memo_panics_after_scope_dispose() {
		let memo = crate::reactive::ReactiveScope::run(|| Memo::new(|| 1_i32));
		let _ = memo.get();
	}

	#[derive(Clone)]
	struct DropTrackedValue {
		drops: Rc<RefCell<usize>>,
	}

	impl Drop for DropTrackedValue {
		fn drop(&mut self) {
			*self.drops.borrow_mut() += 1;
		}
	}

	#[rstest]
	#[serial(reactive_runtime)]
	fn memo_dispose_drops_its_cached_value() {
		crate::reactive::ReactiveScope::run(|| {
			let drops = Rc::new(RefCell::new(0));
			let memo = Memo::new({
				let drops = Rc::clone(&drops);
				move || DropTrackedValue {
					drops: Rc::clone(&drops),
				}
			});

			memo.dispose();

			assert_eq!(*drops.borrow(), 1);
		});
	}

	#[rstest]
	#[serial(reactive_runtime)]
	fn memo_recomputes_nested_reactive_nodes_in_its_owning_scope() {
		let scope = crate::reactive::ReactiveScope::new();
		let (source, memo) = scope.enter(|| {
			let source = Signal::new(1_i32);
			let source_for_memo = source;
			let memo = Memo::new(move || {
				let nested = Signal::new(source_for_memo.get());
				nested.get()
			});
			(source, memo)
		});

		scope.enter(|| source.set(2));
		memo.mark_dirty();

		assert_eq!(memo.get(), 2);
	}

	#[test]
	#[serial(reactive_runtime)]
	fn memo_self_dispose_keeps_its_current_run_scope_alive() {
		crate::reactive::ReactiveScope::run(|| {
			let source = Signal::new(0_i32);
			let holder: Rc<RefCell<Option<Memo<i32>>>> = Rc::new(RefCell::new(None));
			let source_for_memo = source;
			let holder_for_memo = Rc::clone(&holder);
			let memo = Memo::new(move || {
				let _ = source_for_memo.get();
				if let Some(memo) = *holder_for_memo.borrow() {
					memo.dispose();
					let nested = Signal::new(1_i32);
					assert_eq!(nested.get(), 1);
				}
				42
			});
			*holder.borrow_mut() = Some(memo);

			source.set(1);
			memo.mark_dirty();

			assert_eq!(memo.get(), 42);
		});
	}

	#[test]
	#[serial(reactive_runtime)]
	fn memo_recomputation_disposes_nodes_created_by_the_previous_computation() {
		crate::reactive::ReactiveScope::run(|| {
			let source = Signal::new(0);
			let nested_runs = Rc::new(RefCell::new(0));
			let source_for_memo = source;
			let nested_runs_for_memo = Rc::clone(&nested_runs);
			let memo = Memo::new(move || {
				let source_for_nested = source_for_memo;
				let nested_runs = Rc::clone(&nested_runs_for_memo);
				let _nested = super::super::effect::Effect::new(move || {
					let _ = source_for_nested.get();
					*nested_runs.borrow_mut() += 1;
				});
				0
			});

			assert_eq!(*nested_runs.borrow(), 1);
			memo.mark_dirty();
			assert_eq!(memo.get(), 0);
			assert_eq!(*nested_runs.borrow(), 2);
			source.set(1);
			with_runtime(|rt| rt.flush_updates());
			assert_eq!(*nested_runs.borrow(), 3);
		});
	}

	#[test]
	#[serial]
	fn test_memo_creation() {
		crate::reactive::ReactiveScope::run(|| {
			let memo = Memo::new(|| 42);
			assert_eq!(memo.get(), 42);
		});
	}

	#[test]
	#[serial]
	fn test_memo_caching() {
		crate::reactive::ReactiveScope::run(|| {
			let compute_count = Rc::new(RefCell::new(0));
			let compute_count_clone = compute_count.clone();

			let memo = Memo::new(move || {
				*compute_count_clone.borrow_mut() += 1;
				42
			});

			// First access computes
			assert_eq!(memo.get(), 42);
			assert_eq!(*compute_count.borrow(), 1);

			// Second access uses cache
			assert_eq!(memo.get(), 42);
			assert_eq!(*compute_count.borrow(), 1);
		});
	}

	#[test]
	#[serial]
	fn test_memo_with_signal_dependency() {
		crate::reactive::ReactiveScope::run(|| {
			let signal = Signal::new(5);
			let signal_for_memo = signal;

			let memo = Memo::new(move || signal_for_memo.get() * 2);

			// Initial value
			assert_eq!(memo.get(), 10);

			// Change the source signal.
			signal.set(10);

			// The runtime must mark the memo dirty before its next read.
			assert_eq!(memo.get(), 20);
		});
	}

	#[test]
	#[serial]
	fn test_memo_clone() {
		crate::reactive::ReactiveScope::run(|| {
			let memo1 = Memo::new(|| 42);
			let memo2 = memo1;

			assert_eq!(memo1.get(), 42);
			assert_eq!(memo2.get(), 42);
		});
	}

	#[test]
	#[serial]
	fn test_memo_dependency_tracking() {
		crate::reactive::ReactiveScope::run(|| {
			let signal = Signal::new(1);
			let signal_for_memo = signal;

			let memo = Memo::new(move || signal_for_memo.get() + 10);

			// Access the memo inside an effect-like observer
			with_runtime(|rt| {
				let observer_id = NodeId::new();
				rt.push_observer(Observer {
					id: observer_id,
					node_type: NodeType::Effect,
					timing: EffectTiming::default(), // Test observer uses default timing
					cleanup: None,
				});

				// This should track the dependency
				let _ = memo.get();

				rt.pop_observer();

				// Verify dependency was tracked
				let graph = rt.dependency_graph.borrow();
				let memo_node = graph.get(&memo.id()).unwrap();
				assert!(memo_node.subscribers.contains(&observer_id));
			});
		});
	}

	// Note: Memo chain test removed due to Drop ordering issues with thread-local storage.
	// While chained memos are a valid pattern, the test creates Drop ordering complexities
	// with TLS. In production code, memo chains work correctly during normal execution;
	// the issue only manifests during test cleanup.

	#[rstest::rstest]
	#[serial]
	fn test_memo_creates_effect_during_computation() {
		crate::reactive::ReactiveScope::run(|| {
			// Arrange
			let effect_ran = Rc::new(RefCell::new(false));
			let effect_ran_clone = effect_ran.clone();

			// Act - create a memo whose computation creates an effect
			let memo = Memo::new(move || {
				use crate::reactive::Effect;
				let ran = effect_ran_clone.clone();
				let _effect = Effect::new(move || {
					*ran.borrow_mut() = true;
				});
				42
			});

			// Assert - memo returns the correct value and nested effect executed
			assert_eq!(memo.get(), 42);
			assert!(*effect_ran.borrow());
		});
	}

	#[test]
	#[serial]
	fn test_memo_get_untracked() {
		crate::reactive::ReactiveScope::run(|| {
			let signal = Signal::new(5);
			let signal_for_memo = signal;

			let memo = Memo::new(move || signal_for_memo.get() * 2);

			with_runtime(|rt| {
				let observer_id = NodeId::new();
				rt.push_observer(Observer {
					id: observer_id,
					node_type: NodeType::Effect,
					timing: EffectTiming::default(), // Test observer uses default timing
					cleanup: None,
				});

				// get_untracked should not create dependency
				let _ = memo.get_untracked();

				rt.pop_observer();

				// Verify NO dependency was tracked
				let graph = rt.dependency_graph.borrow();
				if let Some(memo_node) = graph.get(&memo.id()) {
					assert!(!memo_node.subscribers.contains(&observer_id));
				}
			});
		});
	}

	#[test]
	#[serial]
	fn new_with_deps_recomputes_only_on_listed_dep() {
		crate::reactive::ReactiveScope::run(|| {
			use core::cell::Cell;

			// Arrange
			let listed = Signal::new(2_i32);
			let unlisted = Signal::new(100_i32);
			let computations = Rc::new(Cell::new(0_i32));
			let computations_for_memo = computations.clone();
			let listed_for_memo = listed;
			let unlisted_for_memo = unlisted;
			let deps = crate::reactive::deps::Deps::from_signals(&[listed.id()]);

			// Act
			let memo = Memo::new_with_deps(
				move || {
					computations_for_memo.set(computations_for_memo.get() + 1);
					listed_for_memo.get() * 10 + unlisted_for_memo.get()
				},
				deps,
			);
			// Force initial value materialization without a spurious recompute.
			let _ = memo.get();
			let before_changes = computations.get();

			unlisted.set(200);
			let _ = memo.get();
			let after_unlisted = computations.get();

			listed.set(3);
			let _ = memo.get();
			let after_listed = computations.get();

			// Assert
			assert_eq!(
				after_unlisted, before_changes,
				"memo MUST NOT recompute when unlisted Signal changes (Option A)"
			);
			assert_eq!(
				after_listed,
				before_changes + 1,
				"memo MUST recompute exactly once when listed dep changes"
			);
		});
	}
}
