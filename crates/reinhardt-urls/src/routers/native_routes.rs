//! Native route aggregate passed from `#[routes]` to server startup.

use crate::routers::ServerRouter;
use reinhardt_core::ws::WebSocketRouter;
use reinhardt_di::{DiRegistrationList, InjectionContext};
use std::sync::Arc;

/// HTTP ownership shape inside a native route aggregate.
#[doc(hidden)]
pub enum NativeHttpRoutes {
	/// HTTP router owned by a native `UnifiedRouter` aggregate.
	Owned(Box<ServerRouter>),
	/// Shared HTTP router produced by a legacy registration factory.
	LegacyShared(Arc<ServerRouter>),
}

/// Complete native route payload materialized from a registration factory.
#[doc(hidden)]
pub struct NativeRoutes {
	/// HTTP routes and their ownership shape.
	pub server: NativeHttpRoutes,
	/// WebSocket consumer routes.
	pub websocket: WebSocketRouter,
	/// Per-consumer DI contexts retained from merged child routers.
	pub websocket_contexts: Vec<(
		reinhardt_core::ws::WebSocketConsumerKey,
		Arc<InjectionContext>,
	)>,
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
	#[doc(hidden)]
	pub fn from_legacy(server: Arc<ServerRouter>) -> Self {
		let di_context = server.di_context().cloned();
		Self {
			server: NativeHttpRoutes::LegacyShared(server),
			websocket: WebSocketRouter::new(),
			websocket_contexts: Vec::new(),
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

	pub(crate) fn into_legacy_server(self) -> Arc<ServerRouter> {
		let Self {
			server,
			di_context,
			di_registrations,
			..
		} = self;
		let mut server = match server {
			NativeHttpRoutes::Owned(server) => *server,
			NativeHttpRoutes::LegacyShared(server) => return server,
		};

		match di_context {
			Some(context) => {
				if server.di_context().is_none() {
					server = server.with_di_context(Arc::clone(&context));
				}
				if !di_registrations.is_empty() {
					di_registrations.apply_to(context.singleton_scope());
				}
			}
			None if !di_registrations.is_empty() => {
				crate::routers::register_di_registrations(di_registrations);
			}
			None => {}
		}
		let errors = server.register_all_routes();
		for error in &errors {
			tracing::warn!("{}", error);
		}
		Arc::new(server)
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
