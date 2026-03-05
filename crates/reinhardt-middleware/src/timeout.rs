//! Timeout middleware for limiting request processing time
//!
//! This middleware wraps requests with a timeout, returning an error
//! if the handler doesn't complete within the specified duration.

use async_trait::async_trait;
use hyper::StatusCode;
use reinhardt_http::{Handler, Middleware, Request, Response, Result};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;

/// Configuration for timeout middleware
///
/// # Examples
///
/// ```
/// use reinhardt_middleware::timeout::TimeoutConfig;
/// use std::time::Duration;
///
/// let config = TimeoutConfig::new(Duration::from_secs(30));
/// ```
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct TimeoutConfig {
	/// Request timeout duration
	pub duration: Duration,
}

impl TimeoutConfig {
	/// Create a new timeout configuration
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_middleware::timeout::TimeoutConfig;
	/// use std::time::Duration;
	///
	/// let config = TimeoutConfig::new(Duration::from_secs(60));
	/// ```
	pub fn new(duration: Duration) -> Self {
		Self { duration }
	}
}

impl Default for TimeoutConfig {
	fn default() -> Self {
		Self {
			duration: Duration::from_secs(30),
		}
	}
}

/// Timeout middleware
///
/// Wraps request processing with a timeout, returning REQUEST_TIMEOUT (408)
/// if the handler doesn't complete within the configured duration.
///
/// # Examples
///
/// ```
/// use reinhardt_middleware::timeout::{TimeoutMiddleware, TimeoutConfig};
/// use std::time::Duration;
///
/// let config = TimeoutConfig::new(Duration::from_secs(30));
/// let middleware = TimeoutMiddleware::new(config);
/// ```
pub struct TimeoutMiddleware {
	config: TimeoutConfig,
}

impl TimeoutMiddleware {
	/// Create a new timeout middleware
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_middleware::timeout::{TimeoutMiddleware, TimeoutConfig};
	/// use std::time::Duration;
	///
	/// let config = TimeoutConfig::new(Duration::from_secs(30));
	/// let middleware = TimeoutMiddleware::new(config);
	/// ```
	pub fn new(config: TimeoutConfig) -> Self {
		Self { config }
	}
}

#[async_trait]
impl Middleware for TimeoutMiddleware {
	async fn process(&self, request: Request, next: Arc<dyn Handler>) -> Result<Response> {
		match timeout(self.config.duration, next.handle(request)).await {
			Ok(result) => result,
			Err(_) => {
				Ok(Response::new(StatusCode::REQUEST_TIMEOUT)
					.with_body("Request Timeout".to_string()))
			}
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use bytes::Bytes;
	use hyper::{HeaderMap, Method, StatusCode, Version};
	use std::time::Duration;
	use tokio::time::sleep;

	struct FastHandler;

	#[async_trait]
	impl Handler for FastHandler {
		async fn handle(&self, _request: Request) -> Result<Response> {
			Ok(Response::ok())
		}
	}

	struct SlowHandler {
		delay: Duration,
	}

	#[async_trait]
	impl Handler for SlowHandler {
		async fn handle(&self, _request: Request) -> Result<Response> {
			sleep(self.delay).await;
			Ok(Response::ok())
		}
	}

	#[tokio::test]
	async fn test_fast_request_completes() {
		let config = TimeoutConfig::new(Duration::from_secs(1));
		let middleware = TimeoutMiddleware::new(config);
		let handler = Arc::new(FastHandler);

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
	}

	#[tokio::test]
	async fn test_slow_request_times_out() {
		let config = TimeoutConfig::new(Duration::from_millis(100));
		let middleware = TimeoutMiddleware::new(config);
		let handler = Arc::new(SlowHandler {
			delay: Duration::from_millis(500),
		});

		let request = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();
		let response = middleware.process(request, handler).await.unwrap();

		assert_eq!(response.status, StatusCode::REQUEST_TIMEOUT);
		assert_eq!(response.body, Bytes::from("Request Timeout"));
	}

	#[tokio::test]
	async fn test_request_just_within_timeout() {
		let config = TimeoutConfig::new(Duration::from_millis(200));
		let middleware = TimeoutMiddleware::new(config);
		let handler = Arc::new(SlowHandler {
			delay: Duration::from_millis(50),
		});

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
	}

	#[tokio::test]
	async fn test_custom_timeout_duration() {
		let custom_duration = Duration::from_secs(5);
		let config = TimeoutConfig::new(custom_duration);

		assert_eq!(config.duration, custom_duration);
	}

	#[tokio::test]
	async fn test_default_timeout_config() {
		let config = TimeoutConfig::default();

		assert_eq!(config.duration, Duration::from_secs(30));
	}
}
