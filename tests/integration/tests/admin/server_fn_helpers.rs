//! Shared test helpers for admin server function integration tests
//!
//! Provides helper functions to construct `ServerFnRequest`, `AuthUser<DefaultUser>`,
//! and a permission-granting ModelAdmin for testing server functions.

use reinhardt_admin::core::{AdminDatabase, AdminSite, AdminUser, ModelAdmin};
use reinhardt_admin::server::AdminDefaultUser;
use reinhardt_auth::AuthUser;
use reinhardt_db::backends::connection::DatabaseConnection as BackendsConnection;
use reinhardt_db::backends::dialect::PostgresBackend;
use reinhardt_db::orm::connection::{DatabaseBackend, DatabaseConnection};
use reinhardt_http::AuthState;
use reinhardt_pages::server_fn::ServerFnRequest;
use reinhardt_test::fixtures::shared_postgres::shared_db_pool;
use rstest::*;
use sqlx::Executor;
use std::sync::Arc;
use uuid::Uuid;

/// Fixed CSRF token value for testing.
/// Both the request body and the cookie must use this same value.
pub const TEST_CSRF_TOKEN: &str = "test-csrf-token-for-integration-tests";

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

/// Creates an `AuthUser<AdminDefaultUser>` with staff privileges for testing.
pub fn make_auth_user() -> AuthUser<AdminDefaultUser> {
	AuthUser(make_staff_user())
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
