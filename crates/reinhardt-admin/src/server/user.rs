//! Admin default user type defined via `#[user]` macro.
//!
//! Provides the internal user type used by admin server functions for
//! authentication and permission checking. This replaces the deprecated
//! `DefaultUser` from `reinhardt-auth`.

use chrono::{DateTime, Utc};
use reinhardt_db::orm::{FilterValue, Model};
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
// `manager = false`: admin user CRUD is delegated to `reinhardt-admin`'s own
// query layer; the auto-generated in-memory manager would be unused and would
// require `Default` on `AdminDefaultUser`, which is not currently derivable.
#[user(hasher = reinhardt_auth::Argon2Hasher, username_field = "username", full = true, manager = false)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdminDefaultUser {
	/// Unique identifier.
	pub id: Uuid,
	/// Login username.
	pub username: String,
	/// Email address.
	pub email: String,
	/// First name.
	pub first_name: String,
	/// Last name.
	pub last_name: String,
	/// Hashed password (None if not set).
	pub password_hash: Option<String>,
	/// Timestamp of last login.
	pub last_login: Option<DateTime<Utc>>,
	/// Whether the user account is active.
	pub is_active: bool,
	/// Whether the user has staff privileges.
	pub is_staff: bool,
	/// Whether the user has superuser privileges.
	pub is_superuser: bool,
	/// Timestamp when the user joined.
	pub date_joined: DateTime<Utc>,
	/// List of permission codenames assigned to this user.
	#[serde(deserialize_with = "super::serde_helpers::string_or_vec", default)]
	pub user_permissions: Vec<String>,
	/// List of group names this user belongs to.
	#[serde(deserialize_with = "super::serde_helpers::string_or_vec", default)]
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
	type Objects = reinhardt_db::orm::Manager<Self>;

	fn table_name() -> &'static str {
		"auth_user"
	}

	fn new_fields() -> Self::Fields {
		AdminDefaultUserFields::new()
	}

	fn primary_key(&self) -> Option<Self::PrimaryKey> {
		Some(self.id)
	}

	fn primary_key_filter_value(pk: Self::PrimaryKey) -> FilterValue {
		FilterValue::Uuid(pk)
	}

	fn set_primary_key(&mut self, value: Self::PrimaryKey) {
		self.id = value;
	}

	fn primary_key_field() -> &'static str {
		"id"
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn primary_key_filter_value_preserves_uuid_binding() {
		// Arrange
		let primary_key =
			Uuid::parse_str("123e4567-e89b-12d3-a456-426614174000").expect("UUID should be valid");

		// Act
		let value = AdminDefaultUser::primary_key_filter_value(primary_key);

		// Assert
		match value {
			FilterValue::Uuid(value) => assert_eq!(value, primary_key),
			_ => panic!("admin user primary key should retain its UUID binding"),
		}
	}
}
