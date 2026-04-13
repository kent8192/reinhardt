//! URL configuration for {{ app_name }} app (RESTful)

use reinhardt::url_patterns;
use reinhardt::ServerRouter;

#[url_patterns]
pub fn url_patterns() -> ServerRouter {
    ServerRouter::new()
}
