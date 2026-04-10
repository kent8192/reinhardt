//! Unit tests for Depends<T> and DependsBuilder

use async_trait::async_trait;
use reinhardt_di::{Depends, DiResult, Injectable, InjectionContext};
use reinhardt_test::fixtures::*;
use rstest::*;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

// Test type definitions
#[derive(Clone, Debug, PartialEq)]
struct Config {
	value: String,
}

#[async_trait]
impl Injectable for Config {
	async fn inject(_ctx: &InjectionContext) -> DiResult<Self> {
		Ok(Config {
			value: "config_value".to_string(),
		})
	}
}

#[derive(Clone, Debug, PartialEq)]
struct UncachedConfig {
	id: u32,
}

static UNCACHED_COUNTER: AtomicU32 = AtomicU32::new(0);

#[async_trait]
impl Injectable for UncachedConfig {
	async fn inject(_ctx: &InjectionContext) -> DiResult<Self> {
		let id = UNCACHED_COUNTER.fetch_add(1, Ordering::SeqCst) + 1;
		Ok(UncachedConfig { id })
	}

	async fn inject_uncached(ctx: &InjectionContext) -> DiResult<Self> {
		Self::inject(ctx).await
	}
}

#[derive(Clone, Debug, PartialEq)]
struct NestedService {
	config: Config,
}

#[async_trait]
impl Injectable for NestedService {
	async fn inject(ctx: &InjectionContext) -> DiResult<Self> {
		let config_depends = Depends::<Config>::builder().resolve(ctx).await?;
		Ok(NestedService {
			config: config_depends.into_inner(),
		})
	}
}

/// Register test types in the global registry for Depends<T> resolution.
/// Depends<T> resolves via ctx.resolve() which requires registry entries.
fn register_test_types() {
	let registry = reinhardt_di::global_registry();
	if !registry.is_registered::<Config>() {
		registry.register::<Config>(
			reinhardt_di::DependencyScope::Request,
			reinhardt_di::InjectableFactory::<Config>::new(),
		);
	}
	if !registry.is_registered::<UncachedConfig>() {
		registry.register::<UncachedConfig>(
			reinhardt_di::DependencyScope::Transient,
			reinhardt_di::InjectableFactory::<UncachedConfig>::new(),
		);
	}
	if !registry.is_registered::<NestedService>() {
		registry.register::<NestedService>(
			reinhardt_di::DependencyScope::Request,
			reinhardt_di::InjectableFactory::<NestedService>::new(),
		);
	}
}

#[rstest]
#[tokio::test]
async fn depends_builder_creates_instance(injection_context: InjectionContext) {
	register_test_types();

	// Act
	let depends = Depends::<Config>::builder()
		.resolve(&injection_context)
		.await;

	// Assert
	assert!(depends.is_ok());
	assert_eq!(depends.unwrap().value, "config_value");
}

#[rstest]
#[tokio::test]
async fn depends_resolve_calls_injectable(injection_context: InjectionContext) {
	register_test_types();

	// Act
	let depends = Depends::<Config>::resolve(&injection_context, true).await;

	// Assert
	assert!(depends.is_ok());
	let config = depends.unwrap();
	assert_eq!(config.value, "config_value");
}

#[serial_test::serial(uncached_counter)]
#[tokio::test]
async fn depends_with_use_cache_true() {
	// Arrange
	register_test_types();
	let singleton_scope = Arc::new(reinhardt_di::SingletonScope::new());
	let injection_context = InjectionContext::builder(singleton_scope).build();
	UNCACHED_COUNTER.store(0, Ordering::SeqCst);

	// Act — UncachedConfig is registered with Transient scope, so each
	// resolve creates a new instance regardless of use_cache flag.
	// The builder() (use_cache=true) no longer affects caching; scope does.
	let depends1 = Depends::<UncachedConfig>::builder()
		.resolve(&injection_context)
		.await
		.unwrap();

	let depends2 = Depends::<UncachedConfig>::builder()
		.resolve(&injection_context)
		.await
		.unwrap();

	// Assert — Transient scope: factory is called each time
	assert_ne!(depends1.id, depends2.id);
	assert_eq!(UNCACHED_COUNTER.load(Ordering::SeqCst), 2);
}

#[serial_test::serial(uncached_counter)]
#[tokio::test]
async fn depends_with_use_cache_false() {
	// Arrange
	register_test_types();
	let singleton_scope = Arc::new(reinhardt_di::SingletonScope::new());
	let injection_context = InjectionContext::builder(singleton_scope).build();
	UNCACHED_COUNTER.store(0, Ordering::SeqCst);

	// Act — UncachedConfig is registered with Transient scope, so each
	// resolve creates a new instance. builder_no_cache() behaves the same
	// as builder() since caching is now scope-driven, not per-call.
	let depends1 = Depends::<UncachedConfig>::builder_no_cache()
		.resolve(&injection_context)
		.await
		.unwrap();

	let depends2 = Depends::<UncachedConfig>::builder_no_cache()
		.resolve(&injection_context)
		.await
		.unwrap();

	// Assert — Transient scope: factory is called each time
	assert_ne!(depends1.id, depends2.id);
	assert_eq!(UNCACHED_COUNTER.load(Ordering::SeqCst), 2);
}

#[rstest]
#[tokio::test]
async fn depends_nested_dependencies(injection_context: InjectionContext) {
	register_test_types();

	// Act
	let service = NestedService::inject(&injection_context).await;

	// Assert
	assert!(service.is_ok());
	let service = service.unwrap();
	assert_eq!(service.config.value, "config_value");
}
