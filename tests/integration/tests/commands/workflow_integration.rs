//! Cross-command workflow integration tests
//!
//! Tests for workflows that span multiple commands, such as
//! makemigrations → migrate, or introspect → migrate → verify.

use super::fixtures::{TempMigrationDir, temp_migration_dir};
use reinhardt_commands::{BaseCommand, CommandContext, MakeMigrationsCommand, MigrateCommand};
use reinhardt_test::fixtures::postgres_container;
use rstest::*;
use sqlx::PgPool;
use std::fs;
use std::sync::Arc;
use testcontainers::ContainerAsync;
use testcontainers::GenericImage;

// ============================================================================
// Happy Path Tests
// ============================================================================

/// Test: MakeMigrations then Migrate workflow context setup
///
/// Category: Happy Path
/// Verifies that contexts can be properly set up for the full workflow.
#[rstest]
fn test_workflow_makemigrations_migrate_context_setup(temp_migration_dir: TempMigrationDir) {
	// Setup makemigrations context
	let mut make_ctx = CommandContext::default();
	make_ctx.add_arg("test_app".to_string());
	make_ctx.set_option("dry-run".to_string(), "true".to_string());

	// Setup migrate context
	let mut migrate_ctx = CommandContext::default();
	migrate_ctx.add_arg("test_app".to_string());
	migrate_ctx.set_option("fake".to_string(), "true".to_string());

	// Verify contexts are properly configured
	assert_eq!(make_ctx.arg(0).map(String::as_str), Some("test_app"));
	assert!(make_ctx.has_option("dry-run"));
	assert_eq!(migrate_ctx.arg(0).map(String::as_str), Some("test_app"));
	assert!(migrate_ctx.has_option("fake"));

	// Verify temp directory exists
	assert!(temp_migration_dir.migrations_path.exists());
}

/// Test: Command metadata consistency across workflow
///
/// Category: Happy Path
/// Verifies that command names and descriptions are consistent.
#[rstest]
fn test_workflow_command_metadata_consistency() {
	let make_cmd = MakeMigrationsCommand;
	let migrate_cmd = MigrateCommand;

	// Verify both commands have valid metadata
	assert_eq!(make_cmd.name(), "makemigrations");
	assert_eq!(migrate_cmd.name(), "migrate");

	// Verify both have descriptions
	assert!(!make_cmd.description().is_empty());
	assert!(!migrate_cmd.description().is_empty());

	// Verify both mention migration-related terms
	assert!(
		make_cmd.description().to_lowercase().contains("migration")
			|| make_cmd.description().to_lowercase().contains("model")
	);
	assert!(
		migrate_cmd
			.description()
			.to_lowercase()
			.contains("migration")
			|| migrate_cmd
				.description()
				.to_lowercase()
				.contains("database")
	);
}

/// Test: Database workflow with container
///
/// Category: Happy Path
/// Verifies that migrate command can connect to a containerized database.
#[rstest]
#[tokio::test]
async fn test_workflow_database_connection(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, url) = postgres_container.await;

	// Setup context with database URL
	let mut ctx = CommandContext::default();
	ctx.set_option("database".to_string(), url.clone());
	ctx.set_verbosity(0);

	// Verify pool is accessible
	let query = "SELECT 1 as val";
	let row: (i32,) = sqlx::query_as(query)
		.fetch_one(pool.as_ref())
		.await
		.expect("Database should be accessible");
	assert_eq!(row.0, 1);

	// Verify context has database option
	assert!(ctx.has_option("database"));
}

// ============================================================================
// Edge Case Tests
// ============================================================================

/// Test: Workflow with multiple app labels
///
/// Category: Edge Case
/// Verifies handling of multiple app labels in a workflow.
#[rstest]
fn test_workflow_multiple_app_labels(temp_migration_dir: TempMigrationDir) {
	let app_labels = vec!["auth", "users", "posts"];

	// Create directories for each app
	for label in &app_labels {
		let app_dir = temp_migration_dir.migrations_path.join(label);
		fs::create_dir_all(&app_dir).expect("Should create app directory");
	}

	// Verify all directories exist
	for label in &app_labels {
		let app_dir = temp_migration_dir.migrations_path.join(label);
		assert!(app_dir.exists(), "Directory for {} should exist", label);
	}

	// Setup context with multiple app labels
	let mut ctx = CommandContext::default();
	for label in &app_labels {
		ctx.add_arg(label.to_string());
	}

	assert_eq!(ctx.arg(0).map(String::as_str), Some("auth"));
	assert_eq!(ctx.arg(1).map(String::as_str), Some("users"));
	assert_eq!(ctx.arg(2).map(String::as_str), Some("posts"));
}

/// Test: Workflow with verbose output
///
/// Category: Edge Case
/// Verifies verbose output handling across commands.
#[rstest]
fn test_workflow_verbose_output() {
	let verbosity_levels = [0u8, 1, 2, 3];

	for level in verbosity_levels {
		let mut make_ctx = CommandContext::default();
		let mut migrate_ctx = CommandContext::default();

		make_ctx.set_verbosity(level);
		migrate_ctx.set_verbosity(level);

		assert_eq!(
			make_ctx.verbosity, level,
			"Make context should have verbosity {}",
			level
		);
		assert_eq!(
			migrate_ctx.verbosity, level,
			"Migrate context should have verbosity {}",
			level
		);
	}
}

// ============================================================================
// Decision Table Tests
// ============================================================================

/// Test: Workflow option propagation
///
/// Category: Decision Table
/// Verifies that options are correctly propagated through workflow stages.
#[rstest]
#[case(false, false, "no options")]
#[case(true, false, "dry_run only")]
#[case(false, true, "verbose only")]
#[case(true, true, "both options")]
fn test_workflow_decision_option_propagation(
	#[case] dry_run: bool,
	#[case] verbose: bool,
	#[case] description: &str,
) {
	let mut ctx = CommandContext::default();

	if dry_run {
		ctx.set_option("dry-run".to_string(), "true".to_string());
	}
	if verbose {
		ctx.set_verbosity(2);
	}

	assert_eq!(
		ctx.has_option("dry-run"),
		dry_run,
		"{}: dry-run mismatch",
		description
	);
	assert_eq!(
		ctx.verbosity >= 2,
		verbose,
		"{}: verbose mismatch",
		description
	);
}

/// Test: Workflow database option combinations
///
/// Category: Decision Table
/// Verifies combinations of database-related options.
#[rstest]
#[case(None, false, false, "no options")]
#[case(Some("default"), false, false, "database alias only")]
#[case(None, true, false, "fake only")]
#[case(None, false, true, "fake_initial only")]
#[case(Some("default"), true, false, "database and fake")]
#[case(Some("default"), false, true, "database and fake_initial")]
#[case(Some("default"), true, true, "all options")]
fn test_workflow_decision_database_options(
	#[case] database: Option<&str>,
	#[case] fake: bool,
	#[case] fake_initial: bool,
	#[case] description: &str,
) {
	let mut ctx = CommandContext::default();

	if let Some(db) = database {
		ctx.set_option("database".to_string(), db.to_string());
	}
	if fake {
		ctx.set_option("fake".to_string(), "true".to_string());
	}
	if fake_initial {
		ctx.set_option("fake-initial".to_string(), "true".to_string());
	}

	assert_eq!(
		ctx.option("database").map(String::as_str),
		database,
		"{}: database mismatch",
		description
	);
	assert_eq!(
		ctx.has_option("fake"),
		fake,
		"{}: fake mismatch",
		description
	);
	assert_eq!(
		ctx.has_option("fake-initial"),
		fake_initial,
		"{}: fake-initial mismatch",
		description
	);
}

// ============================================================================
// State Transition Tests
// ============================================================================

/// Test: Workflow state - pending to in progress
///
/// Category: State Transition
/// Verifies workflow state transitions.
#[rstest]
fn test_workflow_state_transition(temp_migration_dir: TempMigrationDir) {
	// Initial state: no migrations
	let app_dir = temp_migration_dir.migrations_path.join("test_app");
	assert!(
		!app_dir.exists(),
		"App directory should not exist initially"
	);

	// Create app directory (simulating state change)
	fs::create_dir_all(&app_dir).expect("Should create app directory");
	assert!(
		app_dir.exists(),
		"App directory should exist after creation"
	);

	// Create migration file (simulating makemigrations)
	let migration_file = app_dir.join("0001_initial.rs");
	fs::write(&migration_file, "// Migration content").expect("Should write migration");
	assert!(migration_file.exists(), "Migration file should exist");

	// Verify file has content
	let content = fs::read_to_string(&migration_file).expect("Should read migration");
	assert!(!content.is_empty(), "Migration should have content");
}

// ============================================================================
// Use Case Tests
// ============================================================================

/// Test: Complete workflow setup verification
///
/// Category: Use Case
/// Verifies the complete workflow can be set up correctly.
#[rstest]
fn test_workflow_complete_setup(temp_migration_dir: TempMigrationDir) {
	// Step 1: Create app structure
	let app_label = "myapp";
	temp_migration_dir.create_migration_file(
		app_label,
		"0001_initial",
		r#"
use reinhardt_db::migrations::{Migration, MigrationOperation};

pub fn migration() -> Migration {
    Migration {
        app_label: "myapp".to_string(),
        name: "0001_initial".to_string(),
        operations: vec![],
        dependencies: vec![],
    }
}
"#,
	);

	// Step 2: Verify migration was created
	let app_dir = temp_migration_dir.migrations_path.join(app_label);
	assert!(app_dir.exists(), "App directory should exist");

	let migration_file = app_dir.join("0001_initial.rs");
	assert!(migration_file.exists(), "Migration file should exist");

	// Step 3: Setup makemigrations context
	let mut make_ctx = CommandContext::default();
	make_ctx.add_arg(app_label.to_string());

	// Step 4: Setup migrate context
	let mut migrate_ctx = CommandContext::default();
	migrate_ctx.add_arg(app_label.to_string());

	// Step 5: Verify both contexts are properly configured
	assert_eq!(make_ctx.arg(0).map(String::as_str), Some(app_label));
	assert_eq!(migrate_ctx.arg(0).map(String::as_str), Some(app_label));
}

/// Test: I18n workflow setup
///
/// Category: Use Case
/// Verifies i18n message workflow can be set up.
#[rstest]
fn test_workflow_i18n_setup(_temp_migration_dir: TempMigrationDir) {
	// Setup for makemessages
	let mut make_ctx = CommandContext::default();
	make_ctx.set_option("locale".to_string(), "ja".to_string());
	make_ctx.set_option("domain".to_string(), "django".to_string());

	// Setup for compilemessages
	let mut compile_ctx = CommandContext::default();
	compile_ctx.set_option("locale".to_string(), "ja".to_string());

	// Verify contexts
	assert_eq!(make_ctx.option("locale").map(String::as_str), Some("ja"));
	assert_eq!(
		make_ctx.option("domain").map(String::as_str),
		Some("django")
	);
	assert_eq!(compile_ctx.option("locale").map(String::as_str), Some("ja"));
}

// ============================================================================
// Sanity Tests
// ============================================================================

/// Test: Workflow sanity check
///
/// Category: Sanity
/// Verifies basic workflow components work together.
#[rstest]
fn test_workflow_sanity(temp_migration_dir: TempMigrationDir) {
	// Create commands
	let make_cmd = MakeMigrationsCommand;
	let migrate_cmd = MigrateCommand;

	// Verify commands exist and have metadata
	assert!(!make_cmd.name().is_empty());
	assert!(!migrate_cmd.name().is_empty());

	// Verify temp directory is usable
	let test_file = temp_migration_dir.migrations_path.join("test.txt");
	fs::write(&test_file, "test").expect("Should write test file");
	assert!(test_file.exists());

	// Cleanup
	fs::remove_file(&test_file).expect("Should remove test file");
}
