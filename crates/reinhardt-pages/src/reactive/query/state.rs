use std::time::Duration;

/// Application-wide defaults used to resolve query observer options.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QueryDefaults {
	stale_time: Duration,
	gc_time: Duration,
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
}

impl Default for QueryDefaults {
	fn default() -> Self {
		Self {
			stale_time: Duration::from_secs(30),
			gc_time: Duration::from_secs(300),
		}
	}
}

/// Per-observer query behavior.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QueryOptions {
	enabled: bool,
	stale_time: Option<Duration>,
	gc_time: Option<Duration>,
	refetch_interval: Option<Duration>,
}

impl QueryOptions {
	/// Creates the standard enabled query options.
	pub fn new() -> Self {
		Self::default()
	}

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
}

impl Default for QueryOptions {
	fn default() -> Self {
		Self {
			enabled: true,
			stale_time: None,
			gc_time: None,
			refetch_interval: None,
		}
	}
}

/// Current phase of a query.
#[derive(Clone, Debug, PartialEq)]
pub enum QueryPhase<T, E> {
	/// The query has no successful value or error yet.
	Pending,
	/// The query has loaded successfully.
	Success(T),
	/// The latest fetch failed.
	Error(E),
}

impl<T, E> QueryPhase<T, E> {
	/// Returns `true` if the query is pending.
	pub fn is_pending(&self) -> bool {
		matches!(self, Self::Pending)
	}

	/// Returns `true` if the query is successful.
	pub fn is_success(&self) -> bool {
		matches!(self, Self::Success(_))
	}

	/// Returns `true` if the query is in an error state.
	pub fn is_error(&self) -> bool {
		matches!(self, Self::Error(_))
	}

	/// Returns the success value if available.
	pub fn result(&self) -> Option<&T> {
		match self {
			Self::Success(value) => Some(value),
			_ => None,
		}
	}

	/// Returns the error value if available.
	pub fn error(&self) -> Option<&E> {
		match self {
			Self::Error(error) => Some(error),
			_ => None,
		}
	}
}
