//! URL patterns for the articles app.

use reinhardt::ServerRouter;

use super::views;

/// Standard article URL patterns (top-level).
pub fn url_patterns() -> ServerRouter {
	ServerRouter::new()
		.endpoint(views::list_articles)
		.endpoint(views::retrieve_article)
		.endpoint(views::create_article)
		.endpoint(views::update_article)
		.endpoint(views::partial_update_article)
		.endpoint(views::delete_article)
		.endpoint(views::bulk_create_articles)
}

/// Nested URL patterns for articles under an author.
///
/// Mounted at `/api/authors/{author_id}/articles/`
pub fn nested_url_patterns() -> ServerRouter {
	ServerRouter::new().endpoint(views::list_author_articles)
}
