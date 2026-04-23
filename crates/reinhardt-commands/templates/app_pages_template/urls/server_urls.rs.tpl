//! Server-side URL patterns for {{ app_name }}.

use reinhardt::url_patterns;
use reinhardt::ServerRouter;

use crate::config::apps::InstalledApp;

#[url_patterns(InstalledApp::{{ app_name }}, mode = server)]
pub fn server_url_patterns() -> ServerRouter {
    ServerRouter::new()
    // Register endpoints here.
    // Example: .endpoint(views::index)
}
