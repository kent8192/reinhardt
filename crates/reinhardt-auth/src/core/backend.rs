// This module uses the deprecated User trait for backward compatibility.
// AuthBackend and CompositeAuthBackend are keyed on User to preserve existing APIs
// until a full migration to AuthIdentity is complete.
#![allow(deprecated)]
use async_trait::async_trait;

use crate::core::user::User;

/// Authentication backend trait
///
/// Implement this trait to create custom authentication backends.
/// A backend handles user authentication (login) and user retrieval.
///
/// # Examples
///
/// ```
/// use reinhardt_auth::{AuthBackend, User, SimpleUser, PasswordHasher};
/// #[cfg(feature = "argon2-hasher")]
/// use reinhardt_auth::Argon2Hasher;
/// use async_trait::async_trait;
/// use std::collections::HashMap;
/// use uuid::Uuid;
///
/// # #[cfg(feature = "argon2-hasher")]
/// # {
/// struct InMemoryAuthBackend {
///     users: HashMap<String, (String, SimpleUser)>, // username -> (password_hash, user)
///     hasher: Argon2Hasher,
/// }
///
/// impl InMemoryAuthBackend {
///     fn new() -> Self {
///         let mut users = HashMap::new();
///         let hasher = Argon2Hasher::new();
///
///         let user = SimpleUser {
///             id: Uuid::now_v7(),
///             username: "alice".to_string(),
///             email: "alice@example.com".to_string(),
///             is_active: true,
///             is_admin: false,
///             is_staff: false,
///             is_superuser: false,
///         };
///         let hash = hasher.hash("password123").unwrap();
///         users.insert("alice".to_string(), (hash, user));
///
///         Self { users, hasher }
///     }
/// }
///
/// #[async_trait]
/// impl AuthBackend for InMemoryAuthBackend {
///     type User = SimpleUser;
///
///     async fn authenticate(&self, username: &str, password: &str)
///         -> Result<Option<Self::User>, reinhardt_core::exception::Error> {
///         if let Some((hash, user)) = self.users.get(username) {
///             if self.hasher.verify(password, hash)? {
///                 return Ok(Some(user.clone()));
///             }
///         }
///         Ok(None)
///     }
///
///     async fn get_user(&self, user_id: &str)
///         -> Result<Option<Self::User>, reinhardt_core::exception::Error> {
///         Ok(self.users.values()
///             .find(|(_, u)| u.id.to_string() == user_id)
///             .map(|(_, u)| u.clone()))
///     }
/// }
/// # }
/// ```
#[async_trait]
pub trait AuthBackend: Send + Sync {
	/// The user type this backend works with
	type User: User;

	/// Authenticates a user with username and password
	///
	/// Returns `Ok(Some(user))` if authentication succeeds,
	/// `Ok(None)` if credentials are invalid,
	/// `Err(_)` if an error occurs.
	///
	/// # Arguments
	///
	/// * `username` - The username or email to authenticate
	/// * `password` - The plaintext password to verify
	async fn authenticate(
		&self,
		username: &str,
		password: &str,
	) -> Result<Option<Self::User>, reinhardt_core::exception::Error>;

	/// Retrieves a user by their ID
	///
	/// Returns `Ok(Some(user))` if found, `Ok(None)` if not found,
	/// `Err(_)` if an error occurs.
	///
	/// # Arguments
	///
	/// * `user_id` - The user's unique identifier as a string
	async fn get_user(
		&self,
		user_id: &str,
	) -> Result<Option<Self::User>, reinhardt_core::exception::Error>;
}

/// Composite auth backend - tries multiple backends in order
///
/// This backend allows you to configure multiple authentication backends
/// and try them in sequence until one succeeds.
///
/// # Examples
///
/// ```
/// use reinhardt_auth::{CompositeAuthBackend, SimpleUser};
///
/// let backend: CompositeAuthBackend<SimpleUser> = CompositeAuthBackend::new();
/// // Add custom backends with backend.add_backend(Box::new(my_backend))
/// ```
pub struct CompositeAuthBackend<U: User> {
	backends: Vec<Box<dyn AuthBackend<User = U>>>,
}

impl<U: User> CompositeAuthBackend<U> {
	/// Creates a new composite authentication backend with no backends
	pub fn new() -> Self {
		Self {
			backends: Vec::new(),
		}
	}

	/// Adds an authentication backend to the composite
	///
	/// Backends are tried in the order they are added.
	pub fn add_backend(&mut self, backend: Box<dyn AuthBackend<User = U>>) {
		self.backends.push(backend);
	}
}

impl<U: User> Default for CompositeAuthBackend<U> {
	fn default() -> Self {
		Self::new()
	}
}

#[async_trait]
impl<U: User + 'static> AuthBackend for CompositeAuthBackend<U> {
	type User = U;

	async fn authenticate(
		&self,
		username: &str,
		password: &str,
	) -> Result<Option<Self::User>, reinhardt_core::exception::Error> {
		for backend in &self.backends {
			if let Some(user) = backend.authenticate(username, password).await? {
				return Ok(Some(user));
			}
		}
		Ok(None)
	}

	async fn get_user(
		&self,
		user_id: &str,
	) -> Result<Option<Self::User>, reinhardt_core::exception::Error> {
		for backend in &self.backends {
			if let Some(user) = backend.get_user(user_id).await? {
				return Ok(Some(user));
			}
		}
		Ok(None)
	}
}
