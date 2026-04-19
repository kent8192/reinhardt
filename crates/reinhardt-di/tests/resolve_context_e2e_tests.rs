//! E2E tests for InjectionContext self-registration (Issue #3402 use case).
//!
//! These tests verify the complete factory chain scenario: a "router factory"
//! uses `get_di_context()` to obtain the context and passes it along, while
//! a chained factory also has correct `get_di_context()` behavior.

use std::sync::Arc;

use rstest::*;

use reinhardt_di::resolve_context::{RESOLVE_CTX, ResolveContext};
use reinhardt_di::{
	ContextLevel, InjectionContext, SingletonScope, get_di_context, try_get_di_context,
};

/// Helper to build a context with a shared singleton scope.
fn build_context_with_scope(scope: Arc<SingletonScope>) -> Arc<InjectionContext> {
	Arc::new(InjectionContext::builder(scope).build())
}

// ---------------------------------------------------------------------------
// Task 7: E2E tests for Issue #3402 use case
// ---------------------------------------------------------------------------

#[rstest]
#[tokio::test]
async fn factory_chain_context_propagation() {
	// Arrange -- simulate application startup
	let singleton_scope = Arc::new(SingletonScope::new());
	singleton_scope.set("app-config".to_string());
	let root = build_context_with_scope(Arc::clone(&singleton_scope));
	let request_ctx = build_context_with_scope(Arc::clone(&singleton_scope));
	let root_clone = Arc::clone(&root);
	let request_ctx_clone = Arc::clone(&request_ctx);

	// Act -- simulate the Issue #3402 use case:
	//
	// Factory A (e.g., RouterFactory) calls get_di_context() to obtain the
	// context, then "resolves" type B (e.g., UnifiedRouter) through a nested
	// scope. Factory B also calls get_di_context() and should see the correct
	// root context propagated through the chain.

	let (factory_a_ctx, factory_b_root, factory_b_current, factory_b_config) = RESOLVE_CTX
		.scope(
			ResolveContext {
				root: Arc::clone(&root),
				current: Arc::clone(&request_ctx),
			},
			async {
				// Factory A: get context (simulates router factory)
				let factory_a_ctx = get_di_context(ContextLevel::Current);

				// Factory A resolves type B, which triggers factory B
				// Factory B gets a nested scope with the same root
				let factory_b_current = build_context_with_scope(Arc::clone(&singleton_scope));
				let _factory_b_current_clone = Arc::clone(&factory_b_current);

				let inner = ResolveContext {
					root: Arc::clone(&root_clone),
					current: factory_b_current,
				};

				let (b_root, b_current, b_config) = RESOLVE_CTX
					.scope(inner, async {
						// Factory B: also calls get_di_context
						let b_root = get_di_context(ContextLevel::Root);
						let b_current = get_di_context(ContextLevel::Current);

						// Factory B retrieves shared singleton data through the context
						let config = b_root.get_singleton::<String>().map(|s| s.as_ref().clone());

						(b_root, b_current, config)
					})
					.await;

				(factory_a_ctx, b_root, b_current, b_config)
			},
		)
		.await;

	// Assert -- Factory A sees the request context
	assert!(Arc::ptr_eq(&factory_a_ctx, &request_ctx_clone));

	// Factory B sees the same root as Factory A
	assert!(Arc::ptr_eq(&factory_b_root, &root_clone));

	// Factory B has its own current context (distinct from Factory A's)
	assert!(!Arc::ptr_eq(&factory_b_current, &factory_a_ctx));

	// Factory B can access shared singleton data through the root context
	assert_eq!(factory_b_config, Some("app-config".to_string()));
}

#[rstest]
#[tokio::test]
async fn router_factory_passes_context_to_unified_router() {
	// Arrange -- simulate the specific Issue #3402 scenario:
	// RouterFactory uses get_di_context() to get the InjectionContext,
	// then passes it as a constructor argument to UnifiedRouter
	let singleton_scope = Arc::new(SingletonScope::new());
	let root = build_context_with_scope(Arc::clone(&singleton_scope));
	let request_ctx = build_context_with_scope(Arc::clone(&singleton_scope));
	let request_ctx_clone = Arc::clone(&request_ctx);

	let ctx = ResolveContext {
		root: Arc::clone(&root),
		current: Arc::clone(&request_ctx),
	};

	// Act
	let captured_ctx: Arc<InjectionContext> = RESOLVE_CTX
		.scope(ctx, async {
			// This is what the RouterFactory body does:
			// let ctx = get_di_context(ContextLevel::Current);
			// UnifiedRouter::new(ctx)
			//
			// We verify that the context obtained is the correct one
			get_di_context(ContextLevel::Current)
		})
		.await;

	// Assert -- the captured context is the request-scoped fork
	assert!(Arc::ptr_eq(&captured_ctx, &request_ctx_clone));
}

#[rstest]
#[tokio::test]
async fn context_unavailable_outside_factory_chain() {
	// Arrange -- no RESOLVE_CTX scope is active

	// Act
	let result = try_get_di_context(ContextLevel::Current);

	// Assert -- outside of any factory/dispatch scope, context is None
	assert!(result.is_none());
}

#[rstest]
#[tokio::test]
async fn three_level_factory_chain_propagates_root() {
	// Arrange -- three-level chain: App -> Router -> Handler
	// Each level gets its own current context, but root is shared throughout
	let singleton_scope = Arc::new(SingletonScope::new());
	singleton_scope.set(42u64);
	let root = build_context_with_scope(Arc::clone(&singleton_scope));
	let root_clone = Arc::clone(&root);

	let app_current = build_context_with_scope(Arc::clone(&singleton_scope));
	let router_current = build_context_with_scope(Arc::clone(&singleton_scope));
	let handler_current = build_context_with_scope(Arc::clone(&singleton_scope));
	let handler_current_clone = Arc::clone(&handler_current);

	// Act
	let (app_root, router_root, handler_root, handler_ctx, handler_val) = RESOLVE_CTX
		.scope(
			ResolveContext {
				root: Arc::clone(&root),
				current: app_current,
			},
			async {
				let app_root = get_di_context(ContextLevel::Root);

				RESOLVE_CTX
					.scope(
						ResolveContext {
							root: Arc::clone(&root_clone),
							current: router_current,
						},
						async {
							let router_root = get_di_context(ContextLevel::Root);

							let (handler_root, handler_ctx, handler_val) = RESOLVE_CTX
								.scope(
									ResolveContext {
										root: Arc::clone(&root_clone),
										current: handler_current,
									},
									async {
										let h_root = get_di_context(ContextLevel::Root);
										let h_current = get_di_context(ContextLevel::Current);
										let val = h_root.get_singleton::<u64>().map(|v| *v);
										(h_root, h_current, val)
									},
								)
								.await;

							(
								app_root,
								router_root,
								handler_root,
								handler_ctx,
								handler_val,
							)
						},
					)
					.await
			},
		)
		.await;

	// Assert -- all three levels see the same root
	assert!(Arc::ptr_eq(&app_root, &root_clone));
	assert!(Arc::ptr_eq(&router_root, &root_clone));
	assert!(Arc::ptr_eq(&handler_root, &root_clone));

	// Handler has its own current context
	assert!(Arc::ptr_eq(&handler_ctx, &handler_current_clone));
	assert!(!Arc::ptr_eq(&handler_ctx, &root_clone));

	// Singleton data is accessible at the deepest level
	assert_eq!(handler_val, Some(42u64));
}
