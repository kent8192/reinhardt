//! Error handling integration tests for reinhardt-di
//!
//! Tests covering:
//! - Circular dependency detection (using automatic runtime detection)
//! - Injectable failure propagation
//! - Async operation timeout handling
//! - Depends lifetime management with Arc

use super::test_helpers::resolve_injectable;
use reinhardt_di::{Depends, DiError, DiResult, Injectable, InjectionContext, SingletonScope};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;

// === Circular Dependency Test Structures ===
// Note: Manual stack management is no longer needed due to automatic cycle detection

#[derive(Clone, Debug)]
struct ServiceA {
	_name: String,
	_service_b: Arc<ServiceB>,
}

#[derive(Clone, Debug)]
struct ServiceB {
	_name: String,
	_service_a: Arc<ServiceA>,
}

#[async_trait::async_trait]
impl Injectable for ServiceA {
	async fn inject(ctx: &InjectionContext) -> DiResult<Self> {
		// ServiceA depends on ServiceB
		// Cycle detection is performed automatically in resolve_injectable()
		let service_b = resolve_injectable::<ServiceB>(ctx).await?;

		Ok(ServiceA {
			_name: "ServiceA".to_string(),
			_service_b: service_b,
		})
	}
}

#[async_trait::async_trait]
impl Injectable for ServiceB {
	async fn inject(ctx: &InjectionContext) -> DiResult<Self> {
		// ServiceB depends on ServiceA - creates circular dependency
		// Cycle detection is performed automatically in resolve_injectable()
		let service_a = resolve_injectable::<ServiceA>(ctx).await?;

		Ok(ServiceB {
			_name: "ServiceB".to_string(),
			_service_a: service_a,
		})
	}
}

// === Injectable Failure Test Structures ===

#[derive(Clone, Debug)]
struct FailingService {
	_should_fail: bool,
}

#[async_trait::async_trait]
impl Injectable for FailingService {
	async fn inject(_ctx: &InjectionContext) -> DiResult<Self> {
		Err(DiError::ProviderError(
			"Intentional failure for testing".to_string(),
		))
	}
}

#[derive(Clone, Debug)]
struct DependentService {
	_failing_service: Arc<FailingService>,
}

#[async_trait::async_trait]
impl Injectable for DependentService {
	async fn inject(ctx: &InjectionContext) -> DiResult<Self> {
		// This will fail because FailingService::inject() fails
		let failing = FailingService::inject(ctx).await?;
		Ok(DependentService {
			_failing_service: Arc::new(failing),
		})
	}
}

// === Async Timeout Test Structures ===

#[derive(Clone, Debug)]
struct SlowService {
	delay_ms: u64,
}

#[async_trait::async_trait]
impl Injectable for SlowService {
	async fn inject(_ctx: &InjectionContext) -> DiResult<Self> {
		// Simulate slow initialization
		tokio::time::sleep(Duration::from_millis(1000)).await;
		Ok(SlowService { delay_ms: 1000 })
	}
}

// === Lifetime Management Test Structures ===

#[derive(Clone, Debug)]
struct ResourceOwner {
	id: u32,
	data: String,
}

#[async_trait::async_trait]
impl Injectable for ResourceOwner {
	async fn inject(_ctx: &InjectionContext) -> DiResult<Self> {
		Ok(ResourceOwner {
			id: 1,
			data: "owned-data".to_string(),
		})
	}
}

// === Test Cases ===

#[tokio::test]
async fn test_circular_dependency_detection() {
	let singleton = Arc::new(SingletonScope::new());
	let ctx = InjectionContext::builder(singleton).build();

	// Attempt to resolve ServiceA, which depends on ServiceB, which depends on ServiceA
	// Automatic cycle detection will raise DiError::CircularDependency
	let result = resolve_injectable::<ServiceA>(&ctx).await;

	// Verify circular dependency is detected
	assert!(result.is_err(), "Circular dependency should be detected");
	match result.unwrap_err() {
		DiError::CircularDependency(msg) => {
			// Verify error message contains circular type names
			assert!(
				msg.contains("ServiceA") || msg.contains("ServiceB"),
				"Error message should contain circular types: {}",
				msg
			);
		}
		other => panic!("Expected CircularDependency error, got: {:?}", other),
	}
}

#[tokio::test]
async fn test_injectable_failure_propagation() {
	let singleton = Arc::new(SingletonScope::new());
	let ctx = InjectionContext::builder(singleton).build();

	// Direct injection of failing service
	let failing_result = FailingService::inject(&ctx).await;
	assert!(failing_result.is_err());
	match failing_result.unwrap_err() {
		DiError::ProviderError(msg) => {
			assert_eq!(msg, "Intentional failure for testing");
		}
		other => panic!("Expected ProviderError, got: {:?}", other),
	}

	// Injection of dependent service should propagate the error
	let dependent_result = DependentService::inject(&ctx).await;
	assert!(dependent_result.is_err());
	match dependent_result.unwrap_err() {
		DiError::ProviderError(msg) => {
			assert_eq!(msg, "Intentional failure for testing");
		}
		other => panic!("Expected ProviderError (propagated), got: {:?}", other),
	}
}

#[tokio::test]
async fn test_injectable_async_timeout() {
	let singleton = Arc::new(SingletonScope::new());
	let ctx = InjectionContext::builder(singleton).build();

	// Attempt injection with timeout shorter than service delay
	let timeout_duration = Duration::from_millis(100);
	let result = timeout(timeout_duration, SlowService::inject(&ctx)).await;

	// Verify timeout occurred
	assert!(result.is_err());
	assert!(
		result
			.unwrap_err()
			.to_string()
			.contains("deadline has elapsed")
	);

	// Verify with longer timeout that injection succeeds
	let longer_timeout = Duration::from_millis(2000);
	let result_ok = timeout(longer_timeout, SlowService::inject(&ctx)).await;
	assert!(result_ok.is_ok());
	let service = result_ok.unwrap().unwrap();
	assert_eq!(service.delay_ms, 1000);
}

#[tokio::test]
async fn test_depends_lifetime_management() {
	let singleton = Arc::new(SingletonScope::new());
	let ctx = InjectionContext::builder(singleton).build();

	// Resolve dependency using Depends
	let depends_resource = Depends::<ResourceOwner>::builder()
		.resolve(&ctx)
		.await
		.unwrap();

	// Verify Depends uses Arc internally by cloning
	let depends_clone = depends_resource.clone();

	// Both should access the same data via Deref
	assert_eq!(depends_resource.id, 1);
	assert_eq!(depends_clone.id, 1);
	assert_eq!(depends_resource.data, "owned-data");
	assert_eq!(depends_clone.data, "owned-data");

	// Drop clone - original should still be accessible
	drop(depends_clone);
	assert_eq!(depends_resource.data, "owned-data");

	// Resolve again from context (should return cached instance)
	let depends_resource2 = Depends::<ResourceOwner>::builder()
		.resolve(&ctx)
		.await
		.unwrap();

	// Verify data matches (same instance from cache)
	assert_eq!(depends_resource.id, depends_resource2.id);
	assert_eq!(depends_resource.data, depends_resource2.data);

	// Drop all Depends instances
	drop(depends_resource);
	drop(depends_resource2);

	// Verify context cache still holds the value
	let cached: Option<Arc<ResourceOwner>> = ctx.get_request();
	assert!(cached.is_some());
	let cached_value = cached.unwrap();
	assert_eq!(cached_value.id, 1);
	assert_eq!(cached_value.data, "owned-data");

	// Create a weak reference from cached Arc
	let weak_ref = Arc::downgrade(&cached_value);
	drop(cached_value);

	// Weak reference should still be upgradeable because cache holds strong ref
	let upgraded = weak_ref.upgrade();
	assert!(upgraded.is_some());
	assert_eq!(upgraded.unwrap().data, "owned-data");
}
