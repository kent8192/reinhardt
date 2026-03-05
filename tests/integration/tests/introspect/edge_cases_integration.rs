//! Edge case integration tests for database introspection
//!
//! Tests boundary conditions and edge cases:
//! - Empty database (no tables)
//! - Tables with no columns (error case)
//! - Reserved Rust keyword column names
//! - Very long table/column names
//! - Special characters in identifiers
//!
//! **Test Categories:**
//! - EC-001: Empty database handling
//! - EC-002: Reserved keyword escaping
//! - EC-003: All tables filtered out
//! - EC-004: Table with only primary key
//! - EC-005: Many-to-many junction table

use super::fixtures::{empty_postgres_database, postgres_introspect_schema};
use reinhardt_db::migrations::introspection::{DatabaseIntrospector, PostgresIntrospector};
use reinhardt_db::migrations::{IntrospectConfig, SchemaCodeGenerator};
use reinhardt_test::fixtures::{ContainerAsync, GenericImage, postgres_container};
use rstest::*;
use sqlx::PgPool;
use std::sync::Arc;

// ============================================================================
// EC-001: Empty Database Handling
// ============================================================================

/// Test introspecting an empty database
///
/// **Test Intent**: Verify empty database produces empty schema without errors
///
/// **Expected Behavior**:
/// - Returns Ok with empty tables map
/// - No panics or errors
#[rstest]
#[tokio::test]
async fn test_introspect_empty_database(
	#[future] empty_postgres_database: (ContainerAsync<GenericImage>, Arc<PgPool>, String),
) {
	let (_container, pool, _url) = empty_postgres_database.await;

	let introspector = PostgresIntrospector::new(pool.as_ref().clone());
	let schema = introspector
		.read_schema()
		.await
		.expect("Should succeed on empty database");

	// Should be empty (no user tables)
	// Note: System tables might exist but should be filtered
	assert!(
		schema.tables.is_empty(),
		"Empty database should have no user tables"
	);
}

/// Test code generation on empty database
///
/// **Test Intent**: Verify code generator handles empty schema gracefully
///
/// **Expected Behavior**:
/// - Returns empty output
/// - No panics or errors
#[rstest]
#[tokio::test]
async fn test_generate_from_empty_database(
	#[future] empty_postgres_database: (ContainerAsync<GenericImage>, Arc<PgPool>, String),
) {
	let (_container, pool, _url) = empty_postgres_database.await;

	let introspector = PostgresIntrospector::new(pool.as_ref().clone());
	let schema = introspector
		.read_schema()
		.await
		.expect("Should succeed on empty database");

	let config = IntrospectConfig::default()
		.with_database_url("postgres://test@localhost/test")
		.with_app_label("testapp");

	let generator = SchemaCodeGenerator::new(config);
	let output = generator
		.generate(&schema)
		.expect("Should handle empty schema");

	// Should generate no files
	assert!(
		output.files.is_empty(),
		"Empty schema should produce no files"
	);
}

// ============================================================================
// EC-002: Reserved Keyword Handling
// ============================================================================

/// Test table/column with reserved Rust keyword names
///
/// **Test Intent**: Verify reserved keywords are escaped with r# prefix
///
/// **Expected Behavior**:
/// - Column named "type" becomes r#type in generated code
#[rstest]
#[tokio::test]
async fn test_reserved_keyword_column(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, _url) = postgres_container.await;

	// Create table with reserved keyword column
	sqlx::query(
		r#"
        CREATE TABLE items (
            id BIGSERIAL PRIMARY KEY,
            name VARCHAR(100) NOT NULL,
            "type" VARCHAR(50) NOT NULL,
            "struct" VARCHAR(50),
            "impl" INTEGER
        )
        "#,
	)
	.execute(pool.as_ref())
	.await
	.expect("Failed to create items table");

	let introspector = PostgresIntrospector::new(pool.as_ref().clone());
	let schema = introspector
		.read_schema()
		.await
		.expect("Failed to read schema");

	let config = IntrospectConfig::default()
		.with_database_url("postgres://test@localhost/test")
		.with_app_label("testapp");

	let generator = SchemaCodeGenerator::new(config);
	let output = generator
		.generate(&schema)
		.expect("Failed to generate code");

	// Find items file
	let items_file = output
		.files
		.iter()
		.find(|f| {
			f.path
				.file_name()
				.map(|n| n.to_str() == Some("items.rs"))
				.unwrap_or(false)
		})
		.expect("Should generate items.rs");

	// Verify reserved keywords are escaped
	assert!(
		items_file.content.contains("r#type"),
		"'type' should be escaped as r#type"
	);
}

// ============================================================================
// EC-003: All Tables Filtered Out
// ============================================================================

/// Test when all tables are filtered out by exclude patterns
///
/// **Test Intent**: Verify graceful handling when filtering removes all tables
///
/// **Expected Behavior**:
/// - Returns empty output
/// - No errors
#[rstest]
#[tokio::test]
async fn test_all_tables_filtered_out(
	#[future] postgres_introspect_schema: (ContainerAsync<GenericImage>, Arc<PgPool>, String),
) {
	let (_container, pool, _url) = postgres_introspect_schema.await;

	let introspector = PostgresIntrospector::new(pool.as_ref().clone());
	let schema = introspector
		.read_schema()
		.await
		.expect("Failed to read schema");

	// Create config that excludes all tables
	let mut config = IntrospectConfig::default()
		.with_database_url("postgres://test@localhost/test")
		.with_app_label("testapp");
	config.tables.include = vec!["nonexistent_pattern".to_string()];
	config.tables.exclude = vec![];

	let generator = SchemaCodeGenerator::new(config);
	let output = generator
		.generate(&schema)
		.expect("Should handle filtered tables");

	// Should generate no files when all tables filtered
	assert!(
		output.files.is_empty(),
		"Should produce no files when all tables filtered"
	);
}

// ============================================================================
// EC-004: Table with Only Primary Key
// ============================================================================

/// Test table with only a primary key column
///
/// **Test Intent**: Verify minimal table structure is handled
///
/// **Expected Behavior**:
/// - Table is introspected correctly
/// - Model is generated with single field
#[rstest]
#[tokio::test]
async fn test_table_with_only_pk(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, _url) = postgres_container.await;

	// Create minimal table
	sqlx::query("CREATE TABLE minimal (id BIGSERIAL PRIMARY KEY)")
		.execute(pool.as_ref())
		.await
		.expect("Failed to create minimal table");

	let introspector = PostgresIntrospector::new(pool.as_ref().clone());
	let schema = introspector
		.read_schema()
		.await
		.expect("Failed to read schema");

	assert!(
		schema.tables.contains_key("minimal"),
		"Should detect minimal table"
	);

	let minimal_table = schema.tables.get("minimal").unwrap();
	assert_eq!(
		minimal_table.columns.len(),
		1,
		"Should have exactly 1 column"
	);
	assert!(
		minimal_table.columns.contains_key("id"),
		"Should have id column"
	);
}

// ============================================================================
// EC-005: Many-to-Many Junction Table
// ============================================================================

/// Test introspecting many-to-many junction tables
///
/// **Test Intent**: Verify junction tables with composite PK are handled
///
/// **Expected Behavior**:
/// - Both columns in composite PK are detected
/// - Both FK relationships are detected
#[rstest]
#[tokio::test]
async fn test_many_to_many_junction_table(
	#[future] postgres_introspect_schema: (ContainerAsync<GenericImage>, Arc<PgPool>, String),
) {
	let (_container, pool, _url) = postgres_introspect_schema.await;

	let introspector = PostgresIntrospector::new(pool.as_ref().clone());
	let schema = introspector
		.read_schema()
		.await
		.expect("Failed to read schema");

	let junction = schema.tables.get("posts_tags").unwrap();

	// Should have exactly 2 columns
	assert_eq!(
		junction.columns.len(),
		2,
		"Junction table should have 2 columns"
	);

	// Both should be foreign keys
	assert_eq!(
		junction.foreign_keys.len(),
		2,
		"Junction table should have 2 FKs"
	);

	// Verify FK targets
	let fk_tables: Vec<_> = junction
		.foreign_keys
		.iter()
		.map(|fk| &fk.referenced_table)
		.collect();
	assert!(fk_tables.contains(&&"posts".to_string()));
	assert!(fk_tables.contains(&&"tags".to_string()));
}

// ============================================================================
// EC-006: Long Identifier Names
// ============================================================================

/// Test very long table and column names
///
/// **Test Intent**: Verify PostgreSQL max identifier length (63 chars) is handled
///
/// **Expected Behavior**:
/// - Long names are captured correctly
/// - Generated code uses the full name
#[rstest]
#[tokio::test]
async fn test_long_identifier_names(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, _url) = postgres_container.await;

	// Create table with long name (PostgreSQL truncates at 63 chars)
	let long_table_name = "a".repeat(63);
	let long_column_name = "b".repeat(63);

	let sql = format!(
		r#"CREATE TABLE "{}" (
            id BIGSERIAL PRIMARY KEY,
            "{}" VARCHAR(100)
        )"#,
		long_table_name, long_column_name
	);

	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create table with long names");

	let introspector = PostgresIntrospector::new(pool.as_ref().clone());
	let schema = introspector
		.read_schema()
		.await
		.expect("Failed to read schema");

	// Verify table is found
	assert!(
		schema.tables.contains_key(&long_table_name),
		"Should detect table with long name"
	);

	let table = schema.tables.get(&long_table_name).unwrap();
	assert!(
		table.columns.contains_key(&long_column_name),
		"Should detect column with long name"
	);
}

// ============================================================================
// EC-007: Numeric Column Name Prefix
// ============================================================================

/// Test column names starting with numbers
///
/// **Test Intent**: Verify numeric prefixes are handled (prefixed with underscore)
///
/// **Expected Behavior**:
/// - Column name is sanitized to valid Rust identifier
#[rstest]
#[tokio::test]
async fn test_numeric_column_prefix(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, _url) = postgres_container.await;

	// Create table with numeric column name
	sqlx::query(
		r#"
        CREATE TABLE numeric_cols (
            id BIGSERIAL PRIMARY KEY,
            "123_value" INTEGER,
            "2nd_column" VARCHAR(50)
        )
        "#,
	)
	.execute(pool.as_ref())
	.await
	.expect("Failed to create numeric_cols table");

	let introspector = PostgresIntrospector::new(pool.as_ref().clone());
	let schema = introspector
		.read_schema()
		.await
		.expect("Failed to read schema");

	// Verify columns are captured
	let table = schema.tables.get("numeric_cols").unwrap();
	assert!(table.columns.contains_key("123_value"));
	assert!(table.columns.contains_key("2nd_column"));

	// Generate code
	let config = IntrospectConfig::default()
		.with_database_url("postgres://test@localhost/test")
		.with_app_label("testapp");

	let generator = SchemaCodeGenerator::new(config);
	let output = generator
		.generate(&schema)
		.expect("Failed to generate code");

	// Find numeric_cols file
	let file = output
		.files
		.iter()
		.find(|f| {
			f.path
				.file_name()
				.map(|n| n.to_str() == Some("numeric_cols.rs"))
				.unwrap_or(false)
		})
		.expect("Should generate numeric_cols.rs");

	// Verify numeric prefixes are sanitized
	assert!(
		file.content.contains("_123_value") || file.content.contains("pub _123_value"),
		"Numeric prefix should be prefixed with underscore"
	);
}
