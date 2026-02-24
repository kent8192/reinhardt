//! # Configuration Tests
//!
//! This module contains unit tests for the deeplink configuration module using the new Builder API.
//!
//! ## Test Coverage
//! - Builder pattern for DeeplinkConfig
//! - Configuration validation
//! - Platform-specific configurations (iOS, Android, Custom Schemes)
//! - Query methods (is_configured, has_ios, has_android, has_custom_schemes)
//!
//! ## Standards Compliance
//! - Uses `#[rstest]` for all tests
//! - All tests use at least one Reinhardt component (TP-2)
//! - No skeleton implementations (TP-1)

use reinhardt_deeplink::{AndroidConfig, CustomSchemeConfig, DeeplinkConfig, IosConfig};
use rstest::*;

const VALID_FINGERPRINT: &str = "FA:C6:17:45:DC:09:03:78:6F:B9:ED:E6:2A:96:2B:39:9F:73:48:F0:BB:6F:89:9B:83:32:66:75:91:03:3B:9C";

// Import fixtures
mod fixtures;
use fixtures::*;

// ============================================================================
// Happy Path Tests (正常系)
// ============================================================================

#[rstest]
fn test_deeplink_config_builder_basic(#[from(deeplink_config)] config: DeeplinkConfig) {
	assert!(config.is_configured());
	assert!(config.has_ios());
	assert!(config.has_android());
}

#[rstest]
fn test_deeplink_config_ios_only(#[from(deeplink_config_ios_only)] config: DeeplinkConfig) {
	assert!(config.is_configured());
	assert!(config.has_ios());
	assert!(!config.has_android());
	assert!(!config.has_custom_schemes());
}

#[rstest]
fn test_deeplink_config_android_only(#[from(deeplink_config_android_only)] config: DeeplinkConfig) {
	assert!(config.is_configured());
	assert!(!config.has_ios());
	assert!(config.has_android());
	assert!(!config.has_custom_schemes());
}

#[rstest]
fn test_deeplink_config_full() {
	let config = DeeplinkConfig::builder()
		.ios(
			IosConfig::builder()
				.app_id("TEAM123456.com.example.app")
				.paths(&["/products/*", "/users/*"])
				.build(),
		)
		.android(
			AndroidConfig::builder()
				.package_name("com.example.app")
				.sha256_fingerprint(VALID_FINGERPRINT)
				.build()
				.unwrap(),
		)
		.custom_scheme("myapp")
		.build();

	assert!(config.is_configured());
	assert!(config.has_ios());
	assert!(config.has_android());
	assert!(config.has_custom_schemes());
	assert_eq!(config.custom_schemes.len(), 1);
	assert_eq!(config.custom_schemes[0].name, "myapp");
}

#[rstest]
fn test_deeplink_config_custom_scheme() {
	let custom = CustomSchemeConfig::builder()
		.scheme("myapp")
		.host("open")
		.paths(&["/products/*"])
		.build();

	let config = DeeplinkConfig::builder()
		.custom_scheme_config(custom)
		.build();

	assert!(config.is_configured());
	assert!(config.has_custom_schemes());
	assert_eq!(config.custom_schemes.len(), 1);
	assert_eq!(config.custom_schemes[0].name, "myapp");
}

// ============================================================================
// Error Path Tests (異常系) - Builder Validation
// ============================================================================

#[rstest]
fn test_ios_builder_validate_no_app_ids() {
	let builder = IosConfig::builder().paths(&["/products/*"]);
	let result = builder.validate();
	assert!(result.is_err());
}

#[rstest]
fn test_ios_builder_validate_no_paths() {
	let builder = IosConfig::builder().app_id("TEAM123456.com.example.app");
	let result = builder.validate();
	assert!(result.is_err());
}

#[rstest]
fn test_android_builder_validate_no_package() {
	let builder = AndroidConfig::builder().sha256_fingerprint(VALID_FINGERPRINT);
	let result = builder.validate();
	assert!(result.is_err());
}

#[rstest]
fn test_android_builder_validate_no_fingerprint() {
	let builder = AndroidConfig::builder().package_name("com.example.app");
	let result = builder.validate();
	assert!(result.is_err());
}

// ============================================================================
// Edge Cases Tests (エッジケース)
// ============================================================================

#[rstest]
fn test_empty_config(#[from(deeplink_config_empty)] config: DeeplinkConfig) {
	assert!(!config.is_configured());
	assert!(!config.has_ios());
	assert!(!config.has_android());
	assert!(!config.has_custom_schemes());
}

#[rstest]
fn test_multiple_custom_schemes() {
	let config = DeeplinkConfig::builder()
		.custom_scheme("myapp")
		.custom_scheme("myapp2")
		.custom_scheme("myapp3")
		.build();

	assert!(config.has_custom_schemes());
	assert_eq!(config.custom_schemes.len(), 3);
}

#[rstest]
fn test_ios_config_with_web_credentials() {
	let config = IosConfig::builder()
		.app_id("TEAM123456.com.example.app")
		.paths(&["/products/*"])
		.with_web_credentials()
		.build();

	let json = serde_json::to_string(&config).unwrap();
	assert!(json.contains("webcredentials"));
}

#[rstest]
fn test_ios_config_with_app_clips() {
	let config = IosConfig::builder()
		.app_id("TEAM123456.com.example.app")
		.paths(&["/products/*"])
		.app_clip("TEAM123456.com.example.app.Clip")
		.build();

	let json = serde_json::to_string(&config).unwrap();
	assert!(json.contains("appclips"));
}

// ============================================================================
// JSON Serialization Tests
// ============================================================================

#[rstest]
fn test_config_debug_format(#[from(deeplink_config)] config: DeeplinkConfig) {
	let debug_str = format!("{:?}", config);
	assert!(debug_str.contains("DeeplinkConfig"));
}

#[rstest]
fn test_ios_config_json_serialization() {
	let config = IosConfig::builder()
		.app_id("TEAM123456.com.example.app")
		.paths(&["/products/*"])
		.build();

	let json = serde_json::to_string_pretty(&config).unwrap();
	assert!(json.contains("applinks"));
	assert!(json.contains("TEAM123456.com.example.app"));
	assert!(json.contains("/products/*"));
}

#[rstest]
fn test_android_config_json_serialization() {
	let config = AndroidConfig::builder()
		.package_name("com.example.app")
		.sha256_fingerprint(VALID_FINGERPRINT)
		.build()
		.unwrap();

	let json = serde_json::to_string(&config).unwrap();
	assert!(json.contains("android_app"));
	assert!(json.contains("com.example.app"));
}

// ============================================================================
// Combinatorial Tests (組み合わせテスト)
// ============================================================================

#[rstest]
fn test_config_all_platforms_enabled() {
	let config = DeeplinkConfig::builder()
		.ios(
			IosConfig::builder()
				.app_id("TEAM123456.com.example.app")
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
		.custom_scheme("myapp")
		.build();

	assert!(config.ios.is_some());
	assert!(config.android.is_some());
	assert!(!config.custom_schemes.is_empty());
}

#[rstest]
fn test_ios_config_multiple_app_ids() {
	let config = IosConfig::builder()
		.app_id("TEAM123456.com.example.app")
		.additional_app_id("TEAM789XYZ.com.example.app2")
		.paths(&["/products/*"])
		.build();

	let json = serde_json::to_string(&config).unwrap();
	assert!(json.contains("TEAM123456.com.example.app"));
	assert!(json.contains("TEAM789XYZ.com.example.app2"));
}

#[rstest]
fn test_android_config_multiple_fingerprints() {
	let fp2 = "00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00";

	let config = AndroidConfig::builder()
		.package_name("com.example.app")
		.sha256_fingerprint(VALID_FINGERPRINT)
		.sha256_fingerprint(fp2)
		.build()
		.unwrap();

	let json = serde_json::to_string(&config).unwrap();
	assert!(json.contains(VALID_FINGERPRINT));
	assert!(json.contains(fp2));
}

// ============================================================================
// Sanity Tests (サニティテスト)
// ============================================================================

#[rstest]
fn test_config_default_empty() {
	let config = DeeplinkConfig::default();
	assert!(!config.is_configured());
	assert!(config.ios.is_none());
	assert!(config.android.is_none());
	assert!(config.custom_schemes.is_empty());
}
