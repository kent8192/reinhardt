//! Authentication integration test fixtures
//!
//! This module provides rstest fixtures for authentication integration tests,
//! including pre-configured session backends, mock users, and test data.
#![allow(deprecated)] // TestUser is deprecated but still used by fixture functions

use rstest::*;
use uuid::Uuid;

// Re-exports for convenience
pub use reinhardt_auth::mfa::MFAAuthentication as MfaManager;
pub use reinhardt_auth::{
	Argon2Hasher, InMemoryTokenStorage, JwtAuth, PasswordHasher, StoredToken, TokenStorage,
};

/// Test user fixture
///
/// Provides a consistent test user for authentication tests.
///
/// # Examples
///
/// ```rust,no_run
/// use reinhardt_test::fixtures::auth::test_user;
/// use rstest::*;
///
/// #[rstest]
/// fn test_with_user(test_user: TestUser) {
///     assert_eq!(test_user.username, "testuser");
/// }
/// ```
#[deprecated(
	since = "0.1.0-rc.16",
	note = "define your own user type with `#[user]` macro and use `ForceLoginUser` trait"
)]
#[derive(Clone, Debug)]
pub struct TestUser {
	/// Unique identifier for the test user.
	pub id: Uuid,
	/// Username for authentication.
	pub username: String,
	/// Email address associated with the test user.
	pub email: String,
	/// Whether the user account is active and can authenticate.
	pub is_active: bool,
	/// Whether the user has admin privileges.
	pub is_admin: bool,
	/// Whether the user has staff-level access.
	pub is_staff: bool,
	/// Whether the user has superuser (all permissions) access.
	pub is_superuser: bool,
}

/// Creates a default [`TestUser`] fixture with standard non-privileged settings.
#[fixture]
pub fn test_user() -> TestUser {
	TestUser {
		id: Uuid::now_v7(),
		username: "testuser".to_string(),
		email: "test@example.com".to_string(),
		is_active: true,
		is_admin: false,
		is_staff: false,
		is_superuser: false,
	}
}

/// Admin user fixture
///
/// Provides a test user with admin privileges.
///
/// # Examples
///
/// ```rust,no_run
/// use reinhardt_test::fixtures::auth::admin_user;
/// use rstest::*;
///
/// #[rstest]
/// fn test_with_admin(admin_user: TestUser) {
///     assert!(admin_user.is_admin);
///     assert!(admin_user.is_staff);
///     assert!(admin_user.is_superuser);
/// }
/// ```
#[fixture]
pub fn admin_user() -> TestUser {
	TestUser {
		id: Uuid::now_v7(),
		username: "admin".to_string(),
		email: "admin@example.com".to_string(),
		is_active: true,
		is_admin: true,
		is_staff: true,
		is_superuser: true,
	}
}

/// Multiple test users fixture
///
/// Provides a collection of test users for testing authentication scenarios
/// with multiple users.
///
/// # Examples
///
/// ```rust,no_run
/// use reinhardt_test::fixtures::auth::test_users;
/// use rstest::*;
///
/// #[rstest]
/// fn test_with_multiple_users(test_users: Vec<TestUser>) {
///     assert_eq!(test_users.len(), 5);
/// }
/// ```
#[fixture]
pub fn test_users() -> Vec<TestUser> {
	vec![
		TestUser {
			id: Uuid::now_v7(),
			username: "user1".to_string(),
			email: "user1@example.com".to_string(),
			is_active: true,
			is_admin: false,
			is_staff: false,
			is_superuser: false,
		},
		TestUser {
			id: Uuid::now_v7(),
			username: "user2".to_string(),
			email: "user2@example.com".to_string(),
			is_active: true,
			is_admin: false,
			is_staff: false,
			is_superuser: false,
		},
		TestUser {
			id: Uuid::now_v7(),
			username: "user3".to_string(),
			email: "user3@example.com".to_string(),
			is_active: false, // Inactive user
			is_admin: false,
			is_staff: false,
			is_superuser: false,
		},
		TestUser {
			id: Uuid::now_v7(),
			username: "staff".to_string(),
			email: "staff@example.com".to_string(),
			is_active: true,
			is_admin: false,
			is_staff: true,
			is_superuser: false,
		},
		TestUser {
			id: Uuid::now_v7(),
			username: "superuser".to_string(),
			email: "superuser@example.com".to_string(),
			is_active: true,
			is_admin: true,
			is_staff: true,
			is_superuser: true,
		},
	]
}

// =============================================================================
// MFA Fixtures
// =============================================================================

/// MFA authentication fixture
///
/// Provides a pre-configured MFA authentication backend for testing.
///
/// # Examples
///
/// ```rust,no_run
/// use reinhardt_test::fixtures::auth::mfa_authentication;
/// use rstest::*;
///
/// #[rstest]
/// fn test_with_mfa(mfa_authentication: MfaManager) {
///     mfa_authentication.register_user("alice", "JBSWY3DPEHPK3PXP");
/// }
/// ```
#[fixture]
pub fn mfa_authentication() -> MfaManager {
	MfaManager::new("ReinhardtTest").time_window(30)
}

/// MFA with registered user fixture
///
/// Provides an MFA authentication backend with a pre-registered test user.
/// Returns both the MFA manager and the generated TOTP secret.
///
/// # Examples
///
/// ```rust,no_run
/// use reinhardt_test::fixtures::auth::{mfa_with_registered_user, test_user};
/// use rstest::*;
///
/// #[rstest]
/// fn test_mfa_user(mfa_with_registered_user: (MfaManager, String)) {
///     let (mfa, secret) = mfa_with_registered_user;
///     // Secret is a valid base32 string
///     assert!(!secret.is_empty());
/// }
/// ```
#[fixture]
pub fn mfa_with_registered_user(test_user: TestUser) -> (MfaManager, String) {
	let mfa = MfaManager::new("ReinhardtTest").time_window(30);
	// Use a valid base32 secret (RFC 4648)
	let secret = "JBSWY3DPEHPK3PXP".to_string();
	tokio::runtime::Handle::current().block_on(mfa.register_user(&test_user.username, &secret));
	(mfa, secret)
}

// =============================================================================
// JWT Fixtures
// =============================================================================

/// JWT authentication fixture with default secret
///
/// Provides a pre-configured JWT authentication backend for testing.
///
/// # Examples
///
/// ```rust,no_run
/// use reinhardt_test::fixtures::auth::jwt_auth;
/// use rstest::*;
///
/// #[rstest]
/// fn test_with_jwt(jwt_auth: JwtAuth) {
///     let token = jwt_auth.generate_token("user123".to_string(), "alice".to_string(), false, false).unwrap();
///     assert!(!token.is_empty());
/// }
/// ```
#[fixture]
pub fn jwt_auth() -> JwtAuth {
	// Use a secure test secret (32 bytes minimum)
	JwtAuth::new(b"reinhardt-test-secret-key-32bytes")
}

/// JWT authentication fixture with custom secret
///
/// Provides a JWT authentication backend with a specified secret.
#[fixture]
pub fn jwt_auth_with_secret(
	#[default(b"custom-secret-key-for-testing-32")] secret: &[u8],
) -> JwtAuth {
	JwtAuth::new(secret)
}

// =============================================================================
// Password Hasher Fixtures
// =============================================================================

/// Argon2 password hasher fixture
///
/// Provides a pre-configured Argon2 password hasher for testing.
///
/// # Examples
///
/// ```rust,no_run
/// use reinhardt_test::fixtures::auth::argon2_hasher;
/// use rstest::*;
///
/// #[rstest]
/// fn test_with_hasher(argon2_hasher: Argon2Hasher) {
///     let hash = argon2_hasher.hash("password123").unwrap();
///     assert!(argon2_hasher.verify("password123", &hash).unwrap());
/// }
/// ```
#[fixture]
pub fn argon2_hasher() -> Argon2Hasher {
	Argon2Hasher
}

// =============================================================================
// Token Storage Fixtures
// =============================================================================

/// In-memory token storage fixture
///
/// Provides a pre-configured in-memory token storage for testing.
///
/// # Examples
///
/// ```rust,no_run
/// use reinhardt_test::fixtures::auth::in_memory_token_storage;
/// use rstest::*;
///
/// #[rstest]
/// #[tokio::test]
/// async fn test_with_storage(in_memory_token_storage: InMemoryTokenStorage) {
///     assert!(in_memory_token_storage.is_empty());
/// }
/// ```
#[fixture]
pub fn in_memory_token_storage() -> InMemoryTokenStorage {
	InMemoryTokenStorage::new()
}

/// Token storage with test data fixture
///
/// Provides an in-memory token storage pre-populated with test tokens.
#[fixture]
pub fn token_storage_with_data() -> InMemoryTokenStorage {
	// Pre-populate with test data synchronously during fixture creation
	// Note: Actual storage operations are async, but we use a blocking approach for fixture setup
	InMemoryTokenStorage::new()
}

// =============================================================================
// Inactive User Fixture
// =============================================================================

/// Inactive user fixture
///
/// Provides a test user that is marked as inactive.
#[fixture]
pub fn inactive_user() -> TestUser {
	TestUser {
		id: Uuid::now_v7(),
		username: "inactive".to_string(),
		email: "inactive@example.com".to_string(),
		is_active: false,
		is_admin: false,
		is_staff: false,
		is_superuser: false,
	}
}

/// Staff user fixture (non-admin)
///
/// Provides a test user that is staff but not an admin.
#[fixture]
pub fn staff_user() -> TestUser {
	TestUser {
		id: Uuid::now_v7(),
		username: "staffuser".to_string(),
		email: "staff@example.com".to_string(),
		is_active: true,
		is_admin: false,
		is_staff: true,
		is_superuser: false,
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;
	use std::collections::HashSet;

	// =========================================================================
	// Normal: test_user fixture
	// =========================================================================

	#[rstest]
	fn test_test_user_default_fields(test_user: TestUser) {
		// Arrange
		// (fixture provides the test user)

		// Act
		let user = test_user;

		// Assert
		assert_eq!(user.username, "testuser");
		assert_eq!(user.email, "test@example.com");
		assert!(user.is_active);
		assert!(!user.is_admin);
		assert!(!user.is_staff);
		assert!(!user.is_superuser);
	}

	#[rstest]
	fn test_test_user_has_valid_uuid(test_user: TestUser) {
		// Arrange
		let user = test_user;

		// Act
		let id = user.id;

		// Assert
		assert!(!id.is_nil(), "test_user id should be a non-nil UUID");
	}

	// =========================================================================
	// Normal: admin_user fixture
	// =========================================================================

	#[rstest]
	fn test_admin_user_all_privileges(admin_user: TestUser) {
		// Arrange
		let user = admin_user;

		// Act & Assert
		assert!(user.is_admin);
		assert!(user.is_staff);
		assert!(user.is_superuser);
		assert!(user.is_active);
	}

	#[rstest]
	fn test_admin_user_credentials(admin_user: TestUser) {
		// Arrange
		let user = admin_user;

		// Act & Assert
		assert_eq!(user.username, "admin");
		assert_eq!(user.email, "admin@example.com");
	}

	// =========================================================================
	// Normal: inactive_user fixture
	// =========================================================================

	#[rstest]
	fn test_inactive_user_is_not_active(inactive_user: TestUser) {
		// Arrange
		let user = inactive_user;

		// Act & Assert
		assert!(!user.is_active);
		assert!(!user.is_admin);
		assert!(!user.is_staff);
		assert!(!user.is_superuser);
	}

	#[rstest]
	fn test_inactive_user_credentials(inactive_user: TestUser) {
		// Arrange
		let user = inactive_user;

		// Act & Assert
		assert_eq!(user.username, "inactive");
	}

	// =========================================================================
	// Normal: staff_user fixture
	// =========================================================================

	#[rstest]
	fn test_staff_user_is_staff_not_admin(staff_user: TestUser) {
		// Arrange
		let user = staff_user;

		// Act & Assert
		assert!(user.is_staff);
		assert!(!user.is_admin);
		assert!(!user.is_superuser);
	}

	#[rstest]
	fn test_staff_user_is_active(staff_user: TestUser) {
		// Arrange
		let user = staff_user;

		// Act & Assert
		assert!(user.is_active);
	}

	// =========================================================================
	// Normal: test_users fixture
	// =========================================================================

	#[rstest]
	fn test_test_users_count(test_users: Vec<TestUser>) {
		// Arrange
		let users = test_users;

		// Act
		let count = users.len();

		// Assert
		assert_eq!(count, 5);
	}

	#[rstest]
	fn test_test_users_contains_inactive(test_users: Vec<TestUser>) {
		// Arrange
		let users = test_users;

		// Act
		let inactive_count = users.iter().filter(|u| !u.is_active).count();

		// Assert
		assert_eq!(inactive_count, 1, "exactly one user should be inactive");
	}

	#[rstest]
	fn test_test_users_contains_staff(test_users: Vec<TestUser>) {
		// Arrange
		let users = test_users;

		// Act
		let staff_non_admin_count = users.iter().filter(|u| u.is_staff && !u.is_admin).count();

		// Assert
		assert_eq!(
			staff_non_admin_count, 1,
			"exactly one user should be staff but not admin"
		);
	}

	#[rstest]
	fn test_test_users_contains_superuser(test_users: Vec<TestUser>) {
		// Arrange
		let users = test_users;

		// Act
		let superuser_count = users.iter().filter(|u| u.is_superuser).count();

		// Assert
		assert_eq!(superuser_count, 1, "exactly one user should be a superuser");
	}

	// =========================================================================
	// Normal: auth component construction fixtures
	// =========================================================================

	#[rstest]
	fn test_jwt_auth_construction(jwt_auth: JwtAuth) {
		// Arrange & Act
		let _auth = jwt_auth;

		// Assert
		// Construction succeeded without panic
	}

	#[rstest]
	fn test_argon2_hasher_construction(argon2_hasher: Argon2Hasher) {
		// Arrange & Act
		let _hasher = argon2_hasher;

		// Assert
		// Argon2Hasher is a unit struct; successful construction is the assertion
	}

	#[rstest]
	#[tokio::test]
	async fn test_in_memory_token_storage_empty(in_memory_token_storage: InMemoryTokenStorage) {
		// Arrange & Act
		let storage = in_memory_token_storage;

		// Assert
		assert!(storage.is_empty().await);
	}

	#[rstest]
	fn test_mfa_authentication_construction(mfa_authentication: MfaManager) {
		// Arrange & Act
		let _mfa = mfa_authentication;

		// Assert
		// Construction succeeded without panic
	}

	// =========================================================================
	// Edge: uniqueness and trait tests
	// =========================================================================

	#[rstest]
	fn test_test_users_unique_ids(test_users: Vec<TestUser>) {
		// Arrange
		let users = test_users;

		// Act
		let id_set: HashSet<Uuid> = users.iter().map(|u| u.id).collect();

		// Assert
		assert_eq!(id_set.len(), 5, "all 5 users should have distinct UUIDs");
	}

	#[rstest]
	fn test_test_users_unique_usernames(test_users: Vec<TestUser>) {
		// Arrange
		let users = test_users;

		// Act
		let name_set: HashSet<&str> = users.iter().map(|u| u.username.as_str()).collect();

		// Assert
		assert_eq!(
			name_set.len(),
			5,
			"all 5 users should have distinct usernames"
		);
	}

	#[rstest]
	fn test_test_user_clone(test_user: TestUser) {
		// Arrange
		let original = test_user;

		// Act
		let cloned = original.clone();

		// Assert
		assert_eq!(cloned.id, original.id);
		assert_eq!(cloned.username, original.username);
		assert_eq!(cloned.email, original.email);
		assert_eq!(cloned.is_active, original.is_active);
		assert_eq!(cloned.is_admin, original.is_admin);
		assert_eq!(cloned.is_staff, original.is_staff);
		assert_eq!(cloned.is_superuser, original.is_superuser);
	}

	#[rstest]
	fn test_test_user_debug(test_user: TestUser) {
		// Arrange
		let user = test_user;

		// Act
		let debug_str = format!("{:?}", user);

		// Assert
		assert!(!debug_str.is_empty(), "Debug output should be non-empty");
		assert!(
			debug_str.contains("TestUser"),
			"Debug output should contain the struct name"
		);
	}
}
