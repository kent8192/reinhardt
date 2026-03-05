//! Circular dependency detection integration tests
//!
//! This test suite verifies the automatic circular dependency detection functionality
//! of the DI system.

use reinhardt_di::{DiError, Injected, InjectionContext, SingletonScope, injectable};
use std::sync::Arc;

/// Test fixture: ServiceA (depends on ServiceB)
#[derive(Clone)]
#[injectable]
#[allow(dead_code)]
struct ServiceA {
	#[inject]
	b: Injected<ServiceB>,
}

/// Test fixture: ServiceB (depends on ServiceC)
#[derive(Clone)]
#[injectable]
#[allow(dead_code)]
struct ServiceB {
	#[inject]
	c: Injected<ServiceC>,
}

/// Test fixture: ServiceC (depends on ServiceA - circular!)
#[derive(Clone)]
#[injectable]
#[allow(dead_code)]
struct ServiceC {
	#[inject]
	a: Injected<ServiceA>,
}

/// Direct circular dependency: A -> B -> A
#[tokio::test]
async fn test_direct_circular_dependency() {
	#[derive(Clone)]
	#[injectable]
	#[allow(dead_code)]
	struct DirectA {
		#[inject]
		b: Injected<DirectB>,
	}

	#[derive(Clone)]
	#[injectable]
	#[allow(dead_code)]
	struct DirectB {
		#[inject]
		a: Injected<DirectA>,
	}

	let singleton_scope = Arc::new(SingletonScope::new());
	let ctx = InjectionContext::builder(singleton_scope).build();
	let result = Injected::<DirectA>::resolve(&ctx).await;

	assert!(
		result.is_err(),
		"Direct circular dependency should be detected"
	);

	if let Err(DiError::CircularDependency(msg)) = result {
		assert!(
			msg.contains("DirectA") || msg.contains("DirectB"),
			"Error message should contain circular types: {}",
			msg
		);
	} else {
		panic!("Expected CircularDependency error");
	}
}

/// Indirect circular dependency: A -> B -> C -> A
#[tokio::test]
async fn test_indirect_circular_dependency() {
	let singleton_scope = Arc::new(SingletonScope::new());
	let ctx = InjectionContext::builder(singleton_scope).build();
	let result = Injected::<ServiceA>::resolve(&ctx).await;

	assert!(
		result.is_err(),
		"Indirect circular dependency should be detected"
	);

	if let Err(DiError::CircularDependency(msg)) = result {
		// Verify cycle path contains involved types
		let contains_services =
			msg.contains("ServiceA") || msg.contains("ServiceB") || msg.contains("ServiceC");
		assert!(
			contains_services,
			"Error message should contain circular types: {}",
			msg
		);
	} else {
		panic!("Expected CircularDependency error");
	}
}

/// Self-reference: A -> A
#[tokio::test]
async fn test_self_dependency() {
	#[derive(Clone)]
	#[injectable]
	#[allow(dead_code)]
	struct SelfDependent {
		#[inject]
		inner: Injected<SelfDependent>,
	}

	let singleton_scope = Arc::new(SingletonScope::new());
	let ctx = InjectionContext::builder(singleton_scope).build();
	let result = Injected::<SelfDependent>::resolve(&ctx).await;

	assert!(result.is_err(), "Self-dependency should be detected");
	assert!(
		matches!(result, Err(DiError::CircularDependency(_))),
		"Expected CircularDependency error"
	);
}

/// Complex circular dependency: A -> B -> C -> D -> B
#[tokio::test]
async fn test_complex_circular_dependency() {
	#[derive(Clone)]
	#[injectable]
	#[allow(dead_code)]
	struct ComplexA {
		#[inject]
		b: Injected<ComplexB>,
	}

	#[derive(Clone)]
	#[injectable]
	#[allow(dead_code)]
	struct ComplexB {
		#[inject]
		c: Injected<ComplexC>,
	}

	#[derive(Clone)]
	#[injectable]
	#[allow(dead_code)]
	struct ComplexC {
		#[inject]
		d: Injected<ComplexD>,
	}

	#[derive(Clone)]
	#[injectable]
	#[allow(dead_code)]
	struct ComplexD {
		#[inject]
		b: Injected<ComplexB>, // Circular: B -> C -> D -> B
	}

	let singleton_scope = Arc::new(SingletonScope::new());
	let ctx = InjectionContext::builder(singleton_scope).build();
	let result = Injected::<ComplexA>::resolve(&ctx).await;

	assert!(
		result.is_err(),
		"Complex circular dependency should be detected"
	);
	assert!(
		matches!(result, Err(DiError::CircularDependency(_))),
		"Expected CircularDependency error"
	);
}

/// No circular dependency should succeed
#[tokio::test]
async fn test_no_circular_dependency_succeeds() {
	#[derive(Clone)]
	#[injectable]
	#[allow(dead_code)]
	struct NoCycleA {
		#[inject]
		b: Injected<NoCycleB>,
	}

	#[derive(Clone, Default)]
	#[injectable]
	#[allow(dead_code)]
	struct NoCycleB {
		#[no_inject]
		value: i32,
	}

	let singleton_scope = Arc::new(SingletonScope::new());
	let ctx = InjectionContext::builder(singleton_scope).build();
	let result = Injected::<NoCycleA>::resolve(&ctx).await;

	assert!(result.is_ok(), "Non-circular dependency should succeed");
}

/// Deep dependency chain (without cycle) should not error
#[tokio::test]
async fn test_deep_dependency_chain_without_cycle() {
	#[derive(Clone, Default)]
	#[injectable]
	struct Level1;

	#[derive(Clone)]
	#[injectable]
	struct Level2 {
		#[inject]
		_dep: Injected<Level1>,
	}

	#[derive(Clone)]
	#[injectable]
	struct Level3 {
		#[inject]
		_dep: Injected<Level2>,
	}

	#[derive(Clone)]
	#[injectable]
	struct Level4 {
		#[inject]
		_dep: Injected<Level3>,
	}

	#[derive(Clone)]
	#[injectable]
	struct Level5 {
		#[inject]
		_dep: Injected<Level4>,
	}

	let singleton_scope = Arc::new(SingletonScope::new());
	let ctx = InjectionContext::builder(singleton_scope).build();
	let result = Injected::<Level5>::resolve(&ctx).await;

	assert!(
		result.is_ok(),
		"Deep dependency chain (without cycle) should succeed"
	);
}
