//! URL routing for the {{ app_name }} app.
//!
//! The `routes` function is mounted from the project-level `config/urls.rs`
//! via `.mount("/{{ app_name }}/", crate::apps::{{ app_name }}::urls::routes())`.
//! Do not annotate this function with `#[routes]` directly — that would
//! register it without the mount prefix.

use reinhardt::ServerRouter;

#[allow(unused_imports)] // `views` will be used once endpoints are added.
use super::views;

pub fn routes() -> ServerRouter {
	ServerRouter::new()
	// Register endpoints here, e.g.:
	//     .endpoint(views::index)
}
