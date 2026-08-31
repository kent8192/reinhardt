//! Runtime contracts for asynchronous route navigation guards.

use crate::cancellation::{CancellationHandle, CancellationToken, scope_cancellation};
use crate::reactive::query::QueryRetryConfig;
use crate::reactive::{
	QueryAcquireOptions, QueryClient, QueryConsumer, QueryDescriptor, QueryErrorPolicy,
	QueryOptions, QueryResultError,
};
use crate::router::loader::RouteLoaderError;
use reinhardt_urls::routers::client_router::{NavigationGuardId, RouteContext};
use serde::{Serialize, de::DeserializeOwned};
use std::cell::RefCell;
use std::error::Error;
use std::fmt;
use std::rc::Rc;

/// Contract implemented by the marker generated for a navigation guard.
pub trait NavigationGuard {
	/// Stable identifier used in route metadata and registration.
	const ID: NavigationGuardId;
}

/// The lifecycle that initiated a navigation attempt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum NavigationKind {
	/// Initial route rendering.
	Initial,
	/// History push navigation.
	Push,
	/// History replacement navigation.
	Replace,
	/// Browser history traversal.
	Pop,
	/// Link prefetch.
	Prefetch,
}

/// The expected control-flow result of a navigation guard.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum NavigationDecision {
	/// Continue route preparation.
	Allow,
	/// Start a new navigation without committing the current destination.
	Redirect {
		/// Destination for the replacement navigation.
		location: String,
		/// Whether the redirect replaces the current history entry.
		replace: bool,
	},
	/// Select the normal unmatched-route surface.
	NotFound,
	/// Select the navigation forbidden surface.
	Forbidden,
}

/// A safe navigation-guard failure with an optional server-side diagnostic.
#[derive(Clone)]
pub struct NavigationGuardError(RouteLoaderError);

impl NavigationGuardError {
	/// Creates an error with a browser-safe public message.
	pub fn new(message: impl Into<String>) -> Self {
		Self(RouteLoaderError::new(message))
	}

	/// Creates an error with a browser-safe message and HTTP-like status.
	pub fn with_status(message: impl Into<String>, status: u16) -> Self {
		Self(RouteLoaderError::with_status(message, status))
	}

	/// Creates an error while retaining an application diagnostic cause.
	pub fn from_diagnostic<E>(message: impl Into<String>, status: Option<u16>, error: E) -> Self
	where
		E: Error + 'static,
	{
		Self(RouteLoaderError::from_diagnostic(message, status, error))
	}

	/// Returns the browser-safe public message.
	pub fn public_message(&self) -> &str {
		self.0.public_message()
	}

	/// Returns the optional status code.
	pub fn status(&self) -> Option<u16> {
		self.0.status()
	}

	/// Returns the retained diagnostic cause, when one exists.
	pub fn diagnostic(&self) -> Option<&(dyn Error + 'static)> {
		self.0.diagnostic()
	}
}

impl fmt::Debug for NavigationGuardError {
	fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
		self.0.fmt(formatter)
	}
}

impl fmt::Display for NavigationGuardError {
	fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
		self.0.fmt(formatter)
	}
}

impl Error for NavigationGuardError {
	fn source(&self) -> Option<&(dyn Error + 'static)> {
		self.0.source()
	}
}

impl From<RouteLoaderError> for NavigationGuardError {
	fn from(error: RouteLoaderError) -> Self {
		Self(error)
	}
}

impl From<NavigationGuardError> for RouteLoaderError {
	fn from(error: NavigationGuardError) -> Self {
		error.0
	}
}

#[cfg(native)]
pub(crate) type NavigationGuardHydrationCollector = Rc<RefCell<Vec<(String, serde_json::Value)>>>;

/// Read-only state passed to one navigation guard invocation.
#[derive(Clone)]
pub struct NavigationContext {
	destination: String,
	route_context: RouteContext,
	navigation_kind: NavigationKind,
	query_client: QueryClient,
	cancellation: CancellationHandle,
	consumer: QueryConsumer,
	#[cfg(native)]
	hydration_collector: Option<NavigationGuardHydrationCollector>,
}

impl NavigationContext {
	/// Creates a context owned by a router navigation attempt.
	pub(crate) fn new(
		destination: String,
		route_context: RouteContext,
		navigation_kind: NavigationKind,
		query_client: QueryClient,
		cancellation: CancellationHandle,
		consumer: QueryConsumer,
		#[cfg(native)] hydration_collector: Option<NavigationGuardHydrationCollector>,
	) -> Self {
		Self {
			destination,
			route_context,
			navigation_kind,
			query_client,
			cancellation,
			consumer,
			#[cfg(native)]
			hydration_collector,
		}
	}

	/// Returns the complete destination, including its query string.
	pub fn destination(&self) -> &str {
		&self.destination
	}

	/// Returns the matched route context.
	pub fn route_context(&self) -> &RouteContext {
		&self.route_context
	}

	/// Returns the lifecycle that initiated this navigation.
	pub fn navigation_type(&self) -> NavigationKind {
		self.navigation_kind
	}

	/// Returns the cancellation token for this navigation attempt.
	pub fn cancellation_token(&self) -> CancellationToken {
		CancellationToken(self.cancellation.clone())
	}

	/// Acquires and awaits a query using this navigation's shared cache entry.
	pub async fn query<T, E, R>(
		&self,
		descriptor: QueryDescriptor<T, E>,
		options: QueryOptions<R>,
	) -> Result<T, NavigationGuardError>
	where
		T: Clone + Serialize + DeserializeOwned + 'static,
		E: Clone + Serialize + DeserializeOwned + Into<NavigationGuardError> + 'static,
		R: QueryRetryConfig<E>,
	{
		#[cfg(wasm)]
		if let Ok(mut hydration) = crate::hydration::HydrationContext::from_window() {
			hydration
				.seed_query_descriptor(&self.query_client, &descriptor)
				.unwrap_or_else(|error| {
					panic!(
						"query hydration payload `{}` is invalid: {error}",
						descriptor.key().hydration_id()
					)
				});
		}
		let lease = self.query_client.acquire_with_options(
			descriptor,
			QueryAcquireOptions {
				consumer: self.consumer,
				error_policy: QueryErrorPolicy::Retain,
			},
			options,
		);
		let cancellation = self.cancellation.clone();
		let result = scope_cancellation(self.cancellation.clone(), lease.result()).await;
		if cancellation.is_cancelled() {
			return Err(NavigationGuardError::new(
				"navigation guard query was cancelled",
			));
		}
		let value = result.map_err(|error| match error {
			QueryResultError::Fetch(error) => error.into(),
			QueryResultError::Evicted => {
				NavigationGuardError::with_status("navigation guard query was evicted", 500)
			}
		})?;
		#[cfg(native)]
		if let Some(collector) = &self.hydration_collector
			&& let Some(snapshot) = lease.hydration_snapshot_value()
		{
			collector
				.borrow_mut()
				.push((lease.hydration_key().to_string(), snapshot));
		}
		Ok(value)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn guard_ids_compare_by_stable_value() {
		assert_eq!(
			NavigationGuardId::new("session"),
			NavigationGuardId::new("session")
		);
		assert_ne!(
			NavigationGuardId::new("session"),
			NavigationGuardId::new("permission")
		);
	}

	#[test]
	fn errors_keep_diagnostics_out_of_the_public_message() {
		let error = NavigationGuardError::from_diagnostic(
			"navigation guard failed",
			Some(500),
			std::io::Error::other("database credential"),
		);
		assert_eq!(error.public_message(), "navigation guard failed");
		assert_eq!(error.status(), Some(500));
		assert_eq!(
			error.diagnostic().unwrap().to_string(),
			"database credential"
		);
	}
}
