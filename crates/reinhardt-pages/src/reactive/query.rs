//! Keyed async query cache hooks.

mod browser;
mod canonical_json;
mod client;
mod context;
mod hook;
mod identity;
mod runtime;
mod state;

#[cfg(test)]
mod tests;

pub use client::QueryClient;
pub(crate) use client::{
	QueryAcquireOptions, QueryConsumer, QueryErrorPolicy, QueryLease, acquire_query,
	seed_query_from_serialized,
};
pub use context::queries;
pub(crate) use context::{
	QueryClientGuard, provide_query_client, with_query_client, with_query_client_async,
};
pub use hook::{QueryHandle, use_mutation, use_query};
pub use identity::{QueryDescriptor, QueryFamily, QueryKey};
pub use state::{QueryDefaults, QueryOptions, QueryPhase};
