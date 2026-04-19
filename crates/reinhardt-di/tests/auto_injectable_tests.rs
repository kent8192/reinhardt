//! Tests for automatic Injectable implementation
//!
//! This module tests the `#[injectable]` macro for automatic dependency injection
//! on structs with `#[inject]` and `#[no_inject]` fields.

use reinhardt_di::{Depends, Injectable, InjectionContext, SingletonScope};
use reinhardt_macros::injectable;
use std::sync::Arc;

#[injectable]
#[derive(Clone, Default, Debug, PartialEq)]
struct SimpleConfig {
	#[no_inject(default = Default)]
	host: String,
	#[no_inject(default = Default)]
	port: u16,
}

#[injectable]
#[derive(Clone, Default)]
struct AnotherConfig {
	#[no_inject(default = Default)]
	api_key: String,
}

#[tokio::test]
async fn test_auto_injectable_simple() {
	let singleton_scope = Arc::new(SingletonScope::new());
	let ctx = InjectionContext::builder(singleton_scope).build();
	let config = <SimpleConfig as Injectable>::inject(&ctx).await.unwrap();
	assert_eq!(config.host, "");
	assert_eq!(config.port, 0);
}

#[tokio::test]
async fn test_auto_injectable_with_depends() {
	// Register SimpleConfig in the global registry for Depends<T> resolution
	let registry = reinhardt_di::global_registry();
	registry.register_async::<SimpleConfig, _, _>(
		reinhardt_di::DependencyScope::Request,
		|_ctx| async { Ok(SimpleConfig::default()) },
	);

	let singleton_scope = Arc::new(SingletonScope::new());
	let ctx = InjectionContext::builder(singleton_scope).build();
	let depends_config = Depends::<SimpleConfig>::builder()
		.resolve(&ctx)
		.await
		.unwrap();
	assert_eq!(depends_config.host, "");
	assert_eq!(depends_config.port, 0);
}

#[tokio::test]
async fn test_auto_injectable_caching() {
	let singleton_scope = Arc::new(SingletonScope::new());
	let ctx = InjectionContext::builder(singleton_scope).build();
	let config1 = <SimpleConfig as Injectable>::inject(&ctx).await.unwrap();
	let config2 = <SimpleConfig as Injectable>::inject(&ctx).await.unwrap();
	assert_eq!(config1, config2);
}

#[tokio::test]
async fn test_multiple_auto_injectable_types() {
	let singleton_scope = Arc::new(SingletonScope::new());
	let ctx = InjectionContext::builder(singleton_scope).build();
	let config1 = <SimpleConfig as Injectable>::inject(&ctx).await.unwrap();
	let config2 = <AnotherConfig as Injectable>::inject(&ctx).await.unwrap();
	assert_eq!(config1.host, "");
	assert_eq!(config2.api_key, "");
}

// Custom implementation should still work
struct CustomInjectable {
	value: i32,
}

#[async_trait::async_trait]
impl Injectable for CustomInjectable {
	async fn inject(_ctx: &InjectionContext) -> reinhardt_di::DiResult<Self> {
		Ok(CustomInjectable { value: 42 })
	}
}

#[tokio::test]
async fn test_custom_injectable_override() {
	let singleton_scope = Arc::new(SingletonScope::new());
	let ctx = InjectionContext::builder(singleton_scope).build();

	// Custom implementation should be used
	let custom = CustomInjectable::inject(&ctx).await.unwrap();
	assert_eq!(custom.value, 42);
}
