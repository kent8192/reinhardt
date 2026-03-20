//! Comprehensive integration tests for migration numbering and overwrite prevention
//!
//! This test suite validates:
//! 1. Sequential migration creation (0001 → 0002 → 0003)
//! 2. Overwrite prevention (existing files are protected)
//! 3. Numbering rollover (0099 → 0100)
//! 4. Multi-app independence (migrations don't interfere across apps)
//! 5. Directory structure correctness (migrations/{app}/NNNN_name.rs)

use reinhardt_db::migrations::{
	Migration, MigrationError, MigrationRepository, migration_numbering::MigrationNumbering,
	operations::Operation, repository::filesystem::FilesystemRepository,
};
use serial_test::serial;
use tempfile::TempDir;

/// Helper function to create a test migration
fn create_test_migration(app_label: &str, name: &str) -> Migration {
	Migration::new(name, app_label)
}

/// Test Case 1: Sequential migration creation
/// Validates that consecutive migrations are numbered correctly (0001, 0002, 0003)
#[tokio::test]
#[serial(migration_overwrite)]
async fn test_sequential_migration_creation() {
	let temp_dir = TempDir::new().unwrap();
	let migrations_dir = temp_dir.path();

	// Create migrations sequentially
	let numbers: Vec<String> = (1..=5)
		.map(|i| {
			let num = MigrationNumbering::next_number(migrations_dir, "blog");

			// Create the migration file to simulate real scenario
			let app_dir = migrations_dir.join("blog");
			std::fs::create_dir_all(&app_dir).unwrap();
			std::fs::write(app_dir.join(format!("{}_migration_{}.rs", num, i)), "").unwrap();

			num
		})
		.collect();

	// Verify sequential numbering
	assert_eq!(numbers, vec!["0001", "0002", "0003", "0004", "0005"]);
}

/// Test Case 2: Overwrite prevention
/// Validates that attempting to save a migration with an existing name fails
#[tokio::test]
#[serial(migration_overwrite)]
async fn test_overwrite_prevention() {
	let temp_dir = TempDir::new().unwrap();
	let mut repo = FilesystemRepository::new(temp_dir.path());

	// Save initial migration
	let migration = create_test_migration("blog", "0001_initial");
	repo.save(&migration).await.unwrap();

	// Verify file exists
	let path = temp_dir.path().join("blog").join("0001_initial.rs");
	assert!(path.exists());

	// Attempt to save duplicate migration
	let duplicate = create_test_migration("blog", "0001_initial");
	let result = repo.save(&duplicate).await;

	// Should fail with appropriate error
	assert!(result.is_err());
	let err_msg = result.unwrap_err().to_string();
	assert!(err_msg.contains("already exists"));
}

/// Test Case 3: Migration numbering rollover
/// Validates that numbering correctly handles 0099 → 0100 transition
#[tokio::test]
#[serial(migration_overwrite)]
async fn test_migration_numbering_rollover() {
	let temp_dir = TempDir::new().unwrap();
	let migrations_dir = temp_dir.path();

	// Create 0099 migration file
	let app_dir = migrations_dir.join("blog");
	std::fs::create_dir_all(&app_dir).unwrap();
	std::fs::write(app_dir.join("0099_before_rollover.rs"), "").unwrap();

	// Get next number
	let next_number = MigrationNumbering::next_number(migrations_dir, "blog");

	// Should be 0100
	assert_eq!(next_number, "0100");
}

/// Test Case 4: Multi-app independence
/// Validates that different apps maintain independent migration numbering
#[tokio::test]
#[serial(migration_overwrite)]
async fn test_multi_app_independence() {
	let temp_dir = TempDir::new().unwrap();
	let mut repo = FilesystemRepository::new(temp_dir.path());

	// Create migrations for multiple apps
	let blog_migration_1 = create_test_migration("blog", "0001_initial");
	let blog_migration_2 = create_test_migration("blog", "0002_add_field");
	let auth_migration_1 = create_test_migration("auth", "0001_initial");
	let auth_migration_2 = create_test_migration("auth", "0002_add_field");

	// Save migrations
	repo.save(&blog_migration_1).await.unwrap();
	repo.save(&blog_migration_2).await.unwrap();
	repo.save(&auth_migration_1).await.unwrap();
	repo.save(&auth_migration_2).await.unwrap();

	// Verify all files exist independently
	let blog_dir = temp_dir.path().join("blog");
	let auth_dir = temp_dir.path().join("auth");

	assert!(blog_dir.join("0001_initial.rs").exists());
	assert!(blog_dir.join("0002_add_field.rs").exists());
	assert!(auth_dir.join("0001_initial.rs").exists());
	assert!(auth_dir.join("0002_add_field.rs").exists());

	// Verify next numbers are independent
	let blog_next = MigrationNumbering::next_number(temp_dir.path(), "blog");
	let auth_next = MigrationNumbering::next_number(temp_dir.path(), "auth");

	assert_eq!(blog_next, "0003");
	assert_eq!(auth_next, "0003");
}

/// Test Case 5: Directory structure correctness
/// Validates that migrations are created in the correct directory structure
#[tokio::test]
#[serial(migration_overwrite)]
async fn test_directory_structure_correctness() {
	let temp_dir = TempDir::new().unwrap();
	let mut repo = FilesystemRepository::new(temp_dir.path());

	// Create migration
	let migration = create_test_migration("blog", "0001_initial");
	repo.save(&migration).await.unwrap();

	// Verify correct directory structure: migrations/{app}/NNNN_name.rs
	let expected_path = temp_dir.path().join("blog").join("0001_initial.rs");
	assert!(expected_path.exists());

	// Verify incorrect structure does NOT exist
	let incorrect_path = temp_dir
		.path()
		.join("blog")
		.join("migrations")
		.join("0001_initial.rs");
	assert!(!incorrect_path.exists());
}

/// Test Case 6: Empty directory handling
/// Validates that get_highest_number returns 0 for non-existent directories
#[test]
fn test_empty_directory_handling() {
	let temp_dir = TempDir::new().unwrap();
	let migrations_dir = temp_dir.path();

	// Get highest number for non-existent app
	let highest = MigrationNumbering::get_highest_number(migrations_dir, "nonexistent");

	// Should return 0
	assert_eq!(highest, 0);

	// Next number should be 0001
	let next = MigrationNumbering::next_number(migrations_dir, "nonexistent");
	assert_eq!(next, "0001");
}

/// Test Case 7: Invalid file name handling
/// Validates that files without proper numbering are ignored
#[test]
fn test_invalid_file_name_handling() {
	let temp_dir = TempDir::new().unwrap();
	let migrations_dir = temp_dir.path();

	// Create app directory
	let app_dir = migrations_dir.join("blog");
	std::fs::create_dir_all(&app_dir).unwrap();

	// Create valid migration
	std::fs::write(app_dir.join("0005_valid.rs"), "").unwrap();

	// Create invalid files (should be ignored)
	std::fs::write(app_dir.join("README.md"), "").unwrap();
	std::fs::write(app_dir.join("invalid_name.rs"), "").unwrap();
	std::fs::write(app_dir.join("abc_not_number.rs"), "").unwrap();
	std::fs::write(app_dir.join("__init__.py"), "").unwrap();

	// Get highest number
	let highest = MigrationNumbering::get_highest_number(migrations_dir, "blog");

	// Should only consider valid files
	assert_eq!(highest, 5);

	// Next number should be 0006
	let next = MigrationNumbering::next_number(migrations_dir, "blog");
	assert_eq!(next, "0006");
}

/// Test Case 8: Gap in numbering
/// Validates that the system handles gaps in migration numbers correctly
#[test]
fn test_gap_in_numbering() {
	let temp_dir = TempDir::new().unwrap();
	let migrations_dir = temp_dir.path();

	// Create app directory
	let app_dir = migrations_dir.join("blog");
	std::fs::create_dir_all(&app_dir).unwrap();

	// Create migrations with gaps: 0001, 0003, 0007
	std::fs::write(app_dir.join("0001_first.rs"), "").unwrap();
	std::fs::write(app_dir.join("0003_third.rs"), "").unwrap();
	std::fs::write(app_dir.join("0007_seventh.rs"), "").unwrap();

	// Get highest number
	let highest = MigrationNumbering::get_highest_number(migrations_dir, "blog");

	// Should return the highest number (7)
	assert_eq!(highest, 7);

	// Next number should be 0008
	let next = MigrationNumbering::next_number(migrations_dir, "blog");
	assert_eq!(next, "0008");
}

/// Test Case 9: Migration list accuracy
/// Validates that list() returns all migrations correctly
#[tokio::test]
#[serial(migration_overwrite)]
async fn test_migration_list_accuracy() {
	let temp_dir = TempDir::new().unwrap();
	let mut repo = FilesystemRepository::new(temp_dir.path());

	// Create multiple migrations
	for i in 1..=5 {
		let name = format!("000{}_migration_{}", i, i);
		let migration = create_test_migration("blog", &name);
		repo.save(&migration).await.unwrap();
	}

	// List migrations
	let migrations = repo.list("blog").await.unwrap();

	// Verify count
	assert_eq!(migrations.len(), 5);

	// Verify names
	let names: Vec<String> = migrations.iter().map(|m| m.name.to_string()).collect();
	assert!(names.contains(&"0001_migration_1".to_string()));
	assert!(names.contains(&"0002_migration_2".to_string()));
	assert!(names.contains(&"0003_migration_3".to_string()));
	assert!(names.contains(&"0004_migration_4".to_string()));
	assert!(names.contains(&"0005_migration_5".to_string()));
}

/// Test Case 10: Large number handling
/// Validates that the system handles large migration numbers correctly
#[test]
fn test_large_number_handling() {
	let temp_dir = TempDir::new().unwrap();
	let migrations_dir = temp_dir.path();

	// Create app directory
	let app_dir = migrations_dir.join("blog");
	std::fs::create_dir_all(&app_dir).unwrap();

	// Create migration with large number
	std::fs::write(app_dir.join("9998_large.rs"), "").unwrap();

	// Get next number
	let next = MigrationNumbering::next_number(migrations_dir, "blog");

	// Should be 9999
	assert_eq!(next, "9999");

	// Create 9999 migration
	std::fs::write(app_dir.join("9999_largest.rs"), "").unwrap();

	// Next should be 10000 (but formatted as 4 digits, so it wraps)
	let next_after_9999 = MigrationNumbering::next_number(migrations_dir, "blog");
	assert_eq!(next_after_9999, "10000");
}

/// Helper function to create a migration with operations for duplicate check tests
fn create_migration_with_operations(app_label: &str, name: &str) -> Migration {
	let mut migration = Migration::new(name, app_label);
	migration.operations.push(Operation::DropTable {
		name: "test_table".to_string(),
	});
	migration
}

/// Test Case 11: Merge migration (empty operations) saves successfully
/// Validates that merge migrations with empty operations don't trigger
/// DuplicateOperations error even when existing migrations have empty operations too.
/// Regression test for issue #2484.
#[tokio::test]
#[serial(migration_overwrite)]
async fn test_merge_migration_empty_operations_saves_successfully() {
	// Arrange
	let temp_dir = TempDir::new().unwrap();
	let mut repo = FilesystemRepository::new(temp_dir.path());

	// Save a regular migration (which will also have empty operations from new())
	let existing = create_test_migration("blog", "0001_initial");
	repo.save(&existing).await.unwrap();

	// Act: save a merge migration (empty operations, has dependencies)
	let mut merge_migration = create_test_migration("blog", "0002_merge");
	merge_migration
		.dependencies
		.push(("blog".to_string(), "0001_initial".to_string()));

	// Assert: should succeed without DuplicateOperations error
	let result = repo.save(&merge_migration).await;
	assert!(result.is_ok(), "merge migration save failed: {result:?}");
	assert!(temp_dir.path().join("blog").join("0002_merge.rs").exists());
}

/// Test Case 12: Multiple merge migrations save successfully
/// Validates that multiple merge migrations (all with empty operations)
/// can coexist without false-positive duplicate detection.
/// Regression test for issue #2484.
#[tokio::test]
#[serial(migration_overwrite)]
async fn test_multiple_merge_migrations_save_successfully() {
	// Arrange
	let temp_dir = TempDir::new().unwrap();
	let mut repo = FilesystemRepository::new(temp_dir.path());

	let base = create_test_migration("blog", "0001_initial");
	repo.save(&base).await.unwrap();

	let mut merge1 = create_test_migration("blog", "0002_merge_branch_a");
	merge1
		.dependencies
		.push(("blog".to_string(), "0001_initial".to_string()));
	repo.save(&merge1).await.unwrap();

	// Act: save a second merge migration
	let mut merge2 = create_test_migration("blog", "0003_merge_branch_b");
	merge2
		.dependencies
		.push(("blog".to_string(), "0002_merge_branch_a".to_string()));

	// Assert
	let result = repo.save(&merge2).await;
	assert!(
		result.is_ok(),
		"second merge migration save failed: {result:?}"
	);
	assert!(
		temp_dir
			.path()
			.join("blog")
			.join("0003_merge_branch_b.rs")
			.exists()
	);
}

/// Test Case 13: Duplicate operations check still works for non-empty operations
/// Validates that the duplicate detection still catches actual duplicates
/// (migrations with identical non-empty operations).
/// Regression prevention for the fix in issue #2484.
#[tokio::test]
#[serial(migration_overwrite)]
async fn test_duplicate_operations_check_still_catches_real_duplicates() {
	// Arrange
	let temp_dir = TempDir::new().unwrap();
	let mut repo = FilesystemRepository::new(temp_dir.path());

	let migration1 = create_migration_with_operations("blog", "0001_drop_table");
	repo.save(&migration1).await.unwrap();

	// Act: save another migration with identical operations but different name
	let migration2 = create_migration_with_operations("blog", "0002_drop_table_again");
	let result = repo.save(&migration2).await;

	// Assert: should fail with DuplicateOperations error
	assert!(result.is_err());
	let err_msg = result.unwrap_err().to_string();
	assert!(
		err_msg.contains("identical operations"),
		"expected DuplicateOperations error, got: {err_msg}"
	);
}

/// Helper function to create a merge migration with dependencies and empty operations
fn create_merge_migration(app_label: &str, name: &str, deps: Vec<(&str, &str)>) -> Migration {
	let mut migration = Migration::new(name, app_label);
	migration.dependencies = deps
		.into_iter()
		.map(|(a, n)| (a.to_string(), n.to_string()))
		.collect();
	// Merge migrations have empty operations by design
	migration
}

/// Test Case 14: Merge migration save and get round-trip
/// Validates that a merge migration can be saved and retrieved correctly.
#[tokio::test]
#[serial(migration_overwrite)]
async fn test_merge_migration_save_and_get_round_trip() {
	// Arrange
	let temp_dir = TempDir::new().unwrap();
	let mut repo = FilesystemRepository::new(temp_dir.path());

	let m1 = create_test_migration("blog", "0001_initial");
	repo.save(&m1).await.unwrap();
	let m2 = create_test_migration("blog", "0002_add_field");
	repo.save(&m2).await.unwrap();

	let merge = create_merge_migration(
		"blog",
		"0003_merge",
		vec![("blog", "0001_initial"), ("blog", "0002_add_field")],
	);
	repo.save(&merge).await.unwrap();

	// Act
	let retrieved = repo.get("blog", "0003_merge").await;

	// Assert
	assert!(retrieved.is_ok(), "get failed: {:?}", retrieved.err());
	let retrieved = retrieved.unwrap();
	assert_eq!(retrieved.app_label, "blog");
	assert_eq!(retrieved.name, "0003_merge");
	assert!(
		retrieved.operations.is_empty(),
		"merge migration should have no operations"
	);
}

/// Test Case 15: Merge migration appears in list
/// Validates that merge migrations are included in list() results.
#[tokio::test]
#[serial(migration_overwrite)]
async fn test_merge_migration_appears_in_list() {
	// Arrange
	let temp_dir = TempDir::new().unwrap();
	let mut repo = FilesystemRepository::new(temp_dir.path());

	let m1 = create_test_migration("blog", "0001_initial");
	repo.save(&m1).await.unwrap();
	let m2 = create_test_migration("blog", "0002_add_field");
	repo.save(&m2).await.unwrap();
	let merge = create_merge_migration(
		"blog",
		"0003_merge",
		vec![("blog", "0001_initial"), ("blog", "0002_add_field")],
	);
	repo.save(&merge).await.unwrap();

	// Act
	let migrations = repo.list("blog").await.unwrap();

	// Assert
	assert_eq!(migrations.len(), 3);
	let names: Vec<&str> = migrations.iter().map(|m| m.name.as_str()).collect();
	assert!(names.contains(&"0003_merge"));
}

/// Test Case 16: Sequential merge saves coexist on disk
/// Validates that two merge migrations at different points coexist.
#[tokio::test]
#[serial(migration_overwrite)]
async fn test_sequential_merge_saves_coexist_on_disk() {
	// Arrange
	let temp_dir = TempDir::new().unwrap();
	let mut repo = FilesystemRepository::new(temp_dir.path());

	let m1 = create_test_migration("blog", "0001_initial");
	repo.save(&m1).await.unwrap();
	let m2a = create_test_migration("blog", "0002_a");
	repo.save(&m2a).await.unwrap();
	let m2b = create_test_migration("blog", "0002_b");
	repo.save(&m2b).await.unwrap();
	let merge1 = create_merge_migration(
		"blog",
		"0003_merge",
		vec![("blog", "0002_a"), ("blog", "0002_b")],
	);
	repo.save(&merge1).await.unwrap();
	let m4x = create_test_migration("blog", "0004_x");
	repo.save(&m4x).await.unwrap();
	let m4y = create_test_migration("blog", "0004_y");
	repo.save(&m4y).await.unwrap();
	let merge2 = create_merge_migration(
		"blog",
		"0005_merge",
		vec![("blog", "0004_x"), ("blog", "0004_y")],
	);
	repo.save(&merge2).await.unwrap();

	// Act
	let blog_dir = temp_dir.path().join("blog");
	let migrations = repo.list("blog").await.unwrap();

	// Assert: both merge files exist on disk
	assert!(blog_dir.join("0003_merge.rs").exists());
	assert!(blog_dir.join("0005_merge.rs").exists());
	assert_eq!(migrations.len(), 7);
}

/// Test Case 17: Merge migration overwrite prevention
/// Validates that saving a merge migration with an existing name fails.
#[tokio::test]
#[serial(migration_overwrite)]
async fn test_merge_overwrite_prevention() {
	// Arrange
	let temp_dir = TempDir::new().unwrap();
	let mut repo = FilesystemRepository::new(temp_dir.path());

	let m1 = create_test_migration("blog", "0001_initial");
	repo.save(&m1).await.unwrap();
	let merge = create_merge_migration("blog", "0003_merge", vec![("blog", "0001_initial")]);
	repo.save(&merge).await.unwrap();

	// Act: save duplicate merge migration
	let duplicate = create_merge_migration("blog", "0003_merge", vec![("blog", "0001_initial")]);
	let result = repo.save(&duplicate).await;

	// Assert
	assert!(result.is_err());
	let err_msg = result.unwrap_err().to_string();
	assert!(
		err_msg.contains("already exists"),
		"expected 'already exists' error, got: {err_msg}"
	);
}

/// Test Case 18: Merge migration with path traversal rejected
/// Validates that path traversal in app_label is rejected.
#[tokio::test]
#[serial(migration_overwrite)]
async fn test_merge_with_path_traversal_rejected() {
	// Arrange
	let temp_dir = TempDir::new().unwrap();
	let mut repo = FilesystemRepository::new(temp_dir.path());

	let merge = create_merge_migration("../evil", "0001_merge", vec![("blog", "0001_initial")]);

	// Act
	let result = repo.save(&merge).await;

	// Assert
	assert!(result.is_err(), "Path traversal should be rejected");
	assert!(matches!(
		result.unwrap_err(),
		MigrationError::PathTraversal(_)
	));
}

/// Test Case 19: Merge migration with empty app label rejected
/// Validates that empty app_label is rejected by path validation.
#[tokio::test]
#[serial(migration_overwrite)]
async fn test_merge_with_empty_app_label_rejected() {
	// Arrange
	let temp_dir = TempDir::new().unwrap();
	let mut repo = FilesystemRepository::new(temp_dir.path());

	let merge = create_merge_migration("", "0001_merge", vec![("blog", "0001_initial")]);

	// Act
	let result = repo.save(&merge).await;

	// Assert
	assert!(result.is_err(), "Empty app_label should be rejected");
	assert!(matches!(
		result.unwrap_err(),
		MigrationError::PathTraversal(_)
	));
}

/// Test Case 20: Merge migration generated code contains dependencies
/// Validates that the generated .rs file includes dependency information.
#[tokio::test]
#[serial(migration_overwrite)]
async fn test_merge_migration_generated_code_contains_dependencies() {
	// Arrange
	let temp_dir = TempDir::new().unwrap();
	let mut repo = FilesystemRepository::new(temp_dir.path());

	let merge = create_merge_migration(
		"blog",
		"0003_merge",
		vec![("blog", "0001_initial"), ("blog", "0002_add_field")],
	);
	repo.save(&merge).await.unwrap();

	// Act: read the generated file
	let file_path = temp_dir.path().join("blog").join("0003_merge.rs");
	let content = tokio::fs::read_to_string(&file_path).await.unwrap();

	// Assert: file contains dependency information and app label
	assert!(
		content.contains("blog"),
		"generated code should reference app label 'blog'"
	);
	assert!(
		content.contains("0001_initial") || content.contains("dependencies"),
		"generated code should contain dependency references"
	);

	// Merge migration should have empty operations
	// (no CreateTable, AddColumn, etc. in the generated code)
	assert!(
		!content.contains("CreateTable") && !content.contains("AddColumn"),
		"merge migration should not contain table/column operations"
	);
}
