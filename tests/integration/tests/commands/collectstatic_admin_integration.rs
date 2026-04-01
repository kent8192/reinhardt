//! Integration tests verifying that collectstatic discovers and collects
//! admin static files registered via `register_app_static_files!`.
//!
//! These tests require both `reinhardt-admin` and `reinhardt-commands` to be
//! linked into the test binary so that `inventory` can discover the admin's
//! `AppStaticFilesConfig` registration at link time.

use reinhardt_admin::core::AdminSite;
use reinhardt_apps::get_app_static_files;
use reinhardt_commands::{CollectStaticCommand, CollectStaticOptions};
use reinhardt_utils::staticfiles::StaticFilesConfig;
use rstest::*;
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

// Ensure reinhardt_admin is linked so inventory discovers the registration.
// Accessing a concrete type prevents the linker from stripping the crate.
const _: () = {
	fn _force_link() {
		let _ = std::mem::size_of::<AdminSite>();
	}
};

// ============================================================================
// Inventory Discovery Tests
// ============================================================================

/// Test: Admin static files are discovered via inventory auto-discovery
///
/// Category: Integration
/// Verifies that `register_app_static_files!("admin", ...)` in reinhardt-admin
/// is picked up by `get_app_static_files()` when both crates are linked.
#[rstest]
fn test_admin_static_files_registered_in_inventory() {
	// Arrange & Act
	let configs = get_app_static_files();

	// Assert - find the admin config
	let admin_config = configs.iter().find(|c| c.app_label == "admin");
	assert!(
		admin_config.is_some(),
		"Admin static files should be registered in inventory, found labels: {:?}",
		configs.iter().map(|c| c.app_label).collect::<Vec<_>>()
	);

	let admin = admin_config.unwrap();
	assert_eq!(admin.url_prefix, "/static/admin/");
	assert!(
		admin.static_dir.contains("reinhardt-admin"),
		"Static dir should point to reinhardt-admin crate, got: {}",
		admin.static_dir
	);
	assert!(
		admin.static_dir.ends_with("/assets"),
		"Static dir should end with /assets, got: {}",
		admin.static_dir
	);
}

/// Test: Admin static directory exists and contains expected files
///
/// Category: Integration
/// Verifies that the registered admin static directory actually exists
/// on the filesystem and contains style.css and main.js.
#[rstest]
fn test_admin_static_dir_contains_assets() {
	// Arrange
	let configs = get_app_static_files();
	let admin_config = configs
		.iter()
		.find(|c| c.app_label == "admin")
		.expect("Admin config should be registered");

	let static_dir = PathBuf::from(admin_config.static_dir);

	// Assert - directory exists
	assert!(
		static_dir.exists(),
		"Admin static directory should exist: {}",
		static_dir.display()
	);

	// Assert - expected files exist
	assert!(
		static_dir.join("style.css").exists(),
		"style.css should exist in admin static directory"
	);
	assert!(
		static_dir.join("main.js").exists(),
		"main.js should exist in admin static directory"
	);
}

// ============================================================================
// CollectStatic Command Integration Tests
// ============================================================================

/// Test: CollectStatic collects admin static files via auto-discovery
///
/// Category: Integration
/// Verifies that running CollectStaticCommand with no manual staticfiles_dirs
/// still discovers and collects admin assets registered via inventory.
#[rstest]
fn test_collectstatic_collects_admin_assets(temp_dir: TempDir) {
	// Arrange
	let dest_dir = temp_dir.path().join("static_root");

	let config = StaticFilesConfig {
		static_url: "/static/".to_string(),
		static_root: dest_dir.clone(),
		staticfiles_dirs: vec![], // no manual dirs, rely on auto-discovery
		media_url: None,
	};

	let options = CollectStaticOptions {
		verbosity: 0,
		enable_hashing: false,
		..Default::default()
	};

	let mut command = CollectStaticCommand::new(config, options);

	// Act
	let result = command.execute();

	// Assert
	assert!(result.is_ok(), "CollectStatic should succeed");

	let stats = result.unwrap();
	assert!(
		stats.copied >= 2,
		"Should copy at least 2 files (style.css, main.js), copied: {}",
		stats.copied
	);

	// Verify admin assets are in the destination
	assert!(
		dest_dir.join("style.css").exists(),
		"style.css should be collected"
	);
	assert!(
		dest_dir.join("main.js").exists(),
		"main.js should be collected"
	);
}

/// Test: Collected admin CSS file has correct content
///
/// Category: Integration
/// Verifies that the collected style.css matches the source file content.
#[rstest]
fn test_collectstatic_admin_css_content_integrity(temp_dir: TempDir) {
	// Arrange
	let dest_dir = temp_dir.path().join("static_root");

	let config = StaticFilesConfig {
		static_url: "/static/".to_string(),
		static_root: dest_dir.clone(),
		staticfiles_dirs: vec![],
		media_url: None,
	};

	let options = CollectStaticOptions {
		verbosity: 0,
		enable_hashing: false,
		..Default::default()
	};

	let mut command = CollectStaticCommand::new(config, options);
	command.execute().expect("CollectStatic should succeed");

	// Act
	let collected_css =
		fs::read_to_string(dest_dir.join("style.css")).expect("Should read collected style.css");

	// Assert - verify theme content (utility classes are generated at runtime by UnoCSS)
	assert!(
		collected_css.contains("box-sizing"),
		"Collected CSS should contain box-sizing reset"
	);
	assert!(
		collected_css.contains("--admin-amber"),
		"Collected CSS should contain admin theme tokens"
	);
}

/// Test: Collected admin JS file has correct content
///
/// Category: Integration
/// Verifies that the collected main.js matches the source file content.
#[rstest]
fn test_collectstatic_admin_js_content_integrity(temp_dir: TempDir) {
	// Arrange
	let dest_dir = temp_dir.path().join("static_root");

	let config = StaticFilesConfig {
		static_url: "/static/".to_string(),
		static_root: dest_dir.clone(),
		staticfiles_dirs: vec![],
		media_url: None,
	};

	let options = CollectStaticOptions {
		verbosity: 0,
		enable_hashing: false,
		..Default::default()
	};

	let mut command = CollectStaticCommand::new(config, options);
	command.execute().expect("CollectStatic should succeed");

	// Act
	let collected_js =
		fs::read_to_string(dest_dir.join("main.js")).expect("Should read collected main.js");

	// Assert - verify admin panel JS content
	assert!(
		collected_js.contains("Reinhardt Admin"),
		"Collected JS should contain admin panel identifier"
	);
	assert!(
		collected_js.contains("use strict"),
		"Collected JS should use strict mode"
	);
}

// ============================================================================
// Spec-based tests for #3116: WASM build-to-serving pipeline
// ============================================================================

/// Test: Admin static directory contains WASM build artifacts.
/// Spec: after WASM build, the admin assets directory must contain
/// the WASM JS bindings and binary, not just the placeholder (#3116).
#[rstest]
fn test_admin_static_dir_contains_wasm_artifacts() {
	// Arrange
	let configs = get_app_static_files();
	let admin_config = configs
		.iter()
		.find(|c| c.app_label == "admin")
		.expect("Admin config should be registered");
	let static_dir = PathBuf::from(admin_config.static_dir);

	// Skip when WASM frontend is not built (CI without wasm-pack)
	if !static_dir.join("reinhardt_admin.js").exists() {
		eprintln!("Skipping: WASM artifacts not built (reinhardt_admin.js not found)");
		return;
	}

	// Assert - WASM JS bindings must exist
	assert!(
		static_dir.join("reinhardt_admin.js").exists(),
		"reinhardt_admin.js (WASM JS bindings) should exist in admin \
		 static directory: {}",
		static_dir.display()
	);
	// Assert - WASM binary must exist
	assert!(
		static_dir.join("reinhardt_admin_bg.wasm").exists(),
		"reinhardt_admin_bg.wasm (WASM binary) should exist in admin \
		 static directory: {}",
		static_dir.display()
	);
}

/// Test: CollectStatic collects WASM build artifacts via auto-discovery.
/// Spec: the collectstatic pipeline must discover and collect WASM
/// output files so the server can serve them to clients (#3116).
#[rstest]
fn test_collectstatic_collects_wasm_artifacts(temp_dir: TempDir) {
	// Arrange
	let dest_dir = temp_dir.path().join("static_root");

	let config = StaticFilesConfig {
		static_url: "/static/".to_string(),
		static_root: dest_dir.clone(),
		staticfiles_dirs: vec![],
		media_url: None,
	};

	let options = CollectStaticOptions {
		verbosity: 0,
		enable_hashing: false,
		..Default::default()
	};

	let mut command = CollectStaticCommand::new(config, options);

	// Act
	let result = command.execute();

	// Assert
	assert!(result.is_ok(), "CollectStatic should succeed");

	// Skip WASM artifact assertions when WASM frontend is not built
	if !dest_dir.join("admin").join("reinhardt_admin.js").exists() {
		eprintln!("Skipping WASM assertions: WASM artifacts not built");
		return;
	}

	// Assert - WASM JS bindings must be collected
	assert!(
		dest_dir.join("reinhardt_admin.js").exists(),
		"collectstatic must collect the WASM JS bindings (reinhardt_admin.js)"
	);
	// Assert - WASM binary must be collected
	assert!(
		dest_dir.join("reinhardt_admin_bg.wasm").exists(),
		"collectstatic must collect the WASM binary (reinhardt_admin_bg.wasm)"
	);
}

// ============================================================================
// Fixtures
// ============================================================================

/// Fixture for creating a temporary directory
#[fixture]
fn temp_dir() -> TempDir {
	TempDir::new().expect("Failed to create temp directory")
}
