//! Database execution tests for composite primary key migrations
//!
//! Tests actual DDL execution on PostgreSQL for composite primary key tables.

use reinhardt_db::backends::schema::BaseDatabaseSchemaEditor;
use reinhardt_db::migrations::FieldType;
use reinhardt_db::migrations::operations::FieldDefinition;
use reinhardt_db::migrations::operations::models::CreateModel;
use reinhardt_test::fixtures::postgres_container;
use rstest::*;
use serial_test::serial;
use sqlx::{PgPool, Row};
use std::sync::Arc;
use testcontainers::{ContainerAsync, GenericImage};

// ============================================================================
// Test 1: CREATE TABLE execution for composite PK in PostgreSQL
// ============================================================================

#[rstest]
#[serial(composite_pk_db)]
#[tokio::test]
async fn test_composite_pk_create_table_execution_postgresql(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	// Test intent: Execute CREATE TABLE with composite PK on real PostgreSQL database
	// Not intent: SQL generation verification, error cases, data manipulation
	let (_container, pool, _port, _url) = postgres_container.await;

	// Create migration operation
	let create_model = CreateModel::new(
		"post_tags",
		vec![
			FieldDefinition::new("post_id", FieldType::BigInteger, true, false, None::<&str>),
			FieldDefinition::new("tag_id", FieldType::BigInteger, true, false, None::<&str>),
			FieldDefinition::new(
				"description",
				FieldType::VarChar(200),
				false,
				false,
				None::<&str>,
			),
		],
	)
	.with_composite_primary_key(vec!["post_id".to_string(), "tag_id".to_string()])
	.expect("Valid composite primary key");

	// Create mock schema editor (migrations use database_forwards which returns SQL)
	struct MockEditor;

	#[async_trait::async_trait]
	impl BaseDatabaseSchemaEditor for MockEditor {
		async fn execute(
			&mut self,
			_sql: &str,
		) -> Result<(), reinhardt_db::backends::schema::SchemaEditorError> {
			Ok(())
		}

		fn database_type(&self) -> reinhardt_db::backends::DatabaseType {
			reinhardt_db::backends::DatabaseType::Postgres
		}
	}

	let schema_editor = MockEditor;

	// Generate SQL
	let sql_statements = create_model.database_forwards(&schema_editor);
	assert_eq!(sql_statements.len(), 1);
	let create_table_sql = &sql_statements[0];

	// Execute CREATE TABLE on real database
	sqlx::query(create_table_sql)
		.execute(&*pool)
		.await
		.expect("Failed to execute CREATE TABLE");

	// Verify table exists
	let table_check = sqlx::query(
		"SELECT table_name FROM information_schema.tables
         WHERE table_schema = 'public' AND table_name = 'post_tags'",
	)
	.fetch_one(&*pool)
	.await;

	assert!(table_check.is_ok(), "Table should exist");
	let row = table_check.unwrap();
	let table_name: String = row.get("table_name");
	assert_eq!(table_name, "post_tags");

	// Verify composite primary key constraint exists
	let pk_check = sqlx::query(
		"SELECT constraint_name, constraint_type
         FROM information_schema.table_constraints
         WHERE table_name = 'post_tags' AND constraint_type = 'PRIMARY KEY'",
	)
	.fetch_one(&*pool)
	.await;

	assert!(pk_check.is_ok(), "Primary key constraint should exist");
	let pk_row = pk_check.unwrap();
	let constraint_type: String = pk_row.get("constraint_type");
	assert_eq!(constraint_type, "PRIMARY KEY");

	// Verify columns are NOT NULL
	let column_check = sqlx::query(
		"SELECT column_name, is_nullable
         FROM information_schema.columns
         WHERE table_name = 'post_tags' AND column_name IN ('post_id', 'tag_id')
         ORDER BY column_name",
	)
	.fetch_all(&*pool)
	.await
	.expect("Failed to query columns");

	assert_eq!(column_check.len(), 2);
	for row in &column_check {
		let is_nullable: String = row.get("is_nullable");
		assert_eq!(is_nullable, "NO", "Composite PK fields should be NOT NULL");
	}
}

// ============================================================================
// Test 2: Data insertion and retrieval with composite PK
// ============================================================================

#[rstest]
#[serial(composite_pk_db)]
#[tokio::test]
async fn test_composite_pk_data_insertion_with_real_db(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	// Test intent: Insert and query data in composite PK table on real database
	// Not intent: DDL execution, NULL handling, FK constraints
	let (_container, pool, _port, _url) = postgres_container.await;

	// Create table
	sqlx::query(
		"CREATE TABLE user_roles (
            user_id BIGINT NOT NULL,
            role_id BIGINT NOT NULL,
            granted_by VARCHAR(100),
            PRIMARY KEY (user_id, role_id)
        )",
	)
	.execute(&*pool)
	.await
	.expect("Failed to create table");

	// Insert test data
	sqlx::query(
		"INSERT INTO user_roles (user_id, role_id, granted_by) VALUES
         (1, 10, 'admin'),
         (1, 20, 'admin'),
         (2, 10, 'manager')",
	)
	.execute(&*pool)
	.await
	.expect("Failed to insert data");

	// Query single record by composite PK
	let result = sqlx::query(
		"SELECT user_id, role_id, granted_by
         FROM user_roles
         WHERE user_id = $1 AND role_id = $2",
	)
	.bind(1_i64)
	.bind(10_i64)
	.fetch_one(&*pool)
	.await;

	assert!(result.is_ok(), "Query should succeed");
	let row = result.unwrap();
	let user_id: i64 = row.get("user_id");
	let role_id: i64 = row.get("role_id");
	let granted_by: Option<String> = row.get("granted_by");

	assert_eq!(user_id, 1);
	assert_eq!(role_id, 10);
	assert_eq!(granted_by, Some("admin".to_string()));

	// Verify composite PK uniqueness constraint
	let duplicate_insert = sqlx::query(
		"INSERT INTO user_roles (user_id, role_id, granted_by) VALUES (1, 10, 'duplicate')",
	)
	.execute(&*pool)
	.await;

	assert!(
		duplicate_insert.is_err(),
		"Duplicate composite PK should fail"
	);

	// Verify all records can be queried
	let all_records =
		sqlx::query("SELECT user_id, role_id FROM user_roles ORDER BY user_id, role_id")
			.fetch_all(&*pool)
			.await
			.expect("Failed to fetch all records");

	assert_eq!(all_records.len(), 3);
}

// ============================================================================
// Test 3: Foreign key reference to composite PK table
// ============================================================================

#[rstest]
#[serial(composite_pk_db)]
#[tokio::test]
async fn test_composite_pk_foreign_key_reference_integration(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	// Test intent: Create FK referencing composite PK table and verify integrity
	// Not intent: Complex FK cascades, NULL FK values, circular references
	let (_container, pool, _port, _url) = postgres_container.await;

	// Create parent table with composite PK
	sqlx::query(
		"CREATE TABLE order_items (
            order_id BIGINT NOT NULL,
            item_id BIGINT NOT NULL,
            quantity INTEGER NOT NULL DEFAULT 1,
            PRIMARY KEY (order_id, item_id)
        )",
	)
	.execute(&*pool)
	.await
	.expect("Failed to create order_items table");

	// Create child table with FK to composite PK
	sqlx::query(
		"CREATE TABLE order_item_notes (
            note_id SERIAL PRIMARY KEY,
            order_id BIGINT NOT NULL,
            item_id BIGINT NOT NULL,
            note_text TEXT NOT NULL,
            FOREIGN KEY (order_id, item_id) REFERENCES order_items(order_id, item_id)
        )",
	)
	.execute(&*pool)
	.await
	.expect("Failed to create order_item_notes table");

	// Insert parent record
	sqlx::query("INSERT INTO order_items (order_id, item_id, quantity) VALUES (100, 1, 5)")
		.execute(&*pool)
		.await
		.expect("Failed to insert parent record");

	// Insert child record with valid FK
	let valid_insert = sqlx::query(
		"INSERT INTO order_item_notes (order_id, item_id, note_text)
         VALUES (100, 1, 'Urgent delivery')",
	)
	.execute(&*pool)
	.await;

	assert!(valid_insert.is_ok(), "Valid FK insert should succeed");

	// Attempt insert with invalid FK
	let invalid_insert = sqlx::query(
		"INSERT INTO order_item_notes (order_id, item_id, note_text)
         VALUES (999, 999, 'Invalid reference')",
	)
	.execute(&*pool)
	.await;

	assert!(
		invalid_insert.is_err(),
		"Invalid FK insert should fail with referential integrity violation"
	);

	// Verify FK constraint exists
	let fk_check = sqlx::query(
		"SELECT constraint_name, constraint_type
         FROM information_schema.table_constraints
         WHERE table_name = 'order_item_notes' AND constraint_type = 'FOREIGN KEY'",
	)
	.fetch_one(&*pool)
	.await;

	assert!(fk_check.is_ok(), "FK constraint should exist");
	let fk_row = fk_check.unwrap();
	let constraint_type: String = fk_row.get("constraint_type");
	assert_eq!(constraint_type, "FOREIGN KEY");

	// Query joined data
	let joined = sqlx::query(
		"SELECT oi.order_id, oi.item_id, oi.quantity, oin.note_text
         FROM order_items oi
         JOIN order_item_notes oin ON oi.order_id = oin.order_id AND oi.item_id = oin.item_id
         WHERE oi.order_id = 100",
	)
	.fetch_one(&*pool)
	.await;

	assert!(joined.is_ok(), "JOIN query should succeed");
	let row = joined.unwrap();
	let note_text: String = row.get("note_text");
	assert_eq!(note_text, "Urgent delivery");
}
