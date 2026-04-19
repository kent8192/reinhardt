//! Callback types and event handler conversion traits.
//!
//! This module provides type-safe callback wrappers and the [`IntoEventHandler`] trait
//! for converting various handler types to [`PageEventHandler`].
//!
//! ## Features
//!
//! - **Callback<Args, Ret>**: A type-safe, cloneable wrapper for event handlers
//! - **IntoEventHandler**: Trait for converting closures, Callbacks, and `Arc<Fn>` to PageEventHandler
//!
//! ## Example
//!
//! ```ignore
//! use reinhardt_pages::{Callback, page};
//!
//! // Define handler outside macro
//! let handle_click = Callback::new(|_event| {
//!     log!("Button clicked!");
//! });
//!
//! // Use in page! macro
//! page!(|| {
//!     button {
//!         @click: handle_click,
//!         "Click me"
//!     }
//! })
//! ```

use std::future::Future;
use std::sync::Arc;

use crate::component::PageEventHandler;
#[cfg(wasm)]
use crate::spawn::spawn_task;

#[cfg(wasm)]
type EventArg = web_sys::Event;

#[cfg(native)]
type EventArg = crate::component::DummyEvent;

/// A type-safe, cloneable callback wrapper for event handlers.
///
/// `Callback` wraps a function in an `Arc`, making it cheaply cloneable while
/// providing a stable reference that won't change between renders.
///
/// ## Type Parameters
///
/// - `Args`: The argument type the callback receives (defaults to Event)
/// - `Ret`: The return type of the callback (defaults to `()`)
///
/// ## Example
///
/// ```ignore
/// use reinhardt_pages::Callback;
///
/// // Simple click handler
/// let on_click = Callback::new(|_event| {
///     log!("Clicked!");
/// });
///
/// // Handler with state capture
/// let count = Signal::new(0);
/// let increment = Callback::new({
///     let count = count.clone();
///     move |_| count.update(|n| *n += 1)
/// });
/// ```
// Callback struct with conditional Send + Sync bounds for non-WASM targets
#[cfg(wasm)]
pub struct Callback<Args = EventArg, Ret = ()> {
	inner: Arc<dyn Fn(Args) -> Ret + 'static>,
}

/// A type-safe, cloneable callback wrapper for event handlers (server-side version).
///
/// See the WASM version for full documentation.
/// This version requires `Send + Sync` bounds for thread-safe server-side usage.
#[cfg(native)]
pub struct Callback<Args = EventArg, Ret = ()> {
	inner: Arc<dyn Fn(Args) -> Ret + Send + Sync + 'static>,
}

// WASM implementation without Send + Sync bounds
#[cfg(wasm)]
impl<Args, Ret> Callback<Args, Ret> {
	/// Creates a new Callback from a function or closure.
	///
	/// # Arguments
	///
	/// * `f` - The function or closure to wrap
	///
	/// # Example
	///
	/// ```no_run
	/// let handler = Callback::new(|event| {
	///     // Handle event
	/// });
	/// ```
	pub fn new<F>(f: F) -> Self
	where
		F: Fn(Args) -> Ret + 'static,
	{
		Self { inner: Arc::new(f) }
	}

	/// Calls the callback with the given arguments.
	///
	/// # Arguments
	///
	/// * `args` - The arguments to pass to the callback
	pub fn call(&self, args: Args) -> Ret {
		(self.inner)(args)
	}
}

// Non-WASM implementation with Send + Sync bounds
#[cfg(native)]
impl<Args, Ret> Callback<Args, Ret> {
	/// Creates a new Callback from a function or closure.
	///
	/// # Arguments
	///
	/// * `f` - The function or closure to wrap
	///
	/// # Example
	///
	/// ```ignore
	/// let handler = Callback::new(|event| {
	///     // Handle event
	/// });
	/// ```
	pub fn new<F>(f: F) -> Self
	where
		F: Fn(Args) -> Ret + Send + Sync + 'static,
	{
		Self { inner: Arc::new(f) }
	}

	/// Calls the callback with the given arguments.
	///
	/// # Arguments
	///
	/// * `args` - The arguments to pass to the callback
	pub fn call(&self, args: Args) -> Ret {
		(self.inner)(args)
	}
}

impl<Args, Ret> Clone for Callback<Args, Ret> {
	fn clone(&self) -> Self {
		Self {
			inner: Arc::clone(&self.inner),
		}
	}
}

impl<Args, Ret> std::fmt::Debug for Callback<Args, Ret> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("Callback")
			.field("inner", &"<function>")
			.finish()
	}
}

/// Trait for converting various handler types to [`PageEventHandler`].
///
/// This trait is implemented for:
/// - Closures that take an Event argument
/// - [`Callback<Event, ()>`]
/// - [`PageEventHandler`] (identity conversion)
///
/// The `page!` macro uses this trait internally to allow both inline closures
/// and external handler references.
///
/// ## Example
///
/// ```ignore
/// use reinhardt_pages::{Callback, IntoEventHandler};
///
/// // Closure implementation
/// let closure_handler = |_event| { log!("clicked"); };
/// let view_handler = closure_handler.into_event_handler();
///
/// // Callback implementation
/// let callback = Callback::new(|_| { log!("clicked"); });
/// let view_handler = callback.into_event_handler();
/// ```
pub trait IntoEventHandler {
	/// Converts self into a [`PageEventHandler`].
	fn into_event_handler(self) -> PageEventHandler;
}

/// Blanket implementation for closures that match the event handler signature.
///
/// # WASM Build
/// Accepts `Fn(web_sys::Event) + 'static`
///
/// # Non-WASM Build
/// Accepts `Fn(DummyEvent) + Send + Sync + 'static`
#[cfg(wasm)]
impl<F> IntoEventHandler for F
where
	F: Fn(web_sys::Event) + 'static,
{
	fn into_event_handler(self) -> PageEventHandler {
		Arc::new(self)
	}
}

#[cfg(native)]
impl<F> IntoEventHandler for F
where
	F: Fn(crate::component::DummyEvent) + 'static,
{
	fn into_event_handler(self) -> PageEventHandler {
		Arc::new(self)
	}
}

/// Implementation for Callback type.
impl IntoEventHandler for Callback<EventArg, ()> {
	fn into_event_handler(self) -> PageEventHandler {
		self.inner
	}
}

/// Identity implementation for PageEventHandler.
impl IntoEventHandler for PageEventHandler {
	fn into_event_handler(self) -> PageEventHandler {
		self
	}
}

/// Convenience function for converting handlers in generated code.
///
/// This function is used by the `page!` macro's code generation to convert
/// event handlers of various types to [`PageEventHandler`].
///
/// # Example
///
/// ```ignore
/// // Generated by page! macro:
/// ElementView::new("button")
///     .on(EventType::Click, into_event_handler(|_| {}))
/// ```
pub fn into_event_handler<H: IntoEventHandler>(handler: H) -> PageEventHandler {
	handler.into_event_handler()
}

/// Event handler helper with concrete type for better type inference.
///
/// This function is used by the `page!` macro's code generation.
/// Unlike [`into_event_handler`], this function has a concrete argument type,
/// allowing Rust to infer the closure parameter type automatically.
///
/// # Example
///
/// ```ignore
/// // This works without explicit type annotation
/// let handler = event_handler(|_| {
///     log!("clicked");
/// });
/// ```
#[cfg(wasm)]
pub fn event_handler(f: impl Fn(web_sys::Event) + 'static) -> PageEventHandler {
	Arc::new(f)
}

/// Event handler helper with concrete type for better type inference (server-side version).
///
/// See WASM version for documentation.
#[cfg(native)]
pub fn event_handler(f: impl Fn(crate::component::DummyEvent) + 'static) -> PageEventHandler {
	Arc::new(f)
}

/// Creates an async event handler that automatically spawns the future.
///
/// This function wraps an async closure in a synchronous event handler that
/// calls `spawn_task` to execute the async task. This eliminates the need
/// for users to manually call `spawn_local` in their event handlers.
///
/// # Example (WASM)
///
/// ```ignore
/// use reinhardt_pages::callback::async_handler;
///
/// let handler = async_handler(async move |event| {
///     let data = fetch_data().await;
///     process(data);
/// });
///
/// button {
///     @click: handler,
///     "Click me"
/// }
/// ```
#[cfg(wasm)]
pub fn async_handler<F, Fut>(f: F) -> PageEventHandler
where
	F: Fn(web_sys::Event) -> Fut + 'static,
	Fut: Future<Output = ()> + 'static,
{
	Arc::new(move |event| {
		let fut = f(event);
		spawn_task(fut);
	})
}

/// Creates an async event handler stub for non-WASM targets.
#[cfg(native)]
pub fn async_handler<F, Fut>(f: F) -> PageEventHandler
where
	F: Fn(crate::component::DummyEvent) -> Fut + Send + Sync + 'static,
	Fut: Future<Output = ()> + Send + 'static,
{
	Arc::new(move |event| {
		// Non-WASM stub: drop the future without awaiting
		std::mem::drop(f(event));
	})
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_callback_creation() {
		let callback = Callback::new(|_: i32| 42);
		assert_eq!(callback.call(0), 42);
	}

	#[test]
	fn test_callback_clone() {
		let callback1 = Callback::new(|x: i32| x * 2);
		let callback2 = callback1.clone();

		assert_eq!(callback1.call(5), 10);
		assert_eq!(callback2.call(5), 10);
	}

	#[test]
	fn test_callback_with_captured_state() {
		use std::sync::{Arc, Mutex};

		// Use Arc<Mutex<T>> for thread-safe state (required for Send + Sync on non-WASM)
		let counter = Arc::new(Mutex::new(0));
		let callback = Callback::new({
			let counter = Arc::clone(&counter);
			move |increment: i32| {
				*counter.lock().unwrap() += increment;
			}
		});

		callback.call(1);
		callback.call(2);
		callback.call(3);

		assert_eq!(*counter.lock().unwrap(), 6);
	}

	#[test]
	fn test_callback_debug() {
		let callback = Callback::new(|_: ()| {});
		let debug_str = format!("{:?}", callback);
		assert!(debug_str.contains("Callback"));
	}

	#[cfg(native)]
	#[test]
	fn test_into_event_handler_closure() {
		use crate::component::DummyEvent;

		let closure = |_: DummyEvent| {};
		let _handler: PageEventHandler = closure.into_event_handler();
	}

	#[cfg(native)]
	#[test]
	fn test_into_event_handler_callback() {
		let callback = Callback::new(|_: crate::component::DummyEvent| {});
		let _handler: PageEventHandler = callback.into_event_handler();
	}

	#[cfg(native)]
	#[test]
	fn test_into_event_handler_function() {
		use crate::component::DummyEvent;

		let handler: PageEventHandler = into_event_handler(|_: DummyEvent| {});
		// Verify it's callable
		handler(DummyEvent::default());
	}
}
