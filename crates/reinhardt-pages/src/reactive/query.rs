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

pub(crate) use client::{
	QueryAcquireOptions, QueryConsumer, QueryErrorPolicy, QueryLease, acquire_query,
	seed_query_from_serialized,
};
pub use hook::{QueryHandle, use_mutation, use_query};
pub use identity::QueryKey;
pub use state::QueryPhase;
