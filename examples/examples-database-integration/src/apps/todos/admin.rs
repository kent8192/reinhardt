//! Admin configuration for todos app

use crate::apps::todos::models::Todo;
use reinhardt::admin;

/// Admin configuration for Todo model
///
/// This configures the admin panel display for Todo items:
/// - List view with id, title, completion status, and creation date
/// - Filtering by completion status
/// - Search by title and description
/// - Sorted by creation date (newest first)
/// - 50 items per page
#[admin(model,
	for = Todo,
	name = "Todo",
	list_display = [id, title, completed, created_at],
	list_filter = [completed],
	search_fields = [title, description],
	ordering = [(created_at, desc)],
	list_per_page = 50
)]
pub struct TodoAdmin;
