//! URL configuration for {{ app_name }} app (Pages)

use reinhardt::url_patterns;
use reinhardt::UnifiedRouter;

use crate::config::apps::InstalledApp;

#[url_patterns(InstalledApp::{{ app_name }}, mode = unified)]
pub fn unified_url_patterns() -> UnifiedRouter {
    UnifiedRouter::new()
}
