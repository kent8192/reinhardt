//! Macro expansion tests for `get_di_context` within factory-like scopes.
//!
//! These tests verify that `RESOLVE_CTX.scope()` (which is what the
//! `#[injectable_factory]` macro generates) correctly sets the task-local
//! resolve context, enabling `get_di_context()` within factory bodies.

use std::sync::Arc;

use rstest::*;

use reinhardt_di::resolve_context::{RESOLVE_CTX, ResolveContext};
use reinhardt_di::{ContextLevel, InjectionContext, SingletonScope, get_di_context};

/// Helper to build a fresh `InjectionContext` wrapped in `Arc`.
fn build_context() -> Arc<InjectionContext> {
	let scope = SingletonScope::new();
	Arc::new(InjectionContext::builder(scope).build())
}

// ---------------------------------------------------------------------------
// Task 5: Macro expansion tests
// ---------------------------------------------------------------------------

#[rstest]
#[tokio::test]
async fn factory_no_inject_params_sets_task_local() {
	// Arrange
	let root = build_context();
	let current = Arc::clone(&root);
	let expected = Arc::clone(&current);
	let ctx = ResolveContext { root, current };

	// Act -- simulate a factory with no #[inject] params calling get_di_context
	let result = RESOLVE_CTX
		.scope(ctx, async { get_di_context(ContextLevel::Current) })
		.await;

	// Assert
	assert!(Arc::ptr_eq(&result, &expected));
}

#[rstest]
#[tokio::test]
async fn factory_returning_injection_context() {
	// Arrange
	let root = build_context();
	let current = Arc::clone(&root);
	let expected = Arc::clone(&current);
	let ctx = ResolveContext { root, current };

	// Act -- simulate a factory that returns the InjectionContext itself
	let returned: Arc<InjectionContext> = RESOLVE_CTX
		.scope(ctx, async {
			// Factory body: retrieve the context and return it as the product
			get_di_context(ContextLevel::Current)
		})
		.await;

	// Assert
	assert!(Arc::ptr_eq(&returned, &expected));
}

#[rstest]
#[tokio::test]
async fn factory_scope_singleton_sets_task_local() {
	// Arrange -- singleton scope: root == current (no fork)
	let shared = build_context();
	let root_clone = Arc::clone(&shared);
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

	// Assert -- both levels point to the same shared context
	assert!(Arc::ptr_eq(&got_root, &root_clone));
	assert!(Arc::ptr_eq(&got_current, &root_clone));
	assert!(Arc::ptr_eq(&got_root, &got_current));
}

#[rstest]
#[tokio::test]
async fn factory_scope_request_sets_task_local() {
	// Arrange -- request scope: root != current (forked context)
	let root = build_context();
	let current = build_context();
	let root_clone = Arc::clone(&root);
	let current_clone = Arc::clone(&current);
	let ctx = ResolveContext { root, current };

	// Act
	let (got_root, got_current) = RESOLVE_CTX
		.scope(ctx, async {
			(
				get_di_context(ContextLevel::Root),
				get_di_context(ContextLevel::Current),
			)
		})
		.await;

	// Assert -- root and current are distinct, matching their respective originals
	assert!(Arc::ptr_eq(&got_root, &root_clone));
	assert!(Arc::ptr_eq(&got_current, &current_clone));
	assert!(!Arc::ptr_eq(&got_root, &got_current));
}

#[rstest]
#[tokio::test]
async fn factory_scope_transient_sets_task_local() {
	// Arrange -- transient scope: each resolution gets its own context pair
	let root = build_context();
	let current_1 = build_context();
	let current_2 = build_context();
	let current_1_clone = Arc::clone(&current_1);
	let current_2_clone = Arc::clone(&current_2);

	// Act -- two sequential "transient" factory invocations
	let result_1 = RESOLVE_CTX
		.scope(
			ResolveContext {
				root: Arc::clone(&root),
				current: current_1,
			},
			async { get_di_context(ContextLevel::Current) },
		)
		.await;

	let result_2 = RESOLVE_CTX
		.scope(
			ResolveContext {
				root: Arc::clone(&root),
				current: current_2,
			},
			async { get_di_context(ContextLevel::Current) },
		)
		.await;

	// Assert -- each invocation sees its own current context
	assert!(Arc::ptr_eq(&result_1, &current_1_clone));
	assert!(Arc::ptr_eq(&result_2, &current_2_clone));
	assert!(!Arc::ptr_eq(&result_1, &result_2));
}

#[rstest]
#[tokio::test]
async fn factory_inject_and_get_di_context_coexist() {
	// Arrange -- simulate a factory that has both #[inject] params AND
	// calls get_di_context() internally
	let root = build_context();
	let current = build_context();
	let root_clone = Arc::clone(&root);
	let current_clone = Arc::clone(&current);
	let ctx = ResolveContext { root, current };

	// Act
	let (injected_value, ctx_from_get) = RESOLVE_CTX
		.scope(ctx, async {
			// Simulate the #[inject] param being resolved from context
			let injected = 42u64;

			// The factory body also calls get_di_context
			let ctx = get_di_context(ContextLevel::Current);
			(injected, ctx)
		})
		.await;

	// Assert -- both the "injected" value and the context retrieval work
	assert_eq!(injected_value, 42);
	assert!(Arc::ptr_eq(&ctx_from_get, &current_clone));

	// Also verify root is accessible alongside the current
	let root_result = RESOLVE_CTX
		.scope(
			ResolveContext {
				root: Arc::clone(&root_clone),
				current: Arc::clone(&current_clone),
			},
			async { get_di_context(ContextLevel::Root) },
		)
		.await;
	assert!(Arc::ptr_eq(&root_result, &root_clone));
}
