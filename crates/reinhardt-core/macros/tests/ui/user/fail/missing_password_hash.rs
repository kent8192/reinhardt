#![allow(unused_imports)]

use chrono::{DateTime, Utc};
use reinhardt_auth::Argon2Hasher;
use reinhardt_macros::user;
use serde::{Deserialize, Serialize};

#[user(hasher = Argon2Hasher, username_field = "email")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BadUser {
	pub id: i64,
	pub email: String,
	// missing: password_hash
	pub last_login: Option<DateTime<Utc>>,
	pub is_active: bool,
	pub is_superuser: bool,
}

fn main() {}
