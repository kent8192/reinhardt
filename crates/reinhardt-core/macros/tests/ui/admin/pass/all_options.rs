//! Admin macro with all options specified

use reinhardt_macros::admin;

struct Profile;

#[admin(model,
	for = Profile,
	name = "Profile",
	list_display = [id, user_id, bio, location, created_at],
	date_hierarchy = created_at,
	search_fields = [user_id, bio, location],
	list_filter = [created_at, updated_at],
	ordering = [(created_at, desc)],
	readonly_fields = [id, created_at, updated_at],
	fields = [user_id, bio, avatar_url, location, website],
	list_per_page = 50
)]
pub struct ProfileAdmin;

fn main() {
	// Compile test only - verify the macro expands with all options
}
