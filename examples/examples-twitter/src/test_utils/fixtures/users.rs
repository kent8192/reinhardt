//! User fixtures for examples-twitter tests.
//!
//! Provides pre-configured test users and authenticated sessions.

use rstest::*;
use uuid::Uuid;

use crate::apps::auth::shared::types::SessionData;

/// Test user data for fixture creation.
///
/// Contains all fields needed to create a test user, with sensible defaults.
#[derive(Clone, Debug)]
pub struct TestTwitterUser {
	pub id: Uuid,
	pub username: String,
	pub email: String,
	pub password: String,
	pub is_active: bool,
	pub bio: Option<String>,
}

impl TestTwitterUser {
	/// Create a new test user with the given username.
	pub fn new(username: &str) -> Self {
		let id = Uuid::new_v4();
		Self {
			id,
			username: username.to_string(),
			email: format!("{}@example.com", username),
			password: "password123".to_string(),
			is_active: true,
			bio: None,
		}
	}

	/// Create a test user with a specific ID.
	pub fn with_id(mut self, id: Uuid) -> Self {
		self.id = id;
		self
	}

	/// Set the user's email.
	pub fn with_email(mut self, email: &str) -> Self {
		self.email = email.to_string();
		self
	}

	/// Set the user's password.
	pub fn with_password(mut self, password: &str) -> Self {
		self.password = password.to_string();
		self
	}

	/// Set the user's active status.
	pub fn with_active(mut self, is_active: bool) -> Self {
		self.is_active = is_active;
		self
	}

	/// Set the user's bio.
	pub fn with_bio(mut self, bio: &str) -> Self {
		self.bio = Some(bio.to_string());
		self
	}

	/// Convert to SessionData for authenticated requests.
	pub fn to_session_data(&self) -> SessionData {
		SessionData {
			user_id: self.id,
			username: self.username.clone(),
			email: self.email.clone(),
		}
	}
}

impl Default for TestTwitterUser {
	fn default() -> Self {
		Self::new("testuser")
	}
}

/// Fixture providing a standard test user.
///
/// # Example
///
/// ```rust,ignore
/// use examples_twitter::test_utils::fixtures::twitter_user;
/// use rstest::*;
///
/// #[rstest]
/// fn test_with_user(twitter_user: TestTwitterUser) {
///     assert_eq!(twitter_user.username, "testuser");
///     assert!(twitter_user.is_active);
/// }
/// ```
#[fixture]
pub fn twitter_user() -> TestTwitterUser {
	TestTwitterUser::default()
}

/// Fixture providing a second test user for multi-user scenarios.
///
/// # Example
///
/// ```rust,ignore
/// use examples_twitter::test_utils::fixtures::{twitter_user, twitter_user_2};
/// use rstest::*;
///
/// #[rstest]
/// fn test_two_users(twitter_user: TestTwitterUser, twitter_user_2: TestTwitterUser) {
///     assert_ne!(twitter_user.id, twitter_user_2.id);
/// }
/// ```
#[fixture]
pub fn twitter_user_2() -> TestTwitterUser {
	TestTwitterUser::new("testuser2")
}

/// Fixture providing an inactive test user.
///
/// # Example
///
/// ```rust,ignore
/// use examples_twitter::test_utils::fixtures::inactive_twitter_user;
/// use rstest::*;
///
/// #[rstest]
/// fn test_inactive(inactive_twitter_user: TestTwitterUser) {
///     assert!(!inactive_twitter_user.is_active);
/// }
/// ```
#[fixture]
pub fn inactive_twitter_user() -> TestTwitterUser {
	TestTwitterUser::new("inactive").with_active(false)
}

/// Fixture providing an authenticated session for the default test user.
///
/// # Example
///
/// ```rust,ignore
/// use examples_twitter::test_utils::fixtures::authenticated_session;
/// use rstest::*;
///
/// #[rstest]
/// fn test_authenticated(authenticated_session: SessionData) {
///     assert_eq!(authenticated_session.username, "testuser");
/// }
/// ```
#[fixture]
pub fn authenticated_session(twitter_user: TestTwitterUser) -> SessionData {
	twitter_user.to_session_data()
}

/// Fixture providing multiple test users for list/relationship tests.
///
/// Returns 5 users: user1, user2, user3 (inactive), staff, admin
///
/// # Example
///
/// ```rust,ignore
/// use examples_twitter::test_utils::fixtures::twitter_users;
/// use rstest::*;
///
/// #[rstest]
/// fn test_multiple_users(twitter_users: Vec<TestTwitterUser>) {
///     assert_eq!(twitter_users.len(), 5);
/// }
/// ```
#[fixture]
pub fn twitter_users() -> Vec<TestTwitterUser> {
	vec![
		TestTwitterUser::new("user1"),
		TestTwitterUser::new("user2"),
		TestTwitterUser::new("user3").with_active(false),
		TestTwitterUser::new("staff").with_bio("Staff member"),
		TestTwitterUser::new("admin").with_bio("Administrator"),
	]
}

#[cfg(test)]
mod tests {
	use super::*;

	#[rstest]
	fn test_twitter_user_fixture(twitter_user: TestTwitterUser) {
		assert_eq!(twitter_user.username, "testuser");
		assert_eq!(twitter_user.email, "testuser@example.com");
		assert!(twitter_user.is_active);
	}

	#[rstest]
	fn test_twitter_user_2_fixture(twitter_user_2: TestTwitterUser) {
		assert_eq!(twitter_user_2.username, "testuser2");
		assert_eq!(twitter_user_2.email, "testuser2@example.com");
	}

	#[rstest]
	fn test_inactive_twitter_user_fixture(inactive_twitter_user: TestTwitterUser) {
		assert!(!inactive_twitter_user.is_active);
	}

	#[rstest]
	fn test_authenticated_session_fixture(authenticated_session: SessionData) {
		assert_eq!(authenticated_session.username, "testuser");
		assert_eq!(authenticated_session.email, "testuser@example.com");
	}

	#[rstest]
	fn test_twitter_users_fixture(twitter_users: Vec<TestTwitterUser>) {
		assert_eq!(twitter_users.len(), 5);
		assert!(!twitter_users[2].is_active); // user3 is inactive
	}

	#[rstest]
	fn test_to_session_data() {
		let user = TestTwitterUser::new("alice").with_email("alice@test.com");
		let session = user.to_session_data();

		assert_eq!(session.user_id, user.id);
		assert_eq!(session.username, "alice");
		assert_eq!(session.email, "alice@test.com");
	}
}
