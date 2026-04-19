use chrono::{DateTime, Utc};
use reinhardt_auth::Argon2Hasher;
use reinhardt_macros::user;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[user(hasher = Argon2Hasher, username_field = "username", full = true)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FullConventionUser {
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
}

fn main() {
	use reinhardt_auth::{AuthIdentity, BaseUser, FullUser, PermissionsMixin};

	let user = FullConventionUser {
		id: Uuid::now_v7(),
		username: "alice".to_string(),
		email: "alice@example.com".to_string(),
		first_name: "Alice".to_string(),
		last_name: "Smith".to_string(),
		password_hash: None,
		last_login: None,
		is_active: true,
		is_staff: true,
		is_superuser: false,
		date_joined: Utc::now(),
		user_permissions: vec!["blog.add_post".to_string()],
		groups: vec!["editors".to_string()],
	};

	// FullUser
	assert_eq!(user.username(), "alice");
	assert_eq!(user.email(), "alice@example.com");
	assert_eq!(user.get_full_name(), "Alice Smith");
	assert!(user.is_staff());

	// PermissionsMixin
	assert!(user.has_perm("blog.add_post"));
	assert!(!user.has_perm("blog.delete_post"));

	// AuthIdentity
	assert!(user.is_authenticated());
	assert!(!user.is_admin());
}
