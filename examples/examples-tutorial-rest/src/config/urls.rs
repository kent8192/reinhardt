//! URL configuration for examples-tutorial-rest project
//!
//! The `routes` function defines all URL patterns for this project.

use reinhardt::prelude::*;
use reinhardt::routes;

#[routes]
pub fn routes() -> UnifiedRouter {
	UnifiedRouter::new().mount(
		"/api/snippets/",
		crate::apps::snippets::urls::url_patterns(),
	)
}
