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
	pub email: String,
	pub first_name: String,
	pub last_name: String,
	pub password_hash: Option<String>,
	pub last_login: Option<DateTime<Utc>>,
	pub is_active: bool,
	// missing: is_staff
	pub is_superuser: bool,
	pub date_joined: DateTime<Utc>,
}

fn main() {}
