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
use reinhardt_query::prelude::{
	Alias, ColumnDef, Expr, PostgresQueryBuilder, Query, QueryStatementBuilder,
};
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

	/// Creates a new instance configured for a UUID primary key test model.
	pub fn uuid_pk_model(table_name: &str) -> Self {
		Self {
			model_name: "UuidModel".to_string(),
			table_name: table_name.to_string(),
			pk_field: "id".to_string(),
			list_display: vec!["id".to_string(), "name".to_string(), "status".to_string()],
			list_filter: vec!["status".to_string()],
			search_fields: vec!["name".to_string()],
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

/// Builds the CREATE TABLE SQL for the standard `test_models` table using SeaQuery.
fn build_test_models_create_table_sql() -> String {
	Query::create_table()
		.table(Alias::new("test_models"))
		.if_not_exists()
		.col(
			ColumnDef::new(Alias::new("id"))
				.integer()
				.not_null(true)
				.auto_increment(true)
				.primary_key(true),
		)
		.col(
			ColumnDef::new(Alias::new("name"))
				.string_len(255)
				.not_null(true),
		)
		.col(
			ColumnDef::new(Alias::new("status"))
				.string_len(50)
				.default("active".into()),
		)
		.col(ColumnDef::new(Alias::new("description")).text())
		.col(
			ColumnDef::new(Alias::new("created_at"))
				.timestamp_with_time_zone()
				.default(Expr::current_timestamp().into()),
		)
		.to_string(PostgresQueryBuilder::new())
}

/// Builds the TRUNCATE TABLE SQL for the standard `test_models` table using SeaQuery.
fn build_test_models_truncate_sql() -> String {
	Query::truncate_table()
		.table(Alias::new("test_models"))
		.restart_identity()
		.cascade()
		.to_string(PostgresQueryBuilder::new())
}

/// Creates the test_models table and truncates any leftover data.
async fn setup_test_models_table(pool: &sqlx::PgPool) {
	pool.execute(build_test_models_create_table_sql().as_str())
		.await
		.expect("Failed to create test_models table");

	pool.execute(build_test_models_truncate_sql().as_str())
		.await
		.expect("Failed to truncate test_models table");
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

	setup_test_models_table(&pool).await;

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

/// Builds the CREATE TABLE SQL for the `auth_user` table using SeaQuery.
///
/// The ORM generates `SELECT * FROM auth_user WHERE id = $1` and deserializes ALL columns
/// into `AdminDefaultUser`. Every field in the struct must have a matching column.
/// Note: `user_permissions` and `groups` use `TEXT` (not `TEXT[]`) because the ORM
/// row-mapping uses JSON deserialization for `Vec<String>` fields.
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
	setup_test_models_table(&pool).await;

	// Create auth_user table for AuthUser::inject() DB lookup.
	// DROP and re-create to ensure schema matches AdminDefaultUser fields exactly.
	let drop_sql = Query::drop_table()
		.table(Alias::new("auth_user"))
		.if_exists()
		.cascade()
		.to_string(PostgresQueryBuilder::new());
	pool.execute(drop_sql.as_str())
		.await
		.expect("Failed to drop auth_user table");

	let create_auth_sql = build_auth_user_create_table_sql();
	pool.execute(create_auth_sql.as_str())
		.await
		.expect("Failed to create auth_user table");

	// Insert test staff user (upsert to avoid conflicts across test runs)
	pool.execute(
		sqlx::query(&format!(
			"INSERT INTO auth_user (id, username, email, is_active, is_staff, is_superuser, date_joined) \
				 VALUES ($1, 'test_staff', 'staff@test.example', true, true, false, NOW()) \
				 ON CONFLICT (id) DO UPDATE SET is_staff = true, is_active = true"
		))
		.bind(Uuid::parse_str(TEST_USER_UUID).expect("Invalid TEST_USER_UUID")),
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

/// Composite fixture providing a `ServerRouter` WITHOUT `DatabaseConnection`.
///
/// Intentionally omits `DatabaseConnection` from the singleton scope to test
/// error behavior when DI dependencies are missing. All admin routes and
/// `AdminUserLoader` are still registered via `admin_routes_with_di_deferred()`.
///
/// Unlike `e2e_router_context`, this fixture:
/// - Does NOT require a database pool
/// - Does NOT create any tables
/// - Returns only `ServerRouter` (no `AdminDatabase`)
#[fixture]
pub async fn e2e_router_context_no_db() -> ServerRouter {
	use reinhardt_admin::core::admin_routes_with_di_deferred;

	// Build AdminSite and register test model
	let site = Arc::new(AdminSite::new("E2E Test Admin (No DB)"));
	let admin = AllPermissionsModelAdmin::test_model("test_models");
	site.register("TestModel", admin)
		.expect("Failed to register TestModel");

	// Build admin router with deferred DI
	let (admin_router, admin_di) = admin_routes_with_di_deferred(site);

	// Build singleton scope WITHOUT DatabaseConnection
	let singleton = Arc::new(SingletonScope::new());
	let di_ctx = Arc::new(InjectionContext::builder(singleton).build());

	reinhardt_urls::routers::UnifiedRouter::new()
		.with_di_context(di_ctx)
		.mount("/admin/", admin_router)
		.with_di_registrations(admin_di)
		.into_server()
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

/// Composite fixture providing AdminSite + AdminDatabase + PgPool with a UUID primary key table.
///
/// Creates a PostgreSQL table with a UUID PK column and registers an
/// `AllPermissionsModelAdmin` configured for UUID lookups.
/// Returns the PgPool alongside AdminSite and AdminDatabase so tests can
/// insert records with UUID PKs directly via SQL.
#[fixture]
pub async fn uuid_pk_context(
	#[future] shared_db_pool: (sqlx::PgPool, String),
) -> (Arc<AdminSite>, Arc<AdminDatabase>, sqlx::PgPool) {
	let (pool, _) = shared_db_pool.await;

	// Create a table with UUID primary key using SeaQuery
	let create_uuid_table_sql = Query::create_table()
		.table(Alias::new("uuid_test_models"))
		.if_not_exists()
		.col(
			ColumnDef::new(Alias::new("id"))
				.uuid()
				.not_null(true)
				.primary_key(true)
				.default(Expr::cust("gen_random_uuid()").into()),
		)
		.col(
			ColumnDef::new(Alias::new("name"))
				.string_len(255)
				.not_null(true),
		)
		.col(
			ColumnDef::new(Alias::new("status"))
				.string_len(50)
				.default("active".into()),
		)
		.to_string(PostgresQueryBuilder::new());
	pool.execute(create_uuid_table_sql.as_str())
		.await
		.expect("Failed to create uuid_test_models table");

	let truncate_uuid_sql = Query::truncate_table()
		.table(Alias::new("uuid_test_models"))
		.cascade()
		.to_string(PostgresQueryBuilder::new());
	pool.execute(truncate_uuid_sql.as_str())
		.await
		.expect("Failed to truncate uuid_test_models table");

	// Register the UUID field type in the migration registry so that
	// parse_pk_value can look up the correct type at runtime.
	use reinhardt_db::migrations::FieldType;
	use reinhardt_db::migrations::model_registry::{FieldMetadata, ModelMetadata, global_registry};
	let mut model_meta = ModelMetadata::new("test", "UuidModel", "uuid_test_models");
	model_meta
		.fields
		.insert("id".to_string(), FieldMetadata::new(FieldType::Uuid));
	model_meta.fields.insert(
		"name".to_string(),
		FieldMetadata::new(FieldType::VarChar(255)),
	);
	model_meta.fields.insert(
		"status".to_string(),
		FieldMetadata::new(FieldType::VarChar(50)),
	);
	global_registry().register_model(model_meta);

	let pool_clone = pool.clone();
	let backend = Arc::new(PostgresBackend::new(pool));
	let backends_conn = BackendsConnection::new(backend);
	let connection = DatabaseConnection::new(DatabaseBackend::Postgres, backends_conn);
	let db = Arc::new(AdminDatabase::new(connection));

	let site = Arc::new(AdminSite::new("UUID Test Admin Site"));
	let admin = AllPermissionsModelAdmin::uuid_pk_model("uuid_test_models");
	site.register("UuidModel", admin)
		.expect("Failed to register UuidModel");

	(site, db, pool_clone)
}

// ==================== Permission Denial Test Infrastructure ====================

/// A ModelAdmin implementation that denies ALL permissions.
///
/// Used for testing that server functions correctly reject unauthorized operations.
/// All `has_*_permission` methods return `false`.
pub struct DenyAllPermissionsModelAdmin {
	model_name: String,
	table_name: String,
	pk_field: String,
	list_display: Vec<String>,
	list_filter: Vec<String>,
	search_fields: Vec<String>,
}

impl DenyAllPermissionsModelAdmin {
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
impl ModelAdmin for DenyAllPermissionsModelAdmin {
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
		Some(vec!["id", "name", "status", "description", "created_at"])
	}

	async fn has_view_permission(&self, _user: &dyn AdminUser) -> bool {
		false
	}

	async fn has_add_permission(&self, _user: &dyn AdminUser) -> bool {
		false
	}

	async fn has_change_permission(&self, _user: &dyn AdminUser) -> bool {
		false
	}

	async fn has_delete_permission(&self, _user: &dyn AdminUser) -> bool {
		false
	}
}

/// A ModelAdmin implementation that grants only view permission.
///
/// Used for testing partial permission scenarios where a user can
/// view/export records but cannot create, update, or delete them.
pub struct ViewOnlyModelAdmin {
	model_name: String,
	table_name: String,
	pk_field: String,
	list_display: Vec<String>,
	list_filter: Vec<String>,
	search_fields: Vec<String>,
}

impl ViewOnlyModelAdmin {
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
impl ModelAdmin for ViewOnlyModelAdmin {
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
		Some(vec!["id", "name", "status", "description", "created_at"])
	}

	async fn has_view_permission(&self, _user: &dyn AdminUser) -> bool {
		true
	}

	async fn has_add_permission(&self, _user: &dyn AdminUser) -> bool {
		false
	}

	async fn has_change_permission(&self, _user: &dyn AdminUser) -> bool {
		false
	}

	async fn has_delete_permission(&self, _user: &dyn AdminUser) -> bool {
		false
	}
}

/// Composite fixture providing AdminSite + AdminDatabase with ALL permissions denied.
///
/// Same structure as `server_fn_context` but registers `DenyAllPermissionsModelAdmin`
/// instead. Used for testing permission rejection at the server function level.
#[fixture]
pub async fn server_fn_context_deny_all(
	#[future] shared_db_pool: (sqlx::PgPool, String),
) -> (Arc<AdminSite>, Arc<AdminDatabase>) {
	let (pool, _) = shared_db_pool.await;

	setup_test_models_table(&pool).await;

	let backend = Arc::new(PostgresBackend::new(pool));
	let backends_conn = BackendsConnection::new(backend);
	let connection = DatabaseConnection::new(DatabaseBackend::Postgres, backends_conn);
	let db = Arc::new(AdminDatabase::new(connection));

	let site = Arc::new(AdminSite::new("Deny All Test Site"));
	let admin = DenyAllPermissionsModelAdmin::test_model("test_models");
	site.register("TestModel", admin)
		.expect("Failed to register TestModel");

	(site, db)
}

/// Composite fixture providing AdminSite + AdminDatabase with view-only permissions.
///
/// Registers `ViewOnlyModelAdmin` that grants only view permission.
/// Used for testing partial permission scenarios.
#[fixture]
pub async fn server_fn_context_view_only(
	#[future] shared_db_pool: (sqlx::PgPool, String),
) -> (Arc<AdminSite>, Arc<AdminDatabase>) {
	let (pool, _) = shared_db_pool.await;

	setup_test_models_table(&pool).await;

	// Seed one record so view tests have data to read
	let seed_sql = Query::insert()
		.into_table(Alias::new("test_models"))
		.columns([Alias::new("name"), Alias::new("status")])
		.values_panic(["Seeded Record".into(), "active".into()])
		.to_string(PostgresQueryBuilder::new());
	pool.execute(seed_sql.as_str())
		.await
		.expect("Failed to seed test record");

	let backend = Arc::new(PostgresBackend::new(pool));
	let backends_conn = BackendsConnection::new(backend);
	let connection = DatabaseConnection::new(DatabaseBackend::Postgres, backends_conn);
	let db = Arc::new(AdminDatabase::new(connection));

	let site = Arc::new(AdminSite::new("View Only Test Site"));
	let admin = ViewOnlyModelAdmin::test_model("test_models");
	site.register("TestModel", admin)
		.expect("Failed to register TestModel");

	(site, db)
}
