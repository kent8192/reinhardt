//! WebSocket routes for the {{ app_name }} application.
//!
//! Register handlers with their final absolute paths, such as
//! `#[websocket("/ws/chat/")]`; the app router is mounted at `/`.

use reinhardt::WebSocketRouter;

/// Return the WebSocket routes contributed by this application.
pub fn ws_url_patterns() -> WebSocketRouter {
	WebSocketRouter::new()
}
