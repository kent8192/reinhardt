//! Server integration tests (DI + Server)
//!
//! Tests dependency injection with reinhardt-server:
//! 1. Server with DI middleware
//! 2. Endpoint handler with DI
//! 3. WebSocket connection with DI
//! 4. Server shutdown cleanup

use reinhardt_di::{DiResult, Injectable, InjectionContext, SingletonScope};
use rstest::*;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

// Application state service
#[derive(Clone)]
struct AppState {
	request_count: Arc<AtomicUsize>,
	is_healthy: Arc<AtomicBool>,
}

impl AppState {
	fn new() -> Self {
		Self {
			request_count: Arc::new(AtomicUsize::new(0)),
			is_healthy: Arc::new(AtomicBool::new(true)),
		}
	}

	fn increment_requests(&self) -> usize {
		self.request_count.fetch_add(1, Ordering::SeqCst) + 1
	}

	fn get_request_count(&self) -> usize {
		self.request_count.load(Ordering::SeqCst)
	}

	fn set_healthy(&self, healthy: bool) {
		self.is_healthy.store(healthy, Ordering::SeqCst);
	}

	fn is_healthy(&self) -> bool {
		self.is_healthy.load(Ordering::SeqCst)
	}
}

#[async_trait::async_trait]
impl Injectable for AppState {
	async fn inject(ctx: &InjectionContext) -> DiResult<Self> {
		// AppState is singleton
		if let Some(cached) = ctx.get_singleton::<AppState>() {
			return Ok((*cached).clone());
		}

		let state = AppState::new();
		ctx.set_singleton(state.clone());
		Ok(state)
	}
}

// Request-scoped service
#[derive(Clone)]
struct RequestContext {
	request_id: String,
}

#[async_trait::async_trait]
impl Injectable for RequestContext {
	async fn inject(ctx: &InjectionContext) -> DiResult<Self> {
		if let Some(cached) = ctx.get_request::<RequestContext>() {
			return Ok((*cached).clone());
		}

		let request_ctx = RequestContext {
			request_id: uuid::Uuid::new_v4().to_string(),
		};

		ctx.set_request(request_ctx.clone());
		Ok(request_ctx)
	}
}

// Endpoint handler service
#[derive(Clone)]
struct EndpointHandler {
	app_state: AppState,
	request_ctx: RequestContext,
}

#[async_trait::async_trait]
impl Injectable for EndpointHandler {
	async fn inject(ctx: &InjectionContext) -> DiResult<Self> {
		let app_state = AppState::inject(ctx).await?;
		let request_ctx = RequestContext::inject(ctx).await?;

		Ok(EndpointHandler {
			app_state,
			request_ctx,
		})
	}
}

#[rstest]
#[tokio::test]
async fn test_server_with_di_middleware() {
	// Setup DI context (simulating server middleware)
	let singleton = Arc::new(SingletonScope::new());
	let ctx = InjectionContext::builder(singleton).build();

	// Inject AppState (singleton)
	let state = AppState::inject(&ctx).await.unwrap();

	// Simulate middleware incrementing request count
	assert_eq!(state.increment_requests(), 1);
	assert_eq!(state.increment_requests(), 2);

	// Verify state is healthy
	assert!(state.is_healthy());
}

#[rstest]
#[tokio::test]
async fn test_endpoint_dependency_injection() {
	// Setup DI context
	let singleton = Arc::new(SingletonScope::new());

	// Request 1
	let ctx1 = InjectionContext::builder(singleton.clone()).build();

	let handler1 = EndpointHandler::inject(&ctx1).await.unwrap();
	let request_id1 = handler1.request_ctx.request_id.clone();
	assert_eq!(handler1.app_state.increment_requests(), 1);

	// Request 2 (different request scope)
	let ctx2 = InjectionContext::builder(singleton.clone()).build();

	let handler2 = EndpointHandler::inject(&ctx2).await.unwrap();
	let request_id2 = handler2.request_ctx.request_id.clone();

	// Request IDs should be different (request-scoped)
	assert_ne!(request_id1, request_id2);

	// But app state is shared (singleton)
	assert_eq!(handler2.app_state.get_request_count(), 1);
}

#[rstest]
#[tokio::test]
async fn test_websocket_with_di() {
	// Setup DI context for WebSocket connection
	let singleton = Arc::new(SingletonScope::new());
	let ctx = InjectionContext::builder(singleton).build();

	// Inject app state
	let state = AppState::inject(&ctx).await.unwrap();

	// Simulate WebSocket connection lifecycle
	state.set_healthy(true);
	assert!(state.is_healthy());

	// Simulate connection handling
	state.increment_requests();
	assert_eq!(state.get_request_count(), 1);

	// Simulate connection close
	state.set_healthy(false);
	assert!(!state.is_healthy());
}

#[rstest]
#[tokio::test]
async fn test_server_shutdown_cleanup() {
	// Setup DI context
	let singleton = Arc::new(SingletonScope::new());

	// Create multiple request contexts
	let mut contexts = Vec::new();

	for _ in 0..5 {
		let ctx = InjectionContext::builder(singleton.clone()).build();

		// Inject services
		let state = AppState::inject(&ctx).await.unwrap();
		state.increment_requests();

		contexts.push(ctx);
	}

	// Verify all requests were counted
	let final_ctx = InjectionContext::builder(singleton.clone()).build();
	let final_state = AppState::inject(&final_ctx).await.unwrap();
	assert_eq!(final_state.get_request_count(), 5);

	// Simulate server shutdown by dropping all contexts
	drop(contexts);

	// AppState (singleton) should still exist
	let state_after_shutdown = AppState::inject(&final_ctx).await.unwrap();
	assert_eq!(state_after_shutdown.get_request_count(), 5);
}
