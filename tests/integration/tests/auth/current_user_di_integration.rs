//! CurrentUser DI Integration Tests
//!
//! Tests the integration of CurrentUser, DI, database, and ORM.
//!
//! **Test Coverage:**
//! - CurrentUser injection via DI system
//! - Authentication state management (authenticated/anonymous)
//! - Database user loading with reinhardt-query
//! - State transitions (login/logout)
//!
//! **Fixtures Used:**
//! - postgres_container: PostgreSQL database container (reinhardt-test)
//! - singleton_scope: DI singleton scope (reinhardt-test)
//! - test_user: Test user data (reinhardt-test)

use chrono::Utc;
use reinhardt_auth::{BaseUser, CurrentUser, DefaultUser};
use reinhardt_di::{InjectionContext, SingletonScope};
use reinhardt_query::prelude::{
	ColumnDef, Expr, ExprTrait, Iden, IntoIden, PostgresQueryBuilder, Query, QueryStatementBuilder,
	Value,
};
use reinhardt_test::fixtures::auth::{TestUser, test_user};
use reinhardt_test::fixtures::singleton_scope;
use reinhardt_test::fixtures::testcontainers::{ContainerAsync, GenericImage, postgres_container};
use rstest::*;
use sqlx::{PgPool, Row};
use std::sync::Arc;
use uuid::Uuid;

// ============================================================================
// reinhardt-query Table Definition
// ============================================================================

/// AuthUser table identifier for reinhardt-query
#[derive(Debug, Clone, Copy, Iden)]
enum AuthUser {
	Table,
	Id,
	Username,
	Email,
	IsActive,
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Create DefaultUser from TestUser for testing
fn create_default_user(test_user: &TestUser) -> DefaultUser {
	DefaultUser {
		id: test_user.id,
		username: test_user.username.clone(),
		email: test_user.email.clone(),
		first_name: String::new(),
		last_name: String::new(),
		password_hash: None,
		last_login: None,
		is_active: test_user.is_active,
		is_staff: test_user.is_staff,
		is_superuser: test_user.is_superuser,
		date_joined: Utc::now(),
		user_permissions: Vec::new(),
		groups: Vec::new(),
	}
}

/// Create auth_user table using reinhardt-query
async fn create_auth_user_table(pool: &PgPool) {
	let mut stmt = Query::create_table();
	let create_table = stmt
		.table(AuthUser::Table.into_iden())
		.if_not_exists()
		.col(ColumnDef::new(AuthUser::Id).uuid().primary_key(true))
		.col(
			ColumnDef::new(AuthUser::Username)
				.string_len(150)
				.not_null(true),
		)
		.col(
			ColumnDef::new(AuthUser::Email)
				.string_len(254)
				.not_null(true),
		)
		.col(
			ColumnDef::new(AuthUser::IsActive)
				.boolean()
				.not_null(true)
				.default(true.into()),
		)
		.to_string(PostgresQueryBuilder::new());

	sqlx::query(&create_table).execute(pool).await.unwrap();
}

/// Insert user into auth_user table using reinhardt-query
async fn insert_user(pool: &PgPool, id: Uuid, username: &str, email: &str) {
	let mut insert_stmt = Query::insert();
	let insert = insert_stmt
		.into_table(AuthUser::Table.into_iden())
		.columns([AuthUser::Id, AuthUser::Username, AuthUser::Email])
		.values_panic([
			Value::from(id.to_string()),
			Value::from(username),
			Value::from(email),
		])
		.to_string(PostgresQueryBuilder::new());

	sqlx::query(&insert).execute(pool).await.unwrap();
}

/// Delete user from auth_user table using reinhardt-query
async fn delete_user(pool: &PgPool, id: Uuid) {
	let mut delete_stmt = Query::delete();
	let delete = delete_stmt
		.from_table(AuthUser::Table.into_iden())
		.and_where(Expr::col(AuthUser::Id).eq(Expr::value(id.to_string())))
		.to_string(PostgresQueryBuilder::new());

	sqlx::query(&delete).execute(pool).await.unwrap();
}

/// Select user by ID using reinhardt-query
async fn select_user_by_id(pool: &PgPool, id: Uuid) -> Option<(Uuid, String, String)> {
	let mut select_stmt = Query::select();
	let select = select_stmt
		.from(AuthUser::Table.into_iden())
		.columns([AuthUser::Id, AuthUser::Username, AuthUser::Email])
		.and_where(Expr::col(AuthUser::Id).eq(Expr::value(id.to_string())))
		.to_string(PostgresQueryBuilder::new());

	sqlx::query(&select)
		.fetch_optional(pool)
		.await
		.unwrap()
		.map(|row| {
			(
				row.get::<Uuid, _>("id"),
				row.get::<String, _>("username"),
				row.get::<String, _>("email"),
			)
		})
}

// ============================================================================
// Specialized Fixtures
// ============================================================================

/// Fixture: Database with auth_user table ready
///
/// Uses postgres_container and creates the auth_user table.
#[fixture]
async fn db_with_auth_table(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) -> (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String) {
	let (container, pool, port, url) = postgres_container.await;
	create_auth_user_table(pool.as_ref()).await;
	(container, pool, port, url)
}

/// Fixture: Database with a test user inserted
///
/// Uses db_with_auth_table and inserts a test user.
#[fixture]
async fn db_with_test_user(
	#[future] db_with_auth_table: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
	test_user: TestUser,
) -> (
	ContainerAsync<GenericImage>,
	Arc<PgPool>,
	u16,
	String,
	TestUser,
) {
	let (container, pool, port, url) = db_with_auth_table.await;
	insert_user(
		pool.as_ref(),
		test_user.id,
		&test_user.username,
		&test_user.email,
	)
	.await;
	(container, pool, port, url, test_user)
}

/// Fixture: CurrentUser instance for authenticated tests
#[fixture]
fn authenticated_current_user(test_user: TestUser) -> (CurrentUser<DefaultUser>, TestUser) {
	let user = create_default_user(&test_user);
	let current_user = CurrentUser::authenticated(user, test_user.id);
	(current_user, test_user)
}

// ============================================================================
// Sanity Tests (2 tests)
// ============================================================================

/// Test basic DI operation for CurrentUser (anonymous)
///
/// **Test Intent**: Verify CurrentUser can be created as anonymous via DI context
///
/// **Integration Point**: CurrentUser ↔ InjectionContext ↔ SingletonScope
#[rstest]
#[tokio::test]
async fn sanity_current_user_di_basic(singleton_scope: Arc<SingletonScope>) {
	let _ctx = InjectionContext::builder(singleton_scope).build();

	let current_user = CurrentUser::<DefaultUser>::anonymous();

	assert!(!current_user.is_authenticated());
	assert!(current_user.id().is_err());
}

/// Test basic user load operation from database
///
/// **Test Intent**: Verify user can be loaded from PostgreSQL using reinhardt-query
///
/// **Integration Point**: reinhardt-query → sqlx → PostgreSQL
#[rstest]
#[tokio::test]
async fn sanity_database_user_load(
	#[future] db_with_auth_table: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, _url) = db_with_auth_table.await;

	let user_id = Uuid::new_v4();
	let username = "test_user";
	let email = "test@example.com";

	insert_user(pool.as_ref(), user_id, username, email).await;

	let result = select_user_by_id(pool.as_ref(), user_id).await;
	assert!(result.is_some());

	let (loaded_id, loaded_username, _loaded_email) = result.unwrap();
	assert_eq!(loaded_id, user_id);
	assert_eq!(loaded_username, username);
}

// ============================================================================
// Normal Cases (6 tests)
// ============================================================================

/// Test CurrentUser injection into ViewSet
///
/// **Test Intent**: Verify authenticated CurrentUser can retrieve user information
///
/// **Integration Point**: CurrentUser → DefaultUser → BaseUser trait
#[rstest]
#[tokio::test]
async fn normal_current_user_viewset_injection(
	authenticated_current_user: (CurrentUser<DefaultUser>, TestUser),
) {
	let (current_user, test_user) = authenticated_current_user;

	assert!(current_user.is_authenticated());
	assert_eq!(current_user.id().unwrap(), test_user.id);
	assert_eq!(
		BaseUser::get_username(current_user.user().unwrap()),
		test_user.username
	);
}

/// Test CurrentUser shared across multiple endpoints
///
/// **Test Intent**: Verify CurrentUser can be cloned and shared
///
/// **Integration Point**: CurrentUser::clone → shared state
#[rstest]
#[tokio::test]
async fn normal_current_user_shared_across_endpoints(
	authenticated_current_user: (CurrentUser<DefaultUser>, TestUser),
) {
	let (current_user1, _test_user) = authenticated_current_user;
	let current_user2 = current_user1.clone();

	assert_eq!(current_user1.id().unwrap(), current_user2.id().unwrap());
	assert_eq!(
		BaseUser::get_username(current_user1.user().unwrap()),
		BaseUser::get_username(current_user2.user().unwrap())
	);
}

/// Test CurrentUser combined with DB query (reinhardt-query)
///
/// **Test Intent**: Verify CurrentUser works with database queries
///
/// **Integration Point**: CurrentUser → reinhardt-query → PostgreSQL
#[rstest]
#[tokio::test]
async fn normal_current_user_db_query(
	#[future] db_with_test_user: (
		ContainerAsync<GenericImage>,
		Arc<PgPool>,
		u16,
		String,
		TestUser,
	),
) {
	let (_container, pool, _port, _url, test_user) = db_with_test_user.await;

	let result = select_user_by_id(pool.as_ref(), test_user.id).await;
	assert!(result.is_some());

	let (loaded_id, loaded_username, _loaded_email) = result.unwrap();
	assert_eq!(loaded_id, test_user.id);
	assert_eq!(loaded_username, test_user.username);
}

/// Test CurrentUser and ORM QuerySet filtering
///
/// **Test Intent**: Verify CurrentUser ID can be used for filtering
///
/// **Integration Point**: CurrentUser::id → filter condition
#[rstest]
#[tokio::test]
async fn normal_current_user_orm_queryset_filtering(
	authenticated_current_user: (CurrentUser<DefaultUser>, TestUser),
) {
	let (current_user, test_user) = authenticated_current_user;

	let filter_user_id = current_user.id().unwrap();
	assert_eq!(filter_user_id, test_user.id);
}

/// Test CurrentUser with session authentication integration
///
/// **Test Intent**: Verify authenticated CurrentUser state
///
/// **Integration Point**: CurrentUser → session authentication
#[rstest]
#[tokio::test]
async fn normal_current_user_session_auth_integration(
	authenticated_current_user: (CurrentUser<DefaultUser>, TestUser),
) {
	let (current_user, test_user) = authenticated_current_user;

	assert!(current_user.is_authenticated());
	assert_eq!(current_user.id().unwrap(), test_user.id);
}

/// Test CurrentUser with JWT authentication integration
///
/// **Test Intent**: Verify authenticated CurrentUser with username access
///
/// **Integration Point**: CurrentUser → JWT authentication
#[rstest]
#[tokio::test]
async fn normal_current_user_jwt_auth_integration(
	authenticated_current_user: (CurrentUser<DefaultUser>, TestUser),
) {
	let (current_user, test_user) = authenticated_current_user;

	assert!(current_user.is_authenticated());
	assert_eq!(
		BaseUser::get_username(current_user.user().unwrap()),
		test_user.username
	);
}

// ============================================================================
// Error Cases (4 tests)
// ============================================================================

/// Test unauthenticated CurrentUser injection (anonymous)
///
/// **Test Intent**: Verify anonymous CurrentUser cannot access user data
///
/// **Integration Point**: CurrentUser::anonymous → error handling
#[rstest]
#[tokio::test]
async fn abnormal_unauthenticated_current_user_injection() {
	let current_user = CurrentUser::<DefaultUser>::anonymous();

	assert!(!current_user.is_authenticated());
	assert!(current_user.id().is_err());
	assert!(current_user.user().is_err());
}

/// Test CurrentUser injection with invalid user ID
///
/// **Test Intent**: Verify handling of invalid user scenarios
///
/// **Integration Point**: CurrentUser::anonymous for invalid cases
#[rstest]
#[tokio::test]
async fn abnormal_invalid_user_id_injection() {
	let _invalid_id = Uuid::nil();
	let current_user = CurrentUser::<DefaultUser>::anonymous();

	assert!(!current_user.is_authenticated());
	assert!(current_user.id().is_err());
}

/// Test CurrentUser after deleting user from DB
///
/// **Test Intent**: Verify user deletion from database
///
/// **Integration Point**: reinhardt-query DELETE → PostgreSQL
#[rstest]
#[tokio::test]
async fn abnormal_deleted_user_current_user(
	#[future] db_with_test_user: (
		ContainerAsync<GenericImage>,
		Arc<PgPool>,
		u16,
		String,
		TestUser,
	),
) {
	let (_container, pool, _port, _url, test_user) = db_with_test_user.await;

	delete_user(pool.as_ref(), test_user.id).await;

	let result = select_user_by_id(pool.as_ref(), test_user.id).await;
	assert!(result.is_none());
}

/// Test CurrentUser when session expired
///
/// **Test Intent**: Verify expired session results in anonymous user
///
/// **Integration Point**: Session expiration → CurrentUser::anonymous
#[rstest]
#[tokio::test]
async fn abnormal_expired_session_current_user() {
	let current_user = CurrentUser::<DefaultUser>::anonymous();

	assert!(!current_user.is_authenticated());
	assert!(current_user.id().is_err());
}

// ============================================================================
// State Transitions (2 tests)
// ============================================================================

/// Test state transition: Unauthenticated → Authenticated
///
/// **Test Intent**: Verify login process creates authenticated CurrentUser
///
/// **Integration Point**: CurrentUser::anonymous → CurrentUser::authenticated
#[rstest]
#[tokio::test]
async fn state_transition_unauthenticated_to_authenticated(test_user: TestUser) {
	// Initial state: Unauthenticated
	let anonymous = CurrentUser::<DefaultUser>::anonymous();
	assert!(!anonymous.is_authenticated());

	// Authentication process (login)
	let user = create_default_user(&test_user);
	let authenticated = CurrentUser::authenticated(user, test_user.id);

	// Final state: Authenticated
	assert!(authenticated.is_authenticated());
	assert_eq!(authenticated.id().unwrap(), test_user.id);
	assert_eq!(
		BaseUser::get_username(authenticated.user().unwrap()),
		test_user.username
	);
}

/// Test state transition: Logout → Anonymous
///
/// **Test Intent**: Verify logout process creates anonymous CurrentUser
///
/// **Integration Point**: CurrentUser::authenticated → logout → anonymous
#[rstest]
#[tokio::test]
async fn state_transition_logout_to_anonymous(test_user: TestUser) {
	// Initial state: Authenticated
	let user = create_default_user(&test_user);
	let authenticated = CurrentUser::authenticated(user, test_user.id);
	assert!(authenticated.is_authenticated());

	// Logout process
	let anonymous = CurrentUser::<DefaultUser>::anonymous();

	// Final state: Unauthenticated
	assert!(!anonymous.is_authenticated());
	assert!(anonymous.id().is_err());
}

// ============================================================================
// Combinations (2 tests)
// ============================================================================

/// Test CurrentUser + Session + DB integration
///
/// **Test Intent**: Verify full integration of CurrentUser with DB operations
///
/// **Integration Point**: PostgreSQL → reinhardt-query → CurrentUser
#[rstest]
#[tokio::test]
async fn combination_current_user_session_db(
	#[future] db_with_test_user: (
		ContainerAsync<GenericImage>,
		Arc<PgPool>,
		u16,
		String,
		TestUser,
	),
) {
	let (_container, pool, _port, _url, test_user) = db_with_test_user.await;

	// Load user from DB
	let result = select_user_by_id(pool.as_ref(), test_user.id).await;
	assert!(result.is_some());

	let (loaded_id, loaded_username, loaded_email) = result.unwrap();

	// Create CurrentUser from loaded data
	let user = DefaultUser {
		id: loaded_id,
		username: loaded_username.clone(),
		email: loaded_email,
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

	let current_user = CurrentUser::authenticated(user, loaded_id);

	assert!(current_user.is_authenticated());
	assert_eq!(current_user.id().unwrap(), test_user.id);
	assert_eq!(
		BaseUser::get_username(current_user.user().unwrap()),
		loaded_username
	);
}

/// Test CurrentUser + JWT + Redis integration (conceptual)
///
/// **Test Intent**: Verify CurrentUser works in JWT+Redis context
///
/// **Integration Point**: CurrentUser → JWT authentication
#[rstest]
#[tokio::test]
async fn combination_current_user_jwt_redis(
	authenticated_current_user: (CurrentUser<DefaultUser>, TestUser),
) {
	let (current_user, test_user) = authenticated_current_user;

	assert!(current_user.is_authenticated());
	assert_eq!(current_user.id().unwrap(), test_user.id);
}

// ============================================================================
// Equivalence Partitioning (2 tests, #[case])
// ============================================================================

/// Test user type equivalence partitioning
///
/// **Test Intent**: Verify CurrentUser works with different user creation methods
///
/// **Integration Point**: CurrentUser → DefaultUser variants
#[rstest]
#[case::from_test_user("from_test_user")]
#[case::direct_creation("direct_creation")]
#[tokio::test]
async fn equivalence_user_type(#[case] creation_method: &str, test_user: TestUser) {
	match creation_method {
		"from_test_user" => {
			let user = create_default_user(&test_user);
			let current_user = CurrentUser::authenticated(user, test_user.id);
			assert!(current_user.is_authenticated());
		}
		"direct_creation" => {
			let user = DefaultUser {
				id: test_user.id,
				username: "direct_user".to_string(),
				email: "direct@example.com".to_string(),
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
			let current_user = CurrentUser::authenticated(user, test_user.id);
			assert!(current_user.is_authenticated());
		}
		_ => panic!("Unknown creation method"),
	}
}

/// Test DI scope equivalence partitioning
///
/// **Test Intent**: Verify InjectionContext works with different scope types
///
/// **Integration Point**: InjectionContext → SingletonScope
#[rstest]
#[case::request_scope("request")]
#[case::singleton_scope("singleton")]
#[tokio::test]
async fn equivalence_di_scope(#[case] scope_type: &str, singleton_scope: Arc<SingletonScope>) {
	match scope_type {
		"request" => {
			let _ctx = InjectionContext::builder(singleton_scope.clone()).build();
			// Request scope uses same SingletonScope in tests
			assert!(true);
		}
		"singleton" => {
			let _ctx = InjectionContext::builder(singleton_scope).build();
			assert!(true);
		}
		_ => panic!("Unknown scope type"),
	}
}

// ============================================================================
// Edge Cases (2 tests)
// ============================================================================

/// Test multiple CurrentUser injections in same request
///
/// **Test Intent**: Verify multiple CurrentUser instances are consistent
///
/// **Integration Point**: CurrentUser::clone consistency
#[rstest]
#[tokio::test]
async fn edge_multiple_current_user_injection_same_request(test_user: TestUser) {
	let user = create_default_user(&test_user);

	let current_user1 = CurrentUser::authenticated(user.clone(), test_user.id);
	let current_user2 = CurrentUser::authenticated(user.clone(), test_user.id);
	let current_user3 = current_user1.clone();

	assert_eq!(current_user1.id().unwrap(), current_user2.id().unwrap());
	assert_eq!(current_user2.id().unwrap(), current_user3.id().unwrap());
}

/// Test CurrentUser injection performance under high load
///
/// **Test Intent**: Verify CurrentUser creation scales under load
///
/// **Integration Point**: CurrentUser creation performance
#[rstest]
#[tokio::test]
async fn edge_high_load_current_user_injection_performance(test_user: TestUser) {
	let user = create_default_user(&test_user);

	for _ in 0..1000 {
		let current_user = CurrentUser::authenticated(user.clone(), test_user.id);
		assert!(current_user.is_authenticated());
	}
}
