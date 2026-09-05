//! Keyed async query cache hooks.

mod browser;
pub(crate) mod canonical_json;
mod client;
mod context;
mod hook;
mod identity;
mod retry;
mod runtime;
mod state;

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests;

#[cfg(any(feature = "testing", test))]
pub use browser::QueryBrowserResourceProbe;
#[cfg(native)]
pub(crate) use client::NormalizedRecipeRefresh;
pub use client::QueryClient;
#[cfg(all(test, not(target_arch = "wasm32")))]
pub(crate) use client::TestQueryRuntime;
#[cfg(test)]
pub(super) use client::acquire_query;
pub(crate) use client::{
	QueryAcquireOptions, QueryConsumer, QueryErrorPolicy, QueryLease, QueryResultError,
};
#[cfg(any(feature = "testing", test))]
pub use client::{
	query_browser_resource_counts, query_browser_resource_probe_for_test,
	set_query_visibility_for_test,
};
#[cfg(test)]
pub(crate) use context::QueryClientGuard;
#[cfg(any(wasm, test))]
pub(crate) use context::provide_query_client;
pub use context::queries;
pub(crate) use context::{current_query_client, with_query_client, with_query_client_async};
pub use hook::{QueryHandle, use_query};
pub use identity::{QueryDescriptor, QueryFamily, QueryKey};
pub use retry::{NoRetry, QueryRetryConfig, RetryPolicy};
pub use state::{QueryDefaults, QueryOptions, QuerySnapshot, QueryStatus};
