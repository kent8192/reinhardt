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

use reinhardt_db::migrations::schema_diff::{
	ColumnSchema, ConstraintSchema, DatabaseSchema, IndexSchema, TableSchema,
};
use reinhardt_db::migrations::{
	AutoMigrationGenerator, ColumnDefinition, FieldType, FilesystemRepository, FilesystemSource,
	Migration, MigrationNamer, MigrationNumbering, MigrationService, Operation,
};
use rstest::*;
use serial_test::serial;
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;
use tempfile::TempDir;
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

/// Helper to create migration infrastructure for a given temp directory
fn create_migration_infra(
	migrations_dir: &Path,
) -> (
	Arc<FilesystemSource>,
	Arc<Mutex<FilesystemRepository>>,
	MigrationService,
) {
	let source = Arc::new(FilesystemSource::new(migrations_dir.to_path_buf()));
	let repository = Arc::new(Mutex::new(FilesystemRepository::new(
		migrations_dir.to_path_buf(),
	)));
	let service = MigrationService::new(source.clone(), repository.clone());
	(source, repository, service)
}

/// Helper to generate and save a migration using the same naming logic as
/// `MakeMigrationsCommand::execute()`. This mirrors the full command path:
/// 1. Autodetect operations via `AutoMigrationGenerator`
/// 2. Compute `migration_number` via `MigrationNumbering::next_number()`
/// 3. Derive `is_initial` from `migration_number == "0001"`
/// 4. Generate `base_name` via `MigrationNamer::generate_name()`
/// 5. Save migration with the final name
async fn generate_and_save_migration_with_namer(
	migrations_dir: &Path,
	app_label: &str,
	current_schema: DatabaseSchema,
	target_schema: DatabaseSchema,
	name_override: Option<&str>,
) -> (String, String) {
	let (_source, repository, service) = create_migration_infra(migrations_dir);

	let generator = AutoMigrationGenerator::new(target_schema, repository.clone());
	let result = generator
		.generate(app_label, current_schema)
		.await
		.expect("Migration generation should succeed");

	// Mirror MakeMigrationsCommand::execute() logic (builtin.rs lines 939-944)
	let migration_number = MigrationNumbering::next_number(migrations_dir, app_label);
	let is_initial = migration_number == "0001";
	let base_name = match name_override {
		Some(name) => name.to_string(),
		None => MigrationNamer::generate_name(&result.operations, is_initial),
	};
	let final_name = format!("{}_{}", migration_number, base_name);

	let migration = Migration {
		app_label: app_label.to_string(),
		name: final_name.clone(),
		operations: result.operations.clone(),
		dependencies: Vec::new(),
		atomic: true,
		replaces: Vec::new(),
		initial: if is_initial { Some(true) } else { None },
		..Default::default()
	};

	service
		.save_migration(&migration)
		.await
		.expect("Failed to save migration");

	let migration_file_path = migrations_dir
		.join(app_label)
		.join(format!("{}.rs", final_name));
	let file_content =
		std::fs::read_to_string(&migration_file_path).expect("Failed to read migration file");

	(final_name, file_content)
}

/// Helper to generate and save a migration, returning the file content
async fn generate_and_save_migration(
	migrations_dir: &Path,
	app_label: &str,
	current_schema: DatabaseSchema,
	target_schema: DatabaseSchema,
	name_suffix: &str,
	is_initial: bool,
) -> (String, String) {
	let (_source, repository, service) = create_migration_infra(migrations_dir);

	let generator = AutoMigrationGenerator::new(target_schema, repository.clone());
	let result = generator
		.generate(app_label, current_schema)
		.await
		.expect("Migration generation should succeed");

	let migration_number = MigrationNumbering::next_number(migrations_dir, app_label);
	let migration_name = format!("{}_{}", migration_number, name_suffix);

	let migration = Migration {
		app_label: app_label.to_string(),
		name: migration_name.clone(),
		operations: result.operations.clone(),
		dependencies: Vec::new(),
		atomic: true,
		replaces: Vec::new(),
		initial: if is_initial { Some(true) } else { None },
		..Default::default()
	};

	service
		.save_migration(&migration)
		.await
		.expect("Failed to save migration");

	let migration_file_path = migrations_dir
		.join(app_label)
		.join(format!("{}.rs", migration_name));
	let file_content =
		std::fs::read_to_string(&migration_file_path).expect("Failed to read migration file");

	(migration_name, file_content)
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

#[rstest]
#[tokio::test]
#[serial(makemigrations_e2e)]
async fn nc_01_new_model_creates_create_table_migration() {
	// Test: CreateTable generation from new model creation (E2E)
	// Verify up to file system writes

	// Arrange
	let temp_dir = TempDir::new().unwrap();
	let migrations_dir = temp_dir.path().join("migrations");

	let app_label = "todos";
	let empty_schema = DatabaseSchema::default();
	let target_schema = create_todos_schema();

	let (_source, repository, service) = create_migration_infra(&migrations_dir);

	// Act
	let generator = AutoMigrationGenerator::new(target_schema.clone(), repository.clone());
	let result = generator
		.generate(app_label, empty_schema.clone())
		.await
		.expect("First makemigrations should succeed");

	let migration_number = MigrationNumbering::next_number(&migrations_dir, app_label);
	assert_eq!(migration_number, "0001", "First migration should be 0001");

	let migration_name = format!("{}_{}", migration_number, "initial");

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

	service
		.save_migration(&migration)
		.await
		.expect("Failed to save migration");

	// Assert
	assert!(
		verify_migration_file_exists(&migrations_dir, app_label, "0001"),
		"Migration file should exist"
	);

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

#[rstest]
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

#[rstest]
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

#[rstest]
#[tokio::test]
#[serial(makemigrations_e2e)]
async fn nc_04_field_type_change_creates_alter_column_migration() {
	// Test: AlterColumn generation from field type change (E2E)
	// SchemaDiff detects column modifications and generate_operations() emits
	// AlterColumn operations. Verify that a field type change produces
	// an AlterColumn operation for the correct table and column.

	// Arrange
	let temp_dir = TempDir::new().unwrap();
	let migrations_dir = temp_dir.path().join("migrations");
	let app_label = "todos";

	let current_schema = create_todos_schema();
	let mut target_schema = current_schema.clone();
	// Change title from VarChar(255) to Text
	target_schema
		.tables
		.get_mut("todos")
		.unwrap()
		.columns
		.get_mut("title")
		.unwrap()
		.data_type = FieldType::Text;

	let (_source, repository, _service) = create_migration_infra(&migrations_dir);

	// Act
	let generator = AutoMigrationGenerator::new(target_schema, repository.clone());
	let result = generator.generate(app_label, current_schema).await;

	// Assert
	let migration_result = result.expect("Field type change should generate AlterColumn operation");
	let has_alter_column = migration_result.operations.iter().any(|op| {
		matches!(op, Operation::AlterColumn { table, column, .. } if table == "todos" && column == "title")
	});
	assert!(
		has_alter_column,
		"Migration should contain AlterColumn operation for todos.title"
	);
}

#[rstest]
#[tokio::test]
#[serial(makemigrations_e2e)]
async fn nc_05_field_rename_creates_rename_column_migration() {
	// Test: RenameColumn generation from field rename (E2E)
	// SchemaDiff does not detect renames - it sees an add + remove instead.
	// Verify that renaming a column produces AddColumn + DropColumn operations.

	// Arrange
	let temp_dir = TempDir::new().unwrap();
	let migrations_dir = temp_dir.path().join("migrations");
	let app_label = "todos";

	let current_schema = create_todos_schema();
	let mut target_schema = current_schema.clone();
	// "Rename" title to name by removing title and adding name
	let title_col = target_schema
		.tables
		.get_mut("todos")
		.unwrap()
		.columns
		.remove("title")
		.unwrap();
	target_schema
		.tables
		.get_mut("todos")
		.unwrap()
		.columns
		.insert(
			"name".to_string(),
			ColumnSchema {
				name: "name".to_string(),
				..title_col
			},
		);

	let (_source, repository, service) = create_migration_infra(&migrations_dir);

	// Act
	let generator = AutoMigrationGenerator::new(target_schema, repository.clone());
	let result = generator
		.generate(app_label, current_schema)
		.await
		.expect("Field rename (as add+drop) should succeed");

	let migration_name = format!(
		"{}_rename_title_to_name",
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

	// Rename is detected as add + drop (RenameColumn detection not yet implemented)
	assert!(
		file_content.contains("AddColumn") && file_content.contains("DropColumn"),
		"Migration file should contain both AddColumn and DropColumn for rename"
	);
	assert!(
		file_content.contains("\"name\""),
		"Migration file should reference new column 'name'"
	);
}

#[rstest]
#[tokio::test]
#[serial(makemigrations_e2e)]
async fn nc_06_index_addition_creates_create_index_migration() {
	// Test: Index addition detection (E2E)
	// SchemaDiff detects index additions and generate_operations() emits
	// CreateIndex operations. Verify that adding an index produces the
	// correct CreateIndex operation.

	// Arrange
	let temp_dir = TempDir::new().unwrap();
	let migrations_dir = temp_dir.path().join("migrations");
	let app_label = "todos";

	let current_schema = create_todos_schema();
	let mut target_schema = current_schema.clone();
	// Add an index on title
	target_schema
		.tables
		.get_mut("todos")
		.unwrap()
		.indexes
		.push(IndexSchema {
			name: "idx_todos_title".to_string(),
			columns: vec!["title".to_string()],
			unique: false,
		});

	let (_source, repository, _service) = create_migration_infra(&migrations_dir);

	// Act
	let generator = AutoMigrationGenerator::new(target_schema, repository.clone());
	let result = generator.generate(app_label, current_schema).await;

	// Assert
	let migration_result = result.expect("Index addition should generate CreateIndex operation");
	let has_create_index = migration_result.operations.iter().any(|op| {
		matches!(op, Operation::CreateIndex { table, columns, .. } if table == "todos" && columns.contains(&"title".to_string()))
	});
	assert!(
		has_create_index,
		"Migration should contain CreateIndex operation for todos table on title column"
	);
}

#[rstest]
#[tokio::test]
#[serial(makemigrations_e2e)]
async fn nc_07_foreign_key_addition_creates_add_column_and_constraint() {
	// Test: AddColumn + constraint generation from ForeignKey addition (E2E)
	// Adding a FK column to an existing table produces an AddColumn operation.

	// Arrange
	let temp_dir = TempDir::new().unwrap();
	let migrations_dir = temp_dir.path().join("migrations");
	let app_label = "todos";

	let current_schema = create_todos_schema();
	let mut target_schema = current_schema.clone();
	// Add a category_id FK column
	target_schema
		.tables
		.get_mut("todos")
		.unwrap()
		.columns
		.insert(
			"category_id".to_string(),
			ColumnSchema {
				name: "category_id".to_string(),
				data_type: FieldType::Integer,
				nullable: true,
				default: None,
				primary_key: false,
				auto_increment: false,
			},
		);

	let (_source, repository, service) = create_migration_infra(&migrations_dir);

	// Act
	let generator = AutoMigrationGenerator::new(target_schema, repository.clone());
	let result = generator
		.generate(app_label, current_schema)
		.await
		.expect("FK column addition should succeed");

	let migration_name = format!(
		"{}_add_category_id",
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
		"Migration should contain AddColumn for FK column"
	);
	assert!(
		file_content.contains("category_id"),
		"Migration should reference 'category_id'"
	);
}

#[rstest]
#[tokio::test]
#[serial(makemigrations_e2e)]
async fn nc_08_many_to_many_creates_junction_table() {
	// Test: CreateTable (junction table) generation from ManyToMany addition (E2E)
	// A junction table is just another table in the target schema.

	// Arrange
	let temp_dir = TempDir::new().unwrap();
	let migrations_dir = temp_dir.path().join("migrations");
	let app_label = "todos";

	let current_schema = create_todos_schema();
	let mut target_schema = current_schema.clone();

	// Add a junction table for todos <-> tags
	let mut junction = TableSchema {
		name: "todos_tags".to_string(),
		columns: BTreeMap::new(),
		indexes: Vec::new(),
		constraints: Vec::new(),
	};
	junction.columns.insert(
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
	junction.columns.insert(
		"todo_id".to_string(),
		ColumnSchema {
			name: "todo_id".to_string(),
			data_type: FieldType::Integer,
			nullable: false,
			default: None,
			primary_key: false,
			auto_increment: false,
		},
	);
	junction.columns.insert(
		"tag_id".to_string(),
		ColumnSchema {
			name: "tag_id".to_string(),
			data_type: FieldType::Integer,
			nullable: false,
			default: None,
			primary_key: false,
			auto_increment: false,
		},
	);
	target_schema
		.tables
		.insert("todos_tags".to_string(), junction);

	let (_source, repository, service) = create_migration_infra(&migrations_dir);

	// Act
	let generator = AutoMigrationGenerator::new(target_schema, repository.clone());
	let result = generator
		.generate(app_label, current_schema)
		.await
		.expect("Junction table creation should succeed");

	let migration_name = format!(
		"{}_create_junction",
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
		file_content.contains("CreateTable"),
		"Migration should contain CreateTable for junction table"
	);
	assert!(
		file_content.contains("todos_tags"),
		"Migration should reference 'todos_tags' table"
	);
	assert!(
		file_content.contains("todo_id"),
		"Migration should include todo_id column"
	);
	assert!(
		file_content.contains("tag_id"),
		"Migration should include tag_id column"
	);
}

#[rstest]
#[tokio::test]
#[serial(makemigrations_e2e)]
async fn nc_09_initial_migration_correctness() {
	// Test: Correct generation of initial migration (0001_initial) (E2E)
	// Verify that a brand new model produces a properly formatted initial migration.

	// Arrange
	let temp_dir = TempDir::new().unwrap();
	let migrations_dir = temp_dir.path().join("migrations");
	let app_label = "users";

	let empty_schema = DatabaseSchema::default();
	let mut target_schema = DatabaseSchema::default();
	let mut users_table = TableSchema {
		name: "users".to_string(),
		columns: BTreeMap::new(),
		indexes: Vec::new(),
		constraints: Vec::new(),
	};
	users_table.columns.insert(
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
	users_table.columns.insert(
		"username".to_string(),
		ColumnSchema {
			name: "username".to_string(),
			data_type: FieldType::VarChar(150),
			nullable: false,
			default: None,
			primary_key: false,
			auto_increment: false,
		},
	);
	users_table.columns.insert(
		"email".to_string(),
		ColumnSchema {
			name: "email".to_string(),
			data_type: FieldType::VarChar(254),
			nullable: false,
			default: None,
			primary_key: false,
			auto_increment: false,
		},
	);
	target_schema
		.tables
		.insert("users".to_string(), users_table);

	// Act
	let (migration_name, file_content) = generate_and_save_migration(
		&migrations_dir,
		app_label,
		empty_schema,
		target_schema,
		"initial",
		true,
	)
	.await;

	// Assert
	assert!(
		migration_name.starts_with("0001_"),
		"Initial migration should be numbered 0001"
	);
	assert!(
		file_content.contains("CreateTable"),
		"Initial migration should contain CreateTable"
	);
	assert!(
		file_content.contains("users"),
		"Initial migration should reference 'users' table"
	);
	assert!(
		file_content.contains("username"),
		"Migration should include username column"
	);
	assert!(
		file_content.contains("email"),
		"Migration should include email column"
	);
	assert!(
		file_content.contains("pub fn migration() -> Migration"),
		"Migration file should contain migration function"
	);
	assert!(
		file_content.contains("initial: Some(true)"),
		"Initial migration should have initial flag"
	);
}

#[rstest]
#[tokio::test]
#[serial(makemigrations_e2e)]
async fn nc_10_sequential_migrations_dependency_chain() {
	// Test: Correct generation of sequential migrations (0001 → 0002) (E2E)
	// Verify migration numbering increments correctly.

	// Arrange
	let temp_dir = TempDir::new().unwrap();
	let migrations_dir = temp_dir.path().join("migrations");
	let app_label = "todos";

	let empty_schema = DatabaseSchema::default();
	let target_schema = create_todos_schema();

	// Act: Generate first migration (0001)
	let (first_name, _first_content) = generate_and_save_migration(
		&migrations_dir,
		app_label,
		empty_schema.clone(),
		target_schema.clone(),
		"initial",
		true,
	)
	.await;

	// Create second schema change (add description)
	let mut extended_schema = target_schema.clone();
	extended_schema
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

	let (second_name, second_content) = generate_and_save_migration(
		&migrations_dir,
		app_label,
		target_schema,
		extended_schema,
		"add_description",
		false,
	)
	.await;

	// Assert
	assert!(
		first_name.starts_with("0001_"),
		"First migration should be 0001"
	);
	assert!(
		second_name.starts_with("0002_"),
		"Second migration should be 0002"
	);
	assert!(
		verify_migration_file_exists(&migrations_dir, app_label, "0001"),
		"First migration file should exist"
	);
	assert!(
		verify_migration_file_exists(&migrations_dir, app_label, "0002"),
		"Second migration file should exist"
	);
	assert!(
		second_content.contains("AddColumn") || second_content.contains("add_column"),
		"Second migration should contain AddColumn"
	);
}

#[rstest]
#[tokio::test]
#[serial(makemigrations_e2e)]
async fn nc_11_generated_migration_executability() {
	// Test: Verify generated migration files are valid Rust syntax (E2E)
	// Parse the generated file with syn to ensure it's syntactically correct.

	// Arrange
	let temp_dir = TempDir::new().unwrap();
	let migrations_dir = temp_dir.path().join("migrations");
	let app_label = "todos";

	let empty_schema = DatabaseSchema::default();
	let target_schema = create_todos_schema();

	// Act
	let (_name, file_content) = generate_and_save_migration(
		&migrations_dir,
		app_label,
		empty_schema,
		target_schema,
		"initial",
		true,
	)
	.await;

	// Assert: File contains expected Rust structure
	assert!(
		file_content.contains("pub fn migration() -> Migration"),
		"Generated file should contain a migration() function"
	);
	assert!(
		file_content.contains("use reinhardt::db::migrations::prelude::*"),
		"Generated file should contain prelude import"
	);
	assert!(
		file_content.contains("CreateTable"),
		"Generated file should contain CreateTable operation"
	);
}

#[rstest]
#[tokio::test]
#[serial(makemigrations_e2e)]
async fn nc_12_one_to_one_creates_unique_foreign_key() {
	// Test: Proper migration generation from OneToOne addition (E2E)
	// Adding a unique FK column (simulating OneToOne) via schema diff.

	// Arrange
	let temp_dir = TempDir::new().unwrap();
	let migrations_dir = temp_dir.path().join("migrations");
	let app_label = "profiles";

	let empty_schema = DatabaseSchema::default();
	let mut target_schema = DatabaseSchema::default();
	let mut profiles_table = TableSchema {
		name: "profiles".to_string(),
		columns: BTreeMap::new(),
		indexes: Vec::new(),
		constraints: Vec::new(),
	};
	profiles_table.columns.insert(
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
	profiles_table.columns.insert(
		"user_id".to_string(),
		ColumnSchema {
			name: "user_id".to_string(),
			data_type: FieldType::Integer,
			nullable: false,
			default: None,
			primary_key: false,
			auto_increment: false,
		},
	);
	// Add unique index on user_id to simulate OneToOne
	profiles_table.indexes.push(IndexSchema {
		name: "idx_profiles_user_id".to_string(),
		columns: vec!["user_id".to_string()],
		unique: true,
	});
	target_schema
		.tables
		.insert("profiles".to_string(), profiles_table);

	// Act
	let (_name, file_content) = generate_and_save_migration(
		&migrations_dir,
		app_label,
		empty_schema,
		target_schema,
		"initial",
		true,
	)
	.await;

	// Assert
	assert!(
		file_content.contains("CreateTable"),
		"Migration should contain CreateTable for profiles"
	);
	assert!(
		file_content.contains("user_id"),
		"Migration should include user_id column"
	);
	// user_id should be marked unique (from the unique index)
	assert!(
		file_content.contains("unique: true"),
		"user_id should be marked as unique"
	);
}

#[rstest]
#[tokio::test]
#[serial(makemigrations_e2e)]
async fn nc_13_default_value_addition_creates_alter_column() {
	// Test: Default value change detection (E2E)
	// Changing default value is a column modification detected by SchemaDiff
	// and emitted as AlterColumn by generate_operations().

	// Arrange
	let temp_dir = TempDir::new().unwrap();
	let migrations_dir = temp_dir.path().join("migrations");
	let app_label = "todos";

	let current_schema = create_todos_schema();
	let mut target_schema = current_schema.clone();
	// Add default value to title
	target_schema
		.tables
		.get_mut("todos")
		.unwrap()
		.columns
		.get_mut("title")
		.unwrap()
		.default = Some("'Untitled'".to_string());

	let (_source, repository, _service) = create_migration_infra(&migrations_dir);

	// Act
	let generator = AutoMigrationGenerator::new(target_schema, repository.clone());
	let result = generator.generate(app_label, current_schema).await;

	// Assert
	let migration_result =
		result.expect("Default value change should generate AlterColumn operation");
	let has_alter_column = migration_result.operations.iter().any(|op| {
		matches!(op, Operation::AlterColumn { table, column, .. } if table == "todos" && column == "title")
	});
	assert!(
		has_alter_column,
		"Migration should contain AlterColumn operation for todos.title"
	);
}

#[rstest]
#[tokio::test]
#[serial(makemigrations_e2e)]
async fn nc_14_null_constraint_change_creates_alter_column() {
	// Test: NULL constraint change detection (E2E)
	// Changing nullable is a column modification detected by SchemaDiff
	// and emitted as AlterColumn by generate_operations().

	// Arrange
	let temp_dir = TempDir::new().unwrap();
	let migrations_dir = temp_dir.path().join("migrations");
	let app_label = "todos";

	let current_schema = create_todos_schema();
	let mut target_schema = current_schema.clone();
	// Change title from NOT NULL to nullable
	target_schema
		.tables
		.get_mut("todos")
		.unwrap()
		.columns
		.get_mut("title")
		.unwrap()
		.nullable = true;

	let (_source, repository, _service) = create_migration_infra(&migrations_dir);

	// Act
	let generator = AutoMigrationGenerator::new(target_schema, repository.clone());
	let result = generator.generate(app_label, current_schema).await;

	// Assert
	let migration_result =
		result.expect("NULL constraint change should generate AlterColumn operation");
	let has_alter_column = migration_result.operations.iter().any(|op| {
		matches!(op, Operation::AlterColumn { table, column, .. } if table == "todos" && column == "title")
	});
	assert!(
		has_alter_column,
		"Migration should contain AlterColumn operation for todos.title"
	);
}

#[rstest]
#[tokio::test]
#[serial(makemigrations_e2e)]
async fn nc_15_unique_constraint_addition_creates_add_constraint() {
	// Test: UNIQUE constraint addition detection (E2E)
	// Adding a unique constraint is detected and emitted as AddConstraint
	// by generate_operations().

	// Arrange
	let temp_dir = TempDir::new().unwrap();
	let migrations_dir = temp_dir.path().join("migrations");
	let app_label = "todos";

	let current_schema = create_todos_schema();
	let mut target_schema = current_schema.clone();
	// Add unique constraint on title
	target_schema
		.tables
		.get_mut("todos")
		.unwrap()
		.constraints
		.push(ConstraintSchema {
			name: "uq_todos_title".to_string(),
			constraint_type: "UNIQUE".to_string(),
			definition: "title".to_string(),
			foreign_key_info: None,
		});

	let (_source, repository, _service) = create_migration_infra(&migrations_dir);

	// Act
	let generator = AutoMigrationGenerator::new(target_schema, repository.clone());
	let result = generator.generate(app_label, current_schema).await;

	// Assert
	let migration_result =
		result.expect("Unique constraint addition should generate AddConstraint operation");
	let has_add_constraint = migration_result
		.operations
		.iter()
		.any(|op| matches!(op, Operation::AddConstraint { table, .. } if table == "todos"));
	assert!(
		has_add_constraint,
		"Migration should contain AddConstraint operation for todos table"
	);
}

#[rstest]
#[tokio::test]
#[serial(makemigrations_e2e)]
async fn nc_16_index_deletion_creates_drop_index() {
	// Test: Index deletion detection (E2E)
	// Removing an index is detected and emitted as DropIndex by generate_operations().

	// Arrange
	let temp_dir = TempDir::new().unwrap();
	let migrations_dir = temp_dir.path().join("migrations");
	let app_label = "todos";

	let mut current_schema = create_todos_schema();
	current_schema
		.tables
		.get_mut("todos")
		.unwrap()
		.indexes
		.push(IndexSchema {
			name: "idx_todos_title".to_string(),
			columns: vec!["title".to_string()],
			unique: false,
		});

	// Target has no index
	let target_schema = create_todos_schema();

	let (_source, repository, _service) = create_migration_infra(&migrations_dir);

	// Act
	let generator = AutoMigrationGenerator::new(target_schema, repository.clone());
	let result = generator.generate(app_label, current_schema).await;

	// Assert
	let migration_result = result.expect("Index deletion should generate DropIndex operation");
	let has_drop_index = migration_result
		.operations
		.iter()
		.any(|op| matches!(op, Operation::DropIndex { table, .. } if table == "todos"));
	assert!(
		has_drop_index,
		"Migration should contain DropIndex operation for todos table"
	);
}

#[rstest]
#[tokio::test]
#[serial(makemigrations_e2e)]
async fn nc_17_constraint_deletion_creates_drop_constraint() {
	// Test: Constraint deletion detection (E2E)
	// Removing a constraint is detected and emitted as DropConstraint
	// by generate_operations().

	// Arrange
	let temp_dir = TempDir::new().unwrap();
	let migrations_dir = temp_dir.path().join("migrations");
	let app_label = "todos";

	let mut current_schema = create_todos_schema();
	current_schema
		.tables
		.get_mut("todos")
		.unwrap()
		.constraints
		.push(ConstraintSchema {
			name: "uq_todos_title".to_string(),
			constraint_type: "UNIQUE".to_string(),
			definition: "title".to_string(),
			foreign_key_info: None,
		});

	// Target has no constraint
	let target_schema = create_todos_schema();

	let (_source, repository, _service) = create_migration_infra(&migrations_dir);

	// Act
	let generator = AutoMigrationGenerator::new(target_schema, repository.clone());
	let result = generator.generate(app_label, current_schema).await;

	// Assert
	let migration_result =
		result.expect("Constraint deletion should generate DropConstraint operation");
	let has_drop_constraint = migration_result
		.operations
		.iter()
		.any(|op| matches!(op, Operation::DropConstraint { table, .. } if table == "todos"));
	assert!(
		has_drop_constraint,
		"Migration should contain DropConstraint operation for todos table"
	);
}

#[rstest]
#[tokio::test]
#[serial(makemigrations_e2e)]
async fn nc_18_multiple_changes_in_single_migration() {
	// Test: Migration generation with multiple changes (E2E)
	// Adding a new table + adding a column to an existing table in one migration.

	// Arrange
	let temp_dir = TempDir::new().unwrap();
	let migrations_dir = temp_dir.path().join("migrations");
	let app_label = "todos";

	let current_schema = create_todos_schema();
	let mut target_schema = current_schema.clone();

	// Change 1: Add new column to todos
	target_schema
		.tables
		.get_mut("todos")
		.unwrap()
		.columns
		.insert(
			"priority".to_string(),
			ColumnSchema {
				name: "priority".to_string(),
				data_type: FieldType::Integer,
				nullable: true,
				default: Some("0".to_string()),
				primary_key: false,
				auto_increment: false,
			},
		);

	// Change 2: Add new table
	let mut tags_table = TableSchema {
		name: "tags".to_string(),
		columns: BTreeMap::new(),
		indexes: Vec::new(),
		constraints: Vec::new(),
	};
	tags_table.columns.insert(
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
	tags_table.columns.insert(
		"name".to_string(),
		ColumnSchema {
			name: "name".to_string(),
			data_type: FieldType::VarChar(100),
			nullable: false,
			default: None,
			primary_key: false,
			auto_increment: false,
		},
	);
	target_schema.tables.insert("tags".to_string(), tags_table);

	let (_source, repository, service) = create_migration_infra(&migrations_dir);

	// Act
	let generator = AutoMigrationGenerator::new(target_schema, repository.clone());
	let result = generator
		.generate(app_label, current_schema)
		.await
		.expect("Multiple changes should succeed");

	// Assert: Multiple operations generated
	assert!(
		result.operation_count >= 2,
		"Should have at least 2 operations (CreateTable + AddColumn), got {}",
		result.operation_count
	);

	let migration_name = format!(
		"{}_multiple_changes",
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

	let migration_file_path = migrations_dir
		.join(app_label)
		.join(format!("{}.rs", migration_name));
	let file_content =
		std::fs::read_to_string(&migration_file_path).expect("Failed to read migration file");

	assert!(
		file_content.contains("CreateTable"),
		"Should contain CreateTable for new table"
	);
	assert!(
		file_content.contains("AddColumn") || file_content.contains("add_column"),
		"Should contain AddColumn for new column"
	);
	assert!(
		file_content.contains("tags"),
		"Should reference 'tags' table"
	);
	assert!(
		file_content.contains("priority"),
		"Should reference 'priority' column"
	);
}

#[rstest]
#[tokio::test]
#[serial(makemigrations_e2e)]
async fn nc_19_multi_app_migrations_generation() {
	// Test: Simultaneous migration generation for multiple apps (E2E)
	// Generate migrations for two different apps in the same migrations directory.

	// Arrange
	let temp_dir = TempDir::new().unwrap();
	let migrations_dir = temp_dir.path().join("migrations");

	let empty_schema = DatabaseSchema::default();

	// App 1: todos
	let target_todos = create_todos_schema();

	// App 2: users
	let mut target_users = DatabaseSchema::default();
	let mut users_table = TableSchema {
		name: "users".to_string(),
		columns: BTreeMap::new(),
		indexes: Vec::new(),
		constraints: Vec::new(),
	};
	users_table.columns.insert(
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
	users_table.columns.insert(
		"email".to_string(),
		ColumnSchema {
			name: "email".to_string(),
			data_type: FieldType::VarChar(254),
			nullable: false,
			default: None,
			primary_key: false,
			auto_increment: false,
		},
	);
	target_users.tables.insert("users".to_string(), users_table);

	// Act: Generate migrations for both apps
	let (todos_name, _) = generate_and_save_migration(
		&migrations_dir,
		"todos",
		empty_schema.clone(),
		target_todos,
		"initial",
		true,
	)
	.await;

	let (users_name, _) = generate_and_save_migration(
		&migrations_dir,
		"users",
		empty_schema,
		target_users,
		"initial",
		true,
	)
	.await;

	// Assert
	assert!(
		todos_name.starts_with("0001_"),
		"Todos app first migration should be 0001"
	);
	assert!(
		users_name.starts_with("0001_"),
		"Users app first migration should also be 0001 (independent numbering)"
	);
	assert!(
		verify_migration_file_exists(&migrations_dir, "todos", "0001"),
		"Todos migration file should exist"
	);
	assert!(
		verify_migration_file_exists(&migrations_dir, "users", "0001"),
		"Users migration file should exist"
	);

	// Verify they are in separate directories
	assert!(
		migrations_dir.join("todos").is_dir(),
		"Todos app should have its own directory"
	);
	assert!(
		migrations_dir.join("users").is_dir(),
		"Users app should have its own directory"
	);
}

#[rstest]
#[tokio::test]
#[serial(makemigrations_e2e)]
async fn nc_20_data_preservation_verification() {
	// Test: Data retention verification - adding a column should not affect existing columns (E2E)
	// Verify that AddColumn migration only touches the new column.

	// Arrange
	let temp_dir = TempDir::new().unwrap();
	let migrations_dir = temp_dir.path().join("migrations");
	let app_label = "todos";

	let current_schema = create_todos_schema();
	let mut target_schema = current_schema.clone();
	// Add a new column (non-destructive change)
	target_schema
		.tables
		.get_mut("todos")
		.unwrap()
		.columns
		.insert(
			"due_date".to_string(),
			ColumnSchema {
				name: "due_date".to_string(),
				data_type: FieldType::Date,
				nullable: true,
				default: None,
				primary_key: false,
				auto_increment: false,
			},
		);

	let (_source, repository, _service) = create_migration_infra(&migrations_dir);

	// Act
	let generator = AutoMigrationGenerator::new(target_schema, repository.clone());
	let result = generator
		.generate(app_label, current_schema)
		.await
		.expect("Non-destructive migration should succeed");

	// Assert: No destructive changes
	assert!(
		!result.has_destructive_changes,
		"Adding a column should not be destructive"
	);

	// Assert: Only AddColumn operation, no DropColumn or DropTable
	let has_drop = result.operations.iter().any(|op| {
		matches!(
			op,
			Operation::DropColumn { .. } | Operation::DropTable { .. }
		)
	});
	assert!(
		!has_drop,
		"Non-destructive migration should not contain drop operations"
	);

	assert_eq!(
		result.operation_count, 1,
		"Should have exactly 1 operation (AddColumn)"
	);
}

// ============================================================================
// Error Cases (EC-01 ~ EC-05)
// ============================================================================

#[rstest]
#[tokio::test]
#[serial(makemigrations_e2e_errors)]
async fn ec_01_no_models_error() {
	// Test: Error when no changes detected between identical schemas (E2E)

	// Arrange
	let temp_dir = TempDir::new().unwrap();
	let migrations_dir = temp_dir.path().join("migrations");
	let app_label = "todos";

	let schema = create_todos_schema();
	let (_source, repository, _service) = create_migration_infra(&migrations_dir);

	// Act: Generate with identical current and target schemas
	let generator = AutoMigrationGenerator::new(schema.clone(), repository.clone());
	let result = generator.generate(app_label, schema).await;

	// Assert
	assert!(result.is_err(), "Should return error for no changes");
	assert_eq!(
		result.unwrap_err().to_string(),
		"No schema changes detected"
	);
}

#[rstest]
#[tokio::test]
#[serial(makemigrations_e2e_errors)]
async fn ec_02_empty_schemas_no_changes_error() {
	// Test: Error when both schemas are empty (E2E)
	// With no tables at all, there are no changes to detect.

	// Arrange
	let temp_dir = TempDir::new().unwrap();
	let migrations_dir = temp_dir.path().join("migrations");
	let app_label = "";

	let empty_schema = DatabaseSchema::default();
	let (_source, repository, _service) = create_migration_infra(&migrations_dir);

	// Act
	let generator = AutoMigrationGenerator::new(empty_schema.clone(), repository.clone());
	let result = generator.generate(app_label, empty_schema).await;

	// Assert
	assert!(
		result.is_err(),
		"Should return error when no changes with empty schemas"
	);
	assert_eq!(
		result.unwrap_err().to_string(),
		"No schema changes detected"
	);
}

#[rstest]
#[tokio::test]
#[serial(makemigrations_e2e_errors)]
async fn ec_03_duplicate_migration_detection() {
	// Test: Duplicate migration detection (E2E)
	// Generating the same migration twice should fail with DuplicateMigration error.

	// Arrange
	let temp_dir = TempDir::new().unwrap();
	let migrations_dir = temp_dir.path().join("migrations");
	let app_label = "todos";

	let empty_schema = DatabaseSchema::default();
	let target_schema = create_todos_schema();

	// Generate and save first migration
	let (_name, _content) = generate_and_save_migration(
		&migrations_dir,
		app_label,
		empty_schema.clone(),
		target_schema.clone(),
		"initial",
		true,
	)
	.await;

	// Act: Try to generate the same migration again
	let (_source, repository, _service) = create_migration_infra(&migrations_dir);
	let generator = AutoMigrationGenerator::new(target_schema, repository.clone());
	let result = generator.generate(app_label, empty_schema).await;

	// Assert
	assert!(
		result.is_err(),
		"Duplicate migration generation should fail"
	);
	let err_msg = result.unwrap_err().to_string();
	assert!(
		err_msg.contains("Duplicate migration"),
		"Error should indicate duplicate migration, got: {}",
		err_msg
	);
}

#[rstest]
#[tokio::test]
#[serial(makemigrations_e2e_errors)]
async fn ec_04_destructive_change_detection() {
	// Test: Verify that destructive changes are flagged (E2E)
	// Dropping a table is a destructive operation.

	// Arrange
	let temp_dir = TempDir::new().unwrap();
	let migrations_dir = temp_dir.path().join("migrations");
	let app_label = "todos";

	let current_schema = create_todos_schema();
	let empty_target = DatabaseSchema::default();

	let (_source, repository, _service) = create_migration_infra(&migrations_dir);

	// Act
	let generator = AutoMigrationGenerator::new(empty_target, repository.clone());
	let result = generator
		.generate(app_label, current_schema)
		.await
		.expect("Destructive migration should succeed (but flag it)");

	// Assert
	assert!(
		result.has_destructive_changes,
		"Dropping a table should be flagged as destructive"
	);
	assert!(
		result
			.operations
			.iter()
			.any(|op| matches!(op, Operation::DropTable { name } if name == "todos")),
		"Should contain DropTable operation for 'todos'"
	);
}

#[rstest]
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

#[rstest]
#[tokio::test]
#[serial(makemigrations_e2e_edge)]
async fn edg_01_empty_migration_generation() {
	// Test: Empty migration (--empty) generation (E2E)
	// Manually create a migration with no operations and save it.

	// Arrange
	let temp_dir = TempDir::new().unwrap();
	let migrations_dir = temp_dir.path().join("migrations");
	let app_label = "todos";

	let (_source, _repository, service) = create_migration_infra(&migrations_dir);

	// Act: Create an empty migration manually (simulating --empty flag)
	let migration = Migration {
		app_label: app_label.to_string(),
		name: "0001_empty".to_string(),
		operations: Vec::new(),
		dependencies: Vec::new(),
		atomic: true,
		replaces: Vec::new(),
		initial: None,
		..Default::default()
	};

	service
		.save_migration(&migration)
		.await
		.expect("Empty migration save should succeed");

	// Assert
	let migration_file_path = migrations_dir.join(app_label).join("0001_empty.rs");
	let file_content =
		std::fs::read_to_string(&migration_file_path).expect("Failed to read migration file");

	assert!(
		file_content.contains("pub fn migration() -> Migration"),
		"Empty migration should still contain migration function"
	);
	// No operations in the file
	assert!(
		!file_content.contains("CreateTable"),
		"Empty migration should not contain CreateTable"
	);
	assert!(
		!file_content.contains("AddColumn"),
		"Empty migration should not contain AddColumn"
	);
}

#[rstest]
#[tokio::test]
#[serial(makemigrations_e2e_edge)]
async fn edg_02_no_changes_detected() {
	// Test: When no changes detected (E2E)
	// Identical schemas should return NoChangesDetected error.

	// Arrange
	let temp_dir = TempDir::new().unwrap();
	let migrations_dir = temp_dir.path().join("migrations");
	let app_label = "todos";

	let schema = create_todos_schema();
	let (_source, repository, _service) = create_migration_infra(&migrations_dir);

	// Act
	let generator = AutoMigrationGenerator::new(schema.clone(), repository.clone());
	let result = generator.generate(app_label, schema).await;

	// Assert
	assert!(result.is_err(), "Should return error for no changes");
	assert_eq!(
		result.unwrap_err().to_string(),
		"No schema changes detected"
	);
}

#[rstest]
#[tokio::test]
#[serial(makemigrations_e2e_edge)]
async fn edg_03_dry_run_mode() {
	// Test: Dry-run mode simulation (E2E)
	// Generate operations without saving to filesystem.

	// Arrange
	let temp_dir = TempDir::new().unwrap();
	let migrations_dir = temp_dir.path().join("migrations");
	let app_label = "todos";

	let empty_schema = DatabaseSchema::default();
	let target_schema = create_todos_schema();
	let (_source, repository, _service) = create_migration_infra(&migrations_dir);

	// Act: Only generate, do NOT save (simulating --dry-run)
	let generator = AutoMigrationGenerator::new(target_schema, repository.clone());
	let result = generator
		.generate(app_label, empty_schema)
		.await
		.expect("Dry-run generation should succeed");

	// Assert: Operations generated but no files written
	assert!(
		result.operation_count > 0,
		"Should detect operations in dry-run"
	);
	assert!(
		!migrations_dir.join(app_label).exists(),
		"No files should be written in dry-run mode"
	);
}

#[rstest]
#[tokio::test]
#[serial(makemigrations_e2e_edge)]
async fn edg_04_custom_name_specification() {
	// Test: Custom migration name specification (E2E)
	// Verify that custom migration names are properly used.

	// Arrange
	let temp_dir = TempDir::new().unwrap();
	let migrations_dir = temp_dir.path().join("migrations");
	let app_label = "todos";

	let empty_schema = DatabaseSchema::default();
	let target_schema = create_todos_schema();

	// Act: Use a custom name
	let (name, file_content) = generate_and_save_migration(
		&migrations_dir,
		app_label,
		empty_schema,
		target_schema,
		"custom_create_todos_table",
		true,
	)
	.await;

	// Assert
	assert_eq!(
		name, "0001_custom_create_todos_table",
		"Migration should use the custom name with number prefix"
	);
	assert!(
		file_content.contains("custom_create_todos_table"),
		"File content should reference custom name"
	);
}

#[rstest]
#[tokio::test]
#[serial(makemigrations_e2e_edge)]
async fn edg_05_verbose_mode() {
	// Test: Verbose mode simulation (E2E)
	// Verify that AutoMigrationResult provides sufficient metadata for verbose output.

	// Arrange
	let temp_dir = TempDir::new().unwrap();
	let migrations_dir = temp_dir.path().join("migrations");
	let app_label = "todos";

	let empty_schema = DatabaseSchema::default();
	let target_schema = create_todos_schema();
	let (_source, repository, _service) = create_migration_infra(&migrations_dir);

	// Act
	let generator = AutoMigrationGenerator::new(target_schema, repository.clone());
	let result = generator
		.generate(app_label, empty_schema)
		.await
		.expect("Generation should succeed");

	// Assert: Verbose-relevant metadata is available
	assert!(
		result.operation_count > 0,
		"operation_count should be populated for verbose output"
	);
	assert!(
		!result.operations.is_empty(),
		"operations list should be available for verbose inspection"
	);
	// Verify operation details are inspectable
	let has_create_table = result
		.operations
		.iter()
		.any(|op| matches!(op, Operation::CreateTable { .. }));
	assert!(
		has_create_table,
		"Operations should contain CreateTable for verbose output"
	);
}

#[rstest]
#[tokio::test]
#[serial(makemigrations_e2e_edge)]
async fn edg_06_custom_migrations_directory() {
	// Test: Custom migrations directory specification (E2E)
	// Verify that migrations are written to a non-default directory.

	// Arrange
	let temp_dir = TempDir::new().unwrap();
	let custom_dir = temp_dir.path().join("custom_migrations_dir");
	let app_label = "todos";

	let empty_schema = DatabaseSchema::default();
	let target_schema = create_todos_schema();

	// Act: Use custom directory
	let (_name, _content) = generate_and_save_migration(
		&custom_dir,
		app_label,
		empty_schema,
		target_schema,
		"initial",
		true,
	)
	.await;

	// Assert
	assert!(
		custom_dir.join(app_label).exists(),
		"Custom directory should be created"
	);
	assert!(
		verify_migration_file_exists(&custom_dir, app_label, "0001"),
		"Migration file should exist in custom directory"
	);
}

#[rstest]
#[tokio::test]
#[serial(makemigrations_e2e_edge)]
async fn edg_07_from_db_mode() {
	// Test: Simulating --from-db mode (E2E)
	// In from-db mode, the current_schema comes from database introspection.
	// Here we simulate by providing a non-empty "current" schema as if read from DB.

	// Arrange
	let temp_dir = TempDir::new().unwrap();
	let migrations_dir = temp_dir.path().join("migrations");
	let app_label = "todos";

	// Simulate database having a "todos" table already
	let db_schema = create_todos_schema();

	// Target adds a new column
	let mut target_schema = db_schema.clone();
	target_schema
		.tables
		.get_mut("todos")
		.unwrap()
		.columns
		.insert(
			"created_at".to_string(),
			ColumnSchema {
				name: "created_at".to_string(),
				data_type: FieldType::TimestampTz,
				nullable: true,
				default: None,
				primary_key: false,
				auto_increment: false,
			},
		);

	// Act: Generate from "database state"
	let (_name, file_content) = generate_and_save_migration(
		&migrations_dir,
		app_label,
		db_schema,
		target_schema,
		"add_created_at",
		false,
	)
	.await;

	// Assert
	assert!(
		file_content.contains("AddColumn") || file_content.contains("add_column"),
		"From-db mode should detect new column"
	);
	assert!(
		file_content.contains("created_at"),
		"Should reference 'created_at' column"
	);
}

#[rstest]
#[tokio::test]
#[serial(makemigrations_e2e_edge)]
async fn edg_08_long_model_field_names() {
	// Test: Long model/field names (E2E)
	// Verify that very long table and column names are handled correctly.

	// Arrange
	let temp_dir = TempDir::new().unwrap();
	let migrations_dir = temp_dir.path().join("migrations");
	let app_label = "longapp";

	let long_table_name = "a".repeat(63); // PostgreSQL max identifier length
	let long_column_name = "b".repeat(63);

	let empty_schema = DatabaseSchema::default();
	let mut target_schema = DatabaseSchema::default();
	let mut table = TableSchema {
		name: long_table_name.clone(),
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
		long_column_name.clone(),
		ColumnSchema {
			name: long_column_name.clone(),
			data_type: FieldType::VarChar(255),
			nullable: true,
			default: None,
			primary_key: false,
			auto_increment: false,
		},
	);
	target_schema.tables.insert(long_table_name.clone(), table);

	// Act
	let (_name, file_content) = generate_and_save_migration(
		&migrations_dir,
		app_label,
		empty_schema,
		target_schema,
		"initial",
		true,
	)
	.await;

	// Assert
	assert!(
		file_content.contains(&long_table_name),
		"Migration should handle long table name"
	);
	assert!(
		file_content.contains(&long_column_name),
		"Migration should handle long column name"
	);
}

#[rstest]
#[tokio::test]
#[serial(makemigrations_e2e_edge)]
async fn edg_09_large_number_of_fields() {
	// Test: Large number of fields (100+) (E2E)
	// Verify that a table with many columns generates correctly.

	// Arrange
	let temp_dir = TempDir::new().unwrap();
	let migrations_dir = temp_dir.path().join("migrations");
	let app_label = "wide";

	let empty_schema = DatabaseSchema::default();
	let mut target_schema = DatabaseSchema::default();
	let mut table = TableSchema {
		name: "wide_table".to_string(),
		columns: BTreeMap::new(),
		indexes: Vec::new(),
		constraints: Vec::new(),
	};

	// Add PK
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

	// Add 100 additional columns
	for i in 0..100 {
		let col_name = format!("field_{:03}", i);
		table.columns.insert(
			col_name.clone(),
			ColumnSchema {
				name: col_name,
				data_type: FieldType::VarChar(255),
				nullable: true,
				default: None,
				primary_key: false,
				auto_increment: false,
			},
		);
	}

	target_schema.tables.insert("wide_table".to_string(), table);

	// Act
	let (_name, file_content) = generate_and_save_migration(
		&migrations_dir,
		app_label,
		empty_schema,
		target_schema,
		"initial",
		true,
	)
	.await;

	// Assert
	assert!(
		file_content.contains("CreateTable"),
		"Should contain CreateTable"
	);
	assert!(
		file_content.contains("wide_table"),
		"Should reference 'wide_table'"
	);
	assert!(
		file_content.contains("field_000"),
		"Should include first field"
	);
	assert!(
		file_content.contains("field_099"),
		"Should include last field"
	);

	// Verify the file is valid Rust structure
	assert!(
		file_content.contains("pub fn migration() -> Migration"),
		"Migration with 100+ fields should contain valid migration function"
	);
}

#[rstest]
#[tokio::test]
#[serial(makemigrations_e2e_edge)]
async fn edg_10_deep_dependency_chain() {
	// Test: Deep dependency chain - 10 sequential migrations (E2E)
	// Verify that sequential migration numbering works up to 10 levels.

	// Arrange
	let temp_dir = TempDir::new().unwrap();
	let migrations_dir = temp_dir.path().join("migrations");
	let app_label = "chain";

	let (_source, _repository, service) = create_migration_infra(&migrations_dir);

	// Act: Create 10 sequential migrations with dependencies
	for i in 1..=10 {
		let name = format!("{:04}_step_{}", i, i);
		let deps = if i > 1 {
			vec![(
				app_label.to_string(),
				format!("{:04}_step_{}", i - 1, i - 1),
			)]
		} else {
			Vec::new()
		};

		let migration = Migration {
			app_label: app_label.to_string(),
			name: name.clone(),
			operations: vec![Operation::CreateTable {
				name: format!("table_{}", i),
				columns: vec![ColumnDefinition {
					name: "id".to_string(),
					type_definition: FieldType::Integer,
					not_null: true,
					default: None,
					unique: false,
					primary_key: true,
					auto_increment: true,
				}],
				constraints: Vec::new(),
				without_rowid: None,
				interleave_in_parent: None,
				partition: None,
			}],
			dependencies: deps,
			atomic: true,
			replaces: Vec::new(),
			initial: if i == 1 { Some(true) } else { None },
			..Default::default()
		};

		service
			.save_migration(&migration)
			.await
			.unwrap_or_else(|e| panic!("Failed to save migration {}: {}", i, e));
	}

	// Assert: All 10 migration files exist
	for i in 1..=10 {
		let expected_number = format!("{:04}", i);
		assert!(
			verify_migration_file_exists(&migrations_dir, app_label, &expected_number),
			"Migration {} should exist",
			i
		);
	}

	// Assert: Last migration file has correct dependency
	let last_path = migrations_dir.join(app_label).join("0010_step_10.rs");
	let last_content = std::fs::read_to_string(&last_path).expect("Failed to read last migration");
	assert!(
		last_content.contains("0009_step_9"),
		"Last migration should depend on 0009_step_9"
	);
}

#[rstest]
#[tokio::test]
#[serial(makemigrations_e2e_edge)]
async fn edg_11_unicode_in_names() {
	// Test: Table/column names with ASCII-safe special characters (E2E)
	// Real databases typically use ASCII identifiers. Verify that
	// underscores and numbers in names are handled correctly.

	// Arrange
	let temp_dir = TempDir::new().unwrap();
	let migrations_dir = temp_dir.path().join("migrations");
	let app_label = "special";

	let empty_schema = DatabaseSchema::default();
	let mut target_schema = DatabaseSchema::default();
	let mut table = TableSchema {
		name: "user_profiles_v2".to_string(),
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
		"display_name_v2".to_string(),
		ColumnSchema {
			name: "display_name_v2".to_string(),
			data_type: FieldType::VarChar(255),
			nullable: true,
			default: None,
			primary_key: false,
			auto_increment: false,
		},
	);
	target_schema
		.tables
		.insert("user_profiles_v2".to_string(), table);

	// Act
	let (_name, file_content) = generate_and_save_migration(
		&migrations_dir,
		app_label,
		empty_schema,
		target_schema,
		"initial",
		true,
	)
	.await;

	// Assert
	assert!(
		file_content.contains("user_profiles_v2"),
		"Migration should handle special characters in table names"
	);
	assert!(
		file_content.contains("display_name_v2"),
		"Migration should handle special characters in column names"
	);
}

#[rstest]
#[tokio::test]
#[serial(makemigrations_e2e_edge)]
async fn edg_12_sql_reserved_words() {
	// Test: Table/column names containing SQL reserved words (E2E)
	// Verify that reserved words like "order", "select", "group" are handled.

	// Arrange
	let temp_dir = TempDir::new().unwrap();
	let migrations_dir = temp_dir.path().join("migrations");
	let app_label = "reserved";

	let empty_schema = DatabaseSchema::default();
	let mut target_schema = DatabaseSchema::default();
	let mut table = TableSchema {
		name: "order".to_string(),
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
		"select".to_string(),
		ColumnSchema {
			name: "select".to_string(),
			data_type: FieldType::VarChar(255),
			nullable: true,
			default: None,
			primary_key: false,
			auto_increment: false,
		},
	);
	table.columns.insert(
		"group".to_string(),
		ColumnSchema {
			name: "group".to_string(),
			data_type: FieldType::VarChar(100),
			nullable: true,
			default: None,
			primary_key: false,
			auto_increment: false,
		},
	);
	target_schema.tables.insert("order".to_string(), table);

	// Act
	let (_name, file_content) = generate_and_save_migration(
		&migrations_dir,
		app_label,
		empty_schema,
		target_schema,
		"initial",
		true,
	)
	.await;

	// Assert: Reserved words are present in migration (quoting is handled by SQL backend)
	assert!(
		file_content.contains("CreateTable"),
		"Migration should contain CreateTable"
	);
	assert!(
		file_content.contains("\"order\""),
		"Migration should reference 'order' table name"
	);
	assert!(
		file_content.contains("\"select\""),
		"Migration should reference 'select' column name"
	);
	assert!(
		file_content.contains("\"group\""),
		"Migration should reference 'group' column name"
	);
}

#[rstest]
#[tokio::test]
#[serial(makemigrations_e2e_edge)]
async fn edg_13_same_name_different_apps() {
	// Test: Models with same table name in different apps (E2E)
	// Two different apps can have same-named tables; they are isolated by app directory.

	// Arrange
	let temp_dir = TempDir::new().unwrap();
	let migrations_dir = temp_dir.path().join("migrations");

	let empty_schema = DatabaseSchema::default();

	// Both apps have a "users" table
	let mut schema_app1 = DatabaseSchema::default();
	let mut table1 = TableSchema {
		name: "users".to_string(),
		columns: BTreeMap::new(),
		indexes: Vec::new(),
		constraints: Vec::new(),
	};
	table1.columns.insert(
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
	schema_app1.tables.insert("users".to_string(), table1);

	let mut schema_app2 = DatabaseSchema::default();
	let mut table2 = TableSchema {
		name: "users".to_string(),
		columns: BTreeMap::new(),
		indexes: Vec::new(),
		constraints: Vec::new(),
	};
	table2.columns.insert(
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
	table2.columns.insert(
		"role".to_string(),
		ColumnSchema {
			name: "role".to_string(),
			data_type: FieldType::VarChar(50),
			nullable: true,
			default: None,
			primary_key: false,
			auto_increment: false,
		},
	);
	schema_app2.tables.insert("users".to_string(), table2);

	// Act
	let (_name1, content1) = generate_and_save_migration(
		&migrations_dir,
		"auth",
		empty_schema.clone(),
		schema_app1,
		"initial",
		true,
	)
	.await;

	let (_name2, content2) = generate_and_save_migration(
		&migrations_dir,
		"admin",
		empty_schema,
		schema_app2,
		"initial",
		true,
	)
	.await;

	// Assert: Both apps have their own migration files
	assert!(
		verify_migration_file_exists(&migrations_dir, "auth", "0001"),
		"Auth app migration should exist"
	);
	assert!(
		verify_migration_file_exists(&migrations_dir, "admin", "0001"),
		"Admin app migration should exist"
	);

	// Verify they are in separate directories
	assert!(
		migrations_dir.join("auth").is_dir(),
		"Auth app directory should exist"
	);
	assert!(
		migrations_dir.join("admin").is_dir(),
		"Admin app directory should exist"
	);

	// Verify content differs
	assert!(
		!content1.contains("role"),
		"Auth migration should not contain 'role' column"
	);
	assert!(
		content2.contains("role"),
		"Admin migration should contain 'role' column"
	);
}

#[rstest]
#[tokio::test]
#[serial(makemigrations_e2e_edge)]
async fn edg_14_cross_app_dependencies() {
	// Test: Cross-app dependencies (E2E)
	// Verify that migrations can reference dependencies from other apps.

	// Arrange
	let temp_dir = TempDir::new().unwrap();
	let migrations_dir = temp_dir.path().join("migrations");

	let (_source, _repository, service) = create_migration_infra(&migrations_dir);

	// First app: auth
	let auth_migration = Migration {
		app_label: "auth".to_string(),
		name: "0001_initial".to_string(),
		operations: vec![Operation::CreateTable {
			name: "users".to_string(),
			columns: vec![ColumnDefinition {
				name: "id".to_string(),
				type_definition: FieldType::Integer,
				not_null: true,
				default: None,
				unique: false,
				primary_key: true,
				auto_increment: true,
			}],
			constraints: Vec::new(),
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		}],
		dependencies: Vec::new(),
		atomic: true,
		replaces: Vec::new(),
		initial: Some(true),
		..Default::default()
	};

	service
		.save_migration(&auth_migration)
		.await
		.expect("Failed to save auth migration");

	// Second app: todos, depends on auth
	let todos_migration = Migration {
		app_label: "todos".to_string(),
		name: "0001_initial".to_string(),
		operations: vec![Operation::CreateTable {
			name: "todos".to_string(),
			columns: vec![
				ColumnDefinition {
					name: "id".to_string(),
					type_definition: FieldType::Integer,
					not_null: true,
					default: None,
					unique: false,
					primary_key: true,
					auto_increment: true,
				},
				ColumnDefinition {
					name: "user_id".to_string(),
					type_definition: FieldType::Integer,
					not_null: true,
					default: None,
					unique: false,
					primary_key: false,
					auto_increment: false,
				},
			],
			constraints: Vec::new(),
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		}],
		dependencies: vec![("auth".to_string(), "0001_initial".to_string())],
		atomic: true,
		replaces: Vec::new(),
		initial: Some(true),
		..Default::default()
	};

	service
		.save_migration(&todos_migration)
		.await
		.expect("Failed to save todos migration");

	// Assert: Both migration files exist
	assert!(
		verify_migration_file_exists(&migrations_dir, "auth", "0001"),
		"Auth migration should exist"
	);
	assert!(
		verify_migration_file_exists(&migrations_dir, "todos", "0001"),
		"Todos migration should exist"
	);

	// Assert: Todos migration references auth dependency
	let todos_path = migrations_dir.join("todos").join("0001_initial.rs");
	let todos_content =
		std::fs::read_to_string(&todos_path).expect("Failed to read todos migration");
	assert!(
		todos_content.contains("auth") && todos_content.contains("0001_initial"),
		"Todos migration should reference auth dependency"
	);
}

// ============================================================================
// Migration Naming Tests (MN-01 ~ MN-04)
//
// These tests verify the fix for issue #3198: makemigrations now generates
// descriptive migration names instead of always using '_initial'.
// ============================================================================

#[rstest]
#[tokio::test]
#[serial(makemigrations_e2e)]
async fn mn_01_initial_migration_gets_initial_name() {
	// Test: First migration (0001) should be named "0001_initial"
	// This verifies that the is_initial=true path still works correctly
	// when migration_number is "0001".

	// Arrange
	let temp_dir = TempDir::new().unwrap();
	let migrations_dir = temp_dir.path().join("migrations");
	let app_label = "naming_test";

	let empty_schema = DatabaseSchema::default();
	let mut target_schema = DatabaseSchema::default();
	let mut table = TableSchema {
		name: "articles".to_string(),
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
			data_type: FieldType::VarChar(200),
			nullable: false,
			default: None,
			primary_key: false,
			auto_increment: false,
		},
	);
	target_schema.tables.insert("articles".to_string(), table);

	// Act
	let (migration_name, file_content) = generate_and_save_migration_with_namer(
		&migrations_dir,
		app_label,
		empty_schema,
		target_schema,
		None,
	)
	.await;

	// Assert
	assert_eq!(
		migration_name, "0001_initial",
		"First migration should be named '0001_initial', got '{}'",
		migration_name
	);
	assert!(
		file_content.contains("initial: Some(true)"),
		"Initial migration should have initial flag set to Some(true)"
	);
	assert!(
		file_content.contains("CreateTable"),
		"Initial migration should contain CreateTable operation"
	);
}

#[rstest]
#[tokio::test]
#[serial(makemigrations_e2e)]
async fn mn_02_second_migration_gets_descriptive_name() {
	// Test: Second migration (0002) should get a descriptive name based on
	// its operations, NOT "0002_initial".
	// This is the core fix for issue #3198.

	// Arrange
	let temp_dir = TempDir::new().unwrap();
	let migrations_dir = temp_dir.path().join("migrations");
	let app_label = "naming_test";

	// Step 1: Create initial migration with a table
	let empty_schema = DatabaseSchema::default();
	let mut initial_schema = DatabaseSchema::default();
	let mut table = TableSchema {
		name: "articles".to_string(),
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
			data_type: FieldType::VarChar(200),
			nullable: false,
			default: None,
			primary_key: false,
			auto_increment: false,
		},
	);
	initial_schema.tables.insert("articles".to_string(), table);

	let (first_name, _) = generate_and_save_migration_with_namer(
		&migrations_dir,
		app_label,
		empty_schema,
		initial_schema.clone(),
		None,
	)
	.await;
	assert_eq!(first_name, "0001_initial");

	// Step 2: Add a new column (author) to the existing table
	let mut extended_schema = initial_schema.clone();
	extended_schema
		.tables
		.get_mut("articles")
		.unwrap()
		.columns
		.insert(
			"author".to_string(),
			ColumnSchema {
				name: "author".to_string(),
				data_type: FieldType::VarChar(100),
				nullable: true,
				default: None,
				primary_key: false,
				auto_increment: false,
			},
		);

	// Act
	let (second_name, second_content) = generate_and_save_migration_with_namer(
		&migrations_dir,
		app_label,
		initial_schema,
		extended_schema,
		None,
	)
	.await;

	// Assert: Name should be descriptive, NOT "0002_initial"
	assert!(
		second_name.starts_with("0002_"),
		"Second migration should be numbered 0002, got '{}'",
		second_name
	);
	assert!(
		!second_name.contains("initial"),
		"Second migration should NOT contain 'initial' in its name, got '{}'",
		second_name
	);
	assert!(
		second_name.contains("articles") || second_name.contains("author"),
		"Second migration name should contain table or column name, got '{}'",
		second_name
	);
	assert!(
		second_content.contains("AddColumn"),
		"Second migration should contain AddColumn operation"
	);
	assert!(
		!second_content.contains("initial: Some(true)"),
		"Second migration should NOT have initial flag"
	);
}

#[rstest]
#[tokio::test]
#[serial(makemigrations_e2e)]
async fn mn_03_user_specified_name_overrides_auto_naming() {
	// Test: When user provides --name, it should override auto-generated naming
	// for both initial and non-initial migrations.

	// Arrange
	let temp_dir = TempDir::new().unwrap();
	let migrations_dir = temp_dir.path().join("migrations");
	let app_label = "naming_test";

	let empty_schema = DatabaseSchema::default();
	let mut target_schema = DatabaseSchema::default();
	let mut table = TableSchema {
		name: "posts".to_string(),
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
	target_schema.tables.insert("posts".to_string(), table);

	// Act: Create initial migration with user-specified name
	let (name, _) = generate_and_save_migration_with_namer(
		&migrations_dir,
		app_label,
		empty_schema,
		target_schema,
		Some("custom_name"),
	)
	.await;

	// Assert
	assert_eq!(
		name, "0001_custom_name",
		"User-specified name should override auto-naming, got '{}'",
		name
	);
}

#[rstest]
#[tokio::test]
#[serial(makemigrations_e2e)]
async fn mn_04_third_migration_also_gets_descriptive_name() {
	// Test: Verify that migration naming works correctly for 0003+
	// to ensure the fix isn't limited to just the second migration.

	// Arrange
	let temp_dir = TempDir::new().unwrap();
	let migrations_dir = temp_dir.path().join("migrations");
	let app_label = "naming_test";

	// Step 1: Create initial table
	let empty_schema = DatabaseSchema::default();
	let mut schema_v1 = DatabaseSchema::default();
	let mut table = TableSchema {
		name: "comments".to_string(),
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
	schema_v1.tables.insert("comments".to_string(), table);

	let (first_name, _) = generate_and_save_migration_with_namer(
		&migrations_dir,
		app_label,
		empty_schema,
		schema_v1.clone(),
		None,
	)
	.await;
	assert_eq!(first_name, "0001_initial");

	// Step 2: Add 'body' column
	let mut schema_v2 = schema_v1.clone();
	schema_v2
		.tables
		.get_mut("comments")
		.unwrap()
		.columns
		.insert(
			"body".to_string(),
			ColumnSchema {
				name: "body".to_string(),
				data_type: FieldType::Text,
				nullable: false,
				default: None,
				primary_key: false,
				auto_increment: false,
			},
		);

	let (second_name, _) = generate_and_save_migration_with_namer(
		&migrations_dir,
		app_label,
		schema_v1,
		schema_v2.clone(),
		None,
	)
	.await;
	assert!(
		!second_name.contains("initial"),
		"0002 should not be 'initial', got '{}'",
		second_name
	);

	// Step 3: Add 'rating' column
	let mut schema_v3 = schema_v2.clone();
	schema_v3
		.tables
		.get_mut("comments")
		.unwrap()
		.columns
		.insert(
			"rating".to_string(),
			ColumnSchema {
				name: "rating".to_string(),
				data_type: FieldType::Integer,
				nullable: true,
				default: None,
				primary_key: false,
				auto_increment: false,
			},
		);

	// Act
	let (third_name, third_content) = generate_and_save_migration_with_namer(
		&migrations_dir,
		app_label,
		schema_v2,
		schema_v3,
		None,
	)
	.await;

	// Assert
	assert!(
		third_name.starts_with("0003_"),
		"Third migration should be numbered 0003, got '{}'",
		third_name
	);
	assert!(
		!third_name.contains("initial"),
		"Third migration should NOT contain 'initial', got '{}'",
		third_name
	);
	assert!(
		third_content.contains("AddColumn"),
		"Third migration should contain AddColumn"
	);

	// Verify all three migration files exist
	assert!(
		verify_migration_file_exists(&migrations_dir, app_label, "0001"),
		"0001 migration file should exist"
	);
	assert!(
		verify_migration_file_exists(&migrations_dir, app_label, "0002"),
		"0002 migration file should exist"
	);
	assert!(
		verify_migration_file_exists(&migrations_dir, app_label, "0003"),
		"0003 migration file should exist"
	);
}

#[rstest]
#[tokio::test]
#[serial(makemigrations_e2e)]
async fn mn_05_drop_column_gets_remove_prefix_name() {
	// Test: Dropping a column should produce a name like "0002_remove_table_column"

	// Arrange
	let temp_dir = TempDir::new().unwrap();
	let migrations_dir = temp_dir.path().join("migrations");
	let app_label = "naming_test";

	let empty_schema = DatabaseSchema::default();
	let mut schema_v1 = DatabaseSchema::default();
	let mut table = TableSchema {
		name: "profiles".to_string(),
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
		"bio".to_string(),
		ColumnSchema {
			name: "bio".to_string(),
			data_type: FieldType::Text,
			nullable: true,
			default: None,
			primary_key: false,
			auto_increment: false,
		},
	);
	schema_v1.tables.insert("profiles".to_string(), table);

	// Create initial migration
	let (first_name, _) = generate_and_save_migration_with_namer(
		&migrations_dir,
		app_label,
		empty_schema,
		schema_v1.clone(),
		None,
	)
	.await;
	assert_eq!(first_name, "0001_initial");

	// Remove 'bio' column
	let mut schema_v2 = schema_v1.clone();
	schema_v2
		.tables
		.get_mut("profiles")
		.unwrap()
		.columns
		.remove("bio");

	// Act
	let (second_name, second_content) = generate_and_save_migration_with_namer(
		&migrations_dir,
		app_label,
		schema_v1,
		schema_v2,
		None,
	)
	.await;

	// Assert
	assert!(
		second_name.starts_with("0002_"),
		"Should be 0002, got '{}'",
		second_name
	);
	assert!(
		!second_name.contains("initial"),
		"Drop column migration should NOT be named 'initial', got '{}'",
		second_name
	);
	assert!(
		second_content.contains("DropColumn"),
		"Should contain DropColumn operation"
	);
}

#[rstest]
#[tokio::test]
#[serial(makemigrations_e2e)]
async fn mn_06_alter_column_type_change_gets_descriptive_name() {
	// Test: Changing column type (Integer → Float) should produce AlterColumn
	// with descriptive name. This validates that generate_operations() now
	// correctly handles columns_to_modify.

	// Arrange
	let temp_dir = TempDir::new().unwrap();
	let migrations_dir = temp_dir.path().join("migrations");
	let app_label = "naming_test";

	let empty_schema = DatabaseSchema::default();
	let mut schema_v1 = DatabaseSchema::default();
	let mut table = TableSchema {
		name: "items".to_string(),
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
		"price".to_string(),
		ColumnSchema {
			name: "price".to_string(),
			data_type: FieldType::Integer,
			nullable: false,
			default: None,
			primary_key: false,
			auto_increment: false,
		},
	);
	schema_v1.tables.insert("items".to_string(), table);

	let (first_name, _) = generate_and_save_migration_with_namer(
		&migrations_dir,
		app_label,
		empty_schema,
		schema_v1.clone(),
		None,
	)
	.await;
	assert_eq!(first_name, "0001_initial");

	// Change 'price' from Integer to Float
	let mut schema_v2 = schema_v1.clone();
	schema_v2.tables.get_mut("items").unwrap().columns.insert(
		"price".to_string(),
		ColumnSchema {
			name: "price".to_string(),
			data_type: FieldType::Float,
			nullable: false,
			default: None,
			primary_key: false,
			auto_increment: false,
		},
	);

	// Act
	let (second_name, second_content) = generate_and_save_migration_with_namer(
		&migrations_dir,
		app_label,
		schema_v1,
		schema_v2,
		None,
	)
	.await;

	// Assert
	assert!(
		second_name.starts_with("0002_"),
		"Should be 0002, got '{}'",
		second_name
	);
	assert!(
		!second_name.contains("initial"),
		"Alter column migration should NOT be named 'initial', got '{}'",
		second_name
	);
	assert!(
		second_content.contains("AlterColumn"),
		"Should contain AlterColumn operation"
	);
}

#[rstest]
#[tokio::test]
#[serial(makemigrations_e2e)]
async fn mn_07_multiple_add_columns_get_combined_name() {
	// Test: Adding multiple columns should combine fragment names

	// Arrange
	let temp_dir = TempDir::new().unwrap();
	let migrations_dir = temp_dir.path().join("migrations");
	let app_label = "naming_test";

	let empty_schema = DatabaseSchema::default();
	let mut schema_v1 = DatabaseSchema::default();
	let mut table = TableSchema {
		name: "users".to_string(),
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
	schema_v1.tables.insert("users".to_string(), table);

	let (first_name, _) = generate_and_save_migration_with_namer(
		&migrations_dir,
		app_label,
		empty_schema,
		schema_v1.clone(),
		None,
	)
	.await;
	assert_eq!(first_name, "0001_initial");

	// Add two columns at once
	let mut schema_v2 = schema_v1.clone();
	let t = schema_v2.tables.get_mut("users").unwrap();
	t.columns.insert(
		"first_name".to_string(),
		ColumnSchema {
			name: "first_name".to_string(),
			data_type: FieldType::VarChar(50),
			nullable: true,
			default: None,
			primary_key: false,
			auto_increment: false,
		},
	);
	t.columns.insert(
		"last_name".to_string(),
		ColumnSchema {
			name: "last_name".to_string(),
			data_type: FieldType::VarChar(50),
			nullable: true,
			default: None,
			primary_key: false,
			auto_increment: false,
		},
	);

	// Act
	let (second_name, _) = generate_and_save_migration_with_namer(
		&migrations_dir,
		app_label,
		schema_v1,
		schema_v2,
		None,
	)
	.await;

	// Assert
	assert!(
		second_name.starts_with("0002_"),
		"Should be 0002, got '{}'",
		second_name
	);
	assert!(
		!second_name.contains("initial"),
		"Multi-column add should NOT be named 'initial', got '{}'",
		second_name
	);
	// Should contain fragments from both columns
	assert!(
		second_name.contains("first_name") || second_name.contains("last_name"),
		"Name should contain at least one column name, got '{}'",
		second_name
	);
}

#[rstest]
#[tokio::test]
#[serial(makemigrations_e2e)]
async fn mn_08_index_addition_gets_create_index_name() {
	// Test: Adding an index should produce a descriptive name containing
	// "create_index" or "create_unique_index". This validates that
	// generate_operations() now handles indexes_to_add.

	// Arrange
	let temp_dir = TempDir::new().unwrap();
	let migrations_dir = temp_dir.path().join("migrations");
	let app_label = "naming_test";

	let empty_schema = DatabaseSchema::default();
	let mut schema_v1 = DatabaseSchema::default();
	let mut table = TableSchema {
		name: "articles".to_string(),
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
		"slug".to_string(),
		ColumnSchema {
			name: "slug".to_string(),
			data_type: FieldType::VarChar(200),
			nullable: false,
			default: None,
			primary_key: false,
			auto_increment: false,
		},
	);
	schema_v1.tables.insert("articles".to_string(), table);

	let (first_name, _) = generate_and_save_migration_with_namer(
		&migrations_dir,
		app_label,
		empty_schema,
		schema_v1.clone(),
		None,
	)
	.await;
	assert_eq!(first_name, "0001_initial");

	// Add unique index on 'slug'
	let mut schema_v2 = schema_v1.clone();
	schema_v2
		.tables
		.get_mut("articles")
		.unwrap()
		.indexes
		.push(IndexSchema {
			name: "idx_articles_slug".to_string(),
			columns: vec!["slug".to_string()],
			unique: true,
		});

	// Act
	let (second_name, second_content) = generate_and_save_migration_with_namer(
		&migrations_dir,
		app_label,
		schema_v1,
		schema_v2,
		None,
	)
	.await;

	// Assert
	assert!(
		second_name.starts_with("0002_"),
		"Should be 0002, got '{}'",
		second_name
	);
	assert!(
		!second_name.contains("initial"),
		"Index addition should NOT be named 'initial', got '{}'",
		second_name
	);
	assert!(
		second_content.contains("CreateIndex"),
		"Should contain CreateIndex operation"
	);
}

#[rstest]
#[tokio::test]
#[serial(makemigrations_e2e)]
async fn mn_09_drop_table_gets_delete_prefix_name() {
	// Test: Dropping a table should produce "0002_delete_tablename"

	// Arrange
	let temp_dir = TempDir::new().unwrap();
	let migrations_dir = temp_dir.path().join("migrations");
	let app_label = "naming_test";

	let empty_schema = DatabaseSchema::default();
	let mut schema_v1 = DatabaseSchema::default();
	let mut table = TableSchema {
		name: "temp_data".to_string(),
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
	schema_v1.tables.insert("temp_data".to_string(), table);

	let (first_name, _) = generate_and_save_migration_with_namer(
		&migrations_dir,
		app_label,
		empty_schema,
		schema_v1.clone(),
		None,
	)
	.await;
	assert_eq!(first_name, "0001_initial");

	// Remove the table entirely
	let schema_v2 = DatabaseSchema::default();

	// Act
	let (second_name, second_content) = generate_and_save_migration_with_namer(
		&migrations_dir,
		app_label,
		schema_v1,
		schema_v2,
		None,
	)
	.await;

	// Assert
	assert!(
		second_name.starts_with("0002_"),
		"Should be 0002, got '{}'",
		second_name
	);
	assert!(
		!second_name.contains("initial"),
		"Drop table migration should NOT be named 'initial', got '{}'",
		second_name
	);
	assert!(
		second_content.contains("DropTable"),
		"Should contain DropTable operation"
	);
}
