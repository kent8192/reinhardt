//! Shared helpers for middleware-integrated E2E tests (Category 5).
//!
//! Provides a `MiddlewareTestServer` fixture that starts a real HTTP server
//! with the full middleware pipeline (LoggingMiddleware + router-level
//! AdminCookieAuthMiddleware + AdminOriginGuardMiddleware), and helper
//! functions for sending authenticated requests via `reqwest`.

use reinhardt_admin::core::{AdminDatabase, AdminSite, admin_routes_with_di};
use reinhardt_admin::server::security::ADMIN_AUTH_COOKIE_NAME;
use reinhardt_auth::JwtAuth;
use reinhardt_db::backends::connection::DatabaseConnection as BackendsConnection;
use reinhardt_db::backends::dialect::PostgresBackend;
use reinhardt_db::orm::connection::{DatabaseBackend, DatabaseConnection};
use reinhardt_di::{Depends, InjectionContext, SingletonScope};
use reinhardt_middleware::LoggingMiddleware;
use reinhardt_query::prelude::{Alias, PostgresQueryBuilder, Query, QueryStatementBuilder};
use reinhardt_server::HttpServer;
use reinhardt_test::fixtures::shared_postgres::shared_db_pool;
use rstest::*;
use sqlx::Executor;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::task::JoinHandle;
use uuid::Uuid;

use super::server_fn_helpers::{
	AllPermissionsModelAdmin, TEST_CSRF_TOKEN, TEST_INACTIVE_USER_UUID, TEST_NON_STAFF_USER_UUID,
	TEST_USER_UUID, build_auth_user_create_table_sql, setup_test_models_table,
};

/// JWT secret shared between `AdminSite` and test token generation.
/// Must be at least 32 bytes for HMAC-SHA256.
const JWT_SECRET: &[u8] = b"e2e-middleware-test-jwt-secret-32b!!";

/// Test server wrapping a real `HttpServer` bound to a random port.
///
/// Requests sent to `base_url` traverse the full middleware pipeline:
/// `HttpServer` (body limits) → `LoggingMiddleware` → router-level
/// `AdminOriginGuardMiddleware` → `AdminCookieAuthMiddleware` → handler.
pub struct MiddlewareTestServer {
	pub base_url: String,
	_server_task: JoinHandle<()>,
}

impl Drop for MiddlewareTestServer {
	fn drop(&mut self) {
		self._server_task.abort();
	}
}

/// Authentication mode for test HTTP requests.
pub enum AuthMode {
	/// JWT token sent in `reinhardt_admin_token` cookie.
	JwtCookie(String),
	/// JWT token sent in `Authorization: Bearer` header.
	BearerHeader(String),
	/// No authentication credentials.
	NoAuth,
}

/// Origin header behavior for test HTTP requests.
enum OriginMode<'a> {
	/// Same-origin: Origin header matches the server's base URL.
	SameOrigin(&'a str),
	/// Cross-origin: Origin header set to a different domain.
	CrossOrigin,
	/// No Origin or Referer header.
	None,
}

/// Generates a signed JWT token for the given user parameters.
pub fn generate_jwt_token(
	user_id: &str,
	username: &str,
	is_staff: bool,
	is_superuser: bool,
) -> String {
	let jwt_auth = JwtAuth::new(JWT_SECRET);
	jwt_auth
		.generate_token(
			user_id.to_string(),
			username.to_string(),
			is_staff,
			is_superuser,
		)
		.expect("Failed to generate JWT token")
}

/// Generates a valid JWT token for the default staff test user.
pub fn staff_jwt_token() -> String {
	generate_jwt_token(TEST_USER_UUID, "test_staff", true, false)
}

/// Generates a JWT token for the non-staff test user.
pub fn non_staff_jwt_token() -> String {
	generate_jwt_token(TEST_NON_STAFF_USER_UUID, "non_staff", false, false)
}

/// Generates a JWT token for the inactive test user.
pub fn inactive_jwt_token() -> String {
	generate_jwt_token(TEST_INACTIVE_USER_UUID, "inactive_staff", true, false)
}

/// Builds and sends a POST request to an admin server function endpoint.
///
/// This is the core request builder that all public helpers delegate to.
/// It constructs headers (Host, Content-Type, Cookie, Authorization) based
/// on the provided `auth` and `origin` parameters.
async fn send_admin_post(
	client: &reqwest::Client,
	base_url: &str,
	endpoint: &str,
	body: serde_json::Value,
	auth: AuthMode,
	origin: OriginMode<'_>,
) -> reqwest::Response {
	let url = format!("{}/admin/api/server_fn/{}", base_url, endpoint);
	let host = base_url.strip_prefix("http://").unwrap_or(base_url);

	let mut request = client
		.post(&url)
		.header("Content-Type", "application/json")
		.header("Host", host);

	match origin {
		OriginMode::SameOrigin(base) => {
			request = request.header("Origin", base);
		}
		OriginMode::CrossOrigin => {
			request = request.header("Origin", "http://evil.example.com");
		}
		OriginMode::None => {}
	}

	match auth {
		AuthMode::JwtCookie(token) => {
			request = request.header(
				"Cookie",
				format!(
					"csrftoken={}; {}={}",
					TEST_CSRF_TOKEN, ADMIN_AUTH_COOKIE_NAME, token
				),
			);
		}
		AuthMode::BearerHeader(token) => {
			request = request
				.header("Cookie", format!("csrftoken={}", TEST_CSRF_TOKEN))
				.header("Authorization", format!("Bearer {}", token));
		}
		AuthMode::NoAuth => {
			request = request.header("Cookie", format!("csrftoken={}", TEST_CSRF_TOKEN));
		}
	}

	request
		.json(&body)
		.send()
		.await
		.expect("Failed to send HTTP request")
}

/// Sends a POST request with same-origin headers through the full pipeline.
pub async fn post_server_fn(
	client: &reqwest::Client,
	base_url: &str,
	endpoint: &str,
	body: serde_json::Value,
	auth: AuthMode,
) -> reqwest::Response {
	send_admin_post(
		client,
		base_url,
		endpoint,
		body,
		auth,
		OriginMode::SameOrigin(base_url),
	)
	.await
}

/// Sends a POST request WITHOUT Origin header (for origin guard rejection tests).
pub async fn post_without_origin(
	client: &reqwest::Client,
	base_url: &str,
	endpoint: &str,
	body: serde_json::Value,
	auth: AuthMode,
) -> reqwest::Response {
	send_admin_post(client, base_url, endpoint, body, auth, OriginMode::None).await
}

/// Sends a POST request with a cross-origin Origin header.
pub async fn post_cross_origin(
	client: &reqwest::Client,
	base_url: &str,
	endpoint: &str,
	body: serde_json::Value,
	auth: AuthMode,
) -> reqwest::Response {
	send_admin_post(
		client,
		base_url,
		endpoint,
		body,
		auth,
		OriginMode::CrossOrigin,
	)
	.await
}

// ── Fixture ──

/// Composite fixture providing a real HTTP server with the full middleware pipeline.
///
/// Unlike `e2e_router_context` (which calls `router.handle()` directly), this
/// fixture starts a real `HttpServer` bound to a random port. Requests sent to
/// `MiddlewareTestServer::base_url` traverse:
///
/// 1. `HttpServer` — body size limit enforcement (10 MB default)
/// 2. `LoggingMiddleware` — request/response logging
/// 3. `AdminOriginGuardMiddleware` — same-origin validation (router-level)
/// 4. `AdminCookieAuthMiddleware` — JWT extraction from cookie/header (router-level)
/// 5. Route handler with `InjectionContext::fork_for_request()`
///
/// The critical difference from `e2e_router_context`: `AdminSite::set_jwt_secret()`
/// is called, which enables `AdminCookieAuthMiddleware` in `build_admin_router()`.
#[fixture]
pub async fn middleware_e2e_context(
	#[future] shared_db_pool: (sqlx::PgPool, String),
) -> (MiddlewareTestServer, Depends<AdminDatabase>) {
	let (pool, _) = shared_db_pool.await;

	setup_test_models_table(&pool).await;

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

	pool.execute(
		sqlx::query(
			"INSERT INTO auth_user (id, username, email, is_active, is_staff, is_superuser, date_joined) \
			 VALUES ($1, 'test_staff', 'staff@test.example', true, true, false, NOW()) \
			 ON CONFLICT (id) DO UPDATE SET is_staff = true, is_active = true",
		)
		.bind(Uuid::parse_str(TEST_USER_UUID).expect("Invalid TEST_USER_UUID")),
	)
	.await
	.expect("Failed to insert test staff user");

	pool.execute(
		sqlx::query(
			"INSERT INTO auth_user (id, username, email, is_active, is_staff, is_superuser, date_joined) \
			 VALUES ($1, 'inactive_staff', 'inactive@test.example', false, true, false, NOW()) \
			 ON CONFLICT (id) DO UPDATE SET is_active = false, is_staff = true",
		)
		.bind(Uuid::parse_str(TEST_INACTIVE_USER_UUID).expect("Invalid TEST_INACTIVE_USER_UUID")),
	)
	.await
	.expect("Failed to insert inactive test staff user");

	pool.execute(
		sqlx::query(
			"INSERT INTO auth_user (id, username, email, is_active, is_staff, is_superuser, date_joined) \
			 VALUES ($1, 'non_staff', 'nonstaff@test.example', true, false, false, NOW()) \
			 ON CONFLICT (id) DO UPDATE SET is_active = true, is_staff = false",
		)
		.bind(Uuid::parse_str(TEST_NON_STAFF_USER_UUID).expect("Invalid TEST_NON_STAFF_USER_UUID")),
	)
	.await
	.expect("Failed to insert non-staff test user");

	let backend = Arc::new(PostgresBackend::new(pool));
	let backends_conn = BackendsConnection::new(backend);
	let connection = DatabaseConnection::new(DatabaseBackend::Postgres, backends_conn);
	let db_conn = Arc::new(connection);

	let admin_db = Depends::from_value(AdminDatabase::new((*db_conn).clone()));

	let mut site = AdminSite::new("Middleware E2E Test Admin");
	site.set_jwt_secret(JWT_SECRET);
	let site = Depends::from_value(site);
	let admin = AllPermissionsModelAdmin::test_model("test_models");
	site.register("TestModel", admin)
		.expect("Failed to register TestModel");

	let (admin_router, admin_di) = admin_routes_with_di(Arc::clone(site.as_arc()));

	let singleton = Arc::new(SingletonScope::new());
	singleton.set_arc(db_conn);
	let di_ctx = Arc::new(InjectionContext::builder(singleton).build());

	let router = reinhardt_urls::routers::UnifiedRouter::new()
		.with_di_context(di_ctx)
		.mount("/admin/", admin_router)
		.with_di_registrations(admin_di)
		.into_server();

	let server = HttpServer::new(router).with_middleware(LoggingMiddleware::new());

	let listener = TcpListener::bind("127.0.0.1:0")
		.await
		.expect("Failed to bind test server");
	let addr = listener.local_addr().expect("Failed to get local addr");
	let base_url = format!("http://{}", addr);

	let server_task = tokio::spawn(async move {
		loop {
			match listener.accept().await {
				Ok((stream, socket_addr)) => {
					let handler_clone = server.handler();
					tokio::spawn(async move {
						if let Err(e) =
							HttpServer::handle_connection(stream, socket_addr, handler_clone, None)
								.await
						{
							eprintln!("Error handling connection: {:?}", e);
						}
					});
				}
				Err(e) => {
					eprintln!("Error accepting connection: {:?}", e);
					break;
				}
			}
		}
	});

	let test_server = MiddlewareTestServer {
		base_url,
		_server_task: server_task,
	};

	(test_server, admin_db)
}
