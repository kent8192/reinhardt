//! Shared test helpers for admin server function integration tests
//!
//! Provides helper functions to construct `ServerFnRequest`, `AdminAuthenticatedUser`,
//! and a permission-granting ModelAdmin for testing server functions.

use reinhardt_admin::core::{AdminDatabase, AdminSite, AdminUser, ModelAdmin};
use reinhardt_admin::server::{AdminAuthenticatedUser, AdminDefaultUser};
use reinhardt_db::backends::connection::DatabaseConnection as BackendsConnection;
use reinhardt_db::backends::dialect::PostgresBackend;
use reinhardt_db::orm::connection::{DatabaseBackend, DatabaseConnection};
use reinhardt_di::{InjectionContext, SingletonScope};
use reinhardt_http::AuthState;
use reinhardt_pages::server_fn::ServerFnRequest;
use reinhardt_test::fixtures::shared_postgres::shared_db_pool;
use reinhardt_urls::routers::ServerRouter;
use rstest::*;
use sqlx::Executor;
use std::sync::Arc;
use uuid::Uuid;

/// Fixed CSRF token value for testing.
/// Both the request body and the cookie must use this same value.
pub const TEST_CSRF_TOKEN: &str = "test-csrf-token-for-integration-tests";

/// Fixed UUID for the test staff user in E2E tests.
/// Matches the row inserted into auth_user by `e2e_router_context`.
pub const TEST_USER_UUID: &str = "00000000-0000-0000-0000-000000000001";

/// Creates a `ServerFnRequest` with staff authentication and CSRF cookie.
///
/// The request has:
/// - `AuthState::authenticated` with is_admin=true, is_active=true
/// - `Cookie` header containing `__csrf_token={TEST_CSRF_TOKEN}`
pub fn make_staff_request() -> ServerFnRequest {
	let request = reinhardt_http::Request::builder()
		.uri("/admin/test")
		.header("cookie", format!("__csrf_token={}", TEST_CSRF_TOKEN))
		.build()
		.expect("Failed to build test request");

	request
		.extensions
		.insert(AuthState::authenticated("test-staff-user", true, true));

	ServerFnRequest(Arc::new(request))
}

/// Creates an `AdminDefaultUser` with staff privileges for testing.
pub fn make_staff_user() -> AdminDefaultUser {
	AdminDefaultUser {
		id: Uuid::new_v4(),
		username: "test_staff".to_string(),
		email: "staff@test.example".to_string(),
		first_name: "Test".to_string(),
		last_name: "Staff".to_string(),
		password_hash: None,
		last_login: None,
		is_active: true,
		is_staff: true,
		is_superuser: false,
		date_joined: chrono::Utc::now(),
		user_permissions: vec![],
		groups: vec![],
	}
}

/// Creates an `AdminAuthenticatedUser` with staff privileges for testing.
///
/// Wraps the staff user in `Arc<dyn AdminUser>` to match the type-erased
/// authentication used by admin server functions.
pub fn make_auth_user() -> AdminAuthenticatedUser {
	AdminAuthenticatedUser(Arc::new(make_staff_user()))
}

/// A ModelAdmin implementation that grants all permissions.
///
/// Unlike `ModelAdminConfig` (which inherits the trait's default deny-all behavior),
/// this implementation explicitly returns `true` for all permission methods.
pub struct AllPermissionsModelAdmin {
	model_name: String,
	table_name: String,
	pk_field: String,
	list_display: Vec<String>,
	list_filter: Vec<String>,
	search_fields: Vec<String>,
}

impl AllPermissionsModelAdmin {
	/// Creates a new instance configured for the standard test model.
	pub fn test_model(table_name: &str) -> Self {
		Self {
			model_name: "TestModel".to_string(),
			table_name: table_name.to_string(),
			pk_field: "id".to_string(),
			list_display: vec![
				"id".to_string(),
				"name".to_string(),
				"status".to_string(),
				"created_at".to_string(),
			],
			list_filter: vec!["status".to_string()],
			search_fields: vec!["name".to_string(), "description".to_string()],
		}
	}
}

#[async_trait::async_trait]
impl ModelAdmin for AllPermissionsModelAdmin {
	fn model_name(&self) -> &str {
		&self.model_name
	}

	fn table_name(&self) -> &str {
		&self.table_name
	}

	fn pk_field(&self) -> &str {
		&self.pk_field
	}

	fn list_display(&self) -> Vec<&str> {
		self.list_display.iter().map(|s| s.as_str()).collect()
	}

	fn list_filter(&self) -> Vec<&str> {
		self.list_filter.iter().map(|s| s.as_str()).collect()
	}

	fn search_fields(&self) -> Vec<&str> {
		self.search_fields.iter().map(|s| s.as_str()).collect()
	}

	fn fields(&self) -> Option<Vec<&str>> {
		// Return all writable fields (used by validate_mutation_data)
		Some(vec!["id", "name", "status", "description", "created_at"])
	}

	async fn has_view_permission(&self, _user: &dyn AdminUser) -> bool {
		true
	}

	async fn has_add_permission(&self, _user: &dyn AdminUser) -> bool {
		true
	}

	async fn has_change_permission(&self, _user: &dyn AdminUser) -> bool {
		true
	}

	async fn has_delete_permission(&self, _user: &dyn AdminUser) -> bool {
		true
	}
}

/// Composite fixture providing AdminSite + AdminDatabase + test table for server function tests.
///
/// Creates a real PostgreSQL table with columns (id, name, status, description, created_at)
/// and registers an `AllPermissionsModelAdmin` that grants all permissions.
/// Both the table and AdminDatabase use the SAME database connection pool.
#[fixture]
pub async fn server_fn_context(
	#[future] shared_db_pool: (sqlx::PgPool, String),
) -> (Arc<AdminSite>, Arc<AdminDatabase>) {
	let (pool, _) = shared_db_pool.await;

	// Create the test_models table
	pool.execute(
		"CREATE TABLE IF NOT EXISTS test_models (
			id SERIAL PRIMARY KEY,
			name VARCHAR(255) NOT NULL,
			status VARCHAR(50) DEFAULT 'active',
			description TEXT,
			created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
		)",
	)
	.await
	.expect("Failed to create test_models table");

	// Truncate any leftover data from previous test runs
	pool.execute("TRUNCATE TABLE test_models RESTART IDENTITY CASCADE")
		.await
		.expect("Failed to truncate test_models table");

	// Create AdminDatabase from the SAME pool
	let backend = Arc::new(PostgresBackend::new(pool));
	let backends_conn = BackendsConnection::new(backend);
	let connection = DatabaseConnection::new(DatabaseBackend::Postgres, backends_conn);
	let db = Arc::new(AdminDatabase::new(connection));

	// Create AdminSite and register with all permissions
	let site = Arc::new(AdminSite::new("Test Admin Site"));
	let admin = AllPermissionsModelAdmin::test_model("test_models");
	site.register("TestModel", admin)
		.expect("Failed to register TestModel");

	(site, db)
}

// ==================== E2E Test Infrastructure ====================

/// SQL to create the auth_user table required by `AuthUser<AdminDefaultUser>::inject()`.
///
/// The ORM generates `SELECT * FROM auth_user WHERE id = $1` and deserializes ALL columns
/// into `AdminDefaultUser`. Every field in the struct must have a matching column.
/// Note: `user_permissions` and `groups` use `TEXT` (not `TEXT[]`) because the ORM
/// row-mapping uses JSON deserialization for Vec<String> fields.
const AUTH_USER_TABLE_SQL: &str = "CREATE TABLE IF NOT EXISTS auth_user (
	id UUID PRIMARY KEY,
	username VARCHAR(150) NOT NULL,
	email VARCHAR(254) NOT NULL DEFAULT '',
	first_name VARCHAR(150) NOT NULL DEFAULT '',
	last_name VARCHAR(150) NOT NULL DEFAULT '',
	password_hash TEXT,
	last_login TIMESTAMP WITH TIME ZONE,
	is_active BOOLEAN NOT NULL DEFAULT TRUE,
	is_staff BOOLEAN NOT NULL DEFAULT FALSE,
	is_superuser BOOLEAN NOT NULL DEFAULT FALSE,
	date_joined TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
	user_permissions TEXT NOT NULL DEFAULT '[]',
	groups TEXT NOT NULL DEFAULT '[]'
)";

/// Composite fixture providing a fully-wired `ServerRouter` for E2E tests.
///
/// Unlike `server_fn_context` (which provides raw dependencies for direct handler calls),
/// this fixture builds a complete `ServerRouter` with:
/// - Admin routes mounted at `/admin/`
/// - DI registrations applied to singleton scope (AdminSite, DatabaseConnection)
/// - InjectionContext attached to the router
/// - auth_user table with a test staff user row
///
/// This exercises the full pipeline: HTTP request → route resolution → DI fork → Injectable::inject().
#[fixture]
pub async fn e2e_router_context(
	#[future] shared_db_pool: (sqlx::PgPool, String),
) -> (ServerRouter, Arc<AdminDatabase>) {
	use reinhardt_admin::core::admin_routes_with_di_deferred;

	let (pool, _) = shared_db_pool.await;

	// Create test_models table (same as server_fn_context)
	pool.execute(
		"CREATE TABLE IF NOT EXISTS test_models (
			id SERIAL PRIMARY KEY,
			name VARCHAR(255) NOT NULL,
			status VARCHAR(50) DEFAULT 'active',
			description TEXT,
			created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
		)",
	)
	.await
	.expect("Failed to create test_models table");

	pool.execute("TRUNCATE TABLE test_models RESTART IDENTITY CASCADE")
		.await
		.expect("Failed to truncate test_models table");

	// Create auth_user table for AuthUser::inject() DB lookup.
	// DROP and re-create to ensure schema matches AdminDefaultUser fields exactly.
	pool.execute("DROP TABLE IF EXISTS auth_user CASCADE")
		.await
		.expect("Failed to drop auth_user table");
	pool.execute(AUTH_USER_TABLE_SQL)
		.await
		.expect("Failed to create auth_user table");

	// Insert test staff user (upsert to avoid conflicts across test runs)
	pool.execute(
		sqlx::query(
			"INSERT INTO auth_user (id, username, email, is_active, is_staff, is_superuser, date_joined)
			 VALUES ($1, 'test_staff', 'staff@test.example', true, true, false, NOW())
			 ON CONFLICT (id) DO UPDATE SET is_staff = true, is_active = true",
		)
		.bind(
			Uuid::parse_str(TEST_USER_UUID).expect("Invalid TEST_USER_UUID"),
		),
	)
	.await
	.expect("Failed to insert test staff user");

	// Build DatabaseConnection (shared between AdminDatabase and AuthUser injection)
	let backend = Arc::new(PostgresBackend::new(pool));
	let backends_conn = BackendsConnection::new(backend);
	let connection = DatabaseConnection::new(DatabaseBackend::Postgres, backends_conn);
	let db_conn = Arc::new(connection);

	// Build AdminDatabase for test data setup
	let admin_db = Arc::new(AdminDatabase::new((*db_conn).clone()));

	// Build AdminSite and register test model
	let site = Arc::new(AdminSite::new("E2E Test Admin"));
	let admin = AllPermissionsModelAdmin::test_model("test_models");
	site.register("TestModel", admin)
		.expect("Failed to register TestModel");

	// Build admin router with deferred DI
	let (admin_router, admin_di) = admin_routes_with_di_deferred(site);

	// Build the complete router using UnifiedRouter API.
	// Pre-seed singleton scope with DatabaseConnection so get_singleton() finds it.
	let singleton = Arc::new(SingletonScope::new());
	singleton.set_arc(db_conn);
	let di_ctx = Arc::new(InjectionContext::builder(singleton).build());

	let router = reinhardt_urls::routers::UnifiedRouter::new()
		.with_di_context(di_ctx)
		.mount("/admin/", admin_router)
		.with_di_registrations(admin_di)
		.into_server();

	(router, admin_db)
}

/// Builds an HTTP POST request suitable for E2E server function tests.
///
/// Includes:
/// - `Content-Type: application/json`
/// - `Cookie: __csrf_token={TEST_CSRF_TOKEN}`
/// - `AuthState::authenticated` in request extensions (staff user)
/// - JSON-serialized body
pub fn make_e2e_request(path: &str, body: serde_json::Value) -> reinhardt_http::Request {
	let body_bytes = serde_json::to_vec(&body).expect("Failed to serialize request body");

	let request = reinhardt_http::Request::builder()
		.method(hyper::Method::POST)
		.uri(path)
		.header("content-type", "application/json")
		.header("cookie", format!("__csrf_token={}", TEST_CSRF_TOKEN))
		.body(hyper::body::Bytes::from(body_bytes))
		.build()
		.expect("Failed to build E2E request");

	request
		.extensions
		.insert(AuthState::authenticated(TEST_USER_UUID, true, true));

	request
}

/// Builds an HTTP POST request without CSRF cookie for testing CSRF rejection.
pub fn make_e2e_request_no_csrf(path: &str, body: serde_json::Value) -> reinhardt_http::Request {
	let body_bytes = serde_json::to_vec(&body).expect("Failed to serialize request body");

	let request = reinhardt_http::Request::builder()
		.method(hyper::Method::POST)
		.uri(path)
		.header("content-type", "application/json")
		.body(hyper::body::Bytes::from(body_bytes))
		.build()
		.expect("Failed to build E2E request");

	request
		.extensions
		.insert(AuthState::authenticated(TEST_USER_UUID, true, true));

	request
}

/// Builds an HTTP POST request with a mismatched CSRF cookie.
pub fn make_e2e_request_wrong_csrf(path: &str, body: serde_json::Value) -> reinhardt_http::Request {
	let body_bytes = serde_json::to_vec(&body).expect("Failed to serialize request body");

	let request = reinhardt_http::Request::builder()
		.method(hyper::Method::POST)
		.uri(path)
		.header("content-type", "application/json")
		.header("cookie", "__csrf_token=wrong-token-value")
		.body(hyper::body::Bytes::from(body_bytes))
		.build()
		.expect("Failed to build E2E request");

	request
		.extensions
		.insert(AuthState::authenticated(TEST_USER_UUID, true, true));

	request
}

/// Builds an HTTP POST request without authentication for testing auth rejection.
pub fn make_e2e_request_no_auth(path: &str, body: serde_json::Value) -> reinhardt_http::Request {
	let body_bytes = serde_json::to_vec(&body).expect("Failed to serialize request body");

	reinhardt_http::Request::builder()
		.method(hyper::Method::POST)
		.uri(path)
		.header("content-type", "application/json")
		.header("cookie", format!("__csrf_token={}", TEST_CSRF_TOKEN))
		.body(hyper::body::Bytes::from(body_bytes))
		.build()
		.expect("Failed to build E2E request")
}
