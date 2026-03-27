#![allow(unused_imports)]

use reinhardt_auth::Argon2Hasher;
use reinhardt_macros::user;
use serde::{Deserialize, Serialize};

#[user(hasher = Argon2Hasher, username_field = "email")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BadUser {
	pub id: i64,
	pub email: String,
	pub password_hash: Option<String>,
	pub is_active: bool,
	pub is_superuser: bool,

	#[user_field(nonexistent_role)]
	pub something: String,
}

fn main() {}
