//! URL configuration for database-integration example (RESTful)
//!
//! The `routes` function defines all URL patterns for this project.

use reinhardt::UnifiedRouter;
use reinhardt::routes;

use super::views;

#[routes]
pub fn routes() -> UnifiedRouter {
	UnifiedRouter::new()
		.endpoint(views::list_users)
		.mount("/api/todos/", crate::apps::todos::urls::url_patterns())
}
