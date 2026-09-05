//! Regression tests for reinhardt-web#4454:
//!
//! SQLite's `PRAGMA table_info(<table>).dflt_value` returns the literal SQL
//! fragment from the column's `DEFAULT` clause — including the surrounding
//! quotes for string defaults (e.g. `'pending'`, not `pending`). Prior to
//! #4454, both introspection paths (`SQLiteIntrospector::introspect_table`
//! and `read_sqlite_table_via_editor`) stripped those quotes before passing
//! the value to the DDL emission layer. The raw
//! `format!("DEFAULT {}", default)` paths in `migrations/operations.rs`
//! then emitted invalid DDL like `DEFAULT pending` (unquoted identifier
//! reference) instead of `DEFAULT 'pending'` (string literal), corrupting
//! any table recreation triggered by a subsequent `DropColumn`,
//! `AlterColumn`, `AddConstraint`, or `DropConstraint`.
//!
//! The fix preserves `dflt_value` verbatim across both introspection
//! paths so the SQL fragment round-trips through the emission layer
//! unchanged.

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

/// Reproduces issue #4454 directly:
///
/// 1. CreateTable(orders) with `status VARCHAR(20) NOT NULL DEFAULT 'pending'`.
/// 2. DropColumn(orders.note) — triggers SQLite table recreation.
/// 3. Assert: the recreated table still has `status` defaulted to the
///    literal SQL string `'pending'`, and an insert that omits `status`
///    is accepted and populates `status` with `pending`.
///
/// Before the fix, step 3 would fail because the recreation introspector
/// stripped the surrounding quotes from `dflt_value`, then re-emitted
/// `DEFAULT pending` (invalid DDL referencing an unquoted identifier).
/// SQLite either rejected the recreation outright or silently dropped
/// the default expression, breaking the round-trip.
#[rstest]
#[tokio::test]
async fn issue_4454_string_default_survives_drop_column_recreation() {
	// Arrange: create an `orders` table with a string default and an
	// unrelated `note` column that we can drop to force a recreation.
	let connection = DatabaseConnection::connect_sqlite("sqlite::memory:")
		.await
		.expect("connect");
	let conn = connection.clone();
	let mut executor = DatabaseMigrationExecutor::new(connection);

	let m0001 = migration(
		"orders",
		"0001_initial",
		vec![Operation::CreateTable {
			name: "orders".to_string(),
			columns: vec![
				pk_column("id"),
				ColumnDefinition::from_parts(
					"status".to_string(),
					FieldType::VarChar(20),
					true,
					false,
					false,
					false,
					Some("'pending'".to_string()),
					None,
					None,
				),
				ColumnDefinition::from_parts(
					"note".to_string(),
					FieldType::VarChar(255),
					false,
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
	);

	// Act 1: apply the initial migration.
	executor
		.apply_migrations(&[m0001])
		.await
		.expect("initial migration should apply");

	// Sanity check: SQLite stored the default verbatim with quotes.
	let pre_rows = conn
		.fetch_all("PRAGMA table_info(orders)", vec![])
		.await
		.expect("read pre-recreation pragma");
	let pre_status_default: Option<String> = pre_rows
		.iter()
		.find(|r| {
			r.get::<String>("name")
				.map(|n| n == "status")
				.unwrap_or(false)
		})
		.and_then(|r| r.get::<String>("dflt_value").ok());
	assert_eq!(
		pre_status_default.as_deref(),
		Some("'pending'"),
		"SQLite must store the literal SQL fragment 'pending' (with quotes) \
		 in dflt_value before recreation"
	);

	// Act 2: drop the unrelated column. This triggers SQLite's table
	// recreation path, which re-emits the CREATE TABLE from introspection.
	let m0002 = migration(
		"orders",
		"0002_drop_note",
		vec![Operation::DropColumn {
			table: "orders".to_string(),
			column: "note".to_string(),
			old_definition: None,
		}],
	);

	executor
		.apply_migrations(&[m0002])
		.await
		.expect("drop-column recreation should preserve the string default");

	// Assert 1: the recreated table preserves `DEFAULT 'pending'` verbatim
	// in the regenerated CREATE TABLE statement (the strict round-trip
	// guarantee). Read it back via sqlite_master to compare exact text.
	let create_sql_rows = conn
		.fetch_all(
			"SELECT sql FROM sqlite_master WHERE type='table' AND name='orders'",
			vec![],
		)
		.await
		.expect("read sqlite_master");
	let create_sql: String = create_sql_rows
		.first()
		.and_then(|r| r.get::<String>("sql").ok())
		.expect("CREATE TABLE sql for orders");

	assert!(
		create_sql.contains("DEFAULT 'pending'"),
		"recreated CREATE TABLE must contain `DEFAULT 'pending'` verbatim, got: {create_sql}"
	);

	// Assert 2: the recreated default is also enforced at the SQL level.
	// Insert a row WITHOUT supplying `status`; SQLite must use the default.
	conn.execute("INSERT INTO orders (id) VALUES (1)", vec![])
		.await
		.expect("insert with default");

	let inserted = conn
		.fetch_all("SELECT status FROM orders WHERE id = 1", vec![])
		.await
		.expect("read back inserted row");
	let inserted_status: Option<String> = inserted
		.first()
		.and_then(|r| r.get::<String>("status").ok());
	assert_eq!(
		inserted_status.as_deref(),
		Some("pending"),
		"recreated default must actually populate the column on INSERT; \
		 got {inserted_status:?}"
	);

	// Assert 3: PRAGMA table_info also reports the default as the verbatim
	// fragment 'pending' (with quotes), confirming the round-trip is
	// stable across multiple recreations.
	let post_rows = conn
		.fetch_all("PRAGMA table_info(orders)", vec![])
		.await
		.expect("read post-recreation pragma");
	let post_status_default: Option<String> = post_rows
		.iter()
		.find(|r| {
			r.get::<String>("name")
				.map(|n| n == "status")
				.unwrap_or(false)
		})
		.and_then(|r| r.get::<String>("dflt_value").ok());
	assert_eq!(
		post_status_default.as_deref(),
		Some("'pending'"),
		"dflt_value must round-trip as 'pending' (with quotes) across recreation"
	);
}
