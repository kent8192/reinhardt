//! Admin configuration for api app

use crate::apps::api::models::Article;
use reinhardt::admin;

/// Admin configuration for Article model
///
/// This configures the admin panel display for Article items:
/// - List view with id, title, author, publication status, and creation date
/// - Filtering by publication status and creation date
/// - Search by title, author, and content
/// - Sorted by creation date (newest first)
/// - Read-only fields: created_at, updated_at
/// - 25 items per page
#[admin(model,
	for = Article,
	name = "Article",
	list_display = [id, title, author, published, created_at],
	list_filter = [published, created_at],
	search_fields = [title, author, content],
	ordering = [(created_at, desc)],
	readonly_fields = [created_at, updated_at],
	list_per_page = 25
)]
pub struct ArticleAdmin;
