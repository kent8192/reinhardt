//! Admin default user type defined via `#[user]` macro.
//!
//! Provides the internal user type used by admin server functions for
//! authentication and permission checking. This replaces the deprecated
//! `DefaultUser` from `reinhardt-auth`.

use chrono::{DateTime, Utc};
use reinhardt_db::orm::Model;
use reinhardt_macros::user;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Internal user type for admin server function authentication.
///
/// This struct mirrors the fields of the deprecated `DefaultUser` but is
/// defined using the `#[user]` macro, which generates the required trait
/// implementations (`BaseUser`, `FullUser`, `PermissionsMixin`, etc.).
/// `Model` is implemented manually since admin operates on the same
/// `auth_user` table as the original `DefaultUser`.
#[user(hasher = reinhardt_auth::Argon2Hasher, username_field = "username", full = true)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdminDefaultUser {
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

/// Fields struct for `AdminDefaultUser` ORM operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdminDefaultUserFields {
	_private: (),
}

impl AdminDefaultUserFields {
	fn new() -> Self {
		Self { _private: () }
	}
}

impl reinhardt_db::orm::FieldSelector for AdminDefaultUserFields {
	fn with_alias(self, _alias: &str) -> Self {
		self
	}
}

impl Model for AdminDefaultUser {
	type PrimaryKey = Uuid;
	type Fields = AdminDefaultUserFields;

	fn table_name() -> &'static str {
		"auth_user"
	}

	fn new_fields() -> Self::Fields {
		AdminDefaultUserFields::new()
	}

	fn primary_key(&self) -> Option<Self::PrimaryKey> {
		Some(self.id)
	}

	fn set_primary_key(&mut self, value: Self::PrimaryKey) {
		self.id = value;
	}

	fn primary_key_field() -> &'static str {
		"id"
	}
}
