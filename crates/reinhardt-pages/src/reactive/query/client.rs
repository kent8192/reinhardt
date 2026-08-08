use std::any::Any;
use std::cell::{Cell, RefCell};
use std::cmp::{Ordering, Reverse};
#[cfg(all(test, not(wasm)))]
use std::collections::VecDeque;
use std::collections::{BinaryHeap, HashMap, HashSet};
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
	QueryNormalizationContract,
};
use super::runtime::{ScopedQueryFuture, duration_ms, now_ms, spawn_query_task};
use super::state::{QueryDefaults, QueryOptions};
#[cfg(any(wasm, test))]
use super::state::{QueryHydrationSnapshot, QueryHydrationState};
use crate::cancellation::{AbortableTaskGuard, CancellationSource, scope_cancellation};
use crate::reactive::entity::{
	Entity, EntityArena, EntityHandle, EntityIdentity, EntityOverlay, EntityWriteTicket,
	EntityWriter, ErasedEntityProjection, ProjectionMaterialization, ProjectionRemoval,
	QueryTicketLease, RemovedEntities,
};
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

	fn register_maintenance(&self, _callback: Weak<dyn Fn()>) {}

	fn supports_browser_resources(&self) -> bool {
		false
	}

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

	fn supports_browser_resources(&self) -> bool {
		cfg!(wasm)
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
	maintenance: RefCell<Vec<Weak<dyn Fn()>>>,
}

#[cfg(all(test, not(wasm)))]
impl TestQueryRuntime {
	pub(crate) fn new() -> Self {
		Self {
			inner: Rc::new(TestQueryRuntimeInner {
				now_ms: Cell::new(0),
				tasks: RefCell::new(VecDeque::new()),
				maintenance: RefCell::new(Vec::new()),
			}),
		}
	}

	pub(crate) fn clock(&self) -> QueryRuntimeHandle {
		self.handle()
	}

	pub(crate) fn handle(&self) -> QueryRuntimeHandle {
		Rc::clone(&self.inner) as QueryRuntimeHandle
	}

	pub(crate) fn advance(&self, duration: Duration) {
		self.inner.now_ms.set(
			self.inner
				.now_ms
				.get()
				.saturating_add(duration_ms(duration)),
		);
	}

	pub(crate) fn run_due_maintenance(&self) {
		let callbacks = {
			let mut maintenance = self.inner.maintenance.borrow_mut();
			maintenance.retain(|callback| callback.strong_count() > 0);
			maintenance
				.iter()
				.filter_map(Weak::upgrade)
				.collect::<Vec<_>>()
		};
		for callback in callbacks {
			callback();
		}
		self.run_until_stalled();
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

	fn register_maintenance(&self, callback: Weak<dyn Fn()>) {
		self.maintenance.borrow_mut().push(callback);
	}
}

/// Application-owned keyed query cache and runtime.
#[derive(Clone)]
pub struct QueryClient {
	inner: Rc<QueryClientInner>,
}

pub(super) struct QueryClientInner {
	defaults: QueryDefaults,
	runtime: QueryRuntimeHandle,
	entities: EntityArena,
	entries: RefCell<HashMap<QueryIdentity, CachedQueryEntry>>,
	entity_dependents: RefCell<HashMap<EntityIdentity, Vec<Weak<dyn EntityDependent>>>>,
	#[cfg(any(wasm, test))]
	consumed_hydration_identities: RefCell<HashSet<QueryIdentity>>,
	families: RefCell<HashMap<&'static str, QueryFamilyMetadata>>,
	deadlines: RefCell<BinaryHeap<Reverse<QueryDeadline>>>,
	next_deadline_sequence: Cell<u64>,
	maintenance_callback: RefCell<Option<Rc<dyn Fn()>>>,
	document_visible: Cell<bool>,
	browser: super::browser::QueryBrowser,
}

#[derive(Clone)]
struct CachedQueryEntry {
	family_id: &'static str,
	typed: Rc<dyn Any>,
	invalidate: Rc<dyn Fn()>,
	cancel: Rc<dyn Fn()>,
	poll_due: Rc<dyn Fn(u64)>,
	#[cfg(wasm)]
	visibility_changed: Rc<dyn Fn(bool, u64)>,
	deadline_is_current: Rc<dyn Fn(QueryDeadlineKind, u64) -> bool>,
}

pub(crate) trait EntityDependent {
	fn query_identity(&self) -> &QueryIdentity;
	fn prepare_entity_change(
		self: Rc<Self>,
		overlay: &EntityOverlay<'_>,
		removed: &RemovedEntities<'_>,
	) -> PreparedProjectionCommit;
}

pub(crate) struct PreparedProjectionCommit {
	pub(crate) commit_structure: Box<dyn FnOnce()>,
	pub(crate) publish_signal: Box<dyn FnOnce()>,
}

#[derive(Clone, Copy)]
struct QueryFamilyMetadata {
	types: QueryFamilyTypes,
	normalization: QueryNormalizationContract,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum QueryDeadlineKind {
	Poll,
	GarbageCollection,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct QueryDeadline {
	due_ms: u64,
	sequence: u64,
	generation: u64,
	identity: QueryIdentity,
	kind: QueryDeadlineKind,
}

impl Ord for QueryDeadline {
	fn cmp(&self, other: &Self) -> Ordering {
		self.due_ms
			.cmp(&other.due_ms)
			.then_with(|| self.sequence.cmp(&other.sequence))
	}
}

impl PartialOrd for QueryDeadline {
	fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
		Some(self.cmp(other))
	}
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
		let supports_browser_resources = runtime.supports_browser_resources();
		let entity_gc_time = defaults.resolved_gc_time();
		let document_visible =
			super::browser::QueryBrowser::initial_visibility(supports_browser_resources);
		let inner = Rc::new_cyclic(|owner| QueryClientInner {
			defaults,
			runtime,
			entities: EntityArena::new(entity_gc_time),
			entries: RefCell::new(HashMap::new()),
			entity_dependents: RefCell::new(HashMap::new()),
			#[cfg(any(wasm, test))]
			consumed_hydration_identities: RefCell::new(HashSet::new()),
			families: RefCell::new(HashMap::new()),
			deadlines: RefCell::new(BinaryHeap::new()),
			next_deadline_sequence: Cell::new(0),
			maintenance_callback: RefCell::new(None),
			document_visible: Cell::new(document_visible),
			browser: super::browser::QueryBrowser::new(owner.clone(), supports_browser_resources),
		});
		let maintenance_callback: Rc<dyn Fn()> = Rc::new({
			let owner = Rc::downgrade(&inner);
			move || {
				if let Some(owner) = owner.upgrade() {
					owner.run_due_maintenance();
				}
			}
		});
		inner
			.runtime
			.register_maintenance(Rc::downgrade(&maintenance_callback));
		inner
			.maintenance_callback
			.borrow_mut()
			.replace(maintenance_callback);
		Self { inner }
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

	/// Returns a reactive handle for one normalized entity.
	pub fn entity<E>(&self, id: E::Id) -> EntityHandle<E>
	where
		E: Entity,
	{
		self.inner.entities.entity(id)
	}

	/// Replaces one normalized entity in an atomic entity transaction.
	pub fn upsert_entity<E>(&self, entity: E)
	where
		E: Entity,
	{
		self.update_entities(|entities| entities.upsert(entity));
	}

	/// Tombstones one normalized entity in an atomic entity transaction.
	pub fn remove_entity<E>(&self, id: &E::Id)
	where
		E: Entity,
	{
		self.update_entities(|entities| entities.remove::<E>(id));
	}

	/// Applies a group of normalized entity writes atomically.
	pub fn update_entities(&self, update: impl FnOnce(&mut EntityWriter<'_>)) {
		let ticket = self.inner.entities.issue_mutation_ticket();
		let staging = self.inner.entities.stage(update);
		let overlay = EntityOverlay::new(&self.inner.entities, staging, ticket);
		let removed_identities = overlay.removed_identities();
		let prepared = self.inner.prepare_entity_change(
			&overlay,
			&RemovedEntities::borrowed(&removed_identities),
			None,
		);
		let (commit_structures, publish_signals): (Vec<_>, Vec<_>) = prepared
			.into_iter()
			.map(|prepared| (prepared.commit_structure, prepared.publish_signal))
			.unzip();
		self.inner.entities.commit_overlay(
			overlay,
			ticket,
			move || {
				for commit_structure in commit_structures {
					commit_structure();
				}
			},
			move || {
				for publish_signal in publish_signals {
					publish_signal();
				}
			},
		);
	}

	/// Observes a query without installing an application context.
	#[doc(hidden)]
	#[cfg(feature = "testing")]
	pub fn observe_for_test<T, E>(
		&self,
		descriptor: QueryDescriptor<T, E>,
		options: QueryOptions,
	) -> super::hook::QueryHandle<T, E>
	where
		T: Clone + Serialize + DeserializeOwned + 'static,
		E: Clone + Serialize + DeserializeOwned + 'static,
	{
		self.observe(descriptor, options)
	}

	fn register_descriptor_family(
		&self,
		family_id: &'static str,
		actual_types: QueryFamilyTypes,
		actual_normalization: QueryNormalizationContract,
	) {
		let mut families = self.inner.families.borrow_mut();
		let Some(expected) = families.get(&family_id) else {
			families.insert(
				family_id,
				QueryFamilyMetadata {
					types: actual_types,
					normalization: actual_normalization,
				},
			);
			return;
		};
		validate_query_family_types(family_id, expected.types, actual_types);
		if expected.normalization != actual_normalization {
			panic!(
				"incompatible query family normalization for `{family_id}`: expected {}; actual {}",
				expected.normalization, actual_normalization,
			);
		}
	}

	fn validate_registered_family_types(&self, family_id: &'static str, actual: QueryFamilyTypes) {
		if let Some(expected) = self.inner.families.borrow().get(&family_id) {
			validate_query_family_types(family_id, expected.types, actual);
		}
	}

	pub(super) fn same_instance(&self, other: &Self) -> bool {
		Rc::ptr_eq(&self.inner, &other.inner)
	}

	#[cfg(test)]
	pub(crate) fn contains_for_test<T, E>(&self, key: &QueryKey<T, E>) -> bool {
		self.inner.entries.borrow().contains_key(key.identity())
	}

	#[cfg(test)]
	pub(crate) fn entity_arena_for_test(&self) -> EntityArena {
		self.inner.entities.clone()
	}

	#[cfg(test)]
	pub(crate) fn entity_dependency_lease_count_for_test<E>(&self, id: &E::Id) -> usize
	where
		E: Entity,
	{
		self.inner.entities.dependency_lease_count::<E>(id)
	}
}

fn validate_query_family_types(
	family_id: &'static str,
	expected: QueryFamilyTypes,
	actual: QueryFamilyTypes,
) {
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

impl QueryClientInner {
	fn prepare_entity_change(
		&self,
		overlay: &EntityOverlay<'_>,
		removed: &RemovedEntities<'_>,
		excluded: Option<&QueryIdentity>,
	) -> Vec<PreparedProjectionCommit> {
		let affected = overlay.affected_identities();
		let mut dependents = HashMap::<QueryIdentity, Rc<dyn EntityDependent>>::new();
		let mut index = self.entity_dependents.borrow_mut();
		for identity in affected {
			let Some(edges) = index.get_mut(&identity) else {
				continue;
			};
			edges.retain(|edge| edge.strong_count() > 0);
			for dependent in edges.iter().filter_map(Weak::upgrade) {
				if excluded.is_some_and(|excluded| dependent.query_identity() == excluded) {
					continue;
				}
				dependents
					.entry(dependent.query_identity().clone())
					.or_insert(dependent);
			}
		}
		drop(index);

		dependents
			.into_values()
			.map(|dependent| dependent.prepare_entity_change(overlay, removed))
			.collect()
	}

	fn replace_reverse_dependencies(
		&self,
		dependent: Rc<dyn EntityDependent>,
		previous: &HashSet<EntityIdentity>,
		next: &HashSet<EntityIdentity>,
	) {
		let query_identity = dependent.query_identity().clone();
		let mut index = self.entity_dependents.borrow_mut();
		for identity in next {
			let edges = index.entry(identity.clone()).or_default();
			edges.retain(|edge| edge.strong_count() > 0);
			if !edges
				.iter()
				.filter_map(Weak::upgrade)
				.any(|existing| existing.query_identity() == &query_identity)
			{
				edges.push(Rc::downgrade(&dependent));
			}
		}
		for identity in previous.difference(next) {
			let should_remove = if let Some(edges) = index.get_mut(identity) {
				edges.retain(|edge| {
					edge.upgrade()
						.is_some_and(|existing| existing.query_identity() != &query_identity)
				});
				edges.is_empty()
			} else {
				false
			};
			if should_remove {
				index.remove(identity);
			}
		}
	}

	fn polling_is_visible(&self) -> bool {
		#[cfg(wasm)]
		self.document_visible
			.set(self.browser.document_is_visible());
		self.document_visible.get()
	}

	fn schedule_deadline(
		&self,
		identity: QueryIdentity,
		kind: QueryDeadlineKind,
		generation: u64,
		due_ms: u64,
	) {
		let sequence = self.next_deadline_sequence.get();
		self.next_deadline_sequence.set(sequence.wrapping_add(1));
		self.deadlines.borrow_mut().push(Reverse(QueryDeadline {
			due_ms,
			sequence,
			generation,
			identity,
			kind,
		}));
		self.refresh_browser_timer();
	}

	fn deadline_is_current(&self, deadline: &QueryDeadline) -> bool {
		self.entries
			.borrow()
			.get(&deadline.identity)
			.is_some_and(|entry| (entry.deadline_is_current)(deadline.kind, deadline.generation))
	}

	fn next_current_deadline(&self) -> Option<QueryDeadline> {
		loop {
			let deadline = self.deadlines.borrow().peek().cloned()?.0;
			if self.deadline_is_current(&deadline) {
				return Some(deadline);
			}
			self.deadlines.borrow_mut().pop();
		}
	}

	fn refresh_browser_timer(&self) {
		let deadline_ms = self.next_current_deadline().map(|deadline| deadline.due_ms);
		self.browser.schedule(deadline_ms, self.runtime.now_ms());
	}

	fn run_due_maintenance(&self) {
		let now_ms = self.runtime.now_ms();
		while let Some(deadline) = self.next_current_deadline() {
			if deadline.due_ms > now_ms {
				break;
			}
			self.deadlines.borrow_mut().pop();
			if !self.deadline_is_current(&deadline) {
				continue;
			}
			match deadline.kind {
				QueryDeadlineKind::Poll => {
					let poll_due = self
						.entries
						.borrow()
						.get(&deadline.identity)
						.map(|entry| Rc::clone(&entry.poll_due));
					if let Some(poll_due) = poll_due {
						poll_due(deadline.generation);
					}
				}
				QueryDeadlineKind::GarbageCollection => {
					let cached = self.entries.borrow_mut().remove(&deadline.identity);
					if let Some(cached) = cached {
						(cached.cancel)();
					}
				}
			}
		}
		self.refresh_browser_timer();
	}

	#[cfg(wasm)]
	pub(super) fn handle_browser_timer(&self) {
		self.run_due_maintenance();
	}

	#[cfg(wasm)]
	pub(super) fn handle_visibility_change(&self) {
		let visible = self.polling_is_visible();
		let now_ms = self.runtime.now_ms();
		let callbacks = self
			.entries
			.borrow()
			.values()
			.map(|entry| Rc::clone(&entry.visibility_changed))
			.collect::<Vec<_>>();
		for callback in callbacks {
			callback(visible, now_ms);
		}
		self.refresh_browser_timer();
	}
}

impl Drop for QueryClientInner {
	fn drop(&mut self) {
		self.deadlines.get_mut().clear();
		self.maintenance_callback.get_mut().take();
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
	pub(super) ticket: QueryTicketLease,
	_guard: AbortableTaskGuard,
	_marker: PhantomData<fn() -> Result<T, E>>,
}

pub(super) struct QueryEntry<T: Clone + 'static, E: Clone + 'static> {
	pub(super) _scope: Rc<ReactiveScope>,
	pub(super) hydration_id: String,
	identity: QueryIdentity,
	family_id: &'static str,
	pub(super) state: Signal<ResourceState<T, E>>,
	normalization: Option<Rc<ErasedEntityProjection<T>>>,
	recipe: RefCell<Option<Box<dyn Any>>>,
	dependencies: RefCell<HashMap<EntityIdentity, Box<dyn Any>>>,
	normalization_missing: Cell<bool>,
	pub(super) refetch_error: Signal<Option<E>>,
	pub(super) is_fetching: Signal<bool>,
	pub(super) request: RefCell<Option<QueryRequest<T, E>>>,
	next_generation: Cell<u64>,
	invalidation_generation: Cell<u64>,
	invalidated: Signal<bool>,
	pub(super) completed: RefCell<Option<(u64, Result<T, E>)>>,
	waiters: RefCell<Vec<Waker>>,
	pub(super) lease_count: Cell<usize>,
	retain_lease_count: Cell<usize>,
	pub(super) refetch_after_in_flight: Cell<bool>,
	queued_manual_refetch: RefCell<Option<Weak<QueryLeaseInner<T, E>>>>,
	pub(super) last_fetched_ms: Cell<Option<u64>>,
	observers: RefCell<Vec<Weak<QueryLeaseInner<T, E>>>>,
	poll_generation: Cell<u64>,
	gc_generation: Cell<u64>,
	epoch_gc_time: Cell<Duration>,
	runtime: QueryRuntimeHandle,
	entities: EntityArena,
	owner: Option<Weak<QueryClientInner>>,
	inline_task: RefCell<Option<QueryTask>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ObserverPolicy {
	pub(super) enabled: bool,
	pub(super) stale_time: Duration,
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
	consumer: Cell<QueryConsumer>,
	pub(super) policy: ObserverPolicy,
	pub(super) fetcher: Rc<QueryFetcher<T, E>>,
	pub(super) manual_refetch_pending: Cell<bool>,
	poll_deadline_ms: Cell<Option<u64>>,
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
			entry.schedule_garbage_collection();
		} else {
			entry.refresh_polling_deadline();
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
		let (key, _fetcher, _ssr_prefetch, _family_types, normalization) = descriptor.into_parts();
		Self::new_with_hydrated_state(
			key,
			None,
			normalization,
			platform_query_runtime(),
			EntityArena::new(Duration::from_secs(300)),
			None,
		)
	}

	fn new_with_hydrated_state(
		key: QueryKey<T, E>,
		hydrated_state: Option<ResourceState<T, E>>,
		normalization: Option<Rc<ErasedEntityProjection<T>>>,
		runtime: QueryRuntimeHandle,
		entities: EntityArena,
		owner: Option<Weak<QueryClientInner>>,
	) -> Self {
		let (initial_state, last_fetched_ms) =
			initial_query_state_at(hydrated_state, runtime.now_ms());
		let hydration_id = key.hydration_id();
		let identity = key.identity().clone();
		let family_id = key.family_id();
		let scope = Rc::new(ReactiveScope::new());
		let (state, refetch_error, is_fetching, invalidated) = scope.enter(|| {
			(
				Signal::new(initial_state),
				Signal::new(None),
				Signal::new(false),
				Signal::new(false),
			)
		});

		Self {
			_scope: scope,
			hydration_id,
			identity,
			family_id,
			state,
			normalization,
			recipe: RefCell::new(None),
			dependencies: RefCell::new(HashMap::new()),
			normalization_missing: Cell::new(false),
			refetch_error,
			is_fetching,
			request: RefCell::new(None),
			next_generation: Cell::new(0),
			invalidation_generation: Cell::new(0),
			invalidated,
			completed: RefCell::new(None),
			waiters: RefCell::new(Vec::new()),
			lease_count: Cell::new(0),
			retain_lease_count: Cell::new(0),
			refetch_after_in_flight: Cell::new(false),
			queued_manual_refetch: RefCell::new(None),
			last_fetched_ms: Cell::new(last_fetched_ms),
			observers: RefCell::new(Vec::new()),
			poll_generation: Cell::new(0),
			gc_generation: Cell::new(0),
			epoch_gc_time: Cell::new(Duration::ZERO),
			runtime,
			entities,
			owner,
			inline_task: RefCell::new(None),
		}
	}

	pub(super) fn is_stale(&self, stale_time: Duration) -> bool {
		if self.invalidated.get() || self.normalization_missing.get() {
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
			ResourceState::Error(_) => self.is_stale(stale_time),
		})
	}

	pub(super) fn has_request(&self) -> bool {
		self.request.borrow().is_some()
	}

	fn poll_interval_for(policy: ObserverPolicy, consumer: QueryConsumer) -> Option<Duration> {
		if !policy.enabled || consumer != QueryConsumer::MountedQuery {
			return None;
		}
		policy
			.refetch_interval
			.filter(|interval| !interval.is_zero())
			.map(|interval| interval.max(Duration::from_millis(1)))
	}

	fn polling_is_visible(&self) -> bool {
		self.owner
			.as_ref()
			.and_then(Weak::upgrade)
			.is_none_or(|owner| owner.polling_is_visible())
	}

	fn live_observers(&self) -> Vec<Rc<QueryLeaseInner<T, E>>> {
		let mut observers = self.observers.borrow_mut();
		observers.retain(|observer| observer.strong_count() > 0);
		observers.iter().filter_map(Weak::upgrade).collect()
	}

	fn enabled_mounted_observers(&self) -> Vec<Rc<QueryLeaseInner<T, E>>> {
		self.live_observers()
			.into_iter()
			.filter(|observer| {
				observer.policy.enabled && observer.consumer.get() == QueryConsumer::MountedQuery
			})
			.collect()
	}

	fn next_poll_generation(&self) -> u64 {
		let generation = self.poll_generation.get().wrapping_add(1);
		self.poll_generation.set(generation);
		generation
	}

	fn refresh_polling_deadline(&self) {
		let owner = self.owner.as_ref().and_then(Weak::upgrade);
		if owner
			.as_ref()
			.is_some_and(|owner| !owner.polling_is_visible())
		{
			self.suspend_polling();
			if let Some(owner) = owner {
				owner.refresh_browser_timer();
			}
			return;
		}
		let deadline_ms = self
			.live_observers()
			.into_iter()
			.filter_map(|observer| {
				Self::poll_interval_for(observer.policy, observer.consumer.get())
					.and(observer.poll_deadline_ms.get())
			})
			.min();
		let generation = self.next_poll_generation();
		if let Some(owner) = owner {
			if let Some(deadline_ms) = deadline_ms {
				owner.schedule_deadline(
					self.identity.clone(),
					QueryDeadlineKind::Poll,
					generation,
					deadline_ms,
				);
			} else {
				owner.refresh_browser_timer();
			}
		}
	}

	fn reschedule_polling_from(&self, now_ms: u64) {
		if !self.polling_is_visible() {
			self.suspend_polling();
			if let Some(owner) = self.owner.as_ref().and_then(Weak::upgrade) {
				owner.refresh_browser_timer();
			}
			return;
		}
		for observer in self.live_observers() {
			observer.poll_deadline_ms.set(
				Self::poll_interval_for(observer.policy, observer.consumer.get())
					.map(|interval| now_ms.saturating_add(duration_ms(interval))),
			);
		}
		self.refresh_polling_deadline();
	}

	fn suspend_polling(&self) {
		for observer in self.live_observers() {
			observer.poll_deadline_ms.set(None);
		}
		self.next_poll_generation();
	}

	fn schedule_garbage_collection(&self) {
		self.suspend_polling();
		let generation = self.gc_generation.get().wrapping_add(1);
		self.gc_generation.set(generation);
		if let Some(owner) = self.owner.as_ref().and_then(Weak::upgrade) {
			owner.schedule_deadline(
				self.identity.clone(),
				QueryDeadlineKind::GarbageCollection,
				generation,
				self.runtime
					.now_ms()
					.saturating_add(duration_ms(self.epoch_gc_time.get())),
			);
		}
	}

	fn handle_poll_deadline(self: &Rc<Self>, generation: u64) {
		if generation != self.poll_generation.get() || self.lease_count.get() == 0 {
			return;
		}
		self.suspend_polling();
		if !self.polling_is_visible() {
			if let Some(owner) = self.owner.as_ref().and_then(Weak::upgrade) {
				owner.refresh_browser_timer();
			}
			return;
		}
		if !self.has_request() && !self.enabled_mounted_observers().is_empty() {
			self.start_fetch(true);
		}
	}

	#[cfg(wasm)]
	fn handle_visibility_change(self: &Rc<Self>, visible: bool, now_ms: u64) {
		if !visible {
			self.suspend_polling();
			return;
		}
		let observers = self.enabled_mounted_observers();
		if observers.is_empty() {
			return;
		}
		if observers
			.iter()
			.any(|observer| self.is_stale(observer.policy.stale_time))
		{
			self.suspend_polling();
			if !self.has_request() {
				self.start_fetch(true);
			}
		} else {
			self.reschedule_polling_from(now_ms);
		}
	}

	fn deadline_is_current(&self, kind: QueryDeadlineKind, generation: u64) -> bool {
		match kind {
			QueryDeadlineKind::Poll => {
				self.lease_count.get() > 0 && self.poll_generation.get() == generation
			}
			QueryDeadlineKind::GarbageCollection => {
				self.lease_count.get() == 0 && self.gc_generation.get() == generation
			}
		}
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
		let previous_lease_count = self.lease_count.get();
		self.lease_count.set(previous_lease_count + 1);
		if previous_lease_count == 0 {
			self.gc_generation
				.set(self.gc_generation.get().wrapping_add(1));
			self.epoch_gc_time.set(policy.gc_time);
		} else {
			self.epoch_gc_time
				.set(self.epoch_gc_time.get().max(policy.gc_time));
		}
		let retains_errors = error_policy == QueryErrorPolicy::Retain;
		if retains_errors {
			self.retain_lease_count
				.set(self.retain_lease_count.get() + 1);
		}
		let inner = Rc::new(QueryLeaseInner {
			entry: Rc::clone(self),
			generation: Cell::new(generation),
			retains_errors,
			consumer: Cell::new(consumer),
			policy,
			fetcher,
			manual_refetch_pending: Cell::new(false),
			poll_deadline_ms: Cell::new(if self.polling_is_visible() {
				Self::poll_interval_for(policy, consumer)
					.map(|interval| self.runtime.now_ms().saturating_add(duration_ms(interval)))
			} else {
				None
			}),
		});
		self.observers.borrow_mut().push(Rc::downgrade(&inner));
		self.refresh_polling_deadline();
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
					observer.consumer.get(),
					QueryConsumer::MountedRoute(_)
						| QueryConsumer::MountedQuery
						| QueryConsumer::Maintenance
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
		let ticket = self.entities.acquire_query_ticket();
		*self.request.borrow_mut() = Some(QueryRequest {
			generation,
			invalidation_generation,
			manual_observer: manual_observer.as_ref().map(Rc::downgrade),
			source,
			ticket,
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
		drop(request);
		let request = self
			.request
			.borrow_mut()
			.take()
			.expect("matching query request must remain active through completion");
		let request_invalidation_generation = request.invalidation_generation;
		let manual_observer = request.manual_observer.clone();
		let had_success = self
			.state
			.with_untracked(|state| matches!(state, ResourceState::Success(_)));
		let mut normalized_success = false;
		match result {
			Ok(value) => {
				if self.normalization.is_some() {
					normalized_success = true;
					self.complete_normalized_success(
						generation,
						value,
						request.ticket.ticket(),
						request_invalidation_generation,
					);
				} else {
					self.completed
						.borrow_mut()
						.replace((generation, Ok(value.clone())));
					self.last_fetched_ms.set(Some(self.runtime.now_ms()));
					self.refetch_error.set(None);
					self.state.set(ResourceState::Success(value));
					if self.invalidation_generation.get() == request_invalidation_generation {
						self.invalidated.set(false);
					}
				}
			}
			Err(error) => {
				self.completed
					.borrow_mut()
					.replace((generation, Err(error.clone())));
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
		if !normalized_success {
			self.is_fetching.set(false);
		}
		self.wake_waiters();
		self.reschedule_polling_from(self.runtime.now_ms());
		let invalidated_during_request =
			self.invalidation_generation.get() > request_invalidation_generation;
		let manual_refetch_queued = self.refetch_after_in_flight.replace(false);
		let queued_manual_observer = self.queued_manual_refetch.borrow_mut().take();
		let queued_manual_observer_was_present = queued_manual_observer.is_some();
		let queued_manual_observer_is_live = queued_manual_observer
			.as_ref()
			.is_some_and(|observer| observer.strong_count() > 0);
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
		} else if ((manual_refetch_queued
			&& (!queued_manual_observer_was_present || queued_manual_observer_is_live))
			|| (invalidated_during_request && self.has_active_invalidation_interest()))
			&& self.lease_count.get() > 0
		{
			self.start_fetch(true);
		}
	}

	fn complete_normalized_success(
		self: &Rc<Self>,
		generation: u64,
		value: T,
		ticket: EntityWriteTicket,
		request_invalidation_generation: u64,
	) {
		let projection = self
			.normalization
			.as_ref()
			.expect("normalized completion requires an entity projection");
		let fallback = value.clone();
		let mut staged_recipe = None;
		let staging = self.entities.stage(|entities| {
			staged_recipe.replace(projection.normalize(value, entities));
		});
		let mut recipe = staged_recipe.expect("entity normalization must produce a recipe");
		let overlay = EntityOverlay::new(&self.entities, staging, ticket);
		let removed_identities = overlay.removed_identities();
		let removal = projection.apply_removals(
			recipe.as_mut(),
			&RemovedEntities::borrowed(&removed_identities),
		);
		let dependencies = projection.dependencies(recipe.as_ref());
		let materialization = projection.materialize(recipe.as_ref(), &overlay);
		let (candidate, missing) = match materialization {
			ProjectionMaterialization::Ready(candidate) => (
				candidate,
				matches!(removal, ProjectionRemoval::MissingRequired),
			),
			ProjectionMaterialization::MissingRequired => (fallback, true),
		};
		let leases = dependencies.acquire_leases(&self.entities);
		let next_dependencies = dependencies.identities().clone();
		let previous_dependencies = self
			.dependencies
			.borrow()
			.keys()
			.cloned()
			.collect::<HashSet<_>>();
		let removed = RemovedEntities::borrowed(&removed_identities);
		let prepared = self
			.owner
			.as_ref()
			.and_then(Weak::upgrade)
			.map(|owner| owner.prepare_entity_change(&overlay, &removed, Some(&self.identity)))
			.unwrap_or_default();
		let (commit_structures, publish_signals): (Vec<_>, Vec<_>) = prepared
			.into_iter()
			.map(|prepared| (prepared.commit_structure, prepared.publish_signal))
			.unzip();
		let owner = self.owner.as_ref().and_then(Weak::upgrade);
		let dependent: Rc<dyn EntityDependent> = self.clone();
		let published_candidate = candidate.clone();
		self.entities.commit_overlay(
			overlay,
			ticket,
			|| {
				if let Some(owner) = owner {
					owner.replace_reverse_dependencies(
						dependent,
						&previous_dependencies,
						&next_dependencies,
					);
				}
				self.recipe.borrow_mut().replace(recipe);
				let previous_leases =
					std::mem::replace(&mut *self.dependencies.borrow_mut(), leases);
				self.normalization_missing.set(missing);
				self.completed
					.borrow_mut()
					.replace((generation, Ok(candidate)));
				self.last_fetched_ms.set(Some(self.runtime.now_ms()));
				for commit_structure in commit_structures {
					commit_structure();
				}
				drop(previous_leases);
			},
			|| {
				self.refetch_error.set(None);
				self.state.set(ResourceState::Success(published_candidate));
				if self.invalidation_generation.get() == request_invalidation_generation {
					self.invalidated.set(false);
				}
				self.is_fetching.set(false);
				for publish_signal in publish_signals {
					publish_signal();
				}
			},
		);
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

impl<T, E> EntityDependent for QueryEntry<T, E>
where
	T: Clone + 'static,
	E: Clone + 'static,
{
	fn query_identity(&self) -> &QueryIdentity {
		&self.identity
	}

	fn prepare_entity_change(
		self: Rc<Self>,
		overlay: &EntityOverlay<'_>,
		removed: &RemovedEntities<'_>,
	) -> PreparedProjectionCommit {
		let projection = self
			.normalization
			.as_ref()
			.expect("entity dependents require a normalized projection");
		let mut recipe = {
			let stored = self.recipe.borrow();
			projection.clone_recipe(
				stored
					.as_deref()
					.expect("entity dependents require a stored projection recipe"),
			)
		};
		let removal = projection.apply_removals(recipe.as_mut(), removed);
		let dependencies = projection.dependencies(recipe.as_ref());
		let materialization = projection.materialize(recipe.as_ref(), overlay);
		let (candidate, missing) = match materialization {
			ProjectionMaterialization::Ready(candidate) => (
				Some(candidate),
				matches!(removal, ProjectionRemoval::MissingRequired),
			),
			ProjectionMaterialization::MissingRequired => (None, true),
		};
		let leases = dependencies.acquire_leases(&self.entities);
		let next_dependencies = dependencies.identities().clone();
		let previous_dependencies = self
			.dependencies
			.borrow()
			.keys()
			.cloned()
			.collect::<HashSet<_>>();
		let completed_generation = self
			.completed
			.borrow()
			.as_ref()
			.and_then(|(generation, result)| result.as_ref().ok().map(|_| *generation));
		let published_candidate = candidate.clone();
		let structure_entry = Rc::clone(&self);
		let signal_entry = Rc::clone(&self);

		PreparedProjectionCommit {
			commit_structure: Box::new(move || {
				if let Some(owner) = structure_entry.owner.as_ref().and_then(Weak::upgrade) {
					let dependent: Rc<dyn EntityDependent> = structure_entry.clone();
					owner.replace_reverse_dependencies(
						dependent,
						&previous_dependencies,
						&next_dependencies,
					);
				}
				structure_entry.recipe.borrow_mut().replace(recipe);
				let previous_leases =
					std::mem::replace(&mut *structure_entry.dependencies.borrow_mut(), leases);
				structure_entry.normalization_missing.set(missing);
				if let (Some(generation), Some(candidate)) =
					(completed_generation, candidate.as_ref())
				{
					structure_entry
						.completed
						.borrow_mut()
						.replace((generation, Ok(candidate.clone())));
				}
				drop(previous_leases);
			}),
			publish_signal: Box::new(move || {
				if let Some(candidate) = published_candidate {
					signal_entry.state.set(ResourceState::Success(candidate));
				}
			}),
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
	pub(crate) fn promote_to_mounted_route(&self, generation: u64) {
		if matches!(self.inner.consumer.get(), QueryConsumer::Navigation(_)) {
			self.inner
				.consumer
				.set(QueryConsumer::MountedRoute(generation));
			if self.inner.entry.invalidated.get() && !self.inner.entry.has_request() {
				self.inner.entry.start_fetch(true);
			}
		}
	}

	// Route preparation awaits this result while the public hook remains
	// synchronous.
	pub(crate) async fn result(&self) -> Result<T, E> {
		QueryResultFuture {
			entry: Rc::clone(&self.inner.entry),
			generation: self.inner.generation.get(),
		}
		.await
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
		let (key, fetcher, _ssr_prefetch, family_types, normalization) = descriptor.into_parts();
		let contract = QueryNormalizationContract::from_projection(normalization.as_deref());
		self.entry_for_key(key, family_types, contract, normalization)
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
		self.register_descriptor_family(
			key.family_id(),
			key.family_types(),
			QueryNormalizationContract::Plain,
		);
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
			None,
			Rc::clone(&self.inner.runtime),
			self.inner.entities.clone(),
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
		let identity = key.identity().clone();
		if self
			.inner
			.consumed_hydration_identities
			.borrow()
			.contains(&identity)
		{
			return Ok(());
		}
		let snapshot: QueryHydrationSnapshot<T, E> = serde_json::from_value(serialized.clone())?;
		if snapshot.is_fetching {
			return Err(invalid_hydration_snapshot(
				"settled query hydration snapshot is still fetching",
			));
		}
		let refetch_error = snapshot.refetch_error;
		let hydrated_state = match snapshot.state {
			QueryHydrationState::Success(value) => ResourceState::Success(value),
			QueryHydrationState::Error(error) => {
				if refetch_error.is_some() {
					return Err(invalid_hydration_snapshot(
						"initial error query snapshot contains a refetch error",
					));
				}
				ResourceState::Error(error)
			}
		};
		self.inner
			.consumed_hydration_identities
			.borrow_mut()
			.insert(identity.clone());
		self.register_descriptor_family(
			key.family_id(),
			key.family_types(),
			QueryNormalizationContract::Plain,
		);
		let id = key.id();
		#[cfg(any(wasm, test))]
		super::super::resource::reserve_client_resource_key(&id);
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
			None,
			Rc::clone(&self.inner.runtime),
			self.inner.entities.clone(),
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
		let (key, _fetcher, _ssr_prefetch, family_types, normalization) = descriptor.into_parts();
		let contract = QueryNormalizationContract::from_projection(normalization.as_deref());
		self.entry_for_key(key, family_types, contract, normalization)
	}

	fn entry_for_key<T, E>(
		&self,
		key: QueryKey<T, E>,
		family_types: QueryFamilyTypes,
		contract: QueryNormalizationContract,
		normalization: Option<Rc<ErasedEntityProjection<T>>>,
	) -> Rc<QueryEntry<T, E>>
	where
		T: Clone + Serialize + DeserializeOwned + 'static,
		E: Clone + Serialize + DeserializeOwned + 'static,
	{
		self.register_descriptor_family(key.family_id(), family_types, contract);
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
			normalization,
			Rc::clone(&self.inner.runtime),
			self.inner.entities.clone(),
			Some(Rc::downgrade(&self.inner)),
		));
		entries.insert(identity, cached_query_entry(&entry));
		entry
	}

	/// Marks one exact typed query stale and refetches it when actively enabled.
	pub fn invalidate<T, E>(&self, key: &QueryKey<T, E>) {
		self.validate_registered_family_types(key.family_id(), key.family_types());
		if let Some(cached) = self.inner.entries.borrow().get(key.identity()) {
			(cached.invalidate)();
		}
	}

	/// Marks all cached entries in one typed query family stale.
	pub fn invalidate_family<Args: 'static, T: 'static, E: 'static>(
		&self,
		family: QueryFamily<Args, T, E>,
	) {
		self.validate_registered_family_types(family.id(), family.family_types());
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

/// Overrides document visibility for browser query lifecycle tests.
#[cfg(feature = "testing")]
pub fn set_query_visibility_for_test(client: &QueryClient, visible: bool) {
	client.inner.browser.set_visibility_for_test(visible);
}

/// Returns active visibility listeners and polling timers for a query client.
#[cfg(feature = "testing")]
pub fn query_browser_resource_counts(client: &QueryClient) -> (usize, usize) {
	client.inner.browser.resource_counts()
}

/// Captures a weak view of browser resources for final-client-drop tests.
#[cfg(feature = "testing")]
pub fn query_browser_resource_probe_for_test(
	client: &QueryClient,
) -> super::browser::QueryBrowserResourceProbe {
	client.inner.browser.resource_probe()
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
		poll_due: Rc::new({
			let entry = Rc::clone(entry);
			move |generation| entry.handle_poll_deadline(generation)
		}),
		#[cfg(wasm)]
		visibility_changed: Rc::new({
			let entry = Rc::clone(entry);
			move |visible, now_ms| entry.handle_visibility_change(visible, now_ms)
		}),
		deadline_is_current: Rc::new({
			let entry = Rc::clone(entry);
			move |kind, generation| entry.deadline_is_current(kind, generation)
		}),
	}
}
