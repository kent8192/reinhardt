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
#[cfg(test)]
pub(crate) use client::TestQueryRuntime;
#[cfg(test)]
pub(super) use client::acquire_query;
pub(crate) use client::{QueryAcquireOptions, QueryConsumer, QueryErrorPolicy, QueryLease};
pub(crate) use context::provide_query_client;
pub use context::queries;
#[cfg(native)]
pub(crate) use context::{QueryClientGuard, with_query_client, with_query_client_async};
pub use hook::{QueryHandle, use_query};
pub use identity::{QueryDescriptor, QueryFamily, QueryKey};
pub use state::{QueryDefaults, QueryOptions, QuerySnapshot, QueryStatus};
