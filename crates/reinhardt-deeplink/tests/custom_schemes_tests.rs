//! Custom schemes module tests
//!
//! Tests for CustomSchemeConfig builder covering:
//! - Happy path: Builder pattern with scheme, host, paths
//! - Edge cases: URL template generation, multiple hosts/paths
//! - Combinatorial: Multiple paths and hosts
//! - Sanity: URL template format

use reinhardt_deeplink::{CustomScheme, CustomSchemeConfig};
use rstest::*;

// Import fixtures
mod fixtures;
use fixtures::*;

// ============================================================================
// Happy Path Tests
// ============================================================================

#[rstest]
fn test_custom_scheme_builder_basic() {
	let config = CustomSchemeConfig::builder()
		.scheme("myapp")
		.host("open")
		.paths(&["/products/*"])
		.build();

	assert_eq!(config.schemes.len(), 1);
	assert_eq!(config.schemes[0].name, "myapp");
	assert_eq!(config.schemes[0].hosts, vec!["open"]);
	assert_eq!(config.schemes[0].paths, vec!["/products/*"]);
}

#[rstest]
fn test_custom_scheme_builder_with_host() {
	let config = CustomSchemeConfig::builder()
		.scheme("myapp")
		.host("open")
		.build();

	assert_eq!(config.schemes[0].hosts, vec!["open"]);
}

#[rstest]
fn test_custom_scheme_builder_with_paths() {
	let config = CustomSchemeConfig::builder()
		.scheme("myapp")
		.paths(&["/products/*", "/users/*"])
		.build();

	assert_eq!(config.schemes[0].paths.len(), 2);
}

#[rstest]
fn test_custom_scheme_config_contains_schemes(
	#[from(custom_scheme_config)] config: CustomSchemeConfig,
) {
	assert!(!config.schemes.is_empty());
	assert_eq!(config.schemes[0].name, "myapp");
}

// ============================================================================
// URL Template Tests
// ============================================================================

#[rstest]
fn test_custom_scheme_url_template() {
	let config = CustomSchemeConfig::builder()
		.scheme("myapp")
		.host("open")
		.paths(&["/products/*"])
		.build();

	assert_eq!(config.schemes[0].url_template(), "myapp://open/products/*");
}

#[rstest]
fn test_custom_scheme_url_template_no_host() {
	let scheme = CustomScheme {
		name: "myapp".to_string(),
		hosts: vec![],
		paths: vec!["/products".to_string()],
	};

	assert_eq!(scheme.url_template(), "myapp:///products");
}

#[rstest]
fn test_custom_scheme_url_template_no_path() {
	let scheme = CustomScheme {
		name: "myapp".to_string(),
		hosts: vec!["open".to_string()],
		paths: vec![],
	};

	assert_eq!(scheme.url_template(), "myapp://open");
}

#[rstest]
fn test_custom_scheme_url_template_empty() {
	let scheme = CustomScheme {
		name: "myapp".to_string(),
		hosts: vec![],
		paths: vec![],
	};

	assert_eq!(scheme.url_template(), "myapp://");
}

// ============================================================================
// Edge Cases Tests (エッジケース)
// ============================================================================

#[rstest]
fn test_custom_scheme_multiple_paths() {
	let config = CustomSchemeConfig::builder()
		.scheme("myapp")
		.paths(&["/products/*", "/users/*", "/checkout/*"])
		.build();

	assert_eq!(config.schemes[0].paths.len(), 3);
}

#[rstest]
fn test_custom_scheme_multiple_hosts() {
	let config = CustomSchemeConfig::builder()
		.scheme("myapp")
		.hosts(&["open", "launch", "start"])
		.build();

	assert_eq!(config.schemes[0].hosts.len(), 3);
}

#[rstest]
fn test_custom_scheme_url_template_uses_first_host() {
	let config = CustomSchemeConfig::builder()
		.scheme("myapp")
		.hosts(&["open", "launch"])
		.paths(&["/products/*"])
		.build();

	// URL template uses first host and first path
	assert_eq!(config.schemes[0].url_template(), "myapp://open/products/*");
}

#[rstest]
fn test_custom_scheme_url_template_uses_first_path() {
	let config = CustomSchemeConfig::builder()
		.scheme("myapp")
		.host("open")
		.paths(&["/products/*", "/users/*"])
		.build();

	// URL template uses first path
	assert_eq!(config.schemes[0].url_template(), "myapp://open/products/*");
}

// ============================================================================
// Combinatorial Tests (組み合わせテスト)
// ============================================================================

#[rstest]
fn test_custom_scheme_builder_fluent_api() {
	let config = CustomSchemeConfig::builder()
		.scheme("myapp")
		.host("open")
		.path("/products/*")
		.path("/users/*")
		.host("launch")
		.build();

	assert_eq!(config.schemes[0].name, "myapp");
	assert_eq!(config.schemes[0].hosts.len(), 2);
	assert_eq!(config.schemes[0].paths.len(), 2);
}

#[rstest]
fn test_custom_scheme_with_all_components() {
	let config = CustomSchemeConfig::builder()
		.scheme("myapp")
		.hosts(&["open", "launch"])
		.paths(&["/products/*", "/users/*", "/checkout/*"])
		.build();

	assert_eq!(config.schemes[0].name, "myapp");
	assert_eq!(config.schemes[0].hosts.len(), 2);
	assert_eq!(config.schemes[0].paths.len(), 3);
}

// ============================================================================
// Sanity Tests (サニティテスト)
// ============================================================================

#[rstest]
fn test_custom_scheme_default() {
	let config = CustomSchemeConfig::default();
	assert!(config.schemes.is_empty());
}

#[rstest]
fn test_custom_scheme_builder_empty() {
	let config = CustomSchemeConfig::builder().build();
	assert_eq!(config.schemes.len(), 1);
	assert_eq!(config.schemes[0].name, "");
}
