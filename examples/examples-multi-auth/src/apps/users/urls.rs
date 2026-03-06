//! User authentication URL patterns

use reinhardt::ServerRouter;

use super::views;

/// Build URL patterns for authentication endpoints
pub fn url_patterns() -> ServerRouter {
	ServerRouter::new()
		.endpoint(views::register)
		.endpoint(views::login)
		.endpoint(views::generate_token)
		.endpoint(views::me)
		.endpoint(views::logout)
}
