//! Tests for reinhardt-params integration with DI

use reinhardt_di::{DiResult, Injectable, InjectionContext, SingletonScope};
use rstest::*;
use std::sync::Arc;

/// Mock request context for testing parameter extraction
#[derive(Clone, Debug)]
struct MockRequestContext {
	query_params: Vec<(String, String)>,
	path_params: Vec<(String, String)>,
	headers: Vec<(String, String)>,
}

impl MockRequestContext {
	fn new() -> Self {
		Self {
			query_params: Vec::new(),
			path_params: Vec::new(),
			headers: Vec::new(),
		}
	}

	fn with_query(mut self, key: &str, value: &str) -> Self {
		self.query_params.push((key.to_string(), value.to_string()));
		self
	}

	fn with_path(mut self, key: &str, value: &str) -> Self {
		self.path_params.push((key.to_string(), value.to_string()));
		self
	}

	fn with_header(mut self, key: &str, value: &str) -> Self {
		self.headers.push((key.to_string(), value.to_string()));
		self
	}
}

/// Test Query parameter injection through DI context
#[rstest]
#[tokio::test]
async fn test_query_param_injection() {
	let mock_ctx = MockRequestContext::new()
		.with_query("page", "2")
		.with_query("limit", "10");

	let singleton = Arc::new(SingletonScope::new());
	singleton.set(mock_ctx.clone());

	let ctx = InjectionContext::builder(singleton).build();

	// Simulate query parameter extraction
	let request_ctx = ctx.get_singleton::<MockRequestContext>().unwrap();
	let page_param = request_ctx
		.query_params
		.iter()
		.find(|(k, _)| k == "page")
		.map(|(_, v)| v.clone());

	assert_eq!(page_param, Some("2".to_string()));

	let limit_param = request_ctx
		.query_params
		.iter()
		.find(|(k, _)| k == "limit")
		.map(|(_, v)| v.clone());

	assert_eq!(limit_param, Some("10".to_string()));
}

/// Test Path parameter injection through DI context
#[rstest]
#[tokio::test]
async fn test_path_param_injection() {
	let mock_ctx = MockRequestContext::new()
		.with_path("user_id", "123")
		.with_path("org_id", "456");

	let singleton = Arc::new(SingletonScope::new());
	singleton.set(mock_ctx.clone());

	let ctx = InjectionContext::builder(singleton).build();

	// Simulate path parameter extraction
	let request_ctx = ctx.get_singleton::<MockRequestContext>().unwrap();
	let user_id = request_ctx
		.path_params
		.iter()
		.find(|(k, _)| k == "user_id")
		.map(|(_, v)| v.clone());

	assert_eq!(user_id, Some("123".to_string()));

	let org_id = request_ctx
		.path_params
		.iter()
		.find(|(k, _)| k == "org_id")
		.map(|(_, v)| v.clone());

	assert_eq!(org_id, Some("456".to_string()));
}

/// Test Header injection through DI context
#[rstest]
#[tokio::test]
async fn test_header_injection() {
	let mock_ctx = MockRequestContext::new()
		.with_header("authorization", "Bearer token123")
		.with_header("content-type", "application/json");

	let singleton = Arc::new(SingletonScope::new());
	singleton.set(mock_ctx.clone());

	let ctx = InjectionContext::builder(singleton).build();

	// Simulate header extraction
	let request_ctx = ctx.get_singleton::<MockRequestContext>().unwrap();
	let auth_header = request_ctx
		.headers
		.iter()
		.find(|(k, _)| k == "authorization")
		.map(|(_, v)| v.clone());

	assert_eq!(auth_header, Some("Bearer token123".to_string()));

	let content_type = request_ctx
		.headers
		.iter()
		.find(|(k, _)| k == "content-type")
		.map(|(_, v)| v.clone());

	assert_eq!(content_type, Some("application/json".to_string()));
}

/// Test combination of multiple parameter types
#[rstest]
#[tokio::test]
async fn test_multiple_param_types() {
	let mock_ctx = MockRequestContext::new()
		.with_query("filter", "active")
		.with_path("resource_id", "789")
		.with_header("x-api-key", "secret123");

	let singleton = Arc::new(SingletonScope::new());
	singleton.set(mock_ctx.clone());

	let ctx = InjectionContext::builder(singleton).build();

	// Extract all parameter types
	let request_ctx = ctx.get_singleton::<MockRequestContext>().unwrap();

	let filter = request_ctx
		.query_params
		.iter()
		.find(|(k, _)| k == "filter")
		.map(|(_, v)| v.clone());

	let resource_id = request_ctx
		.path_params
		.iter()
		.find(|(k, _)| k == "resource_id")
		.map(|(_, v)| v.clone());

	let api_key = request_ctx
		.headers
		.iter()
		.find(|(k, _)| k == "x-api-key")
		.map(|(_, v)| v.clone());

	assert_eq!(filter, Some("active".to_string()));
	assert_eq!(resource_id, Some("789".to_string()));
	assert_eq!(api_key, Some("secret123".to_string()));
}

/// Test optional parameters handling
#[rstest]
#[tokio::test]
async fn test_optional_params() {
	let mock_ctx = MockRequestContext::new().with_query("page", "1");
	// Note: no "limit" parameter

	let singleton = Arc::new(SingletonScope::new());
	singleton.set(mock_ctx.clone());

	let ctx = InjectionContext::builder(singleton).build();

	let request_ctx = ctx.get_singleton::<MockRequestContext>().unwrap();

	let page = request_ctx
		.query_params
		.iter()
		.find(|(k, _)| k == "page")
		.map(|(_, v)| v.clone());

	let limit = request_ctx
		.query_params
		.iter()
		.find(|(k, _)| k == "limit")
		.map(|(_, v)| v.clone());

	assert_eq!(page, Some("1".to_string()));
	assert_eq!(limit, None); // Optional parameter not provided
}

/// Test parameters combined with other dependencies
#[rstest]
#[tokio::test]
async fn test_params_with_dependencies() {
	#[derive(Clone, Debug)]
	struct Database {
		url: String,
	}

	#[async_trait::async_trait]
	impl Injectable for Database {
		async fn inject(_ctx: &InjectionContext) -> DiResult<Self> {
			Ok(Database {
				url: "postgres://localhost/db".to_string(),
			})
		}
	}

	// Setup request context with parameters
	let mock_ctx = MockRequestContext::new()
		.with_query("tenant_id", "tenant-123")
		.with_header("authorization", "Bearer token");

	let singleton = Arc::new(SingletonScope::new());
	singleton.set(mock_ctx.clone());

	let ctx = InjectionContext::builder(singleton).build();

	// Inject database dependency
	let db = Database::inject(&ctx).await.unwrap();
	assert_eq!(db.url, "postgres://localhost/db");

	// Extract parameters
	let request_ctx = ctx.get_singleton::<MockRequestContext>().unwrap();

	let tenant_id = request_ctx
		.query_params
		.iter()
		.find(|(k, _)| k == "tenant_id")
		.map(|(_, v)| v.clone());

	let auth = request_ctx
		.headers
		.iter()
		.find(|(k, _)| k == "authorization")
		.map(|(_, v)| v.clone());

	assert_eq!(tenant_id, Some("tenant-123".to_string()));
	assert_eq!(auth, Some("Bearer token".to_string()));
}
