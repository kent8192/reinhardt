//! Cross-crate injection tests (DI + HTTP integration)
//!
//! Tests dependency injection with reinhardt-http components:
//! 1. HTTP Request injection
//! 2. Singleton dependency sharing across requests
//! 3. RequestScope isolation per request
//! 4. Dependency overrides in HTTP context

use bytes::Bytes;
use hyper::Method;
use reinhardt_di::{DiResult, Injectable, InjectionContext, SingletonScope};
use reinhardt_http::Request;
use rstest::*;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

// Service that depends on HTTP Request
#[derive(Clone)]
struct RequestService {
	method: String,
	path: String,
}

#[async_trait::async_trait]
impl Injectable for RequestService {
	async fn inject(ctx: &InjectionContext) -> DiResult<Self> {
		// Get HTTP request from context
		let req = ctx.get_http_request().ok_or_else(|| {
			reinhardt_di::DiError::NotFound("Request not found in context".to_string())
		})?;

		Ok(RequestService {
			method: req.method.to_string(),
			path: req.uri.path().to_string(),
		})
	}
}

// Singleton counter service
#[derive(Clone)]
struct CounterService {
	counter: Arc<AtomicUsize>,
}

impl CounterService {
	fn increment(&self) -> usize {
		self.counter.fetch_add(1, Ordering::SeqCst) + 1
	}

	fn get(&self) -> usize {
		self.counter.load(Ordering::SeqCst)
	}
}

#[async_trait::async_trait]
impl Injectable for CounterService {
	async fn inject(ctx: &InjectionContext) -> DiResult<Self> {
		// Check if already in singleton scope
		if let Some(cached) = ctx.get_singleton::<CounterService>() {
			return Ok((*cached).clone());
		}

		// Create new counter
		let service = CounterService {
			counter: Arc::new(AtomicUsize::new(0)),
		};

		ctx.set_singleton(service.clone());
		Ok(service)
	}
}

#[rstest]
#[tokio::test]
async fn test_http_request_injection() {
	// Create HTTP request
	let req = Request::builder()
		.method(Method::GET)
		.uri("/api/users")
		.body(Bytes::new())
		.build()
		.unwrap();

	// Setup DI context with HTTP request
	let singleton = Arc::new(SingletonScope::new());
	let ctx = InjectionContext::builder(singleton)
		.with_request(req)
		.build();

	// Inject RequestService
	let service = RequestService::inject(&ctx).await.unwrap();

	assert_eq!(service.method, "GET");
	assert_eq!(service.path, "/api/users");
}

#[rstest]
#[tokio::test]
async fn test_dependency_shared_across_requests() {
	let singleton = Arc::new(SingletonScope::new());

	// First request context
	let ctx1 = InjectionContext::builder(singleton.clone()).build();

	let counter1 = CounterService::inject(&ctx1).await.unwrap();
	assert_eq!(counter1.increment(), 1);

	// Second request context (same singleton)
	let ctx2 = InjectionContext::builder(singleton.clone()).build();

	let counter2 = CounterService::inject(&ctx2).await.unwrap();

	// Counter is shared (same singleton instance)
	assert_eq!(counter2.get(), 1); // Same count
	assert_eq!(counter2.increment(), 2); // Increment continues
}

#[rstest]
#[tokio::test]
async fn test_request_scope_per_request() {
	let singleton = Arc::new(SingletonScope::new());

	// Request 1 context
	let ctx1 = InjectionContext::builder(singleton.clone()).build();
	ctx1.set_request("request_1_data".to_string());

	// Request 2 context
	let ctx2 = InjectionContext::builder(singleton.clone()).build();
	ctx2.set_request("request_2_data".to_string());

	// Verify isolation
	let data1: Option<Arc<String>> = ctx1.get_request();
	let data2: Option<Arc<String>> = ctx2.get_request();

	assert_eq!(*data1.unwrap(), "request_1_data");
	assert_eq!(*data2.unwrap(), "request_2_data");
}

#[rstest]
#[tokio::test]
async fn test_override_in_http_context() {
	// Setup normal context
	let singleton = Arc::new(SingletonScope::new());
	let ctx = InjectionContext::builder(singleton).build();

	// Inject counter normally
	let counter = CounterService::inject(&ctx).await.unwrap();
	assert_eq!(counter.increment(), 1);

	// Override with custom counter
	let custom_counter = CounterService {
		counter: Arc::new(AtomicUsize::new(100)),
	};

	ctx.set_singleton(custom_counter.clone());

	// Inject again - should get overridden counter
	let overridden = CounterService::inject(&ctx).await.unwrap();
	assert_eq!(overridden.get(), 100);
	assert_eq!(overridden.increment(), 101);
}
