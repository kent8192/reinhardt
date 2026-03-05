//! Specialized test fixtures for reinhardt-deeplink
//!
//! This module provides fixtures for deep link testing using the new Builder API.

use rstest::*;

const VALID_APP_ID: &str = "TEAM123456.com.example.app";
const VALID_FINGERPRINT: &str = "FA:C6:17:45:DC:09:03:78:6F:B9:ED:E6:2A:96:2B:39:9F:73:48:F0:BB:6F:89:9B:83:32:66:75:91:03:3B:9C";

// ============================================================================
// Platform Configuration Fixtures
// ============================================================================

/// iOS configuration fixture with basic setup.
#[fixture]
pub fn ios_config() -> reinhardt_deeplink::IosConfig {
	reinhardt_deeplink::IosConfig::builder()
		.app_id(VALID_APP_ID)
		.paths(&["/products/*"])
		.build()
}

/// Android configuration fixture with basic setup.
#[fixture]
pub fn android_config() -> reinhardt_deeplink::AndroidConfig {
	reinhardt_deeplink::AndroidConfig::builder()
		.package_name("com.example.app")
		.sha256_fingerprint(VALID_FINGERPRINT)
		.build()
		.unwrap()
}

/// Custom scheme configuration fixture.
#[fixture]
pub fn custom_scheme_config() -> reinhardt_deeplink::CustomSchemeConfig {
	reinhardt_deeplink::CustomSchemeConfig::builder()
		.scheme("myapp")
		.host("open")
		.paths(&["/products/*"])
		.build()
}

// ============================================================================
// Unified Configuration Fixtures
// ============================================================================

/// Full deeplink configuration with iOS and Android.
#[fixture]
pub fn deeplink_config() -> reinhardt_deeplink::DeeplinkConfig {
	reinhardt_deeplink::DeeplinkConfig::builder()
		.ios(ios_config_default())
		.android(android_config_default())
		.build()
}

/// Deeplink configuration with iOS only.
#[fixture]
pub fn deeplink_config_ios_only() -> reinhardt_deeplink::DeeplinkConfig {
	reinhardt_deeplink::DeeplinkConfig::builder()
		.ios(ios_config_default())
		.build()
}

/// Deeplink configuration with Android only.
#[fixture]
pub fn deeplink_config_android_only() -> reinhardt_deeplink::DeeplinkConfig {
	reinhardt_deeplink::DeeplinkConfig::builder()
		.android(android_config_default())
		.build()
}

/// Empty deeplink configuration.
#[fixture]
pub fn deeplink_config_empty() -> reinhardt_deeplink::DeeplinkConfig {
	reinhardt_deeplink::DeeplinkConfig::default()
}

// ============================================================================
// Handler Fixtures
// ============================================================================

/// AASA handler fixture with pre-configured iOS config.
#[fixture]
pub fn aasa_handler() -> reinhardt_deeplink::AasaHandler {
	reinhardt_deeplink::AasaHandler::new(ios_config_default()).unwrap()
}

/// AssetLinks handler fixture with pre-configured Android config.
#[fixture]
pub fn assetlinks_handler() -> reinhardt_deeplink::AssetLinksHandler {
	reinhardt_deeplink::AssetLinksHandler::new(android_config_default()).unwrap()
}

// ============================================================================
// Router Fixtures
// ============================================================================

/// Deeplink router fixture with full configuration.
#[fixture]
pub fn deeplink_router() -> reinhardt_deeplink::DeeplinkRouter {
	reinhardt_deeplink::DeeplinkRouter::new(deeplink_config_default()).unwrap()
}

/// Deeplink router with iOS only.
#[fixture]
pub fn deeplink_router_ios_only() -> reinhardt_deeplink::DeeplinkRouter {
	reinhardt_deeplink::DeeplinkRouter::new(deeplink_config_ios_only_default()).unwrap()
}

/// Deeplink router with Android only.
#[fixture]
pub fn deeplink_router_android_only() -> reinhardt_deeplink::DeeplinkRouter {
	reinhardt_deeplink::DeeplinkRouter::new(deeplink_config_android_only_default()).unwrap()
}

// ============================================================================
// Helper Functions (Used by fixtures and tests)
// ============================================================================

/// Creates a default iOS configuration.
fn ios_config_default() -> reinhardt_deeplink::IosConfig {
	reinhardt_deeplink::IosConfig::builder()
		.app_id(VALID_APP_ID)
		.paths(&["/products/*", "/users/*"])
		.build()
}

/// Creates a default Android configuration.
fn android_config_default() -> reinhardt_deeplink::AndroidConfig {
	reinhardt_deeplink::AndroidConfig::builder()
		.package_name("com.example.app")
		.sha256_fingerprint(VALID_FINGERPRINT)
		.build()
		.unwrap()
}

/// Creates a default deeplink configuration with both platforms.
fn deeplink_config_default() -> reinhardt_deeplink::DeeplinkConfig {
	reinhardt_deeplink::DeeplinkConfig::builder()
		.ios(ios_config_default())
		.android(android_config_default())
		.build()
}

/// Creates a deeplink configuration with iOS only.
fn deeplink_config_ios_only_default() -> reinhardt_deeplink::DeeplinkConfig {
	reinhardt_deeplink::DeeplinkConfig::builder()
		.ios(ios_config_default())
		.build()
}

/// Creates a deeplink configuration with Android only.
fn deeplink_config_android_only_default() -> reinhardt_deeplink::DeeplinkConfig {
	reinhardt_deeplink::DeeplinkConfig::builder()
		.android(android_config_default())
		.build()
}
