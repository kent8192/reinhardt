//! WebSocket re-exports.

#[cfg(feature = "websockets-pages")]
pub use reinhardt_websockets::integration::pages::PagesAuthenticator;

pub use reinhardt_websockets::room::{BroadcastResult, Room, RoomError, RoomManager, RoomResult};

pub use reinhardt_websockets::{
	ConsumerBuildError, ConsumerBuildFuture, ConsumerContext, ConsumerPreflightFuture, Message,
	WebSocketConnection, WebSocketConsumer, WebSocketConsumerKey, WebSocketConsumerRegistration,
	WebSocketEndpointInfo, WebSocketEndpointMetadata, WebSocketError, WebSocketResult,
};

pub use reinhardt_websockets::{
	RouteError, RouteResult, WebSocketRoute, WebSocketRouter, clear_websocket_router,
	get_websocket_router, register_websocket_router, reverse_websocket_url,
};

#[cfg(all(test, feature = "routing"))]
mod tests {
	use super::*;
	use crate::reinhardt_websockets::reinhardt_di::{DiResult, Injectable, InjectionContext};

	#[derive(Clone)]
	struct FacadeOnlyDependency;

	#[crate::async_trait]
	impl Injectable for FacadeOnlyDependency {
		async fn inject(_context: &InjectionContext) -> DiResult<Self> {
			Ok(Self)
		}
	}

	#[crate::websocket("/ws/facade-only/")]
	async fn facade_only(
		_context: &mut ConsumerContext,
		_message: Message,
		#[inject] _dependency: FacadeOnlyDependency,
	) -> WebSocketResult<()> {
		Ok(())
	}

	#[test]
	fn injected_selector_compiles_without_the_root_di_feature() {
		let router = WebSocketRouter::new().consumer(facade_only);

		assert_eq!(
			router.routes()[0].consumer_key(),
			FacadeOnlyConsumer::consumer_key()
		);
	}
}
