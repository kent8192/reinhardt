use chrono::{DateTime, Utc};
use reinhardt_auth::Argon2Hasher;
use reinhardt_macros::user;
use serde::{Deserialize, Serialize};

#[user(hasher = Argon2Hasher, username_field = "username")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupsOnlyUser {
	pub id: i64,
	pub username: String,
	pub password_hash: Option<String>,
	pub last_login: Option<DateTime<Utc>>,
	pub is_active: bool,
	pub is_superuser: bool,
	pub groups: Vec<String>,
	// No user_permissions field -> PermissionsMixin should NOT be generated
}

fn main() {
	use reinhardt_auth::{AuthIdentity, BaseUser};

	let user = GroupsOnlyUser {
		id: 1,
		username: "bob".to_string(),
		password_hash: None,
		last_login: None,
		is_active: true,
		is_superuser: false,
		groups: vec!["admin".to_string()],
	};

	assert_eq!(user.get_username(), "bob");
	assert!(user.is_authenticated());
	// groups field is accessible but PermissionsMixin is not generated
	assert_eq!(user.groups.len(), 1);
}
