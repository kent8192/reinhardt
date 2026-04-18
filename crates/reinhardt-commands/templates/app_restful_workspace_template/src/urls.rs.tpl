//! URL configuration for {{ app_name }} app (RESTful)

use reinhardt::url_patterns;
use reinhardt::ServerRouter;

use {{ project_crate_name }}::config::apps::InstalledApp;

#[url_patterns(InstalledApp::{{ app_name }}, mode = server)]
pub fn server_url_patterns() -> ServerRouter {
    ServerRouter::new()
}
