//! Unit tests for DependencyRegistry and AsyncFactory

use reinhardt_di::registry::{AsyncFactory, DependencyRegistry, DependencyScope, global_registry};
use reinhardt_di::{FactoryTrait, InjectionContext};
use reinhardt_test::fixtures::*;
use rstest::*;
use std::sync::Arc;

// Test type definitions
#[derive(Clone, Debug, PartialEq)]
struct TestService {
	name: String,
}

#[derive(Clone, Debug, PartialEq)]
struct AnotherService {
	id: u32,
}

#[rstest]
fn new_creates_empty_registry() {
	// Act
	let registry = DependencyRegistry::new();

	// Assert
	assert_eq!(registry.len(), 0);
	assert!(registry.is_empty());
}

#[rstest]
#[tokio::test]
async fn register_stores_factory(singleton_scope: Arc<reinhardt_di::SingletonScope>) {
	// Arrange
	let registry = DependencyRegistry::new();
	let ctx = InjectionContext::builder(singleton_scope).build();

	// Act
	registry.register_async::<TestService, _, _>(DependencyScope::Singleton, |_ctx| async {
		Ok(TestService {
			name: "test_service".to_string(),
		})
	});

	// Assert
	assert!(registry.is_registered::<TestService>());
	assert_eq!(registry.len(), 1);

	let service = registry.create::<TestService>(&ctx).await.unwrap();
	assert_eq!(service.name, "test_service");
}

#[rstest]
fn register_with_scope_stores_scope_info() {
	// Arrange
	let registry = DependencyRegistry::new();

	// Act
	registry.register_async::<TestService, _, _>(DependencyScope::Request, |_ctx| async {
		Ok(TestService {
			name: "request_scoped".to_string(),
		})
	});

	registry.register_async::<AnotherService, _, _>(DependencyScope::Singleton, |_ctx| async {
		Ok(AnotherService { id: 42 })
	});

	// Assert
	assert_eq!(
		registry.get_scope::<TestService>(),
		Some(DependencyScope::Request)
	);
	assert_eq!(
		registry.get_scope::<AnotherService>(),
		Some(DependencyScope::Singleton)
	);
}

#[rstest]
#[tokio::test]
async fn get_factory_retrieves_registered_factory(
	singleton_scope: Arc<reinhardt_di::SingletonScope>,
) {
	// Arrange
	let registry = DependencyRegistry::new();
	let ctx = InjectionContext::builder(singleton_scope).build();

	registry.register_async::<TestService, _, _>(DependencyScope::Singleton, |_ctx| async {
		Ok(TestService {
			name: "factory_test".to_string(),
		})
	});

	// Act
	let service = registry.create::<TestService>(&ctx).await.unwrap();

	// Assert
	assert_eq!(service.name, "factory_test");
}

#[rstest]
fn get_scope_returns_correct_scope() {
	// Arrange
	let registry = DependencyRegistry::new();

	registry.register_async::<TestService, _, _>(DependencyScope::Request, |_ctx| async {
		Ok(TestService {
			name: "test".to_string(),
		})
	});

	// Act
	let scope = registry.get_scope::<TestService>();

	// Assert
	assert_eq!(scope, Some(DependencyScope::Request));
}

#[serial_test::serial(global_registry)]
#[rstest]
fn global_registry_is_singleton() {
	// Act
	let registry1 = global_registry();
	let registry2 = global_registry();

	// Assert
	assert!(Arc::ptr_eq(registry1, registry2));
}

#[rstest]
#[tokio::test]
async fn async_factory_creates_instance(singleton_scope: Arc<reinhardt_di::SingletonScope>) {
	// Arrange
	let ctx = InjectionContext::builder(singleton_scope).build();

	let factory = AsyncFactory::new(|_ctx: Arc<InjectionContext>| async {
		Ok(TestService {
			name: "async_factory_test".to_string(),
		})
	});

	// Act
	let result = factory.create(&ctx).await;

	// Assert
	assert!(result.is_ok());
	let any_arc = result.unwrap();
	let service = any_arc.downcast::<TestService>().unwrap();
	assert_eq!(service.name, "async_factory_test");
}
