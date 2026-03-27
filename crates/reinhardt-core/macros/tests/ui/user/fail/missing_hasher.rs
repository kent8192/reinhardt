use reinhardt_macros::user;
use serde::{Deserialize, Serialize};

#[user(username_field = "email")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BadUser {
	pub id: i64,
	pub email: String,
	pub password_hash: Option<String>,
}

fn main() {}
