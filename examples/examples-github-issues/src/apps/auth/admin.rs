//! Admin configurations for auth app

use crate::apps::auth::models::User;
use reinhardt::admin;

/// Admin configuration for User model
///
/// Configures the admin panel display for User management:
/// - List view with key user fields
/// - Filtering by status and join date
/// - Search by name and email
/// - Sorted by join date (newest first)
#[admin(model,
	for = User,
	name = "User",
	list_display = [id, username, email, is_active, is_staff, date_joined],
	list_filter = [is_active, is_staff, is_superuser, date_joined],
	search_fields = [username, email, first_name, last_name],
	ordering = [(date_joined, desc)],
	readonly_fields = [id, date_joined, last_login],
	list_per_page = 25,
	permissions = allow_all,
)]
pub struct UserAdmin;
