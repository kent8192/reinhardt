//! Inventory-backed route-loader registration and erased execution.

use super::loader::{LoaderInputSpec, PreparedLoader, RouteLoaderError};
use crate::HydrationContext;
use crate::cancellation::CancellationHandle;
use crate::reactive::{QueryClient, QueryConsumer};
use reinhardt_urls::routers::client_router::{RouteContext, RouteLoaderId};
use std::collections::HashMap;
use std::fmt;
use std::future::Future;
use std::pin::Pin;

/// Erased future returned by a registered route loader.
pub type LoaderFuture =
	Pin<Box<dyn Future<Output = Result<PreparedLoader, RouteLoaderError>> + 'static>>;

/// Publicly nameable consumer context passed to an erased loader executor.
///
/// The pages query cache remains the single owner of the corresponding
/// internal lease policy; this enum only keeps proc-macro-generated function
/// signatures usable across the crate boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LoaderConsumer {
	/// Work started by a link prefetch.
	Prefetch,
	/// Work belonging to a navigation generation.
	Navigation(u64),
	/// Work retained by a mounted route generation.
	MountedRoute(u64),
	/// Work retained by a mounted query hook.
	MountedQuery,
	/// Maintenance or background work.
	Maintenance,
}

impl From<LoaderConsumer> for QueryConsumer {
	fn from(consumer: LoaderConsumer) -> Self {
		match consumer {
			LoaderConsumer::Prefetch => Self::Prefetch,
			LoaderConsumer::Navigation(generation) => Self::Navigation(generation),
			LoaderConsumer::MountedRoute(generation) => Self::MountedRoute(generation),
			LoaderConsumer::MountedQuery => Self::MountedQuery,
			LoaderConsumer::Maintenance => Self::Maintenance,
		}
	}
}

/// Erased loader executor submitted by the `#[loader]` macro.
pub type LoaderExecutor = fn(&RouteContext, CancellationHandle, LoaderConsumer) -> LoaderFuture;

/// Erased query-cache seeder used during route-loader hydration.
pub type LoaderQuerySeeder =
	fn(&QueryClient, &RouteContext, &HydrationContext) -> Result<PreparedLoader, RouteLoaderError>;

/// Static registration record for one route loader.
pub struct LoaderRegistration {
	/// Stable loader identifier.
	pub id: RouteLoaderId,
	/// Declaration-ordered path/query inputs.
	pub inputs: &'static [LoaderInputSpec],
	/// Erased execution entry point.
	pub execute: LoaderExecutor,
}

impl LoaderRegistration {
	/// Creates a static registration record.
	pub const fn new(
		id: RouteLoaderId,
		inputs: &'static [LoaderInputSpec],
		execute: LoaderExecutor,
	) -> Self {
		Self {
			id,
			inputs,
			execute,
		}
	}
}

inventory::collect!(LoaderRegistration);

/// Static query-cache seeding registration generated alongside a `#[loader]`
/// marker.
pub struct LoaderQueryHydrationRegistration {
	/// Stable loader identifier.
	pub id: RouteLoaderId,
	/// Seeds the matching typed query-cache entry from SSR state.
	pub seed_query: LoaderQuerySeeder,
}

impl LoaderQueryHydrationRegistration {
	/// Creates a query-cache seeding registration.
	pub const fn new(id: RouteLoaderId, seed_query: LoaderQuerySeeder) -> Self {
		Self { id, seed_query }
	}
}

inventory::collect!(LoaderQueryHydrationRegistration);

/// Duplicate or lookup errors in the loader registry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LoaderRegistryError {
	/// Multiple registrations use the same stable ID.
	Duplicate(RouteLoaderId),
	/// No registration exists for the requested ID.
	Missing(RouteLoaderId),
}

impl fmt::Display for LoaderRegistryError {
	fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Self::Duplicate(id) => write!(formatter, "duplicate route loader `{}`", id.as_str()),
			Self::Missing(id) => write!(
				formatter,
				"route loader `{}` is not registered",
				id.as_str()
			),
		}
	}
}

impl std::error::Error for LoaderRegistryError {}

/// Read-only lookup table for erased loader registrations.
pub struct LoaderRegistry {
	entries: HashMap<RouteLoaderId, &'static LoaderRegistration>,
	query_seeders: HashMap<RouteLoaderId, LoaderQuerySeeder>,
}

impl LoaderRegistry {
	/// Builds a registry and rejects duplicate IDs.
	pub fn from_entries<I>(entries: I) -> Result<Self, LoaderRegistryError>
	where
		I: IntoIterator<Item = &'static LoaderRegistration>,
	{
		let mut indexed = HashMap::new();
		for entry in entries {
			if indexed.insert(entry.id, entry).is_some() {
				return Err(LoaderRegistryError::Duplicate(entry.id));
			}
		}
		Ok(Self {
			entries: indexed,
			query_seeders: HashMap::new(),
		})
	}

	/// Collects all inventory registrations for the current application.
	pub fn global() -> Result<Self, LoaderRegistryError> {
		let mut registry = Self::from_entries(inventory::iter::<LoaderRegistration>)?;
		for registration in inventory::iter::<LoaderQueryHydrationRegistration> {
			registry
				.query_seeders
				.insert(registration.id, registration.seed_query);
		}
		Ok(registry)
	}

	/// Looks up a registration by stable ID.
	pub fn get(
		&self,
		id: RouteLoaderId,
	) -> Result<&'static LoaderRegistration, LoaderRegistryError> {
		self.entries
			.get(&id)
			.copied()
			.ok_or(LoaderRegistryError::Missing(id))
	}

	/// Returns the number of registered loaders.
	pub fn len(&self) -> usize {
		self.entries.len()
	}

	/// Returns whether no loaders are registered.
	pub fn is_empty(&self) -> bool {
		self.entries.is_empty()
	}

	/// Hydrates one loader and acquires its mounted-route query lease.
	#[doc(hidden)]
	pub fn seed_hydrated_query(
		&self,
		client: &QueryClient,
		id: RouteLoaderId,
		context: &RouteContext,
		hydration: &HydrationContext,
	) -> Result<PreparedLoader, RouteLoaderError> {
		let seed_query = self.query_seeders.get(&id).ok_or_else(|| {
			RouteLoaderError::with_status(
				format!(
					"route loader `{}` has no hydration query seeder",
					id.as_str()
				),
				500,
			)
		})?;
		seed_query(client, context, hydration)
	}
}

/// Executes one loader from an application registry.
pub async fn execute_loader(
	registry: &LoaderRegistry,
	id: RouteLoaderId,
	context: &RouteContext,
	cancellation: CancellationHandle,
	consumer: LoaderConsumer,
) -> Result<PreparedLoader, RouteLoaderError> {
	let registration = registry
		.get(id)
		.map_err(|error| RouteLoaderError::with_status(error.to_string(), 500))?;
	(registration.execute)(context, cancellation, consumer).await
}

#[cfg(test)]
mod tests {
	use super::*;

	fn unused_executor(
		_context: &RouteContext,
		_cancellation: CancellationHandle,
		_consumer: LoaderConsumer,
	) -> LoaderFuture {
		Box::pin(async { Err(RouteLoaderError::new("unused")) })
	}

	#[test]
	fn duplicate_loader_ids_are_rejected() {
		let first: &'static LoaderRegistration = Box::leak(Box::new(LoaderRegistration::new(
			RouteLoaderId::new("duplicate"),
			&[],
			unused_executor,
		)));
		let second: &'static LoaderRegistration = Box::leak(Box::new(LoaderRegistration::new(
			RouteLoaderId::new("duplicate"),
			&[],
			unused_executor,
		)));
		assert!(matches!(
			LoaderRegistry::from_entries([first, second]),
			Err(LoaderRegistryError::Duplicate(id)) if id == RouteLoaderId::new("duplicate")
		));
	}
}
