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
//! use reinhardt_core::reactive::ReactiveScope;
//! use reinhardt_pages::{Callback, page};
//!
//! ReactiveScope::run(|| {
//!     // Define handler outside macro
//!     let handle_click = Callback::new(|_event| {
//!         log!("Button clicked!");
//!     });
//!
//!     // Use in page! macro
//!     page!(|| {
//!         button {
//!             @click: handle_click,
//!             "Click me"
//!         }
//!     })
//! });
//! ```

use core::marker::PhantomData;
use std::future::Future;
use std::rc::Rc;
use std::sync::Arc;

#[cfg(wasm)]
use std::pin::Pin;
#[cfg(wasm)]
use std::task::{Context, Poll};

use crate::component::PageEventHandler;
use crate::event::{CustomEvent, EventPayload};
use crate::platform::spawn_task;
use crate::reactive::pages_arena::{
	PageNodeKey, PageNodeKind, allocate_page_node, dispose_page_node, with_page_node,
};
use reinhardt_core::reactive::ReactiveScope;
#[cfg(wasm)]
use reinhardt_core::reactive::current_scope_id;
use serde::de::DeserializeOwned;

#[cfg(wasm)]
struct ScopedAsyncEventFuture<Fut> {
	scope: Option<reinhardt_core::reactive::ScopeId>,
	future: Pin<Box<Fut>>,
}

#[cfg(wasm)]
impl<Fut> Future for ScopedAsyncEventFuture<Fut>
where
	Fut: Future<Output = ()>,
{
	type Output = ();

	fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
		let this = self.get_mut();
		let mut poll = || this.future.as_mut().poll(cx);
		match this.scope {
			Some(scope) => {
				reinhardt_core::reactive::scope::enter_scope(scope, poll).unwrap_or(Poll::Ready(()))
			}
			None => poll(),
		}
	}
}

#[cfg(wasm)]
fn scope_async_event_future<Fut>(
	scope: Option<reinhardt_core::reactive::ScopeId>,
	future: Fut,
) -> ScopedAsyncEventFuture<Fut>
where
	Fut: Future<Output = ()> + 'static,
{
	ScopedAsyncEventFuture {
		scope,
		future: Box::pin(future),
	}
}

#[cfg(wasm)]
type EventArg = web_sys::Event;

#[cfg(native)]
type EventArg = crate::component::NativeEvent;

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
/// Callbacks are bound to the thread that owns their page arena storage.
///
/// ```compile_fail
/// use reinhardt_pages::Callback;
///
/// let callback = Callback::<(), ()>::new(|_| {});
/// std::thread::spawn(move || callback.call(()));
/// ```
///
/// ## Example
///
/// ```ignore
/// use reinhardt_pages::Callback;
///
/// // Simple click handler
/// ReactiveScope::run(|| {
///     let on_click = Callback::new(|_event| {
///         log!("Clicked!");
///     });
/// });
///
/// // Handler with state capture
/// let count = Signal::new(0);
/// let increment = Callback::new({
///     let count = count.clone();
///     move |_| count.update(|n| *n += 1)
/// });
/// ```
// Callback struct for WASM targets.
#[cfg(wasm)]
struct CallbackSlot<Args, Ret> {
	inner: Arc<dyn Fn(Args) -> Ret + 'static>,
}

/// A type-safe, cloneable callback wrapper for event handlers (server-side version).
///
/// See the WASM version for full documentation.
///
/// Native page handlers share the single-threaded event contract of
/// [`PageEventHandler`], so callbacks may capture reactive state.
#[cfg(native)]
struct CallbackSlot<Args, Ret> {
	inner: Arc<dyn Fn(Args) -> Ret + 'static>,
}

fn allocate_callback<Args: 'static, Ret: 'static>(
	f: impl Fn(Args) -> Ret + 'static,
) -> PageNodeKey {
	allocate_page_node(
		"Callback::new",
		PageNodeKind::Callback,
		CallbackSlot { inner: Arc::new(f) },
	)
}

/// A copied key to a callback stored in the current reactive scope.
pub struct Callback<Args = EventArg, Ret = ()> {
	key: PageNodeKey,
	_marker: PhantomData<fn(Args) -> Ret>,
}

// WASM implementation without Send + Sync bounds
#[cfg(wasm)]
impl<Args: 'static, Ret: 'static> Callback<Args, Ret> {
	/// Creates a new Callback from a function or closure.
	///
	/// The callback is stored in the active [`ReactiveScope`]. For callbacks assembled outside
	/// a render, use [`Callback::new_in_scope`] with an owner that outlives the callback.
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
		Self {
			key: allocate_callback(f),
			_marker: PhantomData,
		}
	}
}

// Non-WASM implementation
#[cfg(native)]
impl<Args: 'static, Ret: 'static> Callback<Args, Ret> {
	/// Creates a new Callback from a function or closure.
	///
	/// The callback is stored in the active [`ReactiveScope`]. For callbacks assembled outside
	/// a render, use [`Callback::new_in_scope`] with an owner that outlives the callback.
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
		F: Fn(Args) -> Ret + 'static,
	{
		Self {
			key: allocate_callback(f),
			_marker: PhantomData,
		}
	}
}

impl<Args, Ret> Clone for Callback<Args, Ret> {
	fn clone(&self) -> Self {
		*self
	}
}

impl<Args, Ret> Copy for Callback<Args, Ret> {}

impl<Args: 'static, Ret: 'static> Callback<Args, Ret> {
	/// Creates a callback in an explicitly owned reactive scope.
	///
	/// This is the lifecycle-safe entry point for callbacks created outside a render. The
	/// callback remains [`Copy`], while the supplied [`ReactiveScope`] owns its storage and
	/// disposes it when the scope is dropped or explicitly disposed.
	pub fn new_in_scope<F>(scope: &ReactiveScope, f: F) -> Self
	where
		F: Fn(Args) -> Ret + 'static,
	{
		scope.enter(|| Self::new(f))
	}

	pub(crate) fn new_in_scope_id<F>(scope: reinhardt_core::reactive::ScopeId, f: F) -> Self
	where
		F: Fn(Args) -> Ret + 'static,
	{
		reinhardt_core::reactive::scope::enter_scope(scope, || Self::new(f))
			.unwrap_or_else(|err| panic!("{err}"))
	}

	/// Call this callback with the supplied arguments.
	pub fn call(&self, args: Args) -> Ret {
		self.try_call(args).unwrap_or_else(|err| panic!("{err}"))
	}

	/// Calls this callback while reporting a disposed owner instead of panicking.
	///
	/// This is intended for callbacks retained by timers, promises, or external
	/// event sources which may fire after their mounted view has unmounted.
	pub fn try_call(&self, args: Args) -> Result<Ret, String> {
		let inner =
			with_page_node::<CallbackSlot<Args, Ret>, _>(self.key, |slot| Arc::clone(&slot.inner))?;
		reinhardt_core::reactive::scope::enter_scope(self.key.scope(), || inner(args))
			.map_err(|err| err.to_string())
	}
}

fn scoped_event_handler<F>(handler: F) -> PageEventHandler
where
	F: Fn(EventArg) + 'static,
{
	let scope = reinhardt_core::reactive::scope::current_scope_id();
	Arc::new(move |event| match scope {
		Some(scope) => {
			let _ = reinhardt_core::reactive::scope::enter_scope(scope, || handler(event));
		}
		None => handler(event),
	})
}

#[cfg(wasm)]
pub(crate) fn run_event_handler_in_scope(
	scope: Option<reinhardt_core::reactive::ScopeId>,
	handler: &PageEventHandler,
	event: web_sys::Event,
) {
	match scope {
		Some(scope) => {
			let _ = reinhardt_core::reactive::scope::enter_scope(scope, || handler(event));
		}
		None => handler(event),
	}
}

// ---------------------------------------------------------------------------
// callback_with_deps — internal helper for the use_callback / use_callback_with
// hooks (Refs #4195).
//
// Maintains Rc/Arc identity of `Callback::inner` across re-entries at the same
// call site while the listed deps NodeIds are unchanged, and swaps the inner
// Arc<Fn> when deps change. Mirrors React's `useCallback(f, deps)` semantics.
// ---------------------------------------------------------------------------

/// Per-call-site state for [`callback_with_deps`].
///
/// Keyed by leaked `&'static str` of the caller's `Location` and stores the
/// most recent deps tuple together with a type-erased `Rc`. The inner value is
/// downcast and copied into the returned `Callback`.
#[allow(dead_code)]
struct CallbackSlotEntry {
	deps: smallvec::SmallVec<[reinhardt_core::reactive::runtime::NodeId; 8]>,
	scope: reinhardt_core::reactive::ScopeId,
	callback_type: std::any::TypeId,
	key_any: Rc<dyn std::any::Any>,
}

thread_local! {
	#[allow(clippy::type_complexity)]
	static CALLBACK_REGISTRY: std::cell::RefCell<
		std::collections::HashMap<(&'static str, reinhardt_core::reactive::ScopeId), CallbackSlotEntry>,
	> = std::cell::RefCell::new(std::collections::HashMap::new());
}

/// Internal helper used by the `use_callback` / `use_callback_with` hooks.
///
/// Returns a `Callback<Args, Ret>` whose internal `Arc<dyn Fn>` identity
/// is stable while `deps` NodeIds are unchanged across re-invocations at
/// the same call site, and is replaced when any dep changes.
///
/// The `#[track_caller]` attribute lets us key the registry by the
/// source `(file, line, column)` of the caller so different call sites
/// have independent slots.
#[cfg(wasm)]
#[track_caller]
#[allow(dead_code)]
pub(crate) fn callback_with_deps<Args, Ret>(
	f: impl Fn(Args) -> Ret + 'static,
	deps: reinhardt_core::reactive::deps::Deps,
) -> Callback<Args, Ret>
where
	Args: 'static,
	Ret: 'static,
{
	let loc = std::panic::Location::caller();
	let key: &'static str = {
		let s = format!("{}:{}:{}", loc.file(), loc.line(), loc.column());
		Box::leak(s.into_boxed_str())
	};

	let scope = reinhardt_core::reactive::scope::current_scope_id().unwrap_or_else(|| {
		panic!(
			"{}",
			reinhardt_core::reactive::ReactiveScopeError::NoActiveScope {
				operation: "use_callback"
			}
		);
	});
	let mut inserted = false;
	let callback = CALLBACK_REGISTRY.with(|reg| {
		let mut reg = reg.borrow_mut();
		let new_ids: smallvec::SmallVec<[reinhardt_core::reactive::runtime::NodeId; 8]> =
			deps.as_slice().iter().copied().collect();

		let slot = match reg.entry((key, scope)) {
			std::collections::hash_map::Entry::Occupied(entry) => entry.into_mut(),
			std::collections::hash_map::Entry::Vacant(entry) => {
				inserted = true;
				entry.insert(CallbackSlotEntry {
					deps: smallvec::SmallVec::new(),
					scope,
					callback_type: std::any::TypeId::of::<()>(),
					key_any: Rc::new(()) as Rc<dyn std::any::Any>,
				})
			}
		};

		let needs_replace = slot.deps.as_slice() != new_ids.as_slice()
			|| slot.callback_type != std::any::TypeId::of::<(Args, Ret)>()
			|| !slot.key_any.is::<PageNodeKey>();

		if needs_replace {
			if let Some(key) = slot.key_any.downcast_ref::<PageNodeKey>() {
				dispose_page_node(*key);
			}
			let callback = Callback::new(f);
			slot.deps = new_ids;
			slot.scope = scope;
			slot.callback_type = std::any::TypeId::of::<(Args, Ret)>();
			slot.key_any = Rc::new(callback.key) as Rc<dyn std::any::Any>;
		}

		let saved_key = slot
			.key_any
			.downcast_ref::<PageNodeKey>()
			.expect("CallbackSlot type mismatch — call site changed signature");

		Callback {
			key: *saved_key,
			_marker: PhantomData,
		}
	});
	if inserted {
		let _ = reinhardt_core::reactive::scope::on_scope_dispose_after_nodes(scope, move || {
			CALLBACK_REGISTRY.with(|registry| {
				registry.borrow_mut().remove(&(key, scope));
			});
		});
	}
	callback
}

/// Internal helper used by `use_callback` / `use_callback_with` (native).
///
/// See the `cfg(wasm)` variant above for full documentation.
#[cfg(native)]
#[track_caller]
#[allow(dead_code)]
pub(crate) fn callback_with_deps<Args, Ret>(
	f: impl Fn(Args) -> Ret + 'static,
	deps: reinhardt_core::reactive::deps::Deps,
) -> Callback<Args, Ret>
where
	Args: 'static,
	Ret: 'static,
{
	type InnerArc<A, R> = Arc<dyn Fn(A) -> R + 'static>;

	let loc = std::panic::Location::caller();
	let key: &'static str = {
		let s = format!("{}:{}:{}", loc.file(), loc.line(), loc.column());
		Box::leak(s.into_boxed_str())
	};

	let scope = reinhardt_core::reactive::scope::current_scope_id().unwrap_or_else(|| {
		panic!(
			"{}",
			reinhardt_core::reactive::ReactiveScopeError::NoActiveScope {
				operation: "use_callback"
			}
		);
	});
	let mut inserted = false;
	let callback = CALLBACK_REGISTRY.with(|reg| {
		let mut reg = reg.borrow_mut();
		let new_ids: smallvec::SmallVec<[reinhardt_core::reactive::runtime::NodeId; 8]> =
			deps.as_slice().iter().copied().collect();

		let slot = match reg.entry((key, scope)) {
			std::collections::hash_map::Entry::Occupied(entry) => entry.into_mut(),
			std::collections::hash_map::Entry::Vacant(entry) => {
				inserted = true;
				entry.insert(CallbackSlotEntry {
					deps: smallvec::SmallVec::new(),
					scope,
					callback_type: std::any::TypeId::of::<()>(),
					key_any: Rc::new(()) as Rc<dyn std::any::Any>,
				})
			}
		};

		let needs_replace = slot.deps.as_slice() != new_ids.as_slice()
			|| slot.callback_type != std::any::TypeId::of::<(Args, Ret)>()
			|| !slot.key_any.is::<PageNodeKey>();

		if needs_replace {
			if let Some(key) = slot.key_any.downcast_ref::<PageNodeKey>() {
				dispose_page_node(*key);
			}
			let callback = Callback::new(f);
			slot.deps = new_ids;
			slot.scope = scope;
			slot.callback_type = std::any::TypeId::of::<(Args, Ret)>();
			slot.key_any = Rc::new(callback.key) as Rc<dyn std::any::Any>;
		}

		let saved_key = slot
			.key_any
			.downcast_ref::<PageNodeKey>()
			.expect("CallbackSlot type mismatch — call site changed signature");

		Callback {
			key: *saved_key,
			_marker: PhantomData,
		}
	});
	if inserted {
		let _ = reinhardt_core::reactive::scope::on_scope_dispose_after_nodes(scope, move || {
			CALLBACK_REGISTRY.with(|registry| {
				registry.borrow_mut().remove(&(key, scope));
			});
		});
	}
	callback
}

#[cfg(test)]
impl<Args: 'static, Ret: 'static> Callback<Args, Ret> {
	/// Test-only clone that keeps the callback allocation alive for identity checks.
	pub(crate) fn inner_arc(&self) -> Arc<dyn Fn(Args) -> Ret + 'static> {
		with_page_node::<CallbackSlot<Args, Ret>, _>(self.key, |slot| Arc::clone(&slot.inner))
			.unwrap_or_else(|err| panic!("{err}"))
	}
}

impl<Args, Ret> std::fmt::Debug for Callback<Args, Ret> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("Callback").field("key", &self.key).finish()
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
/// Accepts `Fn(NativeEvent) + 'static`
#[cfg(wasm)]
impl<F> IntoEventHandler for F
where
	F: Fn(web_sys::Event) + 'static,
{
	fn into_event_handler(self) -> PageEventHandler {
		scoped_event_handler(self)
	}
}

#[cfg(native)]
impl<F> IntoEventHandler for F
where
	F: Fn(crate::component::NativeEvent) + 'static,
{
	fn into_event_handler(self) -> PageEventHandler {
		scoped_event_handler(self)
	}
}

/// Implementation for Callback type.
impl IntoEventHandler for Callback<EventArg, ()> {
	#[allow(
		clippy::arc_with_non_send_sync,
		reason = "PageEventHandler is Arc-backed for cloneable Page elements while Callback intentionally retains thread-affine reactive state."
	)]
	fn into_event_handler(self) -> PageEventHandler {
		scoped_event_handler(move |event| {
			let _ = self.try_call(event);
		})
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
/// PageElement::new("button")
///     .on(EventType::Click, into_event_handler(|_| {}))
/// ```
pub fn into_event_handler<H: IntoEventHandler>(handler: H) -> PageEventHandler {
	handler.into_event_handler()
}

mod typed_sealed {
	pub trait Sealed<P> {}
}

/// Sealed conversion implemented only for synchronous `Fn(P)` and
/// [`Callback<P, ()>`] typed event handlers.
#[cfg(wasm)]
pub trait IntoTypedEventHandler<P>: typed_sealed::Sealed<P> + 'static {
	/// Converts this value into the internal typed callback.
	#[doc(hidden)]
	fn into_typed_event_handler(self) -> Arc<dyn Fn(P) + 'static>;
}

/// Sealed conversion implemented only for synchronous `Fn(P)` and
/// [`Callback<P, ()>`] typed event handlers.
#[cfg(native)]
pub trait IntoTypedEventHandler<P>: typed_sealed::Sealed<P> + 'static {
	/// Converts this value into the internal typed callback.
	#[doc(hidden)]
	fn into_typed_event_handler(self) -> Arc<dyn Fn(P) + 'static>;
}

#[cfg(wasm)]
impl<P, F> typed_sealed::Sealed<P> for F where F: Fn(P) + 'static {}

#[cfg(wasm)]
impl<P, F> IntoTypedEventHandler<P> for F
where
	F: Fn(P) + 'static,
{
	fn into_typed_event_handler(self) -> Arc<dyn Fn(P) + 'static> {
		Arc::new(self)
	}
}

#[cfg(native)]
impl<P, F> typed_sealed::Sealed<P> for F where F: Fn(P) + 'static {}

#[cfg(native)]
impl<P, F> IntoTypedEventHandler<P> for F
where
	F: Fn(P) + 'static,
{
	fn into_typed_event_handler(self) -> Arc<dyn Fn(P) + 'static> {
		Arc::new(self)
	}
}

impl<P> typed_sealed::Sealed<P> for Callback<P, ()> {}

#[cfg(wasm)]
impl<P: 'static> IntoTypedEventHandler<P> for Callback<P, ()> {
	fn into_typed_event_handler(self) -> Arc<dyn Fn(P) + 'static> {
		Arc::new(move |payload| {
			let _ = self.try_call(payload);
		})
	}
}

#[cfg(native)]
impl<P: 'static> IntoTypedEventHandler<P> for Callback<P, ()> {
	fn into_typed_event_handler(self) -> Arc<dyn Fn(P) + 'static> {
		Arc::new(move |payload| {
			let _ = self.try_call(payload);
		})
	}
}

/// Converts a synchronous typed payload handler into raw event storage.
pub fn typed_event_handler<P, H>(handler: H) -> PageEventHandler
where
	P: EventPayload,
	H: IntoTypedEventHandler<P>,
{
	let handler = handler.into_typed_event_handler();
	scoped_event_handler(move |event| {
		let _actual_type = crate::platform::event_type(&event);
		let _listener_target = crate::platform::current_target(&event)
			.map(|target| target.tag_name().to_owned())
			.unwrap_or_else(|| "<none>".to_owned());
		match P::try_from_raw(event) {
			Ok(payload) => handler(payload),
			Err(_error) => crate::error_log!(
				"typed event conversion failed for actual event type `{}` on listener target `{}`: {}",
				_actual_type,
				_listener_target,
				_error
			),
		}
	})
}

/// Converts a synchronous typed custom-event handler into raw event storage.
///
/// This is a P2 API. Both targets invoke the typed handler for each event;
/// native dispatches its stored event payload and WASM dispatches the browser
/// event received by the listener.
///
/// The handler is invoked for every raw event. Call [`CustomEvent::detail`]
/// inside it to inspect the lazily decoded custom-event payload.
pub fn typed_custom_event_handler<T, H>(handler: H) -> PageEventHandler
where
	T: DeserializeOwned + 'static,
	H: IntoTypedEventHandler<CustomEvent<T>>,
{
	let handler = handler.into_typed_event_handler();
	scoped_event_handler(move |event| handler(CustomEvent::from_raw(event)))
}

/// Converts an asynchronous typed payload handler into raw event storage.
#[cfg(wasm)]
pub fn typed_async_event_handler<P, H, Fut>(handler: H) -> PageEventHandler
where
	P: EventPayload,
	H: Fn(P) -> Fut + 'static,
	Fut: Future<Output = ()> + 'static,
{
	let scope = current_scope_id();
	Arc::new(move |event| {
		let _actual_type = crate::platform::event_type(&event);
		let _listener_target = crate::platform::current_target(&event)
			.map(|target| target.tag_name().to_owned())
			.unwrap_or_else(|| "<none>".to_owned());
		match P::try_from_raw(event) {
			Ok(payload) => {
				#[cfg(feature = "i18n")]
				let i18n_context = crate::i18n::current_i18n_callback_context();
				let future = handler(payload);
				#[cfg(feature = "i18n")]
				let future = crate::i18n::with_optional_i18n_context_async(i18n_context, future);
				spawn_task(scope_async_event_future(scope, future));
			}
			Err(_error) => crate::error_log!(
				"typed async event conversion failed for actual event type `{}` on listener target `{}`: {}",
				_actual_type,
				_listener_target,
				_error
			),
		}
	})
}

/// Converts an asynchronous typed custom-event handler into raw event storage.
///
/// This is a P2 API. Both targets schedule the typed handler asynchronously;
/// WASM preserves the listener's reactive scope while scheduling, while native
/// schedules the handler on the native task runner.
#[cfg(wasm)]
pub fn typed_async_custom_event_handler<T, H, Fut>(handler: H) -> PageEventHandler
where
	T: DeserializeOwned + 'static,
	H: Fn(CustomEvent<T>) -> Fut + 'static,
	Fut: Future<Output = ()> + 'static,
{
	let scope = current_scope_id();
	Arc::new(move |event| {
		#[cfg(feature = "i18n")]
		let i18n_context = crate::i18n::current_i18n_callback_context();
		let future = handler(CustomEvent::from_raw(event));
		#[cfg(feature = "i18n")]
		let future = crate::i18n::with_optional_i18n_context_async(i18n_context, future);
		spawn_task(scope_async_event_future(scope, future));
	})
}

/// Converts an asynchronous typed payload handler into raw event storage.
#[cfg(native)]
pub fn typed_async_event_handler<P, H, Fut>(handler: H) -> PageEventHandler
where
	P: EventPayload,
	H: Fn(P) -> Fut + 'static,
	Fut: Future<Output = ()> + 'static,
{
	Arc::new(move |event| {
		let _actual_type = crate::platform::event_type(&event);
		let _listener_target = crate::platform::current_target(&event)
			.map(|target| target.tag_name().to_owned())
			.unwrap_or_else(|| "<none>".to_owned());
		match P::try_from_raw(event) {
			Ok(payload) => spawn_task(handler(payload)),
			Err(_error) => crate::error_log!(
				"typed async event conversion failed for actual event type `{}` on listener target `{}`: {}",
				_actual_type,
				_listener_target,
				_error
			),
		}
	})
}

/// Converts an asynchronous typed custom-event handler into raw event storage.
///
/// This is a P2 API. Both targets schedule the typed handler asynchronously;
/// WASM preserves the listener's reactive scope while scheduling, while native
/// schedules the handler on the native task runner.
#[cfg(native)]
pub fn typed_async_custom_event_handler<T, H, Fut>(handler: H) -> PageEventHandler
where
	T: DeserializeOwned + 'static,
	H: Fn(CustomEvent<T>) -> Fut + 'static,
	Fut: Future<Output = ()> + 'static,
{
	Arc::new(move |event| {
		spawn_task(handler(CustomEvent::from_raw(event)));
	})
}

/// Stores a synchronous raw cross-target event handler.
#[cfg(wasm)]
pub fn raw_event_handler<H>(handler: H) -> PageEventHandler
where
	H: Fn(crate::platform::Event) + 'static,
{
	scoped_event_handler(handler)
}

/// Stores a synchronous raw cross-target event handler.
#[cfg(native)]
pub fn raw_event_handler<H>(handler: H) -> PageEventHandler
where
	H: Fn(crate::platform::Event) + 'static,
{
	scoped_event_handler(handler)
}

/// Stores an asynchronous raw cross-target event handler.
#[cfg(wasm)]
pub fn raw_async_event_handler<H, Fut>(handler: H) -> PageEventHandler
where
	H: Fn(crate::platform::Event) -> Fut + 'static,
	Fut: Future<Output = ()> + 'static,
{
	let scope = current_scope_id();
	Arc::new(move |event| {
		#[cfg(feature = "i18n")]
		let i18n_context = crate::i18n::current_i18n_callback_context();
		let future = handler(event);
		#[cfg(feature = "i18n")]
		let future = crate::i18n::with_optional_i18n_context_async(i18n_context, future);
		spawn_task(scope_async_event_future(scope, future));
	})
}

/// Stores an asynchronous raw cross-target event handler.
#[cfg(native)]
pub fn raw_async_event_handler<H, Fut>(handler: H) -> PageEventHandler
where
	H: Fn(crate::platform::Event) -> Fut + 'static,
	Fut: Future<Output = ()> + 'static,
{
	Arc::new(move |event| spawn_task(handler(event)))
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
	scoped_event_handler(f)
}

/// Event handler helper with concrete type for better type inference (server-side version).
///
/// See WASM version for documentation.
#[cfg(native)]
pub fn event_handler(f: impl Fn(crate::component::NativeEvent) + 'static) -> PageEventHandler {
	scoped_event_handler(f)
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
	let scope = current_scope_id();
	Arc::new(move |event| {
		#[cfg(feature = "i18n")]
		let i18n_context = crate::i18n::current_i18n_callback_context();
		let fut = f(event);
		#[cfg(feature = "i18n")]
		let fut = crate::i18n::with_optional_i18n_context_async(i18n_context, fut);
		spawn_task(scope_async_event_future(scope, fut));
	})
}

/// Creates an async event handler stub for non-WASM targets.
#[cfg(native)]
pub fn async_handler<F, Fut>(f: F) -> PageEventHandler
where
	F: Fn(crate::component::NativeEvent) -> Fut + 'static,
	Fut: Future<Output = ()> + 'static,
{
	Arc::new(move |event| {
		spawn_task(f(event));
	})
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;
	use serial_test::serial;

	#[rstest]
	fn callback_is_copy() {
		fn assert_copy<T: Copy>() {}

		assert_copy::<Callback<i32, i32>>();
	}

	#[rstest]
	#[serial(reactive_runtime)]
	fn callback_new_requires_an_active_scope() {
		let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
			let _callback = Callback::<i32, i32>::new(|value| value + 1);
		}));
		assert!(
			result.is_err(),
			"callbacks without an active owner must use Callback::new_in_scope"
		);
	}

	#[rstest]
	#[serial(reactive_runtime)]
	fn callback_new_in_scope_uses_a_disposable_owner() {
		let scope = ReactiveScope::new();
		let callback = Callback::new_in_scope(&scope, |value: i32| value + 1);

		assert_eq!(callback.call(1), 2);
		scope.dispose();

		assert!(
			callback.try_call(1).is_err(),
			"disposed callback owners must report their invalid slot without panicking"
		);
	}

	#[rstest]
	fn callback_call_works_inside_scope() {
		reinhardt_core::reactive::ReactiveScope::run(|| {
			let callback = Callback::new(|value: i32| value + 1);
			let copied = callback;
			assert_eq!(callback.call(1), 2);
			assert_eq!(copied.call(2), 3);
		});
	}

	#[rstest]
	#[serial(reactive_runtime)]
	fn callback_call_reenters_its_owning_scope() {
		let scope = reinhardt_core::reactive::ReactiveScope::new();
		let callback = scope.enter(|| {
			Callback::new(|_: ()| {
				let signal = reinhardt_core::reactive::Signal::new(42_i32);
				assert_eq!(signal.get(), 42);
			})
		});

		callback.call(());
	}

	#[test]
	fn test_callback_creation() {
		reinhardt_core::reactive::ReactiveScope::run(|| {
			let callback = Callback::new(|_: i32| 42);
			assert_eq!(callback.call(0), 42);
		});
	}

	#[test]
	fn test_callback_clone() {
		reinhardt_core::reactive::ReactiveScope::run(|| {
			let callback1 = Callback::new(|x: i32| x * 2);
			let callback2 = callback1;

			assert_eq!(callback1.call(5), 10);
			assert_eq!(callback2.call(5), 10);
		});
	}

	#[test]
	fn test_callback_with_captured_state() {
		use std::sync::{Arc, Mutex};

		reinhardt_core::reactive::ReactiveScope::run(|| {
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
		});
	}

	#[test]
	fn test_callback_debug() {
		reinhardt_core::reactive::ReactiveScope::run(|| {
			let callback = Callback::new(|_: ()| {});
			let debug_str = format!("{:?}", callback);
			assert!(debug_str.contains("Callback"));
		});
	}

	#[cfg(native)]
	#[test]
	fn test_into_event_handler_closure() {
		use crate::component::NativeEvent;

		let closure = |_: NativeEvent| {};
		let _handler: PageEventHandler = closure.into_event_handler();
	}

	#[cfg(native)]
	#[test]
	fn test_into_event_handler_callback() {
		reinhardt_core::reactive::ReactiveScope::run(|| {
			let callback = Callback::new(|_: crate::component::NativeEvent| {});
			let _handler: PageEventHandler = callback.into_event_handler();
		});
	}

	#[cfg(native)]
	#[test]
	fn test_into_event_handler_function() {
		use crate::component::NativeEvent;
		use reinhardt_core::types::page::{EventType, NativeEventPayload, PointerEventData};

		let handler: PageEventHandler = into_event_handler(|_: NativeEvent| {});
		// Verify it's callable
		handler(NativeEvent::for_known(
			EventType::Click,
			NativeEventPayload::Pointer(PointerEventData::default()),
		));
	}

	#[cfg(native)]
	#[test]
	fn synchronous_event_handler_reenters_its_registration_scope() {
		use crate::component::NativeEvent;
		use reinhardt_core::reactive::Signal;
		use reinhardt_core::types::page::{EventType, NativeEventPayload};

		let scope = ReactiveScope::new();
		let handler = scope.enter(|| {
			event_handler(|_: NativeEvent| {
				let signal = Signal::new(1_i32);
				assert_eq!(signal.get(), 1);
			})
		});

		handler(NativeEvent::for_known(
			EventType::Click,
			NativeEventPayload::default(),
		));
	}

	#[cfg(native)]
	#[rstest]
	fn typed_custom_adapter_always_invokes_handler() {
		use std::borrow::Cow;
		use std::sync::atomic::{AtomicUsize, Ordering};

		use reinhardt_core::types::page::{BaseEventData, NativeEventPayload};
		use reinhardt_event_catalog::EventName;
		use serde::Deserialize;

		use crate::event::CustomEventDetailError;

		#[derive(Deserialize)]
		struct SelectedDetail;

		fn raw_plain_custom_name() -> crate::component::NativeEvent {
			crate::component::NativeEvent::new(
				EventName::Custom(Cow::Borrowed("item-selected")),
				BaseEventData::default(),
				NativeEventPayload::default(),
			)
		}

		let calls = Arc::new(AtomicUsize::new(0));
		let handler = typed_custom_event_handler::<SelectedDetail, _>({
			let calls = Arc::clone(&calls);
			move |event: CustomEvent<SelectedDetail>| {
				assert!(matches!(
					event.detail(),
					Err(CustomEventDetailError::NotCustomEvent { .. })
				));
				calls.fetch_add(1, Ordering::SeqCst);
			}
		});
		handler(raw_plain_custom_name());
		assert_eq!(calls.load(Ordering::SeqCst), 1);
	}
}

#[cfg(all(test, wasm))]
mod wasm_tests {
	use super::*;
	use reinhardt_core::reactive::{ReactiveScope, Signal};
	use reinhardt_core::types::page::{EventType, PageElement};
	use std::cell::Cell;
	use std::rc::Rc;
	use wasm_bindgen_test::*;

	wasm_bindgen_test_configure!(run_in_browser);

	#[wasm_bindgen_test(async)]
	async fn async_handler_reenters_its_owner_scope_after_an_await() {
		let scope = ReactiveScope::new();
		let callback_ran = Rc::new(Cell::new(false));
		let handler = scope.enter(|| {
			let callback_ran = Rc::clone(&callback_ran);
			PageElement::new("button")
				.on(
					EventType::Click,
					async_handler(move |_| {
						let callback_ran = Rc::clone(&callback_ran);
						async move {
							crate::platform::defer_yield().await;
							let signal = Signal::new(1_i32);
							assert_eq!(signal.get(), 1);
							callback_ran.set(true);
						}
					}),
				)
				.into_event_handlers()
				.pop()
				.expect("button should retain its click handler")
				.1
		});

		handler(web_sys::Event::new("click").expect("click event should construct"));
		crate::platform::defer_yield().await;
		crate::platform::defer_yield().await;

		assert!(callback_ran.get());
	}
}

#[cfg(test)]
mod tests_with_deps {
	use super::*;
	use reinhardt_core::deps;
	use reinhardt_core::reactive::signal::Signal;
	use serial_test::serial;

	#[cfg(native)]
	fn callback_from_shared_call_site<Args, Ret>() -> Callback<Args, Ret>
	where
		Args: 'static,
		Ret: Default + 'static,
	{
		callback_with_deps(|_: Args| Ret::default(), deps![].into_deps())
	}

	// `callback_with_deps` keys its registry slot by the caller's
	// `(file, line, column)` via `#[track_caller]`. To exercise the slot
	// reuse path, both invocations MUST originate from the SAME source
	// line — accomplished by driving a loop over a single call site.

	#[cfg(native)]
	#[test]
	#[serial]
	fn callback_stable_when_deps_unchanged() {
		reinhardt_core::reactive::ReactiveScope::run(|| {
			// Arrange
			let s = Signal::new(0_i32);
			let mut previous = None;

			// Act — same call site (loop body) re-entered with same deps.
			for _ in 0..3 {
				let cb = callback_with_deps::<i32, ()>(
					{
						let s = s.clone();
						move |x: i32| {
							let _ = (x, s.get());
						}
					},
					deps![s].into_deps(),
				);
				let inner = cb.inner_arc();

				// Assert
				if let Some(previous) = previous.as_ref() {
					assert!(
						Arc::ptr_eq(&inner, previous),
						"Arc<Fn> identity must be stable when deps unchanged"
					);
				}
				previous = Some(inner);
			}
		});
	}

	#[cfg(native)]
	#[test]
	#[serial]
	fn callback_swaps_on_deps_change() {
		reinhardt_core::reactive::ReactiveScope::run(|| {
			// Arrange
			let signals: Vec<Signal<i32>> = (0..3).map(Signal::new).collect();
			let mut previous = None;

			// Act — same call site (loop body) re-entered with different
			// deps each iteration.
			for s in &signals {
				let cb = callback_with_deps::<i32, ()>(|_: i32| {}, deps![s].into_deps());
				let inner = cb.inner_arc();

				// Assert
				if let Some(previous) = previous.as_ref() {
					assert!(
						!Arc::ptr_eq(&inner, previous),
						"Arc<Fn> identity must change when deps NodeIds differ"
					);
				}
				previous = Some(inner);
			}
		});
	}

	#[cfg(native)]
	#[test]
	#[serial]
	fn callback_replaces_slot_when_call_site_signature_changes() {
		reinhardt_core::reactive::ReactiveScope::run(|| {
			let integer_callback = callback_from_shared_call_site::<i32, i32>();
			assert_eq!(integer_callback.call(1), 0);

			let string_callback = callback_from_shared_call_site::<String, String>();
			assert_eq!(string_callback.call(String::from("input")), "");
		});
	}

	#[cfg(native)]
	#[test]
	#[serial]
	fn callback_slots_are_isolated_between_live_scopes() {
		let first_scope = reinhardt_core::reactive::ReactiveScope::new();
		let first = first_scope.enter(callback_from_shared_call_site::<i32, i32>);
		let second_scope = reinhardt_core::reactive::ReactiveScope::new();
		let second = second_scope.enter(callback_from_shared_call_site::<i32, i32>);

		assert_eq!(first.call(1), 0);
		assert_eq!(second.call(2), 0);
	}

	#[cfg(native)]
	#[test]
	#[serial]
	fn callback_registry_removes_entries_when_their_scope_is_disposed() {
		let scope = reinhardt_core::reactive::ReactiveScope::new();
		let scope_id = scope.id();
		scope.enter(|| {
			let _ = callback_from_shared_call_site::<i32, i32>();
		});

		let has_entry = CALLBACK_REGISTRY.with(|registry| {
			registry
				.borrow()
				.keys()
				.any(|(_, entry_scope)| *entry_scope == scope_id)
		});
		assert!(has_entry);

		drop(scope);

		let has_entry = CALLBACK_REGISTRY.with(|registry| {
			registry
				.borrow()
				.keys()
				.any(|(_, entry_scope)| *entry_scope == scope_id)
		});
		assert!(!has_entry);
	}
}
