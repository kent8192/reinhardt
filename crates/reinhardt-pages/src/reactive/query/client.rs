use std::any::Any;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
#[cfg(all(test, not(wasm)))]
use std::collections::VecDeque;
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
use super::identity::{
	QueryDescriptor, QueryFamily, QueryFamilyTypes, QueryFetcher, QueryIdentity, QueryKey,
};
use super::runtime::{ScopedQueryFuture, duration_ms, now_ms, spawn_query_task};
use super::state::{QueryDefaults, QueryOptions};
#[cfg(any(wasm, test))]
use super::state::{QuerySnapshot, QueryStatus};
use crate::cancellation::{AbortableTaskGuard, CancellationSource, scope_cancellation};
use reinhardt_core::reactive::ReactiveScope;

type QueryTask = Pin<Box<dyn Future<Output = ()> + 'static>>;

struct WeakClientQueryFuture<Fut> {
	owner: Weak<QueryClientInner>,
	future: Pin<Box<Fut>>,
}

impl<Fut> Future for WeakClientQueryFuture<Fut>
where
	Fut: Future<Output = ()>,
{
	type Output = ();

	fn poll(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
		let this = self.get_mut();
		let Some(inner) = this.owner.upgrade() else {
			return Poll::Ready(());
		};
		let client = QueryClient { inner };
		super::context::with_query_client(&client, || this.future.as_mut().poll(context))
	}
}

pub(crate) trait QueryRuntime {
	fn now_ms(&self) -> u64;
	fn spawn(&self, task: QueryTask);

	fn executes_inline(&self) -> bool {
		false
	}
}

pub(crate) type QueryRuntimeHandle = Rc<dyn QueryRuntime>;

struct PlatformQueryRuntime;

impl QueryRuntime for PlatformQueryRuntime {
	fn now_ms(&self) -> u64 {
		now_ms()
	}

	fn spawn(&self, task: QueryTask) {
		spawn_query_task(task);
	}
}

fn platform_query_runtime() -> QueryRuntimeHandle {
	Rc::new(PlatformQueryRuntime)
}

struct SsrQueryRuntime;

impl QueryRuntime for SsrQueryRuntime {
	fn now_ms(&self) -> u64 {
		now_ms()
	}

	fn spawn(&self, _task: QueryTask) {
		panic!("SSR query work must be polled through its owning query lease")
	}

	fn executes_inline(&self) -> bool {
		true
	}
}

#[cfg(all(test, not(wasm)))]
#[derive(Clone)]
pub(crate) struct TestQueryRuntime {
	inner: Rc<TestQueryRuntimeInner>,
}

#[cfg(all(test, not(wasm)))]
struct TestQueryRuntimeInner {
	now_ms: Cell<u64>,
	tasks: RefCell<VecDeque<QueryTask>>,
}

#[cfg(all(test, not(wasm)))]
impl TestQueryRuntime {
	pub(crate) fn new() -> Self {
		Self {
			inner: Rc::new(TestQueryRuntimeInner {
				now_ms: Cell::new(0),
				tasks: RefCell::new(VecDeque::new()),
			}),
		}
	}

	pub(crate) fn handle(&self) -> QueryRuntimeHandle {
		Rc::clone(&self.inner) as QueryRuntimeHandle
	}

	pub(crate) fn run_until_stalled(&self) {
		let mut context = Context::from_waker(Waker::noop());
		loop {
			let task_count = self.inner.tasks.borrow().len();
			if task_count == 0 {
				break;
			}
			let mut completed = 0;
			for _ in 0..task_count {
				let Some(mut task) = self.inner.tasks.borrow_mut().pop_front() else {
					break;
				};
				match task.as_mut().poll(&mut context) {
					Poll::Ready(()) => completed += 1,
					Poll::Pending => self.inner.tasks.borrow_mut().push_back(task),
				}
			}
			if completed == 0 && self.inner.tasks.borrow().len() == task_count {
				break;
			}
		}
	}

	pub(crate) fn pending_task_count(&self) -> usize {
		self.inner.tasks.borrow().len()
	}
}

#[cfg(all(test, not(wasm)))]
impl QueryRuntime for TestQueryRuntimeInner {
	fn now_ms(&self) -> u64 {
		self.now_ms.get()
	}

	fn spawn(&self, task: QueryTask) {
		self.tasks.borrow_mut().push_back(task);
	}
}

/// Application-owned keyed query cache and runtime.
#[derive(Clone)]
pub struct QueryClient {
	inner: Rc<QueryClientInner>,
}

struct QueryClientInner {
	defaults: QueryDefaults,
	runtime: QueryRuntimeHandle,
	entries: RefCell<HashMap<QueryIdentity, CachedQueryEntry>>,
	families: RefCell<HashMap<&'static str, QueryFamilyTypes>>,
}

#[derive(Clone)]
struct CachedQueryEntry {
	family_id: &'static str,
	typed: Rc<dyn Any>,
	invalidate: Rc<dyn Fn()>,
	cancel: Rc<dyn Fn()>,
}

impl QueryClient {
	/// Creates an empty client using the platform query runtime.
	pub fn new(defaults: QueryDefaults) -> Self {
		Self::with_runtime(defaults, platform_query_runtime())
	}

	/// Creates an empty request-owned client whose work is polled inline.
	pub fn new_ssr(defaults: QueryDefaults) -> Self {
		Self::with_runtime(defaults, Rc::new(SsrQueryRuntime))
	}

	pub(crate) fn with_runtime(defaults: QueryDefaults, runtime: QueryRuntimeHandle) -> Self {
		Self {
			inner: Rc::new(QueryClientInner {
				defaults,
				runtime,
				entries: RefCell::new(HashMap::new()),
				families: RefCell::new(HashMap::new()),
			}),
		}
	}

	pub(crate) fn observe<T, E>(
		&self,
		descriptor: QueryDescriptor<T, E>,
		options: QueryOptions,
	) -> super::hook::QueryHandle<T, E>
	where
		T: Clone + Serialize + DeserializeOwned + 'static,
		E: Clone + Serialize + DeserializeOwned + 'static,
	{
		super::hook::observe_query(self, descriptor, options)
	}

	fn register_family(&self, family_id: &'static str, actual: QueryFamilyTypes) {
		let mut families = self.inner.families.borrow_mut();
		let Some(expected) = families.get(&family_id) else {
			families.insert(family_id, actual);
			return;
		};
		if expected.matches(&actual) {
			return;
		}
		panic!(
			"incompatible query family types for `{family_id}`: expected Args=`{}`, data=`{}`, error=`{}`; actual Args=`{}`, data=`{}`, error=`{}`",
			expected.arguments_name,
			expected.data_name,
			expected.error_name,
			actual.arguments_name,
			actual.data_name,
			actual.error_name,
		);
	}

	pub(super) fn same_instance(&self, other: &Self) -> bool {
		Rc::ptr_eq(&self.inner, &other.inner)
	}
}

impl Drop for QueryClientInner {
	fn drop(&mut self) {
		for entry in self.entries.get_mut().values() {
			(entry.cancel)();
		}
		self.entries.get_mut().clear();
	}
}

/// Identifies the runtime consumer holding a query lease.
// Some variants are reserved for configuration-specific route and maintenance
// paths, so not every build constructs all of them.
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

pub(super) struct QueryRequest<T: Clone + 'static, E: Clone + 'static> {
	pub(super) generation: u64,
	invalidation_generation: u64,
	manual_observer: Option<Weak<QueryLeaseInner<T, E>>>,
	pub(super) source: CancellationSource,
	_guard: AbortableTaskGuard,
	_marker: PhantomData<fn() -> Result<T, E>>,
}

pub(super) struct QueryEntry<T: Clone + 'static, E: Clone + 'static> {
	pub(super) _scope: Rc<ReactiveScope>,
	pub(super) hydration_id: String,
	family_id: &'static str,
	pub(super) state: Signal<ResourceState<T, E>>,
	pub(super) refetch_error: Signal<Option<E>>,
	pub(super) is_fetching: Signal<bool>,
	pub(super) request: RefCell<Option<QueryRequest<T, E>>>,
	next_generation: Cell<u64>,
	invalidation_generation: Cell<u64>,
	invalidated: Cell<bool>,
	pub(super) completed: RefCell<Option<(u64, Result<T, E>)>>,
	waiters: RefCell<Vec<Waker>>,
	pub(super) lease_count: Cell<usize>,
	retain_lease_count: Cell<usize>,
	pub(super) refetch_after_in_flight: Cell<bool>,
	queued_manual_refetch: RefCell<Option<Weak<QueryLeaseInner<T, E>>>>,
	pub(super) last_fetched_ms: Cell<Option<u64>>,
	observers: RefCell<Vec<Weak<QueryLeaseInner<T, E>>>>,
	runtime: QueryRuntimeHandle,
	owner: Option<Weak<QueryClientInner>>,
	inline_task: RefCell<Option<QueryTask>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ObserverPolicy {
	pub(super) enabled: bool,
	pub(super) stale_time: Duration,
	// Task 7 consumes the resolved observer retention duration when it adds GC.
	#[allow(dead_code)]
	pub(super) gc_time: Duration,
	pub(super) refetch_interval: Option<Duration>,
}

impl ObserverPolicy {
	pub(super) fn resolve(options: &QueryOptions, defaults: &QueryDefaults) -> Self {
		Self {
			enabled: options.is_enabled(),
			stale_time: options.resolved_stale_time(defaults),
			gc_time: options.resolved_gc_time(defaults),
			refetch_interval: options.refetch_interval_value(),
		}
	}
}

pub(super) struct QueryLeaseInner<T: Clone + 'static, E: Clone + 'static> {
	pub(super) entry: Rc<QueryEntry<T, E>>,
	generation: Cell<Option<u64>>,
	retains_errors: bool,
	consumer: QueryConsumer,
	pub(super) policy: ObserverPolicy,
	pub(super) fetcher: Rc<QueryFetcher<T, E>>,
	pub(super) manual_refetch_pending: Cell<bool>,
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

#[cfg(test)]
pub(super) fn initial_query_state<T, E>(
	hydrated_state: Option<ResourceState<T, E>>,
) -> (ResourceState<T, E>, Option<u64>) {
	initial_query_state_at(hydrated_state, now_ms())
}

fn initial_query_state_at<T, E>(
	hydrated_state: Option<ResourceState<T, E>>,
	now_ms: u64,
) -> (ResourceState<T, E>, Option<u64>) {
	let initial_state = hydrated_state.unwrap_or(ResourceState::Loading);
	let last_fetched_ms = if matches!(
		&initial_state,
		ResourceState::Success(_) | ResourceState::Error(_)
	) {
		Some(now_ms)
	} else {
		None
	};
	(initial_state, last_fetched_ms)
}

impl<T: Clone + 'static, E: Clone + 'static> QueryEntry<T, E> {
	#[cfg(test)]
	pub(super) fn new(descriptor: QueryDescriptor<T, E>) -> Self
	where
		T: Serialize + DeserializeOwned,
		E: Serialize + DeserializeOwned,
	{
		let (key, _fetcher, _ssr_prefetch, _family_types) = descriptor.into_parts();
		Self::new_with_hydrated_state(key, None, platform_query_runtime(), None)
	}

	fn new_with_hydrated_state(
		key: QueryKey<T, E>,
		hydrated_state: Option<ResourceState<T, E>>,
		runtime: QueryRuntimeHandle,
		owner: Option<Weak<QueryClientInner>>,
	) -> Self {
		let (initial_state, last_fetched_ms) =
			initial_query_state_at(hydrated_state, runtime.now_ms());
		let hydration_id = key.hydration_id();
		let family_id = key.family_id();
		let scope = Rc::new(ReactiveScope::new());
		let (state, refetch_error, is_fetching) = scope.enter(|| {
			(
				Signal::new(initial_state),
				Signal::new(None),
				Signal::new(false),
			)
		});

		Self {
			_scope: scope,
			hydration_id,
			family_id,
			state,
			refetch_error,
			is_fetching,
			request: RefCell::new(None),
			next_generation: Cell::new(0),
			invalidation_generation: Cell::new(0),
			invalidated: Cell::new(false),
			completed: RefCell::new(None),
			waiters: RefCell::new(Vec::new()),
			lease_count: Cell::new(0),
			retain_lease_count: Cell::new(0),
			refetch_after_in_flight: Cell::new(false),
			queued_manual_refetch: RefCell::new(None),
			last_fetched_ms: Cell::new(last_fetched_ms),
			observers: RefCell::new(Vec::new()),
			runtime,
			owner,
			inline_task: RefCell::new(None),
		}
	}

	pub(super) fn is_stale(&self, stale_time: Duration) -> bool {
		if self.invalidated.get() {
			return true;
		}
		let Some(last_fetched_ms) = self.last_fetched_ms.get() else {
			return true;
		};
		self.runtime.now_ms().saturating_sub(last_fetched_ms) >= duration_ms(stale_time)
	}

	pub(super) fn should_fetch_on_mount(&self, stale_time: Duration) -> bool {
		self.state.with_untracked(|state| match state {
			ResourceState::Loading => true,
			ResourceState::Success(_) => self.is_stale(stale_time),
			ResourceState::Error(_) => self.invalidated.get(),
		})
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
		self.inline_task.borrow_mut().take();
		if let Some(request) = self.request.borrow_mut().take() {
			request.source.cancel();
			Self::clear_manual_refetch(request.manual_observer);
			Self::clear_manual_refetch(self.queued_manual_refetch.borrow_mut().take());
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

	// A loader result future registers here while navigation waits for its
	// request generation to settle.
	fn register_waiter(&self, waker: &Waker) {
		let mut waiters = self.waiters.borrow_mut();
		if !waiters.iter().any(|previous| previous.will_wake(waker)) {
			waiters.push(waker.clone());
		}
	}

	pub(super) fn make_lease(
		self: &Rc<Self>,
		generation: Option<u64>,
		consumer: QueryConsumer,
		error_policy: QueryErrorPolicy,
		fetcher: Rc<QueryFetcher<T, E>>,
		policy: ObserverPolicy,
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
			consumer,
			policy,
			fetcher,
			manual_refetch_pending: Cell::new(false),
		});
		self.observers.borrow_mut().push(Rc::downgrade(&inner));
		QueryLease { inner }
	}

	fn acquire(
		self: &Rc<Self>,
		fetcher: Rc<QueryFetcher<T, E>>,
		options: QueryAcquireOptions,
		policy: ObserverPolicy,
	) -> QueryLease<T, E>
	where
		T: Serialize + DeserializeOwned,
		E: Serialize + DeserializeOwned,
	{
		let should_fetch = if !policy.enabled || self.has_request() {
			false
		} else if options.error_policy == QueryErrorPolicy::Retain {
			self.should_fetch_on_mount(policy.stale_time)
		} else {
			match self.state.with_untracked(|state| state.clone()) {
				ResourceState::Success(_) => self.is_stale(policy.stale_time),
				ResourceState::Error(_) => true,
				ResourceState::Loading => true,
			}
		};
		// Register interest before starting work. Native test execution may poll
		// a ready fetch synchronously, and completion must observe this lease when
		// deciding whether an error is retainable or whether invalidation queues a
		// follow-up request.
		let lease = self.make_lease(
			None,
			options.consumer,
			options.error_policy,
			fetcher,
			policy,
		);
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
			.find(|observer| observer.policy.enabled)
			.map(|observer| Rc::clone(&observer.fetcher))
	}

	fn has_active_invalidation_interest(&self) -> bool {
		let mut observers = self.observers.borrow_mut();
		observers.retain(|observer| observer.strong_count() > 0);
		observers.iter().filter_map(Weak::upgrade).any(|observer| {
			observer.policy.enabled
				&& matches!(
					observer.consumer,
					QueryConsumer::MountedQuery | QueryConsumer::Maintenance
				)
		})
	}

	pub(super) fn start_fetch(self: &Rc<Self>, force: bool) -> u64 {
		self.start_fetch_with(force, None)
	}

	pub(super) fn start_observer_refetch(
		self: &Rc<Self>,
		observer: &Rc<QueryLeaseInner<T, E>>,
	) -> u64 {
		observer.manual_refetch_pending.set(true);
		self.start_fetch_with(true, Some(Rc::downgrade(observer)))
	}

	fn queue_manual_refetch(&self, observer: Weak<QueryLeaseInner<T, E>>) {
		let mut queued = self.queued_manual_refetch.borrow_mut();
		if let Some(previous) = queued.as_ref()
			&& !Weak::ptr_eq(previous, &observer)
			&& let Some(previous) = previous.upgrade()
		{
			previous.manual_refetch_pending.set(false);
		}
		*queued = Some(observer);
	}

	fn clear_manual_refetch(observer: Option<Weak<QueryLeaseInner<T, E>>>) {
		if let Some(observer) = observer.and_then(|observer| observer.upgrade()) {
			observer.manual_refetch_pending.set(false);
		}
	}

	fn start_fetch_with(
		self: &Rc<Self>,
		force: bool,
		manual_observer: Option<Weak<QueryLeaseInner<T, E>>>,
	) -> u64 {
		if self.has_request() {
			if force {
				self.refetch_after_in_flight.set(true);
			}
			if let Some(observer) = manual_observer {
				self.queue_manual_refetch(observer);
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
		let manual_observer = manual_observer.and_then(|observer| observer.upgrade());
		let fetcher = self.selected_fetcher().or_else(|| {
			manual_observer
				.as_ref()
				.map(|observer| Rc::clone(&observer.fetcher))
		});
		let Some(fetcher) = fetcher else {
			if let Some(observer) = manual_observer {
				observer.manual_refetch_pending.set(false);
			}
			return self.next_generation.get();
		};
		let generation = self.next_request_generation();
		let invalidation_generation = self.invalidation_generation.get();
		let source = CancellationSource::new();
		let token = source.handle();
		let (abort_handle, abort_registration) = AbortHandle::new_pair();
		let guard = AbortableTaskGuard::new(abort_handle);
		*self.request.borrow_mut() = Some(QueryRequest {
			generation,
			invalidation_generation,
			manual_observer: manual_observer.as_ref().map(Rc::downgrade),
			source,
			_guard: guard,
			_marker: PhantomData,
		});
		self.is_fetching.set(true);
		if had_success {
			self.refetch_error.set(None);
		} else {
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
		let task = async move {
			let _ = Abortable::new(scoped, abort_registration).await;
		};
		let task: QueryTask = if let Some(owner) = self.owner.clone() {
			Box::pin(WeakClientQueryFuture {
				owner,
				future: Box::pin(task),
			})
		} else {
			Box::pin(task)
		};
		if self.runtime.executes_inline() {
			self.inline_task.borrow_mut().replace(task);
		} else {
			self.runtime.spawn(task);
		}
		generation
	}

	pub(super) fn complete_fetch(self: &Rc<Self>, generation: u64, result: Result<T, E>) {
		let request = self.request.borrow();
		let cancelled = request
			.as_ref()
			.map(|request| request.source.handle().is_cancelled())
			.unwrap_or(true);
		let matches_request = request
			.as_ref()
			.is_some_and(|request| request.generation == generation);
		if cancelled || !matches_request {
			return;
		}
		let request_invalidation_generation = request
			.as_ref()
			.map(|request| request.invalidation_generation)
			.unwrap_or_default();
		let manual_observer = request
			.as_ref()
			.and_then(|request| request.manual_observer.clone());
		drop(request);
		let had_success = self
			.state
			.with_untracked(|state| matches!(state, ResourceState::Success(_)));
		self.request.borrow_mut().take();
		self.completed
			.borrow_mut()
			.replace((generation, result.clone()));
		match result {
			Ok(value) => {
				self.last_fetched_ms.set(Some(self.runtime.now_ms()));
				self.refetch_error.set(None);
				self.state.set(ResourceState::Success(value));
				if self.invalidation_generation.get() == request_invalidation_generation {
					self.invalidated.set(false);
				}
			}
			Err(error) => {
				if had_success {
					self.refetch_error.set(Some(error));
				} else {
					self.refetch_error.set(None);
					self.state.set(ResourceState::Error(error));
				}
				if !had_success && self.retain_lease_count.get() > 0 {
					self.last_fetched_ms.set(Some(self.runtime.now_ms()));
					if self.invalidation_generation.get() == request_invalidation_generation {
						self.invalidated.set(false);
					}
				} else if !had_success {
					self.last_fetched_ms.set(None);
				}
			}
		}
		self.is_fetching.set(false);
		self.wake_waiters();
		let invalidated_during_request =
			self.invalidation_generation.get() > request_invalidation_generation;
		let manual_refetch_queued = self.refetch_after_in_flight.replace(false);
		let queued_manual_observer = self.queued_manual_refetch.borrow_mut().take();
		let same_observer_is_queued = manual_observer.as_ref().is_some_and(|active| {
			queued_manual_observer
				.as_ref()
				.is_some_and(|queued| Weak::ptr_eq(active, queued))
		});
		if !same_observer_is_queued {
			Self::clear_manual_refetch(manual_observer);
		}
		if let Some(observer) = queued_manual_observer
			&& observer.strong_count() > 0
			&& self.lease_count.get() > 0
		{
			self.start_fetch_with(true, Some(observer));
		} else if (manual_refetch_queued
			|| (invalidated_during_request && self.has_active_invalidation_interest()))
			&& self.lease_count.get() > 0
		{
			self.start_fetch(true);
		}
	}

	fn invalidate(self: &Rc<Self>) {
		self.invalidated.set(true);
		self.invalidation_generation
			.set(self.invalidation_generation.get().wrapping_add(1));
		if !self.has_request() && self.has_active_invalidation_interest() {
			self.start_fetch(true);
		}
	}
}

struct QueryResultFuture<T: Clone + 'static, E: Clone + 'static> {
	entry: Rc<QueryEntry<T, E>>,
	generation: Option<u64>,
}

impl<T: Clone + 'static, E: Clone + 'static> Future for QueryResultFuture<T, E> {
	type Output = Result<T, E>;

	fn poll(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
		let this = self.get_mut();
		let inline_task = this.entry.inline_task.borrow_mut().take();
		if let Some(mut task) = inline_task
			&& task.as_mut().poll(context).is_pending()
			&& this.entry.has_request()
		{
			this.entry.inline_task.borrow_mut().replace(task);
		}
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
	// Route preparation awaits this result while the public hook remains
	// synchronous.
	pub(crate) async fn result(&self) -> Result<T, E> {
		QueryResultFuture {
			entry: Rc::clone(&self.inner.entry),
			generation: self.inner.generation.get(),
		}
		.await
	}

	// Hydration regressions inspect the settled state without creating a public snapshot API.
	#[cfg(test)]
	pub(crate) fn state(&self) -> ResourceState<T, E> {
		self.inner.entry.state.with_untracked(|state| state.clone())
	}
}

#[cfg(test)]
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

#[cfg(test)]
pub(super) fn acquire_query_with_options<T, E>(
	descriptor: QueryDescriptor<T, E>,
	options: QueryAcquireOptions,
	query_options: QueryOptions,
) -> QueryLease<T, E>
where
	T: Clone + Serialize + DeserializeOwned + 'static,
	E: Clone + Serialize + DeserializeOwned + 'static,
{
	super::context::queries().acquire_with_options(descriptor, options, query_options)
}

#[cfg(test)]
pub(super) fn query_entry<T, E>(descriptor: QueryDescriptor<T, E>) -> Rc<QueryEntry<T, E>>
where
	T: Clone + Serialize + DeserializeOwned + 'static,
	E: Clone + Serialize + DeserializeOwned + 'static,
{
	super::context::queries().entry_for_descriptor(descriptor)
}

impl QueryClient {
	pub(crate) fn acquire<T, E>(
		&self,
		descriptor: QueryDescriptor<T, E>,
		options: QueryAcquireOptions,
	) -> QueryLease<T, E>
	where
		T: Clone + Serialize + DeserializeOwned + 'static,
		E: Clone + Serialize + DeserializeOwned + 'static,
	{
		self.acquire_with_options(descriptor, options, QueryOptions::default())
	}

	pub(super) fn acquire_with_options<T, E>(
		&self,
		descriptor: QueryDescriptor<T, E>,
		options: QueryAcquireOptions,
		query_options: QueryOptions,
	) -> QueryLease<T, E>
	where
		T: Clone + Serialize + DeserializeOwned + 'static,
		E: Clone + Serialize + DeserializeOwned + 'static,
	{
		let policy = ObserverPolicy::resolve(&query_options, &self.inner.defaults);
		let (key, fetcher, _ssr_prefetch, family_types) = descriptor.into_parts();
		self.entry_for_key(key, family_types)
			.acquire(fetcher, options, policy)
	}

	pub(crate) fn seed_serialized<T, E>(
		&self,
		key: QueryKey<T, E>,
		serialized: &serde_json::Value,
	) -> Result<(), serde_json::Error>
	where
		T: Clone + Serialize + DeserializeOwned + 'static,
		E: Clone + Serialize + DeserializeOwned + 'static,
	{
		let hydrated_state = serde_json::from_value(serialized.clone())?;
		self.register_family(key.family_id(), key.family_types());
		let id = key.id();
		#[cfg(any(wasm, test))]
		super::super::resource::reserve_client_resource_key(&id);
		let identity = key.identity().clone();
		let mut entries = self.inner.entries.borrow_mut();
		if let Some(cached) = entries.get(&identity) {
			let _entry = Rc::clone(&cached.typed)
				.downcast::<QueryEntry<T, E>>()
				.unwrap_or_else(|_| {
					panic!("query cache key `{id}` was reused with incompatible types")
				});
			return Ok(());
		}

		let entry = Rc::new(QueryEntry::new_with_hydrated_state(
			key,
			Some(hydrated_state),
			Rc::clone(&self.inner.runtime),
			Some(Rc::downgrade(&self.inner)),
		));
		entries.insert(identity, cached_query_entry(&entry));
		Ok(())
	}

	#[cfg(any(wasm, test))]
	pub(crate) fn seed_query_snapshot<T, E>(
		&self,
		key: QueryKey<T, E>,
		serialized: &serde_json::Value,
	) -> Result<(), serde_json::Error>
	where
		T: Clone + Serialize + DeserializeOwned + 'static,
		E: Clone + Serialize + DeserializeOwned + 'static,
	{
		let snapshot: QuerySnapshot<T, E> = serde_json::from_value(serialized.clone())?;
		if snapshot.is_fetching {
			return Err(invalid_hydration_snapshot(
				"settled query hydration snapshot is still fetching",
			));
		}
		let refetch_error = snapshot.refetch_error;
		let hydrated_state = match snapshot.status {
			QueryStatus::Success => {
				let value = snapshot.data.ok_or_else(|| {
					invalid_hydration_snapshot("successful query snapshot is missing data")
				})?;
				if snapshot.error.is_some() {
					return Err(invalid_hydration_snapshot(
						"successful query snapshot contains an initial error",
					));
				}
				ResourceState::Success(value)
			}
			QueryStatus::Error => {
				if refetch_error.is_some() {
					return Err(invalid_hydration_snapshot(
						"initial error query snapshot contains a refetch error",
					));
				}
				let error = snapshot.error.ok_or_else(|| {
					invalid_hydration_snapshot("error query snapshot is missing its error")
				})?;
				if snapshot.data.is_some() {
					return Err(invalid_hydration_snapshot(
						"error query snapshot contains successful data",
					));
				}
				ResourceState::Error(error)
			}
			QueryStatus::Idle | QueryStatus::Pending => {
				if snapshot.data.is_some() || snapshot.error.is_some() || refetch_error.is_some() {
					return Err(invalid_hydration_snapshot(
						"unsettled query snapshot contains settled state",
					));
				}
				ResourceState::Loading
			}
		};
		self.register_family(key.family_id(), key.family_types());
		let id = key.id();
		#[cfg(any(wasm, test))]
		super::super::resource::reserve_client_resource_key(&id);
		let identity = key.identity().clone();
		let mut entries = self.inner.entries.borrow_mut();
		if let Some(cached) = entries.get(&identity) {
			Rc::clone(&cached.typed)
				.downcast::<QueryEntry<T, E>>()
				.unwrap_or_else(|_| {
					panic!("query cache key `{id}` was reused with incompatible types")
				});
			return Ok(());
		}

		let entry = Rc::new(QueryEntry::new_with_hydrated_state(
			key,
			Some(hydrated_state),
			Rc::clone(&self.inner.runtime),
			Some(Rc::downgrade(&self.inner)),
		));
		entry.refetch_error.set(refetch_error);
		entries.insert(identity, cached_query_entry(&entry));
		Ok(())
	}

	#[cfg(test)]
	fn entry_for_descriptor<T, E>(&self, descriptor: QueryDescriptor<T, E>) -> Rc<QueryEntry<T, E>>
	where
		T: Clone + Serialize + DeserializeOwned + 'static,
		E: Clone + Serialize + DeserializeOwned + 'static,
	{
		let (key, _fetcher, _ssr_prefetch, family_types) = descriptor.into_parts();
		self.entry_for_key(key, family_types)
	}

	fn entry_for_key<T, E>(
		&self,
		key: QueryKey<T, E>,
		family_types: QueryFamilyTypes,
	) -> Rc<QueryEntry<T, E>>
	where
		T: Clone + Serialize + DeserializeOwned + 'static,
		E: Clone + Serialize + DeserializeOwned + 'static,
	{
		self.register_family(key.family_id(), family_types);
		let id = key.id();
		#[cfg(any(wasm, test))]
		super::super::resource::reserve_client_resource_key(&id);
		let identity = key.identity().clone();
		let mut entries = self.inner.entries.borrow_mut();
		if let Some(cached) = entries.get(&identity) {
			let entry = Rc::clone(&cached.typed)
				.downcast::<QueryEntry<T, E>>()
				.unwrap_or_else(|_| {
					panic!("query cache key `{id}` was reused with incompatible types")
				});
			return entry;
		}

		let entry = Rc::new(QueryEntry::new_with_hydrated_state(
			key,
			None,
			Rc::clone(&self.inner.runtime),
			Some(Rc::downgrade(&self.inner)),
		));
		entries.insert(identity, cached_query_entry(&entry));
		entry
	}

	/// Marks one exact typed query stale and refetches it when actively enabled.
	pub fn invalidate<T, E>(&self, key: &QueryKey<T, E>) {
		self.register_family(key.family_id(), key.family_types());
		if let Some(cached) = self.inner.entries.borrow().get(key.identity()) {
			(cached.invalidate)();
		}
	}

	/// Marks all cached entries in one typed query family stale.
	pub fn invalidate_family<Args: 'static, T: 'static, E: 'static>(
		&self,
		family: QueryFamily<Args, T, E>,
	) {
		self.register_family(family.id(), family.family_types());
		for cached in self
			.inner
			.entries
			.borrow()
			.values()
			.filter(|cached| cached.family_id == family.id())
		{
			(cached.invalidate)();
		}
	}
}

#[cfg(any(wasm, test))]
fn invalid_hydration_snapshot(message: &'static str) -> serde_json::Error {
	serde_json::Error::io(std::io::Error::new(
		std::io::ErrorKind::InvalidData,
		message,
	))
}

pub(crate) fn hydration_id(identity: &QueryIdentity) -> String {
	use std::fmt::Write;

	let mut digest = String::with_capacity(64);
	for byte in identity.arguments_fingerprint() {
		write!(&mut digest, "{byte:02x}").expect("writing into String cannot fail");
	}
	format!("query:{}:sha256:{digest}", identity.family_id())
}

fn cached_query_entry<T, E>(entry: &Rc<QueryEntry<T, E>>) -> CachedQueryEntry
where
	T: Clone + 'static,
	E: Clone + 'static,
{
	CachedQueryEntry {
		family_id: entry.family_id,
		typed: entry.clone(),
		invalidate: Rc::new({
			let entry = Rc::clone(entry);
			move || entry.invalidate()
		}),
		cancel: Rc::new({
			let entry = Rc::clone(entry);
			move || entry.cancel_request()
		}),
	}
}
