use chrono::{DateTime, Utc};
use reinhardt_auth::Argon2Hasher;
use reinhardt_macros::user;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[user(hasher = Argon2Hasher, username_field = "username", full = true)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtendedUser {
	pub id: Uuid,
	pub username: String,
	pub email: String,
	pub first_name: String,
	pub last_name: String,
	pub password_hash: Option<String>,
	pub last_login: Option<DateTime<Utc>>,
	pub is_active: bool,
	pub is_staff: bool,
	pub is_superuser: bool,
	pub date_joined: DateTime<Utc>,
	pub user_permissions: Vec<String>,
	pub groups: Vec<String>,
	// Extra non-auth fields
	pub bio: String,
	pub avatar_url: Option<String>,
	pub login_count: u32,
}

fn main() {
	use reinhardt_auth::{AuthIdentity, BaseUser, FullUser, PermissionsMixin};

	let user = ExtendedUser {
		id: Uuid::now_v7(),
		username: "alice".to_string(),
		email: "alice@example.com".to_string(),
		first_name: "Alice".to_string(),
		last_name: "Smith".to_string(),
		password_hash: None,
		last_login: None,
		is_active: true,
		is_staff: false,
		is_superuser: false,
		date_joined: Utc::now(),
		user_permissions: Vec::new(),
		groups: Vec::new(),
		bio: "Hello!".to_string(),
		avatar_url: Some("https://example.com/avatar.png".to_string()),
		login_count: 42,
	};

	assert_eq!(user.username(), "alice");
	assert_eq!(user.bio, "Hello!");
	assert_eq!(user.login_count, 42);
	assert!(user.is_authenticated());
}
