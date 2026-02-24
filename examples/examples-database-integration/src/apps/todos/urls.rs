//! URL configuration for todos app

use reinhardt::ServerRouter;

use super::views;

pub fn url_patterns() -> ServerRouter {
	ServerRouter::new()
		.endpoint(views::list_todos)
		.endpoint(views::create_todo)
		.endpoint(views::get_todo)
		.endpoint(views::update_todo)
		.endpoint(views::delete_todo)
}
