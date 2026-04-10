//! Dependency Injection fixtures for testing
//!
//! This module provides rstest fixtures for FastAPI-style dependency injection testing.
//! It simplifies the setup of `InjectionContext` and `SingletonScope` in tests.
//!
//! ## Key Fixtures
//!
//! - [`singleton_scope`]: Provides an `Arc<SingletonScope>` for dependency caching
//! - [`injection_context`]: Provides an `InjectionContext` with empty request scope
//!
//! ## Usage Example
//!
//! ```rust,no_run
//! use reinhardt_di::{Injectable, DiResult};
//! use reinhardt_testkit::fixtures::{injection_context, singleton_scope};
//! use rstest::*;
//!
//! #[derive(Clone, Debug)]
//! struct Database {
//!     connection_string: String,
//! }
//!
//! #[async_trait::async_trait]
//! impl Injectable for Database {
//!     async fn inject(_ctx: &reinhardt_di::InjectionContext) -> DiResult<Self> {
//!         Ok(Database {
//!             connection_string: "postgres://localhost/test".to_string(),
//!         })
//!     }
//! }
//!
//! #[rstest]
//! #[tokio::test]
//! async fn test_with_di_fixture(injection_context: reinhardt_di::InjectionContext) {
//!     let db = Database::inject(&injection_context).await.unwrap();
//!     assert_eq!(db.connection_string, "postgres://localhost/test");
//! }
//! ```
//!
//! ## FastAPI-Style Pattern
//!
//! Similar to FastAPI's `Depends()`, these fixtures enable clean dependency injection
//! in tests without boilerplate setup code:
//!
//! ```rust,no_run
//! use reinhardt_di::Depends;
//! use reinhardt_testkit::fixtures::injection_context;
//! use rstest::*;
//!
//! #[rstest]
//! #[tokio::test]
//! async fn test_depends_pattern(injection_context: reinhardt_di::InjectionContext) {
//!     // Use Depends<T> for automatic dependency resolution
//!     let config = Depends::<Config>::builder()
//!         .resolve(&injection_context)
//!         .await
//!         .unwrap();
//!
//!     // Test with resolved dependency
//! }
//! ```
//!
//! ## Dependency Overrides
//!
//! Similar to FastAPI's `app.dependency_overrides`, you can override dependencies
//! in the singleton scope for testing:
//!
//! ```rust,no_run
//! use reinhardt_di::{Injectable, DiResult, InjectionContext};
//! use reinhardt_testkit::fixtures::{injection_context_with_overrides, singleton_scope};
//! use rstest::*;
//! use std::sync::Arc;
//!
//! #[derive(Clone, Debug)]
//! struct Database {
//!     url: String,
//! }
//!
//! #[async_trait::async_trait]
//! impl Injectable for Database {
//!     async fn inject(_ctx: &InjectionContext) -> DiResult<Self> {
//!         Ok(Database { url: "prod://db".to_string() })
//!     }
//! }
//!
//! #[rstest]
//! #[tokio::test]
//! async fn test_with_mock_database(singleton_scope: Arc<reinhardt_di::SingletonScope>) {
//!     // Override Database with a mock
//!     let mock_db = Database { url: "test://db".to_string() };
//!     singleton_scope.set(mock_db);
//!
//!     let ctx = reinhardt_di::InjectionContext::builder(singleton_scope).build();
//!
//!     // This will return the mock database from singleton scope
//!     let db = Database::inject(&ctx).await.unwrap();
//!     assert_eq!(db.url, "test://db");
//! }
//! ```

use reinhardt_di::{InjectionContext, SingletonScope};
use rstest::*;
use std::sync::Arc;

/// Fixture providing a singleton scope for dependency injection.
///
/// Creates a new `SingletonScope` wrapped in `Arc` for each test.
/// This scope can be used to cache singleton dependencies across
/// the lifetime of a test.
///
/// # Returns
///
/// `Arc<SingletonScope>` - A thread-safe singleton scope instance
///
/// # Examples
///
/// ```rust,no_run
/// use reinhardt_di::SingletonScope;
/// use reinhardt_testkit::fixtures::singleton_scope;
/// use rstest::*;
/// use std::sync::Arc;
///
/// #[rstest]
/// #[tokio::test]
/// async fn test_singleton_scope(singleton_scope: Arc<SingletonScope>) {
///     // Use singleton_scope for manual scope management
///     singleton_scope.set("test_value".to_string());
///     let value: Option<Arc<String>> = singleton_scope.get();
///     assert_eq!(*value.unwrap(), "test_value");
/// }
/// ```
#[fixture]
pub fn singleton_scope() -> Arc<SingletonScope> {
	Arc::new(SingletonScope::new())
}

/// Fixture providing an injection context for dependency injection.
///
/// Creates a new `InjectionContext` with an empty request scope.
/// The context is automatically configured with a singleton scope
/// from the `singleton_scope` fixture.
///
/// This fixture is the primary entry point for FastAPI-style dependency
/// injection in tests. It eliminates the boilerplate of manually creating
/// `SingletonScope` and `InjectionContext` in every test.
///
/// # Dependencies
///
/// - `singleton_scope`: Automatically resolved by rstest
///
/// # Returns
///
/// `InjectionContext` - A configured injection context ready for use
///
/// # Examples
///
/// ```rust,no_run
/// use reinhardt_di::{Injectable, InjectionContext, DiResult};
/// use reinhardt_testkit::fixtures::injection_context;
/// use rstest::*;
///
/// #[derive(Clone)]
/// struct Config {
///     api_key: String,
/// }
///
/// #[async_trait::async_trait]
/// impl Injectable for Config {
///     async fn inject(_ctx: &InjectionContext) -> DiResult<Self> {
///         Ok(Config {
///             api_key: "test_key".to_string(),
///         })
///     }
/// }
///
/// #[rstest]
/// #[tokio::test]
/// async fn test_injection(injection_context: InjectionContext) {
///     let config = Config::inject(&injection_context).await.unwrap();
///     assert_eq!(config.api_key, "test_key");
/// }
/// ```
///
/// ## With `Depends<T>`
///
/// ```rust,no_run
/// use reinhardt_di::{Depends, Injectable, InjectionContext, DiResult};
/// use reinhardt_testkit::fixtures::injection_context;
/// use rstest::*;
///
/// #[derive(Clone, Default)]
/// struct Database {
///     url: String,
/// }
///
/// #[async_trait::async_trait]
/// impl Injectable for Database {
///     async fn inject(_ctx: &InjectionContext) -> DiResult<Self> {
///         Ok(Database {
///             url: "postgres://localhost/db".to_string(),
///         })
///     }
/// }
///
/// #[rstest]
/// #[tokio::test]
/// async fn test_with_depends(injection_context: InjectionContext) {
///     // FastAPI-style dependency resolution
///     let db = Depends::<Database>::builder()
///         .resolve(&injection_context)
///         .await
///         .unwrap();
///
///     assert_eq!(db.url, "postgres://localhost/db");
/// }
/// ```
#[fixture]
pub fn injection_context(singleton_scope: Arc<SingletonScope>) -> InjectionContext {
	InjectionContext::builder(singleton_scope).build()
}

/// Helper function to create an injection context with dependency overrides.
///
/// Similar to FastAPI's `app.dependency_overrides`, this function allows you to
/// pre-populate the singleton scope with mock or test values that will be returned
/// instead of calling the `Injectable::inject()` implementation.
///
/// This is useful for:
/// - Replacing database connections with test databases
/// - Injecting mock services for unit testing
/// - Providing test configurations
///
/// # Arguments
///
/// * `singleton_scope` - The singleton scope to use (typically from the fixture)
/// * `overrides` - A closure that receives a mutable reference to the singleton scope
///   and can set override values using `scope.set(value)`
///
/// # Returns
///
/// `InjectionContext` - A configured injection context with overrides applied
///
/// # Examples
///
/// ## Basic Override
///
/// ```rust,no_run
/// use reinhardt_di::{Injectable, DiResult, InjectionContext};
/// use reinhardt_testkit::fixtures::{injection_context_with_overrides, singleton_scope};
/// use rstest::*;
/// use std::sync::Arc;
///
/// #[derive(Clone, Debug, PartialEq)]
/// struct Database {
///     url: String,
/// }
///
/// #[async_trait::async_trait]
/// impl Injectable for Database {
///     async fn inject(_ctx: &InjectionContext) -> DiResult<Self> {
///         // Production implementation
///         Ok(Database { url: "prod://db".to_string() })
///     }
/// }
///
/// #[rstest]
/// #[tokio::test]
/// async fn test_with_mock_db(singleton_scope: Arc<reinhardt_di::SingletonScope>) {
///     let ctx = reinhardt_testkit::fixtures::injection_context_with_overrides(
///         singleton_scope,
///         |scope| {
///             // Override Database with test value
///             scope.set(Database { url: "test://db".to_string() });
///         },
///     );
///
///     // Database::inject will return the test value from singleton scope
///     let db = Database::inject(&ctx).await.unwrap();
///     assert_eq!(db.url, "test://db");
/// }
/// ```
///
/// ## Multiple Overrides
///
/// ```rust,no_run
/// use reinhardt_di::{Injectable, DiResult, InjectionContext};
/// use reinhardt_testkit::fixtures::{injection_context_with_overrides, singleton_scope};
/// use rstest::*;
/// use std::sync::Arc;
///
/// #[derive(Clone)]
/// struct Config {
///     api_key: String,
/// }
///
/// #[derive(Clone)]
/// struct Database {
///     url: String,
/// }
///
/// #[async_trait::async_trait]
/// impl Injectable for Config {
///     async fn inject(_ctx: &InjectionContext) -> DiResult<Self> {
///         Ok(Config { api_key: "prod_key".to_string() })
///     }
/// }
///
/// #[async_trait::async_trait]
/// impl Injectable for Database {
///     async fn inject(_ctx: &InjectionContext) -> DiResult<Self> {
///         Ok(Database { url: "prod://db".to_string() })
///     }
/// }
///
/// #[rstest]
/// #[tokio::test]
/// async fn test_with_multiple_mocks(singleton_scope: Arc<reinhardt_di::SingletonScope>) {
///     let ctx = reinhardt_testkit::fixtures::injection_context_with_overrides(
///         singleton_scope,
///         |scope| {
///             // Override multiple dependencies
///             scope.set(Config { api_key: "test_key".to_string() });
///             scope.set(Database { url: "test://db".to_string() });
///         },
///     );
///
///     let config = Config::inject(&ctx).await.unwrap();
///     let db = Database::inject(&ctx).await.unwrap();
///
///     assert_eq!(config.api_key, "test_key");
///     assert_eq!(db.url, "test://db");
/// }
/// ```
pub fn injection_context_with_overrides<F>(
	singleton_scope: Arc<SingletonScope>,
	overrides: F,
) -> InjectionContext
where
	F: FnOnce(&SingletonScope),
{
	// Apply overrides to singleton scope
	overrides(&singleton_scope);

	// Build context with overridden singleton scope
	InjectionContext::builder(singleton_scope).build()
}

// ============================================================================
// Server Function Testing with Database Connection
// ============================================================================

/// Fixture providing an injection context with a SQLite database connection.
///
/// This fixture is designed for testing server functions that use `#[inject]`
/// to receive a `DatabaseConnection`. It creates a temporary SQLite database
/// and registers the connection in the singleton scope.
///
/// # Returns
///
/// A tuple containing:
/// - `tempfile::NamedTempFile`: The temporary database file (must be kept alive)
/// - `InjectionContext`: The DI context with `DatabaseConnection` registered
///
/// # Examples
///
/// ```rust,no_run
/// use reinhardt_testkit::fixtures::injection_context_with_sqlite;
/// use rstest::*;
///
/// #[rstest]
/// #[tokio::test]
/// async fn test_server_function(
///     #[future] injection_context_with_sqlite: (tempfile::NamedTempFile, reinhardt_di::InjectionContext),
/// ) {
///     let (_temp_file, ctx) = injection_context_with_sqlite.await;
///
///     // Server functions can now resolve DatabaseConnection from the context
///     // let result = my_server_function().await;
/// }
/// ```
///
/// # Note
///
/// The `NamedTempFile` must be kept alive for the duration of the test.
/// When it goes out of scope, the temporary database file will be deleted.
#[fixture]
pub async fn injection_context_with_sqlite() -> (tempfile::NamedTempFile, InjectionContext) {
	use reinhardt_db::orm::connection::DatabaseConnection;

	// Create temp file for SQLite database
	let temp_file = tempfile::NamedTempFile::new().expect("Failed to create temp file");
	let db_path = temp_file.path().to_str().unwrap().to_string();
	let database_url = format!("sqlite://{}?mode=rwc", db_path);

	// Create DatabaseConnection using ORM layer API
	let db_conn = DatabaseConnection::connect_sqlite(&database_url)
		.await
		.expect("Failed to create DatabaseConnection");

	// Build DI context with DatabaseConnection registered in singleton scope
	let singleton_scope = Arc::new(SingletonScope::new());
	singleton_scope.set(db_conn);

	let ctx = InjectionContext::builder(singleton_scope).build();

	(temp_file, ctx)
}

/// Helper function to create an injection context with a custom database URL.
///
/// This is useful when you need to connect to a specific database
/// (e.g., PostgreSQL, MySQL) for testing.
///
/// # Arguments
///
/// * `database_url` - The database connection URL
///
/// # Returns
///
/// `InjectionContext` - A configured injection context with `DatabaseConnection` registered
///
/// # Examples
///
/// ```rust,no_run
/// use reinhardt_testkit::fixtures::injection_context_with_database;
///
/// #[tokio::test]
/// async fn test_with_postgres() {
///     let ctx = injection_context_with_database("postgres://localhost/test").await;
///     // Use ctx for testing
/// }
/// ```
pub async fn injection_context_with_database(database_url: &str) -> InjectionContext {
	use reinhardt_db::orm::connection::DatabaseConnection;

	// Create DatabaseConnection
	let db_conn = DatabaseConnection::connect(database_url)
		.await
		.expect("Failed to create DatabaseConnection");

	// Build DI context with DatabaseConnection registered
	let singleton_scope = Arc::new(SingletonScope::new());
	singleton_scope.set(db_conn);

	InjectionContext::builder(singleton_scope).build()
}

#[cfg(test)]
mod tests {
	use super::*;
	use reinhardt_di::{Depends, DiResult, Injectable};

	// Test structures
	#[derive(Clone, Debug, PartialEq)]
	struct TestConfig {
		value: String,
	}

	#[async_trait::async_trait]
	impl Injectable for TestConfig {
		async fn inject(_ctx: &InjectionContext) -> DiResult<Self> {
			Ok(TestConfig {
				value: "test_config".to_string(),
			})
		}
	}

	#[rstest]
	#[tokio::test]
	async fn test_singleton_scope_fixture(singleton_scope: Arc<SingletonScope>) {
		// Verify singleton_scope is created
		assert!(
			singleton_scope.get::<String>().is_none(),
			"Singleton scope should be empty initially"
		);

		// Set a value
		singleton_scope.set("test".to_string());

		// Retrieve the value
		let value: Option<Arc<String>> = singleton_scope.get();
		assert_eq!(*value.unwrap(), "test");
	}

	#[rstest]
	#[tokio::test]
	async fn test_injection_context_fixture(injection_context: InjectionContext) {
		// Verify injection_context is created and works
		let config = TestConfig::inject(&injection_context).await.unwrap();
		assert_eq!(config.value, "test_config");
	}

	#[rstest]
	#[tokio::test]
	async fn test_fixture_isolation_first(injection_context: InjectionContext) {
		// Set a value in request scope
		injection_context.set_request("first".to_string());

		// Verify we can retrieve it
		let value: Option<Arc<String>> = injection_context.get_request();
		assert_eq!(*value.unwrap(), "first");
	}

	#[rstest]
	#[tokio::test]
	async fn test_fixture_isolation_second(injection_context: InjectionContext) {
		// This test should NOT see the value from test_fixture_isolation_first
		let value: Option<Arc<String>> = injection_context.get_request();
		assert!(
			value.is_none(),
			"Request scope should be isolated between tests"
		);
	}

	#[rstest]
	#[tokio::test]
	async fn test_depends_with_fixtures(injection_context: InjectionContext) {
		// Register TestConfig in the global registry so Depends<T> can resolve it
		let registry = reinhardt_di::global_registry();
		registry.register_async::<TestConfig, _, _>(
			reinhardt_di::DependencyScope::Request,
			|_ctx| async {
				Ok(TestConfig {
					value: "test_config".to_string(),
				})
			},
		);

		// Test Depends<T> integration with fixtures
		let config = Depends::<TestConfig>::builder()
			.resolve(&injection_context)
			.await
			.unwrap();

		assert_eq!(config.value, "test_config");
	}

	#[rstest]
	#[tokio::test]
	async fn test_request_scope_caching(injection_context: InjectionContext) {
		// First injection - creates and caches
		let config1 = TestConfig::inject(&injection_context).await.unwrap();

		// Second injection - should return same instance from cache
		let config2 = TestConfig::inject(&injection_context).await.unwrap();

		// Verify both are from the same request scope
		assert_eq!(config1, config2);
	}

	#[rstest]
	#[tokio::test]
	async fn test_singleton_scope_sharing(singleton_scope: Arc<SingletonScope>) {
		// Test that singleton scope can be shared across contexts
		let ctx1 = InjectionContext::builder(Arc::clone(&singleton_scope)).build();
		let ctx2 = InjectionContext::builder(Arc::clone(&singleton_scope)).build();

		// Set a value in singleton scope via ctx1
		ctx1.set_singleton("shared_value".to_string());

		// Retrieve from ctx2 - should see the shared value
		let value: Option<Arc<String>> = ctx2.get_singleton();
		assert_eq!(*value.unwrap(), "shared_value");
	}

	// Tests for injection_context_with_overrides

	#[derive(Clone, Debug, PartialEq)]
	struct MockDatabase {
		url: String,
	}

	#[async_trait::async_trait]
	impl Injectable for MockDatabase {
		async fn inject(ctx: &InjectionContext) -> DiResult<Self> {
			// Check singleton scope first
			if let Some(db) = ctx.get_singleton::<MockDatabase>() {
				return Ok((*db).clone());
			}

			// Default production implementation
			Ok(MockDatabase {
				url: "prod://database".to_string(),
			})
		}
	}

	#[rstest]
	#[tokio::test]
	async fn test_injection_context_with_overrides_basic(singleton_scope: Arc<SingletonScope>) {
		// Create context with override
		let ctx = injection_context_with_overrides(singleton_scope, |scope| {
			scope.set(MockDatabase {
				url: "test://database".to_string(),
			});
		});

		// Inject should return the overridden value
		let db = MockDatabase::inject(&ctx).await.unwrap();
		assert_eq!(db.url, "test://database");
	}

	#[derive(Clone, Debug, PartialEq)]
	struct MockConfig {
		api_key: String,
	}

	#[async_trait::async_trait]
	impl Injectable for MockConfig {
		async fn inject(ctx: &InjectionContext) -> DiResult<Self> {
			// Check singleton scope first
			if let Some(config) = ctx.get_singleton::<MockConfig>() {
				return Ok((*config).clone());
			}

			// Default production implementation
			Ok(MockConfig {
				api_key: "prod_key".to_string(),
			})
		}
	}

	#[rstest]
	#[tokio::test]
	async fn test_injection_context_with_overrides_multiple(singleton_scope: Arc<SingletonScope>) {
		// Create context with multiple overrides
		let ctx = injection_context_with_overrides(singleton_scope, |scope| {
			scope.set(MockDatabase {
				url: "test://database".to_string(),
			});
			scope.set(MockConfig {
				api_key: "test_key".to_string(),
			});
		});

		// Both injections should return overridden values
		let db = MockDatabase::inject(&ctx).await.unwrap();
		let config = MockConfig::inject(&ctx).await.unwrap();

		assert_eq!(db.url, "test://database");
		assert_eq!(config.api_key, "test_key");
	}

	#[rstest]
	#[tokio::test]
	async fn test_injection_context_without_overrides_uses_default(
		singleton_scope: Arc<SingletonScope>,
	) {
		// Create context WITHOUT overrides
		let ctx = InjectionContext::builder(singleton_scope).build();

		// Should return production values
		let db = MockDatabase::inject(&ctx).await.unwrap();
		let config = MockConfig::inject(&ctx).await.unwrap();

		assert_eq!(db.url, "prod://database");
		assert_eq!(config.api_key, "prod_key");
	}

	#[rstest]
	#[tokio::test]
	async fn test_injection_context_with_overrides_and_depends(
		singleton_scope: Arc<SingletonScope>,
	) {
		// Create context with override
		let ctx = injection_context_with_overrides(singleton_scope, |scope| {
			scope.set(MockDatabase {
				url: "test://database".to_string(),
			});
		});

		// Use Depends<T> - should also get the overridden value
		let db = Depends::<MockDatabase>::builder()
			.resolve(&ctx)
			.await
			.unwrap();

		assert_eq!(db.url, "test://database");
	}
}
