//! Integration tests for admin login server function
//!
//! Tests authentication flow including CSRF validation, JWT generation,
//! and error handling for various failure modes.

use reinhardt_admin::adapters::LoginResponse;
use reinhardt_admin::core::{AdminSite, admin_routes_with_di};
use reinhardt_admin::server::AdminDefaultUser;
use reinhardt_admin::server::security::{CSRF_COOKIE_NAME, generate_csrf_token};
use reinhardt_auth::BaseUser;
use reinhardt_db::backends::connection::DatabaseConnection as BackendsConnection;
use reinhardt_db::backends::dialect::PostgresBackend;
use reinhardt_db::orm::connection::{DatabaseBackend, DatabaseConnection};
use reinhardt_di::Depends;
use reinhardt_di::{InjectionContext, SingletonScope};
use reinhardt_http::Handler;
use reinhardt_query::prelude::{
	Alias, ColumnDef, Expr, PostgresQueryBuilder, Query, QueryStatementBuilder,
};
use reinhardt_query::value::IntoValue;
use reinhardt_test::fixtures::shared_postgres::shared_db_pool;
use reinhardt_urls::routers::ServerRouter;
use rstest::*;
use sqlx::Executor;
use std::sync::Arc;
use uuid::Uuid;

use super::server_fn_helpers::TEST_USER_UUID;

/// Test-only secret for JWT signing (not a real secret)
const TEST_JWT_SECRET: &[u8] = b"test-only-jwt-secret-key-for-admin-login-tests!";

/// Builds the CREATE TABLE SQL for the auth_user table using SeaQuery.
fn build_auth_user_create_table_sql() -> String {
	Query::create_table()
		.table(Alias::new("auth_user"))
		.if_not_exists()
		.col(
			ColumnDef::new(Alias::new("id"))
				.uuid()
				.not_null(true)
				.primary_key(true),
		)
		.col(
			ColumnDef::new(Alias::new("username"))
				.string_len(150)
				.not_null(true),
		)
		.col(
			ColumnDef::new(Alias::new("email"))
				.string_len(254)
				.not_null(true)
				.default("".into()),
		)
		.col(
			ColumnDef::new(Alias::new("first_name"))
				.string_len(150)
				.not_null(true)
				.default("".into()),
		)
		.col(
			ColumnDef::new(Alias::new("last_name"))
				.string_len(150)
				.not_null(true)
				.default("".into()),
		)
		.col(ColumnDef::new(Alias::new("password_hash")).text())
		.col(ColumnDef::new(Alias::new("last_login")).timestamp_with_time_zone())
		.col(
			ColumnDef::new(Alias::new("is_active"))
				.boolean()
				.not_null(true)
				.default(true.into()),
		)
		.col(
			ColumnDef::new(Alias::new("is_staff"))
				.boolean()
				.not_null(true)
				.default(false.into()),
		)
		.col(
			ColumnDef::new(Alias::new("is_superuser"))
				.boolean()
				.not_null(true)
				.default(false.into()),
		)
		.col(
			ColumnDef::new(Alias::new("date_joined"))
				.timestamp_with_time_zone()
				.not_null(true)
				.default(Expr::current_timestamp().into()),
		)
		.col(
			ColumnDef::new(Alias::new("user_permissions"))
				.text()
				.not_null(true)
				.default("[]".into()),
		)
		.col(
			ColumnDef::new(Alias::new("groups"))
				.text()
				.not_null(true)
				.default("[]".into()),
		)
		.to_string(PostgresQueryBuilder::new())
}

/// Generates an argon2 password hash for the given password using the
/// same hasher that `AdminDefaultUser` uses for `check_password`.
fn hash_test_password(password: &str) -> String {
	let mut user = AdminDefaultUser {
		id: Uuid::nil(),
		username: String::new(),
		email: String::new(),
		first_name: String::new(),
		last_name: String::new(),
		password_hash: None,
		last_login: None,
		is_active: true,
		is_staff: true,
		is_superuser: false,
		date_joined: chrono::Utc::now(),
		user_permissions: vec![],
		groups: vec![],
	};
	user.set_password(password)
		.expect("Failed to hash password");
	user.password_hash
		.expect("password_hash should be set after set_password")
}

// ============================================================
// Test 1: LoginResponse serialization roundtrip
// ============================================================

#[rstest]
fn test_admin_login_response_serialization_roundtrip() {
	// Arrange
	let response = LoginResponse {
		token: "test-token-header.test-token-payload.test-token-signature".to_string(),
		username: "admin_user".to_string(),
		user_id: "550e8400-e29b-41d4-a716-446655440000".to_string(),
		is_staff: true,
		is_superuser: true,
	};

	// Act
	let json = serde_json::to_string(&response).expect("serialization should succeed");
	let deserialized: LoginResponse =
		serde_json::from_str(&json).expect("deserialization should succeed");

	// Assert
	assert_eq!(deserialized.token, response.token);
	assert_eq!(deserialized.username, response.username);
	assert_eq!(deserialized.user_id, response.user_id);
	assert_eq!(deserialized.is_staff, response.is_staff);
	assert_eq!(deserialized.is_superuser, response.is_superuser);
}

// ============================================================
// Helper: build an E2E router with JWT secret configured
// ============================================================

/// Builds a ServerRouter with a test staff user in the database.
///
/// When `with_jwt_secret` is true, the AdminSite is configured with a JWT secret.
async fn build_login_router(pool: sqlx::PgPool, with_jwt_secret: bool) -> ServerRouter {
	// Create auth_user table using SeaQuery
	let drop_sql = Query::drop_table()
		.table(Alias::new("auth_user"))
		.if_exists()
		.cascade()
		.to_string(PostgresQueryBuilder::new());
	pool.execute(drop_sql.as_str())
		.await
		.expect("Failed to drop auth_user table");

	let create_sql = build_auth_user_create_table_sql();
	pool.execute(create_sql.as_str())
		.await
		.expect("Failed to create auth_user table");

	// Hash the test password using the same hasher as AdminDefaultUser
	let password_hash = hash_test_password("test_password");

	// Insert test staff user using SeaQuery with bind parameters for sensitive data
	let upsert_sql = Query::insert()
		.into_table(Alias::new("auth_user"))
		.columns([
			Alias::new("id"),
			Alias::new("username"),
			Alias::new("email"),
			Alias::new("password_hash"),
			Alias::new("is_active"),
			Alias::new("is_staff"),
			Alias::new("is_superuser"),
			Alias::new("date_joined"),
		])
		.values_panic(vec![
			"$1".into_value(),
			"test_staff".into_value(),
			"staff@test.example".into_value(),
			"$2".into_value(),
			true.into_value(),
			true.into_value(),
			false.into_value(),
			chrono::Utc::now().into_value(),
		])
		.on_conflict(
			reinhardt_query::prelude::OnConflict::column(Alias::new("id"))
				.update_columns([
					Alias::new("password_hash"),
					Alias::new("is_staff"),
					Alias::new("is_active"),
				])
				.to_owned(),
		)
		.to_string(PostgresQueryBuilder::new());
	// SeaQuery generates string literals for $1/$2 placeholders, so we
	// fall back to a parameterized query for sensitive bind values.
	pool.execute(
		sqlx::query(
			"INSERT INTO auth_user (id, username, email, password_hash, is_active, is_staff, is_superuser, date_joined) \
			 VALUES ($1, 'test_staff', 'staff@test.example', $2, true, true, false, NOW()) \
			 ON CONFLICT (id) DO UPDATE SET password_hash = $2, is_staff = true, is_active = true",
		)
		.bind(Uuid::parse_str(TEST_USER_UUID).expect("Invalid TEST_USER_UUID"))
		.bind(&password_hash),
	)
	.await
	.expect("Failed to insert test staff user");
	// Suppress unused variable warning for the generated SQL
	let _ = upsert_sql;

	// Build DatabaseConnection
	let backend = Arc::new(PostgresBackend::new(pool));
	let backends_conn = BackendsConnection::new(backend);
	let connection = DatabaseConnection::new(DatabaseBackend::Postgres, backends_conn);
	let db_conn = Arc::new(connection);

	// Build AdminSite
	let mut site = AdminSite::new("Login Test Admin");
	if with_jwt_secret {
		site.set_jwt_secret(TEST_JWT_SECRET);
	}
	let site = Arc::new(site);

	// Build admin router with deferred DI
	let (admin_router, admin_di) = admin_routes_with_di(site);

	// Build complete router with DI
	let singleton = Arc::new(SingletonScope::new());
	singleton.set_arc(db_conn);
	let di_ctx = Arc::new(InjectionContext::builder(singleton).build());

	reinhardt_urls::routers::UnifiedRouter::new()
		.with_di_context(di_ctx)
		.mount("/admin/", admin_router)
		.with_di_registrations(admin_di)
		.into_server()
}

/// Builds an HTTP POST request for the login endpoint.
fn make_login_request(
	username: &str,
	password: &str,
	csrf_token: &str,
	cookie_token: Option<&str>,
) -> reinhardt_http::Request {
	let body = serde_json::json!({
		"username": username,
		"password": password,
		"csrf_token": csrf_token,
	});
	let body_bytes = serde_json::to_vec(&body).expect("Failed to serialize request body");

	let mut builder = reinhardt_http::Request::builder()
		.method(hyper::Method::POST)
		.uri("/admin/api/server_fn/admin_login")
		.header("host", "localhost")
		.header("origin", "http://localhost")
		.header("content-type", "application/json");

	if let Some(cookie_val) = cookie_token {
		builder = builder.header("cookie", format!("{}={}", CSRF_COOKIE_NAME, cookie_val));
	}

	builder
		.body(hyper::body::Bytes::from(body_bytes))
		.build()
		.expect("Failed to build login request")
}

// ============================================================
// Test 2: CSRF validation — missing cookie
// ============================================================

#[rstest]
#[tokio::test]
async fn test_admin_login_csrf_validation_missing_cookie(
	#[future] shared_db_pool: (sqlx::PgPool, String),
) {
	// Arrange
	let (pool, _) = shared_db_pool.await;
	let router = build_login_router(pool, true).await;
	let csrf_token = generate_csrf_token();
	let request = make_login_request("test_staff", "test_password", &csrf_token, None);

	// Act
	let response = router.handle(request).await;

	// Assert
	assert!(
		response.is_ok(),
		"Router should not return routing error: {:?}",
		response.err()
	);
	let response = response.unwrap();
	assert_eq!(
		response.status.as_u16(),
		403,
		"Expected 403 for missing CSRF cookie, got: {}. Body: {}",
		response.status,
		String::from_utf8_lossy(&response.body)
	);
}

// ============================================================
// Test 3: CSRF validation — token mismatch
// ============================================================

#[rstest]
#[tokio::test]
async fn test_admin_login_csrf_validation_mismatch(
	#[future] shared_db_pool: (sqlx::PgPool, String),
) {
	// Arrange
	let (pool, _) = shared_db_pool.await;
	let router = build_login_router(pool, true).await;
	let body_token = generate_csrf_token();
	let cookie_token = generate_csrf_token();
	let request = make_login_request(
		"test_staff",
		"test_password",
		&body_token,
		Some(&cookie_token),
	);

	// Act
	let response = router.handle(request).await;

	// Assert
	assert!(response.is_ok(), "Router should not return routing error");
	let response = response.unwrap();
	assert_eq!(
		response.status.as_u16(),
		403,
		"Expected 403 for CSRF token mismatch, got: {}. Body: {}",
		response.status,
		String::from_utf8_lossy(&response.body)
	);
}

// ============================================================
// Test 4: Missing JWT secret
// ============================================================

#[rstest]
#[tokio::test]
async fn test_admin_login_missing_jwt_secret(#[future] shared_db_pool: (sqlx::PgPool, String)) {
	// Arrange
	let (pool, _) = shared_db_pool.await;
	let router = build_login_router(pool, false).await;
	let csrf_token = generate_csrf_token();
	let request = make_login_request(
		"test_staff",
		"test_password",
		&csrf_token,
		Some(&csrf_token),
	);

	// Act
	let response = router.handle(request).await;

	// Assert
	assert!(response.is_ok(), "Router should not return routing error");
	let response = response.unwrap();
	assert_eq!(
		response.status.as_u16(),
		500,
		"Expected 500 for missing JWT secret, got: {}. Body: {}",
		response.status,
		String::from_utf8_lossy(&response.body)
	);
}

// ============================================================
// Test 5: Authenticator returns None (invalid credentials)
// ============================================================

#[rstest]
#[tokio::test]
async fn test_admin_login_authenticator_returns_none(
	#[future] shared_db_pool: (sqlx::PgPool, String),
) {
	// Arrange
	let (pool, _) = shared_db_pool.await;
	let router = build_login_router(pool, true).await;
	let csrf_token = generate_csrf_token();
	// Use wrong password to cause authenticator to return None
	let request = make_login_request(
		"test_staff",
		"wrong_password",
		&csrf_token,
		Some(&csrf_token),
	);

	// Act
	let response = router.handle(request).await;

	// Assert
	assert!(response.is_ok(), "Router should not return routing error");
	let response = response.unwrap();
	assert_eq!(
		response.status.as_u16(),
		401,
		"Expected 401 for invalid credentials, got: {}. Body: {}",
		response.status,
		String::from_utf8_lossy(&response.body)
	);
}

// ============================================================
// Test 6: Authenticator returns Err (internal error)
// ============================================================

#[rstest]
#[tokio::test]
async fn test_admin_login_authenticator_returns_error(
	#[future] shared_db_pool: (sqlx::PgPool, String),
) {
	// Arrange: Build router WITHOUT creating auth_user table so the
	// authenticator's DB query fails with an error
	let (pool, _) = shared_db_pool.await;

	// Ensure auth_user table does NOT exist
	let drop_sql = Query::drop_table()
		.table(Alias::new("auth_user"))
		.if_exists()
		.cascade()
		.to_string(PostgresQueryBuilder::new());
	pool.execute(drop_sql.as_str())
		.await
		.expect("Failed to drop auth_user table");

	// Build DatabaseConnection
	let backend = Arc::new(PostgresBackend::new(pool));
	let backends_conn = BackendsConnection::new(backend);
	let connection = DatabaseConnection::new(DatabaseBackend::Postgres, backends_conn);
	let db_conn = Arc::new(connection);

	let mut site = AdminSite::new("Error Test Admin");
	site.set_jwt_secret(TEST_JWT_SECRET);
	let site = Arc::new(site);

	let (admin_router, admin_di) = admin_routes_with_di(site);

	let singleton = Arc::new(SingletonScope::new());
	singleton.set_arc(db_conn);
	let di_ctx = Arc::new(InjectionContext::builder(singleton).build());

	let router = reinhardt_urls::routers::UnifiedRouter::new()
		.with_di_context(di_ctx)
		.mount("/admin/", admin_router)
		.with_di_registrations(admin_di)
		.into_server();

	let csrf_token = generate_csrf_token();
	let request = make_login_request(
		"test_staff",
		"test_password",
		&csrf_token,
		Some(&csrf_token),
	);

	// Act
	let response = router.handle(request).await;

	// Assert
	assert!(response.is_ok(), "Router should not return routing error");
	let response = response.unwrap();
	assert_eq!(
		response.status.as_u16(),
		500,
		"Expected 500 for internal authentication error, got: {}. Body: {}",
		response.status,
		String::from_utf8_lossy(&response.body)
	);
}

// ============================================================
// Test 7: Happy path — valid login returns token
// ============================================================

#[rstest]
#[tokio::test]
async fn test_admin_login_happy_path(#[future] shared_db_pool: (sqlx::PgPool, String)) {
	// Arrange
	let (pool, _) = shared_db_pool.await;
	let router = build_login_router(pool, true).await;
	let csrf_token = generate_csrf_token();
	let request = make_login_request(
		"test_staff",
		"test_password",
		&csrf_token,
		Some(&csrf_token),
	);

	// Act
	let response = router.handle(request).await;

	// Assert
	assert!(response.is_ok(), "Router should not return routing error");
	let response = response.unwrap();
	assert_eq!(
		response.status.as_u16(),
		200,
		"Expected 200 for successful login, got: {}. Body: {}",
		response.status,
		String::from_utf8_lossy(&response.body)
	);

	let login_response: LoginResponse =
		serde_json::from_slice(&response.body).expect("Failed to deserialize LoginResponse");

	// JWT is now set as an HTTP-Only cookie (not in the response body).
	// Verify the Set-Cookie header contains the admin auth token.
	let set_cookie = response
		.headers
		.get("set-cookie")
		.expect("Set-Cookie header should be present on successful login");
	let cookie_str = set_cookie.to_str().expect("Invalid Set-Cookie header");
	assert!(
		cookie_str.contains("reinhardt_admin_token="),
		"Set-Cookie should contain reinhardt_admin_token, got: {}",
		cookie_str
	);
	assert!(
		cookie_str.contains("HttpOnly"),
		"Admin auth cookie must be HttpOnly"
	);

	// Token field in response body is intentionally empty (security improvement).
	assert_eq!(login_response.username, "test_staff");
}

// ============================================================
// Test 8: JWT token format (header.payload.signature)
// ============================================================

#[rstest]
#[tokio::test]
async fn test_admin_login_jwt_token_format(#[future] shared_db_pool: (sqlx::PgPool, String)) {
	// Arrange
	let (pool, _) = shared_db_pool.await;
	let router = build_login_router(pool, true).await;
	let csrf_token = generate_csrf_token();
	let request = make_login_request(
		"test_staff",
		"test_password",
		&csrf_token,
		Some(&csrf_token),
	);

	// Act
	let response = router.handle(request).await;

	// Assert
	assert!(response.is_ok(), "Router should not return routing error");
	let response = response.unwrap();
	assert_eq!(response.status.as_u16(), 200);

	let _login_response: LoginResponse =
		serde_json::from_slice(&response.body).expect("Failed to deserialize LoginResponse");

	// JWT is now delivered via Set-Cookie header (HTTP-Only cookie).
	let set_cookie = response
		.headers
		.get("set-cookie")
		.expect("Set-Cookie header should be present on successful login");
	let cookie_str = set_cookie.to_str().expect("Invalid Set-Cookie header");

	// Extract token value from "reinhardt_admin_token=<token>; HttpOnly; ..."
	let token = cookie_str
		.strip_prefix("reinhardt_admin_token=")
		.expect("Cookie should start with reinhardt_admin_token=")
		.split(';')
		.next()
		.expect("Cookie should have a value");

	let parts: Vec<&str> = token.split('.').collect();
	assert_eq!(
		parts.len(),
		3,
		"JWT token should have 3 parts (header.payload.signature), got {} parts: {:?}",
		parts.len(),
		parts
	);
	for (i, part) in parts.iter().enumerate() {
		assert!(!part.is_empty(), "JWT part {} should not be empty", i);
	}
}

// ============================================================
// Test 9: Response contains correct user info
// ============================================================

#[rstest]
#[tokio::test]
async fn test_admin_login_response_contains_correct_user_info(
	#[future] shared_db_pool: (sqlx::PgPool, String),
) {
	// Arrange
	let (pool, _) = shared_db_pool.await;
	let router = build_login_router(pool, true).await;
	let csrf_token = generate_csrf_token();
	let request = make_login_request(
		"test_staff",
		"test_password",
		&csrf_token,
		Some(&csrf_token),
	);

	// Act
	let response = router.handle(request).await;

	// Assert
	assert!(response.is_ok(), "Router should not return routing error");
	let response = response.unwrap();
	assert_eq!(response.status.as_u16(), 200);

	let login_response: LoginResponse =
		serde_json::from_slice(&response.body).expect("Failed to deserialize LoginResponse");

	assert_eq!(login_response.username, "test_staff");
	assert_eq!(login_response.user_id, TEST_USER_UUID);
	assert!(login_response.is_staff, "User should be staff");
	assert!(
		!login_response.is_superuser,
		"User should not be superuser (inserted as is_superuser=false)"
	);
}

// ============================================================
// Test 10: Empty username returns error (not panic)
// ============================================================

#[rstest]
#[tokio::test]
async fn test_admin_login_empty_username(#[future] shared_db_pool: (sqlx::PgPool, String)) {
	// Arrange
	let (pool, _) = shared_db_pool.await;
	let router = build_login_router(pool, true).await;
	let csrf_token = generate_csrf_token();
	let request = make_login_request("", "test_password", &csrf_token, Some(&csrf_token));

	// Act
	let response = router.handle(request).await;

	// Assert: Should return an error status, not panic
	assert!(response.is_ok(), "Router should not return routing error");
	let response = response.unwrap();
	let status_code = response.status.as_u16();
	assert!(
		status_code == 401 || status_code == 400 || status_code == 500,
		"Expected error status for empty username, got: {}. Body: {}",
		status_code,
		String::from_utf8_lossy(&response.body)
	);
}

// ============================================================
// Test 11: Empty password returns error (not panic)
// ============================================================

#[rstest]
#[tokio::test]
async fn test_admin_login_empty_password(#[future] shared_db_pool: (sqlx::PgPool, String)) {
	// Arrange
	let (pool, _) = shared_db_pool.await;
	let router = build_login_router(pool, true).await;
	let csrf_token = generate_csrf_token();
	let request = make_login_request("test_staff", "", &csrf_token, Some(&csrf_token));

	// Act
	let response = router.handle(request).await;

	// Assert: Should return an error status, not panic
	assert!(response.is_ok(), "Router should not return routing error");
	let response = response.unwrap();
	let status_code = response.status.as_u16();
	assert!(
		status_code == 401 || status_code == 400 || status_code == 500,
		"Expected error status for empty password, got: {}. Body: {}",
		status_code,
		String::from_utf8_lossy(&response.body)
	);
}
