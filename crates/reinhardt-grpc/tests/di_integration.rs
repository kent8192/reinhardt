//! Integration tests for gRPC dependency injection
//!
//! These tests verify that the `#[grpc_handler]` macro correctly integrates
//! with the DI system.

#![cfg(feature = "di")]

use reinhardt_di::{DiError, Injectable, InjectionContext, SingletonScope};
use reinhardt_grpc::grpc_handler;
use std::sync::{Arc, Mutex};
use tonic::{Request, Response, Status};

/// Mock database connection for testing
#[derive(Clone)]
struct MockDatabase {
	calls: Arc<Mutex<Vec<String>>>,
}

impl MockDatabase {
	fn new() -> Self {
		Self {
			calls: Arc::new(Mutex::new(Vec::new())),
		}
	}

	async fn fetch_user(&self, user_id: &str) -> Result<String, String> {
		self.calls
			.lock()
			.unwrap()
			.push(format!("fetch_user({})", user_id));
		Ok(format!("User:{}", user_id))
	}
}

#[async_trait::async_trait]
impl Injectable for MockDatabase {
	async fn inject(_ctx: &InjectionContext) -> Result<Self, DiError> {
		Ok(MockDatabase::new())
	}
}

/// Mock cache for testing
#[derive(Clone)]
struct MockCache {
	calls: Arc<Mutex<Vec<String>>>,
}

impl MockCache {
	fn new() -> Self {
		Self {
			calls: Arc::new(Mutex::new(Vec::new())),
		}
	}

	async fn get(&self, key: &str) -> Option<String> {
		self.calls.lock().unwrap().push(format!("get({})", key));
		None
	}
}

#[async_trait::async_trait]
impl Injectable for MockCache {
	async fn inject(_ctx: &InjectionContext) -> Result<Self, DiError> {
		Ok(MockCache::new())
	}
}

/// Mock request type
#[derive(Debug)]
struct GetUserRequest {
	id: String,
}

/// Test service implementation
struct TestService {}

impl TestService {
	#[grpc_handler]
	async fn get_user(
		&self,
		request: Request<GetUserRequest>,
		#[inject] db: MockDatabase,
	) -> Result<Response<String>, Status> {
		let user_id = &request.into_inner().id;
		let user = db
			.fetch_user(user_id)
			.await
			.map_err(|e| Status::not_found(e))?;
		Ok(Response::new(user))
	}

	#[grpc_handler]
	async fn get_user_with_cache(
		&self,
		request: Request<GetUserRequest>,
		#[inject] db: MockDatabase,
		#[inject] cache: MockCache,
	) -> Result<Response<String>, Status> {
		let user_id = &request.into_inner().id;

		// Check cache first
		if let Some(user) = cache.get(user_id).await {
			return Ok(Response::new(user));
		}

		// Fetch from database
		let user = db
			.fetch_user(user_id)
			.await
			.map_err(|e| Status::not_found(e))?;
		Ok(Response::new(user))
	}

	#[grpc_handler]
	async fn get_user_uncached(
		&self,
		request: Request<GetUserRequest>,
		#[inject(cache = false)] db: MockDatabase,
	) -> Result<Response<String>, Status> {
		let user_id = &request.into_inner().id;
		let user = db
			.fetch_user(user_id)
			.await
			.map_err(|e| Status::not_found(e))?;
		Ok(Response::new(user))
	}
}

#[tokio::test]
async fn test_grpc_handler_basic_di() {
	// Setup
	let singleton_scope = Arc::new(SingletonScope::new());
	let ctx = Arc::new(InjectionContext::builder(singleton_scope).build());
	let service = TestService {};

	// Create request with DI context
	let mut request = Request::new(GetUserRequest {
		id: "123".to_string(),
	});
	request.extensions_mut().insert(ctx.clone());

	// Call handler
	let response = service.get_user(request).await;

	// Verify
	assert!(response.is_ok());
	let user = response.unwrap().into_inner();
	assert_eq!(user, "User:123");
}

#[tokio::test]
async fn test_grpc_handler_multiple_dependencies() {
	// Setup
	let singleton_scope = Arc::new(SingletonScope::new());
	let ctx = Arc::new(InjectionContext::builder(singleton_scope).build());
	let service = TestService {};

	// Create request with DI context
	let mut request = Request::new(GetUserRequest {
		id: "456".to_string(),
	});
	request.extensions_mut().insert(ctx.clone());

	// Call handler
	let response = service.get_user_with_cache(request).await;

	// Verify
	assert!(response.is_ok());
	let user = response.unwrap().into_inner();
	assert_eq!(user, "User:456");
}

#[tokio::test]
async fn test_grpc_handler_missing_di_context() {
	// Setup
	let singleton_scope = Arc::new(SingletonScope::new());
	let _ctx = Arc::new(InjectionContext::builder(singleton_scope).build());
	let service = TestService {};

	// Create request WITHOUT DI context
	let request = Request::new(GetUserRequest {
		id: "789".to_string(),
	});

	// Call handler - should fail
	let response = service.get_user(request).await;

	// Verify
	assert!(response.is_err());
	let status = response.unwrap_err();
	assert_eq!(status.code(), tonic::Code::Internal);
	// Security: error message should be generic, not exposing DI internals
	assert_eq!(status.message(), "Internal server error");
}

#[tokio::test]
async fn test_grpc_handler_cache_control() {
	// Setup
	let singleton_scope = Arc::new(SingletonScope::new());
	let ctx = Arc::new(InjectionContext::builder(singleton_scope).build());
	let service = TestService {};

	// Create first request with DI context
	let mut request1 = Request::new(GetUserRequest {
		id: "111".to_string(),
	});
	request1.extensions_mut().insert(ctx.clone());

	// Call handler first time
	let response1 = service.get_user_uncached(request1).await;
	assert!(response1.is_ok());

	// Create second request with DI context
	let mut request2 = Request::new(GetUserRequest {
		id: "222".to_string(),
	});
	request2.extensions_mut().insert(ctx.clone());

	// Call handler second time
	let response2 = service.get_user_uncached(request2).await;
	assert!(response2.is_ok());
}
