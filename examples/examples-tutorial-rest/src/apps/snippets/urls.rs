//! URL configuration for snippets app
//!
//! This module demonstrates two approaches for defining URL patterns:
//! 1. Function-based views (Tutorial 1-5) - Explicit endpoint registration
//! 2. ViewSet-based (Tutorial 6) - Automatic CRUD endpoint generation
//!
//! Switch between approaches using the USE_VIEWSET environment variable:
//! - Default: Function-based views
//! - USE_VIEWSET=1: ViewSet-based views

use reinhardt::ServerRouter;

use super::views;

pub fn url_patterns() -> ServerRouter {
	// Check which approach to use
	if std::env::var("USE_VIEWSET").is_ok() {
		// Option 2: ViewSet-based approach (Tutorial 6)
		// Automatically generates all CRUD endpoints with pagination, filtering, and ordering
		// - GET    /api/snippets-viewset/         - List all snippets (with pagination)
		// - POST   /api/snippets-viewset/         - Create a new snippet
		// - GET    /api/snippets-viewset/{id}/    - Retrieve a specific snippet
		// - PUT    /api/snippets-viewset/{id}/    - Update a snippet
		// - PATCH  /api/snippets-viewset/{id}/    - Partially update a snippet
		// - DELETE /api/snippets-viewset/{id}/    - Delete a snippet
		//
		// Additional query parameters:
		// - ?page=1&page_size=10                  - Pagination
		// - ?language=rust&title=hello            - Filtering
		// - ?ordering=created_at,-title           - Ordering (- for descending)
		ServerRouter::new().viewset("/snippets-viewset", views::SnippetViewSet::viewset())
	} else {
		// Option 1: Function-based approach (Tutorial 1-5)
		// Explicitly register each endpoint
		ServerRouter::new()
			.endpoint(views::list)
			.endpoint(views::create)
			.endpoint(views::retrieve)
			.endpoint(views::update)
			.endpoint(views::delete)
	}
}
