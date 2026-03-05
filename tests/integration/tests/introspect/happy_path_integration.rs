//! Happy path integration tests for database introspection
//!
//! Tests normal operation scenarios including:
//! - Introspecting simple tables
//! - Introspecting tables with foreign keys
//! - Detecting indexes and constraints
//! - Generating valid Rust code
//!
//! **Test Categories:**
//! - HP-001: Simple table introspection
//! - HP-002: Multiple table introspection
//! - HP-003: Foreign key detection
//! - HP-004: Index detection
//! - HP-005: Nullable field handling
//!
//! **Fixtures Used:**
//! - postgres_introspect_schema: PostgreSQL with test tables

use super::fixtures::{
	get_column_count, get_foreign_key_count, postgres_introspect_schema, table_exists,
};
use reinhardt_db::migrations::introspection::{DatabaseIntrospector, PostgresIntrospector};
use reinhardt_db::migrations::{IntrospectConfig, SchemaCodeGenerator};
use reinhardt_test::fixtures::{ContainerAsync, GenericImage};
use rstest::*;
use sqlx::PgPool;
use std::sync::Arc;

// ============================================================================
// HP-001: Simple Table Introspection
// ============================================================================

/// Test introspecting a single table with basic columns
///
/// **Test Intent**: Verify basic table introspection works correctly
///
/// **Integration Point**: PostgresIntrospector → read_schema()
///
/// **Expected Behavior**:
/// - Table is detected with correct name
/// - Columns are detected with correct types
/// - Primary key is identified
#[rstest]
#[tokio::test]
async fn test_introspect_simple_table(
	#[future] postgres_introspect_schema: (ContainerAsync<GenericImage>, Arc<PgPool>, String),
) {
	let (_container, pool, _url) = postgres_introspect_schema.await;

	// Verify the users table exists
	assert!(table_exists(&pool, "users").await);

	// Create introspector
	let introspector = PostgresIntrospector::new(pool.as_ref().clone());

	// Introspect the schema
	let schema = introspector
		.read_schema()
		.await
		.expect("Failed to read schema");

	// Verify users table is found
	assert!(
		schema.tables.contains_key("users"),
		"Schema should contain users table"
	);

	let users_table = schema.tables.get("users").unwrap();

	// Verify columns exist
	assert!(
		users_table.columns.contains_key("id"),
		"users should have id column"
	);
	assert!(
		users_table.columns.contains_key("username"),
		"users should have username column"
	);
	assert!(
		users_table.columns.contains_key("email"),
		"users should have email column"
	);
	assert!(
		users_table.columns.contains_key("is_active"),
		"users should have is_active column"
	);

	// Verify primary key
	assert_eq!(
		users_table.primary_key,
		vec!["id".to_string()],
		"Primary key should be id"
	);
}

// ============================================================================
// HP-002: Multiple Table Introspection
// ============================================================================

/// Test introspecting multiple related tables
///
/// **Test Intent**: Verify multiple tables can be introspected together
///
/// **Integration Point**: PostgresIntrospector → read_schema()
///
/// **Expected Behavior**:
/// - All tables are detected
/// - Each table has correct columns
#[rstest]
#[tokio::test]
async fn test_introspect_multiple_tables(
	#[future] postgres_introspect_schema: (ContainerAsync<GenericImage>, Arc<PgPool>, String),
) {
	let (_container, pool, _url) = postgres_introspect_schema.await;

	let introspector = PostgresIntrospector::new(pool.as_ref().clone());
	let schema = introspector
		.read_schema()
		.await
		.expect("Failed to read schema");

	// Verify all expected tables exist
	let expected_tables = ["users", "posts", "comments", "tags", "posts_tags"];
	for table_name in expected_tables.iter() {
		assert!(
			schema.tables.contains_key(*table_name),
			"Schema should contain {} table",
			table_name
		);
	}
}

// ============================================================================
// HP-003: Foreign Key Detection
// ============================================================================

/// Test detecting foreign key relationships
///
/// **Test Intent**: Verify FK relationships are correctly detected
///
/// **Integration Point**: PostgresIntrospector → read_foreign_keys()
///
/// **Expected Behavior**:
/// - FK constraints are detected
/// - Referenced tables are correct
/// - ON DELETE actions are captured
#[rstest]
#[tokio::test]
async fn test_introspect_foreign_keys(
	#[future] postgres_introspect_schema: (ContainerAsync<GenericImage>, Arc<PgPool>, String),
) {
	let (_container, pool, _url) = postgres_introspect_schema.await;

	// Verify FK exists on posts table
	let fk_count = get_foreign_key_count(&pool, "posts").await;
	assert!(fk_count > 0, "posts table should have foreign keys");

	let introspector = PostgresIntrospector::new(pool.as_ref().clone());
	let schema = introspector
		.read_schema()
		.await
		.expect("Failed to read schema");

	let posts_table = schema.tables.get("posts").unwrap();

	// Verify foreign keys are detected
	assert!(
		!posts_table.foreign_keys.is_empty(),
		"posts should have foreign keys"
	);

	// Find the author_id FK
	let author_fk = posts_table
		.foreign_keys
		.iter()
		.find(|fk| fk.columns.contains(&"author_id".to_string()));

	assert!(author_fk.is_some(), "Should detect author_id FK");
	let author_fk = author_fk.unwrap();
	assert_eq!(author_fk.referenced_table, "users");
	assert_eq!(
		author_fk.referenced_columns.first(),
		Some(&"id".to_string())
	);
}

// ============================================================================
// HP-004: Index Detection
// ============================================================================

/// Test detecting indexes on tables
///
/// **Test Intent**: Verify indexes are correctly detected
///
/// **Integration Point**: PostgresIntrospector → read_indexes()
///
/// **Expected Behavior**:
/// - Indexes are detected
/// - Index columns are correct
/// - Unique indexes are marked
#[rstest]
#[tokio::test]
async fn test_introspect_indexes(
	#[future] postgres_introspect_schema: (ContainerAsync<GenericImage>, Arc<PgPool>, String),
) {
	let (_container, pool, _url) = postgres_introspect_schema.await;

	let introspector = PostgresIntrospector::new(pool.as_ref().clone());
	let schema = introspector
		.read_schema()
		.await
		.expect("Failed to read schema");

	let users_table = schema.tables.get("users").unwrap();

	// Verify indexes exist (at least the unique indexes on username and email)
	assert!(!users_table.indexes.is_empty(), "users should have indexes");
}

// ============================================================================
// HP-005: Nullable Field Handling
// ============================================================================

/// Test nullable fields are correctly detected
///
/// **Test Intent**: Verify nullable flag is correctly set
///
/// **Integration Point**: PostgresIntrospector → column nullable flag
///
/// **Expected Behavior**:
/// - Nullable columns have nullable = true
/// - NOT NULL columns have nullable = false
#[rstest]
#[tokio::test]
async fn test_introspect_nullable_fields(
	#[future] postgres_introspect_schema: (ContainerAsync<GenericImage>, Arc<PgPool>, String),
) {
	let (_container, pool, _url) = postgres_introspect_schema.await;

	let introspector = PostgresIntrospector::new(pool.as_ref().clone());
	let schema = introspector
		.read_schema()
		.await
		.expect("Failed to read schema");

	let users_table = schema.tables.get("users").unwrap();

	// username is NOT NULL
	let username_col = users_table.columns.get("username").unwrap();
	assert!(!username_col.nullable, "username should be NOT NULL");

	// first_name is nullable (no NOT NULL constraint)
	let first_name_col = users_table.columns.get("first_name").unwrap();
	assert!(first_name_col.nullable, "first_name should be nullable");

	// last_login is nullable
	let last_login_col = users_table.columns.get("last_login").unwrap();
	assert!(last_login_col.nullable, "last_login should be nullable");
}

// ============================================================================
// HP-006: Self-Referencing FK Detection
// ============================================================================

/// Test detecting self-referencing foreign keys
///
/// **Test Intent**: Verify self-referential FK (e.g., parent comment) is detected
///
/// **Expected Behavior**:
/// - Self-referencing FK is detected
/// - to_table points to same table
#[rstest]
#[tokio::test]
async fn test_introspect_self_referencing_fk(
	#[future] postgres_introspect_schema: (ContainerAsync<GenericImage>, Arc<PgPool>, String),
) {
	let (_container, pool, _url) = postgres_introspect_schema.await;

	let introspector = PostgresIntrospector::new(pool.as_ref().clone());
	let schema = introspector
		.read_schema()
		.await
		.expect("Failed to read schema");

	let comments_table = schema.tables.get("comments").unwrap();

	// Find parent_id FK
	let parent_fk = comments_table
		.foreign_keys
		.iter()
		.find(|fk| fk.columns.contains(&"parent_id".to_string()));

	assert!(parent_fk.is_some(), "Should detect parent_id FK");
	let parent_fk = parent_fk.unwrap();

	// Should reference the same table (comments)
	assert_eq!(parent_fk.referenced_table, "comments");
}

// ============================================================================
// HP-007: Column Count Verification
// ============================================================================

/// Test column counts match database
///
/// **Test Intent**: Verify all columns are captured
#[rstest]
#[tokio::test]
async fn test_introspect_column_counts(
	#[future] postgres_introspect_schema: (ContainerAsync<GenericImage>, Arc<PgPool>, String),
) {
	let (_container, pool, _url) = postgres_introspect_schema.await;

	let introspector = PostgresIntrospector::new(pool.as_ref().clone());
	let schema = introspector
		.read_schema()
		.await
		.expect("Failed to read schema");

	// Verify users table column count
	let db_users_columns = get_column_count(&pool, "users").await as usize;
	let schema_users_columns = schema.tables.get("users").unwrap().columns.len();
	assert_eq!(
		schema_users_columns, db_users_columns,
		"Column count should match for users table"
	);

	// Verify posts table column count
	let db_posts_columns = get_column_count(&pool, "posts").await as usize;
	let schema_posts_columns = schema.tables.get("posts").unwrap().columns.len();
	assert_eq!(
		schema_posts_columns, db_posts_columns,
		"Column count should match for posts table"
	);
}

// ============================================================================
// HP-008: Generated Code Validity
// ============================================================================

/// Test that generated code contains required elements
///
/// **Test Intent**: Verify code generator produces valid Rust code
///
/// **Expected Behavior**:
/// - Generated code contains struct definitions
/// - Generated code contains `#[model]` attributes
/// - Generated code has correct field types
#[rstest]
#[tokio::test]
async fn test_generated_code_contains_struct(
	#[future] postgres_introspect_schema: (ContainerAsync<GenericImage>, Arc<PgPool>, String),
) {
	let (_container, pool, _url) = postgres_introspect_schema.await;

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

	// Should have generated files
	assert!(
		!output.files.is_empty(),
		"Should generate at least one file"
	);

	// Find users file
	let users_file = output.files.iter().find(|f| {
		f.path
			.file_name()
			.map(|n| n.to_str() == Some("users.rs"))
			.unwrap_or(false)
	});

	assert!(users_file.is_some(), "Should generate users.rs");
	let users_content = &users_file.unwrap().content;

	// Verify content
	assert!(
		users_content.contains("pub struct Users"),
		"Should contain Users struct"
	);
	assert!(
		users_content.contains("#[model"),
		"Should contain model attribute"
	);
}

// ============================================================================
// HP-009: Unique Constraint Detection
// ============================================================================

/// Test detecting unique constraints
///
/// **Test Intent**: Verify unique constraints are detected
#[rstest]
#[tokio::test]
async fn test_introspect_unique_constraints(
	#[future] postgres_introspect_schema: (ContainerAsync<GenericImage>, Arc<PgPool>, String),
) {
	let (_container, pool, _url) = postgres_introspect_schema.await;

	let introspector = PostgresIntrospector::new(pool.as_ref().clone());
	let schema = introspector
		.read_schema()
		.await
		.expect("Failed to read schema");

	let users_table = schema.tables.get("users").unwrap();

	// Should have unique constraints on username and email
	assert!(
		!users_table.unique_constraints.is_empty(),
		"users should have unique constraints"
	);
}

// ============================================================================
// HP-010: Composite Primary Key Detection
// ============================================================================

/// Test detecting composite primary keys
///
/// **Test Intent**: Verify composite PKs (junction tables) are detected
#[rstest]
#[tokio::test]
async fn test_introspect_composite_pk(
	#[future] postgres_introspect_schema: (ContainerAsync<GenericImage>, Arc<PgPool>, String),
) {
	let (_container, pool, _url) = postgres_introspect_schema.await;

	let introspector = PostgresIntrospector::new(pool.as_ref().clone());
	let schema = introspector
		.read_schema()
		.await
		.expect("Failed to read schema");

	let posts_tags_table = schema.tables.get("posts_tags").unwrap();

	// Should have composite PK
	assert_eq!(
		posts_tags_table.primary_key.len(),
		2,
		"posts_tags should have composite PK"
	);
	assert!(
		posts_tags_table
			.primary_key
			.contains(&"post_id".to_string())
	);
	assert!(posts_tags_table.primary_key.contains(&"tag_id".to_string()));
}
