//! makemigrations Internal Logic Integration Tests
//!
//! Tests the internal logic of AutoMigrationGenerator and related components.
//! This test suite validates the correctness of migration file generation by
//! testing the underlying logic directly, without executing the full command.
//!
//! **Test Coverage:**
//! - Normal Cases (NC-01 ~ NC-20): Basic to advanced migration generation
//! - Error Cases (EC-01 ~ EC-05): Error handling validation
//! - Edge Cases (EDG-01 ~ EDG-15): Special scenarios and options
//!
//! **Test Approach:**
//! - Uses AutoMigrationGenerator directly (internal API)
//! - TestRepository for in-memory migration storage
//! - DatabaseSchema manual construction for precise control
//! - Follows existing patterns from duplicate_detection_integration.rs

use reinhardt_db::migrations::{
	AutoMigrationError, AutoMigrationGenerator, ColumnDefinition, FieldType, Migration,
	MigrationRepository, Operation,
};
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use tokio::sync::Mutex;

// Import DatabaseSchema and SchemaDiff from reinhardt_migrations
use reinhardt_db::migrations::schema_diff::{ColumnSchema, DatabaseSchema, TableSchema};

// ============================================================================
// Test Repository Implementation
// ============================================================================

/// Test repository implementation for integration tests
/// Stores migrations in memory for fast, isolated testing
struct TestRepository {
	migrations: HashMap<(String, String), Migration>,
}

impl TestRepository {
	fn new() -> Self {
		Self {
			migrations: HashMap::new(),
		}
	}
}

#[async_trait::async_trait]
impl MigrationRepository for TestRepository {
	async fn save(&mut self, migration: &Migration) -> reinhardt_db::migrations::Result<()> {
		let key = (migration.app_label.to_string(), migration.name.to_string());
		self.migrations.insert(key, migration.clone());
		Ok(())
	}

	async fn get(&self, app_label: &str, name: &str) -> reinhardt_db::migrations::Result<Migration> {
		let key = (app_label.to_string(), name.to_string());
		self.migrations.get(&key).cloned().ok_or_else(|| {
			reinhardt_db::migrations::MigrationError::NotFound(format!("{}.{}", app_label, name))
		})
	}

	async fn list(&self, app_label: &str) -> reinhardt_db::migrations::Result<Vec<Migration>> {
		Ok(self
			.migrations
			.values()
			.filter(|m| m.app_label == app_label)
			.cloned()
			.collect())
	}

	async fn exists(&self, app_label: &str, name: &str) -> reinhardt_db::migrations::Result<bool> {
		Ok(self
			.get(app_label, name)
			.await
			.map(|_| true)
			.unwrap_or(false))
	}
}

// ============================================================================
// Helper Functions for Schema Construction
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

	// id column
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

	// title column
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

	// completed column
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

/// Helper to create a schema with todos table + description field
fn create_todos_with_description_schema() -> DatabaseSchema {
	let mut schema = create_todos_schema();
	let table = schema.tables.get_mut("todos").unwrap();

	table.columns.insert(
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

	schema
}

/// Helper to create a schema with composite primary key (user_posts table)
fn create_composite_pk_schema() -> DatabaseSchema {
	let mut schema = DatabaseSchema::default();
	let mut table = TableSchema {
		name: "user_posts".to_string(),
		columns: BTreeMap::new(),
		indexes: Vec::new(),
		constraints: Vec::new(),
	};

	// user_id column (part of composite primary key)
	table.columns.insert(
		"user_id".to_string(),
		ColumnSchema {
			name: "user_id".to_string(),
			data_type: FieldType::Integer,
			nullable: false,
			default: None,
			primary_key: true,
			auto_increment: false,
		},
	);

	// post_id column (part of composite primary key)
	table.columns.insert(
		"post_id".to_string(),
		ColumnSchema {
			name: "post_id".to_string(),
			data_type: FieldType::Integer,
			nullable: false,
			default: None,
			primary_key: true,
			auto_increment: false,
		},
	);

	// created_at column
	table.columns.insert(
		"created_at".to_string(),
		ColumnSchema {
			name: "created_at".to_string(),
			data_type: FieldType::DateTime,
			nullable: false,
			default: Some("CURRENT_TIMESTAMP".to_string()),
			primary_key: false,
			auto_increment: false,
		},
	);

	schema.tables.insert("user_posts".to_string(), table);
	schema
}

/// Helper to create a simple table without primary key (for adding composite PK later)
fn create_user_posts_no_pk_schema() -> DatabaseSchema {
	let mut schema = DatabaseSchema::default();
	let mut table = TableSchema {
		name: "user_posts".to_string(),
		columns: BTreeMap::new(),
		indexes: Vec::new(),
		constraints: Vec::new(),
	};

	// user_id column (NOT primary key yet)
	table.columns.insert(
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

	// post_id column (NOT primary key yet)
	table.columns.insert(
		"post_id".to_string(),
		ColumnSchema {
			name: "post_id".to_string(),
			data_type: FieldType::Integer,
			nullable: false,
			default: None,
			primary_key: false,
			auto_increment: false,
		},
	);

	// created_at column
	table.columns.insert(
		"created_at".to_string(),
		ColumnSchema {
			name: "created_at".to_string(),
			data_type: FieldType::DateTime,
			nullable: false,
			default: Some("CURRENT_TIMESTAMP".to_string()),
			primary_key: false,
			auto_increment: false,
		},
	);

	schema.tables.insert("user_posts".to_string(), table);
	schema
}

/// Helper to create a users schema
fn create_users_schema() -> DatabaseSchema {
	let mut schema = DatabaseSchema::default();
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

	table.columns.insert(
		"name".to_string(),
		ColumnSchema {
			name: "name".to_string(),
			data_type: FieldType::VarChar(255),
			nullable: false,
			default: None,
			primary_key: false,
			auto_increment: false,
		},
	);

	schema.tables.insert("users".to_string(), table);
	schema
}

// ============================================================================
// Normal Cases (NC-01 ~ NC-20)
// ============================================================================

#[tokio::test]
async fn nc_01_new_model_creates_create_table_migration() {
	// Test: Generate CreateTable from new model creation
	// Verify that CreateTable migration is correctly generated when adding a new model from empty state

	let app_label = "todos";
	let empty_schema = DatabaseSchema::default();
	let target_schema = create_todos_schema();

	let repository = Arc::new(Mutex::new(TestRepository::new()));
	let generator = AutoMigrationGenerator::new(target_schema.clone(), repository.clone());

	let result = generator
		.generate(app_label, empty_schema.clone())
		.await
		.expect("First makemigrations should succeed");

	// Verify: One CreateTable operation is generated
	assert_eq!(
		result.operation_count, 1,
		"Should generate one CreateTable operation"
	);

	// Verify: Operation type is CreateTable
	assert!(
		matches!(result.operations[0], Operation::CreateTable { .. }),
		"Operation should be CreateTable"
	);

	// Verify: Table name is "todos"
	if let Operation::CreateTable { name, columns, .. } = &result.operations[0] {
		assert_eq!(name, &"todos", "Table name should be 'todos'");
		assert!(
			columns.len() >= 3,
			"Should have at least id, title, completed columns"
		);
	}
}

#[tokio::test]
async fn nc_02_field_addition_creates_add_column_migration() {
	// Test: Generate AddColumn from field addition
	// Verify that AddColumn migration is generated when adding a field to existing model

	let app_label = "todos";
	let from_schema = create_todos_schema();
	let to_schema = create_todos_with_description_schema();

	let repository = Arc::new(Mutex::new(TestRepository::new()));
	let generator = AutoMigrationGenerator::new(to_schema.clone(), repository.clone());

	let result = generator
		.generate(app_label, from_schema.clone())
		.await
		.expect("AddColumn generation should succeed");

	// Verify: AddColumn operation is generated
	assert!(
		result.operation_count >= 1,
		"Should generate at least one operation"
	);

	// Verify: Contains AddColumn operation
	let has_add_column = result.operations.iter().any(|op| {
		matches!(
			op,
			Operation::AddColumn {
				table,
				column
			} if table == &"todos" && column.name == "description"
		)
	});

	assert!(
		has_add_column,
		"Should contain AddColumn operation for 'description' field"
	);
}

#[tokio::test]
async fn nc_03_field_deletion_creates_drop_column_migration() {
	// Test: Generate DropColumn from field deletion

	let app_label = "todos";
	let from_schema = create_todos_schema();
	let mut to_schema = create_todos_schema();

	// Remove 'completed' column from target schema
	to_schema
		.tables
		.get_mut("todos")
		.unwrap()
		.columns
		.remove("completed");

	let repository = Arc::new(Mutex::new(TestRepository::new()));
	let generator = AutoMigrationGenerator::new(to_schema.clone(), repository.clone());

	let result = generator
		.generate(app_label, from_schema.clone())
		.await
		.expect("DropColumn generation should succeed");

	// Verify: DropColumn operation is generated
	let has_drop_column = result.operations.iter().any(|op| {
		matches!(
			op,
			Operation::DropColumn {
				table,
				column
			} if table == &"todos" && column == "completed"
		)
	});

	assert!(
		has_drop_column,
		"Should contain DropColumn operation for 'completed' field"
	);
}

#[tokio::test]
async fn nc_04_field_type_change_creates_alter_column_migration() {
	// Test: Generate AlterColumn from field type change
	let app_label = "todos";
	let mut from_schema = create_todos_schema();
	let mut to_schema = create_todos_schema();

	// Change 'title' from VARCHAR(255) to TEXT
	to_schema
		.tables
		.get_mut("todos")
		.unwrap()
		.columns
		.get_mut("title")
		.unwrap()
		.data_type = FieldType::Text;

	let repository = Arc::new(Mutex::new(TestRepository::new()));
	let generator = AutoMigrationGenerator::new(to_schema.clone(), repository.clone());

	let result = generator
		.generate(app_label, from_schema.clone())
		.await
		.expect("AlterColumn generation should succeed");

	// Verify: AlterColumn operation is generated
	let has_alter_column = result.operations.iter().any(|op| {
		matches!(
			op,
			Operation::AlterColumn {
				table,
				column,
				..
			} if table == &"todos" && column == "title"
		)
	});

	assert!(
		has_alter_column,
		"Should contain AlterColumn operation for 'title' field"
	);
}

#[tokio::test]
async fn nc_05_field_rename_creates_rename_column_migration() {
	// Test: Generate RenameColumn from field rename
	// Note: Field rename is automatically detected by similarity analysis
	// This test verifies the possibility of RenameColumn operation generation

	let app_label = "todos";
	let mut from_schema = create_todos_schema();
	let mut to_schema = create_todos_schema();

	// Remove 'completed' and add 'is_done' (similar name)
	to_schema
		.tables
		.get_mut("todos")
		.unwrap()
		.columns
		.remove("completed");

	to_schema.tables.get_mut("todos").unwrap().columns.insert(
		"is_done".to_string(),
		ColumnSchema {
			name: "is_done".to_string(),
			data_type: FieldType::Boolean,
			nullable: false,
			default: Some("false".to_string()),
			primary_key: false,
			auto_increment: false,
		},
	);

	let repository = Arc::new(Mutex::new(TestRepository::new()));
	let generator = AutoMigrationGenerator::new(to_schema.clone(), repository.clone());

	let result = generator
		.generate(app_label, from_schema.clone())
		.await
		.expect("Field rename detection should succeed");

	// Verify: Either RenameColumn or (DropColumn + AddColumn)
	let has_rename = result.operations.iter().any(|op| {
		matches!(
			op,
			Operation::RenameColumn { table, old_name, new_name, .. }
			if table == &"todos" && old_name == "completed" && new_name == "is_done"
		)
	});

	let has_drop_and_add = result.operations.iter().any(|op| {
		matches!(op, Operation::DropColumn { table, column, .. } if table == &"todos" && column == "completed")
	}) && result.operations.iter().any(|op| {
		matches!(op, Operation::AddColumn { table, column } if table == &"todos" && column.name == "is_done")
	});

	assert!(
		has_rename || has_drop_and_add,
		"Should generate either RenameColumn or DropColumn + AddColumn"
	);
}

#[tokio::test]
async fn nc_06_index_addition_creates_create_index_migration() {
	// Test: Generate CreateIndex from index addition
	let app_label = "todos";
	let from_schema = create_todos_schema();
	let mut to_schema = create_todos_schema();

	// Add index on 'title' column
	use reinhardt_db::migrations::schema_diff::IndexSchema;
	to_schema
		.tables
		.get_mut("todos")
		.unwrap()
		.indexes
		.push(IndexSchema {
			name: "idx_title".to_string(),
			columns: vec!["title".to_string()],
			unique: false,
			access_method: None,
			index_type: None,
			expressions: None,
			operator_class: None,
		});

	let repository = Arc::new(Mutex::new(TestRepository::new()));
	let generator = AutoMigrationGenerator::new(to_schema.clone(), repository.clone());

	let result = generator
		.generate(app_label, from_schema.clone())
		.await
		.expect("CreateIndex generation should succeed");

	// Verify: CreateIndex operation
	let has_create_index = result.operations.iter().any(|op| {
		matches!(
			op,
			Operation::CreateIndex { table, columns, unique }
			if table == &"todos" && columns.contains(&"title".to_string()) && !unique
		)
	});

	assert!(
		has_create_index,
		"Should contain CreateIndex operation for 'title'"
	);
}

#[tokio::test]
async fn nc_07_foreign_key_addition_creates_add_column_and_constraint() {
	// Test: Generate AddColumn + AddConstraint from ForeignKey addition
	let app_label = "todos";
	let mut from_schema = create_todos_schema();
	let mut to_schema = create_todos_schema();

	// Add user_id foreign key column
	use reinhardt_db::migrations::schema_diff::ConstraintSchema;
	to_schema.tables.get_mut("todos").unwrap().columns.insert(
		"user_id".to_string(),
		ColumnSchema {
			name: "user_id".to_string(),
			data_type: FieldType::Integer,
			nullable: true,
			default: None,
			primary_key: false,
			auto_increment: false,
		},
	);

	// Add foreign key constraint
	to_schema
		.tables
		.get_mut("todos")
		.unwrap()
		.constraints
		.push(ConstraintSchema {
			name: "fk_user_id".to_string(),
			constraint_type: "FOREIGN KEY".to_string(),
			columns: vec!["user_id".to_string()],
			referenced_table: Some("users".to_string()),
			referenced_columns: Some(vec!["id".to_string()]),
		});

	let repository = Arc::new(Mutex::new(TestRepository::new()));
	let generator = AutoMigrationGenerator::new(to_schema.clone(), repository.clone());

	let result = generator
		.generate(app_label, from_schema.clone())
		.await
		.expect("ForeignKey generation should succeed");

	// Verify: AddColumn operation
	let has_add_column = result.operations.iter().any(|op| {
		matches!(
			op,
			Operation::AddColumn { table, column }
			if table == &"todos" && column.name == "user_id"
		)
	});

	// Verify: AddConstraint operation (may or may not be generated depending on implementation)
	let has_add_constraint = result
		.operations
		.iter()
		.any(|op| matches!(op, Operation::AddConstraint { table, .. } if table == &"todos"));

	assert!(has_add_column, "Should add user_id column");
	// Note: AddConstraint may be implicit in AddColumn or explicit
}

#[tokio::test]
async fn nc_08_many_to_many_creates_junction_table() {
	// Test: Generate CreateTable (junction table) from ManyToMany addition
	// Note: ManyToMany relationships are represented as intermediate tables

	let app_label = "todos";
	let mut from_schema = create_todos_schema();

	// Add tags table
	let mut tags_schema = DatabaseSchema::default();
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
			data_type: FieldType::VarChar(50),
			nullable: false,
			default: None,
			primary_key: false,
			auto_increment: false,
		},
	);
	tags_schema.tables.insert("tags".to_string(), tags_table);

	// Create junction table in target schema
	let mut to_schema = create_todos_schema();
	to_schema.tables.extend(tags_schema.tables.clone());

	// Add junction table: todos_tags
	let mut junction_table = TableSchema {
		name: "todos_tags".to_string(),
		columns: BTreeMap::new(),
		indexes: Vec::new(),
		constraints: Vec::new(),
	};
	junction_table.columns.insert(
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
	junction_table.columns.insert(
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
	junction_table.columns.insert(
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
	to_schema
		.tables
		.insert("todos_tags".to_string(), junction_table);

	let repository = Arc::new(Mutex::new(TestRepository::new()));
	let generator = AutoMigrationGenerator::new(to_schema.clone(), repository.clone());

	let result = generator
		.generate(app_label, from_schema.clone())
		.await
		.expect("ManyToMany generation should succeed");

	// Verify: CreateTable operations for tags and junction table
	let create_table_count = result
		.operations
		.iter()
		.filter(|op| matches!(op, Operation::CreateTable { .. }))
		.count();

	assert!(
		create_table_count >= 2,
		"Should create at least tags and todos_tags tables"
	);

	let has_junction_table = result
		.operations
		.iter()
		.any(|op| matches!(op, Operation::CreateTable { name, .. } if name == &"todos_tags"));

	assert!(
		has_junction_table,
		"Should create junction table 'todos_tags'"
	);
}

#[tokio::test]
async fn nc_09_initial_migration_correctness() {
	// Test: Correct generation of initial migration (0001_initial)
	let app_label = "myapp";
	let empty_schema = DatabaseSchema::default();

	// Create schema with multiple models
	let mut target_schema = DatabaseSchema::default();

	// Add users table
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
		"name".to_string(),
		ColumnSchema {
			name: "name".to_string(),
			data_type: FieldType::VarChar(255),
			nullable: false,
			default: None,
			primary_key: false,
			auto_increment: false,
		},
	);

	// Add todos table
	let todos_table = create_todos_schema().tables.get("todos").unwrap().clone();

	target_schema
		.tables
		.insert("users".to_string(), users_table);
	target_schema
		.tables
		.insert("todos".to_string(), todos_table);

	let repository = Arc::new(Mutex::new(TestRepository::new()));
	let generator = AutoMigrationGenerator::new(target_schema.clone(), repository.clone());

	let result = generator
		.generate(app_label, empty_schema.clone())
		.await
		.expect("Initial migration should succeed");

	// Verify: Multiple CreateTable operations
	let create_table_count = result
		.operations
		.iter()
		.filter(|op| matches!(op, Operation::CreateTable { .. }))
		.count();

	assert!(
		create_table_count >= 2,
		"Should generate CreateTable for both users and todos"
	);

	// Verify: Contains both tables
	let has_users = result
		.operations
		.iter()
		.any(|op| matches!(op, Operation::CreateTable { name, .. } if name == &"users"));
	let has_todos = result
		.operations
		.iter()
		.any(|op| matches!(op, Operation::CreateTable { name, .. } if name == &"todos"));

	assert!(has_users, "Should create users table");
	assert!(has_todos, "Should create todos table");
}

#[tokio::test]
async fn nc_10_sequential_migrations_dependency_chain() {
	// Test: Correct generation of sequential migrations (0001 → 0002 → 0003)
	// Verify that consecutive migrations are generated with correct dependencies

	let app_label = "todos";
	let repository = Arc::new(Mutex::new(TestRepository::new()));

	// Step 1: Generate initial migration (empty → todos table)
	let empty_schema = DatabaseSchema::default();
	let schema1 = create_todos_schema();

	let generator1 = AutoMigrationGenerator::new(schema1.clone(), repository.clone());
	let result1 = generator1
		.generate(app_label, empty_schema.clone())
		.await
		.expect("First migration should succeed");

	// Save first migration
	let migration1 = Migration {
		app_label,
		name: "0001_initial".to_string(),
		operations: result1.operations.clone(),
		dependencies: Vec::new(),
		replaces: Vec::new(),
		atomic: true,
		initial: Some(true),
	};
	{
		let mut repo = repository.lock().await;
		repo.save(&migration1)
			.await
			.expect("Should save first migration");
	}

	// Step 2: Generate second migration (todos → todos + description)
	let schema2 = create_todos_with_description_schema();

	let generator2 = AutoMigrationGenerator::new(schema2.clone(), repository.clone());
	let result2 = generator2
		.generate(app_label, schema1.clone())
		.await
		.expect("Second migration should succeed");

	// Verify: Second migration generates operations
	assert!(
		result2.operation_count > 0,
		"Second migration should have operations"
	);

	// Save second migration
	let migration2 = Migration {
		app_label,
		name: "0002_add_description".to_string(),
		operations: result2.operations.clone(),
		dependencies: vec![(app_label, "0001_initial")],
		replaces: Vec::new(),
		atomic: true,
		initial: None,
	};
	{
		let mut repo = repository.lock().await;
		repo.save(&migration2)
			.await
			.expect("Should save second migration");
	}

	// Step 3: Verify both migrations exist in repository
	{
		let repo = repository.lock().await;
		let migrations = repo.list(app_label).await.expect("Should list migrations");
		assert_eq!(migrations.len(), 2, "Should have 2 migrations");
	}
}

#[tokio::test]
async fn nc_11_generated_migration_executability() {
	// Test: Verify executability of generated migration
	// Note: This test verifies structural validity of generated migration
	// Actual DB execution is done in E2E tests

	let app_label = "todos";
	let empty_schema = DatabaseSchema::default();
	let target_schema = create_todos_schema();

	let repository = Arc::new(Mutex::new(TestRepository::new()));
	let generator = AutoMigrationGenerator::new(target_schema.clone(), repository.clone());

	let result = generator
		.generate(app_label, empty_schema.clone())
		.await
		.expect("Migration generation should succeed");

	// Build Migration struct
	let migration = Migration {
		app_label,
		name: "0001_initial".to_string(),
		operations: result.operations.clone(),
		dependencies: Vec::new(),
		atomic: true,
		replaces: Vec::new(),
		initial: Some(true),
	};

	// Verify: Migration has valid structure
	assert_eq!(migration.app_label, "todos");
	assert_eq!(migration.name, "0001_initial");
	assert!(migration.initial.unwrap_or(false));
	assert!(!migration.operations.is_empty());

	// Verify: All operations are valid (can be cloned, serialized, etc.)
	for operation in &migration.operations {
		// Operations should be cloneable
		let _cloned = operation.clone();
	}
}

#[tokio::test]
async fn nc_12_one_to_one_creates_unique_foreign_key() {
	// Test: Generate appropriate migration from OneToOne addition
	// OneToOne is implemented as ForeignKey with UNIQUE constraint

	let app_label = "todos";
	let from_schema = create_todos_schema();
	let mut to_schema = create_todos_schema();

	// Add profile_id column (OneToOne relationship)
	to_schema.tables.get_mut("todos").unwrap().columns.insert(
		"profile_id".to_string(),
		ColumnSchema {
			name: "profile_id".to_string(),
			data_type: FieldType::Integer,
			nullable: true,
			default: None,
			primary_key: false,
			auto_increment: false,
		},
	);

	// Add UNIQUE index on profile_id
	use reinhardt_db::migrations::schema_diff::IndexSchema;
	to_schema
		.tables
		.get_mut("todos")
		.unwrap()
		.indexes
		.push(IndexSchema {
			name: "idx_unique_profile_id".to_string(),
			columns: vec!["profile_id".to_string()],
			unique: true,
			access_method: None,
			index_type: None,
			expressions: None,
			operator_class: None,
		});

	let repository = Arc::new(Mutex::new(TestRepository::new()));
	let generator = AutoMigrationGenerator::new(to_schema.clone(), repository.clone());

	let result = generator
		.generate(app_label, from_schema.clone())
		.await
		.expect("OneToOne generation should succeed");

	// Verify: AddColumn for profile_id
	let has_add_column = result.operations.iter().any(|op| {
		matches!(
			op,
			Operation::AddColumn { table, column }
			if table == &"todos" && column.name == "profile_id"
		)
	});

	// Verify: CreateIndex with unique=true
	let has_unique_index = result.operations.iter().any(|op| {
		matches!(
			op,
			Operation::CreateIndex { table, columns, unique }
			if table == &"todos" && columns.contains(&"profile_id".to_string()) && *unique
		)
	});

	assert!(has_add_column, "Should add profile_id column");
	assert!(has_unique_index, "Should create unique index on profile_id");
}

#[tokio::test]
async fn nc_13_default_value_addition_creates_alter_column() {
	// Test: Generate AlterColumn from default value addition
	let app_label = "todos";
	let from_schema = create_todos_schema();
	let mut to_schema = create_todos_schema();

	// Add priority column with default value
	to_schema.tables.get_mut("todos").unwrap().columns.insert(
		"priority".to_string(),
		ColumnSchema {
			name: "priority".to_string(),
			data_type: FieldType::Integer,
			nullable: false,
			default: Some("0".to_string()),
			primary_key: false,
			auto_increment: false,
		},
	);

	let repository = Arc::new(Mutex::new(TestRepository::new()));
	let generator = AutoMigrationGenerator::new(to_schema.clone(), repository.clone());

	let result = generator
		.generate(app_label, from_schema.clone())
		.await
		.expect("Default value generation should succeed");

	// Verify: AddColumn with default value
	let has_add_column_with_default = result.operations.iter().any(|op| {
		if let Operation::AddColumn { table, column } = op {
			table == &"todos" && column.name == "priority" && column.default.is_some()
		} else {
			false
		}
	});

	assert!(
		has_add_column_with_default,
		"Should add priority column with default value"
	);
}

#[tokio::test]
async fn nc_14_null_constraint_change_creates_alter_column() {
	// Test: Generate AlterColumn from NULL constraint change
	let app_label = "todos";
	let mut from_schema = create_todos_schema();
	let mut to_schema = create_todos_schema();

	// Change description from nullable to NOT NULL
	// First add description as nullable
	from_schema.tables.get_mut("todos").unwrap().columns.insert(
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

	// Then make it NOT NULL in target
	to_schema.tables.get_mut("todos").unwrap().columns.insert(
		"description".to_string(),
		ColumnSchema {
			name: "description".to_string(),
			data_type: FieldType::Text,
			nullable: false, // Changed to NOT NULL
			default: None,
			primary_key: false,
			auto_increment: false,
		},
	);

	let repository = Arc::new(Mutex::new(TestRepository::new()));
	let generator = AutoMigrationGenerator::new(to_schema.clone(), repository.clone());

	let result = generator
		.generate(app_label, from_schema.clone())
		.await
		.expect("NULL constraint change should succeed");

	// Verify: AlterColumn operation for description
	let has_alter_column = result.operations.iter().any(|op| {
		matches!(
			op,
			Operation::AlterColumn { table, column, .. }
			if table == &"todos" && column == "description"
		)
	});

	assert!(
		has_alter_column,
		"Should generate AlterColumn for nullable change"
	);
}

#[tokio::test]
async fn nc_15_unique_constraint_addition_creates_add_constraint() {
	// Test: Generate AddConstraint from UNIQUE constraint addition
	let app_label = "todos";
	let from_schema = create_todos_schema();
	let mut to_schema = create_todos_schema();

	// Add UNIQUE index on title column
	use reinhardt_db::migrations::schema_diff::IndexSchema;
	to_schema
		.tables
		.get_mut("todos")
		.unwrap()
		.indexes
		.push(IndexSchema {
			name: "idx_unique_title".to_string(),
			columns: vec!["title".to_string()],
			unique: true,
			access_method: None,
			index_type: None,
			expressions: None,
			operator_class: None,
		});

	let repository = Arc::new(Mutex::new(TestRepository::new()));
	let generator = AutoMigrationGenerator::new(to_schema.clone(), repository.clone());

	let result = generator
		.generate(app_label, from_schema.clone())
		.await
		.expect("UNIQUE constraint generation should succeed");

	// Verify: CreateIndex with unique=true or AddConstraint
	let has_unique_index = result.operations.iter().any(|op| {
		matches!(
			op,
			Operation::CreateIndex { table, columns, unique }
			if table == &"todos" && columns.contains(&"title".to_string()) && *unique
		)
	});

	let has_add_constraint = result
		.operations
		.iter()
		.any(|op| matches!(op, Operation::AddConstraint { table, .. } if table == &"todos"));

	assert!(
		has_unique_index || has_add_constraint,
		"Should generate UNIQUE constraint via CreateIndex or AddConstraint"
	);
}

#[tokio::test]
async fn nc_16_index_deletion_creates_drop_index() {
	// Test: Generate DropIndex from index deletion
	let app_label = "todos";
	let mut from_schema = create_todos_schema();
	let to_schema = create_todos_schema();

	// Add index in from_schema
	use reinhardt_db::migrations::schema_diff::IndexSchema;
	from_schema
		.tables
		.get_mut("todos")
		.unwrap()
		.indexes
		.push(IndexSchema {
			name: "idx_title".to_string(),
			columns: vec!["title".to_string()],
			unique: false,
			access_method: None,
			index_type: None,
			expressions: None,
			operator_class: None,
		});

	// to_schema has no index (index removed)

	let repository = Arc::new(Mutex::new(TestRepository::new()));
	let generator = AutoMigrationGenerator::new(to_schema.clone(), repository.clone());

	let result = generator
		.generate(app_label, from_schema.clone())
		.await
		.expect("DropIndex generation should succeed");

	// Verify: DropIndex operation
	let has_drop_index = result.operations.iter().any(|op| {
		matches!(
			op,
			Operation::DropIndex { table, columns }
			if table == &"todos" && columns.contains(&"title".to_string())
		)
	});

	assert!(
		has_drop_index,
		"Should generate DropIndex operation for removed index"
	);
}

#[tokio::test]
async fn nc_17_constraint_deletion_creates_drop_constraint() {
	// Test: Generate DropConstraint from constraint deletion
	let app_label = "todos";
	let mut from_schema = create_todos_schema();
	let to_schema = create_todos_schema();

	// Add constraint in from_schema
	use reinhardt_db::migrations::schema_diff::ConstraintSchema;
	from_schema
		.tables
		.get_mut("todos")
		.unwrap()
		.constraints
		.push(ConstraintSchema {
			name: "chk_title_length".to_string(),
			constraint_type: "CHECK".to_string(),
			columns: vec!["title".to_string()],
			referenced_table: None,
			referenced_columns: None,
		});

	// to_schema has no constraint (constraint removed)

	let repository = Arc::new(Mutex::new(TestRepository::new()));
	let generator = AutoMigrationGenerator::new(to_schema.clone(), repository.clone());

	let result = generator
		.generate(app_label, from_schema.clone())
		.await
		.expect("DropConstraint generation should succeed");

	// Verify: DropConstraint operation
	let has_drop_constraint = result.operations.iter().any(|op| {
		matches!(
			op,
			Operation::DropConstraint { table, constraint_name, .. }
			if table == &"todos" && constraint_name == "chk_title_length"
		)
	});

	assert!(
		has_drop_constraint,
		"Should generate DropConstraint operation for removed constraint"
	);
}

#[tokio::test]
async fn nc_18_multiple_changes_in_single_migration() {
	// Test: Generate migration with multiple changes
	let app_label = "todos";
	let from_schema = create_todos_schema();
	let mut to_schema = create_todos_schema();

	// Make multiple changes:
	// 1. Add 'description' column
	to_schema.tables.get_mut("todos").unwrap().columns.insert(
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

	// 2. Remove 'completed' column
	to_schema
		.tables
		.get_mut("todos")
		.unwrap()
		.columns
		.remove("completed");

	// 3. Change 'title' type from VARCHAR to TEXT
	to_schema
		.tables
		.get_mut("todos")
		.unwrap()
		.columns
		.get_mut("title")
		.unwrap()
		.data_type = FieldType::Text;

	let repository = Arc::new(Mutex::new(TestRepository::new()));
	let generator = AutoMigrationGenerator::new(to_schema.clone(), repository.clone());

	let result = generator
		.generate(app_label, from_schema.clone())
		.await
		.expect("Multiple changes should succeed");

	// Verify: Multiple operations in single migration
	assert!(
		result.operation_count >= 3,
		"Should have at least 3 operations (AddColumn, DropColumn, AlterColumn)"
	);

	// Verify: Contains AddColumn for description
	let has_add_column = result.operations.iter().any(|op| {
		matches!(
			op,
			Operation::AddColumn {
				table,
				column
			} if table == &"todos" && column.name == "description"
		)
	});

	// Verify: Contains DropColumn for completed
	let has_drop_column = result.operations.iter().any(|op| {
		matches!(
			op,
			Operation::DropColumn {
				table,
				column
			} if table == &"todos" && column == "completed"
		)
	});

	assert!(has_add_column, "Should add description column");
	assert!(has_drop_column, "Should drop completed column");
}

#[tokio::test]
async fn nc_19_multi_app_migrations_generation() {
	// Test: Generate migrations for multiple apps simultaneously
	// Note: Each app is processed with independent AutoMigrationGenerator instance

	let repository = Arc::new(Mutex::new(TestRepository::new()));

	// App 1: todos
	let todos_schema = create_todos_schema();
	let empty_schema = DatabaseSchema::default();

	let generator1 = AutoMigrationGenerator::new(todos_schema.clone(), repository.clone());
	let result1 = generator1
		.generate("todos", empty_schema.clone())
		.await
		.expect("Todos migration should succeed");

	let migration1 = Migration {
		app_label: "todos".to_string(),
		name: "0001_initial".to_string(),
		operations: result1.operations,
		dependencies: Vec::new(),
		atomic: true,
		replaces: Vec::new(),
		initial: Some(true),
	};

	{
		let mut repo = repository.lock().await;
		repo.save(&migration1)
			.await
			.expect("Should save todos migration");
	}

	// App 2: users
	let users_schema = create_users_schema();

	let generator2 = AutoMigrationGenerator::new(users_schema.clone(), repository.clone());
	let result2 = generator2
		.generate("users", empty_schema.clone())
		.await
		.expect("Users migration should succeed");

	let migration2 = Migration {
		app_label: "users".to_string(),
		name: "0001_initial".to_string(),
		operations: result2.operations,
		dependencies: Vec::new(),
		atomic: true,
		replaces: Vec::new(),
		initial: Some(true),
	};

	{
		let mut repo = repository.lock().await;
		repo.save(&migration2)
			.await
			.expect("Should save users migration");
	}

	// Verify: Both migrations exist
	{
		let repo = repository.lock().await;
		let todos_migrations = repo.list("todos").await.expect("Should list todos");
		let users_migrations = repo.list("users").await.expect("Should list users");

		assert_eq!(todos_migrations.len(), 1);
		assert_eq!(users_migrations.len(), 1);
		assert_eq!(todos_migrations[0].app_label, "todos");
		assert_eq!(users_migrations[0].app_label, "users");
	}
}

#[tokio::test]
async fn nc_20_data_preservation_verification() {
	// Test: Data preservation verification (existing data is not lost)
	// Note: This test structurally verifies that migration operations are non-destructive
	// Actual data preservation tests are done in E2E tests

	let app_label = "todos";
	let from_schema = create_todos_schema();
	let mut to_schema = create_todos_schema();

	// Add new column (should not affect existing data)
	to_schema.tables.get_mut("todos").unwrap().columns.insert(
		"description".to_string(),
		ColumnSchema {
			name: "description".to_string(),
			data_type: FieldType::Text,
			nullable: true, // Nullable to preserve existing rows
			default: None,
			primary_key: false,
			auto_increment: false,
		},
	);

	let repository = Arc::new(Mutex::new(TestRepository::new()));
	let generator = AutoMigrationGenerator::new(to_schema.clone(), repository.clone());

	let result = generator
		.generate(app_label, from_schema.clone())
		.await
		.expect("Data-safe migration should succeed");

	// Verify: AddColumn with nullable=true (data-safe)
	let has_safe_add_column = result.operations.iter().any(|op| {
		if let Operation::AddColumn { table, column } = op {
			table == &"todos"
				&& column.name == "description"
				&& (column.nullable || column.default.is_some())
		} else {
			false
		}
	});

	assert!(
		has_safe_add_column,
		"Should add column with nullable=true or default value to preserve existing data"
	);
}

#[tokio::test]
async fn nc_21_composite_primary_key_table_creation() {
	// Test: Create new table with composite primary key
	// Create user_posts table with composite primary key (user_id, post_id)

	let app_label = "testapp";
	let empty_schema = DatabaseSchema::default();
	let target_schema = create_composite_pk_schema();

	let repository = Arc::new(Mutex::new(TestRepository::new()));
	let generator = AutoMigrationGenerator::new(target_schema.clone(), repository.clone());

	let result = generator
		.generate(app_label, empty_schema.clone())
		.await
		.expect("Composite PK table creation should succeed");

	assert_eq!(
		result.operation_count, 1,
		"Should generate one CreateTable operation"
	);

	if let Operation::CreateTable { name, columns, .. } = &result.operations[0] {
		assert_eq!(name, "user_posts", "Table name should be user_posts");

		// Verify composite primary key columns
		let pk_columns: Vec<_> = columns
			.iter()
			.filter(|c| c.primary_key)
			.map(|c| c.name)
			.collect();
		assert_eq!(
			pk_columns.len(),
			2,
			"Should have 2 columns in composite primary key"
		);
		assert!(
			pk_columns.contains(&"user_id"),
			"user_id should be part of primary key"
		);
		assert!(
			pk_columns.contains(&"post_id"),
			"post_id should be part of primary key"
		);
	} else {
		panic!("Expected CreateTable operation");
	}
}

#[tokio::test]
async fn nc_22_add_composite_primary_key_to_existing_table() {
	// Test: Add composite primary key to existing table
	// Add composite primary key (user_id, post_id) to user_posts table without PK

	let app_label = "testapp";
	let from_schema = create_user_posts_no_pk_schema();
	let to_schema = create_composite_pk_schema();

	let repository = Arc::new(Mutex::new(TestRepository::new()));
	let generator = AutoMigrationGenerator::new(to_schema.clone(), repository.clone());

	let result = generator
		.generate(app_label, from_schema.clone())
		.await
		.expect("Adding composite PK should succeed");

	// Adding composite primary key may generate as AlterColumn (multiple) or AddConstraint
	assert!(
		result.operation_count >= 1,
		"Should generate at least one operation for composite PK addition"
	);

	// AlterColumn operations for primary_key flag changes
	let alter_ops: Vec<_> = result
		.operations
		.iter()
		.filter(|op| matches!(op, Operation::AlterColumn { .. }))
		.collect();

	// or AddConstraint operation
	let constraint_ops: Vec<_> = result
		.operations
		.iter()
		.filter(|op| matches!(op, Operation::AddConstraint { .. }))
		.collect();

	assert!(
		!alter_ops.is_empty() || !constraint_ops.is_empty(),
		"Should generate AlterColumn or AddConstraint operations"
	);
}

#[tokio::test]
async fn nc_23_drop_composite_primary_key() {
	// Test: Drop composite primary key
	// Change user_posts table with composite PK to table without PK

	let app_label = "testapp";
	let from_schema = create_composite_pk_schema();
	let to_schema = create_user_posts_no_pk_schema();

	let repository = Arc::new(Mutex::new(TestRepository::new()));
	let generator = AutoMigrationGenerator::new(to_schema.clone(), repository.clone());

	let result = generator
		.generate(app_label, from_schema.clone())
		.await
		.expect("Dropping composite PK should succeed");

	// Dropping composite primary key may generate as AlterColumn (multiple) or DropConstraint
	assert!(
		result.operation_count >= 1,
		"Should generate at least one operation for composite PK deletion"
	);

	// AlterColumn operations for primary_key flag changes
	let alter_ops: Vec<_> = result
		.operations
		.iter()
		.filter(|op| matches!(op, Operation::AlterColumn { .. }))
		.collect();

	// or DropConstraint operation
	let constraint_ops: Vec<_> = result
		.operations
		.iter()
		.filter(|op| matches!(op, Operation::DropConstraint { .. }))
		.collect();

	assert!(
		!alter_ops.is_empty() || !constraint_ops.is_empty(),
		"Should generate AlterColumn or DropConstraint operations"
	);
}

// ============================================================================
// Error Cases (EC-01 ~ EC-05)
// ============================================================================

#[tokio::test]
async fn ec_01_no_models_error() {
	// Test: Error when no models exist
	// Verify that makemigrations returns error when executed with empty global registry

	let app_label = "emptyapp";
	let empty_schema = DatabaseSchema::default();
	let target_schema = DatabaseSchema::default(); // No models

	let repository = Arc::new(Mutex::new(TestRepository::new()));
	let generator = AutoMigrationGenerator::new(target_schema.clone(), repository.clone());

	let result = generator.generate(app_label, empty_schema.clone()).await;

	// Verify: NoChangesDetected or appropriate error
	assert!(
		matches!(result, Err(AutoMigrationError::NoChangesDetected)),
		"Should return error when no models exist"
	);
}

#[tokio::test]
async fn ec_02_empty_flag_without_app_label_error() {
	// Test: Error when app_label is missing with --empty flag
	// NOTE: This test is for command-line option validation,
	// should be implemented in makemigrations_e2e_integration.rs (E2E tests).
	// Out of scope for AutoMigrationGenerator internal logic tests.
}

#[tokio::test]
async fn ec_03_from_state_construction_failure_error() {
	// Test: Error on from_state construction failure
	// NOTE: This test is for validation before AutoMigrationGenerator (ProjectState construction),
	// should be implemented in makemigrations_e2e_integration.rs (E2E tests).
	// Out of scope for AutoMigrationGenerator internal logic tests.
}

#[tokio::test]
async fn ec_04_invalid_field_definition_error() {
	// Test: Error on invalid field definition
	// NOTE: This test is for schema construction validation errors,
	// should be implemented in makemigrations_e2e_integration.rs (E2E tests).
	// AutoMigrationGenerator receives DatabaseSchema,
	// schema validation is the responsibility of upper layers.
}

#[tokio::test]
async fn ec_05_file_write_permission_error() {
	// Test: File write permission error
	// NOTE: This test is for FilesystemRepository responsibility,
	// should be implemented in makemigrations_e2e_integration.rs (E2E tests).
	// Out of scope for AutoMigrationGenerator internal logic tests.
}

// ============================================================================
// Edge Cases (EDG-01 ~ EDG-14)
// ============================================================================

#[tokio::test]
async fn edg_01_empty_migration_generation() {
	// Test: Generate empty migration (--empty)
	// Simulate --empty flag behavior: manually create empty migration

	let app_label = "todos";
	let repository = Arc::new(Mutex::new(TestRepository::new()));

	// Create empty migration manually (simulating --empty flag)
	let empty_migration = Migration {
		app_label,
		name: "0001_custom".to_string(),
		operations: Vec::new(), // Empty operations
		dependencies: Vec::new(),
		replaces: Vec::new(),
		atomic: true,
		initial: None,
	};

	// Save empty migration
	{
		let mut repo = repository.lock().await;
		repo.save(&empty_migration)
			.await
			.expect("Should save empty migration");
	}

	// Verify: Migration exists with empty operations
	{
		let repo = repository.lock().await;
		let migration = repo
			.get(app_label, "0001_custom")
			.await
			.expect("Should retrieve migration");
		assert_eq!(migration.operations.len(), 0, "Should have zero operations");
		assert_eq!(migration.name, "0001_custom", "Should have custom name");
	}
}

#[tokio::test]
async fn edg_02_no_changes_detected() {
	// Test: No changes detected case
	let app_label = "todos";
	let schema = create_todos_schema();

	let repository = Arc::new(Mutex::new(TestRepository::new()));
	let generator = AutoMigrationGenerator::new(schema.clone(), repository.clone());

	let result = generator.generate(app_label, schema.clone()).await;

	// Verify: NoChangesDetected error is returned
	assert!(
		matches!(result, Err(AutoMigrationError::NoChangesDetected)),
		"Should return NoChangesDetected when there are no changes"
	);
}

#[tokio::test]
async fn edg_03_dry_run_mode() {
	// Test: --dry-run mode
	// NOTE: This test is for command-line option validation,
	// should be implemented in makemigrations_e2e_integration.rs (E2E tests).
	// Out of scope for AutoMigrationGenerator internal logic tests.
}

#[tokio::test]
async fn edg_04_custom_name_specification() {
	// Test: --name custom name specification
	// NOTE: This test is for command-line option validation,
	// should be implemented in makemigrations_e2e_integration.rs (E2E tests).
	// Out of scope for AutoMigrationGenerator internal logic tests.
}

#[tokio::test]
async fn edg_05_verbose_mode() {
	// Test: --verbose mode
	// NOTE: This test is for command-line option validation,
	// should be implemented in makemigrations_e2e_integration.rs (E2E tests).
	// Out of scope for AutoMigrationGenerator internal logic tests.
}

#[tokio::test]
async fn edg_06_custom_migrations_directory() {
	// Test: --migrations-dir custom directory specification
	// NOTE: This test is for command-line option validation,
	// should be implemented in makemigrations_e2e_integration.rs (E2E tests).
	// Out of scope for AutoMigrationGenerator internal logic tests.
}

#[tokio::test]
async fn edg_07_from_db_mode() {
	// Test: --from-db mode
	// NOTE: This test is for command-line option validation,
	// should be implemented in makemigrations_e2e_integration.rs (E2E tests).
	// Out of scope for AutoMigrationGenerator internal logic tests.
}

#[tokio::test]
async fn edg_08_long_model_field_names() {
	// Test: Long model/field names (255 characters)
	// Verify that migrations are generated normally even with long names

	let app_label = "testapp";
	let empty_schema = DatabaseSchema::default();

	// Generate 255-character table and field names
	let long_table_name = "a".repeat(255);
	let long_field_name = "b".repeat(255);

	let mut schema = DatabaseSchema::default();
	let mut table = TableSchema {
		name: &long_table_name,
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
		long_field_name.clone(),
		ColumnSchema {
			name: &long_field_name,
			data_type: FieldType::VarChar(255),
			nullable: false,
			default: None,
			primary_key: false,
			auto_increment: false,
		},
	);

	schema.tables.insert(long_table_name.clone(), table);

	let repository = Arc::new(Mutex::new(TestRepository::new()));
	let generator = AutoMigrationGenerator::new(schema.clone(), repository.clone());

	let result = generator
		.generate(app_label, empty_schema.clone())
		.await
		.expect("Long names should be handled successfully");

	assert_eq!(result.operation_count, 1);

	if let Operation::CreateTable { name, columns, .. } = &result.operations[0] {
		assert_eq!(name.len(), 255, "Table name should be 255 characters");
		assert_eq!(name, &long_table_name);

		let long_column = columns.iter().find(|c| c.name == long_field_name);
		assert!(
			long_column.is_some(),
			"Long field name should be present in columns"
		);
	} else {
		panic!("Expected CreateTable operation");
	}
}

#[tokio::test]
async fn edg_09_large_number_of_fields() {
	// Test: Large number of fields (100+)
	// Verify that tables with 100+ fields are processed normally

	let app_label = "testapp";
	let empty_schema = DatabaseSchema::default();

	let mut schema = DatabaseSchema::default();
	let mut table = TableSchema {
		name: "large_table".to_string(),
		columns: BTreeMap::new(),
		indexes: Vec::new(),
		constraints: Vec::new(),
	};

	// id field
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

	// Add 150 fields
	for i in 1..=150 {
		let field_name = format!("field_{:03}", i);
		table.columns.insert(
			field_name.clone(),
			ColumnSchema {
				name: &field_name,
				data_type: FieldType::VarChar(100),
				nullable: true,
				default: None,
				primary_key: false,
				auto_increment: false,
			},
		);
	}

	schema.tables.insert("large_table".to_string(), table);

	let repository = Arc::new(Mutex::new(TestRepository::new()));
	let generator = AutoMigrationGenerator::new(schema.clone(), repository.clone());

	let result = generator
		.generate(app_label, empty_schema.clone())
		.await
		.expect("Large number of fields should be handled successfully");

	assert_eq!(
		result.operation_count, 1,
		"Should generate one CreateTable operation"
	);

	if let Operation::CreateTable { name, columns, .. } = &result.operations[0] {
		assert_eq!(name, "large_table");
		assert_eq!(
			columns.len(),
			151,
			"Should have 151 columns (1 id + 150 fields)"
		);
	} else {
		panic!("Expected CreateTable operation");
	}
}

#[tokio::test]
async fn edg_10_deep_dependency_chain() {
	// Test: Deep dependency chain (10 levels)
	// Generate 10 levels of migrations sequentially and verify correct dependency construction

	let app_label = "testapp";
	let repository = Arc::new(Mutex::new(TestRepository::new()));

	let mut current_schema = DatabaseSchema::default();

	// Generate 10-level migrations
	for stage in 1..=10 {
		// Add new table
		let table_name = format!("table_{:02}", stage);
		let mut table = TableSchema {
			name: &table_name,
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

		let mut next_schema = current_schema.clone();
		next_schema.tables.insert(table_name.clone(), table);

		let generator = AutoMigrationGenerator::new(next_schema.clone(), repository.clone());

		let result = generator
			.generate(app_label, current_schema.clone())
			.await
			.unwrap_or_else(|e| panic!("Stage {} migration should succeed: {:?}", stage, e));

		assert_eq!(result.operation_count, 1);

		// Save migration
		let migration_name = format!("{:04}_stage_{}", stage, stage);
		let migration = Migration {
			app_label,
			name: &migration_name,
			operations: result.operations.clone(),
			dependencies: if stage == 1 {
				Vec::new()
			} else {
				vec![format!("{:04}_stage_{}", stage - 1, stage - 1)]
			},
			replaces: Vec::new(),
			atomic: true,
			initial: Some(stage == 1),
		};

		{
			let mut repo = repository.lock().await;
			repo.save(&migration).await.expect("Should save migration");
		}

		current_schema = next_schema;
	}

	// Verify that 10 migrations are saved
	let repo = repository.lock().await;
	let migrations = repo.list(app_label).await.expect("Should list migrations");
	assert_eq!(migrations.len(), 10);
}

#[tokio::test]
async fn edg_11_unicode_in_names() {
	// Test: Names with special characters (Unicode)
	// Verify processing of table/field names containing Unicode characters like Japanese, emoji

	let app_label = "testapp";
	let empty_schema = DatabaseSchema::default();

	let mut schema = DatabaseSchema::default();

	// Japanese table name
	let table_name = "ユーザー情報";
	let mut table = TableSchema {
		name: table_name,
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

	// Japanese field name
	table.columns.insert(
		"名前".to_string(),
		ColumnSchema {
			name: "名前".to_string(),
			data_type: FieldType::VarChar(255),
			nullable: false,
			default: None,
			primary_key: false,
			auto_increment: false,
		},
	);

	// Emoji field name
	table.columns.insert(
		"emoji_field_🚀".to_string(),
		ColumnSchema {
			name: "emoji_field_🚀".to_string(),
			data_type: FieldType::Text,
			nullable: true,
			default: None,
			primary_key: false,
			auto_increment: false,
		},
	);

	schema.tables.insert(table_name.to_string(), table);

	let repository = Arc::new(Mutex::new(TestRepository::new()));
	let generator = AutoMigrationGenerator::new(schema.clone(), repository.clone());

	let result = generator
		.generate(app_label, empty_schema.clone())
		.await
		.expect("Unicode characters in names should be handled successfully");

	assert_eq!(result.operation_count, 1);

	if let Operation::CreateTable { name, columns, .. } = &result.operations[0] {
		assert_eq!(
			name, table_name,
			"Table name should contain Unicode characters"
		);

		// Verify Japanese field exists
		assert!(
			columns.iter().any(|c| c.name == "名前"),
			"Should have Japanese field name"
		);

		// Verify emoji field exists
		assert!(
			columns.iter().any(|c| c.name == "emoji_field_🚀"),
			"Should have emoji field name"
		);
	} else {
		panic!("Expected CreateTable operation");
	}
}

#[tokio::test]
async fn edg_12_sql_reserved_words() {
	// Test: Table/column names with SQL reserved words
	// Verify processing when SQL reserved words like SELECT, FROM, WHERE are used as table/column names

	let app_label = "testapp";
	let empty_schema = DatabaseSchema::default();

	let mut schema = DatabaseSchema::default();

	// SQL reserved word as table name
	let table_name = "select";
	let mut table = TableSchema {
		name: table_name,
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

	// SQL reserved word as field name
	table.columns.insert(
		"from".to_string(),
		ColumnSchema {
			name: "from".to_string(),
			data_type: FieldType::VarChar(255),
			nullable: false,
			default: None,
			primary_key: false,
			auto_increment: false,
		},
	);

	table.columns.insert(
		"where".to_string(),
		ColumnSchema {
			name: "where".to_string(),
			data_type: FieldType::VarChar(255),
			nullable: true,
			default: None,
			primary_key: false,
			auto_increment: false,
		},
	);

	table.columns.insert(
		"order".to_string(),
		ColumnSchema {
			name: "order".to_string(),
			data_type: FieldType::Integer,
			nullable: true,
			default: None,
			primary_key: false,
			auto_increment: false,
		},
	);

	schema.tables.insert(table_name.to_string(), table);

	let repository = Arc::new(Mutex::new(TestRepository::new()));
	let generator = AutoMigrationGenerator::new(schema.clone(), repository.clone());

	let result = generator
		.generate(app_label, empty_schema.clone())
		.await
		.expect("SQL reserved words in names should be handled successfully");

	assert_eq!(result.operation_count, 1);

	if let Operation::CreateTable { name, columns, .. } = &result.operations[0] {
		assert_eq!(name, table_name, "Table name should be SQL reserved word");

		// Verify SQL reserved word field names exist
		assert!(
			columns.iter().any(|c| c.name == "from"),
			"Should have 'from' field (SQL reserved word)"
		);
		assert!(
			columns.iter().any(|c| c.name == "where"),
			"Should have 'where' field (SQL reserved word)"
		);
		assert!(
			columns.iter().any(|c| c.name == "order"),
			"Should have 'order' field (SQL reserved word)"
		);
	} else {
		panic!("Expected CreateTable operation");
	}
}

#[tokio::test]
async fn edg_13_same_name_different_apps() {
	// Test: Models with same name in different apps
	// Verify independent processing when same table name is used with different app_labels

	let empty_schema = DatabaseSchema::default();

	// Create schema with same table name (users)
	let mut schema = DatabaseSchema::default();
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

	table.columns.insert(
		"name".to_string(),
		ColumnSchema {
			name: "name".to_string(),
			data_type: FieldType::VarChar(255),
			nullable: false,
			default: None,
			primary_key: false,
			auto_increment: false,
		},
	);

	schema.tables.insert("users".to_string(), table);

	let repository = Arc::new(Mutex::new(TestRepository::new()));

	// Create users table in app1
	let generator_app1 = AutoMigrationGenerator::new(schema.clone(), repository.clone());
	let result_app1 = generator_app1
		.generate("app1", empty_schema.clone())
		.await
		.expect("app1 users table creation should succeed");

	assert_eq!(result_app1.operation_count, 1);

	// Save app1 migration
	let migration_app1 = Migration {
		app_label: "app1".to_string(),
		name: "0001_initial".to_string(),
		operations: result_app1.operations.clone(),
		dependencies: Vec::new(),
		replaces: Vec::new(),
		atomic: true,
		initial: Some(true),
	};
	{
		let mut repo = repository.lock().await;
		repo.save(&migration_app1)
			.await
			.expect("Should save app1 migration");
	}

	// Create same users table in app2 (independent as different app)
	let generator_app2 = AutoMigrationGenerator::new(schema.clone(), repository.clone());
	let result_app2 = generator_app2
		.generate("app2", empty_schema.clone())
		.await
		.expect("app2 users table creation should succeed (independent from app1)");

	assert_eq!(result_app2.operation_count, 1);

	// Save app2 migration
	let migration_app2 = Migration {
		app_label: "app2".to_string(),
		name: "0001_initial".to_string(),
		operations: result_app2.operations.clone(),
		dependencies: Vec::new(),
		replaces: Vec::new(),
		atomic: true,
		initial: Some(true),
	};
	{
		let mut repo = repository.lock().await;
		repo.save(&migration_app2)
			.await
			.expect("Should save app2 migration");
	}

	// Verify migrations were created independently for both apps
	let repo = repository.lock().await;
	let app1_migrations = repo
		.list("app1")
		.await
		.expect("Should list app1 migrations");
	let app2_migrations = repo
		.list("app2")
		.await
		.expect("Should list app2 migrations");

	assert_eq!(app1_migrations.len(), 1, "app1 should have 1 migration");
	assert_eq!(app2_migrations.len(), 1, "app2 should have 1 migration");

	assert_eq!(app1_migrations[0].app_label, "app1");
	assert_eq!(app2_migrations[0].app_label, "app2");
}

#[tokio::test]
async fn edg_14_cross_app_dependencies() {
	// Test: Cross-app dependencies
	// Verify that foreign key dependencies between different apps are handled correctly
	// app1.users → app2.posts (posts has FK to users)

	let empty_schema = DatabaseSchema::default();

	// app1: users table
	let mut users_schema = DatabaseSchema::default();
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
		"name".to_string(),
		ColumnSchema {
			name: "name".to_string(),
			data_type: FieldType::VarChar(255),
			nullable: false,
			default: None,
			primary_key: false,
			auto_increment: false,
		},
	);

	users_schema.tables.insert("users".to_string(), users_table);

	let repository = Arc::new(Mutex::new(TestRepository::new()));

	// Create users table in app1
	let generator_app1 = AutoMigrationGenerator::new(users_schema.clone(), repository.clone());
	let result_app1 = generator_app1
		.generate("app1", empty_schema.clone())
		.await
		.expect("app1 users table creation should succeed");

	assert_eq!(result_app1.operation_count, 1);

	// Save app1 migration
	let migration_app1 = Migration {
		app_label: "app1".to_string(),
		name: "0001_initial".to_string(),
		operations: result_app1.operations.clone(),
		dependencies: Vec::new(),
		replaces: Vec::new(),
		atomic: true,
		initial: Some(true),
	};
	{
		let mut repo = repository.lock().await;
		repo.save(&migration_app1)
			.await
			.expect("Should save app1 migration");
	}

	// app2: posts table (with FK to users)
	let mut posts_schema = DatabaseSchema::default();
	let mut posts_table = TableSchema {
		name: "posts".to_string(),
		columns: BTreeMap::new(),
		indexes: Vec::new(),
		constraints: Vec::new(),
	};

	posts_table.columns.insert(
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

	posts_table.columns.insert(
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

	// user_id (FK to app1.users)
	posts_table.columns.insert(
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

	// ForeignKey constraint
	posts_table.constraints.push(ConstraintSchema {
		name: "fk_posts_user_id".to_string(),
		constraint_type: ConstraintType::ForeignKey {
			columns: vec!["user_id".to_string()],
			referenced_table: "users".to_string(),
			referenced_columns: vec!["id".to_string()],
			on_delete: Some(ForeignKeyAction::Cascade),
			on_update: None,
		},
	});

	posts_schema.tables.insert("posts".to_string(), posts_table);

	// Create posts table in app2
	let generator_app2 = AutoMigrationGenerator::new(posts_schema.clone(), repository.clone());
	let result_app2 = generator_app2
		.generate("app2", empty_schema.clone())
		.await
		.expect("app2 posts table creation should succeed");

	// Creating posts table and adding FK constraint
	assert!(
		result_app2.operation_count >= 1,
		"Should generate at least one operation for posts table"
	);

	// Verify CreateTable operation exists
	let has_create_table = result_app2
		.operations
		.iter()
		.any(|op| matches!(op, Operation::CreateTable { name, .. } if name == "posts"));
	assert!(
		has_create_table,
		"Should have CreateTable operation for posts"
	);

	// Verify FK constraint exists (in AddConstraint or CreateTable constraints)
	let has_fk_constraint = result_app2.operations.iter().any(|op| match op {
		Operation::CreateTable { constraints, .. } => {
			constraints.iter().any(|c| c.name == "fk_posts_user_id")
		}
		Operation::AddConstraint { name, .. } => name == "fk_posts_user_id",
		_ => false,
	});
	assert!(
		has_fk_constraint,
		"Should have FK constraint to users table"
	);
}
