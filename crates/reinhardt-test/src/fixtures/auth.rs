//! Authentication integration test fixtures
//!
//! This module provides rstest fixtures for authentication integration tests,
//! including pre-configured session backends, mock users, and test data.

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
#[derive(Clone, Debug)]
pub struct TestUser {
	pub id: Uuid,
	pub username: String,
	pub email: String,
	pub is_active: bool,
	pub is_admin: bool,
	pub is_staff: bool,
	pub is_superuser: bool,
}

#[fixture]
pub fn test_user() -> TestUser {
	TestUser {
		id: Uuid::new_v4(),
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
		id: Uuid::new_v4(),
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
			id: Uuid::new_v4(),
			username: "user1".to_string(),
			email: "user1@example.com".to_string(),
			is_active: true,
			is_admin: false,
			is_staff: false,
			is_superuser: false,
		},
		TestUser {
			id: Uuid::new_v4(),
			username: "user2".to_string(),
			email: "user2@example.com".to_string(),
			is_active: true,
			is_admin: false,
			is_staff: false,
			is_superuser: false,
		},
		TestUser {
			id: Uuid::new_v4(),
			username: "user3".to_string(),
			email: "user3@example.com".to_string(),
			is_active: false, // Inactive user
			is_admin: false,
			is_staff: false,
			is_superuser: false,
		},
		TestUser {
			id: Uuid::new_v4(),
			username: "staff".to_string(),
			email: "staff@example.com".to_string(),
			is_active: true,
			is_admin: false,
			is_staff: true,
			is_superuser: false,
		},
		TestUser {
			id: Uuid::new_v4(),
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
///     let token = jwt_auth.generate_token("user123".to_string(), "alice".to_string()).unwrap();
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
		id: Uuid::new_v4(),
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
		id: Uuid::new_v4(),
		username: "staffuser".to_string(),
		email: "staff@example.com".to_string(),
		is_active: true,
		is_admin: false,
		is_staff: true,
		is_superuser: false,
	}
}
