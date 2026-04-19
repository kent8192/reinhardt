//! Ref hooks: use_ref
//!
//! This hook provides a way to hold mutable values that don't trigger re-renders.

use std::cell::RefCell;
use std::rc::Rc;

/// A mutable reference container that doesn't trigger re-renders.
///
/// Unlike Signal, updating a Ref's value won't cause Effects or Memos to re-run.
/// This is useful for holding values like DOM references, timers, or any mutable
/// data that shouldn't affect the reactive graph.
///
/// ## Example
///
/// ```ignore
/// use reinhardt_pages::reactive::hooks::{use_ref, use_effect};
///
/// let render_count = use_ref(0);
///
/// use_effect(move || {
///     // Increment without triggering reactivity
///     *render_count.current_mut() += 1;
///     None::<fn()>
/// });
/// ```
pub struct Ref<T: 'static> {
	inner: Rc<RefCell<T>>,
}

impl<T: 'static> Ref<T> {
	/// Creates a new Ref with the given initial value.
	fn new(value: T) -> Self {
		Self {
			inner: Rc::new(RefCell::new(value)),
		}
	}

	/// Gets a reference to the current value.
	///
	/// # Panics
	///
	/// Panics if the value is currently mutably borrowed.
	pub fn current(&self) -> std::cell::Ref<'_, T> {
		self.inner.borrow()
	}

	/// Gets a mutable reference to the current value.
	///
	/// # Panics
	///
	/// Panics if the value is currently borrowed.
	pub fn current_mut(&self) -> std::cell::RefMut<'_, T> {
		self.inner.borrow_mut()
	}

	/// Sets the current value.
	///
	/// This does NOT trigger any reactive updates.
	pub fn set(&self, value: T) {
		*self.inner.borrow_mut() = value;
	}

	/// Updates the current value using a function.
	///
	/// This does NOT trigger any reactive updates.
	pub fn update<F>(&self, f: F)
	where
		F: FnOnce(&mut T),
	{
		f(&mut *self.inner.borrow_mut());
	}
}

impl<T: Clone + 'static> Ref<T> {
	/// Gets a clone of the current value.
	pub fn get(&self) -> T {
		self.inner.borrow().clone()
	}
}

impl<T: 'static> Clone for Ref<T> {
	fn clone(&self) -> Self {
		Self {
			inner: Rc::clone(&self.inner),
		}
	}
}

impl<T: std::fmt::Debug + 'static> std::fmt::Debug for Ref<T> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("Ref")
			.field("current", &*self.inner.borrow())
			.finish()
	}
}

/// Creates a mutable reference that persists across renders without triggering updates.
///
/// This is the React-like equivalent of `useRef`. The value is stored in a `Ref<T>`
/// which can be mutated without causing Effects or Memos to re-run.
///
/// # Common Use Cases
///
/// - Storing DOM element references
/// - Keeping track of previous values
/// - Storing mutable values that shouldn't trigger updates
/// - Holding timer IDs
///
/// # Arguments
///
/// * `initial` - The initial value for the ref
///
/// # Returns
///
/// A `Ref<T>` that can be read and mutated
///
/// # Example
///
/// ```no_run
/// use reinhardt_pages::reactive::hooks::use_ref;
///
/// // Track render count without triggering re-renders
/// let render_count = use_ref(0);
/// *render_count.current_mut() += 1;
///
/// // Store a timer ID
/// let timer_id = use_ref(None::<i32>);
/// timer_id.set(Some(123));
/// ```
pub fn use_ref<T: 'static>(initial: T) -> Ref<T> {
	Ref::new(initial)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_use_ref_basic() {
		let r = use_ref(42);
		assert_eq!(*r.current(), 42);

		r.set(100);
		assert_eq!(*r.current(), 100);
	}

	#[test]
	fn test_ref_update() {
		let r = use_ref(vec![1, 2, 3]);

		r.update(|v| v.push(4));
		assert_eq!(*r.current(), vec![1, 2, 3, 4]);

		r.update(|v| {
			v.pop();
		});
		assert_eq!(*r.current(), vec![1, 2, 3]);
	}

	#[test]
	fn test_ref_clone() {
		let r1 = use_ref(0);
		let r2 = r1.clone();

		r1.set(10);
		assert_eq!(*r2.current(), 10);

		r2.set(20);
		assert_eq!(*r1.current(), 20);
	}

	#[test]
	fn test_ref_get() {
		let r = use_ref("hello".to_string());
		let value = r.get();
		assert_eq!(value, "hello");
	}

	#[test]
	fn test_ref_with_option() {
		let r = use_ref(None::<String>);
		assert!(r.current().is_none());

		r.set(Some("value".to_string()));
		assert_eq!(r.current().as_ref(), Some(&"value".to_string()));
	}

	#[test]
	fn test_ref_debug() {
		let r = use_ref(42);
		let debug_str = format!("{:?}", r);
		assert!(debug_str.contains("Ref"));
		assert!(debug_str.contains("42"));
	}
}
