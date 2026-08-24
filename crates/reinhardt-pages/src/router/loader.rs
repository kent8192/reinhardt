//! Route-level loader contracts and deterministic cache-key helpers.

use super::loader_registry::LoaderConsumer;
use crate::cancellation::CancellationHandle;
use crate::hydration::HydrationContext;
use crate::reactive::{
	QueryAcquireOptions, QueryClient, QueryConsumer, QueryErrorPolicy, QueryFamily, QueryLease,
	QueryResultError, queries,
};
use reinhardt_urls::routers::client_router::{ClientRouteTreeMatch, RouteContext, RouteLoaderId};
use serde::{Deserialize, Deserializer, Serialize, Serializer, de::DeserializeOwned};
use sha2::{Digest, Sha256};
use std::any::{Any, TypeId};
use std::cell::RefCell;
use std::collections::HashMap;
use std::error::Error;
use std::fmt;
use std::rc::Rc;

/// A typed value made available to a routed component or layout.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Loader<T>(pub T);

/// Contract implemented by the marker generated for a route loader.
pub trait RouteLoader {
	/// Value produced by the loader.
	type Data: Clone + Serialize + DeserializeOwned + 'static;
	/// Application error returned by the loader.
	type Error: Into<RouteLoaderError> + 'static;
	/// Stable identifier used in route metadata and cache keys.
	const ID: RouteLoaderId;
}

/// A safe, serializable route-loader failure.
///
/// The public message and status are safe to send to the browser. The optional
/// diagnostic cause is retained for server-side logging and is intentionally
/// omitted from serialization.
pub struct RouteLoaderError {
	public_message: String,
	status: Option<u16>,
	diagnostic: Option<Rc<dyn Error>>,
}

impl RouteLoaderError {
	/// Creates an error with a browser-safe public message.
	pub fn new(message: impl Into<String>) -> Self {
		Self {
			public_message: message.into(),
			status: None,
			diagnostic: None,
		}
	}

	/// Creates an error with a browser-safe message and HTTP-like status.
	pub fn with_status(message: impl Into<String>, status: u16) -> Self {
		Self {
			public_message: message.into(),
			status: Some(status),
			diagnostic: None,
		}
	}

	/// Creates an error while retaining an application diagnostic cause.
	pub fn from_diagnostic<E>(message: impl Into<String>, status: Option<u16>, error: E) -> Self
	where
		E: Error + 'static,
	{
		Self {
			public_message: message.into(),
			status,
			diagnostic: Some(Rc::new(error)),
		}
	}

	/// Returns the browser-safe public message.
	pub fn public_message(&self) -> &str {
		&self.public_message
	}

	/// Returns the optional status code.
	pub fn status(&self) -> Option<u16> {
		self.status
	}

	/// Returns the retained diagnostic cause, when one exists.
	pub fn diagnostic(&self) -> Option<&(dyn Error + 'static)> {
		self.diagnostic.as_deref()
	}
}

impl Clone for RouteLoaderError {
	fn clone(&self) -> Self {
		Self {
			public_message: self.public_message.clone(),
			status: self.status,
			diagnostic: self.diagnostic.clone(),
		}
	}
}

impl fmt::Debug for RouteLoaderError {
	fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
		formatter
			.debug_struct("RouteLoaderError")
			.field("public_message", &self.public_message)
			.field("status", &self.status)
			.field(
				"diagnostic",
				&self.diagnostic.as_ref().map(|error| error.to_string()),
			)
			.finish()
	}
}

impl fmt::Display for RouteLoaderError {
	fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
		formatter.write_str(&self.public_message)
	}
}

impl Error for RouteLoaderError {
	fn source(&self) -> Option<&(dyn Error + 'static)> {
		self.diagnostic()
	}
}

impl From<String> for RouteLoaderError {
	fn from(message: String) -> Self {
		Self::new(message)
	}
}

impl From<&str> for RouteLoaderError {
	fn from(message: &str) -> Self {
		Self::new(message)
	}
}

impl Serialize for RouteLoaderError {
	fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
	where
		S: Serializer,
	{
		#[derive(Serialize)]
		struct Wire<'a> {
			public_message: &'a str,
			status: Option<u16>,
		}

		Wire {
			public_message: &self.public_message,
			status: self.status,
		}
		.serialize(serializer)
	}
}

impl<'de> Deserialize<'de> for RouteLoaderError {
	fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
	where
		D: Deserializer<'de>,
	{
		#[derive(Deserialize)]
		struct Wire {
			public_message: String,
			status: Option<u16>,
		}

		let wire = Wire::deserialize(deserializer)?;
		Ok(Self {
			public_message: wire.public_message,
			status: wire.status,
			diagnostic: None,
		})
	}
}

/// The source of a loader input used in the canonical key.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LoaderInputKind {
	/// A named path parameter.
	Path,
	/// A named query parameter.
	Query,
}

/// One declaration-ordered loader input.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LoaderInputSpec {
	/// Extractor source.
	pub kind: LoaderInputKind,
	/// Raw extractor name.
	pub name: &'static str,
}

impl LoaderInputSpec {
	/// Declares a path input.
	pub const fn path(name: &'static str) -> Self {
		Self {
			kind: LoaderInputKind::Path,
			name,
		}
	}

	/// Declares a query input.
	pub const fn query(name: &'static str) -> Self {
		Self {
			kind: LoaderInputKind::Query,
			name,
		}
	}
}

/// Error produced when a loader input is absent from a matched route.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LoaderInputError {
	/// The route did not provide the named input.
	Missing {
		/// Input source.
		kind: LoaderInputKind,
		/// Input name.
		name: String,
	},
}

impl fmt::Display for LoaderInputError {
	fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Self::Missing { kind, name } => {
				write!(formatter, "missing {:?} loader input `{name}`", kind)
			}
		}
	}
}

impl Error for LoaderInputError {}

/// Builds the canonical, declaration-ordered input shape used before hashing.
pub fn canonical_loader_inputs(
	context: &RouteContext,
	specs: &[LoaderInputSpec],
) -> Result<String, LoaderInputError> {
	let mut path_values = Vec::new();
	let mut query_values = Vec::new();
	for spec in specs {
		let value = match spec.kind {
			LoaderInputKind::Path => context.path_param(spec.name),
			LoaderInputKind::Query => query_value(context.query(), spec.name),
		}
		.ok_or_else(|| LoaderInputError::Missing {
			kind: spec.kind,
			name: spec.name.to_string(),
		})?;
		let encoded_name =
			serde_json::to_string(spec.name).expect("static loader input names serialize");
		let encoded_value = serde_json::to_string(&value).expect("loader input values serialize");
		let pair = format!("[{encoded_name},{encoded_value}]");
		match spec.kind {
			LoaderInputKind::Path => path_values.push(pair),
			LoaderInputKind::Query => query_values.push(pair),
		}
	}
	Ok(format!(
		"{{\"path\":[{}],\"query\":[{}]}}",
		path_values.join(","),
		query_values.join(",")
	))
}

/// Returns the opaque query-cache ID for a route loader.
pub fn loader_cache_id(
	id: RouteLoaderId,
	context: &RouteContext,
	specs: &[LoaderInputSpec],
) -> Result<String, LoaderInputError> {
	let shape = canonical_loader_inputs(context, specs)?;
	let mut digest = Sha256::new();
	digest.update(id.as_str().as_bytes());
	digest.update([0]);
	digest.update(shape.as_bytes());
	Ok(format!(
		"route_loader:{}:sha256:{:x}",
		id.as_str(),
		digest.finalize()
	))
}

/// Converts a matched route tree into the context consumed by loader inputs.
pub fn route_context(matched: &ClientRouteTreeMatch) -> RouteContext {
	RouteContext::new(
		matched.path().to_string(),
		matched.params().clone(),
		matched.query().unwrap_or_default().to_string(),
	)
}

fn query_value(query: &str, name: &str) -> Option<String> {
	query
		.split('&')
		.filter(|pair| !pair.is_empty())
		.find_map(|pair| {
			let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
			if key == name {
				Some(percent_decode(value, true))
			} else {
				None
			}
		})
}

fn percent_decode(value: &str, plus_as_space: bool) -> String {
	let bytes = value.as_bytes();
	let mut output = Vec::with_capacity(bytes.len());
	let mut index = 0;
	while index < bytes.len() {
		match bytes[index] {
			b'+' if plus_as_space => {
				output.push(b' ');
				index += 1;
			}
			b'%' if index + 2 < bytes.len() => {
				if let (Some(high), Some(low)) =
					(hex_digit(bytes[index + 1]), hex_digit(bytes[index + 2]))
				{
					output.push(high * 16 + low);
					index += 3;
				} else {
					output.push(b'%');
					index += 1;
				}
			}
			byte => {
				output.push(byte);
				index += 1;
			}
		}
	}
	String::from_utf8_lossy(&output).into_owned()
}

fn hex_digit(byte: u8) -> Option<u8> {
	match byte {
		b'0'..=b'9' => Some(byte - b'0'),
		b'a'..=b'f' => Some(byte - b'a' + 10),
		b'A'..=b'F' => Some(byte - b'A' + 10),
		_ => None,
	}
}

/// A typed value plus the query lease that keeps its underlying work alive.
#[doc(hidden)]
// The coordinator and hydration phases consume these fields after a loader
// future settles; this module is compiled before those phases are wired.
#[allow(dead_code)]
pub struct PreparedLoader {
	id: RouteLoaderId,
	type_id: TypeId,
	value: Rc<dyn Any>,
	serialized: serde_json::Value,
	lease: ErasedQueryLease,
}

// See the struct-level explanation above; these accessors form the internal
// handoff contract for the registry and coordinator.
#[allow(dead_code)]
impl PreparedLoader {
	pub(crate) fn new<T>(
		id: RouteLoaderId,
		value: T,
		serialized: serde_json::Value,
		lease: QueryLease<T, RouteLoaderError>,
	) -> Self
	where
		T: Clone + Serialize + DeserializeOwned + 'static,
	{
		Self {
			id,
			type_id: TypeId::of::<T>(),
			value: Rc::new(value),
			serialized,
			lease: ErasedQueryLease::new(lease),
		}
	}

	pub(crate) fn id(&self) -> RouteLoaderId {
		self.id
	}

	pub(crate) fn type_id(&self) -> TypeId {
		self.type_id
	}

	pub(crate) fn value(&self) -> Rc<dyn Any> {
		Rc::clone(&self.value)
	}

	pub(crate) fn serialized(&self) -> &serde_json::Value {
		&self.serialized
	}

	pub(crate) fn into_parts(
		self,
	) -> (
		RouteLoaderId,
		TypeId,
		Rc<dyn Any>,
		serde_json::Value,
		ErasedQueryLease,
	) {
		(
			self.id,
			self.type_id,
			self.value,
			self.serialized,
			self.lease,
		)
	}
}

#[derive(Clone)]
pub(crate) struct ErasedQueryLease {
	lease: Rc<dyn Any>,
	promote_to_mounted_route: Rc<dyn Fn(u64)>,
	is_evicted: Rc<dyn Fn() -> bool>,
}

impl ErasedQueryLease {
	fn new<T, E>(lease: QueryLease<T, E>) -> Self
	where
		T: Clone + 'static,
		E: Clone + 'static,
	{
		let promote_lease = lease.clone();
		let evicted_lease = lease.clone();
		Self {
			lease: Rc::new(lease),
			promote_to_mounted_route: Rc::new(move |generation| {
				promote_lease.promote_to_mounted_route(generation);
			}),
			is_evicted: Rc::new(move || evicted_lease.is_evicted()),
		}
	}

	fn promote_to_mounted_route(&self, generation: u64) {
		(self.promote_to_mounted_route)(generation);
	}

	fn is_evicted(&self) -> bool {
		(self.is_evicted)()
	}

	#[allow(dead_code)]
	fn as_any(&self) -> &dyn Any {
		self.lease.as_ref()
	}
}

/// A scoped typed store for prepared route-loader values.
#[derive(Clone, Default)]
pub struct LoaderStore {
	values: Rc<RefCell<HashMap<RouteLoaderId, StoredLoader>>>,
}

struct StoredLoader {
	type_id: TypeId,
	value: Rc<dyn Any>,
	serialized: serde_json::Value,
	// The lease is intentionally retained for as long as the prepared value is mounted.
	#[allow(dead_code)]
	lease: Option<ErasedQueryLease>,
}

impl LoaderStore {
	/// Creates an empty store.
	pub fn new() -> Self {
		Self::default()
	}

	/// Inserts a typed value without a query lease.
	pub fn insert<T>(&self, id: RouteLoaderId, value: T) -> Result<(), serde_json::Error>
	where
		T: Clone + Serialize + DeserializeOwned + 'static,
	{
		let serialized = serde_json::to_value(&value)?;
		self.values.borrow_mut().insert(
			id,
			StoredLoader {
				type_id: TypeId::of::<T>(),
				value: Rc::new(value),
				serialized,
				lease: None,
			},
		);
		Ok(())
	}

	/// Returns a typed loader value, checking both its ID and concrete type.
	pub fn get<T>(&self, id: RouteLoaderId) -> Result<Loader<T>, LoaderStoreError>
	where
		T: Clone + 'static,
	{
		let evicted = self
			.values
			.borrow()
			.get(&id)
			.and_then(|stored| stored.lease.as_ref())
			.is_some_and(ErasedQueryLease::is_evicted);
		if evicted {
			self.values.borrow_mut().remove(&id);
			return Err(LoaderStoreError::Missing { id });
		}
		let values = self.values.borrow();
		let stored = values.get(&id).ok_or(LoaderStoreError::Missing { id })?;
		if stored.type_id != TypeId::of::<T>() {
			return Err(LoaderStoreError::TypeMismatch { id });
		}
		let value = Rc::clone(&stored.value)
			.downcast::<T>()
			.map_err(|_| LoaderStoreError::TypeMismatch { id })?;
		Ok(Loader((*value).clone()))
	}

	/// Returns a serialized success value for SSR/hydration transfer.
	pub fn serialized(&self, id: RouteLoaderId) -> Result<serde_json::Value, LoaderStoreError> {
		let evicted = self
			.values
			.borrow()
			.get(&id)
			.and_then(|stored| stored.lease.as_ref())
			.is_some_and(ErasedQueryLease::is_evicted);
		if evicted {
			self.values.borrow_mut().remove(&id);
			return Err(LoaderStoreError::Missing { id });
		}
		self.values
			.borrow()
			.get(&id)
			.map(|stored| stored.serialized.clone())
			.ok_or(LoaderStoreError::Missing { id })
	}

	// The navigation coordinator transfers leases only after all loaders settle.
	#[allow(dead_code)]
	pub(crate) fn insert_prepared(&self, prepared: PreparedLoader) {
		let (id, type_id, value, serialized, lease) = prepared.into_parts();
		self.values.borrow_mut().insert(
			id,
			StoredLoader {
				type_id,
				value,
				serialized,
				lease: Some(lease),
			},
		);
	}

	pub(crate) fn promote_navigation_leases(&self, generation: u64) {
		for stored in self.values.borrow().values() {
			if let Some(lease) = &stored.lease {
				lease.promote_to_mounted_route(generation);
			}
		}
	}
}

/// Errors raised by a loader-store lookup indicate a framework invariant.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LoaderStoreError {
	/// No prepared value exists for this ID.
	Missing {
		/// Missing loader ID.
		id: RouteLoaderId,
	},
	/// The ID exists but was prepared with another concrete data type.
	TypeMismatch {
		/// Mismatched loader ID.
		id: RouteLoaderId,
	},
}

impl fmt::Display for LoaderStoreError {
	fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Self::Missing { id } => write!(formatter, "loader `{}` is not prepared", id.as_str()),
			Self::TypeMismatch { id } => write!(
				formatter,
				"loader `{}` has an incompatible data type",
				id.as_str()
			),
		}
	}
}

impl Error for LoaderStoreError {}

thread_local! {
	static ACTIVE_LOADER_STORES: RefCell<Vec<LoaderStore>> = const { RefCell::new(Vec::new()) };
}

/// RAII guard for the active loader-store render scope.
pub struct LoaderStoreScope {
	active: bool,
}

impl Drop for LoaderStoreScope {
	fn drop(&mut self) {
		if self.active {
			ACTIVE_LOADER_STORES.with(|stores| {
				stores.borrow_mut().pop();
			});
			self.active = false;
		}
	}
}

/// Enters a loader store for the duration of a render or poll scope.
pub fn enter_loader_store(store: LoaderStore) -> LoaderStoreScope {
	ACTIVE_LOADER_STORES.with(|stores| stores.borrow_mut().push(store));
	LoaderStoreScope { active: true }
}

/// Runs a closure with `store` as the ambient typed loader store.
pub fn with_loader_store<R>(store: &LoaderStore, f: impl FnOnce() -> R) -> R {
	let _scope = enter_loader_store(store.clone());
	f()
}

/// Returns the innermost ambient loader store.
pub fn active_loader_store() -> Option<LoaderStore> {
	ACTIVE_LOADER_STORES.with(|stores| stores.borrow().last().cloned())
}

/// Seeds the typed query cache entry that backs a hydrated route loader.
#[doc(hidden)]
pub fn seed_loader_query<T>(
	client: &QueryClient,
	id: RouteLoaderId,
	context: &RouteContext,
	specs: &[LoaderInputSpec],
	hydration: &HydrationContext,
	fetcher: impl Fn(
		CancellationHandle,
	) -> std::pin::Pin<
		Box<dyn std::future::Future<Output = Result<T, RouteLoaderError>> + 'static>,
	> + 'static,
) -> Result<PreparedLoader, RouteLoaderError>
where
	T: Clone + Serialize + DeserializeOwned + 'static,
{
	let cache_id = loader_cache_id(id, context, specs)
		.map_err(|error| RouteLoaderError::with_status(error.to_string(), 400))?;
	let query_state = hydration.get_resource_state(&cache_id).ok_or_else(|| {
		RouteLoaderError::with_status(
			format!(
				"route loader `{}` query state is missing from SSR state",
				id.as_str()
			),
			500,
		)
	})?;
	let serialized = hydration
		.get_route_loader_state(id.as_str())
		.ok_or_else(|| {
			RouteLoaderError::with_status(
				format!("route loader `{}` is missing from SSR state", id.as_str()),
				500,
			)
		})?;
	let value: T = serde_json::from_value(serialized.clone()).map_err(|error| {
		RouteLoaderError::from_diagnostic(
			"route loader hydration value is invalid",
			Some(500),
			error,
		)
	})?;
	let descriptor = QueryFamily::<String, T, RouteLoaderError>::new(id.as_str())
		.query_with_cancellation(cache_id, fetcher);
	client
		.seed_serialized(descriptor.key().clone(), query_state)
		.map_err(|error| {
			RouteLoaderError::from_diagnostic(
				"route loader query hydration state is invalid",
				Some(500),
				error,
			)
		})?;
	let lease = client.acquire(
		descriptor,
		QueryAcquireOptions {
			consumer: QueryConsumer::MountedRoute(0),
			error_policy: QueryErrorPolicy::Discard,
		},
	);
	Ok(PreparedLoader::new(id, value, serialized.clone(), lease))
}

/// Acquires a registered loader using the shared query cache.
// Generated `#[loader]` executors call this function so route preparation uses
// the same query client as `use_query`.
pub async fn acquire_loader_query<T>(
	id: RouteLoaderId,
	context: &RouteContext,
	specs: &'static [LoaderInputSpec],
	cancellation: CancellationHandle,
	consumer: LoaderConsumer,
	fetcher: impl Fn(
		CancellationHandle,
	) -> std::pin::Pin<
		Box<dyn std::future::Future<Output = Result<T, RouteLoaderError>> + 'static>,
	> + 'static,
) -> Result<PreparedLoader, RouteLoaderError>
where
	T: Clone + Serialize + DeserializeOwned + 'static,
{
	let cache_id = loader_cache_id(id, context, specs)
		.map_err(|error| RouteLoaderError::with_status(error.to_string(), 400))?;
	let client = queries();
	let descriptor = QueryFamily::<String, T, RouteLoaderError>::new(id.as_str())
		.query_with_cancellation(cache_id, fetcher);
	let lease = client.acquire(
		descriptor,
		QueryAcquireOptions {
			consumer: consumer.into(),
			error_policy: QueryErrorPolicy::Discard,
		},
	);
	let cancellation_check = cancellation.clone();
	let value = crate::cancellation::scope_cancellation(cancellation, lease.result())
		.await
		.map_err(|error| match error {
			QueryResultError::Fetch(error) => error,
			QueryResultError::Evicted => RouteLoaderError::new("route loader query was evicted"),
		})?;
	if cancellation_check.is_cancelled() {
		return Err(RouteLoaderError::new(
			"route loader navigation was cancelled",
		));
	}
	let serialized = serde_json::to_value(&value).map_err(|error| {
		RouteLoaderError::from_diagnostic("loader value serialization failed", Some(500), error)
	})?;
	Ok(PreparedLoader::new(id, value, serialized, lease))
}

#[cfg(test)]
mod tests {
	use super::*;
	#[cfg(native)]
	use crate::reactive::query::TestQueryRuntime;
	#[cfg(native)]
	use crate::reactive::{QueryConsumer, QueryDefaults};
	#[cfg(native)]
	use crate::{HydrationContext, SsrState};
	#[cfg(native)]
	use reinhardt_core::reactive::ReactiveScope;
	#[cfg(native)]
	use std::cell::Cell;
	use std::collections::HashMap;
	#[cfg(native)]
	use std::collections::VecDeque;
	#[cfg(native)]
	use std::future::Future;
	#[cfg(native)]
	use std::pin::Pin;
	#[cfg(native)]
	use std::rc::Rc;
	#[cfg(native)]
	use std::task::{Context, Poll, Waker};
	#[cfg(native)]
	use std::time::Duration;

	#[cfg(native)]
	type QueryTask = Pin<Box<dyn Future<Output = ()> + 'static>>;

	#[cfg(native)]
	fn poll_query_tasks(tasks: &Rc<RefCell<VecDeque<QueryTask>>>, rounds: usize) {
		for _ in 0..rounds {
			let count = tasks.borrow().len();
			if count == 0 {
				return;
			}
			for _ in 0..count {
				let Some(mut task) = tasks.borrow_mut().pop_front() else {
					break;
				};
				let mut context = Context::from_waker(Waker::noop());
				if task.as_mut().poll(&mut context) == Poll::Pending {
					tasks.borrow_mut().push_back(task);
				}
			}
		}
	}

	fn context(path_params: &[(&str, &str)], query: &str) -> RouteContext {
		RouteContext::new(
			"/projects/42".to_string(),
			path_params
				.iter()
				.map(|(name, value)| ((*name).to_string(), (*value).to_string()))
				.collect::<HashMap<_, _>>(),
			query.to_string(),
		)
	}

	#[test]
	fn canonical_key_preserves_declared_path_and_query_order() {
		let context = context(&[("project_id", "42")], "tab=open&unused=1");
		let specs = [
			LoaderInputSpec::path("project_id"),
			LoaderInputSpec::query("tab"),
		];
		assert_eq!(
			canonical_loader_inputs(&context, &specs).unwrap(),
			r#"{"path":[["project_id","42"]],"query":[["tab","open"]]}"#
		);
		let id = RouteLoaderId::new("writing.jobs");
		let cache_id = loader_cache_id(id, &context, &specs).unwrap();
		assert!(cache_id.starts_with("route_loader:writing.jobs:sha256:"));
		assert!(!cache_id.contains("project_id") || !cache_id.contains("42"));
	}

	#[test]
	fn canonical_key_keeps_path_values_aligned_with_path_extractors() {
		let first = context(&[("project_id", "a%2Fb")], "tab=hello%20world&other=one");
		let second = context(&[("project_id", "a/b")], "other=changed&tab=hello+world");
		let specs = [
			LoaderInputSpec::path("project_id"),
			LoaderInputSpec::query("tab"),
		];
		assert_ne!(
			canonical_loader_inputs(&first, &specs).unwrap(),
			canonical_loader_inputs(&second, &specs).unwrap()
		);
		assert_ne!(
			loader_cache_id(RouteLoaderId::new("jobs"), &first, &specs).unwrap(),
			loader_cache_id(RouteLoaderId::new("jobs"), &second, &specs).unwrap()
		);
	}

	#[test]
	fn canonical_key_matches_query_names_without_decoding_them() {
		let encoded_name = context(&[], "ta%62=stale&tab=current");
		let canonical_name = context(&[], "tab=current");
		let specs = [LoaderInputSpec::query("tab")];

		assert_eq!(
			canonical_loader_inputs(&encoded_name, &specs).unwrap(),
			canonical_loader_inputs(&canonical_name, &specs).unwrap()
		);
		assert_eq!(
			loader_cache_id(RouteLoaderId::new("jobs"), &encoded_name, &specs).unwrap(),
			loader_cache_id(RouteLoaderId::new("jobs"), &canonical_name, &specs).unwrap()
		);
	}

	#[test]
	fn route_loader_error_serializes_without_diagnostic() {
		let error = RouteLoaderError::from_diagnostic(
			"not found",
			Some(404),
			std::io::Error::other("secret"),
		);
		let json = serde_json::to_value(&error).unwrap();
		assert_eq!(json["public_message"], "not found");
		assert_eq!(json["status"], 404);
		assert!(json.get("diagnostic").is_none());
		let decoded: RouteLoaderError = serde_json::from_value(json).unwrap();
		assert_eq!(decoded.public_message(), "not found");
		assert!(decoded.diagnostic().is_none());
	}

	#[test]
	fn loader_store_checks_id_and_type() {
		let store = LoaderStore::new();
		let id = RouteLoaderId::new("jobs");
		store.insert(id, vec![1_u32, 2]).unwrap();
		assert_eq!(store.get::<Vec<u32>>(id).unwrap(), Loader(vec![1, 2]));
		assert_eq!(
			store.get::<String>(id),
			Err(LoaderStoreError::TypeMismatch { id })
		);
		assert_eq!(
			store.get::<Vec<u32>>(RouteLoaderId::new("missing")),
			Err(LoaderStoreError::Missing {
				id: RouteLoaderId::new("missing")
			})
		);
	}

	#[test]
	fn loader_store_scope_restores_nested_store() {
		let outer = LoaderStore::new();
		let inner = LoaderStore::new();
		with_loader_store(&outer, || {
			assert!(active_loader_store().is_some());
			with_loader_store(&inner, || assert!(active_loader_store().is_some()));
			assert!(active_loader_store().is_some());
		});
		assert!(active_loader_store().is_none());
	}

	#[cfg(native)]
	#[test]
	fn one_client_shares_loader_work_between_prefetch_and_navigation() {
		ReactiveScope::run(|| {
			// Arrange
			let runtime = TestQueryRuntime::new();
			let client = QueryClient::with_runtime(QueryDefaults::default(), runtime.handle());
			let fetch_count = Rc::new(Cell::new(0));
			let released = Rc::new(Cell::new(false));
			let descriptor = QueryFamily::<String, String, String>::new("tests::shared_loader")
				.query_with_cancellation("project:42".to_owned(), {
					let fetch_count = Rc::clone(&fetch_count);
					let released = Rc::clone(&released);
					move |_cancellation| {
						let fetch_count = Rc::clone(&fetch_count);
						let released = Rc::clone(&released);
						async move {
							fetch_count.set(fetch_count.get() + 1);
							std::future::poll_fn(move |_| {
								if released.get() {
									Poll::Ready(())
								} else {
									Poll::Pending
								}
							})
							.await;
							Ok("project".to_owned())
						}
					}
				});

			// Act
			let prefetch = client.acquire(
				descriptor.clone(),
				QueryAcquireOptions {
					consumer: QueryConsumer::Prefetch,
					error_policy: QueryErrorPolicy::Discard,
				},
			);
			let navigation = client.acquire(
				descriptor,
				QueryAcquireOptions {
					consumer: QueryConsumer::Navigation(1),
					error_policy: QueryErrorPolicy::Discard,
				},
			);
			runtime.run_until_stalled();
			let mut prefetch_result = tokio_test::task::spawn(prefetch.result());
			let mut navigation_result = tokio_test::task::spawn(navigation.result());

			// Assert
			assert_eq!(prefetch_result.poll(), Poll::Pending);
			assert_eq!(navigation_result.poll(), Poll::Pending);
			assert_eq!(fetch_count.get(), 1);
			assert_eq!(runtime.pending_task_count(), 1);

			released.set(true);
			runtime.run_until_stalled();

			assert!(prefetch_result.is_woken());
			assert!(navigation_result.is_woken());
			assert_eq!(
				prefetch_result.poll(),
				Poll::Ready(Ok("project".to_owned()))
			);
			assert_eq!(
				navigation_result.poll(),
				Poll::Ready(Ok("project".to_owned()))
			);
		});
	}

	#[cfg(native)]
	#[test]
	fn separate_clients_do_not_share_loader_work() {
		ReactiveScope::run(|| {
			// Arrange
			let first_client = QueryClient::new(QueryDefaults::default());
			let second_client = QueryClient::new(QueryDefaults::default());
			let fetch_count = Rc::new(Cell::new(0));
			let tasks = Rc::new(RefCell::new(VecDeque::new()));
			let tasks_for_sink = Rc::clone(&tasks);
			let _sink = crate::platform::install_task_sink(move |task| {
				tasks_for_sink.borrow_mut().push_back(task);
			});
			let descriptor = QueryFamily::<String, String, String>::new("tests::isolated_loader")
				.query_with_cancellation("project:42".to_owned(), {
					let fetch_count = Rc::clone(&fetch_count);
					move |_cancellation| {
						let fetch_count = Rc::clone(&fetch_count);
						async move {
							fetch_count.set(fetch_count.get() + 1);
							Ok("project".to_owned())
						}
					}
				});

			// Act
			let first = first_client.acquire(
				descriptor.clone(),
				QueryAcquireOptions {
					consumer: QueryConsumer::Prefetch,
					error_policy: QueryErrorPolicy::Discard,
				},
			);
			let second = second_client.acquire(
				descriptor,
				QueryAcquireOptions {
					consumer: QueryConsumer::Navigation(1),
					error_policy: QueryErrorPolicy::Discard,
				},
			);
			poll_query_tasks(&tasks, 4);

			// Assert
			tokio_test::block_on(async {
				assert_eq!(first.result().await, Ok("project".to_owned()));
				assert_eq!(second.result().await, Ok("project".to_owned()));
			});
			assert_eq!(fetch_count.get(), 2);
		});
	}

	#[cfg(native)]
	#[test]
	fn hydration_seed_keeps_route_loader_query_fresh_without_refetching() {
		ReactiveScope::run(|| {
			// Arrange
			let runtime = TestQueryRuntime::new();
			let query_client = QueryClient::with_runtime(
				QueryDefaults::default().gc_time(Duration::ZERO),
				runtime.handle(),
			);
			let route_context = context(&[("project_id", "42")], "tab=open");
			let loader_id = RouteLoaderId::new("tests::hydrated_loader");
			let inputs = [
				LoaderInputSpec::path("project_id"),
				LoaderInputSpec::query("tab"),
			];
			let cache_id = loader_cache_id(loader_id, &route_context, &inputs).expect("cache key");
			let mut state = SsrState::new();
			state.add_route_loader_query_state(&cache_id, "SSR loader value");
			state.add_route_loader_state(loader_id.as_str(), "SSR loader value");
			let hydration = HydrationContext::from_state(state);
			let fetches = Rc::new(Cell::new(0));

			let prepared = seed_loader_query::<String>(
				&query_client,
				loader_id,
				&route_context,
				&inputs,
				&hydration,
				{
					let fetches = Rc::clone(&fetches);
					move |_cancellation| {
						let fetches = Rc::clone(&fetches);
						Box::pin(async move {
							fetches.set(fetches.get() + 1);
							Ok::<_, RouteLoaderError>("client refetch".to_string())
						})
					}
				},
			)
			.expect("seeded SSR query");
			let store = LoaderStore::new();
			store.insert_prepared(prepared);
			assert_eq!(
				store
					.get::<String>(loader_id)
					.expect("hydrated route value"),
				Loader("SSR loader value".to_string())
			);
			let query_key =
				QueryFamily::<String, String, RouteLoaderError>::new(loader_id.as_str())
					.key(cache_id);
			assert_eq!(fetches.get(), 0, "the hydrated query must remain fresh");

			// Act
			query_client.invalidate(&query_key);
			runtime.run_until_stalled();

			// Assert
			assert_eq!(
				fetches.get(),
				1,
				"the hydrated mounted route must refetch when invalidated"
			);
			drop(store);
			runtime.run_due_maintenance();
			assert!(
				!query_client.contains_for_test(&query_key),
				"dropping the final mounted route owner must release the query"
			);
		});
	}
}
