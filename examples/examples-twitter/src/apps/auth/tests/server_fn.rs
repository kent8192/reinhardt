//! Auth server function integration tests
//!
//! Tests that call actual server_fn functions (login, current_user, logout)
//! with dependency injection dependencies provided directly.
//! Verifies the end-to-end authentication flow including session persistence.

use rstest::*;
use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use crate::apps::auth::server::server_fn::{current_user, login, logout};
use crate::apps::auth::shared::types::UserInfo;
use crate::test_utils::factories::user::UserFactory;
use crate::test_utils::fixtures::database::twitter_db_pool;
use crate::test_utils::fixtures::users::TestTwitterUser;
use reinhardt::BaseUser;
use reinhardt::DatabaseConnection;
use reinhardt::db::orm::reinitialize_database;
use reinhardt::middleware::session::{SessionData, SessionStore, SessionStoreRef};

// ============================================================================
// Helper: Create a fresh SessionData for testing
// ============================================================================

/// Create a new empty session with a 1-hour TTL.
fn new_test_session() -> SessionData {
	let now = SystemTime::now();
	SessionData {
		id: uuid::Uuid::new_v4().to_string(),
		data: HashMap::new(),
		created_at: now,
		last_accessed: now,
		expires_at: now + Duration::from_secs(3600),
	}
}

// ============================================================================
// Login Server Function Tests
// ============================================================================

#[rstest]
#[tokio::test]
async fn test_login_server_fn_success(#[future] twitter_db_pool: (PgPool, String)) {
	// Arrange
	let (pool, url) = twitter_db_pool.await;
	reinitialize_database(&url)
		.await
		.expect("Database initialization should succeed");

	let factory = UserFactory::new();
	let test_user = TestTwitterUser::new("loginfnuser").with_password("ValidPassword123");
	let user = factory
		.create_from_test_user(&pool, &test_user)
		.await
		.expect("User creation should succeed");

	let db = DatabaseConnection::connect_postgres(&url)
		.await
		.expect("DB connection should succeed");
	let store = Arc::new(SessionStore::new());
	let session = new_test_session();
	store.save(session.clone());
	let store_ref = SessionStoreRef(Arc::clone(&store));

	// Act
	let result = login(
		test_user.email.clone(),
		test_user.password.clone(),
		db,
		session,
		store_ref,
	)
	.await;

	// Assert
	assert!(result.is_ok(), "Login should succeed: {:?}", result.err());
	let user_info = result.unwrap();
	assert_eq!(user_info.id, user.id());
	assert_eq!(user_info.username, test_user.username);
	assert_eq!(user_info.email, test_user.email);
	assert!(user_info.is_active);
}

#[rstest]
#[tokio::test]
async fn test_login_server_fn_invalid_credentials(#[future] twitter_db_pool: (PgPool, String)) {
	// Arrange
	let (pool, url) = twitter_db_pool.await;
	reinitialize_database(&url)
		.await
		.expect("Database initialization should succeed");

	let factory = UserFactory::new();
	let test_user = TestTwitterUser::new("badpwdfnuser").with_password("CorrectPassword123");
	factory
		.create_from_test_user(&pool, &test_user)
		.await
		.expect("User creation should succeed");

	let db = DatabaseConnection::connect_postgres(&url)
		.await
		.expect("DB connection should succeed");
	let store = Arc::new(SessionStore::new());
	let session = new_test_session();
	store.save(session.clone());
	let store_ref = SessionStoreRef(Arc::clone(&store));

	// Act
	let result = login(
		test_user.email.clone(),
		"WrongPassword456".to_string(),
		db,
		session,
		store_ref,
	)
	.await;

	// Assert
	assert!(result.is_err(), "Login with wrong password should fail");
}

#[rstest]
#[tokio::test]
async fn test_login_server_fn_nonexistent_user(#[future] twitter_db_pool: (PgPool, String)) {
	// Arrange
	let (_pool, url) = twitter_db_pool.await;
	reinitialize_database(&url)
		.await
		.expect("Database initialization should succeed");

	let db = DatabaseConnection::connect_postgres(&url)
		.await
		.expect("DB connection should succeed");
	let store = Arc::new(SessionStore::new());
	let session = new_test_session();
	store.save(session.clone());
	let store_ref = SessionStoreRef(Arc::clone(&store));

	// Act
	let result = login(
		"nonexistent@example.com".to_string(),
		"SomePassword123".to_string(),
		db,
		session,
		store_ref,
	)
	.await;

	// Assert
	assert!(result.is_err(), "Login with nonexistent user should fail");
}

#[rstest]
#[tokio::test]
async fn test_login_server_fn_inactive_user(#[future] twitter_db_pool: (PgPool, String)) {
	// Arrange
	let (pool, url) = twitter_db_pool.await;
	reinitialize_database(&url)
		.await
		.expect("Database initialization should succeed");

	let factory = UserFactory::new();
	let test_user = TestTwitterUser::new("inactivefnuser")
		.with_password("ValidPassword123")
		.with_active(false);
	factory
		.create_from_test_user(&pool, &test_user)
		.await
		.expect("User creation should succeed");

	let db = DatabaseConnection::connect_postgres(&url)
		.await
		.expect("DB connection should succeed");
	let store = Arc::new(SessionStore::new());
	let session = new_test_session();
	store.save(session.clone());
	let store_ref = SessionStoreRef(Arc::clone(&store));

	// Act
	let result = login(
		test_user.email.clone(),
		test_user.password.clone(),
		db,
		session,
		store_ref,
	)
	.await;

	// Assert
	assert!(result.is_err(), "Login with inactive user should fail");
}

// ============================================================================
// Current User Server Function Tests
// ============================================================================

#[rstest]
#[tokio::test]
async fn test_current_user_authenticated(#[future] twitter_db_pool: (PgPool, String)) {
	// Arrange
	let (pool, url) = twitter_db_pool.await;
	reinitialize_database(&url)
		.await
		.expect("Database initialization should succeed");

	let factory = UserFactory::new();
	let test_user = TestTwitterUser::new("currentfnuser").with_password("ValidPassword123");
	let created_user = factory
		.create_from_test_user(&pool, &test_user)
		.await
		.expect("User creation should succeed");

	let db = DatabaseConnection::connect_postgres(&url)
		.await
		.expect("DB connection should succeed");

	// Create session with user_id set (simulating post-login state)
	let mut session = new_test_session();
	session
		.set("user_id".to_string(), created_user.id())
		.expect("Session set should succeed");

	// Act
	let result = current_user(db, session).await;

	// Assert
	assert!(
		result.is_ok(),
		"current_user should succeed: {:?}",
		result.err()
	);
	let user_info = result.unwrap();
	assert!(user_info.is_some(), "Should return user info");
	let user_info = user_info.unwrap();
	assert_eq!(user_info.id, created_user.id());
	assert_eq!(user_info.username, test_user.username);
	assert_eq!(user_info.email, test_user.email);
}

#[rstest]
#[tokio::test]
async fn test_current_user_unauthenticated(#[future] twitter_db_pool: (PgPool, String)) {
	// Arrange
	let (_pool, url) = twitter_db_pool.await;
	reinitialize_database(&url)
		.await
		.expect("Database initialization should succeed");

	let db = DatabaseConnection::connect_postgres(&url)
		.await
		.expect("DB connection should succeed");

	// Create session without user_id (unauthenticated)
	let session = new_test_session();

	// Act
	let result = current_user(db, session).await;

	// Assert
	assert!(
		result.is_ok(),
		"current_user should succeed: {:?}",
		result.err()
	);
	let user_info = result.unwrap();
	assert!(
		user_info.is_none(),
		"Should return None for unauthenticated session"
	);
}

// ============================================================================
// Logout Server Function Tests
// ============================================================================

#[rstest]
#[tokio::test]
async fn test_logout_server_fn(#[future] twitter_db_pool: (PgPool, String)) {
	// Arrange
	let (_pool, url) = twitter_db_pool.await;
	reinitialize_database(&url)
		.await
		.expect("Database initialization should succeed");

	let store = Arc::new(SessionStore::new());
	let mut session = new_test_session();
	session
		.set("user_id".to_string(), uuid::Uuid::new_v4())
		.expect("Session set should succeed");
	store.save(session.clone());
	let store_ref = SessionStoreRef(Arc::clone(&store));

	let session_id = session.id.clone();

	// Verify session exists before logout
	assert!(
		store.get(&session_id).is_some(),
		"Session should exist before logout"
	);

	// Act
	let result = logout(session, store_ref).await;

	// Assert
	assert!(result.is_ok(), "Logout should succeed: {:?}", result.err());
	assert!(
		store.get(&session_id).is_none(),
		"Session should be deleted after logout"
	);
}

// ============================================================================
// Login Session Persistence Tests
// ============================================================================

#[rstest]
#[tokio::test]
async fn test_login_persists_session_data(#[future] twitter_db_pool: (PgPool, String)) {
	// Arrange
	let (pool, url) = twitter_db_pool.await;
	reinitialize_database(&url)
		.await
		.expect("Database initialization should succeed");

	let factory = UserFactory::new();
	let test_user = TestTwitterUser::new("persistfnuser").with_password("ValidPassword123");
	let user = factory
		.create_from_test_user(&pool, &test_user)
		.await
		.expect("User creation should succeed");

	let db = DatabaseConnection::connect_postgres(&url)
		.await
		.expect("DB connection should succeed");
	let store = Arc::new(SessionStore::new());
	let session = new_test_session();
	let old_session_id = session.id.clone();
	store.save(session.clone());
	let store_ref = SessionStoreRef(Arc::clone(&store));

	// Act
	let result = login(
		test_user.email.clone(),
		test_user.password.clone(),
		db,
		session,
		store_ref,
	)
	.await;

	// Assert
	assert!(result.is_ok(), "Login should succeed");

	// Verify old session was deleted (session fixation prevention)
	assert!(
		store.get(&old_session_id).is_none(),
		"Old session should be deleted after login"
	);

	// Verify new session was created (store should have exactly 1 session)
	assert_eq!(
		store.len(),
		1,
		"Store should have exactly one session after login"
	);
}

// ============================================================================
// Full Auth Flow Integration Test
// ============================================================================

#[rstest]
#[tokio::test]
async fn test_auth_flow_login_then_current_user(#[future] twitter_db_pool: (PgPool, String)) {
	// Arrange
	let (pool, url) = twitter_db_pool.await;
	reinitialize_database(&url)
		.await
		.expect("Database initialization should succeed");

	let factory = UserFactory::new();
	let test_user = TestTwitterUser::new("flowfnuser").with_password("ValidPassword123");
	let created_user = factory
		.create_from_test_user(&pool, &test_user)
		.await
		.expect("User creation should succeed");

	// Step 1: Login
	let db = DatabaseConnection::connect_postgres(&url)
		.await
		.expect("DB connection should succeed");
	let store = Arc::new(SessionStore::new());
	let login_session = new_test_session();
	store.save(login_session.clone());
	let store_ref = SessionStoreRef(Arc::clone(&store));

	let login_result = login(
		test_user.email.clone(),
		test_user.password.clone(),
		db,
		login_session,
		store_ref,
	)
	.await;

	// Assert login succeeded
	assert!(
		login_result.is_ok(),
		"Login should succeed: {:?}",
		login_result.err()
	);
	let login_user_info = login_result.unwrap();
	assert_eq!(login_user_info.id, created_user.id());

	// Step 2: Simulate current_user with a session containing the logged-in user's ID.
	// After login, user_id is stored in the session within the store.
	// We construct a session with the same user_id to simulate the post-login state.
	let db = DatabaseConnection::connect_postgres(&url)
		.await
		.expect("DB connection should succeed");
	let mut post_login_session = new_test_session();
	post_login_session
		.set("user_id".to_string(), created_user.id())
		.expect("Session set should succeed");

	let current_result = current_user(db, post_login_session).await;

	// Assert current_user returns the same user
	assert!(
		current_result.is_ok(),
		"current_user should succeed: {:?}",
		current_result.err()
	);
	let current_user_info = current_result.unwrap();
	assert!(
		current_user_info.is_some(),
		"Should return user after login"
	);
	let current_user_info = current_user_info.unwrap();
	assert_eq!(current_user_info.id, created_user.id());
	assert_eq!(current_user_info.username, test_user.username);
	assert_eq!(current_user_info.email, test_user.email);

	// Step 3: Logout
	let logout_session = new_test_session();
	store.save(logout_session.clone());
	let logout_session_id = logout_session.id.clone();
	let store_ref = SessionStoreRef(Arc::clone(&store));

	let logout_result = logout(logout_session, store_ref).await;
	assert!(
		logout_result.is_ok(),
		"Logout should succeed: {:?}",
		logout_result.err()
	);
	assert!(
		store.get(&logout_session_id).is_none(),
		"Session should be removed after logout"
	);

	// Step 4: Verify current_user returns None after logout
	let db = DatabaseConnection::connect_postgres(&url)
		.await
		.expect("DB connection should succeed");
	let empty_session = new_test_session();
	let after_logout_result = current_user(db, empty_session).await;

	assert!(
		after_logout_result.is_ok(),
		"current_user after logout should succeed: {:?}",
		after_logout_result.err()
	);
	assert!(
		after_logout_result.unwrap().is_none(),
		"Should return None after logout"
	);
}

// ============================================================================
// Existing Model-Level Tests (preserved)
// ============================================================================

#[rstest]
#[tokio::test]
async fn test_login_success(#[future] twitter_db_pool: (PgPool, String)) {
	// Arrange
	let (pool, _url) = twitter_db_pool.await;
	let factory = UserFactory::new();

	let test_user = TestTwitterUser::new("loginuser").with_password("ValidPassword123");
	let user = factory
		.create_from_test_user(&pool, &test_user)
		.await
		.expect("User creation should succeed");

	// Act & Assert
	assert_eq!(user.email(), &test_user.email);
	assert!(user.is_active());

	let password_valid = user
		.check_password(&test_user.password)
		.expect("Password check should succeed");
	assert!(password_valid, "Password should be valid");
}

#[rstest]
#[tokio::test]
async fn test_login_invalid_password(#[future] twitter_db_pool: (PgPool, String)) {
	// Arrange
	let (pool, _url) = twitter_db_pool.await;
	let factory = UserFactory::new();

	let test_user = TestTwitterUser::new("wrongpwduser").with_password("CorrectPassword123");
	let user = factory
		.create_from_test_user(&pool, &test_user)
		.await
		.expect("User creation should succeed");

	// Act
	let password_valid = user
		.check_password("WrongPassword456")
		.expect("Password check should succeed");

	// Assert
	assert!(!password_valid, "Wrong password should fail");
}

#[rstest]
#[tokio::test]
async fn test_login_inactive_user(#[future] twitter_db_pool: (PgPool, String)) {
	// Arrange
	let (pool, _url) = twitter_db_pool.await;
	let factory = UserFactory::new();

	let test_user = TestTwitterUser::new("inactiveuser").with_active(false);
	let user = factory
		.create_from_test_user(&pool, &test_user)
		.await
		.expect("User creation should succeed");

	// Assert
	assert!(!user.is_active(), "User should be inactive");
}

#[rstest]
#[tokio::test]
async fn test_register_success(#[future] twitter_db_pool: (PgPool, String)) {
	// Arrange
	let (pool, _url) = twitter_db_pool.await;

	let existing = sqlx::query("SELECT id FROM auth_user WHERE email = $1")
		.bind("newuser@example.com")
		.fetch_optional(&pool)
		.await
		.expect("Query should succeed");
	assert!(existing.is_none(), "No user should exist with this email");

	let factory = UserFactory::new();
	let test_user = TestTwitterUser::new("newuser")
		.with_email("newuser@example.com")
		.with_password("SecurePassword123");

	// Act
	let user = factory
		.create_from_test_user(&pool, &test_user)
		.await
		.expect("User creation should succeed");

	// Assert
	assert_eq!(user.username(), "newuser");
	assert_eq!(user.email(), "newuser@example.com");
	assert!(user.is_active());

	let password_valid = user
		.check_password("SecurePassword123")
		.expect("Password check should succeed");
	assert!(password_valid);
}

#[rstest]
#[tokio::test]
async fn test_register_duplicate_email(#[future] twitter_db_pool: (PgPool, String)) {
	// Arrange
	let (pool, _url) = twitter_db_pool.await;
	let factory = UserFactory::new();

	let test_user = TestTwitterUser::new("firstuser").with_email("duplicate@example.com");
	factory
		.create_from_test_user(&pool, &test_user)
		.await
		.expect("First user creation should succeed");

	// Act
	let test_user2 = TestTwitterUser::new("seconduser").with_email("duplicate@example.com");
	let result = factory.create_from_test_user(&pool, &test_user2).await;

	// Assert
	assert!(result.is_err(), "Duplicate email should fail");
}

#[rstest]
#[tokio::test]
async fn test_register_password_validation() {
	use crate::apps::auth::shared::types::RegisterRequest;
	use validator::Validate;

	// Arrange
	let request = RegisterRequest {
		username: "testuser".to_string(),
		email: "test@example.com".to_string(),
		password: "short".to_string(),
		password_confirmation: "short".to_string(),
	};

	// Act
	let result = request.validate();

	// Assert
	assert!(result.is_err(), "Short password should fail validation");
}

#[rstest]
#[tokio::test]
async fn test_register_password_mismatch() {
	use crate::apps::auth::shared::types::RegisterRequest;

	// Arrange
	let request = RegisterRequest {
		username: "testuser".to_string(),
		email: "test@example.com".to_string(),
		password: "SecurePassword123".to_string(),
		password_confirmation: "DifferentPassword456".to_string(),
	};

	// Act
	let result = request.validate_passwords_match();

	// Assert
	assert!(result.is_err(), "Password mismatch should fail");
	assert!(result.unwrap_err().contains("do not match"));
}

#[rstest]
#[tokio::test]
async fn test_register_invalid_email() {
	use crate::apps::auth::shared::types::RegisterRequest;
	use validator::Validate;

	// Arrange
	let request = RegisterRequest {
		username: "testuser".to_string(),
		email: "not-an-email".to_string(),
		password: "SecurePassword123".to_string(),
		password_confirmation: "SecurePassword123".to_string(),
	};

	// Act
	let result = request.validate();

	// Assert
	assert!(result.is_err(), "Invalid email should fail validation");
}

#[rstest]
#[tokio::test]
async fn test_register_short_username() {
	use crate::apps::auth::shared::types::RegisterRequest;
	use validator::Validate;

	// Arrange
	let request = RegisterRequest {
		username: "ab".to_string(), // Too short (min 3)
		email: "test@example.com".to_string(),
		password: "SecurePassword123".to_string(),
		password_confirmation: "SecurePassword123".to_string(),
	};

	// Act
	let result = request.validate();

	// Assert
	assert!(result.is_err(), "Short username should fail validation");
}

#[rstest]
#[tokio::test]
async fn test_login_request_validation_empty_password() {
	use crate::apps::auth::shared::types::LoginRequest;
	use validator::Validate;

	// Arrange
	let request = LoginRequest {
		email: "test@example.com".to_string(),
		password: "".to_string(),
	};

	// Act
	let result = request.validate();

	// Assert
	assert!(result.is_err(), "Empty password should fail validation");
}

#[rstest]
#[tokio::test]
async fn test_login_request_validation_invalid_email() {
	use crate::apps::auth::shared::types::LoginRequest;
	use validator::Validate;

	// Arrange
	let request = LoginRequest {
		email: "invalid-email".to_string(),
		password: "ValidPassword123".to_string(),
	};

	// Act
	let result = request.validate();

	// Assert
	assert!(result.is_err(), "Invalid email should fail validation");
}

#[rstest]
#[tokio::test]
async fn test_user_info_conversion(#[future] twitter_db_pool: (PgPool, String)) {
	// Arrange
	let (pool, _url) = twitter_db_pool.await;
	let factory = UserFactory::new();

	let test_user = TestTwitterUser::new("infouser");
	let user = factory
		.create_from_test_user(&pool, &test_user)
		.await
		.expect("User creation should succeed");

	// Act
	let user_info = UserInfo::from(user.clone());

	// Assert
	assert_eq!(user_info.id, user.id());
	assert_eq!(&user_info.username, user.username());
	assert_eq!(&user_info.email, user.email());
	assert_eq!(user_info.is_active, user.is_active());
}
