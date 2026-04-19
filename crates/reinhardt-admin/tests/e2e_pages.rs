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
use reinhardt_test::fixtures::shared_postgres::shared_db_pool;
use reinhardt_test::fixtures::wasm::e2e_cdp::*;
use rstest::*;
use sqlx::Executor;
use std::net::SocketAddr;
use std::sync::Arc;

// ============================================================================
// Constants
// ============================================================================

const TEST_USERNAME: &str = "test_staff";
const TEST_PASSWORD: &str = "e2e-test-password-2026";
const TEST_USER_UUID: &str = "00000000-0000-0000-0000-000000000001";
const JWT_SECRET: &[u8] = b"e2e-test-jwt-secret-at-least-32-bytes!!";

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
	// Hold server and db alive for the test lifetime.
	_server: TestServer,
	_admin_db: Arc<AdminDatabase>,
}

/// Fixture that creates a fully isolated E2E environment:
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

	// ---- Database setup ----

	pool.execute(
		"CREATE TABLE IF NOT EXISTS test_models (
			id SERIAL PRIMARY KEY,
			name VARCHAR(255) NOT NULL,
			status VARCHAR(50) DEFAULT 'active',
			description TEXT,
			created_at TIMESTAMPTZ DEFAULT NOW()
		)",
	)
	.await
	.expect("Failed to create test_models table");

	pool.execute("TRUNCATE TABLE test_models RESTART IDENTITY CASCADE")
		.await
		.expect("Failed to truncate test_models");

	pool.execute(
		"INSERT INTO test_models (name, status) VALUES
		 ('Alice', 'active'),
		 ('Bob', 'inactive'),
		 ('Charlie', 'active')",
	)
	.await
	.expect("Failed to insert test data");

	pool.execute("DROP TABLE IF EXISTS auth_user CASCADE")
		.await
		.expect("Failed to drop auth_user");

	pool.execute(
		"CREATE TABLE auth_user (
			id UUID PRIMARY KEY,
			username VARCHAR(150) NOT NULL,
			email VARCHAR(254) NOT NULL DEFAULT '',
			first_name VARCHAR(150) NOT NULL DEFAULT '',
			last_name VARCHAR(150) NOT NULL DEFAULT '',
			password_hash TEXT,
			last_login TIMESTAMPTZ,
			is_active BOOLEAN NOT NULL DEFAULT true,
			is_staff BOOLEAN NOT NULL DEFAULT false,
			is_superuser BOOLEAN NOT NULL DEFAULT false,
			date_joined TIMESTAMPTZ NOT NULL DEFAULT NOW(),
			user_permissions TEXT NOT NULL DEFAULT '[]',
			groups TEXT NOT NULL DEFAULT '[]'
		)",
	)
	.await
	.expect("Failed to create auth_user table");

	let hasher = Argon2Hasher::new();
	let password_hash = hasher
		.hash(TEST_PASSWORD)
		.expect("Failed to hash test password");

	sqlx::query(
		"INSERT INTO auth_user (id, username, password_hash, is_active, is_staff, date_joined)
		 VALUES ($1, $2, $3, true, true, NOW())
		 ON CONFLICT (id) DO UPDATE SET password_hash = $3, is_staff = true, is_active = true",
	)
	.bind(uuid::Uuid::parse_str(TEST_USER_UUID).unwrap())
	.bind(TEST_USERNAME)
	.bind(&password_hash)
	.execute(&pool)
	.await
	.expect("Failed to insert test staff user");

	// ---- Build router ----

	let backend = Arc::new(PostgresBackend::new(pool));
	let backends_conn = BackendsConnection::new(backend);
	let connection = DatabaseConnection::new(DatabaseBackend::Postgres, backends_conn);
	let db_conn = Arc::new(connection);
	let admin_db = Arc::new(AdminDatabase::new((*db_conn).clone()));

	let mut site = AdminSite::new("E2E Test Admin");
	site.set_jwt_secret(JWT_SECRET);
	let site = Arc::new(site);
	site.register(
		"TestModel",
		AllPermissionsModelAdmin::test_model("test_models"),
	)
	.expect("Failed to register TestModel");

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
		_server: server,
		_admin_db: admin_db,
	}
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
	page.navigate(&format!("{}/admin/login/", server_url))
		.await
		.expect("Failed to navigate to login page");

	// Wait for WASM to initialize (downloads ~5.6MB WASM binary in dev mode)
	wait_for_wasm_init(page).await;

	page.type_into("input[name='username']", TEST_USERNAME)
		.await
		.expect("Failed to type username");
	page.type_into("input[name='password']", TEST_PASSWORD)
		.await
		.expect("Failed to type password");
	page.click("button[type='submit']")
		.await
		.expect("Failed to click submit");

	// Wait for login server function call to complete and WASM to navigate
	tokio::time::sleep(std::time::Duration::from_secs(5)).await;
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
