//! User model for authentication
//!
//! Implements BaseUser trait with Argon2id password hashing.
//! Note: `#[user]` macro cannot be used here due to ManyToManyField<User, User>
//! self-referential relationships causing type inference issues.

use chrono::{DateTime, Utc};
use reinhardt::db::associations::ManyToManyField;
use reinhardt::prelude::*;
use reinhardt::{Argon2Hasher, BaseUser};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// Test-only dependency for sqlx::FromRow (server-side only)
#[cfg(all(test, native))]
use sqlx::FromRow;

#[model(app_label = "auth", table_name = "auth_user")]
#[derive(Serialize, Deserialize)]
#[cfg_attr(all(test, native), derive(FromRow))]
pub struct User {
	#[field(primary_key = true)]
	pub id: Uuid,

	#[field(max_length = 150, unique = true)]
	pub username: String,

	#[field(max_length = 255, unique = true)]
	pub email: String,

	#[field(max_length = 255)]
	pub password_hash: Option<String>,

	#[field(default = true)]
	pub is_active: bool,

	#[field(include_in_new = false)]
	pub last_login: Option<DateTime<Utc>>,

	#[field(auto_now_add = true)]
	pub created_at: DateTime<Utc>,

	#[field(max_length = 500, null = true)]
	pub bio: Option<String>,

	// ManyToMany relationships for following/blocking functionality
	#[serde(skip, default)]
	#[cfg_attr(all(test, native), sqlx(skip))]
	#[rel(many_to_many, related_name = "followers")]
	pub following: ManyToManyField<User, User>,

	#[serde(skip, default)]
	#[cfg_attr(all(test, native), sqlx(skip))]
	#[rel(many_to_many, related_name = "blocked_by")]
	pub blocked_users: ManyToManyField<User, User>,
}

// Workaround for kent8192/reinhardt-web#3651 (tracked in reinhardt-web#3651)
// Remove this workaround when the upstream issue is resolved.
//
// Ideal implementation (without workaround):
//   #[user(hasher = Argon2Hasher, username_field = "email")]
//   #[model(app_label = "auth", table_name = "auth_user")]
//   pub struct User { ... }
impl BaseUser for User {
	type PrimaryKey = Uuid;
	type Hasher = Argon2Hasher;

	fn get_username_field() -> &'static str {
		"email"
	}

	fn get_username(&self) -> &str {
		&self.email
	}

	fn password_hash(&self) -> Option<&str> {
		self.password_hash.as_deref()
	}

	fn set_password_hash(&mut self, hash: String) {
		self.password_hash = Some(hash);
	}

	fn last_login(&self) -> Option<DateTime<Utc>> {
		self.last_login
	}

	fn set_last_login(&mut self, time: DateTime<Utc>) {
		self.last_login = Some(time);
	}

	fn is_active(&self) -> bool {
		self.is_active
	}
}
