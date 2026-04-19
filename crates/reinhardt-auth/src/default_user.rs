#[cfg(feature = "argon2-hasher")]
use crate::Argon2Hasher;
#[cfg(feature = "argon2-hasher")]
use crate::{BaseUser, FullUser, PermissionsMixin, User};
#[cfg(feature = "argon2-hasher")]
use chrono::{DateTime, Utc};
#[cfg(feature = "argon2-hasher")]
use reinhardt_db::orm::Model;
#[cfg(feature = "argon2-hasher")]
use serde::{Deserialize, Serialize};
#[cfg(feature = "argon2-hasher")]
use uuid::Uuid;

/// DefaultUser struct - Django's AbstractUser equivalent
///
/// A complete, ready-to-use user model that combines BaseUser, FullUser, and PermissionsMixin.
/// This is the default user model provided by Reinhardt, suitable for most applications.
///
/// # Relationship with Django
///
/// This struct is equivalent to Django's `django.contrib.auth.models.AbstractUser`.
/// It provides a full-featured user model with:
/// - Username-based authentication
/// - Email address
/// - First and last name
/// - Password hashing (Argon2id by default)
/// - Active/staff/superuser flags
/// - Timestamps (last_login, date_joined)
/// - Permissions and groups
///
/// # Database Schema
///
/// The default table name is `auth_user` with the following columns:
/// - `id` (UUID, primary key)
/// - `username` (String, unique)
/// - `email` (String)
/// - `first_name` (String)
/// - `last_name` (String)
/// - `password_hash` (`Option<String>`)
/// - `last_login` (`Option<DateTime<Utc>>`)
/// - `is_active` (bool)
/// - `is_staff` (bool)
/// - `is_superuser` (bool)
/// - `date_joined` (`DateTime<Utc>`)
/// - `user_permissions` (`Vec<String>`)
/// - `groups` (`Vec<String>`)
///
/// # Examples
///
/// Creating a new user with automatic Argon2id password hashing:
///
/// ```
/// use reinhardt_auth::{BaseUser, DefaultUser};
/// use uuid::Uuid;
/// use chrono::Utc;
///
/// let mut user = DefaultUser {
///     id: Uuid::now_v7(),
///     username: "alice".to_string(),
///     email: "alice@example.com".to_string(),
///     first_name: "Alice".to_string(),
///     last_name: "Smith".to_string(),
///     password_hash: None,
///     last_login: None,
///     is_active: true,
///     is_staff: false,
///     is_superuser: false,
///     date_joined: Utc::now(),
///     user_permissions: Vec::new(),
///     groups: Vec::new(),
/// };
///
/// // Password is automatically hashed with Argon2id
/// user.set_password("securepass123").unwrap();
///
/// // Verify password
/// assert!(user.check_password("securepass123").unwrap());
/// assert!(!user.check_password("wrongpass").unwrap());
/// ```
///
/// Using with permissions:
///
/// ```
/// use reinhardt_auth::{DefaultUser, PermissionsMixin};
/// use uuid::Uuid;
/// use chrono::Utc;
///
/// let mut user = DefaultUser {
///     id: Uuid::now_v7(),
///     username: "bob".to_string(),
///     email: "bob@example.com".to_string(),
///     first_name: "Bob".to_string(),
///     last_name: "Johnson".to_string(),
///     password_hash: None,
///     last_login: None,
///     is_active: true,
///     is_staff: true,
///     is_superuser: false,
///     date_joined: Utc::now(),
///     user_permissions: vec![
///         "blog.add_post".to_string(),
///         "blog.change_post".to_string(),
///     ],
///     groups: vec!["editors".to_string()],
/// };
///
/// // Check permissions
/// assert!(user.has_perm("blog.add_post"));
/// assert!(user.has_perm("blog.change_post"));
/// assert!(!user.has_perm("blog.delete_post"));
/// assert!(user.has_module_perms("blog"));
/// ```
#[cfg(feature = "argon2-hasher")]
#[deprecated(
	since = "0.1.0-rc.15",
	note = "Use the `user` attribute macro to define your own user struct instead"
)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DefaultUser {
	/// Unique identifier (primary key)
	pub id: Uuid,

	/// Username (unique, used for login)
	pub username: String,

	/// Email address
	pub email: String,

	/// First name
	pub first_name: String,

	/// Last name
	pub last_name: String,

	/// Password hash (hashed with Argon2id by default)
	pub password_hash: Option<String>,

	/// Last login timestamp
	pub last_login: Option<DateTime<Utc>>,

	/// Whether this user account is active
	pub is_active: bool,

	/// Whether this user can access the admin site
	pub is_staff: bool,

	/// Whether this user has all permissions (superuser)
	pub is_superuser: bool,

	/// When this user account was created
	pub date_joined: DateTime<Utc>,

	/// List of permissions (format: "app_label.permission_name")
	pub user_permissions: Vec<String>,

	/// List of groups this user belongs to
	pub groups: Vec<String>,
}

#[cfg(feature = "argon2-hasher")]
impl BaseUser for DefaultUser {
	type PrimaryKey = Uuid;
	type Hasher = Argon2Hasher;

	fn get_username_field() -> &'static str {
		"username"
	}

	fn get_username(&self) -> &str {
		&self.username
	}

	fn password_hash(&self) -> Option<&str> {
		self.password_hash.as_deref()
	}

	fn set_password_hash(&mut self, hash: String) {
		self.password_hash = Some(hash);
	}

	fn last_login(&self) -> Option<DateTime<Utc>> {
		self.last_login
	}

	fn set_last_login(&mut self, time: DateTime<Utc>) {
		self.last_login = Some(time);
	}

	fn is_active(&self) -> bool {
		self.is_active
	}
}

#[cfg(feature = "argon2-hasher")]
impl FullUser for DefaultUser {
	fn username(&self) -> &str {
		&self.username
	}

	fn email(&self) -> &str {
		&self.email
	}

	fn first_name(&self) -> &str {
		&self.first_name
	}

	fn last_name(&self) -> &str {
		&self.last_name
	}

	fn is_staff(&self) -> bool {
		self.is_staff
	}

	fn is_superuser(&self) -> bool {
		self.is_superuser
	}

	fn date_joined(&self) -> DateTime<Utc> {
		self.date_joined
	}
}

#[cfg(feature = "argon2-hasher")]
impl PermissionsMixin for DefaultUser {
	fn is_superuser(&self) -> bool {
		self.is_superuser
	}

	fn user_permissions(&self) -> &[String] {
		&self.user_permissions
	}

	fn groups(&self) -> &[String] {
		&self.groups
	}
}

/// Query field descriptors for the `DefaultUser` model.
#[cfg(feature = "argon2-hasher")]
#[derive(Debug, Clone)]
pub struct DefaultUserFields {
	/// The user's unique identifier field.
	pub id: reinhardt_db::orm::query_fields::Field<DefaultUser, Uuid>,
	/// The username field.
	pub username: reinhardt_db::orm::query_fields::Field<DefaultUser, String>,
	/// The email address field.
	pub email: reinhardt_db::orm::query_fields::Field<DefaultUser, String>,
	/// The first name field.
	pub first_name: reinhardt_db::orm::query_fields::Field<DefaultUser, String>,
	/// The last name field.
	pub last_name: reinhardt_db::orm::query_fields::Field<DefaultUser, String>,
	/// The password hash field.
	pub password_hash: reinhardt_db::orm::query_fields::Field<DefaultUser, Option<String>>,
	/// The last login timestamp field.
	pub last_login: reinhardt_db::orm::query_fields::Field<DefaultUser, Option<DateTime<Utc>>>,
	/// The active status field.
	pub is_active: reinhardt_db::orm::query_fields::Field<DefaultUser, bool>,
	/// The staff status field.
	pub is_staff: reinhardt_db::orm::query_fields::Field<DefaultUser, bool>,
	/// The superuser status field.
	pub is_superuser: reinhardt_db::orm::query_fields::Field<DefaultUser, bool>,
	/// The date joined field.
	pub date_joined: reinhardt_db::orm::query_fields::Field<DefaultUser, DateTime<Utc>>,
	/// The user permissions field.
	pub user_permissions: reinhardt_db::orm::query_fields::Field<DefaultUser, Vec<String>>,
	/// The groups field.
	pub groups: reinhardt_db::orm::query_fields::Field<DefaultUser, Vec<String>>,
}

#[cfg(feature = "argon2-hasher")]
impl Default for DefaultUserFields {
	fn default() -> Self {
		Self::new()
	}
}

#[cfg(feature = "argon2-hasher")]
impl DefaultUserFields {
	/// Creates a new `DefaultUserFields` with default field mappings.
	pub fn new() -> Self {
		Self {
			id: reinhardt_db::orm::query_fields::Field::new(vec!["id"]),
			username: reinhardt_db::orm::query_fields::Field::new(vec!["username"]),
			email: reinhardt_db::orm::query_fields::Field::new(vec!["email"]),
			first_name: reinhardt_db::orm::query_fields::Field::new(vec!["first_name"]),
			last_name: reinhardt_db::orm::query_fields::Field::new(vec!["last_name"]),
			password_hash: reinhardt_db::orm::query_fields::Field::new(vec!["password_hash"]),
			last_login: reinhardt_db::orm::query_fields::Field::new(vec!["last_login"]),
			is_active: reinhardt_db::orm::query_fields::Field::new(vec!["is_active"]),
			is_staff: reinhardt_db::orm::query_fields::Field::new(vec!["is_staff"]),
			is_superuser: reinhardt_db::orm::query_fields::Field::new(vec!["is_superuser"]),
			date_joined: reinhardt_db::orm::query_fields::Field::new(vec!["date_joined"]),
			user_permissions: reinhardt_db::orm::query_fields::Field::new(vec!["user_permissions"]),
			groups: reinhardt_db::orm::query_fields::Field::new(vec!["groups"]),
		}
	}
}

#[cfg(feature = "argon2-hasher")]
impl reinhardt_db::orm::FieldSelector for DefaultUserFields {
	fn with_alias(mut self, alias: &str) -> Self {
		self.id = self.id.with_alias(alias);
		self.username = self.username.with_alias(alias);
		self.email = self.email.with_alias(alias);
		self.first_name = self.first_name.with_alias(alias);
		self.last_name = self.last_name.with_alias(alias);
		self.password_hash = self.password_hash.with_alias(alias);
		self.last_login = self.last_login.with_alias(alias);
		self.is_active = self.is_active.with_alias(alias);
		self.is_staff = self.is_staff.with_alias(alias);
		self.is_superuser = self.is_superuser.with_alias(alias);
		self.date_joined = self.date_joined.with_alias(alias);
		self.user_permissions = self.user_permissions.with_alias(alias);
		self.groups = self.groups.with_alias(alias);
		self
	}
}

#[cfg(feature = "argon2-hasher")]
impl Model for DefaultUser {
	type PrimaryKey = Uuid;
	type Fields = DefaultUserFields;

	fn table_name() -> &'static str {
		"auth_user"
	}

	fn new_fields() -> Self::Fields {
		DefaultUserFields::new()
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

#[cfg(feature = "argon2-hasher")]
impl Default for DefaultUser {
	fn default() -> Self {
		Self {
			id: Uuid::nil(),
			username: String::new(),
			email: String::new(),
			first_name: String::new(),
			last_name: String::new(),
			password_hash: None,
			last_login: None,
			is_active: true,
			is_staff: false,
			is_superuser: false,
			date_joined: Utc::now(),
			user_permissions: Vec::new(),
			groups: Vec::new(),
		}
	}
}

#[cfg(feature = "argon2-hasher")]
#[allow(deprecated)] // Implementing deprecated User trait for backward compatibility
impl User for DefaultUser {
	fn id(&self) -> String {
		self.id.to_string()
	}

	fn username(&self) -> &str {
		&self.username
	}

	fn get_username(&self) -> &str {
		&self.username
	}

	fn is_authenticated(&self) -> bool {
		true
	}

	fn is_active(&self) -> bool {
		self.is_active
	}

	fn is_admin(&self) -> bool {
		self.is_superuser
	}

	fn is_staff(&self) -> bool {
		self.is_staff
	}

	fn is_superuser(&self) -> bool {
		self.is_superuser
	}
}
