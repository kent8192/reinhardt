//! Router integration tests
//!
//! Tests for DeeplinkRouter and DeeplinkRouterExt covering:
//! - Happy path: Router creation, configuration access
//! - Extension trait: UnifiedRouter and ServerRouter integration
//! - Edge cases: Empty config, single platform configs
//! - Sanity: Debug format, output types

use reinhardt_deeplink::{
	AndroidConfig, DeeplinkConfig, DeeplinkRouter, DeeplinkRouterExt, IosConfig,
};
use reinhardt_urls::routers::{ServerRouter, UnifiedRouter};
use rstest::*;

const VALID_APP_ID: &str = "TEAM123456.com.example.app";
const VALID_FINGERPRINT: &str = "FA:C6:17:45:DC:09:03:78:6F:B9:ED:E6:2A:96:2B:39:9F:73:48:F0:BB:6F:89:9B:83:32:66:75:91:03:3B:9C";

// Import fixtures
mod fixtures;
use fixtures::*;

// ============================================================================
// DeeplinkRouter Tests
// ============================================================================

#[rstest]
fn test_router_creation_ios_only() {
	let config = DeeplinkConfig::builder()
		.ios(
			IosConfig::builder()
				.app_id(VALID_APP_ID)
				.paths(&["/products/*"])
				.build(),
		)
		.build();

	let router = DeeplinkRouter::new(config).unwrap();
	assert!(router.config().has_ios());
	assert!(!router.config().has_android());
}

#[rstest]
fn test_router_creation_android_only() {
	let config = DeeplinkConfig::builder()
		.android(
			AndroidConfig::builder()
				.package_name("com.example.app")
				.sha256_fingerprint(VALID_FINGERPRINT)
				.build()
				.unwrap(),
		)
		.build();

	let router = DeeplinkRouter::new(config).unwrap();
	assert!(!router.config().has_ios());
	assert!(router.config().has_android());
}

#[rstest]
fn test_router_creation_both() {
	let config = DeeplinkConfig::builder()
		.ios(
			IosConfig::builder()
				.app_id(VALID_APP_ID)
				.paths(&["/products/*"])
				.build(),
		)
		.android(
			AndroidConfig::builder()
				.package_name("com.example.app")
				.sha256_fingerprint(VALID_FINGERPRINT)
				.build()
				.unwrap(),
		)
		.build();

	let router = DeeplinkRouter::new(config).unwrap();
	assert!(router.config().has_ios());
	assert!(router.config().has_android());
}

#[rstest]
fn test_router_creation_empty() {
	let config = DeeplinkConfig::default();
	let router = DeeplinkRouter::new(config);

	// Empty config should still create a valid router
	assert!(router.is_ok());
	assert!(!router.unwrap().config().is_configured());
}

#[rstest]
fn test_router_into_server() {
	let config = DeeplinkConfig::builder()
		.ios(
			IosConfig::builder()
				.app_id(VALID_APP_ID)
				.paths(&["/products/*"])
				.build(),
		)
		.build();

	let router = DeeplinkRouter::new(config).unwrap();
	let _server = router.into_server();
}

#[rstest]
fn test_router_config_accessor(#[from(deeplink_router)] router: DeeplinkRouter) {
	let config = router.config();
	assert!(config.is_configured());
}

// ============================================================================
// DeeplinkRouterExt Tests
// ============================================================================

#[rstest]
fn test_unified_router_with_deeplinks_ios() {
	let config = DeeplinkConfig::builder()
		.ios(
			IosConfig::builder()
				.app_id(VALID_APP_ID)
				.paths(&["/products/*"])
				.build(),
		)
		.build();

	let router = UnifiedRouter::new().with_deeplinks(config);
	assert!(router.is_ok());
}

#[rstest]
fn test_unified_router_with_deeplinks_android() {
	let config = DeeplinkConfig::builder()
		.android(
			AndroidConfig::builder()
				.package_name("com.example.app")
				.sha256_fingerprint(VALID_FINGERPRINT)
				.build()
				.unwrap(),
		)
		.build();

	let router = UnifiedRouter::new().with_deeplinks(config);
	assert!(router.is_ok());
}

#[rstest]
fn test_unified_router_with_deeplinks_both() {
	let config = DeeplinkConfig::builder()
		.ios(
			IosConfig::builder()
				.app_id(VALID_APP_ID)
				.paths(&["/products/*"])
				.build(),
		)
		.android(
			AndroidConfig::builder()
				.package_name("com.example.app")
				.sha256_fingerprint(VALID_FINGERPRINT)
				.build()
				.unwrap(),
		)
		.build();

	let router = UnifiedRouter::new().with_deeplinks(config);
	assert!(router.is_ok());
}

#[rstest]
fn test_server_router_with_deeplinks() {
	let config = DeeplinkConfig::builder()
		.ios(
			IosConfig::builder()
				.app_id(VALID_APP_ID)
				.paths(&["/products/*"])
				.build(),
		)
		.build();

	let router = ServerRouter::new().with_deeplinks(config);
	assert!(router.is_ok());
}

// ============================================================================
// Edge Cases Tests (エッジケース)
// ============================================================================

#[rstest]
fn test_router_without_ios() {
	let config = DeeplinkConfig::builder()
		.android(
			AndroidConfig::builder()
				.package_name("com.example.app")
				.sha256_fingerprint(VALID_FINGERPRINT)
				.build()
				.unwrap(),
		)
		.build();

	let router = DeeplinkRouter::new(config).unwrap();
	assert!(!router.config().has_ios());
	assert!(router.config().has_android());
}

#[rstest]
fn test_router_without_android() {
	let config = DeeplinkConfig::builder()
		.ios(
			IosConfig::builder()
				.app_id(VALID_APP_ID)
				.paths(&["/products/*"])
				.build(),
		)
		.build();

	let router = DeeplinkRouter::new(config).unwrap();
	assert!(router.config().has_ios());
	assert!(!router.config().has_android());
}

#[rstest]
fn test_router_with_custom_schemes() {
	let config = DeeplinkConfig::builder()
		.custom_scheme("myapp")
		.custom_scheme("myapp2")
		.build();

	let router = DeeplinkRouter::new(config).unwrap();
	assert!(router.config().has_custom_schemes());
	assert_eq!(router.config().custom_schemes.len(), 2);
}

// ============================================================================
// Sanity Tests (サニティテスト)
// ============================================================================

#[rstest]
fn test_router_debug_format() {
	let config = DeeplinkConfig::builder()
		.ios(
			IosConfig::builder()
				.app_id(VALID_APP_ID)
				.paths(&["/products/*"])
				.build(),
		)
		.build();

	let router = DeeplinkRouter::new(config).unwrap();
	let debug_str = format!("{:?}", router);

	assert!(debug_str.contains("DeeplinkRouter"));
	assert!(debug_str.contains("config"));
}

#[rstest]
fn test_ext_output_type() {
	// Verify that the extension trait returns the correct type
	let unified_router: UnifiedRouter = UnifiedRouter::new()
		.with_deeplinks(
			DeeplinkConfig::builder()
				.ios(
					IosConfig::builder()
						.app_id(VALID_APP_ID)
						.paths(&["/products/*"])
						.build(),
				)
				.build(),
		)
		.unwrap();

	let server_router: ServerRouter = ServerRouter::new()
		.with_deeplinks(
			DeeplinkConfig::builder()
				.ios(
					IosConfig::builder()
						.app_id(VALID_APP_ID)
						.paths(&["/products/*"])
						.build(),
				)
				.build(),
		)
		.unwrap();

	// Just verify the types are correct (compilation test)
	let _ = unified_router;
	let _ = server_router;
}

#[rstest]
fn test_router_server_accessor() {
	let config = DeeplinkConfig::builder()
		.ios(
			IosConfig::builder()
				.app_id(VALID_APP_ID)
				.paths(&["/products/*"])
				.build(),
		)
		.build();

	let router = DeeplinkRouter::new(config).unwrap();
	let server = router.server();

	// Verify we can access the underlying server router
	let _ = server;
}
