//! Rate limiting permission for IP/user-based request control
//!
//! Provides permission checking based on rate limits, integrating with
//! the throttling backend for distributed rate limiting support.

// This module uses the deprecated User trait for backward compatibility.
// RateLimitPermission reads user.id() from PermissionContext which holds Box<dyn User>.
#![allow(deprecated)]
use crate::{Permission, PermissionContext};
use async_trait::async_trait;
pub use reinhardt_core::RateLimitStrategy;
use reinhardt_throttling::ThrottleBackend;
use std::sync::Arc;

// Type alias to simplify custom key extraction function signature
/// Custom key extraction function that takes a PermissionContext and returns an optional key string
pub type CustomKeyFn = Arc<dyn Fn(&PermissionContext) -> Option<String> + Send + Sync>;

/// Internal configuration for rate limiting permission
#[derive(Debug, Clone)]
struct RateLimitPermissionConfig {
	/// Maximum number of requests allowed
	rate: usize,
	/// Time window in seconds
	window: u64,
	/// Key generation strategy
	strategy: RateLimitStrategy,
	/// Allow requests on backend errors (fail-open)
	allow_on_error: bool,
	/// Scope identifier for namespacing rate limits
	scope: Option<String>,
}

/// Rate limiting permission
///
/// Checks if a request should be allowed based on rate limits.
/// Integrates with throttling backends for distributed rate limiting.
///
/// # Examples
///
/// ```
/// use reinhardt_auth::rate_limit_permission::{RateLimitPermission, RateLimitStrategy};
/// use reinhardt_throttling::MemoryBackend;
/// use std::sync::Arc;
///
/// let backend = Arc::new(MemoryBackend::new());
/// let permission = RateLimitPermission::new(
///     backend,
///     RateLimitStrategy::PerIp,
///     100.0,
///     1.0
/// );
///
/// // Permission will now enforce rate limits per IP
/// ```
pub struct RateLimitPermission<B: ThrottleBackend> {
	backend: Arc<B>,
	config: RateLimitPermissionConfig,
	custom_key_fn: Option<CustomKeyFn>,
}

impl<B: ThrottleBackend> RateLimitPermission<B> {
	/// Creates a new rate limit permission
	///
	/// # Arguments
	///
	/// * `backend` - The throttle backend for distributed rate limiting
	/// * `strategy` - Rate limiting strategy (PerIp, PerUser, etc.)
	/// * `capacity` - Maximum number of tokens (requests)
	/// * `refill_rate` - Rate at which tokens are refilled (per second)
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_auth::rate_limit_permission::{RateLimitPermission, RateLimitStrategy};
	/// use reinhardt_throttling::MemoryBackend;
	/// use std::sync::Arc;
	///
	/// let backend = Arc::new(MemoryBackend::new());
	/// let permission = RateLimitPermission::new(
	///     backend,
	///     RateLimitStrategy::PerUser,
	///     1000.0,
	///     1.0
	/// );
	/// ```
	pub fn new(
		backend: Arc<B>,
		strategy: RateLimitStrategy,
		capacity: f64,
		refill_rate: f64,
	) -> Self {
		// Convert capacity/refill_rate to rate/window
		let rate = capacity as usize;
		let window = (capacity / refill_rate).max(1.0) as u64;

		Self {
			backend,
			config: RateLimitPermissionConfig {
				rate,
				window,
				strategy,
				allow_on_error: false,
				scope: None,
			},
			custom_key_fn: None,
		}
	}

	/// Creates a builder for fluent configuration
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_auth::rate_limit_permission::{RateLimitPermission, RateLimitStrategy};
	/// use reinhardt_throttling::MemoryBackend;
	/// use std::sync::Arc;
	///
	/// let backend = Arc::new(MemoryBackend::new());
	///
	/// let permission = RateLimitPermission::builder()
	///     .backend(backend)
	///     .strategy(RateLimitStrategy::PerIp)
	///     .capacity(100.0)
	///     .refill_rate(1.0)
	///     .build();
	/// ```
	pub fn builder() -> RateLimitPermissionBuilder<B> {
		RateLimitPermissionBuilder {
			backend: None,
			strategy: None,
			capacity: None,
			refill_rate: None,
			custom_key_fn: None,
		}
	}

	/// Set custom key extraction function
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_auth::rate_limit_permission::{RateLimitPermission, RateLimitStrategy};
	/// use reinhardt_throttling::MemoryBackend;
	/// use std::sync::Arc;
	///
	/// let backend = Arc::new(MemoryBackend::new());
	///
	/// let permission = RateLimitPermission::new(
	///     backend,
	///     RateLimitStrategy::PerRoute,
	///     100.0,
	///     1.0
	/// )
	///     .with_custom_key(|ctx| {
	///         // Extract custom key from request
	///         Some("custom_key".to_string())
	///     });
	/// ```
	pub fn with_custom_key<F>(mut self, f: F) -> Self
	where
		F: Fn(&PermissionContext) -> Option<String> + Send + Sync + 'static,
	{
		self.custom_key_fn = Some(Arc::new(f));
		self
	}

	/// Extract IP address from request
	///
	/// Delegates to `Request::get_client_ip()` which only trusts proxy headers
	/// (X-Forwarded-For, X-Real-IP) when the request originates from a configured
	/// trusted proxy. Falls back to the actual connection IP otherwise.
	fn extract_ip(&self, context: &PermissionContext) -> Option<String> {
		context.request.get_client_ip().map(|ip| ip.to_string())
	}

	/// Extract user ID from context
	fn extract_user_id(&self, context: &PermissionContext) -> Option<String> {
		context.user.as_ref().map(|user| user.id())
	}

	/// Generate rate limit key based on strategy
	///
	/// If a custom key function is configured, it takes priority over
	/// the built-in strategy. The built-in strategy is only used as a
	/// fallback when no custom key function is set.
	fn generate_key(&self, context: &PermissionContext) -> Option<String> {
		// Check custom key function first; fall back to built-in strategy
		let base_key = if let Some(ref custom_fn) = self.custom_key_fn {
			custom_fn(context)
		} else {
			match self.config.strategy {
				RateLimitStrategy::PerIp => self.extract_ip(context),
				RateLimitStrategy::PerUser => self.extract_user_id(context),
				RateLimitStrategy::PerIpAndUser => {
					if let (Some(ip), Some(user_id)) =
						(self.extract_ip(context), self.extract_user_id(context))
					{
						Some(format!("{}:{}", ip, user_id))
					} else {
						None
					}
				}
				RateLimitStrategy::PerRoute => {
					// Use request path as key
					Some(context.request.uri.path().to_string())
				}
			}
		};

		// Add scope prefix if configured
		base_key.map(|key| {
			if let Some(ref scope) = self.config.scope {
				format!("{}:{}", scope, key)
			} else {
				key
			}
		})
	}
}

/// Builder for RateLimitPermission
pub struct RateLimitPermissionBuilder<B: ThrottleBackend> {
	backend: Option<Arc<B>>,
	strategy: Option<RateLimitStrategy>,
	capacity: Option<f64>,
	refill_rate: Option<f64>,
	custom_key_fn: Option<CustomKeyFn>,
}

impl<B: ThrottleBackend> RateLimitPermissionBuilder<B> {
	/// Set the throttle backend
	pub fn backend(mut self, backend: Arc<B>) -> Self {
		self.backend = Some(backend);
		self
	}

	/// Set the rate limiting strategy
	pub fn strategy(mut self, strategy: RateLimitStrategy) -> Self {
		self.strategy = Some(strategy);
		self
	}

	/// Set the bucket capacity
	pub fn capacity(mut self, capacity: f64) -> Self {
		self.capacity = Some(capacity);
		self
	}

	/// Set the token refill rate
	pub fn refill_rate(mut self, refill_rate: f64) -> Self {
		self.refill_rate = Some(refill_rate);
		self
	}

	/// Set custom key extraction function
	pub fn custom_key<F>(mut self, f: F) -> Self
	where
		F: Fn(&PermissionContext) -> Option<String> + Send + Sync + 'static,
	{
		self.custom_key_fn = Some(Arc::new(f));
		self
	}

	/// Build the permission
	///
	/// # Panics
	///
	/// Panics if backend, strategy, capacity, or refill_rate are not set
	pub fn build(self) -> RateLimitPermission<B> {
		let capacity = self.capacity.expect("capacity must be set");
		let refill_rate = self.refill_rate.expect("refill_rate must be set");
		let strategy = self.strategy.expect("strategy must be set");

		let rate = capacity as usize;
		let window = (capacity / refill_rate).max(1.0) as u64;

		RateLimitPermission {
			backend: self.backend.expect("backend must be set"),
			config: RateLimitPermissionConfig {
				rate,
				window,
				strategy,
				allow_on_error: false,
				scope: None,
			},
			custom_key_fn: self.custom_key_fn,
		}
	}
}

#[async_trait]
impl<B: ThrottleBackend> Permission for RateLimitPermission<B> {
	async fn has_permission(&self, context: &PermissionContext<'_>) -> bool {
		// Generate rate limit key
		let key = match self.generate_key(context) {
			Some(k) => k,
			None => {
				// No key could be generated (e.g., unauthenticated user with UserId strategy)
				return false;
			}
		};

		// Check rate limit using backend
		match self.backend.increment(&key, self.config.window).await {
			Ok(count) => {
				// Allow if under rate limit
				count <= self.config.rate
			}
			Err(_) => {
				// On error, use configured fail-open/fail-closed behavior
				self.config.allow_on_error
			}
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use bytes::Bytes;
	use hyper::{HeaderMap, Method};
	use reinhardt_http::{Request, TrustedProxies};
	use reinhardt_throttling::MemoryBackend;
	use rstest::rstest;
	use std::net::{IpAddr, Ipv4Addr, SocketAddr};

	fn create_test_request(headers: HeaderMap) -> Request {
		Request::builder()
			.method(Method::GET)
			.uri("/test")
			.headers(headers)
			.body(Bytes::new())
			.build()
			.unwrap()
	}

	/// Create a test request with remote_addr set for IP-based tests
	fn create_test_request_with_addr(headers: HeaderMap, addr: SocketAddr) -> Request {
		Request::builder()
			.method(Method::GET)
			.uri("/test")
			.headers(headers)
			.remote_addr(addr)
			.body(Bytes::new())
			.build()
			.unwrap()
	}

	#[tokio::test]
	async fn test_rate_limit_permission_ip_strategy() {
		// Arrange - use remote_addr directly (no proxy headers)
		let backend = Arc::new(MemoryBackend::new());
		let permission = RateLimitPermission::new(backend, RateLimitStrategy::PerIp, 2.0, 1.0);

		let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 100)), 12345);
		let request = create_test_request_with_addr(HeaderMap::new(), addr);
		let context = PermissionContext {
			request: &request,
			is_authenticated: false,
			is_admin: false,
			is_active: false,
			user: None,
		};

		// Act & Assert
		// First two requests should be allowed
		assert!(permission.has_permission(&context).await);
		assert!(permission.has_permission(&context).await);

		// Third request should be denied
		assert!(!permission.has_permission(&context).await);
	}

	#[rstest::rstest]
	#[tokio::test]
	async fn test_rate_limit_permission_ip_strategy_trusted_proxy() {
		// Arrange - X-Forwarded-For from a trusted proxy
		let backend = Arc::new(MemoryBackend::new());
		let permission = RateLimitPermission::new(backend, RateLimitStrategy::PerIp, 2.0, 1.0);

		let proxy_ip = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));
		let proxy_addr = SocketAddr::new(proxy_ip, 8080);

		let mut headers = HeaderMap::new();
		headers.insert("X-Forwarded-For", "192.168.1.100".parse().unwrap());

		let request = create_test_request_with_addr(headers, proxy_addr);
		request.set_trusted_proxies(TrustedProxies::new(vec![proxy_ip]));

		let context = PermissionContext {
			request: &request,
			is_authenticated: false,
			is_admin: false,
			is_active: false,
			user: None,
		};

		// Act & Assert - uses 192.168.1.100 from X-Forwarded-For (trusted proxy)
		assert!(permission.has_permission(&context).await);
		assert!(permission.has_permission(&context).await);
		assert!(!permission.has_permission(&context).await);
	}

	#[rstest::rstest]
	#[tokio::test]
	async fn test_rate_limit_permission_ip_strategy_untrusted_proxy_header_ignored() {
		// Arrange - X-Forwarded-For from an UNTRUSTED source
		let backend = Arc::new(MemoryBackend::new());
		let permission = RateLimitPermission::new(backend, RateLimitStrategy::PerIp, 2.0, 1.0);

		let actual_ip = IpAddr::V4(Ipv4Addr::new(203, 0, 113, 50));
		let actual_addr = SocketAddr::new(actual_ip, 12345);

		let mut headers = HeaderMap::new();
		// Attacker spoofs X-Forwarded-For to bypass rate limiting
		headers.insert("X-Forwarded-For", "1.2.3.4".parse().unwrap());

		let request = create_test_request_with_addr(headers, actual_addr);
		// No trusted proxies configured

		let context = PermissionContext {
			request: &request,
			is_authenticated: false,
			is_admin: false,
			is_active: false,
			user: None,
		};

		// Act & Assert - should use actual connection IP (203.0.113.50), NOT spoofed 1.2.3.4
		assert!(permission.has_permission(&context).await);
		assert!(permission.has_permission(&context).await);
		assert!(!permission.has_permission(&context).await);
	}

	#[tokio::test]
	async fn test_rate_limit_permission_user_strategy() {
		use crate::SimpleUser;
		use uuid::Uuid;

		let backend = Arc::new(MemoryBackend::new());
		let permission = RateLimitPermission::new(backend, RateLimitStrategy::PerUser, 3.0, 1.0);

		let headers = HeaderMap::new();
		let request = create_test_request(headers);

		let test_user = SimpleUser {
			id: Uuid::now_v7(),
			username: "testuser".to_string(),
			email: "test@example.com".to_string(),
			is_active: true,
			is_admin: false,
			is_staff: false,
			is_superuser: false,
		};

		let context = PermissionContext {
			request: &request,
			is_authenticated: true,
			is_admin: false,
			is_active: true,
			user: Some(Box::new(test_user)),
		};

		// First three requests should be allowed
		assert!(permission.has_permission(&context).await);
		assert!(permission.has_permission(&context).await);
		assert!(permission.has_permission(&context).await);

		// Fourth request should be denied
		assert!(!permission.has_permission(&context).await);
	}

	#[tokio::test]
	async fn test_rate_limit_permission_unauthenticated_user_strategy() {
		let backend = Arc::new(MemoryBackend::new());
		let permission = RateLimitPermission::new(backend, RateLimitStrategy::PerUser, 10.0, 1.0);

		let headers = HeaderMap::new();
		let request = create_test_request(headers);
		let context = PermissionContext {
			request: &request,
			is_authenticated: false,
			is_admin: false,
			is_active: false,
			user: None,
		};

		// Should be denied for unauthenticated users
		assert!(!permission.has_permission(&context).await);
	}

	#[tokio::test]
	async fn test_rate_limit_permission_custom_strategy() {
		let backend = Arc::new(MemoryBackend::new());
		let permission = RateLimitPermission::new(backend, RateLimitStrategy::PerRoute, 2.0, 1.0)
			.with_custom_key(|_ctx| Some("custom_key".to_string()));

		let headers = HeaderMap::new();
		let request = create_test_request(headers);
		let context = PermissionContext {
			request: &request,
			is_authenticated: false,
			is_admin: false,
			is_active: false,
			user: None,
		};

		// First two requests should be allowed
		assert!(permission.has_permission(&context).await);
		assert!(permission.has_permission(&context).await);

		// Third request should be denied
		assert!(!permission.has_permission(&context).await);
	}

	#[tokio::test]
	async fn test_rate_limit_strategy_equality() {
		assert_eq!(RateLimitStrategy::PerIp, RateLimitStrategy::PerIp);
		assert_ne!(RateLimitStrategy::PerIp, RateLimitStrategy::PerUser);
	}

	#[tokio::test]
	async fn test_rate_limit_permission_builder() {
		let backend = Arc::new(MemoryBackend::new());

		let _permission = RateLimitPermission::builder()
			.backend(backend)
			.strategy(RateLimitStrategy::PerIp)
			.capacity(5.0)
			.refill_rate(1.0)
			.build();

		// Successfully built
	}

	#[tokio::test]
	async fn test_rate_limit_permission_with_scope() {
		// Arrange - use remote_addr directly
		let backend = Arc::new(MemoryBackend::new());
		let permission = RateLimitPermission::new(backend, RateLimitStrategy::PerIp, 2.0, 1.0);

		let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)), 12345);
		let request = create_test_request_with_addr(HeaderMap::new(), addr);
		let context = PermissionContext {
			request: &request,
			is_authenticated: false,
			is_admin: false,
			is_active: false,
			user: None,
		};

		// Act & Assert
		assert!(permission.has_permission(&context).await);
		assert!(permission.has_permission(&context).await);
		assert!(!permission.has_permission(&context).await);
	}

	#[tokio::test]
	async fn test_rate_limit_permission_x_real_ip_header() {
		// Arrange - X-Real-IP from trusted proxy
		let backend = Arc::new(MemoryBackend::new());
		let permission = RateLimitPermission::new(backend, RateLimitStrategy::PerIp, 1.0, 1.0);

		let proxy_ip = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));
		let proxy_addr = SocketAddr::new(proxy_ip, 8080);

		let mut headers = HeaderMap::new();
		headers.insert("X-Real-IP", "172.16.0.1".parse().unwrap());

		let request = create_test_request_with_addr(headers, proxy_addr);
		request.set_trusted_proxies(TrustedProxies::new(vec![proxy_ip]));

		let context = PermissionContext {
			request: &request,
			is_authenticated: false,
			is_admin: false,
			is_active: false,
			user: None,
		};

		// Act & Assert
		// First request allowed
		assert!(permission.has_permission(&context).await);
		// Second request denied
		assert!(!permission.has_permission(&context).await);
	}

	#[rstest]
	#[tokio::test]
	async fn test_custom_key_fn_takes_priority_over_builtin_strategy() {
		// Arrange
		let backend = Arc::new(MemoryBackend::new());
		// Use PerIp strategy as the built-in, but override with custom key fn
		let permission = RateLimitPermission::new(backend, RateLimitStrategy::PerIp, 2.0, 1.0)
			.with_custom_key(|_ctx| Some("my_custom_key".to_string()));

		// Create two requests with different IPs -- they should share the
		// same rate limit because the custom key function returns a fixed key
		let mut headers_a = HeaderMap::new();
		headers_a.insert("X-Forwarded-For", "10.0.0.1".parse().unwrap());
		let request_a = create_test_request(headers_a);
		let ctx_a = PermissionContext {
			request: &request_a,
			is_authenticated: false,
			is_admin: false,
			is_active: false,
			user: None,
		};

		let mut headers_b = HeaderMap::new();
		headers_b.insert("X-Forwarded-For", "10.0.0.2".parse().unwrap());
		let request_b = create_test_request(headers_b);
		let ctx_b = PermissionContext {
			request: &request_b,
			is_authenticated: false,
			is_admin: false,
			is_active: false,
			user: None,
		};

		// Act & Assert
		// Both IPs share a single bucket via the custom key
		assert!(permission.has_permission(&ctx_a).await);
		assert!(permission.has_permission(&ctx_b).await);

		// Third request from either IP should be denied (shared limit of 2)
		assert!(!permission.has_permission(&ctx_a).await);
	}

	#[rstest]
	#[tokio::test]
	async fn test_custom_key_fn_returning_none_denies_request() {
		// Arrange
		let backend = Arc::new(MemoryBackend::new());
		let permission = RateLimitPermission::new(backend, RateLimitStrategy::PerIp, 10.0, 1.0)
			.with_custom_key(|_ctx| None);

		let headers = HeaderMap::new();
		let request = create_test_request(headers);
		let context = PermissionContext {
			request: &request,
			is_authenticated: false,
			is_admin: false,
			is_active: false,
			user: None,
		};

		// Act & Assert
		// Custom key fn returns None, so request should be denied
		assert!(!permission.has_permission(&context).await);
	}
}
