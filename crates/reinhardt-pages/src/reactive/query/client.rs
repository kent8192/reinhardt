use std::any::Any;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::future::Future;
use std::marker::PhantomData;
use std::pin::Pin;
use std::rc::{Rc, Weak};
use std::task::{Context, Poll, Waker};
use std::time::Duration;

use futures_util::future::{AbortHandle, Abortable};
use serde::Serialize;
use serde::de::DeserializeOwned;

use super::super::Signal;
use super::super::resource::ResourceState;
use super::identity::{QueryDescriptor, QueryFetcher, QueryKey};
use super::runtime::{ScopedQueryFuture, duration_ms, now_ms, spawn_query_task};
use super::state::{QueryDefaults, QueryOptions};
use crate::cancellation::{AbortableTaskGuard, CancellationSource, scope_cancellation};
use reinhardt_core::reactive::ReactiveScope;

thread_local! {
	static QUERY_CACHE: RefCell<HashMap<String, CachedQueryEntry>> = RefCell::new(HashMap::new());
}

#[derive(Clone)]
struct CachedQueryEntry {
	typed: Rc<dyn Any>,
	refetch: Rc<dyn Fn()>,
}

/// Identifies the runtime consumer holding a query lease.
// These consumer variants are part of the internal loader contract; later
// navigation and prefetch phases construct the variants that are not used by
// the ordinary `use_query` hook yet.
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum QueryConsumer {
	Prefetch,
	Navigation(u64),
	MountedRoute(u64),
	MountedQuery,
	Maintenance,
}

/// Controls whether a failed fetch remains a reusable cache error.
// The discard policy is exercised by route loaders added in later tasks.
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum QueryErrorPolicy {
	Retain,
	Discard,
}

/// Options for an imperative query acquisition.
pub(crate) struct QueryAcquireOptions {
	pub consumer: QueryConsumer,
	pub error_policy: QueryErrorPolicy,
}

pub(super) struct QueryRequest<T, E> {
	pub(super) generation: u64,
	pub(super) source: CancellationSource,
	_guard: AbortableTaskGuard,
	_marker: PhantomData<fn() -> Result<T, E>>,
}

pub(super) struct QueryEntry<T: Clone + 'static, E: Clone + 'static> {
	pub(super) _scope: Rc<ReactiveScope>,
	pub(super) id: String,
	pub(super) state: Signal<ResourceState<T, E>>,
	pub(super) is_fetching: Signal<bool>,
	pub(super) request: RefCell<Option<QueryRequest<T, E>>>,
	next_generation: Cell<u64>,
	pub(super) completed: RefCell<Option<(u64, Result<T, E>)>>,
	waiters: RefCell<Vec<Waker>>,
	pub(super) lease_count: Cell<usize>,
	retain_lease_count: Cell<usize>,
	pub(super) refetch_after_in_flight: Cell<bool>,
	pub(super) last_fetched_ms: Cell<Option<u64>>,
	pub(super) stale_time: Cell<Duration>,
	pub(super) gc_time: Cell<Duration>,
	observers: RefCell<Vec<Weak<QueryLeaseInner<T, E>>>>,
}

pub(super) struct QueryLeaseInner<T: Clone + 'static, E: Clone + 'static> {
	pub(super) entry: Rc<QueryEntry<T, E>>,
	generation: Cell<Option<u64>>,
	retains_errors: bool,
	enabled: bool,
	fetcher: Rc<QueryFetcher<T, E>>,
}

/// RAII interest in one keyed query entry.
pub(crate) struct QueryLease<T: Clone + 'static, E: Clone + 'static> {
	pub(super) inner: Rc<QueryLeaseInner<T, E>>,
}

impl<T: Clone + 'static, E: Clone + 'static> Clone for QueryLease<T, E> {
	fn clone(&self) -> Self {
		Self {
			inner: Rc::clone(&self.inner),
		}
	}
}

impl<T: Clone + 'static, E: Clone + 'static> Drop for QueryLeaseInner<T, E> {
	fn drop(&mut self) {
		let entry = &self.entry;
		let remaining = entry.lease_count.get().saturating_sub(1);
		entry.lease_count.set(remaining);
		if self.retains_errors {
			let retained = entry.retain_lease_count.get().saturating_sub(1);
			entry.retain_lease_count.set(retained);
		}
		if remaining == 0 {
			entry.cancel_request();
		}
	}
}

pub(super) fn initial_query_state<T, E>(
	hydrated_state: Option<ResourceState<T, E>>,
) -> (ResourceState<T, E>, Option<u64>) {
	let initial_state = hydrated_state.unwrap_or(ResourceState::Loading);
	let last_fetched_ms = if matches!(
		&initial_state,
		ResourceState::Success(_) | ResourceState::Error(_)
	) {
		Some(now_ms())
	} else {
		None
	};
	(initial_state, last_fetched_ms)
}

impl<T: Clone + 'static, E: Clone + 'static> QueryEntry<T, E> {
	pub(super) fn new(descriptor: QueryDescriptor<T, E>, options: &QueryOptions) -> Self
	where
		T: Serialize + DeserializeOwned,
		E: Serialize + DeserializeOwned,
	{
		let (key, _fetcher, _ssr_prefetch) = descriptor.into_parts();
		let id = key.id();
		let hydrated_state = hydrated_query_state(&id);
		let defaults = QueryDefaults::default();
		Self::new_with_hydrated_state(
			key,
			hydrated_state,
			options.resolved_stale_time(&defaults),
			options.resolved_gc_time(&defaults),
		)
	}

	fn new_with_hydrated_state(
		key: QueryKey<T, E>,
		hydrated_state: Option<ResourceState<T, E>>,
		stale_time: Duration,
		gc_time: Duration,
	) -> Self {
		let (initial_state, last_fetched_ms) = initial_query_state(hydrated_state);
		let id = key.id();
		let scope = Rc::new(ReactiveScope::new());
		let (state, is_fetching) = scope.enter(|| (Signal::new(initial_state), Signal::new(false)));

		Self {
			_scope: scope,
			id,
			state,
			is_fetching,
			request: RefCell::new(None),
			next_generation: Cell::new(0),
			completed: RefCell::new(None),
			waiters: RefCell::new(Vec::new()),
			lease_count: Cell::new(0),
			retain_lease_count: Cell::new(0),
			refetch_after_in_flight: Cell::new(false),
			last_fetched_ms: Cell::new(last_fetched_ms),
			stale_time: Cell::new(stale_time),
			gc_time: Cell::new(gc_time),
			observers: RefCell::new(Vec::new()),
		}
	}

	fn update_policy(&self, stale_time: Duration, gc_time: Duration) {
		self.stale_time.set(stale_time);
		self.gc_time.set(gc_time);
	}

	pub(super) fn is_stale(&self) -> bool {
		let Some(last_fetched_ms) = self.last_fetched_ms.get() else {
			return true;
		};
		now_ms().saturating_sub(last_fetched_ms) >= duration_ms(self.stale_time.get())
	}

	pub(super) fn should_fetch_on_mount(&self) -> bool {
		self.state
			.with_untracked(|state| matches!(state, ResourceState::Loading) || self.is_stale())
	}

	pub(super) fn has_request(&self) -> bool {
		self.request.borrow().is_some()
	}

	fn next_request_generation(&self) -> u64 {
		let generation = self.next_generation.get();
		self.next_generation.set(generation.wrapping_add(1));
		generation
	}

	fn cancel_request(&self) {
		if let Some(request) = self.request.borrow_mut().take() {
			request.source.cancel();
			self.refetch_after_in_flight.set(false);
			self.is_fetching.set(false);
			self.wake_waiters();
		}
	}

	fn wake_waiters(&self) {
		let waiters = std::mem::take(&mut *self.waiters.borrow_mut());
		for waiter in waiters {
			waiter.wake();
		}
	}

	// The lease result future registers here while a navigation waits for a
	// generation to settle; later loader tasks will exercise this path.
	#[allow(dead_code)]
	fn register_waiter(&self, waker: &Waker) {
		let mut waiters = self.waiters.borrow_mut();
		if !waiters.iter().any(|previous| previous.will_wake(waker)) {
			waiters.push(waker.clone());
		}
	}

	pub(super) fn make_lease(
		self: &Rc<Self>,
		generation: Option<u64>,
		error_policy: QueryErrorPolicy,
		fetcher: Rc<QueryFetcher<T, E>>,
		enabled: bool,
	) -> QueryLease<T, E> {
		self.lease_count.set(self.lease_count.get() + 1);
		let retains_errors = error_policy == QueryErrorPolicy::Retain;
		if retains_errors {
			self.retain_lease_count
				.set(self.retain_lease_count.get() + 1);
		}
		let inner = Rc::new(QueryLeaseInner {
			entry: Rc::clone(self),
			generation: Cell::new(generation),
			retains_errors,
			enabled,
			fetcher,
		});
		self.observers.borrow_mut().push(Rc::downgrade(&inner));
		QueryLease { inner }
	}

	fn acquire(
		self: &Rc<Self>,
		fetcher: Rc<QueryFetcher<T, E>>,
		options: QueryAcquireOptions,
		query_options: QueryOptions,
	) -> QueryLease<T, E>
	where
		T: Serialize + DeserializeOwned,
		E: Serialize + DeserializeOwned,
	{
		let _consumer = options.consumer;
		let enabled = query_options.is_enabled();
		let should_fetch = if !enabled || self.has_request() {
			false
		} else if options.error_policy == QueryErrorPolicy::Retain {
			self.should_fetch_on_mount()
		} else {
			match self.state.with_untracked(|state| state.clone()) {
				ResourceState::Success(_) => self.is_stale(),
				ResourceState::Error(_) => true,
				ResourceState::Loading => true,
			}
		};
		// Register interest before starting work. Native test execution may poll
		// a ready fetch synchronously, and completion must observe this lease when
		// deciding whether an error is retainable or whether invalidation queues a
		// follow-up request.
		let lease = self.make_lease(None, options.error_policy, fetcher, enabled);
		let generation = if should_fetch {
			Some(self.start_fetch(false))
		} else {
			self.request
				.borrow()
				.as_ref()
				.map(|request| request.generation)
		};
		lease.inner.generation.set(generation);
		lease
	}

	fn selected_fetcher(&self) -> Option<Rc<QueryFetcher<T, E>>> {
		let mut observers = self.observers.borrow_mut();
		observers.retain(|observer| observer.strong_count() > 0);
		observers
			.iter()
			.filter_map(Weak::upgrade)
			.find(|observer| observer.enabled)
			.map(|observer| Rc::clone(&observer.fetcher))
	}

	#[cfg(native)]
	pub(super) fn mark_resolved_fetched(&self) {
		if self.state.with_untracked(|state| {
			matches!(state, ResourceState::Success(_) | ResourceState::Error(_))
		}) {
			self.last_fetched_ms.set(Some(now_ms()));
		}
	}

	pub(super) fn start_fetch(self: &Rc<Self>, force: bool) -> u64 {
		if self.has_request() {
			if force {
				self.refetch_after_in_flight.set(true);
			}
			return self
				.request
				.borrow()
				.as_ref()
				.map(|request| request.generation)
				.unwrap_or_default();
		}

		let had_success = self
			.state
			.with_untracked(|state| matches!(state, ResourceState::Success(_)));
		if !force && had_success && !self.is_stale() {
			return self.next_generation.get();
		}
		let Some(fetcher) = self.selected_fetcher() else {
			return self.next_generation.get();
		};
		let generation = self.next_request_generation();
		let source = CancellationSource::new();
		let token = source.handle();
		let (abort_handle, abort_registration) = AbortHandle::new_pair();
		let guard = AbortableTaskGuard::new(abort_handle);
		*self.request.borrow_mut() = Some(QueryRequest {
			generation,
			source,
			_guard: guard,
			_marker: PhantomData,
		});
		self.is_fetching.set(true);
		if !had_success {
			self.state.set(ResourceState::Loading);
		}

		let entry = Rc::clone(self);
		let scope = entry._scope.id();
		let fetch_cancellation = token.clone();
		let scoped = ScopedQueryFuture {
			scope,
			future: Box::pin(async move {
				let result = scope_cancellation(token, fetcher(fetch_cancellation)).await;
				entry.complete_fetch(generation, result);
			}),
		};
		spawn_query_task(async move {
			let _ = Abortable::new(scoped, abort_registration).await;
		});
		generation
	}

	pub(super) fn complete_fetch(self: &Rc<Self>, generation: u64, result: Result<T, E>) {
		let cancelled = self
			.request
			.borrow()
			.as_ref()
			.map(|request| request.source.handle().is_cancelled())
			.unwrap_or(true);
		let matches_request = self
			.request
			.borrow()
			.as_ref()
			.is_some_and(|request| request.generation == generation);
		if cancelled || !matches_request {
			return;
		}
		self.request.borrow_mut().take();
		self.completed
			.borrow_mut()
			.replace((generation, result.clone()));
		match result {
			Ok(value) => {
				self.last_fetched_ms.set(Some(now_ms()));
				self.state.set(ResourceState::Success(value));
			}
			Err(error) => {
				if self.retain_lease_count.get() > 0 {
					self.last_fetched_ms.set(Some(now_ms()));
				} else {
					self.last_fetched_ms.set(None);
				}
				self.state.set(ResourceState::Error(error));
			}
		}
		self.is_fetching.set(false);
		self.wake_waiters();
		if self.refetch_after_in_flight.replace(false) && self.lease_count.get() > 0 {
			self.start_fetch(true);
		}
	}
}

// Route preparation consumes this future in later implementation tasks.
#[allow(dead_code)]
struct QueryResultFuture<T: Clone + 'static, E: Clone + 'static> {
	entry: Rc<QueryEntry<T, E>>,
	generation: Option<u64>,
}

impl<T: Clone + 'static, E: Clone + 'static> Future for QueryResultFuture<T, E> {
	type Output = Result<T, E>;

	fn poll(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
		let this = self.get_mut();
		if let Some(generation) = this.generation {
			if let Some((completed_generation, result)) = this.entry.completed.borrow().as_ref()
				&& *completed_generation == generation
			{
				return Poll::Ready(result.clone());
			}
		} else {
			match this.entry.state.with_untracked(|state| state.clone()) {
				ResourceState::Success(value) => return Poll::Ready(Ok(value)),
				ResourceState::Error(error) => return Poll::Ready(Err(error)),
				ResourceState::Loading => {}
			}
		}
		this.entry.register_waiter(context.waker());
		Poll::Pending
	}
}

impl<T: Clone + 'static, E: Clone + 'static> QueryLease<T, E> {
	// Route preparation consumes this result operation in later implementation
	// tasks; keep it available while the public hook remains synchronous.
	#[allow(dead_code)]
	pub(crate) async fn result(&self) -> Result<T, E> {
		QueryResultFuture {
			entry: Rc::clone(&self.inner.entry),
			generation: self.inner.generation.get(),
		}
		.await
	}

	// Route preparation reads the settled state when a loader joins cached work.
	#[allow(dead_code)]
	pub(crate) fn state(&self) -> ResourceState<T, E> {
		self.inner.entry.state.with_untracked(|state| state.clone())
	}
}

pub(crate) fn acquire_query<T, E>(
	descriptor: QueryDescriptor<T, E>,
	options: QueryAcquireOptions,
) -> QueryLease<T, E>
where
	T: Clone + Serialize + DeserializeOwned + 'static,
	E: Clone + Serialize + DeserializeOwned + 'static,
{
	acquire_query_with_options(descriptor, options, QueryOptions::default())
}

pub(super) fn acquire_query_with_options<T, E>(
	descriptor: QueryDescriptor<T, E>,
	options: QueryAcquireOptions,
	query_options: QueryOptions,
) -> QueryLease<T, E>
where
	T: Clone + Serialize + DeserializeOwned + 'static,
	E: Clone + Serialize + DeserializeOwned + 'static,
{
	let defaults = QueryDefaults::default();
	let stale_time = query_options.resolved_stale_time(&defaults);
	let gc_time = query_options.resolved_gc_time(&defaults);
	let (key, fetcher, _ssr_prefetch) = descriptor.into_parts();
	query_entry_for_key(key, stale_time, gc_time).acquire(fetcher, options, query_options)
}

pub(crate) fn seed_query_from_serialized<T, E>(
	descriptor: QueryDescriptor<T, E>,
	serialized: &serde_json::Value,
) -> Result<(), serde_json::Error>
where
	T: Clone + Serialize + DeserializeOwned + 'static,
	E: Clone + Serialize + DeserializeOwned + 'static,
{
	let hydrated_state = serde_json::from_value(serialized.clone())?;
	let defaults = QueryDefaults::default();
	let (key, _fetcher, _ssr_prefetch) = descriptor.into_parts();
	let id = key.id();
	#[cfg(any(wasm, test))]
	super::super::resource::reserve_client_resource_key(&id);
	let cache_id = scoped_query_cache_id(&id);
	QUERY_CACHE.with(|cache| {
		let mut cache = cache.borrow_mut();
		if let Some(cached) = cache.get(&cache_id) {
			let _entry = Rc::clone(&cached.typed)
				.downcast::<QueryEntry<T, E>>()
				.unwrap_or_else(|_| {
					panic!("query cache key `{id}` was reused with incompatible types")
				});
			return;
		}

		let entry = Rc::new(QueryEntry::new_with_hydrated_state(
			key,
			Some(hydrated_state),
			defaults.resolved_stale_time(),
			defaults.resolved_gc_time(),
		));
		cache.insert(
			cache_id,
			CachedQueryEntry {
				typed: entry.clone(),
				refetch: Rc::new({
					let entry = Rc::clone(&entry);
					move || {
						entry.start_fetch(true);
					}
				}),
			},
		);
	});
	Ok(())
}

#[cfg(test)]
pub(super) fn query_entry<T, E>(descriptor: QueryDescriptor<T, E>) -> Rc<QueryEntry<T, E>>
where
	T: Clone + Serialize + DeserializeOwned + 'static,
	E: Clone + Serialize + DeserializeOwned + 'static,
{
	let defaults = QueryDefaults::default();
	let (key, _fetcher, _ssr_prefetch) = descriptor.into_parts();
	query_entry_for_key(
		key,
		defaults.resolved_stale_time(),
		defaults.resolved_gc_time(),
	)
}

fn query_entry_for_key<T, E>(
	key: QueryKey<T, E>,
	stale_time: Duration,
	gc_time: Duration,
) -> Rc<QueryEntry<T, E>>
where
	T: Clone + Serialize + DeserializeOwned + 'static,
	E: Clone + Serialize + DeserializeOwned + 'static,
{
	let id = key.id();
	#[cfg(any(wasm, test))]
	super::super::resource::reserve_client_resource_key(&id);
	let cache_id = scoped_query_cache_id(&id);
	QUERY_CACHE.with(|cache| {
		let mut cache = cache.borrow_mut();
		if let Some(cached) = cache.get(&cache_id) {
			let entry = Rc::clone(&cached.typed)
				.downcast::<QueryEntry<T, E>>()
				.unwrap_or_else(|_| {
					panic!("query cache key `{id}` was reused with incompatible types")
				});
			entry.update_policy(stale_time, gc_time);
			return entry;
		}

		let hydrated_state = hydrated_query_state(&id);
		let entry = Rc::new(QueryEntry::new_with_hydrated_state(
			key,
			hydrated_state,
			stale_time,
			gc_time,
		));
		cache.insert(
			cache_id,
			CachedQueryEntry {
				typed: entry.clone(),
				refetch: Rc::new({
					let entry = Rc::clone(&entry);
					move || {
						entry.start_fetch(true);
					}
				}),
			},
		);
		entry
	})
}

pub(super) fn invalidate_query_id(id: &str) {
	let cache_id = scoped_query_cache_id(id);
	QUERY_CACHE.with(|cache| {
		if let Some(cached) = cache.borrow().get(&cache_id) {
			(cached.refetch)();
		}
	});
}

pub(super) fn scoped_query_cache_id(id: &str) -> String {
	#[cfg(all(native, feature = "testing"))]
	if let Some(scope_id) = crate::testing::component::active_query_scope_id() {
		return format!("test-screen:{scope_id}:{id}");
	}

	id.to_string()
}

#[cfg(wasm)]
pub(super) fn hydrated_query_state<T, E>(key: &str) -> Option<ResourceState<T, E>>
where
	T: Clone + Serialize + DeserializeOwned + 'static,
	E: Clone + Serialize + DeserializeOwned + 'static,
{
	let context = crate::hydration::HydrationContext::from_window().ok()?;
	let value = context.get_resource_state(key)?;
	serde_json::from_value(value.clone()).ok()
}

#[cfg(not(wasm))]
pub(super) fn hydrated_query_state<T, E>(_key: &str) -> Option<ResourceState<T, E>>
where
	T: Clone + Serialize + DeserializeOwned + 'static,
	E: Clone + Serialize + DeserializeOwned + 'static,
{
	None
}

#[cfg(all(test, not(wasm)))]
pub(crate) fn clear_query_cache_for_test() {
	QUERY_CACHE.with(|cache| cache.borrow_mut().clear());
}
