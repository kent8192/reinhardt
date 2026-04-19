//! URL configuration for examples-tutorial-rest project
//!
//! The `routes` function defines all URL patterns for this project.

use reinhardt::prelude::*;
use reinhardt::routes;

#[routes(standalone)]
pub fn routes() -> UnifiedRouter {
	UnifiedRouter::new().mount("/api/", crate::apps::snippets::urls::url_patterns())
}
