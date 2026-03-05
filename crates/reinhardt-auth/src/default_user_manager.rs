#[cfg(feature = "argon2-hasher")]
use crate::BaseUser;
#[cfg(feature = "argon2-hasher")]
use crate::base_user_manager::BaseUserManager;
#[cfg(feature = "argon2-hasher")]
use crate::default_user::DefaultUser;
#[cfg(feature = "argon2-hasher")]
use async_trait::async_trait;
#[cfg(feature = "argon2-hasher")]
use chrono::Utc;
#[cfg(feature = "argon2-hasher")]
use reinhardt_core::exception::Error;

#[cfg(feature = "argon2-hasher")]
type Result<T> = std::result::Result<T, Error>;
#[cfg(feature = "argon2-hasher")]
use serde_json::Value;
#[cfg(feature = "argon2-hasher")]
use std::collections::HashMap;
#[cfg(feature = "argon2-hasher")]
use std::sync::{Arc, RwLock};
#[cfg(feature = "argon2-hasher")]
use uuid::Uuid;

/// DefaultUserManager - In-memory user manager for DefaultUser
///
/// A simple in-memory implementation of BaseUserManager for DefaultUser.
/// This is primarily for demonstration and testing purposes.
///
/// For production use, you should implement your own manager that persists
/// users to a database using Reinhardt's ORM.
///
/// # Relationship with Django
///
/// This corresponds to Django's `UserManager` class, but with in-memory storage
/// instead of database persistence. In Django, UserManager is typically bound
/// to a model via `objects = UserManager()`.
///
/// # Examples
///
/// Creating and managing users:
///
/// ```
/// use reinhardt_auth::{BaseUserManager, DefaultUserManager};
/// use std::collections::HashMap;
///
/// # tokio_test::block_on(async {
/// let mut manager = DefaultUserManager::new();
///
/// // Create a regular user
/// let user = manager.create_user(
///     "alice",
///     Some("securepass123"),
///     HashMap::new()
/// ).await.unwrap();
///
/// assert_eq!(user.username, "alice");
/// assert!(user.is_active);
/// assert!(!user.is_staff);
/// assert!(!user.is_superuser);
///
/// // Create a superuser
/// let admin = manager.create_superuser(
///     "admin",
///     Some("adminsecret"),
///     HashMap::new()
/// ).await.unwrap();
///
/// assert!(admin.is_staff);
/// assert!(admin.is_superuser);
/// # })
/// ```
///
/// Creating user with extra fields:
///
/// ```
/// use reinhardt_auth::{BaseUserManager, DefaultUserManager};
/// use std::collections::HashMap;
/// use serde_json::json;
///
/// # tokio_test::block_on(async {
/// let mut manager = DefaultUserManager::new();
/// let mut extra = HashMap::new();
/// extra.insert("email".to_string(), json!("bob@example.com"));
/// extra.insert("first_name".to_string(), json!("Bob"));
/// extra.insert("last_name".to_string(), json!("Johnson"));
///
/// let user = manager.create_user(
///     "bob",
///     Some("password"),
///     extra
/// ).await.unwrap();
///
/// assert_eq!(user.email, "bob@example.com");
/// assert_eq!(user.first_name, "Bob");
/// assert_eq!(user.last_name, "Johnson");
/// # })
/// ```
#[cfg(feature = "argon2-hasher")]
pub struct DefaultUserManager {
	users: Arc<RwLock<HashMap<Uuid, DefaultUser>>>,
}

#[cfg(feature = "argon2-hasher")]
impl DefaultUserManager {
	/// Creates a new DefaultUserManager with empty user storage
	pub fn new() -> Self {
		Self {
			users: Arc::new(RwLock::new(HashMap::new())),
		}
	}

	/// Gets a user by ID (for internal use)
	pub fn get_by_id(&self, id: Uuid) -> Option<DefaultUser> {
		let users = self.users.read().unwrap_or_else(|e| e.into_inner());
		users.get(&id).cloned()
	}

	/// Gets a user by username (for internal use)
	pub fn get_by_username(&self, username: &str) -> Option<DefaultUser> {
		let users = self.users.read().unwrap_or_else(|e| e.into_inner());
		users.values().find(|u| u.username == username).cloned()
	}

	/// Lists all users (for internal use)
	pub fn list_all(&self) -> Vec<DefaultUser> {
		let users = self.users.read().unwrap_or_else(|e| e.into_inner());
		users.values().cloned().collect()
	}
}

#[cfg(feature = "argon2-hasher")]
impl Default for DefaultUserManager {
	fn default() -> Self {
		Self::new()
	}
}

#[cfg(feature = "argon2-hasher")]
#[async_trait]
impl BaseUserManager<DefaultUser> for DefaultUserManager {
	async fn create_user(
		&mut self,
		username: &str,
		password: Option<&str>,
		extra: HashMap<String, Value>,
	) -> Result<DefaultUser> {
		// Check if username already exists
		if self.get_by_username(username).is_some() {
			return Err(reinhardt_core::exception::Error::Validation(format!(
				"Username '{}' already exists",
				username
			)));
		}

		// Create user with default values
		let mut user = DefaultUser {
			id: Uuid::new_v4(),
			username: username.to_string(),
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
		};

		// Apply extra fields
		if let Some(email) = extra.get("email")
			&& let Some(email_str) = email.as_str()
		{
			user.email = Self::normalize_email(email_str);
		}

		if let Some(first_name) = extra.get("first_name")
			&& let Some(name) = first_name.as_str()
		{
			user.first_name = name.to_string();
		}

		if let Some(last_name) = extra.get("last_name")
			&& let Some(name) = last_name.as_str()
		{
			user.last_name = name.to_string();
		}

		if let Some(is_active) = extra.get("is_active")
			&& let Some(active) = is_active.as_bool()
		{
			user.is_active = active;
		}

		// Set password if provided (automatically hashed via BaseUser trait)
		if let Some(pwd) = password {
			user.set_password(pwd)?;
		}

		// Store user
		let mut users = self.users.write().unwrap_or_else(|e| e.into_inner());
		users.insert(user.id, user.clone());

		Ok(user)
	}

	async fn create_superuser(
		&mut self,
		username: &str,
		password: Option<&str>,
		extra: HashMap<String, Value>,
	) -> Result<DefaultUser> {
		// Create base user first
		let mut user = self.create_user(username, password, extra).await?;

		// Promote to superuser
		user.is_staff = true;
		user.is_superuser = true;

		// Update storage
		let mut users = self.users.write().unwrap_or_else(|e| e.into_inner());
		users.insert(user.id, user.clone());

		Ok(user)
	}
}

#[cfg(feature = "argon2-hasher")]
#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;
	use std::collections::HashMap;

	#[rstest]
	#[tokio::test]
	async fn test_rwlock_poison_recovery_default_user_manager() {
		// Arrange
		let mut manager = DefaultUserManager::new();
		let user = manager
			.create_user("pre_poison", Some("password123"), HashMap::new())
			.await
			.unwrap();
		let user_id = user.id;

		// Act - poison the RwLock by panicking while holding a write guard
		let users_clone = Arc::clone(&manager.users);
		let _ = std::thread::spawn(move || {
			let _guard = users_clone.write().unwrap();
			panic!("intentional panic to poison lock");
		})
		.join();

		// Assert - operations still work after poison recovery
		let found = manager.get_by_id(user_id);
		assert!(found.is_some());
		assert_eq!(found.unwrap().username, "pre_poison");

		let found_by_name = manager.get_by_username("pre_poison");
		assert!(found_by_name.is_some());

		let all_users = manager.list_all();
		assert_eq!(all_users.len(), 1);

		// Create a new user after poison recovery
		let new_user = manager
			.create_user("post_poison", Some("password456"), HashMap::new())
			.await
			.unwrap();
		assert_eq!(new_user.username, "post_poison");
		assert_eq!(manager.list_all().len(), 2);
	}
}
