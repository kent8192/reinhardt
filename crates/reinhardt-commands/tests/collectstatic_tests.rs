//! Collectstatic command tests for reinhardt-commands
//!
//! Tests for the CollectStaticCommand, CollectStaticOptions, and CollectStaticStats.
//! These tests verify static file collection functionality.

use reinhardt_commands::{CollectStaticCommand, CollectStaticOptions, CollectStaticStats};
use reinhardt_utils::staticfiles::StaticFilesConfig;
use rstest::*;
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

/// Count files recursively in a directory (matching collectstatic's behavior).
fn count_files_recursive(dir: &std::path::Path) -> usize {
	if !dir.exists() {
		return 0;
	}
	let mut count = 0;
	for entry in std::fs::read_dir(dir).unwrap() {
		let entry = entry.unwrap();
		let path = entry.path();
		// Skip hidden files (starting with '.') to match collectstatic's should_ignore()
		if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
			if name.starts_with('.') {
				continue;
			}
		}
		if path.is_file() {
			count += 1;
		} else if path.is_dir() {
			count += count_files_recursive(&path);
		}
	}
	count
}

// ============================================================================
// Fixtures
// ============================================================================

/// Fixture for creating a temporary directory structure
#[fixture]
fn temp_dir() -> TempDir {
	TempDir::new().expect("Failed to create temp directory")
}

/// Fixture for default CollectStaticOptions
#[fixture]
fn default_options() -> CollectStaticOptions {
	CollectStaticOptions::default()
}

/// Fixture for creating a temp directory with source static files
#[fixture]
fn temp_with_static_files(temp_dir: TempDir) -> (TempDir, PathBuf, PathBuf) {
	let source_dir = temp_dir.path().join("static_source");
	let dest_dir = temp_dir.path().join("static_root");

	// Create source directory with test files
	fs::create_dir_all(&source_dir).expect("Failed to create source dir");
	fs::write(source_dir.join("app.js"), b"console.log('app');").expect("Failed to write app.js");
	fs::write(source_dir.join("style.css"), b".app { color: red; }")
		.expect("Failed to write style.css");

	// Create subdirectory
	let sub_dir = source_dir.join("images");
	fs::create_dir_all(&sub_dir).expect("Failed to create images dir");
	fs::write(sub_dir.join("logo.png"), b"PNG_DATA").expect("Failed to write logo.png");

	(temp_dir, source_dir, dest_dir)
}

/// Fixture for creating CollectStaticCommand with temp directories
#[fixture]
fn collectstatic_command(
	temp_with_static_files: (TempDir, PathBuf, PathBuf),
) -> (TempDir, CollectStaticCommand) {
	let (_temp_dir, source_dir, dest_dir) = temp_with_static_files;

	let config = StaticFilesConfig {
		static_url: "/static/".to_string(),
		static_root: dest_dir,
		staticfiles_dirs: vec![source_dir],
		media_url: Some("/media/".to_string()),
	};

	let options = CollectStaticOptions::default();
	let command = CollectStaticCommand::new(config, options);

	(_temp_dir, command)
}

// ============================================================================
// Happy Path Tests - CollectStaticOptions
// ============================================================================

/// Test: Default CollectStaticOptions values
///
/// Category: Happy Path
/// Verifies that default options are correctly set.
#[rstest]
fn test_collectstatic_options_default(default_options: CollectStaticOptions) {
	assert!(!default_options.clear, "clear should be false by default");
	assert!(
		!default_options.no_input,
		"no_input should be false by default"
	);
	assert!(
		!default_options.dry_run,
		"dry_run should be false by default"
	);
	assert!(
		default_options.interactive,
		"interactive should be true by default"
	);
	assert_eq!(
		default_options.verbosity, 1,
		"verbosity should be 1 by default"
	);
	assert!(!default_options.link, "link should be false by default");
	assert!(
		default_options.ignore_patterns.is_empty(),
		"ignore_patterns should be empty by default"
	);
}

/// Test: CollectStaticOptions with all flags set
///
/// Category: Happy Path
/// Verifies that all options can be set correctly.
#[rstest]
fn test_collectstatic_options_all_set() {
	let options = CollectStaticOptions {
		clear: true,
		no_input: true,
		dry_run: true,
		interactive: false,
		verbosity: 3,
		link: true,
		ignore_patterns: vec!["*.map".to_string(), "*.log".to_string()],
		enable_hashing: false,
		fast_compare: true,
	};

	assert!(options.clear, "clear should be true");
	assert!(options.no_input, "no_input should be true");
	assert!(options.dry_run, "dry_run should be true");
	assert!(!options.interactive, "interactive should be false");
	assert_eq!(options.verbosity, 3, "verbosity should be 3");
	assert!(options.link, "link should be true");
	assert_eq!(
		options.ignore_patterns.len(),
		2,
		"Should have 2 ignore patterns"
	);
}

/// Test: CollectStaticOptions Clone trait
///
/// Category: Happy Path
/// Verifies that Clone trait works correctly.
#[rstest]
fn test_collectstatic_options_clone() {
	let original = CollectStaticOptions {
		clear: true,
		no_input: true,
		dry_run: false,
		interactive: true,
		verbosity: 2,
		link: true,
		ignore_patterns: vec!["*.map".to_string()],
		enable_hashing: false,
		fast_compare: true,
	};

	let cloned = original.clone();

	assert_eq!(cloned.clear, original.clear);
	assert_eq!(cloned.no_input, original.no_input);
	assert_eq!(cloned.dry_run, original.dry_run);
	assert_eq!(cloned.interactive, original.interactive);
	assert_eq!(cloned.verbosity, original.verbosity);
	assert_eq!(cloned.link, original.link);
	assert_eq!(cloned.ignore_patterns, original.ignore_patterns);
}

/// Test: CollectStaticOptions Debug trait
///
/// Category: Happy Path
/// Verifies that Debug trait is implemented.
#[rstest]
fn test_collectstatic_options_debug() {
	let options = CollectStaticOptions::default();
	let debug_str = format!("{:?}", options);

	assert!(
		debug_str.contains("CollectStaticOptions"),
		"Debug should contain struct name"
	);
	assert!(
		debug_str.contains("clear"),
		"Debug should contain field name"
	);
}

// ============================================================================
// Happy Path Tests - CollectStaticStats
// ============================================================================

/// Test: Default CollectStaticStats values
///
/// Category: Happy Path
/// Verifies that default stats are zeroed.
#[rstest]
fn test_collectstatic_stats_default() {
	let stats = CollectStaticStats::default();

	assert_eq!(stats.copied, 0, "copied should be 0 by default");
	assert_eq!(stats.skipped, 0, "skipped should be 0 by default");
	assert_eq!(stats.deleted, 0, "deleted should be 0 by default");
	assert_eq!(stats.unmodified, 0, "unmodified should be 0 by default");
}

/// Test: CollectStaticStats::new() is same as Default
///
/// Category: Happy Path
/// Verifies that new() and default() produce the same result.
#[rstest]
fn test_collectstatic_stats_new_equals_default() {
	let stats_new = CollectStaticStats::new();
	let stats_default = CollectStaticStats::default();

	assert_eq!(
		stats_new.copied, stats_default.copied,
		"copied should match"
	);
	assert_eq!(
		stats_new.skipped, stats_default.skipped,
		"skipped should match"
	);
	assert_eq!(
		stats_new.deleted, stats_default.deleted,
		"deleted should match"
	);
	assert_eq!(
		stats_new.unmodified, stats_default.unmodified,
		"unmodified should match"
	);
}

/// Test: CollectStaticStats Clone trait
///
/// Category: Happy Path
/// Verifies that Clone trait works correctly.
#[rstest]
fn test_collectstatic_stats_clone() {
	let original = CollectStaticStats {
		copied: 10,
		skipped: 5,
		deleted: 2,
		unmodified: 8,
	};

	let cloned = original.clone();

	assert_eq!(cloned.copied, 10, "copied should match");
	assert_eq!(cloned.skipped, 5, "skipped should match");
	assert_eq!(cloned.deleted, 2, "deleted should match");
	assert_eq!(cloned.unmodified, 8, "unmodified should match");
}

/// Test: CollectStaticStats Debug trait
///
/// Category: Happy Path
/// Verifies that Debug trait is implemented.
#[rstest]
fn test_collectstatic_stats_debug() {
	let stats = CollectStaticStats {
		copied: 5,
		skipped: 3,
		deleted: 1,
		unmodified: 2,
	};
	let debug_str = format!("{:?}", stats);

	assert!(
		debug_str.contains("CollectStaticStats"),
		"Debug should contain struct name"
	);
	assert!(debug_str.contains("5"), "Debug should contain copied value");
}

// ============================================================================
// Happy Path Tests - CollectStaticCommand
// ============================================================================

/// Test: CollectStaticCommand basic file copy
///
/// Category: Happy Path
/// Verifies that files are copied to destination.
#[rstest]
fn test_collectstatic_basic_copy(collectstatic_command: (TempDir, CollectStaticCommand)) {
	let (_temp_dir, mut command) = collectstatic_command;

	let result = command.execute();
	assert!(result.is_ok(), "Execute should succeed ");

	let stats = result.unwrap();
	assert!(stats.copied > 0, "Should have copied at least one file");
}

/// Test: CollectStaticCommand dry run mode
///
/// Category: Happy Path
/// Verifies that --dry-run does not write files.
#[rstest]
fn test_collectstatic_dry_run(temp_with_static_files: (TempDir, PathBuf, PathBuf)) {
	let (_temp_dir, source_dir, dest_dir) = temp_with_static_files;

	let config = StaticFilesConfig {
		static_url: "/static/".to_string(),
		static_root: dest_dir.clone(),
		staticfiles_dirs: vec![source_dir],
		media_url: Some("/media/".to_string()),
	};

	let options = CollectStaticOptions {
		dry_run: true,
		verbosity: 0,
		..Default::default()
	};

	let mut command = CollectStaticCommand::new(config, options);
	let result = command.execute();

	assert!(result.is_ok(), "Execute should succeed ");

	// Destination directory should not exist or be empty
	if dest_dir.exists() {
		let entry_count = fs::read_dir(&dest_dir).map(|r| r.count()).unwrap_or(0);
		// In dry-run mode, the directory might be created but should be empty
		// or the files should not be copied
		assert!(
			entry_count == 0 || !dest_dir.join("app.js").exists(),
			"Files should not be copied in dry-run mode"
		);
	}
}

/// Test: CollectStaticCommand clear option
///
/// Category: Happy Path
/// Verifies that --clear deletes existing files.
#[rstest]
fn test_collectstatic_clear(temp_with_static_files: (TempDir, PathBuf, PathBuf)) {
	let (_temp_dir, source_dir, dest_dir) = temp_with_static_files;

	// Create destination with existing file
	fs::create_dir_all(&dest_dir).expect("Failed to create dest dir");
	fs::write(dest_dir.join("old_file.txt"), b"old content").expect("Failed to write old file");

	let config = StaticFilesConfig {
		static_url: "/static/".to_string(),
		static_root: dest_dir.clone(),
		staticfiles_dirs: vec![source_dir],
		media_url: Some("/media/".to_string()),
	};

	let options = CollectStaticOptions {
		clear: true,
		verbosity: 0,
		..Default::default()
	};

	let mut command = CollectStaticCommand::new(config, options);
	let result = command.execute();

	assert!(result.is_ok(), "Execute should succeed ");

	let stats = result.unwrap();
	assert!(stats.deleted > 0, "Should have deleted at least one file");

	// Old file should be removed
	assert!(
		!dest_dir.join("old_file.txt").exists(),
		"Old file should be deleted"
	);
}

/// Test: CollectStaticCommand link option
///
/// Category: Happy Path
/// Verifies that --link creates symlinks instead of copying.
#[cfg(unix)]
#[rstest]
fn test_collectstatic_link(temp_with_static_files: (TempDir, PathBuf, PathBuf)) {
	let (_temp_dir, source_dir, dest_dir) = temp_with_static_files;

	let config = StaticFilesConfig {
		static_url: "/static/".to_string(),
		static_root: dest_dir.clone(),
		staticfiles_dirs: vec![source_dir],
		media_url: Some("/media/".to_string()),
	};

	let options = CollectStaticOptions {
		link: true,
		verbosity: 0,
		..Default::default()
	};

	let mut command = CollectStaticCommand::new(config, options);
	let result = command.execute();

	assert!(result.is_ok(), "Execute should succeed ");

	// Check if files are symlinks
	let app_js = dest_dir.join("app.js");
	if app_js.exists() {
		let metadata = fs::symlink_metadata(&app_js).expect("Failed to get metadata");
		assert!(
			metadata.file_type().is_symlink(),
			"app.js should be a symlink"
		);
	}
}

/// Test: CollectStaticCommand ignore patterns
///
/// Category: Happy Path
/// Verifies that --ignore patterns skip matching files.
#[rstest]
fn test_collectstatic_ignore(temp_with_static_files: (TempDir, PathBuf, PathBuf)) {
	let (_temp_dir, source_dir, dest_dir) = temp_with_static_files;

	// Add a .map file to source
	fs::write(source_dir.join("app.js.map"), b"source map content")
		.expect("Failed to write map file");

	let config = StaticFilesConfig {
		static_url: "/static/".to_string(),
		static_root: dest_dir.clone(),
		staticfiles_dirs: vec![source_dir],
		media_url: Some("/media/".to_string()),
	};

	let options = CollectStaticOptions {
		ignore_patterns: vec!["*.map".to_string()],
		verbosity: 0,
		..Default::default()
	};

	let mut command = CollectStaticCommand::new(config, options);
	let result = command.execute();

	assert!(result.is_ok(), "Execute should succeed ");

	let stats = result.unwrap();
	assert!(stats.skipped > 0, "Should have skipped at least one file");

	// .map file should not be copied
	assert!(
		!dest_dir.join("app.js.map").exists(),
		".map file should be ignored"
	);
}

/// Test: CollectStaticCommand multiple source directories
///
/// Category: Happy Path
/// Verifies that files from multiple sources are collected.
#[rstest]
fn test_collectstatic_multiple_sources(temp_dir: TempDir) {
	let source1 = temp_dir.path().join("source1");
	let source2 = temp_dir.path().join("source2");
	let dest_dir = temp_dir.path().join("static_root");

	fs::create_dir_all(&source1).expect("Failed to create source1");
	fs::create_dir_all(&source2).expect("Failed to create source2");

	fs::write(source1.join("file1.js"), b"// from source1").expect("Failed to write file1");
	fs::write(source2.join("file2.js"), b"// from source2").expect("Failed to write file2");

	let config = StaticFilesConfig {
		static_url: "/static/".to_string(),
		static_root: dest_dir.clone(),
		staticfiles_dirs: vec![source1, source2],
		media_url: Some("/media/".to_string()),
	};

	let options = CollectStaticOptions {
		verbosity: 0,
		enable_hashing: false,
		..Default::default()
	};

	let mut command = CollectStaticCommand::new(config, options);
	let result = command.execute();

	assert!(result.is_ok(), "Execute should succeed ");

	// Both files should be present
	assert!(dest_dir.join("file1.js").exists(), "file1.js should exist ");
	assert!(dest_dir.join("file2.js").exists(), "file2.js should exist ");
}

// ============================================================================
// Edge Case Tests
// ============================================================================

/// Test: CollectStaticCommand file collision (same name in multiple sources)
///
/// Category: Edge Case
/// Verifies that later sources override earlier ones.
#[rstest]
fn test_collectstatic_file_collision(temp_dir: TempDir) {
	let source1 = temp_dir.path().join("source1");
	let source2 = temp_dir.path().join("source2");
	let dest_dir = temp_dir.path().join("static_root");

	fs::create_dir_all(&source1).expect("Failed to create source1");
	fs::create_dir_all(&source2).expect("Failed to create source2");

	// Same filename with different content
	fs::write(source1.join("app.js"), b"// source1 version").expect("Failed to write source1");
	fs::write(source2.join("app.js"), b"// source2 version").expect("Failed to write source2");

	let config = StaticFilesConfig {
		static_url: "/static/".to_string(),
		static_root: dest_dir.clone(),
		staticfiles_dirs: vec![source1, source2],
		media_url: Some("/media/".to_string()),
	};

	let options = CollectStaticOptions {
		verbosity: 0,
		enable_hashing: false,
		..Default::default()
	};

	let mut command = CollectStaticCommand::new(config, options);
	let result = command.execute();

	assert!(result.is_ok(), "Execute should succeed ");

	// File should exist with content from the later source directory
	let content = fs::read_to_string(dest_dir.join("app.js")).expect("Failed to read app.js ");
	assert_eq!(
		content, "// source2 version",
		"later source directory should override earlier one (Django-compatible behavior)"
	);
}

/// Test: CollectStaticCommand no user-configured sources
///
/// Category: Edge Case
/// Verifies handling of empty `staticfiles_dirs`. Auto-discovered app static
/// files (e.g., from `reinhardt-admin`) are still collected because the
/// `inventory`-based discovery runs independently of user configuration.
#[rstest]
fn test_collectstatic_no_sources(temp_dir: TempDir) {
	let dest_dir = temp_dir.path().join("static_root");

	let config = StaticFilesConfig {
		static_url: "/static/".to_string(),
		static_root: dest_dir,
		staticfiles_dirs: vec![],
		media_url: Some("/media/".to_string()),
	};

	let options = CollectStaticOptions {
		verbosity: 0,
		enable_hashing: false,
		..Default::default()
	};

	let mut command = CollectStaticCommand::new(config, options);
	let result = command.execute();

	assert!(
		result.is_ok(),
		"Execute should succeed with no user-configured sources"
	);

	let stats = result.unwrap();
	// Auto-discovered app static files (e.g., reinhardt-admin assets) are
	// collected even when staticfiles_dirs is empty.
	let app_static_count: usize = ::reinhardt_apps::get_app_static_files()
		.iter()
		.map(|config| count_files_recursive(std::path::Path::new(config.static_dir)))
		.sum();
	assert_eq!(
		stats.copied, app_static_count,
		"Should have copied only auto-discovered app static files"
	);
}

// ============================================================================
// Error Path Tests
// ============================================================================

/// Test: CollectStaticCommand with empty STATIC_ROOT
///
/// Category: Error Path
/// Verifies that empty STATIC_ROOT returns an error.
#[rstest]
fn test_collectstatic_empty_root() {
	let config = StaticFilesConfig {
		static_url: "/static/".to_string(),
		static_root: PathBuf::new(), // Empty path
		staticfiles_dirs: vec![],
		media_url: Some("/media/".to_string()),
	};

	let options = CollectStaticOptions {
		verbosity: 0,
		enable_hashing: false,
		..Default::default()
	};

	let mut command = CollectStaticCommand::new(config, options);
	let result = command.execute();

	assert!(
		result.is_err(),
		"Execute should fail with empty STATIC_ROOT"
	);

	let error = result.unwrap_err();
	assert!(
		error.to_string().contains("STATIC_ROOT"),
		"Error should mention STATIC_ROOT"
	);
}

// ============================================================================
// State Transition Tests
// ============================================================================

/// Test: CollectStaticCommand skips unmodified files
///
/// Category: State Transition
/// Verifies that running twice skips unchanged files.
#[rstest]
fn test_collectstatic_unmodified_skip(temp_with_static_files: (TempDir, PathBuf, PathBuf)) {
	let (_temp_dir, source_dir, dest_dir) = temp_with_static_files;

	let config = StaticFilesConfig {
		static_url: "/static/".to_string(),
		static_root: dest_dir.clone(),
		staticfiles_dirs: vec![source_dir],
		media_url: Some("/media/".to_string()),
	};

	// First run
	let options1 = CollectStaticOptions {
		verbosity: 0,
		..Default::default()
	};

	let mut command1 = CollectStaticCommand::new(config.clone(), options1);
	let result1 = command1.execute();
	assert!(result1.is_ok(), "First execute should succeed");

	let stats1 = result1.unwrap();
	assert!(stats1.copied > 0, "First run should copy files");

	// Second run (files should be unmodified)
	let options2 = CollectStaticOptions {
		verbosity: 0,
		..Default::default()
	};

	let mut command2 = CollectStaticCommand::new(config, options2);
	let result2 = command2.execute();
	assert!(result2.is_ok(), "Second execute should succeed");

	let stats2 = result2.unwrap();
	assert!(
		stats2.unmodified > 0 || stats2.copied == 0,
		"Second run should skip unmodified files or copy nothing new"
	);
}

// ============================================================================
// Use Case Tests
// ============================================================================

/// Test: CollectStaticCommand stats accuracy
///
/// Category: Use Case
/// Verifies that stats counts are accurate.
#[rstest]
fn test_collectstatic_stats_accuracy(temp_with_static_files: (TempDir, PathBuf, PathBuf)) {
	let (_temp_dir, source_dir, dest_dir) = temp_with_static_files;

	// Add a file to ignore
	fs::write(source_dir.join("debug.log"), b"log content").expect("Failed to write log");

	let config = StaticFilesConfig {
		static_url: "/static/".to_string(),
		static_root: dest_dir,
		staticfiles_dirs: vec![source_dir],
		media_url: Some("/media/".to_string()),
	};

	let options = CollectStaticOptions {
		ignore_patterns: vec!["*.log".to_string()],
		verbosity: 0,
		..Default::default()
	};

	let mut command = CollectStaticCommand::new(config, options);
	let result = command.execute();

	assert!(result.is_ok(), "Execute should succeed ");

	let stats = result.unwrap();

	// Verify counts make sense
	// We have: app.js, style.css, images/logo.png, debug.log (ignored)
	assert!(
		stats.copied + stats.skipped + stats.unmodified >= 1,
		"Stats should reflect processed files"
	);
	assert!(stats.skipped >= 1, "Should have at least one skipped file");
}

// ============================================================================
// Boundary Value Tests
// ============================================================================

/// Test: CollectStaticCommand large file handling
///
/// Category: Boundary
/// Verifies that large files (> 1MB) are handled correctly.
#[rstest]
fn test_collectstatic_large_file(temp_dir: TempDir) {
	let source_dir = temp_dir.path().join("source");
	let dest_dir = temp_dir.path().join("dest");

	fs::create_dir_all(&source_dir).expect("Failed to create source");

	// Create a 1MB+ file
	let large_content = vec![b'x'; 1024 * 1024 + 100]; // 1MB + 100 bytes
	fs::write(source_dir.join("large.bin"), &large_content).expect("Failed to write large file");

	let config = StaticFilesConfig {
		static_url: "/static/".to_string(),
		static_root: dest_dir.clone(),
		staticfiles_dirs: vec![source_dir],
		media_url: Some("/media/".to_string()),
	};

	let options = CollectStaticOptions {
		verbosity: 0,
		enable_hashing: false,
		..Default::default()
	};

	let mut command = CollectStaticCommand::new(config, options);
	let result = command.execute();

	assert!(result.is_ok(), "Execute should succeed with large file");

	// Verify file was copied correctly
	let copied_size = fs::metadata(dest_dir.join("large.bin"))
		.map(|m| m.len())
		.unwrap_or(0);
	assert_eq!(
		copied_size,
		1024 * 1024 + 100,
		"Large file should be copied with correct size"
	);
}

// ============================================================================
// Decision Table Tests
// ============================================================================

/// Test: CollectStaticOptions flag combinations
///
/// Category: Decision Table
/// Verifies various flag combinations.
#[rstest]
#[case(false, false, false, "no flags")]
#[case(true, false, false, "clear only")]
#[case(false, true, false, "dry_run only")]
#[case(false, false, true, "link only")]
#[case(true, true, false, "clear and dry_run")]
#[case(true, false, true, "clear and link")]
#[case(false, true, true, "dry_run and link")]
#[case(true, true, true, "all flags")]
fn test_collectstatic_decision_flag_combinations(
	#[case] clear: bool,
	#[case] dry_run: bool,
	#[case] link: bool,
	#[case] description: &str,
) {
	let options = CollectStaticOptions {
		clear,
		dry_run,
		link,
		..Default::default()
	};

	assert_eq!(options.clear, clear, "{}: clear mismatch", description);
	assert_eq!(
		options.dry_run, dry_run,
		"{}: dry_run mismatch",
		description
	);
	assert_eq!(options.link, link, "{}: link mismatch", description);
}

// ============================================================================
// Sanity Tests
// ============================================================================

/// Test: CollectStaticCommand basic workflow
///
/// Category: Sanity
/// Verifies the basic workflow of creating and executing a command.
#[rstest]
fn test_collectstatic_sanity_workflow(temp_dir: TempDir) {
	// 1. Create source with file
	let source_dir = temp_dir.path().join("static");
	let dest_dir = temp_dir.path().join("collected");

	fs::create_dir_all(&source_dir).expect("Failed to create source");
	fs::write(source_dir.join("test.js"), b"// test").expect("Failed to write test file");

	// 2. Create config
	let config = StaticFilesConfig {
		static_url: "/static/".to_string(),
		static_root: dest_dir.clone(),
		staticfiles_dirs: vec![source_dir],
		media_url: Some("/media/".to_string()),
	};

	// 3. Create options
	let options = CollectStaticOptions {
		verbosity: 0,
		enable_hashing: false,
		..Default::default()
	};

	// 4. Create command
	let mut command = CollectStaticCommand::new(config, options);

	// 5. Execute
	let result = command.execute();

	// 6. Verify
	assert!(result.is_ok(), "Sanity: execute should succeed");
	let stats = result.unwrap();
	assert!(stats.copied >= 1, "Sanity: should copy at least one file");
	assert!(
		dest_dir.join("test.js").exists(),
		"Sanity: test.js should be collected"
	);
}

// ============================================================================
// Inventory Auto-Discovery Tests
// ============================================================================

/// Test: Inventory auto-discovery integration
///
/// Category: Integration
/// Verifies that get_app_static_files() can be called and returns a valid collection.
/// Note: The actual content depends on what's registered in the test binary.
#[rstest]
fn test_inventory_auto_discovery_callable() {
	// Call inventory getter function
	let app_static_configs = reinhardt_apps::get_app_static_files();

	// Should return a valid Vec (no panic)
	let _count = app_static_configs.len();

	// Verify each config has valid fields
	for config in app_static_configs {
		assert!(
			!config.app_label.is_empty(),
			"App label should not be empty"
		);
		assert!(
			!config.static_dir.is_empty(),
			"Static dir path should not be empty"
		);
		assert!(
			!config.url_prefix.is_empty(),
			"URL prefix should not be empty"
		);
	}
}

/// Test: CollectStaticCommand with inventory auto-discovery
///
/// Category: Integration
/// Verifies that CollectStaticCommand automatically includes directories from inventory.
/// This test creates manual static files and verifies that the command processes them
/// along with any auto-discovered directories (though auto-discovered dirs may be empty
/// in the test environment).
#[rstest]
fn test_collectstatic_with_inventory_auto_discovery(
	temp_with_static_files: (TempDir, PathBuf, PathBuf),
) {
	let (_temp_dir, source_dir, dest_dir) = temp_with_static_files;

	// Create config with only manual staticfiles_dirs
	let config = StaticFilesConfig {
		static_url: "/static/".to_string(),
		static_root: dest_dir.clone(),
		staticfiles_dirs: vec![source_dir.clone()],
		media_url: None,
	};

	// Create options with verbosity to see auto-discovery logs
	let options = CollectStaticOptions {
		verbosity: 2, // Enable verbose logging
		enable_hashing: false,
		..Default::default()
	};

	let mut command = CollectStaticCommand::new(config, options);

	// Execute command
	let result = command.execute();
	assert!(
		result.is_ok(),
		"CollectStatic with auto-discovery should succeed"
	);

	let stats = result.unwrap();

	// Verify that manual static files were collected
	assert!(stats.copied >= 3, "Should copy at least 3 manual files");
	assert!(
		dest_dir.join("app.js").exists(),
		"app.js should be collected"
	);
	assert!(
		dest_dir.join("style.css").exists(),
		"style.css should be collected"
	);
	assert!(
		dest_dir.join("images/logo.png").exists(),
		"images/logo.png should be collected"
	);

	// Note: Auto-discovered directories (if any) would also be processed,
	// but we can't assert on their contents since they depend on what's
	// registered via inventory in the test binary
}

/// Test: CollectStaticCommand skips non-existent auto-discovered directories
///
/// Category: Integration
/// Verifies that auto-discovered directories that don't exist are gracefully skipped.
/// This is important because inventory registrations may point to directories that
/// haven't been built yet (e.g., WASM dist/ directory before building).
#[rstest]
fn test_collectstatic_skips_nonexistent_autodiscovered_dirs(temp_dir: TempDir) {
	let dest_dir = temp_dir.path().join("static_root");

	// Create config with empty staticfiles_dirs
	// All directories will come from inventory (if any)
	let config = StaticFilesConfig {
		static_url: "/static/".to_string(),
		static_root: dest_dir.clone(),
		staticfiles_dirs: vec![],
		media_url: None,
	};

	let options = CollectStaticOptions {
		verbosity: 2,
		..Default::default()
	};

	let mut command = CollectStaticCommand::new(config, options);

	// Execute command - should not fail even if auto-discovered dirs don't exist
	let result = command.execute();
	assert!(
		result.is_ok(),
		"CollectStatic should succeed even with non-existent auto-discovered dirs"
	);

	// Stats may be 0 if no auto-discovered directories exist
	let _stats = result.unwrap();
	// Test passes if execute() succeeded without errors
}

// ============================================================================
// HTML Template STATIC_URL Tests
// ============================================================================

/// Test: HTML template processing uses static_url configuration
///
/// Category: Happy Path
/// Verifies that {{ static_url("...") }} patterns are replaced with the
/// configured static_url prefix, not hardcoded "/".
#[rstest]
fn test_collectstatic_html_template_uses_static_url(temp_dir: TempDir) {
	// Arrange
	let source_dir = temp_dir.path().join("static_source");
	let dest_dir = temp_dir.path().join("static_root");
	let css_dir = source_dir.join("css");
	fs::create_dir_all(&css_dir).expect("Failed to create css dir");
	fs::write(css_dir.join("style.css"), b".app { color: red; }")
		.expect("Failed to write style.css");

	let html_content = r#"<link href="{{ static_url("css/style.css") }}" rel="stylesheet">"#;
	fs::write(source_dir.join("index.html"), html_content).expect("Failed to write index.html");

	let config = StaticFilesConfig {
		static_url: "/assets/".to_string(),
		static_root: dest_dir.clone(),
		staticfiles_dirs: vec![source_dir],
		media_url: None,
	};

	let options = CollectStaticOptions {
		enable_hashing: true,
		verbosity: 0,
		..Default::default()
	};

	let mut command = CollectStaticCommand::new(config, options);

	// Act
	let result = command.execute();
	assert!(result.is_ok(), "CollectStatic should succeed");

	// Assert
	let output_html =
		fs::read_to_string(dest_dir.join("index.html")).expect("Failed to read output HTML");
	assert!(
		output_html.contains("/assets/css/style."),
		"HTML should contain /assets/ prefix, got: {}",
		output_html
	);
}

/// Test: HTML template processing with CDN URL
///
/// Category: Happy Path
/// Verifies that {{ static_url("...") }} patterns work with full CDN URLs.
#[rstest]
fn test_collectstatic_html_template_cdn_url(temp_dir: TempDir) {
	// Arrange
	let source_dir = temp_dir.path().join("static_source");
	let dest_dir = temp_dir.path().join("static_root");
	let css_dir = source_dir.join("css");
	fs::create_dir_all(&css_dir).expect("Failed to create css dir");
	fs::write(css_dir.join("style.css"), b".app { color: blue; }")
		.expect("Failed to write style.css");

	let html_content = r#"<link href="{{ static_url("css/style.css") }}" rel="stylesheet">"#;
	fs::write(source_dir.join("index.html"), html_content).expect("Failed to write index.html");

	let config = StaticFilesConfig {
		static_url: "https://cdn.example.com/static/".to_string(),
		static_root: dest_dir.clone(),
		staticfiles_dirs: vec![source_dir],
		media_url: None,
	};

	let options = CollectStaticOptions {
		enable_hashing: true,
		verbosity: 0,
		..Default::default()
	};

	let mut command = CollectStaticCommand::new(config, options);

	// Act
	let result = command.execute();
	assert!(result.is_ok(), "CollectStatic should succeed");

	// Assert
	let output_html =
		fs::read_to_string(dest_dir.join("index.html")).expect("Failed to read output HTML");
	assert!(
		output_html.contains("https://cdn.example.com/static/css/style."),
		"HTML should contain CDN URL prefix, got: {}",
		output_html
	);
}

// ============================================================================
// Index Source Tests — Refs #2869
// ============================================================================

/// Test: collectstatic with index_source copies the file to static_root
///
/// Category: Happy Path
/// Spec group: G1
#[rstest]
fn test_collectstatic_with_index_source_copies_file() {
	// Arrange
	let dir = tempfile::tempdir().unwrap();
	let static_root = dir.path().join("staticfiles");
	let index_source = dir.path().join("index.html");
	fs::write(&index_source, "<html>test</html>").unwrap();

	let config = StaticFilesConfig {
		static_root: static_root.clone(),
		static_url: "/static/".to_string(),
		staticfiles_dirs: vec![],
		media_url: None,
	};
	let options = CollectStaticOptions {
		no_input: true,
		..Default::default()
	};

	let mut cmd = CollectStaticCommand::new(config, options);
	cmd.set_index_source(Some(index_source));

	// Act
	let stats = cmd.execute().unwrap();

	// Assert
	assert!(static_root.join("index.html").exists());
	assert!(stats.copied >= 1);
}

/// Test: collectstatic with nonexistent index_source returns error
///
/// Category: Error Path
/// Spec group: G2
#[rstest]
fn test_collectstatic_index_source_not_found_returns_error() {
	// Arrange
	let dir = tempfile::tempdir().unwrap();
	let config = StaticFilesConfig {
		static_root: dir.path().join("staticfiles"),
		static_url: "/static/".to_string(),
		staticfiles_dirs: vec![],
		media_url: None,
	};
	let options = CollectStaticOptions {
		no_input: true,
		..Default::default()
	};

	let mut cmd = CollectStaticCommand::new(config, options);
	cmd.set_index_source(Some(PathBuf::from("/nonexistent/index.html")));

	// Act
	let result = cmd.execute();

	// Assert
	assert!(result.is_err());
}

/// Test: collectstatic without index_source uses existing behavior
///
/// Category: Backward Compatibility
/// Spec group: H1
/// Auto-discovered app static files (e.g., from `reinhardt-admin`) are still
/// collected because the `inventory`-based discovery runs independently of
/// user configuration.
#[rstest]
fn test_collectstatic_without_index_source_uses_existing_behavior() {
	// Arrange
	let dir = tempfile::tempdir().unwrap();
	let config = StaticFilesConfig {
		static_root: dir.path().join("staticfiles"),
		static_url: "/static/".to_string(),
		staticfiles_dirs: vec![],
		media_url: None,
	};
	let options = CollectStaticOptions {
		no_input: true,
		..Default::default()
	};

	let mut cmd = CollectStaticCommand::new(config, options);

	// Act
	let stats = cmd.execute().unwrap();

	// Assert
	let app_static_count: usize = ::reinhardt_apps::get_app_static_files()
		.iter()
		.map(|config| count_files_recursive(std::path::Path::new(config.static_dir)))
		.sum();
	assert_eq!(stats.copied, app_static_count);
}
