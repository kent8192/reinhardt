//! URL routing for hello app

use reinhardt::ServerRouter;

use crate::apps::hello::views;

pub fn url_patterns() -> ServerRouter {
	ServerRouter::new()
		.endpoint(views::hello_world)
		.endpoint(views::health_check)
}
