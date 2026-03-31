//! User model for authentication
//!
//! Implements BaseUser and FullUser traits with Argon2id password hashing.
//! Uses reinhardt ORM for database operations.

use chrono::{DateTime, Utc};
use reinhardt::prelude::*;
use reinhardt::{Argon2Hasher, BaseUser, FullUser};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// User model for authentication
///
/// Defines a custom user model using `#[model(...)]` macro with `BaseUser`
/// and `FullUser` trait implementations for admin panel integration.
#[model(app_label = "auth", table_name = "auth_user")]
#[derive(Serialize, Deserialize)]
#[allow(clippy::too_many_arguments)] // macro-generated new() has 8 args from model fields
pub struct User {
	#[field(primary_key = true)]
	pub id: Uuid,

	#[field(max_length = 150, unique = true)]
	pub username: String,

	#[field(max_length = 255)]
	pub email: String,

	#[field(max_length = 255)]
	pub password_hash: Option<String>,

	#[field(max_length = 150)]
	pub first_name: String,

	#[field(max_length = 150)]
	pub last_name: String,

	#[field(default = true)]
	pub is_active: bool,

	#[field(default = false)]
	pub is_staff: bool,

	#[field(default = false)]
	pub is_superuser: bool,

	#[field(include_in_new = false)]
	pub last_login: Option<DateTime<Utc>>,

	#[field(auto_now_add = true)]
	pub date_joined: DateTime<Utc>,
}

impl BaseUser for User {
	type PrimaryKey = Uuid;
	type Hasher = Argon2Hasher;

	fn get_username_field() -> &'static str {
		"username"
	}

	fn get_username(&self) -> &str {
		&self.username
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

impl FullUser for User {
	fn username(&self) -> &str {
		&self.username
	}

	fn email(&self) -> &str {
		&self.email
	}

	fn first_name(&self) -> &str {
		&self.first_name
	}

	fn last_name(&self) -> &str {
		&self.last_name
	}

	fn is_staff(&self) -> bool {
		self.is_staff
	}

	fn is_superuser(&self) -> bool {
		self.is_superuser
	}

	fn date_joined(&self) -> DateTime<Utc> {
		self.date_joined
	}
}
