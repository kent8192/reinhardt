//! SQLite-specific edge case tests
//!
//! Tests SQLite table recreation scenarios:
//! - Foreign key constraint preservation (EC-DB-01)
//! - Data preservation during recreation (EC-DB-02)
//!
//! SQLite has limited ALTER TABLE support, requiring table recreation
//! for some schema changes. This module verifies that constraints and
//! data are properly preserved during the recreation process.
//!
//! **Test Coverage:**
//! - EC-DB-01: Table recreation with FK - Test SQLite table recreation preserves FK constraints
//! - EC-DB-02: Table recreation with data - Test data preservation (1000+ rows) during recreation
//!
//! **Fixtures Used:**
//! - sqlite_db: In-memory SQLite connection (via reinhardt_db::DatabaseConnection)

use reinhardt_db::backends::connection::DatabaseConnection;
use reinhardt_db::migrations::{
	ColumnDefinition, FieldType, ForeignKeyAction, Migration,
	executor::DatabaseMigrationExecutor,
	operations::{Constraint, Operation},
};
use rstest::*;
use std::sync::Arc;

// ============================================================================
// Test Helper Functions
// ============================================================================

/// Create a basic column definition
fn create_column(name: &str, type_def: FieldType) -> ColumnDefinition {
	ColumnDefinition {
		name: name.to_string(),
		type_definition: type_def,
		not_null: false,
		unique: false,
		primary_key: false,
		auto_increment: false,
		default: None,
	}
}

/// Create a primary key column with auto-increment
fn create_pk_column(name: &str) -> ColumnDefinition {
	ColumnDefinition {
		name: name.to_string(),
		type_definition: FieldType::Integer,
		not_null: true,
		unique: false,
		primary_key: true,
		auto_increment: true,
		default: None,
	}
}

/// Create a NOT NULL column
fn create_required_column(name: &str, type_def: FieldType) -> ColumnDefinition {
	ColumnDefinition {
		name: name.to_string(),
		type_definition: type_def,
		not_null: true,
		unique: false,
		primary_key: false,
		auto_increment: false,
		default: None,
	}
}

/// Create a test migration
fn create_test_migration(app: &str, name: &str, operations: Vec<Operation>) -> Migration {
	Migration {
		app_label: app.to_string(),
		name: name.to_string(),
		operations,
		dependencies: vec![],
		replaces: vec![],
		atomic: true,
		initial: None,
		state_only: false,
		database_only: false,
		swappable_dependencies: vec![],
		optional_dependencies: vec![],
	}
}

// ============================================================================
// Fixtures
// ============================================================================

/// Create SQLite database with parent-child tables for FK testing
#[fixture]
pub async fn sqlite_with_fk_tables() -> (Arc<DatabaseConnection>, DatabaseMigrationExecutor) {
	let connection = DatabaseConnection::connect_sqlite("sqlite::memory:")
		.await
		.expect("Failed to connect to in-memory SQLite");
	let conn = Arc::new(connection.clone());

	let mut executor = DatabaseMigrationExecutor::new(connection);

	// Create parent table
	let create_parent = create_test_migration(
		"testapp",
		"0001_create_parent",
		vec![Operation::CreateTable {
			name: "edge_parent".to_string(),
			columns: vec![
				create_pk_column("id"),
				create_required_column("name", FieldType::Text),
			],
			constraints: vec![],
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		}],
	);

	// Create child table with FK
	let create_child = create_test_migration(
		"testapp",
		"0002_create_child",
		vec![Operation::CreateTable {
			name: "edge_child".to_string(),
			columns: vec![
				create_pk_column("id"),
				create_column("parent_id", FieldType::Integer),
				create_column("value", FieldType::Text),
			],
			constraints: vec![Constraint::ForeignKey {
				name: "fk_child_parent".to_string(),
				columns: vec!["parent_id".to_string()],
				referenced_table: "edge_parent".to_string(),
				referenced_columns: vec!["id".to_string()],
				on_delete: ForeignKeyAction::Cascade,
				on_update: ForeignKeyAction::NoAction,
				deferrable: None,
			}],
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		}],
	);

	executor
		.apply_migrations(&[create_parent, create_child])
		.await
		.expect("Failed to create FK tables");

	(conn, executor)
}

/// Create SQLite database with table for data preservation testing
#[fixture]
pub async fn sqlite_with_data_table() -> (Arc<DatabaseConnection>, DatabaseMigrationExecutor) {
	let connection = DatabaseConnection::connect_sqlite("sqlite::memory:")
		.await
		.expect("Failed to connect to in-memory SQLite");
	let conn = Arc::new(connection.clone());

	let mut executor = DatabaseMigrationExecutor::new(connection);

	// Create table for large dataset testing
	let create_table = create_test_migration(
		"testapp",
		"0001_create_data_table",
		vec![Operation::CreateTable {
			name: "edge_data".to_string(),
			columns: vec![
				create_pk_column("id"),
				create_column("name", FieldType::Text),
				create_column("description", FieldType::Text),
				create_column("extra", FieldType::Text),
			],
			constraints: vec![],
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		}],
	);

	executor
		.apply_migrations(&[create_table])
		.await
		.expect("Failed to create data table");

	(conn, executor)
}

// ============================================================================
// EC-DB-01: Table Recreation with FK
// ============================================================================

/// Test: EC-DB-01 - Table recreation preserves foreign key constraints
///
/// Category: Edge Case - Foreign Key Preservation
/// Verifies that SQLite table recreation properly preserves foreign key
/// constraints when a table requiring recreation has FK dependencies.
#[rstest]
#[tokio::test]
async fn ec_db_01_table_recreation_preserves_fk(
	#[future] sqlite_with_fk_tables: (Arc<DatabaseConnection>, DatabaseMigrationExecutor),
) {
	let (conn, mut executor) = sqlite_with_fk_tables.await;

	// Arrange
	// Insert valid parent-child data
	conn.execute(
		"INSERT INTO edge_parent (name) VALUES ('Parent1')",
		vec![],
	)
	.await
	.expect("Failed to insert parent");

	conn.execute(
		"INSERT INTO edge_child (parent_id, value) VALUES (1, 'Child1')",
		vec![],
	)
	.await
	.expect("Failed to insert child");

	// Get FK constraint info before recreation
	let fk_info_before = conn
		.fetch_all("PRAGMA foreign_key_list(edge_child)", vec![])
		.await
		.expect("Failed to get FK info before recreation");

	assert!(!fk_info_before.is_empty(), "FK constraint should exist before recreation");

	// Act
	// Apply DROP COLUMN operation that triggers table recreation
	let drop_column = create_test_migration(
		"testapp",
		"0003_drop_value_column",
		vec![Operation::DropColumn {
			table: "edge_child".to_string(),
			column: "value".to_string(),
		}],
	);

	executor
		.apply_migrations(&[drop_column])
		.await
		.expect("Table recreation should succeed");

	// Assert
	// Verify FK constraint is preserved after recreation
	let fk_info_after = conn
		.fetch_all("PRAGMA foreign_key_list(edge_child)", vec![])
		.await
		.expect("Failed to get FK info after recreation");

	assert!(
		!fk_info_after.is_empty(),
		"FK constraint should exist after recreation"
	);

	// Verify FK constraint properties are preserved
	let fk_before: Vec<String> = fk_info_before
		.iter()
		.map(|row| {
			format!(
				"{:?}:{:?}->{:?}:{:?}",
				row.get::<String>("table").unwrap_or_default(),
				row.get::<String>("from").unwrap_or_default(),
				row.get::<String>("to").unwrap_or_default(),
				row.get::<String>("on_update").unwrap_or_default()
			)
		})
		.collect();

	let fk_after: Vec<String> = fk_info_after
		.iter()
		.map(|row| {
			format!(
				"{:?}:{:?}->{:?}:{:?}",
				row.get::<String>("table").unwrap_or_default(),
				row.get::<String>("from").unwrap_or_default(),
				row.get::<String>("to").unwrap_or_default(),
				row.get::<String>("on_update").unwrap_or_default()
			)
		})
		.collect();

	assert_eq!(
		fk_before, fk_after,
		"FK constraint properties should match before and after recreation"
	);

	// Verify FK constraint is still enforced
	conn.execute("PRAGMA foreign_keys = ON", vec![])
		.await
		.expect("Failed to enable FK checks");

	// Attempt to insert orphan record should fail
	let orphan_result = conn
		.execute(
			"INSERT INTO edge_child (parent_id) VALUES (999)",
			vec![],
		)
		.await;

	assert!(
		orphan_result.is_err(),
		"FK constraint should be enforced after recreation - orphan insert should fail"
	);

	// Verify original data is preserved
	let count: i64 = conn
		.fetch_one("SELECT COUNT(*) as count FROM edge_child", vec![])
		.await
		.expect("Failed to count rows")
		.get("count")
		.unwrap_or_default();

	assert_eq!(count, 1, "Original data should be preserved after recreation");

	let parent_id: i64 = conn
		.fetch_one("SELECT parent_id FROM edge_child", vec![])
		.await
		.expect("Failed to fetch parent_id")
		.get("parent_id")
		.unwrap_or_default();

	assert_eq!(parent_id, 1, "parent_id should match original value");
}

/// Test: EC-DB-01 - Multiple FK constraints preserved during recreation
///
/// Category: Edge Case - Multiple Foreign Keys
/// Verifies that all FK constraints are preserved when a table
/// with multiple FK constraints undergoes recreation.
#[rstest]
#[tokio::test]
async fn ec_db_01_multiple_fk_constraints_preserved() {
	let connection = DatabaseConnection::connect_sqlite("sqlite::memory:")
		.await
		.expect("Failed to connect to in-memory SQLite");
	let conn = Arc::new(connection.clone());
	let mut executor = DatabaseMigrationExecutor::new(connection);

	// Create parent tables
	let create_parent1 = create_test_migration(
		"testapp",
		"0001_parent1",
		vec![Operation::CreateTable {
			name: "parent1".to_string(),
			columns: vec![
				create_pk_column("id"),
				create_column("name", FieldType::Text),
			],
			constraints: vec![],
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		}],
	);

	let create_parent2 = create_test_migration(
		"testapp",
		"0002_parent2",
		vec![Operation::CreateTable {
			name: "parent2".to_string(),
			columns: vec![
				create_pk_column("id"),
				create_column("value", FieldType::Text),
			],
			constraints: vec![],
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		}],
	);

	// Create child with multiple FKs
	let create_child = create_test_migration(
		"testapp",
		"0003_child",
		vec![Operation::CreateTable {
			name: "multi_fk_child".to_string(),
			columns: vec![
				create_pk_column("id"),
				create_column("parent1_id", FieldType::Integer),
				create_column("parent2_id", FieldType::Integer),
				create_column("extra", FieldType::Text),
			],
			constraints: vec![
				Constraint::ForeignKey {
					name: "fk_parent1".to_string(),
					columns: vec!["parent1_id".to_string()],
					referenced_table: "parent1".to_string(),
					referenced_columns: vec!["id".to_string()],
					on_delete: ForeignKeyAction::Cascade,
					on_update: ForeignKeyAction::NoAction,
					deferrable: None,
				},
				Constraint::ForeignKey {
					name: "fk_parent2".to_string(),
					columns: vec!["parent2_id".to_string()],
					referenced_table: "parent2".to_string(),
					referenced_columns: vec!["id".to_string()],
					on_delete: ForeignKeyAction::SetNull,
					on_update: ForeignKeyAction::NoAction,
					deferrable: None,
				},
			],
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		}],
	);

	executor
		.apply_migrations(&[create_parent1, create_parent2, create_child])
		.await
		.expect("Failed to create tables with multiple FKs");

	// Get FK count before recreation
	let fk_count_before: i64 = conn
		.fetch_one(
			"SELECT COUNT(*) as count FROM pragma_foreign_key_list('multi_fk_child')",
			vec![],
		)
		.await
		.expect("Failed to count FKs")
		.get("count")
		.unwrap_or_default();

	assert_eq!(fk_count_before, 2, "Should have 2 FK constraints before recreation");

	// Drop column to trigger recreation
	let drop_extra = create_test_migration(
		"testapp",
		"0004_drop_extra",
		vec![Operation::DropColumn {
			table: "multi_fk_child".to_string(),
			column: "extra".to_string(),
		}],
	);

	executor
		.apply_migrations(&[drop_extra])
		.await
		.expect("Recreation with multiple FKs should succeed");

	// Verify both FK constraints preserved
	let fk_count_after: i64 = conn
		.fetch_one(
			"SELECT COUNT(*) as count FROM pragma_foreign_key_list('multi_fk_child')",
			vec![],
		)
		.await
		.expect("Failed to count FKs")
		.get("count")
		.unwrap_or_default();

	assert_eq!(
		fk_count_after, 2,
		"Both FK constraints should be preserved after recreation"
	);
}

// ============================================================================
// EC-DB-02: Table Recreation with Data
// ============================================================================

/// Test: EC-DB-02 - Data preservation with 1000+ rows during recreation
///
/// Category: Edge Case - Large Dataset Preservation
/// Verifies that all data is preserved when a table with 1000+ rows
/// undergoes SQLite table recreation.
#[rstest]
#[tokio::test]
async fn ec_db_02_data_preservation_with_1000_rows(
	#[future] sqlite_with_data_table: (Arc<DatabaseConnection>, DatabaseMigrationExecutor),
) {
	let (conn, mut executor) = sqlite_with_data_table.await;

	// Arrange
	let row_count = 1000;

	// Insert 1000 rows using batch operations
	for batch in 0..10 {
		let mut values = vec![];
		for i in (batch * 100)..((batch + 1) * 100) {
			values.push(format!(
				"('Name{}', 'Description{}', 'Extra{}')",
				i, i, i
			));
		}
		let insert_sql = format!(
			"INSERT INTO edge_data (name, description, extra) VALUES {}",
			values.join(",")
		);
		conn.execute(&insert_sql, vec![])
			.await
			.expect("Failed to insert batch of rows");
	}

	// Verify row count before recreation
	let count_before: i64 = conn
		.fetch_one("SELECT COUNT(*) as count FROM edge_data", vec![])
		.await
		.expect("Failed to count rows before recreation")
		.get("count")
		.unwrap_or_default();

	assert_eq!(count_before, row_count, "Should have {} rows before recreation", row_count);

	// Calculate expected sum of ids for verification
	let sum_before: i64 = conn
		.fetch_one("SELECT SUM(id) as sum_id FROM edge_data", vec![])
		.await
		.expect("Failed to calculate sum before recreation")
		.get("sum_id")
		.unwrap_or_default();

	// Act
	// Apply DROP COLUMN that triggers table recreation
	let drop_column = create_test_migration(
		"testapp",
		"0002_drop_extra_column",
		vec![Operation::DropColumn {
			table: "edge_data".to_string(),
			column: "extra".to_string(),
		}],
	);

	executor
		.apply_migrations(&[drop_column])
		.await
		.expect("Table recreation with 1000+ rows should succeed");

	// Assert
	// Verify row count preserved
	let count_after: i64 = conn
		.fetch_one("SELECT COUNT(*) as count FROM edge_data", vec![])
		.await
		.expect("Failed to count rows after recreation")
		.get("count")
		.unwrap_or_default();

	assert_eq!(
		count_after, row_count,
		"All {} rows should be preserved after recreation",
		row_count
	);

	// Verify data integrity by checking sum of ids
	let sum_after: i64 = conn
		.fetch_one("SELECT SUM(id) as sum_id FROM edge_data", vec![])
		.await
		.expect("Failed to calculate sum after recreation")
		.get("sum_id")
		.unwrap_or_default();

	assert_eq!(
		sum_after, sum_before,
		"Sum of ids should match - data integrity verified"
	);

	// Verify sample data at different positions
	let samples = vec![1i64, 500, 1000];
	for sample_id in samples {
		let row = conn
			.fetch_one(
				&format!("SELECT name, description FROM edge_data WHERE id = {}", sample_id),
				vec![],
			)
			.await
			.expect(&format!("Failed to fetch sample row {}", sample_id));

		let name: String = row.get("name").unwrap_or_default();
		let expected_name = format!("Name{}", sample_id - 1);
		assert_eq!(
			name, expected_name,
			"Sample row {} should have correct name",
			sample_id
		);
	}
}

/// Test: EC-DB-02 - Data preservation with 2000+ rows during recreation
///
/// Category: Edge Case - Large Dataset (2000+ rows)
/// Verifies data preservation with an even larger dataset to ensure
/// the recreation logic scales properly.
#[rstest]
#[tokio::test]
async fn ec_db_02_data_preservation_with_2000_rows() {
	let connection = DatabaseConnection::connect_sqlite("sqlite::memory:")
		.await
		.expect("Failed to connect to in-memory SQLite");
	let conn = Arc::new(connection.clone());
	let mut executor = DatabaseMigrationExecutor::new(connection);

	// Create table
	let create_table = create_test_migration(
		"testapp",
		"0001_create",
		vec![Operation::CreateTable {
			name: "large_data".to_string(),
			columns: vec![
				create_pk_column("id"),
				create_column("field_a", FieldType::Text),
				create_column("field_b", FieldType::Text),
				create_column("field_c", FieldType::Integer),
			],
			constraints: vec![],
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		}],
	);

	executor
		.apply_migrations(&[create_table])
		.await
		.expect("Failed to create table");

	// Arrange - Insert 2000 rows
	let row_count = 2000;
	for batch in 0..20 {
		let mut values = vec![];
		for i in (batch * 100)..((batch + 1) * 100) {
			values.push(format!("('A{}', 'B{}', {})", i, i, i * 10));
		}
		let insert_sql = format!(
			"INSERT INTO large_data (field_a, field_b, field_c) VALUES {}",
			values.join(",")
		);
		conn.execute(&insert_sql, vec![])
			.await
			.expect("Failed to insert batch");
	}

	// Verify initial row count
	let count_before: i64 = conn
		.fetch_one("SELECT COUNT(*) as count FROM large_data", vec![])
		.await
		.expect("Failed to count")
		.get("count")
		.unwrap_or_default();

	assert_eq!(count_before, row_count);

	// Act - Drop column via recreation
	let drop_column = create_test_migration(
		"testapp",
		"0002_drop_field_c",
		vec![Operation::DropColumn {
			table: "large_data".to_string(),
			column: "field_c".to_string(),
		}],
	);

	executor
		.apply_migrations(&[drop_column])
		.await
		.expect("Recreation with 2000 rows should succeed");

	// Assert - Verify all rows preserved
	let count_after: i64 = conn
		.fetch_one("SELECT COUNT(*) as count FROM large_data", vec![])
		.await
		.expect("Failed to count")
		.get("count")
		.unwrap_or_default();

	assert_eq!(count_after, row_count, "All 2000 rows should be preserved");

	// Verify data integrity with multiple sample checks
	let test_ids = vec![1, 500, 1000, 1500, 2000];
	for test_id in test_ids {
		let row = conn
			.fetch_one(
				&format!("SELECT field_a, field_b FROM large_data WHERE id = {}", test_id),
				vec![],
			)
			.await
			.expect(&format!("Failed to fetch row {}", test_id));

		let field_a: String = row.get("field_a").unwrap_or_default();
		let field_b: String = row.get("field_b").unwrap_or_default();

		let expected_a = format!("A{}", test_id - 1);
		let expected_b = format!("B{}", test_id - 1);

		assert_eq!(field_a, expected_a, "field_a should match for row {}", test_id);
		assert_eq!(field_b, expected_b, "field_b should match for row {}", test_id);
	}
}

/// Test: EC-DB-02 - Data preservation with text containing special characters
///
/// Category: Edge Case - Special Characters in Data
/// Verifies that data with special characters (quotes, newlines, etc.)
/// is preserved during table recreation.
#[rstest]
#[tokio::test]
async fn ec_db_02_data_preservation_with_special_characters() {
	let connection = DatabaseConnection::connect_sqlite("sqlite::memory:")
		.await
		.expect("Failed to connect to in-memory SQLite");
	let conn = Arc::new(connection.clone());
	let mut executor = DatabaseMigrationExecutor::new(connection);

	// Create table
	let create_table = create_test_migration(
		"testapp",
		"0001_create",
		vec![Operation::CreateTable {
			name: "special_chars".to_string(),
			columns: vec![
				create_pk_column("id"),
				create_column("content", FieldType::Text),
				create_column("extra", FieldType::Text),
			],
			constraints: vec![],
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		}],
	);

	executor
		.apply_migrations(&[create_table])
		.await
		.expect("Failed to create table");

	// Arrange - Insert data with special characters
	let test_data = vec![
		"Text with 'single quotes'",
		"Text with \"double quotes\"",
		"Text with\nnewline",
		"Text with\ttab",
		"Text with \\ backslash",
		"Text with ; semicolon",
		"Text with -- comment",
		"Text with /* comment */",
		"Text with 'mix\"ed'quotes",
		"Text with $$ dollar signs $$",
	];

	for (i, content) in test_data.iter().enumerate() {
		conn.execute(
			"INSERT INTO special_chars (content, extra) VALUES (?, ?)",
			vec![content.to_string(), format!("extra{}", i)],
		)
		.await
		.expect("Failed to insert special char data");
	}

	// Act - Drop column via recreation
	let drop_extra = create_test_migration(
		"testapp",
		"0002_drop_extra",
		vec![Operation::DropColumn {
			table: "special_chars".to_string(),
			column: "extra".to_string(),
		}],
	);

	executor
		.apply_migrations(&[drop_extra])
		.await
		.expect("Recreation should preserve special characters");

	// Assert - Verify all special character data preserved
	let count_after: i64 = conn
		.fetch_one("SELECT COUNT(*) as count FROM special_chars", vec![])
		.await
		.expect("Failed to count")
		.get("count")
		.unwrap_or_default();

	assert_eq!(count_after, test_data.len() as i64, "All rows should be preserved");

	// Verify each special character entry
	for (i, expected_content) in test_data.iter().enumerate() {
		let row = conn
			.fetch_one(
				&format!("SELECT content FROM special_chars WHERE id = {}", i + 1),
				vec![],
			)
			.await
			.expect(&format!("Failed to fetch row {}", i + 1));

		let content: String = row.get("content").unwrap_or_default();
		assert_eq!(
			content, *expected_content,
			"Special character content should match for row {}",
			i + 1
		);
	}
}

/// Test: EC-DB-02 - Data preservation with NULL values during recreation
///
/// Category: Edge Case - NULL Values
/// Verifies that NULL values are properly preserved during recreation.
#[rstest]
#[tokio::test]
async fn ec_db_02_data_preservation_with_nulls() {
	let connection = DatabaseConnection::connect_sqlite("sqlite::memory:")
		.await
		.expect("Failed to connect to in-memory SQLite");
	let conn = Arc::new(connection.clone());
	let mut executor = DatabaseMigrationExecutor::new(connection);

	// Create table with nullable columns
	let create_table = create_test_migration(
		"testapp",
		"0001_create",
		vec![Operation::CreateTable {
			name: "null_test".to_string(),
			columns: vec![
				create_pk_column("id"),
				create_column("col_a", FieldType::Text),
				create_column("col_b", FieldType::Text),
				create_column("col_c", FieldType::Integer),
			],
			constraints: vec![],
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		}],
	);

	executor
		.apply_migrations(&[create_table])
		.await
		.expect("Failed to create table");

	// Arrange - Insert rows with various NULL combinations
	let test_cases = vec![
		("Value1", "Value2", 10),   // No NULLs
		("Value3", NULL, 20),        // col_b is NULL
		(NULL, "Value4", 30),        // col_a is NULL
		(NULL, NULL, 40),            // Both NULL
		("Value5", "Value6", NULL),  // col_c is NULL
	];

	for (col_a, col_b, col_c) in &test_cases {
		let sql = if col_a.is_some() && col_b.is_some() && col_c.is_some() {
			format!(
				"INSERT INTO null_test (col_a, col_b, col_c) VALUES ('{}', '{}', {})",
				col_a.unwrap(), col_b.unwrap(), col_c.unwrap()
			)
		} else {
			let mut cols = vec![];
			let mut vals = vec![];

			if let Some(v) = col_a {
				cols.push("col_a");
				vals.push(format!("'{}'", v));
			}
			if let Some(v) = col_b {
				cols.push("col_b");
				vals.push(format!("'{}'", v));
			}
			if let Some(v) = col_c {
				cols.push("col_c");
				vals.push(format!("{}", v));
			}

			format!(
				"INSERT INTO null_test ({}) VALUES ({})",
				cols.join(", "),
				vals.join(", ")
			)
		};

		conn.execute(&sql, vec![])
			.await
			.expect("Failed to insert test case");
	}

	// Act - Drop column via recreation
	let drop_col_c = create_test_migration(
		"testapp",
		"0002_drop_col_c",
		vec![Operation::DropColumn {
			table: "null_test".to_string(),
			column: "col_c".to_string(),
		}],
	);

	executor
		.apply_migrations(&[drop_col_c])
		.await
		.expect("Recreation with NULLs should succeed");

	// Assert - Verify NULL values preserved
	let rows = conn
		.fetch_all("SELECT col_a, col_b FROM null_test ORDER BY id", vec![])
		.await
		.expect("Failed to fetch rows");

	assert_eq!(rows.len(), test_cases.len(), "All rows should be preserved");

	// Verify each row's NULL values are preserved
	for (i, (expected_a, expected_b, _)) in test_cases.iter().enumerate() {
		let col_a: Option<String> = rows[i].get("col_a").ok();
		let col_b: Option<String> = rows[i].get("col_b").ok();

		assert_eq!(
			col_a.as_deref(),
			expected_a.map(|s| s.as_str()),
			"col_a NULL status should match for row {}",
			i + 1
		);
		assert_eq!(
			col_b.as_deref(),
			expected_b.map(|s| s.as_str()),
			"col_b NULL status should match for row {}",
			i + 1
		);
	}
}
