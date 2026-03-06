//! URL configuration for examples-viewset-advanced project
//!
//! Demonstrates advanced ViewSet routing patterns including nested resources.

use reinhardt::prelude::*;
use reinhardt::routes;

#[routes]
pub fn routes() -> UnifiedRouter {
	UnifiedRouter::new()
		// Authors: GenericViewSet with composable Mixins + custom actions
		.mount("/api/authors/", crate::apps::authors::urls::url_patterns())
		// Books: ReadOnlyModelViewSet with caching
		.mount("/api/books/", crate::apps::books::urls::url_patterns())
		// Articles: Full ModelViewSet with batch, middleware, PATCH, DI
		.mount(
			"/api/articles/",
			crate::apps::articles::urls::url_patterns(),
		)
		// Nested: Articles under an author
		.mount(
			"/api/authors/{author_id}/articles/",
			crate::apps::articles::urls::nested_url_patterns(),
		)
}
