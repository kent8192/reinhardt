//! Integration test utilities for Reinhardt
//!
//! This crate provides common utilities for integration testing
//! across multiple Reinhardt crates with HTTP framework integration.
//!
//! # Proc Macro Path Resolution
//!
//! The `reinhardt` dependency in Cargo.toml provides the paths that proc macros
//! like `#[derive(Model)]` and `#[endpoint]` generate (e.g., `::reinhardt::db::orm::Model`).

use bytes::Bytes;
use reinhardt_db::migrations::{Constraint, ForeignKeyAction};
use reinhardt_http::Handler;
use reinhardt_http::Request;
use reinhardt_server::{HttpServer, ShutdownCoordinator};
use sqlx::{Pool, Postgres};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

// pub mod flatpages_app;
pub mod message_middleware_mock;
pub mod messages_helpers;

// New shared modules
// pub mod flatpages_common;
pub mod db_transaction;
pub mod migration_duplicate;
pub mod migrations;
pub mod validator_test_common;

/// Test database setup using TestContainers
///
/// This function uses the shared PostgreSQL container managed by reinhardt-test
/// fixtures to create an isolated test database.
pub async fn setup_test_db() -> Pool<Postgres> {
	reinhardt_test::fixtures::get_test_pool().await
}

/// Create test tables for flatpages using reinhardt-migrations
// Note: Test helper function - may appear unused but available for flatpages integration tests
#[allow(dead_code)]
pub async fn create_flatpages_tables(pool: &Pool<Postgres>) {
	use reinhardt_db::migrations::{ColumnDefinition, FieldType, Migration, Operation, SqlDialect};

	// Define flatpages migration with all required tables
	let flatpages_migration = Migration::new("test_flatpages_schema", "test")
		.add_operation(Operation::CreateTable {
			name: "flatpages".to_string(),
			columns: vec![
				ColumnDefinition::new("id", FieldType::Custom("BIGSERIAL PRIMARY KEY".to_string())),
				ColumnDefinition::new(
					"url",
					FieldType::Custom("VARCHAR(255) NOT NULL UNIQUE".to_string()),
				),
				ColumnDefinition::new(
					"title",
					FieldType::Custom("VARCHAR(255) NOT NULL".to_string()),
				),
				ColumnDefinition::new("content", FieldType::Custom("TEXT NOT NULL".to_string())),
				ColumnDefinition::new(
					"enable_comments",
					FieldType::Custom("BOOLEAN NOT NULL DEFAULT FALSE".to_string()),
				),
				ColumnDefinition::new("template_name", FieldType::VarChar(255)),
				ColumnDefinition::new(
					"registration_required",
					FieldType::Custom("BOOLEAN NOT NULL DEFAULT FALSE".to_string()),
				),
				ColumnDefinition::new(
					"created_at",
					FieldType::Custom("TIMESTAMPTZ NOT NULL DEFAULT NOW()".to_string()),
				),
				ColumnDefinition::new(
					"updated_at",
					FieldType::Custom("TIMESTAMPTZ NOT NULL DEFAULT NOW()".to_string()),
				),
			],
			constraints: vec![],
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		})
		.add_operation(Operation::CreateTable {
			name: "sites".to_string(),
			columns: vec![
				ColumnDefinition::new("id", FieldType::Custom("BIGSERIAL PRIMARY KEY".to_string())),
				ColumnDefinition::new(
					"domain",
					FieldType::Custom("VARCHAR(255) NOT NULL UNIQUE".to_string()),
				),
				ColumnDefinition::new(
					"name",
					FieldType::Custom("VARCHAR(255) NOT NULL".to_string()),
				),
			],
			constraints: vec![],
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		})
		.add_operation(Operation::CreateTable {
			name: "flatpage_sites".to_string(),
			columns: vec![
				ColumnDefinition::new("id", FieldType::Custom("BIGSERIAL PRIMARY KEY".to_string())),
				ColumnDefinition::new(
					"flatpage_id",
					FieldType::Custom("BIGINT NOT NULL".to_string()),
				),
				ColumnDefinition::new("site_id", FieldType::Custom("BIGINT NOT NULL".to_string())),
			],
			constraints: vec![
				Constraint::ForeignKey {
					name: "fk_flatpages_sites_flatpage_id".to_string(),
					columns: vec!["flatpage_id".to_string()],
					referenced_table: "flatpages".to_string(),
					referenced_columns: vec!["id".to_string()],
					on_delete: ForeignKeyAction::Cascade,
					on_update: ForeignKeyAction::Cascade,
					deferrable: None,
				},
				Constraint::ForeignKey {
					name: "fk_flatpages_sites_site_id".to_string(),
					columns: vec!["site_id".to_string()],
					referenced_table: "sites".to_string(),
					referenced_columns: vec!["id".to_string()],
					on_delete: ForeignKeyAction::Cascade,
					on_update: ForeignKeyAction::Cascade,
					deferrable: None,
				},
				Constraint::Unique {
					name: "uq_flatpage_site".to_string(),
					columns: vec!["flatpage_id".to_string(), "site_id".to_string()],
				},
			],
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		});

	// Generate and execute SQL for each operation
	for operation in &flatpages_migration.operations {
		let sql = operation.to_sql(&SqlDialect::Postgres);
		sqlx::query(sql.as_str())
			.execute(pool)
			.await
			.expect("Failed to create table");
	}
}

/// Clean up test tables using reinhardt-migrations
// Note: Test helper function - may appear unused but available for test cleanup
#[allow(dead_code)]
pub async fn cleanup_test_tables(pool: &Pool<Postgres>) {
	use reinhardt_db::migrations::{Migration, Operation, SqlDialect};

	// Define cleanup migration - drop tables in reverse order
	let cleanup_migration = Migration::new("cleanup_test_schema", "test")
		.add_operation(Operation::DropTable {
			name: "flatpage_sites".to_string(),
		})
		.add_operation(Operation::DropTable {
			name: "flatpages".to_string(),
		})
		.add_operation(Operation::DropTable {
			name: "sites".to_string(),
		});

	// Generate and execute SQL for each operation
	for operation in &cleanup_migration.operations {
		let sql = operation.to_sql(&SqlDialect::Postgres);
		// Ignore errors during cleanup (tables might not exist)
		let _ = sqlx::query(sql.as_str()).execute(pool).await;
	}
}

/// Test server operating mode
#[allow(dead_code)]
pub enum TestServerMode {
	/// HTTP mode: Actual TCP port listener (for E2E tests)
	Http {
		addr: SocketAddr,
		coordinator: ShutdownCoordinator,
	},
	/// Direct mode: Direct handler invocation (for fast unit tests)
	Direct { handler: Arc<dyn Handler> },
}

/// Test server that can be spawned for integration tests
///
/// Supports two modes:
/// - **HTTP mode**: Actual server listening on a TCP port (for E2E tests)
/// - **Direct mode**: Direct handler invocation without network (for fast unit tests)
#[allow(dead_code)]
pub struct TestServer {
	mode: TestServerMode,
	pub pool: Pool<Postgres>,
}

#[allow(dead_code)]
impl TestServer {
	/// Create a test server in HTTP mode (actual TCP listener)
	///
	/// This mode is suitable for E2E tests that need to make real HTTP requests.
	pub async fn new_http<H: Handler + 'static>(handler: H) -> Self {
		let pool = setup_test_db().await;
		create_flatpages_tables(&pool).await;

		// Create ShutdownCoordinator with 30 second timeout
		let coordinator = ShutdownCoordinator::new(Duration::from_secs(30));

		// Bind to an available port
		let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
			.await
			.expect("Failed to bind to address");
		let addr = listener.local_addr().expect("Failed to get local address");

		// Drop listener to release port for HttpServer
		drop(listener);

		// Start server in background
		let server = HttpServer::new(handler);
		let coordinator_clone = coordinator.clone();

		tokio::spawn(async move {
			let _ = server.listen_with_shutdown(addr, coordinator_clone).await;
		});

		// Wait for server to start
		tokio::time::sleep(Duration::from_millis(100)).await;

		Self {
			mode: TestServerMode::Http { addr, coordinator },
			pool,
		}
	}

	/// Create a test server in Direct mode (handler invocation without network)
	///
	/// This mode is suitable for fast unit tests that don't need actual HTTP.
	pub async fn new_direct<H: Handler + 'static>(handler: H) -> Self {
		let pool = setup_test_db().await;
		create_flatpages_tables(&pool).await;

		Self {
			mode: TestServerMode::Direct {
				handler: Arc::new(handler),
			},
			pool,
		}
	}

	/// Create a test server (backward compatible - uses Direct mode)
	pub async fn new(router: Arc<dyn Handler>) -> Self {
		let pool = setup_test_db().await;
		create_flatpages_tables(&pool).await;

		Self {
			mode: TestServerMode::Direct { handler: router },
			pool,
		}
	}

	/// Get the address (HTTP mode only)
	pub fn addr(&self) -> Option<SocketAddr> {
		match &self.mode {
			TestServerMode::Http { addr, .. } => Some(*addr),
			TestServerMode::Direct { .. } => None,
		}
	}

	/// Get the base URL for the test server (HTTP mode only)
	///
	/// # Panics
	/// Panics if called in Direct mode
	pub fn url(&self, path: &str) -> String {
		match &self.mode {
			TestServerMode::Http { addr, .. } => format!("http://{}{}", addr, path),
			TestServerMode::Direct { .. } => panic!("url() is only available in HTTP mode"),
		}
	}

	/// Execute a request using the appropriate method based on mode
	///
	/// - HTTP mode: Makes actual HTTP request via reqwest
	/// - Direct mode: Invokes handler directly
	pub async fn request(&self, request: Request) -> reinhardt_http::Response {
		match &self.mode {
			TestServerMode::Http { addr, .. } => {
				let client = reqwest::Client::new();
				let url = format!("http://{}{}", addr, request.uri);
				let method = reqwest::Method::from_bytes(request.method.as_str().as_bytes())
					.expect("Invalid method");

				let mut req_builder = client.request(method, &url);

				// Copy headers
				for (name, value) in request.headers.iter() {
					req_builder = req_builder.header(name.as_str(), value.to_str().unwrap_or(""));
				}

				// Set body
				req_builder = req_builder.body(request.body().to_vec());

				let resp = req_builder.send().await.expect("Failed to send request");

				reinhardt_http::Response::new(
					hyper::StatusCode::from_u16(resp.status().as_u16())
						.unwrap_or(hyper::StatusCode::INTERNAL_SERVER_ERROR),
				)
				.with_body(resp.bytes().await.unwrap_or_default())
			}
			TestServerMode::Direct { handler } => {
				handler.handle(request).await.expect("Handler error")
			}
		}
	}

	/// Graceful shutdown (HTTP mode only)
	pub async fn shutdown(&self) {
		if let TestServerMode::Http { coordinator, .. } = &self.mode {
			coordinator.shutdown();
			coordinator.wait_for_shutdown().await;
		}
	}
}

impl Drop for TestServer {
	fn drop(&mut self) {
		if let TestServerMode::Http { coordinator, .. } = &self.mode {
			coordinator.shutdown();
		}
	}
}

pub use reinhardt_test::mock::SimpleHandler;

/// Helper to make HTTP requests in tests
pub async fn make_request(
	router: Arc<dyn Handler>,
	method: &str,
	uri: &str,
	body: Option<String>,
) -> (hyper::StatusCode, String) {
	make_request_with_headers(router, method, uri, body, vec![]).await
}

/// Helper to make HTTP requests with custom headers
pub async fn make_request_with_headers(
	router: Arc<dyn Handler>,
	method: &str,
	uri: &str,
	body: Option<String>,
	headers: Vec<(&str, &str)>,
) -> (hyper::StatusCode, String) {
	let method = method.parse::<hyper::Method>().expect("Invalid method");
	let uri = uri.parse::<hyper::Uri>().expect("Invalid URI");

	let mut header_map = hyper::HeaderMap::new();
	header_map.insert(
		hyper::header::CONTENT_TYPE,
		hyper::header::HeaderValue::from_static("application/json"),
	);

	// Add custom headers
	for (name, value) in headers {
		header_map.insert(
			name.parse::<hyper::header::HeaderName>().unwrap(),
			hyper::header::HeaderValue::from_str(value).unwrap(),
		);
	}

	let body_bytes = body.map(Bytes::from).unwrap_or_default();

	let request = Request::builder()
		.method(method)
		.uri(uri)
		.version(hyper::Version::HTTP_11)
		.headers(header_map)
		.body(body_bytes)
		.build()
		.expect("Failed to build request");

	let response = router
		.handle(request)
		.await
		.expect("Failed to execute request");

	let status = response.status;
	let body_text = String::from_utf8(response.body.to_vec()).expect("Invalid UTF-8 in response");

	(status, body_text)
}
