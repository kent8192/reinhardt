//! # Universal Links Tests
//!
//! This module contains unit tests for iOS Universal Links functionality using the new IosConfig builder.
//!
//! ## Test Coverage
//! - IosConfig builder pattern
//! - Multiple app IDs
//! - Component support (iOS 13+)
//! - Path pattern handling
//! - JSON serialization
//! - Web credentials and App Clips
//! - AasaHandler functionality
//!
//! ## Standards Compliance
//! - Uses `#[rstest]` for all tests
//! - All tests use at least one Reinhardt component (TP-2)
//! - No skeleton implementations (TP-1)

use reinhardt_deeplink::{AasaHandler, AppLinkComponent, IosConfig};
use rstest::*;

const VALID_APP_ID: &str = "TEAM123456.com.example.app";

// ============================================================================
// Happy Path Tests (正常系)
// ============================================================================

#[rstest]
fn test_ios_config_basic() {
	let config = IosConfig::builder()
		.app_id(VALID_APP_ID)
		.paths(&["/products/*", "/profile/*"])
		.build();

	let json = serde_json::to_string_pretty(&config).unwrap();
	assert!(json.contains("applinks"));
	assert!(json.contains(VALID_APP_ID));
	assert!(json.contains("/products/*"));
}

#[rstest]
fn test_ios_config_multiple_app_ids() {
	let config = IosConfig::builder()
		.app_id("TEAM123456.com.example.app1")
		.additional_app_id("TEAM789XYZ.com.example.app2")
		.paths(&["/products/*"])
		.build();

	let json = serde_json::to_string(&config).unwrap();
	assert!(json.contains("TEAM123456.com.example.app1"));
	assert!(json.contains("TEAM789XYZ.com.example.app2"));
}

#[rstest]
fn test_ios_config_with_exclude_paths() {
	let config = IosConfig::builder()
		.app_id(VALID_APP_ID)
		.paths(&["/products/*"])
		.exclude_paths(&["/api/*", "/admin/*"])
		.build();

	let json = serde_json::to_string_pretty(&config).unwrap();
	assert!(json.contains("/products/*"));
	assert!(json.contains("/api/*"));
	assert!(json.contains("/admin/*"));
}

#[rstest]
fn test_ios_config_json_serialization() {
	let config = IosConfig::builder()
		.app_id(VALID_APP_ID)
		.paths(&["/products/*"])
		.build();

	let json = serde_json::to_string(&config);
	assert!(json.is_ok());
	let json = json.unwrap();
	assert!(json.contains("applinks"));
	assert!(json.contains(VALID_APP_ID));
}

#[rstest]
fn test_ios_config_with_components() {
	let component = AppLinkComponent {
		path: "/products/*".to_string(),
		query: Some("ref=*".to_string()),
		fragment: None,
		exclude: None,
		comment: Some("Product pages with referral".to_string()),
	};

	let config = IosConfig::builder()
		.app_id(VALID_APP_ID)
		.component(component)
		.build();

	let json = serde_json::to_string_pretty(&config).unwrap();
	assert!(json.contains("components"));
	assert!(json.contains("ref=*"));
}

#[rstest]
fn test_ios_config_with_web_credentials() {
	let config = IosConfig::builder()
		.app_id(VALID_APP_ID)
		.paths(&["/"])
		.with_web_credentials()
		.build();

	let json = serde_json::to_string_pretty(&config).unwrap();
	assert!(json.contains("webcredentials"));
	assert!(json.contains(VALID_APP_ID));
}

#[rstest]
fn test_ios_config_with_app_clips() {
	let config = IosConfig::builder()
		.app_id(VALID_APP_ID)
		.paths(&["/"])
		.app_clip("TEAM123456.com.example.app.Clip")
		.build();

	let json = serde_json::to_string_pretty(&config).unwrap();
	assert!(json.contains("appclips"));
	assert!(json.contains("TEAM123456.com.example.app.Clip"));
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
	let builder = IosConfig::builder().app_id(VALID_APP_ID);
	let result = builder.validate();
	assert!(result.is_err());
}

// ============================================================================
// Edge Cases Tests (エッジケース)
// ============================================================================

#[rstest]
fn test_ios_config_wildcard_patterns() {
	let config = IosConfig::builder()
		.app_id(VALID_APP_ID)
		.paths(&["/*", "/products/*", "/profile/*/settings"])
		.build();

	let json = serde_json::to_string(&config).unwrap();
	assert!(json.contains("/*"));
	assert!(json.contains("/products/*"));
	assert!(json.contains("/profile/*/settings"));
}

#[rstest]
fn test_ios_config_component_query_fragment() {
	let component = AppLinkComponent {
		path: "/search/*".to_string(),
		query: Some("q=*".to_string()),
		fragment: Some("results".to_string()),
		exclude: None,
		comment: None,
	};

	let config = IosConfig::builder()
		.app_id(VALID_APP_ID)
		.component(component)
		.build();

	let json = serde_json::to_string(&config).unwrap();
	assert!(json.contains("?"));
	assert!(json.contains("#"));
}

#[rstest]
fn test_ios_config_additional_apps() {
	let config = IosConfig::builder()
		.app_id(VALID_APP_ID)
		.paths(&["/products/*"])
		.additional_app("TEAM789XYZ.com.example.app2", &["/profile/*"])
		.build();

	assert_eq!(config.applinks.details.len(), 2);
}

// ============================================================================
// App ID Validation Tests
// ============================================================================

#[rstest]
#[case("TEAM123456.com.example.app", true)]
#[case("ABC123XYZ0.com.example.myapp", true)]
#[case("TEAM.com.example", true)]
#[case("TEAM.bundle", false)] // single-segment bundle ID (not reverse-domain)
#[case("invalid", false)]
#[case("", false)]
#[case(".com.example", false)]
#[case("TEAM.", false)]
fn test_app_id_validation_valid(#[case] app_id: &str, #[case] expected_valid: bool) {
	let result = reinhardt_deeplink::validate_app_id(app_id);
	assert_eq!(result.is_ok(), expected_valid);
}

// ============================================================================
// JSON Structure Tests
// ============================================================================

#[rstest]
fn test_aasa_json_structure() {
	let config = IosConfig::builder()
		.app_id(VALID_APP_ID)
		.paths(&["/products/*"])
		.build();

	let json = serde_json::to_value(&config).unwrap();

	// Verify structure matches Apple AASA spec
	assert!(json.get("applinks").is_some());
	let applinks = json.get("applinks").unwrap();
	assert!(applinks.get("details").is_some());
	let details = applinks.get("details").unwrap().as_array().unwrap();
	assert_eq!(details.len(), 1);

	let first_detail = &details[0];
	assert!(first_detail.get("appIDs").is_some());
	assert!(first_detail.get("paths").is_some());
}

#[rstest]
fn test_aasa_json_pretty_format() {
	let config = IosConfig::builder()
		.app_id(VALID_APP_ID)
		.paths(&["/products/*"])
		.build();

	let json = serde_json::to_string_pretty(&config).unwrap();
	// Pretty printed JSON has newlines
	assert!(json.contains("\n"));
}

// ============================================================================
// Combinatorial Tests (組み合わせテスト)
// ============================================================================

#[rstest]
fn test_ios_config_multiple_apps_same_paths() {
	let paths = vec!["/products/*", "/profile/*"];

	let config = IosConfig::builder()
		.app_id("TEAM123456.com.example.app1")
		.paths(&paths)
		.additional_app("TEAM789XYZ.com.example.app2", &paths)
		.build();

	assert_eq!(config.applinks.details.len(), 2);
}

#[rstest]
fn test_ios_config_with_components_and_web_credentials() {
	let component = AppLinkComponent {
		path: "/products/*".to_string(),
		query: None,
		fragment: None,
		exclude: None,
		comment: None,
	};

	let config = IosConfig::builder()
		.app_id(VALID_APP_ID)
		.paths(&["/"])
		.component(component)
		.with_web_credentials()
		.build();

	let json = serde_json::to_string(&config).unwrap();
	assert!(json.contains("components"));
	assert!(json.contains("webcredentials"));
}

// ============================================================================
// Sanity Tests (サニティテスト)
// ============================================================================

#[rstest]
fn test_aasa_handler_creation() {
	let config = IosConfig::builder()
		.app_id(VALID_APP_ID)
		.paths(&["/products/*"])
		.build();

	let handler = AasaHandler::new(config);
	assert!(handler.is_ok());
}

#[rstest]
fn test_aasa_handler_json_content() {
	let config = IosConfig::builder()
		.app_id(VALID_APP_ID)
		.paths(&["/products/*"])
		.build();

	let handler = AasaHandler::new(config).unwrap();
	let json = handler.json();
	assert!(json.contains("applinks"));
}

#[rstest]
fn test_aasa_handler_clone() {
	let config = IosConfig::builder()
		.app_id(VALID_APP_ID)
		.paths(&["/products/*"])
		.build();

	let handler = AasaHandler::new(config).unwrap();
	let cloned = handler.clone();
	assert_eq!(handler.json(), cloned.json());
}
