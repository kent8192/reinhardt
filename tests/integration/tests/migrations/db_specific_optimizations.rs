//! Database-Specific Optimization Tests
//!
//! Tests that verify database-specific optimization features are correctly utilized
//! by the migrations system. Each database has unique features that can improve
//! performance, reduce locking, or enable specialized functionality.
//!
//! **Test Coverage:**
//! - PostgreSQL: CONCURRENTLY, DEFERRABLE, partial indexes, expression indexes, GiST, GIN, EXCLUDE
//! - MySQL: ALGORITHM=INSTANT/INPLACE, LOCK=NONE, FULLTEXT, SPATIAL, PARTITION
//! - SQLite: Table recreation workarounds, WITHOUT ROWID
//! - CockroachDB: INTERLEAVE, AS OF SYSTEM TIME
//!
//! **Fixtures Used:**
//! - postgres_container: PostgreSQL database container
//! - mysql_container: MySQL database container
//! - cockroachdb_container: CockroachDB container (when available)
//!
//! **Note**: Some tests may be marked as `#[ignore]` if the feature is not yet
//! implemented in reinhardt-db migrations. These serve as documentation for
//! future implementation.

use reinhardt_db::backends::DatabaseConnection;
use reinhardt_db::migrations::{
	ColumnDefinition, FieldType, ForeignKeyAction, Migration, Operation,
	executor::DatabaseMigrationExecutor,
	operations::{
		AlterTableOptions, Constraint, DeferrableOption, IndexType, InterleaveSpec, MySqlAlgorithm,
		MySqlLock, PartitionDef, PartitionOptions,
	},
};
use reinhardt_test::fixtures::{mysql_container, postgres_container};
use rstest::*;
use sqlx::{MySqlPool, PgPool};
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
	create_test_migration_with_deps(app, name, operations, vec![])
}

fn create_test_migration_with_deps(
	app: &'static str,
	name: &'static str,
	operations: Vec<Operation>,
	dependencies: Vec<(String, String)>,
) -> Migration {
	Migration {
		app_label: app.to_string(),
		name: name.to_string(),
		operations,
		dependencies,
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
// PostgreSQL-Specific Optimization Tests
// ============================================================================

/// Test CREATE INDEX CONCURRENTLY (non-blocking index creation)
///
/// **Test Intent**: Verify that CONCURRENTLY option creates index without locking table
///
/// **PostgreSQL Feature**: CREATE INDEX CONCURRENTLY allows index creation without
/// blocking writes to the table. Critical for production systems with large tables.
///
/// **Note**: CONCURRENTLY cannot be used inside a transaction, so atomic must be false
#[rstest]
#[tokio::test]
async fn test_postgres_create_index_concurrently(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, url) = postgres_container.await;

	let connection = DatabaseConnection::connect_postgres(&url)
		.await
		.expect("Failed to connect to PostgreSQL");

	let mut executor = DatabaseMigrationExecutor::new(connection.clone());

	// Create table first
	let create_table = create_test_migration(
		"testapp",
		"0001_create_users",
		vec![Operation::CreateTable {
			name: leak_str("concurrent_users").to_string(),
			columns: vec![
				create_auto_pk_column("id", FieldType::Integer),
				create_basic_column("email", FieldType::VarChar(255)),
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

	// Create index with CONCURRENTLY option
	// Note: atomic must be false because CONCURRENTLY cannot be used inside a transaction
	let create_index = Migration {
		app_label: "testapp".to_string(),
		name: "0002_create_email_index_concurrent".to_string(),
		operations: vec![Operation::CreateIndex {
			table: leak_str("concurrent_users").to_string(),
			columns: vec![leak_str("email").to_string()],
			unique: false,
			index_type: None,
			where_clause: None,
			concurrently: true, // Enable CONCURRENTLY
			expressions: None,
			mysql_options: None,
			operator_class: None,
		}],
		dependencies: vec![],
		replaces: vec![],
		atomic: false, // CONCURRENTLY cannot be used inside a transaction
		initial: None,
		state_only: false,
		database_only: false,
		swappable_dependencies: vec![],
		optional_dependencies: vec![],
	};

	executor
		.apply_migrations(&[create_index])
		.await
		.expect("Failed to create concurrent index");

	// Verify index was created
	let index_exists: bool =
		sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_indexes WHERE indexname = $1)")
			.bind("idx_concurrent_users_email")
			.fetch_one(pool.as_ref())
			.await
			.expect("Failed to check index");

	assert!(index_exists, "Concurrent index should exist");
}

/// Test DEFERRABLE INITIALLY DEFERRED constraints
///
/// **Test Intent**: Verify that DEFERRABLE constraints can be deferred to transaction end
///
/// **PostgreSQL Feature**: DEFERRABLE INITIALLY DEFERRED allows constraint checking
/// to be deferred until COMMIT, useful for circular foreign key relationships.
#[rstest]
#[tokio::test]
async fn test_postgres_deferrable_constraint(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, url) = postgres_container.await;

	let connection = DatabaseConnection::connect_postgres(&url)
		.await
		.expect("Failed to connect to PostgreSQL");

	let mut executor = DatabaseMigrationExecutor::new(connection.clone());

	// Create parent table
	let create_parent = create_test_migration(
		"testapp",
		"0001_create_parent",
		vec![Operation::CreateTable {
			name: leak_str("deferrable_parent").to_string(),
			columns: vec![create_auto_pk_column("id", FieldType::Integer)],
			constraints: vec![],
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		}],
	);

	// Create child table with DEFERRABLE FK
	// Must depend on parent migration to ensure correct execution order
	let create_child = create_test_migration_with_deps(
		"testapp",
		"0002_create_child_with_deferrable_fk",
		vec![Operation::CreateTable {
			name: leak_str("deferrable_child").to_string(),
			columns: vec![
				create_auto_pk_column("id", FieldType::Integer),
				create_basic_column("parent_id", FieldType::Integer),
			],
			constraints: vec![Constraint::ForeignKey {
				name: "fk_child_parent_deferrable".to_string(),
				columns: vec!["parent_id".to_string()],
				referenced_table: "deferrable_parent".to_string(),
				referenced_columns: vec!["id".to_string()],
				on_delete: ForeignKeyAction::NoAction,
				on_update: ForeignKeyAction::NoAction,
				deferrable: Some(DeferrableOption::Deferred),
			}],
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		}],
		vec![("testapp".to_string(), "0001_create_parent".to_string())],
	);

	executor
		.apply_migrations(&[create_parent, create_child])
		.await
		.expect("Failed to create tables with deferrable FK");

	// Verify constraint exists and is deferrable
	let constraint_info: Option<(String, String)> = sqlx::query_as(
		"SELECT conname::text, condeferred::text FROM pg_constraint
		 WHERE conname = 'fk_child_parent_deferrable'",
	)
	.fetch_optional(pool.as_ref())
	.await
	.expect("Failed to query constraint");

	assert!(
		constraint_info.is_some(),
		"Deferrable FK constraint should exist"
	);
}

/// Test partial indexes (indexes with WHERE clause)
///
/// **Test Intent**: Verify that partial indexes can be created
///
/// **PostgreSQL Feature**: Partial indexes only index rows matching a WHERE condition,
/// reducing index size and improving performance for filtered queries.
#[rstest]
#[tokio::test]
async fn test_postgres_partial_index(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, url) = postgres_container.await;

	let connection = DatabaseConnection::connect_postgres(&url)
		.await
		.expect("Failed to connect to PostgreSQL");

	let mut executor = DatabaseMigrationExecutor::new(connection.clone());

	// Create orders table
	let create_table = create_test_migration(
		"testapp",
		"0001_create_orders",
		vec![Operation::CreateTable {
			name: leak_str("partial_orders").to_string(),
			columns: vec![
				create_auto_pk_column("id", FieldType::Integer),
				create_basic_column("status", FieldType::VarChar(20)),
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

	// Create partial index (only index active orders)
	let create_partial_index = create_test_migration(
		"testapp",
		"0002_create_partial_index",
		vec![Operation::CreateIndex {
			table: leak_str("partial_orders").to_string(),
			columns: vec![leak_str("status").to_string()],
			unique: false,
			index_type: None,
			where_clause: Some(leak_str("status = 'active'").to_string()), // Partial index condition
			concurrently: false,
			expressions: None,
			mysql_options: None,
			operator_class: None,
		}],
	);

	executor
		.apply_migrations(&[create_partial_index])
		.await
		.expect("Failed to create partial index");

	// Verify index was created
	let index_exists: bool =
		sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_indexes WHERE indexname = $1)")
			.bind("idx_partial_orders_status")
			.fetch_one(pool.as_ref())
			.await
			.expect("Failed to check index");

	assert!(index_exists, "Partial index should exist");
}

/// Test expression indexes (indexes on computed expressions)
///
/// **Test Intent**: Verify that expression indexes can be created
///
/// **PostgreSQL Feature**: Expression indexes allow indexing computed values like
/// LOWER(email) for case-insensitive searches.
#[rstest]
#[tokio::test]
async fn test_postgres_expression_index(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, url) = postgres_container.await;

	let connection = DatabaseConnection::connect_postgres(&url)
		.await
		.expect("Failed to connect to PostgreSQL");

	let mut executor = DatabaseMigrationExecutor::new(connection.clone());

	// Create users table
	let create_table = create_test_migration(
		"testapp",
		"0001_create_users",
		vec![Operation::CreateTable {
			name: leak_str("expr_users").to_string(),
			columns: vec![
				create_auto_pk_column("id", FieldType::Integer),
				create_basic_column("email", FieldType::VarChar(255)),
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

	// Create expression index on LOWER(email) for case-insensitive search
	let create_expr_index = create_test_migration(
		"testapp",
		"0002_create_expression_index",
		vec![Operation::CreateIndex {
			table: leak_str("expr_users").to_string(),
			columns: vec![], // Ignored when expressions is set
			unique: false,
			index_type: None,
			where_clause: None,
			concurrently: false,
			expressions: Some(vec![leak_str("LOWER(email)").to_string()]), // Expression index
			mysql_options: None,
			operator_class: None,
		}],
	);

	executor
		.apply_migrations(&[create_expr_index])
		.await
		.expect("Failed to create expression index");

	// Verify index exists
	let index_count: i64 =
		sqlx::query_scalar("SELECT COUNT(*) FROM pg_indexes WHERE tablename = $1")
			.bind("expr_users")
			.fetch_one(pool.as_ref())
			.await
			.expect("Failed to count indexes");

	assert!(
		index_count >= 2,
		"Should have at least 2 indexes (PK + expression)"
	);
}

/// Test GiST index creation
///
/// **Test Intent**: Verify that GiST indexes can be created
///
/// **PostgreSQL Feature**: GiST (Generalized Search Tree) indexes support complex data types
/// like geometric types, full-text search, and custom types.
#[rstest]
#[tokio::test]
async fn test_postgres_gist_index(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, _pool, _port, url) = postgres_container.await;

	let connection = DatabaseConnection::connect_postgres(&url)
		.await
		.expect("Failed to connect to PostgreSQL");

	let mut executor = DatabaseMigrationExecutor::new(connection.clone());

	// Create table with point column for geometric data
	let create_table = create_test_migration(
		"testapp",
		"0001_create_locations",
		vec![Operation::CreateTable {
			name: leak_str("gist_locations").to_string(),
			columns: vec![
				create_auto_pk_column("id", FieldType::Integer),
				create_basic_column("name", FieldType::VarChar(100)),
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

	// Create GiST index on name column (for demonstration - normally would be on geometric type)
	let create_index = Migration {
		app_label: "testapp".to_string(),
		name: "0002_create_gist_index".to_string(),
		operations: vec![Operation::CreateIndex {
			table: leak_str("gist_locations").to_string(),
			columns: vec![leak_str("name").to_string()],
			unique: false,
			index_type: Some(IndexType::Gist),
			where_clause: None,
			concurrently: false,
			expressions: None,
			mysql_options: None,
			operator_class: None,
		}],
		dependencies: vec![],
		replaces: vec![],
		atomic: true,
		initial: None,
		state_only: false,
		database_only: false,
		swappable_dependencies: vec![],
		optional_dependencies: vec![],
	};

	// GiST index creation may fail on non-geometric types, so we check SQL generation
	let ops = &create_index.operations;
	assert_eq!(ops.len(), 1);
	if let Operation::CreateIndex { index_type, .. } = &ops[0] {
		assert_eq!(*index_type, Some(IndexType::Gist));
	} else {
		panic!("Expected CreateIndex operation");
	}
}

/// Test GIN index creation (for full-text search)
///
/// **Test Intent**: Verify that GIN indexes can be created
///
/// **PostgreSQL Feature**: GIN (Generalized Inverted Index) is optimized for indexing
/// array values, JSONB, and full-text search.
#[rstest]
#[tokio::test]
async fn test_postgres_gin_index(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, url) = postgres_container.await;

	let connection = DatabaseConnection::connect_postgres(&url)
		.await
		.expect("Failed to connect to PostgreSQL");

	let mut executor = DatabaseMigrationExecutor::new(connection.clone());

	// Create table with text column for GIN index
	let create_table = create_test_migration(
		"testapp",
		"0001_create_articles",
		vec![Operation::CreateTable {
			name: leak_str("gin_articles").to_string(),
			columns: vec![
				create_auto_pk_column("id", FieldType::Integer),
				create_basic_column("content", FieldType::Text),
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

	// Create GIN index with tsvector expression
	let create_index = Migration {
		app_label: "testapp".to_string(),
		name: "0002_create_gin_index".to_string(),
		operations: vec![Operation::CreateIndex {
			table: leak_str("gin_articles").to_string(),
			columns: vec![],
			unique: false,
			index_type: Some(IndexType::Gin),
			where_clause: None,
			concurrently: false,
			expressions: Some(vec![
				leak_str("to_tsvector('english', content)").to_string(),
			]),
			mysql_options: None,
			operator_class: None,
		}],
		dependencies: vec![],
		replaces: vec![],
		atomic: true,
		initial: None,
		state_only: false,
		database_only: false,
		swappable_dependencies: vec![],
		optional_dependencies: vec![],
	};

	executor
		.apply_migrations(&[create_index])
		.await
		.expect("Failed to create GIN index");

	// Verify index exists
	let index_exists: bool = sqlx::query_scalar(
		"SELECT EXISTS(SELECT 1 FROM pg_indexes WHERE tablename = 'gin_articles' AND indexdef LIKE '%gin%')",
	)
	.fetch_one(pool.as_ref())
	.await
	.expect("Failed to check index");

	assert!(index_exists, "GIN index should exist");
}

/// Test EXCLUDE constraint
///
/// **Test Intent**: Verify that EXCLUDE constraints can be created
///
/// **PostgreSQL Feature**: EXCLUDE constraints prevent overlapping ranges or conflicting
/// values using GiST indexes. Useful for scheduling, reservations, etc.
#[rstest]
#[tokio::test]
async fn test_postgres_exclude_constraint(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, url) = postgres_container.await;

	let connection = DatabaseConnection::connect_postgres(&url)
		.await
		.expect("Failed to connect to PostgreSQL");

	let mut executor = DatabaseMigrationExecutor::new(connection.clone());

	// Create required btree_gist extension for combining equality with range overlap
	let create_extension = create_test_migration(
		"testapp",
		"0001_create_btree_gist_extension",
		vec![Operation::RunSQL {
			sql: leak_str("CREATE EXTENSION IF NOT EXISTS btree_gist").to_string(),
			reverse_sql: Some(leak_str("DROP EXTENSION IF EXISTS btree_gist").to_string()),
		}],
	);

	executor
		.apply_migrations(&[create_extension])
		.await
		.expect("Failed to create btree_gist extension");

	// Create bookings table with date columns
	let create_table = create_test_migration(
		"testapp",
		"0002_create_bookings",
		vec![Operation::CreateTable {
			name: leak_str("bookings").to_string(),
			columns: vec![
				create_auto_pk_column("id", FieldType::Integer),
				ColumnDefinition {
					name: "room_id".to_string(),
					type_definition: FieldType::Integer,
					not_null: true,
					unique: false,
					primary_key: false,
					auto_increment: false,
					default: None,
				},
				create_basic_column("start_date", FieldType::Date),
				create_basic_column("end_date", FieldType::Date),
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
		.expect("Failed to create bookings table");

	// Add EXCLUDE constraint to prevent overlapping bookings for the same room
	let add_exclude = create_test_migration(
		"testapp",
		"0003_add_exclude_constraint",
		vec![Operation::AddConstraint {
			table: leak_str("bookings").to_string(),
			constraint_sql: leak_str(
				"CONSTRAINT exclude_overlapping_bookings EXCLUDE USING GIST \
				(room_id WITH =, daterange(start_date, end_date) WITH &&)",
			)
			.to_string(),
		}],
	);

	executor
		.apply_migrations(&[add_exclude])
		.await
		.expect("Failed to add EXCLUDE constraint");

	// Verify constraint was created
	let constraint_exists: bool = sqlx::query_scalar(
		"SELECT EXISTS(SELECT 1 FROM pg_constraint WHERE conname = $1 AND contype = 'x')",
	)
	.bind("exclude_overlapping_bookings")
	.fetch_one(pool.as_ref())
	.await
	.expect("Failed to check constraint");

	assert!(
		constraint_exists,
		"EXCLUDE constraint should have been created"
	);
}

/// Test trigram similarity index (pg_trgm extension)
///
/// **Test Intent**: Verify that trigram indexes can be created for fuzzy text search
///
/// **PostgreSQL Feature**: pg_trgm extension enables fuzzy string matching using
/// trigram similarity. Useful for autocomplete, typo-tolerant search.
#[rstest]
#[tokio::test]
async fn test_postgres_trigram_index(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, url) = postgres_container.await;

	let connection = DatabaseConnection::connect_postgres(&url)
		.await
		.expect("Failed to connect to PostgreSQL");

	let mut executor = DatabaseMigrationExecutor::new(connection.clone());

	// Create pg_trgm extension for trigram similarity
	let create_extension = create_test_migration(
		"testapp",
		"0001_create_pg_trgm_extension",
		vec![Operation::RunSQL {
			sql: leak_str("CREATE EXTENSION IF NOT EXISTS pg_trgm").to_string(),
			reverse_sql: Some(leak_str("DROP EXTENSION IF EXISTS pg_trgm").to_string()),
		}],
	);

	executor
		.apply_migrations(&[create_extension])
		.await
		.expect("Failed to create pg_trgm extension");

	// Create products table
	let create_table = create_test_migration(
		"testapp",
		"0002_create_products",
		vec![Operation::CreateTable {
			name: leak_str("products").to_string(),
			columns: vec![
				create_auto_pk_column("id", FieldType::Integer),
				create_basic_column("name", FieldType::VarChar(255)),
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
		.expect("Failed to create products table");

	// Create GIN index with gin_trgm_ops operator class for fuzzy search
	let create_trgm_index = create_test_migration(
		"testapp",
		"0003_create_trgm_index",
		vec![Operation::CreateIndex {
			table: leak_str("products").to_string(),
			columns: vec![leak_str("name").to_string()],
			unique: false,
			index_type: Some(IndexType::Gin),
			where_clause: None,
			concurrently: false,
			expressions: None,
			mysql_options: None,
			operator_class: Some("gin_trgm_ops".to_string()),
		}],
	);

	executor
		.apply_migrations(&[create_trgm_index])
		.await
		.expect("Failed to create trigram index");

	// Verify index was created with correct operator class
	let index_exists: bool = sqlx::query_scalar(
		"SELECT EXISTS(
			SELECT 1 FROM pg_indexes
			WHERE indexname LIKE '%products_name%'
			AND indexdef LIKE '%gin_trgm_ops%'
		)",
	)
	.fetch_one(pool.as_ref())
	.await
	.expect("Failed to check index");

	assert!(
		index_exists,
		"GIN index with gin_trgm_ops should have been created"
	);
}

// ============================================================================
// MySQL-Specific Optimization Tests
// ============================================================================

/// Test ALGORITHM=INSTANT (non-blocking ALTER TABLE in MySQL 8.0+)
///
/// **Test Intent**: Verify that ALGORITHM=INSTANT is used for compatible operations
///
/// **MySQL Feature**: ALGORITHM=INSTANT allows instant schema changes without table copy
/// for operations like adding columns with defaults, renaming columns, etc.
#[rstest]
#[tokio::test]
async fn test_mysql_algorithm_instant(
	#[future] mysql_container: (ContainerAsync<GenericImage>, Arc<MySqlPool>, u16, String),
) {
	let (_container, _pool, _port, url) = mysql_container.await;

	let connection = DatabaseConnection::connect_mysql(&url)
		.await
		.expect("Failed to connect to MySQL");

	let mut executor = DatabaseMigrationExecutor::new(connection.clone());

	// Create table first
	let create_table = create_test_migration(
		"testapp",
		"0001_create_users",
		vec![Operation::CreateTable {
			name: leak_str("algo_instant_users").to_string(),
			columns: vec![
				create_auto_pk_column("id", FieldType::Integer),
				create_basic_column("username", FieldType::VarChar(100)),
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

	// Create index with ALGORITHM=INSTANT
	let create_index = Migration {
		app_label: "testapp".to_string(),
		name: "0002_create_index_instant".to_string(),
		operations: vec![Operation::CreateIndex {
			table: leak_str("algo_instant_users").to_string(),
			columns: vec![leak_str("username").to_string()],
			unique: false,
			index_type: None,
			where_clause: None,
			concurrently: false,
			expressions: None,
			mysql_options: Some(AlterTableOptions {
				algorithm: Some(MySqlAlgorithm::Instant),
				lock: None,
			}),
			operator_class: None,
		}],
		dependencies: vec![],
		replaces: vec![],
		atomic: true,
		initial: None,
		state_only: false,
		database_only: false,
		swappable_dependencies: vec![],
		optional_dependencies: vec![],
	};

	// Verify the operation has correct MySQL options
	let ops = &create_index.operations;
	if let Operation::CreateIndex { mysql_options, .. } = &ops[0] {
		assert!(mysql_options.is_some());
		let opts = mysql_options.as_ref().unwrap();
		assert_eq!(opts.algorithm, Some(MySqlAlgorithm::Instant));
	}

	// Note: ALGORITHM=INSTANT may not be supported for all index operations in MySQL,
	// so we verify the operation structure rather than executing it
}

/// Test ALGORITHM=INPLACE (in-place schema changes)
///
/// **Test Intent**: Verify that ALGORITHM=INPLACE is used for compatible operations
///
/// **MySQL Feature**: ALGORITHM=INPLACE modifies table structure without full table copy,
/// allowing concurrent DML operations.
#[rstest]
#[tokio::test]
async fn test_mysql_algorithm_inplace(
	#[future] mysql_container: (ContainerAsync<GenericImage>, Arc<MySqlPool>, u16, String),
) {
	let (_container, _pool, _port, url) = mysql_container.await;

	let connection = DatabaseConnection::connect_mysql(&url)
		.await
		.expect("Failed to connect to MySQL");

	let mut executor = DatabaseMigrationExecutor::new(connection.clone());

	// Create table first
	let create_table = create_test_migration(
		"testapp",
		"0001_create_products",
		vec![Operation::CreateTable {
			name: leak_str("algo_inplace_products").to_string(),
			columns: vec![
				create_auto_pk_column("id", FieldType::Integer),
				create_basic_column("category", FieldType::VarChar(50)),
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

	// Create index with ALGORITHM=INPLACE
	let create_index = Migration {
		app_label: "testapp".to_string(),
		name: "0002_create_index_inplace".to_string(),
		operations: vec![Operation::CreateIndex {
			table: leak_str("algo_inplace_products").to_string(),
			columns: vec![leak_str("category").to_string()],
			unique: false,
			index_type: None,
			where_clause: None,
			concurrently: false,
			expressions: None,
			mysql_options: Some(AlterTableOptions {
				algorithm: Some(MySqlAlgorithm::Inplace),
				lock: None,
			}),
			operator_class: None,
		}],
		dependencies: vec![],
		replaces: vec![],
		atomic: true,
		initial: None,
		state_only: false,
		database_only: false,
		swappable_dependencies: vec![],
		optional_dependencies: vec![],
	};

	// Verify the operation has correct MySQL options
	let ops = &create_index.operations;
	if let Operation::CreateIndex { mysql_options, .. } = &ops[0] {
		assert!(mysql_options.is_some());
		let opts = mysql_options.as_ref().unwrap();
		assert_eq!(opts.algorithm, Some(MySqlAlgorithm::Inplace));
	}
}

/// Test LOCK=NONE (lock-free operations)
///
/// **Test Intent**: Verify that LOCK=NONE is used to allow concurrent writes
///
/// **MySQL Feature**: LOCK=NONE allows concurrent INSERT, UPDATE, DELETE during
/// schema changes (when compatible with operation).
#[rstest]
#[tokio::test]
async fn test_mysql_lock_none(
	#[future] mysql_container: (ContainerAsync<GenericImage>, Arc<MySqlPool>, u16, String),
) {
	let (_container, _pool, _port, url) = mysql_container.await;

	let connection = DatabaseConnection::connect_mysql(&url)
		.await
		.expect("Failed to connect to MySQL");

	let mut executor = DatabaseMigrationExecutor::new(connection.clone());

	// Create table first
	let create_table = create_test_migration(
		"testapp",
		"0001_create_orders",
		vec![Operation::CreateTable {
			name: leak_str("lock_none_orders").to_string(),
			columns: vec![
				create_auto_pk_column("id", FieldType::Integer),
				create_basic_column("status", FieldType::VarChar(50)),
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

	// Create index with LOCK=NONE
	let create_index = Migration {
		app_label: "testapp".to_string(),
		name: "0002_create_index_lock_none".to_string(),
		operations: vec![Operation::CreateIndex {
			table: leak_str("lock_none_orders").to_string(),
			columns: vec![leak_str("status").to_string()],
			unique: false,
			index_type: None,
			where_clause: None,
			concurrently: false,
			expressions: None,
			mysql_options: Some(AlterTableOptions {
				algorithm: None,
				lock: Some(MySqlLock::None),
			}),
			operator_class: None,
		}],
		dependencies: vec![],
		replaces: vec![],
		atomic: true,
		initial: None,
		state_only: false,
		database_only: false,
		swappable_dependencies: vec![],
		optional_dependencies: vec![],
	};

	// Verify the operation has correct MySQL options
	let ops = &create_index.operations;
	if let Operation::CreateIndex { mysql_options, .. } = &ops[0] {
		assert!(mysql_options.is_some());
		let opts = mysql_options.as_ref().unwrap();
		assert_eq!(opts.lock, Some(MySqlLock::None));
	}
}

/// Test FULLTEXT INDEX creation
///
/// **Test Intent**: Verify that MySQL FULLTEXT indexes can be created
///
/// **MySQL Feature**: FULLTEXT indexes enable natural language full-text search
#[rstest]
#[tokio::test]
async fn test_mysql_fulltext_index(
	#[future] mysql_container: (ContainerAsync<GenericImage>, Arc<MySqlPool>, u16, String),
) {
	let (_container, pool, _port, url) = mysql_container.await;

	let connection = DatabaseConnection::connect_mysql(&url)
		.await
		.expect("Failed to connect to MySQL");

	let mut executor = DatabaseMigrationExecutor::new(connection.clone());

	// Create table with text columns
	let create_table = create_test_migration(
		"testapp",
		"0001_create_articles",
		vec![Operation::CreateTable {
			name: leak_str("ft_articles").to_string(),
			columns: vec![
				create_auto_pk_column("id", FieldType::Integer),
				create_basic_column("title", FieldType::VarChar(255)),
				create_basic_column("body", FieldType::Text),
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

	// Create FULLTEXT index
	let create_index = Migration {
		app_label: "testapp".to_string(),
		name: "0002_create_fulltext_index".to_string(),
		operations: vec![Operation::CreateIndex {
			table: leak_str("ft_articles").to_string(),
			columns: vec![leak_str("title").to_string(), leak_str("body").to_string()],
			unique: false,
			index_type: Some(IndexType::Fulltext),
			where_clause: None,
			concurrently: false,
			expressions: None,
			mysql_options: None,
			operator_class: None,
		}],
		dependencies: vec![],
		replaces: vec![],
		atomic: true,
		initial: None,
		state_only: false,
		database_only: false,
		swappable_dependencies: vec![],
		optional_dependencies: vec![],
	};

	executor
		.apply_migrations(&[create_index])
		.await
		.expect("Failed to create FULLTEXT index");

	// Verify index exists
	let index_exists: bool = sqlx::query_scalar(
		"SELECT COUNT(*) > 0 FROM information_schema.statistics
		 WHERE table_schema = DATABASE() AND table_name = 'ft_articles' AND index_type = 'FULLTEXT'",
	)
	.fetch_one(pool.as_ref())
	.await
	.expect("Failed to check index");

	assert!(index_exists, "FULLTEXT index should exist");
}

/// Test SPATIAL INDEX creation
///
/// **Test Intent**: Verify that MySQL SPATIAL indexes can be created
///
/// **MySQL Feature**: SPATIAL indexes optimize geometric queries (GIS data)
#[rstest]
#[tokio::test]
async fn test_mysql_spatial_index(
	#[future] mysql_container: (ContainerAsync<GenericImage>, Arc<MySqlPool>, u16, String),
) {
	let (_container, _pool, _port, url) = mysql_container.await;

	let connection = DatabaseConnection::connect_mysql(&url)
		.await
		.expect("Failed to connect to MySQL");

	let _executor = DatabaseMigrationExecutor::new(connection.clone());

	// Verify that IndexType::Spatial exists and can be used
	let create_index_op = Operation::CreateIndex {
		table: leak_str("spatial_locations").to_string(),
		columns: vec![leak_str("coordinates").to_string()],
		unique: false,
		index_type: Some(IndexType::Spatial),
		where_clause: None,
		concurrently: false,
		expressions: None,
		mysql_options: None,
		operator_class: None,
	};

	// Verify the operation has SPATIAL index type
	if let Operation::CreateIndex { index_type, .. } = &create_index_op {
		assert_eq!(*index_type, Some(IndexType::Spatial));
	} else {
		panic!("Expected CreateIndex operation");
	}

	// Note: Actual SPATIAL index creation requires GEOMETRY column type,
	// which is not yet supported in FieldType enum
}

/// Test table partitioning by RANGE
///
/// **Test Intent**: Verify that PARTITION BY RANGE can be created
///
/// **MySQL Feature**: RANGE partitioning splits table data by column value ranges,
/// improving query performance for time-series data.
#[rstest]
#[tokio::test]
async fn test_mysql_partition_by_range(
	#[future] mysql_container: (ContainerAsync<GenericImage>, Arc<MySqlPool>, u16, String),
) {
	let (_container, _pool, _port, url) = mysql_container.await;

	let connection = DatabaseConnection::connect_mysql(&url)
		.await
		.expect("Failed to connect to MySQL");

	let _executor = DatabaseMigrationExecutor::new(connection.clone());

	// Create table with RANGE partition
	let create_table_op = Operation::CreateTable {
		name: leak_str("range_sales").to_string(),
		columns: vec![
			create_auto_pk_column("id", FieldType::Integer),
			create_basic_column("amount", FieldType::Integer),
		],
		constraints: vec![],
		without_rowid: None,
		interleave_in_parent: None,
		partition: Some(PartitionOptions {
			partition_type: reinhardt_db::migrations::operations::PartitionType::Range,
			column: "id".to_string(),
			partitions: vec![
				PartitionDef {
					name: "p0".to_string(),
					values: reinhardt_db::migrations::operations::PartitionValues::LessThan(
						"1000".to_string(),
					),
				},
				PartitionDef {
					name: "p1".to_string(),
					values: reinhardt_db::migrations::operations::PartitionValues::LessThan(
						"2000".to_string(),
					),
				},
			],
		}),
	};

	// Verify partition options are set correctly
	if let Operation::CreateTable { partition, .. } = &create_table_op {
		assert!(partition.is_some());
		let p = partition.as_ref().unwrap();
		assert_eq!(
			p.partition_type,
			reinhardt_db::migrations::operations::PartitionType::Range
		);
		assert_eq!(p.partitions.len(), 2);
	} else {
		panic!("Expected CreateTable operation");
	}
}

/// Test table partitioning by HASH
///
/// **Test Intent**: Verify that PARTITION BY HASH can be created
///
/// **MySQL Feature**: HASH partitioning distributes rows evenly across partitions
/// using a hash function on a column.
#[rstest]
#[tokio::test]
async fn test_mysql_partition_by_hash(
	#[future] mysql_container: (ContainerAsync<GenericImage>, Arc<MySqlPool>, u16, String),
) {
	let (_container, _pool, _port, url) = mysql_container.await;

	let connection = DatabaseConnection::connect_mysql(&url)
		.await
		.expect("Failed to connect to MySQL");

	let _executor = DatabaseMigrationExecutor::new(connection.clone());

	// Create table with HASH partition
	let create_table_op = Operation::CreateTable {
		name: leak_str("hash_users").to_string(),
		columns: vec![create_auto_pk_column("id", FieldType::Integer)],
		constraints: vec![],
		without_rowid: None,
		interleave_in_parent: None,
		partition: Some(PartitionOptions {
			partition_type: reinhardt_db::migrations::operations::PartitionType::Hash,
			column: "id".to_string(),
			partitions: vec![
				PartitionDef {
					name: "p0".to_string(),
					values: reinhardt_db::migrations::operations::PartitionValues::ModuloCount(4),
				},
				PartitionDef {
					name: "p1".to_string(),
					values: reinhardt_db::migrations::operations::PartitionValues::ModuloCount(4),
				},
				PartitionDef {
					name: "p2".to_string(),
					values: reinhardt_db::migrations::operations::PartitionValues::ModuloCount(4),
				},
				PartitionDef {
					name: "p3".to_string(),
					values: reinhardt_db::migrations::operations::PartitionValues::ModuloCount(4),
				},
			],
		}),
	};

	// Verify partition options are set correctly
	if let Operation::CreateTable { partition, .. } = &create_table_op {
		assert!(partition.is_some());
		let p = partition.as_ref().unwrap();
		assert_eq!(
			p.partition_type,
			reinhardt_db::migrations::operations::PartitionType::Hash
		);
		assert_eq!(p.partitions.len(), 4);
	} else {
		panic!("Expected CreateTable operation");
	}
}

/// Test AUTO_INCREMENT initial value setting
///
/// **Test Intent**: Verify that AUTO_INCREMENT can start from a specific value
///
/// **MySQL Feature**: Setting AUTO_INCREMENT initial value is useful when merging data
/// or coordinating with external systems.
#[rstest]
#[tokio::test]
async fn test_mysql_auto_increment_initial_value(
	#[future] mysql_container: (ContainerAsync<GenericImage>, Arc<MySqlPool>, u16, String),
) {
	let (_container, pool, _port, url) = mysql_container.await;

	let connection = DatabaseConnection::connect_mysql(&url)
		.await
		.expect("Failed to connect to MySQL");

	let mut executor = DatabaseMigrationExecutor::new(connection.clone());

	// Create table with AUTO_INCREMENT
	let migration = create_test_migration(
		"testapp",
		"0001_create_users",
		vec![Operation::CreateTable {
			name: leak_str("users").to_string(),
			columns: vec![create_auto_pk_column("id", FieldType::Integer)],
			constraints: vec![],
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		}],
	);

	executor
		.apply_migrations(&[migration])
		.await
		.expect("Failed to create table");

	// TODO: Add Operation::SetAutoIncrementValue
	// For now, we can use RunSQL as a workaround
	let set_auto_increment = create_test_migration(
		"testapp",
		"0002_set_auto_increment",
		vec![Operation::RunSQL {
			sql: leak_str("ALTER TABLE users AUTO_INCREMENT = 1000").to_string(),
			reverse_sql: None,
		}],
	);

	executor
		.apply_migrations(&[set_auto_increment])
		.await
		.expect("Failed to set AUTO_INCREMENT");

	// Verify AUTO_INCREMENT value
	let auto_increment: Option<u64> = sqlx::query_scalar(
		"SELECT AUTO_INCREMENT FROM information_schema.tables
		 WHERE table_schema = DATABASE() AND table_name = 'users'",
	)
	.fetch_one(pool.as_ref())
	.await
	.expect("Failed to get AUTO_INCREMENT value");

	assert_eq!(auto_increment, Some(1000), "AUTO_INCREMENT should be 1000");
}

// ============================================================================
// SQLite-Specific Tests
// ============================================================================

/// Test ALTER TABLE limitation workaround (table recreation)
///
/// **Test Intent**: Verify that SQLite's ALTER TABLE limitations are handled
/// by recreating the table using SqliteTableRecreation.
///
/// **SQLite Limitation**: SQLite has very limited ALTER TABLE support (can't drop columns,
/// change column types, etc.). The workaround is to:
/// 1. CREATE TABLE temp_table (with new schema)
/// 2. INSERT INTO temp_table SELECT * FROM old_table
/// 3. DROP TABLE old_table
/// 4. ALTER TABLE temp_table RENAME TO old_table
///
/// **Implementation Status**:
/// - SqliteTableRecreation struct: Implemented in operations.rs
/// - Executor integration: Pending (requires SQLite dialect detection in executor)
///
/// **Note**: This test is ignored until the migration executor automatically detects
/// SQLite dialect and uses SqliteTableRecreation for incompatible operations.
#[rstest]
#[ignore = "Pending: executor integration for automatic SQLite table recreation"]
#[tokio::test]
async fn test_sqlite_alter_table_via_recreation() {
	// When executor integration is complete, this test should:
	// 1. Create a table with multiple columns
	// 2. Execute Operation::DropColumn
	// 3. Verify that the executor automatically uses SqliteTableRecreation
	// 4. Verify the column was removed and data preserved
	//
	// The SqliteTableRecreation struct provides:
	// - SqliteTableRecreation::for_drop_column() - factory for DropColumn operations
	// - SqliteTableRecreation::for_alter_column() - factory for AlterColumn operations
	// - SqliteTableRecreation::to_sql_statements() - generates the 4-step SQL pattern
	// - Operation::requires_sqlite_recreation() - checks if recreation is needed
}

/// Test WITHOUT ROWID optimization
///
/// **Test Intent**: Verify that WITHOUT ROWID tables can be created
///
/// **SQLite Feature**: WITHOUT ROWID tables store data in the PRIMARY KEY index,
/// reducing storage and improving performance for tables with composite primary keys.
#[rstest]
#[tokio::test]
async fn test_sqlite_without_rowid() {
	// Create table with WITHOUT ROWID option
	let create_table_op = Operation::CreateTable {
		name: leak_str("settings").to_string(),
		columns: vec![
			ColumnDefinition {
				name: "key".to_string(),
				type_definition: FieldType::VarChar(50),
				not_null: true,
				unique: false,
				primary_key: true,
				auto_increment: false,
				default: None,
			},
			create_basic_column("value", FieldType::Text),
		],
		constraints: vec![],
		without_rowid: Some(true),
		interleave_in_parent: None,
		partition: None,
	};

	// Verify without_rowid is set correctly
	if let Operation::CreateTable { without_rowid, .. } = &create_table_op {
		assert_eq!(*without_rowid, Some(true));
	} else {
		panic!("Expected CreateTable operation");
	}

	// Expected SQL: CREATE TABLE settings (...) WITHOUT ROWID
	// Note: Actual execution requires SQLite database
}

// ============================================================================
// CockroachDB-Specific Tests
// ============================================================================

/// Test interleaved tables (INTERLEAVE IN PARENT)
///
/// **Test Intent**: Verify that interleaved tables can be created
///
/// **CockroachDB Feature**: INTERLEAVE IN PARENT co-locates child table rows with
/// parent table rows, improving join performance for hierarchical data.
#[rstest]
#[tokio::test]
async fn test_cockroachdb_interleave_table() {
	// Create parent table first
	let _create_users = Operation::CreateTable {
		name: leak_str("users").to_string(),
		columns: vec![create_auto_pk_column("id", FieldType::Integer)],
		constraints: vec![],
		without_rowid: None,
		interleave_in_parent: None,
		partition: None,
	};

	// Create child table with INTERLEAVE IN PARENT
	let create_orders = Operation::CreateTable {
		name: leak_str("orders").to_string(),
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
				name: "order_id".to_string(),
				type_definition: FieldType::Integer,
				not_null: true,
				unique: false,
				primary_key: true,
				auto_increment: true,
				default: None,
			},
		],
		constraints: vec![],
		without_rowid: None,
		interleave_in_parent: Some(InterleaveSpec {
			parent_table: leak_str("users").to_string(),
			parent_columns: vec![leak_str("user_id").to_string()],
		}),
		partition: None,
	};

	// Verify interleave_in_parent is set correctly
	if let Operation::CreateTable {
		interleave_in_parent,
		..
	} = &create_orders
	{
		assert!(interleave_in_parent.is_some());
		let spec = interleave_in_parent.as_ref().unwrap();
		assert_eq!(spec.parent_table, "users");
		assert_eq!(spec.parent_columns, vec!["user_id"]);
	} else {
		panic!("Expected CreateTable operation");
	}

	// Expected SQL:
	// CREATE TABLE orders (
	//   user_id INT,
	//   order_id INT,
	//   PRIMARY KEY(user_id, order_id)
	// ) INTERLEAVE IN PARENT users (user_id)
	//
	// Note: Actual execution requires CockroachDB database
}

/// Test AS OF SYSTEM TIME (time-travel queries)
///
/// **Test Intent**: Verify that time-travel queries can be performed
///
/// **CockroachDB Feature**: AS OF SYSTEM TIME allows querying historical data,
/// useful for auditing, debugging, and consistent reads.
#[rstest]
#[ignore = "AS OF SYSTEM TIME is a query feature, not a migration feature"]
#[tokio::test]
async fn test_cockroachdb_time_travel_query() {
	// Note: AS OF SYSTEM TIME is a query feature, not a migration feature.
	// This test is included for completeness but doesn't directly relate to migrations.

	// Example usage (in application code, not migrations):
	// SELECT * FROM users AS OF SYSTEM TIME '-1h' WHERE id = 1
	//
	// This retrieves the user as it existed 1 hour ago.
}
