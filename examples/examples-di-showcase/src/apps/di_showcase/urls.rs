//! URL routing for di_showcase app

use reinhardt::url_patterns;
use reinhardt::ServerRouter;

use crate::apps::di_showcase::views;

#[url_patterns]
pub fn url_patterns() -> ServerRouter {
	ServerRouter::new()
		.endpoint(views::config_info)
		.endpoint(views::greet_user)
		.endpoint(views::request_counter)
		.endpoint(views::uncached_injection)
		.endpoint(views::dashboard)
		.endpoint(views::multiple_deps)
		.endpoint(views::manual_injected)
		.endpoint(views::manual_uncached)
}
