use std::sync::Arc;

use rstest::*;

use crate::context::InjectionContext;
use crate::resolve_context::{
	ContextLevel, RESOLVE_CTX, ResolveContext, get_di_context, try_get_di_context,
};
use crate::scope::SingletonScope;

/// Helper to build a fresh `InjectionContext` wrapped in `Arc`.
fn build_context() -> Arc<InjectionContext> {
	let scope = SingletonScope::new();
	Arc::new(InjectionContext::builder(scope).build())
}

// ---------------------------------------------------------------------------
// Normal tests (1-7)
// ---------------------------------------------------------------------------

#[rstest]
#[tokio::test]
async fn root_returns_root_context() {
	// Arrange
	let root = build_context();
	let current = build_context();
	let root_clone = Arc::clone(&root);
	let ctx = ResolveContext { root, current };

	// Act
	let result = RESOLVE_CTX
		.scope(ctx, async { get_di_context(ContextLevel::Root) })
		.await;

	// Assert
	assert!(Arc::ptr_eq(&result, &root_clone));
}

#[rstest]
#[tokio::test]
async fn current_returns_current_context() {
	// Arrange
	let root = build_context();
	let current = build_context();
	let current_clone = Arc::clone(&current);
	let ctx = ResolveContext { root, current };

	// Act
	let result = RESOLVE_CTX
		.scope(ctx, async { get_di_context(ContextLevel::Current) })
		.await;

	// Assert
	assert!(Arc::ptr_eq(&result, &current_clone));
}

#[rstest]
#[tokio::test]
async fn root_and_current_are_different_instances() {
	// Arrange
	let root = build_context();
	let current = build_context();
	let ctx = ResolveContext {
		root: Arc::clone(&root),
		current: Arc::clone(&current),
	};

	// Act
	let (got_root, got_current) = RESOLVE_CTX
		.scope(ctx, async {
			(
				get_di_context(ContextLevel::Root),
				get_di_context(ContextLevel::Current),
			)
		})
		.await;

	// Assert
	assert!(!Arc::ptr_eq(&got_root, &got_current));
}

#[rstest]
#[tokio::test]
async fn root_and_current_same_when_no_fork() {
	// Arrange
	let shared = build_context();
	let ctx = ResolveContext {
		root: Arc::clone(&shared),
		current: Arc::clone(&shared),
	};

	// Act
	let (got_root, got_current) = RESOLVE_CTX
		.scope(ctx, async {
			(
				get_di_context(ContextLevel::Root),
				get_di_context(ContextLevel::Current),
			)
		})
		.await;

	// Assert
	assert!(Arc::ptr_eq(&got_root, &got_current));
}

#[rstest]
#[tokio::test]
async fn try_get_di_context_root_returns_some() {
	// Arrange
	let root = build_context();
	let root_clone = Arc::clone(&root);
	let ctx = ResolveContext {
		root,
		current: build_context(),
	};

	// Act
	let result = RESOLVE_CTX
		.scope(ctx, async { try_get_di_context(ContextLevel::Root) })
		.await;

	// Assert
	let result = result.expect("should be Some inside scope");
	assert!(Arc::ptr_eq(&result, &root_clone));
}

#[rstest]
#[tokio::test]
async fn try_get_di_context_current_returns_some() {
	// Arrange
	let current = build_context();
	let current_clone = Arc::clone(&current);
	let ctx = ResolveContext {
		root: build_context(),
		current,
	};

	// Act
	let result = RESOLVE_CTX
		.scope(ctx, async { try_get_di_context(ContextLevel::Current) })
		.await;

	// Assert
	let result = result.expect("should be Some inside scope");
	assert!(Arc::ptr_eq(&result, &current_clone));
}

#[rstest]
#[tokio::test]
async fn returned_arc_is_same_instance_on_repeated_calls() {
	// Arrange
	let root = build_context();
	let root_clone = Arc::clone(&root);
	let ctx = ResolveContext {
		root,
		current: build_context(),
	};

	// Act
	let (first, second) = RESOLVE_CTX
		.scope(ctx, async {
			let a = get_di_context(ContextLevel::Root);
			let b = get_di_context(ContextLevel::Root);
			(a, b)
		})
		.await;

	// Assert
	assert!(Arc::ptr_eq(&first, &second));
	assert!(Arc::ptr_eq(&first, &root_clone));
}

// ---------------------------------------------------------------------------
// Error tests (8-11)
// ---------------------------------------------------------------------------

#[rstest]
#[tokio::test]
#[should_panic]
async fn get_di_context_outside_factory_panics_root() {
	// Act (no Arrange needed -- absence of RESOLVE_CTX is the condition)
	let _ = get_di_context(ContextLevel::Root);
}

#[rstest]
#[tokio::test]
#[should_panic]
async fn get_di_context_outside_factory_panics_current() {
	// Act
	let _ = get_di_context(ContextLevel::Current);
}

#[rstest]
#[tokio::test]
async fn try_get_di_context_outside_factory_returns_none_root() {
	// Act
	let result = try_get_di_context(ContextLevel::Root);

	// Assert
	assert!(result.is_none());
}

#[rstest]
#[tokio::test]
async fn try_get_di_context_outside_factory_returns_none_current() {
	// Act
	let result = try_get_di_context(ContextLevel::Current);

	// Assert
	assert!(result.is_none());
}

// ---------------------------------------------------------------------------
// Edge-case tests (12-18)
// ---------------------------------------------------------------------------

#[rstest]
#[tokio::test]
async fn nested_factory_preserves_root() {
	// Arrange
	let root = build_context();
	let outer_current = build_context();
	let inner_current = build_context();
	let root_clone = Arc::clone(&root);

	let outer = ResolveContext {
		root: Arc::clone(&root),
		current: outer_current,
	};

	// Act
	let inner_root = RESOLVE_CTX
		.scope(outer, async {
			let inner = ResolveContext {
				root: Arc::clone(&root_clone),
				current: inner_current,
			};
			RESOLVE_CTX
				.scope(inner, async { get_di_context(ContextLevel::Root) })
				.await
		})
		.await;

	// Assert
	assert!(Arc::ptr_eq(&inner_root, &root_clone));
}

#[rstest]
#[tokio::test]
async fn nested_factory_updates_current() {
	// Arrange
	let root = build_context();
	let outer_current = build_context();
	let inner_current = build_context();
	let inner_current_clone = Arc::clone(&inner_current);

	let outer = ResolveContext {
		root: Arc::clone(&root),
		current: outer_current,
	};

	// Act
	let got_current = RESOLVE_CTX
		.scope(outer, async {
			let inner = ResolveContext {
				root: Arc::clone(&root),
				current: inner_current,
			};
			RESOLVE_CTX
				.scope(inner, async { get_di_context(ContextLevel::Current) })
				.await
		})
		.await;

	// Assert
	assert!(Arc::ptr_eq(&got_current, &inner_current_clone));
}

#[rstest]
#[tokio::test]
async fn deeply_nested_three_levels_preserves_root() {
	// Arrange
	let root = build_context();
	let root_clone = Arc::clone(&root);

	let level1 = ResolveContext {
		root: Arc::clone(&root),
		current: build_context(),
	};

	// Act
	let deepest_root = RESOLVE_CTX
		.scope(level1, async {
			let level2 = ResolveContext {
				root: Arc::clone(&root_clone),
				current: build_context(),
			};
			RESOLVE_CTX
				.scope(level2, async {
					let level3 = ResolveContext {
						root: Arc::clone(&root_clone),
						current: build_context(),
					};
					RESOLVE_CTX
						.scope(level3, async { get_di_context(ContextLevel::Root) })
						.await
				})
				.await
		})
		.await;

	// Assert
	assert!(Arc::ptr_eq(&deepest_root, &root_clone));
}

#[rstest]
#[tokio::test]
async fn scope_restores_after_nested_call() {
	// Arrange
	let root = build_context();
	let outer_current = build_context();
	let outer_current_clone = Arc::clone(&outer_current);
	let inner_current = build_context();

	let outer = ResolveContext {
		root: Arc::clone(&root),
		current: outer_current,
	};

	// Act
	let after_inner = RESOLVE_CTX
		.scope(outer, async {
			let inner = ResolveContext {
				root: Arc::clone(&root),
				current: inner_current,
			};
			// Enter and leave inner scope
			RESOLVE_CTX.scope(inner, async {}).await;

			// Should see outer current again
			get_di_context(ContextLevel::Current)
		})
		.await;

	// Assert
	assert!(Arc::ptr_eq(&after_inner, &outer_current_clone));
}

#[rstest]
#[tokio::test]
async fn concurrent_tasks_isolated() {
	// Arrange
	let root_a = build_context();
	let root_b = build_context();
	let root_a_clone = Arc::clone(&root_a);
	let root_b_clone = Arc::clone(&root_b);

	// Act
	let handle_a = tokio::spawn(async move {
		let ctx = ResolveContext {
			root: root_a,
			current: build_context(),
		};
		RESOLVE_CTX
			.scope(ctx, async { get_di_context(ContextLevel::Root) })
			.await
	});
	let handle_b = tokio::spawn(async move {
		let ctx = ResolveContext {
			root: root_b,
			current: build_context(),
		};
		RESOLVE_CTX
			.scope(ctx, async { get_di_context(ContextLevel::Root) })
			.await
	});

	let (result_a, result_b) = tokio::join!(handle_a, handle_b);
	let result_a = result_a.expect("task A should succeed");
	let result_b = result_b.expect("task B should succeed");

	// Assert
	assert!(Arc::ptr_eq(&result_a, &root_a_clone));
	assert!(Arc::ptr_eq(&result_b, &root_b_clone));
	assert!(!Arc::ptr_eq(&result_a, &result_b));
}

#[rstest]
#[tokio::test]
async fn concurrent_tasks_different_roots() {
	// Arrange
	let root1 = build_context();
	let root2 = build_context();
	let root1_clone = Arc::clone(&root1);
	let root2_clone = Arc::clone(&root2);

	// Act
	let h1 = tokio::spawn(async move {
		let ctx = ResolveContext {
			root: root1,
			current: build_context(),
		};
		RESOLVE_CTX
			.scope(ctx, async { get_di_context(ContextLevel::Root) })
			.await
	});
	let h2 = tokio::spawn(async move {
		let ctx = ResolveContext {
			root: root2,
			current: build_context(),
		};
		RESOLVE_CTX
			.scope(ctx, async { get_di_context(ContextLevel::Root) })
			.await
	});

	let (r1, r2) = tokio::join!(h1, h2);
	let r1 = r1.expect("task 1 should succeed");
	let r2 = r2.expect("task 2 should succeed");

	// Assert
	assert!(Arc::ptr_eq(&r1, &root1_clone));
	assert!(Arc::ptr_eq(&r2, &root2_clone));
}

#[rstest]
#[tokio::test]
async fn task_local_not_inherited_by_spawned_task() {
	// Arrange
	let ctx = ResolveContext {
		root: build_context(),
		current: build_context(),
	};

	// Act
	let child_result = RESOLVE_CTX
		.scope(ctx, async {
			let handle = tokio::spawn(async { try_get_di_context(ContextLevel::Root) });
			handle.await.expect("child task should succeed")
		})
		.await;

	// Assert
	assert!(child_result.is_none());
}
