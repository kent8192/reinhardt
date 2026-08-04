#![deny(unexpected_cfgs)]

use reinhardt::{model, user};
use serde::{Deserialize, Serialize};

#[user(
	hasher = reinhardt::Argon2Hasher,
	username_field = "username",
	manager = false
)]
#[model(app_label = "users", table_name = "shared_users")]
#[derive(Default, Clone, Serialize, Deserialize)]
pub struct SharedUser {
	#[field(primary_key = true)]
	pub id: i64,

	#[field(max_length = 150, unique = true)]
	pub username: String,

	#[user_field(password_hash)]
	#[field(max_length = 255)]
	pub password: Option<String>,

	#[user_field(last_login)]
	#[field(max_length = 64, skip_info = true)]
	pub signed_in_at: Option<String>,

	#[field(default = true)]
	pub is_active: bool,

	#[field(default = false, skip_info = true)]
	pub is_superuser: bool,

	#[field(skip = true)]
	pub user_permissions: Vec<String>,

	#[field(skip = true)]
	pub groups: Vec<String>,
}

pub fn construct_shared_user() -> SharedUser {
	SharedUser::default()
}

pub fn construct_shared_user_info() -> SharedUserInfo {
	SharedUserInfo {
		id: 1,
		username: "alice".to_string(),
		is_active: true,
	}
}
