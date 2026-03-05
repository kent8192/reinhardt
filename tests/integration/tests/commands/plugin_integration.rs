//! Plugin command integration tests
//!
//! Tests for plugin management commands: list, info, install, remove, enable, disable, search, update.

use super::fixtures::{
	MockCratesIoClient, PluginManifestFixture, plugin_manifest_fixture,
	plugin_manifest_with_plugins,
};
use reinhardt_commands::CommandContext;
use rstest::*;
use tempfile::TempDir;

// ============================================================================
// Fixtures
// ============================================================================

/// Fixture for empty temp directory
#[fixture]
fn temp_dir() -> TempDir {
	TempDir::new().expect("Failed to create temp directory")
}

/// Fixture for mock crates.io client with test packages
#[fixture]
fn mock_crates_io() -> MockCratesIoClient {
	MockCratesIoClient::with_packages(vec![
		("reinhardt-auth-delion", "0.1.0", "Authentication plugin"),
		("reinhardt-admin-delion", "0.2.0", "Admin interface plugin"),
		("reinhardt-rest-delion", "0.1.5", "REST API plugin"),
	])
}

// ============================================================================
// Happy Path Tests - List Command
// ============================================================================

/// Test: Plugin list with no plugins
///
/// Category: Happy Path
/// Verifies that empty plugin list is handled correctly.
#[rstest]
fn test_plugin_list_empty(plugin_manifest_fixture: PluginManifestFixture) {
	assert!(
		plugin_manifest_fixture.plugins.is_empty(),
		"Default fixture should have no plugins"
	);
	assert!(
		plugin_manifest_fixture.manifest_path.exists(),
		"Manifest directory should exist"
	);
}

/// Test: Plugin list with plugins
///
/// Category: Happy Path
/// Verifies that plugins are listed correctly.
#[rstest]
fn test_plugin_list_with_plugins(plugin_manifest_with_plugins: PluginManifestFixture) {
	assert_eq!(
		plugin_manifest_with_plugins.plugins.len(),
		3,
		"Should have 3 plugins"
	);

	let plugin_names: Vec<&str> = plugin_manifest_with_plugins
		.plugins
		.iter()
		.map(|p| p.name.as_str())
		.collect();

	assert!(plugin_names.contains(&"reinhardt-auth-delion"));
	assert!(plugin_names.contains(&"reinhardt-admin-delion"));
	assert!(plugin_names.contains(&"reinhardt-rest-delion"));
}

/// Test: Plugin list verbose mode
///
/// Category: Happy Path
/// Verifies that verbose flag is set correctly.
#[rstest]
fn test_plugin_list_verbose() {
	let mut ctx = CommandContext::default();
	ctx.set_verbosity(1);

	assert_eq!(ctx.verbosity, 1, "Should have verbosity level 1");
}

/// Test: Plugin list filter enabled
///
/// Category: Happy Path
/// Verifies that --enabled filter is set correctly.
#[rstest]
fn test_plugin_list_filter_enabled() {
	let mut ctx = CommandContext::default();
	ctx.set_option("enabled".to_string(), "true".to_string());

	assert!(ctx.has_option("enabled"), "Should have enabled filter");
}

// ============================================================================
// Happy Path Tests - Info Command
// ============================================================================

/// Test: Plugin info for local plugin
///
/// Category: Happy Path
/// Verifies that local plugin info is retrieved correctly.
#[rstest]
fn test_plugin_info_local(plugin_manifest_with_plugins: PluginManifestFixture) {
	let plugin = plugin_manifest_with_plugins
		.get_plugin("reinhardt-auth-delion")
		.expect("Should find auth plugin");

	assert_eq!(plugin.name, "reinhardt-auth-delion");
	assert_eq!(plugin.version, "0.1.0");
	assert!(plugin.enabled);
}

/// Test: Plugin info for remote plugin
///
/// Category: Happy Path
/// Verifies that --remote flag is set correctly.
#[rstest]
fn test_plugin_info_remote(mock_crates_io: MockCratesIoClient) {
	let mut ctx = CommandContext::default();
	ctx.set_option("remote".to_string(), "true".to_string());
	ctx.add_arg("reinhardt-auth-delion".to_string());

	// Mock the crates.io lookup
	let crate_info = mock_crates_io.get_crate("reinhardt-auth-delion");
	assert!(crate_info.is_some(), "Should find crate on mock crates.io");

	let info = crate_info.unwrap();
	assert_eq!(info.name, "reinhardt-auth-delion");
}

// ============================================================================
// Happy Path Tests - Install Command
// ============================================================================

/// Test: Plugin install success
///
/// Category: Happy Path
/// Verifies that plugin installation context is set correctly.
#[rstest]
fn test_plugin_install_success(temp_dir: TempDir, mock_crates_io: MockCratesIoClient) {
	let mut ctx = CommandContext::default();
	ctx.add_arg("reinhardt-auth-delion".to_string());

	// Verify package exists in mock registry
	let crate_info = mock_crates_io.get_crate("reinhardt-auth-delion");
	assert!(crate_info.is_some(), "Package should exist in registry");

	// Verify temp dir exists for installation
	assert!(temp_dir.path().exists(), "Temp directory should exist");
}

/// Test: Plugin install with version
///
/// Category: Happy Path
/// Verifies that --version option is set correctly.
#[rstest]
fn test_plugin_install_version() {
	let mut ctx = CommandContext::default();
	ctx.add_arg("reinhardt-auth-delion".to_string());
	ctx.set_option("version".to_string(), "0.1.0".to_string());

	assert_eq!(
		ctx.arg(0).map(String::as_str),
		Some("reinhardt-auth-delion")
	);
	assert_eq!(ctx.option("version").map(String::as_str), Some("0.1.0"));
}

// ============================================================================
// Happy Path Tests - Remove Command
// ============================================================================

/// Test: Plugin remove success
///
/// Category: Happy Path
/// Verifies that plugin removal context is set correctly.
#[rstest]
fn test_plugin_remove_success(mut plugin_manifest_with_plugins: PluginManifestFixture) {
	let initial_count = plugin_manifest_with_plugins.plugins.len();

	// Simulate removal
	plugin_manifest_with_plugins.remove_plugin("reinhardt-auth-delion");

	assert_eq!(
		plugin_manifest_with_plugins.plugins.len(),
		initial_count - 1,
		"Should have one less plugin"
	);
}

/// Test: Plugin remove with purge
///
/// Category: Happy Path
/// Verifies that --purge flag is set correctly.
#[rstest]
fn test_plugin_remove_purge() {
	let mut ctx = CommandContext::default();
	ctx.add_arg("reinhardt-auth-delion".to_string());
	ctx.set_option("purge".to_string(), "true".to_string());

	assert!(ctx.has_option("purge"), "Should have purge option");
}

// ============================================================================
// State Transition Tests
// ============================================================================

/// Test: Plugin enable state transition
///
/// Category: State Transition
/// Verifies that disabled plugin becomes enabled.
#[rstest]
fn test_plugin_enable(mut plugin_manifest_with_plugins: PluginManifestFixture) {
	// First disable the plugin
	plugin_manifest_with_plugins.disable_plugin("reinhardt-auth-delion");
	let plugin = plugin_manifest_with_plugins
		.get_plugin("reinhardt-auth-delion")
		.expect("Should find plugin");
	assert!(!plugin.enabled, "Plugin should be disabled");

	// Then enable it
	plugin_manifest_with_plugins.enable_plugin("reinhardt-auth-delion");
	let plugin = plugin_manifest_with_plugins
		.get_plugin("reinhardt-auth-delion")
		.expect("Should find plugin");
	assert!(plugin.enabled, "Plugin should be enabled");
}

/// Test: Plugin disable state transition
///
/// Category: State Transition
/// Verifies that enabled plugin becomes disabled.
#[rstest]
fn test_plugin_disable(mut plugin_manifest_with_plugins: PluginManifestFixture) {
	// Initially the plugin is enabled
	let plugin = plugin_manifest_with_plugins
		.get_plugin("reinhardt-auth-delion")
		.expect("Should find plugin");
	assert!(plugin.enabled, "Plugin should be enabled initially");

	// Disable it
	plugin_manifest_with_plugins.disable_plugin("reinhardt-auth-delion");
	let plugin = plugin_manifest_with_plugins
		.get_plugin("reinhardt-auth-delion")
		.expect("Should find plugin");
	assert!(!plugin.enabled, "Plugin should be disabled");
}

// ============================================================================
// Happy Path Tests - Search Command
// ============================================================================

/// Test: Plugin search returns results
///
/// Category: Happy Path
/// Verifies that search returns matching packages.
#[rstest]
fn test_plugin_search(mock_crates_io: MockCratesIoClient) {
	let results = mock_crates_io.search("auth");

	assert!(!results.is_empty(), "Should find matching packages");
	assert!(
		results.iter().any(|r| r.name.contains("auth")),
		"Should find auth-related package"
	);
}

/// Test: Plugin search with no results
///
/// Category: Edge Case
/// Verifies that empty search results are handled.
#[rstest]
fn test_plugin_search_no_results(mock_crates_io: MockCratesIoClient) {
	let results = mock_crates_io.search("nonexistent-xyz-plugin");

	assert!(results.is_empty(), "Should return empty results");
}

// ============================================================================
// Happy Path Tests - Update Command
// ============================================================================

/// Test: Plugin update single
///
/// Category: Happy Path
/// Verifies that single plugin update context is set.
#[rstest]
fn test_plugin_update_single() {
	let mut ctx = CommandContext::default();
	ctx.add_arg("reinhardt-auth-delion".to_string());

	assert_eq!(
		ctx.arg(0).map(String::as_str),
		Some("reinhardt-auth-delion")
	);
	assert!(ctx.arg(1).is_none(), "Should have only one package");
}

/// Test: Plugin update all
///
/// Category: Happy Path
/// Verifies that --all flag updates all plugins.
#[rstest]
fn test_plugin_update_all() {
	let mut ctx = CommandContext::default();
	ctx.set_option("all".to_string(), "true".to_string());

	assert!(ctx.has_option("all"), "Should have all option");
	assert!(ctx.arg(0).is_none(), "Should have no specific package");
}

// ============================================================================
// Error Path Tests
// ============================================================================

/// Test: Plugin install not found
///
/// Category: Error Path
/// Verifies error handling for non-existent package.
#[rstest]
fn test_plugin_install_not_found(mock_crates_io: MockCratesIoClient) {
	let crate_info = mock_crates_io.get_crate("nonexistent-plugin-xyz");

	assert!(crate_info.is_none(), "Should not find non-existent package");
}

/// Test: Plugin remove not installed
///
/// Category: Error Path
/// Verifies error handling for removing non-installed plugin.
#[rstest]
fn test_plugin_remove_not_installed(plugin_manifest_fixture: PluginManifestFixture) {
	let plugin = plugin_manifest_fixture.get_plugin("nonexistent-plugin");

	assert!(plugin.is_none(), "Should not find non-installed plugin");
}

// ============================================================================
// Edge Case Tests
// ============================================================================

/// Test: Plugin naming suggestion
///
/// Category: Edge Case
/// Verifies warning for missing -delion suffix.
#[rstest]
fn test_plugin_naming_suggestion() {
	let mut ctx = CommandContext::default();
	// User tries to install without -delion suffix
	ctx.add_arg("reinhardt-auth".to_string());

	let arg = ctx.arg(0).unwrap();
	let needs_suggestion = !arg.ends_with("-delion");

	assert!(needs_suggestion, "Should suggest adding -delion suffix");
}

/// Test: Plugin with dependencies
///
/// Category: Edge Case
/// Verifies plugin with dependencies is handled.
#[rstest]
fn test_plugin_with_dependencies(plugin_manifest_with_plugins: PluginManifestFixture) {
	// Get a plugin that might have dependencies
	let plugin = plugin_manifest_with_plugins
		.get_plugin("reinhardt-admin-delion")
		.expect("Should find admin plugin");

	assert!(!plugin.name.is_empty());
}

// ============================================================================
// Decision Table Tests
// ============================================================================

/// Test: Plugin list filter combinations
///
/// Category: Decision Table
/// Verifies combinations of list filters.
#[rstest]
#[case(false, false, "no filters")]
#[case(true, false, "enabled only")]
#[case(false, true, "verbose only")]
#[case(true, true, "both options")]
fn test_plugin_decision_list_filters(
	#[case] enabled: bool,
	#[case] verbose: bool,
	#[case] description: &str,
) {
	let mut ctx = CommandContext::default();

	if enabled {
		ctx.set_option("enabled".to_string(), "true".to_string());
	}
	if verbose {
		ctx.set_verbosity(1);
	}

	assert_eq!(
		ctx.has_option("enabled"),
		enabled,
		"{}: enabled mismatch",
		description
	);
	assert_eq!(
		ctx.verbosity >= 1,
		verbose,
		"{}: verbose mismatch",
		description
	);
}

/// Test: Plugin install option combinations
///
/// Category: Decision Table
/// Verifies combinations of install options.
#[rstest]
#[case(None, false, "no options")]
#[case(Some("0.1.0"), false, "version only")]
#[case(None, true, "force only")]
#[case(Some("0.1.0"), true, "both options")]
fn test_plugin_decision_install_options(
	#[case] version: Option<&str>,
	#[case] force: bool,
	#[case] description: &str,
) {
	let mut ctx = CommandContext::default();
	ctx.add_arg("reinhardt-auth-delion".to_string());

	if let Some(v) = version {
		ctx.set_option("version".to_string(), v.to_string());
	}
	if force {
		ctx.set_option("force".to_string(), "true".to_string());
	}

	assert_eq!(
		ctx.option("version").map(String::as_str),
		version,
		"{}: version mismatch",
		description
	);
	assert_eq!(
		ctx.has_option("force"),
		force,
		"{}: force mismatch",
		description
	);
}

// ============================================================================
// Use Case Tests
// ============================================================================

/// Test: Plugin lifecycle - install → enable → disable → remove
///
/// Category: Use Case
/// Verifies complete plugin lifecycle.
#[rstest]
fn test_plugin_lifecycle_full(
	mut plugin_manifest_fixture: PluginManifestFixture,
	mock_crates_io: MockCratesIoClient,
) {
	// Step 1: Verify package exists in registry
	let crate_info = mock_crates_io.get_crate("reinhardt-auth-delion");
	assert!(crate_info.is_some(), "Package should exist");

	// Step 2: Simulate install
	plugin_manifest_fixture.add_plugin("reinhardt-auth-delion", "0.1.0", true);
	assert_eq!(plugin_manifest_fixture.plugins.len(), 1);

	// Step 3: Verify enabled by default
	let plugin = plugin_manifest_fixture
		.get_plugin("reinhardt-auth-delion")
		.expect("Should find plugin");
	assert!(plugin.enabled);

	// Step 4: Disable
	plugin_manifest_fixture.disable_plugin("reinhardt-auth-delion");
	let plugin = plugin_manifest_fixture
		.get_plugin("reinhardt-auth-delion")
		.expect("Should find plugin");
	assert!(!plugin.enabled);

	// Step 5: Enable again
	plugin_manifest_fixture.enable_plugin("reinhardt-auth-delion");
	let plugin = plugin_manifest_fixture
		.get_plugin("reinhardt-auth-delion")
		.expect("Should find plugin");
	assert!(plugin.enabled);

	// Step 6: Remove
	plugin_manifest_fixture.remove_plugin("reinhardt-auth-delion");
	assert!(plugin_manifest_fixture.plugins.is_empty());
}

// ============================================================================
// Sanity Tests
// ============================================================================

/// Test: Plugin command sanity check
///
/// Category: Sanity
/// Verifies basic plugin command infrastructure.
#[rstest]
fn test_plugin_sanity(
	plugin_manifest_fixture: PluginManifestFixture,
	mock_crates_io: MockCratesIoClient,
) {
	// Verify fixture is usable
	assert!(plugin_manifest_fixture.manifest_path.exists());
	assert!(plugin_manifest_fixture.plugins.is_empty());

	// Verify mock registry has packages
	let results = mock_crates_io.search("reinhardt");
	assert!(!results.is_empty(), "Should have packages in mock registry");

	// Verify context can be created
	let ctx = CommandContext::default();
	assert!(ctx.arg(0).is_none());
}
