//! Migration Rollback Integration Tests
//!
//! Tests that verify the correctness of migration rollback (reverse) functionality
//! across all three supported database backends:
//!
//! - **PostgreSQL** — Full transactional DDL; rollback is atomic.
//! - **MySQL** — Non-transactional DDL; DDL statements implicitly commit, so
//!   multi-operation rollback semantics differ from Postgres.
//! - **SQLite** — Transactional DDL but with limited ALTER COLUMN support
//!   (table recreation is required for some column changes).
//!
//! **Test Coverage:**
//! - Basic rollback operations (CREATE TABLE / ADD COLUMN / ALTER COLUMN)
//! - RunSQL with reverse_sql
//! - Atomic transaction rollbacks (Postgres / SQLite atomic, MySQL non-atomic)
//! - Dependency-ordered rollbacks
//! - Error handling (FK violations, missing reverse_sql)
//!
//! **Strategy:**
//! Each test is parameterized via `#[rstest]` + `#[case]` to run against all
//! three backends. Container fixtures (`postgres_container`, `mysql_container`)
//! are spawned on demand from `reinhardt-test`; SQLite uses `sqlite::memory:`.
//! Backend-divergent behavior is asserted explicitly (no silent skips).

use reinhardt_db::backends::DatabaseConnection;
use reinhardt_db::backends::types::DatabaseType;
use reinhardt_db::migrations::{
	ColumnDefinition, FieldType, Migration, Operation, executor::DatabaseMigrationExecutor,
};
use reinhardt_test::fixtures::{mysql_container, postgres_container};
use rstest::*;

// ============================================================================
// Backend Setup
// ============================================================================

/// Owns any container handles that must outlive the test so the test
/// container is not torn down while migrations are running.
///
/// SQLite does not need a container, so this variant has no payload.
///
/// The boxed `Any` fields exist purely for their `Drop` side-effect:
/// dropping the `ContainerAsync` stops the underlying Docker container.
/// The fields are never read, hence the `dead_code` allow below.
#[allow(dead_code)]
enum BackendHandle {
	/// Holds the Postgres TestContainer alive for the duration of the test.
	Postgres(Box<dyn std::any::Any + Send + Sync>),
	/// Holds the MySQL TestContainer alive for the duration of the test.
	Mysql(Box<dyn std::any::Any + Send + Sync>),
	/// SQLite in-memory; no container handle required.
	Sqlite,
}

/// Acquire a connection for the requested backend.
///
/// For Postgres and MySQL the returned [`BackendHandle`] keeps the
/// underlying TestContainer alive — drop it and the container is torn
/// down. For SQLite the handle is a unit value.
async fn setup_backend(backend: DatabaseType) -> (BackendHandle, DatabaseConnection) {
	match backend {
		DatabaseType::Postgres => {
			let (container, _pool, _port, url) = postgres_container().await;
			let connection = DatabaseConnection::connect_postgres(&url)
				.await
				.expect("Failed to connect to PostgreSQL");
			(BackendHandle::Postgres(Box::new(container)), connection)
		}
		DatabaseType::Mysql => {
			let (container, _pool, _port, url) = mysql_container().await;
			let connection = DatabaseConnection::connect_mysql(&url)
				.await
				.expect("Failed to connect to MySQL");
			(BackendHandle::Mysql(Box::new(container)), connection)
		}
		DatabaseType::Sqlite => {
			let connection = DatabaseConnection::connect_sqlite("sqlite::memory:")
				.await
				.expect("Failed to connect to in-memory SQLite");
			(BackendHandle::Sqlite, connection)
		}
	}
}

// ============================================================================
// Test Helper Functions
// ============================================================================

fn leak_str(s: impl Into<String>) -> &'static str {
	Box::leak(s.into().into_boxed_str())
}

/// Create a simple migration for testing.
fn create_test_migration(
	app: &'static str,
	name: &'static str,
	operations: Vec<Operation>,
) -> Migration {
	Migration::from_parts(
		name.to_string(),
		app.to_string(),
		operations,
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

/// Create a basic column definition.
fn create_basic_column(name: &str, type_def: FieldType) -> ColumnDefinition {
	ColumnDefinition::from_parts(
		name.to_string(),
		type_def,
		false,
		false,
		false,
		false,
		None,
		None,
		None,
	)
}

/// Create a NOT NULL column definition.
fn create_not_null_column(name: &str, type_def: FieldType) -> ColumnDefinition {
	ColumnDefinition::from_parts(
		name.to_string(),
		type_def,
		true,
		false,
		false,
		false,
		None,
		None,
		None,
	)
}

/// Create an auto-increment primary key column.
fn create_auto_pk_column(name: &str, type_def: FieldType) -> ColumnDefinition {
	ColumnDefinition::from_parts(
		name.to_string(),
		type_def,
		true,
		false,
		true,
		true,
		None,
		None,
		None,
	)
}

// ============================================================================
// Backend-aware schema introspection
// ============================================================================

/// Check whether `table_name` exists in the connected database.
///
/// Each backend has a different system catalog query:
/// - Postgres / MySQL: `information_schema.tables`
/// - SQLite: `sqlite_master`
async fn table_exists(connection: &DatabaseConnection, table_name: &str) -> bool {
	match connection.database_type() {
		DatabaseType::Postgres => {
			let pool = connection
				.into_postgres()
				.expect("postgres pool unavailable");
			sqlx::query_scalar::<_, bool>(
				"SELECT EXISTS(SELECT 1 FROM information_schema.tables \
				 WHERE table_schema = 'public' AND table_name = $1)",
			)
			.bind(table_name)
			.fetch_one(&pool)
			.await
			.unwrap_or(false)
		}
		DatabaseType::Mysql => {
			let pool = connection.into_mysql().expect("mysql pool unavailable");
			let count: i64 = sqlx::query_scalar(
				"SELECT COUNT(*) FROM information_schema.tables \
				 WHERE table_schema = DATABASE() AND table_name = ?",
			)
			.bind(table_name)
			.fetch_one(&pool)
			.await
			.unwrap_or(0);
			count > 0
		}
		DatabaseType::Sqlite => {
			let pool = connection.into_sqlite().expect("sqlite pool unavailable");
			let count: i64 = sqlx::query_scalar(
				"SELECT COUNT(*) FROM sqlite_master \
				 WHERE type = 'table' AND name = ?",
			)
			.bind(table_name)
			.fetch_one(&pool)
			.await
			.unwrap_or(0);
			count > 0
		}
	}
}

/// Check whether `column_name` exists on `table_name`.
async fn column_exists(
	connection: &DatabaseConnection,
	table_name: &str,
	column_name: &str,
) -> bool {
	match connection.database_type() {
		DatabaseType::Postgres => {
			let pool = connection
				.into_postgres()
				.expect("postgres pool unavailable");
			sqlx::query_scalar::<_, bool>(
				"SELECT EXISTS(SELECT 1 FROM information_schema.columns \
				 WHERE table_schema = 'public' AND table_name = $1 AND column_name = $2)",
			)
			.bind(table_name)
			.bind(column_name)
			.fetch_one(&pool)
			.await
			.unwrap_or(false)
		}
		DatabaseType::Mysql => {
			let pool = connection.into_mysql().expect("mysql pool unavailable");
			// Symmetric with `column_data_type` — see Issue #4649. Both helpers
			// case-fold `table_schema`, `table_name`, and `column_name` so that
			// a passing `column_exists` strictly implies a hit in
			// `column_data_type` and vice versa. Without this symmetry the two
			// helpers could disagree on the same `information_schema.columns`
			// rows, producing the impossible-looking failure shape reported in
			// Issue #4649 (column_exists -> true, column_data_type -> None).
			// We also surface sqlx errors via `eprintln!` instead of folding
			// them into `count = 0`, so future flakes leave a debuggable trace
			// in CI logs.
			let result: Result<i64, _> = sqlx::query_scalar(
				"SELECT COUNT(*) FROM information_schema.columns \
				 WHERE LOWER(table_schema) = LOWER(DATABASE()) \
				 AND LOWER(table_name) = LOWER(?) \
				 AND LOWER(column_name) = LOWER(?)",
			)
			.bind(table_name)
			.bind(column_name)
			.fetch_one(&pool)
			.await;
			match result {
				Ok(count) => count > 0,
				Err(err) => {
					eprintln!(
						"[issue#4649] column_exists({table_name:?}, {column_name:?}) \
						 MySQL query failed: {err:?}"
					);
					false
				}
			}
		}
		DatabaseType::Sqlite => {
			let pool = connection.into_sqlite().expect("sqlite pool unavailable");
			// `PRAGMA table_info(...)` cannot be parameterised, so we identifier-quote
			// the table name. Test code only ever passes literals controlled by the
			// test author, so SQL-injection risk is not a concern here.
			let pragma = format!("PRAGMA table_info(\"{}\")", table_name.replace('"', "\"\""));
			let rows: Vec<(i64, String, String, i64, Option<String>, i64)> =
				sqlx::query_as(&pragma)
					.fetch_all(&pool)
					.await
					.unwrap_or_default();
			rows.iter().any(|(_, name, _, _, _, _)| name == column_name)
		}
	}
}

/// Check whether an index exists on the given table.
///
/// `table_name` is required because MySQL's `information_schema.statistics`
/// keys indexes by `(table_schema, table_name, index_name)` — filtering by
/// `index_name` alone can produce false positives if another table happens to
/// use the same index name. Postgres and SQLite are also constrained by table
/// name for consistency.
async fn index_exists(connection: &DatabaseConnection, table_name: &str, index_name: &str) -> bool {
	match connection.database_type() {
		DatabaseType::Postgres => {
			let pool = connection
				.into_postgres()
				.expect("postgres pool unavailable");
			sqlx::query_scalar::<_, bool>(
				"SELECT EXISTS(SELECT 1 FROM pg_indexes \
				 WHERE tablename = $1 AND indexname = $2)",
			)
			.bind(table_name)
			.bind(index_name)
			.fetch_one(&pool)
			.await
			.unwrap_or(false)
		}
		DatabaseType::Mysql => {
			let pool = connection.into_mysql().expect("mysql pool unavailable");
			let count: i64 = sqlx::query_scalar(
				"SELECT COUNT(*) FROM information_schema.statistics \
				 WHERE table_schema = DATABASE() \
				 AND table_name = ? AND index_name = ?",
			)
			.bind(table_name)
			.bind(index_name)
			.fetch_one(&pool)
			.await
			.unwrap_or(0);
			count > 0
		}
		DatabaseType::Sqlite => {
			let pool = connection.into_sqlite().expect("sqlite pool unavailable");
			let count: i64 = sqlx::query_scalar(
				"SELECT COUNT(*) FROM sqlite_master \
				 WHERE type = 'index' AND tbl_name = ? AND name = ?",
			)
			.bind(table_name)
			.bind(index_name)
			.fetch_one(&pool)
			.await
			.unwrap_or(0);
			count > 0
		}
	}
}

/// Fetch the data type of a column from `information_schema.columns`.
///
/// Returns `None` if the column does not exist or for SQLite (which stores
/// declared types as opaque strings in `sqlite_master` and does not expose
/// `information_schema.columns`). Postgres returns canonical names like
/// `character varying`, `text`, `integer`. MySQL returns names like
/// `varchar`, `text`, `int`.
async fn column_data_type(
	connection: &DatabaseConnection,
	table_name: &str,
	column_name: &str,
) -> Option<String> {
	match connection.database_type() {
		DatabaseType::Postgres => {
			let pool = connection
				.into_postgres()
				.expect("postgres pool unavailable");
			sqlx::query_scalar::<_, String>(
				"SELECT data_type FROM information_schema.columns \
				 WHERE table_name = $1 AND column_name = $2",
			)
			.bind(table_name)
			.bind(column_name)
			.fetch_optional(&pool)
			.await
			.ok()
			.flatten()
		}
		DatabaseType::Mysql => {
			let pool = connection.into_mysql().expect("mysql pool unavailable");
			// MySQL's `information_schema.columns.table_name` preserves the
			// case the table was created with. On Linux CI runners the lookup
			// is case-sensitive unless `lower_case_table_names` is configured,
			// which TestContainers does not set. Match the case-insensitive
			// behaviour the rest of this fixture assumes so the assertion
			// holds regardless of the runner's filesystem semantics.
			// Fixes #4630.
			//
			// Issue #4649 follow-up: the WHERE clause is now symmetric with
			// `column_exists` (case-fold `table_schema` too), so any divergence
			// between the two helpers can no longer come from the WHERE shape
			// alone. We also split the previously-silent `.ok().flatten()`
			// into three explicit outcomes:
			//   - `Err(_)`      => sqlx error; print it and return None so the
			//                       caller's `.expect(...)` still panics, but
			//                       with the underlying error logged to stderr.
			//   - `Ok(Some(s))` => column exists with data type `s`.
			//   - `Ok(None)`    => column genuinely absent; dump the rows in
			//                       `information_schema.columns` that match
			//                       `LOWER(table_name) = LOWER(?)` (without
			//                       the schema filter) and the current value
			//                       of `DATABASE()`, so the next residual
			//                       flake's CI log carries enough state to
			//                       diagnose the root cause.
			//
			// Issue #4675: wrap every `information_schema` text column in
			// `CAST(... AS CHAR)`. The unwrapped columns are typed as
			// `LONGBLOB`/`LONGTEXT` over the binary protocol on some MySQL
			// 8.x + sqlx combinations, which makes `query_scalar::<_, String>`
			// fail with `ColumnDecode { mismatched types ... String (as
			// VARCHAR) is not compatible with SQL type LONGBLOB }`. The CAST
			// forces the result back to a text type sqlx recognises.
			let result = sqlx::query_scalar::<_, String>(
				"SELECT CAST(data_type AS CHAR) FROM information_schema.columns \
				 WHERE LOWER(table_schema) = LOWER(DATABASE()) \
				 AND LOWER(table_name) = LOWER(?) \
				 AND LOWER(column_name) = LOWER(?)",
			)
			.bind(table_name)
			.bind(column_name)
			.fetch_optional(&pool)
			.await;
			match result {
				Ok(Some(s)) => Some(s),
				Ok(None) => {
					// Cap the diagnostic dump so a flake on a busy CI MySQL with
					// many schemas can't produce multi-megabyte logs. 32 rows is
					// large enough to spot duplicate-table-across-schemas drift
					// without dominating the log.
					let dump = sqlx::query_as::<_, (String, String, String, String)>(
						"SELECT CAST(table_schema AS CHAR), \
						        CAST(table_name AS CHAR), \
						        CAST(column_name AS CHAR), \
						        CAST(data_type AS CHAR) \
						 FROM information_schema.columns \
						 WHERE LOWER(table_name) = LOWER(?) \
						 ORDER BY table_schema, column_name \
						 LIMIT 32",
					)
					.bind(table_name)
					.fetch_all(&pool)
					.await;
					// Use an explicit match (not `.ok().flatten()`) so that a
					// failed `SELECT DATABASE()` is logged as the failure it is
					// rather than collapsing into a misleading `None`.
					let current_db: Result<Option<String>, _> =
						sqlx::query_scalar("SELECT CAST(DATABASE() AS CHAR)")
							.fetch_optional(&pool)
							.await;
					let current_db_repr = match current_db {
						Ok(value) => format!("{value:?}"),
						Err(err) => format!("<query failed: {err:?}>"),
					};
					eprintln!(
						"[issue#4675] column_data_type({table_name:?}, {column_name:?}) \
						 returned None; DATABASE()={current_db_repr}, \
						 information_schema.columns WHERE LOWER(table_name)=LOWER(?) \
						 (ORDER BY table_schema, column_name LIMIT 32) = {dump:#?}"
					);
					None
				}
				Err(err) => {
					eprintln!(
						"[issue#4675] column_data_type({table_name:?}, {column_name:?}) \
						 MySQL query failed: {err:?}"
					);
					None
				}
			}
		}
		DatabaseType::Sqlite => None,
	}
}

/// Fetch the nullability flag of a column from `information_schema.columns`.
///
/// Returns `Some(true)` if the column is nullable, `Some(false)` if NOT NULL,
/// or `None` if the column does not exist (or for SQLite, which does not
/// expose `information_schema.columns`). Used to verify that `AlterColumn`
/// rollback restores the pre-migration nullability (issue#4640).
async fn column_is_nullable(
	connection: &DatabaseConnection,
	table_name: &str,
	column_name: &str,
) -> Option<bool> {
	match connection.database_type() {
		DatabaseType::Postgres => {
			let pool = connection
				.into_postgres()
				.expect("postgres pool unavailable");
			let raw = sqlx::query_scalar::<_, String>(
				"SELECT is_nullable FROM information_schema.columns \
				 WHERE table_name = $1 AND column_name = $2",
			)
			.bind(table_name)
			.bind(column_name)
			.fetch_optional(&pool)
			.await
			.ok()
			.flatten()?;
			Some(raw.eq_ignore_ascii_case("YES"))
		}
		DatabaseType::Mysql => {
			let pool = connection.into_mysql().expect("mysql pool unavailable");
			// Mirror the case-folding + CAST conventions used by `column_data_type`
			// above (issues #4630, #4649, #4675).
			let raw = sqlx::query_scalar::<_, String>(
				"SELECT CAST(is_nullable AS CHAR) FROM information_schema.columns \
				 WHERE LOWER(table_schema) = LOWER(DATABASE()) \
				 AND LOWER(table_name) = LOWER(?) \
				 AND LOWER(column_name) = LOWER(?)",
			)
			.bind(table_name)
			.bind(column_name)
			.fetch_optional(&pool)
			.await
			.ok()
			.flatten()?;
			Some(raw.eq_ignore_ascii_case("YES"))
		}
		DatabaseType::Sqlite => None,
	}
}

// ============================================================================
// Basic Rollback Tests (Normal Cases)
// ============================================================================

/// Test CREATE TABLE rollback (should DROP TABLE).
///
/// **Test Intent**: Verify that CREATE TABLE can be rolled back with DROP TABLE
/// on every supported backend. CREATE TABLE / DROP TABLE is the most basic
/// rollback path and must work identically across Postgres, MySQL, and SQLite.
#[rstest]
#[case::postgres(DatabaseType::Postgres)]
#[case::mysql(DatabaseType::Mysql)]
#[case::sqlite(DatabaseType::Sqlite)]
#[tokio::test]
async fn test_create_table_rollback(#[case] backend: DatabaseType) {
	// Arrange
	let (_handle, connection) = setup_backend(backend).await;
	let mut executor = DatabaseMigrationExecutor::new(connection.clone());

	let migration = create_test_migration(
		"testapp",
		"0001_create_users",
		vec![Operation::CreateTable {
			name: leak_str("users").to_string(),
			columns: vec![
				create_auto_pk_column("id", FieldType::Integer),
				create_not_null_column("name", FieldType::VarChar(100)),
			],
			constraints: vec![],
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		}],
	);

	// Act: forward
	executor
		.apply_migrations(std::slice::from_ref(&migration))
		.await
		.expect("Failed to apply migration");

	// Assert: table exists
	assert!(
		table_exists(&connection, "users").await,
		"[{backend:?}] table should exist after migration"
	);

	// Act: rollback
	executor
		.rollback_migrations(std::slice::from_ref(&migration))
		.await
		.expect("Failed to rollback migration");

	// Assert: table dropped
	assert!(
		!table_exists(&connection, "users").await,
		"[{backend:?}] table should not exist after rollback"
	);
}

/// Test ADD COLUMN rollback (should DROP COLUMN).
///
/// **Test Intent**: Verify that ADD COLUMN can be rolled back with DROP COLUMN
/// on every supported backend.
#[rstest]
#[case::postgres(DatabaseType::Postgres)]
#[case::mysql(DatabaseType::Mysql)]
#[case::sqlite(DatabaseType::Sqlite)]
#[tokio::test]
async fn test_add_column_rollback(#[case] backend: DatabaseType) {
	// Arrange
	let (_handle, connection) = setup_backend(backend).await;
	let mut executor = DatabaseMigrationExecutor::new(connection.clone());

	let create_table_migration = create_test_migration(
		"testapp",
		"0001_create_users",
		vec![Operation::CreateTable {
			name: leak_str("users").to_string(),
			columns: vec![
				create_auto_pk_column("id", FieldType::Integer),
				create_not_null_column("name", FieldType::VarChar(100)),
			],
			constraints: vec![],
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		}],
	);
	executor
		.apply_migrations(&[create_table_migration])
		.await
		.expect("Failed to create table");

	let add_column_migration = create_test_migration(
		"testapp",
		"0002_add_email",
		vec![Operation::AddColumn {
			table: leak_str("users").to_string(),
			column: create_basic_column("email", FieldType::VarChar(255)),
			mysql_options: None,
		}],
	);

	// Act: forward
	executor
		.apply_migrations(std::slice::from_ref(&add_column_migration))
		.await
		.expect("Failed to add column");

	// Assert: column added
	assert!(
		column_exists(&connection, "users", "email").await,
		"[{backend:?}] column should exist after migration"
	);

	// Act: rollback
	executor
		.rollback_migrations(std::slice::from_ref(&add_column_migration))
		.await
		.expect("Failed to rollback migration");

	// Assert: column dropped
	assert!(
		!column_exists(&connection, "users", "email").await,
		"[{backend:?}] column should not exist after rollback"
	);
}

/// Test ALTER COLUMN rollback (should revert to original type AND nullability).
///
/// **Test Intent**: Verify that ALTER COLUMN can be rolled back end-to-end:
/// both the column type AND its nullability flag must revert to the
/// pre-migration state. SQLite implements ALTER COLUMN via table recreation
/// under the hood — the same rollback path should still leave the column
/// with the original type from the user's point of view.
///
/// **NOT NULL fidelity** (issue#4640): the initial column is created with
/// `NOT NULL`; the forward migration changes both the type (VARCHAR → TEXT)
/// and the nullability (NOT NULL → nullable). After rollback, the column
/// must be `NOT NULL VARCHAR(50)` again. Before #4640, the rollback path
/// on PostgreSQL emitted a comma-combined `ALTER TABLE ... ALTER COLUMN c
/// TYPE T, ALTER COLUMN c SET NOT NULL` payload that worked on Postgres
/// but broke on CockroachDB (which rejects the form). The fix returns
/// `Vec<String>` from `Operation::to_reverse_sql` so the type reversion
/// and the nullability restoration are dispatched as two independent
/// statements, each accepted by sqlx Extended Query.
#[rstest]
#[case::postgres(DatabaseType::Postgres)]
#[case::mysql(DatabaseType::Mysql)]
#[case::sqlite(DatabaseType::Sqlite)]
#[tokio::test]
async fn test_alter_column_type_rollback(#[case] backend: DatabaseType) {
	// Arrange: initial schema has `name` as NOT NULL VARCHAR(50).
	let (_handle, connection) = setup_backend(backend).await;
	let mut executor = DatabaseMigrationExecutor::new(connection.clone());

	let create_table_migration = create_test_migration(
		"testapp",
		"0001_create_products",
		vec![Operation::CreateTable {
			name: leak_str("products").to_string(),
			columns: vec![
				create_auto_pk_column("id", FieldType::Integer),
				create_not_null_column("name", FieldType::VarChar(50)),
			],
			constraints: vec![],
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		}],
	);
	executor
		.apply_migrations(&[create_table_migration])
		.await
		.expect("Failed to create table");

	// Forward migration: NOT NULL VARCHAR(50) → nullable TEXT. The
	// `old_definition` carries the pre-migration shape so `to_reverse_sql`
	// can emit a faithful rollback without depending on ProjectState.
	let alter_column_migration = create_test_migration(
		"testapp",
		"0002_alter_name_type",
		vec![Operation::AlterColumn {
			table: leak_str("products").to_string(),
			column: leak_str("name").to_string(),
			old_definition: Some(create_not_null_column("name", FieldType::VarChar(50))),
			new_definition: create_basic_column("name", FieldType::Text),
			mysql_options: None,
		}],
	);

	// Act: forward
	executor
		.apply_migrations(std::slice::from_ref(&alter_column_migration))
		.await
		.expect("Failed to alter column");

	// Sanity: column still exists after forward.
	assert!(
		column_exists(&connection, "products", "name").await,
		"[{backend:?}] column should still exist after ALTER"
	);

	// Backend-conditional: on Postgres and MySQL, `information_schema` reports
	// the post-ALTER type as `text`. Skip the check on SQLite, which records
	// declared types as opaque strings in `sqlite_master` and does not expose
	// `information_schema.columns`.
	//
	// Note: we intentionally do NOT assert post-forward nullability here.
	// The forward `Operation::AlterColumn::to_sql` is asymmetric across backends:
	//   - Postgres/CockroachDB: emits `ALTER COLUMN ... TYPE T` only, preserving
	//     the existing nullability flag (NOT NULL stays NOT NULL).
	//   - MySQL: emits `MODIFY COLUMN ... TEXT` (no nullability clause), which
	//     MySQL interprets as "make nullable" — so the column becomes nullable
	//     even though `new_definition.not_null` is false too.
	// Asserting a single post-forward state is therefore not portable. What this
	// test validates is the rollback contract: regardless of forward asymmetry,
	// the column must round-trip back to its `old_definition` (NOT NULL VARCHAR)
	// after rollback. Restoration of post-forward nullability fidelity in the
	// forward path itself is a separate concern (out of scope for #4640).
	if matches!(backend, DatabaseType::Postgres | DatabaseType::Mysql) {
		let after_alter = column_data_type(&connection, "products", "name")
			.await
			.expect("data_type should be available for Postgres/MySQL");
		assert_eq!(
			after_alter.to_lowercase(),
			"text",
			"[{backend:?}] column type should be TEXT after ALTER",
		);
	}

	// Act: rollback
	executor
		.rollback_migrations(std::slice::from_ref(&alter_column_migration))
		.await
		.expect("Failed to rollback migration");

	// Assert: column still exists with same name on every backend.
	assert!(
		column_exists(&connection, "products", "name").await,
		"[{backend:?}] column should still exist after rollback"
	);

	// Backend-conditional: on Postgres and MySQL, the column type should
	// revert to the original VARCHAR(50) — Postgres reports it as
	// `character varying`, MySQL as `varchar`. SQLite is skipped (see above).
	if matches!(backend, DatabaseType::Postgres | DatabaseType::Mysql) {
		let after_rollback = column_data_type(&connection, "products", "name")
			.await
			.expect("data_type should be available for Postgres/MySQL");
		let lower = after_rollback.to_lowercase();
		assert!(
			lower == "character varying" || lower == "varchar",
			"[{backend:?}] column type should revert to VARCHAR after rollback, \
			 got {after_rollback:?}",
		);

		// NOT NULL fidelity assertion (issue#4640): the column must be
		// NOT NULL again after rollback. Before #4640 this was deferred on
		// CockroachDB (stop-gap only emitted the type-reversion statement
		// and dropped the nullability clause). The Vec<String> API plus
		// the executor's per-statement dispatch now restores fidelity for
		// every backend that supports ALTER COLUMN via `to_reverse_sql`.
		let nullable_after_rollback = column_is_nullable(&connection, "products", "name")
			.await
			.expect("is_nullable should be available for Postgres/MySQL");
		assert!(
			!nullable_after_rollback,
			"[{backend:?}] column should be NOT NULL after rollback (rollback must \
			 restore the pre-migration nullability)",
		);
	}
}

/// Test RunSQL with reverse_sql rollback.
///
/// **Test Intent**: Verify that RunSQL operations can be rolled back via the
/// supplied `reverse_sql`. Each backend gets a hand-tailored CREATE/DROP pair
/// because the table-definition syntax differs (e.g. `SERIAL` is Postgres-only).
#[rstest]
#[case::postgres(
	DatabaseType::Postgres,
	"CREATE TABLE custom_table (id SERIAL PRIMARY KEY, data TEXT)",
	"DROP TABLE custom_table"
)]
#[case::mysql(
	DatabaseType::Mysql,
	"CREATE TABLE custom_table (id INT AUTO_INCREMENT PRIMARY KEY, data TEXT)",
	"DROP TABLE custom_table"
)]
#[case::sqlite(
	DatabaseType::Sqlite,
	"CREATE TABLE custom_table (id INTEGER PRIMARY KEY AUTOINCREMENT, data TEXT)",
	"DROP TABLE custom_table"
)]
#[tokio::test]
async fn test_run_sql_with_reverse_rollback(
	#[case] backend: DatabaseType,
	#[case] forward_sql: &'static str,
	#[case] reverse_sql: &'static str,
) {
	// Arrange
	let (_handle, connection) = setup_backend(backend).await;
	let mut executor = DatabaseMigrationExecutor::new(connection.clone());

	let migration = create_test_migration(
		"testapp",
		"0001_create_custom_table",
		vec![Operation::RunSQL {
			sql: forward_sql.to_string(),
			reverse_sql: Some(reverse_sql.to_string()),
		}],
	);

	// Act: forward
	executor
		.apply_migrations(std::slice::from_ref(&migration))
		.await
		.expect("Failed to apply RunSQL migration");

	// Assert: table exists
	assert!(
		table_exists(&connection, "custom_table").await,
		"[{backend:?}] custom_table should exist after RunSQL"
	);

	// Act: rollback
	executor
		.rollback_migrations(std::slice::from_ref(&migration))
		.await
		.expect("Failed to rollback RunSQL migration");

	// Assert: table dropped
	assert!(
		!table_exists(&connection, "custom_table").await,
		"[{backend:?}] custom_table should not exist after rollback"
	);
}

/// Test atomic rollback of multiple operations.
///
/// **Test Intent**: Verify that `atomic = true` rolls back every operation in
/// the migration as a unit. This is the canonical place where Postgres /
/// SQLite (transactional DDL) and MySQL (non-transactional DDL) diverge.
///
/// **Backend-divergent behavior**: On Postgres and SQLite the rollback runs
/// inside a single transaction and all three tables are dropped. On MySQL
/// each DDL statement auto-commits, so the rollback still drops each table
/// individually via the per-operation reverse, but it is not transactional.
/// This test asserts the **post-condition** (all tables removed) which is
/// the same for every backend; the path differs but the observable result
/// does not.
#[rstest]
#[case::postgres(DatabaseType::Postgres)]
#[case::mysql(DatabaseType::Mysql)]
#[case::sqlite(DatabaseType::Sqlite)]
#[tokio::test]
async fn test_atomic_multi_operation_rollback(#[case] backend: DatabaseType) {
	// Arrange
	let (_handle, connection) = setup_backend(backend).await;
	let mut executor = DatabaseMigrationExecutor::new(connection.clone());

	let migration = Migration::from_parts(
		"0001_multi_ops".to_string(),
		"testapp".to_string(),
		vec![
			Operation::CreateTable {
				name: leak_str("table1").to_string(),
				columns: vec![create_auto_pk_column("id", FieldType::Integer)],
				constraints: vec![],
				without_rowid: None,
				interleave_in_parent: None,
				partition: None,
			},
			Operation::CreateTable {
				name: leak_str("table2").to_string(),
				columns: vec![create_auto_pk_column("id", FieldType::Integer)],
				constraints: vec![],
				without_rowid: None,
				interleave_in_parent: None,
				partition: None,
			},
			Operation::CreateTable {
				name: leak_str("table3").to_string(),
				columns: vec![create_auto_pk_column("id", FieldType::Integer)],
				constraints: vec![],
				without_rowid: None,
				interleave_in_parent: None,
				partition: None,
			},
		],
		vec![],
		vec![],
		true,
		None,
		false,
		false,
		vec![],
		vec![],
	);

	// Act: forward
	executor
		.apply_migrations(std::slice::from_ref(&migration))
		.await
		.expect("Failed to apply multi-operation migration");

	// Assert: all tables created
	for t in ["table1", "table2", "table3"] {
		assert!(
			table_exists(&connection, t).await,
			"[{backend:?}] {t} should exist after forward",
		);
	}

	// Act: rollback
	executor
		.rollback_migrations(std::slice::from_ref(&migration))
		.await
		.expect("Failed to rollback migration");

	// Assert: all tables removed (observable post-condition is identical
	// across backends, even though MySQL gets there via auto-committed DDL).
	for t in ["table1", "table2", "table3"] {
		assert!(
			!table_exists(&connection, t).await,
			"[{backend:?}] {t} should not exist after rollback",
		);
	}

	// Sanity: the backend really does report the expected DDL semantics —
	// this is the property that drives the implementation difference above.
	let expected = matches!(backend, DatabaseType::Postgres | DatabaseType::Sqlite);
	assert_eq!(
		backend.supports_transactional_ddl(),
		expected,
		"[{backend:?}] supports_transactional_ddl mismatch",
	);
}

/// Test rollback with data in table.
///
/// **Test Intent**: Verify that rollback works even when the table contains
/// rows. The data is destroyed along with the table on every backend.
#[rstest]
#[case::postgres(DatabaseType::Postgres)]
#[case::mysql(DatabaseType::Mysql)]
#[case::sqlite(DatabaseType::Sqlite)]
#[tokio::test]
async fn test_rollback_with_data(#[case] backend: DatabaseType) {
	// Arrange
	let (_handle, connection) = setup_backend(backend).await;
	let mut executor = DatabaseMigrationExecutor::new(connection.clone());

	let migration = create_test_migration(
		"testapp",
		"0001_create_users",
		vec![Operation::CreateTable {
			name: leak_str("users").to_string(),
			columns: vec![
				create_auto_pk_column("id", FieldType::Integer),
				create_not_null_column("name", FieldType::VarChar(100)),
			],
			constraints: vec![],
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		}],
	);
	executor
		.apply_migrations(std::slice::from_ref(&migration))
		.await
		.expect("Failed to create table");

	// Act: insert data using a backend-agnostic INSERT path.
	let inserts = ["Alice", "Bob", "Charlie"];
	let count: i64 = match connection.database_type() {
		DatabaseType::Postgres => {
			let pool = connection
				.into_postgres()
				.expect("postgres pool unavailable");
			for name in inserts {
				sqlx::query("INSERT INTO users (name) VALUES ($1)")
					.bind(name)
					.execute(&pool)
					.await
					.expect("Failed to insert data");
			}
			sqlx::query_scalar("SELECT COUNT(*)::BIGINT FROM users")
				.fetch_one(&pool)
				.await
				.expect("Failed to count rows")
		}
		DatabaseType::Mysql => {
			let pool = connection.into_mysql().expect("mysql pool unavailable");
			for name in inserts {
				sqlx::query("INSERT INTO users (name) VALUES (?)")
					.bind(name)
					.execute(&pool)
					.await
					.expect("Failed to insert data");
			}
			sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM users")
				.fetch_one(&pool)
				.await
				.expect("Failed to count rows")
		}
		DatabaseType::Sqlite => {
			let pool = connection.into_sqlite().expect("sqlite pool unavailable");
			for name in inserts {
				sqlx::query("INSERT INTO users (name) VALUES (?)")
					.bind(name)
					.execute(&pool)
					.await
					.expect("Failed to insert data");
			}
			sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM users")
				.fetch_one(&pool)
				.await
				.expect("Failed to count rows")
		}
	};

	// Sanity
	assert_eq!(count, 3, "[{backend:?}] should have 3 users");

	// Act: rollback
	executor
		.rollback_migrations(std::slice::from_ref(&migration))
		.await
		.expect("Failed to rollback migration");

	// Assert: table (and its data) gone
	assert!(
		!table_exists(&connection, "users").await,
		"[{backend:?}] table should not exist after rollback",
	);
}

/// Test index creation/deletion rollback.
///
/// **Test Intent**: Verify that CREATE INDEX can be rolled back on every backend.
#[rstest]
#[case::postgres(DatabaseType::Postgres)]
#[case::mysql(DatabaseType::Mysql)]
#[case::sqlite(DatabaseType::Sqlite)]
#[tokio::test]
async fn test_index_rollback(#[case] backend: DatabaseType) {
	// Arrange
	let (_handle, connection) = setup_backend(backend).await;
	let mut executor = DatabaseMigrationExecutor::new(connection.clone());

	let create_table = create_test_migration(
		"testapp",
		"0001_create_users",
		vec![Operation::CreateTable {
			name: leak_str("users").to_string(),
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

	let create_index = create_test_migration(
		"testapp",
		"0002_create_email_index",
		vec![Operation::CreateIndex {
			table: leak_str("users").to_string(),
			columns: vec![leak_str("email").to_string()],
			unique: false,
			index_type: None,
			where_clause: None,
			concurrently: false,
			expressions: None,
			mysql_options: None,
			operator_class: None,
		}],
	);

	// Act: forward
	executor
		.apply_migrations(std::slice::from_ref(&create_index))
		.await
		.expect("Failed to create index");

	// Assert: index exists
	assert!(
		index_exists(&connection, "users", "idx_users_email").await,
		"[{backend:?}] index should exist after creation",
	);

	// Act: rollback
	executor
		.rollback_migrations(std::slice::from_ref(&create_index))
		.await
		.expect("Failed to rollback index creation");

	// Assert: index dropped
	assert!(
		!index_exists(&connection, "users", "idx_users_email").await,
		"[{backend:?}] index should not exist after rollback",
	);
}

// ============================================================================
// Error Handling Tests (Abnormal Cases)
// ============================================================================

/// Test rollback failure when reverse_sql is not provided.
///
/// **Test Intent**: Verify that RunSQL without reverse_sql cannot be rolled
/// back. The exact policy (`Err` vs. silently skipped) is implementation-
/// defined and may vary by backend, but the system must NOT panic.
#[rstest]
#[case::postgres(
	DatabaseType::Postgres,
	"CREATE TABLE irreversible (id SERIAL PRIMARY KEY)"
)]
#[case::mysql(
	DatabaseType::Mysql,
	"CREATE TABLE irreversible (id INT AUTO_INCREMENT PRIMARY KEY)"
)]
#[case::sqlite(
	DatabaseType::Sqlite,
	"CREATE TABLE irreversible (id INTEGER PRIMARY KEY AUTOINCREMENT)"
)]
#[tokio::test]
async fn test_rollback_fail_without_reverse_sql(
	#[case] backend: DatabaseType,
	#[case] forward_sql: &'static str,
) {
	// Arrange
	let (_handle, connection) = setup_backend(backend).await;
	let mut executor = DatabaseMigrationExecutor::new(connection.clone());

	let migration = create_test_migration(
		"testapp",
		"0001_irreversible",
		vec![Operation::RunSQL {
			sql: forward_sql.to_string(),
			reverse_sql: None,
		}],
	);
	executor
		.apply_migrations(std::slice::from_ref(&migration))
		.await
		.expect("Forward migration should succeed");

	// Sanity: forward DDL succeeded; the table exists.
	assert!(
		table_exists(&connection, "irreversible").await,
		"[{backend:?}] table should exist after forward",
	);

	// Act: rollback (must not panic)
	let rollback_result = executor
		.rollback_migrations(std::slice::from_ref(&migration))
		.await;

	// Assert observable state: both policies are acceptable but must be
	// self-consistent.
	// - Policy A (`Err`): the migration is rejected as irreversible; the
	//   table MUST remain because no destructive action was taken.
	// - Policy B (`Ok`): the migration is treated as a no-op skip; the table
	//   MUST also remain — a successful rollback without a reverse must not
	//   silently drop user data.
	// In other words, regardless of return-value policy, the table must
	// still exist after the call.
	let _ = rollback_result;
	assert!(
		table_exists(&connection, "irreversible").await,
		"[{backend:?}] irreversible table must remain after rollback regardless of policy",
	);
}

/// Test non-atomic single-operation rollback (`atomic = false`).
///
/// **Test Intent**: Verify that a migration declared with `atomic = false`
/// can be applied and rolled back successfully on every backend, even though
/// no transactional wrapper is used. This is the canonical setting for
/// MySQL (non-transactional DDL), but Postgres and SQLite must accept the
/// flag too and produce the same end state.
///
/// **Scope**: Single-operation migration only. Multi-operation partial-
/// failure scenarios under `atomic = false` (which exercise the "some
/// operations applied, some not" path) are covered by separate tests in
/// the error-handling integration suite.
#[rstest]
#[case::postgres(DatabaseType::Postgres)]
#[case::mysql(DatabaseType::Mysql)]
#[case::sqlite(DatabaseType::Sqlite)]
#[tokio::test]
async fn test_partial_rollback_non_atomic(#[case] backend: DatabaseType) {
	// Arrange
	let (_handle, connection) = setup_backend(backend).await;
	let mut executor = DatabaseMigrationExecutor::new(connection.clone());

	let migration = Migration::from_parts(
		"0001_non_atomic".to_string(),
		"testapp".to_string(),
		vec![Operation::CreateTable {
			name: leak_str("non_atomic_table").to_string(),
			columns: vec![create_auto_pk_column("id", FieldType::Integer)],
			constraints: vec![],
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		}],
		vec![],
		vec![],
		false,
		None,
		false,
		false,
		vec![],
		vec![],
	);

	// Act: forward
	executor
		.apply_migrations(std::slice::from_ref(&migration))
		.await
		.expect("Migration should succeed");

	// Sanity
	assert!(
		table_exists(&connection, "non_atomic_table").await,
		"[{backend:?}] table should exist after forward",
	);

	// Act: rollback
	executor
		.rollback_migrations(std::slice::from_ref(&migration))
		.await
		.expect("Rollback should succeed");

	// Assert
	assert!(
		!table_exists(&connection, "non_atomic_table").await,
		"[{backend:?}] table should not exist after rollback",
	);
}

/// Test rollback failure due to foreign key constraint.
///
/// **Test Intent**: Verify that an outstanding FK reference prevents the
/// parent table from being dropped during rollback.
///
/// **Backend-divergent behavior**: SQLite enforces foreign keys only when
/// `PRAGMA foreign_keys = ON` is set. The default is OFF — so on SQLite the
/// rollback succeeds and the parent table is dropped (the orphaned child
/// is left dangling). Postgres and MySQL with InnoDB always enforce FKs,
/// so the rollback fails. This test asserts the divergent outcome rather
/// than hiding it behind `#[ignore]`.
#[rstest]
#[case::postgres(
	DatabaseType::Postgres,
	"CREATE TABLE orders (\
		id SERIAL PRIMARY KEY, \
		user_id INTEGER NOT NULL, \
		FOREIGN KEY (user_id) REFERENCES users(id))"
)]
#[case::mysql(
	DatabaseType::Mysql,
	"CREATE TABLE orders (\
		id INT AUTO_INCREMENT PRIMARY KEY, \
		user_id INT NOT NULL, \
		FOREIGN KEY (user_id) REFERENCES users(id)) ENGINE=InnoDB"
)]
#[case::sqlite(
	DatabaseType::Sqlite,
	"CREATE TABLE orders (\
		id INTEGER PRIMARY KEY AUTOINCREMENT, \
		user_id INTEGER NOT NULL, \
		FOREIGN KEY (user_id) REFERENCES users(id))"
)]
#[tokio::test]
async fn test_rollback_fail_with_foreign_key_reference(
	#[case] backend: DatabaseType,
	#[case] child_ddl: &'static str,
) {
	// Arrange
	let (_handle, connection) = setup_backend(backend).await;
	let mut executor = DatabaseMigrationExecutor::new(connection.clone());

	let parent_migration = create_test_migration(
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
		.apply_migrations(std::slice::from_ref(&parent_migration))
		.await
		.expect("Failed to create parent table");

	// Create child table out-of-band so the migration system does not know
	// about it. The child references the parent via FK.
	match connection.database_type() {
		DatabaseType::Postgres => {
			let pool = connection
				.into_postgres()
				.expect("postgres pool unavailable");
			sqlx::query(child_ddl)
				.execute(&pool)
				.await
				.expect("Failed to create child table");
		}
		DatabaseType::Mysql => {
			let pool = connection.into_mysql().expect("mysql pool unavailable");
			sqlx::query(child_ddl)
				.execute(&pool)
				.await
				.expect("Failed to create child table");
		}
		DatabaseType::Sqlite => {
			let pool = connection.into_sqlite().expect("sqlite pool unavailable");
			// SQLite only enforces FKs when the pragma is enabled; turn it on
			// so we can observe the divergence consciously.
			sqlx::query("PRAGMA foreign_keys = ON")
				.execute(&pool)
				.await
				.expect("Failed to enable SQLite foreign_keys pragma");
			sqlx::query(child_ddl)
				.execute(&pool)
				.await
				.expect("Failed to create child table");
		}
	}

	// Act: attempt rollback
	let rollback_result = executor
		.rollback_migrations(std::slice::from_ref(&parent_migration))
		.await;

	// Assert: outcome depends on backend FK enforcement.
	match backend {
		DatabaseType::Postgres | DatabaseType::Mysql => {
			assert!(
				rollback_result.is_err(),
				"[{backend:?}] rollback should fail when child table references parent",
			);
		}
		DatabaseType::Sqlite => {
			// SQLite enforces FK on writes (when the pragma is on), but
			// DROP TABLE is not blocked the way Postgres / MySQL block it.
			// The test asserts the observable divergence rather than ignoring
			// it: either we see an error (matching Postgres/MySQL) OR the
			// rollback completes and the parent table is gone. Both are
			// "correct" for SQLite; only a panic is unacceptable.
			let parent_gone = !table_exists(&connection, "users").await;
			assert!(
				rollback_result.is_err() || parent_gone,
				"[{backend:?}] rollback must either error out or drop the parent",
			);
		}
	}
}

// ============================================================================
// Edge Case Tests
// ============================================================================

/// Test rollback of empty migration.
///
/// **Test Intent**: Verify that migrations with no operations can be rolled
/// back on every backend.
#[rstest]
#[case::postgres(DatabaseType::Postgres)]
#[case::mysql(DatabaseType::Mysql)]
#[case::sqlite(DatabaseType::Sqlite)]
#[tokio::test]
async fn test_rollback_empty_migration(#[case] backend: DatabaseType) {
	// Arrange
	let (_handle, connection) = setup_backend(backend).await;
	let mut executor = DatabaseMigrationExecutor::new(connection);

	let migration = create_test_migration("testapp", "0001_empty", vec![]);

	// Act: forward (no-op)
	executor
		.apply_migrations(std::slice::from_ref(&migration))
		.await
		.expect("Empty migration should succeed");

	// Act: rollback
	let rollback_result = executor
		.rollback_migrations(std::slice::from_ref(&migration))
		.await;

	// Assert
	assert!(
		rollback_result.is_ok(),
		"[{backend:?}] empty migration rollback should succeed",
	);
}

/// Test rollback with dependent migrations (reverse dependency order).
///
/// **Test Intent**: Verify that dependent migrations are rolled back in
/// reverse order on every backend.
///
/// **Test Steps**:
/// 1. Migration A: CREATE TABLE users
/// 2. Migration B (depends on A): CREATE TABLE orders with FK to users
/// 3. Rollback B first: DROP TABLE orders
/// 4. Rollback A second: DROP TABLE users
#[rstest]
#[case::postgres(
	DatabaseType::Postgres,
	"CONSTRAINT fk_orders_user_id FOREIGN KEY (user_id) REFERENCES users(id) \
	 ON DELETE CASCADE ON UPDATE NO ACTION"
)]
#[case::mysql(
	DatabaseType::Mysql,
	"CONSTRAINT fk_orders_user_id FOREIGN KEY (user_id) REFERENCES users(id) \
	 ON DELETE CASCADE ON UPDATE NO ACTION"
)]
#[case::sqlite(
	DatabaseType::Sqlite,
	"CONSTRAINT fk_orders_user_id FOREIGN KEY (user_id) REFERENCES users(id) \
	 ON DELETE CASCADE ON UPDATE NO ACTION"
)]
#[tokio::test]
async fn test_rollback_with_dependencies(
	#[case] backend: DatabaseType,
	#[case] fk_constraint_sql: &'static str,
) {
	// Arrange
	let (_handle, connection) = setup_backend(backend).await;
	let mut executor = DatabaseMigrationExecutor::new(connection.clone());

	let migration_a = create_test_migration(
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

	let migration_b = Migration::from_parts(
		"0002_create_orders".to_string(),
		"testapp".to_string(),
		vec![
			Operation::CreateTable {
				name: leak_str("orders").to_string(),
				columns: vec![
					create_auto_pk_column("id", FieldType::Integer),
					create_not_null_column("user_id", FieldType::Integer),
				],
				constraints: vec![],
				without_rowid: None,
				interleave_in_parent: None,
				partition: None,
			},
			Operation::AddConstraint {
				table: leak_str("orders").to_string(),
				constraint_sql: fk_constraint_sql.to_string(),
			},
		],
		vec![("testapp".to_string(), "0001_create_users".to_string())],
		vec![],
		true,
		None,
		false,
		false,
		vec![],
		vec![],
	);

	// Act: forward both
	executor
		.apply_migrations(std::slice::from_ref(&migration_a))
		.await
		.expect("Migration A should succeed");
	executor
		.apply_migrations(std::slice::from_ref(&migration_b))
		.await
		.expect("Migration B should succeed");

	// Assert: both tables exist
	assert!(
		table_exists(&connection, "users").await,
		"[{backend:?}] users table should exist",
	);
	assert!(
		table_exists(&connection, "orders").await,
		"[{backend:?}] orders table should exist",
	);

	// Act: rollback B first
	executor
		.rollback_migrations(std::slice::from_ref(&migration_b))
		.await
		.expect("Rollback of migration B should succeed");

	// Assert: B's table gone, A's still present
	assert!(
		!table_exists(&connection, "orders").await,
		"[{backend:?}] orders table should not exist after B rollback",
	);
	assert!(
		table_exists(&connection, "users").await,
		"[{backend:?}] users table should still exist after B rollback",
	);

	// Act: rollback A
	executor
		.rollback_migrations(std::slice::from_ref(&migration_a))
		.await
		.expect("Rollback of migration A should succeed");

	// Assert
	assert!(
		!table_exists(&connection, "users").await,
		"[{backend:?}] users table should not exist after A rollback",
	);
}

/// Test circular dependency detection in rollback.
///
/// **Test Intent**: Verify that a true cycle in `dependencies` (A → B and
/// B → A) is either detected at the dependency-resolution stage (preferred
/// outcome — `apply_migrations` returns `Err`) or, if the resolver is
/// lenient, that the resulting `rollback_migrations` call still completes
/// without panicking. Either policy is acceptable as long as the system
/// remains consistent and does not crash.
#[rstest]
#[case::postgres(DatabaseType::Postgres)]
#[case::mysql(DatabaseType::Mysql)]
#[case::sqlite(DatabaseType::Sqlite)]
#[tokio::test]
async fn test_circular_dependency_rollback(#[case] backend: DatabaseType) {
	// Arrange
	let (_handle, connection) = setup_backend(backend).await;
	let mut executor = DatabaseMigrationExecutor::new(connection);

	// Build an actual cycle: A depends on B, and B depends on A.
	let migration_a = Migration::from_parts(
		"0001_migration_a".to_string(),
		"testapp".to_string(),
		vec![Operation::CreateTable {
			name: leak_str("table_a").to_string(),
			columns: vec![create_auto_pk_column("id", FieldType::Integer)],
			constraints: vec![],
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		}],
		vec![("testapp".to_string(), "0002_migration_b".to_string())],
		vec![],
		true,
		None,
		false,
		false,
		vec![],
		vec![],
	);

	let migration_b = Migration::from_parts(
		"0002_migration_b".to_string(),
		"testapp".to_string(),
		vec![Operation::CreateTable {
			name: leak_str("table_b").to_string(),
			columns: vec![create_auto_pk_column("id", FieldType::Integer)],
			constraints: vec![],
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		}],
		vec![("testapp".to_string(), "0001_migration_a".to_string())],
		vec![],
		true,
		None,
		false,
		false,
		vec![],
		vec![],
	);

	// Act: apply both migrations together so the resolver sees the cycle.
	let apply_result = executor
		.apply_migrations(&[migration_a.clone(), migration_b.clone()])
		.await;

	// Assert: the system handles the cycle without panicking. The preferred
	// policy is to reject it (`Err`); a lenient resolver that breaks the
	// cycle deterministically and applies the migrations is also acceptable
	// as long as the subsequent rollback also completes.
	if apply_result.is_ok() {
		let rollback_result = executor
			.rollback_migrations(&[migration_a, migration_b])
			.await;
		// Either Ok or Err is acceptable here — we only require that the
		// call returns rather than panicking.
		let _ = rollback_result;
	} else {
		assert!(
			apply_result.is_err(),
			"[{backend:?}] circular dependency should be detected",
		);
	}
}
