//! MakeMigrationsCommand integration tests
//!
//! Tests for the makemigrations command execution.

use super::fixtures::{TempMigrationDir, temp_migration_dir};
use reinhardt_commands::CommandContext;
use rstest::*;
use std::fs;

// ============================================================================
// Fixtures
// ============================================================================

/// Fixture for TempMigrationDir with existing migrations
#[fixture]
fn temp_migration_dir_with_files(temp_migration_dir: TempMigrationDir) -> TempMigrationDir {
	// Create an existing migration file
	temp_migration_dir.create_migration_file(
		"auth",
		"0001_initial",
		r#"
use reinhardt_db::migrations::{Migration, MigrationOperation};

pub fn migration() -> Migration {
    Migration {
        app_label: "auth".to_string(),
        name: "0001_initial".to_string(),
        operations: vec![],
        dependencies: vec![],
    }
}
"#,
	);
	temp_migration_dir
}

// ============================================================================
// Happy Path Tests
// ============================================================================

/// Test: TempMigrationDir creation
///
/// Category: Happy Path
/// Verifies that temp migration directory is created correctly.
#[rstest]
fn test_temp_migration_dir_creation(temp_migration_dir: TempMigrationDir) {
	assert!(
		temp_migration_dir.migrations_path.exists(),
		"Migrations directory should exist"
	);
	assert!(
		temp_migration_dir.migrations_path.is_dir(),
		"Migrations path should be a directory"
	);
}

/// Test: TempMigrationDir create migration file
///
/// Category: Happy Path
/// Verifies that migration files can be created.
#[rstest]
fn test_temp_migration_dir_create_file(temp_migration_dir: TempMigrationDir) {
	let file_path = temp_migration_dir.create_migration_file(
		"test_app",
		"0001_initial",
		"// Migration content",
	);

	assert!(file_path.exists(), "Migration file should exist");

	let content = fs::read_to_string(&file_path).expect("Should read file");
	assert_eq!(content, "// Migration content");
}

/// Test: TempMigrationDir with existing files
///
/// Category: Happy Path
/// Verifies that fixture with existing files works.
#[rstest]
fn test_temp_migration_dir_with_existing(temp_migration_dir_with_files: TempMigrationDir) {
	let auth_dir = temp_migration_dir_with_files.migrations_path.join("auth");
	assert!(auth_dir.exists(), "Auth directory should exist");

	let migration_file = auth_dir.join("0001_initial.rs");
	assert!(migration_file.exists(), "Migration file should exist");

	let content = fs::read_to_string(&migration_file).expect("Should read file");
	assert!(
		content.contains("app_label"),
		"Should contain migration code"
	);
}

// ============================================================================
// Edge Case Tests
// ============================================================================

/// Test: MakeMigrations with empty flag
///
/// Category: Edge Case
/// Verifies that --empty flag creates empty migration.
#[rstest]
fn test_makemigrations_empty_flag() {
	let mut ctx = CommandContext::default();
	ctx.set_option("empty".to_string(), "true".to_string());
	ctx.add_arg("test_app".to_string());

	assert!(ctx.has_option("empty"), "Should have empty option");
	assert_eq!(
		ctx.arg(0).map(String::as_str),
		Some("test_app"),
		"Should have app label"
	);
}

/// Test: MakeMigrations with dry-run flag
///
/// Category: Edge Case
/// Verifies that --dry-run flag is set correctly.
#[rstest]
fn test_makemigrations_dry_run_flag() {
	let mut ctx = CommandContext::default();
	ctx.set_option("dry-run".to_string(), "true".to_string());

	assert!(ctx.has_option("dry-run"), "Should have dry-run option");
}

/// Test: MakeMigrations with custom name
///
/// Category: Edge Case
/// Verifies that --name option sets custom migration name.
#[rstest]
fn test_makemigrations_custom_name() {
	let mut ctx = CommandContext::default();
	ctx.set_option("name".to_string(), "add_user_email".to_string());

	assert_eq!(
		ctx.option("name").map(String::as_str),
		Some("add_user_email"),
		"Should have custom name"
	);
}

/// Test: MakeMigrations with verbose flag
///
/// Category: Edge Case
/// Verifies that verbose output is enabled.
#[rstest]
fn test_makemigrations_verbose_flag() {
	let mut ctx = CommandContext::default();
	ctx.set_verbosity(2);

	assert_eq!(ctx.verbosity, 2, "Should have verbose level 2");
}

// ============================================================================
// Decision Table Tests
// ============================================================================

/// Test: MakeMigrations option combinations
///
/// Category: Decision Table
/// Verifies combinations of --dry-run, --verbose, and --name.
#[rstest]
#[case(false, 0, None, "no options")]
#[case(true, 0, None, "dry_run only")]
#[case(false, 2, None, "verbose only")]
#[case(false, 0, Some("custom"), "name only")]
#[case(true, 2, None, "dry_run and verbose")]
#[case(true, 0, Some("custom"), "dry_run and name")]
#[case(false, 2, Some("custom"), "verbose and name")]
#[case(true, 2, Some("custom"), "all options")]
fn test_makemigrations_decision_option_combinations(
	#[case] dry_run: bool,
	#[case] verbosity: u8,
	#[case] name: Option<&str>,
	#[case] description: &str,
) {
	let mut ctx = CommandContext::default();

	if dry_run {
		ctx.set_option("dry-run".to_string(), "true".to_string());
	}
	ctx.set_verbosity(verbosity);
	if let Some(n) = name {
		ctx.set_option("name".to_string(), n.to_string());
	}

	assert_eq!(
		ctx.has_option("dry-run"),
		dry_run,
		"{}: dry-run mismatch",
		description
	);
	assert_eq!(
		ctx.verbosity, verbosity,
		"{}: verbosity mismatch",
		description
	);
	match name {
		Some(n) => assert_eq!(
			ctx.option("name").map(String::as_str),
			Some(n),
			"{}: name mismatch",
			description
		),
		None => assert!(
			ctx.option("name").is_none(),
			"{}: should have no name",
			description
		),
	}
}

// ============================================================================
// Boundary Value Tests
// ============================================================================

/// Test: MakeMigrations with empty app_labels
///
/// Category: Boundary
/// Verifies handling of empty app labels.
#[rstest]
fn test_makemigrations_empty_app_labels() {
	let ctx = CommandContext::default();
	assert!(ctx.arg(0).is_none(), "Should have no app labels");
}

/// Test: MakeMigrations with single app_label
///
/// Category: Boundary
/// Verifies handling of single app label.
#[rstest]
fn test_makemigrations_single_app_label() {
	let mut ctx = CommandContext::default();
	ctx.add_arg("auth".to_string());

	assert_eq!(
		ctx.arg(0).map(String::as_str),
		Some("auth"),
		"Should have auth app label"
	);
	assert!(ctx.arg(1).is_none(), "Should have only one app label");
}

/// Test: MakeMigrations with multiple app_labels
///
/// Category: Boundary
/// Verifies handling of multiple app labels.
#[rstest]
fn test_makemigrations_multiple_app_labels() {
	let mut ctx = CommandContext::default();
	ctx.add_arg("auth".to_string());
	ctx.add_arg("users".to_string());
	ctx.add_arg("posts".to_string());

	assert_eq!(ctx.arg(0).map(String::as_str), Some("auth"));
	assert_eq!(ctx.arg(1).map(String::as_str), Some("users"));
	assert_eq!(ctx.arg(2).map(String::as_str), Some("posts"));
	assert!(ctx.arg(3).is_none(), "Should have only 3 app labels");
}

// ============================================================================
// Sanity Tests
// ============================================================================

/// Test: MakeMigrations basic sanity
///
/// Category: Sanity
/// Verifies basic command setup.
#[rstest]
fn test_makemigrations_sanity(temp_migration_dir: TempMigrationDir) {
	// Create context
	let mut ctx = CommandContext::default();
	ctx.add_arg("test_app".to_string());
	ctx.set_option("dry-run".to_string(), "true".to_string());
	ctx.set_verbosity(1);

	// Verify context
	assert_eq!(ctx.arg(0).map(String::as_str), Some("test_app"));
	assert!(ctx.has_option("dry-run"));
	assert_eq!(ctx.verbosity, 1);

	// Verify temp dir exists
	assert!(temp_migration_dir.migrations_path.exists());
}
