//! Keyed async query cache hooks.

mod browser;
mod canonical_json;
mod client;
mod hook;
mod identity;
mod runtime;
mod state;

#[cfg(test)]
mod tests;

#[cfg(test)]
pub(crate) use client::clear_query_cache_for_test;
pub(crate) use client::{
	QueryAcquireOptions, QueryConsumer, QueryErrorPolicy, QueryLease, acquire_query,
	seed_query_from_serialized,
};
pub use hook::{QueryHandle, use_mutation, use_query};
pub use identity::{QueryDescriptor, QueryFamily, QueryKey};
pub use state::{QueryDefaults, QueryOptions, QueryPhase};
