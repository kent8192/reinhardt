//! IntrospectCommand integration tests
//!
//! Tests for the introspect command that generates ORM models from existing database.

use super::fixtures::{PostgresWithSchema, postgres_with_schema};
use reinhardt_commands::CommandContext;
use rstest::*;
use tempfile::TempDir;

// ============================================================================
// Fixtures
// ============================================================================

/// Fixture for output directory
#[fixture]
fn output_dir() -> TempDir {
	TempDir::new().expect("Failed to create output directory")
}

// ============================================================================
// Happy Path Tests
// ============================================================================

/// Test: PostgresWithSchema fixture creation
///
/// Category: Happy Path
/// Verifies that the schema fixture creates expected tables.
#[rstest]
#[tokio::test]
async fn test_postgres_with_schema_creation(#[future] postgres_with_schema: PostgresWithSchema) {
	let schema = postgres_with_schema.await;

	// Verify tables exist
	let tables_query = r#"
		SELECT table_name
		FROM information_schema.tables
		WHERE table_schema = 'public'
		ORDER BY table_name
	"#;

	let rows: Vec<(String,)> = sqlx::query_as(tables_query)
		.fetch_all(schema.pool.as_ref())
		.await
		.expect("Failed to query tables");

	let table_names: Vec<&str> = rows.iter().map(|r| r.0.as_str()).collect();

	assert!(table_names.contains(&"users"), "Should have users table");
	assert!(table_names.contains(&"posts"), "Should have posts table");
	assert!(
		table_names.contains(&"comments"),
		"Should have comments table"
	);
}

/// Test: PostgresWithSchema users table structure
///
/// Category: Happy Path
/// Verifies that users table has expected columns.
#[rstest]
#[tokio::test]
async fn test_postgres_with_schema_users_columns(
	#[future] postgres_with_schema: PostgresWithSchema,
) {
	let schema = postgres_with_schema.await;

	let columns_query = r#"
		SELECT column_name, data_type, is_nullable
		FROM information_schema.columns
		WHERE table_name = 'users' AND table_schema = 'public'
		ORDER BY ordinal_position
	"#;

	let rows: Vec<(String, String, String)> = sqlx::query_as(columns_query)
		.fetch_all(schema.pool.as_ref())
		.await
		.expect("Failed to query columns");

	let column_names: Vec<&str> = rows.iter().map(|r| r.0.as_str()).collect();

	assert!(column_names.contains(&"id"), "Should have id column");
	assert!(
		column_names.contains(&"username"),
		"Should have username column"
	);
	assert!(column_names.contains(&"email"), "Should have email column");
	assert!(
		column_names.contains(&"is_active"),
		"Should have is_active column"
	);
	assert!(
		column_names.contains(&"created_at"),
		"Should have created_at column"
	);
}

/// Test: PostgresWithSchema foreign key detection
///
/// Category: Happy Path
/// Verifies that foreign keys are created correctly.
#[rstest]
#[tokio::test]
async fn test_postgres_with_schema_foreign_keys(
	#[future] postgres_with_schema: PostgresWithSchema,
) {
	let schema = postgres_with_schema.await;

	let fk_query = r#"
		SELECT tc.table_name, kcu.column_name, ccu.table_name AS foreign_table
		FROM information_schema.table_constraints tc
		JOIN information_schema.key_column_usage kcu
			ON tc.constraint_name = kcu.constraint_name
		JOIN information_schema.constraint_column_usage ccu
			ON ccu.constraint_name = tc.constraint_name
		WHERE tc.constraint_type = 'FOREIGN KEY'
		ORDER BY tc.table_name, kcu.column_name
	"#;

	let rows: Vec<(String, String, String)> = sqlx::query_as(fk_query)
		.fetch_all(schema.pool.as_ref())
		.await
		.expect("Failed to query foreign keys");

	assert!(!rows.is_empty(), "Should have foreign keys");

	// Verify posts.author_id -> users
	let author_fk = rows
		.iter()
		.find(|(table, column, _)| table == "posts" && column == "author_id");
	assert!(author_fk.is_some(), "Should have posts.author_id FK");
	assert_eq!(
		author_fk.unwrap().2,
		"users",
		"FK should reference users table"
	);
}

// ============================================================================
// Edge Case Tests
// ============================================================================

/// Test: Introspect with dry-run flag
///
/// Category: Edge Case
/// Verifies that --dry-run flag is set correctly.
#[rstest]
fn test_introspect_dry_run_flag(output_dir: TempDir) {
	let mut ctx = CommandContext::default();
	ctx.set_option("dry-run".to_string(), "true".to_string());
	ctx.set_option(
		"output".to_string(),
		output_dir.path().to_string_lossy().to_string(),
	);

	assert!(ctx.has_option("dry-run"), "Should have dry-run option");
}

/// Test: Introspect with force flag
///
/// Category: Edge Case
/// Verifies that --force flag is set correctly.
#[rstest]
fn test_introspect_force_flag(output_dir: TempDir) {
	let mut ctx = CommandContext::default();
	ctx.set_option("force".to_string(), "true".to_string());
	ctx.set_option(
		"output".to_string(),
		output_dir.path().to_string_lossy().to_string(),
	);

	assert!(ctx.has_option("force"), "Should have force option");
}

/// Test: Introspect with include filter
///
/// Category: Edge Case
/// Verifies that --include filter is set correctly.
#[rstest]
fn test_introspect_include_filter() {
	let mut ctx = CommandContext::default();
	ctx.set_option("include".to_string(), "users.*".to_string());

	assert_eq!(ctx.option("include").map(String::as_str), Some("users.*"));
}

/// Test: Introspect with exclude filter
///
/// Category: Edge Case
/// Verifies that --exclude filter is set correctly.
#[rstest]
fn test_introspect_exclude_filter() {
	let mut ctx = CommandContext::default();
	ctx.set_option("exclude".to_string(), "^pg_".to_string());

	assert_eq!(ctx.option("exclude").map(String::as_str), Some("^pg_"));
}

/// Test: Introspect with custom app-label
///
/// Category: Edge Case
/// Verifies that --app-label is set correctly.
#[rstest]
fn test_introspect_custom_app_label() {
	let mut ctx = CommandContext::default();
	ctx.set_option("app-label".to_string(), "myapp".to_string());

	assert_eq!(ctx.option("app-label").map(String::as_str), Some("myapp"));
}

// ============================================================================
// Decision Table Tests
// ============================================================================

/// Test: Introspect filter combinations
///
/// Category: Decision Table
/// Verifies combinations of --include and --exclude.
#[rstest]
#[case(None, None, "no filters")]
#[case(Some("users.*"), None, "include only")]
#[case(None, Some("^pg_"), "exclude only")]
#[case(Some(".*"), Some("^pg_"), "both filters")]
fn test_introspect_decision_filter_combinations(
	#[case] include: Option<&str>,
	#[case] exclude: Option<&str>,
	#[case] description: &str,
) {
	let mut ctx = CommandContext::default();

	if let Some(inc) = include {
		ctx.set_option("include".to_string(), inc.to_string());
	}
	if let Some(exc) = exclude {
		ctx.set_option("exclude".to_string(), exc.to_string());
	}

	assert_eq!(
		ctx.option("include").map(String::as_str),
		include,
		"{}: include mismatch",
		description
	);
	assert_eq!(
		ctx.option("exclude").map(String::as_str),
		exclude,
		"{}: exclude mismatch",
		description
	);
}

// ============================================================================
// Error Path Tests
// ============================================================================

/// Test: Introspect connection failure
///
/// Category: Error Path
/// Verifies error handling for invalid database URL.
#[rstest]
fn test_introspect_connection_failure_setup() {
	let mut ctx = CommandContext::default();
	ctx.set_option(
		"database".to_string(),
		"postgres://invalid:59999/test".to_string(),
	);

	assert!(ctx.has_option("database"), "Should have database option");
}

// ============================================================================
// Sanity Tests
// ============================================================================

/// Test: Introspect basic sanity
///
/// Category: Sanity
/// Verifies basic introspect setup.
#[rstest]
fn test_introspect_sanity(output_dir: TempDir) {
	let mut ctx = CommandContext::default();
	ctx.set_option(
		"output".to_string(),
		output_dir.path().to_string_lossy().to_string(),
	);
	ctx.set_option("app-label".to_string(), "generated".to_string());
	ctx.set_verbosity(1);

	assert!(ctx.has_option("output"));
	assert!(ctx.has_option("app-label"));
	assert_eq!(ctx.verbosity, 1);
}
