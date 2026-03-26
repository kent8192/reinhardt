use chrono::{DateTime, Utc};
use reinhardt_auth::Argon2Hasher;
use reinhardt_macros::user;
use serde::{Deserialize, Serialize};

#[user(hasher = Argon2Hasher, username_field = "email")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct I32PkUser {
	pub id: i32,
	pub email: String,
	pub password_hash: Option<String>,
	pub last_login: Option<DateTime<Utc>>,
	pub is_active: bool,
	pub is_superuser: bool,
}

fn main() {
	use reinhardt_auth::AuthIdentity;

	let user = I32PkUser {
		id: 999,
		email: "test@example.com".to_string(),
		password_hash: None,
		last_login: None,
		is_active: true,
		is_superuser: false,
	};

	assert_eq!(user.id(), "999");
}
