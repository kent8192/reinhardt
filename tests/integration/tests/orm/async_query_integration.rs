//! Integration tests for async query API with PostgreSQL and MySQL
//!
//! These tests verify that AsyncQuery and AsyncSession work correctly
//! with real database containers using reinhardt-test fixtures.
//!
//! **Test Coverage:**
//! - Async query builder SQL generation (PostgreSQL, MySQL)
//! - Async query execution with real database
//! - Async session operations
//!
//! **Fixtures Used:**
//! - postgres_container: PostgreSQL database container (reinhardt-test)
//! - mysql_suite: MySQL database container (reinhardt-test, planned)

use reinhardt_db::orm::{
	expressions::Q, manager::reinitialize_database, query_execution::QueryCompiler,
	types::DatabaseDialect,
};
use reinhardt_db::{DatabaseConnection, orm::Model};
use reinhardt_integration_tests::migrations::apply_async_query_test_migrations;
use reinhardt_macros::model;
use reinhardt_query::QueryStatementBuilder;
use reinhardt_test::fixtures::postgres_container;
use rstest::*;
use serde::{Deserialize, Serialize};
use serial_test::serial;
use sqlx::PgPool;
use std::sync::Arc;
use testcontainers::{ContainerAsync, GenericImage};

/// Test model for async query tests
#[model(table_name = "test_models")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct TestModel {
	#[field(primary_key = true)]
	id: i64,
	#[field(max_length = 255)]
	name: String,
}

// ========================================================================
// Fixtures
// ========================================================================

/// Dedicated fixture for Async query integration tests
///
/// Uses postgres_container to obtain a container and
/// applies async query test migrations
#[fixture]
async fn async_query_test_db(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) -> (
	ContainerAsync<GenericImage>,
	Arc<DatabaseConnection>,
	u16,
	String,
) {
	let (container, _pool, port, url) = postgres_container.await;

	// Create DatabaseConnection from URL (not from pool)
	let connection = DatabaseConnection::connect(&url).await.unwrap();

	// Apply async query test migrations using MigrationExecutor
	// Note: apply_async_query_test_migrations expects BackendsConnection, so we use inner()
	apply_async_query_test_migrations(connection.inner())
		.await
		.unwrap();

	(container, Arc::new(connection), port, url)
}

// ========================================================================
// PostgreSQL Tests
// ========================================================================

#[cfg(feature = "postgres")]
mod postgres_tests {
	use super::*;

	/// Test basic SQL generation with QueryCompiler for PostgreSQL
	///
	/// **Test Intent**: Verify QueryCompiler generates correct SQL for PostgreSQL dialect
	///
	/// **Integration Point**: QueryCompiler → PostgreSQL SQL syntax
	///
	/// **Not Intent**: Query execution, database operations
	#[rstest]
	#[tokio::test]
	async fn test_postgres_async_query_builder(
		#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
	) {
		let (_container, _pool, _port, _url) = postgres_container.await;

		// Test basic SQL generation with QueryCompiler
		let compiler = QueryCompiler::new(DatabaseDialect::PostgreSQL);
		let stmt = compiler.compile_select::<TestModel>(
			TestModel::table_name(),
			&[],
			Some(&Q::new("age", ">=", "18")),
			&["name"],
			Some(10),
			None,
		);

		let sql = stmt.to_string(reinhardt_query::prelude::PostgresQueryBuilder::new());
		assert!(sql.contains("test_model"));
		assert!(sql.contains("ORDER BY"));
	}

	/// Test async query execution with real PostgreSQL database
	///
	/// **Test Intent**: Verify async query execution (INSERT, COUNT)
	/// works with real PostgreSQL database using ORM
	///
	/// **Integration Point**: ORM Manager API → PostgreSQL database operations
	///
	/// **Not Intent**: Query optimization, complex queries
	#[rstest]
	#[tokio::test]
	#[serial(async_query_db)]
	async fn test_postgres_async_query_execution(
		#[future] async_query_test_db: (
			ContainerAsync<GenericImage>,
			Arc<DatabaseConnection>,
			u16,
			String,
		),
	) {
		let (_container, _connection, _port, url) = async_query_test_db.await;

		// Initialize global database connection for ORM
		reinitialize_database(&url)
			.await
			.expect("Failed to reinitialize database");

		// No CREATE TABLE - migration handles it

		// Insert data with ORM
		let alice = TestModel::new("Alice".to_string());
		let bob = TestModel::new("Bob".to_string());

		let manager = TestModel::objects();
		manager
			.create(&alice)
			.await
			.expect("Failed to insert Alice");
		manager.create(&bob).await.expect("Failed to insert Bob");

		// Count records with ORM
		let count = manager.count().await.expect("Count failed");
		assert_eq!(count, 2);
	}

	/// Test async session operations with PostgreSQL
	///
	/// **Test Intent**: Verify async session can perform basic database operations
	/// (data insertion, existence check) using ORM
	///
	/// **Integration Point**: ORM Manager API → PostgreSQL database
	///
	/// **Not Intent**: Session management, transaction handling
	#[rstest]
	#[tokio::test]
	#[serial(async_query_db)]
	async fn test_postgres_async_session(
		#[future] async_query_test_db: (
			ContainerAsync<GenericImage>,
			Arc<DatabaseConnection>,
			u16,
			String,
		),
	) {
		let (_container, _connection, _port, url) = async_query_test_db.await;

		// Initialize global database connection for ORM
		reinitialize_database(&url)
			.await
			.expect("Failed to reinitialize database");

		// No CREATE TABLE - migration handles it

		// Insert data with ORM
		let test_model = TestModel::new("Test".to_string());

		let manager = TestModel::objects();
		manager.create(&test_model).await.expect("Insert failed");

		// Check existence
		let count = manager.count().await.expect("Exists check failed");
		assert!(count > 0);
	}
}

// ========================================================================
// MySQL Tests
// ========================================================================

#[cfg(any())] // MySQL support not yet enabled in integration tests
mod mysql_tests {
	use super::*;
	use sqlx::mysql::{MySqlPool, MySqlPoolOptions};

	async fn create_mysql_pool(
		container: &ContainerAsync<GenericImage>,
	) -> Result<MySqlPool, sqlx::Error> {
		let port = container
			.get_host_port_ipv4(testcontainers::core::ContainerPort::Tcp(3306))
			.await
			.expect("Failed to get MySQL port");
		let url = format!("mysql://root:test@localhost:{}/test", port);

		// Retry connection up to 30 times with 2 second delay (MySQL takes longer to start)
		for attempt in 1..=30 {
			match MySqlPoolOptions::new()
				.min_connections(1)
				.max_connections(5)
				.acquire_timeout(Duration::from_secs(10))
				.connect(&url)
				.await
			{
				Ok(pool) => return Ok(pool),
				Err(_e) if attempt < 30 => {
					tokio::time::sleep(Duration::from_secs(2)).await;
					continue;
				}
				Err(e) => return Err(e),
			}
		}
		unreachable!()
	}

	#[tokio::test]
	async fn test_mysql_async_query_builder() {
		let mysql_image = GenericImage::new("mysql", "8.0")
			.with_exposed_port(testcontainers::core::ContainerPort::Tcp(3306))
			.with_env_var("MYSQL_ROOT_PASSWORD", "test")
			.with_env_var("MYSQL_DATABASE", "test");

		// Use AsyncRunner for compatibility with #[tokio::test]
		let container = mysql_image
			.start()
			.await
			.expect("Failed to start MySQL container");

		let pool = create_mysql_pool(&container)
			.await
			.expect("Failed to create MySQL pool");

		// Test basic SQL generation with QueryCompiler
		let compiler = QueryCompiler::new(DatabaseDialect::MySQL);
		let stmt = compiler.compile_select::<TestModel>(
			TestModel::table_name(),
			&[],
			Some(&Q::new("age", ">=", "18")),
			&["name"],
			Some(10),
			None,
		);

		let sql = stmt.to_string(reinhardt_query::prelude::MySqlQueryBuilder::new());
		assert!(sql.contains("test_model"));
		assert!(sql.contains("ORDER BY"));

		pool.close().await;
	}

	#[tokio::test]
	async fn test_mysql_async_query_execution() {
		let mysql_image = GenericImage::new("mysql", "8.0")
			.with_exposed_port(testcontainers::core::ContainerPort::Tcp(3306))
			.with_env_var("MYSQL_ROOT_PASSWORD", "test")
			.with_env_var("MYSQL_DATABASE", "test");

		// Use AsyncRunner for compatibility with #[tokio::test]
		let container = mysql_image
			.start()
			.await
			.expect("Failed to start MySQL container");

		let pool = create_mysql_pool(&container)
			.await
			.expect("Failed to create MySQL pool");

		sqlx::query("CREATE TABLE test_models (id INT AUTO_INCREMENT PRIMARY KEY, name TEXT)")
			.execute(&pool)
			.await
			.expect("Failed to create table");

		sqlx::query("INSERT INTO test_models (name) VALUES ('Alice'), ('Bob')")
			.execute(&pool)
			.await
			.expect("Failed to insert data");

		let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM test_models")
			.fetch_one(&pool)
			.await
			.expect("Count failed");
		assert_eq!(count, 2);

		pool.close().await;
	}

	#[tokio::test]
	async fn test_mysql_async_session() {
		// MySQL's "ready for connections" message appears twice:
		// 1st time: during initialization
		// 2nd time: when fully ready
		// So we remove the WaitFor condition and rely on create_mysql_pool's retry logic
		let mysql_image = GenericImage::new("mysql", "8.0")
			.with_exposed_port(testcontainers::core::ContainerPort::Tcp(3306))
			.with_env_var("MYSQL_ROOT_PASSWORD", "test")
			.with_env_var("MYSQL_DATABASE", "test");

		// Use AsyncRunner for compatibility with #[tokio::test]
		let container = mysql_image
			.start()
			.await
			.expect("Failed to start MySQL container");

		// create_mysql_pool will retry up to 30 times (60 seconds total)
		// to ensure MySQL is fully initialized
		let pool = create_mysql_pool(&container)
			.await
			.expect("Failed to create MySQL pool");

		sqlx::query("CREATE TABLE test_models (id INT AUTO_INCREMENT PRIMARY KEY, name TEXT)")
			.execute(&pool)
			.await
			.unwrap();

		sqlx::query("INSERT INTO test_models (name) VALUES ('Test')")
			.execute(&pool)
			.await
			.expect("Insert failed");

		let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM test_models")
			.fetch_one(&pool)
			.await
			.expect("Exists check failed");
		assert!(count > 0);

		pool.close().await;
	}
}
