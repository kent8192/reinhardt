//! Admin URL patterns

use reinhardt::ServerRouter;

use super::views;

/// Build URL patterns for admin endpoints
pub fn url_patterns() -> ServerRouter {
	ServerRouter::new()
		.endpoint(views::list_users)
		.endpoint(views::stats)
}
