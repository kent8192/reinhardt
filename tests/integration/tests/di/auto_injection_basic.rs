//! Basic auto injection tests
//!
//! Tests for manual `Injectable` trait implementations registered with the
//! global `DependencyRegistry`, verifying that `InjectionContext::resolve()`
//! and singleton caching work end-to-end.

use reinhardt_di::{DependencyScope, DiResult, InjectionContext, SingletonScope, global_registry};
use serial_test::serial;
use std::sync::Arc;

// Named factory functions so that `register_async` can infer all three type
// parameters (`T`, `F`, `Fut`) from the function item type.
async fn make_app_config(_ctx: Arc<InjectionContext>) -> DiResult<AppConfig> {
	Ok(AppConfig::default())
}

async fn make_database_connection(_ctx: Arc<InjectionContext>) -> DiResult<DatabaseConnection> {
	// `ctx.resolve()` inside a factory future is non-`Sync` due to `RefCell` in
	// `with_cycle_detection_scope`, which conflicts with `register_async`'s
	// `Fut: Sync` bound. We therefore replicate the dependency values inline.
	let config = AppConfig::default();
	Ok(DatabaseConnection {
		url: config.database_url.clone(),
		connected: true,
	})
}

/// Simple config struct registered via `global_registry().register_async()`
#[derive(Clone, Debug, PartialEq)]
struct AppConfig {
	database_url: String,
	api_key: String,
}

impl Default for AppConfig {
	fn default() -> Self {
		Self {
			database_url: "postgres://localhost:5432/test".to_string(),
			api_key: "test-key-12345".to_string(),
		}
	}
}

/// Database connection registered with a factory that depends on AppConfig
#[derive(Clone, Debug)]
struct DatabaseConnection {
	url: String,
	connected: bool,
}

/// Register test types in the global registry (idempotent).
///
/// `DashMap` is thread-safe so duplicate insertions simply overwrite. The
/// `is_registered` guard avoids unnecessary churn across parallel test runs.
fn setup_registry() {
	let registry = global_registry();
	if !registry.is_registered::<AppConfig>() {
		registry.register_async(DependencyScope::Singleton, make_app_config);
	}
	if !registry.is_registered::<DatabaseConnection>() {
		registry.register_async(DependencyScope::Singleton, make_database_connection);
	}
}

#[tokio::test]
#[serial(di_auto_injection)]
async fn test_injectable_macro_registration() {
	// Arrange
	setup_registry();
	let registry = global_registry();

	// Assert
	assert!(
		registry.is_registered::<AppConfig>(),
		"AppConfig should be registered in global registry"
	);
	assert_eq!(
		registry.get_scope::<AppConfig>(),
		Some(DependencyScope::Singleton),
		"AppConfig should have Singleton scope"
	);
}

#[tokio::test]
#[serial(di_auto_injection)]
async fn test_injectable_factory_registration() {
	// Arrange
	setup_registry();
	let registry = global_registry();

	// Assert
	assert!(
		registry.is_registered::<DatabaseConnection>(),
		"DatabaseConnection should be registered in global registry"
	);
	assert_eq!(
		registry.get_scope::<DatabaseConnection>(),
		Some(DependencyScope::Singleton),
		"DatabaseConnection should have Singleton scope"
	);
}

#[tokio::test]
#[serial(di_auto_injection)]
async fn test_resolve_injectable_struct() {
	// Arrange
	setup_registry();
	let singleton_scope = Arc::new(SingletonScope::new());
	let ctx = InjectionContext::builder(singleton_scope).build();

	// Act
	let config = ctx.resolve::<AppConfig>().await;

	// Assert
	assert!(
		config.is_ok(),
		"Should successfully resolve AppConfig: {:?}",
		config.err()
	);
	let config = config.unwrap();
	assert_eq!(config.database_url, "postgres://localhost:5432/test");
	assert_eq!(config.api_key, "test-key-12345");
}

#[tokio::test]
#[serial(di_auto_injection)]
async fn test_resolve_injectable_factory_with_dependency() {
	// Arrange
	setup_registry();
	let singleton_scope = Arc::new(SingletonScope::new());
	let ctx = InjectionContext::builder(singleton_scope).build();

	// Act
	let db = ctx.resolve::<DatabaseConnection>().await;

	// Assert
	assert!(
		db.is_ok(),
		"Should successfully resolve DatabaseConnection: {:?}",
		db.err()
	);
	let db = db.unwrap();
	assert_eq!(db.url, "postgres://localhost:5432/test");
	assert!(db.connected);
}

#[tokio::test]
#[serial(di_auto_injection)]
async fn test_singleton_caching() {
	// Arrange
	setup_registry();
	let singleton_scope = Arc::new(SingletonScope::new());
	let ctx = InjectionContext::builder(singleton_scope).build();

	// Act - first resolution creates and caches the singleton
	let config1 = ctx.resolve::<AppConfig>().await.unwrap();

	// Second resolution should use the cached singleton instance
	let config2 = ctx.resolve::<AppConfig>().await.unwrap();

	// Assert - both resolutions return the same Arc instance
	assert!(
		Arc::ptr_eq(&config1, &config2),
		"Singleton dependencies should return the same Arc instance"
	);
}

/// Helper that exercises the registry `create` path used by `resolve`.
async fn resolve_config() -> DiResult<Arc<AppConfig>> {
	setup_registry();
	let singleton_scope = Arc::new(SingletonScope::new());
	let ctx = InjectionContext::builder(singleton_scope).build();
	ctx.resolve::<AppConfig>().await
}

#[tokio::test]
#[serial(di_auto_injection)]
async fn test_registry_create_path() {
	// Arrange / Act
	let result = resolve_config().await;

	// Assert
	assert!(
		result.is_ok(),
		"Registry create path should succeed: {:?}",
		result.err()
	);
	let config = result.unwrap();
	assert_eq!(config.database_url, "postgres://localhost:5432/test");
}
