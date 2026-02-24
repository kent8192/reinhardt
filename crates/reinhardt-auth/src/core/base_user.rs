use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt::Display;
use unicode_normalization::UnicodeNormalization;

use crate::core::hasher::PasswordHasher;

/// BaseUser trait - Django-style authentication
///
/// This trait provides the core user authentication functionality, including
/// password hashing and verification. It is inspired by Django's AbstractBaseUser.
///
/// # Type Parameters
///
/// * `PrimaryKey` - The type of the user's primary key (e.g., `Uuid`, `i64`)
/// * `Hasher` - The password hasher implementation (default: `Argon2Hasher`)
///
/// # Examples
///
/// ```
/// use reinhardt_auth::{BaseUser, PasswordHasher};
/// #[cfg(feature = "argon2-hasher")]
/// use reinhardt_auth::Argon2Hasher;
/// use uuid::Uuid;
/// use chrono::{DateTime, Utc};
/// use serde::{Serialize, Deserialize};
///
/// #[derive(Serialize, Deserialize)]
/// struct MyUser {
///     id: Uuid,
///     email: String,
///     password_hash: Option<String>,
///     last_login: Option<DateTime<Utc>>,
///     is_active: bool,
/// }
///
/// #[cfg(feature = "argon2-hasher")]
/// impl BaseUser for MyUser {
///     type PrimaryKey = Uuid;
///     type Hasher = Argon2Hasher;
///
///     fn get_username_field() -> &'static str { "email" }
///     fn get_username(&self) -> &str { &self.email }
///     fn password_hash(&self) -> Option<&str> { self.password_hash.as_deref() }
///     fn set_password_hash(&mut self, hash: String) { self.password_hash = Some(hash); }
///     fn last_login(&self) -> Option<DateTime<Utc>> { self.last_login }
///     fn set_last_login(&mut self, time: DateTime<Utc>) { self.last_login = Some(time); }
///     fn is_active(&self) -> bool { self.is_active }
/// }
///
/// # #[cfg(feature = "argon2-hasher")]
/// # {
/// let mut user = MyUser {
///     id: Uuid::new_v4(),
///     email: "user@example.com".to_string(),
///     password_hash: None,
///     last_login: None,
///     is_active: true,
/// };
///
/// // Set password
/// user.set_password("secure_password123").unwrap();
/// assert!(user.has_usable_password());
///
/// // Check password
/// assert!(user.check_password("secure_password123").unwrap());
/// assert!(!user.check_password("wrong_password").unwrap());
/// # }
/// ```
pub trait BaseUser: Send + Sync + Serialize + for<'de> Deserialize<'de> {
	/// The type of the primary key (e.g., Uuid, i64)
	type PrimaryKey: Clone + Send + Sync + Display;

	/// The password hasher to use (e.g., Argon2Hasher)
	type Hasher: PasswordHasher + Default;

	/// Returns the name of the username field
	///
	/// This is typically "username" or "email", depending on your user model.
	fn get_username_field() -> &'static str;

	/// Returns the username value
	fn get_username(&self) -> &str;

	/// Returns the hashed password, if set
	fn password_hash(&self) -> Option<&str>;

	/// Sets the hashed password
	fn set_password_hash(&mut self, hash: String);

	/// Returns the last login time
	fn last_login(&self) -> Option<DateTime<Utc>>;

	/// Sets the last login time
	fn set_last_login(&mut self, time: DateTime<Utc>);

	/// Returns whether the user account is active
	fn is_active(&self) -> bool;

	/// Normalizes the username for consistent storage
	///
	/// By default, applies NFKC Unicode normalization. Override this method
	/// if you need different normalization behavior.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_auth::BaseUser;
	/// # use reinhardt_auth::PasswordHasher;
	/// # #[cfg(feature = "argon2-hasher")]
	/// # use reinhardt_auth::Argon2Hasher;
	/// # use uuid::Uuid;
	/// # use chrono::{DateTime, Utc};
	/// # use serde::{Serialize, Deserialize};
	/// # #[derive(Serialize, Deserialize)]
	/// # struct MyUser { id: Uuid, email: String, password_hash: Option<String>,
	/// #   last_login: Option<DateTime<Utc>>, is_active: bool }
	/// # #[cfg(feature = "argon2-hasher")]
	/// # impl BaseUser for MyUser {
	/// #     type PrimaryKey = Uuid;
	/// #     type Hasher = Argon2Hasher;
	/// #     fn get_username_field() -> &'static str { "email" }
	/// #     fn get_username(&self) -> &str { &self.email }
	/// #     fn password_hash(&self) -> Option<&str> { self.password_hash.as_deref() }
	/// #     fn set_password_hash(&mut self, hash: String) { self.password_hash = Some(hash); }
	/// #     fn last_login(&self) -> Option<DateTime<Utc>> { self.last_login }
	/// #     fn set_last_login(&mut self, time: DateTime<Utc>) { self.last_login = Some(time); }
	/// #     fn is_active(&self) -> bool { self.is_active }
	/// # }
	///
	/// # #[cfg(feature = "argon2-hasher")]
	/// # {
	/// let normalized = MyUser::normalize_username("Åsa@example.com");
	/// assert_eq!(normalized, "Åsa@example.com"); // NFKC normalized
	/// # }
	/// ```
	fn normalize_username(username: &str) -> String {
		username.nfkc().collect()
	}

	/// Sets the password, hashing it first
	///
	/// # Examples
	///
	/// ```
	/// # use reinhardt_auth::BaseUser;
	/// # use reinhardt_auth::PasswordHasher;
	/// # #[cfg(feature = "argon2-hasher")]
	/// # use reinhardt_auth::Argon2Hasher;
	/// # use uuid::Uuid;
	/// # use chrono::{DateTime, Utc};
	/// # use serde::{Serialize, Deserialize};
	/// # #[derive(Serialize, Deserialize)]
	/// # struct MyUser { id: Uuid, email: String, password_hash: Option<String>,
	/// #   last_login: Option<DateTime<Utc>>, is_active: bool }
	/// # #[cfg(feature = "argon2-hasher")]
	/// # impl BaseUser for MyUser {
	/// #     type PrimaryKey = Uuid;
	/// #     type Hasher = Argon2Hasher;
	/// #     fn get_username_field() -> &'static str { "email" }
	/// #     fn get_username(&self) -> &str { &self.email }
	/// #     fn password_hash(&self) -> Option<&str> { self.password_hash.as_deref() }
	/// #     fn set_password_hash(&mut self, hash: String) { self.password_hash = Some(hash); }
	/// #     fn last_login(&self) -> Option<DateTime<Utc>> { self.last_login }
	/// #     fn set_last_login(&mut self, time: DateTime<Utc>) { self.last_login = Some(time); }
	/// #     fn is_active(&self) -> bool { self.is_active }
	/// # }
	///
	/// # #[cfg(feature = "argon2-hasher")]
	/// # {
	/// let mut user = MyUser {
	///     id: Uuid::new_v4(),
	///     email: "user@example.com".to_string(),
	///     password_hash: None,
	///     last_login: None,
	///     is_active: true,
	/// };
	///
	/// user.set_password("my_secure_password").unwrap();
	/// assert!(user.password_hash().is_some());
	/// # }
	/// ```
	fn set_password(&mut self, password: &str) -> Result<(), reinhardt_core::exception::Error> {
		let hasher = Self::Hasher::default();
		let hash = hasher.hash(password)?;
		self.set_password_hash(hash);
		Ok(())
	}

	/// Checks if the given password is correct
	///
	/// # Examples
	///
	/// ```
	/// # use reinhardt_auth::BaseUser;
	/// # use reinhardt_auth::PasswordHasher;
	/// # #[cfg(feature = "argon2-hasher")]
	/// # use reinhardt_auth::Argon2Hasher;
	/// # use uuid::Uuid;
	/// # use chrono::{DateTime, Utc};
	/// # use serde::{Serialize, Deserialize};
	/// # #[derive(Serialize, Deserialize)]
	/// # struct MyUser { id: Uuid, email: String, password_hash: Option<String>,
	/// #   last_login: Option<DateTime<Utc>>, is_active: bool }
	/// # #[cfg(feature = "argon2-hasher")]
	/// # impl BaseUser for MyUser {
	/// #     type PrimaryKey = Uuid;
	/// #     type Hasher = Argon2Hasher;
	/// #     fn get_username_field() -> &'static str { "email" }
	/// #     fn get_username(&self) -> &str { &self.email }
	/// #     fn password_hash(&self) -> Option<&str> { self.password_hash.as_deref() }
	/// #     fn set_password_hash(&mut self, hash: String) { self.password_hash = Some(hash); }
	/// #     fn last_login(&self) -> Option<DateTime<Utc>> { self.last_login }
	/// #     fn set_last_login(&mut self, time: DateTime<Utc>) { self.last_login = Some(time); }
	/// #     fn is_active(&self) -> bool { self.is_active }
	/// # }
	///
	/// # #[cfg(feature = "argon2-hasher")]
	/// # {
	/// let mut user = MyUser {
	///     id: Uuid::new_v4(),
	///     email: "user@example.com".to_string(),
	///     password_hash: None,
	///     last_login: None,
	///     is_active: true,
	/// };
	///
	/// user.set_password("correct_password").unwrap();
	///
	/// assert!(user.check_password("correct_password").unwrap());
	/// assert!(!user.check_password("wrong_password").unwrap());
	/// # }
	/// ```
	fn check_password(&self, password: &str) -> Result<bool, reinhardt_core::exception::Error> {
		// Return false early if password is not usable (e.g., "!" marker)
		if !self.has_usable_password() {
			return Ok(false);
		}

		match self.password_hash() {
			Some(hash) => {
				let hasher = Self::Hasher::default();
				hasher.verify(password, hash)
			}
			None => Ok(false),
		}
	}

	/// Sets an unusable password (user cannot log in with password)
	///
	/// # Examples
	///
	/// ```ignore
	/// # use reinhardt_auth::BaseUser;
	/// # use reinhardt_auth::PasswordHasher;
	/// # #[cfg(feature = "argon2-hasher")]
	/// # use reinhardt_auth::Argon2Hasher;
	/// # use uuid::Uuid;
	/// # use chrono::{DateTime, Utc};
	/// # use serde::{Serialize, Deserialize};
	/// # #[derive(Serialize, Deserialize)]
	/// # struct MyUser { id: Uuid, email: String, password_hash: Option<String>,
	/// #   last_login: Option<DateTime<Utc>>, is_active: bool }
	/// # #[cfg(feature = "argon2-hasher")]
	/// # impl BaseUser for MyUser {
	/// #     type PrimaryKey = Uuid;
	/// #     type Hasher = Argon2Hasher;
	/// #     fn get_username_field() -> &'static str { "email" }
	/// #     fn get_username(&self) -> &str { &self.email }
	/// #     fn password_hash(&self) -> Option<&str> { self.password_hash.as_deref() }
	/// #     fn set_password_hash(&mut self, hash: String) { self.password_hash = Some(hash); }
	/// #     fn last_login(&self) -> Option<DateTime<Utc>> { self.last_login }
	/// #     fn set_last_login(&mut self, time: DateTime<Utc>) { self.last_login = Some(time); }
	/// #     fn is_active(&self) -> bool { self.is_active }
	/// # }
	///
	/// let mut user = MyUser {
	///     id: Uuid::new_v4(),
	///     email: "user@example.com".to_string(),
	///     password_hash: None,
	///     last_login: None,
	///     is_active: true,
	/// };
	///
	/// user.set_unusable_password();
	/// assert!(!user.has_usable_password());
	/// ```
	fn set_unusable_password(&mut self) {
		self.set_password_hash("!".to_string());
	}

	/// Returns whether the user has a usable password
	///
	/// # Examples
	///
	/// ```
	/// # use reinhardt_auth::BaseUser;
	/// # use reinhardt_auth::PasswordHasher;
	/// # #[cfg(feature = "argon2-hasher")]
	/// # use reinhardt_auth::Argon2Hasher;
	/// # use uuid::Uuid;
	/// # use chrono::{DateTime, Utc};
	/// # use serde::{Serialize, Deserialize};
	/// # #[derive(Serialize, Deserialize)]
	/// # struct MyUser { id: Uuid, email: String, password_hash: Option<String>,
	/// #   last_login: Option<DateTime<Utc>>, is_active: bool }
	/// # #[cfg(feature = "argon2-hasher")]
	/// # impl BaseUser for MyUser {
	/// #     type PrimaryKey = Uuid;
	/// #     type Hasher = Argon2Hasher;
	/// #     fn get_username_field() -> &'static str { "email" }
	/// #     fn get_username(&self) -> &str { &self.email }
	/// #     fn password_hash(&self) -> Option<&str> { self.password_hash.as_deref() }
	/// #     fn set_password_hash(&mut self, hash: String) { self.password_hash = Some(hash); }
	/// #     fn last_login(&self) -> Option<DateTime<Utc>> { self.last_login }
	/// #     fn set_last_login(&mut self, time: DateTime<Utc>) { self.last_login = Some(time); }
	/// #     fn is_active(&self) -> bool { self.is_active }
	/// # }
	///
	/// # #[cfg(feature = "argon2-hasher")]
	/// # {
	/// let mut user = MyUser {
	///     id: Uuid::new_v4(),
	///     email: "user@example.com".to_string(),
	///     password_hash: None,
	///     last_login: None,
	///     is_active: true,
	/// };
	///
	/// assert!(!user.has_usable_password());
	///
	/// user.set_password("password123").unwrap();
	/// assert!(user.has_usable_password());
	///
	/// user.set_unusable_password();
	/// assert!(!user.has_usable_password());
	/// # }
	/// ```
	fn has_usable_password(&self) -> bool {
		match self.password_hash() {
			Some(hash) => !hash.is_empty() && hash != "!",
			None => false,
		}
	}

	/// Returns a hash of the session authentication credentials
	///
	/// Uses HMAC-SHA256 with the provided secret as key material combined with the
	/// password hash. The secret should be a server-side secret (e.g., `SECRET_KEY`
	/// from settings) to prevent session forgery.
	///
	/// # Arguments
	///
	/// * `secret` - A server-side secret used as HMAC key material. This should be
	///   the application's `SECRET_KEY` or equivalent cryptographic secret.
	///
	/// # Examples
	///
	/// ```
	/// # use reinhardt_auth::BaseUser;
	/// # use reinhardt_auth::PasswordHasher;
	/// # #[cfg(feature = "argon2-hasher")]
	/// # use reinhardt_auth::Argon2Hasher;
	/// # use uuid::Uuid;
	/// # use chrono::{DateTime, Utc};
	/// # use serde::{Serialize, Deserialize};
	/// # #[derive(Serialize, Deserialize)]
	/// # struct MyUser { id: Uuid, email: String, password_hash: Option<String>,
	/// #   last_login: Option<DateTime<Utc>>, is_active: bool }
	/// # #[cfg(feature = "argon2-hasher")]
	/// # impl BaseUser for MyUser {
	/// #     type PrimaryKey = Uuid;
	/// #     type Hasher = Argon2Hasher;
	/// #     fn get_username_field() -> &'static str { "email" }
	/// #     fn get_username(&self) -> &str { &self.email }
	/// #     fn password_hash(&self) -> Option<&str> { self.password_hash.as_deref() }
	/// #     fn set_password_hash(&mut self, hash: String) { self.password_hash = Some(hash); }
	/// #     fn last_login(&self) -> Option<DateTime<Utc>> { self.last_login }
	/// #     fn set_last_login(&mut self, time: DateTime<Utc>) { self.last_login = Some(time); }
	/// #     fn is_active(&self) -> bool { self.is_active }
	/// # }
	///
	/// # #[cfg(feature = "argon2-hasher")]
	/// # {
	/// let mut user = MyUser {
	///     id: Uuid::new_v4(),
	///     email: "user@example.com".to_string(),
	///     password_hash: None,
	///     last_login: None,
	///     is_active: true,
	/// };
	///
	/// let secret = "my-server-secret-key";
	/// user.set_password("password123").unwrap();
	/// let hash1 = user.get_session_auth_hash(secret);
	///
	/// user.set_password("new_password").unwrap();
	/// let hash2 = user.get_session_auth_hash(secret);
	///
	/// assert_ne!(hash1, hash2); // Hash changes when password changes
	/// # }
	/// ```
	fn get_session_auth_hash(&self, secret: &str) -> String {
		use hmac::{Hmac, Mac};
		use sha2::Sha256;

		let password_hash = self.password_hash().unwrap_or("");
		// Derive HMAC key from the server secret combined with a domain separator
		let key = format!("reinhardt.auth.session_hash:{}", secret);

		let mut mac =
			Hmac::<Sha256>::new_from_slice(key.as_bytes()).expect("HMAC can take key of any size");
		mac.update(password_hash.as_bytes());

		hex::encode(mac.finalize().into_bytes())
	}
}
