//! User model for authentication
//!
//! Implements BaseUser trait with Argon2id password hashing.
//! Uses reinhardt ORM (Manager/QuerySet) for database operations.

use chrono::{DateTime, Utc};
use reinhardt::db::associations::ManyToManyField;
use reinhardt::prelude::*;
use reinhardt::{Argon2Hasher, BaseUser};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// Test-only dependency for sqlx::FromRow (server-side only)
#[cfg(all(test, server))]
use sqlx::FromRow;

#[model(app_label = "auth", table_name = "auth_user")]
#[derive(Serialize, Deserialize)]
#[cfg_attr(all(test, server), derive(FromRow))]
pub struct User {
	#[field(primary_key = true)]
	id: Uuid,

	#[field(max_length = 150, unique = true)]
	username: String,

	#[field(max_length = 255, unique = true)]
	email: String,

	#[field(max_length = 255)]
	password_hash: Option<String>,

	#[field(default = true)]
	is_active: bool,

	#[field(include_in_new = false)]
	last_login: Option<DateTime<Utc>>,

	#[field(auto_now_add = true)]
	created_at: DateTime<Utc>,

	#[field(max_length = 500, null = true)]
	bio: Option<String>,

	// ManyToMany relationships for following/blocking functionality
	// ManyToManyField<Source, Target> format - intermediate table auto-generated:
	// - auth_user_following (user_id -> user_id, following_id -> user_id)
	// - auth_user_blocked_users (user_id -> user_id, blocked_user_id -> user_id)
	#[serde(skip, default)]
	#[cfg_attr(all(test, server), sqlx(skip))]
	#[rel(many_to_many, related_name = "followers")]
	following: ManyToManyField<User, User>,

	#[serde(skip, default)]
	#[cfg_attr(all(test, server), sqlx(skip))]
	#[rel(many_to_many, related_name = "blocked_by")]
	blocked_users: ManyToManyField<User, User>,
}

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
