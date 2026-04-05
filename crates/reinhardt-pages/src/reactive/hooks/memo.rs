//! Memoization hooks: use_memo and use_callback
//!
//! These hooks provide React-like memoization built on top of Memo and Callback.

use crate::callback::Callback;
use crate::reactive::Memo;

#[cfg(wasm)]
type EventArg = web_sys::Event;

#[cfg(native)]
type EventArg = crate::component::DummyEvent;

/// Memoizes an expensive calculation.
///
/// This is the React-like equivalent of `useMemo`. The calculation is re-run
/// only when its reactive dependencies change.
///
/// Unlike React's useMemo, dependencies are automatically tracked - you don't
/// need to specify a dependency array. Any Signal accessed inside the calculation
/// will be tracked as a dependency.
///
/// # Type Parameters
///
/// * `T` - The return type of the calculation
/// * `F` - The calculation function type
///
/// # Arguments
///
/// * `f` - A function that performs the calculation
///
/// # Returns
///
/// A `Memo<T>` that can be read with `.get()`
///
/// # Example
///
/// ```no_run
/// use reinhardt_pages::reactive::hooks::{use_state, use_memo};
///
/// let (items, set_items) = use_state(vec![1, 2, 3, 4, 5]);
/// let (filter, set_filter) = use_state(2);
///
/// // Expensive filtering operation - only re-runs when items or filter change
/// let filtered = use_memo({
///     let items = items.clone();
///     let filter = filter.clone();
///     move || {
///         items.get()
///             .into_iter()
///             .filter(|&x| x > filter.get())
///             .collect::<Vec<_>>()
///     }
/// });
///
/// // Reading the memoized value
/// let result = filtered.get();
/// ```
pub fn use_memo<T, F>(f: F) -> Memo<T>
where
	T: Clone + 'static,
	F: FnMut() -> T + 'static,
{
	Memo::new(f)
}

/// Memoizes a callback function to maintain a stable reference.
///
/// This is the React-like equivalent of `useCallback`. It wraps a function
/// in a `Callback` that maintains a stable identity, preventing unnecessary
/// re-renders of child components.
///
/// # Type Parameters
///
/// * `F` - The callback function type
///
/// # Arguments
///
/// * `f` - The callback function to memoize
///
/// # Returns
///
/// A `Callback<EventArg, ()>` that can be passed to event handlers
///
/// # Example
///
/// ```ignore
/// use reinhardt_pages::reactive::hooks::{use_state, use_callback};
/// use reinhardt_pages::page;
///
/// let (count, set_count) = use_state(0);
///
/// // Memoized callback - reference won't change between renders
/// let increment = use_callback({
///     let count = count.clone();
///     let set_count = set_count.clone();
///     move |_event| {
///         set_count(count.get() + 1);
///     }
/// });
///
/// page!(|| {
///     button {
///         @click: increment,
///         "Increment"
///     }
/// })
/// ```
///
/// # Note
///
/// Unlike React's useCallback, this doesn't require a dependency array.
/// The callback captures values at creation time. To use the latest values,
/// capture Signals (which are cheap to clone) rather than their values.
#[cfg(wasm)]
pub fn use_callback<F>(f: F) -> Callback<EventArg, ()>
where
	F: Fn(EventArg) + 'static,
{
	Callback::new(f)
}

/// Memoizes a callback function to maintain a stable reference (server-side version).
///
/// See the WASM version for full documentation.
/// Requires `Send + Sync` bounds for thread-safe server-side usage.
#[cfg(native)]
pub fn use_callback<F>(f: F) -> Callback<EventArg, ()>
where
	F: Fn(EventArg) + Send + Sync + 'static,
{
	Callback::new(f)
}

/// Creates a memoized callback with custom argument and return types.
///
/// This is a more flexible version of `use_callback` that allows specifying
/// custom argument and return types.
///
/// # Type Parameters
///
/// * `Args` - The argument type
/// * `Ret` - The return type
/// * `F` - The callback function type
///
/// # Arguments
///
/// * `f` - The callback function to memoize
///
/// # Returns
///
/// A `Callback<Args, Ret>`
///
/// # Example
///
/// ```no_run
/// use reinhardt_pages::reactive::hooks::use_callback_with;
///
/// let add = use_callback_with(|x: i32| x + 1);
/// assert_eq!(add.call(5), 6);
/// ```
#[cfg(wasm)]
pub fn use_callback_with<Args, Ret, F>(f: F) -> Callback<Args, Ret>
where
	F: Fn(Args) -> Ret + 'static,
{
	Callback::new(f)
}

/// Creates a memoized callback with custom argument and return types (server-side version).
///
/// See the WASM version for full documentation.
/// Requires `Send + Sync` bounds for thread-safe server-side usage.
#[cfg(native)]
pub fn use_callback_with<Args, Ret, F>(f: F) -> Callback<Args, Ret>
where
	F: Fn(Args) -> Ret + Send + Sync + 'static,
{
	Callback::new(f)
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::reactive::Signal;
	use serial_test::serial;

	#[test]
	#[serial]
	fn test_use_memo_basic() {
		let memo = use_memo(|| 42);
		assert_eq!(memo.get(), 42);
	}

	#[test]
	#[serial]
	fn test_use_memo_with_signal() {
		let count = Signal::new(5);

		let doubled = use_memo({
			let count = count.clone();
			move || count.get() * 2
		});

		assert_eq!(doubled.get(), 10);
	}

	#[test]
	#[serial]
	fn test_use_memo_complex() {
		let items = Signal::new(vec![1, 2, 3, 4, 5]);

		let sum = use_memo({
			let items = items.clone();
			move || items.get().iter().sum::<i32>()
		});

		assert_eq!(sum.get(), 15);
	}

	#[cfg(native)]
	#[test]
	fn test_use_callback() {
		use crate::component::DummyEvent;

		let callback = use_callback(|_: DummyEvent| {});
		callback.call(DummyEvent::default());
	}

	#[cfg(native)]
	#[test]
	fn test_use_callback_with() {
		let add_one = use_callback_with(|x: i32| x + 1);
		assert_eq!(add_one.call(5), 6);

		let concat = use_callback_with(|s: String| format!("Hello, {}", s));
		assert_eq!(concat.call("World".to_string()), "Hello, World");
	}
}
