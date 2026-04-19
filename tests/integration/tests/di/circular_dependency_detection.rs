//! Circular dependency detection integration tests
//!
//! This test suite verifies the automatic circular dependency detection functionality
//! of the DI system.

use reinhardt_di::{Depends, DiError, InjectionContext, SingletonScope, injectable};
use std::sync::Arc;

/// Test fixture: ServiceA (depends on ServiceB)
#[injectable]
#[allow(dead_code)]
struct ServiceA {
	#[inject]
	b: Depends<ServiceB>,
}

/// Test fixture: ServiceB (depends on ServiceC)
#[injectable]
#[allow(dead_code)]
struct ServiceB {
	#[inject]
	c: Depends<ServiceC>,
}

/// Test fixture: ServiceC (depends on ServiceA - circular!)
#[injectable]
#[allow(dead_code)]
struct ServiceC {
	#[inject]
	a: Depends<ServiceA>,
}

/// Direct circular dependency: A -> B -> A
#[tokio::test]
async fn test_direct_circular_dependency() {
	#[injectable]
	#[allow(dead_code)]
	struct DirectA {
		#[inject]
		b: Depends<DirectB>,
	}

	#[injectable]
	#[allow(dead_code)]
	struct DirectB {
		#[inject]
		a: Depends<DirectA>,
	}

	let singleton_scope = Arc::new(SingletonScope::new());
	let ctx = InjectionContext::builder(singleton_scope).build();
	let result = Depends::<DirectA>::resolve(&ctx, true).await;

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
	let result = Depends::<ServiceA>::resolve(&ctx, true).await;

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
	#[injectable]
	#[allow(dead_code)]
	struct SelfDependent {
		#[inject]
		inner: Depends<SelfDependent>,
	}

	let singleton_scope = Arc::new(SingletonScope::new());
	let ctx = InjectionContext::builder(singleton_scope).build();
	let result = Depends::<SelfDependent>::resolve(&ctx, true).await;

	assert!(result.is_err(), "Self-dependency should be detected");
	assert!(
		matches!(result, Err(DiError::CircularDependency(_))),
		"Expected CircularDependency error"
	);
}

/// Complex circular dependency: A -> B -> C -> D -> B
#[tokio::test]
async fn test_complex_circular_dependency() {
	#[injectable]
	#[allow(dead_code)]
	struct ComplexA {
		#[inject]
		b: Depends<ComplexB>,
	}

	#[injectable]
	#[allow(dead_code)]
	struct ComplexB {
		#[inject]
		c: Depends<ComplexC>,
	}

	#[injectable]
	#[allow(dead_code)]
	struct ComplexC {
		#[inject]
		d: Depends<ComplexD>,
	}

	#[injectable]
	#[allow(dead_code)]
	struct ComplexD {
		#[inject]
		b: Depends<ComplexB>, // Circular: B -> C -> D -> B
	}

	let singleton_scope = Arc::new(SingletonScope::new());
	let ctx = InjectionContext::builder(singleton_scope).build();
	let result = Depends::<ComplexA>::resolve(&ctx, true).await;

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
	#[injectable]
	#[allow(dead_code)]
	struct NoCycleA {
		#[inject]
		b: Depends<NoCycleB>,
	}

	#[injectable]
	#[derive(Default)]
	#[allow(dead_code)]
	struct NoCycleB {
		#[no_inject]
		value: i32,
	}

	let singleton_scope = Arc::new(SingletonScope::new());
	let ctx = InjectionContext::builder(singleton_scope).build();
	let result = Depends::<NoCycleA>::resolve(&ctx, true).await;

	assert!(result.is_ok(), "Non-circular dependency should succeed");
}

/// Deep dependency chain (without cycle) should not error
#[tokio::test]
async fn test_deep_dependency_chain_without_cycle() {
	#[injectable]
	#[derive(Default)]
	struct Level1;

	#[injectable]
	struct Level2 {
		#[inject]
		_dep: Depends<Level1>,
	}

	#[injectable]
	struct Level3 {
		#[inject]
		_dep: Depends<Level2>,
	}

	#[injectable]
	struct Level4 {
		#[inject]
		_dep: Depends<Level3>,
	}

	#[injectable]
	struct Level5 {
		#[inject]
		_dep: Depends<Level4>,
	}

	let singleton_scope = Arc::new(SingletonScope::new());
	let ctx = InjectionContext::builder(singleton_scope).build();
	let result = Depends::<Level5>::resolve(&ctx, true).await;

	assert!(
		result.is_ok(),
		"Deep dependency chain (without cycle) should succeed"
	);
}
