//! Rate Limit Redis Integration Tests
//!
//! Comprehensive integration tests for rate limiting with Redis and memory backends.
//! These tests verify rate limiting behavior, key strategies, and window management.
//!
//! # Test Categories
//!
//! - Happy path: Requests within rate limit allowed
//! - Error path: Rate limit exceeded, missing keys
//! - State transition: Window reset, limit recovery
//! - Edge cases: Boundary limits, concurrent access
//! - Decision table: Various key strategy and limit combinations

use bytes::Bytes;
use hyper::{HeaderMap, Method};
use reinhardt_auth::rate_limit_permission::RateLimitStrategy;
use reinhardt_auth::{Permission, PermissionContext, RateLimitPermission, SimpleUser};
use reinhardt_http::{Request, TrustedProxies};
use reinhardt_throttling::{MemoryBackend, ThrottleBackend};
use rstest::*;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use uuid::Uuid;

// =============================================================================
// Test Fixtures
// =============================================================================

/// Creates a memory backend for rate limiting
#[fixture]
fn memory_backend() -> Arc<MemoryBackend> {
	Arc::new(MemoryBackend::new())
}

/// Creates a rate limit permission with memory backend (5 requests per 60 seconds, IP-based)
#[fixture]
fn rate_limit_permission(memory_backend: Arc<MemoryBackend>) -> RateLimitPermission<MemoryBackend> {
	// 5 requests per 60 seconds = capacity 5.0, refill_rate = 5.0/60.0
	RateLimitPermission::new(memory_backend, RateLimitStrategy::PerIp, 5.0, 5.0 / 60.0)
}

/// Trusted proxy address used in test requests (127.0.0.1:8080)
const PROXY_IP: Ipv4Addr = Ipv4Addr::new(127, 0, 0, 1);
const PROXY_PORT: u16 = 8080;

/// Creates a test request with specified IP in X-Forwarded-For header.
///
/// Configures trusted proxies so `get_client_ip()` reads proxy headers.
fn create_request_with_ip(ip: &str) -> Request {
	let mut headers = HeaderMap::new();
	headers.insert("X-Forwarded-For", ip.parse().unwrap());

	let proxy_addr = SocketAddr::new(IpAddr::V4(PROXY_IP), PROXY_PORT);
	let trusted = TrustedProxies::new(vec![IpAddr::V4(PROXY_IP)]);

	let request = Request::builder()
		.method(Method::GET)
		.uri("/api/test")
		.headers(headers)
		.remote_addr(proxy_addr)
		.body(Bytes::new())
		.build()
		.unwrap();
	request.set_trusted_proxies(trusted);
	request
}

/// Creates a test request with X-Real-IP header.
///
/// Configures trusted proxies so `get_client_ip()` reads proxy headers.
fn create_request_with_real_ip(ip: &str) -> Request {
	let mut headers = HeaderMap::new();
	headers.insert("X-Real-IP", ip.parse().unwrap());

	let proxy_addr = SocketAddr::new(IpAddr::V4(PROXY_IP), PROXY_PORT);
	let trusted = TrustedProxies::new(vec![IpAddr::V4(PROXY_IP)]);

	let request = Request::builder()
		.method(Method::GET)
		.uri("/api/test")
		.headers(headers)
		.remote_addr(proxy_addr)
		.body(Bytes::new())
		.build()
		.unwrap();
	request.set_trusted_proxies(trusted);
	request
}

/// Creates a basic test request without IP headers or remote address.
///
/// Used for tests where IP extraction is not needed (e.g., `PerUser`,
/// `PerRoute` strategies) or where the absence of IP is tested explicitly.
fn create_basic_request() -> Request {
	Request::builder()
		.method(Method::GET)
		.uri("/api/test")
		.body(Bytes::new())
		.build()
		.unwrap()
}

/// Creates a test user
fn create_test_user(username: &str) -> SimpleUser {
	SimpleUser {
		id: Uuid::now_v7(),
		username: username.to_string(),
		email: format!("{}@example.com", username),
		is_active: true,
		is_admin: false,
		is_staff: false,
		is_superuser: false,
	}
}

// =============================================================================
// Happy Path Tests
// =============================================================================

#[rstest]
#[tokio::test]
async fn test_requests_allowed_within_limit(memory_backend: Arc<MemoryBackend>) {
	let config_rate = 5;
	let config_window = 60;
	// Create permission with new API: capacity = rate, refill_rate = rate / window
	let permission = RateLimitPermission::new(
		memory_backend,
		RateLimitStrategy::PerIp,
		config_rate as f64,
		(config_rate as f64) / (config_window as f64),
	);

	let request = create_request_with_ip("192.168.1.100");
	let context = PermissionContext {
		request: &request,
		is_authenticated: false,
		is_admin: false,
		is_active: false,
		user: None,
	};

	// First 5 requests should be allowed
	for i in 1..=5 {
		assert!(
			permission.has_permission(&context).await,
			"Request {} should be allowed (within limit of 5)",
			i
		);
	}
}

#[rstest]
#[tokio::test]
async fn test_different_ips_have_separate_limits(memory_backend: Arc<MemoryBackend>) {
	let config_rate = 2;
	let config_window = 60;
	// Create permission with new API: capacity = rate, refill_rate = rate / window
	let permission = RateLimitPermission::new(
		memory_backend,
		RateLimitStrategy::PerIp,
		config_rate as f64,
		(config_rate as f64) / (config_window as f64),
	);

	// IP 1: Use up the limit
	let request1 = create_request_with_ip("10.0.0.1");
	let context1 = PermissionContext {
		request: &request1,
		is_authenticated: false,
		is_admin: false,
		is_active: false,
		user: None,
	};

	assert!(permission.has_permission(&context1).await);
	assert!(permission.has_permission(&context1).await);
	assert!(
		!permission.has_permission(&context1).await,
		"IP1 should be rate limited"
	);

	// IP 2: Should have its own fresh limit
	let request2 = create_request_with_ip("10.0.0.2");
	let context2 = PermissionContext {
		request: &request2,
		is_authenticated: false,
		is_admin: false,
		is_active: false,
		user: None,
	};

	assert!(
		permission.has_permission(&context2).await,
		"IP2 should have its own limit"
	);
	assert!(permission.has_permission(&context2).await);
	assert!(
		!permission.has_permission(&context2).await,
		"IP2 should be rate limited"
	);
}

#[rstest]
#[tokio::test]
async fn test_user_id_strategy(memory_backend: Arc<MemoryBackend>) {
	let config_rate = 3;
	let config_window = 60;
	// Create permission with new API: capacity = rate, refill_rate = rate / window
	let permission = RateLimitPermission::new(
		memory_backend,
		RateLimitStrategy::PerUser,
		config_rate as f64,
		(config_rate as f64) / (config_window as f64),
	);

	let request = create_basic_request();
	let user = create_test_user("alice");

	let context = PermissionContext {
		request: &request,
		is_authenticated: true,
		is_admin: false,
		is_active: true,
		user: Some(Box::new(user)),
	};

	// First 3 requests allowed
	assert!(permission.has_permission(&context).await);
	assert!(permission.has_permission(&context).await);
	assert!(permission.has_permission(&context).await);
	// 4th request denied
	assert!(!permission.has_permission(&context).await);
}

// test_config_getter removed - config() method no longer exists in new API

// =============================================================================
// Error Path Tests
// =============================================================================

#[rstest]
#[tokio::test]
async fn test_rate_limit_exceeded(memory_backend: Arc<MemoryBackend>) {
	let config_rate = 2;
	let config_window = 60;
	// Create permission with new API: capacity = rate, refill_rate = rate / window
	let permission = RateLimitPermission::new(
		memory_backend,
		RateLimitStrategy::PerIp,
		config_rate as f64,
		(config_rate as f64) / (config_window as f64),
	);

	let request = create_request_with_ip("172.16.0.1");
	let context = PermissionContext {
		request: &request,
		is_authenticated: false,
		is_admin: false,
		is_active: false,
		user: None,
	};

	// Use up the limit
	assert!(permission.has_permission(&context).await);
	assert!(permission.has_permission(&context).await);

	// Exceed the limit
	assert!(
		!permission.has_permission(&context).await,
		"3rd request should be denied"
	);
	assert!(
		!permission.has_permission(&context).await,
		"4th request should also be denied"
	);
}

#[rstest]
#[tokio::test]
async fn test_user_strategy_without_authentication(memory_backend: Arc<MemoryBackend>) {
	let config_rate = 10;
	let config_window = 60;
	// Create permission with new API: capacity = rate, refill_rate = rate / window
	let permission = RateLimitPermission::new(
		memory_backend,
		RateLimitStrategy::PerUser,
		config_rate as f64,
		(config_rate as f64) / (config_window as f64),
	);

	let request = create_basic_request();
	let context = PermissionContext {
		request: &request,
		is_authenticated: false,
		is_admin: false,
		is_active: false,
		user: None, // No user
	};

	// Should be denied because no user ID can be extracted
	assert!(
		!permission.has_permission(&context).await,
		"Unauthenticated user with UserId strategy should be denied"
	);
}

#[rstest]
#[tokio::test]
async fn test_ip_strategy_without_ip_headers(memory_backend: Arc<MemoryBackend>) {
	let config_rate = 5;
	let config_window = 60;
	// Create permission with new API: capacity = rate, refill_rate = rate / window
	let permission = RateLimitPermission::new(
		memory_backend,
		RateLimitStrategy::PerIp,
		config_rate as f64,
		(config_rate as f64) / (config_window as f64),
	);

	// Request without any IP headers or remote_addr
	let request = create_basic_request();
	let context = PermissionContext {
		request: &request,
		is_authenticated: false,
		is_admin: false,
		is_active: false,
		user: None,
	};

	// Should be denied because no IP can be extracted
	assert!(
		!permission.has_permission(&context).await,
		"Request without IP should be denied for IP strategy"
	);
}

#[rstest]
#[tokio::test]
async fn test_ip_and_user_strategy_partial_info(memory_backend: Arc<MemoryBackend>) {
	let config_rate = 5;
	let config_window = 60;
	// Create permission with new API: capacity = rate, refill_rate = rate / window
	let permission = RateLimitPermission::new(
		memory_backend,
		RateLimitStrategy::PerIpAndUser,
		config_rate as f64,
		(config_rate as f64) / (config_window as f64),
	);

	// Request with IP but no user
	let request = create_request_with_ip("192.168.1.1");
	let context = PermissionContext {
		request: &request,
		is_authenticated: false,
		is_admin: false,
		is_active: false,
		user: None,
	};

	// Should be denied because IpAndUser requires both
	assert!(
		!permission.has_permission(&context).await,
		"IpAndUser strategy requires both IP and user"
	);
}

// =============================================================================
// State Transition Tests
// =============================================================================

#[rstest]
#[tokio::test]
async fn test_multiple_users_independent_limits(memory_backend: Arc<MemoryBackend>) {
	let config_rate = 2;
	let config_window = 60;
	// Create permission with new API: capacity = rate, refill_rate = rate / window
	let permission = RateLimitPermission::new(
		memory_backend,
		RateLimitStrategy::PerUser,
		config_rate as f64,
		(config_rate as f64) / (config_window as f64),
	);

	let request = create_basic_request();

	// User Alice
	let alice = create_test_user("alice");
	let context_alice = PermissionContext {
		request: &request,
		is_authenticated: true,
		is_admin: false,
		is_active: true,
		user: Some(Box::new(alice)),
	};

	// Alice uses her limit
	assert!(permission.has_permission(&context_alice).await);
	assert!(permission.has_permission(&context_alice).await);
	assert!(
		!permission.has_permission(&context_alice).await,
		"Alice should be limited"
	);

	// User Bob has fresh limit
	let bob = create_test_user("bob");
	let context_bob = PermissionContext {
		request: &request,
		is_authenticated: true,
		is_admin: false,
		is_active: true,
		user: Some(Box::new(bob)),
	};

	assert!(
		permission.has_permission(&context_bob).await,
		"Bob should have fresh limit"
	);
	assert!(permission.has_permission(&context_bob).await);
	assert!(
		!permission.has_permission(&context_bob).await,
		"Bob should be limited"
	);
}

// test_scoped_rate_limits_are_independent removed - scope functionality not available in new API
// New API doesn't support scope configuration in builder

// =============================================================================
// Edge Cases Tests
// =============================================================================

#[rstest]
#[case(1, 1)] // 1 request per window
#[case(100, 60)] // 100 requests per minute
#[case(1000, 3600)] // 1000 requests per hour
#[tokio::test]
async fn test_various_rate_limits(
	memory_backend: Arc<MemoryBackend>,
	#[case] rate: usize,
	#[case] window: u64,
) {
	let permission = RateLimitPermission::new(
		memory_backend,
		RateLimitStrategy::PerIp,
		rate as f64,
		(rate as f64) / (window as f64),
	);

	let request = create_request_with_ip("192.168.1.100");
	let context = PermissionContext {
		request: &request,
		is_authenticated: false,
		is_admin: false,
		is_active: false,
		user: None,
	};

	// All requests up to limit should be allowed
	for i in 1..=rate {
		assert!(
			permission.has_permission(&context).await,
			"Request {} of {} should be allowed",
			i,
			rate
		);
	}

	// Next request should be denied
	assert!(
		!permission.has_permission(&context).await,
		"Request {} should be denied (over limit {})",
		rate + 1,
		rate
	);
}

#[rstest]
#[tokio::test]
async fn test_custom_key_strategy(memory_backend: Arc<MemoryBackend>) {
	let permission =
		RateLimitPermission::new(memory_backend, RateLimitStrategy::PerRoute, 2.0, 2.0 / 60.0)
			.with_custom_key(|ctx| {
				// Use a custom key based on User-Agent header
				ctx.request
					.headers
					.get("User-Agent")
					.and_then(|v| v.to_str().ok())
					.map(|s| s.to_string())
			});

	let mut headers = HeaderMap::new();
	headers.insert("User-Agent", "TestBot/1.0".parse().unwrap());

	let request = Request::builder()
		.method(Method::GET)
		.uri("/api/test")
		.headers(headers)
		.body(Bytes::new())
		.build()
		.unwrap();

	let context = PermissionContext {
		request: &request,
		is_authenticated: false,
		is_admin: false,
		is_active: false,
		user: None,
	};

	// Custom key strategy should work
	assert!(permission.has_permission(&context).await);
	assert!(permission.has_permission(&context).await);
	assert!(
		!permission.has_permission(&context).await,
		"Should be rate limited by User-Agent"
	);
}

#[rstest]
#[tokio::test]
async fn test_x_forwarded_for_with_multiple_ips(memory_backend: Arc<MemoryBackend>) {
	let config_rate = 2;
	let config_window = 60;
	// Create permission with new API: capacity = rate, refill_rate = rate / window
	let permission = RateLimitPermission::new(
		memory_backend,
		RateLimitStrategy::PerIp,
		config_rate as f64,
		(config_rate as f64) / (config_window as f64),
	);

	// X-Forwarded-For with multiple IPs (should use first one)
	let mut headers = HeaderMap::new();
	headers.insert(
		"X-Forwarded-For",
		"203.0.113.195, 70.41.3.18, 150.172.238.178"
			.parse()
			.unwrap(),
	);

	let proxy_addr = SocketAddr::new(IpAddr::V4(PROXY_IP), PROXY_PORT);
	let trusted = TrustedProxies::new(vec![IpAddr::V4(PROXY_IP)]);

	let request = Request::builder()
		.method(Method::GET)
		.uri("/api/test")
		.headers(headers)
		.remote_addr(proxy_addr)
		.body(Bytes::new())
		.build()
		.unwrap();
	request.set_trusted_proxies(trusted);

	let context = PermissionContext {
		request: &request,
		is_authenticated: false,
		is_admin: false,
		is_active: false,
		user: None,
	};

	// Should extract first IP (203.0.113.195)
	assert!(permission.has_permission(&context).await);
	assert!(permission.has_permission(&context).await);
	assert!(!permission.has_permission(&context).await);
}

#[rstest]
#[tokio::test]
async fn test_x_real_ip_header_extraction(memory_backend: Arc<MemoryBackend>) {
	let config_rate = 2;
	let config_window = 60;
	// Create permission with new API: capacity = rate, refill_rate = rate / window
	let permission = RateLimitPermission::new(
		memory_backend,
		RateLimitStrategy::PerIp,
		config_rate as f64,
		(config_rate as f64) / (config_window as f64),
	);

	let request = create_request_with_real_ip("198.51.100.42");
	let context = PermissionContext {
		request: &request,
		is_authenticated: false,
		is_admin: false,
		is_active: false,
		user: None,
	};

	assert!(permission.has_permission(&context).await);
	assert!(permission.has_permission(&context).await);
	assert!(!permission.has_permission(&context).await);
}

#[rstest]
#[tokio::test]
async fn test_ip_and_user_combined_strategy(memory_backend: Arc<MemoryBackend>) {
	let config_rate = 2;
	let config_window = 60;
	// Create permission with new API: capacity = rate, refill_rate = rate / window
	let permission = RateLimitPermission::new(
		memory_backend,
		RateLimitStrategy::PerIpAndUser,
		config_rate as f64,
		(config_rate as f64) / (config_window as f64),
	);

	let request = create_request_with_ip("192.168.1.1");
	let user = create_test_user("combined_user");

	let context = PermissionContext {
		request: &request,
		is_authenticated: true,
		is_admin: false,
		is_active: true,
		user: Some(Box::new(user)),
	};

	// Should work with combined key
	assert!(permission.has_permission(&context).await);
	assert!(permission.has_permission(&context).await);
	assert!(!permission.has_permission(&context).await);
}

// =============================================================================
// Decision Table Tests
// =============================================================================

#[rstest]
#[case(RateLimitStrategy::PerIp, true, false, true)] // IP + no user = key from IP
#[case(RateLimitStrategy::PerUser, false, true, true)] // User + no IP = key from user
#[case(RateLimitStrategy::PerIpAndUser, true, true, true)] // Both = combined key
#[case(RateLimitStrategy::PerIp, false, false, false)] // No IP = denied
#[case(RateLimitStrategy::PerUser, true, false, false)] // User strategy but no user = denied
#[case(RateLimitStrategy::PerIpAndUser, true, false, false)] // IpAndUser missing user = denied
#[case(RateLimitStrategy::PerIpAndUser, false, true, false)] // IpAndUser missing IP = denied
#[tokio::test]
async fn test_key_strategy_decision_table(
	memory_backend: Arc<MemoryBackend>,
	#[case] strategy: RateLimitStrategy,
	#[case] has_ip: bool,
	#[case] has_user: bool,
	#[case] expected_first_allowed: bool,
) {
	let permission = RateLimitPermission::new(memory_backend, strategy, 5.0, 5.0 / 60.0);

	// Create request with or without IP
	let request = if has_ip {
		create_request_with_ip("192.168.1.100")
	} else {
		create_basic_request()
	};

	// Create context with or without user
	let user = if has_user {
		Some(Box::new(create_test_user("test_user")) as Box<SimpleUser>)
	} else {
		None
	};

	let context = PermissionContext {
		request: &request,
		is_authenticated: has_user,
		is_admin: false,
		is_active: has_user,
		user: user.map(|u| u as Box<dyn reinhardt_auth::User>),
	};

	let result = permission.has_permission(&context).await;
	assert_eq!(
		result,
		expected_first_allowed,
		"Strategy {:?} with IP={}, User={} should {} first request",
		strategy,
		has_ip,
		has_user,
		if expected_first_allowed {
			"allow"
		} else {
			"deny"
		}
	);
}

// =============================================================================
// Builder Pattern Tests
// =============================================================================

// test_config_builder removed - RateLimitConfig::builder() no longer exists in new API
// New API uses RateLimitPermission::builder() instead

#[rstest]
#[tokio::test]
async fn test_permission_builder(memory_backend: Arc<MemoryBackend>) {
	let permission = RateLimitPermission::builder()
		.backend(memory_backend)
		.strategy(RateLimitStrategy::PerIp)
		.capacity(5.0)
		.refill_rate(5.0 / 60.0)
		.build();

	// Should work correctly
	let request = create_request_with_ip("192.168.1.1");
	let context = PermissionContext {
		request: &request,
		is_authenticated: false,
		is_admin: false,
		is_active: false,
		user: None,
	};

	assert!(permission.has_permission(&context).await);
}

// =============================================================================
// Concurrent Access Tests
// =============================================================================

#[rstest]
#[tokio::test]
async fn test_concurrent_requests_same_ip(memory_backend: Arc<MemoryBackend>) {
	use std::sync::atomic::{AtomicUsize, Ordering};
	use tokio::sync::Barrier;

	let permission = Arc::new(RateLimitPermission::new(
		memory_backend,
		RateLimitStrategy::PerIp,
		5.0,
		5.0 / 60.0,
	));

	let allowed_count = Arc::new(AtomicUsize::new(0));
	let barrier = Arc::new(Barrier::new(10));

	let mut handles = vec![];

	for _ in 0..10 {
		let perm = Arc::clone(&permission);
		let count = Arc::clone(&allowed_count);
		let bar = Arc::clone(&barrier);

		handles.push(tokio::spawn(async move {
			bar.wait().await;

			let request = create_request_with_ip("192.168.1.1");
			let context = PermissionContext {
				request: &request,
				is_authenticated: false,
				is_admin: false,
				is_active: false,
				user: None,
			};

			if perm.has_permission(&context).await {
				count.fetch_add(1, Ordering::SeqCst);
			}
		}));
	}

	for handle in handles {
		handle.await.unwrap();
	}

	// Should have allowed exactly 5 requests
	let total_allowed = allowed_count.load(Ordering::SeqCst);
	assert_eq!(
		total_allowed, 5,
		"Expected 5 requests allowed, got {}",
		total_allowed
	);
}

// =============================================================================
// Backend Tests
// =============================================================================

#[rstest]
#[tokio::test]
async fn test_memory_backend_increment() {
	let backend = MemoryBackend::new();

	// First increment
	let count1 = backend.increment("test_key", 60).await.unwrap();
	assert_eq!(count1, 1, "First increment should return 1");

	// Second increment
	let count2 = backend.increment("test_key", 60).await.unwrap();
	assert_eq!(count2, 2, "Second increment should return 2");

	// Get count
	let count = backend.get_count("test_key").await.unwrap();
	assert_eq!(count, 2, "Count should be 2");
}

#[rstest]
#[tokio::test]
async fn test_memory_backend_get_nonexistent_key() {
	let backend = MemoryBackend::new();

	let count = backend.get_count("nonexistent").await.unwrap();
	assert_eq!(count, 0, "Nonexistent key should return 0");
}

#[rstest]
#[tokio::test]
async fn test_memory_backend_separate_keys() {
	let backend = MemoryBackend::new();

	backend.increment("key1", 60).await.unwrap();
	backend.increment("key1", 60).await.unwrap();
	backend.increment("key2", 60).await.unwrap();

	let count1 = backend.get_count("key1").await.unwrap();
	let count2 = backend.get_count("key2").await.unwrap();

	assert_eq!(count1, 2);
	assert_eq!(count2, 1);
}

// =============================================================================
// Sanity Tests
// =============================================================================

#[rstest]
fn test_rate_limit_key_strategy_clone() {
	let strategy = RateLimitStrategy::PerIp;
	let cloned = strategy;

	assert_eq!(strategy, cloned);
}

#[rstest]
fn test_rate_limit_key_strategy_debug() {
	let strategy = RateLimitStrategy::PerUser;
	let debug_str = format!("{:?}", strategy);

	assert!(debug_str.contains("PerUser"));
}

// test_rate_limit_config_clone removed - RateLimitConfig no longer exists in new API

// test_rate_limit_config_debug removed - RateLimitConfig no longer exists in new API
