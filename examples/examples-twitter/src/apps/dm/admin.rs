//! Admin configuration for dm app

use crate::apps::dm::models::{DMMessage, DMRoom};
use reinhardt::admin;

/// Admin configuration for DMRoom model
///
/// This configures the admin panel display for DMRoom items:
/// - List view with id, name, is_group, and created_at
/// - Filtering by is_group
/// - Search by name
/// - Sorted by creation date (newest first)
#[admin(model,
	for = DMRoom,
	name = "DM Room",
	list_display = [id, name, is_group, created_at],
	list_filter = [is_group],
	search_fields = [name],
	ordering = [(created_at, desc)]
)]
pub struct DMRoomAdmin;

/// Admin configuration for DMMessage model
///
/// This configures the admin panel display for DMMessage items:
/// - List view with id, room_id, sender_id, content, is_read, and created_at
/// - Filtering by is_read and created_at
/// - Search by content
/// - Sorted by creation date (newest first)
#[admin(model,
	for = DMMessage,
	name = "DM Message",
	list_display = [id, room_id, sender_id, content, is_read, created_at],
	list_filter = [is_read, created_at],
	search_fields = [content],
	ordering = [(created_at, desc)]
)]
pub struct DMMessageAdmin;
