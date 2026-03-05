use reinhardt_http::Handler;
use reinhardt_http::{Request, Response};
use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// Rate limiting strategy
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RateLimitStrategy {
	/// Fixed window rate limiting
	FixedWindow,
	/// Sliding window rate limiting
	SlidingWindow,
}

/// Rate limiter configuration
#[derive(Debug, Clone)]
pub struct RateLimitConfig {
	/// Maximum requests allowed in the window
	pub max_requests: usize,
	/// Time window duration
	pub window_duration: Duration,
	/// Rate limiting strategy
	pub strategy: RateLimitStrategy,
	/// Trusted proxy IP addresses/CIDRs.
	/// Only requests from these IPs will have their X-Forwarded-For/X-Real-IP headers trusted.
	pub trusted_proxies: Vec<String>,
}

impl RateLimitConfig {
	/// Create a new rate limit configuration
	///
	/// # Examples
	///
	/// ```
	/// use std::time::Duration;
	/// use reinhardt_server::server::{RateLimitConfig, RateLimitStrategy};
	///
	/// let config = RateLimitConfig::new(
	///     100,
	///     Duration::from_secs(60),
	///     RateLimitStrategy::FixedWindow,
	/// );
	/// ```
	pub fn new(
		max_requests: usize,
		window_duration: Duration,
		strategy: RateLimitStrategy,
	) -> Self {
		Self {
			max_requests,
			window_duration,
			strategy,
			trusted_proxies: Vec::new(),
		}
	}

	/// Create a per-minute rate limit
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_server::server::RateLimitConfig;
	///
	/// let config = RateLimitConfig::per_minute(60);
	/// ```
	pub fn per_minute(max_requests: usize) -> Self {
		Self::new(
			max_requests,
			Duration::from_secs(60),
			RateLimitStrategy::FixedWindow,
		)
	}

	/// Create a per-hour rate limit
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_server::server::RateLimitConfig;
	///
	/// let config = RateLimitConfig::per_hour(1000);
	/// ```
	pub fn per_hour(max_requests: usize) -> Self {
		Self::new(
			max_requests,
			Duration::from_secs(3600),
			RateLimitStrategy::FixedWindow,
		)
	}

	/// Set trusted proxy addresses.
	///
	/// Only requests originating from these IP addresses will have their
	/// `X-Forwarded-For` and `X-Real-IP` headers trusted for client IP extraction.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_server::server::RateLimitConfig;
	///
	/// let config = RateLimitConfig::per_minute(60)
	///     .with_trusted_proxies(vec!["10.0.0.0/8".to_string()]);
	/// ```
	pub fn with_trusted_proxies(mut self, proxies: Vec<String>) -> Self {
		self.trusted_proxies = proxies;
		self
	}
}

/// Rate limit entry for tracking requests (fixed window strategy)
#[derive(Debug, Clone)]
struct RateLimitEntry {
	count: usize,
	window_start: Instant,
}

/// Rate limit entry for tracking requests (sliding window strategy)
///
/// Stores individual request timestamps to enable true sliding window behavior,
/// where only requests within the most recent `window_duration` are counted.
#[derive(Debug, Clone)]
struct SlidingWindowEntry {
	timestamps: Vec<Instant>,
}

/// Middleware that implements rate limiting
///
/// Tracks requests by client IP address and enforces rate limits.
/// Only trusts proxy headers (X-Forwarded-For, X-Real-IP) when the
/// request comes from a configured trusted proxy address.
///
/// # Examples
///
/// ```
/// use std::sync::Arc;
/// use std::time::Duration;
/// use reinhardt_server::server::{RateLimitHandler, RateLimitConfig};
/// use reinhardt_http::Handler;
/// use reinhardt_http::{Request, Response};
///
/// struct MyHandler;
///
/// #[async_trait::async_trait]
/// impl Handler for MyHandler {
///     async fn handle(&self, _req: Request) -> reinhardt_core::exception::Result<Response> {
///         Ok(Response::ok())
///     }
/// }
///
/// let handler = Arc::new(MyHandler);
/// let config = RateLimitConfig::per_minute(60);
/// let rate_limit_handler = RateLimitHandler::new(handler, config);
/// ```
pub struct RateLimitHandler {
	inner: Arc<dyn Handler>,
	config: RateLimitConfig,
	limits: Arc<RwLock<HashMap<IpAddr, RateLimitEntry>>>,
	sliding_limits: Arc<RwLock<HashMap<IpAddr, SlidingWindowEntry>>>,
}

impl RateLimitHandler {
	/// Create a new rate limit handler
	///
	/// # Arguments
	///
	/// * `inner` - The inner handler to wrap
	/// * `config` - Rate limit configuration
	///
	/// # Examples
	///
	/// ```
	/// use std::sync::Arc;
	/// use reinhardt_server::server::{RateLimitHandler, RateLimitConfig};
	/// use reinhardt_http::Handler;
	/// use reinhardt_http::{Request, Response};
	///
	/// struct MyHandler;
	///
	/// #[async_trait::async_trait]
	/// impl Handler for MyHandler {
	///     async fn handle(&self, _req: Request) -> reinhardt_core::exception::Result<Response> {
	///         Ok(Response::ok())
	///     }
	/// }
	///
	/// let handler = Arc::new(MyHandler);
	/// let config = RateLimitConfig::per_minute(100);
	/// let rate_limit_handler = RateLimitHandler::new(handler, config);
	/// ```
	pub fn new(inner: Arc<dyn Handler>, config: RateLimitConfig) -> Self {
		Self {
			inner,
			config,
			limits: Arc::new(RwLock::new(HashMap::new())),
			sliding_limits: Arc::new(RwLock::new(HashMap::new())),
		}
	}

	/// Check if a request is allowed for the given IP
	///
	/// Dispatches to the appropriate rate limiting algorithm based on the
	/// configured strategy.
	async fn is_allowed(&self, ip: IpAddr) -> bool {
		match self.config.strategy {
			RateLimitStrategy::FixedWindow => self.is_allowed_fixed_window(ip).await,
			RateLimitStrategy::SlidingWindow => self.is_allowed_sliding_window(ip).await,
		}
	}

	/// Fixed window rate limiting: resets the counter when the window expires.
	///
	/// Also performs periodic eviction of stale entries to prevent
	/// unbounded memory growth from accumulated per-IP state.
	async fn is_allowed_fixed_window(&self, ip: IpAddr) -> bool {
		let now = Instant::now();
		let mut limits = self.limits.write().await;

		// Periodically evict stale entries (entries whose window has expired)
		// to prevent unbounded memory growth.
		if limits.len() > 1024 {
			limits.retain(|_, entry| {
				now.duration_since(entry.window_start) < self.config.window_duration * 2
			});
		}

		let entry = limits.entry(ip).or_insert(RateLimitEntry {
			count: 0,
			window_start: now,
		});

		// Check if window has expired
		if now.duration_since(entry.window_start) >= self.config.window_duration {
			// Reset window
			entry.count = 0;
			entry.window_start = now;
		}

		// Check if under limit
		if entry.count < self.config.max_requests {
			entry.count += 1;
			true
		} else {
			false
		}
	}

	/// Sliding window rate limiting: counts requests within the most recent
	/// `window_duration` period, allowing smoother rate distribution.
	///
	/// Unlike fixed window, this approach does not have boundary spikes where
	/// `2 * max_requests` could be served across a window boundary.
	async fn is_allowed_sliding_window(&self, ip: IpAddr) -> bool {
		let now = Instant::now();
		let window = self.config.window_duration;
		let mut limits = self.sliding_limits.write().await;

		// Periodically evict stale entries to prevent unbounded memory growth.
		if limits.len() > 1024 {
			limits.retain(|_, entry| {
				entry
					.timestamps
					.last()
					.is_some_and(|&last| now.duration_since(last) < window * 2)
			});
		}

		let entry = limits.entry(ip).or_insert(SlidingWindowEntry {
			timestamps: Vec::new(),
		});

		// Remove timestamps outside the current window
		entry
			.timestamps
			.retain(|&ts| now.duration_since(ts) < window);

		// Check if under limit
		if entry.timestamps.len() < self.config.max_requests {
			entry.timestamps.push(now);
			true
		} else {
			false
		}
	}

	/// Extract client IP from request
	///
	/// Only trusts proxy headers (X-Forwarded-For, X-Real-IP) when the request
	/// originates from a configured trusted proxy address. Otherwise, uses the
	/// direct connection IP (remote_addr) or falls back to localhost.
	fn extract_client_ip(&self, request: &Request) -> IpAddr {
		let peer_ip = request.remote_addr.map(|addr| addr.ip());

		// Only trust proxy headers if the direct connection is from a trusted proxy
		let from_trusted_proxy = peer_ip.map(|ip| self.is_trusted_proxy(ip)).unwrap_or(false);

		if from_trusted_proxy {
			// Check X-Forwarded-For header
			if let Some(xff) = request.headers.get("X-Forwarded-For")
				&& let Ok(xff_str) = xff.to_str()
				&& let Some(first_ip) = xff_str.split(',').next()
				&& let Ok(ip) = first_ip.trim().parse()
			{
				return ip;
			}

			// Check X-Real-IP header
			if let Some(xri) = request.headers.get("X-Real-IP")
				&& let Ok(ip_str) = xri.to_str()
				&& let Ok(ip) = ip_str.parse()
			{
				return ip;
			}
		}

		// Use remote_addr (direct connection IP)
		if let Some(ip) = peer_ip {
			return ip;
		}

		// Fallback to localhost
		"127.0.0.1".parse().unwrap()
	}

	/// Check if an IP address belongs to a trusted proxy
	fn is_trusted_proxy(&self, ip: IpAddr) -> bool {
		self.config.trusted_proxies.iter().any(|proxy| {
			// Try parsing as CIDR network
			if let Ok(network) = proxy.parse::<ipnet::IpNet>() {
				return network.contains(&ip);
			}
			// Try parsing as single IP
			if let Ok(proxy_ip) = proxy.parse::<IpAddr>() {
				return proxy_ip == ip;
			}
			false
		})
	}
}

#[async_trait::async_trait]
impl Handler for RateLimitHandler {
	async fn handle(&self, request: Request) -> reinhardt_core::exception::Result<Response> {
		let client_ip = self.extract_client_ip(&request);

		if self.is_allowed(client_ip).await {
			self.inner.handle(request).await
		} else {
			Ok(Response::new(http::StatusCode::TOO_MANY_REQUESTS).with_body("Rate limit exceeded"))
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use std::time::Duration;

	/// Polls a condition until it returns true or timeout is reached.
	async fn poll_until<F, Fut>(
		timeout: std::time::Duration,
		interval: std::time::Duration,
		mut condition: F,
	) -> Result<(), String>
	where
		F: FnMut() -> Fut,
		Fut: std::future::Future<Output = bool>,
	{
		let start = std::time::Instant::now();
		while start.elapsed() < timeout {
			if condition().await {
				return Ok(());
			}
			tokio::time::sleep(interval).await;
		}
		Err(format!("Timeout after {:?} waiting for condition", timeout))
	}

	struct TestHandler;

	#[async_trait::async_trait]
	impl Handler for TestHandler {
		async fn handle(&self, _request: Request) -> reinhardt_core::exception::Result<Response> {
			Ok(Response::ok().with_body("Success"))
		}
	}

	#[tokio::test]
	async fn test_rate_limit_config_creation() {
		// Arrange / Act
		let config = RateLimitConfig::per_minute(60);

		// Assert
		assert_eq!(config.max_requests, 60);
		assert_eq!(config.window_duration, Duration::from_secs(60));

		let config = RateLimitConfig::per_hour(1000);
		assert_eq!(config.max_requests, 1000);
		assert_eq!(config.window_duration, Duration::from_secs(3600));
	}

	#[tokio::test]
	async fn test_rate_limit_handler_creation() {
		// Arrange / Act
		let handler = Arc::new(TestHandler);
		let config = RateLimitConfig::per_minute(10);
		let _rate_limit_handler = RateLimitHandler::new(handler, config);
	}

	#[tokio::test]
	async fn test_requests_within_limit() {
		// Arrange
		let handler = Arc::new(TestHandler);
		let config = RateLimitConfig::per_minute(5);
		let rate_limit_handler = RateLimitHandler::new(handler, config);

		// Act / Assert
		for _ in 0..5 {
			let request = Request::builder()
				.method(http::Method::GET)
				.uri("/")
				.version(http::Version::HTTP_11)
				.headers(http::HeaderMap::new())
				.body(bytes::Bytes::new())
				.build()
				.unwrap();

			let response = rate_limit_handler.handle(request).await.unwrap();
			assert_eq!(response.status, http::StatusCode::OK);
		}
	}

	#[tokio::test]
	async fn test_requests_exceed_limit() {
		// Arrange
		let handler = Arc::new(TestHandler);
		let config = RateLimitConfig::per_minute(3);
		let rate_limit_handler = RateLimitHandler::new(handler, config);

		// Act - first 3 requests should succeed
		for _ in 0..3 {
			let request = Request::builder()
				.method(http::Method::GET)
				.uri("/")
				.version(http::Version::HTTP_11)
				.headers(http::HeaderMap::new())
				.body(bytes::Bytes::new())
				.build()
				.unwrap();

			let response = rate_limit_handler.handle(request).await.unwrap();
			assert_eq!(response.status, http::StatusCode::OK);
		}

		// 4th request should be rate limited
		let request = Request::builder()
			.method(http::Method::GET)
			.uri("/")
			.version(http::Version::HTTP_11)
			.headers(http::HeaderMap::new())
			.body(bytes::Bytes::new())
			.build()
			.unwrap();

		let response = rate_limit_handler.handle(request).await.unwrap();

		// Assert
		assert_eq!(response.status, http::StatusCode::TOO_MANY_REQUESTS);
	}

	#[tokio::test]
	async fn test_rate_limit_window_reset() {
		// Arrange
		let handler = Arc::new(TestHandler);
		let config = RateLimitConfig::new(
			2,
			Duration::from_millis(100),
			RateLimitStrategy::FixedWindow,
		);
		let rate_limit_handler = RateLimitHandler::new(handler, config);

		// Act - use up the limit
		for _ in 0..2 {
			let request = Request::builder()
				.method(http::Method::GET)
				.uri("/")
				.version(http::Version::HTTP_11)
				.headers(http::HeaderMap::new())
				.body(bytes::Bytes::new())
				.build()
				.unwrap();
			let response = rate_limit_handler.handle(request).await.unwrap();
			assert_eq!(response.status, http::StatusCode::OK);
		}

		// Assert - poll until rate limit window resets (100ms window duration)
		poll_until(
			Duration::from_millis(200),
			Duration::from_millis(10),
			|| async {
				let test_request = Request::builder()
					.method(http::Method::GET)
					.uri("/")
					.version(http::Version::HTTP_11)
					.headers(http::HeaderMap::new())
					.body(bytes::Bytes::new())
					.build()
					.unwrap();
				let test_response = rate_limit_handler.handle(test_request).await.unwrap();
				test_response.status == http::StatusCode::OK
			},
		)
		.await
		.expect("Window should reset within 200ms");
	}

	// Client IP extraction tests

	#[tokio::test]
	async fn test_sliding_window_requests_within_limit() {
		// Arrange
		let handler = Arc::new(TestHandler);
		let config = RateLimitConfig::new(
			3,
			Duration::from_millis(200),
			RateLimitStrategy::SlidingWindow,
		);
		let rate_limit_handler = RateLimitHandler::new(handler, config);

		// Act / Assert - first 3 requests should succeed
		for _ in 0..3 {
			let request = Request::builder()
				.method(http::Method::GET)
				.uri("/")
				.version(http::Version::HTTP_11)
				.headers(http::HeaderMap::new())
				.body(bytes::Bytes::new())
				.build()
				.unwrap();

			let response = rate_limit_handler.handle(request).await.unwrap();
			assert_eq!(response.status, http::StatusCode::OK);
		}
	}

	#[tokio::test]
	async fn test_sliding_window_requests_exceed_limit() {
		// Arrange
		let handler = Arc::new(TestHandler);
		let config = RateLimitConfig::new(
			2,
			Duration::from_millis(200),
			RateLimitStrategy::SlidingWindow,
		);
		let rate_limit_handler = RateLimitHandler::new(handler, config);

		// Act - first 2 requests should succeed
		for _ in 0..2 {
			let request = Request::builder()
				.method(http::Method::GET)
				.uri("/")
				.version(http::Version::HTTP_11)
				.headers(http::HeaderMap::new())
				.body(bytes::Bytes::new())
				.build()
				.unwrap();
			let response = rate_limit_handler.handle(request).await.unwrap();
			assert_eq!(response.status, http::StatusCode::OK);
		}

		// 3rd request should be rate limited
		let request = Request::builder()
			.method(http::Method::GET)
			.uri("/")
			.version(http::Version::HTTP_11)
			.headers(http::HeaderMap::new())
			.body(bytes::Bytes::new())
			.build()
			.unwrap();
		let response = rate_limit_handler.handle(request).await.unwrap();

		// Assert
		assert_eq!(response.status, http::StatusCode::TOO_MANY_REQUESTS);
	}

	#[tokio::test]
	async fn test_sliding_window_expires_old_requests() {
		// Arrange
		let handler = Arc::new(TestHandler);
		let config = RateLimitConfig::new(
			2,
			Duration::from_millis(100),
			RateLimitStrategy::SlidingWindow,
		);
		let rate_limit_handler = RateLimitHandler::new(handler, config);

		// Act - use up the limit
		for _ in 0..2 {
			let request = Request::builder()
				.method(http::Method::GET)
				.uri("/")
				.version(http::Version::HTTP_11)
				.headers(http::HeaderMap::new())
				.body(bytes::Bytes::new())
				.build()
				.unwrap();
			let response = rate_limit_handler.handle(request).await.unwrap();
			assert_eq!(response.status, http::StatusCode::OK);
		}

		// Assert - poll until old timestamps expire (sliding window)
		poll_until(
			Duration::from_millis(200),
			Duration::from_millis(10),
			|| async {
				let test_request = Request::builder()
					.method(http::Method::GET)
					.uri("/")
					.version(http::Version::HTTP_11)
					.headers(http::HeaderMap::new())
					.body(bytes::Bytes::new())
					.build()
					.unwrap();
				let test_response = rate_limit_handler.handle(test_request).await.unwrap();
				test_response.status == http::StatusCode::OK
			},
		)
		.await
		.expect("Sliding window should allow requests after old timestamps expire");
	}

	#[test]
	fn test_extract_client_ip_from_trusted_xff() {
		// Arrange
		let handler = Arc::new(TestHandler);
		let config =
			RateLimitConfig::per_minute(10).with_trusted_proxies(vec!["10.0.0.1".to_string()]);
		let rate_limit_handler = RateLimitHandler::new(handler, config);

		let mut headers = http::HeaderMap::new();
		headers.insert(
			"X-Forwarded-For",
			"192.168.1.100, 10.0.0.1, 172.16.0.1".parse().unwrap(),
		);

		let mut request = Request::builder()
			.method(http::Method::GET)
			.uri("/")
			.version(http::Version::HTTP_11)
			.headers(headers)
			.body(bytes::Bytes::new())
			.build()
			.unwrap();
		request.remote_addr = Some("10.0.0.1:12345".parse().unwrap());

		// Act
		let ip = rate_limit_handler.extract_client_ip(&request);

		// Assert
		assert_eq!(ip, "192.168.1.100".parse::<IpAddr>().unwrap());
	}

	#[test]
	fn test_extract_client_ip_ignores_untrusted_xff() {
		// Arrange
		let handler = Arc::new(TestHandler);
		let config =
			RateLimitConfig::per_minute(10).with_trusted_proxies(vec!["10.0.0.1".to_string()]);
		let rate_limit_handler = RateLimitHandler::new(handler, config);

		let mut headers = http::HeaderMap::new();
		headers.insert("X-Forwarded-For", "192.168.1.100".parse().unwrap());

		let mut request = Request::builder()
			.method(http::Method::GET)
			.uri("/")
			.version(http::Version::HTTP_11)
			.headers(headers)
			.body(bytes::Bytes::new())
			.build()
			.unwrap();
		// Untrusted source
		request.remote_addr = Some("203.0.113.42:54321".parse().unwrap());

		// Act
		let ip = rate_limit_handler.extract_client_ip(&request);

		// Assert - should use remote_addr, not spoofed header
		assert_eq!(ip, "203.0.113.42".parse::<IpAddr>().unwrap());
	}

	#[test]
	fn test_extract_client_ip_from_trusted_x_real_ip() {
		// Arrange
		let handler = Arc::new(TestHandler);
		let config =
			RateLimitConfig::per_minute(10).with_trusted_proxies(vec!["10.0.0.0/8".to_string()]);
		let rate_limit_handler = RateLimitHandler::new(handler, config);

		let mut headers = http::HeaderMap::new();
		headers.insert("X-Real-IP", "203.0.113.42".parse().unwrap());

		let mut request = Request::builder()
			.method(http::Method::GET)
			.uri("/")
			.version(http::Version::HTTP_11)
			.headers(headers)
			.body(bytes::Bytes::new())
			.build()
			.unwrap();
		request.remote_addr = Some("10.0.0.5:8080".parse().unwrap());

		// Act
		let ip = rate_limit_handler.extract_client_ip(&request);

		// Assert
		assert_eq!(ip, "203.0.113.42".parse::<IpAddr>().unwrap());
	}

	#[test]
	fn test_extract_client_ip_fallback_to_localhost() {
		// Arrange
		let handler = Arc::new(TestHandler);
		let config = RateLimitConfig::per_minute(10);
		let rate_limit_handler = RateLimitHandler::new(handler, config);

		let headers = http::HeaderMap::new();
		let request = Request::builder()
			.method(http::Method::GET)
			.uri("/")
			.version(http::Version::HTTP_11)
			.headers(headers)
			.body(bytes::Bytes::new())
			.build()
			.unwrap();

		// Act
		let ip = rate_limit_handler.extract_client_ip(&request);

		// Assert
		assert_eq!(ip, "127.0.0.1".parse::<IpAddr>().unwrap());
	}

	#[test]
	fn test_extract_client_ip_no_trusted_proxies() {
		// Arrange - no trusted proxies, proxy headers should be ignored
		let handler = Arc::new(TestHandler);
		let config = RateLimitConfig::per_minute(10);
		let rate_limit_handler = RateLimitHandler::new(handler, config);

		let mut headers = http::HeaderMap::new();
		headers.insert("X-Forwarded-For", "192.168.1.100".parse().unwrap());

		let mut request = Request::builder()
			.method(http::Method::GET)
			.uri("/")
			.version(http::Version::HTTP_11)
			.headers(headers)
			.body(bytes::Bytes::new())
			.build()
			.unwrap();
		request.remote_addr = Some("203.0.113.1:8080".parse().unwrap());

		// Act
		let ip = rate_limit_handler.extract_client_ip(&request);

		// Assert - uses remote_addr since no proxies are trusted
		assert_eq!(ip, "203.0.113.1".parse::<IpAddr>().unwrap());
	}

	#[test]
	fn test_extract_client_ip_with_invalid_header() {
		// Arrange
		let handler = Arc::new(TestHandler);
		let config =
			RateLimitConfig::per_minute(10).with_trusted_proxies(vec!["10.0.0.1".to_string()]);
		let rate_limit_handler = RateLimitHandler::new(handler, config);

		let mut headers = http::HeaderMap::new();
		headers.insert("X-Forwarded-For", "invalid-ip".parse().unwrap());

		let mut request = Request::builder()
			.method(http::Method::GET)
			.uri("/")
			.version(http::Version::HTTP_11)
			.headers(headers)
			.body(bytes::Bytes::new())
			.build()
			.unwrap();
		request.remote_addr = Some("10.0.0.1:8080".parse().unwrap());

		// Act
		let ip = rate_limit_handler.extract_client_ip(&request);

		// Assert - falls back to remote_addr when header is invalid
		assert_eq!(ip, "10.0.0.1".parse::<IpAddr>().unwrap());
	}
}
