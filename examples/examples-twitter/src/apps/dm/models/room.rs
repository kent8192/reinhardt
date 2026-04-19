//! DMRoom model for direct messaging

use chrono::{DateTime, Utc};
use reinhardt::db::associations::ManyToManyField;
use reinhardt::model;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// Used by #[model] macro for type inference in ManyToManyField<DMRoom, User> relationship field.
// The macro requires this type to be in scope for generating the correct intermediate table schema
// and relationship metadata, even though it appears unused to the compiler.
#[allow(unused_imports)]
use crate::apps::auth::models::User;

// Test-only dependency for sqlx::FromRow (server-side only)
#[cfg(all(test, native))]
use sqlx::FromRow;

/// DMRoom model representing a chat room for direct messaging
///
/// Supports both 1-on-1 and group conversations through ManyToMany relationship.
/// Room members are managed via the members ManyToManyField.
#[model(app_label = "dm", table_name = "dm_room")]
#[derive(Serialize, Deserialize)]
#[cfg_attr(all(test, native), derive(FromRow))]
pub struct DMRoom {
	#[field(primary_key = true)]
	pub id: Uuid,

	/// Room name (optional, used for group chats)
	#[field(max_length = 100)]
	pub name: Option<String>,

	/// Is this a group chat (more than 2 members)
	#[field(default = false)]
	pub is_group: bool,

	/// Room members via ManyToMany relationship
	/// Intermediate table: dm_room_members
	#[serde(skip, default)]
	#[cfg_attr(all(test, native), sqlx(skip))]
	#[rel(many_to_many, related_name = "rooms")]
	pub members: ManyToManyField<DMRoom, User>,

	#[field(auto_now_add = true)]
	pub created_at: DateTime<Utc>,

	#[field(auto_now = true)]
	pub updated_at: DateTime<Utc>,
}
