//! URL configuration for api app

use reinhardt::ServerRouter;

use super::views;

pub fn url_patterns() -> ServerRouter {
	ServerRouter::new()
		.endpoint(views::list_articles)
		.endpoint(views::create_article)
		.endpoint(views::get_article)
		.endpoint(views::update_article)
		.endpoint(views::delete_article)
}
