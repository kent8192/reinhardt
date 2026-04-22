//! Client-side URL patterns for {{ app_name }}.

use reinhardt::url_patterns;
use reinhardt::ClientRouter;

use crate::config::apps::InstalledApp;

#[url_patterns(InstalledApp::{{ app_name }}, mode = client)]
pub fn client_url_patterns() -> ClientRouter {
    ClientRouter::new()
    // Register client routes here.
    // Example: .named_route("home", "/", || {})
}
