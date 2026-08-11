use std::rc::Rc;

use serde::Serialize;
use serde::de::DeserializeOwned;

use super::super::resource::ResourceState;
use super::client::{
	QueryAcquireOptions, QueryClient, QueryConsumer, QueryEntry, QueryErrorPolicy, QueryLease,
};
use super::context::queries;
use super::identity::QueryDescriptor;
use super::retry::QueryRetryConfig;
#[cfg(native)]
use super::state::{QueryHydrationSnapshot, QueryHydrationState};
use super::state::{QueryOptions, QuerySnapshot, QueryStatus};

/// Reactive handle returned by [`use_query`].
pub struct QueryHandle<T: Clone + 'static, E: Clone + 'static> {
	pub(super) entry: Rc<QueryEntry<T, E>>,
	pub(super) lease: QueryLease<T, E>,
}

impl<T: Clone + 'static, E: Clone + 'static> Clone for QueryHandle<T, E> {
	fn clone(&self) -> Self {
		Self {
			entry: Rc::clone(&self.entry),
			lease: self.lease.clone(),
		}
	}
}

impl<T: Clone + 'static, E: Clone + 'static> QueryHandle<T, E> {
	fn mark_ssr_read(&self) {
		#[cfg(native)]
		crate::ssr::resource_context::mark_resource_read(&self.entry.hydration_id);
	}

	/// Returns this query's deterministic SSR hydration key.
	pub fn ssr_key(&self) -> &str {
		&self.entry.hydration_id
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

	#[cfg(native)]
	fn hydration_snapshot(&self) -> QueryHydrationSnapshot<T, E> {
		let snapshot = self.snapshot();
		let state = match snapshot.status {
			QueryStatus::Success => QueryHydrationState::Success(
				snapshot
					.data
					.expect("successful query hydration requires settled data"),
			),
			QueryStatus::Error => QueryHydrationState::Error(
				snapshot
					.error
					.expect("error query hydration requires a settled error"),
			),
			QueryStatus::Idle | QueryStatus::Pending => {
				panic!("query hydration requires a settled query")
			}
		};
		QueryHydrationSnapshot {
			state,
			refetch_error: snapshot.refetch_error,
			is_fetching: snapshot.is_fetching,
			is_stale: snapshot.is_stale,
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
	options: QueryOptions<impl QueryRetryConfig<E>>,
) -> QueryHandle<T, E>
where
	T: Clone + Serialize + DeserializeOwned + 'static,
	E: Clone + Serialize + DeserializeOwned + 'static,
{
	queries().observe(descriptor, options)
}

pub(super) fn observe_query<T, E, R>(
	client: &QueryClient,
	descriptor: QueryDescriptor<T, E>,
	options: QueryOptions<R>,
) -> QueryHandle<T, E>
where
	T: Clone + Serialize + DeserializeOwned + 'static,
	E: Clone + Serialize + DeserializeOwned + 'static,
	R: QueryRetryConfig<E>,
{
	#[cfg(wasm)]
	if let Ok(hydration) = crate::hydration::HydrationContext::from_window() {
		hydration
			.seed_query(client, descriptor.key().clone())
			.unwrap_or_else(|error| {
				panic!(
					"query hydration payload `{}` is invalid: {error}",
					descriptor.key().hydration_id()
				)
			});
	}
	#[cfg(native)]
	let ssr_prefetch = descriptor.ssr_prefetch && options.is_enabled();
	let lease = client.acquire_with_options(
		descriptor,
		QueryAcquireOptions {
			consumer: QueryConsumer::MountedQuery,
			error_policy: QueryErrorPolicy::Retain,
		},
		options,
	);
	let entry = Rc::clone(&lease.inner.entry);

	let query = QueryHandle { entry, lease };
	#[cfg(native)]
	if ssr_prefetch {
		let hydration_id = query.entry.hydration_id.clone();
		let query_for_resource = query.clone();
		let owner = crate::ssr::resource_context::current_render_owner();
		crate::ssr::resource_context::with_active_context(|context| {
			context
				.borrow_mut()
				.register_serialized_resource_with_owner(
					hydration_id,
					move || async move {
						let _ = query_for_resource.lease.result().await;
						serde_json::to_value(query_for_resource.hydration_snapshot())
							.expect("query snapshots must serialize for hydration")
					},
					owner,
				);
		});
	}
	query
}
