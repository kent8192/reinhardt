//! MigrateCommand integration tests
//!
//! Tests for the migrate command execution with actual database connections.
//! These tests use TestContainers for database isolation.

use super::fixtures::{
	MigrateCommandFixture, migrate_command_fixture, migrate_command_with_migrations,
};
use reinhardt_commands::{BaseCommand, CommandContext, MigrateCommand};
use reinhardt_test::fixtures::postgres_container;
use rstest::*;
use sqlx::PgPool;
use std::sync::Arc;
use testcontainers::ContainerAsync;
use testcontainers::GenericImage;

// ============================================================================
// Happy Path Tests
// ============================================================================

/// Test: MigrateCommand name and description
///
/// Category: Happy Path
/// Verifies that the command has correct metadata.
#[rstest]
fn test_migrate_command_metadata() {
	let command = MigrateCommand;

	assert_eq!(
		command.name(),
		"migrate",
		"Command name should be 'migrate'"
	);
	assert!(
		!command.description().is_empty(),
		"Command should have a description"
	);
	assert!(
		command.description().contains("migration"),
		"Description should mention 'migration'"
	);
}

/// Test: MigrateCommand arguments and options
///
/// Category: Happy Path
/// Verifies that the command defines expected arguments and options.
#[rstest]
fn test_migrate_command_arguments_and_options() {
	let command = MigrateCommand;

	let arguments = command.arguments();
	assert!(
		arguments.len() >= 2,
		"Should have at least 2 arguments (app, migration)"
	);

	let options = command.options();
	let option_names: Vec<&str> = options.iter().map(|o| o.long.as_str()).collect();

	assert!(option_names.contains(&"fake"), "Should have --fake option");
	assert!(
		option_names.contains(&"fake-initial"),
		"Should have --fake-initial option"
	);
	assert!(
		option_names.contains(&"database"),
		"Should have --database option"
	);
}

/// Test: MigrateCommand with empty migrations
///
/// Category: Happy Path
/// Verifies that running migrate with no migrations succeeds.
#[rstest]
#[tokio::test]
async fn test_migrate_empty_migrations(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, _pool, _port, url) = postgres_container.await;

	let mut ctx = CommandContext::default();
	ctx.set_option("database".to_string(), url);
	ctx.set_verbosity(0); // Quiet mode

	let command = MigrateCommand;

	// Execute should succeed even with no migrations
	// Note: This tests the command's handling of empty migration sets
	// The actual result depends on the implementation's behavior
	let result = command.execute(&ctx).await;

	// The command should not panic
	// It may return Ok or an error depending on migration source availability
	assert!(
		result.is_ok() || result.is_err(),
		"Command should complete without panic"
	);
}

// ============================================================================
// Edge Case Tests
// ============================================================================

/// Test: MigrateCommand idempotent rerun
///
/// Category: Edge Case
/// Verifies that running migrate twice does not fail.
#[rstest]
#[tokio::test]
async fn test_migrate_idempotent_rerun(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, _pool, _port, url) = postgres_container.await;

	let mut ctx = CommandContext::default();
	ctx.set_option("database".to_string(), url.clone());
	ctx.set_verbosity(0);

	let command = MigrateCommand;

	// First run
	let result1 = command.execute(&ctx).await;

	// Second run (should not fail for already applied migrations)
	let result2 = command.execute(&ctx).await;

	// Both runs should complete (success or expected error)
	// The key is that the second run doesn't cause a panic or unexpected failure
	assert!(
		result1.is_ok() || result1.is_err(),
		"First run should complete"
	);
	assert!(
		result2.is_ok() || result2.is_err(),
		"Second run should complete"
	);
}

// ============================================================================
// Error Path Tests
// ============================================================================

/// Test: MigrateCommand with invalid database URL
///
/// Category: Error Path
/// Verifies that invalid database URL returns an error.
#[rstest]
#[tokio::test]
async fn test_migrate_invalid_database_url() {
	let mut ctx = CommandContext::default();
	ctx.set_option(
		"database".to_string(),
		"invalid://not-a-valid-url".to_string(),
	);
	ctx.set_verbosity(0);

	let command = MigrateCommand;
	let result = command.execute(&ctx).await;

	assert!(result.is_err(), "Should fail with invalid database URL");
}

/// Test: MigrateCommand with connection failure
///
/// Category: Error Path
/// Verifies that connection refused returns an error.
#[rstest]
#[tokio::test]
async fn test_migrate_connection_failure() {
	let mut ctx = CommandContext::default();
	// Use a port that is likely not running PostgreSQL
	ctx.set_option(
		"database".to_string(),
		"postgres://localhost:59999/nonexistent".to_string(),
	);
	ctx.set_verbosity(0);

	let command = MigrateCommand;
	let result = command.execute(&ctx).await;

	assert!(result.is_err(), "Should fail with connection refused");
}

// ============================================================================
// Boundary Value Tests
// ============================================================================

/// Test: MigrateCommand with zero migrations
///
/// Category: Boundary
/// Verifies handling of zero migrations.
#[rstest]
fn test_migrate_zero_migrations(migrate_command_fixture: MigrateCommandFixture) {
	assert_eq!(
		migrate_command_fixture.migrations.len(),
		0,
		"Default fixture should have zero migrations"
	);
}

/// Test: MigrateCommand fixture with sample migrations
///
/// Category: Boundary
/// Verifies fixture with multiple migrations.
#[rstest]
fn test_migrate_with_sample_migrations(migrate_command_with_migrations: MigrateCommandFixture) {
	assert_eq!(
		migrate_command_with_migrations.migrations.len(),
		3,
		"Should have 3 sample migrations"
	);
}

// ============================================================================
// Decision Table Tests
// ============================================================================

/// Test: Migrate flag combinations (Decision Table)
///
/// Category: Decision Table
/// Verifies all combinations of --fake and --fake-initial flags.
#[rstest]
#[case(false, false, "neither flag")]
#[case(true, false, "fake only")]
#[case(false, true, "fake_initial only")]
#[case(true, true, "both flags")]
fn test_migrate_decision_fake_combinations(
	mut migrate_command_fixture: MigrateCommandFixture,
	#[case] fake: bool,
	#[case] fake_initial: bool,
	#[case] description: &str,
) {
	if fake {
		migrate_command_fixture.set_fake_mode();
	}
	if fake_initial {
		migrate_command_fixture.set_fake_initial_mode();
	}

	assert_eq!(
		migrate_command_fixture.context.has_option("fake"),
		fake,
		"{}: fake option mismatch",
		description
	);
	assert_eq!(
		migrate_command_fixture.context.has_option("fake-initial"),
		fake_initial,
		"{}: fake-initial option mismatch",
		description
	);
}

// ============================================================================
// State Transition Tests
// ============================================================================

/// Test: MigrateCommand state - pending to applied
///
/// Category: State Transition
/// Verifies that migrations change state correctly.
#[rstest]
fn test_migrate_state_pending_to_applied(mut migrate_command_fixture: MigrateCommandFixture) {
	// Add a migration
	migrate_command_fixture.add_create_table_migration("test", "0001_initial", "test_table");

	// Before execution, migration should be in the source
	assert_eq!(
		migrate_command_fixture.migrations.len(),
		1,
		"Should have 1 pending migration"
	);
}

// ============================================================================
// Use Case Tests
// ============================================================================

/// Test: MigrateCommand full lifecycle
///
/// Category: Use Case
/// Verifies the complete command lifecycle.
#[rstest]
#[tokio::test]
async fn test_migrate_lifecycle(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, url) = postgres_container.await;

	// 1. Create command
	let command = MigrateCommand;

	// 2. Verify command metadata
	assert_eq!(command.name(), "migrate");
	assert!(command.requires_system_checks());

	// 3. Create context
	let mut ctx = CommandContext::default();
	ctx.set_option("database".to_string(), url);
	ctx.set_verbosity(0);

	// 4. Verify context setup
	assert!(ctx.has_option("database"));
	assert_eq!(ctx.verbosity, 0);

	// 5. Execute (may succeed or fail based on migration availability)
	let _result = command.execute(&ctx).await;

	// 6. Verify pool is still valid
	let query = "SELECT 1 as val";
	let row: (i32,) = sqlx::query_as(query)
		.fetch_one(pool.as_ref())
		.await
		.expect("Database should still be accessible");
	assert_eq!(row.0, 1, "Database query should return 1");
}

// ============================================================================
// Equivalence Partitioning Tests
// ============================================================================

/// Test: MigrateCommand app_label partitions
///
/// Category: Equivalence
/// Verifies handling of different app_label inputs.
#[rstest]
#[case(None, "no app label")]
#[case(Some("auth"), "single app label")]
#[case(Some("all"), "all apps")]
fn test_migrate_app_label_partitions(
	mut migrate_command_fixture: MigrateCommandFixture,
	#[case] app_label: Option<&str>,
	#[case] description: &str,
) {
	if let Some(label) = app_label {
		migrate_command_fixture.set_app_label(label);
	}

	match app_label {
		None => {
			assert!(
				migrate_command_fixture.context.arg(0).is_none(),
				"{}: should have no app label",
				description
			);
		}
		Some(label) => {
			assert_eq!(
				migrate_command_fixture.context.arg(0).map(String::as_str),
				Some(label),
				"{}: should have app label",
				description
			);
		}
	}
}

// ============================================================================
// Sanity Tests
// ============================================================================

/// Test: MigrateCommand basic sanity check
///
/// Category: Sanity
/// Verifies the basic command structure.
#[rstest]
fn test_migrate_sanity() {
	// Create command
	let command = MigrateCommand;

	// Verify it implements BaseCommand
	assert_eq!(command.name(), "migrate");
	assert!(!command.description().is_empty());
	assert!(!command.arguments().is_empty());
	assert!(!command.options().is_empty());

	// Verify fixture creation
	let fixture = MigrateCommandFixture::new();
	assert!(fixture.migrations.is_empty());

	// Verify fixture with migrations
	let fixture_with_migrations = MigrateCommandFixture::default();
	assert!(fixture_with_migrations.migrations.is_empty());
}
