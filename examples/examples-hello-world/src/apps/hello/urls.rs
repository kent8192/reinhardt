//! URL routing for hello app

use reinhardt::url_patterns;
use reinhardt::ServerRouter;

use crate::apps::hello::views;

#[url_patterns]
pub fn url_patterns() -> ServerRouter {
	ServerRouter::new()
		.endpoint(views::hello_world)
		.endpoint(views::health_check)
}
