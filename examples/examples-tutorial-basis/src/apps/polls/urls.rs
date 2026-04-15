use reinhardt::ServerRouter;

use super::views;

// Note: This function is called by config/urls.rs via .mount("/polls/", ...).
// Do NOT add #[routes] here - that would create a duplicate registration
// without the mount prefix.
pub fn routes() -> ServerRouter {
	ServerRouter::new()
		.endpoint(views::index)
		.endpoint(views::detail)
		.endpoint(views::results)
		.endpoint(views::vote)
}
