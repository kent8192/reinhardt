//! Profile model for user profiles
//!
//! One-to-one relationship with User model.
//! Uses reinhardt ORM (Manager/QuerySet) for database operations.

use chrono::{DateTime, Utc};
use reinhardt::model;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// Test-only dependency for sqlx::FromRow (server-side only)
#[cfg(all(test, server))]
use sqlx::FromRow;

/// Profile model representing a user's profile information
///
/// One-to-one relationship with User model via user_id foreign key.
#[model(app_label = "profile", table_name = "profile_profile")]
#[derive(Clone, Serialize, Deserialize)]
#[cfg_attr(all(test, server), derive(FromRow))]
pub struct Profile {
	#[field(primary_key = true)]
	id: Uuid,

	/// Foreign key to User (one-to-one relationship)
	#[field(unique = true)]
	user_id: Uuid,

	#[field(max_length = 500)]
	bio: String,

	#[field(max_length = 255)]
	avatar_url: String,

	#[field(max_length = 255, null = true)]
	location: Option<String>,

	#[field(max_length = 255, null = true)]
	website: Option<String>,

	#[field(auto_now_add = true)]
	created_at: DateTime<Utc>,

	#[field(auto_now = true)]
	updated_at: DateTime<Utc>,
}
