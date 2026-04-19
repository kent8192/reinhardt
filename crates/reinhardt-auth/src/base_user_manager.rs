use crate::BaseUser;
use async_trait::async_trait;
use reinhardt_core::exception::Error;

type Result<T> = std::result::Result<T, Error>;
use serde_json::Value;
use std::collections::HashMap;

/// BaseUserManager trait - Django's BaseUserManager equivalent
///
/// Provides an interface for creating and managing user objects. This trait defines
/// the essential methods needed for user management, including user and superuser creation.
///
/// # Relationship with Django
///
/// This trait corresponds to Django's `BaseUserManager`, which provides:
/// - `create_user()` - Creates a normal user
/// - `create_superuser()` - Creates a superuser/admin
/// - `normalize_email()` - Normalizes email addresses
///
/// # Examples
///
/// Implementing a simple in-memory user manager:
///
/// ```no_run
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// use reinhardt_auth::{BaseUser, BaseUserManager, Argon2Hasher};
/// use reinhardt_core::exception::Result;
/// use async_trait::async_trait;
/// use std::collections::HashMap;
/// use serde_json::Value;
/// use uuid::Uuid;
/// use chrono::Utc;
/// use serde::{Serialize, Deserialize};
///
/// #[derive(Clone, Serialize, Deserialize)]
/// struct MyUser {
///     id: Uuid,
///     email: String,
///     password_hash: Option<String>,
///     last_login: Option<chrono::DateTime<Utc>>,
///     is_active: bool,
///     is_admin: bool,
/// }
///
/// impl BaseUser for MyUser {
///     type PrimaryKey = Uuid;
///     type Hasher = Argon2Hasher;
///
///     fn get_username_field() -> &'static str { "email" }
///     fn get_username(&self) -> &str { &self.email }
///     fn password_hash(&self) -> Option<&str> { self.password_hash.as_deref() }
///     fn set_password_hash(&mut self, hash: String) { self.password_hash = Some(hash); }
///     fn last_login(&self) -> Option<chrono::DateTime<Utc>> { self.last_login }
///     fn set_last_login(&mut self, time: chrono::DateTime<Utc>) { self.last_login = Some(time); }
///     fn is_active(&self) -> bool { self.is_active }
/// }
///
/// struct MyUserManager {
///     users: HashMap<Uuid, MyUser>,
/// }
///
/// #[async_trait]
/// impl BaseUserManager<MyUser> for MyUserManager {
///     async fn create_user(
///         &mut self,
///         username: &str,
///         password: Option<&str>,
///         extra: HashMap<String, Value>,
///     ) -> Result<MyUser> {
///         let mut user = MyUser {
///             id: Uuid::now_v7(),
///             email: username.to_string(),
///             password_hash: None,
///             last_login: None,
///             is_active: true,
///             is_admin: false,
///         };
///
///         if let Some(pwd) = password {
///             user.set_password(pwd)?;
///         }
///
///         self.users.insert(user.id, user.clone());
///         Ok(user)
///     }
///
///     async fn create_superuser(
///         &mut self,
///         username: &str,
///         password: Option<&str>,
///         extra: HashMap<String, Value>,
///     ) -> Result<MyUser> {
///         let mut user = self.create_user(username, password, extra).await?;
///         user.is_admin = true;
///         self.users.insert(user.id, user.clone());
///         Ok(user)
///     }
/// }
/// # Ok(())
/// # }
/// ```
#[async_trait]
pub trait BaseUserManager<U: BaseUser>: Send + Sync {
	/// Creates a new user with the given username and password
	///
	/// This method should:
	/// 1. Validate the username (check uniqueness, format, etc.)
	/// 2. Create a new user instance
	/// 3. Set the password using `set_password()` (which automatically hashes it)
	/// 4. Apply any additional fields from `extra`
	/// 5. Save the user to the backing store
	///
	/// # Arguments
	///
	/// * `username` - The username/email for the new user
	/// * `password` - Optional password (will be hashed automatically)
	/// * `extra` - Additional fields to set on the user
	///
	/// # Examples
	///
	/// ```ignore
	/// let mut manager = MyUserManager::new();
	/// let user = manager.create_user(
	///     "alice@example.com",
	///     Some("securepass123"),
	///     HashMap::new()
	/// ).await?;
	/// ```
	async fn create_user(
		&mut self,
		username: &str,
		password: Option<&str>,
		extra: HashMap<String, Value>,
	) -> Result<U>;

	/// Creates a new superuser with the given username and password
	///
	/// This method should:
	/// 1. Call `create_user()` to create the base user
	/// 2. Set superuser flags (is_staff=true, is_superuser=true, etc.)
	/// 3. Save the updated user
	///
	/// # Arguments
	///
	/// * `username` - The username/email for the new superuser
	/// * `password` - Optional password (will be hashed automatically)
	/// * `extra` - Additional fields to set on the user
	///
	/// # Examples
	///
	/// ```ignore
	/// let mut manager = MyUserManager::new();
	/// let superuser = manager.create_superuser(
	///     "admin@example.com",
	///     Some("adminsecret"),
	///     HashMap::new()
	/// ).await?;
	/// ```
	async fn create_superuser(
		&mut self,
		username: &str,
		password: Option<&str>,
		extra: HashMap<String, Value>,
	) -> Result<U>;

	/// Normalizes an email address
	///
	/// Converts the domain part of the email to lowercase to prevent case-sensitivity issues.
	/// This is the same normalization used by Django.
	///
	/// # Arguments
	///
	/// * `email` - The email address to normalize
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_auth::BaseUserManager;
	///
	/// # struct DummyManager;
	/// # impl DummyManager {
	/// #     fn normalize_email(email: &str) -> String {
	/// #         let parts: Vec<&str> = email.split('@').collect();
	/// #         if parts.len() == 2 {
	/// #             format!("{}@{}", parts[0], parts[1].to_lowercase())
	/// #         } else {
	/// #             email.to_string()
	/// #         }
	/// #     }
	/// # }
	/// let normalized = DummyManager::normalize_email("Alice@EXAMPLE.COM");
	/// assert_eq!(normalized, "Alice@example.com");
	///
	/// let already_normal = DummyManager::normalize_email("bob@example.com");
	/// assert_eq!(already_normal, "bob@example.com");
	/// ```
	fn normalize_email(email: &str) -> String {
		let parts: Vec<&str> = email.split('@').collect();
		if parts.len() == 2 {
			format!("{}@{}", parts[0], parts[1].to_lowercase())
		} else {
			email.to_string()
		}
	}
}
