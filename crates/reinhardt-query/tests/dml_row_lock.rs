//! Row-lock SQL generation and backend capability tests.

use reinhardt_query::QueryBuildError;
use reinhardt_query::prelude::*;
use rstest::rstest;

fn locked_select(lock_type: LockType) -> SelectStatement {
	let mut statement = Query::select();
	statement.column("id").from("users").lock(lock_type);
	statement
}

#[rstest]
#[case(LockType::Update, "FOR UPDATE")]
#[case(LockType::NoKeyUpdate, "FOR NO KEY UPDATE")]
#[case(LockType::Share, "FOR SHARE")]
#[case(LockType::KeyShare, "FOR KEY SHARE")]
fn postgres_renders_each_lock_strength(#[case] lock_type: LockType, #[case] sql_suffix: &str) {
	let statement = locked_select(lock_type);

	let (sql, values) = PostgresQueryBuilder::new()
		.build_select_checked(&statement)
		.expect("PostgreSQL supports every row lock strength");

	assert_eq!(sql, format!(r#"SELECT "id" FROM "users" {sql_suffix}"#));
	assert_eq!(values.len(), 0);
}

#[rstest]
#[case(LockBehavior::Nowait, "NOWAIT")]
#[case(LockBehavior::SkipLocked, "SKIP LOCKED")]
fn postgres_renders_lock_behavior(#[case] behavior: LockBehavior, #[case] sql_suffix: &str) {
	let mut statement = locked_select(LockType::Update);
	statement.lock_behavior(behavior);

	let (sql, _) = PostgresQueryBuilder::new()
		.build_select_checked(&statement)
		.expect("PostgreSQL supports row lock wait behaviors");

	assert_eq!(
		sql,
		format!(r#"SELECT "id" FROM "users" FOR UPDATE {sql_suffix}"#)
	);
}

#[rstest]
fn lock_behavior_is_mutually_exclusive() {
	let mut statement = locked_select(LockType::Update);
	statement
		.lock_behavior(LockBehavior::Nowait)
		.lock_behavior(LockBehavior::SkipLocked);

	let (sql, _) = PostgresQueryBuilder::new()
		.build_select_checked(&statement)
		.expect("the last configured lock behavior is valid");

	assert_eq!(sql, r#"SELECT "id" FROM "users" FOR UPDATE SKIP LOCKED"#);
}

#[rstest]
fn postgres_renders_typed_lock_targets_using_aliases() {
	let mut statement = Query::select();
	statement
		.column(("u", "id"))
		.from(TableRef::table_alias("users", "u"))
		.lock(LockType::NoKeyUpdate)
		.lock_tables([
			TableRef::table_alias("users", "u"),
			TableRef::schema_table("audit", "events"),
		])
		.lock_behavior(LockBehavior::Nowait);

	let (sql, _) = PostgresQueryBuilder::new()
		.build_select_checked(&statement)
		.expect("PostgreSQL supports typed row lock targets");

	assert_eq!(
		sql,
		r#"SELECT "u"."id" FROM "users" AS "u" FOR NO KEY UPDATE OF "u", "audit"."events" NOWAIT"#
	);
}

#[rstest]
#[case(LockType::Update, "FOR UPDATE")]
#[case(LockType::Share, "FOR SHARE")]
fn mysql_renders_supported_lock_strengths(#[case] lock_type: LockType, #[case] suffix: &str) {
	let mut statement = locked_select(lock_type);
	statement
		.lock_tables([TableRef::table("users")])
		.lock_behavior(LockBehavior::SkipLocked);

	let (sql, _) = MySqlQueryBuilder::new()
		.build_select_checked(&statement)
		.expect("MySQL supports UPDATE and SHARE locks with targets and wait behavior");

	assert_eq!(
		sql,
		format!("SELECT `id` FROM `users` {suffix} OF `users` SKIP LOCKED")
	);
}

#[rstest]
#[case(LockType::NoKeyUpdate)]
#[case(LockType::KeyShare)]
fn mysql_checked_builder_rejects_postgres_only_lock_strength(#[case] lock_type: LockType) {
	let statement = locked_select(lock_type);

	assert_eq!(
		MySqlQueryBuilder::new().build_select_checked(&statement),
		Err(QueryBuildError::UnsupportedBackendFeature {
			feature: "the requested row lock strength",
			backend: "MySQL",
		})
	);
}

#[rstest]
fn sqlite_checked_builder_rejects_row_locking() {
	let statement = locked_select(LockType::Update);

	assert_eq!(
		SqliteQueryBuilder::new().build_select_checked(&statement),
		Err(QueryBuildError::UnsupportedBackendFeature {
			feature: "row locking",
			backend: "SQLite",
		})
	);
}

#[rstest]
#[case(LockType::Update)]
#[case(LockType::NoKeyUpdate)]
#[case(LockType::Share)]
#[case(LockType::KeyShare)]
fn cockroach_checked_builder_explicitly_accepts_lock_strengths(#[case] lock_type: LockType) {
	let mut statement = locked_select(lock_type);
	statement.lock_behavior(LockBehavior::Nowait);

	let (sql, _) = CockroachDBQueryBuilder::new()
		.build_select_checked(&statement)
		.expect("CockroachDB supports all strengths as two semantic lock pairs");

	assert!(sql.ends_with(" NOWAIT"));
}

#[rstest]
fn cockroach_checked_builder_rejects_lock_targets() {
	let mut statement = locked_select(LockType::Update);
	statement.lock_tables([TableRef::table("users")]);

	assert_eq!(
		CockroachDBQueryBuilder::new().build_select_checked(&statement),
		Err(QueryBuildError::UnsupportedBackendFeature {
			feature: "row lock table targets",
			backend: "CockroachDB",
		})
	);
}
