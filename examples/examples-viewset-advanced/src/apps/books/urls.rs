//! URL patterns for the books app.

use reinhardt::ServerRouter;

use super::views;

pub fn url_patterns() -> ServerRouter {
	ServerRouter::new()
		.endpoint(views::list_books)
		.endpoint(views::retrieve_book)
		.endpoint(views::reject_create)
}
