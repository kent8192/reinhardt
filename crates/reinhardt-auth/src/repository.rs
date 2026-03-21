//! User repository abstraction
//!
//! Provides the [`UserRepository`] trait for retrieving user data from storage backends,
//! shared across multiple authentication backends.

use crate::{SimpleUser, User};
use async_trait::async_trait;
use uuid::Uuid;

/// User repository trait for authentication backends
///
/// Provides an abstraction for retrieving user data from various storage backends.
///
/// # Examples
///
/// ```
/// use reinhardt_auth::{UserRepository, User};
/// use async_trait::async_trait;
///
/// struct MyUserRepository;
///
/// #[async_trait]
/// impl UserRepository for MyUserRepository {
///     async fn get_user_by_id(&self, user_id: &str) -> Result<Option<Box<dyn User>>, String> {
///         // Custom implementation
///         Ok(None)
///     }
/// }
/// ```
#[async_trait]
pub trait UserRepository: Send + Sync {
	/// Get user by ID
	///
	/// Returns `Ok(Some(user))` if found, `Ok(None)` if not found, or `Err` on error.
	async fn get_user_by_id(&self, user_id: &str) -> Result<Option<Box<dyn User>>, String>;
}

/// Simple in-memory user repository
///
/// Creates [`SimpleUser`] instances on-the-fly without database access.
/// Suitable for testing and development environments.
///
/// # Examples
///
/// ```
/// use reinhardt_auth::{SimpleUserRepository, UserRepository};
///
/// #[tokio::main]
/// async fn main() {
///     let repo = SimpleUserRepository;
///     let user = repo.get_user_by_id("user_123").await.unwrap();
///     assert!(user.is_some());
/// }
/// ```
pub struct SimpleUserRepository;

#[async_trait]
impl UserRepository for SimpleUserRepository {
	async fn get_user_by_id(&self, user_id: &str) -> Result<Option<Box<dyn User>>, String> {
		// Create a simple user object for development/testing purposes.
		// NOTE: This implementation uses a deterministic UUID and an empty email because
		// real user data is not available without a database connection.
		// For production use, implement UserRepository with a real database backend.
		Ok(Some(Box::new(SimpleUser {
			id: Uuid::new_v5(&Uuid::NAMESPACE_URL, user_id.as_bytes()),
			username: user_id.to_string(),
			email: String::new(),
			is_active: true,
			is_admin: false,
			is_staff: false,
			is_superuser: false,
		})))
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	#[rstest]
	#[tokio::test]
	async fn test_simple_user_repo_returns_user() {
		// Arrange
		let repo = SimpleUserRepository;
		let user_id = "test_user_42";

		// Act
		let result = repo.get_user_by_id(user_id).await;

		// Assert
		let user = result
			.expect("get_user_by_id should not return Err")
			.expect("get_user_by_id should return Some for any input");
		assert_eq!(user.username(), user_id);
		assert!(user.is_active());
		assert!(user.is_authenticated());
	}

	#[rstest]
	#[tokio::test]
	async fn test_simple_user_repo_deterministic_uuid() {
		// Arrange
		let repo = SimpleUserRepository;
		let user_id = "deterministic_id_input";
		let expected_uuid = Uuid::new_v5(&Uuid::NAMESPACE_URL, user_id.as_bytes());

		// Act
		let first_result = repo.get_user_by_id(user_id).await;
		let second_result = repo.get_user_by_id(user_id).await;

		// Assert
		let first_user = first_result.unwrap().unwrap();
		let second_user = second_result.unwrap().unwrap();
		assert_eq!(first_user.id(), expected_uuid.to_string());
		assert_eq!(first_user.id(), second_user.id());
	}
}
