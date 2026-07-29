use std::cell::RefCell;
use std::future::Future;
use std::rc::Rc;
use std::time::Duration;

use serde::Serialize;
use serde::de::DeserializeOwned;

use super::super::hooks::async_action::{Action, use_action};
use super::super::resource::ResourceState;
use super::browser::QueryGuard;
use super::client::{
	QueryAcquireOptions, QueryConsumer, QueryEntry, QueryErrorPolicy, QueryLease,
	acquire_query_with_options, invalidate_query_id,
};
use super::identity::{QueryDescriptor, QueryKey};
use super::state::{QueryOptions, QueryPhase};
use crate::cancellation::CancellationSource;

/// Reactive handle returned by [`use_query`].
pub struct QueryHandle<T: Clone + 'static, E: Clone + 'static> {
	pub(super) entry: Rc<QueryEntry<T, E>>,
	pub(super) lease: QueryLease<T, E>,
	pub(super) guards: Rc<RefCell<Vec<QueryGuard>>>,
}

impl<T: Clone + 'static, E: Clone + 'static> Clone for QueryHandle<T, E> {
	fn clone(&self) -> Self {
		Self {
			entry: Rc::clone(&self.entry),
			lease: self.lease.clone(),
			guards: Rc::clone(&self.guards),
		}
	}
}

impl<T: Clone + 'static, E: Clone + 'static> QueryHandle<T, E> {
	fn mark_ssr_read(&self) {
		#[cfg(native)]
		crate::ssr::resource_context::mark_resource_read(&self.entry.id);
	}

	/// Returns this query's deterministic SSR hydration key.
	pub fn ssr_key(&self) -> &str {
		&self.entry.id
	}

	/// Returns the underlying resource-style state.
	pub fn get(&self) -> ResourceState<T, E> {
		self.mark_ssr_read();
		self.entry.state.get()
	}

	/// Returns the current query phase.
	pub fn phase(&self) -> QueryPhase<T, E> {
		self.mark_ssr_read();
		match self.entry.state.get() {
			ResourceState::Loading => QueryPhase::Pending,
			ResourceState::Success(value) => QueryPhase::Success(value),
			ResourceState::Error(error) => QueryPhase::Error(error),
		}
	}

	/// Returns `true` while a fetch is in progress.
	pub fn is_fetching(&self) -> bool {
		self.mark_ssr_read();
		self.entry.is_fetching.get()
	}

	/// Returns `true` until the query has a successful value or error.
	pub fn is_pending(&self) -> bool {
		self.phase().is_pending()
	}

	/// Returns `true` if the query has a successful value.
	pub fn is_success(&self) -> bool {
		self.phase().is_success()
	}

	/// Returns `true` if the query is in an error state.
	pub fn is_error(&self) -> bool {
		self.phase().is_error()
	}

	/// Returns the current successful value, if present.
	pub fn data(&self) -> Option<T> {
		self.mark_ssr_read();
		match self.entry.state.get() {
			ResourceState::Success(value) => Some(value),
			_ => None,
		}
	}

	/// Returns the current error value, if present.
	pub fn error(&self) -> Option<E> {
		self.mark_ssr_read();
		match self.entry.state.get() {
			ResourceState::Error(error) => Some(error),
			_ => None,
		}
	}

	/// Manually refetches this query.
	pub fn refetch(&self) {
		self.entry.start_fetch(true);
	}

	/// Refetches this query at a fixed interval while the handle is alive.
	pub fn poll(self, interval: Duration) -> Self {
		if !interval.is_zero() {
			self.guards
				.borrow_mut()
				.push(QueryGuard::poll(interval, Rc::clone(&self.entry)));
		}
		self
	}

	/// Updates the stale-time policy for this mounted query.
	pub fn stale_time(self, stale_time: Duration) -> Self {
		self.entry.stale_time.set(stale_time);
		if self.entry.is_stale() {
			self.entry.start_fetch(false);
		}
		self
	}

	/// Updates the cache retention policy for this mounted query.
	pub fn gc_time(self, gc_time: Duration) -> Self {
		self.entry.gc_time.set(gc_time);
		self
	}

	/// Returns the current stale-time policy.
	pub fn stale_time_policy(&self) -> Duration {
		self.entry.stale_time.get()
	}

	/// Returns the current cache retention policy.
	pub fn gc_time_policy(&self) -> Duration {
		self.entry.gc_time.get()
	}
}

/// Creates or subscribes to an app-wide keyed query.
pub fn use_query<T, E>(
	descriptor: QueryDescriptor<T, E>,
	options: QueryOptions,
) -> QueryHandle<T, E>
where
	T: Clone + Serialize + DeserializeOwned + 'static,
	E: Clone + Serialize + DeserializeOwned + 'static,
{
	#[cfg(native)]
	if let Some(query) = try_create_ssr_query(descriptor.clone(), options.clone()) {
		return query;
	}

	let refetch_interval = options.refetch_interval_value();
	let enabled = options.is_enabled();
	let lease = acquire_query_with_options(
		descriptor,
		QueryAcquireOptions {
			consumer: QueryConsumer::MountedQuery,
			error_policy: QueryErrorPolicy::Retain,
		},
		options,
	);
	let entry = Rc::clone(&lease.inner.entry);
	let guards = Rc::new(RefCell::new(Vec::new()));
	if enabled
		&& let Some(interval) = refetch_interval
		&& !interval.is_zero()
	{
		guards
			.borrow_mut()
			.push(QueryGuard::poll(interval, Rc::clone(&entry)));
	}

	QueryHandle {
		entry,
		lease,
		guards,
	}
}

/// Creates a mutation action that can invalidate queries on success.
pub fn use_mutation<P, T, E, F, Fut>(mutation_fn: F) -> Action<T, E>
where
	P: 'static,
	T: Clone + 'static,
	E: Clone + 'static,
	F: Fn(P) -> Fut + 'static,
	Fut: Future<Output = Result<T, E>> + 'static,
{
	use_action(mutation_fn)
}

impl<T, E> Action<T, E>
where
	T: Clone + 'static,
	E: Clone + 'static,
{
	/// Refetches `key` after this mutation succeeds.
	pub fn invalidates<QT, QE>(self, key: QueryKey<QT, QE>) -> Self
	where
		QT: Clone + 'static,
		QE: Clone + 'static,
	{
		let id = key.id().to_string();
		self.on_success(move |_| {
			invalidate_query_id(&id);
		})
	}
}

#[cfg(native)]
pub(super) fn try_create_ssr_query<T, E>(
	descriptor: QueryDescriptor<T, E>,
	options: QueryOptions,
) -> Option<QueryHandle<T, E>>
where
	T: Clone + Serialize + DeserializeOwned + 'static,
	E: Clone + Serialize + DeserializeOwned + 'static,
{
	crate::ssr::resource_context::with_active_context(|context| {
		let id = descriptor.key().id();
		context.borrow_mut().reserve_call_order_key(&id);
		let ssr_prefetch = descriptor.ssr_prefetch && options.is_enabled();
		let fetcher = Rc::clone(&descriptor.fetcher);
		let entry = Rc::new(QueryEntry::new(descriptor, &options));
		if ssr_prefetch {
			let resource_fetcher = Rc::clone(&fetcher);
			context.borrow_mut().register_resource_with_owner(
				entry.id.clone(),
				move || {
					let source = CancellationSource::new();
					let cancellation = source.handle();
					let resource_fetcher = Rc::clone(&resource_fetcher);
					async move {
						let _source = source;
						resource_fetcher(cancellation).await
					}
				},
				entry.state,
				Some(Rc::clone(&entry._scope)),
			);
			entry.mark_resolved_fetched();
		}
		let lease = entry.make_lease(
			None,
			QueryErrorPolicy::Retain,
			fetcher,
			options.is_enabled(),
		);
		QueryHandle {
			entry: Rc::clone(&entry),
			lease,
			guards: Rc::new(RefCell::new(Vec::new())),
		}
	})
}
