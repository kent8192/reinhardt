//! makemigrations Command E2E Integration Tests
//!
//! Tests the complete end-to-end workflow of makemigrations functionality,
//! including file generation and filesystem operations.
//!
//! **Test Coverage:**
//! - Normal Cases (NC-01 ~ NC-20): Basic to advanced migration generation
//! - Error Cases (EC-01 ~ EC-05): Error handling validation
//! - Edge Cases (EDG-01 ~ EDG-14): Special scenarios and options
//!
//! **Test Approach:**
//! - Uses FilesystemRepository for actual file generation
//! - TempDir for isolated filesystem operations
//! - Verifies generated migration files
//! - Tests migration executability on real databases

use reinhardt_db::backends::DatabaseConnection;
use reinhardt_db::migrations::schema_diff::{ColumnSchema, DatabaseSchema, TableSchema};
use reinhardt_db::migrations::{
	AutoMigrationGenerator, ColumnDefinition, FieldType, FilesystemRepository, FilesystemSource,
	Migration, MigrationNamer, MigrationNumbering, MigrationService, Operation,
	autodetector::ProjectState,
};
use reinhardt_test::fixtures::postgres_container;
use rstest::*;
use serial_test::serial;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tempfile::TempDir;
use testcontainers::ContainerAsync;
use testcontainers::GenericImage;
use testcontainers::core::WaitFor;
use tokio::sync::Mutex;

// ============================================================================
// Helper Functions
// ============================================================================

/// Helper to create a simple schema with a todos table
fn create_todos_schema() -> DatabaseSchema {
	let mut schema = DatabaseSchema::default();
	let mut table = TableSchema {
		name: "todos".to_string(),
		columns: BTreeMap::new(),
		indexes: Vec::new(),
		constraints: Vec::new(),
	};

	table.columns.insert(
		"id".to_string(),
		ColumnSchema {
			name: "id".to_string(),
			data_type: FieldType::Integer,
			nullable: false,
			default: None,
			primary_key: true,
			auto_increment: true,
		},
	);

	table.columns.insert(
		"title".to_string(),
		ColumnSchema {
			name: "title".to_string(),
			data_type: FieldType::VarChar(255),
			nullable: false,
			default: None,
			primary_key: false,
			auto_increment: false,
		},
	);

	table.columns.insert(
		"completed".to_string(),
		ColumnSchema {
			name: "completed".to_string(),
			data_type: FieldType::Boolean,
			nullable: false,
			default: Some("false".to_string()),
			primary_key: false,
			auto_increment: false,
		},
	);

	schema.tables.insert("todos".to_string(), table);
	schema
}

/// Verify that a migration file exists at the specified path
fn verify_migration_file_exists(
	migrations_dir: &Path,
	app_label: &str,
	expected_number: &str,
) -> bool {
	let app_dir = migrations_dir.join(app_label);
	if !app_dir.exists() {
		return false;
	}

	std::fs::read_dir(&app_dir)
		.ok()
		.and_then(|entries| {
			entries.filter_map(Result::ok).find(|entry| {
				entry
					.file_name()
					.to_str()
					.map(|name| name.starts_with(expected_number) && name.ends_with(".rs"))
					.unwrap_or(false)
			})
		})
		.is_some()
}

/// Read and parse a generated migration file
fn read_migration_file(path: &Path) -> Result<String, std::io::Error> {
	std::fs::read_to_string(path)
}

// ============================================================================
// Normal Cases (NC-01 ~ NC-20)
// ============================================================================

#[tokio::test]
#[serial(makemigrations_e2e)]
async fn nc_01_new_model_creates_create_table_migration() {
	// Test: CreateTable generation from new model creation (E2E)
	// Verify up to file system writes

	let temp_dir = TempDir::new().unwrap();
	let migrations_dir = temp_dir.path().join("migrations");

	let app_label = "todos";
	let empty_schema = DatabaseSchema::default();
	let target_schema = create_todos_schema();

	// Setup FilesystemRepository and Service
	let source = Arc::new(FilesystemSource::new(migrations_dir.clone()));
	let repository = Arc::new(Mutex::new(FilesystemRepository::new(
		migrations_dir.clone(),
	)));
	let service = MigrationService::new(source.clone(), repository.clone());

	// Generate migration
	let generator = AutoMigrationGenerator::new(target_schema.clone(), repository.clone());
	let result = generator
		.generate(app_label, empty_schema.clone())
		.await
		.expect("First makemigrations should succeed");

	// Build migration name
	let migration_number = MigrationNumbering::next_number(&migrations_dir, app_label);
	assert_eq!(migration_number, "0001", "First migration should be 0001");

	let migration_name = format!("{}_{}", migration_number, "initial");

	// Create Migration struct
	let migration = Migration {
		app_label: app_label.to_string(),
		name: migration_name.clone(),
		operations: result.operations.clone(),
		dependencies: Vec::new(),
		atomic: true,
		replaces: Vec::new(),
		initial: Some(true),
		..Default::default()
	};

	// Save migration to filesystem
	service
		.save_migration(&migration)
		.await
		.expect("Failed to save migration");

	// Verify: File exists
	assert!(
		verify_migration_file_exists(&migrations_dir, app_label, "0001"),
		"Migration file should exist"
	);

	// Verify: File content contains CreateTable
	let migration_file_path = migrations_dir
		.join(app_label)
		.join(format!("{}.rs", migration_name));

	let file_content =
		read_migration_file(&migration_file_path).expect("Failed to read migration file");

	assert!(
		file_content.contains("CreateTable"),
		"Migration file should contain CreateTable operation"
	);
	assert!(
		file_content.contains("todos"),
		"Migration file should reference 'todos' table"
	);
	assert!(
		file_content.contains("initial: Some(true)"),
		"Migration file should have initial flag set"
	);
}

#[tokio::test]
#[serial(makemigrations_e2e)]
async fn nc_02_field_addition_creates_add_column_migration() {
	// Test: AddColumn generation from field addition (E2E)
	// Arrange
	let temp_dir = TempDir::new().unwrap();
	let migrations_dir = temp_dir.path().join("migrations");
	let app_label = "todos";

	let mut initial_schema = DatabaseSchema::default();
	let mut table = TableSchema {
		name: "todos".to_string(),
		columns: BTreeMap::new(),
		indexes: Vec::new(),
		constraints: Vec::new(),
	};
	table.columns.insert(
		"id".to_string(),
		ColumnSchema {
			name: "id".to_string(),
			data_type: FieldType::Integer,
			nullable: false,
			default: None,
			primary_key: true,
			auto_increment: true,
		},
	);
	table.columns.insert(
		"title".to_string(),
		ColumnSchema {
			name: "title".to_string(),
			data_type: FieldType::VarChar(255),
			nullable: false,
			default: None,
			primary_key: false,
			auto_increment: false,
		},
	);
	initial_schema.tables.insert("todos".to_string(), table);

	let mut target_schema = initial_schema.clone();
	target_schema
		.tables
		.get_mut("todos")
		.unwrap()
		.columns
		.insert(
			"description".to_string(),
			ColumnSchema {
				name: "description".to_string(),
				data_type: FieldType::Text,
				nullable: true,
				default: None,
				primary_key: false,
				auto_increment: false,
			},
		);

	let source = Arc::new(FilesystemSource::new(migrations_dir.clone()));
	let repository = Arc::new(Mutex::new(FilesystemRepository::new(
		migrations_dir.clone(),
	)));
	let service = MigrationService::new(source.clone(), repository.clone());

	// Act
	let generator = AutoMigrationGenerator::new(target_schema, repository.clone());
	let result = generator
		.generate(app_label, initial_schema)
		.await
		.expect("Field addition makemigrations should succeed");

	let migration_name = format!(
		"{}_add_description",
		MigrationNumbering::next_number(&migrations_dir, app_label)
	);

	let migration = Migration {
		app_label: app_label.to_string(),
		name: migration_name.clone(),
		operations: result.operations.clone(),
		dependencies: Vec::new(),
		atomic: true,
		replaces: Vec::new(),
		initial: None,
		..Default::default()
	};

	service
		.save_migration(&migration)
		.await
		.expect("Failed to save migration");

	// Assert
	let migration_file_path = migrations_dir
		.join(app_label)
		.join(format!("{}.rs", migration_name));
	let file_content =
		std::fs::read_to_string(&migration_file_path).expect("Failed to read migration file");

	assert!(
		file_content.contains("AddColumn") || file_content.contains("add_column"),
		"Migration file should contain AddColumn operation"
	);
	assert!(
		file_content.contains("description"),
		"Migration file should reference 'description' field"
	);
}

#[tokio::test]
#[serial(makemigrations_e2e)]
async fn nc_03_field_deletion_creates_drop_column_migration() {
	// Test: DropColumn generation from field deletion (E2E)

	// Arrange
	let temp_dir = TempDir::new().unwrap();
	let migrations_dir = temp_dir.path().join("migrations");
	let app_label = "todos";
	let initial_schema = create_todos_schema();

	let mut target_schema = DatabaseSchema::default();
	let mut table = TableSchema {
		name: "todos".to_string(),
		columns: BTreeMap::new(),
		indexes: Vec::new(),
		constraints: Vec::new(),
	};
	table.columns.insert(
		"id".to_string(),
		ColumnSchema {
			name: "id".to_string(),
			data_type: FieldType::Integer,
			nullable: false,
			default: None,
			primary_key: true,
			auto_increment: true,
		},
	);
	table.columns.insert(
		"title".to_string(),
		ColumnSchema {
			name: "title".to_string(),
			data_type: FieldType::VarChar(255),
			nullable: false,
			default: None,
			primary_key: false,
			auto_increment: false,
		},
	);
	target_schema.tables.insert("todos".to_string(), table);

	let source = Arc::new(FilesystemSource::new(migrations_dir.clone()));
	let repository = Arc::new(Mutex::new(FilesystemRepository::new(
		migrations_dir.clone(),
	)));
	let service = MigrationService::new(source.clone(), repository.clone());

	// Act
	let generator = AutoMigrationGenerator::new(target_schema, repository.clone());
	let result = generator
		.generate(app_label, initial_schema)
		.await
		.expect("Field deletion makemigrations should succeed");

	let migration_name = format!(
		"{}_remove_completed",
		MigrationNumbering::next_number(&migrations_dir, app_label)
	);

	let migration = Migration {
		app_label: app_label.to_string(),
		name: migration_name.clone(),
		operations: result.operations.clone(),
		dependencies: Vec::new(),
		atomic: true,
		replaces: Vec::new(),
		initial: None,
		..Default::default()
	};

	service
		.save_migration(&migration)
		.await
		.expect("Failed to save migration");

	// Assert
	let migration_file_path = migrations_dir
		.join(app_label)
		.join(format!("{}.rs", migration_name));
	let file_content =
		std::fs::read_to_string(&migration_file_path).expect("Failed to read migration file");

	assert!(
		file_content.contains("DropColumn") || file_content.contains("drop_column"),
		"Migration file should contain DropColumn operation"
	);
	assert!(
		file_content.contains("completed"),
		"Migration file should reference 'completed' field"
	);
}

#[tokio::test]
#[serial(makemigrations_e2e)]
async fn nc_04_field_type_change_creates_alter_column_migration() {
	// Test: AlterColumn generation from field type change (E2E)
	// Note: SchemaDiff does not yet support AlterColumn detection
	todo!("Implement E2E field type change test (requires SchemaDiff AlterColumn support)")
}

#[tokio::test]
#[serial(makemigrations_e2e)]
async fn nc_05_field_rename_creates_rename_column_migration() {
	// Test: RenameColumn generation from field rename (E2E)
	todo!("Implement E2E field rename test")
}

#[tokio::test]
#[serial(makemigrations_e2e)]
async fn nc_06_index_addition_creates_create_index_migration() {
	// Test: CreateIndex generation from index addition (E2E)
	todo!("Implement E2E index addition test")
}

#[tokio::test]
#[serial(makemigrations_e2e)]
async fn nc_07_foreign_key_addition_creates_add_column_and_constraint() {
	// Test: AddColumn + AddConstraint generation from ForeignKey addition (E2E)
	todo!("Implement E2E foreign key test")
}

#[tokio::test]
#[serial(makemigrations_e2e)]
async fn nc_08_many_to_many_creates_junction_table() {
	// Test: CreateTable (junction table) generation from ManyToMany addition (E2E)
	todo!("Implement E2E many-to-many test")
}

#[tokio::test]
#[serial(makemigrations_e2e)]
async fn nc_09_initial_migration_correctness() {
	// Test: Correct generation of initial migration (0001_initial) (E2E)
	todo!("Implement E2E initial migration test")
}

#[tokio::test]
#[serial(makemigrations_e2e)]
async fn nc_10_sequential_migrations_dependency_chain() {
	// Test: Correct generation of sequential migrations (0001 → 0002 → 0003) (E2E)
	todo!("Implement E2E sequential migrations test")
}

#[tokio::test]
#[serial(makemigrations_e2e)]
async fn nc_11_generated_migration_executability() {
	// Test: Verify generated migrations are executable (E2E)
	todo!("Implement E2E executability test with real database")
}

#[tokio::test]
#[serial(makemigrations_e2e)]
async fn nc_12_one_to_one_creates_unique_foreign_key() {
	// Test: Proper migration generation from OneToOne addition (E2E)
	todo!("Implement E2E one-to-one test")
}

#[tokio::test]
#[serial(makemigrations_e2e)]
async fn nc_13_default_value_addition_creates_alter_column() {
	// Test: AlterColumn generation from default value addition (E2E)
	todo!("Implement E2E default value test")
}

#[tokio::test]
#[serial(makemigrations_e2e)]
async fn nc_14_null_constraint_change_creates_alter_column() {
	// Test: AlterColumn generation from NULL constraint change (E2E)
	todo!("Implement E2E null constraint test")
}

#[tokio::test]
#[serial(makemigrations_e2e)]
async fn nc_15_unique_constraint_addition_creates_add_constraint() {
	// Test: AddConstraint generation from UNIQUE constraint addition (E2E)
	todo!("Implement E2E unique constraint test")
}

#[tokio::test]
#[serial(makemigrations_e2e)]
async fn nc_16_index_deletion_creates_drop_index() {
	// Test: DropIndex generation from index deletion (E2E)
	todo!("Implement E2E index deletion test")
}

#[tokio::test]
#[serial(makemigrations_e2e)]
async fn nc_17_constraint_deletion_creates_drop_constraint() {
	// Test: DropConstraint generation from constraint removal (E2E)
	todo!("Implement E2E constraint deletion test")
}

#[tokio::test]
#[serial(makemigrations_e2e)]
async fn nc_18_multiple_changes_in_single_migration() {
	// Test: Migration generation with multiple changes (E2E)
	todo!("Implement E2E multiple changes test")
}

#[tokio::test]
#[serial(makemigrations_e2e)]
async fn nc_19_multi_app_migrations_generation() {
	// Test: Simultaneous migration generation for multiple apps (E2E)
	todo!("Implement E2E multi-app test")
}

#[tokio::test]
#[serial(makemigrations_e2e)]
async fn nc_20_data_preservation_verification() {
	// Test: Data retention verification (existing data is not lost) (E2E)
	todo!("Implement E2E data preservation test")
}

// ============================================================================
// Error Cases (EC-01 ~ EC-05)
// ============================================================================

#[tokio::test]
#[serial(makemigrations_e2e_errors)]
async fn ec_01_no_models_error() {
	// Test: Error when model does not exist (E2E)
	todo!("Implement E2E no models error test")
}

#[tokio::test]
#[serial(makemigrations_e2e_errors)]
async fn ec_02_empty_flag_without_app_label_error() {
	// Test: Error when app_label is missing with --empty (E2E)
	todo!("Implement E2E empty flag error test")
}

#[tokio::test]
#[serial(makemigrations_e2e_errors)]
async fn ec_03_from_state_construction_failure_error() {
	// Test: Error when from_state construction fails (E2E)
	todo!("Implement E2E from_state failure test")
}

#[tokio::test]
#[serial(makemigrations_e2e_errors)]
async fn ec_04_invalid_field_definition_error() {
	// Test: Error for invalid field definition (E2E)
	todo!("Implement E2E invalid field error test")
}

#[tokio::test]
#[serial(makemigrations_e2e_errors)]
async fn ec_05_file_write_permission_error() {
	// Test: File write permission error (E2E)

	let temp_dir = TempDir::new().unwrap();
	let migrations_dir = temp_dir.path().join("migrations");
	let app_dir = migrations_dir.join("todos");

	// Create directory
	std::fs::create_dir_all(&app_dir).unwrap();

	// Make directory read-only (remove write permission)
	#[cfg(unix)]
	{
		use std::os::unix::fs::PermissionsExt;
		let metadata = std::fs::metadata(&app_dir).unwrap();
		let mut permissions = metadata.permissions();
		permissions.set_mode(0o444); // Read-only
		std::fs::set_permissions(&app_dir, permissions).unwrap();
	}

	let target_schema = create_todos_schema();
	let empty_schema = DatabaseSchema::default();

	let repository = Arc::new(Mutex::new(FilesystemRepository::new(
		migrations_dir.clone(),
	)));
	let source = Arc::new(FilesystemSource::new(migrations_dir.clone()));
	let service = MigrationService::new(source, repository.clone());

	let generator = AutoMigrationGenerator::new(target_schema, repository);
	let result = generator.generate("todos", empty_schema).await.unwrap();

	let migration = Migration {
		app_label: "todos".to_string(),
		name: "0001_initial".to_string(),
		operations: result.operations,
		dependencies: Vec::new(),
		atomic: true,
		replaces: Vec::new(),
		initial: Some(true),
		..Default::default()
	};

	// Try to save migration (should fail with permission error)
	let save_result = service.save_migration(&migration).await;

	#[cfg(unix)]
	{
		use std::os::unix::fs::PermissionsExt;
		assert!(save_result.is_err(), "Should fail with permission error");
		// Cleanup: restore permissions
		let metadata = std::fs::metadata(&app_dir).unwrap();
		let mut permissions = metadata.permissions();
		permissions.set_mode(0o755);
		std::fs::set_permissions(&app_dir, permissions).unwrap();
	}
}

// ============================================================================
// Edge Cases (EDG-01 ~ EDG-14)
// ============================================================================

#[tokio::test]
#[serial(makemigrations_e2e_edge)]
async fn edg_01_empty_migration_generation() {
	// Test: Empty migration (--empty) generation (E2E)
	todo!("Implement E2E empty migration test")
}

#[tokio::test]
#[serial(makemigrations_e2e_edge)]
async fn edg_02_no_changes_detected() {
	// Test: When no changes detected (E2E)
	todo!("Implement E2E no changes test")
}

#[tokio::test]
#[serial(makemigrations_e2e_edge)]
async fn edg_03_dry_run_mode() {
	// Test: --dry-run mode (E2E)
	todo!("Implement E2E dry-run test")
}

#[tokio::test]
#[serial(makemigrations_e2e_edge)]
async fn edg_04_custom_name_specification() {
	// Test: --name custom name specification (E2E)
	todo!("Implement E2E custom name test")
}

#[tokio::test]
#[serial(makemigrations_e2e_edge)]
async fn edg_05_verbose_mode() {
	// Test: --verbose mode (E2E)
	todo!("Implement E2E verbose mode test")
}

#[tokio::test]
#[serial(makemigrations_e2e_edge)]
async fn edg_06_custom_migrations_directory() {
	// Test: --migrations-dir custom directory specification (E2E)
	todo!("Implement E2E custom directory test")
}

#[tokio::test]
#[serial(makemigrations_e2e_edge)]
async fn edg_07_from_db_mode() {
	// Test: --from-db mode (E2E)
	todo!("Implement E2E from-db mode test")
}

#[tokio::test]
#[serial(makemigrations_e2e_edge)]
async fn edg_08_long_model_field_names() {
	// Test: Long model/field names (255 characters) (E2E)
	todo!("Implement E2E long names test")
}

#[tokio::test]
#[serial(makemigrations_e2e_edge)]
async fn edg_09_large_number_of_fields() {
	// Test: Large number of fields (100+) (E2E)
	todo!("Implement E2E many fields test")
}

#[tokio::test]
#[serial(makemigrations_e2e_edge)]
async fn edg_10_deep_dependency_chain() {
	// Test: Deep dependency chain (10 levels) (E2E)
	todo!("Implement E2E deep dependency test")
}

#[tokio::test]
#[serial(makemigrations_e2e_edge)]
async fn edg_11_unicode_in_names() {
	// Test: Names with special characters (Unicode) (E2E)
	todo!("Implement E2E unicode test")
}

#[tokio::test]
#[serial(makemigrations_e2e_edge)]
async fn edg_12_sql_reserved_words() {
	// Test: Table/column names containing SQL reserved words (E2E)
	todo!("Implement E2E reserved words test")
}

#[tokio::test]
#[serial(makemigrations_e2e_edge)]
async fn edg_13_same_name_different_apps() {
	// Test: Models with same name in different apps (E2E)
	todo!("Implement E2E same name test")
}

#[tokio::test]
#[serial(makemigrations_e2e_edge)]
async fn edg_14_cross_app_dependencies() {
	// Test: Cross-app dependencies (E2E)
	todo!("Implement E2E cross-app test")
}
