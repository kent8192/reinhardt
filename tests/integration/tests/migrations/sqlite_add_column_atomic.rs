//! Regression tests for reinhardt-web#4447:
//!
//! `manage migrate` reported a migration as `Applied` even though its
//! `Operation::AddColumn` step did not survive on a SQLite database.
//!
//! The migration's other operations (e.g. `Operation::AddConstraint` in the
//! same migration) DID apply, so the schema ended up partially migrated while
//! the migration history claimed success.
//!
//! Root cause: when SQLite recreation runs *inside* the schema editor's open
//! transaction, the introspector used to read the pre-recreation column list
//! goes through a *separate* connection in the pool. That separate connection
//! cannot see the just-`ALTER`'d column (still inside the editor's TX), so the
//! recreation rebuilds the table from a stale column set and effectively
//! discards the prior `AddColumn`.

use reinhardt_db::backends::connection::DatabaseConnection;
use reinhardt_db::migrations::{
	ColumnDefinition, FieldType, Migration, executor::DatabaseMigrationExecutor,
	operations::Operation,
};
use rstest::*;

fn pk_column(name: &str) -> ColumnDefinition {
	ColumnDefinition::from_parts(
		name.to_string(),
		FieldType::Integer,
		true,
		false,
		true,
		true,
		None,
		None,
		None,
	)
}

fn col(name: &str, ty: FieldType, not_null: bool) -> ColumnDefinition {
	ColumnDefinition::from_parts(
		name.to_string(),
		ty,
		not_null,
		false,
		false,
		false,
		None,
		None,
		None,
	)
}

fn migration(app: &str, name: &str, ops: Vec<Operation>) -> Migration {
	Migration::from_parts(
		name.to_string(),
		app.to_string(),
		ops,
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

/// Reproduces issue #4447 directly:
///
/// 1. CreateTable(users) with no rows.
/// 2. A second migration that combines AddColumn(NOT NULL) + AddConstraint.
///    Atomic = true (the default). On SQLite, AddConstraint triggers table
///    recreation. The recreation must NOT lose the just-added column.
#[rstest]
#[tokio::test]
async fn issue_4447_add_column_then_add_constraint_preserves_column() {
	let connection = DatabaseConnection::connect_sqlite("sqlite::memory:")
		.await
		.expect("connect");
	let conn = connection.clone();
	let mut executor = DatabaseMigrationExecutor::new(connection);

	// Step 1: create the users table without is_superuser.
	let m0001 = migration(
		"users",
		"0001_initial",
		vec![Operation::CreateTable {
			name: "users".to_string(),
			columns: vec![
				pk_column("id"),
				col("username", FieldType::VarChar(150), true),
			],
			constraints: vec![],
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		}],
	);

	// Step 2: two-op migration. AddColumn first, AddConstraint second.
	// This mirrors the exact sequence from the reproduction in #4447.
	let m0002 = migration(
		"users",
		"0002_add_is_superuser_and_constraint",
		vec![
			Operation::AddColumn {
				table: "users".to_string(),
				column: ColumnDefinition::from_parts(
					"is_superuser".to_string(),
					FieldType::Boolean,
					true,
					false,
					false,
					false,
					// Note: a default IS provided here. The "no default" path
					// is exercised by the macro-level regression test.
					Some("false".to_string()),
					None,
					None,
				),
				mysql_options: None,
			},
			Operation::AddConstraint {
				table: "users".to_string(),
				constraint_sql: "CONSTRAINT users_user_username_uniq UNIQUE (username)".to_string(),
			},
		],
	);

	executor
		.apply_migrations(&[m0001, m0002])
		.await
		.expect("migrations should apply cleanly");

	// The just-added column must survive the AddConstraint recreation.
	let rows = conn
		.fetch_all("PRAGMA table_info(users)", vec![])
		.await
		.expect("read pragma");
	let names: Vec<String> = rows
		.iter()
		.filter_map(|r| r.get::<String>("name").ok())
		.collect();

	assert!(
		names.iter().any(|n| n == "is_superuser"),
		"is_superuser column was lost across AddConstraint recreation: have {names:?}"
	);

	// And the unique constraint added by op #2 must exist on `username`.
	// SQLite reports the underlying auto-index in PRAGMA index_list; the name
	// it picks for `CONSTRAINT … UNIQUE` is implementation-defined (often
	// `sqlite_autoindex_<table>_<n>`), so we verify by column instead of by
	// constraint name.
	let idxs = conn
		.fetch_all("PRAGMA index_list(users)", vec![])
		.await
		.expect("read index_list");
	let mut covers_username = false;
	for row in &idxs {
		let origin: String = row.get("origin").unwrap_or_default();
		let unique: i64 = row.get("unique").unwrap_or(0);
		if origin != "u" || unique != 1 {
			continue;
		}
		let idx_name: String = row.get("name").unwrap_or_default();
		let info = conn
			.fetch_all(&format!("PRAGMA index_info({})", idx_name), vec![])
			.await
			.expect("read index_info");
		let cols: Vec<String> = info
			.iter()
			.filter_map(|r| r.get::<String>("name").ok())
			.collect();
		if cols == vec!["username".to_string()] {
			covers_username = true;
			break;
		}
	}
	assert!(
		covers_username,
		"AddConstraint UNIQUE(username) was not preserved across recreation: \
		 PRAGMA index_list returned {idxs:?}"
	);

	// Verify the constraint is actually enforced (orthogonal to introspection
	// quirks above): duplicate inserts must fail.
	conn.execute("INSERT INTO users (username) VALUES ('alice')", vec![])
		.await
		.expect("first insert");
	let dup = conn
		.execute("INSERT INTO users (username) VALUES ('alice')", vec![])
		.await;
	assert!(
		dup.is_err(),
		"AddConstraint UNIQUE(username) must be enforced after recreation"
	);
}

/// When a single-op AddColumn(NOT NULL, default=None) migration is applied to
/// a NON-empty SQLite table, the operation MUST surface an error from the
/// runner AND the migration MUST NOT be recorded as applied. The migration
/// history must stay consistent with the actual DB state.
#[rstest]
#[tokio::test]
async fn issue_4447_failed_add_column_does_not_record_applied() {
	let connection = DatabaseConnection::connect_sqlite("sqlite::memory:")
		.await
		.expect("connect");
	let conn = connection.clone();
	let mut executor = DatabaseMigrationExecutor::new(connection);

	// Create the table and insert a row to make ADD COLUMN NOT NULL impossible
	// without a default.
	let create = migration(
		"users",
		"0001_initial",
		vec![Operation::CreateTable {
			name: "users".to_string(),
			columns: vec![
				pk_column("id"),
				col("username", FieldType::VarChar(150), true),
			],
			constraints: vec![],
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		}],
	);
	executor
		.apply_migrations(&[create])
		.await
		.expect("initial migration applies");

	conn.execute("INSERT INTO users (username) VALUES ('alice')", vec![])
		.await
		.expect("seed row");

	// Now attempt AddColumn(NOT NULL, default=None). SQLite must reject it on
	// a non-empty table.
	let bad = migration(
		"users",
		"0002_add_super_no_default",
		vec![Operation::AddColumn {
			table: "users".to_string(),
			column: ColumnDefinition::from_parts(
				"is_superuser".to_string(),
				FieldType::Boolean,
				true,
				false,
				false,
				false,
				None,
				None,
				None,
			),
			mysql_options: None,
		}],
	);

	let result = executor.apply_migrations(&[bad]).await;
	assert!(
		result.is_err(),
		"AddColumn(NOT NULL, default=None) on a non-empty SQLite table must fail; got {result:?}"
	);

	// And critically: the migration MUST NOT be recorded as applied.
	let recorded = conn
		.fetch_all(
			"SELECT name FROM reinhardt_migrations WHERE app = 'users' AND name = '0002_add_super_no_default'",
			vec![],
		)
		.await
		.expect("query history");
	assert!(
		recorded.is_empty(),
		"migration was recorded as applied despite failure (atomicity violation)"
	);
}
