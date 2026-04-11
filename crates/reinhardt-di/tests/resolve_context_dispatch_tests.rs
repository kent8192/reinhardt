//! Integration tests for request dispatch resolve context.
//!
//! These tests verify that `fork_for_request()` followed by
//! `RESOLVE_CTX.scope()` correctly sets root and current contexts,
//! with proper isolation across sequential and concurrent requests.

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
// Task 6: Integration tests for request dispatch
// ---------------------------------------------------------------------------

#[rstest]
#[tokio::test]
async fn root_context_shares_singleton_scope() {
	// Arrange -- simulate app startup: root context with singleton data
	let singleton_scope = Arc::new(SingletonScope::new());
	singleton_scope.set(99u64);
	let root = Arc::new(InjectionContext::builder(Arc::clone(&singleton_scope)).build());

	// Simulate request dispatch: fork creates a new request context that
	// shares the singleton scope with the root
	let request_ctx = Arc::new(root.fork());
	let root_clone = Arc::clone(&root);

	let ctx = ResolveContext {
		root: Arc::clone(&root),
		current: Arc::clone(&request_ctx),
	};

	// Act -- inside the "factory", retrieve both root and current
	let (got_root, got_current) = RESOLVE_CTX
		.scope(ctx, async {
			(
				get_di_context(ContextLevel::Root),
				get_di_context(ContextLevel::Current),
			)
		})
		.await;

	// Assert -- root's singleton scope data is accessible from both contexts
	assert!(Arc::ptr_eq(&got_root, &root_clone));
	assert!(Arc::ptr_eq(&got_current, &request_ctx));
	assert!(!Arc::ptr_eq(&got_root, &got_current));

	// Both share the same singleton scope value
	let root_val = got_root.get_singleton::<u64>();
	let current_val = got_current.get_singleton::<u64>();
	assert_eq!(root_val.map(|v| *v), Some(99u64));
	assert_eq!(current_val.map(|v| *v), Some(99u64));
}

#[rstest]
#[tokio::test]
async fn nested_resolve_preserves_root_in_factory() {
	// Arrange -- root context at application level
	let root = build_context();
	let outer_current = build_context();
	let root_clone = Arc::clone(&root);

	let outer = ResolveContext {
		root: Arc::clone(&root),
		current: outer_current,
	};

	// Act -- outer factory triggers an inner resolution (nested scope)
	// simulating: factory A resolves type B, which triggers factory B
	let (outer_root, inner_root, inner_current) = RESOLVE_CTX
		.scope(outer, async {
			let outer_root = get_di_context(ContextLevel::Root);

			// Inner factory gets its own current context but preserves root
			let inner_current_ctx = build_context();
			let _inner_current_clone = Arc::clone(&inner_current_ctx);
			let inner = ResolveContext {
				root: Arc::clone(&root_clone),
				current: inner_current_ctx,
			};

			let (inner_r, inner_c) = RESOLVE_CTX
				.scope(inner, async {
					(
						get_di_context(ContextLevel::Root),
						get_di_context(ContextLevel::Current),
					)
				})
				.await;

			(outer_root, inner_r, inner_c)
		})
		.await;

	// Assert -- both outer and inner factories see the same root context
	assert!(Arc::ptr_eq(&outer_root, &root_clone));
	assert!(Arc::ptr_eq(&inner_root, &root_clone));
	assert!(Arc::ptr_eq(&outer_root, &inner_root));

	// Inner factory has its own current context, distinct from root
	assert!(!Arc::ptr_eq(&inner_current, &root_clone));
}

#[rstest]
#[tokio::test]
async fn sequential_requests_have_isolated_contexts() {
	// Arrange -- simulate two sequential HTTP requests
	let root = build_context();
	let request_1 = build_context();
	let request_2 = build_context();
	let request_1_clone = Arc::clone(&request_1);
	let request_2_clone = Arc::clone(&request_2);

	// Act -- request 1
	let result_1 = RESOLVE_CTX
		.scope(
			ResolveContext {
				root: Arc::clone(&root),
				current: request_1,
			},
			async { get_di_context(ContextLevel::Current) },
		)
		.await;

	// Act -- request 2
	let result_2 = RESOLVE_CTX
		.scope(
			ResolveContext {
				root: Arc::clone(&root),
				current: request_2,
			},
			async { get_di_context(ContextLevel::Current) },
		)
		.await;

	// Assert -- each request sees its own current context
	assert!(Arc::ptr_eq(&result_1, &request_1_clone));
	assert!(Arc::ptr_eq(&result_2, &request_2_clone));
	assert!(!Arc::ptr_eq(&result_1, &result_2));
}

#[rstest]
#[tokio::test]
async fn concurrent_requests_have_isolated_contexts() {
	// Arrange -- simulate two concurrent HTTP requests
	let root = build_context();
	let request_a = build_context();
	let request_b = build_context();
	let request_a_clone = Arc::clone(&request_a);
	let request_b_clone = Arc::clone(&request_b);

	let root_a = Arc::clone(&root);
	let root_b = Arc::clone(&root);

	// Act -- two concurrent request dispatch scopes
	let handle_a = tokio::spawn(async move {
		let ctx = ResolveContext {
			root: root_a,
			current: request_a,
		};
		RESOLVE_CTX
			.scope(ctx, async { get_di_context(ContextLevel::Current) })
			.await
	});
	let handle_b = tokio::spawn(async move {
		let ctx = ResolveContext {
			root: root_b,
			current: request_b,
		};
		RESOLVE_CTX
			.scope(ctx, async { get_di_context(ContextLevel::Current) })
			.await
	});

	let (result_a, result_b) = tokio::join!(handle_a, handle_b);
	let result_a = result_a.expect("task A should succeed");
	let result_b = result_b.expect("task B should succeed");

	// Assert -- each concurrent request sees its own context
	assert!(Arc::ptr_eq(&result_a, &request_a_clone));
	assert!(Arc::ptr_eq(&result_b, &request_b_clone));
	assert!(!Arc::ptr_eq(&result_a, &result_b));
}
