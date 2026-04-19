//! DMMessage model for direct messaging

use chrono::{DateTime, Utc};
use reinhardt::db::associations::ForeignKeyField;
use reinhardt::model;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// Used by #[model] macro for type inference in ForeignKeyField<T> relationship fields.
// The macro requires these types to be in scope for generating the correct column types
// and relationship metadata, even though they appear unused to the compiler.
#[allow(unused_imports)]
use super::room::DMRoom;
#[allow(unused_imports)]
use crate::apps::auth::models::User;

// Test-only dependency for sqlx::FromRow (server-side only)
#[cfg(all(test, native))]
use sqlx::FromRow;

/// DMMessage model representing a message within a room
///
/// Each message belongs to a specific room and is sent by a user.
/// `ForeignKeyField<T>` automatically generates the `_id` column.
#[model(app_label = "dm", table_name = "dm_message")]
#[derive(Serialize, Deserialize)]
#[cfg_attr(all(test, native), derive(FromRow))]
pub struct DMMessage {
	#[field(primary_key = true)]
	pub id: Uuid,

	/// Room this message belongs to (generates room_id column)
	#[cfg_attr(all(test, native), sqlx(skip))]
	#[rel(foreign_key, related_name = "messages")]
	pub room: ForeignKeyField<DMRoom>,

	/// User who sent the message (generates sender_id column)
	#[cfg_attr(all(test, native), sqlx(skip))]
	#[rel(foreign_key, related_name = "sent_messages")]
	pub sender: ForeignKeyField<User>,

	#[field(max_length = 1000)]
	pub content: String,

	#[field(default = false, include_in_new = false)]
	pub is_read: bool,

	#[field(auto_now_add = true)]
	pub created_at: DateTime<Utc>,

	#[field(auto_now = true)]
	pub updated_at: DateTime<Utc>,
}

impl DMMessage {
	/// Set the is_read status of the message
	///
	/// This setter is manually defined because `include_in_new = false` fields
	/// do not have auto-generated setters, but updates are still required.
	pub fn set_is_read(&mut self, value: bool) {
		self.is_read = value;
	}
}
