use chrono::{DateTime, Utc};
use reinhardt_auth::Argon2Hasher;
use reinhardt_macros::user;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[user(hasher = Argon2Hasher, username_field = "email")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MinimalUser {
	pub id: Uuid,
	pub email: String,
	pub password_hash: Option<String>,
	pub last_login: Option<DateTime<Utc>>,
	pub is_active: bool,
	pub is_superuser: bool,
}

fn main() {
	use reinhardt_auth::{AuthIdentity, BaseUser};

	let mut user = MinimalUser {
		id: Uuid::now_v7(),
		email: "test@example.com".to_string(),
		password_hash: None,
		last_login: None,
		is_active: true,
		is_superuser: false,
	};

	assert_eq!(MinimalUser::get_username_field(), "email");
	assert_eq!(user.get_username(), "test@example.com");
	assert!(user.is_authenticated());
	assert!(!user.is_admin());

	user.set_password("test123").unwrap();
	assert!(user.check_password("test123").unwrap());
}
