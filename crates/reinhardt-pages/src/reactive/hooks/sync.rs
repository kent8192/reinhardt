//! External store hook: use_sync_external_store
//!
//! This hook provides a way to subscribe to external data stores.

use std::ops::Deref;
use std::rc::Rc;

use crate::reactive::Signal;

/// A handle that manages the subscription lifecycle.
///
/// When dropped, the subscription is automatically cancelled,
/// preventing memory leaks.
pub struct SubscriptionHandle {
	unsubscribe: Option<Box<dyn FnOnce()>>,
}

impl SubscriptionHandle {
	fn new(unsubscribe: Box<dyn FnOnce()>) -> Self {
		Self {
			unsubscribe: Some(unsubscribe),
		}
	}
}

impl Drop for SubscriptionHandle {
	fn drop(&mut self) {
		if let Some(unsub) = self.unsubscribe.take() {
			unsub();
		}
	}
}

/// A Signal paired with its subscription handle.
///
/// This type combines a reactive `Signal<T>` with its subscription cleanup logic.
/// When dropped, it automatically unsubscribes from the external store.
///
/// # Cloning Behavior
///
/// When cloned, the `SignalWithSubscription` creates a new instance that shares
/// the same underlying `Signal`, but does NOT share the subscription handle.
/// Only the original instance will call `unsubscribe` when dropped.
///
/// This design ensures:
/// - Multiple parts of the code can read from the same signal
/// - The subscription cleanup happens exactly once
/// - No double-free or use-after-free issues
///
/// # Example
///
/// ```ignore
/// let signal_with_sub = use_sync_external_store(subscribe, get_snapshot);
///
/// // Use it like a normal Signal
/// let value = signal_with_sub.get();
///
/// // Clone shares the signal but not the subscription
/// let cloned = signal_with_sub.clone();
/// assert_eq!(cloned.get(), signal_with_sub.get());
///
/// // When signal_with_sub is dropped, unsubscribe is called
/// // When cloned is dropped, nothing happens
/// ```
pub struct SignalWithSubscription<T: 'static> {
	signal: Signal<T>,
	_handle: SubscriptionHandle,
}

impl<T: 'static> SignalWithSubscription<T> {
	fn new(signal: Signal<T>, handle: SubscriptionHandle) -> Self {
		Self {
			signal,
			_handle: handle,
		}
	}

	/// Get a reference to the underlying signal.
	pub fn signal(&self) -> &Signal<T> {
		&self.signal
	}
}

impl<T: Clone + 'static> SignalWithSubscription<T> {
	/// Get the current value.
	pub fn get(&self) -> T {
		self.signal.get()
	}
}

impl<T: 'static> Deref for SignalWithSubscription<T> {
	type Target = Signal<T>;

	fn deref(&self) -> &Self::Target {
		&self.signal
	}
}

impl<T: Clone + 'static> Clone for SignalWithSubscription<T> {
	fn clone(&self) -> Self {
		// Note: Cloning does NOT clone the subscription handle.
		// This is intentional - only the original instance manages the subscription.
		// The cloned instance shares the same signal but won't unsubscribe on drop.
		Self {
			signal: self.signal.clone(),
			_handle: SubscriptionHandle { unsubscribe: None },
		}
	}
}

/// Subscribes to an external store.
///
/// This is the React-like equivalent of `useSyncExternalStore`. It allows you
/// to subscribe to external data sources like Redux stores, browser APIs, or
/// any other mutable state that exists outside the reactive system.
///
/// # Type Parameters
///
/// * `T` - The type of value in the store
/// * `S` - The subscribe function type
/// * `G` - The get snapshot function type
///
/// # Arguments
///
/// * `subscribe` - A function that subscribes to the store and returns an unsubscribe function
/// * `get_snapshot` - A function that returns the current value from the store
///
/// # Returns
///
/// A `SignalWithSubscription<T>` that stays in sync with the external store.
/// When dropped, the subscription is automatically cancelled.
///
/// # Example
///
/// ```ignore
/// use reinhardt_pages::reactive::hooks::use_sync_external_store;
///
/// // Subscribe to browser online status
/// let is_online = use_sync_external_store(
///     // Subscribe function
///     |on_change| {
///         window().add_event_listener("online", on_change.clone());
///         window().add_event_listener("offline", on_change.clone());
///
///         // Return unsubscribe
///         Box::new(move || {
///             window().remove_event_listener("online", on_change.clone());
///             window().remove_event_listener("offline", on_change);
///         })
///     },
///     // Get current value
///     || navigator().on_line(),
/// );
///
/// if is_online.get() {
///     // ... online content
/// } else {
///     // ... offline content
/// }
///
/// // When is_online goes out of scope, unsubscribe is called automatically
/// ```
///
/// # When to Use
///
/// Use this hook when you need to:
/// - Subscribe to browser APIs (window size, online status, etc.)
/// - Integrate with external state management libraries
/// - Read from mutable sources that update outside React
///
/// # Note
///
/// The `get_snapshot` function should return consistent results when called
/// multiple times in a row. If the store's data changes, it should return
/// a different reference (for objects) or value (for primitives).
pub fn use_sync_external_store<T, S, G>(subscribe: S, get_snapshot: G) -> SignalWithSubscription<T>
where
	T: Clone + PartialEq + 'static,
	S: FnOnce(Rc<dyn Fn()>) -> Box<dyn FnOnce()> + 'static,
	G: Fn() -> T + 'static,
{
	let state = Signal::new(get_snapshot());

	// Set up the subscription
	let state_clone = state.clone();
	let get_snapshot = Rc::new(get_snapshot);
	let get_snapshot_clone = Rc::clone(&get_snapshot);

	let on_change: Rc<dyn Fn()> = Rc::new({
		let state = state_clone.clone();
		move || {
			let new_value = get_snapshot_clone();
			if state.get() != new_value {
				state.set(new_value);
			}
		}
	});

	// Subscribe and store the unsubscribe function
	let unsubscribe = subscribe(on_change);
	let handle = SubscriptionHandle::new(unsubscribe);

	SignalWithSubscription::new(state, handle)
}

/// Subscribes to an external store with server snapshot support.
///
/// This variant is useful for SSR where you need different snapshots
/// for server and client environments.
///
/// # Type Parameters
///
/// * `T` - The type of value in the store
/// * `S` - The subscribe function type
/// * `G` - The get snapshot function type
/// * `GS` - The get server snapshot function type
///
/// # Arguments
///
/// * `subscribe` - A function that subscribes to the store and returns an unsubscribe function
/// * `get_snapshot` - A function that returns the current value from the store (client)
/// * `get_server_snapshot` - A function that returns the value during SSR
///
/// # Returns
///
/// A `SignalWithSubscription<T>` that stays in sync with the external store.
/// On server-side (non-WASM), the subscription is a no-op.
pub fn use_sync_external_store_with_server<T, S, G, GS>(
	subscribe: S,
	get_snapshot: G,
	get_server_snapshot: GS,
) -> SignalWithSubscription<T>
where
	T: Clone + PartialEq + 'static,
	S: FnOnce(Rc<dyn Fn()>) -> Box<dyn FnOnce()> + 'static,
	G: Fn() -> T + 'static,
	GS: Fn() -> T + 'static,
{
	// Use server snapshot during SSR
	#[cfg(native)]
	{
		let _ = subscribe;
		let _ = get_snapshot;
		// For server-side, create a no-op subscription handle
		let signal = Signal::new(get_server_snapshot());
		let handle = SubscriptionHandle::new(Box::new(|| {}));
		SignalWithSubscription::new(signal, handle)
	}

	// Use client snapshot in browser
	#[cfg(wasm)]
	{
		let _ = get_server_snapshot;
		use_sync_external_store(subscribe, get_snapshot)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use std::cell::RefCell;

	#[test]
	fn test_use_sync_external_store_basic() {
		let store_value = Rc::new(RefCell::new(42));

		let signal_with_sub = use_sync_external_store(
			|_on_change| {
				// No-op subscribe for test
				Box::new(|| {})
			},
			{
				let store_value = Rc::clone(&store_value);
				move || *store_value.borrow()
			},
		);

		assert_eq!(signal_with_sub.get(), 42);
	}

	#[test]
	fn test_use_sync_external_store_with_update() {
		let store_value = Rc::new(RefCell::new(0));
		let on_change_fn: Rc<RefCell<Option<Rc<dyn Fn()>>>> = Rc::new(RefCell::new(None));

		let signal_with_sub = use_sync_external_store(
			{
				let on_change_fn = Rc::clone(&on_change_fn);
				move |on_change| {
					*on_change_fn.borrow_mut() = Some(on_change);
					Box::new(|| {})
				}
			},
			{
				let store_value = Rc::clone(&store_value);
				move || *store_value.borrow()
			},
		);

		assert_eq!(signal_with_sub.get(), 0);

		// Simulate store update
		*store_value.borrow_mut() = 100;

		// Trigger the on_change callback
		if let Some(on_change) = on_change_fn.borrow().as_ref() {
			on_change();
		}

		assert_eq!(signal_with_sub.get(), 100);
	}

	#[cfg(native)]
	#[test]
	fn test_use_sync_external_store_with_server() {
		let signal_with_sub = use_sync_external_store_with_server(
			|_| Box::new(|| {}),
			|| 42, // Client snapshot
			|| 0,  // Server snapshot
		);

		// Should use server snapshot in non-WASM environment
		assert_eq!(signal_with_sub.get(), 0);
	}

	#[test]
	fn test_subscription_cleanup() {
		let unsubscribed = Rc::new(RefCell::new(false));

		{
			let signal_with_sub = use_sync_external_store(
				{
					let unsubscribed = Rc::clone(&unsubscribed);
					move |_on_change| {
						Box::new(move || {
							*unsubscribed.borrow_mut() = true;
						})
					}
				},
				|| 42,
			);

			assert_eq!(signal_with_sub.get(), 42);
			assert!(
				!*unsubscribed.borrow(),
				"Should not unsubscribe while in scope"
			);
		}

		// SignalWithSubscription dropped, unsubscribe should be called
		assert!(*unsubscribed.borrow(), "Should unsubscribe when dropped");
	}
}
