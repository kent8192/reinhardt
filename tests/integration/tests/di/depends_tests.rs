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

#[rstest]
#[tokio::test]
async fn depends_builder_creates_instance(injection_context: InjectionContext) {
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
	let singleton_scope = Arc::new(reinhardt_di::SingletonScope::new());
	let injection_context = InjectionContext::builder(singleton_scope).build();
	UNCACHED_COUNTER.store(0, Ordering::SeqCst);

	// Act
	let depends1 = Depends::<UncachedConfig>::builder()
		.resolve(&injection_context)
		.await
		.unwrap();

	let depends2 = Depends::<UncachedConfig>::builder()
		.resolve(&injection_context)
		.await
		.unwrap();

	// Assert
	assert_eq!(depends1.id, depends2.id);
	// Cache is enabled, so called only once
	assert_eq!(UNCACHED_COUNTER.load(Ordering::SeqCst), 1);
}

#[serial_test::serial(uncached_counter)]
#[tokio::test]
async fn depends_with_use_cache_false() {
	// Arrange
	let singleton_scope = Arc::new(reinhardt_di::SingletonScope::new());
	let injection_context = InjectionContext::builder(singleton_scope).build();
	UNCACHED_COUNTER.store(0, Ordering::SeqCst);

	// Act
	let depends1 = Depends::<UncachedConfig>::builder_no_cache()
		.resolve(&injection_context)
		.await
		.unwrap();

	let depends2 = Depends::<UncachedConfig>::builder_no_cache()
		.resolve(&injection_context)
		.await
		.unwrap();

	// Assert
	assert_ne!(depends1.id, depends2.id);
	// Cache is disabled, so called twice
	assert_eq!(UNCACHED_COUNTER.load(Ordering::SeqCst), 2);
}

#[rstest]
#[tokio::test]
async fn depends_nested_dependencies(injection_context: InjectionContext) {
	// Act
	let service = NestedService::inject(&injection_context).await;

	// Assert
	assert!(service.is_ok());
	let service = service.unwrap();
	assert_eq!(service.config.value, "config_value");
}
