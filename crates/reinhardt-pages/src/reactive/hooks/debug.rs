//! Debug hooks: use_debug_value and use_effect_event
//!
//! These hooks provide debugging and event handling utilities.

use crate::callback::Callback;

#[cfg(wasm)]
type EventArg = web_sys::Event;

#[cfg(native)]
type EventArg = crate::component::DummyEvent;

/// Adds a label to a custom hook in DevTools.
///
/// This is the React-like equivalent of `useDebugValue`. It allows you to
/// display a custom label for your custom hooks in browser DevTools.
///
/// # Type Parameters
///
/// * `T` - The type of the debug value
///
/// # Arguments
///
/// * `value` - The value to display in DevTools
///
/// # Example
///
/// ```ignore
/// use reinhardt_pages::reactive::hooks::{use_state, use_debug_value};
///
/// fn use_online_status() -> Signal<bool> {
///     let (is_online, set_online) = use_state(true);
///
///     // Show "Online" or "Offline" in DevTools
///     use_debug_value(if is_online.get() { "Online" } else { "Offline" });
///
///     is_online
/// }
/// ```
///
/// # Note
///
/// This is a no-op in production builds. It's only used for debugging purposes.
/// The actual DevTools integration would require browser-specific implementation.
pub fn use_debug_value<T: std::fmt::Debug>(_value: T) {
	// Note: Full DevTools integration (like React DevTools) would require a
	// dedicated browser extension. The current implementation logs values to
	// the console via debug_log! macro when debug-hooks feature is enabled.

	#[cfg(debug_assertions)]
	{
		crate::debug_log!("Debug value: {:?}", _value);
	}
}

/// Adds a label with formatting to a custom hook in DevTools.
///
/// This variant accepts a formatting function that is only called when
/// DevTools is attached, avoiding unnecessary formatting work.
///
/// # Arguments
///
/// * `value` - The raw value
/// * `format` - A function that formats the value for display
///
/// # Example
///
/// ```ignore
/// use reinhardt_pages::reactive::hooks::use_debug_value_with;
///
/// let items = use_state(vec![1, 2, 3, 4, 5]);
///
/// // Only format when DevTools is open
/// use_debug_value_with(&items.get(), |items| {
///     format!("Items: {} total", items.len())
/// });
/// ```
pub fn use_debug_value_with<T, F, R>(_value: T, _format: F)
where
	F: FnOnce(T) -> R,
	R: std::fmt::Debug,
{
	#[cfg(debug_assertions)]
	{
		let _formatted = _format(_value);
		crate::debug_log!("Debug value: {:?}", _formatted);
	}
}

/// Creates an event handler that always sees the latest props and state.
///
/// This is the React-like equivalent of `useEffectEvent` (experimental).
/// It allows you to extract non-reactive logic from Effects while still
/// reading the latest values.
///
/// # Use Case
///
/// When you have an Effect that needs to call a function with the latest
/// props/state, but you don't want changes to that function to re-run the Effect.
///
/// # Type Parameters
///
/// * `F` - The function type
///
/// # Arguments
///
/// * `f` - The function to wrap
///
/// # Returns
///
/// A `Callback` that always calls the latest version of `f`
///
/// # Example
///
/// ```ignore
/// use reinhardt_pages::reactive::hooks::{use_state, use_effect, use_effect_event};
///
/// let (url, set_url) = use_state("https://api.example.com".to_string());
/// let (item_id, set_item_id) = use_state(1);
///
/// // This callback always sees the latest url, but won't cause
/// // the effect to re-run when url changes
/// let on_fetch = use_effect_event({
///     let url = url.clone();
///     move |data: &str| {
///         log!("Fetched from {}: {}", url.get(), data);
///     }
/// });
///
/// // Effect only re-runs when item_id changes
/// use_effect({
///     let item_id = item_id.clone();
///     let on_fetch = on_fetch.clone();
///     move || {
///         let id = item_id.get();
///         // fetch(id).then(|data| on_fetch.call(data));
///     }
/// });
/// ```
///
/// # Note
///
/// Implementation note: Unlike React's experimental useEffectEvent, this
/// implementation does not prevent dependency tracking within the callback.
/// The callback provides stable identity but Signals accessed inside will
/// still be tracked if called within an Effect context.
#[cfg(wasm)]
pub fn use_effect_event<F>(f: F) -> Callback<EventArg, ()>
where
	F: Fn(EventArg) + 'static,
{
	Callback::new(f)
}

/// Creates an event handler that always sees the latest props and state (server-side version).
///
/// See the WASM version for full documentation.
#[cfg(native)]
pub fn use_effect_event<F>(f: F) -> Callback<EventArg, ()>
where
	F: Fn(EventArg) + Send + Sync + 'static,
{
	Callback::new(f)
}

/// Creates an effect event handler with custom argument types.
///
/// This is a more flexible version that allows specifying custom argument types.
#[cfg(wasm)]
pub fn use_effect_event_with<Args, Ret, F>(f: F) -> Callback<Args, Ret>
where
	F: Fn(Args) -> Ret + 'static,
{
	Callback::new(f)
}

/// Creates an effect event handler with custom argument types (server-side version).
///
/// This is a more flexible version that allows specifying custom argument and return types.
/// Requires `Send + Sync` bounds for thread-safe server-side usage.
///
/// # Type Parameters
///
/// * `Args` - The argument type
/// * `Ret` - The return type
/// * `F` - The callback function type
///
/// # Arguments
///
/// * `f` - The function to wrap
///
/// # Returns
///
/// A `Callback<Args, Ret>` that always calls the latest version of `f`
#[cfg(native)]
pub fn use_effect_event_with<Args, Ret, F>(f: F) -> Callback<Args, Ret>
where
	F: Fn(Args) -> Ret + Send + Sync + 'static,
{
	Callback::new(f)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_use_debug_value() {
		// Should not panic
		use_debug_value(42);
		use_debug_value("test");
		use_debug_value(vec![1, 2, 3]);
	}

	#[test]
	fn test_use_debug_value_with() {
		// Should not panic
		use_debug_value_with(vec![1, 2, 3], |v| format!("len: {}", v.len()));
	}

	#[cfg(native)]
	#[test]
	fn test_use_effect_event() {
		use std::sync::{Arc, Mutex};

		// Use Arc<Mutex<T>> for thread-safe state (required for Send + Sync on non-WASM)
		let called = Arc::new(Mutex::new(false));

		let handler = use_effect_event({
			let called = Arc::clone(&called);
			move |_: crate::component::DummyEvent| {
				*called.lock().unwrap() = true;
			}
		});

		handler.call(crate::component::DummyEvent::default());
		assert!(*called.lock().unwrap());
	}

	#[cfg(native)]
	#[test]
	fn test_use_effect_event_with() {
		let add_one = use_effect_event_with(|x: i32| x + 1);
		assert_eq!(add_one.call(5), 6);
	}
}
