//! Reactive primitive trait definitions.
//!
//! These traits define the abstract interface for reactive primitives
//! that backends (like `reinhardt-pages`) implement.

/// A reactive signal that can be read and written.
///
/// Signals are the primary reactive primitive for managing state.
pub trait Signal<T> {
	/// Gets the current value.
	fn get(&self) -> T;

	/// Sets a new value.
	fn set(&self, value: T);

	/// Updates the value using a function.
	fn update(&self, f: impl FnOnce(&mut T));
}

/// A read-only signal.
///
/// Used for derived values or signals that should not be modified directly.
pub trait ReadSignal<T> {
	/// Gets the current value.
	fn get(&self) -> T;
}

/// A reactive effect that runs when dependencies change.
pub trait Effect {
	/// Creates a new effect with the given function.
	fn new(f: impl FnMut() + 'static) -> Self;
}

/// A memoized computation that caches its result.
pub trait Memo<T> {
	/// Creates a new memo with the given computation function.
	fn new(f: impl Fn() -> T + 'static) -> Self;

	/// Gets the memoized value, recomputing if dependencies changed.
	fn get(&self) -> T;
}
