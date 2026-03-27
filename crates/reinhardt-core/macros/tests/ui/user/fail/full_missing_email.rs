#![allow(unused_imports)]

use chrono::{DateTime, Utc};
use reinhardt_auth::Argon2Hasher;
use reinhardt_macros::user;
use serde::{Deserialize, Serialize};

#[user(hasher = Argon2Hasher, username_field = "username", full = true)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BadUser {
	pub id: i64,
	pub username: String,
	pub password_hash: Option<String>,
	pub last_login: Option<DateTime<Utc>>,
	pub is_active: bool,
	pub is_superuser: bool,
	pub is_staff: bool,
	pub first_name: String,
	pub last_name: String,
	pub date_joined: DateTime<Utc>,
	// missing: email
}

fn main() {}
