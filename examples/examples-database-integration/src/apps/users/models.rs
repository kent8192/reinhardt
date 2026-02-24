//! Models for users app
//!
//! Database models for user management

use reinhardt::prelude::*;
use serde::{Deserialize, Serialize};

/// User model
///
/// Represents a single user in the system
#[derive(Serialize, Deserialize)]
#[model(app_label = "users", table_name = "users")]
pub struct User {
	/// Primary key (None for auto-increment on insert)
	#[field(primary_key = true)]
	pub id: Option<i64>,

	/// User's full name
	#[field(max_length = 255)]
	pub name: String,

	/// User's email address (unique)
	#[field(max_length = 255, unique = true)]
	pub email: String,

	/// Account creation timestamp
	#[field(auto_now_add = true)]
	pub created_at: chrono::DateTime<chrono::Utc>,
}
