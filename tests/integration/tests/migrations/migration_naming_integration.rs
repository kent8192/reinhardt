//! Integration tests for migration naming system
//!
//! Tests the integration of MigrationNamer and MigrationNumbering modules
//! with the migration system.

use reinhardt_db::migrations::{
	ColumnDefinition, Constraint, FieldType, Migration, MigrationNamer, MigrationNumbering,
	MigrationOperation, Operation,
};
use std::fs;
use tempfile::TempDir;

/// Helper function to leak a string to get a 'static lifetime
fn leak_str(s: impl Into<String>) -> &'static str {
	Box::leak(s.into().into_boxed_str())
}

/// Helper function to create a temp migrations directory with sample files
fn setup_migrations_dir() -> TempDir {
	let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
	let migrations_dir = temp_dir.path().join("migrations");
	let app_dir = migrations_dir.join("myapp");
	fs::create_dir_all(&app_dir).expect("Failed to create app directory");

	// Create sample migration files
	fs::write(app_dir.join("0001_initial.rs"), "// Initial migration")
		.expect("Failed to write file");
	fs::write(app_dir.join("0002_add_user_email.rs"), "// Add email field")
		.expect("Failed to write file");
	fs::write(
		app_dir.join("0003_remove_user_age.rs"),
		"// Remove age field",
	)
	.expect("Failed to write file");

	temp_dir
}

#[test]
fn test_migration_numbering_with_existing_migrations() {
	let temp_dir = setup_migrations_dir();
	let migrations_dir = temp_dir.path().join("migrations");

	// Get next number for myapp
	let next_number = MigrationNumbering::next_number(&migrations_dir, "myapp");

	assert_eq!(
		next_number, "0004",
		"Next migration number should be 0004 after 0003"
	);
}

#[test]
fn test_migration_numbering_for_new_app() {
	let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
	let migrations_dir = temp_dir.path().join("migrations");

	// Get next number for a non-existent app
	let next_number = MigrationNumbering::next_number(&migrations_dir, "newapp");

	assert_eq!(
		next_number, "0001",
		"First migration should be numbered 0001"
	);
}

#[test]
fn test_migration_naming_with_single_operation() {
	let operations = vec![Operation::CreateTable {
		name: leak_str("users").to_string(),
		columns: vec![ColumnDefinition::new("id", FieldType::Integer)],
		constraints: vec![],
		without_rowid: None,
		interleave_in_parent: None,
		partition: None,
	}];

	let name = MigrationNamer::generate_name(&operations, false);

	assert_eq!(name, "users", "Single operation should use its fragment");
}

#[test]
fn test_migration_naming_with_multiple_operations() {
	let operations = vec![
		Operation::AddColumn {
			table: leak_str("users").to_string(),
			column: ColumnDefinition::new("email", FieldType::VarChar(255)),
			mysql_options: None,
		},
		Operation::AddColumn {
			table: leak_str("users").to_string(),
			column: ColumnDefinition::new("phone", FieldType::VarChar(20)),
			mysql_options: None,
		},
	];

	let name = MigrationNamer::generate_name(&operations, false);

	assert_eq!(
		name, "users_email_users_phone",
		"Multiple operations should join fragments with underscore"
	);
}

#[test]
fn test_migration_naming_initial() {
	let operations = vec![];
	let name = MigrationNamer::generate_name(&operations, true);

	assert_eq!(
		name, "initial",
		"Initial migration should be named 'initial'"
	);
}

#[test]
fn test_migration_naming_with_run_sql() {
	let operations = vec![Operation::RunSQL {
		sql: leak_str("CREATE TRIGGER update_timestamp ...").to_string(),
		reverse_sql: None,
	}];

	let name = MigrationNamer::generate_name(&operations, false);

	assert!(
		name.starts_with("auto_"),
		"RunSQL should trigger auto-naming: got {}",
		name
	);
	assert!(
		name.len() > 5,
		"Auto-generated name should include timestamp: got {}",
		name
	);
}

#[test]
fn test_full_migration_name_generation() {
	let temp_dir = setup_migrations_dir();
	let migrations_dir = temp_dir.path().join("migrations");

	// Generate migration number
	let number = MigrationNumbering::next_number(&migrations_dir, "myapp");

	// Generate migration name
	let operations = vec![Operation::AddColumn {
		table: leak_str("users").to_string(),
		column: ColumnDefinition::new("status", FieldType::VarChar(20)),
		mysql_options: None,
	}];
	let name = MigrationNamer::generate_name(&operations, false);

	// Combine to create full migration filename
	let full_name = format!("{}_{}.rs", number, name);

	assert_eq!(
		full_name, "0004_users_status.rs",
		"Full migration name should combine number and name"
	);
}

#[test]
fn test_migration_struct_with_generated_name() {
	let operations = vec![
		Operation::CreateTable {
			name: leak_str("posts").to_string(),
			columns: vec![
				ColumnDefinition::new("id", FieldType::Integer),
				ColumnDefinition::new("title", FieldType::Text),
			],
			constraints: vec![],
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		},
		Operation::CreateIndex {
			table: leak_str("posts").to_string(),
			columns: vec!["title"],
			unique: false,
		},
	];

	let migration_name = MigrationNamer::generate_name(&operations, false);

	// Create migration with generated name
	let migration = Migration::from_parts(leak_str(format!("0001_{}", migration_name)).to_string(), leak_str("blog").to_string(), operations.clone(), vec![], vec![], true, Some(true), false, false, Vec::new(), Vec::new());

	assert_eq!(
		migration.name, "0001_posts_create_index_posts",
		"Migration should use generated name"
	);
	assert_eq!(
		migration.operations.len(),
		2,
		"Migration should contain all operations"
	);
}

#[test]
fn test_migration_numbering_get_all_numbers() {
	let temp_dir = setup_migrations_dir();
	let migrations_dir = temp_dir.path().join("migrations");

	// Add another app
	let app2_dir = migrations_dir.join("otherapp");
	fs::create_dir_all(&app2_dir).expect("Failed to create app2 directory");
	fs::write(app2_dir.join("0001_initial.rs"), "").expect("Failed to write file");
	fs::write(app2_dir.join("0002_add_field.rs"), "").expect("Failed to write file");

	// Get all numbers
	let all_numbers = MigrationNumbering::get_all_numbers(&migrations_dir);

	assert_eq!(all_numbers.len(), 2, "Should have 2 apps");
	assert_eq!(
		all_numbers.get("myapp"),
		Some(&3),
		"myapp should have highest number 3"
	);
	assert_eq!(
		all_numbers.get("otherapp"),
		Some(&2),
		"otherapp should have highest number 2"
	);
}

#[test]
fn test_migration_naming_truncation() {
	// Create operations that will generate a very long name
	let mut operations = Vec::new();
	for i in 0..20 {
		operations.push(Operation::AddColumn {
			table: leak_str(format!("table_{}", i)),
			column: ColumnDefinition::new(
				leak_str(format!("field_{}", i)),
				FieldType::VarChar(255),
			),
		});
	}

	let name = MigrationNamer::generate_name(&operations, false);

	assert!(
		name.len() <= 52,
		"Migration name should not exceed 52 characters, got {} chars",
		name.len()
	);
	assert!(
		name.ends_with("_and_more"),
		"Long migration name should end with '_and_more', got: {}",
		name
	);
}

#[test]
fn test_migration_operation_describe_for_logging() {
	let operations = [
		Operation::CreateTable {
			name: leak_str("users").to_string(),
			columns: vec![],
			constraints: vec![],
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		},
		Operation::AddColumn {
			table: leak_str("posts").to_string(),
			column: ColumnDefinition::new("author_id", FieldType::Integer),
			mysql_options: None,
		},
		Operation::CreateIndex {
			table: leak_str("posts").to_string(),
			columns: vec!["author_id"],
			unique: false,
		},
	];

	let descriptions: Vec<String> = operations.iter().map(|op| op.describe()).collect();

	assert_eq!(descriptions.len(), 3, "Should have 3 descriptions");
	assert_eq!(
		descriptions[0], "Create table users",
		"First operation description"
	);
	assert_eq!(
		descriptions[1], "Add column author_id to posts",
		"Second operation description"
	);
	assert_eq!(
		descriptions[2], "Create index on posts",
		"Third operation description"
	);
}

#[test]
fn test_migration_number_format_consistency() {
	let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
	let migrations_dir = temp_dir.path().join("migrations");
	let app_dir = migrations_dir.join("testapp");
	fs::create_dir_all(&app_dir).expect("Failed to create app directory");

	// Test various numbers
	for i in 0..100 {
		let migration_file = format!("{:04}_test.rs", i);
		fs::write(app_dir.join(&migration_file), "").expect("Failed to write file");

		let next = MigrationNumbering::next_number(&migrations_dir, "testapp");
		let expected = format!("{:04}", i + 1);

		assert_eq!(
			next,
			expected,
			"Next number should be {:04} after {:04}",
			i + 1,
			i
		);
	}
}

#[test]
fn test_combined_workflow_new_migration() {
	// Simulate the full workflow of creating a new migration
	let temp_dir = setup_migrations_dir();
	let migrations_dir = temp_dir.path().join("migrations");
	let app_label = "myapp";

	// Step 1: Define operations
	let operations = vec![
		Operation::CreateTable {
			name: leak_str("products").to_string(),
			columns: vec![
				ColumnDefinition::new("id", FieldType::Integer),
				ColumnDefinition::new("name", FieldType::Text),
				ColumnDefinition::new(
					"price",
					FieldType::Decimal {
						precision: 10,
						scale: 2,
					},
				),
			],
			constraints: vec![Constraint::Check {
				name: "check_price_positive".to_string(),
				expression: "price >= 0".to_string(),
			}],
		},
		Operation::CreateIndex {
			table: leak_str("products").to_string(),
			columns: vec!["name"],
			unique: false,
		},
	];

	// Step 2: Generate migration number
	let migration_number = MigrationNumbering::next_number(&migrations_dir, app_label);

	// Step 3: Generate migration name
	let migration_name = MigrationNamer::generate_name(&operations, false);

	// Step 4: Create full filename
	let full_filename = format!("{}_{}.rs", migration_number, migration_name);

	// Step 5: Create Migration struct
	let migration = Migration::from_parts(leak_str(format!("{}_{}", migration_number, migration_name)).to_string(), leak_str(app_label).to_string(), operations.clone(), vec![("myapp", "0003_remove_user_age")], vec![], true, Some(false), false, false, Vec::new(), Vec::new());

	// Assertions
	assert_eq!(
		migration_number, "0004",
		"Should be the 4th migration in sequence"
	);
	assert_eq!(
		migration_name, "products_create_index_products",
		"Should combine operation fragments"
	);
	assert_eq!(
		full_filename, "0004_products_create_index_products.rs",
		"Should create proper filename"
	);
	assert_eq!(
		migration.name, "0004_products_create_index_products",
		"Migration name should match"
	);
	assert_eq!(
		migration.dependencies.len(),
		1,
		"Should have one dependency"
	);
	assert_eq!(migration.operations.len(), 2, "Should have two operations");
}

#[test]
fn test_migration_naming_consistency_with_case() {
	// Test that case doesn't affect consistency
	let ops1 = vec![Operation::CreateTable {
		name: leak_str("Users").to_string(),
		columns: vec![],
		constraints: vec![],
		without_rowid: None,
		interleave_in_parent: None,
		partition: None,
	}];

	let ops2 = vec![Operation::CreateTable {
		name: leak_str("users").to_string(),
		columns: vec![],
		constraints: vec![],
		without_rowid: None,
		interleave_in_parent: None,
		partition: None,
	}];

	let name1 = MigrationNamer::generate_name(&ops1, false);
	let name2 = MigrationNamer::generate_name(&ops2, false);

	assert_eq!(
		name1, name2,
		"Names should be identical regardless of input case"
	);
	assert_eq!(name1, "users", "Should be lowercase");
}
