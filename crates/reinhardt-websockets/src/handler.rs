//! WebSocket handler

use crate::connection::Message;
use crate::connection::WebSocketResult;
use crate::reconnection::ReconnectionConfig;

/// WebSocket handler trait
pub trait WebSocketHandler: Send + Sync {
	/// Handle incoming message
	fn on_message(
		&self,
		message: Message,
	) -> impl std::future::Future<Output = WebSocketResult<()>> + Send;

	/// Handle connection open
	fn on_connect(&self) -> impl std::future::Future<Output = WebSocketResult<()>> + Send;

	/// Handle connection close
	fn on_disconnect(&self) -> impl std::future::Future<Output = WebSocketResult<()>> + Send;

	/// Returns the reconnection configuration for this handler.
	///
	/// Override this method to enable automatic reconnection with custom settings.
	/// Returns `None` by default, meaning no automatic reconnection is performed.
	///
	/// # Examples
	///
	/// ```rust,no_run
	/// use reinhardt_websockets::handler::WebSocketHandler;
	/// use reinhardt_websockets::{Message, WebSocketResult};
	/// use reinhardt_websockets::reconnection::ReconnectionConfig;
	/// use std::time::Duration;
	///
	/// struct MyHandler;
	///
	/// impl WebSocketHandler for MyHandler {
	///     # async fn on_message(&self, _: Message) -> WebSocketResult<()> { Ok(()) }
	///     # async fn on_connect(&self) -> WebSocketResult<()> { Ok(()) }
	///     # async fn on_disconnect(&self) -> WebSocketResult<()> { Ok(()) }
	///     fn reconnection_config(&self) -> Option<ReconnectionConfig> {
	///         Some(ReconnectionConfig::default()
	///             .with_max_attempts(5)
	///             .with_initial_delay(Duration::from_secs(1)))
	///     }
	/// }
	/// ```
	fn reconnection_config(&self) -> Option<ReconnectionConfig> {
		None
	}

	/// Called when a reconnection attempt is about to be made.
	///
	/// This hook allows handlers to perform any setup needed before
	/// attempting to reconnect (e.g., refreshing tokens, logging).
	///
	/// The `attempt` parameter indicates which attempt this is (1-based).
	///
	/// Returns `Ok(())` to proceed with reconnection, or `Err` to abort.
	fn on_reconnecting(
		&self,
		_attempt: u32,
	) -> impl std::future::Future<Output = WebSocketResult<()>> + Send {
		async { Ok(()) }
	}

	/// Called when a reconnection attempt succeeds.
	///
	/// This hook allows handlers to perform post-reconnection setup
	/// (e.g., resubscribing to channels, restoring state).
	fn on_reconnected(&self) -> impl std::future::Future<Output = WebSocketResult<()>> + Send {
		async { Ok(()) }
	}

	/// Called when all reconnection attempts have been exhausted.
	///
	/// This hook allows handlers to perform cleanup when reconnection
	/// is no longer possible (e.g., notifying the user, releasing resources).
	fn on_reconnect_failed(
		&self,
		_total_attempts: u32,
	) -> impl std::future::Future<Output = WebSocketResult<()>> + Send {
		async { Ok(()) }
	}
}
