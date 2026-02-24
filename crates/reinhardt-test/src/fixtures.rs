//! Test fixtures and utilities for Reinhardt framework testing
//!
//! This module provides reusable test fixtures, mock implementations, and
//! TestContainers-based infrastructure for testing Reinhardt applications.
//!
//! ## Module Organization
//!
//! - `loader` - Fixture data loading from JSON, factory patterns
//! - `mock` - mockall-based mock implementations for database backends
//! - `testcontainers` - Docker container fixtures (PostgreSQL, Redis, LocalStack)
//! - `resources` - Suite-wide shared resources with automatic lifecycle management
//! - `migrations` - Migration registry test fixtures with LocalRegistry for isolation
//! - `validator` - Validator integration test fixtures
//! - `auth` - Authentication integration test fixtures
//! - `admin` - Admin panel integration test fixtures
//!
//! ## Usage Examples
//!
//! ### Using Mock Database Backend
//!
//! ```rust,no_run
//! use reinhardt_test::fixtures::*;
//! use rstest::*;
//!
//! #[rstest]
//! fn test_with_mock(mut mock_database: MockDatabaseBackend) {
//!     use reinhardt_db::backends::types::{QueryResult, QueryValue};
//!
//!     // Set expectations
//!     mock_database.expect_execute()
//!         .withf(|sql, params| sql.contains("INSERT") && params.len() == 2)
//!         .times(1)
//!         .returning(|_, _| Ok(QueryResult { rows_affected: 1 }));
//!
//!     // Test code...
//! }
//! ```
//!
//! ### Using TestContainers PostgreSQL
//!
//! ```rust,no_run
//! use reinhardt_test::fixtures::*;
//! use rstest::*;
//!
//! #[rstest]
//! #[tokio::test]
//! async fn test_with_postgres(
//!     #[future] postgres_container: (ContainerAsync<GenericImage>, Arc<sqlx::PgPool>, u16, String)
//! ) {
//!     let (_container, pool, _port, _url) = postgres_container.await;
//!     let result = sqlx::query("SELECT 1").fetch_one(pool.as_ref()).await;
//!     assert!(result.is_ok());
//! }
//! ```
//!
//! ### Using Suite-Wide Resources
//!
//! ```rust,no_run
//! use reinhardt_test::fixtures::*;
//! use rstest::*;
//!
//! #[rstest]
//! #[tokio::test]
//! async fn test_shared_postgres(postgres_suite: SuiteGuard<PostgresSuiteResource>) {
//!     let pool = &postgres_suite.pool;
//!     // Pool is shared across all tests in suite
//! }
//! ```
//!
//! ### Using Migration Registry Fixture
//!
//! ```rust,no_run
//! use reinhardt_test::fixtures::*;
//! use reinhardt_db::migrations::Migration;
//! use reinhardt_db::migrations::registry::MigrationRegistry;
//! use rstest::*;
//!
//! #[rstest]
//! fn test_migration_registration(migration_registry: reinhardt_db::migrations::registry::LocalRegistry) {
//!     let migration = Migration {
//!         app_label: "polls".to_string(),
//!         name: "0001_initial".to_string(),
//!         operations: vec![],
//!         dependencies: vec![],
//!     };
//!
//!     migration_registry.register(migration).unwrap();
//!     assert_eq!(migration_registry.all_migrations().len(), 1);
//! }
//! ```

// Module declarations
pub mod client;
pub mod loader;
pub mod mock;
pub mod server;

#[cfg(feature = "testcontainers")]
pub mod resources;
#[cfg(feature = "testcontainers")]
pub mod shared_postgres;
#[cfg(feature = "testcontainers")]
pub mod testcontainers;

// New fixture modules for integration tests
#[cfg(feature = "admin")]
pub mod admin;
#[cfg(all(feature = "admin", feature = "testcontainers"))]
pub mod admin_panel;
pub mod auth;

#[cfg(feature = "testcontainers")]
pub mod validator;

// Migration registry test fixtures
pub mod migrations;

// Schema creation fixtures for model-based table creation
#[cfg(feature = "testcontainers")]
pub mod schema;

// Dependency Injection test fixtures
pub mod di;

// Server function test fixtures
#[cfg(feature = "server-fn-test")]
pub mod server_fn;

// WASM frontend test fixtures
#[cfg(all(target_arch = "wasm32", feature = "wasm"))]
pub mod wasm;

// DCL (Data Control Language) test fixtures
pub mod dcl;

// Re-export commonly used items from submodules

// From client module
pub use client::api_client_from_url;

// From loader module
pub use loader::{
	Factory, FactoryBuilder, FixtureError, FixtureLoader, FixtureResult, api_client,
	fixture_loader, random_test_key, temp_dir, test_config_value,
};

// From mock module
pub use mock::{MockDatabaseBackend, mock_connection, mock_database};

// From server module
pub use server::{
	BasicHandler, TestServer, TestServerBuilder, TestServerGuard, http_client, http1_server,
	http2_server, server_with_di, server_with_middleware_chain, server_with_rate_limit,
	server_with_timeout, test_server_guard,
};

#[cfg(feature = "websockets")]
pub use server::{websocket_client, websocket_server};

#[cfg(feature = "graphql")]
pub use server::graphql_server;

// From testcontainers module (conditional on feature)
#[cfg(feature = "testcontainers")]
pub use testcontainers::{
	FileLockGuard, cockroachdb_container, create_test_any_pool, localstack_fixture,
	mongodb_container, mysql_container, mysql_with_all_migrations, mysql_with_apps_migrations,
	mysql_with_migrations_from, postgres_container, postgres_with_all_migrations,
	postgres_with_apps_migrations, postgres_with_migrations_from, rabbitmq_container,
	redis_container, sqlite_with_all_migrations, sqlite_with_apps_migrations,
	sqlite_with_migrations_from,
};

// From shared_postgres module (conditional on feature)
#[cfg(feature = "testcontainers")]
pub use shared_postgres::{
	SharedPostgres, cleanup_shared_postgres, get_shared_postgres, get_test_pool,
	get_test_pool_with_orm, get_test_pool_with_table, shared_db_pool,
};

// From resources module (conditional on feature)
#[cfg(feature = "testcontainers")]
pub use resources::{MySqlSuiteResource, PostgresSuiteResource, mysql_suite, postgres_suite};

// Re-export testcontainers types for convenience
#[cfg(feature = "testcontainers")]
pub use testcontainers::ContainerAsync;
#[cfg(feature = "testcontainers")]
pub use testcontainers::GenericImage;

// From migrations module
pub use migrations::{
	InMemoryRepository, TestMigrationSource, in_memory_repository, migration_registry,
	test_migration_source,
};

// Re-export migration types for use in test schemas
// These are re-exported from reinhardt-migrations to allow functional crates
// to use them in tests without adding reinhardt-migrations to dev-dependencies
pub use reinhardt_db::migrations::{ColumnDefinition, FieldType, Operation, SqlDialect};

// Migration fixtures (conditional on testcontainers feature)
#[cfg(feature = "testcontainers")]
pub use migrations::{
	MigrationExecutorFixture, PostgresTableCreator, migration_executor, postgres_table_creator,
};

// Admin integration fixtures (conditional on admin + testcontainers features)
#[cfg(all(feature = "admin", feature = "testcontainers"))]
pub use migrations::{AdminTableCreator, admin_table_creator};

// From di module
pub use di::{
	injection_context, injection_context_with_database, injection_context_with_overrides,
	injection_context_with_sqlite, singleton_scope,
};

// From schema module (conditional on feature)
#[cfg(feature = "testcontainers")]
pub use schema::{
	ModelSchemaInfo, SchemaError, create_migration_from_model, create_table_for_model,
	create_table_operation_from_model, create_table_operations_from_models,
	create_tables_for_models, extract_model_dependencies, field_info_to_column_definition,
	resolve_model_order,
};
