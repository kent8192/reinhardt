//! WebSocket URL configuration for {{ app_name }} app
//!
//! Defines WebSocket consumer routes for this application.
//! The `#[url_patterns(mode = ws)]` macro generates the `ws_url_resolvers` module
//! consumed by the project-level `#[routes]` macro.
//!
//! # Example
//!
//! To register a WebSocket consumer:
//!
//! ```rust,ignore
//! use reinhardt::url_patterns;
//! use reinhardt::WebSocketRouter;
//! use crate::config::apps::InstalledApp;
//!
//! #[url_patterns(InstalledApp::{{ app_name }}, mode = ws)]
//! pub fn ws_url_patterns() -> WebSocketRouter {
//!     WebSocketRouter::new()
//!     // Register consumers via .consumer(handler)
//! }
//! ```

use reinhardt::url_patterns;
use reinhardt::WebSocketRouter;

use crate::config::apps::InstalledApp;

#[url_patterns(InstalledApp::{{ app_name }}, mode = ws)]
pub fn ws_url_patterns() -> WebSocketRouter {
    WebSocketRouter::new()
    // Register WebSocket consumers here.
    // Example: .consumer(chat_consumer)
}
