//! Regression test for issue #3651: `#[user]` macro with self-referential
//! `ManyToManyField<User, User>` field.
//!
//! Previously, `reinhardt::prelude::*` and `reinhardt::User` exported the
//! deprecated `User` trait from `reinhardt_auth`, which name-conflicted with
//! any local `struct User`. This caused E0255/E0574/E0782 compiler errors
//! when combining `#[user]` + `#[model]` + `ManyToManyField<User, User>`.
//!
//! The fix removes the deprecated `User` trait from the facade re-exports
//! (both top-level and prelude). The trait remains accessible via
//! `reinhardt_auth::User` for code that explicitly needs it.

use chrono::{DateTime, Utc};
use reinhardt::db::associations::ManyToManyField;
use reinhardt::macros::user;
use reinhardt::prelude::*;
use reinhardt::{Argon2Hasher, BaseUser};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[user(hasher = Argon2Hasher, username_field = "email")]
#[model(app_label = "auth", table_name = "auth_user")]
#[derive(Serialize, Deserialize, Default, Clone)]
pub struct User {
	#[field(primary_key = true)]
	pub id: Uuid,

	#[field(max_length = 150, unique = true)]
	pub username: String,

	#[field(max_length = 255, unique = true)]
	pub email: String,

	#[field(max_length = 255)]
	pub password_hash: Option<String>,

	#[field(default = true)]
	pub is_active: bool,

	pub is_superuser: bool,

	#[field(include_in_new = false)]
	pub last_login: Option<DateTime<Utc>>,

	#[field(auto_now_add = true)]
	pub created_at: DateTime<Utc>,

	#[field(max_length = 500, null = true)]
	pub bio: Option<String>,

	// Self-referential ManyToMany relationships
	#[serde(skip, default)]
	#[rel(many_to_many, related_name = "followers")]
	pub following: ManyToManyField<User, User>,

	#[serde(skip, default)]
	#[rel(many_to_many, related_name = "blocked_by")]
	pub blocked_users: ManyToManyField<User, User>,
}

#[test]
fn test_user_macro_with_self_ref_m2m_compiles() {
	// Act -- struct definition with #[user] macro is the test
	let user = User::default();

	// Assert -- basic field access works
	assert_eq!(user.email, "");
	// NOTE: `#[field(default = true)]` is a DB-level default, not a Rust `Default` trait value.
	// `#[derive(Default)]` produces `bool::default()` = `false`.
	assert!(!user.is_active);
}
