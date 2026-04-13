//! Tests for FastAPI-style Depends functionality

#![cfg(feature = "macros")]

use reinhardt_di::{
	DependencyScope, Depends, Injectable, InjectionContext, SingletonScope, global_registry,
	injectable,
};
use serial_test::serial;
use std::sync::atomic::{AtomicUsize, Ordering};

#[injectable]
#[derive(Clone, Default, Debug, PartialEq)]
struct CommonQueryParams {
	#[no_inject]
	q: Option<String>,
	#[no_inject]
	skip: usize,
	#[no_inject]
	limit: usize,
}

#[injectable]
#[derive(Clone, Default)]
struct Database {
	#[no_inject]
	connection_count: usize,
}

#[injectable]
#[derive(Clone, Default)]
struct Config {
	#[no_inject]
	api_key: String,
}

// Custom Injectable with instance counter (thread-safe using AtomicUsize)
static INSTANCE_COUNTER: AtomicUsize = AtomicUsize::new(0);

#[derive(Clone)]
struct CountedService {
	instance_id: usize,
}

#[async_trait::async_trait]
impl Injectable for CountedService {
	async fn inject(_ctx: &InjectionContext) -> reinhardt_di::DiResult<Self> {
		let instance_id = INSTANCE_COUNTER.fetch_add(1, Ordering::SeqCst) + 1;
		Ok(CountedService { instance_id })
	}
}

/// Register CountedService in the global registry so `Depends::resolve()` can find it.
/// Uses Request scope: same instance within one `InjectionContext`, new instance per context.
fn register_counted_service() {
	let registry = global_registry();
	if !registry.is_registered::<CountedService>() {
		registry.register_async::<CountedService, _, _>(DependencyScope::Request, |_ctx| async {
			let instance_id = INSTANCE_COUNTER.fetch_add(1, Ordering::SeqCst) + 1;
			Ok(CountedService { instance_id })
		});
	}
}

#[tokio::test]
async fn test_injected_with_cache_default() {
	let singleton = SingletonScope::new();
	let ctx = InjectionContext::builder(singleton).build();

	// Cache is enabled by default
	let params1 = Depends::<CommonQueryParams>::resolve(&ctx, true)
		.await
		.unwrap();
	let params2 = Depends::<CommonQueryParams>::resolve(&ctx, true)
		.await
		.unwrap();

	// Returns the same instance
	assert_eq!(*params1, *params2);
}

#[tokio::test]
#[serial(counted_service)]
async fn test_separate_contexts_create_new_instances() {
	// Reset counter for this test
	INSTANCE_COUNTER.store(0, Ordering::SeqCst);
	register_counted_service();

	// With Request scope, separate InjectionContexts produce separate instances
	let singleton = SingletonScope::new();
	let ctx1 = InjectionContext::builder(singleton).build();
	let singleton2 = SingletonScope::new();
	let ctx2 = InjectionContext::builder(singleton2).build();

	let service1 = Depends::<CountedService>::resolve(&ctx1, true)
		.await
		.unwrap();
	let service2 = Depends::<CountedService>::resolve(&ctx2, true)
		.await
		.unwrap();

	// Different contexts produce different instances (IDs are sequential)
	assert_ne!(service1.instance_id, service2.instance_id);
	assert_eq!(service1.instance_id + 1, service2.instance_id);
}

#[tokio::test]
#[serial(counted_service)]
async fn test_injected_with_cache_enabled() {
	// Reset counter for this test
	INSTANCE_COUNTER.store(0, Ordering::SeqCst);
	register_counted_service();

	let singleton = SingletonScope::new();
	let ctx = InjectionContext::builder(singleton).build();

	// Cache enabled (default)
	let service1 = Depends::<CountedService>::resolve(&ctx, true)
		.await
		.unwrap();
	let service2 = Depends::<CountedService>::resolve(&ctx, true)
		.await
		.unwrap();

	// Returns the same instance (same ID)
	assert_eq!(service1.instance_id, service2.instance_id);
}

#[tokio::test]
async fn test_injected_from_value() {
	let db = Database {
		connection_count: 10,
	};
	let depends = Depends::from_value(db);

	assert_eq!(depends.connection_count, 10);
}

#[tokio::test]
async fn test_injected_deref() {
	let singleton = SingletonScope::new();
	let ctx = InjectionContext::builder(singleton).build();

	let params = Depends::<CommonQueryParams>::resolve(&ctx, true)
		.await
		.unwrap();

	// Can access fields directly via Deref
	assert_eq!(params.skip, 0);
	assert_eq!(params.limit, 0);
}

#[tokio::test]
async fn test_fastapi_injected_clone() {
	let singleton = SingletonScope::new();
	let ctx = InjectionContext::builder(singleton).build();

	let params1 = Depends::<CommonQueryParams>::resolve(&ctx, true)
		.await
		.unwrap();
	let params2 = params1.clone();

	// Clone copies the reference (Arc::clone)
	assert_eq!(*params1, *params2);
}

// FastAPI-style usage example
#[tokio::test]
async fn test_fastapi_style_usage() {
	async fn endpoint_handler(
		config: Depends<Config>,
		params: Depends<CommonQueryParams>,
	) -> String {
		format!("API Key: {}, Skip: {}", config.api_key, params.skip)
	}

	let singleton = SingletonScope::new();
	let ctx = InjectionContext::builder(singleton).build();

	// Simulate endpoint usage
	let config = Depends::<Config>::resolve(&ctx, true).await.unwrap();
	let params = Depends::<CommonQueryParams>::resolve(&ctx, true)
		.await
		.unwrap();

	let result = endpoint_handler(config, params).await;
	assert_eq!(result, "API Key: , Skip: 0");
}
