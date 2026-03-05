//! User Management
//!
//! Provides CRUD operations for user management.

use crate::PasswordHasher;
use crate::SimpleUser;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

/// User management error
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UserManagementError {
	/// User not found
	UserNotFound,
	/// User already exists
	UserAlreadyExists,
	/// Invalid username
	InvalidUsername,
	/// Invalid email
	InvalidEmail,
	/// Invalid password
	InvalidPassword,
	/// Database error
	DatabaseError(String),
	/// Other error
	Other(String),
}

impl std::fmt::Display for UserManagementError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			UserManagementError::UserNotFound => write!(f, "User not found"),
			UserManagementError::UserAlreadyExists => write!(f, "User already exists"),
			UserManagementError::InvalidUsername => write!(f, "Invalid username"),
			UserManagementError::InvalidEmail => write!(f, "Invalid email"),
			UserManagementError::InvalidPassword => write!(f, "Invalid password"),
			UserManagementError::DatabaseError(msg) => write!(f, "Database error: {}", msg),
			UserManagementError::Other(msg) => write!(f, "Error: {}", msg),
		}
	}
}

impl std::error::Error for UserManagementError {}

/// User management result
pub type UserManagementResult<T> = Result<T, UserManagementError>;

/// User data for creation
///
/// # Examples
///
/// ```
/// use reinhardt_auth::user_management::CreateUserData;
///
/// let user_data = CreateUserData {
///     username: "alice".to_string(),
///     email: "alice@example.com".to_string(),
///     password: "password123".to_string(),
///     is_active: true,
///     is_admin: false,
/// };
///
/// assert_eq!(user_data.username, "alice");
/// assert_eq!(user_data.email, "alice@example.com");
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateUserData {
	pub username: String,
	pub email: String,
	pub password: String,
	pub is_active: bool,
	pub is_admin: bool,
}

/// User data for update
///
/// # Examples
///
/// ```
/// use reinhardt_auth::user_management::UpdateUserData;
///
/// let update_data = UpdateUserData {
///     email: Some("newemail@example.com".to_string()),
///     is_active: Some(false),
///     is_admin: None,
/// };
///
/// assert_eq!(update_data.email, Some("newemail@example.com".to_string()));
/// assert_eq!(update_data.is_active, Some(false));
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UpdateUserData {
	pub email: Option<String>,
	pub is_active: Option<bool>,
	pub is_admin: Option<bool>,
}

/// User manager
///
/// Provides CRUD operations for users.
///
/// # Examples
///
/// ```rust
/// # use reinhardt_auth::PasswordHasher;
/// # struct MockHasher;
/// # impl PasswordHasher for MockHasher {
/// #     fn hash(&self, p: &str) -> Result<String, reinhardt_core::exception::Error> {
/// #         Ok(format!("hash:{p}"))
/// #     }
/// #     fn verify(&self, p: &str, h: &str) -> Result<bool, reinhardt_core::exception::Error> {
/// #         Ok(h == format!("hash:{p}"))
/// #     }
/// # }
/// use reinhardt_auth::user_management::{UserManager, CreateUserData};
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let hasher = MockHasher;
///     let mut manager = UserManager::new(hasher);
///
///     // Create user
///     let user_data = CreateUserData {
///         username: "alice".to_string(),
///         email: "alice@example.com".to_string(),
///         password: "password123".to_string(),
///         is_active: true,
///         is_admin: false,
///     };
///
///     let user = manager.create_user(user_data).await.unwrap();
///     assert_eq!(user.username, "alice");
///
///     // Get user
///     let retrieved = manager.get_user(&user.id.to_string()).await.unwrap();
///     assert_eq!(retrieved.username, "alice");
///
///     // Delete user
///     manager.delete_user(&user.id.to_string()).await.unwrap();
///     assert!(manager.get_user(&user.id.to_string()).await.is_err());
///     Ok(())
/// }
/// ```
pub struct UserManager<H: PasswordHasher> {
	users: Arc<RwLock<HashMap<Uuid, SimpleUser>>>,
	username_index: Arc<RwLock<HashMap<String, Uuid>>>,
	password_hashes: Arc<RwLock<HashMap<Uuid, String>>>,
	hasher: H,
}

impl<H: PasswordHasher> UserManager<H> {
	/// Create a new user manager
	///
	/// # Examples
	///
	/// ```rust
	/// # use reinhardt_auth::PasswordHasher;
	/// # struct MockHasher;
	/// # impl PasswordHasher for MockHasher {
	/// #     fn hash(&self, p: &str) -> Result<String, reinhardt_core::exception::Error> {
	/// #         Ok(format!("hash:{p}"))
	/// #     }
	/// #     fn verify(&self, p: &str, h: &str) -> Result<bool, reinhardt_core::exception::Error> {
	/// #         Ok(h == format!("hash:{p}"))
	/// #     }
	/// # }
	/// use reinhardt_auth::user_management::UserManager;
	///
	/// let manager = UserManager::new(MockHasher);
	/// ```
	pub fn new(hasher: H) -> Self {
		Self {
			users: Arc::new(RwLock::new(HashMap::new())),
			username_index: Arc::new(RwLock::new(HashMap::new())),
			password_hashes: Arc::new(RwLock::new(HashMap::new())),
			hasher,
		}
	}

	/// Create a new user
	///
	/// # Examples
	///
	/// ```rust
	/// # use reinhardt_auth::PasswordHasher;
	/// # struct MockHasher;
	/// # impl PasswordHasher for MockHasher {
	/// #     fn hash(&self, p: &str) -> Result<String, reinhardt_core::exception::Error> {
	/// #         Ok(format!("hash:{p}"))
	/// #     }
	/// #     fn verify(&self, p: &str, h: &str) -> Result<bool, reinhardt_core::exception::Error> {
	/// #         Ok(h == format!("hash:{p}"))
	/// #     }
	/// # }
	/// use reinhardt_auth::user_management::{UserManager, CreateUserData};
	///
	/// #[tokio::main]
	/// async fn main() {
	///     let mut manager = UserManager::new(MockHasher);
	///
	///     let user_data = CreateUserData {
	///         username: "bob".to_string(),
	///         email: "bob@example.com".to_string(),
	///         password: "securepass".to_string(),
	///         is_active: true,
	///         is_admin: false,
	///     };
	///
	///     let user = manager.create_user(user_data).await.unwrap();
	///     assert_eq!(user.username, "bob");
	/// }
	/// ```
	pub async fn create_user(&mut self, data: CreateUserData) -> UserManagementResult<SimpleUser> {
		// Validate username
		if data.username.is_empty() || data.username.len() < 3 {
			return Err(UserManagementError::InvalidUsername);
		}

		// Validate email
		if !data.email.contains('@') || !data.email.contains('.') {
			return Err(UserManagementError::InvalidEmail);
		}

		// Validate password
		if data.password.len() < 8 {
			return Err(UserManagementError::InvalidPassword);
		}

		// Check if username already exists
		let username_index = self.username_index.read().await;
		if username_index.contains_key(&data.username) {
			return Err(UserManagementError::UserAlreadyExists);
		}
		drop(username_index);

		// Hash password
		let password_hash = self
			.hasher
			.hash(&data.password)
			.map_err(|e| UserManagementError::Other(e.to_string()))?;

		// Create user
		let user = SimpleUser {
			id: Uuid::new_v4(),
			username: data.username.clone(),
			email: data.email,
			is_active: data.is_active,
			is_admin: data.is_admin,
			is_staff: false,
			is_superuser: false,
		};

		// Store user
		let mut users = self.users.write().await;
		let mut username_index = self.username_index.write().await;
		let mut password_hashes = self.password_hashes.write().await;

		users.insert(user.id, user.clone());
		username_index.insert(data.username, user.id);
		password_hashes.insert(user.id, password_hash);

		Ok(user)
	}

	/// Get user by ID
	///
	/// # Examples
	///
	/// ```rust
	/// # use reinhardt_auth::PasswordHasher;
	/// # struct MockHasher;
	/// # impl PasswordHasher for MockHasher {
	/// #     fn hash(&self, p: &str) -> Result<String, reinhardt_core::exception::Error> {
	/// #         Ok(format!("hash:{p}"))
	/// #     }
	/// #     fn verify(&self, p: &str, h: &str) -> Result<bool, reinhardt_core::exception::Error> {
	/// #         Ok(h == format!("hash:{p}"))
	/// #     }
	/// # }
	/// use reinhardt_auth::user_management::{UserManager, CreateUserData};
	///
	/// #[tokio::main]
	/// async fn main() {
	///     let mut manager = UserManager::new(MockHasher);
	///
	///     let user_data = CreateUserData {
	///         username: "charlie".to_string(),
	///         email: "charlie@example.com".to_string(),
	///         password: "password123".to_string(),
	///         is_active: true,
	///         is_admin: false,
	///     };
	///
	///     let user = manager.create_user(user_data).await.unwrap();
	///     let retrieved = manager.get_user(&user.id.to_string()).await.unwrap();
	///     assert_eq!(retrieved.username, "charlie");
	/// }
	/// ```
	pub async fn get_user(&self, user_id: &str) -> UserManagementResult<SimpleUser> {
		let uuid = Uuid::parse_str(user_id)
			.map_err(|_| UserManagementError::Other("Invalid UUID".to_string()))?;

		let users = self.users.read().await;
		users
			.get(&uuid)
			.cloned()
			.ok_or(UserManagementError::UserNotFound)
	}

	/// Get user by username
	///
	/// # Examples
	///
	/// ```rust
	/// # use reinhardt_auth::PasswordHasher;
	/// # struct MockHasher;
	/// # impl PasswordHasher for MockHasher {
	/// #     fn hash(&self, p: &str) -> Result<String, reinhardt_core::exception::Error> {
	/// #         Ok(format!("hash:{p}"))
	/// #     }
	/// #     fn verify(&self, p: &str, h: &str) -> Result<bool, reinhardt_core::exception::Error> {
	/// #         Ok(h == format!("hash:{p}"))
	/// #     }
	/// # }
	/// use reinhardt_auth::user_management::{UserManager, CreateUserData};
	///
	/// #[tokio::main]
	/// async fn main() {
	///     let mut manager = UserManager::new(MockHasher);
	///
	///     let user_data = CreateUserData {
	///         username: "diana".to_string(),
	///         email: "diana@example.com".to_string(),
	///         password: "password123".to_string(),
	///         is_active: true,
	///         is_admin: false,
	///     };
	///
	///     manager.create_user(user_data).await.unwrap();
	///     let retrieved = manager.get_user_by_username("diana").await.unwrap();
	///     assert_eq!(retrieved.username, "diana");
	/// }
	/// ```
	pub async fn get_user_by_username(&self, username: &str) -> UserManagementResult<SimpleUser> {
		let username_index = self.username_index.read().await;
		let user_id = username_index
			.get(username)
			.ok_or(UserManagementError::UserNotFound)?;

		let users = self.users.read().await;
		users
			.get(user_id)
			.cloned()
			.ok_or(UserManagementError::UserNotFound)
	}

	/// Update user
	///
	/// # Examples
	///
	/// ```rust
	/// # use reinhardt_auth::PasswordHasher;
	/// # struct MockHasher;
	/// # impl PasswordHasher for MockHasher {
	/// #     fn hash(&self, p: &str) -> Result<String, reinhardt_core::exception::Error> {
	/// #         Ok(format!("hash:{p}"))
	/// #     }
	/// #     fn verify(&self, p: &str, h: &str) -> Result<bool, reinhardt_core::exception::Error> {
	/// #         Ok(h == format!("hash:{p}"))
	/// #     }
	/// # }
	/// use reinhardt_auth::user_management::{UserManager, CreateUserData, UpdateUserData};
	///
	/// #[tokio::main]
	/// async fn main() {
	///     let mut manager = UserManager::new(MockHasher);
	///
	///     let user_data = CreateUserData {
	///         username: "eve".to_string(),
	///         email: "eve@example.com".to_string(),
	///         password: "password123".to_string(),
	///         is_active: true,
	///         is_admin: false,
	///     };
	///
	///     let user = manager.create_user(user_data).await.unwrap();
	///
	///     let update_data = UpdateUserData {
	///         email: Some("newemail@example.com".to_string()),
	///         is_active: Some(false),
	///         is_admin: None,
	///     };
	///
	///     let updated = manager.update_user(&user.id.to_string(), update_data).await.unwrap();
	///     assert_eq!(updated.email, "newemail@example.com");
	///     assert!(!updated.is_active);
	/// }
	/// ```
	pub async fn update_user(
		&mut self,
		user_id: &str,
		data: UpdateUserData,
	) -> UserManagementResult<SimpleUser> {
		let uuid = Uuid::parse_str(user_id)
			.map_err(|_| UserManagementError::Other("Invalid UUID".to_string()))?;

		let mut users = self.users.write().await;
		let user = users
			.get_mut(&uuid)
			.ok_or(UserManagementError::UserNotFound)?;

		if let Some(email) = data.email {
			if !email.contains('@') || !email.contains('.') {
				return Err(UserManagementError::InvalidEmail);
			}
			user.email = email;
		}

		if let Some(is_active) = data.is_active {
			user.is_active = is_active;
		}

		if let Some(is_admin) = data.is_admin {
			user.is_admin = is_admin;
		}

		Ok(user.clone())
	}

	/// Delete user
	///
	/// # Examples
	///
	/// ```rust
	/// # use reinhardt_auth::PasswordHasher;
	/// # struct MockHasher;
	/// # impl PasswordHasher for MockHasher {
	/// #     fn hash(&self, p: &str) -> Result<String, reinhardt_core::exception::Error> {
	/// #         Ok(format!("hash:{p}"))
	/// #     }
	/// #     fn verify(&self, p: &str, h: &str) -> Result<bool, reinhardt_core::exception::Error> {
	/// #         Ok(h == format!("hash:{p}"))
	/// #     }
	/// # }
	/// use reinhardt_auth::user_management::{UserManager, CreateUserData};
	///
	/// #[tokio::main]
	/// async fn main() {
	///     let mut manager = UserManager::new(MockHasher);
	///
	///     let user_data = CreateUserData {
	///         username: "frank".to_string(),
	///         email: "frank@example.com".to_string(),
	///         password: "password123".to_string(),
	///         is_active: true,
	///         is_admin: false,
	///     };
	///
	///     let user = manager.create_user(user_data).await.unwrap();
	///     manager.delete_user(&user.id.to_string()).await.unwrap();
	///     assert!(manager.get_user(&user.id.to_string()).await.is_err());
	/// }
	/// ```
	pub async fn delete_user(&mut self, user_id: &str) -> UserManagementResult<()> {
		let uuid = Uuid::parse_str(user_id)
			.map_err(|_| UserManagementError::Other("Invalid UUID".to_string()))?;

		let mut users = self.users.write().await;
		let user = users
			.get(&uuid)
			.ok_or(UserManagementError::UserNotFound)?
			.clone();

		let mut username_index = self.username_index.write().await;
		let mut password_hashes = self.password_hashes.write().await;

		users.remove(&uuid);
		username_index.remove(&user.username);
		password_hashes.remove(&uuid);

		Ok(())
	}

	/// List all users
	///
	/// # Examples
	///
	/// ```rust
	/// # use reinhardt_auth::PasswordHasher;
	/// # struct MockHasher;
	/// # impl PasswordHasher for MockHasher {
	/// #     fn hash(&self, p: &str) -> Result<String, reinhardt_core::exception::Error> {
	/// #         Ok(format!("hash:{p}"))
	/// #     }
	/// #     fn verify(&self, p: &str, h: &str) -> Result<bool, reinhardt_core::exception::Error> {
	/// #         Ok(h == format!("hash:{p}"))
	/// #     }
	/// # }
	/// use reinhardt_auth::user_management::{UserManager, CreateUserData};
	///
	/// #[tokio::main]
	/// async fn main() {
	///     let mut manager = UserManager::new(MockHasher);
	///
	///     let user_data1 = CreateUserData {
	///         username: "grace".to_string(),
	///         email: "grace@example.com".to_string(),
	///         password: "password123".to_string(),
	///         is_active: true,
	///         is_admin: false,
	///     };
	///
	///     let user_data2 = CreateUserData {
	///         username: "henry".to_string(),
	///         email: "henry@example.com".to_string(),
	///         password: "password123".to_string(),
	///         is_active: true,
	///         is_admin: false,
	///     };
	///
	///     manager.create_user(user_data1).await.unwrap();
	///     manager.create_user(user_data2).await.unwrap();
	///
	///     let users = manager.list_users().await;
	///     assert_eq!(users.len(), 2);
	/// }
	/// ```
	pub async fn list_users(&self) -> Vec<SimpleUser> {
		let users = self.users.read().await;
		users.values().cloned().collect()
	}

	/// Verify user password
	///
	/// # Examples
	///
	/// ```rust
	/// # use reinhardt_auth::PasswordHasher;
	/// # struct MockHasher;
	/// # impl PasswordHasher for MockHasher {
	/// #     fn hash(&self, p: &str) -> Result<String, reinhardt_core::exception::Error> {
	/// #         Ok(format!("hash:{p}"))
	/// #     }
	/// #     fn verify(&self, p: &str, h: &str) -> Result<bool, reinhardt_core::exception::Error> {
	/// #         Ok(h == format!("hash:{p}"))
	/// #     }
	/// # }
	/// use reinhardt_auth::user_management::{UserManager, CreateUserData};
	///
	/// #[tokio::main]
	/// async fn main() {
	///     let mut manager = UserManager::new(MockHasher);
	///
	///     let user_data = CreateUserData {
	///         username: "iris".to_string(),
	///         email: "iris@example.com".to_string(),
	///         password: "mypassword".to_string(),
	///         is_active: true,
	///         is_admin: false,
	///     };
	///
	///     let user = manager.create_user(user_data).await.unwrap();
	///     assert!(manager.verify_password(&user.id.to_string(), "mypassword").await.unwrap());
	///     assert!(!manager.verify_password(&user.id.to_string(), "wrongpassword").await.unwrap());
	/// }
	/// ```
	pub async fn verify_password(
		&self,
		user_id: &str,
		password: &str,
	) -> UserManagementResult<bool> {
		let uuid = Uuid::parse_str(user_id)
			.map_err(|_| UserManagementError::Other("Invalid UUID".to_string()))?;

		let password_hashes = self.password_hashes.read().await;
		let hash = password_hashes
			.get(&uuid)
			.ok_or(UserManagementError::UserNotFound)?;

		self.hasher
			.verify(password, hash)
			.map_err(|e| UserManagementError::Other(e.to_string()))
	}
}

#[cfg(all(test, feature = "argon2-hasher"))]
mod tests {
	use super::*;
	use crate::Argon2Hasher;

	#[tokio::test]
	async fn test_create_user() {
		let hasher = Argon2Hasher::new();
		let mut manager = UserManager::new(hasher);

		let user_data = CreateUserData {
			username: "alice".to_string(),
			email: "alice@example.com".to_string(),
			password: "password123".to_string(),
			is_active: true,
			is_admin: false,
		};

		let user = manager.create_user(user_data).await.unwrap();
		assert_eq!(user.username, "alice");
		assert_eq!(user.email, "alice@example.com");
		assert!(user.is_active);
		assert!(!user.is_admin);
	}

	#[tokio::test]
	async fn test_create_user_duplicate_username() {
		let hasher = Argon2Hasher::new();
		let mut manager = UserManager::new(hasher);

		let user_data1 = CreateUserData {
			username: "bob".to_string(),
			email: "bob@example.com".to_string(),
			password: "password123".to_string(),
			is_active: true,
			is_admin: false,
		};

		let user_data2 = CreateUserData {
			username: "bob".to_string(),
			email: "bob2@example.com".to_string(),
			password: "password456".to_string(),
			is_active: true,
			is_admin: false,
		};

		manager.create_user(user_data1).await.unwrap();
		let result = manager.create_user(user_data2).await;
		assert!(result.is_err());
	}

	#[tokio::test]
	async fn test_get_user() {
		let hasher = Argon2Hasher::new();
		let mut manager = UserManager::new(hasher);

		let user_data = CreateUserData {
			username: "charlie".to_string(),
			email: "charlie@example.com".to_string(),
			password: "password123".to_string(),
			is_active: true,
			is_admin: false,
		};

		let user = manager.create_user(user_data).await.unwrap();
		let retrieved = manager.get_user(&user.id.to_string()).await.unwrap();
		assert_eq!(retrieved.username, "charlie");
	}

	#[tokio::test]
	async fn test_get_user_by_username() {
		let hasher = Argon2Hasher::new();
		let mut manager = UserManager::new(hasher);

		let user_data = CreateUserData {
			username: "diana".to_string(),
			email: "diana@example.com".to_string(),
			password: "password123".to_string(),
			is_active: true,
			is_admin: false,
		};

		manager.create_user(user_data).await.unwrap();
		let retrieved = manager.get_user_by_username("diana").await.unwrap();
		assert_eq!(retrieved.username, "diana");
	}

	#[tokio::test]
	async fn test_update_user() {
		let hasher = Argon2Hasher::new();
		let mut manager = UserManager::new(hasher);

		let user_data = CreateUserData {
			username: "eve".to_string(),
			email: "eve@example.com".to_string(),
			password: "password123".to_string(),
			is_active: true,
			is_admin: false,
		};

		let user = manager.create_user(user_data).await.unwrap();

		let update_data = UpdateUserData {
			email: Some("newemail@example.com".to_string()),
			is_active: Some(false),
			is_admin: Some(true),
		};

		let updated = manager
			.update_user(&user.id.to_string(), update_data)
			.await
			.unwrap();
		assert_eq!(updated.email, "newemail@example.com");
		assert!(!updated.is_active);
		assert!(updated.is_admin);
	}

	#[tokio::test]
	async fn test_delete_user() {
		let hasher = Argon2Hasher::new();
		let mut manager = UserManager::new(hasher);

		let user_data = CreateUserData {
			username: "frank".to_string(),
			email: "frank@example.com".to_string(),
			password: "password123".to_string(),
			is_active: true,
			is_admin: false,
		};

		let user = manager.create_user(user_data).await.unwrap();
		manager.delete_user(&user.id.to_string()).await.unwrap();
		let result = manager.get_user(&user.id.to_string()).await;
		assert!(result.is_err());
	}

	#[tokio::test]
	async fn test_list_users() {
		let hasher = Argon2Hasher::new();
		let mut manager = UserManager::new(hasher);

		let user_data1 = CreateUserData {
			username: "grace".to_string(),
			email: "grace@example.com".to_string(),
			password: "password123".to_string(),
			is_active: true,
			is_admin: false,
		};

		let user_data2 = CreateUserData {
			username: "henry".to_string(),
			email: "henry@example.com".to_string(),
			password: "password123".to_string(),
			is_active: true,
			is_admin: false,
		};

		manager.create_user(user_data1).await.unwrap();
		manager.create_user(user_data2).await.unwrap();

		let users = manager.list_users().await;
		assert_eq!(users.len(), 2);
	}

	#[tokio::test]
	async fn test_verify_password() {
		let hasher = Argon2Hasher::new();
		let mut manager = UserManager::new(hasher);

		let user_data = CreateUserData {
			username: "iris".to_string(),
			email: "iris@example.com".to_string(),
			password: "mypassword".to_string(),
			is_active: true,
			is_admin: false,
		};

		let user = manager.create_user(user_data).await.unwrap();
		assert!(
			manager
				.verify_password(&user.id.to_string(), "mypassword")
				.await
				.unwrap()
		);
		assert!(
			!manager
				.verify_password(&user.id.to_string(), "wrongpassword")
				.await
				.unwrap()
		);
	}
}
