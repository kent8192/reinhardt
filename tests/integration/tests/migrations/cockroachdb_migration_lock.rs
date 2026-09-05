//! CockroachDB Migration Lock Regression Tests (Issue #4642)
//!
//! `DatabaseMigrationExecutor` previously called `pg_advisory_lock()` on the
//! generic Postgres path. CockroachDB is wire-compatible with PostgreSQL but
//! does NOT implement `pg_advisory_lock` and rejects the call with
//! `unknown function: pg_advisory_lock(): function undefined`, which made
//! every `apply_migrations` / `rollback_migrations` call against CockroachDB
//! fail at bootstrap.
//!
//! The fix routes CockroachDB connections through
//! `MigrationRecorder::ensure_schema_table_cockroachdb`, which locks a
//! sentinel row in `_reinhardt_migration_lock` via `SELECT ... FOR UPDATE`.
//!
//! These tests verify:
//!
//! 1. `DatabaseConnection::connect_postgres()` against a CockroachDB
//!    container correctly sets `is_cockroachdb() == true`.
//! 2. A migration applies and rolls back end-to-end against CockroachDB.
//! 3. Concurrent `apply_migrations` calls do not race or trigger the
//!    `pg_advisory_lock` error.
//! 4. The `_reinhardt_migration_lock` sentinel table is created and seeded.

use reinhardt_db::backends::DatabaseConnection;
use reinhardt_db::migrations::{
	ColumnDefinition, FieldType, Migration, Operation, executor::DatabaseMigrationExecutor,
};
use reinhardt_test::fixtures::testcontainers::cockroachdb_container;
use rstest::rstest;
use serial_test::serial;

// ============================================================================
// Helpers
// ============================================================================

fn create_users_migration() -> Migration {
	Migration::from_parts(
		"0001_create_users".to_string(),
		"testapp".to_string(),
		vec![Operation::CreateTable {
			name: "users_crdb_lock_test".to_string(),
			columns: vec![
				ColumnDefinition::from_parts(
					"id".to_string(),
					FieldType::Integer,
					true,
					false,
					true,
					true,
					None,
					None,
					None,
				),
				ColumnDefinition::from_parts(
					"name".to_string(),
					FieldType::VarChar(100),
					true,
					false,
					false,
					false,
					None,
					None,
					None,
				),
			],
			constraints: vec![],
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		}],
		vec![],
		vec![],
		true,
		None,
		false,
		false,
		vec![],
		vec![],
	)
}

/// Check whether `table_name` exists in the CockroachDB connection.
///
/// Panics on query error so a misconfigured connection (RBAC, network, etc.)
/// fails loudly with a diagnostic message instead of being silently coerced
/// into `false`, which would otherwise produce misleading assertion output
/// like "expected table to exist, found absent" when the real cause is a
/// driver-level error.
async fn table_exists(connection: &DatabaseConnection, table_name: &str) -> bool {
	let pool = connection
		.into_postgres()
		.expect("postgres pool unavailable on CockroachDB connection");
	sqlx::query_scalar::<_, bool>(
		"SELECT EXISTS(SELECT 1 FROM information_schema.tables \
		 WHERE table_schema = 'public' AND table_name = $1)",
	)
	.bind(table_name)
	.fetch_one(&pool)
	.await
	.unwrap_or_else(|e| {
		panic!("table_exists({table_name:?}) query failed against CockroachDB: {e}")
	})
}

/// Check whether the sentinel row `id = 1` exists in
/// `_reinhardt_migration_lock`. Only the row presence — combined with
/// `SELECT ... FOR UPDATE` of that row — serialises concurrent migrators,
/// so a regression in the seed `INSERT ... ON CONFLICT DO NOTHING` must
/// fail the test rather than slip through a table-only check.
async fn sentinel_row_exists(connection: &DatabaseConnection) -> bool {
	let pool = connection
		.into_postgres()
		.expect("postgres pool unavailable on CockroachDB connection");
	sqlx::query_scalar::<_, bool>(
		"SELECT EXISTS(SELECT 1 FROM _reinhardt_migration_lock WHERE id = 1)",
	)
	.fetch_one(&pool)
	.await
	.unwrap_or_else(|e| panic!("sentinel_row_exists query failed against CockroachDB: {e}"))
}

// ============================================================================
// Regression tests for #4642
// ============================================================================

/// `DatabaseConnection::connect_postgres()` must detect CockroachDB so the
/// migration-lock dispatch can route around `pg_advisory_lock`.
#[rstest]
#[tokio::test]
#[serial(cockroachdb_migration_lock)]
async fn test_database_connection_detects_cockroachdb() {
	// Arrange
	let (_container, _pool, _port, url) = cockroachdb_container().await;

	// Act
	let connection = DatabaseConnection::connect_postgres(&url)
		.await
		.expect("Failed to connect to CockroachDB via connect_postgres");

	// Assert
	assert!(
		connection.is_cockroachdb(),
		"connect_postgres against a CockroachDB container should report \
		 is_cockroachdb() == true"
	);
}

/// Primary regression: `apply_migrations` and `rollback_migrations` must
/// succeed end-to-end against CockroachDB. Previously failed with
/// `unknown function: pg_advisory_lock(): function undefined`.
#[rstest]
#[tokio::test]
#[serial(cockroachdb_migration_lock)]
async fn test_apply_and_rollback_migrations_on_cockroachdb() {
	// Arrange
	let (_container, _pool, _port, url) = cockroachdb_container().await;
	let connection = DatabaseConnection::connect_postgres(&url)
		.await
		.expect("Failed to connect to CockroachDB");
	let mut executor = DatabaseMigrationExecutor::new(connection.clone());
	let migration = create_users_migration();

	// Act: forward
	executor
		.apply_migrations(std::slice::from_ref(&migration))
		.await
		.expect("apply_migrations should not fail with pg_advisory_lock error");

	// Assert: target table exists
	assert!(
		table_exists(&connection, "users_crdb_lock_test").await,
		"users_crdb_lock_test should exist after apply_migrations on CockroachDB"
	);

	// Assert: the sentinel lock table was created by the new lock path
	assert!(
		table_exists(&connection, "_reinhardt_migration_lock").await,
		"_reinhardt_migration_lock sentinel table should exist after a \
		 CockroachDB migration acquires the schema lock"
	);

	// Assert: the sentinel row itself is present. `SELECT ... FOR UPDATE`
	// only serialises migrators when the `id = 1` row exists — a regression
	// in the seed `INSERT ... DO NOTHING` would silently lose the locking
	// guarantee, so we require row presence, not just table presence.
	assert!(
		sentinel_row_exists(&connection).await,
		"_reinhardt_migration_lock should contain the sentinel row (id = 1) \
		 after a CockroachDB migration acquires the schema lock"
	);

	// Act: rollback
	executor
		.rollback_migrations(std::slice::from_ref(&migration))
		.await
		.expect("rollback_migrations should not fail with pg_advisory_lock error");

	// Assert: table dropped
	assert!(
		!table_exists(&connection, "users_crdb_lock_test").await,
		"users_crdb_lock_test should not exist after rollback on CockroachDB"
	);
}

/// Two concurrent `apply_migrations` callers must both succeed. The sentinel
/// row serialises them; previously they both failed immediately at
/// `pg_advisory_lock`.
#[rstest]
#[tokio::test]
#[serial(cockroachdb_migration_lock)]
async fn test_concurrent_apply_migrations_serializes_on_cockroachdb() {
	// Arrange — share one container, but each task owns its own connection
	// (mirrors two real `migrate` processes running concurrently).
	let (_container, _pool, _port, url) = cockroachdb_container().await;

	let conn_a = DatabaseConnection::connect_postgres(&url)
		.await
		.expect("conn_a");
	let conn_b = DatabaseConnection::connect_postgres(&url)
		.await
		.expect("conn_b");

	// Distinct table names so neither task fails on table-already-exists —
	// the test is about the lock path, not idempotency of CREATE TABLE.
	let mut migration_a = create_users_migration();
	migration_a.name = "0001_create_users_a".to_string();
	if let Operation::CreateTable { ref mut name, .. } = migration_a.operations[0] {
		*name = "users_crdb_concurrent_a".to_string();
	}
	let mut migration_b = create_users_migration();
	migration_b.name = "0001_create_users_b".to_string();
	if let Operation::CreateTable { ref mut name, .. } = migration_b.operations[0] {
		*name = "users_crdb_concurrent_b".to_string();
	}

	// Act
	let task_a = tokio::spawn(async move {
		let mut exec = DatabaseMigrationExecutor::new(conn_a);
		exec.apply_migrations(&[migration_a]).await
	});
	let task_b = tokio::spawn(async move {
		let mut exec = DatabaseMigrationExecutor::new(conn_b);
		exec.apply_migrations(&[migration_b]).await
	});

	let result_a = task_a.await.expect("task_a join");
	let result_b = task_b.await.expect("task_b join");

	// Assert: both succeed without surfacing pg_advisory_lock errors
	assert!(
		result_a.is_ok(),
		"concurrent apply_migrations task A failed: {:?}",
		result_a.err()
	);
	assert!(
		result_b.is_ok(),
		"concurrent apply_migrations task B failed: {:?}",
		result_b.err()
	);

	// Assert: both tables exist after the dust settles
	let verify_conn = DatabaseConnection::connect_postgres(&url)
		.await
		.expect("verify_conn");
	assert!(
		table_exists(&verify_conn, "users_crdb_concurrent_a").await,
		"users_crdb_concurrent_a should exist"
	);
	assert!(
		table_exists(&verify_conn, "users_crdb_concurrent_b").await,
		"users_crdb_concurrent_b should exist"
	);
}
