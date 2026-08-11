use std::time::Duration;

use serde::{Deserialize, Serialize};

use super::retry::{NoRetry, RetryPolicy};

/// Application-wide defaults used to resolve query observer options.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QueryDefaults {
	stale_time: Duration,
	gc_time: Duration,
	ssr_query_retries: bool,
}

impl QueryDefaults {
	/// Creates the standard query defaults.
	pub fn new() -> Self {
		Self::default()
	}

	/// Sets how long resolved query state remains fresh by default.
	pub fn stale_time(mut self, stale_time: Duration) -> Self {
		self.stale_time = stale_time;
		self
	}

	/// Sets how long an inactive cache entry may be retained by default.
	pub fn gc_time(mut self, gc_time: Duration) -> Self {
		self.gc_time = gc_time;
		self
	}

	pub(crate) fn resolved_stale_time(&self) -> Duration {
		self.stale_time
	}

	pub(crate) fn resolved_gc_time(&self) -> Duration {
		self.gc_time
	}

	// Only `ssr::renderer` (native-only, see `ssr.rs`'s `#[cfg(native)] mod renderer;`)
	// calls this setter; gate it the same way so wasm32 builds don't flag it as
	// dead code.
	#[cfg(native)]
	pub(crate) fn with_ssr_query_retries(mut self, enabled: bool) -> Self {
		self.ssr_query_retries = enabled;
		self
	}

	pub(crate) fn ssr_query_retries_enabled(&self) -> bool {
		self.ssr_query_retries
	}
}

impl Default for QueryDefaults {
	fn default() -> Self {
		Self {
			stale_time: Duration::from_secs(30),
			gc_time: Duration::from_secs(300),
			ssr_query_retries: false,
		}
	}
}

/// Per-observer query behavior.
///
/// The retry type is [`NoRetry`] until [`QueryOptions::retry`] installs a typed
/// [`RetryPolicy`]. Because retry predicates are closures, options that contain
/// a retry policy do not implement equality.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QueryOptions<R = NoRetry> {
	enabled: bool,
	stale_time: Option<Duration>,
	gc_time: Option<Duration>,
	refetch_interval: Option<Duration>,
	retry: R,
}

impl QueryOptions<NoRetry> {
	/// Creates the standard enabled query options.
	pub fn new() -> Self {
		Self::default()
	}

	/// Installs a typed retry policy for this observer.
	///
	/// The policy error type must match the descriptor error type when the
	/// options are passed to `use_query`.
	///
	/// # Panics
	///
	/// Panics if `max_attempts` is zero or `max_delay` is less than
	/// `base_delay`. Zero delay values are valid when both bounds are ordered.
	pub fn retry<E>(self, retry: RetryPolicy<E>) -> QueryOptions<RetryPolicy<E>> {
		retry.validate();
		QueryOptions {
			enabled: self.enabled,
			stale_time: self.stale_time,
			gc_time: self.gc_time,
			refetch_interval: self.refetch_interval,
			retry,
		}
	}
}

impl<R> QueryOptions<R> {
	/// Enables or disables fetching for this observer.
	pub fn enabled(mut self, enabled: bool) -> Self {
		self.enabled = enabled;
		self
	}

	/// Overrides how long resolved state remains fresh for this observer.
	pub fn stale_time(mut self, stale_time: Duration) -> Self {
		self.stale_time = Some(stale_time);
		self
	}

	/// Overrides how long the inactive cache entry may be retained.
	pub fn gc_time(mut self, gc_time: Duration) -> Self {
		self.gc_time = Some(gc_time);
		self
	}

	/// Refetches at the given interval while this observer is mounted.
	pub fn refetch_interval(mut self, refetch_interval: Duration) -> Self {
		self.refetch_interval = Some(refetch_interval);
		self
	}

	pub(crate) fn is_enabled(&self) -> bool {
		self.enabled
	}

	pub(crate) fn resolved_stale_time(&self, defaults: &QueryDefaults) -> Duration {
		self.stale_time
			.unwrap_or_else(|| defaults.resolved_stale_time())
	}

	pub(crate) fn resolved_gc_time(&self, defaults: &QueryDefaults) -> Duration {
		self.gc_time.unwrap_or_else(|| defaults.resolved_gc_time())
	}

	pub(crate) fn refetch_interval_value(&self) -> Option<Duration> {
		self.refetch_interval
	}

	pub(crate) fn retry_state(&self) -> &R {
		&self.retry
	}
}

impl Default for QueryOptions<NoRetry> {
	fn default() -> Self {
		Self {
			enabled: true,
			stale_time: None,
			gc_time: None,
			refetch_interval: None,
			retry: NoRetry,
		}
	}
}

/// Current lifecycle status of a query observer.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum QueryStatus {
	/// The observer is disabled and has no cached result.
	Idle,
	/// The first fetch is in progress.
	Pending,
	/// Cached data is available.
	Success,
	/// The initial fetch failed without producing data.
	Error,
}

/// Observer-specific view of one cached query.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct QuerySnapshot<T, E> {
	/// Current lifecycle status.
	pub status: QueryStatus,
	/// Latest successfully fetched data.
	pub data: Option<T>,
	/// Terminal initial-fetch error when no data is available.
	///
	/// Intermediate attempt errors remain private and this stays `None` during
	/// retry backoff.
	pub error: Option<E>,
	/// Terminal background-fetch error that preserved existing data.
	///
	/// Intermediate attempt errors remain private and this stays `None` until
	/// the retry sequence is exhausted.
	pub refetch_error: Option<E>,
	/// Whether this query currently has a fetch attempt in flight.
	///
	/// This is `false` while a retry sequence is waiting in backoff.
	pub is_fetching: bool,
	/// Whether this observer considers the cached state stale.
	pub is_stale: bool,
}

#[derive(Deserialize, Serialize)]
pub(crate) struct QueryHydrationSnapshot<T, E> {
	pub(crate) state: QueryHydrationState<T, E>,
	pub(crate) refetch_error: Option<E>,
	pub(crate) is_fetching: bool,
	pub(crate) is_stale: bool,
}

#[derive(Deserialize, Serialize)]
pub(crate) enum QueryHydrationState<T, E> {
	Success(T),
	Error(E),
}
