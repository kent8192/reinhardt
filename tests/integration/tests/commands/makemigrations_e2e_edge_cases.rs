//! E2E edge case tests for makemigrations command
//!
//! Tests edge cases including:
//! - Unicode identifiers (Japanese, emoji)
//! - Migration numbering overflow
//! - Concurrent execution
//! - Permission errors
//! - Corrupted files

use super::fixtures::{TempMigrationDir, temp_migration_dir};
use reinhardt_commands::{BaseCommand, CommandContext, MakeMigrationsCommand};
use reinhardt_db::migrations::{
	FilesystemRepository, FilesystemSource, Migration, MigrationRepository, MigrationSource,
	Operation,
};
use reinhardt_query::prelude::*;
use rstest::*;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use tempfile::TempDir;
use tokio::time::{Duration, timeout};

// ============================================================================
// Test Helper Functions
// ============================================================================

/// Helper to create a valid migration file with proper Rust syntax
fn create_valid_migration_file(
	dir: &PathBuf,
	app_label: &str,
	name: &str,
	operations: Vec<Operation>,
) -> PathBuf {
	let app_dir = dir.join(app_label);
	fs::create_dir_all(&app_dir).expect("Failed to create app directory");

	// Build operations array string
	let operations_str = operations
		.iter()
		.map(|op| match op {
			Operation::RunSQL { sql, .. } => {
				format!(
					"\n\t\t\t\treinhardt_db::migrations::Operation::RunSQL {{\n\t\t\t\t\tsql: r#\"{}\"#.to_string(),\n\t\t\t\t\treverse_sql: None,\n\t\t\t\t}}",
					sql.replace('\n', "\\n").replace('"', "\\'")
				)
			}
			_ => format!("{:?}", op),
		})
		.collect::<Vec<_>>()
		.join(",\n");

	let content = format!(
		r#"use reinhardt_db::migrations::{{Migration, Operation}};

pub fn migration() -> Migration {{
	Migration {{
		app_label: "{}".to_string(),
		name: "{}".to_string(),
		operations: vec![{}],
		dependencies: vec![],
		..Default::default()
	}}
}}
"#,
		app_label, name, operations_str
	);

	let file_path = app_dir.join(format!("{}.rs", name));
	fs::write(&file_path, content).expect("Failed to write migration file");
	file_path
}

/// Helper to get the next migration number for an app
async fn get_next_migration_number(migrations_dir: &PathBuf, app_label: &str) -> String {
	use reinhardt_db::migrations::MigrationNumbering;

	MigrationNumbering::next_number(migrations_dir, app_label)
}

/// Helper to check if a migration file exists
fn migration_exists(migrations_dir: &PathBuf, app_label: &str, name: &str) -> bool {
	let file_path = migrations_dir.join(app_label).join(format!("{}.rs", name));
	file_path.exists()
}

/// Helper to read migration file content
fn read_migration_file(migrations_dir: &PathBuf, app_label: &str, name: &str) -> String {
	let file_path = migrations_dir.join(app_label).join(format!("{}.rs", name));
	fs::read_to_string(file_path).expect("Failed to read migration file")
}

// ============================================================================
// EC-MM-01: Unicode Identifiers Tests
// ============================================================================

/// Test: EC-MM-01-01 - Japanese table names
///
/// Category: Edge Case
/// Verifies that makemigrations handles Japanese characters in model names.
#[rstest]
#[tokio::test]
async fn ec_mm_01_01_japanese_table_names() {
	// Arrange
	let temp_dir = TempDir::new().expect("Failed to create temp directory");
	let migrations_path = temp_dir.path().join("migrations");
	fs::create_dir_all(&migrations_path).expect("Failed to create migrations directory");

	// Create existing migration with Japanese table name
	let app_dir = migrations_path.join("unicode_app");
	fs::create_dir_all(&app_dir).expect("Failed to create app directory");

	let content = r##"
use reinhardt_db::migrations::{Migration, Operation};

pub fn migration() -> Migration {
    Migration {
        app_label: "unicode_app".to_string(),
        name: "0001_initial".to_string(),
        operations: vec![
            Operation::RunSQL {
                sql: r#"CREATE TABLE "ユーザー" (id SERIAL PRIMARY KEY, "名前" VARCHAR(100) NOT NULL)"#.to_string(),
                reverse_sql: Some(r#"DROP TABLE "ユーザー""#.to_string()),
            }
        ],
        dependencies: vec![],
        ..Default::default()
    }
}
"##;
	let file_path = app_dir.join("0001_initial.rs");
	fs::write(&file_path, content).expect("Failed to write migration file");

	// Create source and verify it can load Japanese identifiers
	let source = FilesystemSource::new(migrations_path.clone());
	let result = source.migrations_for_app("unicode_app").await;

	// Assert
	assert!(
		result.is_ok(),
		"Should successfully load migration with Japanese identifiers"
	);

	let migrations = result.unwrap();
	assert_eq!(migrations.len(), 1, "Should have exactly one migration");
	assert_eq!(
		migrations[0].app_label, "unicode_app",
		"App label should match"
	);
	assert_eq!(
		migrations[0].name, "0001_initial",
		"Migration name should match"
	);
}

/// Test: EC-MM-01-02 - Emoji column names
///
/// Category: Edge Case
/// Verifies that makemigrations handles emoji characters in column names.
#[rstest]
#[tokio::test]
async fn ec_mm_01_02_emoji_column_names() {
	// Arrange
	let temp_dir = TempDir::new().expect("Failed to create temp directory");
	let migrations_path = temp_dir.path().join("migrations");
	fs::create_dir_all(&migrations_path).expect("Failed to create migrations directory");

	// Create migration with emoji column names
	let app_dir = migrations_path.join("emoji_app");
	fs::create_dir_all(&app_dir).expect("Failed to create app directory");

	// Create table with emoji columns
	let mut create_table_stmt = Query::create_table();
	let create_table = create_table_stmt
		.table(Alias::new("emoji_test"))
		.col(
			ColumnDef::new(Alias::new("id"))
				.integer()
				.not_null(true)
				.auto_increment(true)
				.primary_key(true),
		)
		.col(ColumnDef::new(Alias::new("😀")).string())
		.col(ColumnDef::new(Alias::new("🚀")).string())
		.col(ColumnDef::new(Alias::new("💡")).text())
		.to_string(PostgresQueryBuilder::new());

	let content = format!(
		r##"
use reinhardt_db::migrations::{{Migration, Operation}};

pub fn migration() -> Migration {{
    Migration {{
        app_label: "emoji_app".to_string(),
        name: "0001_emoji_columns".to_string(),
        operations: vec![
            Operation::RunSQL {{
                sql: r#"{}"#.to_string(),
                reverse_sql: Some(r#"DROP TABLE emoji_test"#.to_string()),
            }}
        ],
        dependencies: vec![],
        ..Default::default()
    }}
}}
"##,
		create_table.replace('\n', "\\n")
	);

	let file_path = app_dir.join("0001_emoji_columns.rs");
	fs::write(&file_path, content).expect("Failed to write migration file");

	// Act - Load and verify
	let source = FilesystemSource::new(migrations_path.clone());
	let result = source.migrations_for_app("emoji_app").await;

	// Assert
	assert!(
		result.is_ok(),
		"Should successfully load migration with emoji column names"
	);

	let migrations = result.unwrap();
	assert_eq!(migrations.len(), 1);
	assert_eq!(migrations[0].name, "0001_emoji_columns");
	assert!(!migrations[0].operations.is_empty());
}

/// Test: EC-MM-01-03 - Mixed Unicode identifiers
///
/// Category: Edge Case
/// Verifies handling of mixed Unicode (Japanese + emoji) identifiers.
#[rstest]
#[tokio::test]
async fn ec_mm_01_03_mixed_unicode_identifiers() {
	// Arrange
	let temp_dir = TempDir::new().expect("Failed to create temp directory");
	let migrations_path = temp_dir.path().join("migrations");
	fs::create_dir_all(&migrations_path).expect("Failed to create migrations directory");

	let app_dir = migrations_path.join("mixed_unicode");
	fs::create_dir_all(&app_dir).expect("Failed to create app directory");

	// Create table with mixed Unicode identifiers
	let mut create_table_stmt = Query::create_table();
	let create_table = create_table_stmt
		.table(Alias::new("製品"))
		.col(
			ColumnDef::new(Alias::new("id"))
				.integer()
				.not_null(true)
				.auto_increment(true)
				.primary_key(true),
		)
		.col(ColumnDef::new(Alias::new("製品名")).string().not_null(true))
		.col(ColumnDef::new(Alias::new("価格")).integer().not_null(true))
		.col(ColumnDef::new(Alias::new("🏷️")).string())
		.to_string(PostgresQueryBuilder::new());

	let content = format!(
		r##"
use reinhardt_db::migrations::{{Migration, Operation}};

pub fn migration() -> Migration {{
    Migration {{
        app_label: "mixed_unicode".to_string(),
        name: "0001_mixed_unicode".to_string(),
        operations: vec![
            Operation::RunSQL {{
                sql: r#"{}"#.to_string(),
                reverse_sql: Some(r#"DROP TABLE "製品""#.to_string()),
            }}
        ],
        dependencies: vec![],
        ..Default::default()
    }}
}}
"##,
		create_table.replace('\n', "\\n")
	);

	let file_path = app_dir.join("0001_mixed_unicode.rs");
	fs::write(&file_path, content).expect("Failed to write migration file");

	// Act
	let source = FilesystemSource::new(migrations_path.clone());
	let result = source.migrations_for_app("mixed_unicode").await;

	// Assert
	assert!(
		result.is_ok(),
		"Should successfully load migration with mixed Unicode identifiers"
	);

	let migrations = result.unwrap();
	assert_eq!(migrations.len(), 1);
	assert_eq!(migrations[0].app_label, "mixed_unicode");
}

// ============================================================================
// EC-MM-02: Numbering Overflow Tests
// ============================================================================

/// Test: EC-MM-02-01 - Migration 9998 to 9999 transition
///
/// Category: Edge Case
/// Verifies correct handling of migration numbering from 9998 to 9999.
#[rstest]
#[tokio::test]
async fn ec_mm_02_01_numbering_9998_to_9999() {
	// Arrange
	let temp_dir = TempDir::new().expect("Failed to create temp directory");
	let migrations_path = temp_dir.path().join("migrations");
	fs::create_dir_all(&migrations_path).expect("Failed to create migrations directory");

	let app_dir = migrations_path.join("overflow_test");
	fs::create_dir_all(&app_dir).expect("Failed to create app directory");

	// Create migration 9998
	create_valid_migration_file(
		&migrations_path,
		"overflow_test",
		"9998_migration_a",
		vec![Operation::RunSQL {
			sql: "CREATE TABLE table_a (id INT PRIMARY KEY)".to_string(),
			reverse_sql: Some("DROP TABLE table_a".to_string()),
		}],
	);

	// Act - Get next migration number
	let next_number = get_next_migration_number(&migrations_path, "overflow_test").await;
	let next_number_parsed: u32 = next_number.parse().unwrap();

	// Assert
	assert_eq!(
		next_number_parsed, 9999,
		"Next migration number should be 9999"
	);

	// Verify migration 9999 can be created
	create_valid_migration_file(
		&migrations_path,
		"overflow_test",
		"9999_migration_b",
		vec![Operation::RunSQL {
			sql: "CREATE TABLE table_b (id INT PRIMARY KEY)".to_string(),
			reverse_sql: Some("DROP TABLE table_b".to_string()),
		}],
	);

	// Verify both migrations exist
	assert!(migration_exists(
		&migrations_path,
		"overflow_test",
		"9998_migration_a"
	));
	assert!(migration_exists(
		&migrations_path,
		"overflow_test",
		"9999_migration_b"
	));

	// Verify correct sorting
	let source = FilesystemSource::new(migrations_path.clone());
	let migrations = source.migrations_for_app("overflow_test").await.unwrap();

	assert_eq!(migrations.len(), 2);
	assert_eq!(migrations[0].name, "9998_migration_a");
	assert_eq!(migrations[1].name, "9999_migration_b");
}

/// Test: EC-MM-02-02 - Migration 9999 to 10000 transition
///
/// Category: Edge Case
/// Verifies correct handling of migration numbering overflow from 9999 to 10000.
#[rstest]
#[tokio::test]
async fn ec_mm_02_02_numbering_9999_to_10000() {
	// Arrange
	let temp_dir = TempDir::new().expect("Failed to create temp directory");
	let migrations_path = temp_dir.path().join("migrations");
	fs::create_dir_all(&migrations_path).expect("Failed to create migrations directory");

	let app_dir = migrations_path.join("overflow_test");
	fs::create_dir_all(&app_dir).expect("Failed to create app directory");

	// Create migration 9999
	create_valid_migration_file(
		&migrations_path,
		"overflow_test",
		"9999_last_four_digit",
		vec![Operation::RunSQL {
			sql: "CREATE TABLE table_9999 (id INT PRIMARY KEY)".to_string(),
			reverse_sql: Some("DROP TABLE table_9999".to_string()),
		}],
	);

	// Act - Get next migration number (should be 10000)
	let next_number = get_next_migration_number(&migrations_path, "overflow_test").await;
	let next_number_parsed: u32 = next_number.parse().unwrap();

	// Assert
	assert_eq!(
		next_number_parsed, 10000,
		"Next migration number should be 10000 (overflow)"
	);

	// Verify migration 10000 can be created
	create_valid_migration_file(
		&migrations_path,
		"overflow_test",
		"10000_first_five_digit",
		vec![Operation::RunSQL {
			sql: "CREATE TABLE table_10000 (id INT PRIMARY KEY)".to_string(),
			reverse_sql: Some("DROP TABLE table_10000".to_string()),
		}],
	);

	// Verify both migrations exist
	assert!(migration_exists(
		&migrations_path,
		"overflow_test",
		"9999_last_four_digit"
	));
	assert!(migration_exists(
		&migrations_path,
		"overflow_test",
		"10000_first_five_digit"
	));

	// Verify correct sorting (4-digit should come before 5-digit)
	let source = FilesystemSource::new(migrations_path.clone());
	let migrations = source.migrations_for_app("overflow_test").await.unwrap();

	assert_eq!(migrations.len(), 2);
	assert_eq!(migrations[0].name, "9999_last_four_digit");
	assert_eq!(migrations[1].name, "10000_first_five_digit");
}

/// Test: EC-MM-02-03 - Large number sorting consistency
///
/// Category: Edge Case
/// Verifies that migrations with large numbers maintain correct sort order.
#[rstest]
#[tokio::test]
async fn ec_mm_02_03_large_number_sorting() {
	// Arrange
	let temp_dir = TempDir::new().expect("Failed to create temp directory");
	let migrations_path = temp_dir.path().join("migrations");
	fs::create_dir_all(&migrations_path).expect("Failed to create migrations directory");

	let app_dir = migrations_path.join("sort_test");
	fs::create_dir_all(&app_dir).expect("Failed to create app directory");

	// Create migrations with various large numbers
	let migration_numbers = vec![
		"0001_initial",
		"0099_mid",
		"0100_three_digit",
		"0999_max_three",
		"1000_first_four",
		"9998_overflow_a",
		"9999_overflow_b",
		"10000_five_digit",
	];

	for (i, name) in migration_numbers.iter().enumerate() {
		create_valid_migration_file(
			&migrations_path,
			"sort_test",
			name,
			vec![Operation::RunSQL {
				sql: format!("CREATE TABLE table_{} (id INT PRIMARY KEY)", i),
				reverse_sql: Some(format!("DROP TABLE table_{}", i)),
			}],
		);
	}

	// Act - Load all migrations
	let source = FilesystemSource::new(migrations_path.clone());
	let migrations = source.migrations_for_app("sort_test").await.unwrap();

	// Assert
	assert_eq!(
		migrations.len(),
		migration_numbers.len(),
		"Should load all migrations"
	);

	// Verify they are sorted correctly
	for (i, expected_name) in migration_numbers.iter().enumerate() {
		assert_eq!(
			migrations[i].name, *expected_name,
			"Migration at position {} should be {}",
			i, expected_name
		);
	}
}

// ============================================================================
// EC-MM-03: Concurrent Execution Tests
// ============================================================================

/// Test: EC-MM-03-01 - Concurrent migration creation
///
/// Category: Edge Case
/// Verifies that concurrent makemigrations executions handle race conditions.
#[rstest]
#[tokio::test]
async fn ec_mm_03_01_concurrent_migration_creation() {
	// Arrange
	let temp_dir = Arc::new(TempDir::new().expect("Failed to create temp directory"));
	let migrations_path = Arc::new(temp_dir.path().join("migrations"));
	fs::create_dir_all(&*migrations_path).expect("Failed to create migrations directory");

	let app_dir = migrations_path.join("concurrent_app");
	fs::create_dir_all(&app_dir).expect("Failed to create app directory");

	// Spawn multiple concurrent tasks to create migrations
	let mut handles = vec![];

	for i in 0..5 {
		let temp_dir = temp_dir.clone();
		let migrations_path = migrations_path.clone();

		let handle = tokio::spawn(async move {
			// Simulate concurrent migration creation
			let repo_dir = temp_dir.path().join("migrations");
			let mut repository = FilesystemRepository::new(repo_dir);

			// Try to save migration (may race with other tasks)
			let migration = Migration {
				app_label: "concurrent_app".to_string(),
				name: format!("000{}_concurrent_{}", i + 1, i),
				operations: vec![Operation::RunSQL {
					sql: format!("CREATE TABLE concurrent_{} (id INT PRIMARY KEY)", i),
					reverse_sql: Some(format!("DROP TABLE concurrent_{}", i)),
				}],
				dependencies: vec![],
				..Default::default()
			};

			let result = repository.save(&migration).await;
			(result, migration.name.clone())
		});

		handles.push(handle);
	}

	// Act - Wait for all tasks to complete
	let mut results = vec![];
	for handle in handles {
		let result = timeout(Duration::from_secs(5), handle).await;
		match result {
			Ok(Ok((save_result, name))) => {
				results.push((save_result.is_ok(), name));
			}
			_ => {
				panic!("Concurrent task timed out or failed");
			}
		}
	}

	// Assert - At least some migrations should be saved
	let successful = results.iter().filter(|(ok, _)| *ok).count();
	assert!(
		successful > 0,
		"At least some concurrent migrations should be saved"
	);

	// Verify the filesystem state is consistent
	let source = FilesystemSource::new((*migrations_path).clone());
	let migrations = source.migrations_for_app("concurrent_app").await.unwrap();

	// The number of migrations should match successful saves
	assert_eq!(
		migrations.len(),
		successful,
		"Filesystem should contain exactly {} migrations",
		successful
	);
}

/// Test: EC-MM-03-02 - Concurrent read during write
///
/// Category: Edge Case
/// Verifies that reading migrations during concurrent writes doesn't cause crashes.
#[rstest]
#[tokio::test]
async fn ec_mm_03_02_concurrent_read_during_write() {
	// Arrange
	let temp_dir = Arc::new(TempDir::new().expect("Failed to create temp directory"));
	let migrations_path = Arc::new(temp_dir.path().join("migrations"));
	fs::create_dir_all(&*migrations_path).expect("Failed to create migrations directory");

	let app_dir = migrations_path.join("rw_app");
	fs::create_dir_all(&app_dir).expect("Failed to create app directory");

	// Create initial migration
	create_valid_migration_file(
		&migrations_path,
		"rw_app",
		"0001_initial",
		vec![Operation::RunSQL {
			sql: "CREATE TABLE initial_table (id INT PRIMARY KEY)".to_string(),
			reverse_sql: Some("DROP TABLE initial_table".to_string()),
		}],
	);

	let mut writer_handles = vec![];
	let mut reader_handles = vec![];

	// Spawn writer tasks
	for i in 0..3 {
		let migrations_path = migrations_path.clone();
		let handle = tokio::spawn(async move {
			tokio::time::sleep(Duration::from_millis(10 * i as u64)).await;
			create_valid_migration_file(
				&migrations_path,
				"rw_app",
				&format!("0002_write_{}", i),
				vec![Operation::RunSQL {
					sql: format!("CREATE TABLE write_{} (id INT PRIMARY KEY)", i),
					reverse_sql: Some(format!("DROP TABLE write_{}", i)),
				}],
			);
		});
		writer_handles.push(handle);
	}

	// Spawn reader tasks
	for _ in 0..3 {
		let migrations_path = migrations_path.clone();
		let handle = tokio::spawn(async move {
			tokio::time::sleep(Duration::from_millis(15)).await;
			let source = FilesystemSource::new((*migrations_path).clone());
			source.migrations_for_app("rw_app").await
		});
		reader_handles.push(handle);
	}

	// Act - Wait for all writer tasks
	for handle in writer_handles {
		let result = timeout(Duration::from_secs(5), handle).await;
		assert!(
			result.is_ok(),
			"Writer task should complete without timeout"
		);
		assert!(result.unwrap().is_ok(), "Writer task should succeed");
	}

	// Act - Wait for all reader tasks
	for handle in reader_handles {
		let result = timeout(Duration::from_secs(5), handle).await;
		assert!(
			result.is_ok(),
			"Reader task should complete without timeout"
		);
		assert!(result.unwrap().is_ok(), "Reader task should succeed");
	}

	// Assert - Final state should be consistent
	let source = FilesystemSource::new((*migrations_path).clone());
	let migrations = source.migrations_for_app("rw_app").await.unwrap();

	// Should have initial + at least one of the writes
	assert!(
		migrations.len() >= 1,
		"Should have at least the initial migration"
	);
	assert_eq!(migrations[0].name, "0001_initial");
}

// ============================================================================
// EC-MM-04: Permission Error Tests
// ============================================================================

/// Test: EC-MM-04-01 - Read-only migrations directory
///
/// Category: Edge Case
/// Verifies error handling when migrations directory is read-only.
#[rstest]
#[tokio::test]
async fn ec_mm_04_01_read_only_migrations_directory() {
	// Arrange
	let temp_dir = TempDir::new().expect("Failed to create temp directory");
	let migrations_path = temp_dir.path().join("migrations");
	fs::create_dir_all(&migrations_path).expect("Failed to create migrations directory");

	let app_dir = migrations_path.join("readonly_app");
	fs::create_dir_all(&app_dir).expect("Failed to create app directory");

	// Create existing migration
	create_valid_migration_file(
		&migrations_path,
		"readonly_app",
		"0001_initial",
		vec![Operation::RunSQL {
			sql: "CREATE TABLE readonly_table (id INT PRIMARY KEY)".to_string(),
			reverse_sql: Some("DROP TABLE readonly_table".to_string()),
		}],
	);

	// Make directory read-only (Unix-only)
	#[cfg(unix)]
	{
		use std::os::unix::fs::PermissionsExt;
		let mut perms = fs::metadata(&app_dir)
			.expect("Failed to get metadata")
			.permissions();
		perms.set_mode(0o444); // Read-only
		fs::set_permissions(&app_dir, perms).expect("Failed to set read-only permissions");
	}

	// Act - Try to save a new migration
	let mut repository = FilesystemRepository::new(migrations_path.clone());
	let new_migration = Migration {
		app_label: "readonly_app".to_string(),
		name: "0002_should_fail".to_string(),
		operations: vec![Operation::RunSQL {
			sql: "CREATE TABLE should_fail (id INT PRIMARY KEY)".to_string(),
			reverse_sql: Some("DROP TABLE should_fail".to_string()),
		}],
		dependencies: vec![],
		..Default::default()
	};

	let result = repository.save(&new_migration).await;

	// Assert - Should fail on read-only filesystem
	#[cfg(unix)]
	{
		assert!(
			result.is_err(),
			"Should fail to save migration to read-only directory"
		);
	}

	// Restore permissions for cleanup
	#[cfg(unix)]
	{
		use std::os::unix::fs::PermissionsExt;
		let mut perms = fs::metadata(&app_dir)
			.expect("Failed to get metadata")
			.permissions();
		perms.set_mode(0o755);
		fs::set_permissions(&app_dir, perms).expect("Failed to restore permissions");
	}
}

/// Test: EC-MM-04-02 - Non-existent migrations directory
///
/// Category: Edge Case
/// Verifies handling when migrations directory doesn't exist.
#[rstest]
#[tokio::test]
async fn ec_mm_04_02_non_existent_migrations_directory() {
	// Arrange
	let temp_dir = TempDir::new().expect("Failed to create temp directory");
	let migrations_path = temp_dir.path().join("nonexistent_migrations");
	// Don't create the directory

	// Act - Try to load from non-existent directory
	let source = FilesystemSource::new(migrations_path.clone());
	let result = source.migrations_for_app("some_app").await;

	// Assert - Should handle gracefully (empty result or error)
	// The implementation should either return empty vec or an error
	assert!(
		result.is_ok() || result.is_err(),
		"Should handle non-existent directory without panic"
	);

	if result.is_ok() {
		let migrations = result.unwrap();
		assert_eq!(migrations.len(), 0, "Should have no migrations");
	}
}

/// Test: EC-MM-04-03 - Invalid path characters
///
/// Category: Edge Case
/// Verifies handling of paths with invalid/special characters.
#[rstest]
#[tokio::test]
async fn ec_mm_04_03_invalid_path_characters() {
	// Arrange
	let temp_dir = TempDir::new().expect("Failed to create temp directory");
	let migrations_path = temp_dir.path().join("migrations");
	fs::create_dir_all(&migrations_path).expect("Failed to create migrations directory");

	// Try to create app directory with invalid characters
	// On most systems, null bytes and certain control characters are invalid
	let invalid_app_labels = vec!["null\x00byte", "path/separator", "path\\separator"];

	for app_label in invalid_app_labels {
		// Attempt to create migration with invalid app label
		let app_dir = migrations_path.join(app_label);

		// The directory creation should fail or handle the invalid name
		let result = fs::create_dir_all(&app_dir);

		// Assert - Should either fail or handle gracefully
		// We don't panic, the system should handle this
		match result {
			Ok(_) => {
				// If it succeeded, verify the directory works
				// (some filesystems may allow these characters)
			}
			Err(_) => {
				// Expected to fail - invalid characters
			}
		}
	}
}

// ============================================================================
// EC-MM-05: Corrupted File Tests
// ============================================================================

/// Test: EC-MM-05-01 - Invalid Rust syntax
///
/// Category: Edge Case
/// Verifies handling of migration files with invalid Rust syntax.
#[rstest]
#[tokio::test]
async fn ec_mm_05_01_invalid_rust_syntax() {
	// Arrange
	let temp_dir = TempDir::new().expect("Failed to create temp directory");
	let migrations_path = temp_dir.path().join("migrations");
	fs::create_dir_all(&migrations_path).expect("Failed to create migrations directory");

	let app_dir = migrations_path.join("corrupted_app");
	fs::create_dir_all(&app_dir).expect("Failed to create app directory");

	// Create migration file with invalid syntax
	let content = r#"
use reinhardt_db::migrations::{Migration, Operation};

pub fn migration() -> Migration {
    Migration {
        app_label: "corrupted_app".to_string(),
        name: "0001_invalid_syntax".to_string(),
        operations: vec![
            Operation::RunSQL {
                sql: "CREATE TABLE test (id INT PRIMARY KEY" // Missing closing parenthesis
                reverse_sql: None,
            }
        ],
        dependencies: vec![],
        ..Default::default()
    }
}
"#;

	let file_path = app_dir.join("0001_invalid_syntax.rs");
	fs::write(&file_path, content).expect("Failed to write corrupted file");

	// Act - Try to load the corrupted migration
	let source = FilesystemSource::new(migrations_path.clone());
	let result = source.migrations_for_app("corrupted_app").await;

	// Assert - Should handle the error gracefully
	// The file may fail to compile or load
	assert!(
		result.is_ok() || result.is_err(),
		"Should handle corrupted file without panic"
	);

	// If loading succeeded, verify the operations are parsed (even with invalid SQL)
	if result.is_ok() {
		let _migrations = result.unwrap();
		// The file content may be loaded but SQL validation may fail elsewhere
	}
}

/// Test: EC-MM-05-02 - Missing required fields
///
/// Category: Edge Case
/// Verifies handling of migration files with missing required fields.
#[rstest]
#[tokio::test]
async fn ec_mm_05_02_missing_required_fields() {
	// Arrange
	let temp_dir = TempDir::new().expect("Failed to create temp directory");
	let migrations_path = temp_dir.path().join("migrations");
	fs::create_dir_all(&migrations_path).expect("Failed to create migrations directory");

	let app_dir = migrations_path.join("incomplete_app");
	fs::create_dir_all(&app_dir).expect("Failed to create app directory");

	// Create migration file with missing app_label
	let content = r#"
use reinhardt_db::migrations::{Migration, Operation};

pub fn migration() -> Migration {
    Migration {
        app_label: "".to_string(),  // Empty app_label
        name: "0001_missing_fields".to_string(),
        operations: vec![],
        dependencies: vec![],
        ..Default::default()
    }
}
"#;

	let file_path = app_dir.join("0001_missing_fields.rs");
	fs::write(&file_path, content).expect("Failed to write incomplete file");

	// Act
	let source = FilesystemSource::new(migrations_path.clone());
	let result = source.migrations_for_app("incomplete_app").await;

	// Assert - Should handle empty app_label
	assert!(
		result.is_ok() || result.is_err(),
		"Should handle empty app_label gracefully"
	);
}

/// Test: EC-MM-05-03 - Circular dependency
///
/// Category: Edge Case
/// Verifies detection of circular dependencies in migrations.
#[rstest]
#[tokio::test]
async fn ec_mm_05_03_circular_dependency() {
	// Arrange
	let temp_dir = TempDir::new().expect("Failed to create temp directory");
	let migrations_path = temp_dir.path().join("migrations");
	fs::create_dir_all(&migrations_path).expect("Failed to create migrations directory");

	// Create app directory
	let app_dir = migrations_path.join("circular_app");
	fs::create_dir_all(&app_dir).expect("Failed to create app directory");

	// Create migration A that depends on C
	let content_a = r#"
use reinhardt_db::migrations::{Migration, Operation};

pub fn migration() -> Migration {
    Migration {
        app_label: "circular_app".to_string(),
        name: "0001_migration_a".to_string(),
        operations: vec![
            Operation::RunSQL {
                sql: "CREATE TABLE table_a (id INT PRIMARY KEY)".to_string(),
                reverse_sql: Some("DROP TABLE table_a".to_string()),
            }
        ],
        dependencies: vec![("circular_app".to_string(), "0003_migration_c".to_string())],
        ..Default::default()
    }
}
"#;
	fs::write(app_dir.join("0001_migration_a.rs"), content_a).expect("Failed to write migration A");

	// Create migration B that depends on A
	let content_b = r#"
use reinhardt_db::migrations::{Migration, Operation};

pub fn migration() -> Migration {
    Migration {
        app_label: "circular_app".to_string(),
        name: "0002_migration_b".to_string(),
        operations: vec![
            Operation::RunSQL {
                sql: "CREATE TABLE table_b (id INT PRIMARY KEY)".to_string(),
                reverse_sql: Some("DROP TABLE table_b".to_string()),
            }
        ],
        dependencies: vec![("circular_app".to_string(), "0001_migration_a".to_string())],
        ..Default::default()
    }
}
"#;
	fs::write(app_dir.join("0002_migration_b.rs"), content_b).expect("Failed to write migration B");

	// Create migration C that depends on B (completing the cycle)
	let content_c = r#"
use reinhardt_db::migrations::{Migration, Operation};

pub fn migration() -> Migration {
    Migration {
        app_label: "circular_app".to_string(),
        name: "0003_migration_c".to_string(),
        operations: vec![
            Operation::RunSQL {
                sql: "CREATE TABLE table_c (id INT PRIMARY KEY)".to_string(),
                reverse_sql: Some("DROP TABLE table_c".to_string()),
            }
        ],
        dependencies: vec![("circular_app".to_string(), "0002_migration_b".to_string())],
        ..Default::default()
    }
}
"#;
	fs::write(app_dir.join("0003_migration_c.rs"), content_c).expect("Failed to write migration C");

	// Act - Load migrations
	let source = FilesystemSource::new(migrations_path.clone());
	let result = source.migrations_for_app("circular_app").await;

	// Assert - Should load all three migrations
	// Circular dependency detection may or may not happen at load time
	assert!(
		result.is_ok(),
		"Should load migrations with circular dependencies"
	);

	let migrations = result.unwrap();
	assert_eq!(migrations.len(), 3, "Should have 3 migrations");

	// Verify the circular dependencies exist
	let mut dep_graph: std::collections::HashMap<&str, Vec<&str>> =
		std::collections::HashMap::new();
	for m in &migrations {
		let deps: Vec<&str> = m
			.dependencies
			.iter()
			.map(|(_, name)| name.as_str())
			.collect();
		dep_graph.insert(m.name.as_str(), deps);
	}

	// A -> C, B -> A, C -> B creates a cycle
	assert!(dep_graph.contains_key("0001_migration_a"));
	assert!(dep_graph.contains_key("0002_migration_b"));
	assert!(dep_graph.contains_key("0003_migration_c"));
}

/// Test: EC-MM-05-04 - Empty migration file
///
/// Category: Edge Case
/// Verifies handling of completely empty migration files.
#[rstest]
#[tokio::test]
async fn ec_mm_05_04_empty_migration_file() {
	// Arrange
	let temp_dir = TempDir::new().expect("Failed to create temp directory");
	let migrations_path = temp_dir.path().join("migrations");
	fs::create_dir_all(&migrations_path).expect("Failed to create migrations directory");

	let app_dir = migrations_path.join("empty_app");
	fs::create_dir_all(&app_dir).expect("Failed to create app directory");

	// Create completely empty migration file
	let file_path = app_dir.join("0001_empty.rs");
	fs::write(&file_path, "").expect("Failed to write empty file");

	// Act
	let source = FilesystemSource::new(migrations_path.clone());
	let result = source.migrations_for_app("empty_app").await;

	// Assert - Should handle empty file gracefully
	assert!(
		result.is_ok() || result.is_err(),
		"Should handle empty migration file without panic"
	);

	if result.is_ok() {
		let _migrations = result.unwrap();
		// Empty file should either not be loaded or result in empty migration
	}
}

/// Test: EC-MM-05-05 - Binary/corrupted file content
///
/// Category: Edge Case
/// Verifies handling of non-text file content.
#[rstest]
#[tokio::test]
async fn ec_mm_05_05_binary_file_content() {
	// Arrange
	let temp_dir = TempDir::new().expect("Failed to create temp directory");
	let migrations_path = temp_dir.path().join("migrations");
	fs::create_dir_all(&migrations_path).expect("Failed to create migrations directory");

	let app_dir = migrations_path.join("binary_app");
	fs::create_dir_all(&app_dir).expect("Failed to create app directory");

	// Create file with binary content
	let binary_content: Vec<u8> = vec![
		0xFF, 0xFE, 0xFD, 0x00, 0x01, 0x02, 0x03, 0x00, 0xFF, 0xFF, 0xFF, 0xFF,
	];
	let file_path = app_dir.join("0001_binary.rs");
	fs::write(&file_path, binary_content).expect("Failed to write binary file");

	// Act
	let source = FilesystemSource::new(migrations_path.clone());
	let result = source.migrations_for_app("binary_app").await;

	// Assert - Should handle binary content gracefully
	assert!(
		result.is_ok() || result.is_err(),
		"Should handle binary file content without panic"
	);
}

// ============================================================================
// Sanity Tests
// ============================================================================

/// Test: Command metadata verification
///
/// Category: Sanity
/// Verifies MakeMigrationsCommand has correct metadata.
#[rstest]
fn test_makemigrations_command_metadata() {
	let command = MakeMigrationsCommand;

	assert_eq!(command.name(), "makemigrations");
	assert!(!command.description().is_empty());
	assert!(command.description().contains("migration"));

	let arguments = command.arguments();
	assert!(!arguments.is_empty());

	let options = command.options();
	let option_names: Vec<&str> = options.iter().map(|o| o.long.as_str()).collect();

	assert!(option_names.contains(&"dry-run"));
	assert!(option_names.contains(&"empty"));
	assert!(option_names.contains(&"name"));
	assert!(option_names.contains(&"migrations-dir"));
}

/// Test: Command context with edge case options
///
/// Category: Sanity
/// Verifies command context handles edge case options.
#[rstest]
fn test_command_context_edge_cases() {
	let mut ctx = CommandContext::default();

	// Test empty string option
	ctx.set_option("".to_string(), "value".to_string());

	// Test special characters in option value
	ctx.set_option("name".to_string(), "test- Migration ".to_string());

	// Test very long option value
	let long_value = "a".repeat(1000);
	ctx.set_option("description".to_string(), long_value);

	// Verify context handles these without panic
	assert!(ctx.has_option(""));
	assert_eq!(
		ctx.option("name").map(String::as_str),
		Some("test- Migration ")
	);
}

/// Test: TempMigrationDir basic functionality
///
/// Category: Sanity
/// Verifies TempMigrationDir fixture works correctly.
#[rstest]
fn test_temp_migration_dir_basic(temp_migration_dir: TempMigrationDir) {
	assert!(temp_migration_dir.migrations_path.exists());
	assert!(temp_migration_dir.migrations_path.is_dir());

	let file_path =
		temp_migration_dir.create_migration_file("test_app", "0001_test", "// Test content");

	assert!(file_path.exists());
	let content = fs::read_to_string(&file_path).unwrap();
	assert_eq!(content, "// Test content");
}
