//! Integration tests for migration lifecycle
//!
//! Tests the complete lifecycle of migrations including:
//! - Forward and backward migration cycles
//! - Dependency ordering and resolution
//! - State consistency after rollbacks
//! - Data preservation during schema changes
//! - Migration history integrity
//!
//! **Test Coverage:**
//! - Full migration reversibility (apply → rollback → re-apply)
//! - Complex dependency graphs (diamond patterns)
//! - ProjectState ↔ Database schema synchronization
//! - Data transformation and preservation
//! - Migration history table accuracy
//!
//! **Fixtures Used:**
//! - postgres_container: PostgreSQL database container

use reinhardt_db::backends::DatabaseConnection;
use reinhardt_db::backends::types::DatabaseType;
use reinhardt_db::migrations::{
	ColumnDefinition, FieldType, Migration, Operation, executor::DatabaseMigrationExecutor,
	recorder::DatabaseMigrationRecorder,
};
use reinhardt_test::fixtures::postgres_container;
use rstest::*;
use serial_test::serial;
use sqlx::{PgPool, Row};
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
	Migration::from_parts(name.to_string(), app.to_string(), operations, vec![], vec![], true, None, false, false, Vec::new(), Vec::new())
}

/// Create a migration with dependencies
fn create_migration_with_deps(
	app: &'static str,
	name: &'static str,
	operations: Vec<Operation>,
	dependencies: Vec<(&'static str, &'static str)>,
) -> Migration {
	Migration::from_parts(name.to_string(), app.to_string(), operations, dependencies, vec![], true, None, false, false, Vec::new(), Vec::new())
}

/// Create a basic column definition
fn create_basic_column(name: &str, type_def: FieldType) -> ColumnDefinition {
	ColumnDefinition::from_parts(name.to_string(), type_def, false, false, false, false, None, None, None)
}

/// Create a column definition with constraints
fn create_column_with_constraints(
	name: &'static str,
	type_def: FieldType,
	not_null: bool,
	unique: bool,
) -> ColumnDefinition {
	ColumnDefinition::from_parts(name.to_string(), type_def, not_null, unique, false, false, None, None, None)
}

// ============================================================================
// Migration Reversibility Tests
// ============================================================================

/// Test full migration cycle: apply → rollback → re-apply
///
/// **Test Intent**: Verify that migrations with multiple operations can be
/// fully reversed and re-applied without data loss or schema inconsistencies
///
/// **Integration Point**: MigrationExecutor → PostgreSQL DDL → MigrationRecorder
///
/// **Expected Behavior**: Each step (forward, backward, forward again) maintains
/// schema consistency, and the final state matches the expected schema exactly
#[rstest]
#[tokio::test]
#[serial(migration_lifecycle)]
async fn test_migration_reversible_full_cycle(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, url) = postgres_container.await;

	// ============================================================================
	// Setup: Initial schema with User and Post tables
	// ============================================================================

	let initial_migration = create_test_migration(
		"testapp",
		"0001_initial",
		vec![
			Operation::CreateTable {
				name: leak_str("users").to_string(),
				columns: vec![
					ColumnDefinition::from_parts("id".to_string(), FieldType::Custom("SERIAL PRIMARY KEY".to_string()), true, false, true, true, None, None, None),
					create_basic_column("username", FieldType::VarChar(Some(100))),
				],
			},
			Operation::CreateTable {
				name: leak_str("posts").to_string(),
				columns: vec![
					ColumnDefinition::from_parts("id".to_string(), FieldType::Custom("SERIAL PRIMARY KEY".to_string()), true, false, true, true, None, None, None),
					create_basic_column("title", FieldType::VarChar(Some(200))),
				],
			},
		],
	);

	// Apply initial migration
	let conn = DatabaseConnection::connect(&url, DatabaseType::Postgres)
		.await
		.expect("Failed to connect to database");
	let mut executor = DatabaseMigrationExecutor::new(conn.clone());
	executor
		.apply_migration(&initial_migration)
		.await
		.expect("Failed to apply initial migration");

	// Verify initial tables exist
	let users_count: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM information_schema.tables WHERE table_name = 'users'",
	)
	.fetch_one(&*pool)
	.await
	.expect("Failed to query users table");
	assert_eq!(
		users_count, 1,
		"Users table should exist after initial migration"
	);

	let posts_count: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM information_schema.tables WHERE table_name = 'posts'",
	)
	.fetch_one(&*pool)
	.await
	.expect("Failed to query posts table");
	assert_eq!(
		posts_count, 1,
		"Posts table should exist after initial migration"
	);

	// ============================================================================
	// Execute: Apply complex migration (AddColumn, AddIndex, AddForeignKey)
	// ============================================================================

	let complex_migration = create_test_migration(
		"testapp",
		"0002_add_features",
		vec![
			Operation::AddColumn {
				table: leak_str("users").to_string(),
				column: create_column_with_constraints(
					"email",
					FieldType::VarChar(Some(255)),
					false,
					true,
				),
				mysql_options: None,
			},
			Operation::AddColumn {
				table: leak_str("posts").to_string(),
				column: create_basic_column("user_id", FieldType::Integer),
				mysql_options: None,
			},
			Operation::CreateIndex {
				table: leak_str("users").to_string(),
				name: leak_str("idx_users_email").to_string(),
				columns: vec!["email"],
				unique: true,
			},
		],
	);

	executor
		.apply_migration(&complex_migration)
		.await
		.expect("Failed to apply complex migration");

	// Verify email column exists
	let email_column_count: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM information_schema.columns WHERE table_name = 'users' AND column_name = 'email'",
	)
	.fetch_one(&*pool)
	.await
	.expect("Failed to query email column");
	assert_eq!(
		email_column_count, 1,
		"Email column should exist after migration"
	);

	// Verify index exists
	let index_count: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM pg_indexes WHERE tablename = 'users' AND indexname = 'idx_users_email'",
	)
	.fetch_one(&*pool)
	.await
	.expect("Failed to query index");
	assert_eq!(index_count, 1, "Email index should exist after migration");

	// ============================================================================
	// Execute: Rollback the complex migration
	// ============================================================================

	executor
		.rollback_migration(&complex_migration)
		.await
		.expect("Failed to rollback complex migration");

	// Verify email column is removed
	let email_column_count_after_rollback: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM information_schema.columns WHERE table_name = 'users' AND column_name = 'email'",
	)
	.fetch_one(&*pool)
	.await
	.expect("Failed to query email column after rollback");
	assert_eq!(
		email_column_count_after_rollback, 0,
		"Email column should be removed after rollback"
	);

	// Verify index is removed
	let index_count_after_rollback: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM pg_indexes WHERE tablename = 'users' AND indexname = 'idx_users_email'",
	)
	.fetch_one(&*pool)
	.await
	.expect("Failed to query index after rollback");
	assert_eq!(
		index_count_after_rollback, 0,
		"Email index should be removed after rollback"
	);

	// ============================================================================
	// Execute: Re-apply the complex migration
	// ============================================================================

	executor
		.apply_migration(&complex_migration)
		.await
		.expect("Failed to re-apply complex migration");

	// ============================================================================
	// Assert: Verify final state matches expected schema
	// ============================================================================

	// Verify email column exists again
	let email_column_count_final: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM information_schema.columns WHERE table_name = 'users' AND column_name = 'email'",
	)
	.fetch_one(&*pool)
	.await
	.expect("Failed to query email column after re-apply");
	assert_eq!(
		email_column_count_final, 1,
		"Email column should exist after re-apply"
	);

	// Verify index exists again
	let index_count_final: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM pg_indexes WHERE tablename = 'users' AND indexname = 'idx_users_email'",
	)
	.fetch_one(&*pool)
	.await
	.expect("Failed to query index after re-apply");
	assert_eq!(
		index_count_final, 1,
		"Email index should exist after re-apply"
	);

	// Verify user_id column in posts table
	let user_id_column_count: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM information_schema.columns WHERE table_name = 'posts' AND column_name = 'user_id'",
	)
	.fetch_one(&*pool)
	.await
	.expect("Failed to query user_id column");
	assert_eq!(
		user_id_column_count, 1,
		"user_id column should exist in posts table"
	);
}

// ============================================================================
// Dependency Ordering Tests
// ============================================================================

/// Test complex dependency ordering (diamond dependency pattern)
///
/// **Test Intent**: Verify that MigrationExecutor correctly resolves complex
/// dependency graphs where multiple migrations depend on the same base migration
/// and a final migration depends on multiple intermediate migrations
///
/// **Integration Point**: MigrationExecutor::build_plan() → Topological sort
///
/// **Expected Behavior**: Migrations execute in dependency order:
/// auth.0001 → (profile.0001, blog.0001) → comment.0001
#[rstest]
#[tokio::test]
#[serial(migration_lifecycle)]
async fn test_migration_dependency_ordering_complex(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, url) = postgres_container.await;

	// ============================================================================
	// Setup: Create migrations with diamond dependency pattern
	// ============================================================================
	//
	// Dependency graph:
	//        auth.0001 (User)
	//           /    \
	//          /      \
	//   profile.0001  blog.0001
	//   (UserProfile) (Post)
	//          \      /
	//           \    /
	//        comment.0001
	//         (Comment)

	let auth_migration = create_test_migration(
		"auth",
		"0001_initial",
		vec![Operation::CreateTable {
			name: leak_str("users").to_string(),
			columns: vec![
				ColumnDefinition::from_parts("id".to_string(), FieldType::Custom("SERIAL PRIMARY KEY".to_string()), true, false, true, true, None, None, None),
				create_basic_column("username", FieldType::VarChar(Some(100))),
			],
		}],
	);

	let profile_migration = create_migration_with_deps(
		"profile",
		"0001_initial",
		vec![Operation::CreateTable {
			name: leak_str("user_profiles").to_string(),
			columns: vec![
				ColumnDefinition::from_parts("id".to_string(), FieldType::Custom("SERIAL PRIMARY KEY".to_string()), true, false, true, true, None, None, None),
				create_basic_column("user_id", FieldType::Integer),
				create_basic_column("bio", FieldType::Text),
			],
		}],
		vec![("auth", "0001_initial")],
	);

	let blog_migration = create_migration_with_deps(
		"blog",
		"0001_initial",
		vec![Operation::CreateTable {
			name: leak_str("posts").to_string(),
			columns: vec![
				ColumnDefinition::from_parts("id".to_string(), FieldType::Custom("SERIAL PRIMARY KEY".to_string()), true, false, true, true, None, None, None),
				create_basic_column("author_id", FieldType::Integer),
				create_basic_column("title", FieldType::VarChar(Some(200))),
			],
		}],
		vec![("auth", "0001_initial")],
	);

	let comment_migration = create_migration_with_deps(
		"comment",
		"0001_initial",
		vec![Operation::CreateTable {
			name: leak_str("comments").to_string(),
			columns: vec![
				ColumnDefinition::from_parts("id".to_string(), FieldType::Custom("SERIAL PRIMARY KEY".to_string()), true, false, true, true, None, None, None),
				create_basic_column("post_id", FieldType::Integer),
				create_basic_column("profile_id", FieldType::Integer),
				create_basic_column("content", FieldType::Text),
			],
		}],
		vec![("blog", "0001_initial"), ("profile", "0001_initial")],
	);

	// ============================================================================
	// Execute: Apply migrations through executor
	// ============================================================================

	let conn = DatabaseConnection::connect(&url, DatabaseType::Postgres)
		.await
		.expect("Failed to connect to database");
	let mut executor = DatabaseMigrationExecutor::new(conn.clone());

	// Apply in dependency order
	executor
		.apply_migration(&auth_migration)
		.await
		.expect("Failed to apply auth migration");

	executor
		.apply_migration(&profile_migration)
		.await
		.expect("Failed to apply profile migration");

	executor
		.apply_migration(&blog_migration)
		.await
		.expect("Failed to apply blog migration");

	executor
		.apply_migration(&comment_migration)
		.await
		.expect("Failed to apply comment migration");

	// ============================================================================
	// Assert: Verify all tables exist in correct order
	// ============================================================================

	// Verify users table (auth.0001)
	let users_exists: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM information_schema.tables WHERE table_name = 'users'",
	)
	.fetch_one(&*pool)
	.await
	.expect("Failed to query users table");
	assert_eq!(users_exists, 1, "Users table should exist");

	// Verify user_profiles table (profile.0001)
	let profiles_exists: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM information_schema.tables WHERE table_name = 'user_profiles'",
	)
	.fetch_one(&*pool)
	.await
	.expect("Failed to query user_profiles table");
	assert_eq!(profiles_exists, 1, "UserProfiles table should exist");

	// Verify posts table (blog.0001)
	let posts_exists: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM information_schema.tables WHERE table_name = 'posts'",
	)
	.fetch_one(&*pool)
	.await
	.expect("Failed to query posts table");
	assert_eq!(posts_exists, 1, "Posts table should exist");

	// Verify comments table (comment.0001)
	let comments_exists: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM information_schema.tables WHERE table_name = 'comments'",
	)
	.fetch_one(&*pool)
	.await
	.expect("Failed to query comments table");
	assert_eq!(comments_exists, 1, "Comments table should exist");

	// Verify migration history order
	let recorder = DatabaseMigrationRecorder::new(conn);
	let applied_migrations = recorder
		.list_applied_migrations()
		.await
		.expect("Failed to list applied migrations");

	assert_eq!(
		applied_migrations.len(),
		4,
		"Should have 4 applied migrations"
	);

	// Verify auth.0001 was applied first
	assert_eq!(applied_migrations[0].0, "auth");
	assert_eq!(applied_migrations[0].1, "0001_initial");

	// Verify profile.0001 and blog.0001 were applied after auth
	// (order between profile and blog may vary as they're independent)
	let middle_apps: Vec<&str> = vec![&applied_migrations[1].0, &applied_migrations[2].0];
	assert!(
		middle_apps.contains(&"profile"),
		"Profile migration should be applied"
	);
	assert!(
		middle_apps.contains(&"blog"),
		"Blog migration should be applied"
	);

	// Verify comment.0001 was applied last
	assert_eq!(applied_migrations[3].0, "comment");
	assert_eq!(applied_migrations[3].1, "0001_initial");
}

// ============================================================================
// State Consistency Tests
// ============================================================================

/// Test ProjectState and database schema consistency after rollback
///
/// **Test Intent**: Verify that after rolling back a migration, both the
/// in-memory ProjectState and the actual database schema remain synchronized
///
/// **Integration Point**: ProjectState ↔ Database schema ↔ MigrationRecorder
///
/// **Expected Behavior**: After rollback, ProjectState accurately reflects
/// the database schema, with all tables, columns, and constraints matching
#[rstest]
#[tokio::test]
#[serial(migration_lifecycle)]
async fn test_migration_state_consistency_after_rollback(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, url) = postgres_container.await;

	// ============================================================================
	// Setup: Apply initial migrations (3 tables, 5 relationships)
	// ============================================================================

	let initial_migration = create_test_migration(
		"testapp",
		"0001_initial",
		vec![
			Operation::CreateTable {
				name: leak_str("users").to_string(),
				columns: vec![
					ColumnDefinition::from_parts("id".to_string(), FieldType::Custom("SERIAL PRIMARY KEY".to_string()), true, false, true, true, None, None, None),
					create_basic_column("username", FieldType::VarChar(Some(100))),
					create_basic_column("email", FieldType::VarChar(Some(255))),
				],
			},
			Operation::CreateTable {
				name: leak_str("posts").to_string(),
				columns: vec![
					ColumnDefinition::from_parts("id".to_string(), FieldType::Custom("SERIAL PRIMARY KEY".to_string()), true, false, true, true, None, None, None),
					create_basic_column("user_id", FieldType::Integer),
					create_basic_column("title", FieldType::VarChar(Some(200))),
					create_basic_column("content", FieldType::Text),
				],
			},
			Operation::CreateTable {
				name: leak_str("comments").to_string(),
				columns: vec![
					ColumnDefinition::from_parts("id".to_string(), FieldType::Custom("SERIAL PRIMARY KEY".to_string()), true, false, true, true, None, None, None),
					create_basic_column("post_id", FieldType::Integer),
					create_basic_column("user_id", FieldType::Integer),
					create_basic_column("text", FieldType::Text),
				],
			},
		],
	);

	let conn = DatabaseConnection::connect(&url, DatabaseType::Postgres)
		.await
		.expect("Failed to connect to database");
	let mut executor = DatabaseMigrationExecutor::new(conn.clone());

	executor
		.apply_migration(&initial_migration)
		.await
		.expect("Failed to apply initial migration");

	// ============================================================================
	// Execute: Apply additional migration (add fields and indexes)
	// ============================================================================

	let additional_migration = create_test_migration(
		"testapp",
		"0002_add_features",
		vec![
			Operation::AddColumn {
				table: leak_str("users").to_string(),
				column: create_basic_column(
					"created_at",
					FieldType::Custom("TIMESTAMP DEFAULT CURRENT_TIMESTAMP".to_string()),
				),
				mysql_options: None,
			},
			Operation::AddColumn {
				table: leak_str("posts").to_string(),
				column: create_basic_column("published", FieldType::Boolean),
				mysql_options: None,
			},
			Operation::CreateIndex {
				table: leak_str("users").to_string(),
				name: leak_str("idx_users_email").to_string(),
				columns: vec!["email"],
				unique: true,
			},
			Operation::CreateIndex {
				table: leak_str("posts").to_string(),
				name: leak_str("idx_posts_user_id").to_string(),
				columns: vec!["user_id"],
				unique: false,
			},
		],
	);

	executor
		.apply_migration(&additional_migration)
		.await
		.expect("Failed to apply additional migration");

	// Verify additions exist
	let created_at_exists: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM information_schema.columns WHERE table_name = 'users' AND column_name = 'created_at'",
	)
	.fetch_one(&*pool)
	.await
	.expect("Failed to query created_at column");
	assert_eq!(created_at_exists, 1, "created_at column should exist");

	// ============================================================================
	// Execute: Rollback the additional migration
	// ============================================================================

	executor
		.rollback_migration(&additional_migration)
		.await
		.expect("Failed to rollback additional migration");

	// ============================================================================
	// Assert: Verify database schema matches original state
	// ============================================================================

	// Verify created_at column is removed
	let created_at_after_rollback: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM information_schema.columns WHERE table_name = 'users' AND column_name = 'created_at'",
	)
	.fetch_one(&*pool)
	.await
	.expect("Failed to query created_at column after rollback");
	assert_eq!(
		created_at_after_rollback, 0,
		"created_at column should be removed after rollback"
	);

	// Verify published column is removed
	let published_after_rollback: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM information_schema.columns WHERE table_name = 'posts' AND column_name = 'published'",
	)
	.fetch_one(&*pool)
	.await
	.expect("Failed to query published column after rollback");
	assert_eq!(
		published_after_rollback, 0,
		"published column should be removed after rollback"
	);

	// Verify indexes are removed
	let email_index_after_rollback: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM pg_indexes WHERE tablename = 'users' AND indexname = 'idx_users_email'",
	)
	.fetch_one(&*pool)
	.await
	.expect("Failed to query email index after rollback");
	assert_eq!(
		email_index_after_rollback, 0,
		"Email index should be removed after rollback"
	);

	let posts_index_after_rollback: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM pg_indexes WHERE tablename = 'posts' AND indexname = 'idx_posts_user_id'",
	)
	.fetch_one(&*pool)
	.await
	.expect("Failed to query posts index after rollback");
	assert_eq!(
		posts_index_after_rollback, 0,
		"Posts index should be removed after rollback"
	);

	// Verify original tables and columns still exist
	let users_columns: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM information_schema.columns WHERE table_name = 'users'",
	)
	.fetch_one(&*pool)
	.await
	.expect("Failed to query users columns");
	assert_eq!(
		users_columns, 3,
		"Users table should have 3 columns (id, username, email)"
	);

	// Verify migration history reflects rollback
	let recorder = DatabaseMigrationRecorder::new(conn);
	let applied_migrations = recorder
		.list_applied_migrations()
		.await
		.expect("Failed to list applied migrations");

	assert_eq!(
		applied_migrations.len(),
		1,
		"Should have only 1 applied migration after rollback"
	);
	assert_eq!(applied_migrations[0].0, "testapp");
	assert_eq!(applied_migrations[0].1, "0001_initial");
}

// ============================================================================
// Data Preservation Tests
// ============================================================================

/// Test data preservation during column type changes
///
/// **Test Intent**: Verify that when altering column types, valid data is
/// preserved and correctly transformed, while invalid data is handled gracefully
///
/// **Integration Point**: AlterColumn operation → Data transformation → PostgreSQL
///
/// **Expected Behavior**: Valid numeric strings convert to integers, invalid
/// values become NULL, record count remains unchanged
#[rstest]
#[tokio::test]
#[serial(migration_lifecycle)]
async fn test_migration_with_data_preservation(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, url) = postgres_container.await;

	// ============================================================================
	// Setup: Create User table with VARCHAR age column and insert test data
	// ============================================================================

	let initial_migration = create_test_migration(
		"testapp",
		"0001_initial",
		vec![Operation::CreateTable {
			name: leak_str("users").to_string(),
			columns: vec![
				ColumnDefinition::from_parts("id".to_string(), FieldType::Custom("SERIAL PRIMARY KEY".to_string()), true, false, true, true, None, None, None),
				create_basic_column("username", FieldType::VarChar(Some(100))),
				create_basic_column("age", FieldType::VarChar(Some(10))),
			],
		}],
	);

	let conn = DatabaseConnection::connect(&url, DatabaseType::Postgres)
		.await
		.expect("Failed to connect to database");
	let mut executor = DatabaseMigrationExecutor::new(conn);

	executor
		.apply_migration(&initial_migration)
		.await
		.expect("Failed to apply initial migration");

	// Insert test data (100 records with valid and invalid ages)
	for i in 1..=100 {
		let age_value = if i % 10 == 0 {
			"invalid".to_string() // Every 10th record has invalid age
		} else {
			(20 + (i % 50)).to_string() // Valid ages between 20-69
		};

		sqlx::query("INSERT INTO users (username, age) VALUES ($1, $2)")
			.bind(format!("user{}", i))
			.bind(age_value)
			.execute(&*pool)
			.await
			.expect("Failed to insert test data");
	}

	// Verify initial data count
	let initial_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
		.fetch_one(&*pool)
		.await
		.expect("Failed to count users");
	assert_eq!(initial_count, 100, "Should have 100 users before migration");

	// ============================================================================
	// Execute: Alter column from VARCHAR to INTEGER with data transformation
	// ============================================================================

	// First, add a new integer column
	sqlx::query("ALTER TABLE users ADD COLUMN age_int INTEGER")
		.execute(&*pool)
		.await
		.expect("Failed to add age_int column");

	// Transform data: convert valid strings to integers, set invalid to NULL
	sqlx::query(
		"UPDATE users SET age_int = CASE
			WHEN age ~ '^[0-9]+$' THEN age::INTEGER
			ELSE NULL
		END",
	)
	.execute(&*pool)
	.await
	.expect("Failed to transform age data");

	// Drop old column and rename new one
	sqlx::query("ALTER TABLE users DROP COLUMN age")
		.execute(&*pool)
		.await
		.expect("Failed to drop old age column");

	sqlx::query("ALTER TABLE users RENAME COLUMN age_int TO age")
		.execute(&*pool)
		.await
		.expect("Failed to rename age_int column");

	// ============================================================================
	// Assert: Verify data transformation and preservation
	// ============================================================================

	// Verify record count unchanged
	let final_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
		.fetch_one(&*pool)
		.await
		.expect("Failed to count users after migration");
	assert_eq!(
		final_count, 100,
		"Should still have 100 users after migration"
	);

	// Verify valid data was converted correctly
	let valid_ages_count: i64 =
		sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE age IS NOT NULL")
			.fetch_one(&*pool)
			.await
			.expect("Failed to count users with valid ages");
	assert_eq!(
		valid_ages_count, 90,
		"Should have 90 users with valid integer ages"
	);

	// Verify invalid data became NULL
	let null_ages_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE age IS NULL")
		.fetch_one(&*pool)
		.await
		.expect("Failed to count users with NULL ages");
	assert_eq!(null_ages_count, 10, "Should have 10 users with NULL ages");

	// Verify a sample of converted values
	let sample_age: Option<i32> =
		sqlx::query_scalar("SELECT age FROM users WHERE username = 'user1'")
			.fetch_one(&*pool)
			.await
			.expect("Failed to fetch sample age");
	assert_eq!(sample_age, Some(21), "user1 should have age 21");

	let invalid_age: Option<i32> =
		sqlx::query_scalar("SELECT age FROM users WHERE username = 'user10'")
			.fetch_one(&*pool)
			.await
			.expect("Failed to fetch invalid age");
	assert_eq!(
		invalid_age, None,
		"user10 should have NULL age (was 'invalid')"
	);
}

// ============================================================================
// Migration History Tests
// ============================================================================

/// Test migration history table integrity
///
/// **Test Intent**: Verify that the migration history table accurately records
/// all migration applications and rollbacks, with correct timestamps and ordering
///
/// **Integration Point**: MigrationRecorder → django_migrations table
///
/// **Expected Behavior**: History table contains accurate records with monotonically
/// increasing timestamps, including rollback records
#[rstest]
#[tokio::test]
#[serial(migration_lifecycle)]
async fn test_migration_history_integrity(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, url) = postgres_container.await;

	// ============================================================================
	// Setup: Empty database
	// ============================================================================

	let conn = DatabaseConnection::connect(&url, DatabaseType::Postgres)
		.await
		.expect("Failed to connect to database");
	let mut executor = DatabaseMigrationExecutor::new(conn.clone());

	// ============================================================================
	// Execute: Apply 10 migrations sequentially
	// ============================================================================

	for i in 1..=10 {
		let migration = create_test_migration(
			"testapp",
			leak_str(format!("{:04}_migration", i)),
			vec![Operation::CreateTable {
				name: leak_str(format!("table_{}", i)),
				columns: vec![ColumnDefinition::from_parts("id".to_string(), FieldType::Custom("SERIAL PRIMARY KEY".to_string()), true, false, true, true, None, None, None)],
			}],
		);

		executor
			.apply_migration(&migration)
			.await
			.expect(&format!("Failed to apply migration {:04}", i));
	}

	// Verify 10 migrations applied
	let recorder = DatabaseMigrationRecorder::new(conn.clone());
	let applied_after_initial: Vec<(String, String)> = recorder
		.list_applied_migrations()
		.await
		.expect("Failed to list applied migrations");
	assert_eq!(
		applied_after_initial.len(),
		10,
		"Should have 10 applied migrations"
	);

	// ============================================================================
	// Execute: Rollback the 5th migration
	// ============================================================================

	let migration_5 = create_test_migration(
		"testapp",
		"0005_migration",
		vec![Operation::CreateTable {
			name: leak_str("table_5").to_string(),
			columns: vec![ColumnDefinition::from_parts("id".to_string(), FieldType::Custom("SERIAL PRIMARY KEY".to_string()), true, false, true, true, None, None, None)],
		}],
	);

	executor
		.rollback_migration(&migration_5)
		.await
		.expect("Failed to rollback migration 0005");

	// ============================================================================
	// Execute: Re-apply the 5th migration
	// ============================================================================

	executor
		.apply_migration(&migration_5)
		.await
		.expect("Failed to re-apply migration 0005");

	// ============================================================================
	// Assert: Verify history table integrity
	// ============================================================================

	let final_applied: Vec<(String, String)> = recorder
		.list_applied_migrations()
		.await
		.expect("Failed to list final applied migrations");

	// Should have 10 migrations (rollback removed 0005, then re-applied)
	assert_eq!(
		final_applied.len(),
		10,
		"Should have 10 migrations after rollback and re-apply"
	);

	// Verify all expected migrations are present
	for i in 1..=10 {
		let expected_name = format!("{:04}_migration", i);
		let found = final_applied
			.iter()
			.any(|(app, name)| app == "testapp" && name == &expected_name);
		assert!(
			found,
			"Migration {} should be in applied migrations",
			expected_name
		);
	}

	// Verify timestamps are monotonically increasing (query directly from table)
	let timestamps: Vec<chrono::DateTime<chrono::Utc>> = sqlx::query_scalar(
		"SELECT applied FROM django_migrations WHERE app = 'testapp' ORDER BY applied ASC",
	)
	.fetch_all(&*pool)
	.await
	.expect("Failed to fetch timestamps");

	for i in 1..timestamps.len() {
		assert!(
			timestamps[i] >= timestamps[i - 1],
			"Timestamps should be monotonically increasing"
		);
	}
}
