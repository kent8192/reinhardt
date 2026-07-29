use std::any::{TypeId, type_name};
use std::fmt;
use std::fmt::Write as _;
use std::future::Future;
use std::marker::PhantomData;
use std::pin::Pin;
use std::rc::Rc;

use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::cancellation::CancellationHandle;

use super::canonical_json;

pub(super) type QueryFuture<T, E> = Pin<Box<dyn Future<Output = Result<T, E>> + 'static>>;
pub(super) type QueryFetcher<T, E> = dyn Fn(CancellationHandle) -> QueryFuture<T, E> + 'static;
type QueryDescriptorParts<T, E> = (
	QueryKey<T, E>,
	Rc<QueryFetcher<T, E>>,
	bool,
	QueryFamilyTypes,
);

/// Stable, type-erased identity shared by a query family and one argument set.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) struct QueryIdentity {
	family_id: &'static str,
	arguments_fingerprint: [u8; 32],
}

#[derive(Clone, Copy)]
pub(crate) struct QueryFamilyTypes {
	pub(crate) arguments: TypeId,
	pub(crate) data: TypeId,
	pub(crate) error: TypeId,
	pub(crate) arguments_name: &'static str,
	pub(crate) data_name: &'static str,
	pub(crate) error_name: &'static str,
}

impl QueryFamilyTypes {
	fn of<Args: 'static, T: 'static, E: 'static>() -> Self {
		Self {
			arguments: TypeId::of::<Args>(),
			data: TypeId::of::<T>(),
			error: TypeId::of::<E>(),
			arguments_name: type_name::<Args>(),
			data_name: type_name::<T>(),
			error_name: type_name::<E>(),
		}
	}

	pub(crate) fn matches(&self, other: &Self) -> bool {
		self.arguments == other.arguments && self.data == other.data && self.error == other.error
	}
}

/// Defines the typed identity shared by queries with the same arguments.
pub struct QueryFamily<Args, T, E> {
	id: &'static str,
	marker: PhantomData<fn(Args) -> Result<T, E>>,
}

impl<Args, T, E> Clone for QueryFamily<Args, T, E> {
	fn clone(&self) -> Self {
		*self
	}
}

impl<Args, T, E> Copy for QueryFamily<Args, T, E> {}

impl<Args, T, E> fmt::Debug for QueryFamily<Args, T, E> {
	fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
		formatter
			.debug_struct("QueryFamily")
			.field("id", &self.id)
			.finish_non_exhaustive()
	}
}

impl<Args, T, E> QueryFamily<Args, T, E> {
	/// Creates a typed query family with a stable application-wide identifier.
	pub const fn new(id: &'static str) -> Self {
		Self {
			id,
			marker: PhantomData,
		}
	}

	/// Returns this family's stable identifier.
	pub const fn id(&self) -> &'static str {
		self.id
	}

	/// Builds the exact typed key for one argument set.
	pub fn key(&self, args: Args) -> QueryKey<T, E>
	where
		Args: Serialize + 'static,
		T: 'static,
		E: 'static,
	{
		QueryKey {
			identity: QueryIdentity {
				family_id: self.id,
				arguments_fingerprint: fingerprint(&args),
			},
			family_types: QueryFamilyTypes::of::<Args, T, E>(),
			marker: PhantomData,
		}
	}

	/// Builds a query descriptor using a fetcher without cancellation input.
	pub fn query<F, Fut>(&self, args: Args, fetcher: F) -> QueryDescriptor<T, E>
	where
		Args: Serialize + 'static,
		T: 'static,
		E: 'static,
		F: Fn() -> Fut + 'static,
		Fut: Future<Output = Result<T, E>> + 'static,
	{
		self.query_with_cancellation(args, move |_| fetcher())
	}

	/// Builds a query descriptor whose fetcher receives its cancellation handle.
	pub fn query_with_cancellation<F, Fut>(&self, args: Args, fetcher: F) -> QueryDescriptor<T, E>
	where
		Args: Serialize + 'static,
		T: 'static,
		E: 'static,
		F: Fn(CancellationHandle) -> Fut + 'static,
		Fut: Future<Output = Result<T, E>> + 'static,
	{
		QueryDescriptor {
			key: self.key(args),
			fetcher: Rc::new(move |cancellation| Box::pin(fetcher(cancellation))),
			ssr_prefetch: true,
		}
	}
}

/// Exact typed cache identity for one query argument set.
pub struct QueryKey<T, E> {
	identity: QueryIdentity,
	family_types: QueryFamilyTypes,
	marker: PhantomData<fn() -> Result<T, E>>,
}

impl<T, E> Clone for QueryKey<T, E> {
	fn clone(&self) -> Self {
		Self {
			identity: self.identity.clone(),
			family_types: self.family_types,
			marker: PhantomData,
		}
	}
}

impl<T, E> fmt::Debug for QueryKey<T, E> {
	fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
		formatter
			.debug_struct("QueryKey")
			.field("identity", &self.identity)
			.finish_non_exhaustive()
	}
}

impl<T, E> PartialEq for QueryKey<T, E> {
	fn eq(&self, other: &Self) -> bool {
		self.identity == other.identity
	}
}

impl<T, E> Eq for QueryKey<T, E> {}

impl<T, E> QueryKey<T, E> {
	/// Returns the type-erased exact identity.
	#[allow(private_interfaces)]
	pub fn identity(&self) -> &QueryIdentity {
		&self.identity
	}

	/// Returns the stable query-family identifier.
	pub fn family_id(&self) -> &'static str {
		self.identity.family_id
	}

	/// Returns the deterministic cache and hydration ID.
	pub fn id(&self) -> String {
		let mut id = format!("{}:sha256:", self.identity.family_id);
		for byte in self.identity.arguments_fingerprint {
			write!(&mut id, "{byte:02x}").expect("writing a query ID to String must succeed");
		}
		id
	}

	pub(crate) fn family_types(&self) -> QueryFamilyTypes {
		self.family_types
	}
}

/// A typed query key paired with one observer-owned fetcher.
pub struct QueryDescriptor<T, E> {
	key: QueryKey<T, E>,
	pub(super) fetcher: Rc<QueryFetcher<T, E>>,
	pub(super) ssr_prefetch: bool,
}

impl<T, E> Clone for QueryDescriptor<T, E> {
	fn clone(&self) -> Self {
		Self {
			key: self.key.clone(),
			fetcher: Rc::clone(&self.fetcher),
			ssr_prefetch: self.ssr_prefetch,
		}
	}
}

impl<T, E> fmt::Debug for QueryDescriptor<T, E> {
	fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
		formatter
			.debug_struct("QueryDescriptor")
			.field("key", &self.key)
			.field("ssr_prefetch", &self.ssr_prefetch)
			.finish_non_exhaustive()
	}
}

impl<T, E> QueryDescriptor<T, E> {
	/// Returns the descriptor's exact typed key.
	pub fn key(&self) -> &QueryKey<T, E> {
		&self.key
	}

	/// Configures whether native SSR may prefetch this descriptor.
	pub fn with_ssr_prefetch(mut self, enabled: bool) -> Self {
		self.ssr_prefetch = enabled;
		self
	}

	pub(super) fn into_parts(self) -> QueryDescriptorParts<T, E> {
		let family_types = self.key.family_types();
		(self.key, self.fetcher, self.ssr_prefetch, family_types)
	}
}

fn fingerprint<Args: Serialize>(args: &Args) -> [u8; 32] {
	let encoded = canonical_json::encode(args)
		.expect("query family arguments must serialize into a stable cache key");
	Sha256::digest(encoded.as_bytes()).into()
}
