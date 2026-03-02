//! Tracing middleware
//!
//! Provides distributed tracing support for request/response cycles.
//! Compatible with OpenTelemetry and similar tracing systems.

use async_trait::async_trait;
use hyper::header::HeaderName;
use reinhardt_http::{Handler, Middleware, Request, Response, Result};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::Instant;

/// Trace span information
#[derive(Debug, Clone)]
pub struct Span {
	/// Span ID
	pub span_id: String,
	/// Parent span ID (if any)
	pub parent_span_id: Option<String>,
	/// Trace ID
	pub trace_id: String,
	/// Operation name
	pub operation_name: String,
	/// Start time
	pub start_time: Instant,
	/// End time
	pub end_time: Option<Instant>,
	/// Tags/attributes
	pub tags: HashMap<String, String>,
	/// Status
	pub status: SpanStatus,
}

/// Span status
#[derive(Debug, Clone, PartialEq)]
pub enum SpanStatus {
	/// Span is still active
	Active,
	/// Span completed successfully
	Ok,
	/// Span completed with error
	Error,
}

impl Span {
	/// Create a new span
	pub fn new(trace_id: String, operation_name: String) -> Self {
		Self {
			span_id: uuid::Uuid::new_v4().to_string(),
			parent_span_id: None,
			trace_id,
			operation_name,
			start_time: Instant::now(),
			end_time: None,
			tags: HashMap::new(),
			status: SpanStatus::Active,
		}
	}

	/// Set parent span
	pub fn with_parent(mut self, parent_span_id: String) -> Self {
		self.parent_span_id = Some(parent_span_id);
		self
	}

	/// Add a tag
	pub fn add_tag(&mut self, key: String, value: String) {
		self.tags.insert(key, value);
	}

	/// End the span
	pub fn end(&mut self) {
		self.end_time = Some(Instant::now());
		// If still active, mark as Ok
		if self.status == SpanStatus::Active {
			self.status = SpanStatus::Ok;
		}
	}

	/// Mark as error
	pub fn mark_error(&mut self) {
		self.status = SpanStatus::Error;
	}

	/// Get duration in milliseconds
	pub fn duration_ms(&self) -> Option<f64> {
		self.end_time
			.map(|end| (end - self.start_time).as_secs_f64() * 1000.0)
	}
}

/// Trace context storage
#[derive(Debug, Default)]
pub struct TraceStore {
	/// Active spans
	spans: RwLock<HashMap<String, Span>>,
}

impl TraceStore {
	/// Create a new trace store
	pub fn new() -> Self {
		Self::default()
	}

	/// Start a new span
	pub fn start_span(&self, trace_id: String, operation_name: String) -> String {
		let span = Span::new(trace_id, operation_name);
		let span_id = span.span_id.clone();
		self.spans
			.write()
			.unwrap_or_else(|e| e.into_inner())
			.insert(span_id.clone(), span);
		span_id
	}

	/// End a span
	pub fn end_span(&self, span_id: &str) {
		if let Some(span) = self
			.spans
			.write()
			.unwrap_or_else(|e| e.into_inner())
			.get_mut(span_id)
		{
			span.end();
		}
	}

	/// Mark span as error
	pub fn mark_span_error(&self, span_id: &str) {
		if let Some(span) = self
			.spans
			.write()
			.unwrap_or_else(|e| e.into_inner())
			.get_mut(span_id)
		{
			span.mark_error();
		}
	}

	/// Add tag to span
	pub fn add_span_tag(&self, span_id: &str, key: String, value: String) {
		if let Some(span) = self
			.spans
			.write()
			.unwrap_or_else(|e| e.into_inner())
			.get_mut(span_id)
		{
			span.add_tag(key, value);
		}
	}

	/// Get span
	pub fn get_span(&self, span_id: &str) -> Option<Span> {
		self.spans
			.read()
			.unwrap_or_else(|e| e.into_inner())
			.get(span_id)
			.cloned()
	}

	/// Get all completed spans
	pub fn completed_spans(&self) -> Vec<Span> {
		self.spans
			.read()
			.unwrap()
			.values()
			.filter(|s| s.end_time.is_some())
			.cloned()
			.collect()
	}

	/// Clear completed spans
	pub fn clear_completed(&self) {
		self.spans
			.write()
			.unwrap()
			.retain(|_, span| span.end_time.is_none());
	}
}

/// Tracing header names
pub const TRACE_ID_HEADER: &str = "X-Trace-ID";
pub const SPAN_ID_HEADER: &str = "X-Span-ID";
pub const PARENT_SPAN_ID_HEADER: &str = "X-Parent-Span-ID";

/// Configuration for tracing middleware
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct TracingConfig {
	/// Enable tracing
	pub enabled: bool,
	/// Sample rate (0.0 to 1.0)
	pub sample_rate: f64,
	/// Custom trace ID header name
	pub trace_id_header: String,
	/// Custom span ID header name
	pub span_id_header: String,
	/// Paths to exclude from tracing
	pub exclude_paths: Vec<String>,
}

impl TracingConfig {
	/// Create a new default configuration
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_middleware::TracingConfig;
	///
	/// let config = TracingConfig::new();
	/// assert!(config.enabled);
	/// assert_eq!(config.sample_rate, 1.0);
	/// ```
	pub fn new() -> Self {
		Self {
			enabled: true,
			sample_rate: 1.0,
			trace_id_header: TRACE_ID_HEADER.to_string(),
			span_id_header: SPAN_ID_HEADER.to_string(),
			exclude_paths: vec!["/health".to_string(), "/metrics".to_string()],
		}
	}

	/// Set sample rate
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_middleware::TracingConfig;
	///
	/// let config = TracingConfig::new().with_sample_rate(0.1);
	/// assert_eq!(config.sample_rate, 0.1);
	/// ```
	pub fn with_sample_rate(mut self, rate: f64) -> Self {
		self.sample_rate = rate.clamp(0.0, 1.0);
		self
	}

	/// Disable tracing
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_middleware::TracingConfig;
	///
	/// let config = TracingConfig::new().disabled();
	/// assert!(!config.enabled);
	/// ```
	pub fn disabled(mut self) -> Self {
		self.enabled = false;
		self
	}

	/// Add paths to exclude
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_middleware::TracingConfig;
	///
	/// let config = TracingConfig::new()
	///     .with_excluded_paths(vec!["/admin".to_string()]);
	/// ```
	pub fn with_excluded_paths(mut self, paths: Vec<String>) -> Self {
		self.exclude_paths.extend(paths);
		self
	}
}

impl Default for TracingConfig {
	fn default() -> Self {
		Self::new()
	}
}

/// Middleware for distributed tracing
///
/// # Examples
///
/// ```
/// use std::sync::Arc;
/// use reinhardt_middleware::{TracingMiddleware, TracingConfig};
/// use reinhardt_http::{Handler, Middleware, Request, Response};
/// use hyper::{StatusCode, Method, Version, HeaderMap};
/// use bytes::Bytes;
///
/// struct TestHandler;
///
/// #[async_trait::async_trait]
/// impl Handler for TestHandler {
///     async fn handle(&self, _request: Request) -> reinhardt_core::exception::Result<Response> {
///         Ok(Response::new(StatusCode::OK).with_body(Bytes::from("OK")))
///     }
/// }
///
/// # tokio_test::block_on(async {
/// let config = TracingConfig::new();
/// let middleware = TracingMiddleware::new(config);
/// let handler = Arc::new(TestHandler);
///
/// let request = Request::builder()
///     .method(Method::GET)
///     .uri("/test")
///     .version(Version::HTTP_11)
///     .headers(HeaderMap::new())
///     .body(Bytes::new())
///     .build()
///     .unwrap();
///
/// let response = middleware.process(request, handler).await.unwrap();
/// assert!(response.headers.contains_key("X-Trace-ID"));
/// assert!(response.headers.contains_key("X-Span-ID"));
/// # });
/// ```
pub struct TracingMiddleware {
	config: TracingConfig,
	store: Arc<TraceStore>,
}

impl TracingMiddleware {
	/// Create a new TracingMiddleware with the given configuration
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_middleware::{TracingMiddleware, TracingConfig};
	///
	/// let config = TracingConfig::new();
	/// let middleware = TracingMiddleware::new(config);
	/// ```
	pub fn new(config: TracingConfig) -> Self {
		Self {
			config,
			store: Arc::new(TraceStore::new()),
		}
	}

	/// Create a new TracingMiddleware with default configuration
	pub fn with_defaults() -> Self {
		Self::new(TracingConfig::default())
	}

	/// Create from an existing Arc-wrapped trace store
	///
	/// This is provided for cases where you already have an `Arc<TraceStore>`.
	/// In most cases, you should use `new()` instead, which creates the store internally.
	pub fn from_arc(config: TracingConfig, store: Arc<TraceStore>) -> Self {
		Self { config, store }
	}

	/// Get a reference to the trace store
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_middleware::{TracingMiddleware, TracingConfig};
	///
	/// let middleware = TracingMiddleware::new(TracingConfig::new());
	///
	/// // Access the store
	/// let store = middleware.store();
	/// let completed = store.completed_spans();
	/// ```
	pub fn store(&self) -> &TraceStore {
		&self.store
	}

	/// Get a cloned Arc of the store (for cases where you need ownership)
	///
	/// In most cases, you should use `store()` instead to get a reference.
	pub fn store_arc(&self) -> Arc<TraceStore> {
		Arc::clone(&self.store)
	}

	/// Check if path should be excluded
	fn should_exclude(&self, path: &str) -> bool {
		self.config
			.exclude_paths
			.iter()
			.any(|p| path.starts_with(p))
	}

	/// Check if request should be sampled
	fn should_sample(&self) -> bool {
		if self.config.sample_rate >= 1.0 {
			return true;
		}
		if self.config.sample_rate <= 0.0 {
			return false;
		}
		use std::collections::hash_map::RandomState;
		use std::hash::BuildHasher;
		let random_state = RandomState::new();

		let hash = random_state.hash_one(Instant::now());
		(hash as f64 / u64::MAX as f64) < self.config.sample_rate
	}

	/// Get or generate trace ID
	fn get_or_generate_trace_id(&self, request: &Request) -> String {
		request
			.headers
			.get(&self.config.trace_id_header)
			.and_then(|v| v.to_str().ok())
			.map(|s| s.to_string())
			.unwrap_or_else(|| uuid::Uuid::new_v4().to_string())
	}
}

impl Default for TracingMiddleware {
	fn default() -> Self {
		Self::with_defaults()
	}
}

#[async_trait]
impl Middleware for TracingMiddleware {
	async fn process(&self, request: Request, handler: Arc<dyn Handler>) -> Result<Response> {
		// Skip if disabled or excluded path
		let path = request.uri.path();
		if !self.config.enabled || self.should_exclude(path) {
			return handler.handle(request).await;
		}

		// Skip if not sampled
		if !self.should_sample() {
			return handler.handle(request).await;
		}

		// Get or generate trace ID
		let trace_id = self.get_or_generate_trace_id(&request);

		// Start span
		let operation_name = format!("{} {}", request.method.as_str(), path);
		let span_id = self.store.start_span(trace_id.clone(), operation_name);

		// Add request metadata to span
		self.store.add_span_tag(
			&span_id,
			"http.method".to_string(),
			request.method.as_str().to_string(),
		);
		self.store
			.add_span_tag(&span_id, "http.path".to_string(), path.to_string());

		// Call handler
		let result = handler.handle(request).await;

		// End span
		match &result {
			Ok(response) => {
				self.store.add_span_tag(
					&span_id,
					"http.status_code".to_string(),
					response.status.as_u16().to_string(),
				);
				if !response.status.is_success() {
					self.store.mark_span_error(&span_id);
				}
			}
			Err(_) => {
				self.store.mark_span_error(&span_id);
			}
		}
		self.store.end_span(&span_id);

		// Add trace headers to response
		let mut response = result?;
		let trace_header: HeaderName = self.config.trace_id_header.parse().unwrap();
		response
			.headers
			.insert(trace_header, trace_id.parse().unwrap());

		let span_header: HeaderName = self.config.span_id_header.parse().unwrap();
		response
			.headers
			.insert(span_header, span_id.parse().unwrap());

		Ok(response)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use bytes::Bytes;
	use hyper::{HeaderMap, Method, StatusCode, Version};

	struct TestHandler {
		status: StatusCode,
	}

	impl TestHandler {
		fn new(status: StatusCode) -> Self {
			Self { status }
		}
	}

	#[async_trait]
	impl Handler for TestHandler {
		async fn handle(&self, _request: Request) -> Result<Response> {
			Ok(Response::new(self.status).with_body(Bytes::from("OK")))
		}
	}

	#[tokio::test]
	async fn test_basic_tracing() {
		let config = TracingConfig::new();
		let middleware = TracingMiddleware::new(config);
		let handler = Arc::new(TestHandler::new(StatusCode::OK));

		let request = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();

		// Should have trace headers
		assert!(response.headers.contains_key(TRACE_ID_HEADER));
		assert!(response.headers.contains_key(SPAN_ID_HEADER));

		// Should have recorded span
		let spans = middleware.store.completed_spans();
		assert_eq!(spans.len(), 1);
		assert_eq!(spans[0].status, SpanStatus::Ok);
	}

	#[tokio::test]
	async fn test_propagate_trace_id() {
		let config = TracingConfig::new();
		let middleware = TracingMiddleware::new(config);
		let handler = Arc::new(TestHandler::new(StatusCode::OK));

		let existing_trace_id = "existing-trace-123";
		let mut headers = HeaderMap::new();
		headers.insert(TRACE_ID_HEADER, existing_trace_id.parse().unwrap());

		let request = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(headers)
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();

		// Should propagate existing trace ID
		assert_eq!(
			response.headers.get(TRACE_ID_HEADER).unwrap(),
			existing_trace_id
		);
	}

	#[tokio::test]
	async fn test_error_status() {
		let config = TracingConfig::new();
		let middleware = TracingMiddleware::new(config);
		let handler = Arc::new(TestHandler::new(StatusCode::INTERNAL_SERVER_ERROR));

		let request = Request::builder()
			.method(Method::GET)
			.uri("/error")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		let _response = middleware.process(request, handler).await.unwrap();

		// Span should be marked as error
		let spans = middleware.store.completed_spans();
		assert_eq!(spans.len(), 1);
		assert_eq!(spans[0].status, SpanStatus::Error);
	}

	#[tokio::test]
	async fn test_exclude_paths() {
		let config = TracingConfig::new();
		let middleware = TracingMiddleware::new(config);
		let handler = Arc::new(TestHandler::new(StatusCode::OK));

		let request = Request::builder()
			.method(Method::GET)
			.uri("/health")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();

		// Should not have trace headers for excluded path
		assert!(!response.headers.contains_key(TRACE_ID_HEADER));
		assert_eq!(middleware.store.completed_spans().len(), 0);
	}

	#[tokio::test]
	async fn test_disabled_tracing() {
		let config = TracingConfig::new().disabled();
		let middleware = TracingMiddleware::new(config);
		let handler = Arc::new(TestHandler::new(StatusCode::OK));

		let request = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();

		// Should not have trace headers when disabled
		assert!(!response.headers.contains_key(TRACE_ID_HEADER));
		assert_eq!(middleware.store.completed_spans().len(), 0);
	}

	#[tokio::test]
	async fn test_span_metadata() {
		let config = TracingConfig::new();
		let middleware = TracingMiddleware::new(config);
		let handler = Arc::new(TestHandler::new(StatusCode::OK));

		let request = Request::builder()
			.method(Method::POST)
			.uri("/api/users")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		let _response = middleware.process(request, handler).await.unwrap();

		let spans = middleware.store.completed_spans();
		assert_eq!(spans.len(), 1);

		let span = &spans[0];
		assert_eq!(span.operation_name, "POST /api/users");
		assert_eq!(span.tags.get("http.method").unwrap(), "POST");
		assert_eq!(span.tags.get("http.path").unwrap(), "/api/users");
		assert_eq!(span.tags.get("http.status_code").unwrap(), "200");
	}

	#[tokio::test]
	async fn test_span_duration() {
		let config = TracingConfig::new();
		let middleware = TracingMiddleware::new(config);
		let handler = Arc::new(TestHandler::new(StatusCode::OK));

		let request = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		let _response = middleware.process(request, handler).await.unwrap();

		let spans = middleware.store.completed_spans();
		let span = &spans[0];

		// Span should have a duration
		assert!(span.duration_ms().is_some());
		assert!(span.duration_ms().unwrap() >= 0.0);
	}

	#[tokio::test]
	async fn test_clear_completed_spans() {
		let config = TracingConfig::new();
		let middleware = Arc::new(TracingMiddleware::new(config));
		let handler = Arc::new(TestHandler::new(StatusCode::OK));

		// Generate some spans
		for _ in 0..5 {
			let request = Request::builder()
				.method(Method::GET)
				.uri("/test")
				.version(Version::HTTP_11)
				.headers(HeaderMap::new())
				.body(Bytes::new())
				.build()
				.unwrap();
			let _response = middleware.process(request, handler.clone()).await.unwrap();
		}

		assert_eq!(middleware.store.completed_spans().len(), 5);

		// Clear completed spans
		middleware.store.clear_completed();

		assert_eq!(middleware.store.completed_spans().len(), 0);
	}

	#[tokio::test]
	async fn test_sample_rate_zero() {
		let config = TracingConfig::new().with_sample_rate(0.0);
		let middleware = TracingMiddleware::new(config);
		let handler = Arc::new(TestHandler::new(StatusCode::OK));

		let request = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();

		// Should not trace with 0% sample rate
		assert!(!response.headers.contains_key(TRACE_ID_HEADER));
	}

	#[tokio::test]
	async fn test_sample_rate_one() {
		let config = TracingConfig::new().with_sample_rate(1.0);
		let middleware = TracingMiddleware::new(config);
		let handler = Arc::new(TestHandler::new(StatusCode::OK));

		let request = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();

		// Should always trace with 100% sample rate
		assert!(response.headers.contains_key(TRACE_ID_HEADER));
	}

	#[tokio::test]
	async fn test_default_middleware() {
		let middleware = TracingMiddleware::default();
		let handler = Arc::new(TestHandler::new(StatusCode::OK));

		let request = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();

		assert!(response.headers.contains_key(TRACE_ID_HEADER));
	}
}
