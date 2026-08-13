//! Executable WebSocket consumer registrations.

use std::{future::Future, pin::Pin, sync::Arc};

use reinhardt_di::{DiError, InjectionContext};
use thiserror::Error;

use crate::{WebSocketConsumer, WebSocketConsumerKey};

/// Future returned by an executable WebSocket consumer factory.
pub type ConsumerBuildFuture =
	Pin<Box<dyn Future<Output = Result<Box<dyn WebSocketConsumer>, ConsumerBuildError>> + Send>>;

/// Future returned by a WebSocket consumer dependency preflight.
pub type ConsumerPreflightFuture =
	Pin<Box<dyn Future<Output = Result<(), ConsumerBuildError>> + Send>>;

/// Error returned when a WebSocket consumer dependency cannot be resolved.
#[derive(Debug, Error)]
#[error(
	"failed to build WebSocket consumer `{consumer_source}`: dependency `{dependency_type}`: {cause}"
)]
pub struct ConsumerBuildError {
	consumer_source: &'static str,
	dependency_type: &'static str,
	#[source]
	cause: DiError,
}

impl ConsumerBuildError {
	/// Creates an error for a dependency resolution failure.
	pub fn new(
		consumer_source: &'static str,
		dependency_type: &'static str,
		cause: DiError,
	) -> Self {
		Self {
			consumer_source,
			dependency_type,
			cause,
		}
	}

	/// Returns the module-qualified consumer source.
	pub const fn consumer_source(&self) -> &'static str {
		self.consumer_source
	}

	/// Returns the dependency type that failed to resolve.
	pub const fn dependency_type(&self) -> &'static str {
		self.dependency_type
	}
}

/// Executable factory registered for a WebSocket consumer key.
pub struct WebSocketConsumerRegistration {
	/// Key shared with the structural WebSocket route.
	pub key: WebSocketConsumerKey,
	/// Module-qualified handler source used for diagnostics.
	pub source: &'static str,
	/// Validates dependencies without constructing a consumer.
	pub preflight: fn(Arc<InjectionContext>) -> ConsumerPreflightFuture,
	/// Constructs a consumer from the supplied dependency injection context.
	pub build: fn(Arc<InjectionContext>) -> ConsumerBuildFuture,
}

impl WebSocketConsumerRegistration {
	/// Creates an executable registration for a manually implemented consumer.
	pub const fn new(
		key: WebSocketConsumerKey,
		source: &'static str,
		preflight: fn(Arc<InjectionContext>) -> ConsumerPreflightFuture,
		build: fn(Arc<InjectionContext>) -> ConsumerBuildFuture,
	) -> Self {
		Self {
			key,
			source,
			preflight,
			build,
		}
	}
}

inventory::collect!(WebSocketConsumerRegistration);
