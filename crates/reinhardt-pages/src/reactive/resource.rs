//! Resource - Async data fetching primitive
//!
//! This module provides `Resource<T>`, a reactive primitive for handling async operations.
//! It manages Loading/Success/Error states and integrates with the Signal-based
//! reactivity system.

use super::{Effect, Signal};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::cell::Cell;
use std::fmt;
use std::future::Future;
use std::marker::PhantomData;
use std::pin::Pin;
use std::rc::Rc;
use std::task::{Context, Poll};

use crate::platform::{defer_yield, spawn_task};
use crate::reactive::pages_arena::{PageNodeKey, PageNodeKind, allocate_page_node, with_page_node};
#[cfg(native)]
use reinhardt_core::deps;
use reinhardt_core::reactive::{ScopeId, current_scope_id, scope::enter_scope};

use crate::reactive::ExplicitDeps;

type RefetchCallback = Rc<dyn Fn()>;

/// Polls a resource fetch future with the scope that owns its state active.
///
/// Browser task executors poll futures after their creator's render turn has
/// completed. Re-entering the owner scope keeps fetchers free to allocate
/// reactive primitives while still treating a disposed scope as cancellation.
struct ScopedResourceFuture<Fut> {
	scope: ScopeId,
	future: Pin<Box<Fut>>,
}

impl<Fut> Future for ScopedResourceFuture<Fut>
where
	Fut: Future<Output = ()>,
{
	type Output = ();

	fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
		let this = self.get_mut();
		let poll = || this.future.as_mut().poll(cx);
		enter_scope(this.scope, poll).unwrap_or(Poll::Ready(()))
	}
}

fn scope_resource_future<Fut>(scope: ScopeId, future: Fut) -> ScopedResourceFuture<Fut>
where
	Fut: Future<Output = ()> + 'static,
{
	ScopedResourceFuture {
		scope,
		future: Box::pin(future),
	}
}

struct ResourceSlot<T: Clone + 'static, E: Clone + 'static> {
	state: Signal<ResourceState<T, E>>,
	refetch_fn: RefetchCallback,
	ssr_key: Option<String>,
	_effect_guard: Effect,
}

#[cfg(any(wasm, test))]
thread_local! {
	static CLIENT_RESOURCE_COUNTER: Cell<usize> = const { Cell::new(0) };
}

/// Returns the current client call-order resource ID offset.
#[cfg(any(wasm, test))]
pub(crate) fn current_client_resource_counter() -> usize {
	CLIENT_RESOURCE_COUNTER.with(Cell::get)
}

/// Restores the client call-order resource ID offset.
#[cfg(any(wasm, test))]
pub(crate) fn set_client_resource_counter(value: usize) {
	CLIENT_RESOURCE_COUNTER.with(|counter| counter.set(value));
}

#[cfg(wasm)]
fn next_client_resource_key() -> String {
	CLIENT_RESOURCE_COUNTER.with(|counter| {
		let id = counter.get();
		counter.set(id + 1);
		format!("rh-res-{id}")
	})
}

#[cfg(any(wasm, test))]
pub(crate) fn reserve_client_resource_key(key: &str) {
	if let Some(id) = key.strip_prefix("rh-res-")
		&& let Ok(index) = id.parse::<usize>()
	{
		CLIENT_RESOURCE_COUNTER
			.with(|counter| counter.set(counter.get().max(index.saturating_add(1))));
	}
}

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
/// use reinhardt_pages::{deps, reactive::{Resource, use_resource}};
///
/// async fn fetch_user(id: u32) -> Result<User, String> {
///     // Fetch from API...
/// }
///
/// let resource = use_resource(|| fetch_user(42), deps![]);
///
/// match resource.get() {
///     ResourceState::Loading => println!("Loading..."),
///     ResourceState::Success(user) => println!("User: {:?}", user),
///     ResourceState::Error(err) => println!("Error: {}", err),
/// }
/// ```
pub struct Resource<T: Clone + 'static, E: Clone + 'static = String> {
	key: PageNodeKey,
	_marker: PhantomData<fn() -> (T, E)>,
}

impl<T: Clone + 'static, E: Clone + 'static> Clone for Resource<T, E> {
	fn clone(&self) -> Self {
		*self
	}
}

impl<T: Clone + 'static, E: Clone + 'static> Copy for Resource<T, E> {}

impl<T: Clone + 'static, E: Clone + 'static> Resource<T, E> {
	fn with_slot<R>(&self, f: impl FnOnce(&ResourceSlot<T, E>) -> R) -> R {
		with_page_node::<ResourceSlot<T, E>, _>(self.key, f).unwrap_or_else(|err| panic!("{err}"))
	}

	fn state(&self) -> Signal<ResourceState<T, E>> {
		self.with_slot(|slot| slot.state)
	}

	fn mark_ssr_read(&self) {
		#[cfg(native)]
		if let Some(key) = self.ssr_key() {
			crate::ssr::resource_context::mark_resource_read(&key);
		}
	}

	/// Get the current state of the resource
	///
	/// This method tracks the access for reactivity - any Effect or Memo
	/// that calls this will automatically re-run when the state changes.
	/// During SSR, a read outside an active Suspense boundary also keeps
	/// the resource in the external resolution set for the shell render.
	pub fn get(&self) -> ResourceState<T, E> {
		self.mark_ssr_read();
		self.state().get()
	}

	/// Update the resource state
	///
	/// This is typically used internally by the fetcher function.
	pub fn set(&self, new_state: ResourceState<T, E>) {
		self.state().set(new_state);
	}

	/// Trigger a refetch of the resource
	///
	/// This sets the state to Loading and re-executes the fetcher function.
	pub fn refetch(&self) {
		let Ok(refetch) =
			with_page_node::<ResourceSlot<T, E>, _>(self.key, |slot| Rc::clone(&slot.refetch_fn))
		else {
			return;
		};
		refetch();
	}

	/// Returns `true` if the resource is currently loading
	pub fn is_loading(&self) -> bool {
		self.mark_ssr_read();
		self.state().with_untracked(|s| s.is_loading())
	}

	/// Returns `true` if the resource has successfully loaded
	pub fn is_success(&self) -> bool {
		self.mark_ssr_read();
		self.state().with_untracked(|s| s.is_success())
	}

	/// Returns `true` if the resource failed to load
	pub fn is_error(&self) -> bool {
		self.mark_ssr_read();
		self.state().with_untracked(|s| s.is_error())
	}

	/// Returns this resource's deterministic SSR hydration key, if known.
	pub fn ssr_key(&self) -> Option<String> {
		self.with_slot(|slot| slot.ssr_key.clone())
	}
}

impl<T: Clone + 'static, E: Clone + 'static> reinhardt_core::reactive::deps::Trackable
	for Resource<T, E>
{
	/// Returns the underlying state `Signal`'s `NodeId`, allowing this
	/// `Resource` to participate in hook deps tuples alongside `Signal`
	/// and `Memo` (Refs #4195).
	fn node_id(&self) -> reinhardt_core::reactive::runtime::NodeId {
		self.state().id()
	}
}

/// Reactive async data hook — the resource counterpart of `use_effect`.
///
/// `use_resource(fetcher, deps)` runs `fetcher` and tracks its result as a
/// [`Resource`] (`Loading → Success/Error`). The `deps` argument follows the
/// same explicit dependency-list convention as [`use_effect`](super::hooks::use_effect):
///
/// - `deps![]` → fetch once on mount (never automatically refetches).
/// - `deps![signal]` / `deps![a, b]` → refetch whenever any listed dependency
///   changes. Dependencies are the explicitly listed [`Trackable`]s
///   (`Signal`/`Memo`/`Resource`); signals merely *read* inside the async
///   `fetcher` do not subscribe (they cross an `await` boundary), so list
///   everything that should drive a refetch — the same stale-deps rule as
///   `use_effect`.
///
/// The initial fetch and every dependency-driven refetch are deferred one
/// microtask (`defer_yield`) so they cannot hang when created during WASM
/// initialization before the event loop is running (#3316).
/// Each poll re-enters the resource's owning reactive scope, so fetchers may
/// allocate scoped primitives after an `await` without outliving their owner.
///
/// [`Trackable`]: reinhardt_core::reactive::deps::Trackable
///
/// # Dual-target behavior
///
/// Like [`use_action`](super::hooks::use_action), this hook is available on all
/// targets:
///
/// - **WASM**: the fetcher runs via `spawn_task` on the browser event loop.
/// - **Non-WASM (SSR)**: when called under [`SsrRenderer`](crate::ssr::SsrRenderer),
///   the fetcher is registered in the request-scoped SSR resource context. The
///   renderer awaits it up to [`SsrOptions::resource_timeout`](crate::ssr::SsrOptions::resource_timeout),
///   serializes resolved `Success`/`Error` state into the hydration payload,
///   and replays the render so the client can adopt the server value instead of
///   refetching on mount. Outside an SSR context, native `spawn_task` remains a
///   no-op and the resource stays `Loading`.
///
/// # Example
///
/// ```ignore
/// use reinhardt_pages::{deps, reactive::{Signal, use_resource}};
///
/// // Fetch once on mount:
/// let user = use_resource(|| async { fetch_user_from_api(42).await }, deps![]);
///
/// // Refetch whenever `user_id` changes:
/// let user_id = Signal::new(42u32);
/// let user = use_resource(
///     {
///         let user_id = user_id.clone();
///         move || {
///             let id = user_id.get();
///             async move { fetch_user_from_api(id).await }
///         }
///     },
///     deps![user_id],
/// );
/// user_id.set(100); // triggers a refetch
/// ```
pub fn use_resource<T, E, F, Fut>(fetcher: F, deps: ExplicitDeps) -> Resource<T, E>
where
	T: Clone + Serialize + DeserializeOwned + 'static,
	E: Clone + Serialize + DeserializeOwned + 'static,
	F: Fn() -> Fut + 'static,
	Fut: std::future::Future<Output = Result<T, E>> + 'static,
{
	use_resource_with_optional_key(None, fetcher, deps)
}

/// Reactive async data hook with an explicit SSR hydration key.
///
/// Prefer this when call-order keys would be unstable across server and client
/// renders, such as conditionally rendered resource hooks.
pub fn use_resource_with_key<K, T, E, F, Fut>(
	key: K,
	fetcher: F,
	deps: ExplicitDeps,
) -> Resource<T, E>
where
	K: Into<String>,
	T: Clone + Serialize + DeserializeOwned + 'static,
	E: Clone + Serialize + DeserializeOwned + 'static,
	F: Fn() -> Fut + 'static,
	Fut: std::future::Future<Output = Result<T, E>> + 'static,
{
	use_resource_with_optional_key(Some(key.into()), fetcher, deps)
}

fn use_resource_with_optional_key<T, E, F, Fut>(
	key: Option<String>,
	fetcher: F,
	deps: ExplicitDeps,
) -> Resource<T, E>
where
	T: Clone + Serialize + DeserializeOwned + 'static,
	E: Clone + Serialize + DeserializeOwned + 'static,
	F: Fn() -> Fut + 'static,
	Fut: std::future::Future<Output = Result<T, E>> + 'static,
{
	let fetcher = Rc::new(fetcher);

	#[cfg(native)]
	if let Some(resource) = try_create_ssr_resource(key.clone(), Rc::clone(&fetcher)) {
		return resource;
	}

	create_client_resource(key, fetcher, deps)
}

fn create_client_resource<T, E, F, Fut>(
	resource_key: Option<String>,
	fetcher: Rc<F>,
	deps: ExplicitDeps,
) -> Resource<T, E>
where
	T: Clone + Serialize + DeserializeOwned + 'static,
	E: Clone + Serialize + DeserializeOwned + 'static,
	F: Fn() -> Fut + 'static,
	Fut: std::future::Future<Output = Result<T, E>> + 'static,
{
	let ssr_key = resource_key.clone();

	#[cfg(wasm)]
	let initial_state = {
		let key = if let Some(key) = resource_key.clone() {
			reserve_client_resource_key(&key);
			key
		} else {
			next_client_resource_key()
		};
		hydrated_resource_state(&key)
	};

	#[cfg(native)]
	let initial_state = {
		let _ = resource_key;
		None
	};

	let run_initial_fetch = initial_state.is_none();
	let state = Signal::new(initial_state.unwrap_or(ResourceState::Loading));
	let owner_scope = current_scope_id().expect("resource state requires an active scope");
	let generation = Rc::new(Cell::new(0_u64));

	// Single fetch routine shared by the dependency-driven Effect and manual
	// refetch. `defer_yield` runs on every path (initial, dependency change, and
	// manual refetch) so the fetch cannot hang when spawned during WASM
	// initialization before the event loop ticks (#3316).
	let run: Rc<dyn Fn()> = {
		let fetcher = Rc::clone(&fetcher);
		let generation = Rc::clone(&generation);
		Rc::new(move || {
			state.set(ResourceState::Loading);
			let current_generation = generation.get().wrapping_add(1);
			generation.set(current_generation);
			let fetcher = Rc::clone(&fetcher);
			let generation = Rc::clone(&generation);
			spawn_task(scope_resource_future(owner_scope, async move {
				defer_yield().await;
				match fetcher().await {
					Ok(data) => {
						if generation.get() == current_generation {
							let _ = state.try_set(ResourceState::Success(data));
						}
					}
					Err(err) => {
						if generation.get() == current_generation {
							let _ = state.try_set(ResourceState::Error(err));
						}
					}
				}
			}));
		})
	};

	// Drive the initial fetch and dependency-change refetches. `new_with_deps`
	// (not `new`) means only the explicitly listed `deps` trigger re-runs; the
	// Effect is stored in the returned `Resource` (see `effect_guard`) so it
	// stays alive for the Resource's lifetime instead of being disposed on drop.
	build_resource_from_run(state, run, deps, run_initial_fetch, ssr_key)
}

fn build_resource_from_run<T, E>(
	state: Signal<ResourceState<T, E>>,
	run: Rc<dyn Fn()>,
	deps: ExplicitDeps,
	run_initial_fetch: bool,
	ssr_key: Option<String>,
) -> Resource<T, E>
where
	T: Clone + 'static,
	E: Clone + 'static,
{
	let first_run = Rc::new(Cell::new(true));
	let effect = {
		let run = Rc::clone(&run);
		let first_run = Rc::clone(&first_run);
		Effect::new_with_deps(
			move || {
				let is_first = first_run.replace(false);
				if run_initial_fetch || !is_first {
					run();
				}
				None::<fn()>
			},
			deps.into_deps(),
		)
	};

	let refetch_fn: RefetchCallback = Rc::new({
		let run = Rc::clone(&run);
		move || run()
	});

	Resource {
		key: allocate_page_node(
			"use_resource",
			PageNodeKind::Resource,
			ResourceSlot {
				state,
				refetch_fn,
				ssr_key,
				_effect_guard: effect,
			},
		),
		_marker: PhantomData,
	}
}

#[cfg(native)]
fn try_create_ssr_resource<T, E, F, Fut>(
	explicit_key: Option<String>,
	fetcher: Rc<F>,
) -> Option<Resource<T, E>>
where
	T: Clone + Serialize + DeserializeOwned + 'static,
	E: Clone + Serialize + DeserializeOwned + 'static,
	F: Fn() -> Fut + 'static,
	Fut: std::future::Future<Output = Result<T, E>> + 'static,
{
	crate::ssr::resource_context::with_active_context(|context| {
		let mut context = context.borrow_mut();
		let owner = crate::ssr::resource_context::current_render_owner();
		let key = if let Some(key) = explicit_key {
			context.reserve_call_order_key(&key);
			key
		} else {
			context.next_call_order_key()
		};
		if let Some(resolved_state) = context.resolved_resource_state::<T, E>(&key) {
			let state = Signal::new(resolved_state);
			let run: Rc<dyn Fn()> = Rc::new(move || state.set(ResourceState::Loading));
			build_resource_from_run(state, run, deps![], false, Some(key))
		} else {
			let state = Signal::new(ResourceState::Loading);
			context.register_resource_with_owner::<T, E, _, Fut>(
				key.clone(),
				{
					let fetcher = Rc::clone(&fetcher);
					move || fetcher()
				},
				state,
				owner,
			);

			let run: Rc<dyn Fn()> = Rc::new(move || state.set(ResourceState::Loading));

			build_resource_from_run(state, run, deps![], false, Some(key))
		}
	})
}

#[cfg(any(wasm, test))]
fn deserialize_resource_state<T, E>(value: &serde_json::Value) -> Option<ResourceState<T, E>>
where
	T: Clone + Serialize + DeserializeOwned + 'static,
	E: Clone + Serialize + DeserializeOwned + 'static,
{
	serde_json::from_value(value.clone()).ok()
}

#[cfg(wasm)]
fn hydrated_resource_state<T, E>(key: &str) -> Option<ResourceState<T, E>>
where
	T: Clone + Serialize + DeserializeOwned + 'static,
	E: Clone + Serialize + DeserializeOwned + 'static,
{
	let context = crate::hydration::HydrationContext::from_window().ok()?;
	let value = context.get_resource_state(key)?;
	deserialize_resource_state(value)
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;
	#[cfg(all(native, feature = "testing"))]
	use std::cell::RefCell;
	#[cfg(all(native, feature = "testing"))]
	use std::collections::VecDeque;
	#[cfg(all(native, feature = "testing"))]
	use std::task::Waker;

	#[rstest]
	fn resource_is_copy() {
		fn assert_copy<T: Copy>() {}

		assert_copy::<Resource<i32, String>>();
	}

	#[rstest]
	fn resource_state_reads_without_clone_handle() {
		reinhardt_core::reactive::ReactiveScope::run(|| {
			let resource: Resource<i32, String> = use_resource(|| async { Ok(1) }, crate::deps![]);
			let copied = resource;
			assert!(resource.is_loading());
			assert!(copied.is_loading());
		});
	}

	#[test]
	#[serial_test::serial(reactive_runtime)]
	fn stale_resource_refetch_is_a_no_op() {
		let scope = reinhardt_core::reactive::ReactiveScope::new();
		let resource: Resource<i32, String> =
			scope.enter(|| use_resource(|| async { Ok(1) }, crate::deps![]));

		scope.dispose();

		resource.refetch();
	}

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

	#[test]
	fn test_deserialize_resource_state() {
		let value = serde_json::json!({"Success": "server"});
		let state: ResourceState<String, String> = deserialize_resource_state(&value).unwrap();
		assert_eq!(state, ResourceState::Success("server".to_string()));
	}

	#[cfg(all(native, feature = "testing"))]
	#[test]
	#[serial_test::serial(reactive_runtime)]
	fn resource_fetcher_is_polled_in_its_owner_scope() {
		let queued = Rc::new(RefCell::new(None));
		let queued_for_sink = Rc::clone(&queued);
		let _task_sink = crate::platform::install_task_sink(move |task| {
			*queued_for_sink.borrow_mut() = Some(task);
		});
		let scope = reinhardt_core::reactive::ReactiveScope::new();
		let fetcher_ran = Rc::new(Cell::new(false));

		scope.enter(|| {
			let fetcher_ran = Rc::clone(&fetcher_ran);
			let _resource: Resource<i32, String> = use_resource(
				move || {
					let fetcher_ran = Rc::clone(&fetcher_ran);
					async move {
						let scoped_signal = Signal::new(1_i32);
						assert_eq!(scoped_signal.get(), 1);
						fetcher_ran.set(true);
						Ok(42)
					}
				},
				crate::deps![],
			);
		});

		let mut task = queued
			.borrow_mut()
			.take()
			.expect("resource creation should queue a native task");
		let mut context = Context::from_waker(Waker::noop());

		assert_eq!(task.as_mut().poll(&mut context), Poll::Ready(()));
		assert!(fetcher_ran.get());
	}

	#[cfg(all(native, feature = "testing"))]
	#[test]
	#[serial_test::serial(reactive_runtime)]
	fn stale_resource_result_cannot_replace_newer_result() {
		let tasks = Rc::new(RefCell::new(VecDeque::new()));
		let tasks_for_sink = Rc::clone(&tasks);
		let _task_sink = crate::platform::install_task_sink(move |task| {
			tasks_for_sink.borrow_mut().push_back(task);
		});
		let calls = Rc::new(Cell::new(0_u32));
		let scope = reinhardt_core::reactive::ReactiveScope::new();
		let resource = scope.enter(|| {
			let calls = Rc::clone(&calls);
			let resource: Resource<String, String> = use_resource(
				move || {
					let call = calls.get() + 1;
					calls.set(call);
					async move { Ok(format!("result-{call}")) }
				},
				crate::deps![],
			);
			resource.refetch();
			resource
		});

		assert_eq!(tasks.borrow().len(), 2);
		let mut context = Context::from_waker(Waker::noop());
		let newer = tasks.borrow_mut().pop_back().expect("refetch task");
		let mut newer = newer;
		assert_eq!(newer.as_mut().poll(&mut context), Poll::Ready(()));
		assert_eq!(
			resource.get(),
			ResourceState::Success("result-1".to_string())
		);

		let older = tasks.borrow_mut().pop_front().expect("initial task");
		let mut older = older;
		assert_eq!(older.as_mut().poll(&mut context), Poll::Ready(()));
		assert_eq!(
			resource.get(),
			ResourceState::Success("result-1".to_string())
		);
	}
}
