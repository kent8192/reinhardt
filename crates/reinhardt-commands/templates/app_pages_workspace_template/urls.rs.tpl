//! URL configuration for {{ app_name }} app.
//!
//! Routes can be registered in either of two ways (they merge):
//! 1. Inline here, via `unified_url_patterns` below.
//! 2. In submodules under `urls/` (`server_urls`, `client_urls`, `ws_urls`).

#[cfg(server)]
pub mod server_urls;

#[cfg(client)]
pub mod client_urls;

#[cfg(server)]
pub mod ws_urls;

use reinhardt::url_patterns;
use reinhardt::UnifiedRouter;

use {{ project_crate_name }}::config::apps::InstalledApp;

#[url_patterns(InstalledApp::{{ app_name }}, mode = unified)]
pub fn unified_url_patterns() -> UnifiedRouter {
    UnifiedRouter::new()
    // Example:
    //     .server(|s| s.endpoint(views::index))
    //     .client(|c| c.named_route("home", "/", || {}))
}
