//! WebSocket routes for the {{ app_name }} application.

use reinhardt::WebSocketRouter;

/// Return the WebSocket routes contributed by this application.
pub fn ws_url_patterns() -> WebSocketRouter {
	WebSocketRouter::new()
}
