//! Resource - Async data fetching primitive
//!
//! This module provides `Resource<T>`, a reactive primitive for handling async operations.
//! Similar to Leptos's `create_resource`, it manages Loading/Success/Error states
//! and integrates with the Signal-based reactivity system.

use super::Signal;
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;

#[cfg(wasm)]
use crate::spawn::{defer_yield, spawn_task};

/// Type alias for the refetch callback function
///
/// This reduces type complexity for the `Resource` struct's `refetch_fn` field.
type RefetchCallback = Rc<RefCell<Option<Box<dyn Fn()>>>>;

/// State of a Resource
///
/// A Resource can be in one of three states:
/// - `Loading`: Initial state or during refetch
/// - `Success(T)`: Successfully fetched data
/// - `Error(E)`: Failed to fetch with error
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ResourceState<T, E> {
	/// Resource is currently loading
	Loading,
	/// Resource loaded successfully with data
	Success(T),
	/// Resource failed to load with error
	Error(E),
}

impl<T, E> ResourceState<T, E> {
	/// Returns `true` if the state is `Loading`
	pub fn is_loading(&self) -> bool {
		matches!(self, ResourceState::Loading)
	}

	/// Returns `true` if the state is `Success`
	pub fn is_success(&self) -> bool {
		matches!(self, ResourceState::Success(_))
	}

	/// Returns `true` if the state is `Error`
	pub fn is_error(&self) -> bool {
		matches!(self, ResourceState::Error(_))
	}

	/// Returns the success value if available
	pub fn as_ref(&self) -> Option<&T> {
		match self {
			ResourceState::Success(data) => Some(data),
			_ => None,
		}
	}

	/// Returns the error value if available
	pub fn error(&self) -> Option<&E> {
		match self {
			ResourceState::Error(err) => Some(err),
			_ => None,
		}
	}
}

impl<T: fmt::Display, E: fmt::Display> fmt::Display for ResourceState<T, E> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			ResourceState::Loading => write!(f, "Loading..."),
			ResourceState::Success(data) => write!(f, "{}", data),
			ResourceState::Error(err) => write!(f, "Error: {}", err),
		}
	}
}

/// Resource - Reactive async data container
///
/// A Resource manages async data fetching with automatic state management.
/// It tracks Loading/Success/Error states and integrates with the Signal system
/// for fine-grained reactivity.
///
/// # Example
///
/// ```ignore
/// use reinhardt_pages::reactive::{Resource, create_resource};
///
/// async fn fetch_user(id: u32) -> Result<User, String> {
///     // Fetch from API...
/// }
///
/// let resource = create_resource(|| fetch_user(42));
///
/// match resource.get() {
///     ResourceState::Loading => println!("Loading..."),
///     ResourceState::Success(user) => println!("User: {:?}", user),
///     ResourceState::Error(err) => println!("Error: {}", err),
/// }
/// ```
pub struct Resource<T: Clone + 'static, E: Clone + 'static = String> {
	state: Signal<ResourceState<T, E>>,
	refetch_fn: RefetchCallback,
}

impl<T: Clone + 'static, E: Clone + 'static> Clone for Resource<T, E> {
	fn clone(&self) -> Self {
		Resource {
			state: self.state.clone(),
			refetch_fn: Rc::clone(&self.refetch_fn),
		}
	}
}

impl<T: Clone + 'static, E: Clone + 'static> Resource<T, E> {
	/// Get the current state of the resource
	///
	/// This method tracks the access for reactivity - any Effect or Memo
	/// that calls this will automatically re-run when the state changes.
	pub fn get(&self) -> ResourceState<T, E> {
		self.state.get()
	}

	/// Update the resource state
	///
	/// This is typically used internally by the fetcher function.
	pub fn set(&self, new_state: ResourceState<T, E>) {
		self.state.set(new_state);
	}

	/// Trigger a refetch of the resource
	///
	/// This sets the state to Loading and re-executes the fetcher function.
	pub fn refetch(&self) {
		if let Some(ref refetch) = *self.refetch_fn.borrow() {
			refetch();
		}
	}

	/// Returns `true` if the resource is currently loading
	pub fn is_loading(&self) -> bool {
		self.get().is_loading()
	}

	/// Returns `true` if the resource has successfully loaded
	pub fn is_success(&self) -> bool {
		self.get().is_success()
	}

	/// Returns `true` if the resource failed to load
	pub fn is_error(&self) -> bool {
		self.get().is_error()
	}
}

/// Create a resource from an async function
///
/// This function takes an async fetcher and returns a `Resource<T>`.
/// The fetcher is immediately executed, and the Resource state transitions
/// from Loading → Success/Error based on the result.
///
/// # WASM-only
///
/// This function is only available on WASM targets, as it uses `spawn_local`
/// for async execution.
///
/// # Example
///
/// ```ignore
/// use reinhardt_pages::reactive::create_resource;
///
/// let user_resource = create_resource(|| async {
///     fetch_user_from_api(42).await
/// });
/// ```
#[cfg(wasm)]
pub fn create_resource<T, E, F, Fut>(fetcher: F) -> Resource<T, E>
where
	T: Clone + 'static,
	E: Clone + 'static,
	F: Fn() -> Fut + 'static,
	Fut: std::future::Future<Output = Result<T, E>> + 'static,
{
	let state = Signal::new(ResourceState::Loading);
	let refetch_fn: RefetchCallback = Rc::new(RefCell::new(None));

	let state_for_fetch = state.clone();
	let fetcher_for_initial = Rc::new(fetcher);
	let fetcher_for_refetch = Rc::clone(&fetcher_for_initial);

	// Defer initial fetch to the next microtask so the JS event loop can tick first.
	// Without this, JsFuture from fetch/Response.text() hangs when spawned during
	// WASM initialization (inside main()) before the event loop is running. (#3316)
	spawn_task({
		let state = state_for_fetch.clone();
		let fetcher = Rc::clone(&fetcher_for_initial);
		async move {
			defer_yield().await;
			match fetcher().await {
				Ok(data) => state.set(ResourceState::Success(data)),
				Err(err) => state.set(ResourceState::Error(err)),
			}
		}
	});

	// Setup refetch function
	let state_for_refetch = state.clone();
	*refetch_fn.borrow_mut() = Some(Box::new(move || {
		state_for_refetch.set(ResourceState::Loading);

		spawn_task({
			let state = state_for_refetch.clone();
			let fetcher = Rc::clone(&fetcher_for_refetch);
			async move {
				match fetcher().await {
					Ok(data) => state.set(ResourceState::Success(data)),
					Err(err) => state.set(ResourceState::Error(err)),
				}
			}
		});
	}));

	Resource { state, refetch_fn }
}

/// Create a resource with dependency tracking
///
/// This function creates a Resource that automatically refetches when
/// the dependency Signal changes. The fetcher receives the current value
/// of the dependency.
///
/// # WASM-only
///
/// This function is only available on WASM targets.
///
/// # Example
///
/// ```ignore
/// use reinhardt_pages::reactive::{Signal, create_resource_with_deps};
///
/// let user_id = Signal::new(42u32);
///
/// let user_resource = create_resource_with_deps(
///     user_id.clone(),
///     |id| async move {
///         fetch_user_from_api(id).await
///     }
/// );
///
/// // When user_id changes, the resource automatically refetches
/// user_id.set(100);
/// ```
#[cfg(wasm)]
pub fn create_resource_with_deps<T, E, D, F, Fut>(deps: Signal<D>, fetcher: F) -> Resource<T, E>
where
	T: Clone + 'static,
	E: Clone + 'static,
	D: Clone + PartialEq + 'static,
	F: Fn(D) -> Fut + 'static,
	Fut: std::future::Future<Output = Result<T, E>> + 'static,
{
	let state = Signal::new(ResourceState::Loading);
	let refetch_fn: RefetchCallback = Rc::new(RefCell::new(None));

	let fetcher = Rc::new(fetcher);
	let fetcher_for_effect = Rc::clone(&fetcher);
	let state_for_effect = state.clone();
	let deps_for_effect = deps.clone();

	// Setup effect to track dependency changes
	use super::Effect;
	Effect::new(move || {
		let deps_value = deps_for_effect.get();
		let state = state_for_effect.clone();
		let fetcher = Rc::clone(&fetcher_for_effect);

		state.set(ResourceState::Loading);

		spawn_task(async move {
			match fetcher(deps_value).await {
				Ok(data) => state.set(ResourceState::Success(data)),
				Err(err) => state.set(ResourceState::Error(err)),
			}
		});
	});

	// Setup refetch function (refetches with current deps value)
	let state_for_refetch = state.clone();
	let deps_for_refetch = deps.clone();
	let fetcher_for_refetch = Rc::clone(&fetcher);

	*refetch_fn.borrow_mut() = Some(Box::new(move || {
		state_for_refetch.set(ResourceState::Loading);

		spawn_task({
			let state = state_for_refetch.clone();
			let deps_value = deps_for_refetch.get();
			let fetcher = Rc::clone(&fetcher_for_refetch);
			async move {
				match fetcher(deps_value).await {
					Ok(data) => state.set(ResourceState::Success(data)),
					Err(err) => state.set(ResourceState::Error(err)),
				}
			}
		});
	}));

	Resource { state, refetch_fn }
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_resource_state_constructors() {
		let loading: ResourceState<String, String> = ResourceState::Loading;
		assert!(loading.is_loading());
		assert!(!loading.is_success());
		assert!(!loading.is_error());

		let success: ResourceState<String, String> = ResourceState::Success("data".to_string());
		assert!(!success.is_loading());
		assert!(success.is_success());
		assert!(!success.is_error());
		assert_eq!(success.as_ref(), Some(&"data".to_string()));

		let error: ResourceState<String, String> = ResourceState::Error("failed".to_string());
		assert!(!error.is_loading());
		assert!(!error.is_success());
		assert!(error.is_error());
		assert_eq!(error.error(), Some(&"failed".to_string()));
	}

	#[test]
	fn test_resource_state_display() {
		let loading: ResourceState<String, String> = ResourceState::Loading;
		assert_eq!(format!("{}", loading), "Loading...");

		let success: ResourceState<String, String> = ResourceState::Success("Hello".to_string());
		assert_eq!(format!("{}", success), "Hello");

		let error: ResourceState<String, String> =
			ResourceState::Error("Connection failed".to_string());
		assert_eq!(format!("{}", error), "Error: Connection failed");
	}
}
