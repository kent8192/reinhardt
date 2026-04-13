//! URL configuration for {{ app_name }} app (Pages)

use reinhardt::url_patterns;
use reinhardt::UnifiedRouter;

#[url_patterns]
pub fn url_patterns() -> UnifiedRouter {
    UnifiedRouter::new()
}
