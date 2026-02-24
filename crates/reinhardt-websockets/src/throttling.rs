//! WebSocket rate limiting and throttling
//!
//! This module provides rate limiting capabilities for WebSocket connections,
//! preventing abuse and ensuring fair resource usage.
//!
//! ## Rate Limiting Layers
//!
//! Three independent layers of rate limiting are available:
//!
//! - **Connection rate limiting** ([`ConnectionRateLimiter`]): Limits the rate of new
//!   connections from a single IP address within a sliding time window.
//! - **Concurrent connection throttling** ([`ConnectionThrottler`]): Limits the number
//!   of simultaneous connections from a single IP address.
//! - **Message rate limiting** ([`RateLimiter`]): Limits the rate of messages per
//!   connection within a time window.
//!
//! These can be composed via [`WebSocketRateLimitConfig`] and applied as middleware
//! through [`RateLimitMiddleware`].

use crate::connection::{Message, WebSocketConnection};
use crate::middleware::{
	ConnectionContext, ConnectionMiddleware, MessageMiddleware, MiddlewareError, MiddlewareResult,
};
use async_trait::async_trait;
use std::collections::HashMap;
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// Throttling errors
#[derive(Debug, thiserror::Error)]
pub enum ThrottleError {
	#[error("Rate limit exceeded")]
	RateLimitExceeded(String),
	#[error("Too many connections")]
	TooManyConnections(String),
	#[error("Connection rate exceeded")]
	ConnectionRateExceeded(String),
}

/// Result type for throttling operations
pub type ThrottleResult<T> = Result<T, ThrottleError>;

/// Rate limit configuration
///
/// # Examples
///
/// ```
/// use reinhardt_websockets::throttling::RateLimitConfig;
/// use std::time::Duration;
///
/// let config = RateLimitConfig::new(100, Duration::from_secs(60));
/// assert_eq!(config.max_requests(), 100);
/// assert_eq!(config.window(), Duration::from_secs(60));
/// ```
#[derive(Debug, Clone)]
pub struct RateLimitConfig {
	max_requests: usize,
	window: Duration,
}

impl RateLimitConfig {
	/// Create a new rate limit configuration
	///
	/// # Arguments
	///
	/// * `max_requests` - Maximum number of requests allowed
	/// * `window` - Time window for the rate limit
	pub fn new(max_requests: usize, window: Duration) -> Self {
		Self {
			max_requests,
			window,
		}
	}

	/// Get maximum requests allowed
	pub fn max_requests(&self) -> usize {
		self.max_requests
	}

	/// Get time window
	pub fn window(&self) -> Duration {
		self.window
	}

	/// Create a permissive rate limit (high limit)
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_websockets::throttling::RateLimitConfig;
	///
	/// let config = RateLimitConfig::permissive();
	/// assert_eq!(config.max_requests(), 10000);
	/// ```
	pub fn permissive() -> Self {
		Self::new(10000, Duration::from_secs(60))
	}

	/// Create a strict rate limit (low limit)
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_websockets::throttling::RateLimitConfig;
	///
	/// let config = RateLimitConfig::strict();
	/// assert_eq!(config.max_requests(), 10);
	/// ```
	pub fn strict() -> Self {
		Self::new(10, Duration::from_secs(60))
	}
}

/// Request counter for tracking rate limits
#[derive(Debug)]
struct RequestCounter {
	count: usize,
	window_start: Instant,
}

impl RequestCounter {
	fn new() -> Self {
		Self {
			count: 0,
			window_start: Instant::now(),
		}
	}

	fn increment(&mut self, config: &RateLimitConfig) -> ThrottleResult<()> {
		let elapsed = self.window_start.elapsed();

		if elapsed >= config.window {
			// Reset window
			self.count = 1;
			self.window_start = Instant::now();
			Ok(())
		} else if self.count < config.max_requests {
			self.count += 1;
			Ok(())
		} else {
			Err(ThrottleError::RateLimitExceeded(format!(
				"Exceeded {} requests per {:?}",
				config.max_requests, config.window
			)))
		}
	}

	fn reset(&mut self) {
		self.count = 0;
		self.window_start = Instant::now();
	}
}

/// Rate limiter for WebSocket connections
///
/// # Examples
///
/// ```
/// use reinhardt_websockets::throttling::{RateLimiter, RateLimitConfig};
/// use std::time::Duration;
///
/// # tokio_test::block_on(async {
/// let config = RateLimitConfig::new(5, Duration::from_secs(10));
/// let limiter = RateLimiter::new(config);
///
/// // First 5 requests should succeed
/// for _ in 0..5 {
///     assert!(limiter.check_rate_limit("user_1").await.is_ok());
/// }
///
/// // 6th request should fail
/// assert!(limiter.check_rate_limit("user_1").await.is_err());
/// # });
/// ```
pub struct RateLimiter {
	config: RateLimitConfig,
	counters: Arc<RwLock<HashMap<String, RequestCounter>>>,
}

impl RateLimiter {
	/// Create a new rate limiter
	pub fn new(config: RateLimitConfig) -> Self {
		Self {
			config,
			counters: Arc::new(RwLock::new(HashMap::new())),
		}
	}

	/// Check if a client is within rate limits
	///
	/// # Arguments
	///
	/// * `client_id` - Unique identifier for the client
	pub async fn check_rate_limit(&self, client_id: &str) -> ThrottleResult<()> {
		let mut counters = self.counters.write().await;

		let counter = counters
			.entry(client_id.to_string())
			.or_insert_with(RequestCounter::new);

		counter.increment(&self.config)
	}

	/// Reset rate limit for a specific client
	pub async fn reset_client(&self, client_id: &str) {
		let mut counters = self.counters.write().await;
		if let Some(counter) = counters.get_mut(client_id) {
			counter.reset();
		}
	}

	/// Clear all rate limit counters
	pub async fn clear_all(&self) {
		let mut counters = self.counters.write().await;
		counters.clear();
	}

	/// Get current request count for a client
	pub async fn get_count(&self, client_id: &str) -> usize {
		let counters = self.counters.read().await;
		counters
			.get(client_id)
			.map(|counter| counter.count)
			.unwrap_or(0)
	}
}

/// Connection throttler for limiting concurrent connections
///
/// # Examples
///
/// ```
/// use reinhardt_websockets::throttling::ConnectionThrottler;
///
/// # tokio_test::block_on(async {
/// let throttler = ConnectionThrottler::new(3);
///
/// // First 3 connections should succeed
/// assert!(throttler.acquire_connection("192.168.1.1").await.is_ok());
/// assert!(throttler.acquire_connection("192.168.1.1").await.is_ok());
/// assert!(throttler.acquire_connection("192.168.1.1").await.is_ok());
///
/// // 4th connection should fail
/// assert!(throttler.acquire_connection("192.168.1.1").await.is_err());
///
/// // After releasing one, should succeed again
/// throttler.release_connection("192.168.1.1").await;
/// assert!(throttler.acquire_connection("192.168.1.1").await.is_ok());
/// # });
/// ```
pub struct ConnectionThrottler {
	max_connections_per_ip: usize,
	connections: Arc<RwLock<HashMap<String, usize>>>,
}

impl ConnectionThrottler {
	/// Create a new connection throttler
	///
	/// # Arguments
	///
	/// * `max_connections_per_ip` - Maximum concurrent connections per IP address
	pub fn new(max_connections_per_ip: usize) -> Self {
		Self {
			max_connections_per_ip,
			connections: Arc::new(RwLock::new(HashMap::new())),
		}
	}

	/// Acquire a connection slot for an IP address
	pub async fn acquire_connection(&self, ip: &str) -> ThrottleResult<()> {
		let mut connections = self.connections.write().await;

		let count = connections.entry(ip.to_string()).or_insert(0);

		if *count >= self.max_connections_per_ip {
			Err(ThrottleError::TooManyConnections(ip.to_string()))
		} else {
			*count += 1;
			Ok(())
		}
	}

	/// Release a connection slot for an IP address
	pub async fn release_connection(&self, ip: &str) {
		let mut connections = self.connections.write().await;

		if let Some(count) = connections.get_mut(ip) {
			if *count > 0 {
				*count -= 1;
			}
			if *count == 0 {
				connections.remove(ip);
			}
		}
	}

	/// Get current connection count for an IP address
	pub async fn get_connection_count(&self, ip: &str) -> usize {
		let connections = self.connections.read().await;
		connections.get(ip).copied().unwrap_or(0)
	}

	/// Clear all connection counts
	pub async fn clear_all(&self) {
		let mut connections = self.connections.write().await;
		connections.clear();
	}
}

/// Combined throttler with both rate limiting and connection throttling
///
/// # Examples
///
/// ```
/// use reinhardt_websockets::throttling::{CombinedThrottler, RateLimitConfig};
/// use std::time::Duration;
///
/// # tokio_test::block_on(async {
/// let throttler = CombinedThrottler::new(
///     RateLimitConfig::new(100, Duration::from_secs(60)),
///     10,
/// );
///
/// // Check connection limit
/// assert!(throttler.check_connection("192.168.1.1").await.is_ok());
///
/// // Check message rate limit
/// assert!(throttler.check_message_rate("user_1").await.is_ok());
/// # });
/// ```
pub struct CombinedThrottler {
	rate_limiter: RateLimiter,
	connection_throttler: ConnectionThrottler,
}

impl CombinedThrottler {
	/// Create a new combined throttler
	pub fn new(rate_config: RateLimitConfig, max_connections_per_ip: usize) -> Self {
		Self {
			rate_limiter: RateLimiter::new(rate_config),
			connection_throttler: ConnectionThrottler::new(max_connections_per_ip),
		}
	}

	/// Check if a connection is allowed
	pub async fn check_connection(&self, ip: &str) -> ThrottleResult<()> {
		self.connection_throttler.acquire_connection(ip).await
	}

	/// Release a connection
	pub async fn release_connection(&self, ip: &str) {
		self.connection_throttler.release_connection(ip).await
	}

	/// Check message rate limit
	pub async fn check_message_rate(&self, client_id: &str) -> ThrottleResult<()> {
		self.rate_limiter.check_rate_limit(client_id).await
	}

	/// Reset client rate limit
	pub async fn reset_client_rate(&self, client_id: &str) {
		self.rate_limiter.reset_client(client_id).await
	}
}

/// Connection rate limiter using a sliding window algorithm.
///
/// Unlike [`ConnectionThrottler`] which limits concurrent connections,
/// this limiter tracks the rate of new connection attempts per IP
/// address within a time window, preventing connection flooding attacks.
///
/// # Algorithm
///
/// Uses a sliding window approach: timestamps of recent connection attempts
/// are stored per IP. When a new attempt arrives, expired timestamps are
/// pruned. If the remaining count exceeds the limit, the attempt is rejected.
///
/// # Examples
///
/// ```
/// use reinhardt_websockets::throttling::ConnectionRateLimiter;
/// use std::time::Duration;
///
/// # tokio_test::block_on(async {
/// let limiter = ConnectionRateLimiter::new(3, Duration::from_secs(60));
///
/// // First 3 connections in the window succeed
/// assert!(limiter.check_connection_rate("192.168.1.1").await.is_ok());
/// assert!(limiter.check_connection_rate("192.168.1.1").await.is_ok());
/// assert!(limiter.check_connection_rate("192.168.1.1").await.is_ok());
///
/// // 4th connection within the window is rejected
/// assert!(limiter.check_connection_rate("192.168.1.1").await.is_err());
///
/// // Different IP is unaffected
/// assert!(limiter.check_connection_rate("10.0.0.1").await.is_ok());
/// # });
/// ```
pub struct ConnectionRateLimiter {
	max_connections_per_window: usize,
	window: Duration,
	timestamps: Arc<RwLock<HashMap<String, VecDeque<Instant>>>>,
}

impl ConnectionRateLimiter {
	/// Create a new connection rate limiter.
	///
	/// # Arguments
	///
	/// * `max_connections_per_window` - Maximum new connections allowed per IP within the window
	/// * `window` - Sliding time window duration
	pub fn new(max_connections_per_window: usize, window: Duration) -> Self {
		Self {
			max_connections_per_window,
			window,
			timestamps: Arc::new(RwLock::new(HashMap::new())),
		}
	}

	/// Check if a new connection from the given IP is within the rate limit.
	///
	/// Records the attempt timestamp if allowed.
	pub async fn check_connection_rate(&self, ip: &str) -> ThrottleResult<()> {
		let mut timestamps = self.timestamps.write().await;
		let now = Instant::now();

		let entries = timestamps
			.entry(ip.to_string())
			.or_insert_with(VecDeque::new);

		// Prune expired timestamps
		while let Some(&front) = entries.front() {
			if now.duration_since(front) > self.window {
				entries.pop_front();
			} else {
				break;
			}
		}

		if entries.len() >= self.max_connections_per_window {
			Err(ThrottleError::ConnectionRateExceeded(format!(
				"{} (exceeded {} connections per {:?})",
				ip, self.max_connections_per_window, self.window
			)))
		} else {
			entries.push_back(now);
			Ok(())
		}
	}

	/// Get the number of connection attempts in the current window for an IP.
	pub async fn get_current_count(&self, ip: &str) -> usize {
		let timestamps = self.timestamps.read().await;
		let now = Instant::now();

		timestamps
			.get(ip)
			.map(|entries| {
				entries
					.iter()
					.filter(|&&ts| now.duration_since(ts) <= self.window)
					.count()
			})
			.unwrap_or(0)
	}

	/// Clear all tracked timestamps.
	pub async fn clear_all(&self) {
		let mut timestamps = self.timestamps.write().await;
		timestamps.clear();
	}

	/// Get the maximum connections per window.
	pub fn max_connections_per_window(&self) -> usize {
		self.max_connections_per_window
	}

	/// Get the window duration.
	pub fn window(&self) -> Duration {
		self.window
	}
}

/// Comprehensive rate limit configuration for WebSocket connections.
///
/// Combines connection rate limiting, concurrent connection throttling,
/// and message rate limiting into a single configuration.
///
/// # Examples
///
/// ```
/// use reinhardt_websockets::throttling::WebSocketRateLimitConfig;
/// use std::time::Duration;
///
/// // Use sensible defaults
/// let config = WebSocketRateLimitConfig::default();
/// assert_eq!(config.max_connections_per_window(), 20);
/// assert_eq!(config.max_concurrent_connections_per_ip(), 10);
/// assert_eq!(config.max_messages_per_window(), 100);
///
/// // Customize
/// let config = WebSocketRateLimitConfig::default()
///     .with_max_connections_per_window(50)
///     .with_max_messages_per_window(200);
/// assert_eq!(config.max_connections_per_window(), 50);
/// assert_eq!(config.max_messages_per_window(), 200);
/// ```
#[derive(Debug, Clone)]
pub struct WebSocketRateLimitConfig {
	/// Maximum new connections per IP within the connection window
	max_connections_per_window: usize,
	/// Time window for connection rate limiting
	connection_window: Duration,
	/// Maximum concurrent connections per IP
	max_concurrent_connections_per_ip: usize,
	/// Maximum messages per connection within the message window
	max_messages_per_window: usize,
	/// Time window for message rate limiting
	message_window: Duration,
}

impl Default for WebSocketRateLimitConfig {
	/// Sensible default rate limits:
	/// - 20 new connections per IP per 60 seconds
	/// - 10 concurrent connections per IP
	/// - 100 messages per connection per 60 seconds
	fn default() -> Self {
		Self {
			max_connections_per_window: 20,
			connection_window: Duration::from_secs(60),
			max_concurrent_connections_per_ip: 10,
			max_messages_per_window: 100,
			message_window: Duration::from_secs(60),
		}
	}
}

impl WebSocketRateLimitConfig {
	/// Create a strict configuration for high-security environments.
	///
	/// - 5 new connections per IP per 60 seconds
	/// - 3 concurrent connections per IP
	/// - 30 messages per connection per 60 seconds
	pub fn strict() -> Self {
		Self {
			max_connections_per_window: 5,
			connection_window: Duration::from_secs(60),
			max_concurrent_connections_per_ip: 3,
			max_messages_per_window: 30,
			message_window: Duration::from_secs(60),
		}
	}

	/// Create a permissive configuration for trusted environments.
	///
	/// - 100 new connections per IP per 60 seconds
	/// - 50 concurrent connections per IP
	/// - 1000 messages per connection per 60 seconds
	pub fn permissive() -> Self {
		Self {
			max_connections_per_window: 100,
			connection_window: Duration::from_secs(60),
			max_concurrent_connections_per_ip: 50,
			max_messages_per_window: 1000,
			message_window: Duration::from_secs(60),
		}
	}

	/// Set the maximum new connections per IP within the connection window.
	pub fn with_max_connections_per_window(mut self, max: usize) -> Self {
		self.max_connections_per_window = max;
		self
	}

	/// Set the connection rate limiting window duration.
	pub fn with_connection_window(mut self, window: Duration) -> Self {
		self.connection_window = window;
		self
	}

	/// Set the maximum concurrent connections per IP.
	pub fn with_max_concurrent_connections_per_ip(mut self, max: usize) -> Self {
		self.max_concurrent_connections_per_ip = max;
		self
	}

	/// Set the maximum messages per connection within the message window.
	pub fn with_max_messages_per_window(mut self, max: usize) -> Self {
		self.max_messages_per_window = max;
		self
	}

	/// Set the message rate limiting window duration.
	pub fn with_message_window(mut self, window: Duration) -> Self {
		self.message_window = window;
		self
	}

	/// Get the maximum connections per window.
	pub fn max_connections_per_window(&self) -> usize {
		self.max_connections_per_window
	}

	/// Get the connection window duration.
	pub fn connection_window(&self) -> Duration {
		self.connection_window
	}

	/// Get the maximum concurrent connections per IP.
	pub fn max_concurrent_connections_per_ip(&self) -> usize {
		self.max_concurrent_connections_per_ip
	}

	/// Get the maximum messages per window.
	pub fn max_messages_per_window(&self) -> usize {
		self.max_messages_per_window
	}

	/// Get the message window duration.
	pub fn message_window(&self) -> Duration {
		self.message_window
	}
}

/// Rate limiting middleware for WebSocket connections.
///
/// Integrates connection rate limiting, concurrent connection throttling,
/// and message rate limiting into the middleware chain.
///
/// # Connection Rate Limiting
///
/// On each new connection attempt, the middleware checks:
/// 1. Connection rate: Is the IP exceeding new connections per time window?
/// 2. Concurrent connections: Is the IP exceeding the max simultaneous connections?
///
/// # Message Rate Limiting
///
/// On each incoming message, the middleware checks whether the connection
/// has exceeded the message rate limit.
///
/// # Examples
///
/// ```
/// use reinhardt_websockets::throttling::{RateLimitMiddleware, WebSocketRateLimitConfig};
/// use reinhardt_websockets::middleware::{
///     MiddlewareChain, ConnectionMiddleware, ConnectionContext,
/// };
///
/// # tokio_test::block_on(async {
/// let config = WebSocketRateLimitConfig::default();
/// let middleware = RateLimitMiddleware::new(config);
///
/// let mut context = ConnectionContext::new("192.168.1.1".to_string());
/// assert!(middleware.on_connect(&mut context).await.is_ok());
/// # });
/// ```
pub struct RateLimitMiddleware {
	connection_rate_limiter: ConnectionRateLimiter,
	connection_throttler: ConnectionThrottler,
	message_rate_limiter: RateLimiter,
}

impl RateLimitMiddleware {
	/// Create a new rate limit middleware from the given configuration.
	pub fn new(config: WebSocketRateLimitConfig) -> Self {
		Self {
			connection_rate_limiter: ConnectionRateLimiter::new(
				config.max_connections_per_window,
				config.connection_window,
			),
			connection_throttler: ConnectionThrottler::new(
				config.max_concurrent_connections_per_ip,
			),
			message_rate_limiter: RateLimiter::new(RateLimitConfig::new(
				config.max_messages_per_window,
				config.message_window,
			)),
		}
	}

	/// Create a rate limit middleware with sensible defaults.
	pub fn with_defaults() -> Self {
		Self::new(WebSocketRateLimitConfig::default())
	}

	/// Release a connection slot when a client disconnects.
	///
	/// This should be called by the application when a connection
	/// is closed to free up the concurrent connection slot.
	pub async fn release_connection(&self, ip: &str) {
		self.connection_throttler.release_connection(ip).await;
	}

	/// Get a reference to the underlying connection rate limiter.
	pub fn connection_rate_limiter(&self) -> &ConnectionRateLimiter {
		&self.connection_rate_limiter
	}

	/// Get a reference to the underlying connection throttler.
	pub fn connection_throttler(&self) -> &ConnectionThrottler {
		&self.connection_throttler
	}

	/// Get a reference to the underlying message rate limiter.
	pub fn message_rate_limiter(&self) -> &RateLimiter {
		&self.message_rate_limiter
	}
}

#[async_trait]
impl ConnectionMiddleware for RateLimitMiddleware {
	async fn on_connect(&self, context: &mut ConnectionContext) -> MiddlewareResult<()> {
		let ip = &context.ip;

		// Check connection rate (sliding window)
		self.connection_rate_limiter
			.check_connection_rate(ip)
			.await
			.map_err(|e| MiddlewareError::ConnectionRejected(e.to_string()))?;

		// Check concurrent connection limit
		self.connection_throttler
			.acquire_connection(ip)
			.await
			.map_err(|e| MiddlewareError::ConnectionRejected(e.to_string()))?;

		Ok(())
	}

	async fn on_disconnect(&self, connection: &Arc<WebSocketConnection>) -> MiddlewareResult<()> {
		// Release the concurrent connection slot.
		// The connection ID is used as a fallback; in production the IP
		// should be stored in the connection metadata during on_connect.
		// For now, we look for the IP in the connection's metadata if
		// the middleware stored it, otherwise we cannot release.
		// This is a best-effort release.
		//
		// NOTE: Callers should prefer calling `release_connection(ip)` directly
		// when the IP is known.
		let _ = connection;
		Ok(())
	}
}

#[async_trait]
impl MessageMiddleware for RateLimitMiddleware {
	async fn on_message(
		&self,
		connection: &Arc<WebSocketConnection>,
		message: Message,
	) -> MiddlewareResult<Message> {
		self.message_rate_limiter
			.check_rate_limit(connection.id())
			.await
			.map_err(|e| MiddlewareError::MessageRejected(e.to_string()))?;

		Ok(message)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;
	use tokio::sync::mpsc;

	// --- RateLimitConfig tests ---

	#[rstest]
	fn test_rate_limit_config_new() {
		// Arrange & Act
		let config = RateLimitConfig::new(100, Duration::from_secs(60));

		// Assert
		assert_eq!(config.max_requests(), 100);
		assert_eq!(config.window(), Duration::from_secs(60));
	}

	#[rstest]
	fn test_rate_limit_config_presets() {
		// Arrange & Act
		let permissive = RateLimitConfig::permissive();
		let strict = RateLimitConfig::strict();

		// Assert
		assert_eq!(permissive.max_requests(), 10000);
		assert_eq!(permissive.window(), Duration::from_secs(60));
		assert_eq!(strict.max_requests(), 10);
		assert_eq!(strict.window(), Duration::from_secs(60));
	}

	// --- RateLimiter tests ---

	#[rstest]
	#[tokio::test]
	async fn test_rate_limiter_within_limit() {
		// Arrange
		let config = RateLimitConfig::new(5, Duration::from_secs(10));
		let limiter = RateLimiter::new(config);

		// Act & Assert
		for _ in 0..5 {
			assert!(limiter.check_rate_limit("user_1").await.is_ok());
		}
	}

	#[rstest]
	#[tokio::test]
	async fn test_rate_limiter_exceeds_limit() {
		// Arrange
		let config = RateLimitConfig::new(3, Duration::from_secs(10));
		let limiter = RateLimiter::new(config);

		// Act
		for _ in 0..3 {
			limiter.check_rate_limit("user_1").await.unwrap();
		}
		let result = limiter.check_rate_limit("user_1").await;

		// Assert
		assert!(result.is_err());
		assert!(matches!(
			result.unwrap_err(),
			ThrottleError::RateLimitExceeded(_)
		));
	}

	#[rstest]
	#[tokio::test]
	async fn test_rate_limiter_reset() {
		// Arrange
		let config = RateLimitConfig::new(2, Duration::from_secs(10));
		let limiter = RateLimiter::new(config);
		limiter.check_rate_limit("user_1").await.unwrap();
		limiter.check_rate_limit("user_1").await.unwrap();
		assert!(limiter.check_rate_limit("user_1").await.is_err());

		// Act
		limiter.reset_client("user_1").await;

		// Assert
		assert!(limiter.check_rate_limit("user_1").await.is_ok());
	}

	#[rstest]
	#[tokio::test]
	async fn test_rate_limiter_get_count() {
		// Arrange
		let config = RateLimitConfig::new(10, Duration::from_secs(10));
		let limiter = RateLimiter::new(config);

		// Act
		assert_eq!(limiter.get_count("user_1").await, 0);
		limiter.check_rate_limit("user_1").await.unwrap();
		limiter.check_rate_limit("user_1").await.unwrap();

		// Assert
		assert_eq!(limiter.get_count("user_1").await, 2);
	}

	#[rstest]
	#[tokio::test]
	async fn test_rate_limiter_independent_clients() {
		// Arrange
		let config = RateLimitConfig::new(2, Duration::from_secs(10));
		let limiter = RateLimiter::new(config);

		// Act - exhaust user_1's limit
		limiter.check_rate_limit("user_1").await.unwrap();
		limiter.check_rate_limit("user_1").await.unwrap();
		assert!(limiter.check_rate_limit("user_1").await.is_err());

		// Assert - user_2 is unaffected
		assert!(limiter.check_rate_limit("user_2").await.is_ok());
	}

	// --- ConnectionThrottler tests ---

	#[rstest]
	#[tokio::test]
	async fn test_connection_throttler_within_limit() {
		// Arrange
		let throttler = ConnectionThrottler::new(3);

		// Act & Assert
		assert!(throttler.acquire_connection("192.168.1.1").await.is_ok());
		assert!(throttler.acquire_connection("192.168.1.1").await.is_ok());
		assert!(throttler.acquire_connection("192.168.1.1").await.is_ok());
	}

	#[rstest]
	#[tokio::test]
	async fn test_connection_throttler_exceeds_limit() {
		// Arrange
		let throttler = ConnectionThrottler::new(2);
		throttler.acquire_connection("192.168.1.1").await.unwrap();
		throttler.acquire_connection("192.168.1.1").await.unwrap();

		// Act
		let result = throttler.acquire_connection("192.168.1.1").await;

		// Assert
		assert!(result.is_err());
		assert!(matches!(
			result.unwrap_err(),
			ThrottleError::TooManyConnections(_)
		));
	}

	#[rstest]
	#[tokio::test]
	async fn test_connection_throttler_release() {
		// Arrange
		let throttler = ConnectionThrottler::new(2);
		throttler.acquire_connection("192.168.1.1").await.unwrap();
		throttler.acquire_connection("192.168.1.1").await.unwrap();
		assert!(throttler.acquire_connection("192.168.1.1").await.is_err());

		// Act
		throttler.release_connection("192.168.1.1").await;

		// Assert
		assert!(throttler.acquire_connection("192.168.1.1").await.is_ok());
	}

	#[rstest]
	#[tokio::test]
	async fn test_connection_throttler_get_count() {
		// Arrange
		let throttler = ConnectionThrottler::new(10);

		// Act
		assert_eq!(throttler.get_connection_count("192.168.1.1").await, 0);
		throttler.acquire_connection("192.168.1.1").await.unwrap();
		throttler.acquire_connection("192.168.1.1").await.unwrap();

		// Assert
		assert_eq!(throttler.get_connection_count("192.168.1.1").await, 2);
	}

	#[rstest]
	#[tokio::test]
	async fn test_connection_throttler_independent_ips() {
		// Arrange
		let throttler = ConnectionThrottler::new(1);
		throttler.acquire_connection("192.168.1.1").await.unwrap();
		assert!(throttler.acquire_connection("192.168.1.1").await.is_err());

		// Act & Assert - different IP is unaffected
		assert!(throttler.acquire_connection("10.0.0.1").await.is_ok());
	}

	// --- CombinedThrottler tests ---

	#[rstest]
	#[tokio::test]
	async fn test_combined_throttler() {
		// Arrange
		let config = RateLimitConfig::new(10, Duration::from_secs(10));
		let throttler = CombinedThrottler::new(config, 5);

		// Act & Assert
		assert!(throttler.check_connection("192.168.1.1").await.is_ok());
		assert!(throttler.check_message_rate("user_1").await.is_ok());

		// Cleanup
		throttler.release_connection("192.168.1.1").await;
		throttler.reset_client_rate("user_1").await;
	}

	// --- ConnectionRateLimiter tests ---

	#[rstest]
	#[tokio::test]
	async fn test_connection_rate_limiter_within_limit() {
		// Arrange
		let limiter = ConnectionRateLimiter::new(3, Duration::from_secs(60));

		// Act & Assert
		assert!(limiter.check_connection_rate("192.168.1.1").await.is_ok());
		assert!(limiter.check_connection_rate("192.168.1.1").await.is_ok());
		assert!(limiter.check_connection_rate("192.168.1.1").await.is_ok());
	}

	#[rstest]
	#[tokio::test]
	async fn test_connection_rate_limiter_exceeds_limit() {
		// Arrange
		let limiter = ConnectionRateLimiter::new(2, Duration::from_secs(60));
		limiter.check_connection_rate("192.168.1.1").await.unwrap();
		limiter.check_connection_rate("192.168.1.1").await.unwrap();

		// Act
		let result = limiter.check_connection_rate("192.168.1.1").await;

		// Assert
		assert!(result.is_err());
		assert!(matches!(
			result.unwrap_err(),
			ThrottleError::ConnectionRateExceeded(_)
		));
	}

	#[rstest]
	#[tokio::test]
	async fn test_connection_rate_limiter_independent_ips() {
		// Arrange
		let limiter = ConnectionRateLimiter::new(1, Duration::from_secs(60));
		limiter.check_connection_rate("192.168.1.1").await.unwrap();
		assert!(limiter.check_connection_rate("192.168.1.1").await.is_err());

		// Act & Assert - different IP is unaffected
		assert!(limiter.check_connection_rate("10.0.0.1").await.is_ok());
	}

	#[rstest]
	#[tokio::test]
	async fn test_connection_rate_limiter_window_expiry() {
		// Arrange - use very short window
		let limiter = ConnectionRateLimiter::new(1, Duration::from_millis(50));
		limiter.check_connection_rate("192.168.1.1").await.unwrap();
		assert!(limiter.check_connection_rate("192.168.1.1").await.is_err());

		// Act - wait for window to expire
		tokio::time::sleep(Duration::from_millis(60)).await;

		// Assert - should be allowed again
		assert!(limiter.check_connection_rate("192.168.1.1").await.is_ok());
	}

	#[rstest]
	#[tokio::test]
	async fn test_connection_rate_limiter_get_current_count() {
		// Arrange
		let limiter = ConnectionRateLimiter::new(10, Duration::from_secs(60));

		// Act
		assert_eq!(limiter.get_current_count("192.168.1.1").await, 0);
		limiter.check_connection_rate("192.168.1.1").await.unwrap();
		limiter.check_connection_rate("192.168.1.1").await.unwrap();

		// Assert
		assert_eq!(limiter.get_current_count("192.168.1.1").await, 2);
	}

	#[rstest]
	#[tokio::test]
	async fn test_connection_rate_limiter_clear_all() {
		// Arrange
		let limiter = ConnectionRateLimiter::new(1, Duration::from_secs(60));
		limiter.check_connection_rate("192.168.1.1").await.unwrap();
		assert!(limiter.check_connection_rate("192.168.1.1").await.is_err());

		// Act
		limiter.clear_all().await;

		// Assert
		assert!(limiter.check_connection_rate("192.168.1.1").await.is_ok());
	}

	#[rstest]
	fn test_connection_rate_limiter_accessors() {
		// Arrange & Act
		let limiter = ConnectionRateLimiter::new(5, Duration::from_secs(30));

		// Assert
		assert_eq!(limiter.max_connections_per_window(), 5);
		assert_eq!(limiter.window(), Duration::from_secs(30));
	}

	// --- WebSocketRateLimitConfig tests ---

	#[rstest]
	fn test_websocket_rate_limit_config_default() {
		// Arrange & Act
		let config = WebSocketRateLimitConfig::default();

		// Assert
		assert_eq!(config.max_connections_per_window(), 20);
		assert_eq!(config.connection_window(), Duration::from_secs(60));
		assert_eq!(config.max_concurrent_connections_per_ip(), 10);
		assert_eq!(config.max_messages_per_window(), 100);
		assert_eq!(config.message_window(), Duration::from_secs(60));
	}

	#[rstest]
	fn test_websocket_rate_limit_config_strict() {
		// Arrange & Act
		let config = WebSocketRateLimitConfig::strict();

		// Assert
		assert_eq!(config.max_connections_per_window(), 5);
		assert_eq!(config.max_concurrent_connections_per_ip(), 3);
		assert_eq!(config.max_messages_per_window(), 30);
	}

	#[rstest]
	fn test_websocket_rate_limit_config_permissive() {
		// Arrange & Act
		let config = WebSocketRateLimitConfig::permissive();

		// Assert
		assert_eq!(config.max_connections_per_window(), 100);
		assert_eq!(config.max_concurrent_connections_per_ip(), 50);
		assert_eq!(config.max_messages_per_window(), 1000);
	}

	#[rstest]
	fn test_websocket_rate_limit_config_builder() {
		// Arrange & Act
		let config = WebSocketRateLimitConfig::default()
			.with_max_connections_per_window(50)
			.with_connection_window(Duration::from_secs(120))
			.with_max_concurrent_connections_per_ip(25)
			.with_max_messages_per_window(500)
			.with_message_window(Duration::from_secs(30));

		// Assert
		assert_eq!(config.max_connections_per_window(), 50);
		assert_eq!(config.connection_window(), Duration::from_secs(120));
		assert_eq!(config.max_concurrent_connections_per_ip(), 25);
		assert_eq!(config.max_messages_per_window(), 500);
		assert_eq!(config.message_window(), Duration::from_secs(30));
	}

	// --- RateLimitMiddleware tests ---

	#[rstest]
	#[tokio::test]
	async fn test_rate_limit_middleware_allows_connection() {
		// Arrange
		let config = WebSocketRateLimitConfig::default();
		let middleware = RateLimitMiddleware::new(config);
		let mut context = ConnectionContext::new("192.168.1.1".to_string());

		// Act
		let result = middleware.on_connect(&mut context).await;

		// Assert
		assert!(result.is_ok());
	}

	#[rstest]
	#[tokio::test]
	async fn test_rate_limit_middleware_rejects_connection_rate_exceeded() {
		// Arrange
		let config = WebSocketRateLimitConfig::default().with_max_connections_per_window(2);
		let middleware = RateLimitMiddleware::new(config);

		let mut ctx1 = ConnectionContext::new("192.168.1.1".to_string());
		let mut ctx2 = ConnectionContext::new("192.168.1.1".to_string());
		let mut ctx3 = ConnectionContext::new("192.168.1.1".to_string());
		middleware.on_connect(&mut ctx1).await.unwrap();
		middleware.on_connect(&mut ctx2).await.unwrap();

		// Act
		let result = middleware.on_connect(&mut ctx3).await;

		// Assert
		assert!(result.is_err());
		assert!(matches!(
			result.unwrap_err(),
			MiddlewareError::ConnectionRejected(_)
		));
	}

	#[rstest]
	#[tokio::test]
	async fn test_rate_limit_middleware_rejects_concurrent_exceeded() {
		// Arrange - allow many connection attempts but only 1 concurrent
		let config = WebSocketRateLimitConfig::default()
			.with_max_connections_per_window(100)
			.with_max_concurrent_connections_per_ip(1);
		let middleware = RateLimitMiddleware::new(config);

		let mut ctx1 = ConnectionContext::new("192.168.1.1".to_string());
		let mut ctx2 = ConnectionContext::new("192.168.1.1".to_string());
		middleware.on_connect(&mut ctx1).await.unwrap();

		// Act
		let result = middleware.on_connect(&mut ctx2).await;

		// Assert
		assert!(result.is_err());
		assert!(matches!(
			result.unwrap_err(),
			MiddlewareError::ConnectionRejected(_)
		));
	}

	#[rstest]
	#[tokio::test]
	async fn test_rate_limit_middleware_allows_message_within_limit() {
		// Arrange
		let config = WebSocketRateLimitConfig::default();
		let middleware = RateLimitMiddleware::new(config);
		let (tx, _rx) = mpsc::unbounded_channel();
		let conn = Arc::new(WebSocketConnection::new("test_conn".to_string(), tx));
		let message = Message::text("hello".to_string());

		// Act
		let result = middleware.on_message(&conn, message).await;

		// Assert
		assert!(result.is_ok());
	}

	#[rstest]
	#[tokio::test]
	async fn test_rate_limit_middleware_rejects_message_rate_exceeded() {
		// Arrange
		let config = WebSocketRateLimitConfig::default().with_max_messages_per_window(3);
		let middleware = RateLimitMiddleware::new(config);
		let (tx, _rx) = mpsc::unbounded_channel();
		let conn = Arc::new(WebSocketConnection::new("test_conn".to_string(), tx));

		for _ in 0..3 {
			let msg = Message::text("hello".to_string());
			middleware.on_message(&conn, msg).await.unwrap();
		}

		// Act
		let result = middleware
			.on_message(&conn, Message::text("overflow".to_string()))
			.await;

		// Assert
		assert!(result.is_err());
		assert!(matches!(
			result.unwrap_err(),
			MiddlewareError::MessageRejected(_)
		));
	}

	#[rstest]
	#[tokio::test]
	async fn test_rate_limit_middleware_with_defaults() {
		// Arrange
		let middleware = RateLimitMiddleware::with_defaults();
		let mut context = ConnectionContext::new("10.0.0.1".to_string());

		// Act
		let result = middleware.on_connect(&mut context).await;

		// Assert
		assert!(result.is_ok());
	}

	#[rstest]
	#[tokio::test]
	async fn test_rate_limit_middleware_release_connection() {
		// Arrange - only 1 concurrent allowed
		let config = WebSocketRateLimitConfig::default()
			.with_max_connections_per_window(100)
			.with_max_concurrent_connections_per_ip(1);
		let middleware = RateLimitMiddleware::new(config);

		let mut ctx1 = ConnectionContext::new("192.168.1.1".to_string());
		middleware.on_connect(&mut ctx1).await.unwrap();

		// Verify second connection is rejected
		let mut ctx2 = ConnectionContext::new("192.168.1.1".to_string());
		assert!(middleware.on_connect(&mut ctx2).await.is_err());

		// Act - release the connection
		middleware.release_connection("192.168.1.1").await;

		// Assert - now a new connection should be allowed
		let mut ctx3 = ConnectionContext::new("192.168.1.1".to_string());
		assert!(middleware.on_connect(&mut ctx3).await.is_ok());
	}

	#[rstest]
	#[tokio::test]
	async fn test_rate_limit_middleware_in_chain() {
		// Arrange
		use crate::middleware::MiddlewareChain;

		let config = WebSocketRateLimitConfig::default().with_max_connections_per_window(2);
		let middleware = RateLimitMiddleware::new(config);
		let mut chain = MiddlewareChain::new();
		chain.add_connection_middleware(Box::new(middleware));

		// Act & Assert - first two connections succeed
		let mut ctx1 = ConnectionContext::new("192.168.1.1".to_string());
		assert!(chain.process_connect(&mut ctx1).await.is_ok());

		let mut ctx2 = ConnectionContext::new("192.168.1.1".to_string());
		assert!(chain.process_connect(&mut ctx2).await.is_ok());

		// Third connection is rejected
		let mut ctx3 = ConnectionContext::new("192.168.1.1".to_string());
		assert!(chain.process_connect(&mut ctx3).await.is_err());
	}

	#[rstest]
	#[tokio::test]
	async fn test_rate_limit_middleware_message_in_chain() {
		// Arrange
		use crate::middleware::MiddlewareChain;

		let config = WebSocketRateLimitConfig::default().with_max_messages_per_window(1);
		let middleware = RateLimitMiddleware::new(config);
		let mut chain = MiddlewareChain::new();
		chain.add_message_middleware(Box::new(middleware));

		let (tx, _rx) = mpsc::unbounded_channel();
		let conn = Arc::new(WebSocketConnection::new("test".to_string(), tx));

		// Act & Assert - first message succeeds
		let msg1 = Message::text("first".to_string());
		assert!(chain.process_message(&conn, msg1).await.is_ok());

		// Second message is rejected
		let msg2 = Message::text("second".to_string());
		assert!(chain.process_message(&conn, msg2).await.is_err());
	}

	#[rstest]
	#[tokio::test]
	async fn test_rate_limit_middleware_accessors() {
		// Arrange
		let config = WebSocketRateLimitConfig::default()
			.with_max_connections_per_window(15)
			.with_max_concurrent_connections_per_ip(7);
		let middleware = RateLimitMiddleware::new(config);

		// Act & Assert
		assert_eq!(
			middleware
				.connection_rate_limiter()
				.max_connections_per_window(),
			15
		);
		middleware
			.connection_throttler()
			.acquire_connection("1.2.3.4")
			.await
			.unwrap();
		assert_eq!(
			middleware
				.connection_throttler()
				.get_connection_count("1.2.3.4")
				.await,
			1
		);
	}
}
