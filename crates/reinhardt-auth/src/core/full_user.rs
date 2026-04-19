use chrono::{DateTime, Utc};

use crate::core::base_user::BaseUser;

/// FullUser trait - Django's AbstractUser equivalent
///
/// This trait extends `BaseUser` with additional user information fields
/// commonly needed in web applications. It is inspired by Django's AbstractUser.
///
/// # Examples
///
/// ```no_run
/// use reinhardt_auth::{BaseUser, FullUser, PasswordHasher};
/// #[cfg(feature = "argon2-hasher")]
/// use reinhardt_auth::Argon2Hasher;
/// use uuid::Uuid;
/// use chrono::{DateTime, Utc};
/// use serde::{Serialize, Deserialize};
///
/// #[derive(Serialize, Deserialize)]
/// struct MyUser {
///     id: Uuid,
///     username: String,
///     email: String,
///     first_name: String,
///     last_name: String,
///     password_hash: Option<String>,
///     last_login: Option<DateTime<Utc>>,
///     is_active: bool,
///     is_staff: bool,
///     is_superuser: bool,
///     date_joined: DateTime<Utc>,
/// }
///
/// #[cfg(feature = "argon2-hasher")]
/// impl BaseUser for MyUser {
///     type PrimaryKey = Uuid;
///     type Hasher = Argon2Hasher;
///
///     fn get_username_field() -> &'static str { "username" }
///     fn get_username(&self) -> &str { &self.username }
///     fn password_hash(&self) -> Option<&str> { self.password_hash.as_deref() }
///     fn set_password_hash(&mut self, hash: String) { self.password_hash = Some(hash); }
///     fn last_login(&self) -> Option<DateTime<Utc>> { self.last_login }
///     fn set_last_login(&mut self, time: DateTime<Utc>) { self.last_login = Some(time); }
///     fn is_active(&self) -> bool { self.is_active }
/// }
///
/// impl FullUser for MyUser {
///     fn username(&self) -> &str { &self.username }
///     fn email(&self) -> &str { &self.email }
///     fn first_name(&self) -> &str { &self.first_name }
///     fn last_name(&self) -> &str { &self.last_name }
///     fn is_staff(&self) -> bool { self.is_staff }
///     fn is_superuser(&self) -> bool { self.is_superuser }
///     fn date_joined(&self) -> DateTime<Utc> { self.date_joined }
/// }
///
/// # #[cfg(feature = "argon2-hasher")]
/// # {
/// let user = MyUser {
///     id: Uuid::now_v7(),
///     username: "alice".to_string(),
///     email: "alice@example.com".to_string(),
///     first_name: "Alice".to_string(),
///     last_name: "Smith".to_string(),
///     password_hash: None,
///     last_login: None,
///     is_active: true,
///     is_staff: true,
///     is_superuser: false,
///     date_joined: Utc::now(),
/// };
///
/// assert_eq!(user.get_full_name(), "Alice Smith");
/// assert_eq!(user.get_short_name(), "Alice");
/// assert!(user.is_staff());
/// # }
/// ```
pub trait FullUser: BaseUser {
	/// Returns the username
	fn username(&self) -> &str;

	/// Returns the email address
	fn email(&self) -> &str;

	/// Returns the first name
	fn first_name(&self) -> &str;

	/// Returns the last name
	fn last_name(&self) -> &str;

	/// Returns whether the user is a staff member
	///
	/// Staff members typically have access to the admin interface.
	fn is_staff(&self) -> bool;

	/// Returns whether the user is a superuser
	///
	/// Superusers have all permissions without explicit assignment.
	fn is_superuser(&self) -> bool;

	/// Returns when the user account was created
	fn date_joined(&self) -> DateTime<Utc>;

	/// Returns the full name (first name + last name)
	///
	/// # Examples
	///
	/// ```no_run
	/// # use reinhardt_auth::{BaseUser, FullUser, PasswordHasher};
	/// # #[cfg(feature = "argon2-hasher")]
	/// # use reinhardt_auth::Argon2Hasher;
	/// # use uuid::Uuid;
	/// # use chrono::{DateTime, Utc};
	/// # use serde::{Serialize, Deserialize};
	/// # #[derive(Serialize, Deserialize)]
	/// # struct MyUser { id: Uuid, username: String, email: String,
	/// #   first_name: String, last_name: String, password_hash: Option<String>,
	/// #   last_login: Option<DateTime<Utc>>, is_active: bool, is_staff: bool,
	/// #   is_superuser: bool, date_joined: DateTime<Utc> }
	/// # #[cfg(feature = "argon2-hasher")]
	/// # impl BaseUser for MyUser {
	/// #     type PrimaryKey = Uuid;
	/// #     type Hasher = Argon2Hasher;
	/// #     fn get_username_field() -> &'static str { "username" }
	/// #     fn get_username(&self) -> &str { &self.username }
	/// #     fn password_hash(&self) -> Option<&str> { self.password_hash.as_deref() }
	/// #     fn set_password_hash(&mut self, hash: String) { self.password_hash = Some(hash); }
	/// #     fn last_login(&self) -> Option<DateTime<Utc>> { self.last_login }
	/// #     fn set_last_login(&mut self, time: DateTime<Utc>) { self.last_login = Some(time); }
	/// #     fn is_active(&self) -> bool { self.is_active }
	/// # }
	/// # impl FullUser for MyUser {
	/// #     fn username(&self) -> &str { &self.username }
	/// #     fn email(&self) -> &str { &self.email }
	/// #     fn first_name(&self) -> &str { &self.first_name }
	/// #     fn last_name(&self) -> &str { &self.last_name }
	/// #     fn is_staff(&self) -> bool { self.is_staff }
	/// #     fn is_superuser(&self) -> bool { self.is_superuser }
	/// #     fn date_joined(&self) -> DateTime<Utc> { self.date_joined }
	/// # }
	///
	/// # #[cfg(feature = "argon2-hasher")]
	/// # {
	/// let user = MyUser {
	///     id: Uuid::now_v7(),
	///     username: "bob".to_string(),
	///     email: "bob@example.com".to_string(),
	///     first_name: "Bob".to_string(),
	///     last_name: "Jones".to_string(),
	///     password_hash: None,
	///     last_login: None,
	///     is_active: true,
	///     is_staff: false,
	///     is_superuser: false,
	///     date_joined: Utc::now(),
	/// };
	///
	/// assert_eq!(user.get_full_name(), "Bob Jones");
	/// # }
	/// ```
	fn get_full_name(&self) -> String {
		format!("{} {}", self.first_name(), self.last_name())
			.trim()
			.to_string()
	}

	/// Returns the short name (first name only)
	///
	/// # Examples
	///
	/// ```no_run
	/// # use reinhardt_auth::{BaseUser, FullUser, PasswordHasher};
	/// # #[cfg(feature = "argon2-hasher")]
	/// # use reinhardt_auth::Argon2Hasher;
	/// # use uuid::Uuid;
	/// # use chrono::{DateTime, Utc};
	/// # use serde::{Serialize, Deserialize};
	/// # #[derive(Serialize, Deserialize)]
	/// # struct MyUser { id: Uuid, username: String, email: String,
	/// #   first_name: String, last_name: String, password_hash: Option<String>,
	/// #   last_login: Option<DateTime<Utc>>, is_active: bool, is_staff: bool,
	/// #   is_superuser: bool, date_joined: DateTime<Utc> }
	/// # #[cfg(feature = "argon2-hasher")]
	/// # impl BaseUser for MyUser {
	/// #     type PrimaryKey = Uuid;
	/// #     type Hasher = Argon2Hasher;
	/// #     fn get_username_field() -> &'static str { "username" }
	/// #     fn get_username(&self) -> &str { &self.username }
	/// #     fn password_hash(&self) -> Option<&str> { self.password_hash.as_deref() }
	/// #     fn set_password_hash(&mut self, hash: String) { self.password_hash = Some(hash); }
	/// #     fn last_login(&self) -> Option<DateTime<Utc>> { self.last_login }
	/// #     fn set_last_login(&mut self, time: DateTime<Utc>) { self.last_login = Some(time); }
	/// #     fn is_active(&self) -> bool { self.is_active }
	/// # }
	/// # impl FullUser for MyUser {
	/// #     fn username(&self) -> &str { &self.username }
	/// #     fn email(&self) -> &str { &self.email }
	/// #     fn first_name(&self) -> &str { &self.first_name }
	/// #     fn last_name(&self) -> &str { &self.last_name }
	/// #     fn is_staff(&self) -> bool { self.is_staff }
	/// #     fn is_superuser(&self) -> bool { self.is_superuser }
	/// #     fn date_joined(&self) -> DateTime<Utc> { self.date_joined }
	/// # }
	///
	/// # #[cfg(feature = "argon2-hasher")]
	/// # {
	/// let user = MyUser {
	///     id: Uuid::now_v7(),
	///     username: "charlie".to_string(),
	///     email: "charlie@example.com".to_string(),
	///     first_name: "Charlie".to_string(),
	///     last_name: "Brown".to_string(),
	///     password_hash: None,
	///     last_login: None,
	///     is_active: true,
	///     is_staff: false,
	///     is_superuser: false,
	///     date_joined: Utc::now(),
	/// };
	///
	/// assert_eq!(user.get_short_name(), "Charlie");
	/// # }
	/// ```
	fn get_short_name(&self) -> &str {
		self.first_name()
	}
}
