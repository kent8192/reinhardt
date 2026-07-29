use std::future::Future;
use std::marker::PhantomData;
use std::pin::Pin;
use std::rc::Rc;
use std::time::Duration;

use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::cancellation::CancellationHandle;

use super::canonical_json;

pub(super) type QueryFuture<T, E> = Pin<Box<dyn Future<Output = Result<T, E>> + 'static>>;
pub(super) type QueryFetcher<T, E> = dyn Fn(CancellationHandle) -> QueryFuture<T, E> + 'static;

const DEFAULT_STALE_TIME: Duration = Duration::from_secs(30);
const DEFAULT_GC_TIME: Duration = Duration::from_secs(5 * 60);

/// Typed cache key and fetcher for a query.
///
/// Values are normally produced by the `#[server_fn]` generated `key(...)`
/// helper. Manual keys are also supported for non-server-function fetchers.
pub struct QueryKey<T, E> {
	pub(super) id: String,
	pub(super) fetcher: Rc<QueryFetcher<T, E>>,
	pub(super) stale_time: Duration,
	pub(super) gc_time: Duration,
	pub(super) ssr_prefetch: bool,
	_type: PhantomData<fn() -> Result<T, E>>,
}

impl<T, E> Clone for QueryKey<T, E> {
	fn clone(&self) -> Self {
		Self {
			id: self.id.clone(),
			fetcher: Rc::clone(&self.fetcher),
			stale_time: self.stale_time,
			gc_time: self.gc_time,
			ssr_prefetch: self.ssr_prefetch,
			_type: PhantomData,
		}
	}
}

impl<T, E> QueryKey<T, E> {
	/// Creates a typed query key from an explicit cache ID and fetcher.
	pub fn new<Id, F, Fut>(id: Id, fetcher: F) -> Self
	where
		Id: Into<String>,
		F: Fn() -> Fut + 'static,
		Fut: Future<Output = Result<T, E>> + 'static,
	{
		Self::new_with_cancellation(id, move |_| fetcher())
	}

	pub(crate) fn new_with_cancellation<Id, F, Fut>(id: Id, fetcher: F) -> Self
	where
		Id: Into<String>,
		F: Fn(CancellationHandle) -> Fut + 'static,
		Fut: Future<Output = Result<T, E>> + 'static,
	{
		Self {
			id: id.into(),
			fetcher: Rc::new(move |cancellation| Box::pin(fetcher(cancellation))),
			stale_time: DEFAULT_STALE_TIME,
			gc_time: DEFAULT_GC_TIME,
			ssr_prefetch: true,
			_type: PhantomData,
		}
	}

	/// Creates a typed key for a generated `#[server_fn]` marker.
	///
	/// JSON object keys are sorted recursively so logically equivalent argument
	/// maps produce the same cache and hydration ID. The canonical argument
	/// payload is SHA-256 hashed before it becomes part of the ID.
	pub fn from_server_fn<M, Args, F, Fut>(args: Args, fetcher: F) -> Self
	where
		M: crate::server_fn::ServerFnMetadata,
		Args: Serialize,
		F: Fn() -> Fut + 'static,
		Fut: Future<Output = Result<T, E>> + 'static,
	{
		let encoded_args = canonical_json::encode(&args)
			.expect("server function query arguments must serialize into a cache key");
		let args_digest = Sha256::digest(encoded_args.as_bytes());
		Self::new(
			format!("server_fn:{}:{}:sha256:{args_digest:x}", M::PATH, M::CODEC),
			fetcher,
		)
	}

	/// Returns the stable cache ID for this key.
	pub fn id(&self) -> &str {
		&self.id
	}

	/// Configures how long a resolved value is considered fresh.
	///
	/// SSR-replayed success and error states are both treated as freshly fetched
	/// so the initial replay preserves the server-rendered state before a retry.
	pub fn with_stale_time(mut self, stale_time: Duration) -> Self {
		self.stale_time = stale_time;
		self
	}

	/// Configures the requested cache retention window after the last observer.
	///
	/// The current implementation stores this value for cache policy parity and
	/// future eviction; entries are retained for the app lifetime unless the
	/// cache is explicitly cleared.
	pub fn with_gc_time(mut self, gc_time: Duration) -> Self {
		self.gc_time = gc_time;
		self
	}

	/// Configures whether SSR may prefetch this query in the native resource context.
	pub fn with_ssr_prefetch(mut self, enabled: bool) -> Self {
		self.ssr_prefetch = enabled;
		self
	}
}
