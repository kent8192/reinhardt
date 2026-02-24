//! Admin configuration for profile app

use crate::apps::profile::models::Profile;
use reinhardt::admin;

/// Admin configuration for Profile model
///
/// This configures the admin panel display for Profile items:
/// - List view with id, user_id, bio, location, and created_at
/// - Filtering by created_at
/// - Search by bio and location
/// - Sorted by creation date (newest first)
#[admin(model,
	for = Profile,
	name = "Profile",
	list_display = [id, user_id, bio, location, created_at],
	list_filter = [created_at],
	search_fields = [bio, location],
	ordering = [(created_at, desc)]
)]
pub struct ProfileAdmin;
