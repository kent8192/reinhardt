use chrono::{DateTime, Utc};
use reinhardt_auth::Argon2Hasher;
use reinhardt_macros::user;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[user(hasher = Argon2Hasher, username_field = "username", full = true)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MixedUser {
	pub id: Uuid,
	pub username: String,
	pub email: String,
	pub first_name: String,
	pub last_name: String,
	pub is_active: bool,
	pub is_staff: bool,
	pub is_superuser: bool,
	pub user_permissions: Vec<String>,
	pub groups: Vec<String>,

	// Convention fields mixed with overrides
	pub password_hash: Option<String>,

	// Overrides
	#[user_field(last_login)]
	pub signed_in_at: Option<DateTime<Utc>>,

	#[user_field(date_joined)]
	pub registered_at: DateTime<Utc>,
}

fn main() {
	use reinhardt_auth::{BaseUser, FullUser, PermissionsMixin};

	let mut user = MixedUser {
		id: Uuid::now_v7(),
		username: "mixed".to_string(),
		email: "mixed@example.com".to_string(),
		first_name: "Mix".to_string(),
		last_name: "Ed".to_string(),
		is_active: true,
		is_staff: true,
		is_superuser: false,
		user_permissions: vec!["app.view".to_string()],
		groups: Vec::new(),
		password_hash: None,
		signed_in_at: None,
		registered_at: Utc::now(),
	};

	// Convention-based
	assert_eq!(user.get_username(), "mixed");
	assert!(user.has_perm("app.view"));

	// Override-based: last_login maps to signed_in_at
	assert!(user.last_login().is_none());
	let now = Utc::now();
	user.set_last_login(now);
	assert_eq!(user.last_login(), Some(now));

	// Override-based: date_joined maps to registered_at
	assert!(FullUser::date_joined(&user) <= Utc::now());
}
