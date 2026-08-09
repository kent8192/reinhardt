//! Native route aggregate passed from `#[routes]` to server startup.

use crate::routers::ServerRouter;
use reinhardt_core::ws::WebSocketRouter;
use reinhardt_di::{DiRegistrationList, InjectionContext};
use std::sync::Arc;

/// HTTP ownership shape inside a native route aggregate.
#[doc(hidden)]
pub enum NativeHttpRoutes {
	/// HTTP router owned by a native `UnifiedRouter` aggregate.
	Owned(ServerRouter),
	/// Shared HTTP router produced by a legacy registration factory.
	LegacyShared(Arc<ServerRouter>),
}

impl NativeHttpRoutes {
	pub(crate) fn into_shared(self) -> Arc<ServerRouter> {
		match self {
			Self::Owned(router) => Arc::new(router),
			Self::LegacyShared(router) => router,
		}
	}
}

/// Complete native route payload materialized from a registration factory.
#[doc(hidden)]
pub struct NativeRoutes {
	/// HTTP routes and their ownership shape.
	pub server: NativeHttpRoutes,
	/// WebSocket consumer routes.
	pub websocket: WebSocketRouter,
	/// gRPC service routes.
	#[cfg(feature = "grpc")]
	pub grpc: reinhardt_grpc::GrpcRouter,
	/// DI context already attached by the route function, if any.
	pub di_context: Option<Arc<InjectionContext>>,
	/// Deferred DI registrations in explicit merge order.
	pub di_registrations: DiRegistrationList,
	/// Streaming handlers in explicit merge order.
	#[cfg(feature = "streaming")]
	pub streaming_handlers: Vec<reinhardt_streaming::StreamingHandlerRegistration>,
}

impl NativeRoutes {
	pub(crate) fn from_legacy(server: Arc<ServerRouter>) -> Self {
		let di_context = server.di_context().cloned();
		Self {
			server: NativeHttpRoutes::LegacyShared(server),
			websocket: WebSocketRouter::new(),
			#[cfg(feature = "grpc")]
			grpc: reinhardt_grpc::GrpcRouter::new(),
			di_context,
			di_registrations: DiRegistrationList::new(),
			#[cfg(feature = "streaming")]
			streaming_handlers: Vec::new(),
		}
	}

	/// Install the context used by a DI-aware `#[routes]` root.
	#[doc(hidden)]
	pub fn __with_di_context(
		mut self,
		context: Arc<InjectionContext>,
	) -> Result<Self, NativeRouteError> {
		if self
			.di_context
			.as_ref()
			.is_some_and(|attached| !Arc::ptr_eq(attached, &context))
		{
			return Err(NativeRouteError::ConflictingDiContext);
		}
		self.di_context = Some(context);
		Ok(self)
	}
}

/// Error returned while establishing the native routing boundary.
#[doc(hidden)]
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum NativeRouteError {
	/// The route function returned a context other than the one used for injection.
	#[error("DI-aware #[routes] returned a router with a different InjectionContext")]
	ConflictingDiContext,
}
