//! End-to-End Integration Tests for Dependency Injection in ViewSets
//!
//! Tests the complete DI system integration with HTTP lifecycle, routing,
//! middleware, and multiple ViewSet interactions.

use async_trait::async_trait;
use bytes::Bytes;
use hyper::{HeaderMap, Method, StatusCode, Version};
use reinhardt_core::exception::Result;
use reinhardt_di::{Injectable, Injected, InjectionContext, SingletonScope, injectable};
use reinhardt_http::{Request, Response};
use reinhardt_views::viewsets::{Action, ActionType, ViewSet};
use std::sync::Arc;

// ============================================================================
// Category 1: HTTP Request/Response Cycle (3 tests)
// ============================================================================

#[tokio::test]
async fn test_complete_request_response_cycle() {
	use std::sync::LazyLock;
	use std::sync::atomic::{AtomicUsize, Ordering};

	static REQUEST_COUNTER: LazyLock<AtomicUsize> = LazyLock::new(|| AtomicUsize::new(0));

	#[derive(Clone)]
	#[injectable]
	#[allow(dead_code)]
	struct RequestLogger {
		#[no_inject]
		count: Arc<AtomicUsize>,
	}

	impl Default for RequestLogger {
		fn default() -> Self {
			Self {
				count: Arc::new(REQUEST_COUNTER.fetch_add(1, Ordering::SeqCst).into()),
			}
		}
	}

	#[derive(Clone)]
	#[injectable]
	#[allow(dead_code)]
	struct LoggingViewSet {
		#[inject]
		logger: Injected<RequestLogger>,
	}

	#[async_trait]
	impl ViewSet for LoggingViewSet {
		fn get_basename(&self) -> &str {
			"logging"
		}

		async fn dispatch(&self, request: Request, _action: Action) -> Result<Response> {
			let method = request.method.as_str();
			let path = request.uri.path();
			let body = format!("Handled {} {} with logger", method, path);

			Ok(Response::ok().with_body(body))
		}
	}

	let singleton = Arc::new(SingletonScope::new());
	let ctx = InjectionContext::builder(singleton).build();

	let viewset = LoggingViewSet::inject(&ctx).await.unwrap();

	let request = Request::builder()
		.method(Method::GET)
		.uri("/api/items/")
		.version(Version::HTTP_11)
		.headers(HeaderMap::new())
		.body(Bytes::new())
		.build()
		.unwrap();

	let response = viewset.dispatch(request, Action::list()).await.unwrap();
	let body = String::from_utf8(response.body.to_vec()).unwrap();

	assert!(body.contains("Handled GET /api/items/ with logger"));
}

#[tokio::test]
async fn test_post_request_with_body() {
	#[derive(Clone)]
	#[injectable]
	struct BodyParser {
		#[no_inject]
		max_size: usize,
	}

	impl Default for BodyParser {
		fn default() -> Self {
			Self { max_size: 1024 }
		}
	}

	#[derive(Clone)]
	#[injectable]
	struct ParsingViewSet {
		#[inject]
		parser: Injected<BodyParser>,
	}

	#[async_trait]
	impl ViewSet for ParsingViewSet {
		fn get_basename(&self) -> &str {
			"parsing"
		}

		async fn dispatch(&self, request: Request, _action: Action) -> Result<Response> {
			let body_len = request.body().len();
			let max = self.parser.max_size;

			if body_len > max {
				Ok(Response::new(StatusCode::BAD_REQUEST).with_body("Body too large"))
			} else {
				let data = String::from_utf8_lossy(request.body());
				Ok(Response::ok().with_body(format!("Parsed: {}", data)))
			}
		}
	}

	let singleton = Arc::new(SingletonScope::new());
	let ctx = InjectionContext::builder(singleton).build();

	let viewset = ParsingViewSet::inject(&ctx).await.unwrap();

	let request = Request::builder()
		.method(Method::POST)
		.uri("/api/items/")
		.version(Version::HTTP_11)
		.headers(HeaderMap::new())
		.body(Bytes::from("test data"))
		.build()
		.unwrap();

	let response = viewset.dispatch(request, Action::create()).await.unwrap();
	let body = String::from_utf8(response.body.to_vec()).unwrap();

	assert_eq!(body, "Parsed: test data");
}

#[tokio::test]
async fn test_headers_extraction() {
	#[derive(Clone)]
	#[injectable]
	struct HeaderValidator {
		#[no_inject]
		required_header: String,
	}

	impl Default for HeaderValidator {
		fn default() -> Self {
			Self {
				required_header: "x-api-key".to_string(),
			}
		}
	}

	#[derive(Clone)]
	#[injectable]
	struct HeaderViewSet {
		#[inject]
		validator: Injected<HeaderValidator>,
	}

	#[async_trait]
	impl ViewSet for HeaderViewSet {
		fn get_basename(&self) -> &str {
			"headers"
		}

		async fn dispatch(&self, request: Request, _action: Action) -> Result<Response> {
			let has_key = request
				.headers
				.contains_key(&self.validator.required_header);

			if has_key {
				Ok(Response::ok().with_body("Authorized"))
			} else {
				Ok(Response::new(StatusCode::UNAUTHORIZED).with_body("Missing API key"))
			}
		}
	}

	let singleton = Arc::new(SingletonScope::new());
	let ctx = InjectionContext::builder(singleton).build();

	let viewset = HeaderViewSet::inject(&ctx).await.unwrap();

	// Request without header
	let mut headers = HeaderMap::new();
	let request1 = Request::builder()
		.method(Method::GET)
		.uri("/api/items/")
		.version(Version::HTTP_11)
		.headers(headers.clone())
		.body(Bytes::new())
		.build()
		.unwrap();

	let response1 = viewset.dispatch(request1, Action::list()).await.unwrap();
	let body1 = String::from_utf8(response1.body.to_vec()).unwrap();
	assert_eq!(body1, "Missing API key");

	// Request with header
	headers.insert("x-api-key", "test-key".parse().unwrap());
	let request2 = Request::builder()
		.method(Method::GET)
		.uri("/api/items/")
		.version(Version::HTTP_11)
		.headers(headers)
		.body(Bytes::new())
		.build()
		.unwrap();

	let response2 = viewset.dispatch(request2, Action::list()).await.unwrap();
	let body2 = String::from_utf8(response2.body.to_vec()).unwrap();
	assert_eq!(body2, "Authorized");
}

// ============================================================================
// Category 2: Multiple ViewSet Interactions (3 tests)
// ============================================================================

#[tokio::test]
async fn test_multiple_viewsets_shared_service() {
	use std::sync::Mutex;

	#[derive(Clone)]
	#[injectable]
	struct SharedCounter {
		#[no_inject]
		value: Arc<Mutex<i32>>,
	}

	impl Default for SharedCounter {
		fn default() -> Self {
			Self {
				value: Arc::new(Mutex::new(0)),
			}
		}
	}

	#[derive(Clone)]
	#[injectable]
	struct ViewSetA {
		#[inject]
		counter: Injected<SharedCounter>,
	}

	#[derive(Clone)]
	#[injectable]
	struct ViewSetB {
		#[inject]
		counter: Injected<SharedCounter>,
	}

	#[async_trait]
	impl ViewSet for ViewSetA {
		fn get_basename(&self) -> &str {
			"a"
		}

		async fn dispatch(&self, _request: Request, _action: Action) -> Result<Response> {
			let mut count = self.counter.value.lock().unwrap();
			*count += 1;
			Ok(Response::ok().with_body(format!("A: {}", *count)))
		}
	}

	#[async_trait]
	impl ViewSet for ViewSetB {
		fn get_basename(&self) -> &str {
			"b"
		}

		async fn dispatch(&self, _request: Request, _action: Action) -> Result<Response> {
			let count = self.counter.value.lock().unwrap();
			Ok(Response::ok().with_body(format!("B: {}", *count)))
		}
	}

	let singleton = Arc::new(SingletonScope::new());
	let ctx = InjectionContext::builder(singleton).build();

	let viewset_a = ViewSetA::inject(&ctx).await.unwrap();
	let viewset_b = ViewSetB::inject(&ctx).await.unwrap();

	let request_a = Request::builder()
		.method(Method::GET)
		.uri("/a/")
		.version(Version::HTTP_11)
		.headers(HeaderMap::new())
		.body(Bytes::new())
		.build()
		.unwrap();
	let request_b = Request::builder()
		.method(Method::GET)
		.uri("/b/")
		.version(Version::HTTP_11)
		.headers(HeaderMap::new())
		.body(Bytes::new())
		.build()
		.unwrap();

	let response_a = viewset_a.dispatch(request_a, Action::list()).await.unwrap();
	let response_b = viewset_b.dispatch(request_b, Action::list()).await.unwrap();

	let body_a = String::from_utf8(response_a.body.to_vec()).unwrap();
	let body_b = String::from_utf8(response_b.body.to_vec()).unwrap();

	assert_eq!(body_a, "A: 1");
	assert_eq!(body_b, "B: 1"); // Same shared counter
}

#[tokio::test]
async fn test_viewset_composition() {
	#[derive(Clone)]
	#[injectable]
	struct ValidationService {
		#[no_inject]
		enabled: bool,
	}

	impl Default for ValidationService {
		fn default() -> Self {
			Self { enabled: true }
		}
	}

	#[derive(Clone)]
	#[injectable]
	struct StorageService {
		#[no_inject]
		items: Arc<std::sync::Mutex<Vec<String>>>,
	}

	impl Default for StorageService {
		fn default() -> Self {
			Self {
				items: Arc::new(std::sync::Mutex::new(vec![])),
			}
		}
	}

	#[derive(Clone)]
	#[injectable]
	struct ComposedViewSet {
		#[inject]
		validator: Injected<ValidationService>,
		#[inject]
		storage: Injected<StorageService>,
	}

	#[async_trait]
	impl ViewSet for ComposedViewSet {
		fn get_basename(&self) -> &str {
			"composed"
		}

		async fn dispatch(&self, request: Request, action: Action) -> Result<Response> {
			if !self.validator.enabled {
				return Ok(Response::new(StatusCode::BAD_REQUEST).with_body("Validation disabled"));
			}

			match action.action_type {
				ActionType::Create => {
					let data = String::from_utf8_lossy(request.body()).to_string();
					self.storage.items.lock().unwrap().push(data.clone());
					Ok(Response::ok().with_body(format!("Created: {}", data)))
				}
				ActionType::List => {
					let items = self.storage.items.lock().unwrap();
					let count = items.len();
					Ok(Response::ok().with_body(format!("Items: {}", count)))
				}
				_ => Ok(Response::ok().with_body("OK")),
			}
		}
	}

	let singleton = Arc::new(SingletonScope::new());
	let ctx = InjectionContext::builder(singleton).build();

	let viewset = ComposedViewSet::inject(&ctx).await.unwrap();

	// Create item
	let request1 = Request::builder()
		.method(Method::POST)
		.uri("/items/")
		.version(Version::HTTP_11)
		.headers(HeaderMap::new())
		.body(Bytes::from("item1"))
		.build()
		.unwrap();

	let response1 = viewset.dispatch(request1, Action::create()).await.unwrap();
	let body1 = String::from_utf8(response1.body.to_vec()).unwrap();
	assert_eq!(body1, "Created: item1");

	// List items
	let request2 = Request::builder()
		.method(Method::GET)
		.uri("/items/")
		.version(Version::HTTP_11)
		.headers(HeaderMap::new())
		.body(Bytes::new())
		.build()
		.unwrap();

	let response2 = viewset.dispatch(request2, Action::list()).await.unwrap();
	let body2 = String::from_utf8(response2.body.to_vec()).unwrap();
	assert_eq!(body2, "Items: 1");
}

#[tokio::test]
async fn test_viewset_dependency_chain() {
	#[derive(Clone)]
	#[injectable]
	struct Logger {
		#[no_inject]
		prefix: String,
	}

	impl Default for Logger {
		fn default() -> Self {
			Self {
				prefix: "[LOG]".to_string(),
			}
		}
	}

	#[derive(Clone)]
	#[injectable]
	struct Auditor {
		#[inject]
		logger: Injected<Logger>,
	}

	#[derive(Clone)]
	#[injectable]
	struct ChainedViewSet {
		#[inject]
		auditor: Injected<Auditor>,
	}

	#[async_trait]
	impl ViewSet for ChainedViewSet {
		fn get_basename(&self) -> &str {
			"chained"
		}

		async fn dispatch(&self, _request: Request, _action: Action) -> Result<Response> {
			let log_msg = format!("{} Request processed", self.auditor.logger.prefix);
			Ok(Response::ok().with_body(log_msg))
		}
	}

	let singleton = Arc::new(SingletonScope::new());
	let ctx = InjectionContext::builder(singleton).build();

	let viewset = ChainedViewSet::inject(&ctx).await.unwrap();

	let request = Request::builder()
		.method(Method::GET)
		.uri("/chain/")
		.version(Version::HTTP_11)
		.headers(HeaderMap::new())
		.body(Bytes::new())
		.build()
		.unwrap();

	let response = viewset.dispatch(request, Action::list()).await.unwrap();
	let body = String::from_utf8(response.body.to_vec()).unwrap();

	assert_eq!(body, "[LOG] Request processed");
}

// ============================================================================
// Category 3: Action Routing (3 tests)
// ============================================================================

#[tokio::test]
async fn test_action_based_routing() {
	#[derive(Clone)]
	#[injectable]
	struct ActionRouter {
		#[no_inject]
		routes: Arc<std::sync::Mutex<Vec<String>>>,
	}

	impl Default for ActionRouter {
		fn default() -> Self {
			Self {
				routes: Arc::new(std::sync::Mutex::new(vec![])),
			}
		}
	}

	#[derive(Clone)]
	#[injectable]
	struct RoutedViewSet {
		#[inject]
		router: Injected<ActionRouter>,
	}

	#[async_trait]
	impl ViewSet for RoutedViewSet {
		fn get_basename(&self) -> &str {
			"routed"
		}

		async fn dispatch(&self, _request: Request, action: Action) -> Result<Response> {
			let route = format!("Action: {:?}", action.action_type);
			self.router.routes.lock().unwrap().push(route.clone());
			Ok(Response::ok().with_body(route))
		}
	}

	let singleton = Arc::new(SingletonScope::new());
	let ctx = InjectionContext::builder(singleton).build();

	let viewset = RoutedViewSet::inject(&ctx).await.unwrap();

	let actions = vec![
		Action::list(),
		Action::retrieve(),
		Action::create(),
		Action::update(),
		Action::destroy(),
	];

	for action in actions {
		let request = Request::builder()
			.method(Method::GET)
			.uri("/route/")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = viewset.dispatch(request, action).await.unwrap();
		assert_eq!(response.status, hyper::StatusCode::OK);
	}

	let routes = viewset.router.routes.lock().unwrap();
	assert_eq!(routes.len(), 5);
}

#[tokio::test]
async fn test_custom_action_handling() {
	#[derive(Clone)]
	#[injectable]
	struct CustomActionHandler {
		#[no_inject]
		enabled: bool,
	}

	impl Default for CustomActionHandler {
		fn default() -> Self {
			Self { enabled: true }
		}
	}

	#[derive(Clone)]
	#[injectable]
	struct CustomActionViewSet {
		#[inject]
		handler: Injected<CustomActionHandler>,
	}

	#[async_trait]
	impl ViewSet for CustomActionViewSet {
		fn get_basename(&self) -> &str {
			"custom"
		}

		async fn dispatch(&self, _request: Request, action: Action) -> Result<Response> {
			if !self.handler.enabled {
				return Ok(Response::new(StatusCode::BAD_REQUEST).with_body("Handler disabled"));
			}

			match action.action_type {
				ActionType::Custom(name) => {
					Ok(Response::ok().with_body(format!("Custom: {}", name)))
				}
				_ => Ok(Response::ok().with_body("Standard action")),
			}
		}
	}

	let singleton = Arc::new(SingletonScope::new());
	let ctx = InjectionContext::builder(singleton).build();

	let viewset = CustomActionViewSet::inject(&ctx).await.unwrap();

	let request = Request::builder()
		.method(Method::POST)
		.uri("/custom/action/")
		.version(Version::HTTP_11)
		.headers(HeaderMap::new())
		.body(Bytes::new())
		.build()
		.unwrap();

	let custom_action = Action {
		action_type: ActionType::Custom("special".into()),
		detail: false,
	};

	let response = viewset.dispatch(request, custom_action).await.unwrap();
	let body = String::from_utf8(response.body.to_vec()).unwrap();

	assert_eq!(body, "Custom: special");
}

#[tokio::test]
async fn test_method_based_routing() {
	#[derive(Clone)]
	#[injectable]
	struct MethodRouter {
		#[no_inject]
		allowed_methods: Vec<String>,
	}

	impl Default for MethodRouter {
		fn default() -> Self {
			Self {
				allowed_methods: vec!["GET".to_string(), "POST".to_string()],
			}
		}
	}

	#[derive(Clone)]
	#[injectable]
	struct MethodViewSet {
		#[inject]
		router: Injected<MethodRouter>,
	}

	#[async_trait]
	impl ViewSet for MethodViewSet {
		fn get_basename(&self) -> &str {
			"method"
		}

		async fn dispatch(&self, request: Request, _action: Action) -> Result<Response> {
			let method = request.method.as_str();

			if self.router.allowed_methods.contains(&method.to_string()) {
				Ok(Response::ok().with_body(format!("Allowed: {}", method)))
			} else {
				Ok(Response::new(StatusCode::METHOD_NOT_ALLOWED).with_body("Method not allowed"))
			}
		}
	}

	let singleton = Arc::new(SingletonScope::new());
	let ctx = InjectionContext::builder(singleton).build();

	let viewset = MethodViewSet::inject(&ctx).await.unwrap();

	// Allowed method
	let request1 = Request::builder()
		.method(Method::GET)
		.uri("/method/")
		.version(Version::HTTP_11)
		.headers(HeaderMap::new())
		.body(Bytes::new())
		.build()
		.unwrap();

	let response1 = viewset.dispatch(request1, Action::list()).await.unwrap();
	let body1 = String::from_utf8(response1.body.to_vec()).unwrap();
	assert_eq!(body1, "Allowed: GET");

	// Disallowed method
	let request2 = Request::builder()
		.method(Method::DELETE)
		.uri("/method/")
		.version(Version::HTTP_11)
		.headers(HeaderMap::new())
		.body(Bytes::new())
		.build()
		.unwrap();

	let response2 = viewset.dispatch(request2, Action::destroy()).await.unwrap();
	let body2 = String::from_utf8(response2.body.to_vec()).unwrap();
	assert_eq!(body2, "Method not allowed");
}

// ============================================================================
// Category 4: State Management (3 tests)
// ============================================================================

#[tokio::test]
async fn test_request_scoped_state() {
	use std::sync::Mutex;

	#[derive(Clone)]
	#[injectable]
	struct RequestState {
		#[no_inject]
		request_id: Arc<Mutex<Option<String>>>,
	}

	impl Default for RequestState {
		fn default() -> Self {
			Self {
				request_id: Arc::new(Mutex::new(None)),
			}
		}
	}

	#[derive(Clone)]
	#[injectable]
	struct StateViewSet {
		#[inject]
		state: Injected<RequestState>,
	}

	#[async_trait]
	impl ViewSet for StateViewSet {
		fn get_basename(&self) -> &str {
			"state"
		}

		async fn dispatch(&self, request: Request, _action: Action) -> Result<Response> {
			// Extract request ID from headers or generate one
			let req_id = request
				.headers
				.get("x-request-id")
				.and_then(|v| v.to_str().ok())
				.map(|s| s.to_string())
				.unwrap_or_else(|| "default".to_string());

			*self.state.request_id.lock().unwrap() = Some(req_id.clone());

			Ok(Response::ok().with_body(format!("Request ID: {}", req_id)))
		}
	}

	let singleton = Arc::new(SingletonScope::new());
	let ctx = InjectionContext::builder(singleton).build();

	let viewset = StateViewSet::inject(&ctx).await.unwrap();

	let mut headers = HeaderMap::new();
	headers.insert("x-request-id", "req-123".parse().unwrap());

	let request = Request::builder()
		.method(Method::GET)
		.uri("/state/")
		.version(Version::HTTP_11)
		.headers(headers)
		.body(Bytes::new())
		.build()
		.unwrap();

	let response = viewset.dispatch(request, Action::list()).await.unwrap();
	let body = String::from_utf8(response.body.to_vec()).unwrap();

	assert_eq!(body, "Request ID: req-123");

	// Verify state was stored
	let stored_id = viewset.state.request_id.lock().unwrap();
	assert_eq!(*stored_id, Some("req-123".to_string()));
}

#[tokio::test]
async fn test_singleton_state_persistence() {
	use std::sync::Mutex;

	#[derive(Clone)]
	#[injectable]
	struct SingletonCounter {
		#[no_inject]
		count: Arc<Mutex<i32>>,
	}

	impl Default for SingletonCounter {
		fn default() -> Self {
			Self {
				count: Arc::new(Mutex::new(0)),
			}
		}
	}

	#[derive(Clone)]
	#[injectable]
	struct CounterViewSet {
		#[inject]
		counter: Injected<SingletonCounter>,
	}

	#[async_trait]
	impl ViewSet for CounterViewSet {
		fn get_basename(&self) -> &str {
			"counter"
		}

		async fn dispatch(&self, _request: Request, action: Action) -> Result<Response> {
			match action.action_type {
				ActionType::Create => {
					let mut count = self.counter.count.lock().unwrap();
					*count += 1;
					Ok(Response::ok().with_body(format!("Count: {}", *count)))
				}
				ActionType::List => {
					let count = self.counter.count.lock().unwrap();
					Ok(Response::ok().with_body(format!("Count: {}", *count)))
				}
				_ => Ok(Response::ok().with_body("OK")),
			}
		}
	}

	let singleton = Arc::new(SingletonScope::new());
	let ctx = InjectionContext::builder(singleton).build();

	let viewset = CounterViewSet::inject(&ctx).await.unwrap();

	// Increment counter
	for i in 1..=3 {
		let request = Request::builder()
			.method(Method::POST)
			.uri("/counter/")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = viewset.dispatch(request, Action::create()).await.unwrap();
		let body = String::from_utf8(response.body.to_vec()).unwrap();
		assert_eq!(body, format!("Count: {}", i));
	}

	// Read counter
	let request = Request::builder()
		.method(Method::GET)
		.uri("/counter/")
		.version(Version::HTTP_11)
		.headers(HeaderMap::new())
		.body(Bytes::new())
		.build()
		.unwrap();

	let response = viewset.dispatch(request, Action::list()).await.unwrap();
	let body = String::from_utf8(response.body.to_vec()).unwrap();
	assert_eq!(body, "Count: 3");
}

#[tokio::test]
async fn test_session_state_isolation() {
	use std::sync::Mutex;

	#[derive(Clone)]
	#[injectable]
	struct SessionData {
		#[no_inject]
		data: Arc<Mutex<Vec<String>>>,
	}

	impl Default for SessionData {
		fn default() -> Self {
			Self {
				data: Arc::new(Mutex::new(vec![])),
			}
		}
	}

	#[derive(Clone)]
	#[injectable]
	struct SessionViewSet {
		#[inject]
		session: Injected<SessionData>,
	}

	#[async_trait]
	impl ViewSet for SessionViewSet {
		fn get_basename(&self) -> &str {
			"session"
		}

		async fn dispatch(&self, request: Request, _action: Action) -> Result<Response> {
			let value = String::from_utf8_lossy(request.body()).to_string();
			self.session.data.lock().unwrap().push(value.clone());

			let count = self.session.data.lock().unwrap().len();
			Ok(Response::ok().with_body(format!("Session items: {}", count)))
		}
	}

	let singleton1 = Arc::new(SingletonScope::new());
	let ctx1 = InjectionContext::builder(singleton1).build();

	let singleton2 = Arc::new(SingletonScope::new());
	let ctx2 = InjectionContext::builder(singleton2).build();

	let viewset1 = SessionViewSet::inject(&ctx1).await.unwrap();
	let viewset2 = SessionViewSet::inject(&ctx2).await.unwrap();

	// Add data to first session
	let request1 = Request::builder()
		.method(Method::POST)
		.uri("/session/")
		.version(Version::HTTP_11)
		.headers(HeaderMap::new())
		.body(Bytes::from("data1"))
		.build()
		.unwrap();

	let response1 = viewset1.dispatch(request1, Action::create()).await.unwrap();
	let body1 = String::from_utf8(response1.body.to_vec()).unwrap();
	assert_eq!(body1, "Session items: 1");

	// Check second session shares the same data (cached singleton)
	let request2 = Request::builder()
		.method(Method::GET)
		.uri("/session/")
		.version(Version::HTTP_11)
		.headers(HeaderMap::new())
		.body(Bytes::new())
		.build()
		.unwrap();

	let _response2 = viewset2.dispatch(request2, Action::list()).await.unwrap();

	// Second viewset shares the same SessionData instance due to caching
	let count2 = viewset2.session.data.lock().unwrap().len();
	assert_eq!(count2, 1);
}

// ============================================================================
// Category 5: Error Handling and Edge Cases (3 tests)
// ============================================================================

#[tokio::test]
async fn test_graceful_service_failure() {
	#[derive(Clone, Default)]
	#[injectable]
	struct FallibleService {
		#[no_inject]
		fail: bool,
	}

	#[derive(Clone)]
	#[injectable]
	struct ResilientViewSet {
		#[inject]
		service: Injected<FallibleService>,
	}

	#[async_trait]
	impl ViewSet for ResilientViewSet {
		fn get_basename(&self) -> &str {
			"resilient"
		}

		async fn dispatch(&self, _request: Request, _action: Action) -> Result<Response> {
			if self.service.fail {
				Ok(Response::new(StatusCode::INTERNAL_SERVER_ERROR)
					.with_body("Service unavailable"))
			} else {
				Ok(Response::ok().with_body("Service available"))
			}
		}
	}

	let singleton = Arc::new(SingletonScope::new());
	let ctx = InjectionContext::builder(singleton).build();

	let viewset = ResilientViewSet::inject(&ctx).await.unwrap();

	let request = Request::builder()
		.method(Method::GET)
		.uri("/resilient/")
		.version(Version::HTTP_11)
		.headers(HeaderMap::new())
		.body(Bytes::new())
		.build()
		.unwrap();

	let response = viewset.dispatch(request, Action::list()).await.unwrap();
	let body = String::from_utf8(response.body.to_vec()).unwrap();

	assert_eq!(body, "Service available");
}

#[tokio::test]
async fn test_empty_request_handling() {
	#[derive(Clone)]
	#[injectable]
	struct EmptyRequestHandler;

	impl Default for EmptyRequestHandler {
		fn default() -> Self {
			Self
		}
	}

	#[derive(Clone)]
	#[injectable]
	#[allow(dead_code)]
	struct EmptyViewSet {
		#[inject]
		handler: Injected<EmptyRequestHandler>,
	}

	#[async_trait]
	impl ViewSet for EmptyViewSet {
		fn get_basename(&self) -> &str {
			"empty"
		}

		async fn dispatch(&self, request: Request, _action: Action) -> Result<Response> {
			if request.body().is_empty() {
				Ok(Response::ok().with_body("Empty body accepted"))
			} else {
				Ok(Response::ok().with_body("Body present"))
			}
		}
	}

	let singleton = Arc::new(SingletonScope::new());
	let ctx = InjectionContext::builder(singleton).build();

	let viewset = EmptyViewSet::inject(&ctx).await.unwrap();

	let request = Request::builder()
		.method(Method::POST)
		.uri("/empty/")
		.version(Version::HTTP_11)
		.headers(HeaderMap::new())
		.body(Bytes::new())
		.build()
		.unwrap();

	let response = viewset.dispatch(request, Action::create()).await.unwrap();
	let body = String::from_utf8(response.body.to_vec()).unwrap();

	assert_eq!(body, "Empty body accepted");
}

#[tokio::test]
async fn test_large_payload_handling() {
	#[derive(Clone)]
	#[injectable]
	struct PayloadLimiter {
		#[no_inject]
		max_size: usize,
	}

	impl Default for PayloadLimiter {
		fn default() -> Self {
			Self { max_size: 1024 }
		}
	}

	#[derive(Clone)]
	#[injectable]
	struct LimitedViewSet {
		#[inject]
		limiter: Injected<PayloadLimiter>,
	}

	#[async_trait]
	impl ViewSet for LimitedViewSet {
		fn get_basename(&self) -> &str {
			"limited"
		}

		async fn dispatch(&self, request: Request, _action: Action) -> Result<Response> {
			if request.body().len() > self.limiter.max_size {
				Ok(Response::new(StatusCode::BAD_REQUEST).with_body("Payload too large"))
			} else {
				Ok(Response::ok().with_body(format!("Accepted {} bytes", request.body().len())))
			}
		}
	}

	let singleton = Arc::new(SingletonScope::new());
	let ctx = InjectionContext::builder(singleton).build();

	let viewset = LimitedViewSet::inject(&ctx).await.unwrap();

	// Small payload
	let small_data = vec![0u8; 512];
	let request1 = Request::builder()
		.method(Method::POST)
		.uri("/limited/")
		.version(Version::HTTP_11)
		.headers(HeaderMap::new())
		.body(Bytes::from(small_data))
		.build()
		.unwrap();

	let response1 = viewset.dispatch(request1, Action::create()).await.unwrap();
	let body1 = String::from_utf8(response1.body.to_vec()).unwrap();
	assert_eq!(body1, "Accepted 512 bytes");

	// Large payload
	let large_data = vec![0u8; 2048];
	let request2 = Request::builder()
		.method(Method::POST)
		.uri("/limited/")
		.version(Version::HTTP_11)
		.headers(HeaderMap::new())
		.body(Bytes::from(large_data))
		.build()
		.unwrap();

	let response2 = viewset.dispatch(request2, Action::create()).await.unwrap();
	let body2 = String::from_utf8(response2.body.to_vec()).unwrap();
	assert_eq!(body2, "Payload too large");
}
