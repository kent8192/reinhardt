//! Signal - Fine-grained Reactive Primitive
//!
//! `Signal<T>` is the core reactive primitive that holds a value and automatically
//! tracks dependencies when accessed.
//!
//! ## Key Features
//!
//! - **Automatic Dependency Tracking**: When `get()` is called inside an Effect or Memo,
//!   the dependency is automatically recorded.
//! - **Change Notification**: When `set()` or `update()` is called, all dependent Effects
//!   are automatically scheduled for re-execution.
//! - **Lightweight**: `Signal<T>` is just a NodeId wrapper, making it cheap to clone and pass around.
//! - **Type-safe**: The value type is enforced at compile time.
//!
//! ## Example
//!
//! ```rust
//! use reinhardt_core::reactive::Signal;
//!
//! // Create a signal
//! let count = Signal::new(0);
//!
//! // Read the value
//! assert_eq!(count.get(), 0);
//!
//! // Update the value
//! count.set(42);
//! assert_eq!(count.get(), 42);
//!
//! // Update with a function
//! count.update(|n| *n += 1);
//! assert_eq!(count.get(), 43);
//! ```

use core::cell::RefCell;
use core::fmt;

extern crate alloc;
use alloc::rc::Rc;

use super::runtime::{NodeId, try_with_runtime, with_runtime};

/// A reactive signal that holds a value and tracks dependencies
///
/// `Signal<T>` is the fundamental building block of the reactive system. It represents
/// a piece of state that can change over time, and automatically notifies dependent
/// computations when it changes.
///
/// ## Type Parameter
///
/// * `T` - The type of value stored in the signal. Must be `'static` to ensure memory safety.
///
/// ## Cloning
///
/// `Signal<T>` implements `Clone` and shares the value via `Rc<RefCell<T>>`.
/// All clones of the same Signal share the same underlying value and reference count.
#[derive(Clone)]
pub struct Signal<T: 'static> {
	/// Unique identifier for this signal
	id: NodeId,
	/// The actual value, shared via reference counting
	value: Rc<RefCell<T>>,
}

impl<T: 'static> Signal<T> {
	/// Create a new Signal with the given initial value
	///
	/// # Arguments
	///
	/// * `value` - Initial value for the signal
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_core::reactive::Signal;
	///
	/// let count = Signal::new(0);
	/// assert_eq!(count.get(), 0);
	/// ```
	pub fn new(value: T) -> Self {
		Self {
			id: NodeId::new(),
			value: Rc::new(RefCell::new(value)),
		}
	}

	/// Get the current value of the signal
	///
	/// This automatically tracks the dependency if called from within an Effect or Memo.
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_core::reactive::Signal;
	///
	/// let count = Signal::new(42);
	/// assert_eq!(count.get(), 42);
	/// ```
	pub fn get(&self) -> T
	where
		T: Clone,
	{
		// Track dependency with the runtime
		with_runtime(|rt| rt.track_dependency(self.id));

		// Get the value from storage
		self.get_untracked()
	}

	/// Get the current value without tracking dependencies
	///
	/// This is useful when you want to read a signal's value without creating
	/// a dependency relationship.
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_core::reactive::Signal;
	///
	/// let count = Signal::new(42);
	/// // This won't create a dependency
	/// let value = count.get_untracked();
	/// ```
	pub fn get_untracked(&self) -> T
	where
		T: Clone,
	{
		self.value.borrow().clone()
	}

	/// Set the signal to a new value
	///
	/// This notifies all dependent Effects and Memos that the signal has changed.
	///
	/// # Arguments
	///
	/// * `value` - New value for the signal
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_core::reactive::Signal;
	///
	/// let count = Signal::new(0);
	/// count.set(42);
	/// assert_eq!(count.get(), 42);
	/// ```
	pub fn set(&self, value: T) {
		*self.value.borrow_mut() = value;
		with_runtime(|rt| rt.notify_signal_change(self.id));
	}

	/// Update the signal's value using a function
	///
	/// This is more efficient than `get()` + `set()` because it only notifies
	/// dependents once.
	///
	/// # Arguments
	///
	/// * `f` - Function that takes a mutable reference to the current value
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_core::reactive::Signal;
	///
	/// let count = Signal::new(0);
	/// count.update(|n| *n += 1);
	/// assert_eq!(count.get(), 1);
	/// ```
	pub fn update<F>(&self, f: F)
	where
		F: FnOnce(&mut T),
	{
		f(&mut *self.value.borrow_mut());
		with_runtime(|rt| rt.notify_signal_change(self.id));
	}

	/// Get the NodeId of this signal
	///
	/// This is mainly for internal use by the runtime and tests.
	pub fn id(&self) -> NodeId {
		self.id
	}
}

impl<T: 'static> Drop for Signal<T> {
	fn drop(&mut self) {
		// Only cleanup Runtime when this is the last Signal clone
		// (Rc::strong_count() == 1 means we're the only remaining reference)
		if Rc::strong_count(&self.value) == 1 {
			let _ = try_with_runtime(|rt| rt.remove_node(self.id));
		}
		// The Rc<RefCell<T>> will be automatically deallocated when refcount reaches 0
	}
}

impl<T: fmt::Debug + Clone + 'static> fmt::Debug for Signal<T> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("Signal")
			.field("id", &self.id)
			.field("value", &self.get_untracked())
			.finish()
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::reactive::runtime::NodeType;
	use serial_test::serial;

	#[test]
	#[serial]
	fn test_signal_creation() {
		let signal = Signal::new(42);
		assert_eq!(signal.get_untracked(), 42);
	}

	#[test]
	#[serial]
	fn test_signal_set() {
		let signal = Signal::new(0);
		assert_eq!(signal.get_untracked(), 0);

		signal.set(100);
		assert_eq!(signal.get_untracked(), 100);
	}

	#[test]
	#[serial]
	fn test_signal_update() {
		let signal = Signal::new(0);

		signal.update(|n| *n += 1);
		assert_eq!(signal.get_untracked(), 1);

		signal.update(|n| *n *= 2);
		assert_eq!(signal.get_untracked(), 2);
	}

	#[test]
	#[serial]
	fn test_signal_clone() {
		let signal1 = Signal::new(42);
		let signal2 = signal1.clone();

		assert_eq!(signal1.get_untracked(), 42);
		assert_eq!(signal2.get_untracked(), 42);

		signal1.set(100);
		assert_eq!(signal1.get_untracked(), 100);
		assert_eq!(signal2.get_untracked(), 100);
	}

	#[test]
	#[serial]
	fn test_multiple_signals() {
		let signal1 = Signal::new(10);
		let signal2 = Signal::new(20);
		let signal3 = Signal::new("hello");

		assert_eq!(signal1.get_untracked(), 10);
		assert_eq!(signal2.get_untracked(), 20);
		assert_eq!(signal3.get_untracked(), "hello");

		signal1.set(30);
		signal2.set(40);
		signal3.set("world");

		assert_eq!(signal1.get_untracked(), 30);
		assert_eq!(signal2.get_untracked(), 40);
		assert_eq!(signal3.get_untracked(), "world");
	}

	#[test]
	#[serial]
	fn test_signal_dependency_tracking() {
		let signal = Signal::new(42);

		// Without observer, get() should still work
		assert_eq!(signal.get(), 42);

		// With observer, dependency should be tracked
		with_runtime(|rt| {
			let observer_id = NodeId::new();
			rt.push_observer(crate::reactive::runtime::Observer {
				id: observer_id,
				node_type: NodeType::Effect,
				timing: crate::reactive::runtime::EffectTiming::default(),
				cleanup: None,
			});

			// This should track the dependency
			let _ = signal.get();

			rt.pop_observer();

			// Verify dependency was tracked
			let graph = rt.dependency_graph.borrow();
			let signal_node = graph.get(&signal.id()).unwrap();
			assert!(signal_node.subscribers.contains(&observer_id));
		});
	}

	#[test]
	#[serial]
	fn test_signal_change_notification() {
		let signal = Signal::new(0);

		with_runtime(|rt| {
			let effect_id = NodeId::new();

			// Manually set up dependency
			{
				let mut graph = rt.dependency_graph.borrow_mut();
				graph
					.entry(signal.id())
					.or_default()
					.subscribers
					.push(effect_id);
			}

			// Change the signal
			signal.set(42);

			// Verify the effect was scheduled for update
			let pending = rt.pending_updates.borrow();
			assert!(pending.contains(&effect_id));
		});
	}
}
