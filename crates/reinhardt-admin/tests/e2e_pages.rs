//! E2E browser tests for admin WASM frontend pages.
//!
//! Each test gets:
//! - An isolated PostgreSQL database (via testcontainers shared Postgres)
//! - An isolated headless Chrome instance (via testcontainers Docker container)
//! - A dedicated HTTP server on a random port
//!
//! All three are fully isolated, enabling safe **parallel** test execution.
//!
//! # Prerequisites
//!
//! - Docker daemon running (for both Postgres and Chrome containers)
//! - WASM built for full SPA tests (tests are skipped when WASM is not built)
//!
//! # Running
//!
//! ```sh
//! cargo test --package reinhardt-admin --test e2e_pages -- --nocapture
//! ```

#![cfg(not(target_arch = "wasm32"))]

use reinhardt_admin::core::{
	AdminDatabase, AdminSite, AdminUser, ModelAdmin, admin_routes_with_di, admin_static_routes,
};
use reinhardt_auth::{Argon2Hasher, PasswordHasher};
use reinhardt_db::backends::connection::DatabaseConnection as BackendsConnection;
use reinhardt_db::backends::dialect::PostgresBackend;
use reinhardt_db::orm::connection::{DatabaseBackend, DatabaseConnection};
use reinhardt_di::{InjectionContext, SingletonScope};
use reinhardt_query::prelude::{
	ColumnDef, Expr, OnConflict, PostgresQueryBuilder, Query, QueryBuilder, QueryStatementBuilder,
	Value,
};
use reinhardt_test::fixtures::shared_postgres::shared_db_pool;
use reinhardt_test::fixtures::wasm::e2e_cdp::*;
use rstest::*;
use std::net::SocketAddr;
use std::sync::Arc;

// ============================================================================
// SeaQuery <-> sqlx bridge helpers
// ============================================================================
//
// `reinhardt-query` produces a `(sql, Values)` pair from `build_*` calls;
// sqlx requires each value to be bound individually via `.bind()`. The two
// helpers below wrap that boilerplate so fixture code can express its DDL
// and DML through SeaQuery (per project convention) without sprinkling
// `.bind()` chains in every call site.

/// Bind a single SeaQuery `Value` to a sqlx Postgres query.
///
/// Pattern-matches the `Value` variants used by this fixture (Bool, String,
/// Uuid). Other variants are not currently used and trigger a panic; extend
/// the match arms when new types become necessary.
fn bind_pg_value<'q>(
	query: sqlx::query::Query<'q, sqlx::Postgres, sqlx::postgres::PgArguments>,
	value: Value,
) -> sqlx::query::Query<'q, sqlx::Postgres, sqlx::postgres::PgArguments> {
	match value {
		Value::Bool(v) => query.bind(v),
		Value::String(v) => query.bind(v.map(|b| *b)),
		Value::Uuid(v) => query.bind(v.map(|b| *b)),
		other => panic!("bind_pg_value: unsupported Value variant {:?}", other),
	}
}

/// Execute a SeaQuery DML statement (e.g., INSERT) against a Postgres pool.
///
/// Binds all values from `Values` to the prepared statement in order.
async fn execute_dml(pool: &sqlx::PgPool, sql: &str, values: Vec<Value>, context: &str) {
	let mut q = sqlx::query(sql);
	for v in values {
		q = bind_pg_value(q, v);
	}
	q.execute(pool)
		.await
		.unwrap_or_else(|e| panic!("Failed to execute DML ({}): {}", context, e));
}

/// Execute a SeaQuery DDL statement (CREATE/DROP/TRUNCATE) against a Postgres pool.
///
/// DDL statements have no bind parameters; the generated SQL is executed verbatim.
async fn execute_ddl(pool: &sqlx::PgPool, sql: &str, context: &str) {
	sqlx::query(sql)
		.execute(pool)
		.await
		.unwrap_or_else(|e| panic!("Failed to execute DDL ({}): {}", context, e));
}

// ============================================================================
// Constants
// ============================================================================

const TEST_USERNAME: &str = "test_staff";
const TEST_PASSWORD: &str = "e2e-test-password-2026";
const TEST_USER_UUID: &str = "00000000-0000-0000-0000-000000000001";
const JWT_SECRET: &[u8] = b"e2e-test-jwt-secret-at-least-32-bytes!!";

// Non-staff user credentials used by `test_dashboard_non_staff_user_blocked`.
const NON_STAFF_USERNAME: &str = "test_non_staff";
const NON_STAFF_PASSWORD: &str = "e2e-test-non-staff-pw-2026";
const NON_STAFF_UUID: &str = "00000000-0000-0000-0000-000000000002";

// ============================================================================
// AllPermissionsModelAdmin (same pattern as server_fn_helpers.rs)
// ============================================================================

struct AllPermissionsModelAdmin {
	model_name: String,
	table_name: String,
	pk_field: String,
	list_display: Vec<String>,
	list_filter: Vec<String>,
	search_fields: Vec<String>,
}

impl AllPermissionsModelAdmin {
	fn test_model(table_name: &str) -> Self {
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

// ============================================================================
// E2E Test Server (binds 0.0.0.0 for Docker container access)
// ============================================================================

/// A test HTTP server bound to `0.0.0.0` so Docker containers can reach it
/// via `host.docker.internal`.
struct TestServer {
	/// Port the server is listening on.
	port: u16,
	_server_task: tokio::task::JoinHandle<()>,
}

impl TestServer {
	async fn start(router: reinhardt_urls::routers::ServerRouter) -> Self {
		let addr: SocketAddr = "0.0.0.0:0".parse().unwrap();
		let listener = tokio::net::TcpListener::bind(addr)
			.await
			.expect("Failed to bind test server");
		let actual_addr = listener.local_addr().expect("Failed to get local addr");
		let port = actual_addr.port();

		// Release the listener so the HTTP server can re-bind
		drop(listener);

		let coordinator =
			reinhardt_server::ShutdownCoordinator::new(std::time::Duration::from_secs(5));
		let router = Arc::new(router);
		let server_task = tokio::spawn(async move {
			let server = reinhardt_server::HttpServer::new(router);
			let _ = server.listen_with_shutdown(actual_addr, coordinator).await;
		});

		// Wait for the server to start
		tokio::time::sleep(std::time::Duration::from_millis(200)).await;

		Self {
			port,
			_server_task: server_task,
		}
	}

	/// URL reachable from the host (for debugging / non-container access).
	#[allow(dead_code)]
	fn host_url(&self) -> String {
		format!("http://127.0.0.1:{}", self.port)
	}

	/// URL reachable from inside Docker containers.
	///
	/// On macOS/Windows Docker Desktop, `host.docker.internal` resolves automatically.
	/// On Linux (CI), the Docker bridge gateway `172.17.0.1` provides host access.
	fn container_url(&self) -> String {
		let host = if cfg!(target_os = "macos") || cfg!(target_os = "windows") {
			"host.docker.internal"
		} else {
			"172.17.0.1"
		};
		format!("http://{}:{}", host, self.port)
	}
}

impl Drop for TestServer {
	fn drop(&mut self) {
		self._server_task.abort();
	}
}

// ============================================================================
// Composite Fixture: server + browser, fully isolated
// ============================================================================

/// All-in-one E2E context: isolated database, HTTP server, and Chrome browser.
struct E2eContext {
	/// URL for the containerized Chrome to reach the server.
	server_url: String,
	/// The browser page (one per test).
	#[allow(dead_code)]
	browser: CdpBrowser,
	/// The configured admin `site_header` (sourced from `AdminSettings::default()`).
	///
	/// Tests assert against this value rather than hard-coding "Administration",
	/// so changes to the default propagate automatically.
	site_header: String,
	/// Direct pool handle for tests that need additional DB setup
	/// (e.g., inserting a non-staff user).
	#[allow(dead_code)]
	pool: sqlx::PgPool,
	// Hold server and db alive for the test lifetime.
	_server: TestServer,
	_admin_db: Arc<AdminDatabase>,
}

/// Build a fully isolated E2E context with caller-controlled model registration.
///
/// Common setup performed here:
/// 1. Creates `test_models` and `test_models_b` tables (idempotent) and seeds
///    `test_models` with three records.
/// 2. Drops and recreates `auth_user`, inserting one staff user.
/// 3. Constructs `AdminSite`, invokes `register_models` to register zero or more
///    `ModelAdmin` instances, builds the router, and starts the test HTTP server.
///
/// The closure pattern lets tests vary which models are registered while sharing
/// the rest of the setup, avoiding ~80 lines of duplication per fixture.
async fn build_e2e_context<F>(
	pool: sqlx::PgPool,
	browser: CdpBrowser,
	register_models: F,
) -> E2eContext
where
	F: FnOnce(&Arc<AdminSite>),
{
	let builder = PostgresQueryBuilder::new();

	// ---- Database setup ----

	// CREATE TABLE IF NOT EXISTS test_models (...)
	let mut create_test_models = Query::create_table();
	create_test_models
		.table("test_models")
		.if_not_exists()
		.col(
			ColumnDef::new("id")
				.integer()
				.primary_key(true)
				.auto_increment(true)
				.not_null(true),
		)
		.col(ColumnDef::new("name").string_len(255).not_null(true))
		.col(
			ColumnDef::new("status")
				.string_len(50)
				.default(Expr::val("active").into()),
		)
		.col(ColumnDef::new("description").text())
		.col(
			ColumnDef::new("created_at")
				.timestamp_with_time_zone()
				.default(Expr::current_timestamp().into()),
		);
	let sql = create_test_models.to_string(PostgresQueryBuilder);
	execute_ddl(&pool, &sql, "create test_models").await;

	// TRUNCATE TABLE test_models RESTART IDENTITY CASCADE
	let mut truncate_test_models = Query::truncate_table();
	truncate_test_models
		.table("test_models")
		.restart_identity()
		.cascade();
	let sql = truncate_test_models.to_string(PostgresQueryBuilder);
	execute_ddl(&pool, &sql, "truncate test_models").await;

	// Seed test_models with three rows.
	let mut seed_test_models = Query::insert();
	seed_test_models
		.into_table("test_models")
		.columns(["name", "status"])
		.values_panic(["Alice", "active"])
		.values_panic(["Bob", "inactive"])
		.values_panic(["Charlie", "active"]);
	let (sql, values) = builder.build_insert(&seed_test_models);
	execute_dml(&pool, &sql, values.0, "seed test_models").await;

	// CREATE TABLE IF NOT EXISTS test_models_b (...)
	// Created unconditionally because IF NOT EXISTS is harmless for fixtures
	// that don't register a TestModelB.
	let mut create_test_models_b = Query::create_table();
	create_test_models_b
		.table("test_models_b")
		.if_not_exists()
		.col(
			ColumnDef::new("id")
				.integer()
				.primary_key(true)
				.auto_increment(true)
				.not_null(true),
		)
		.col(ColumnDef::new("name").string_len(255).not_null(true));
	let sql = create_test_models_b.to_string(PostgresQueryBuilder);
	execute_ddl(&pool, &sql, "create test_models_b").await;

	let mut truncate_test_models_b = Query::truncate_table();
	truncate_test_models_b
		.table("test_models_b")
		.restart_identity()
		.cascade();
	let sql = truncate_test_models_b.to_string(PostgresQueryBuilder);
	execute_ddl(&pool, &sql, "truncate test_models_b").await;

	// DROP TABLE IF EXISTS auth_user CASCADE
	let mut drop_auth_user = Query::drop_table();
	drop_auth_user.table("auth_user").if_exists().cascade();
	let sql = drop_auth_user.to_string(PostgresQueryBuilder);
	execute_ddl(&pool, &sql, "drop auth_user").await;

	// CREATE TABLE auth_user (...)
	let mut create_auth_user = Query::create_table();
	create_auth_user
		.table("auth_user")
		.col(ColumnDef::new("id").uuid().primary_key(true).not_null(true))
		.col(ColumnDef::new("username").string_len(150).not_null(true))
		.col(
			ColumnDef::new("email")
				.string_len(254)
				.not_null(true)
				.default(Expr::val("").into()),
		)
		.col(
			ColumnDef::new("first_name")
				.string_len(150)
				.not_null(true)
				.default(Expr::val("").into()),
		)
		.col(
			ColumnDef::new("last_name")
				.string_len(150)
				.not_null(true)
				.default(Expr::val("").into()),
		)
		.col(ColumnDef::new("password_hash").text())
		.col(ColumnDef::new("last_login").timestamp_with_time_zone())
		.col(
			ColumnDef::new("is_active")
				.boolean()
				.not_null(true)
				.default(Expr::val(true).into()),
		)
		.col(
			ColumnDef::new("is_staff")
				.boolean()
				.not_null(true)
				.default(Expr::val(false).into()),
		)
		.col(
			ColumnDef::new("is_superuser")
				.boolean()
				.not_null(true)
				.default(Expr::val(false).into()),
		)
		.col(
			ColumnDef::new("date_joined")
				.timestamp_with_time_zone()
				.not_null(true)
				.default(Expr::current_timestamp().into()),
		)
		.col(
			ColumnDef::new("user_permissions")
				.text()
				.not_null(true)
				.default(Expr::val("[]").into()),
		)
		.col(
			ColumnDef::new("groups")
				.text()
				.not_null(true)
				.default(Expr::val("[]").into()),
		);
	let sql = create_auth_user.to_string(PostgresQueryBuilder);
	execute_ddl(&pool, &sql, "create auth_user").await;

	let hasher = Argon2Hasher::new();
	let password_hash = hasher
		.hash(TEST_PASSWORD)
		.expect("Failed to hash test password");

	// INSERT staff user; ON CONFLICT (id) refresh password_hash/is_staff/is_active
	// from the EXCLUDED row (which carries the values we just attempted to insert).
	// `date_joined` is omitted so the column DEFAULT (CURRENT_TIMESTAMP) applies.
	let mut insert_staff = Query::insert();
	insert_staff
		.into_table("auth_user")
		.columns(["id", "username", "password_hash", "is_active", "is_staff"])
		.values(vec![
			Value::Uuid(Some(Box::new(
				uuid::Uuid::parse_str(TEST_USER_UUID).expect("valid TEST_USER_UUID"),
			))),
			Value::String(Some(Box::new(TEST_USERNAME.to_string()))),
			Value::String(Some(Box::new(password_hash.clone()))),
			Value::Bool(Some(true)),
			Value::Bool(Some(true)),
		])
		.expect("staff user value count matches column count")
		.on_conflict(
			OnConflict::column("id")
				.update_columns(["password_hash", "is_staff", "is_active"])
				.to_owned(),
		);
	let (sql, values) = builder.build_insert(&insert_staff);
	execute_dml(&pool, &sql, values.0, "insert staff user").await;

	// ---- Build router ----

	// Snapshot the configured site_header before consuming the pool so tests
	// can compare against it without re-fetching.
	let site_header = reinhardt_admin::settings::get_admin_settings()
		.site_header
		.clone();

	// Clone the pool for retention in E2eContext; sqlx::PgPool is Arc-backed
	// so the clone is cheap.
	let pool_for_ctx = pool.clone();

	let backend = Arc::new(PostgresBackend::new(pool));
	let backends_conn = BackendsConnection::new(backend);
	let connection = DatabaseConnection::new(DatabaseBackend::Postgres, backends_conn);
	let db_conn = Arc::new(connection);
	let admin_db = Arc::new(AdminDatabase::new((*db_conn).clone()));

	let mut site = AdminSite::new("E2E Test Admin");
	site.set_jwt_secret(JWT_SECRET);
	let site = Arc::new(site);
	register_models(&site);

	let (admin_router, admin_di) = admin_routes_with_di(site);

	let singleton = Arc::new(SingletonScope::new());
	singleton.set_arc(db_conn);
	let di_ctx = Arc::new(InjectionContext::builder(singleton).build());

	let router = reinhardt_urls::routers::UnifiedRouter::new()
		.with_di_context(di_ctx)
		.mount("/admin/", admin_router)
		.mount("/static/admin/", admin_static_routes())
		.with_di_registrations(admin_di)
		.into_server();

	// ---- Start server ----

	let server = TestServer::start(router).await;
	let server_url = server.container_url();

	E2eContext {
		server_url,
		browser,
		site_header,
		pool: pool_for_ctx,
		_server: server,
		_admin_db: admin_db,
	}
}

/// Fixture that creates a fully isolated E2E environment with one TestModel registered:
/// 1. Fresh PostgreSQL database with test data (testcontainers)
/// 2. HTTP server on a random port bound to 0.0.0.0
/// 3. Headless Chrome in a Docker container (testcontainers)
#[fixture]
async fn e2e(
	#[future] shared_db_pool: (sqlx::PgPool, String),
	#[future] cdp_browser: CdpBrowser,
) -> E2eContext {
	let (pool, _) = shared_db_pool.await;
	let browser = cdp_browser.await;
	build_e2e_context(pool, browser, |site| {
		site.register(
			"TestModel",
			AllPermissionsModelAdmin::test_model("test_models"),
		)
		.expect("Failed to register TestModel");
	})
	.await
}

/// Fixture variant: no models registered. Used to exercise the dashboard
/// empty-state branch (`No models registered` admin alert).
#[fixture]
async fn e2e_no_models(
	#[future] shared_db_pool: (sqlx::PgPool, String),
	#[future] cdp_browser: CdpBrowser,
) -> E2eContext {
	let (pool, _) = shared_db_pool.await;
	let browser = cdp_browser.await;
	build_e2e_context(pool, browser, |_site| {
		// Intentionally register no models.
	})
	.await
}

/// Fixture variant: two distinct models registered. Used to verify the
/// dashboard renders one card per registered model.
#[fixture]
async fn e2e_multi_models(
	#[future] shared_db_pool: (sqlx::PgPool, String),
	#[future] cdp_browser: CdpBrowser,
) -> E2eContext {
	let (pool, _) = shared_db_pool.await;
	let browser = cdp_browser.await;
	build_e2e_context(pool, browser, |site| {
		site.register(
			"TestModel",
			AllPermissionsModelAdmin::test_model("test_models"),
		)
		.expect("Failed to register TestModel");
		site.register(
			"TestModelB",
			AllPermissionsModelAdmin::test_model("test_models_b"),
		)
		.expect("Failed to register TestModelB");
	})
	.await
}

// ============================================================================
// Helpers
// ============================================================================

/// Returns true if the page content indicates the WASM SPA is loaded
/// (as opposed to the JS placeholder fallback).
fn is_wasm_spa(source: &str) -> bool {
	!source.contains("WASM frontend may not be built yet")
}

macro_rules! require_wasm {
	($source:expr) => {
		if !is_wasm_spa($source) {
			eprintln!("WASM SPA not built — skipping test");
			return;
		}
	};
}

/// Navigates within the WASM SPA without triggering a full page reload.
///
/// Uses `history.pushState` + `popstate` event to trigger the WASM router's
/// re-render effect, then waits for the new view's async data to load.
async fn spa_navigate(page: &CdpPage, path: &str) {
	let js = format!(
		"window.history.pushState({{}}, '', '{}'); \
		 window.dispatchEvent(new PopStateEvent('popstate'));",
		path
	);
	let _ = page.execute_js(&js).await;
	tokio::time::sleep(std::time::Duration::from_secs(5)).await;
}

/// Performs login through the WASM login form.
///
/// JWT is set as an HTTP-Only cookie by the server (not exposed to JS),
/// and the WASM on_success callback updates the reactive auth state.
/// Therefore, form-based login is the correct E2E authentication approach.
async fn inject_auth_token(page: &CdpPage, server_url: &str) {
	login_via_form(page, server_url).await;
}

/// Waits for the WASM SPA to fully initialize by checking for rendered content in `#app`.
///
/// The WASM `main()` function calls `app_element.set_inner_html()` which populates the
/// `#app` div. Before WASM loads, this div is empty.
async fn wait_for_wasm_init(page: &CdpPage) {
	let start = std::time::Instant::now();
	let timeout = std::time::Duration::from_secs(30);
	let poll = std::time::Duration::from_millis(500);

	loop {
		let has_content = page
			.execute_js("document.getElementById('app')?.innerHTML.length > 0")
			.await
			.ok()
			.and_then(|v| v.as_bool())
			.unwrap_or(false);

		if has_content {
			// Extra brief wait for event listeners to attach
			tokio::time::sleep(std::time::Duration::from_millis(500)).await;
			return;
		}

		if start.elapsed() > timeout {
			panic!("WASM SPA did not initialize within {:?}", timeout);
		}

		tokio::time::sleep(poll).await;
	}
}

/// Performs login through the actual WASM login form.
///
/// Waits for WASM to fully load and hydrate before interacting with form elements.
async fn login_via_form(page: &CdpPage, server_url: &str) {
	login_via_form_as(page, server_url, TEST_USERNAME, TEST_PASSWORD).await;
}

/// Performs login as the specified user. Generalization of `login_via_form`.
///
/// Used by tests that need to authenticate as a non-default user
/// (e.g., the non-staff user in `test_dashboard_non_staff_user_blocked`).
async fn login_via_form_as(page: &CdpPage, server_url: &str, username: &str, password: &str) {
	page.navigate(&format!("{}/admin/login/", server_url))
		.await
		.expect("Failed to navigate to login page");

	// Wait for WASM to initialize (downloads ~5.6MB WASM binary in dev mode)
	wait_for_wasm_init(page).await;

	page.type_into("input[name='username']", username)
		.await
		.expect("Failed to type username");
	page.type_into("input[name='password']", password)
		.await
		.expect("Failed to type password");
	page.click("button[type='submit']")
		.await
		.expect("Failed to click submit");

	// Wait for login server function call to complete and WASM to navigate
	tokio::time::sleep(std::time::Duration::from_secs(5)).await;
}

/// Polls until the dashboard resource resolves: either model cards or the
/// empty-state alert appear inside `.dashboard-container`.
///
/// Centralizes wait logic so individual dashboard tests do not need to
/// guess sleep durations. The poll interval is short (200 ms) and the
/// total timeout is 15 s, which exceeds typical `get_dashboard()` server
/// function latencies on CI.
async fn wait_for_dashboard_loaded(page: &CdpPage) {
	let start = std::time::Instant::now();
	let timeout = std::time::Duration::from_secs(15);
	let poll = std::time::Duration::from_millis(200);

	loop {
		let ready = page
			.execute_js(
				"(() => { \
				 const c = document.querySelector('.dashboard-container'); \
				 if (!c) return false; \
				 return c.querySelector('.admin-card, .admin-alert-info, .admin-alert-danger') !== null; \
				 })()",
			)
			.await
			.ok()
			.and_then(|v| v.as_bool())
			.unwrap_or(false);

		if ready {
			// Brief settle for any reactive effects after the card grid commits.
			tokio::time::sleep(std::time::Duration::from_millis(200)).await;
			return;
		}

		if start.elapsed() > timeout {
			panic!(
				"Dashboard did not finish loading within {:?} (looked for .admin-card, .admin-alert-info, or .admin-alert-danger inside .dashboard-container)",
				timeout
			);
		}

		tokio::time::sleep(poll).await;
	}
}

/// Inserts an active non-staff user (`is_active=true`, `is_staff=false`) into
/// `auth_user`. Used by `test_dashboard_non_staff_user_blocked` to verify the
/// dashboard rejects users who lack admin privileges.
async fn create_non_staff_user(pool: &sqlx::PgPool, username: &str, password: &str) {
	let hasher = Argon2Hasher::new();
	let password_hash = hasher
		.hash(password)
		.expect("Failed to hash non-staff password");

	let builder = PostgresQueryBuilder::new();

	let mut stmt = Query::insert();
	stmt.into_table("auth_user")
		.columns(["id", "username", "password_hash", "is_active", "is_staff"])
		.values(vec![
			Value::Uuid(Some(Box::new(
				uuid::Uuid::parse_str(NON_STAFF_UUID).expect("valid NON_STAFF_UUID"),
			))),
			Value::String(Some(Box::new(username.to_string()))),
			Value::String(Some(Box::new(password_hash))),
			Value::Bool(Some(true)),
			Value::Bool(Some(false)),
		])
		.expect("non-staff user value count matches column count")
		.on_conflict(
			OnConflict::column("id")
				.update_columns(["password_hash", "is_staff", "is_active"])
				.to_owned(),
		);
	let (sql, values) = builder.build_insert(&stmt);
	execute_dml(pool, &sql, values.0, "insert non-staff user").await;
}

// ============================================================================
// Test Cases
// ============================================================================

// --- 1. HTML Shell Tests (always pass, no WASM required) ---

#[rstest]
#[tokio::test]
async fn test_admin_html_shell_served(#[future] e2e: E2eContext) {
	let ctx = e2e.await;
	let page = ctx
		.browser
		.new_page(&format!("{}/admin/", ctx.server_url))
		.await
		.expect("Failed to open page");

	tokio::time::sleep(std::time::Duration::from_secs(2)).await;
	let source = page.content().await.expect("Failed to get page source");

	assert!(
		source.contains("id=\"app\""),
		"Should have #app mount point"
	);
	assert!(
		source.contains("Admin") || source.contains("admin"),
		"Should reference admin in title or content"
	);
}

#[rstest]
#[tokio::test]
async fn test_admin_login_shell_served(#[future] e2e: E2eContext) {
	let ctx = e2e.await;
	let page = ctx
		.browser
		.new_page(&format!("{}/admin/login/", ctx.server_url))
		.await
		.expect("Failed to open page");

	tokio::time::sleep(std::time::Duration::from_secs(2)).await;
	let source = page.content().await.expect("Failed to get page source");

	assert!(
		source.contains("id=\"app\""),
		"Login route should serve SPA shell"
	);
}

// --- 2. Login Page Tests (require WASM) ---

#[rstest]
#[tokio::test]
async fn test_login_page_renders(#[future] e2e: E2eContext) {
	let ctx = e2e.await;
	let page = ctx
		.browser
		.new_page(&format!("{}/admin/login/", ctx.server_url))
		.await
		.expect("Failed to open page");

	tokio::time::sleep(std::time::Duration::from_secs(2)).await;
	let source = page.content().await.expect("Failed to get page source");
	require_wasm!(&source);

	assert!(source.contains("username"), "Should contain username field");
	assert!(source.contains("password"), "Should contain password field");
	assert!(
		source.contains("Sign in") || source.contains("Login") || source.contains("submit"),
		"Should contain a submit button"
	);
}

#[rstest]
#[tokio::test]
async fn test_login_invalid_credentials_shows_error(#[future] e2e: E2eContext) {
	let ctx = e2e.await;
	let page = ctx
		.browser
		.new_page(&format!("{}/admin/login/", ctx.server_url))
		.await
		.expect("Failed to open page");

	tokio::time::sleep(std::time::Duration::from_secs(2)).await;
	let source = page.content().await.expect("Failed to get page source");
	require_wasm!(&source);

	page.type_into("input[name='username']", "wrong_user")
		.await
		.unwrap();
	page.type_into("input[name='password']", "wrong_password")
		.await
		.unwrap();
	page.click("button[type='submit']").await.unwrap();

	tokio::time::sleep(std::time::Duration::from_secs(2)).await;

	let source = page.content().await.unwrap();
	let current_url = page.url().await.unwrap().unwrap_or_default();

	assert!(
		current_url.contains("login"),
		"Should stay on login page, got: {}",
		current_url
	);
	assert!(
		source.contains("Invalid")
			|| source.contains("invalid")
			|| source.contains("error")
			|| source.contains("admin-alert-danger"),
		"Should display error message"
	);
}

#[rstest]
#[tokio::test]
async fn test_login_success_redirects_to_dashboard(#[future] e2e: E2eContext) {
	let ctx = e2e.await;
	let page = ctx
		.browser
		.new_page(&format!("{}/admin/login/", ctx.server_url))
		.await
		.expect("Failed to open page");

	tokio::time::sleep(std::time::Duration::from_secs(2)).await;
	let source = page.content().await.unwrap();
	require_wasm!(&source);

	login_via_form(&page, &ctx.server_url).await;

	let current_url = page.url().await.unwrap().unwrap_or_default();
	assert!(
		!current_url.contains("login") || current_url.ends_with("/admin/"),
		"Should redirect to dashboard, got: {}",
		current_url
	);
}

// --- 3. Dashboard Tests (require WASM) ---

#[rstest]
#[tokio::test]
async fn test_dashboard_shows_model_cards(#[future] e2e: E2eContext) {
	let ctx = e2e.await;
	let page = ctx
		.browser
		.new_page(&format!("{}/admin/login/", ctx.server_url))
		.await
		.expect("Failed to open page");

	tokio::time::sleep(std::time::Duration::from_secs(2)).await;
	let source = page.content().await.unwrap();
	require_wasm!(&source);

	// Login via form — on_success navigates to /admin/ via WASM router
	inject_auth_token(&page, &ctx.server_url).await;
	// Wait for dashboard server_fn (get_dashboard) to fetch and render
	tokio::time::sleep(std::time::Duration::from_secs(5)).await;

	let source = page.content().await.unwrap();
	assert!(
		source.contains("TestModel") || source.contains("testmodel"),
		"Should display TestModel card"
	);
	assert!(
		source.contains("Dashboard"),
		"Should contain dashboard heading"
	);
}

#[rstest]
#[tokio::test]
async fn test_dashboard_card_navigates_to_list(#[future] e2e: E2eContext) {
	let ctx = e2e.await;
	let page = ctx
		.browser
		.new_page(&format!("{}/admin/login/", ctx.server_url))
		.await
		.expect("Failed to open page");

	tokio::time::sleep(std::time::Duration::from_secs(2)).await;
	let source = page.content().await.unwrap();
	require_wasm!(&source);

	inject_auth_token(&page, &ctx.server_url).await;
	tokio::time::sleep(std::time::Duration::from_secs(5)).await;

	if page
		.click("a[href*='TestModel'], a[href*='testmodel']")
		.await
		.is_ok()
	{
		tokio::time::sleep(std::time::Duration::from_secs(2)).await;
		let url = page.url().await.unwrap().unwrap_or_default();
		assert!(
			url.to_lowercase().contains("testmodel"),
			"Should navigate to list, got: {}",
			url
		);
	}
}

// --- 4. List View Tests (require WASM) ---

#[rstest]
#[tokio::test]
async fn test_list_view_renders_table(#[future] e2e: E2eContext) {
	let ctx = e2e.await;
	let page = ctx
		.browser
		.new_page(&format!("{}/admin/login/", ctx.server_url))
		.await
		.expect("Failed to open page");

	tokio::time::sleep(std::time::Duration::from_secs(2)).await;
	let source = page.content().await.unwrap();
	require_wasm!(&source);

	inject_auth_token(&page, &ctx.server_url).await;
	spa_navigate(&page, "/admin/TestModel/").await;

	let source = page.content().await.unwrap();
	assert!(
		source.contains("TestModel") || source.contains("List"),
		"Should show list heading"
	);
	assert!(
		source.contains("Alice") || source.contains("Bob") || source.contains("Charlie"),
		"Should display test records"
	);
}

#[rstest]
#[tokio::test]
async fn test_list_view_row_navigates_to_detail(#[future] e2e: E2eContext) {
	let ctx = e2e.await;
	let page = ctx
		.browser
		.new_page(&format!("{}/admin/login/", ctx.server_url))
		.await
		.expect("Failed to open page");

	tokio::time::sleep(std::time::Duration::from_secs(2)).await;
	let source = page.content().await.unwrap();
	require_wasm!(&source);

	inject_auth_token(&page, &ctx.server_url).await;
	spa_navigate(&page, "/admin/TestModel/").await;

	if page.click("a[href*='/admin/TestModel/1/']").await.is_ok() {
		tokio::time::sleep(std::time::Duration::from_secs(2)).await;
		let url = page.url().await.unwrap().unwrap_or_default();
		assert!(
			url.contains("/admin/TestModel/") && url.len() > "/admin/TestModel/".len(),
			"Should navigate to detail, got: {}",
			url
		);
	}
}

// --- 5. Detail View Tests (require WASM) ---

#[rstest]
#[tokio::test]
async fn test_detail_view_has_edit_and_back(#[future] e2e: E2eContext) {
	let ctx = e2e.await;
	let page = ctx
		.browser
		.new_page(&format!("{}/admin/login/", ctx.server_url))
		.await
		.expect("Failed to open page");

	tokio::time::sleep(std::time::Duration::from_secs(2)).await;
	let source = page.content().await.unwrap();
	require_wasm!(&source);

	inject_auth_token(&page, &ctx.server_url).await;
	spa_navigate(&page, "/admin/TestModel/1/").await;

	let source = page.content().await.unwrap();
	assert!(
		source.contains("Alice") || source.contains("Detail"),
		"Should show record data"
	);
	assert!(
		source.contains("Edit") || source.contains("edit"),
		"Should have Edit link"
	);
	assert!(
		source.contains("Back") || source.contains("List"),
		"Should have Back link"
	);
}

// --- 6. Create Form Tests (require WASM) ---

#[rstest]
#[tokio::test]
async fn test_create_form_renders(#[future] e2e: E2eContext) {
	let ctx = e2e.await;
	let page = ctx
		.browser
		.new_page(&format!("{}/admin/login/", ctx.server_url))
		.await
		.expect("Failed to open page");

	tokio::time::sleep(std::time::Duration::from_secs(2)).await;
	let source = page.content().await.unwrap();
	require_wasm!(&source);

	inject_auth_token(&page, &ctx.server_url).await;
	spa_navigate(&page, "/admin/TestModel/add/").await;

	let source = page.content().await.unwrap();
	assert!(
		source.contains("Create") || source.contains("Add") || source.contains("form"),
		"Should show create form"
	);
	assert!(
		source.contains("name") || source.contains("Name"),
		"Should have name field"
	);
}

// --- 7. Auth Redirect Tests (require WASM) ---

#[rstest]
#[tokio::test]
async fn test_unauthenticated_redirect_to_login(#[future] e2e: E2eContext) {
	let ctx = e2e.await;
	let page = ctx
		.browser
		.new_page(&format!("{}/admin/", ctx.server_url))
		.await
		.expect("Failed to open page");

	tokio::time::sleep(std::time::Duration::from_secs(3)).await;

	let source = page.content().await.unwrap();
	require_wasm!(&source);

	let url = page.url().await.unwrap().unwrap_or_default();
	assert!(
		url.contains("login"),
		"Should redirect to login, got: {}",
		url
	);
}

// --- 8. Edit Form Tests (require WASM) ---

#[rstest]
#[tokio::test]
async fn test_edit_form_renders_with_values(#[future] e2e: E2eContext) {
	let ctx = e2e.await;
	let page = ctx
		.browser
		.new_page(&format!("{}/admin/login/", ctx.server_url))
		.await
		.expect("Failed to open page");

	tokio::time::sleep(std::time::Duration::from_secs(2)).await;
	let source = page.content().await.unwrap();
	require_wasm!(&source);

	inject_auth_token(&page, &ctx.server_url).await;
	spa_navigate(&page, "/admin/TestModel/1/change/").await;
	tokio::time::sleep(std::time::Duration::from_secs(3)).await;

	let source = page.content().await.unwrap();
	assert!(
		source.contains("Edit") || source.contains("Change") || source.contains("form"),
		"Should show edit form"
	);
	assert!(
		source.contains("Alice") || source.contains("alice") || source.contains("value="),
		"Should have pre-filled values"
	);
}
// ===== Dashboard tests (extended) =====
//
// The following tests target display branches in `dashboard()`
// (crates/reinhardt-admin/src/pages/components/features.rs) that are not
// covered by the basic dashboard tests above. They follow the AAA pattern
// and use `cdp_browser` for parallel-safe isolation.

/// Verify that the dashboard h1 renders the configured site_header
/// (sourced from `AdminSettings::default()`), not just a hard-coded string.
#[rstest]
#[tokio::test]
async fn test_dashboard_renders_site_header(#[future] e2e: E2eContext) {
	// Arrange
	let ctx = e2e.await;
	let page = ctx
		.browser
		.new_page(&format!("{}/admin/login/", ctx.server_url))
		.await
		.expect("Failed to open page");
	tokio::time::sleep(std::time::Duration::from_secs(2)).await;
	let source = page.content().await.unwrap();
	require_wasm!(&source);
	inject_auth_token(&page, &ctx.server_url).await;
	wait_for_dashboard_loaded(&page).await;

	// Act
	let h1 = page
		.get_text(".dashboard-container h1")
		.await
		.expect("Failed to read dashboard h1")
		.unwrap_or_default();

	// Assert
	assert!(
		h1.contains(&ctx.site_header),
		"h1 should contain site_header `{}`, got `{}`",
		ctx.site_header,
		h1,
	);
	assert!(
		h1.contains("Dashboard"),
		"h1 should contain `Dashboard`, got `{}`",
		h1,
	);
}

/// Verify that the dashboard renders the empty-state alert when no
/// models are registered with the AdminSite.
#[rstest]
#[tokio::test]
async fn test_dashboard_empty_state_shows_alert(#[future] e2e_no_models: E2eContext) {
	// Arrange
	let ctx = e2e_no_models.await;
	let page = ctx
		.browser
		.new_page(&format!("{}/admin/login/", ctx.server_url))
		.await
		.expect("Failed to open page");
	tokio::time::sleep(std::time::Duration::from_secs(2)).await;
	let source = page.content().await.unwrap();
	require_wasm!(&source);
	inject_auth_token(&page, &ctx.server_url).await;
	wait_for_dashboard_loaded(&page).await;

	// Act
	let alert_text = page
		.get_text(".dashboard-container .admin-alert-info")
		.await
		.expect("Failed to read empty-state alert")
		.unwrap_or_default();
	let card_count = page
		.execute_js("document.querySelectorAll('.dashboard-container .admin-card').length")
		.await
		.expect("Failed to count cards");

	// Assert
	assert!(
		alert_text.contains("No models registered"),
		"Expected empty-state alert text, got `{}`",
		alert_text,
	);
	assert_eq!(
		card_count.as_u64().unwrap_or(u64::MAX),
		0,
		"Expected zero `.admin-card` elements when no models are registered"
	);
}

/// Verify that each registered model gets its own `.admin-card` on the dashboard.
#[rstest]
#[tokio::test]
async fn test_dashboard_renders_card_per_model(#[future] e2e_multi_models: E2eContext) {
	// Arrange
	let ctx = e2e_multi_models.await;
	let page = ctx
		.browser
		.new_page(&format!("{}/admin/login/", ctx.server_url))
		.await
		.expect("Failed to open page");
	tokio::time::sleep(std::time::Duration::from_secs(2)).await;
	let source = page.content().await.unwrap();
	require_wasm!(&source);
	inject_auth_token(&page, &ctx.server_url).await;
	wait_for_dashboard_loaded(&page).await;

	// Act
	let card_count = page
		.execute_js("document.querySelectorAll('.dashboard-container .admin-card').length")
		.await
		.expect("Failed to count cards");
	let names_json = page
		.execute_js(
			"JSON.stringify(Array.from(document.querySelectorAll('.dashboard-container .admin-card h3')).map(e => e.textContent.trim()))",
		)
		.await
		.expect("Failed to read card names");
	let names: Vec<String> = match names_json {
		serde_json::Value::String(s) => serde_json::from_str(&s).unwrap_or_default(),
		serde_json::Value::Array(_) => serde_json::from_value(names_json).unwrap_or_default(),
		_ => Vec::new(),
	};
	let names_set: std::collections::HashSet<&str> = names.iter().map(String::as_str).collect();

	// Assert
	assert_eq!(
		card_count.as_u64().unwrap_or(u64::MAX),
		2,
		"Expected exactly 2 `.admin-card` elements for 2 registered models"
	);
	assert!(
		names_set.contains("TestModel") && names_set.contains("TestModelB"),
		"Expected card names to include both `TestModel` and `TestModelB`, got {:?}",
		names_set,
	);
}

/// Verify that a single dashboard card contains the full expected structure:
/// model name in `<h3>`, "Manage X records" description, and a `View X` button
/// with the correct `href` to the list view.
#[rstest]
#[tokio::test]
async fn test_dashboard_card_structure(#[future] e2e: E2eContext) {
	// Arrange
	let ctx = e2e.await;
	let page = ctx
		.browser
		.new_page(&format!("{}/admin/login/", ctx.server_url))
		.await
		.expect("Failed to open page");
	tokio::time::sleep(std::time::Duration::from_secs(2)).await;
	let source = page.content().await.unwrap();
	require_wasm!(&source);
	inject_auth_token(&page, &ctx.server_url).await;
	wait_for_dashboard_loaded(&page).await;

	// Act
	let h3_text = page
		.get_text(".dashboard-container .admin-card h3")
		.await
		.expect("Failed to read card h3")
		.unwrap_or_default();
	let description_text = page
		.get_text(".dashboard-container .admin-card p")
		.await
		.expect("Failed to read card description")
		.unwrap_or_default();
	let button_text = page
		.get_text(".dashboard-container .admin-card a.admin-btn-primary")
		.await
		.expect("Failed to read card button")
		.unwrap_or_default();
	let button_href = page
		.get_attribute(
			".dashboard-container .admin-card a.admin-btn-primary",
			"href",
		)
		.await
		.expect("Failed to read button href")
		.unwrap_or_default();

	// Assert
	assert_eq!(h3_text.trim(), "TestModel", "card h3 should be model name");
	assert!(
		description_text.contains("Manage TestModel records"),
		"card description mismatch: `{}`",
		description_text,
	);
	assert!(
		button_text.contains("View TestModel"),
		"card button label mismatch: `{}`",
		button_text,
	);
	assert!(
		button_href.ends_with("/testmodel/"),
		"card button href should target lowercased model list URL, got `{}`",
		button_href,
	);
}

/// Verify that an active but non-staff user cannot view the dashboard.
///
/// The expected behavior is either a redirect back to `/login` (auth gate)
/// or an error alert rendered in place of the cards. Both outcomes indicate
/// the dashboard correctly blocks unauthorized users.
#[rstest]
#[tokio::test]
async fn test_dashboard_non_staff_user_blocked(#[future] e2e: E2eContext) {
	// Arrange
	let ctx = e2e.await;
	create_non_staff_user(&ctx.pool, NON_STAFF_USERNAME, NON_STAFF_PASSWORD).await;

	let page = ctx
		.browser
		.new_page(&format!("{}/admin/login/", ctx.server_url))
		.await
		.expect("Failed to open page");
	tokio::time::sleep(std::time::Duration::from_secs(2)).await;
	let source = page.content().await.unwrap();
	require_wasm!(&source);

	// Login as the non-staff user. The login server function may succeed
	// (issuing a JWT) but `get_dashboard()` requires `is_staff=true`.
	login_via_form_as(
		&page,
		&ctx.server_url,
		NON_STAFF_USERNAME,
		NON_STAFF_PASSWORD,
	)
	.await;
	tokio::time::sleep(std::time::Duration::from_secs(3)).await;

	// Act
	let url = page.url().await.unwrap().unwrap_or_default();
	let card_count = page
		.execute_js("document.querySelectorAll('.dashboard-container .admin-card').length")
		.await
		.expect("Failed to count cards");
	let danger_present = page
		.execute_js(
			"document.querySelector('.admin-alert-danger, .dashboard-container [class*=error]') !== null",
		)
		.await
		.expect("Failed to check error alert")
		.as_bool()
		.unwrap_or(false);

	// Assert
	assert_eq!(
		card_count.as_u64().unwrap_or(u64::MAX),
		0,
		"Non-staff user must not see any model cards on the dashboard"
	);
	assert!(
		url.contains("/login") || danger_present,
		"Non-staff user should be redirected to login or shown an error alert; \
		 url=`{}`, danger_present={}",
		url,
		danger_present,
	);
}

/// Verify that navigating away from the dashboard and back via `history.back()`
/// re-renders the model cards. Exercises the SPA router's resource re-fetch path.
#[rstest]
#[tokio::test]
async fn test_dashboard_back_navigation_rerenders(#[future] e2e: E2eContext) {
	// Arrange
	let ctx = e2e.await;
	let page = ctx
		.browser
		.new_page(&format!("{}/admin/login/", ctx.server_url))
		.await
		.expect("Failed to open page");
	tokio::time::sleep(std::time::Duration::from_secs(2)).await;
	let source = page.content().await.unwrap();
	require_wasm!(&source);
	inject_auth_token(&page, &ctx.server_url).await;
	wait_for_dashboard_loaded(&page).await;

	// Confirm dashboard is loaded before navigating away.
	let initial_card_count = page
		.execute_js("document.querySelectorAll('.dashboard-container .admin-card').length")
		.await
		.expect("Failed to count initial cards")
		.as_u64()
		.unwrap_or(0);
	assert!(
		initial_card_count >= 1,
		"Dashboard should render at least one card before navigation"
	);

	// Act: navigate to list view, then back to dashboard.
	spa_navigate(&page, "/admin/TestModel/").await;
	let _ = page
		.execute_js("window.history.back(); window.dispatchEvent(new PopStateEvent('popstate'));")
		.await;
	tokio::time::sleep(std::time::Duration::from_secs(2)).await;
	wait_for_dashboard_loaded(&page).await;

	// Assert
	let url = page.url().await.unwrap().unwrap_or_default();
	let final_card_count = page
		.execute_js("document.querySelectorAll('.dashboard-container .admin-card').length")
		.await
		.expect("Failed to count cards after back navigation")
		.as_u64()
		.unwrap_or(0);

	assert!(
		url.ends_with("/admin/") || url.ends_with("/admin"),
		"Should be back on dashboard URL after history.back(), got `{}`",
		url,
	);
	assert!(
		final_card_count >= 1,
		"Dashboard cards should re-render after back navigation, got {} cards",
		final_card_count,
	);
}
