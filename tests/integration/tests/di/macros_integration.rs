//! Dependency injection macros integration tests
//!
//! Integration tests for reinhardt-macros' dependency injection macros
//! (`#[endpoint]` and `#[use_inject]`) working with reinhardt-di's
//! injection context and scope management.
//!
//! These tests verify that the procedural macros correctly integrate with
//! the DI system to inject dependencies into handler functions.
//!
//! # Router-Compatible Signature
//!
//! The macro-generated wrapper functions have signature `Fn(Request) -> Fut`
//! to be compatible with `UnifiedRouter::function()`. The DI context is
//! extracted from `Request.di_context()` inside the wrapper:
//!
//! ```ignore
//! // Generated signature (router-compatible):
//! async fn handler(request: Request) -> Result<T>
//!
//! // Inside the wrapper:
//! let __di_ctx = request.di_context()
//!     .ok_or_else(|| Error::internal_server_error("DI context not set"))?;
//! ```

use bytes::Bytes;
use hyper::{HeaderMap, Method, Version};
use reinhardt_core::exception::Result as ExceptionResult;
use reinhardt_di::{Injectable, InjectionContext, SingletonScope};
use reinhardt_http::Request;
use reinhardt_macros::use_inject;
use std::sync::Arc;

/// Helper to create a test Request with DI context attached
fn create_test_request_with_di(ctx: Arc<InjectionContext>) -> Request {
	let mut req = Request::builder()
		.method(Method::GET)
		.uri("/test")
		.version(Version::HTTP_11)
		.headers(HeaderMap::new())
		.body(Bytes::new())
		.build()
		.expect("Failed to build request");
	req.set_di_context(ctx);
	req
}

// ========== Test Data Structures ==========

#[derive(Clone, Default, Debug, PartialEq)]
struct Database {
	connection_string: String,
}

#[async_trait::async_trait]
impl reinhardt_di::Injectable for Database {
	async fn inject(_ctx: &reinhardt_di::InjectionContext) -> reinhardt_di::DiResult<Self> {
		Ok(Self::default())
	}
}

#[derive(Clone, Default, Debug)]
struct Config {
	api_key: String,
	max_connections: usize,
}

#[async_trait::async_trait]
impl reinhardt_di::Injectable for Config {
	async fn inject(_ctx: &reinhardt_di::InjectionContext) -> reinhardt_di::DiResult<Self> {
		Ok(Self::default())
	}
}

#[derive(Clone, Default, Debug)]
struct Logger {
	level: String,
}

#[async_trait::async_trait]
impl reinhardt_di::Injectable for Logger {
	async fn inject(_ctx: &reinhardt_di::InjectionContext) -> reinhardt_di::DiResult<Self> {
		Ok(Self::default())
	}
}

#[derive(Clone)]
struct CustomService {
	value: i32,
}

#[async_trait::async_trait]
impl Injectable for CustomService {
	async fn inject(_ctx: &InjectionContext) -> reinhardt_di::DiResult<Self> {
		Ok(CustomService { value: 42 })
	}
}

// ========== use_inject Macro Tests ==========
//
// Note: Both #[endpoint] and #[use_inject] require a Request parameter
// because they share the same implementation that extracts InjectionContext
// from Request.extensions.

#[use_inject]
async fn handler_with_inject(
	_req: Request,
	#[inject] db: Database,
	regular_param: String,
) -> ExceptionResult<String> {
	Ok(format!("db: {:?}, param: {}", db, regular_param))
}

#[use_inject]
async fn handler_multiple_inject(
	_req: Request,
	#[inject] db: Database,
	#[inject] config: Config,
	regular_param: i32,
) -> ExceptionResult<String> {
	Ok(format!(
		"db: {:?}, config key: {}, param: {}",
		db, config.api_key, regular_param
	))
}

#[use_inject]
async fn handler_with_cache_control(
	_req: Request,
	#[inject] db: Database,
	#[inject(cache = false)] service: CustomService,
) -> ExceptionResult<i32> {
	Ok(db.connection_string.len() as i32 + service.value)
}

#[use_inject]
async fn handler_only_inject(_req: Request, #[inject] db: Database) -> ExceptionResult<Database> {
	// db is directly injected as Database type
	Ok(db)
}

#[use_inject]
async fn handler_no_inject(
	_req: Request,
	regular_param1: String,
	regular_param2: i32,
) -> ExceptionResult<String> {
	Ok(format!("{}-{}", regular_param1, regular_param2))
}

#[tokio::test]
async fn test_endpoint_with_single_inject() {
	let singleton = Arc::new(SingletonScope::new());
	let ctx = Arc::new(InjectionContext::builder(singleton).build());

	let req = create_test_request_with_di(ctx);

	// Router-compatible signature: handler(request, regular_params...)
	// DI context is extracted from Request.di_context() inside the wrapper
	let result = handler_with_inject(req, "test_value".to_string())
		.await
		.unwrap();

	// Check that it includes db info and regular param
	assert!(result.contains("db:"));
	assert!(result.contains("param: test_value"));
}

#[tokio::test]
async fn test_endpoint_with_multiple_inject() {
	let singleton = Arc::new(SingletonScope::new());
	let ctx = Arc::new(InjectionContext::builder(singleton).build());

	let req = create_test_request_with_di(ctx);

	// Router-compatible signature: handler(request, regular_params...)
	let result = handler_multiple_inject(req, 100).await.unwrap();

	// Check that it includes db, config, and regular param info
	assert!(result.contains("db:"));
	assert!(result.contains("config key:"));
	assert!(result.contains("param: 100"));
}

#[tokio::test]
async fn test_endpoint_with_cache_control() {
	let singleton = Arc::new(SingletonScope::new());
	let ctx = Arc::new(InjectionContext::builder(singleton).build());

	let req = create_test_request_with_di(ctx);

	// Router-compatible signature: handler(request)
	let result = handler_with_cache_control(req).await.unwrap();

	assert_eq!(result, 42); // service.value
}

#[tokio::test]
async fn test_endpoint_only_inject_params() {
	let singleton = Arc::new(SingletonScope::new());
	let ctx = Arc::new(InjectionContext::builder(singleton).build());

	let req = create_test_request_with_di(ctx);

	// Router-compatible signature: handler(request)
	let result = handler_only_inject(req).await.unwrap();

	assert_eq!(result.connection_string, "");
}

#[tokio::test]
async fn test_endpoint_no_inject_params() {
	let singleton = Arc::new(SingletonScope::new());
	let ctx = Arc::new(InjectionContext::builder(singleton).build());

	let req = create_test_request_with_di(ctx);

	// Router-compatible signature: handler(request, regular_params...)
	// Note: No DI context extraction needed when no #[inject] params
	let result = handler_no_inject(req, "hello".to_string(), 42)
		.await
		.unwrap();

	assert_eq!(result, "hello-42");
}

// ========== Additional use_inject Macro Tests ==========

#[use_inject]
async fn process_data(
	_req: Request,
	#[inject] db: Database,
	data: String,
) -> ExceptionResult<String> {
	Ok(format!(
		"Processing {} with db: {:?}",
		data, db.connection_string
	))
}

#[use_inject]
async fn complex_handler(
	_req: Request,
	#[inject] db: Database,
	#[inject] config: Config,
	#[inject] logger: Logger,
	input: i32,
) -> ExceptionResult<String> {
	Ok(format!(
		"Input: {}, DB: {:?}, Config: {}, Logger: {:?}",
		input, db.connection_string, config.max_connections, logger.level
	))
}

#[use_inject]
async fn cache_control(
	_req: Request,
	#[inject] cached_db: Database,
	#[inject(cache = false)] fresh_logger: Logger,
) -> ExceptionResult<String> {
	Ok(format!(
		"DB: {:?}, Logger: {:?}",
		cached_db.connection_string, fresh_logger.level
	))
}

#[use_inject]
async fn validate_user(
	_req: Request,
	#[inject] _db: Database,
	user_id: u64,
) -> ExceptionResult<bool> {
	// For testing purposes, just check user_id validity
	// (db is injected to test DI functionality, not used in validation logic)
	Ok(user_id > 0)
}

#[use_inject]
async fn calculate_report(
	_req: Request,
	#[inject] db: Database,
	#[inject] config: Config,
	year: i32,
	month: i32,
) -> ExceptionResult<String> {
	Ok(format!(
		"Report for {}/{} using {} connections from {:?}",
		year, month, config.max_connections, db.connection_string
	))
}

#[tokio::test]
async fn test_simple_injection() {
	let singleton = Arc::new(SingletonScope::new());
	let ctx = Arc::new(InjectionContext::builder(singleton).build());

	let req = create_test_request_with_di(ctx);

	// Router-compatible signature: handler(request, regular_params...)
	let result = process_data(req, "test_data".to_string()).await.unwrap();
	assert!(result.contains("Processing test_data"));
}

#[tokio::test]
async fn test_multiple_injections() {
	let singleton = Arc::new(SingletonScope::new());
	let ctx = Arc::new(InjectionContext::builder(singleton).build());

	let req = create_test_request_with_di(ctx);

	// Router-compatible signature: handler(request, regular_params...)
	let result = complex_handler(req, 42).await.unwrap();
	assert!(result.contains("Input: 42"));
	assert!(result.contains("DB:"));
	assert!(result.contains("Config:"));
	assert!(result.contains("Logger:"));
}

#[tokio::test]
async fn test_use_inject_cache_control() {
	let singleton = Arc::new(SingletonScope::new());
	let ctx = Arc::new(InjectionContext::builder(singleton).build());

	let req = create_test_request_with_di(ctx);

	// Router-compatible signature: handler(request)
	let result = cache_control(req).await.unwrap();
	assert!(result.contains("DB:"));
	assert!(result.contains("Logger:"));
}

#[tokio::test]
async fn test_helper_function() {
	let singleton = Arc::new(SingletonScope::new());
	let ctx = Arc::new(InjectionContext::builder(singleton).build());

	let req1 = create_test_request_with_di(ctx.clone());
	let req2 = create_test_request_with_di(ctx);

	// Router-compatible signature: handler(request, regular_params...)
	let valid = validate_user(req1, 12345).await.unwrap();
	assert!(valid);

	let invalid = validate_user(req2, 0).await.unwrap();
	assert!(!invalid);
}

#[tokio::test]
async fn test_business_logic() {
	let singleton = Arc::new(SingletonScope::new());
	let ctx = Arc::new(InjectionContext::builder(singleton).build());

	let req = create_test_request_with_di(ctx);

	// Router-compatible signature: handler(request, regular_params...)
	let result = calculate_report(req, 2025, 10).await.unwrap();
	assert!(result.contains("Report for 2025/10"));
	assert!(result.contains("connections"));
}

#[tokio::test]
async fn test_injection_caching() {
	let singleton = Arc::new(SingletonScope::new());
	let ctx = Arc::new(InjectionContext::builder(singleton).build());

	let req1 = create_test_request_with_di(ctx.clone());
	let req2 = create_test_request_with_di(ctx);

	// Router-compatible signature: handler(request, regular_params...)
	// First call
	let result1 = process_data(req1, "data1".to_string()).await.unwrap();
	// Second call - should use cached context
	let result2 = process_data(req2, "data2".to_string()).await.unwrap();

	assert!(result1.contains("data1"));
	assert!(result2.contains("data2"));
}
