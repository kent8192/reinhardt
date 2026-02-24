//! Admin configurations for issues app

use crate::apps::issues::models::Issue;
use reinhardt::admin;

/// Admin configuration for Issue model
#[admin(model,
	for = Issue,
	name = "Issue",
	list_display = [id, number, title, state, project_id, author_id, created_at],
	list_filter = [state, created_at],
	search_fields = [title, body],
	ordering = [(created_at, desc)],
	readonly_fields = [id, number, created_at, updated_at],
	list_per_page = 50
)]
pub struct IssueAdmin;
