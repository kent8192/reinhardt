//! Context System for React-like Context API
//!
//! This module provides a Context system for passing data through the component
//! tree without prop drilling.
//!
//! See [`Context`] for architecture details and usage examples.
//!
//! ## Example
//!
//! ```ignore
//! use reinhardt_core::reactive::context::{Context, create_context, provide_context, get_context};
//!
//! // Create a context for theme
//! static THEME: Context<String> = create_context();
//!
//! // Provide a value
//! fn app() -> View {
//!     provide_context(&THEME, "dark".to_string());
//!     child_component()
//! }
//!
//! // Consume the value
//! fn child_component() -> View {
//!     let theme = get_context(&THEME).unwrap_or_else(|| "light".to_string());
//!     // ... use theme
//! }
//! ```

use core::any::Any;
use core::cell::RefCell;
use core::marker::PhantomData;
use core::sync::atomic::{AtomicUsize, Ordering};

extern crate alloc;
use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::vec::Vec;

/// Global counter for generating unique context IDs.
static CONTEXT_ID_COUNTER: AtomicUsize = AtomicUsize::new(0);

#[cfg_attr(doc, aquamarine::aquamarine)]
/// A type-safe context identifier.
///
/// Each Context has a unique ID that is used to look up values in the context stack.
/// Contexts are typically created as static items using `create_context()`.
///
/// ## Architecture
///
/// The context system uses a thread-local stack to manage context values:
///
/// ```mermaid
/// graph LR
///     subgraph ComponentTree["Component Tree"]
///         direction TB
///         App --> ThemeProvider
///         ThemeProvider --> UserCtx
///         UserCtx --> Child
///     end
///
///     subgraph ContextStack["Context Stack (Thread-Local)"]
///         direction TB
///         Theme["Theme: 'dark'"]
///         User["User: {id: 1}"]
///     end
///
///     ThemeProvider -. "provide_context()" .-> Theme
///     UserCtx -. "provide_context()" .-> User
///     Child -. "get_context()" .-> ContextStack
/// ```
///
/// ## Example
///
/// ```rust
/// use reinhardt_core::reactive::context::{Context, create_context};
///
/// # struct User;
/// // Create a context for user authentication
/// static AUTH_CONTEXT: Context<User> = create_context();
/// ```
pub struct Context<T: 'static> {
	id: usize,
	_phantom: PhantomData<T>,
}

impl<T: 'static> Context<T> {
	/// Creates a new context with a unique ID.
	pub fn new() -> Self {
		Self {
			id: CONTEXT_ID_COUNTER.fetch_add(1, Ordering::Relaxed),
			_phantom: PhantomData,
		}
	}

	/// Returns the unique ID of this context.
	pub fn id(&self) -> usize {
		self.id
	}
}

impl<T: 'static> Default for Context<T> {
	fn default() -> Self {
		Self::new()
	}
}

impl<T: 'static> Clone for Context<T> {
	fn clone(&self) -> Self {
		*self
	}
}

impl<T: 'static> Copy for Context<T> {}

impl<T: 'static> core::fmt::Debug for Context<T> {
	fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
		f.debug_struct("Context")
			.field("id", &self.id)
			.field("type", &core::any::type_name::<T>())
			.finish()
	}
}

/// Internal context value storage.
struct ContextValue {
	value: Box<dyn Any>,
}

// Thread-local context stack.
// Uses a BTreeMap where keys are context IDs and values are stacks of context values.
// This allows nested providers of the same context type.
thread_local! {
	static CONTEXT_STACK: RefCell<BTreeMap<usize, Vec<ContextValue>>> =
		const { RefCell::new(BTreeMap::new()) };
}

/// Creates a new context.
///
/// This is a convenience function for creating Context instances.
/// For static contexts, use `Context::new()` directly.
///
/// ## Example
///
/// ```rust
/// use reinhardt_core::reactive::context::create_context;
///
/// let theme_context = create_context::<String>();
/// ```
pub const fn create_context<T: 'static>() -> Context<T> {
	Context {
		id: 0, // Will be overwritten on first use
		_phantom: PhantomData,
	}
}

/// Provides a value for a context.
///
/// This pushes the value onto the context stack. The value will be available
/// to all descendant components via `get_context()` until `remove_context()`
/// is called or the provider is unmounted.
///
/// ## Arguments
///
/// * `ctx` - The context to provide a value for
/// * `value` - The value to provide
///
/// ## Example
///
/// ```rust
/// use reinhardt_core::reactive::context::{Context, provide_context};
///
/// let ctx: Context<i32> = Context::new();
/// provide_context(&ctx, 42);
/// ```
pub fn provide_context<T: Clone + 'static>(ctx: &Context<T>, value: T) {
	CONTEXT_STACK.with(|stack| {
		let mut stack = stack.borrow_mut();
		let values = stack.entry(ctx.id()).or_insert_with(Vec::new);
		values.push(ContextValue {
			value: Box::new(value),
		});
	});
}

/// Gets the current value from a context.
///
/// Returns the most recently provided value for the context, or `None` if
/// no value has been provided.
///
/// ## Arguments
///
/// * `ctx` - The context to get a value from
///
/// ## Returns
///
/// `Some(T)` if a value is provided, `None` otherwise
///
/// ## Example
///
/// ```rust
/// use reinhardt_core::reactive::context::{Context, provide_context, get_context};
///
/// let ctx: Context<i32> = Context::new();
/// provide_context(&ctx, 42);
///
/// let value = get_context(&ctx);
/// assert_eq!(value, Some(42));
/// ```
pub fn get_context<T: Clone + 'static>(ctx: &Context<T>) -> Option<T> {
	CONTEXT_STACK.with(|stack| {
		let stack = stack.borrow();
		stack.get(&ctx.id()).and_then(|values| {
			values
				.last()
				.and_then(|cv| cv.value.downcast_ref::<T>().cloned())
		})
	})
}

/// Removes the most recently provided value for a context.
///
/// This pops the value from the context stack. Should be called when a
/// provider component unmounts.
///
/// ## Arguments
///
/// * `ctx` - The context to remove a value from
///
/// ## Returns
///
/// `Some(T)` if a value was removed, `None` if the context had no values
pub fn remove_context<T: Clone + 'static>(ctx: &Context<T>) -> Option<T> {
	CONTEXT_STACK.with(|stack| {
		let mut stack = stack.borrow_mut();
		stack.get_mut(&ctx.id()).and_then(|values| {
			values
				.pop()
				.and_then(|cv| cv.value.downcast::<T>().ok().map(|b| *b))
		})
	})
}

/// Clears all context values.
///
/// This is primarily useful for testing to ensure a clean slate between tests.
///
/// ## Warning
///
/// Do not call this in production code as it will clear all context values
/// across the entire application.
#[doc(hidden)]
pub fn clear_all_contexts() {
	CONTEXT_STACK.with(|stack| {
		stack.borrow_mut().clear();
	});
}

/// A RAII guard that removes a context value when dropped.
///
/// This ensures that context values are properly cleaned up even if
/// a component panics.
pub struct ContextGuard<T: Clone + 'static> {
	ctx: Context<T>,
}

impl<T: Clone + 'static> ContextGuard<T> {
	/// Creates a new context guard that provides a value.
	pub fn new(ctx: &Context<T>, value: T) -> Self {
		provide_context(ctx, value);
		Self { ctx: *ctx }
	}
}

impl<T: Clone + 'static> Drop for ContextGuard<T> {
	fn drop(&mut self) {
		remove_context::<T>(&self.ctx);
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	fn setup() {
		clear_all_contexts();
	}

	#[test]
	fn test_context_creation() {
		let ctx1: Context<i32> = Context::new();
		let ctx2: Context<i32> = Context::new();

		// Each context should have a unique ID
		assert_ne!(ctx1.id(), ctx2.id());
	}

	#[test]
	fn test_provide_and_get_context() {
		setup();

		let ctx: Context<i32> = Context::new();

		assert_eq!(get_context(&ctx), None);

		provide_context(&ctx, 42);
		assert_eq!(get_context(&ctx), Some(42));
	}

	#[test]
	fn test_nested_contexts() {
		setup();

		let ctx: Context<alloc::string::String> = Context::new();

		provide_context(&ctx, alloc::string::String::from("outer"));
		assert_eq!(
			get_context(&ctx),
			Some(alloc::string::String::from("outer"))
		);

		provide_context(&ctx, alloc::string::String::from("inner"));
		assert_eq!(
			get_context(&ctx),
			Some(alloc::string::String::from("inner"))
		);

		remove_context::<alloc::string::String>(&ctx);
		assert_eq!(
			get_context(&ctx),
			Some(alloc::string::String::from("outer"))
		);

		remove_context::<alloc::string::String>(&ctx);
		assert_eq!(get_context(&ctx), None);
	}

	#[test]
	fn test_multiple_contexts() {
		setup();

		let ctx1: Context<i32> = Context::new();
		let ctx2: Context<alloc::string::String> = Context::new();

		provide_context(&ctx1, 42);
		provide_context(&ctx2, alloc::string::String::from("hello"));

		assert_eq!(get_context(&ctx1), Some(42));
		assert_eq!(
			get_context(&ctx2),
			Some(alloc::string::String::from("hello"))
		);
	}

	#[test]
	fn test_context_guard() {
		setup();

		let ctx: Context<i32> = Context::new();

		{
			let _guard = ContextGuard::new(&ctx, 42);
			assert_eq!(get_context(&ctx), Some(42));
		}

		// Guard dropped, value should be removed
		assert_eq!(get_context(&ctx), None);
	}

	#[test]
	fn test_context_clone() {
		let ctx1: Context<i32> = Context::new();
		let ctx2 = ctx1;

		assert_eq!(ctx1.id(), ctx2.id());
	}

	#[test]
	fn test_context_debug() {
		let ctx: Context<i32> = Context::new();
		let debug_str = alloc::format!("{:?}", ctx);
		assert!(debug_str.contains("Context"));
		assert!(debug_str.contains("i32"));
	}
}
