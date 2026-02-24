//! URL configuration for example-rest-api project (RESTful)
//!
//! The `routes` function defines all URL patterns for this project.

use reinhardt::UnifiedRouter;
use reinhardt::routes;

use super::views;

#[routes]
pub fn routes() -> UnifiedRouter {
	UnifiedRouter::new()
		.endpoint(views::root)
		.endpoint(views::health)
		.endpoint(views::list_users)
		.mount("/api/", crate::apps::api::urls::url_patterns())
}
