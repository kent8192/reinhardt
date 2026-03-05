//! Circuit Breaker Middleware
//!
//! Provides fault tolerance and resilience.
//! Temporarily blocks requests to services experiencing frequent errors to protect the system.

use async_trait::async_trait;
use hyper::StatusCode;
use reinhardt_http::{Handler, Middleware, Request, Response, Result};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

/// Circuit breaker state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitState {
	/// Closed (normal operation)
	Closed,
	/// Open (excessive errors)
	Open,
	/// Half-open (recovery testing)
	HalfOpen,
}

/// Circuit breaker statistics using a sliding window
///
/// Uses a time-bounded sliding window to track request outcomes, preventing
/// a short burst of errors from permanently tripping the circuit breaker.
/// Old entries are automatically pruned when calculating error rates.
#[derive(Debug, Clone)]
pub struct CircuitStats {
	/// Timestamped request outcomes within the sliding window
	outcomes: Vec<(Instant, bool)>,
	/// Sliding window duration
	window: Duration,
	/// Last failure time
	last_failure_time: Option<Instant>,
	/// Last success time
	last_success_time: Option<Instant>,
}

impl CircuitStats {
	/// Create new statistics with the given sliding window duration
	fn new(window: Duration) -> Self {
		Self {
			outcomes: Vec::new(),
			window,
			last_failure_time: None,
			last_success_time: None,
		}
	}

	/// Prune entries outside the sliding window
	fn prune(&mut self) {
		let cutoff = Instant::now() - self.window;
		self.outcomes.retain(|(time, _)| *time > cutoff);
	}

	/// Get total requests within the sliding window
	pub fn total_requests(&self) -> u64 {
		self.outcomes.len() as u64
	}

	/// Get failed requests within the sliding window
	pub fn failed_requests(&self) -> u64 {
		self.outcomes.iter().filter(|(_, success)| !success).count() as u64
	}

	/// Get successful requests within the sliding window
	pub fn successful_requests(&self) -> u64 {
		self.outcomes.iter().filter(|(_, success)| *success).count() as u64
	}

	/// Record a success
	fn record_success(&mut self) {
		self.prune();
		self.outcomes.push((Instant::now(), true));
		self.last_success_time = Some(Instant::now());
	}

	/// Record a failure
	fn record_failure(&mut self) {
		self.prune();
		self.outcomes.push((Instant::now(), false));
		self.last_failure_time = Some(Instant::now());
	}

	/// Calculate error rate within the sliding window
	fn error_rate(&self) -> f64 {
		let total = self.outcomes.len();
		if total == 0 {
			0.0
		} else {
			let failed = self.outcomes.iter().filter(|(_, success)| !success).count();
			failed as f64 / total as f64
		}
	}

	/// Reset statistics
	fn reset(&mut self) {
		self.outcomes.clear();
	}
}

/// Circuit breaker state management
#[derive(Debug)]
struct CircuitBreakerState {
	/// Current state
	state: CircuitState,
	/// Statistics
	stats: CircuitStats,
	/// Time when state was opened
	opened_at: Option<Instant>,
}

impl CircuitBreakerState {
	/// Create new state with the given sliding window duration
	fn new(window: Duration) -> Self {
		Self {
			state: CircuitState::Closed,
			stats: CircuitStats::new(window),
			opened_at: None,
		}
	}
}

/// Circuit breaker configuration
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct CircuitBreakerConfig {
	/// Error rate threshold (0.0 - 1.0)
	pub error_threshold: f64,
	/// Minimum number of requests required to open the circuit
	pub min_requests: u64,
	/// Duration for which the circuit remains open
	pub timeout: Duration,
	/// Success count threshold in half-open state
	pub half_open_success_threshold: u64,
	/// Custom error message
	pub error_message: Option<String>,
}

impl CircuitBreakerConfig {
	/// Create a new configuration
	///
	/// # Examples
	///
	/// ```
	/// use std::time::Duration;
	/// use reinhardt_middleware::circuit_breaker::CircuitBreakerConfig;
	///
	/// let config = CircuitBreakerConfig::new(0.5, 10, Duration::from_secs(30));
	/// assert_eq!(config.error_threshold, 0.5);
	/// assert_eq!(config.min_requests, 10);
	/// ```
	pub fn new(error_threshold: f64, min_requests: u64, timeout: Duration) -> Self {
		Self {
			error_threshold,
			min_requests,
			timeout,
			half_open_success_threshold: 5,
			error_message: None,
		}
	}

	/// Set the success count threshold in half-open state
	///
	/// # Examples
	///
	/// ```
	/// use std::time::Duration;
	/// use reinhardt_middleware::circuit_breaker::CircuitBreakerConfig;
	///
	/// let config = CircuitBreakerConfig::new(0.5, 10, Duration::from_secs(30))
	///     .with_half_open_success_threshold(10);
	/// assert_eq!(config.half_open_success_threshold, 10);
	/// ```
	pub fn with_half_open_success_threshold(mut self, threshold: u64) -> Self {
		self.half_open_success_threshold = threshold;
		self
	}

	/// Set a custom error message
	///
	/// # Examples
	///
	/// ```
	/// use std::time::Duration;
	/// use reinhardt_middleware::circuit_breaker::CircuitBreakerConfig;
	///
	/// let config = CircuitBreakerConfig::new(0.5, 10, Duration::from_secs(30))
	///     .with_error_message("Service temporarily unavailable".to_string());
	/// ```
	pub fn with_error_message(mut self, message: String) -> Self {
		self.error_message = Some(message);
		self
	}
}

impl Default for CircuitBreakerConfig {
	fn default() -> Self {
		Self::new(0.5, 10, Duration::from_secs(30))
	}
}

/// Circuit breaker middleware
///
/// # Examples
///
/// ```
/// use std::sync::Arc;
/// use std::time::Duration;
/// use reinhardt_middleware::circuit_breaker::{CircuitBreakerMiddleware, CircuitBreakerConfig};
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
/// let config = CircuitBreakerConfig::new(0.5, 5, Duration::from_secs(30));
/// let middleware = CircuitBreakerMiddleware::new(config);
/// let handler = Arc::new(TestHandler);
///
/// let request = Request::builder()
///     .method(Method::GET)
///     .uri("/api/data")
///     .version(Version::HTTP_11)
///     .headers(HeaderMap::new())
///     .body(Bytes::new())
///     .build()
///     .unwrap();
///
/// let response = middleware.process(request, handler).await.unwrap();
/// assert_eq!(response.status, StatusCode::OK);
/// # });
/// ```
pub struct CircuitBreakerMiddleware {
	config: CircuitBreakerConfig,
	state: Arc<RwLock<CircuitBreakerState>>,
}

impl CircuitBreakerMiddleware {
	/// Create a new circuit breaker middleware
	///
	/// # Examples
	///
	/// ```
	/// use std::time::Duration;
	/// use reinhardt_middleware::circuit_breaker::{CircuitBreakerMiddleware, CircuitBreakerConfig};
	///
	/// let config = CircuitBreakerConfig::new(0.5, 10, Duration::from_secs(30));
	/// let middleware = CircuitBreakerMiddleware::new(config);
	/// ```
	pub fn new(config: CircuitBreakerConfig) -> Self {
		let window = config.timeout;
		Self {
			state: Arc::new(RwLock::new(CircuitBreakerState::new(window))),
			config,
		}
	}

	/// Create with default configuration
	pub fn with_defaults() -> Self {
		Self::new(CircuitBreakerConfig::default())
	}

	/// Get the current state
	pub fn state(&self) -> CircuitState {
		self.state.read().unwrap_or_else(|e| e.into_inner()).state
	}

	/// Get statistics
	pub fn stats(&self) -> CircuitStats {
		self.state
			.read()
			.unwrap_or_else(|e| e.into_inner())
			.stats
			.clone()
	}

	/// Reset the circuit breaker
	pub fn reset(&self) {
		let mut state = self.state.write().unwrap_or_else(|e| e.into_inner());
		state.state = CircuitState::Closed;
		state.stats.reset();
		state.opened_at = None;
	}

	/// Open the circuit
	fn open_circuit(&self) {
		let mut state = self.state.write().unwrap_or_else(|e| e.into_inner());
		state.state = CircuitState::Open;
		state.opened_at = Some(Instant::now());
	}

	/// Close the circuit
	fn close_circuit(&self) {
		let mut state = self.state.write().unwrap_or_else(|e| e.into_inner());
		state.state = CircuitState::Closed;
		state.stats.reset();
		state.opened_at = None;
	}

	/// Transition to half-open state
	fn transition_to_half_open(&self) {
		let mut state = self.state.write().unwrap_or_else(|e| e.into_inner());
		state.state = CircuitState::HalfOpen;
		state.stats.reset();
	}

	/// Check if the response is a failure
	fn is_failure_response(&self, status: StatusCode) -> bool {
		status.is_server_error() || status == StatusCode::TOO_MANY_REQUESTS
	}

	/// Create a circuit breaker error response
	fn circuit_breaker_error(&self) -> Response {
		let message = self
			.config
			.error_message
			.clone()
			.unwrap_or_else(|| "Service temporarily unavailable".to_string());

		Response::new(StatusCode::SERVICE_UNAVAILABLE)
			.with_header("X-Circuit-Breaker", "open")
			.with_body(message.into_bytes())
	}

	/// Check and execute state transitions
	fn check_and_update_state(&self) {
		let state = self.state.read().unwrap_or_else(|e| e.into_inner());
		let current_state = state.state;
		let stats = &state.stats;

		match current_state {
			CircuitState::Closed => {
				// Open the circuit if error rate exceeds threshold
				if stats.total_requests() >= self.config.min_requests
					&& stats.error_rate() >= self.config.error_threshold
				{
					drop(state);
					self.open_circuit();
				}
			}
			CircuitState::Open => {
				// Transition to half-open state after timeout
				if let Some(opened_at) = state.opened_at
					&& opened_at.elapsed() >= self.config.timeout
				{
					drop(state);
					self.transition_to_half_open();
				}
			}
			CircuitState::HalfOpen => {
				// Close the circuit if successes exceed threshold
				if stats.successful_requests() >= self.config.half_open_success_threshold {
					drop(state);
					self.close_circuit();
				}
				// Re-open the circuit if there are failures
				else if stats.failed_requests() > 0 {
					drop(state);
					self.open_circuit();
				}
			}
		}
	}
}

impl Default for CircuitBreakerMiddleware {
	fn default() -> Self {
		Self::with_defaults()
	}
}

#[async_trait]
impl Middleware for CircuitBreakerMiddleware {
	async fn process(&self, request: Request, handler: Arc<dyn Handler>) -> Result<Response> {
		// Check state
		let current_state = self.state();

		match current_state {
			CircuitState::Open => {
				// If circuit is open, check timeout
				self.check_and_update_state();

				// If still open, return error
				if self.state() == CircuitState::Open {
					return Ok(self.circuit_breaker_error());
				}
			}
			CircuitState::HalfOpen | CircuitState::Closed => {
				// Process normally
			}
		}

		// Call handler
		let response = handler.handle(request).await?;

		// Record response
		let is_failure = self.is_failure_response(response.status);
		{
			let mut state = self.state.write().unwrap_or_else(|e| e.into_inner());
			if is_failure {
				state.stats.record_failure();
			} else {
				state.stats.record_success();
			}
		}

		// Check state transitions
		self.check_and_update_state();

		Ok(response)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use bytes::Bytes;
	use hyper::{HeaderMap, Method, StatusCode, Version};
	use std::sync::atomic::{AtomicU64, Ordering};
	use std::thread;

	struct TestHandler {
		fail_count: Arc<AtomicU64>,
		max_failures: u64,
	}

	impl TestHandler {
		fn new(max_failures: u64) -> Self {
			Self {
				fail_count: Arc::new(AtomicU64::new(0)),
				max_failures,
			}
		}
	}

	#[async_trait]
	impl Handler for TestHandler {
		async fn handle(&self, _request: Request) -> Result<Response> {
			let count = self.fail_count.fetch_add(1, Ordering::SeqCst);
			if count < self.max_failures {
				Ok(
					Response::new(StatusCode::INTERNAL_SERVER_ERROR)
						.with_body(Bytes::from("Error")),
				)
			} else {
				Ok(Response::new(StatusCode::OK).with_body(Bytes::from("OK")))
			}
		}
	}

	#[tokio::test]
	async fn test_circuit_closed_state() {
		let config = CircuitBreakerConfig::new(0.5, 5, Duration::from_secs(30));
		let middleware = CircuitBreakerMiddleware::new(config);
		let handler = Arc::new(TestHandler::new(0));

		let request = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();

		assert_eq!(response.status, StatusCode::OK);
		assert_eq!(middleware.state(), CircuitState::Closed);
	}

	#[tokio::test]
	async fn test_circuit_opens_on_errors() {
		let config = CircuitBreakerConfig::new(0.5, 5, Duration::from_secs(30));
		let middleware = Arc::new(CircuitBreakerMiddleware::new(config));
		let handler = Arc::new(TestHandler::new(10)); // Always error

		// Generate errors
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

		// Circuit should be open
		assert_eq!(middleware.state(), CircuitState::Open);
	}

	#[tokio::test]
	async fn test_circuit_open_rejects_requests() {
		let config = CircuitBreakerConfig::new(0.5, 5, Duration::from_secs(30));
		let middleware = Arc::new(CircuitBreakerMiddleware::new(config));
		let handler = Arc::new(TestHandler::new(10));

		// Open the circuit
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

		// Request while circuit is open
		let request = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();
		let response = middleware.process(request, handler).await.unwrap();

		assert_eq!(response.status, StatusCode::SERVICE_UNAVAILABLE);
		assert!(response.headers.contains_key("x-circuit-breaker"));
	}

	#[tokio::test]
	async fn test_circuit_half_open_transition() {
		let config = CircuitBreakerConfig::new(0.5, 5, Duration::from_millis(100));
		let middleware = Arc::new(CircuitBreakerMiddleware::new(config));
		let handler = Arc::new(TestHandler::new(5));

		// Open the circuit
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

		assert_eq!(middleware.state(), CircuitState::Open);

		// After timeout
		thread::sleep(Duration::from_millis(150));

		// Transition to half-open state on next request
		let request = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();
		let _response = middleware.process(request, handler).await.unwrap();

		assert_eq!(middleware.state(), CircuitState::HalfOpen);
	}

	#[tokio::test]
	async fn test_circuit_closes_after_recovery() {
		let config = CircuitBreakerConfig::new(0.5, 5, Duration::from_millis(100))
			.with_half_open_success_threshold(3);
		let middleware = Arc::new(CircuitBreakerMiddleware::new(config));
		let handler = Arc::new(TestHandler::new(10));

		// Open the circuit
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

		// After timeout
		thread::sleep(Duration::from_millis(150));

		// Switch to success handler
		let success_handler = Arc::new(TestHandler::new(0));

		// Request in half-open state
		let request = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();
		let _response = middleware
			.process(request, success_handler.clone())
			.await
			.unwrap();

		assert_eq!(middleware.state(), CircuitState::HalfOpen);

		// Continue with successes
		for _ in 0..3 {
			let request = Request::builder()
				.method(Method::GET)
				.uri("/test")
				.version(Version::HTTP_11)
				.headers(HeaderMap::new())
				.body(Bytes::new())
				.build()
				.unwrap();
			let _response = middleware
				.process(request, success_handler.clone())
				.await
				.unwrap();
		}

		// Circuit should be closed
		assert_eq!(middleware.state(), CircuitState::Closed);
	}

	#[tokio::test]
	async fn test_circuit_stats() {
		let config = CircuitBreakerConfig::new(0.5, 10, Duration::from_secs(30));
		let middleware = Arc::new(CircuitBreakerMiddleware::new(config));

		let success_handler = Arc::new(TestHandler::new(0));
		let fail_handler = Arc::new(TestHandler::new(10));

		// Successful requests
		for _ in 0..3 {
			let request = Request::builder()
				.method(Method::GET)
				.uri("/test")
				.version(Version::HTTP_11)
				.headers(HeaderMap::new())
				.body(Bytes::new())
				.build()
				.unwrap();
			let _response = middleware
				.process(request, success_handler.clone())
				.await
				.unwrap();
		}

		// Failed requests
		for _ in 0..2 {
			let request = Request::builder()
				.method(Method::GET)
				.uri("/test")
				.version(Version::HTTP_11)
				.headers(HeaderMap::new())
				.body(Bytes::new())
				.build()
				.unwrap();
			let _response = middleware
				.process(request, fail_handler.clone())
				.await
				.unwrap();
		}

		let stats = middleware.stats();
		assert_eq!(stats.total_requests(), 5);
		assert_eq!(stats.successful_requests(), 3);
		assert_eq!(stats.failed_requests(), 2);
		assert_eq!(stats.error_rate(), 0.4);
	}

	#[tokio::test]
	async fn test_reset_circuit() {
		let config = CircuitBreakerConfig::new(0.5, 5, Duration::from_secs(30));
		let middleware = Arc::new(CircuitBreakerMiddleware::new(config));
		let handler = Arc::new(TestHandler::new(10));

		// Open the circuit
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

		assert_eq!(middleware.state(), CircuitState::Open);

		// Reset
		middleware.reset();

		assert_eq!(middleware.state(), CircuitState::Closed);
		let stats = middleware.stats();
		assert_eq!(stats.total_requests(), 0);
	}

	#[rstest::rstest]
	fn test_rwlock_poison_recovery_circuit_breaker() {
		// Arrange
		let config = CircuitBreakerConfig::new(0.5, 10, Duration::from_secs(30));
		let middleware = CircuitBreakerMiddleware::new(config);

		// Act - poison the RwLock by panicking while holding a write guard
		let state_clone = Arc::clone(&middleware.state);
		let _ = thread::spawn(move || {
			let _guard = state_clone.write().unwrap();
			panic!("intentional panic to poison lock");
		})
		.join();

		// Assert - operations still work after poison recovery
		assert_eq!(middleware.state(), CircuitState::Closed);
		let stats = middleware.stats();
		assert_eq!(stats.total_requests(), 0);
		middleware.reset();
		assert_eq!(middleware.state(), CircuitState::Closed);
	}
}
