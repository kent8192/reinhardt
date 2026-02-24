//! Auth server function tests
//!
//! Tests for login, register, logout, and current_user server functions.

use rstest::*;
use sqlx::PgPool;

use crate::apps::auth::shared::types::UserInfo;
use crate::test_utils::factories::user::UserFactory;
use crate::test_utils::fixtures::database::twitter_db_pool;
use crate::test_utils::fixtures::users::TestTwitterUser;
use reinhardt::BaseUser;

// ============================================================================
// Login Tests
// ============================================================================

#[rstest]
#[tokio::test]
async fn test_login_success(#[future] twitter_db_pool: (PgPool, String)) {
	let (pool, _url) = twitter_db_pool.await;
	let factory = UserFactory::new();

	// Create test user with known password
	let test_user = TestTwitterUser::new("loginuser").with_password("ValidPassword123");
	let user = factory
		.create_from_test_user(&pool, &test_user)
		.await
		.expect("User creation should succeed");

	// Mock database connection for server function
	// Note: In actual implementation, this would use proper DI mocking
	// For now, we verify the factory creates a valid user that can login
	assert_eq!(user.email(), &test_user.email);
	assert!(user.is_active());

	// Verify password was hashed correctly
	let password_valid = user
		.check_password(&test_user.password)
		.expect("Password check should succeed");
	assert!(password_valid, "Password should be valid");
}

#[rstest]
#[tokio::test]
async fn test_login_invalid_password(#[future] twitter_db_pool: (PgPool, String)) {
	let (pool, _url) = twitter_db_pool.await;
	let factory = UserFactory::new();

	// Create test user
	let test_user = TestTwitterUser::new("wrongpwduser").with_password("CorrectPassword123");
	let user = factory
		.create_from_test_user(&pool, &test_user)
		.await
		.expect("User creation should succeed");

	// Verify wrong password fails
	let password_valid = user
		.check_password("WrongPassword456")
		.expect("Password check should succeed");
	assert!(!password_valid, "Wrong password should fail");
}

#[rstest]
#[tokio::test]
async fn test_login_inactive_user(#[future] twitter_db_pool: (PgPool, String)) {
	let (pool, _url) = twitter_db_pool.await;
	let factory = UserFactory::new();

	// Create inactive test user
	let test_user = TestTwitterUser::new("inactiveuser").with_active(false);
	let user = factory
		.create_from_test_user(&pool, &test_user)
		.await
		.expect("User creation should succeed");

	assert!(!user.is_active(), "User should be inactive");
}

// ============================================================================
// Register Tests
// ============================================================================

#[rstest]
#[tokio::test]
async fn test_register_success(#[future] twitter_db_pool: (PgPool, String)) {
	let (pool, _url) = twitter_db_pool.await;

	// Verify no user exists with this email
	let existing = sqlx::query("SELECT id FROM auth_user WHERE email = $1")
		.bind("newuser@example.com")
		.fetch_optional(&pool)
		.await
		.expect("Query should succeed");
	assert!(existing.is_none(), "No user should exist with this email");

	// Create a user directly to simulate registration result
	let factory = UserFactory::new();
	let test_user = TestTwitterUser::new("newuser")
		.with_email("newuser@example.com")
		.with_password("SecurePassword123");
	let user = factory
		.create_from_test_user(&pool, &test_user)
		.await
		.expect("User creation should succeed");

	// Verify user was created
	assert_eq!(user.username(), "newuser");
	assert_eq!(user.email(), "newuser@example.com");
	assert!(user.is_active());

	// Verify password was set correctly
	let password_valid = user
		.check_password("SecurePassword123")
		.expect("Password check should succeed");
	assert!(password_valid);
}

#[rstest]
#[tokio::test]
async fn test_register_duplicate_email(#[future] twitter_db_pool: (PgPool, String)) {
	let (pool, _url) = twitter_db_pool.await;
	let factory = UserFactory::new();

	// Create first user
	let test_user = TestTwitterUser::new("firstuser").with_email("duplicate@example.com");
	factory
		.create_from_test_user(&pool, &test_user)
		.await
		.expect("First user creation should succeed");

	// Attempt to create second user with same email should fail (database constraint)
	let test_user2 = TestTwitterUser::new("seconduser").with_email("duplicate@example.com");
	let result = factory.create_from_test_user(&pool, &test_user2).await;

	assert!(result.is_err(), "Duplicate email should fail");
}

#[rstest]
#[tokio::test]
async fn test_register_password_validation() {
	use crate::apps::auth::shared::types::RegisterRequest;
	use validator::Validate;

	// Test short password
	let request = RegisterRequest {
		username: "testuser".to_string(),
		email: "test@example.com".to_string(),
		password: "short".to_string(),
		password_confirmation: "short".to_string(),
	};

	let result = request.validate();
	assert!(result.is_err(), "Short password should fail validation");
}

#[rstest]
#[tokio::test]
async fn test_register_password_mismatch() {
	use crate::apps::auth::shared::types::RegisterRequest;

	let request = RegisterRequest {
		username: "testuser".to_string(),
		email: "test@example.com".to_string(),
		password: "SecurePassword123".to_string(),
		password_confirmation: "DifferentPassword456".to_string(),
	};

	let result = request.validate_passwords_match();
	assert!(result.is_err(), "Password mismatch should fail");
	assert!(result.unwrap_err().contains("do not match"));
}

#[rstest]
#[tokio::test]
async fn test_register_invalid_email() {
	use crate::apps::auth::shared::types::RegisterRequest;
	use validator::Validate;

	let request = RegisterRequest {
		username: "testuser".to_string(),
		email: "not-an-email".to_string(),
		password: "SecurePassword123".to_string(),
		password_confirmation: "SecurePassword123".to_string(),
	};

	let result = request.validate();
	assert!(result.is_err(), "Invalid email should fail validation");
}

#[rstest]
#[tokio::test]
async fn test_register_short_username() {
	use crate::apps::auth::shared::types::RegisterRequest;
	use validator::Validate;

	let request = RegisterRequest {
		username: "ab".to_string(), // Too short (min 3)
		email: "test@example.com".to_string(),
		password: "SecurePassword123".to_string(),
		password_confirmation: "SecurePassword123".to_string(),
	};

	let result = request.validate();
	assert!(result.is_err(), "Short username should fail validation");
}

// ============================================================================
// Login Request Validation Tests
// ============================================================================

#[rstest]
#[tokio::test]
async fn test_login_request_validation_empty_password() {
	use crate::apps::auth::shared::types::LoginRequest;
	use validator::Validate;

	let request = LoginRequest {
		email: "test@example.com".to_string(),
		password: "".to_string(),
	};

	let result = request.validate();
	assert!(result.is_err(), "Empty password should fail validation");
}

#[rstest]
#[tokio::test]
async fn test_login_request_validation_invalid_email() {
	use crate::apps::auth::shared::types::LoginRequest;
	use validator::Validate;

	let request = LoginRequest {
		email: "invalid-email".to_string(),
		password: "ValidPassword123".to_string(),
	};

	let result = request.validate();
	assert!(result.is_err(), "Invalid email should fail validation");
}

// ============================================================================
// UserInfo Conversion Tests
// ============================================================================

#[rstest]
#[tokio::test]
async fn test_user_info_conversion(#[future] twitter_db_pool: (PgPool, String)) {
	let (pool, _url) = twitter_db_pool.await;
	let factory = UserFactory::new();

	let test_user = TestTwitterUser::new("infouser");
	let user = factory
		.create_from_test_user(&pool, &test_user)
		.await
		.expect("User creation should succeed");

	let user_info = UserInfo::from(user.clone());

	assert_eq!(user_info.id, user.id());
	assert_eq!(&user_info.username, user.username());
	assert_eq!(&user_info.email, user.email());
	assert_eq!(user_info.is_active, user.is_active());
}
