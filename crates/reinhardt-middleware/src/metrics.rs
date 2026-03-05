//! Metrics middleware
//!
//! Collects and exposes application metrics in Prometheus format.
//! Tracks request counts, response times, and status codes.

use async_trait::async_trait;
use hyper::StatusCode;
use reinhardt_http::{Handler, Middleware, Request, Response, Result};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::Instant;

/// Metrics storage
#[derive(Debug, Default)]
pub struct MetricsStore {
	/// Total request count by method and path
	request_count: RwLock<HashMap<String, u64>>,
	/// Response time histogram buckets (in milliseconds)
	response_time_buckets: RwLock<HashMap<String, Vec<f64>>>,
	/// Status code counts
	status_codes: RwLock<HashMap<u16, u64>>,
	/// Custom metrics
	custom_metrics: RwLock<HashMap<String, f64>>,
}

impl MetricsStore {
	/// Create a new metrics store
	pub fn new() -> Self {
		Self::default()
	}

	/// Record a request
	pub fn record_request(&self, method: &str, path: &str) {
		let key = format!("{}:{}", method, path);
		let mut counts = self
			.request_count
			.write()
			.unwrap_or_else(|e| e.into_inner());
		*counts.entry(key).or_insert(0) += 1;
	}

	/// Record response time
	pub fn record_response_time(&self, method: &str, path: &str, duration_ms: f64) {
		let key = format!("{}:{}", method, path);
		let mut buckets = self
			.response_time_buckets
			.write()
			.unwrap_or_else(|e| e.into_inner());
		buckets.entry(key).or_default().push(duration_ms);
	}

	/// Record status code
	pub fn record_status_code(&self, status: u16) {
		let mut codes = self.status_codes.write().unwrap_or_else(|e| e.into_inner());
		*codes.entry(status).or_insert(0) += 1;
	}

	/// Set a custom metric
	pub fn set_custom_metric(&self, name: String, value: f64) {
		let mut metrics = self
			.custom_metrics
			.write()
			.unwrap_or_else(|e| e.into_inner());
		metrics.insert(name, value);
	}

	/// Get all metrics in Prometheus text format
	pub fn export_prometheus(&self) -> String {
		let mut output = String::new();

		// Request counts
		output.push_str("# HELP http_requests_total Total number of HTTP requests\n");
		output.push_str("# TYPE http_requests_total counter\n");
		let counts = self.request_count.read().unwrap_or_else(|e| e.into_inner());
		for (key, count) in counts.iter() {
			let parts: Vec<&str> = key.split(':').collect();
			if parts.len() == 2 {
				output.push_str(&format!(
					"http_requests_total{{method=\"{}\",path=\"{}\"}} {}\n",
					parts[0], parts[1], count
				));
			}
		}

		// Response time summary
		output.push_str("\n# HELP http_response_time_seconds HTTP response time in seconds\n");
		output.push_str("# TYPE http_response_time_seconds summary\n");
		let buckets = self
			.response_time_buckets
			.read()
			.unwrap_or_else(|e| e.into_inner());
		for (key, times) in buckets.iter() {
			let parts: Vec<&str> = key.split(':').collect();
			if parts.len() == 2 && !times.is_empty() {
				let sum: f64 = times.iter().sum();
				let count = times.len();
				let _avg = sum / count as f64;

				output.push_str(&format!(
					"http_response_time_seconds_sum{{method=\"{}\",path=\"{}\"}} {:.6}\n",
					parts[0],
					parts[1],
					sum / 1000.0 // Convert ms to seconds
				));
				output.push_str(&format!(
					"http_response_time_seconds_count{{method=\"{}\",path=\"{}\"}} {}\n",
					parts[0], parts[1], count
				));

				// Calculate percentiles
				let mut sorted_times = times.clone();
				sorted_times.sort_by(|a, b| a.partial_cmp(b).unwrap());
				let p50 = sorted_times[count / 2];
				let p95 = sorted_times[(count * 95) / 100];
				let p99 = sorted_times[(count * 99) / 100];

				output.push_str(&format!(
					"http_response_time_seconds{{method=\"{}\",path=\"{}\",quantile=\"0.5\"}} {:.6}\n",
					parts[0],
					parts[1],
					p50 / 1000.0
				));
				output.push_str(&format!(
					"http_response_time_seconds{{method=\"{}\",path=\"{}\",quantile=\"0.95\"}} {:.6}\n",
					parts[0],
					parts[1],
					p95 / 1000.0
				));
				output.push_str(&format!(
					"http_response_time_seconds{{method=\"{}\",path=\"{}\",quantile=\"0.99\"}} {:.6}\n",
					parts[0],
					parts[1],
					p99 / 1000.0
				));
			}
		}

		// Status codes
		output.push_str(
			"\n# HELP http_responses_total Total number of HTTP responses by status code\n",
		);
		output.push_str("# TYPE http_responses_total counter\n");
		let codes = self.status_codes.read().unwrap_or_else(|e| e.into_inner());
		for (code, count) in codes.iter() {
			output.push_str(&format!(
				"http_responses_total{{status=\"{}\"}} {}\n",
				code, count
			));
		}

		// Custom metrics
		let custom = self
			.custom_metrics
			.read()
			.unwrap_or_else(|e| e.into_inner());
		if !custom.is_empty() {
			output.push_str("\n# Custom metrics\n");
			for (name, value) in custom.iter() {
				output.push_str(&format!("{} {}\n", name, value));
			}
		}

		output
	}

	/// Reset all metrics
	pub fn reset(&self) {
		self.request_count
			.write()
			.unwrap_or_else(|e| e.into_inner())
			.clear();
		self.response_time_buckets
			.write()
			.unwrap_or_else(|e| e.into_inner())
			.clear();
		self.status_codes
			.write()
			.unwrap_or_else(|e| e.into_inner())
			.clear();
		self.custom_metrics
			.write()
			.unwrap_or_else(|e| e.into_inner())
			.clear();
	}

	/// Get total request count
	pub fn total_requests(&self) -> u64 {
		self.request_count
			.read()
			.unwrap_or_else(|e| e.into_inner())
			.values()
			.sum()
	}

	/// Get request count for specific method and path
	pub fn request_count(&self, method: &str, path: &str) -> u64 {
		let key = format!("{}:{}", method, path);
		*self
			.request_count
			.read()
			.unwrap_or_else(|e| e.into_inner())
			.get(&key)
			.unwrap_or(&0)
	}

	/// Get status code count
	pub fn status_count(&self, status: u16) -> u64 {
		*self
			.status_codes
			.read()
			.unwrap_or_else(|e| e.into_inner())
			.get(&status)
			.unwrap_or(&0)
	}
}

/// Configuration for metrics middleware
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct MetricsConfig {
	/// Endpoint to expose metrics (default: /metrics)
	pub metrics_endpoint: String,
	/// Enable response time tracking
	pub track_response_time: bool,
	/// Paths to exclude from metrics
	pub exclude_paths: Vec<String>,
}

impl MetricsConfig {
	/// Create a new default configuration
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_middleware::MetricsConfig;
	///
	/// let config = MetricsConfig::new();
	/// assert_eq!(config.metrics_endpoint, "/metrics");
	/// ```
	pub fn new() -> Self {
		Self {
			metrics_endpoint: "/metrics".to_string(),
			track_response_time: true,
			exclude_paths: vec!["/health".to_string(), "/metrics".to_string()],
		}
	}

	/// Set a custom metrics endpoint
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_middleware::MetricsConfig;
	///
	/// let config = MetricsConfig::new().with_endpoint("/prometheus".to_string());
	/// assert_eq!(config.metrics_endpoint, "/prometheus");
	/// ```
	pub fn with_endpoint(mut self, endpoint: String) -> Self {
		self.metrics_endpoint = endpoint;
		self
	}

	/// Disable response time tracking
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_middleware::MetricsConfig;
	///
	/// let config = MetricsConfig::new().without_response_time();
	/// assert!(!config.track_response_time);
	/// ```
	pub fn without_response_time(mut self) -> Self {
		self.track_response_time = false;
		self
	}

	/// Add paths to exclude from metrics
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_middleware::MetricsConfig;
	///
	/// let config = MetricsConfig::new()
	///     .with_excluded_paths(vec!["/admin".to_string()]);
	/// ```
	pub fn with_excluded_paths(mut self, paths: Vec<String>) -> Self {
		self.exclude_paths.extend(paths);
		self
	}
}

impl Default for MetricsConfig {
	fn default() -> Self {
		Self::new()
	}
}

/// Middleware for collecting application metrics
///
/// # Examples
///
/// ```
/// use std::sync::Arc;
/// use reinhardt_middleware::{MetricsMiddleware, MetricsConfig};
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
/// let config = MetricsConfig::new();
/// let middleware = MetricsMiddleware::new(config);
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
/// assert_eq!(response.status, StatusCode::OK);
///
/// // Check that metrics were recorded
/// assert_eq!(middleware.store().total_requests(), 1);
/// # });
/// ```
pub struct MetricsMiddleware {
	config: MetricsConfig,
	store: Arc<MetricsStore>,
}

impl MetricsMiddleware {
	/// Create a new MetricsMiddleware with the given configuration
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_middleware::{MetricsMiddleware, MetricsConfig};
	///
	/// let config = MetricsConfig::new();
	/// let middleware = MetricsMiddleware::new(config);
	/// ```
	pub fn new(config: MetricsConfig) -> Self {
		Self {
			config,
			store: Arc::new(MetricsStore::new()),
		}
	}

	/// Create a new MetricsMiddleware with default configuration
	pub fn with_defaults() -> Self {
		Self::new(MetricsConfig::default())
	}

	/// Create a new MetricsMiddleware from an Arc-wrapped MetricsStore
	///
	/// This allows sharing the same metrics store across multiple middleware instances.
	///
	/// # Examples
	///
	/// ```
	/// use std::sync::Arc;
	/// use reinhardt_middleware::{MetricsMiddleware, MetricsConfig, MetricsStore};
	///
	/// let store = Arc::new(MetricsStore::new());
	/// let config = MetricsConfig::new();
	/// let middleware = MetricsMiddleware::from_arc(config, store);
	/// ```
	pub fn from_arc(config: MetricsConfig, store: Arc<MetricsStore>) -> Self {
		Self { config, store }
	}

	/// Get a reference to the metrics store
	///
	/// This is the preferred method for accessing the store when you only need
	/// to read data or call methods that don't require ownership.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_middleware::{MetricsMiddleware, MetricsConfig};
	///
	/// let middleware = MetricsMiddleware::new(MetricsConfig::new());
	/// let total = middleware.store().total_requests();
	/// println!("Total requests: {}", total);
	/// ```
	pub fn store(&self) -> &MetricsStore {
		&self.store
	}

	/// Get a cloned Arc of the metrics store
	///
	/// Use this when you need ownership of the Arc, for example when passing
	/// the store to another component that requires `Arc<MetricsStore>`.
	///
	/// In most cases, you should prefer `store()` which returns a reference.
	///
	/// # Examples
	///
	/// ```
	/// use std::sync::Arc;
	/// use reinhardt_middleware::{MetricsMiddleware, MetricsConfig, MetricsStore};
	///
	/// let middleware = MetricsMiddleware::new(MetricsConfig::new());
	/// let store_arc: Arc<MetricsStore> = middleware.store_arc();
	/// // Now you can pass store_arc to other components
	/// ```
	pub fn store_arc(&self) -> Arc<MetricsStore> {
		Arc::clone(&self.store)
	}

	/// Check if path should be excluded from metrics
	fn should_exclude(&self, path: &str) -> bool {
		self.config
			.exclude_paths
			.iter()
			.any(|p| path.starts_with(p))
	}

	/// Handle metrics endpoint request
	async fn handle_metrics_endpoint(&self) -> Result<Response> {
		let metrics = self.store.export_prometheus();
		Ok(Response::new(StatusCode::OK)
			.with_header("Content-Type", "text/plain; version=0.0.4")
			.with_body(metrics.into_bytes()))
	}
}

impl Default for MetricsMiddleware {
	fn default() -> Self {
		Self::with_defaults()
	}
}

#[async_trait]
impl Middleware for MetricsMiddleware {
	async fn process(&self, request: Request, handler: Arc<dyn Handler>) -> Result<Response> {
		let path = request.uri.path().to_string();
		let method = request.method.as_str().to_string();

		// Check if this is a metrics endpoint request
		if path == self.config.metrics_endpoint {
			return self.handle_metrics_endpoint().await;
		}

		// Skip excluded paths
		if self.should_exclude(&path) {
			return handler.handle(request).await;
		}

		// Record request
		self.store.record_request(&method, &path);

		// Start timing
		let start = Instant::now();

		// Call handler
		let response = handler.handle(request).await?;

		// Record response time
		if self.config.track_response_time {
			let duration_ms = start.elapsed().as_secs_f64() * 1000.0;
			self.store.record_response_time(&method, &path, duration_ms);
		}

		// Record status code
		self.store.record_status_code(response.status.as_u16());

		Ok(response)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use bytes::Bytes;
	use hyper::{HeaderMap, Method, StatusCode, Version};
	use std::thread;
	use std::time::Duration;

	struct TestHandler {
		status: StatusCode,
		delay: Option<Duration>,
	}

	impl TestHandler {
		fn new(status: StatusCode) -> Self {
			Self {
				status,
				delay: None,
			}
		}

		fn with_delay(mut self, delay: Duration) -> Self {
			self.delay = Some(delay);
			self
		}
	}

	#[async_trait]
	impl Handler for TestHandler {
		async fn handle(&self, _request: Request) -> Result<Response> {
			if let Some(delay) = self.delay {
				thread::sleep(delay);
			}
			Ok(Response::new(self.status).with_body(Bytes::from("OK")))
		}
	}

	#[tokio::test]
	async fn test_record_request_count() {
		let config = MetricsConfig::new();
		let middleware = MetricsMiddleware::new(config);
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

		assert_eq!(middleware.store.total_requests(), 1);
		assert_eq!(middleware.store.request_count("GET", "/test"), 1);
	}

	#[tokio::test]
	async fn test_record_multiple_requests() {
		let config = MetricsConfig::new();
		let middleware = Arc::new(MetricsMiddleware::new(config));
		let handler = Arc::new(TestHandler::new(StatusCode::OK));

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

		assert_eq!(middleware.store.total_requests(), 5);
		assert_eq!(middleware.store.request_count("GET", "/test"), 5);
	}

	#[tokio::test]
	async fn test_record_status_codes() {
		let config = MetricsConfig::new();
		let middleware = Arc::new(MetricsMiddleware::new(config));

		// Send OK request
		let handler_ok = Arc::new(TestHandler::new(StatusCode::OK));
		let request1 = Request::builder()
			.method(Method::GET)
			.uri("/ok")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();
		let _response1 = middleware.process(request1, handler_ok).await.unwrap();

		// Send NOT_FOUND request
		let handler_404 = Arc::new(TestHandler::new(StatusCode::NOT_FOUND));
		let request2 = Request::builder()
			.method(Method::GET)
			.uri("/missing")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();
		let _response2 = middleware.process(request2, handler_404).await.unwrap();

		assert_eq!(middleware.store.status_count(200), 1);
		assert_eq!(middleware.store.status_count(404), 1);
	}

	#[tokio::test]
	async fn test_response_time_tracking() {
		let config = MetricsConfig::new();
		let middleware = MetricsMiddleware::new(config);
		let handler =
			Arc::new(TestHandler::new(StatusCode::OK).with_delay(Duration::from_millis(10)));

		let request = Request::builder()
			.method(Method::GET)
			.uri("/slow")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		let _response = middleware.process(request, handler).await.unwrap();

		// Check that response time was recorded
		let times = middleware.store.response_time_buckets.read().unwrap();
		assert!(times.contains_key("GET:/slow"));
		let durations = times.get("GET:/slow").unwrap();
		assert!(!durations.is_empty());
		assert!(durations[0] >= 10.0); // At least 10ms
	}

	#[tokio::test]
	async fn test_metrics_endpoint() {
		let config = MetricsConfig::new();
		let middleware = MetricsMiddleware::new(config);
		let handler = Arc::new(TestHandler::new(StatusCode::OK));

		// Generate some metrics
		let request1 = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();
		let _response1 = middleware.process(request1, handler.clone()).await.unwrap();

		// Request metrics endpoint
		let metrics_request = Request::builder()
			.method(Method::GET)
			.uri("/metrics")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();
		let metrics_response = middleware.process(metrics_request, handler).await.unwrap();

		assert_eq!(metrics_response.status, StatusCode::OK);
		assert_eq!(
			metrics_response.headers.get("Content-Type").unwrap(),
			"text/plain; version=0.0.4"
		);

		let body = String::from_utf8(metrics_response.body.to_vec()).unwrap();
		assert!(body.contains("http_requests_total"));
		assert!(body.contains("http_responses_total"));
	}

	#[tokio::test]
	async fn test_exclude_paths() {
		let config = MetricsConfig::new();
		let middleware = MetricsMiddleware::new(config);
		let handler = Arc::new(TestHandler::new(StatusCode::OK));

		// Request to excluded path
		let request = Request::builder()
			.method(Method::GET)
			.uri("/health")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();
		let _response = middleware.process(request, handler).await.unwrap();

		// Should not be recorded
		assert_eq!(middleware.store.total_requests(), 0);
	}

	#[tokio::test]
	async fn test_custom_metrics_endpoint() {
		let config = MetricsConfig::new().with_endpoint("/prometheus".to_string());
		let middleware = MetricsMiddleware::new(config);
		let handler = Arc::new(TestHandler::new(StatusCode::OK));

		let request = Request::builder()
			.method(Method::GET)
			.uri("/prometheus")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();
		let response = middleware.process(request, handler).await.unwrap();

		assert_eq!(response.status, StatusCode::OK);
	}

	#[tokio::test]
	async fn test_disable_response_time() {
		let config = MetricsConfig::new().without_response_time();
		let middleware = MetricsMiddleware::new(config);
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

		// Response time should not be tracked
		let times = middleware.store.response_time_buckets.read().unwrap();
		assert!(times.is_empty());
	}

	#[tokio::test]
	async fn test_different_methods() {
		let config = MetricsConfig::new();
		let middleware = Arc::new(MetricsMiddleware::new(config));
		let handler = Arc::new(TestHandler::new(StatusCode::OK));

		// GET request
		let get_request = Request::builder()
			.method(Method::GET)
			.uri("/api/users")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();
		let _response1 = middleware
			.process(get_request, handler.clone())
			.await
			.unwrap();

		// POST request
		let post_request = Request::builder()
			.method(Method::POST)
			.uri("/api/users")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();
		let _response2 = middleware.process(post_request, handler).await.unwrap();

		assert_eq!(middleware.store.request_count("GET", "/api/users"), 1);
		assert_eq!(middleware.store.request_count("POST", "/api/users"), 1);
		assert_eq!(middleware.store.total_requests(), 2);
	}

	#[tokio::test]
	async fn test_prometheus_format() {
		let store = MetricsStore::new();

		store.record_request("GET", "/test");
		store.record_request("GET", "/test");
		store.record_status_code(200);
		store.record_response_time("GET", "/test", 15.5);

		let output = store.export_prometheus();

		assert!(output.contains("# HELP http_requests_total"));
		assert!(output.contains("# TYPE http_requests_total counter"));
		assert!(output.contains("http_requests_total{method=\"GET\",path=\"/test\"} 2"));
		assert!(output.contains("http_responses_total{status=\"200\"} 1"));
		assert!(output.contains("http_response_time_seconds"));
	}

	#[tokio::test]
	async fn test_custom_metrics() {
		let store = MetricsStore::new();

		store.set_custom_metric("database_connections".to_string(), 25.0);
		store.set_custom_metric("cache_hit_ratio".to_string(), 0.87);

		let output = store.export_prometheus();

		assert!(output.contains("database_connections 25"));
		assert!(output.contains("cache_hit_ratio 0.87"));
	}

	#[tokio::test]
	async fn test_reset_metrics() {
		let config = MetricsConfig::new();
		let middleware = MetricsMiddleware::new(config);
		let handler = Arc::new(TestHandler::new(StatusCode::OK));

		// Generate some metrics
		let request = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();
		let _response = middleware.process(request, handler).await.unwrap();

		assert_eq!(middleware.store.total_requests(), 1);

		// Reset
		middleware.store.reset();

		assert_eq!(middleware.store.total_requests(), 0);
	}

	#[tokio::test]
	async fn test_default_middleware() {
		let middleware = MetricsMiddleware::default();
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

		assert_eq!(middleware.store.total_requests(), 1);
	}
}
