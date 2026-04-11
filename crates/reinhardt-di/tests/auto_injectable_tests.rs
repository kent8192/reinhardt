//! Tests for automatic Injectable implementation
//!
//! This module tests the `#[injectable]` macro for automatic dependency injection
//! on structs with `#[inject]` and `#[no_inject]` fields.

use reinhardt_di::{Depends, Injectable, InjectionContext, SingletonScope};
use reinhardt_macros::injectable;
use rstest::*;
use std::sync::Arc;

#[injectable]
#[derive(Default, Debug, PartialEq)]
struct SimpleConfig {
	#[no_inject(default = Default)]
	host: String,
	#[no_inject(default = Default)]
	port: u16,
}

#[injectable]
#[derive(Default)]
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

// --- Clone auto-derive tests ---

/// Struct without explicit `#[derive(Clone)]` — the `#[injectable]` macro should
/// auto-derive Clone for use with `into_inner()` and `injectable_factory` patterns.
#[injectable]
#[derive(Default, Debug, PartialEq)]
struct AutoCloneConfig {
	#[no_inject(default = Default)]
	name: String,
}

/// Struct that already has `#[derive(Clone)]` — should compile without a
/// duplicate derive error.
#[injectable]
#[derive(Clone, Default, Debug, PartialEq)]
struct ExplicitCloneConfig {
	#[no_inject(default = Default)]
	label: String,
}

/// Injectable without explicit Clone should be cloneable
#[rstest]
#[tokio::test]
async fn test_auto_derive_clone_makes_struct_cloneable() {
	// Arrange
	let singleton_scope = Arc::new(SingletonScope::new());
	let ctx = InjectionContext::builder(singleton_scope).build();

	// Act
	let config = <AutoCloneConfig as Injectable>::inject(&ctx).await.unwrap();
	let cloned = config.clone();

	// Assert
	assert_eq!(config, cloned);
}

/// Injectable without explicit Clone should work with Depends<T>
#[rstest]
#[tokio::test]
async fn test_auto_derive_clone_works_with_depends() {
	// Arrange
	// Register AutoCloneConfig in the global registry for Depends<T> resolution
	let registry = reinhardt_di::global_registry();
	registry.register_async::<AutoCloneConfig, _, _>(
		reinhardt_di::DependencyScope::Request,
		|_ctx| async { Ok(AutoCloneConfig::default()) },
	);

	let singleton_scope = Arc::new(SingletonScope::new());
	let ctx = InjectionContext::builder(singleton_scope).build();

	// Act
	let depends_config = Depends::<AutoCloneConfig>::builder()
		.resolve(&ctx)
		.await
		.unwrap();

	// Assert
	assert_eq!(depends_config.name, "");
}

/// Explicit Clone derive should not cause duplicate derive errors
#[rstest]
#[tokio::test]
async fn test_explicit_clone_derive_no_duplicate() {
	// Arrange
	let singleton_scope = Arc::new(SingletonScope::new());
	let ctx = InjectionContext::builder(singleton_scope).build();

	// Act
	let config = <ExplicitCloneConfig as Injectable>::inject(&ctx)
		.await
		.unwrap();
	let cloned = config.clone();

	// Assert
	assert_eq!(config, cloned);
}

// --- Additional edge-case tests ---

/// Unit struct (no fields) with injectable should also auto-derive Clone
#[injectable]
#[derive(Default, Debug, PartialEq)]
struct UnitLikeConfig;

#[rstest]
#[tokio::test]
async fn test_auto_derive_clone_unit_struct() {
	// Arrange
	let singleton_scope = Arc::new(SingletonScope::new());
	let ctx = InjectionContext::builder(singleton_scope).build();

	// Act
	let config = <UnitLikeConfig as Injectable>::inject(&ctx).await.unwrap();
	let cloned = config.clone();

	// Assert
	assert_eq!(config, cloned);
}

/// Struct with multiple derives but no Clone — auto-derive should add it
#[injectable]
#[derive(Default, Debug, PartialEq, Eq, Hash)]
struct MultiDeriveConfig {
	#[no_inject(default = Default)]
	key: String,
}

#[rstest]
#[tokio::test]
async fn test_auto_derive_clone_with_many_existing_derives() {
	// Arrange
	let singleton_scope = Arc::new(SingletonScope::new());
	let ctx = InjectionContext::builder(singleton_scope).build();

	// Act
	let config = <MultiDeriveConfig as Injectable>::inject(&ctx)
		.await
		.unwrap();
	let cloned = config.clone();

	// Assert
	assert_eq!(config, cloned);
}

/// Struct with Clone in a separate derive group should be detected
#[injectable]
#[derive(Default, Debug, Clone, PartialEq)]
struct SplitDeriveConfig {
	#[no_inject(default = Default)]
	value: u32,
}

#[rstest]
#[tokio::test]
async fn test_explicit_clone_in_separate_derive_group_no_duplicate() {
	// Arrange
	let singleton_scope = Arc::new(SingletonScope::new());
	let ctx = InjectionContext::builder(singleton_scope).build();

	// Act
	let config = <SplitDeriveConfig as Injectable>::inject(&ctx)
		.await
		.unwrap();
	let cloned = config.clone();

	// Assert
	assert_eq!(config, cloned);
}

/// Struct with multiple fields — all Clone-able — should auto-derive Clone
#[injectable]
#[derive(Default, Debug, PartialEq)]
struct MultiFieldConfig {
	#[no_inject(default = Default)]
	host: String,
	#[no_inject(default = Default)]
	port: u16,
	#[no_inject(default = Default)]
	enabled: bool,
}

#[rstest]
#[tokio::test]
async fn test_auto_derive_clone_multi_field_struct() {
	// Arrange
	let singleton_scope = Arc::new(SingletonScope::new());
	let ctx = InjectionContext::builder(singleton_scope).build();

	// Act
	let config = <MultiFieldConfig as Injectable>::inject(&ctx)
		.await
		.unwrap();
	let cloned = config.clone();

	// Assert
	assert_eq!(config, cloned);
}

/// Auto-derived Clone produces independent copies (not shared references)
#[rstest]
#[tokio::test]
async fn test_auto_derive_clone_produces_independent_copy() {
	// Arrange
	let singleton_scope = Arc::new(SingletonScope::new());
	let ctx = InjectionContext::builder(singleton_scope).build();
	let mut original = <AutoCloneConfig as Injectable>::inject(&ctx).await.unwrap();

	// Act
	let cloned = original.clone();
	original.name = "modified".to_string();

	// Assert — cloned should retain the original value
	assert_ne!(original, cloned);
	assert_eq!(cloned.name, "");
}
