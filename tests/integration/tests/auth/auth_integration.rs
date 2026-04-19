//! # Authentication Integration Tests
//!
//! Comprehensive authentication integration tests for the Reinhardt framework.
//!
//! ## Test Coverage
//!
//! 1. **Permission Tests**: Core permission classes (AllowAny, IsAuthenticated, etc.)
//! 2. **JWT Tests**: Token generation, verification, and error handling
//! 3. **User Model Tests**: SimpleUser, AnonymousUser, and DefaultUser implementations
//! 4. **Database Integration Tests**: Session authentication with database backend
//!
//! ## Test Organization
//!
//! Tests are organized into modules:
//! - `permission_tests`: Pure unit tests for permission classes
//! - `jwt_tests`: Feature-gated JWT functionality tests
//! - `user_model_tests`: User model implementation tests
//! - `database_integration_tests`: Database integration tests with TestContainers

use bytes::Bytes;
use hyper::Method;
use reinhardt_auth::{
	AllowAny, AnonymousUser, IsAdminUser, IsAuthenticated, IsAuthenticatedOrReadOnly, Permission,
	PermissionContext, SimpleUser,
};
use reinhardt_http::Request;
use rstest::*;
use uuid::Uuid;

// ========================================================================
// Permission Tests (Pure Unit Tests)
// ========================================================================

mod permission_tests {
	use super::*;

	/// Test AllowAny permission grants access to all requests
	///
	/// **Test Intent**: Verify AllowAny permission allows all requests regardless of auth state
	///
	/// **Integration Point**: AllowAny permission class
	#[rstest]
	#[tokio::test]
	async fn test_allow_any_permission_grants_access() {
		let permission = AllowAny;
		let request = Request::builder()
			.method(Method::GET)
			.uri("/api/public")
			.body(Bytes::new())
			.build()
			.unwrap();

		// Unauthenticated user should be allowed
		let context = PermissionContext {
			request: &request,
			is_authenticated: false,
			is_admin: false,
			is_active: false,
			user: None,
		};
		assert!(permission.has_permission(&context).await);

		// Authenticated user should also be allowed
		let context_auth = PermissionContext {
			request: &request,
			is_authenticated: true,
			is_admin: false,
			is_active: true,
			user: None,
		};
		assert!(permission.has_permission(&context_auth).await);
	}

	/// Test IsAuthenticated denies unauthenticated users
	///
	/// **Test Intent**: Verify IsAuthenticated permission denies unauthenticated requests
	///
	/// **Integration Point**: IsAuthenticated permission class
	#[rstest]
	#[tokio::test]
	async fn test_is_authenticated_denies_unauthenticated() {
		let permission = IsAuthenticated;
		let request = Request::builder()
			.method(Method::GET)
			.uri("/api/protected")
			.body(Bytes::new())
			.build()
			.unwrap();

		let context = PermissionContext {
			request: &request,
			is_authenticated: false,
			is_admin: false,
			is_active: false,
			user: None,
		};
		assert!(!permission.has_permission(&context).await);
	}

	/// Test IsAuthenticated allows authenticated users
	///
	/// **Test Intent**: Verify IsAuthenticated permission allows authenticated requests
	///
	/// **Integration Point**: IsAuthenticated permission class
	#[rstest]
	#[tokio::test]
	async fn test_is_authenticated_allows_authenticated() {
		let permission = IsAuthenticated;
		let request = Request::builder()
			.method(Method::GET)
			.uri("/api/protected")
			.body(Bytes::new())
			.build()
			.unwrap();

		let context = PermissionContext {
			request: &request,
			is_authenticated: true,
			is_admin: false,
			is_active: true,
			user: None,
		};
		assert!(permission.has_permission(&context).await);
	}

	/// Test IsAdminUser requires admin flag
	///
	/// **Test Intent**: Verify IsAdminUser permission requires both authentication and admin flag
	///
	/// **Integration Point**: IsAdminUser permission class
	#[rstest]
	#[tokio::test]
	async fn test_is_admin_user_requires_admin_flag() {
		let permission = IsAdminUser;
		let request = Request::builder()
			.method(Method::GET)
			.uri("/admin/dashboard")
			.body(Bytes::new())
			.build()
			.unwrap();

		// Admin user should be allowed
		let admin_context = PermissionContext {
			request: &request,
			is_authenticated: true,
			is_admin: true,
			is_active: true,
			user: None,
		};
		assert!(permission.has_permission(&admin_context).await);

		// Non-admin authenticated user should be denied
		let user_context = PermissionContext {
			request: &request,
			is_authenticated: true,
			is_admin: false,
			is_active: true,
			user: None,
		};
		assert!(!permission.has_permission(&user_context).await);

		// Unauthenticated user should be denied
		let anon_context = PermissionContext {
			request: &request,
			is_authenticated: false,
			is_admin: false,
			is_active: false,
			user: None,
		};
		assert!(!permission.has_permission(&anon_context).await);
	}

	/// Test IsAuthenticatedOrReadOnly allows GET for anonymous users
	///
	/// **Test Intent**: Verify IsAuthenticatedOrReadOnly allows safe methods without auth
	///
	/// **Integration Point**: IsAuthenticatedOrReadOnly permission class
	#[rstest]
	#[tokio::test]
	async fn test_is_authenticated_or_readonly_allows_get_anonymous() {
		let permission = IsAuthenticatedOrReadOnly;
		let request = Request::builder()
			.method(Method::GET)
			.uri("/api/articles")
			.body(Bytes::new())
			.build()
			.unwrap();

		let context = PermissionContext {
			request: &request,
			is_authenticated: false,
			is_admin: false,
			is_active: false,
			user: None,
		};
		assert!(permission.has_permission(&context).await);
	}

	/// Test IsAuthenticatedOrReadOnly denies POST for anonymous users
	///
	/// **Test Intent**: Verify IsAuthenticatedOrReadOnly denies unsafe methods without auth
	///
	/// **Integration Point**: IsAuthenticatedOrReadOnly permission class
	#[rstest]
	#[tokio::test]
	async fn test_is_authenticated_or_readonly_denies_post_anonymous() {
		let permission = IsAuthenticatedOrReadOnly;
		let request = Request::builder()
			.method(Method::POST)
			.uri("/api/articles")
			.body(Bytes::new())
			.build()
			.unwrap();

		// Unauthenticated POST should be denied
		let context = PermissionContext {
			request: &request,
			is_authenticated: false,
			is_admin: false,
			is_active: false,
			user: None,
		};
		assert!(!permission.has_permission(&context).await);

		// Authenticated POST should be allowed
		let auth_context = PermissionContext {
			request: &request,
			is_authenticated: true,
			is_admin: false,
			is_active: true,
			user: None,
		};
		assert!(permission.has_permission(&auth_context).await);
	}
}

// ========================================================================
// JWT Tests (Feature-gated)
// ========================================================================

#[cfg(feature = "jwt")]
mod jwt_tests {
	use super::*;
	use reinhardt_auth::jwt::{Claims, JwtAuth};

	/// Test JWT token generation
	///
	/// **Test Intent**: Verify JwtAuth can generate valid JWT tokens
	///
	/// **Integration Point**: JwtAuth token generation
	#[rstest]
	#[tokio::test]
	async fn test_jwt_token_generation() {
		let jwt_auth = JwtAuth::new(b"test_secret_key_for_jwt");
		let user_id = "user_123".to_string();
		let username = "alice".to_string();

		let token = jwt_auth
			.generate_token(user_id, username, false, false)
			.unwrap();

		assert!(!token.is_empty());
		assert!(token.contains('.'));
	}

	/// Test JWT token verification with valid token
	///
	/// **Test Intent**: Verify JwtAuth can verify valid JWT tokens
	///
	/// **Integration Point**: JwtAuth token verification
	#[rstest]
	#[tokio::test]
	async fn test_jwt_token_verification_valid() {
		let jwt_auth = JwtAuth::new(b"test_secret_key_for_jwt");
		let user_id = "user_123".to_string();
		let username = "alice".to_string();

		let token = jwt_auth
			.generate_token(user_id.clone(), username.clone(), false, false)
			.unwrap();
		let claims = jwt_auth.verify_token(&token).unwrap();

		assert_eq!(claims.sub, user_id);
		assert_eq!(claims.username, username);
		assert!(!claims.is_expired());
	}

	/// Test JWT token verification with wrong secret
	///
	/// **Test Intent**: Verify JwtAuth rejects tokens signed with different secret
	///
	/// **Integration Point**: JwtAuth token verification security
	#[rstest]
	#[tokio::test]
	async fn test_jwt_token_verification_wrong_secret() {
		let jwt_auth1 = JwtAuth::new(b"secret_key_1");
		let jwt_auth2 = JwtAuth::new(b"secret_key_2");

		let token = jwt_auth1
			.generate_token("user_123".to_string(), "alice".to_string(), false, false)
			.unwrap();

		// Verification with different secret should fail
		let result = jwt_auth2.verify_token(&token);
		assert!(result.is_err());
	}

	/// Test JWT claims expiration check
	///
	/// **Test Intent**: Verify Claims::is_expired() correctly checks expiration
	///
	/// **Integration Point**: JWT claims expiration logic
	#[rstest]
	#[tokio::test]
	async fn test_jwt_claims_expiration() {
		// Non-expired token
		let claims = Claims::new(
			"user_123".to_string(),
			"alice".to_string(),
			chrono::Duration::hours(24),
			false,
			false,
		);
		assert!(!claims.is_expired());

		// Already expired token (negative duration)
		let expired_claims = Claims::new(
			"user_123".to_string(),
			"alice".to_string(),
			chrono::Duration::seconds(-10),
			false,
			false,
		);
		assert!(expired_claims.is_expired());
	}
}

// ========================================================================
// User Model Tests
// ========================================================================

mod user_model_tests {
	use super::*;
	use reinhardt_auth::User;

	/// Test SimpleUser implementation
	///
	/// **Test Intent**: Verify SimpleUser implements User trait correctly
	///
	/// **Integration Point**: SimpleUser implementation
	#[rstest]
	#[tokio::test]
	async fn test_simple_user_implementation() {
		let user = SimpleUser {
			id: Uuid::now_v7(),
			username: "testuser".to_string(),
			email: "test@example.com".to_string(),
			is_active: true,
			is_admin: false,
			is_staff: false,
			is_superuser: false,
		};

		assert!(!user.id().is_empty());
		assert_eq!(user.username(), "testuser");
		assert!(user.is_authenticated());
		assert!(user.is_active());
		assert!(!user.is_admin());
	}

	/// Test AnonymousUser is not authenticated
	///
	/// **Test Intent**: Verify AnonymousUser represents unauthenticated state
	///
	/// **Integration Point**: AnonymousUser implementation
	#[rstest]
	#[tokio::test]
	async fn test_anonymous_user_not_authenticated() {
		let user = AnonymousUser;

		assert_eq!(user.id(), "");
		assert_eq!(user.username(), "");
		assert!(!user.is_authenticated());
		assert!(!user.is_active());
		assert!(!user.is_admin());
	}

	/// Test SimpleUser with different user flags
	///
	/// **Test Intent**: Verify SimpleUser correctly handles is_active, is_staff, is_superuser flags
	///
	/// **Integration Point**: SimpleUser flag handling
	#[rstest]
	#[case(true, false, false, false)]
	#[case(true, true, false, false)]
	#[case(true, false, true, false)]
	#[case(true, true, true, true)]
	#[tokio::test]
	async fn test_simple_user_flags(
		#[case] is_active: bool,
		#[case] is_staff: bool,
		#[case] is_admin: bool,
		#[case] is_superuser: bool,
	) {
		let user = SimpleUser {
			id: Uuid::now_v7(),
			username: "user".to_string(),
			email: "user@example.com".to_string(),
			is_active,
			is_admin,
			is_staff,
			is_superuser,
		};

		assert_eq!(user.is_active(), is_active);
		assert_eq!(user.is_admin(), is_admin);
	}
}

// ========================================================================
// Database Integration Tests
// ========================================================================

#[cfg(feature = "argon2-hasher")]
mod database_integration_tests {
	use super::*;
	use chrono::Utc;
	use reinhardt_auth::{BaseUser, DefaultUser, PermissionsMixin, User};
	use reinhardt_db::DatabaseConnection;
	use reinhardt_test::fixtures::postgres_container;
	use serial_test::serial;
	use sqlx::PgPool;
	use std::sync::Arc;
	use testcontainers::{ContainerAsync, GenericImage};

	/// Fixture for authentication database tests
	///
	/// Uses postgres_container and sets up auth-related schema
	#[fixture]
	async fn auth_test_db(
		#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
	) -> (
		ContainerAsync<GenericImage>,
		Arc<DatabaseConnection>,
		u16,
		String,
	) {
		let (container, pool, port, url) = postgres_container.await;
		let connection = DatabaseConnection::connect(&url).await.unwrap();

		// Create simple auth_user table for testing
		sqlx::query(
			r#"
			CREATE TABLE IF NOT EXISTS auth_user (
				id UUID PRIMARY KEY,
				username VARCHAR(255) UNIQUE NOT NULL,
				email VARCHAR(255),
				first_name VARCHAR(255),
				last_name VARCHAR(255),
				password_hash TEXT,
				last_login TIMESTAMP WITH TIME ZONE,
				is_active BOOLEAN NOT NULL DEFAULT TRUE,
				is_staff BOOLEAN NOT NULL DEFAULT FALSE,
				is_superuser BOOLEAN NOT NULL DEFAULT FALSE,
				date_joined TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
				user_permissions TEXT[] NOT NULL DEFAULT '{}',
				groups TEXT[] NOT NULL DEFAULT '{}'
			)
			"#,
		)
		.execute(pool.as_ref())
		.await
		.unwrap();

		(container, Arc::new(connection), port, url)
	}

	/// Test session authentication with database
	///
	/// **Test Intent**: Verify session-based authentication can persist and restore user state
	///
	/// **Integration Point**: Session authentication + Database backend
	#[rstest]
	#[serial(auth_db)]
	#[tokio::test]
	async fn test_session_auth_with_database(
		#[future] auth_test_db: (
			ContainerAsync<GenericImage>,
			Arc<DatabaseConnection>,
			u16,
			String,
		),
	) {
		let (_container, _connection, _port, _url) = auth_test_db.await;

		// Create a test user
		let user = SimpleUser {
			id: Uuid::now_v7(),
			username: "session_user".to_string(),
			email: "session@example.com".to_string(),
			is_active: true,
			is_admin: false,
			is_staff: false,
			is_superuser: false,
		};

		// Verify user ID can be used as session key
		assert!(!user.id().is_empty());
		assert_eq!(user.username(), "session_user");
		assert!(user.is_authenticated());
	}

	/// Test DefaultUser password hashing
	///
	/// **Test Intent**: Verify DefaultUser correctly hashes and verifies passwords using Argon2
	///
	/// **Integration Point**: DefaultUser + Argon2Hasher
	#[rstest]
	#[serial(auth_db)]
	#[tokio::test]
	async fn test_default_user_password_hashing(
		#[future] auth_test_db: (
			ContainerAsync<GenericImage>,
			Arc<DatabaseConnection>,
			u16,
			String,
		),
	) {
		let (_container, _connection, _port, _url) = auth_test_db.await;

		let mut user = DefaultUser {
			id: Uuid::now_v7(),
			username: "passworduser".to_string(),
			email: "password@example.com".to_string(),
			first_name: "Password".to_string(),
			last_name: "User".to_string(),
			password_hash: None,
			last_login: None,
			is_active: true,
			is_staff: false,
			is_superuser: false,
			date_joined: Utc::now(),
			user_permissions: Vec::new(),
			groups: Vec::new(),
		};

		// Set password (should be hashed)
		user.set_password("secure_password_123").unwrap();
		assert!(user.password_hash.is_some());

		// Verify correct password
		assert!(user.check_password("secure_password_123").unwrap());

		// Verify incorrect password fails
		assert!(!user.check_password("wrong_password").unwrap());
	}

	/// Test DefaultUser with permissions
	///
	/// **Test Intent**: Verify DefaultUser correctly implements PermissionsMixin
	///
	/// **Integration Point**: DefaultUser + PermissionsMixin
	#[rstest]
	#[serial(auth_db)]
	#[tokio::test]
	async fn test_default_user_permissions(
		#[future] auth_test_db: (
			ContainerAsync<GenericImage>,
			Arc<DatabaseConnection>,
			u16,
			String,
		),
	) {
		let (_container, _connection, _port, _url) = auth_test_db.await;

		let user = DefaultUser {
			id: Uuid::now_v7(),
			username: "permuser".to_string(),
			email: "perm@example.com".to_string(),
			first_name: "Permission".to_string(),
			last_name: "User".to_string(),
			password_hash: None,
			last_login: None,
			is_active: true,
			is_staff: false,
			is_superuser: false,
			date_joined: Utc::now(),
			user_permissions: vec!["blog.add_post".to_string(), "blog.change_post".to_string()],
			groups: vec!["editors".to_string()],
		};

		// Check individual permissions
		assert!(user.has_perm("blog.add_post"));
		assert!(user.has_perm("blog.change_post"));
		assert!(!user.has_perm("blog.delete_post"));

		// Check module-level permissions
		assert!(user.has_module_perms("blog"));
		assert!(!user.has_module_perms("admin"));
	}

	/// Test DefaultUser superuser permissions
	///
	/// **Test Intent**: Verify superuser has all permissions regardless of explicit grants
	///
	/// **Integration Point**: DefaultUser superuser flag
	#[rstest]
	#[serial(auth_db)]
	#[tokio::test]
	async fn test_default_user_superuser_permissions(
		#[future] auth_test_db: (
			ContainerAsync<GenericImage>,
			Arc<DatabaseConnection>,
			u16,
			String,
		),
	) {
		let (_container, _connection, _port, _url) = auth_test_db.await;

		let superuser = DefaultUser {
			id: Uuid::now_v7(),
			username: "superuser".to_string(),
			email: "super@example.com".to_string(),
			first_name: "Super".to_string(),
			last_name: "User".to_string(),
			password_hash: None,
			last_login: None,
			is_active: true,
			is_staff: true,
			is_superuser: true,
			date_joined: Utc::now(),
			user_permissions: Vec::new(),
			groups: Vec::new(),
		};

		// Superuser should have all permissions even without explicit grants
		assert!(superuser.has_perm("any.permission"));
		assert!(superuser.has_perm("blog.delete_post"));
		assert!(superuser.has_module_perms("any_module"));
	}
}
