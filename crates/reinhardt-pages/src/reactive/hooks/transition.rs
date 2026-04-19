//! Transition hooks: use_transition and use_deferred_value
//!
//! These hooks provide React-like transition management for non-blocking updates.

use std::cell::RefCell;
use std::rc::Rc;

use crate::reactive::Signal;

/// Type alias for the start_transition function type.
///
/// This represents a function that takes a boxed closure and executes it
/// within a transition context.
type StartTransitionFn = Rc<RefCell<Box<dyn Fn(Box<dyn FnOnce()>)>>>;

/// State returned by use_transition.
///
/// Contains the pending state and a function to start transitions.
pub struct TransitionState {
	/// Whether a transition is currently pending.
	pub is_pending: Signal<bool>,
	/// Function to start a transition.
	start_transition: StartTransitionFn,
}

impl TransitionState {
	/// Starts a transition with the given callback.
	///
	/// Updates inside the callback are marked as transitions and won't block
	/// urgent updates like user input.
	pub fn start_transition<F>(&self, f: F)
	where
		F: FnOnce() + 'static,
	{
		(self.start_transition.borrow())(Box::new(f));
	}
}

/// Marks state updates as non-blocking transitions.
///
/// This is the React-like equivalent of `useTransition`. It allows you to mark
/// certain state updates as non-urgent, preventing them from blocking more
/// important updates like user input.
///
/// # Returns
///
/// A `TransitionState` containing:
/// - `is_pending`: A `Signal<bool>` indicating if a transition is in progress
/// - `start_transition`: A method to wrap state updates as transitions
///
/// # Example
///
/// ```ignore
/// use reinhardt_pages::reactive::hooks::{use_state, use_transition};
///
/// let (items, set_items) = use_state(vec![]);
/// let transition = use_transition();
///
/// let on_filter_change = use_callback({
///     let set_items = set_items.clone();
///     let transition = transition.clone();
///     move |filter: String| {
///         // This update won't block typing
///         transition.start_transition({
///             let set_items = set_items.clone();
///             move || {
///                 let filtered = expensive_filter(&filter);
///                 set_items(filtered);
///             }
///         });
///     }
/// });
///
/// // Show loading indicator during transition
/// if transition.is_pending.get() {
///     // ... show spinner
/// }
/// ```
///
/// # Note
///
/// On WASM, transitions run asynchronously via spawn_local. On native, they run synchronously.
pub fn use_transition() -> TransitionState {
	let is_pending = Signal::new(false);

	let start_transition: StartTransitionFn = {
		let is_pending = is_pending.clone();
		Rc::new(RefCell::new(Box::new(move |f: Box<dyn FnOnce()>| {
			is_pending.set(true);

			#[cfg(wasm)]
			{
				use crate::spawn::spawn_task;
				let is_pending = is_pending.clone();
				spawn_task(async move {
					f();
					is_pending.set(false);
				});
			}

			#[cfg(native)]
			{
				f();
				is_pending.set(false);
			}
		})))
	};

	TransitionState {
		is_pending,
		start_transition,
	}
}

/// Defers updating a value until higher priority updates complete.
///
/// This is the React-like equivalent of `useDeferredValue`. It allows you to
/// defer updating part of the UI, showing stale content while fresh content loads.
///
/// # Type Parameters
///
/// * `T` - The type of the value to defer
///
/// # Arguments
///
/// * `value` - The Signal containing the value to defer
///
/// # Returns
///
/// A `Signal<T>` containing the deferred value
///
/// # Example
///
/// ```ignore
/// use reinhardt_pages::reactive::hooks::{use_state, use_deferred_value};
///
/// let (query, set_query) = use_state("".to_string());
/// let deferred_query = use_deferred_value(query.clone());
///
/// // The search input uses `query` directly for immediate response
/// // The search results use `deferred_query` to avoid blocking input
///
/// page!(|| {
///     div {
///         input {
///             value: query.get(),
///             @input: |e| set_query(e.target_value()),
///         }
///         SearchResults(query: deferred_query.get())
///     }
/// })
/// ```
///
/// # Note
///
/// On WASM, the deferred value updates asynchronously via spawn_local. On native, it updates synchronously.
pub fn use_deferred_value<T: Clone + 'static>(value: Signal<T>) -> Signal<T> {
	let deferred = Signal::new(value.get());

	let deferred_clone = deferred.clone();
	crate::reactive::Effect::new({
		move || {
			let new_value = value.get();

			#[cfg(wasm)]
			{
				use crate::spawn::spawn_task;
				let deferred_clone = deferred_clone.clone();
				spawn_task(async move {
					deferred_clone.set(new_value);
				});
			}

			#[cfg(native)]
			{
				deferred_clone.set(new_value);
			}
		}
	});

	deferred
}

#[cfg(test)]
mod tests {
	use super::*;
	use serial_test::serial;
	use std::cell::RefCell;
	use std::rc::Rc;

	#[test]
	#[serial]
	fn test_use_transition_pending_state() {
		let transition = use_transition();

		assert!(!transition.is_pending.get());

		// Note: On native (non-WASM), transitions execute synchronously,
		// so pending state changes instantly. This is intentional behavior.
		// On WASM, transitions use spawn_local for true async execution.
		let ran = Rc::new(RefCell::new(false));
		transition.start_transition({
			let ran = Rc::clone(&ran);
			move || {
				*ran.borrow_mut() = true;
			}
		});

		assert!(*ran.borrow());
		assert!(!transition.is_pending.get()); // Back to false after sync execution
	}

	#[test]
	#[serial]
	fn test_use_deferred_value() {
		let value = Signal::new(42);
		let deferred = use_deferred_value(value.clone());

		assert_eq!(deferred.get(), 42);
	}
}
