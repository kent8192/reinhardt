//! URL configuration for issues app
//!
//! Routes are handled by the unified GraphQL schema in config/urls.rs.

use reinhardt::url_patterns;
use reinhardt::ServerRouter;

/// Returns an empty router as issue routes are served via unified GraphQL schema
#[url_patterns]
pub fn url_patterns() -> ServerRouter {
	ServerRouter::new()
}
