//! SQL Generation Validation Integration Tests
//!
//! Tests that verify the correctness of generated SQL strings for various migration operations.
//! Covers CREATE TABLE, ALTER TABLE, constraints, indexes, and edge cases.
//!
//! **Test Coverage:**
//! - CREATE TABLE syntax validation (basic, with DEFAULT, with composite PK)
//! - ALTER TABLE operations (ADD COLUMN, ALTER COLUMN, DROP COLUMN, RENAME)
//! - Constraint syntax (FOREIGN KEY, UNIQUE, CHECK)
//! - Index syntax (CREATE INDEX, CREATE UNIQUE INDEX, DROP INDEX)
//! - Edge cases (SQL reserved words, special characters, long identifiers)
//!
//! **Fixtures Used:**
//! - postgres_container: PostgreSQL database container
//! - mysql_container: MySQL database container
//!
//! **Test Strategy:**
//! 1. Generate SQL using Operation::to_sql()
//! 2. Validate syntax with string assertions
//! 3. Validate syntax with regex patterns
//! 4. Execute SQL and verify with information_schema

use regex::Regex;
use reinhardt_db::backends::DatabaseConnection;
use reinhardt_db::migrations::{
	ColumnDefinition, Constraint, FieldType, ForeignKeyAction, Migration, Operation,
	executor::DatabaseMigrationExecutor, operations::SqlDialect,
};
use reinhardt_test::fixtures::postgres_container;
use rstest::*;
use sqlx::PgPool;
use std::sync::Arc;
use testcontainers::{ContainerAsync, GenericImage};

// ============================================================================
// Test Helper Functions
// ============================================================================

fn leak_str(s: impl Into<String>) -> &'static str {
	Box::leak(s.into().into_boxed_str())
}

/// Create a simple migration for testing
fn create_test_migration(
	app: &'static str,
	name: &'static str,
	operations: Vec<Operation>,
) -> Migration {
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

/// Create a basic column definition
fn create_basic_column(name: &str, type_def: FieldType) -> ColumnDefinition {
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

/// Create a NOT NULL column definition
fn create_not_null_column(name: &str, type_def: FieldType) -> ColumnDefinition {
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

/// Create a column with DEFAULT value
fn create_column_with_default(name: &str, type_def: FieldType, default: &str) -> ColumnDefinition {
	ColumnDefinition {
		name: name.to_string(),
		type_definition: type_def,
		not_null: false,
		unique: false,
		primary_key: false,
		auto_increment: false,
		default: Some(default.to_string()),
	}
}

/// Create an auto-increment primary key column
fn create_auto_pk_column(name: &str, type_def: FieldType) -> ColumnDefinition {
	ColumnDefinition {
		name: name.to_string(),
		type_definition: type_def,
		not_null: true,
		unique: false,
		primary_key: true,
		auto_increment: true,
		default: None,
	}
}

// ============================================================================
// CREATE TABLE Syntax Tests
// ============================================================================

/// Test basic CREATE TABLE syntax validation
///
/// **Test Intent**: Verify CREATE TABLE generates correct basic syntax
///
/// **Validation Points**:
/// - SQL starts with "CREATE TABLE"
/// - Table name is present
/// - Column definitions are properly formatted
/// - Statement ends with semicolon
#[rstest]
#[tokio::test]
async fn test_create_table_basic_syntax() {
	let operation = Operation::CreateTable {
		name: leak_str("users").to_string(),
		columns: vec![
			create_auto_pk_column("id", FieldType::Integer),
			create_not_null_column("name", FieldType::VarChar(100)),
			create_basic_column("email", FieldType::VarChar(255)),
		],
		constraints: vec![],
		without_rowid: None,
		interleave_in_parent: None,
		partition: None,
	};

	// PostgreSQL
	let pg_sql = operation.to_sql(&SqlDialect::Postgres);
	assert!(pg_sql.contains("CREATE TABLE"), "Missing CREATE TABLE");
	assert!(pg_sql.contains("users"), "Missing table name");
	assert!(pg_sql.contains("id"), "Missing id column");
	assert!(pg_sql.contains("name"), "Missing name column");
	assert!(pg_sql.contains("email"), "Missing email column");
	assert!(
		pg_sql.contains("VARCHAR(100)") || pg_sql.contains("VARCHAR"),
		"Missing VARCHAR type"
	);
	assert!(pg_sql.ends_with(");"), "Statement should end with );");

	// Verify regex pattern for proper structure
	let re = Regex::new(r#"CREATE TABLE\s+(?:IF NOT EXISTS\s+)?"?users"?\s*\("#).unwrap();
	assert!(re.is_match(&pg_sql), "CREATE TABLE structure mismatch");
}

/// Test CREATE TABLE with DEFAULT values
///
/// **Test Intent**: Verify DEFAULT clause syntax for various value types
///
/// **Validation Points**:
/// - String defaults are properly quoted
/// - Numeric defaults are unquoted
/// - Function calls (NOW(), CURRENT_TIMESTAMP) are unquoted
#[rstest]
#[tokio::test]
async fn test_create_table_with_default_values() {
	let operation = Operation::CreateTable {
		name: leak_str("products").to_string(),
		columns: vec![
			create_auto_pk_column("id", FieldType::Integer),
			create_column_with_default("name", FieldType::VarChar(100), "'Unnamed'"),
			create_column_with_default(
				"price",
				FieldType::Decimal {
					precision: 10,
					scale: 2,
				},
				"0.00",
			),
			create_column_with_default("is_active", FieldType::Boolean, "TRUE"),
			create_column_with_default("created_at", FieldType::DateTime, "CURRENT_TIMESTAMP"),
		],
		constraints: vec![],
		without_rowid: None,
		interleave_in_parent: None,
		partition: None,
	};

	let pg_sql = operation.to_sql(&SqlDialect::Postgres);

	// Verify DEFAULT clauses
	assert!(
		pg_sql.contains("DEFAULT 'Unnamed'") || pg_sql.contains("DEFAULT ''Unnamed''"),
		"String default should be quoted"
	);
	assert!(
		pg_sql.contains("DEFAULT 0.00"),
		"Numeric default should be unquoted"
	);
	assert!(
		pg_sql.contains("DEFAULT TRUE") || pg_sql.contains("DEFAULT true"),
		"Boolean default present"
	);
	assert!(
		pg_sql.contains("DEFAULT CURRENT_TIMESTAMP"),
		"Function default should be unquoted"
	);
}

/// Test CREATE TABLE with composite primary key
///
/// **Test Intent**: Verify composite PK generates CONSTRAINT PRIMARY KEY (col1, col2)
///
/// **Validation Points**:
/// - Individual columns do NOT have PRIMARY KEY constraint
/// - Composite PRIMARY KEY constraint is at table level
/// - Columns in composite PK are NOT NULL
#[rstest]
#[tokio::test]
async fn test_create_table_with_composite_primary_key() {
	let operation = Operation::CreateTable {
		name: leak_str("order_items").to_string(),
		columns: vec![
			ColumnDefinition {
				name: "order_id".to_string(),
				type_definition: FieldType::BigInteger,
				not_null: true,
				unique: false,
				primary_key: true,
				auto_increment: false,
				default: None,
			},
			ColumnDefinition {
				name: "item_id".to_string(),
				type_definition: FieldType::BigInteger,
				not_null: true,
				unique: false,
				primary_key: true,
				auto_increment: false,
				default: None,
			},
			create_basic_column("quantity", FieldType::Integer),
		],
		constraints: vec![],
		without_rowid: None,
		interleave_in_parent: None,
		partition: None,
	};

	let pg_sql = operation.to_sql(&SqlDialect::Postgres);

	// Verify composite PK
	assert!(
		pg_sql.contains("PRIMARY KEY (order_id, item_id)")
			|| pg_sql.contains("PRIMARY KEY(order_id, item_id)"),
		"Missing composite PRIMARY KEY constraint"
	);

	// Verify individual columns do NOT have PRIMARY KEY
	let re = Regex::new(r"order_id\s+BIGINT\s+NOT NULL\s+PRIMARY KEY").unwrap();
	assert!(
		!re.is_match(&pg_sql),
		"Individual column should not have PRIMARY KEY when using composite PK"
	);

	// Verify NOT NULL on composite PK columns
	assert!(
		pg_sql.contains("order_id") && pg_sql.contains("NOT NULL"),
		"Composite PK columns should be NOT NULL"
	);
}

// ============================================================================
// ALTER TABLE Syntax Tests
// ============================================================================

/// Test ALTER TABLE ADD COLUMN syntax
///
/// **Test Intent**: Verify ADD COLUMN generates correct syntax
///
/// **Validation Points**:
/// - PostgreSQL: ALTER TABLE ... ADD COLUMN
/// - MySQL: ALTER TABLE ... ADD COLUMN
/// - NOT NULL constraint positioning
#[rstest]
#[case::postgres(SqlDialect::Postgres)]
#[case::mysql(SqlDialect::Mysql)]
#[tokio::test]
async fn test_alter_table_add_column_syntax(#[case] dialect: SqlDialect) {
	let operation = Operation::AddColumn {
		table: leak_str("users").to_string(),
		column: create_not_null_column("age", FieldType::Integer),
		mysql_options: None,
	};

	let sql = operation.to_sql(&dialect);

	assert!(sql.contains("ALTER TABLE"), "Missing ALTER TABLE");
	assert!(sql.contains("users"), "Missing table name");
	assert!(
		sql.contains("ADD COLUMN") || sql.contains("ADD"),
		"Missing ADD COLUMN"
	);
	assert!(sql.contains("age"), "Missing column name");
	assert!(
		sql.contains("INTEGER") || sql.contains("INT"),
		"Missing column type"
	);
	assert!(sql.contains("NOT NULL"), "Missing NOT NULL constraint");
}

/// Test ALTER TABLE ALTER COLUMN TYPE syntax
///
/// **Test Intent**: Verify column type change syntax for different databases
///
/// **Validation Points**:
/// - PostgreSQL: ALTER TABLE ... ALTER COLUMN ... TYPE
/// - MySQL: ALTER TABLE ... MODIFY COLUMN
#[rstest]
#[tokio::test]
async fn test_alter_table_alter_column_type_syntax() {
	let operation = Operation::AlterColumn {
		table: leak_str("products").to_string(),
		column: leak_str("price").to_string(),
		old_definition: None,
		new_definition: ColumnDefinition {
			name: "price".to_string(),
			type_definition: FieldType::Decimal {
				precision: 12,
				scale: 2,
			},
			not_null: false,
			unique: false,
			primary_key: false,
			auto_increment: false,
			default: None,
		},
		mysql_options: None,
	};

	// PostgreSQL
	let pg_sql = operation.to_sql(&SqlDialect::Postgres);
	assert!(pg_sql.contains("ALTER TABLE"), "PG: Missing ALTER TABLE");
	assert!(pg_sql.contains("ALTER COLUMN"), "PG: Missing ALTER COLUMN");
	assert!(pg_sql.contains("TYPE"), "PG: Missing TYPE keyword");
	assert!(
		pg_sql.contains("DECIMAL") || pg_sql.contains("NUMERIC"),
		"PG: Missing DECIMAL type"
	);

	// MySQL
	let mysql_sql = operation.to_sql(&SqlDialect::Mysql);
	assert!(
		mysql_sql.contains("ALTER TABLE"),
		"MySQL: Missing ALTER TABLE"
	);
	assert!(
		mysql_sql.contains("MODIFY COLUMN") || mysql_sql.contains("MODIFY"),
		"MySQL: Missing MODIFY COLUMN"
	);
	assert!(mysql_sql.contains("DECIMAL"), "MySQL: Missing DECIMAL type");
}

/// Test ALTER TABLE DROP COLUMN syntax
///
/// **Test Intent**: Verify DROP COLUMN generates correct syntax
#[rstest]
#[case::postgres(SqlDialect::Postgres)]
#[case::mysql(SqlDialect::Mysql)]
#[tokio::test]
async fn test_alter_table_drop_column_syntax(#[case] dialect: SqlDialect) {
	let operation = Operation::DropColumn {
		table: leak_str("users").to_string(),
		column: leak_str("middle_name").to_string(),
	};

	let sql = operation.to_sql(&dialect);

	assert!(sql.contains("ALTER TABLE"), "Missing ALTER TABLE");
	assert!(sql.contains("users"), "Missing table name");
	assert!(
		sql.contains("DROP COLUMN") || sql.contains("DROP"),
		"Missing DROP COLUMN"
	);
	assert!(sql.contains("middle_name"), "Missing column name");
}

/// Test ALTER TABLE RENAME COLUMN syntax
///
/// **Test Intent**: Verify RENAME COLUMN generates correct syntax
#[rstest]
#[case::postgres(SqlDialect::Postgres)]
#[case::mysql(SqlDialect::Mysql)]
#[tokio::test]
async fn test_alter_table_rename_column_syntax(#[case] dialect: SqlDialect) {
	let operation = Operation::RenameColumn {
		table: leak_str("users").to_string(),
		old_name: leak_str("username").to_string(),
		new_name: leak_str("login_name").to_string(),
	};

	let sql = operation.to_sql(&dialect);

	assert!(sql.contains("ALTER TABLE"), "Missing ALTER TABLE");
	assert!(sql.contains("users"), "Missing table name");
	assert!(
		sql.contains("RENAME") || sql.contains("CHANGE"),
		"Missing RENAME/CHANGE"
	);
	assert!(sql.contains("username"), "Missing old column name");
	assert!(sql.contains("login_name"), "Missing new column name");
}

// ============================================================================
// Constraint Syntax Tests
// ============================================================================

/// Test FOREIGN KEY constraint syntax
///
/// **Test Intent**: Verify FOREIGN KEY constraint with ON DELETE/UPDATE actions
///
/// **Validation Points**:
/// - CONSTRAINT ... FOREIGN KEY syntax
/// - REFERENCES clause
/// - ON DELETE CASCADE/RESTRICT/SET NULL
/// - ON UPDATE actions
#[rstest]
#[tokio::test]
async fn test_foreign_key_constraint_syntax() {
	let operation = Operation::CreateTable {
		name: leak_str("orders").to_string(),
		columns: vec![
			create_auto_pk_column("id", FieldType::Integer),
			create_not_null_column("user_id", FieldType::Integer),
		],
		constraints: vec![Constraint::ForeignKey {
			name: "fk_orders_user".to_string(),
			columns: vec!["user_id".to_string()],
			referenced_table: "users".to_string(),
			referenced_columns: vec!["id".to_string()],
			on_delete: ForeignKeyAction::Cascade,
			on_update: ForeignKeyAction::Restrict,
			deferrable: None,
		}],
		without_rowid: None,
		interleave_in_parent: None,
		partition: None,
	};

	let pg_sql = operation.to_sql(&SqlDialect::Postgres);

	// Verify FOREIGN KEY syntax
	assert!(pg_sql.contains("FOREIGN KEY"), "Missing FOREIGN KEY");
	assert!(pg_sql.contains("user_id"), "Missing FK column");
	assert!(pg_sql.contains("REFERENCES"), "Missing REFERENCES");
	assert!(pg_sql.contains("users"), "Missing referenced table");
	assert!(
		pg_sql.contains("ON DELETE CASCADE"),
		"Missing ON DELETE CASCADE"
	);
	assert!(
		pg_sql.contains("ON UPDATE RESTRICT") || pg_sql.contains("ON UPDATE NO ACTION"),
		"Missing ON UPDATE action"
	);
}

/// Test UNIQUE constraint syntax
///
/// **Test Intent**: Verify UNIQUE constraint generation
#[rstest]
#[tokio::test]
async fn test_unique_constraint_syntax() {
	let operation = Operation::CreateTable {
		name: leak_str("users").to_string(),
		columns: vec![
			create_auto_pk_column("id", FieldType::Integer),
			create_not_null_column("email", FieldType::VarChar(255)),
		],
		constraints: vec![Constraint::Unique {
			name: "uq_users_email".to_string(),
			columns: vec!["email".to_string()],
		}],
		without_rowid: None,
		interleave_in_parent: None,
		partition: None,
	};

	let pg_sql = operation.to_sql(&SqlDialect::Postgres);

	assert!(
		pg_sql.contains("UNIQUE") || pg_sql.contains("CONSTRAINT"),
		"Missing UNIQUE"
	);
	assert!(pg_sql.contains("email"), "Missing UNIQUE column");
}

/// Test CHECK constraint syntax
///
/// **Test Intent**: Verify CHECK constraint with complex expressions
#[rstest]
#[tokio::test]
async fn test_check_constraint_syntax() {
	let operation = Operation::CreateTable {
		name: leak_str("products").to_string(),
		columns: vec![
			create_auto_pk_column("id", FieldType::Integer),
			create_not_null_column(
				"price",
				FieldType::Decimal {
					precision: 10,
					scale: 2,
				},
			),
			create_basic_column(
				"discount",
				FieldType::Decimal {
					precision: 5,
					scale: 2,
				},
			),
		],
		constraints: vec![
			Constraint::Check {
				name: "chk_price_positive".to_string(),
				expression: "price > 0".to_string(),
			},
			Constraint::Check {
				name: "chk_discount_range".to_string(),
				expression: "discount >= 0 AND discount <= 100".to_string(),
			},
		],
		without_rowid: None,
		interleave_in_parent: None,
		partition: None,
	};

	let pg_sql = operation.to_sql(&SqlDialect::Postgres);

	assert!(pg_sql.contains("CHECK"), "Missing CHECK constraint");
	assert!(pg_sql.contains("price > 0"), "Missing CHECK expression");
	assert!(pg_sql.contains("AND"), "Missing complex CHECK expression");
}

// ============================================================================
// Index Syntax Tests
// ============================================================================

/// Test CREATE INDEX syntax
///
/// **Test Intent**: Verify basic index creation syntax
#[rstest]
#[case::postgres(SqlDialect::Postgres)]
#[case::mysql(SqlDialect::Mysql)]
#[tokio::test]
async fn test_create_index_syntax(#[case] dialect: SqlDialect) {
	let operation = Operation::CreateIndex {
		table: leak_str("users").to_string(),
		columns: vec![leak_str("email").to_string()],
		unique: false,
		index_type: None,
		where_clause: None,
		concurrently: false,
		expressions: None,
		mysql_options: None,
		operator_class: None,
	};

	let sql = operation.to_sql(&dialect);

	assert!(sql.contains("CREATE INDEX"), "Missing CREATE INDEX");
	assert!(sql.contains("idx_users_email"), "Missing index name");
	assert!(sql.contains("users"), "Missing table name");
	assert!(sql.contains("email"), "Missing column name");
}

/// Test CREATE UNIQUE INDEX syntax
///
/// **Test Intent**: Verify unique index creation syntax
#[rstest]
#[case::postgres(SqlDialect::Postgres)]
#[case::mysql(SqlDialect::Mysql)]
#[tokio::test]
async fn test_create_unique_index_syntax(#[case] dialect: SqlDialect) {
	let operation = Operation::CreateIndex {
		table: leak_str("users").to_string(),
		columns: vec![leak_str("username").to_string()],
		unique: true,
		index_type: None,
		where_clause: None,
		concurrently: false,
		expressions: None,
		mysql_options: None,
		operator_class: None,
	};

	let sql = operation.to_sql(&dialect);

	assert!(
		sql.contains("CREATE UNIQUE INDEX"),
		"Missing CREATE UNIQUE INDEX"
	);
	assert!(sql.contains("idx_users_username"), "Missing index name");
	assert!(sql.contains("username"), "Missing column name");
}

/// Test DROP INDEX syntax (database-specific)
///
/// **Test Intent**: Verify DROP INDEX syntax differences
///
/// **Validation Points**:
/// - PostgreSQL: DROP INDEX index_name;
/// - MySQL: DROP INDEX index_name ON table_name;
#[rstest]
#[tokio::test]
async fn test_drop_index_syntax() {
	let operation = Operation::DropIndex {
		table: leak_str("users").to_string(),
		columns: vec![leak_str("email").to_string()],
	};

	// PostgreSQL - no table name needed
	let pg_sql = operation.to_sql(&SqlDialect::Postgres);
	assert!(pg_sql.contains("DROP INDEX"), "PG: Missing DROP INDEX");
	assert!(pg_sql.contains("idx_users_email"), "PG: Missing index name");

	// MySQL - requires table name
	let mysql_sql = operation.to_sql(&SqlDialect::Mysql);
	assert!(
		mysql_sql.contains("DROP INDEX"),
		"MySQL: Missing DROP INDEX"
	);
	assert!(
		mysql_sql.contains("idx_users_email"),
		"MySQL: Missing index name"
	);
	assert!(
		mysql_sql.contains("ON users") || mysql_sql.contains("ON `users`"),
		"MySQL: Missing ON table_name"
	);
}

/// Test composite index syntax
///
/// **Test Intent**: Verify multi-column index syntax
#[rstest]
#[tokio::test]
async fn test_composite_index_syntax() {
	let operation = Operation::CreateIndex {
		table: leak_str("orders").to_string(),
		columns: vec![
			leak_str("user_id").to_string(),
			leak_str("created_at").to_string(),
		],
		unique: false,
		index_type: None,
		where_clause: None,
		concurrently: false,
		expressions: None,
		mysql_options: None,
		operator_class: None,
	};

	let pg_sql = operation.to_sql(&SqlDialect::Postgres);

	assert!(pg_sql.contains("CREATE INDEX"), "Missing CREATE INDEX");
	assert!(pg_sql.contains("user_id"), "Missing first column");
	assert!(pg_sql.contains("created_at"), "Missing second column");

	// Verify column order
	let user_id_pos = pg_sql.find("user_id").unwrap();
	let created_at_pos = pg_sql.find("created_at").unwrap();
	assert!(
		user_id_pos < created_at_pos,
		"Column order should be preserved"
	);
}

// ============================================================================
// Edge Case Tests
// ============================================================================

/// Test SQL reserved words are properly escaped
///
/// **Test Intent**: Verify reserved words (user, order, group) are quoted
///
/// **Validation Points**:
/// - Table names with reserved words are quoted
/// - Column names with reserved words are quoted
#[rstest]
#[tokio::test]
async fn test_sql_reserved_words_escaping() {
	let operation = Operation::CreateTable {
		name: leak_str("user").to_string(), // Reserved word in SQL
		columns: vec![
			create_auto_pk_column("id", FieldType::Integer),
			create_basic_column("order", FieldType::Integer), // Reserved word
			create_basic_column("group", FieldType::VarChar(50)), // Reserved word
		],
		constraints: vec![],
		without_rowid: None,
		interleave_in_parent: None,
		partition: None,
	};

	let pg_sql = operation.to_sql(&SqlDialect::Postgres);

	// PostgreSQL uses double quotes for identifiers
	assert!(
		pg_sql.contains("\"user\"") || pg_sql.contains("`user`"),
		"Reserved word 'user' should be quoted"
	);
	assert!(
		pg_sql.contains("\"order\"") || pg_sql.contains("`order`"),
		"Reserved word 'order' should be quoted"
	);
	assert!(
		pg_sql.contains("\"group\"") || pg_sql.contains("`group`"),
		"Reserved word 'group' should be quoted"
	);
}

/// Test special characters in identifiers are properly escaped
///
/// **Test Intent**: Verify special characters (spaces, hyphens) are handled
#[rstest]
#[tokio::test]
async fn test_special_characters_escaping() {
	let operation = Operation::CreateTable {
		name: leak_str("user-profile").to_string(), // Hyphen
		columns: vec![
			create_auto_pk_column("id", FieldType::Integer),
			create_basic_column("first-name", FieldType::VarChar(50)), // Hyphen
		],
		constraints: vec![],
		without_rowid: None,
		interleave_in_parent: None,
		partition: None,
	};

	let pg_sql = operation.to_sql(&SqlDialect::Postgres);

	// Special characters should trigger quoting
	assert!(
		pg_sql.contains("\"user-profile\"") || pg_sql.contains("`user-profile`"),
		"Hyphenated table name should be quoted"
	);
	assert!(
		pg_sql.contains("\"first-name\"") || pg_sql.contains("`first-name`"),
		"Hyphenated column name should be quoted"
	);
}

/// Test long identifier names (PostgreSQL: 63 chars, MySQL: 64 chars)
///
/// **Test Intent**: Verify long identifiers are handled or truncated
#[rstest]
#[tokio::test]
async fn test_long_identifier_names() {
	// PostgreSQL has 63-character limit for identifiers
	let long_name = "a".repeat(65); // Exceeds limit
	let operation = Operation::CreateTable {
		name: leak_str(long_name.clone()).to_string(),
		columns: vec![create_auto_pk_column("id", FieldType::Integer)],
		constraints: vec![],
		without_rowid: None,
		interleave_in_parent: None,
		partition: None,
	};

	let pg_sql = operation.to_sql(&SqlDialect::Postgres);

	// Should either truncate or include the long name
	// (Actual behavior depends on implementation - this test documents it)
	assert!(
		pg_sql.contains("CREATE TABLE"),
		"Should still generate CREATE TABLE"
	);
}

/// Test complex DEFAULT expressions (functions, concatenation)
///
/// **Test Intent**: Verify complex DEFAULT values are properly formatted
#[rstest]
#[tokio::test]
async fn test_complex_default_expressions() {
	let operation = Operation::CreateTable {
		name: leak_str("events").to_string(),
		columns: vec![
			create_auto_pk_column("id", FieldType::Integer),
			create_column_with_default("created_at", FieldType::TimestampTz, "NOW()"),
			create_column_with_default("uuid", FieldType::Uuid, "gen_random_uuid()"),
			create_column_with_default("counter", FieldType::Integer, "0"),
		],
		constraints: vec![],
		without_rowid: None,
		interleave_in_parent: None,
		partition: None,
	};

	let pg_sql = operation.to_sql(&SqlDialect::Postgres);

	// Function calls should not be quoted
	assert!(pg_sql.contains("DEFAULT NOW()"), "NOW() should be unquoted");
	assert!(
		pg_sql.contains("DEFAULT gen_random_uuid()"),
		"gen_random_uuid() should be unquoted"
	);
	assert!(
		pg_sql.contains("DEFAULT 0"),
		"Numeric defaults should be unquoted"
	);
}

// ============================================================================
// RENAME TABLE and DROP TABLE Tests
// ============================================================================

/// Test RENAME TABLE syntax
///
/// **Test Intent**: Verify table renaming syntax
#[rstest]
#[case::postgres(SqlDialect::Postgres)]
#[case::mysql(SqlDialect::Mysql)]
#[tokio::test]
async fn test_rename_table_syntax(#[case] dialect: SqlDialect) {
	let operation = Operation::RenameTable {
		old_name: leak_str("users_old").to_string(),
		new_name: leak_str("users").to_string(),
	};

	let sql = operation.to_sql(&dialect);

	assert!(
		sql.contains("ALTER TABLE") || sql.contains("RENAME TABLE"),
		"Missing RENAME statement"
	);
	assert!(sql.contains("users_old"), "Missing old table name");
	assert!(sql.contains("users"), "Missing new table name");
	assert!(sql.contains("RENAME"), "Missing RENAME keyword");
}

/// Test DROP TABLE syntax
///
/// **Test Intent**: Verify table deletion syntax
#[rstest]
#[case::postgres(SqlDialect::Postgres)]
#[case::mysql(SqlDialect::Mysql)]
#[case::sqlite(SqlDialect::Sqlite)]
#[tokio::test]
async fn test_drop_table_syntax(#[case] dialect: SqlDialect) {
	let operation = Operation::DropTable {
		name: leak_str("old_table").to_string(),
	};

	let sql = operation.to_sql(&dialect);

	assert!(sql.contains("DROP TABLE"), "Missing DROP TABLE");
	assert!(sql.contains("old_table"), "Missing table name");
}

// ============================================================================
// Composite Primary Key Tests
// ============================================================================

/// Test composite primary key SQL generation (PostgreSQL)
///
/// **Test Intent**: Verify that multiple columns with primary_key=true
/// generate correct CONSTRAINT PRIMARY KEY (col1, col2) syntax
///
/// **Validation Points**:
/// - SQL contains PRIMARY KEY constraint
/// - SQL lists all primary key columns in correct order
/// - PostgreSQL uses CONSTRAINT syntax
#[rstest]
#[tokio::test]
async fn test_composite_primary_key_syntax_postgres() {
	let operation = Operation::CreateTable {
		name: leak_str("order_items").to_string(),
		columns: vec![
			ColumnDefinition {
				name: "order_id".to_string(),
				type_definition: FieldType::BigInteger,
				not_null: true,
				unique: false,
				primary_key: true,
				auto_increment: false,
				default: None,
			},
			ColumnDefinition {
				name: "item_id".to_string(),
				type_definition: FieldType::BigInteger,
				not_null: true,
				unique: false,
				primary_key: true,
				auto_increment: false,
				default: None,
			},
			ColumnDefinition {
				name: "quantity".to_string(),
				type_definition: FieldType::Integer,
				not_null: true,
				unique: false,
				primary_key: false,
				auto_increment: false,
				default: Some("1".to_string()),
			},
		],
		constraints: vec![],
		without_rowid: None,
		interleave_in_parent: None,
		partition: None,
	};

	let sql = operation.to_sql(&SqlDialect::Postgres);

	// Verify CONSTRAINT PRIMARY KEY syntax
	assert!(
		sql.contains("PRIMARY KEY"),
		"Missing PRIMARY KEY constraint"
	);

	// Verify both columns are listed in PRIMARY KEY constraint
	let re = Regex::new(r#"PRIMARY KEY\s*\(\s*"?order_id"?\s*,\s*"?item_id"?\s*\)"#).unwrap();
	assert!(
		re.is_match(&sql),
		"Composite primary key syntax incorrect: {}",
		sql
	);

	// Verify columns are created as NOT NULL
	assert!(
		sql.contains("order_id") && sql.contains("NOT NULL"),
		"Primary key columns should be NOT NULL"
	);
}

/// Test composite primary key SQL generation (MySQL)
///
/// **Test Intent**: Verify MySQL-specific composite primary key syntax
#[rstest]
#[tokio::test]
async fn test_composite_primary_key_syntax_mysql() {
	let operation = Operation::CreateTable {
		name: leak_str("user_roles").to_string(),
		columns: vec![
			ColumnDefinition {
				name: "user_id".to_string(),
				type_definition: FieldType::Integer,
				not_null: true,
				unique: false,
				primary_key: true,
				auto_increment: false,
				default: None,
			},
			ColumnDefinition {
				name: "role_id".to_string(),
				type_definition: FieldType::Integer,
				not_null: true,
				unique: false,
				primary_key: true,
				auto_increment: false,
				default: None,
			},
		],
		constraints: vec![],
		without_rowid: None,
		interleave_in_parent: None,
		partition: None,
	};

	let sql = operation.to_sql(&SqlDialect::Mysql);

	// Verify PRIMARY KEY syntax
	assert!(
		sql.contains("PRIMARY KEY"),
		"Missing PRIMARY KEY constraint"
	);

	// Verify both columns in composite key
	let re = Regex::new(r#"PRIMARY KEY\s*\(\s*`?user_id`?\s*,\s*`?role_id`?\s*\)"#).unwrap();
	assert!(
		re.is_match(&sql),
		"Composite primary key syntax incorrect: {}",
		sql
	);
}

/// Test composite primary key database integration (PostgreSQL)
///
/// **Test Intent**: Verify composite primary key is correctly created in actual database
///
/// **Validation Points**:
/// - Table is created successfully
/// - Primary key constraint exists in information_schema
/// - Both columns are part of the primary key
/// - Uniqueness constraint works (duplicate inserts fail)
#[rstest]
#[tokio::test]
async fn test_composite_primary_key_postgres_integration(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, url) = postgres_container.await;

	let connection = DatabaseConnection::connect_postgres(&url)
		.await
		.expect("Failed to connect to PostgreSQL");

	let mut executor = DatabaseMigrationExecutor::new(connection.clone());

	// Create table with composite primary key
	let migration = create_test_migration(
		"testapp",
		"0001_create_composite_pk",
		vec![Operation::CreateTable {
			name: leak_str("enrollment").to_string(),
			columns: vec![
				ColumnDefinition {
					name: "student_id".to_string(),
					type_definition: FieldType::Integer,
					not_null: true,
					unique: false,
					primary_key: true,
					auto_increment: false,
					default: None,
				},
				ColumnDefinition {
					name: "course_id".to_string(),
					type_definition: FieldType::Integer,
					not_null: true,
					unique: false,
					primary_key: true,
					auto_increment: false,
					default: None,
				},
				ColumnDefinition {
					name: "enrolled_at".to_string(),
					type_definition: FieldType::DateTime,
					not_null: false,
					unique: false,
					primary_key: false,
					auto_increment: false,
					default: Some("CURRENT_TIMESTAMP".to_string()),
				},
			],
			constraints: vec![],
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		}],
	);

	executor
		.apply_migrations(&[migration])
		.await
		.expect("Failed to create table with composite PK");

	// Verify table exists
	let table_exists: bool = sqlx::query_scalar(
		"SELECT EXISTS(
			SELECT 1 FROM information_schema.tables
			WHERE table_name = $1
		)",
	)
	.bind("enrollment")
	.fetch_one(pool.as_ref())
	.await
	.expect("Failed to check table existence");

	assert!(table_exists, "Table should exist");

	// Verify primary key constraint exists
	let pk_exists: bool = sqlx::query_scalar(
		"SELECT EXISTS(
			SELECT 1 FROM information_schema.table_constraints
			WHERE table_name = $1 AND constraint_type = 'PRIMARY KEY'
		)",
	)
	.bind("enrollment")
	.fetch_one(pool.as_ref())
	.await
	.expect("Failed to check PK constraint");

	assert!(pk_exists, "Primary key constraint should exist");

	// Verify both columns are part of primary key
	let pk_columns: Vec<String> = sqlx::query_scalar(
		"SELECT column_name
		FROM information_schema.key_column_usage
		WHERE table_name = $1 AND constraint_name IN (
			SELECT constraint_name FROM information_schema.table_constraints
			WHERE table_name = $1 AND constraint_type = 'PRIMARY KEY'
		)
		ORDER BY ordinal_position",
	)
	.bind("enrollment")
	.fetch_all(pool.as_ref())
	.await
	.expect("Failed to get PK columns");

	assert_eq!(pk_columns.len(), 2, "Should have 2 primary key columns");
	assert_eq!(pk_columns[0], "student_id", "First PK column mismatch");
	assert_eq!(pk_columns[1], "course_id", "Second PK column mismatch");

	// Test uniqueness constraint - insert duplicate should fail
	sqlx::query("INSERT INTO enrollment (student_id, course_id) VALUES (1, 100)")
		.execute(pool.as_ref())
		.await
		.expect("First insert should succeed");

	let duplicate_result =
		sqlx::query("INSERT INTO enrollment (student_id, course_id) VALUES (1, 100)")
			.execute(pool.as_ref())
			.await;

	assert!(
		duplicate_result.is_err(),
		"Duplicate insert should fail due to composite PK constraint"
	);

	// Insert with different combination should succeed
	let different_insert =
		sqlx::query("INSERT INTO enrollment (student_id, course_id) VALUES (1, 101)")
			.execute(pool.as_ref())
			.await;

	assert!(
		different_insert.is_ok(),
		"Insert with different composite key should succeed"
	);
}

/// Test three-column composite primary key
///
/// **Test Intent**: Verify composite primary key works with 3+ columns
#[rstest]
#[tokio::test]
async fn test_composite_primary_key_three_columns(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, url) = postgres_container.await;

	let connection = DatabaseConnection::connect_postgres(&url)
		.await
		.expect("Failed to connect to PostgreSQL");

	let mut executor = DatabaseMigrationExecutor::new(connection.clone());

	// Create table with 3-column composite primary key
	let migration = create_test_migration(
		"testapp",
		"0001_create_three_col_pk",
		vec![Operation::CreateTable {
			name: leak_str("booking").to_string(),
			columns: vec![
				ColumnDefinition {
					name: "hotel_id".to_string(),
					type_definition: FieldType::Integer,
					not_null: true,
					unique: false,
					primary_key: true,
					auto_increment: false,
					default: None,
				},
				ColumnDefinition {
					name: "room_number".to_string(),
					type_definition: FieldType::Integer,
					not_null: true,
					unique: false,
					primary_key: true,
					auto_increment: false,
					default: None,
				},
				ColumnDefinition {
					name: "booking_date".to_string(),
					type_definition: FieldType::Date,
					not_null: true,
					unique: false,
					primary_key: true,
					auto_increment: false,
					default: None,
				},
				ColumnDefinition {
					name: "guest_name".to_string(),
					type_definition: FieldType::VarChar(200),
					not_null: true,
					unique: false,
					primary_key: false,
					auto_increment: false,
					default: None,
				},
			],
			constraints: vec![],
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		}],
	);

	executor
		.apply_migrations(&[migration])
		.await
		.expect("Failed to create table with 3-column composite PK");

	// Verify 3 columns are part of primary key
	let pk_columns: Vec<String> = sqlx::query_scalar(
		"SELECT column_name
		FROM information_schema.key_column_usage
		WHERE table_name = $1 AND constraint_name IN (
			SELECT constraint_name FROM information_schema.table_constraints
			WHERE table_name = $1 AND constraint_type = 'PRIMARY KEY'
		)
		ORDER BY ordinal_position",
	)
	.bind("booking")
	.fetch_all(pool.as_ref())
	.await
	.expect("Failed to get PK columns");

	assert_eq!(pk_columns.len(), 3, "Should have 3 primary key columns");
	assert_eq!(pk_columns[0], "hotel_id", "First PK column mismatch");
	assert_eq!(pk_columns[1], "room_number", "Second PK column mismatch");
	assert_eq!(pk_columns[2], "booking_date", "Third PK column mismatch");
}
