//! Password reset token model for authentication recovery
//!
//! Stores password reset tokens with expiration and usage tracking.
//! Uses ForeignKey relationship to User model.

use chrono::{DateTime, Utc};
use reinhardt::db::associations::ForeignKeyField;
use reinhardt::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// Used by #[model] macro for type inference in ForeignKeyField<User> relationship field.
// The macro requires this type to be in scope for generating the correct foreign key column
// and relationship metadata, even though it appears unused to the compiler.
#[allow(unused_imports)]
use super::user::User;

/// Password reset token for user authentication recovery
///
/// Represents a password reset token with expiration.
/// `ForeignKeyField<User>` automatically generates the `user_id` column.
#[model(app_label = "auth", table_name = "auth_password_reset_token")]
#[derive(Serialize, Deserialize)]
pub struct PasswordResetToken {
	#[field(primary_key = true)]
	id: Uuid,

	/// User who requested password reset (generates user_id column)
	#[rel(foreign_key, related_name = "password_reset_tokens", on_delete = Cascade)]
	user: ForeignKeyField<User>,

	/// Reset token value (UUID v4 string)
	#[field(max_length = 255, unique = true)]
	token: String,

	/// Token expiration timestamp
	expires_at: DateTime<Utc>,

	/// Token creation timestamp
	#[field(auto_now_add = true)]
	created_at: DateTime<Utc>,

	/// Whether this token has been used
	#[field(default = false)]
	is_used: bool,
}
