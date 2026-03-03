//! DM server functions module
//!
//! This module contains server functions for direct messaging operations
//! and WebSocket handlers for real-time message delivery.

pub mod handlers;

pub use handlers::DMHandler;

use crate::apps::dm::shared::types::{MessageInfo, RoomInfo};
use reinhardt::pages::server_fn::{ServerFnError, server_fn};
use uuid::Uuid;

// Server-only imports
#[cfg(server)]
use {
	crate::apps::auth::models::User,
	crate::apps::dm::models::{DMMessage, DMRoom},
	reinhardt::DatabaseConnection,
	reinhardt::db::orm::{Filter, FilterOperator, FilterValue, ManyToManyAccessor, Model},
	reinhardt::middleware::session::SessionData,
};

/// Helper to get current user from session
#[cfg(server)]
async fn get_current_user(session: &SessionData) -> Result<User, ServerFnError> {
	let user_id = session
		.get::<Uuid>("user_id")
		.ok_or_else(|| ServerFnError::server(401, "Not authenticated"))?;

	User::objects()
		.filter(
			User::field_id(),
			FilterOperator::Eq,
			FilterValue::String(user_id.to_string()),
		)
		.first()
		.await
		.map_err(|e| ServerFnError::server(500, format!("Database error: {}", e)))?
		.ok_or_else(|| ServerFnError::server(404, "User not found"))
}

/// Helper to check if user is a member of a room
#[cfg(server)]
async fn is_room_member(
	room: &DMRoom,
	user: &User,
	db: DatabaseConnection,
) -> Result<bool, ServerFnError> {
	let accessor = ManyToManyAccessor::<DMRoom, User>::new(room, "members", db);
	let members = accessor
		.all()
		.await
		.map_err(|e| ServerFnError::server(500, format!("Database error: {}", e)))?;

	Ok(members.iter().any(|m| m.id() == user.id()))
}

/// Create a new DM room
#[server_fn(use_inject = true)]
pub async fn create_room(
	participant_ids: Vec<Uuid>,
	name: Option<String>,
	#[inject] db: DatabaseConnection,
	#[inject] session: SessionData,
) -> std::result::Result<RoomInfo, ServerFnError> {
	let current_user = get_current_user(&session).await?;

	// Validate participants
	if participant_ids.is_empty() {
		return Err(ServerFnError::application(
			"At least one participant is required".to_string(),
		));
	}

	// Determine if group chat (more than 2 total members including current user)
	let is_group = participant_ids.len() > 1;

	// Verify all participants exist
	let mut participants = vec![current_user.clone()];
	for pid in &participant_ids {
		// Skip if participant is the current user
		if *pid == current_user.id() {
			continue;
		}

		let user = User::objects()
			.filter(
				User::field_id(),
				FilterOperator::Eq,
				FilterValue::String(pid.to_string()),
			)
			.first()
			.await
			.map_err(|e| ServerFnError::server(500, format!("Database error: {}", e)))?
			.ok_or_else(|| ServerFnError::application(format!("User not found: {}", pid)))?;
		participants.push(user);
	}

	// For 1:1 chats, check if room already exists with same members
	if !is_group && participants.len() == 2 {
		// Get rooms for current user
		let user_rooms_accessor =
			ManyToManyAccessor::<User, DMRoom>::new(&current_user, "rooms", db.clone());
		let current_user_rooms = user_rooms_accessor
			.all()
			.await
			.map_err(|e| ServerFnError::server(500, format!("Database error: {}", e)))?;

		// Check each room
		for room in current_user_rooms {
			if room.is_group() {
				continue;
			}

			// Get members of this room
			let room_members_accessor =
				ManyToManyAccessor::<DMRoom, User>::new(&room, "members", db.clone());
			let members = room_members_accessor
				.all()
				.await
				.map_err(|e| ServerFnError::server(500, format!("Database error: {}", e)))?;

			// Check if this is a 1:1 room with the same participants
			if members.len() == 2 {
				let member_ids: Vec<Uuid> = members.iter().map(|m| m.id()).collect();
				let participant_ids_set: Vec<Uuid> = participants.iter().map(|p| p.id()).collect();

				if member_ids.iter().all(|id| participant_ids_set.contains(id)) {
					// Room already exists, return it
					return build_room_info(&room, &members, &current_user, db).await;
				}
			}
		}
	}

	// Create new room
	let room_name = name.unwrap_or_else(|| {
		if is_group {
			"Group Chat".to_string()
		} else {
			// Use the other participant's username for 1:1 chats
			participants
				.iter()
				.find(|p| p.id() != current_user.id())
				.map(|p| p.username().to_string())
				.unwrap_or_else(|| "Chat".to_string())
		}
	});

	let room = DMRoom::new(Some(room_name), is_group);

	// Save room to database
	DMRoom::objects()
		.create_with_conn(&db, &room)
		.await
		.map_err(|e| ServerFnError::server(500, format!("Database error: {}", e)))?;

	// Reload room to get the saved version
	let saved_room = DMRoom::objects()
		.filter(
			DMRoom::field_id(),
			FilterOperator::Eq,
			FilterValue::String(room.id().to_string()),
		)
		.first()
		.await
		.map_err(|e| ServerFnError::server(500, format!("Database error: {}", e)))?
		.ok_or_else(|| ServerFnError::server(500, "Failed to create room"))?;

	// Add all participants as members
	let members_accessor =
		ManyToManyAccessor::<DMRoom, User>::new(&saved_room, "members", db.clone());
	for participant in &participants {
		members_accessor
			.add(participant)
			.await
			.map_err(|e| ServerFnError::server(500, format!("Failed to add member: {}", e)))?;
	}

	build_room_info(&saved_room, &participants, &current_user, db).await
}

/// List all DM rooms for the current user
#[server_fn(use_inject = true)]
pub async fn list_rooms(
	#[inject] db: DatabaseConnection,
	#[inject] session: SessionData,
) -> std::result::Result<Vec<RoomInfo>, ServerFnError> {
	let current_user = get_current_user(&session).await?;

	// Get rooms the user is a member of
	let rooms_accessor =
		ManyToManyAccessor::<User, DMRoom>::new(&current_user, "rooms", db.clone());
	let rooms = rooms_accessor
		.all()
		.await
		.map_err(|e| ServerFnError::server(500, format!("Database error: {}", e)))?;

	let mut room_infos = Vec::new();
	for room in rooms {
		// Get members for each room
		let members_accessor =
			ManyToManyAccessor::<DMRoom, User>::new(&room, "members", db.clone());
		let members = members_accessor
			.all()
			.await
			.map_err(|e| ServerFnError::server(500, format!("Database error: {}", e)))?;

		room_infos.push(build_room_info(&room, &members, &current_user, db.clone()).await?);
	}

	// Sort by last_activity (most recent first)
	room_infos.sort_by(|a, b| b.last_activity.cmp(&a.last_activity));

	Ok(room_infos)
}

/// Get details of a specific DM room
#[server_fn(use_inject = true)]
pub async fn get_room(
	room_id: Uuid,
	#[inject] db: DatabaseConnection,
	#[inject] session: SessionData,
) -> std::result::Result<RoomInfo, ServerFnError> {
	let current_user = get_current_user(&session).await?;

	// Find the room
	let room = DMRoom::objects()
		.filter(
			DMRoom::field_id(),
			FilterOperator::Eq,
			FilterValue::String(room_id.to_string()),
		)
		.first()
		.await
		.map_err(|e| ServerFnError::server(500, format!("Database error: {}", e)))?
		.ok_or_else(|| ServerFnError::server(404, "Room not found"))?;

	// Check if user is a member
	if !is_room_member(&room, &current_user, db.clone()).await? {
		return Err(ServerFnError::server(403, "Access denied"));
	}

	// Get members
	let members_accessor = ManyToManyAccessor::<DMRoom, User>::new(&room, "members", db.clone());
	let members = members_accessor
		.all()
		.await
		.map_err(|e| ServerFnError::server(500, format!("Database error: {}", e)))?;

	build_room_info(&room, &members, &current_user, db).await
}

/// Send a message to a DM room
#[server_fn(use_inject = true)]
pub async fn send_message(
	room_id: Uuid,
	content: String,
	#[inject] db: DatabaseConnection,
	#[inject] session: SessionData,
) -> std::result::Result<MessageInfo, ServerFnError> {
	let current_user = get_current_user(&session).await?;

	// Validate content
	if content.trim().is_empty() {
		return Err(ServerFnError::application(
			"Message content cannot be empty".to_string(),
		));
	}

	if content.len() > 1000 {
		return Err(ServerFnError::application(
			"Message content exceeds 1000 characters".to_string(),
		));
	}

	// Find the room
	let room = DMRoom::objects()
		.filter(
			DMRoom::field_id(),
			FilterOperator::Eq,
			FilterValue::String(room_id.to_string()),
		)
		.first()
		.await
		.map_err(|e| ServerFnError::server(500, format!("Database error: {}", e)))?
		.ok_or_else(|| ServerFnError::server(404, "Room not found"))?;

	// Check if user is a member
	if !is_room_member(&room, &current_user, db.clone()).await? {
		return Err(ServerFnError::server(403, "Access denied"));
	}

	// Create message
	// Note: DMMessage::new arguments order is (content, room_id, sender_id)
	// ForeignKeyField parameters come after non-FK fields
	let message = DMMessage::new(content.trim().to_string(), room_id, current_user.id());

	// Save message
	DMMessage::objects()
		.create_with_conn(&db, &message)
		.await
		.map_err(|e| ServerFnError::server(500, format!("Database error: {}", e)))?;

	// Reload to get the saved version with timestamps
	let saved_message = DMMessage::objects()
		.filter(
			DMMessage::field_id(),
			FilterOperator::Eq,
			FilterValue::String(message.id().to_string()),
		)
		.first()
		.await
		.map_err(|e| ServerFnError::server(500, format!("Database error: {}", e)))?
		.ok_or_else(|| ServerFnError::server(500, "Failed to save message"))?;

	Ok(build_message_info(&saved_message, &current_user))
}

/// List messages in a DM room with pagination
#[server_fn(use_inject = true)]
pub async fn list_messages(
	room_id: Uuid,
	limit: Option<i32>,
	before: Option<Uuid>,
	#[inject] db: DatabaseConnection,
	#[inject] session: SessionData,
) -> std::result::Result<Vec<MessageInfo>, ServerFnError> {
	let current_user = get_current_user(&session).await?;

	// Find the room
	let room = DMRoom::objects()
		.filter(
			DMRoom::field_id(),
			FilterOperator::Eq,
			FilterValue::String(room_id.to_string()),
		)
		.first()
		.await
		.map_err(|e| ServerFnError::server(500, format!("Database error: {}", e)))?
		.ok_or_else(|| ServerFnError::server(404, "Room not found"))?;

	// Check if user is a member
	if !is_room_member(&room, &current_user, db.clone()).await? {
		return Err(ServerFnError::server(403, "Access denied"));
	}

	// Build query for messages
	let mut query = DMMessage::objects().filter(
		DMMessage::field_room(),
		FilterOperator::Eq,
		FilterValue::String(room_id.to_string()),
	);

	// Apply before cursor if provided
	if let Some(before_id) = before {
		let before_msg = DMMessage::objects()
			.filter(
				DMMessage::field_id(),
				FilterOperator::Eq,
				FilterValue::String(before_id.to_string()),
			)
			.first()
			.await
			.map_err(|e| ServerFnError::server(500, format!("Database error: {}", e)))?;

		if let Some(msg) = before_msg {
			query = query.filter(Filter::new(
				"created_at",
				FilterOperator::Lt,
				FilterValue::String(msg.created_at().to_rfc3339()),
			));
		}
	}

	// Get messages with limit
	let actual_limit = limit.unwrap_or(50).min(100) as usize;
	let messages = query
		.order_by(&["-created_at"])
		.limit(actual_limit)
		.all()
		.await
		.map_err(|e| ServerFnError::server(500, format!("Database error: {}", e)))?;

	// Build message infos with sender info
	let mut message_infos = Vec::new();
	for msg in messages {
		// Get sender info
		let sender = User::objects()
			.filter(
				User::field_id(),
				FilterOperator::Eq,
				FilterValue::String(msg.sender_id().to_string()),
			)
			.first()
			.await
			.map_err(|e| ServerFnError::server(500, format!("Database error: {}", e)))?;

		if let Some(sender_user) = sender {
			message_infos.push(build_message_info(&msg, &sender_user));
		}
	}

	// Reverse to get chronological order (oldest first)
	message_infos.reverse();

	Ok(message_infos)
}

/// Mark all messages in a room as read for the current user
#[server_fn(use_inject = true)]
pub async fn mark_as_read(
	room_id: Uuid,
	#[inject] db: DatabaseConnection,
	#[inject] session: SessionData,
) -> std::result::Result<(), ServerFnError> {
	let current_user = get_current_user(&session).await?;

	// Find the room
	let room = DMRoom::objects()
		.filter(
			DMRoom::field_id(),
			FilterOperator::Eq,
			FilterValue::String(room_id.to_string()),
		)
		.first()
		.await
		.map_err(|e| ServerFnError::server(500, format!("Database error: {}", e)))?
		.ok_or_else(|| ServerFnError::server(404, "Room not found"))?;

	// Check if user is a member
	if !is_room_member(&room, &current_user, db.clone()).await? {
		return Err(ServerFnError::server(403, "Access denied"));
	}

	// Get all unread messages in this room that weren't sent by the current user
	let unread_messages = DMMessage::objects()
		.filter(
			DMMessage::field_room(),
			FilterOperator::Eq,
			FilterValue::String(room_id.to_string()),
		)
		.filter(Filter::new(
			"is_read",
			FilterOperator::Eq,
			FilterValue::Bool(false),
		))
		.filter(Filter::new(
			"sender_id",
			FilterOperator::Ne,
			FilterValue::String(current_user.id().to_string()),
		))
		.all()
		.await
		.map_err(|e| ServerFnError::server(500, format!("Database error: {}", e)))?;

	// Update each message to mark as read
	for mut msg in unread_messages {
		msg.set_is_read(true);
		DMMessage::objects()
			.update_with_conn(&db, &msg)
			.await
			.map_err(|e| ServerFnError::server(500, format!("Database error: {}", e)))?;
	}

	Ok(())
}

/// Helper to build RoomInfo from DMRoom and members
#[cfg(server)]
async fn build_room_info(
	room: &DMRoom,
	members: &[User],
	current_user: &User,
	_db: DatabaseConnection,
) -> Result<RoomInfo, ServerFnError> {
	// Get last message
	let last_message = DMMessage::objects()
		.filter(
			DMMessage::field_room(),
			FilterOperator::Eq,
			FilterValue::String(room.id().to_string()),
		)
		.order_by(&["-created_at"])
		.first()
		.await
		.map_err(|e| ServerFnError::server(500, format!("Database error: {}", e)))?;

	// Count unread messages (messages not sent by current user and not read)
	let unread_messages = DMMessage::objects()
		.filter(
			DMMessage::field_room(),
			FilterOperator::Eq,
			FilterValue::String(room.id().to_string()),
		)
		.filter(Filter::new(
			"is_read",
			FilterOperator::Eq,
			FilterValue::Bool(false),
		))
		.filter(Filter::new(
			"sender_id",
			FilterOperator::Ne,
			FilterValue::String(current_user.id().to_string()),
		))
		.all()
		.await
		.map_err(|e| ServerFnError::server(500, format!("Database error: {}", e)))?;

	// Build display name (for 1:1 chats, use the other person's name)
	let display_name = if room.is_group() {
		room.name()
			.clone()
			.unwrap_or_else(|| "Group Chat".to_string())
	} else {
		members
			.iter()
			.find(|m| m.id() != current_user.id())
			.map(|m| m.username().to_string())
			.unwrap_or_else(|| room.name().clone().unwrap_or_else(|| "Chat".to_string()))
	};

	Ok(RoomInfo {
		id: room.id(),
		name: display_name,
		is_group: room.is_group(),
		participants: members.iter().map(|m| m.id()).collect(),
		last_message: last_message
			.as_ref()
			.map(|m| truncate_message(m.content(), 50)),
		last_activity: last_message.map(|m| m.created_at().to_rfc3339()),
		unread_count: unread_messages.len() as i32,
	})
}

/// Helper to build MessageInfo from DMMessage and sender
#[cfg(server)]
fn build_message_info(message: &DMMessage, sender: &User) -> MessageInfo {
	MessageInfo {
		id: message.id(),
		room_id: *message.room_id(),
		sender_id: *message.sender_id(),
		sender_username: sender.username().to_string(),
		content: message.content().to_string(),
		created_at: message.created_at().to_rfc3339(),
		is_read: message.is_read(),
	}
}

/// Helper to truncate message for preview
#[cfg(server)]
fn truncate_message(content: &str, max_len: usize) -> String {
	if content.len() <= max_len {
		content.to_string()
	} else {
		// Find a valid UTF-8 char boundary at or before max_len
		let end = content
			.char_indices()
			.map(|(i, _)| i)
			.take_while(|&i| i <= max_len)
			.last()
			.unwrap_or(0);
		format!("{}...", &content[..end])
	}
}
