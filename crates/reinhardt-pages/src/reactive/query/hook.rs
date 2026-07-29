use std::cell::RefCell;
use std::rc::Rc;

use serde::Serialize;
use serde::de::DeserializeOwned;

use super::super::resource::ResourceState;
use super::browser::QueryGuard;
use super::client::{
	ObserverPolicy, QueryAcquireOptions, QueryClient, QueryConsumer, QueryEntry, QueryErrorPolicy,
	QueryLease,
};
use super::context::queries;
use super::identity::QueryDescriptor;
use super::state::{QueryDefaults, QueryOptions, QuerySnapshot, QueryStatus};
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

	/// Returns the current observer-specific query state.
	pub fn snapshot(&self) -> QuerySnapshot<T, E> {
		self.mark_ssr_read();
		let manual_refetch_pending = self.lease.inner.manual_refetch_pending.get();
		let is_fetching = self.entry.is_fetching.get()
			&& (self.lease.inner.policy.enabled || manual_refetch_pending);
		let is_stale = self.entry.is_stale(self.lease.inner.policy.stale_time);
		match self.entry.state.get() {
			ResourceState::Loading => QuerySnapshot {
				status: if self.lease.inner.policy.enabled || manual_refetch_pending {
					QueryStatus::Pending
				} else {
					QueryStatus::Idle
				},
				data: None,
				error: None,
				refetch_error: None,
				is_fetching,
				is_stale,
			},
			ResourceState::Success(data) => QuerySnapshot {
				status: QueryStatus::Success,
				data: Some(data),
				error: None,
				refetch_error: self.entry.refetch_error.get(),
				is_fetching,
				is_stale,
			},
			ResourceState::Error(error) => QuerySnapshot {
				status: QueryStatus::Error,
				data: None,
				error: Some(error),
				refetch_error: None,
				is_fetching,
				is_stale,
			},
		}
	}

	/// Returns the current successful value, if present.
	pub fn data(&self) -> Option<T> {
		self.snapshot().data
	}

	/// Returns the current error value, if present.
	pub fn error(&self) -> Option<E> {
		self.snapshot().error
	}

	/// Returns the latest background-fetch error, if present.
	pub fn refetch_error(&self) -> Option<E> {
		self.snapshot().refetch_error
	}

	/// Returns `true` while a fetch is in progress.
	pub fn is_fetching(&self) -> bool {
		self.snapshot().is_fetching
	}

	/// Returns whether this observer considers the cached state stale.
	pub fn is_stale(&self) -> bool {
		self.snapshot().is_stale
	}

	/// Manually refetches this query.
	pub fn refetch(&self) {
		self.entry.start_observer_refetch(&self.lease.inner);
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

	queries().observe(descriptor, options)
}

pub(super) fn observe_query<T, E>(
	client: &QueryClient,
	descriptor: QueryDescriptor<T, E>,
	options: QueryOptions,
) -> QueryHandle<T, E>
where
	T: Clone + Serialize + DeserializeOwned + 'static,
	E: Clone + Serialize + DeserializeOwned + 'static,
{
	let lease = client.acquire_with_options(
		descriptor,
		QueryAcquireOptions {
			consumer: QueryConsumer::MountedQuery,
			error_policy: QueryErrorPolicy::Retain,
		},
		options,
	);
	let entry = Rc::clone(&lease.inner.entry);
	let guards = Rc::new(RefCell::new(Vec::new()));
	if lease.inner.policy.enabled
		&& let Some(interval) = lease.inner.policy.refetch_interval
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
		let policy = ObserverPolicy::resolve(&options, &QueryDefaults::default());
		let entry = Rc::new(QueryEntry::new(descriptor));
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
			QueryConsumer::MountedQuery,
			QueryErrorPolicy::Retain,
			fetcher,
			policy,
		);
		QueryHandle {
			entry: Rc::clone(&entry),
			lease,
			guards: Rc::new(RefCell::new(Vec::new())),
		}
	})
}
