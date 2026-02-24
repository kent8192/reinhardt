//! Admin configurations for projects app

use crate::apps::projects::models::{Project, ProjectMember};
use reinhardt::admin;

/// Admin configuration for Project model
#[admin(model,
	for = Project,
	name = "Project",
	list_display = [id, name, visibility, owner_id, created_at],
	list_filter = [visibility, created_at],
	search_fields = [name, description],
	ordering = [(created_at, desc)],
	readonly_fields = [id, created_at],
	list_per_page = 25
)]
pub struct ProjectAdmin;

/// Admin configuration for ProjectMember model
#[admin(model,
	for = ProjectMember,
	name = "Project Member",
	list_display = [id, project_id, user_id, role, joined_at],
	list_filter = [role, joined_at],
	search_fields = [project_id, user_id],
	ordering = [(joined_at, desc)],
	readonly_fields = [id, joined_at],
	list_per_page = 50
)]
pub struct ProjectMemberAdmin;
