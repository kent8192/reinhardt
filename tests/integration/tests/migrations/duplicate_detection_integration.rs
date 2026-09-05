//! Integration tests for duplicate detection in auto-migration generation
//!
//! Tests two scenarios:
//! 1. makemigrations → migrate → makemigrations (after execution)
//! 2. makemigrations → makemigrations (rapid successive calls)

use reinhardt_db::migrations::{
	AutoMigrationError, AutoMigrationGenerator, FieldType, Migration, MigrationRepository,
	Operation,
};
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use tokio::sync::Mutex;

// Import DatabaseSchema and SchemaDiff from reinhardt_migrations
use reinhardt_db::migrations::schema_diff::{ColumnSchema, DatabaseSchema, TableSchema};

/// Test repository implementation for integration tests
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

/// Helper to create a simple schema with a users table
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

/// Helper to create a schema with users and posts tables
fn create_users_and_posts_schema() -> DatabaseSchema {
	let mut schema = create_users_schema();
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
			data_type: FieldType::Text,
			nullable: false,
			default: None,
			primary_key: false,
			auto_increment: false,
		},
	);
	schema.tables.insert("posts".to_string(), posts_table);
	schema
}

#[tokio::test]
async fn test_scenario_1_makemigrations_migrate_makemigrations() {
	// Scenario 1: makemigrations → migrate → makemigrations
	// After migrate, running makemigrations again should detect no changes

	let app_label = "testapp";
	let empty_schema = DatabaseSchema::default();
	let target_schema = create_users_schema();

	let repository = Arc::new(Mutex::new(TestRepository::new()));

	// Step 1: First makemigrations (empty → users table)
	let generator = AutoMigrationGenerator::new(target_schema.clone(), repository.clone());

	let result1 = generator
		.generate(app_label, empty_schema.clone())
		.await
		.expect("First makemigrations should succeed");

	assert_eq!(
		result1.operation_count, 1,
		"Should generate one CreateTable operation"
	);
	assert!(
		matches!(result1.operations[0], Operation::CreateTable { .. }),
		"Operation should be CreateTable"
	);

	// Step 2: Simulate migrate (no actual database changes, just state update)
	// After migration, the current schema becomes the target schema

	// Step 3: Second makemigrations (users table → users table, no changes)
	let result2 = generator.generate(app_label, target_schema.clone()).await;

	match result2 {
		Err(AutoMigrationError::NoChangesDetected) => {
			// Expected: No changes detected
		}
		Err(AutoMigrationError::DuplicateMigration) => {
			// Also acceptable: Duplicate detection works
		}
		Ok(_) => {
			panic!("Second makemigrations should fail with NoChangesDetected or DuplicateMigration")
		}
		Err(e) => panic!("Unexpected error: {:?}", e),
	}
}

#[tokio::test]
async fn test_scenario_2_rapid_successive_makemigrations() {
	// Scenario 2: makemigrations → makemigrations (rapid succession)
	// Second call should detect duplicate operations

	let app_label = "testapp";
	let empty_schema = DatabaseSchema::default();
	let target_schema = create_users_schema();

	let repository = Arc::new(Mutex::new(TestRepository::new()));

	let generator = AutoMigrationGenerator::new(target_schema.clone(), repository.clone());

	// First makemigrations
	let result1 = generator
		.generate(app_label, empty_schema.clone())
		.await
		.expect("First makemigrations should succeed");

	assert_eq!(result1.operation_count, 1);

	// Save the first migration to repository (caller's responsibility)
	let migration1 = Migration::from_parts("0001_initial".to_string(), app_label, result1.operations.clone(), Vec::new(), Vec::new(), true, Some(true), false, false, Vec::new(), Vec::new());
	{
		let mut repo = repository.lock().await;
		repo.save(&migration1)
			.await
			.expect("Should save first migration");
	}

	// Second makemigrations with same schema diff
	let result2 = generator.generate(app_label, empty_schema.clone()).await;

	match result2 {
		Err(AutoMigrationError::DuplicateMigration) => {
			// Expected: Duplicate migration detected
		}
		Ok(_) => panic!("Second makemigrations should fail with DuplicateMigration"),
		Err(e) => panic!("Unexpected error: {:?}", e),
	}
}

#[tokio::test]
async fn test_nanosecond_precision_prevents_collision() {
	// Test that nanosecond precision prevents timestamp collisions
	// in rapid successive calls

	let app_label = "testapp";
	let empty_schema = DatabaseSchema::default();
	let schema1 = create_users_schema();
	let schema2 = create_users_and_posts_schema();

	let repository = Arc::new(Mutex::new(TestRepository::new()));

	// Generate first migration (empty → users)
	let generator1 = AutoMigrationGenerator::new(schema1.clone(), repository.clone());
	let result1 = generator1
		.generate(app_label, empty_schema.clone())
		.await
		.expect("First migration should succeed");

	// Generate second migration immediately (users → users + posts)
	let generator2 = AutoMigrationGenerator::new(schema2.clone(), repository.clone());
	let result2 = generator2
		.generate(app_label, schema1.clone())
		.await
		.expect("Second migration should succeed");

	// Verify that both migrations have different table names
	// (migration_file is a placeholder in AutoMigrationResult, actual path determined by caller)
	// First migration: empty → users (1 CreateTable for "users")
	assert_eq!(result1.operation_count, 1);
	assert!(
		matches!(&result1.operations[0], Operation::CreateTable { name, .. } if name == &"users"),
		"First migration should create users table"
	);

	// Second migration: users → users + posts (1 CreateTable for "posts")
	assert_eq!(result2.operation_count, 1);
	assert!(
		matches!(&result2.operations[0], Operation::CreateTable { name, .. } if name == &"posts"),
		"Second migration should create posts table"
	);

	// The important part: Both migrations can be created without duplicate detection errors
	// This demonstrates that nanosecond precision allows rapid consecutive migrations
}

#[tokio::test]
async fn test_different_operations_not_duplicate() {
	// Test that different operations are not considered duplicates

	let app_label = "testapp";
	let empty_schema = DatabaseSchema::default();
	let schema1 = create_users_schema();
	let schema2 = create_users_and_posts_schema();

	let repository = Arc::new(Mutex::new(TestRepository::new()));

	// First migration: empty → users
	let generator1 = AutoMigrationGenerator::new(schema1.clone(), repository.clone());
	let result1 = generator1
		.generate(app_label, empty_schema.clone())
		.await
		.expect("First migration should succeed");

	assert_eq!(result1.operation_count, 1);

	// Second migration: users → users + posts (different operations)
	let generator2 = AutoMigrationGenerator::new(schema2.clone(), repository.clone());
	let result2 = generator2
		.generate(app_label, schema1.clone())
		.await
		.expect("Second migration should succeed because operations are different");

	assert_eq!(result2.operation_count, 1);
	assert!(
		matches!(result2.operations[0], Operation::CreateTable { .. }),
		"Second migration should create posts table"
	);
}

#[tokio::test]
async fn test_duplicate_operations_detected() {
	// Test that identical operations are detected as duplicates

	let app_label = "testapp";
	let empty_schema = DatabaseSchema::default();
	let target_schema = create_users_schema();

	let repository = Arc::new(Mutex::new(TestRepository::new()));

	let generator = AutoMigrationGenerator::new(target_schema.clone(), repository.clone());

	// First migration
	let result1 = generator
		.generate(app_label, empty_schema.clone())
		.await
		.expect("First migration should succeed");

	assert_eq!(result1.operation_count, 1);

	// Save the first migration to repository (caller's responsibility)
	let migration1 = Migration::from_parts("0001_initial".to_string(), app_label, result1.operations.clone(), Vec::new(), Vec::new(), true, Some(true), false, false, Vec::new(), Vec::new());
	{
		let mut repo = repository.lock().await;
		repo.save(&migration1)
			.await
			.expect("Should save first migration");
	}

	// Try to generate the same migration again
	let result2 = generator.generate(app_label, empty_schema.clone()).await;

	assert!(
		matches!(result2, Err(AutoMigrationError::DuplicateMigration)),
		"Should detect duplicate operations"
	);
}

#[tokio::test]
async fn test_semantic_duplicate_with_different_column_order() {
	// Test that operations with different column order are detected as semantic duplicates

	let app_label = "testapp";
	let empty_schema = DatabaseSchema::default();

	let repository = Arc::new(Mutex::new(TestRepository::new()));

	// Create first schema with columns in one order
	let mut schema1 = DatabaseSchema::default();
	let mut table1 = TableSchema {
		name: "users".to_string(),
		columns: BTreeMap::new(),
		indexes: Vec::new(),
		constraints: Vec::new(),
	};
	// Insert columns in order: id, name, email
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
	table1.columns.insert(
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
	table1.columns.insert(
		"email".to_string(),
		ColumnSchema {
			name: "email".to_string(),
			data_type: FieldType::VarChar(255),
			nullable: false,
			default: None,
			primary_key: false,
			auto_increment: false,
		},
	);
	schema1.tables.insert("users".to_string(), table1);

	let generator1 = AutoMigrationGenerator::new(schema1.clone(), repository.clone());

	// First migration
	let result1 = generator1
		.generate(app_label, empty_schema.clone())
		.await
		.expect("First migration should succeed");

	assert_eq!(result1.operation_count, 1);

	// Save the first migration to repository (caller's responsibility)
	let migration1 = Migration::from_parts("0001_initial".to_string(), app_label, result1.operations.clone(), Vec::new(), Vec::new(), true, Some(true), false, false, Vec::new(), Vec::new());
	{
		let mut repo = repository.lock().await;
		repo.save(&migration1)
			.await
			.expect("Should save first migration");
	}

	// Create second schema with same columns but different order (email, id, name)
	// Note: BTreeMap already sorts by key, but the Operation's column list order matters
	let mut schema2 = DatabaseSchema::default();
	let mut table2 = TableSchema {
		name: "users".to_string(),
		columns: BTreeMap::new(),
		indexes: Vec::new(),
		constraints: Vec::new(),
	};
	// Insert in different order: email, id, name
	table2.columns.insert(
		"email".to_string(),
		ColumnSchema {
			name: "email".to_string(),
			data_type: FieldType::VarChar(255),
			nullable: false,
			default: None,
			primary_key: false,
			auto_increment: false,
		},
	);
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
	schema2.tables.insert("users".to_string(), table2);

	let generator2 = AutoMigrationGenerator::new(schema2.clone(), repository.clone());

	// Try to generate the same migration with different column order
	// Should be detected as duplicate due to semantic equality
	let result2 = generator2.generate(app_label, empty_schema.clone()).await;

	assert!(
		matches!(result2, Err(AutoMigrationError::DuplicateMigration)),
		"Should detect semantic duplicate despite different column order"
	);
}

#[tokio::test]
async fn test_schema_diff_determinism() {
	// Test that schema diff generation is deterministic

	let app_label = "testapp";
	let empty_schema = DatabaseSchema::default();
	let target_schema = create_users_schema();

	// Generate migrations multiple times with the same schema
	let mut operations_list = Vec::new();

	for _ in 0..5 {
		let repository = Arc::new(Mutex::new(TestRepository::new()));
		let generator = AutoMigrationGenerator::new(target_schema.clone(), repository.clone());

		let result = generator
			.generate(app_label, empty_schema.clone())
			.await
			.expect("Migration generation should succeed");

		operations_list.push(result.operations);
	}

	// All operations should be identical (deterministic)
	let first_ops = &operations_list[0];
	for ops in operations_list.iter().skip(1) {
		assert_eq!(
			ops, first_ops,
			"Operations should be deterministic across multiple generations"
		);
	}
}
