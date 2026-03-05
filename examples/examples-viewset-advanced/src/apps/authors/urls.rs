//! URL patterns for the authors app.

use reinhardt::ServerRouter;

use super::views;

pub fn url_patterns() -> ServerRouter {
	ServerRouter::new()
		.endpoint(views::list_authors)
		.endpoint(views::retrieve_author)
		.endpoint(views::activate_author)
		.endpoint(views::recent_authors)
}
