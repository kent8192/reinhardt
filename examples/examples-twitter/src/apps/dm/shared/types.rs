//! DM shared types
//!
//! Types shared between client and server for direct messaging.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[cfg(server)]
use reinhardt::rest::ToSchema;
#[cfg(server)]
use reinhardt::rest::openapi::Schema;

/// Information about a DM room
#[cfg_attr(server, derive(Schema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomInfo {
	/// Unique identifier for the room
	pub id: Uuid,
	/// Name of the room (or participant name for 1:1 chats)
	pub name: String,
	/// Whether this is a group chat
	pub is_group: bool,
	/// List of participant IDs in the room
	pub participants: Vec<Uuid>,
	/// Last message preview
	pub last_message: Option<String>,
	/// Timestamp of last activity
	pub last_activity: Option<String>,
	/// Count of unread messages for the current user
	pub unread_count: i32,
}

/// A direct message
#[cfg_attr(server, derive(Schema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageInfo {
	/// Unique identifier for the message
	pub id: Uuid,
	/// Room this message belongs to
	pub room_id: Uuid,
	/// ID of the sender
	pub sender_id: Uuid,
	/// Username of the sender
	pub sender_username: String,
	/// Message content
	pub content: String,
	/// When the message was sent
	pub created_at: String,
	/// Whether the current user has read this message
	pub is_read: bool,
}

/// Request to send a new message
#[cfg_attr(server, derive(Schema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendMessageRequest {
	/// Room to send the message to
	pub room_id: Uuid,
	/// Message content
	pub content: String,
}

/// Request to create a new DM room
#[cfg_attr(server, derive(Schema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateRoomRequest {
	/// User IDs to include in the room
	pub participant_ids: Vec<Uuid>,
	/// Optional room name (for group chats)
	pub name: Option<String>,
}

/// WebSocket notification for new messages (sent to room list subscribers)
#[cfg_attr(server, derive(Schema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewMessageNotification {
	/// Room that received the new message
	pub room_id: Uuid,
	/// Preview of the message content
	pub message_preview: String,
	/// Sender's username
	pub sender_username: String,
	/// When the message was sent
	pub created_at: String,
}
