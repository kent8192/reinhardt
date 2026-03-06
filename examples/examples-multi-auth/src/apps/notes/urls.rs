//! Note URL patterns

use reinhardt::ServerRouter;

use super::views;

/// Build URL patterns for note endpoints
pub fn url_patterns() -> ServerRouter {
	ServerRouter::new()
		.endpoint(views::list_notes)
		.endpoint(views::create_note)
		.endpoint(views::get_note)
		.endpoint(views::delete_note)
}
