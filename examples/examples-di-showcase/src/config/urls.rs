//! URL configuration for examples-di-showcase

use reinhardt::prelude::*;
use reinhardt::routes;

use super::views;

#[routes]
pub fn routes() -> UnifiedRouter {
	UnifiedRouter::new()
		.endpoint(views::root)
		.endpoint(views::health)
		.mount("/", crate::apps::di_showcase::urls::url_patterns())
}
