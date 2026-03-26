use chrono::{DateTime, Utc};
use reinhardt_auth::Argon2Hasher;
use reinhardt_macros::user;
use serde::{Deserialize, Serialize};

#[user(hasher = Argon2Hasher, username_field = "username")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimpleAuthUser {
	pub id: i64,
	pub username: String,
	pub password_hash: Option<String>,
	pub last_login: Option<DateTime<Utc>>,
	pub is_active: bool,
	pub is_superuser: bool,
}

fn main() {
	use reinhardt_auth::{AuthIdentity, BaseUser};

	let user = SimpleAuthUser {
		id: 42,
		username: "charlie".to_string(),
		password_hash: None,
		last_login: None,
		is_active: true,
		is_superuser: true,
	};

	assert_eq!(user.id(), "42");
	assert!(user.is_admin());
	assert_eq!(user.get_username(), "charlie");
}
