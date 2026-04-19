use chrono::{DateTime, Utc};
use reinhardt_auth::Argon2Hasher;
use reinhardt_macros::user;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[user(hasher = Argon2Hasher, username_field = "email", full = true)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomNameUser {
	pub id: Uuid,
	pub email: String,
	pub first_name: String,
	pub last_name: String,
	pub is_active: bool,
	pub is_staff: bool,
	pub is_superuser: bool,

	#[user_field(password_hash)]
	pub pwd_hash: Option<String>,

	#[user_field(last_login)]
	pub last_signed_in: Option<DateTime<Utc>>,

	#[user_field(date_joined)]
	pub created_at: DateTime<Utc>,
}

fn main() {
	use reinhardt_auth::{BaseUser, FullUser};

	let user = CustomNameUser {
		id: Uuid::now_v7(),
		email: "bob@example.com".to_string(),
		first_name: "Bob".to_string(),
		last_name: "Jones".to_string(),
		pwd_hash: None,
		last_signed_in: None,
		is_active: true,
		is_staff: false,
		is_superuser: false,
		created_at: Utc::now(),
	};

	assert_eq!(user.get_username(), "bob@example.com");
	assert!(!user.password_hash().is_some());
	assert_eq!(user.get_full_name(), "Bob Jones");
}
