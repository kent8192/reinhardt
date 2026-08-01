//! Row-lock SQL generation and backend capability tests.

use reinhardt_query::QueryBuildError;
use reinhardt_query::prelude::*;
use rstest::rstest;

#[cfg(feature = "pgvector")]
use reinhardt_query::error::{PgvectorFeature, insert_pgvector_feature};

fn locked_select(lock_type: LockType) -> SelectStatement {
	let mut statement = Query::select();
	statement.column("id").from("users").lock(lock_type);
	statement
}

fn validate_postgres_outer_lock_on_derived(
	derived: SelectStatement,
) -> Result<(), QueryBuildError> {
	let mut statement = Query::select();
	statement
		.column("id")
		.from_subquery(derived, "derived")
		.lock(LockType::Update);
	PostgresQueryBuilder::new()
		.build_select_checked(&statement)
		.map(|_| ())
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
fn postgres_checked_builder_rejects_locks_on_union_queries() {
	let mut statement = locked_select(LockType::Update);
	let mut union = Query::select();
	union.column("id").from("archived_users");
	statement.union(union);

	assert_eq!(
		PostgresQueryBuilder::new().build_select_checked(&statement),
		Err(QueryBuildError::UnsupportedBackendFeature {
			feature: "row locking on UNION queries",
			backend: "PostgreSQL",
		})
	);
}

#[rstest]
fn postgres_checked_builder_rejects_lock_on_union_arm() {
	let mut statement = Query::select();
	statement.column("id").from("users");
	let mut union = Query::select();
	union
		.column("id")
		.from("archived_users")
		.lock(LockType::Update);
	statement.union(union);

	assert_eq!(
		PostgresQueryBuilder::new().build_select_checked(&statement),
		Err(QueryBuildError::UnsupportedBackendFeature {
			feature: "row locking on UNION queries",
			backend: "PostgreSQL",
		})
	);
}

#[rstest]
fn postgres_checked_builder_rejects_lock_on_distinct_query() {
	let mut statement = locked_select(LockType::Update);
	statement.distinct();

	assert_eq!(
		PostgresQueryBuilder::new().build_select_checked(&statement),
		Err(QueryBuildError::UnsupportedBackendFeature {
			feature: "row locking with DISTINCT queries",
			backend: "PostgreSQL",
		})
	);
}

#[rstest]
fn sqlite_checked_builder_rejects_locks_in_expression_subqueries() {
	let mut statement = Query::select();
	statement.expr(Expr::subquery(locked_select(LockType::Update)));

	assert_eq!(
		SqliteQueryBuilder::new().build_select_checked(&statement),
		Err(QueryBuildError::UnsupportedBackendFeature {
			feature: "row locking",
			backend: "SQLite",
		})
	);
}

#[rstest]
fn sqlite_checked_builder_rejects_locks_in_temporal_expression_subqueries() {
	// Arrange
	let projection = Func::temporal_trunc(
		Expr::subquery(locked_select(LockType::Update)).into_simple_expr(),
		TemporalTruncKind::Day,
		None,
		TemporalTruncOutput::Date,
	)
	.expect("a day projection can wrap a scalar subquery");
	let mut statement = Query::select();
	statement.expr(projection);

	// Act
	let result = SqliteQueryBuilder::new().build_select_checked(&statement);

	// Assert
	assert_eq!(
		result,
		Err(QueryBuildError::UnsupportedBackendFeature {
			feature: "row locking",
			backend: "SQLite",
		})
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
		.lock_tables([TableRef::table_alias("users", "u")])
		.lock_behavior(LockBehavior::Nowait);

	let (sql, _) = PostgresQueryBuilder::new()
		.build_select_checked(&statement)
		.expect("PostgreSQL supports typed row lock targets");

	assert_eq!(
		sql,
		r#"SELECT "u"."id" FROM "users" AS "u" FOR NO KEY UPDATE OF "u" NOWAIT"#
	);
}

#[rstest]
fn postgres_checked_builder_rejects_lock_target_absent_from_query() {
	let mut statement = locked_select(LockType::Update);
	statement.lock_tables([TableRef::table("audit_events")]);

	assert_eq!(
		PostgresQueryBuilder::new().build_select_checked(&statement),
		Err(QueryBuildError::UnsupportedBackendFeature {
			feature: "row lock target absent from the query",
			backend: "PostgreSQL",
		})
	);
}

#[rstest]
fn postgres_checked_builder_accepts_unqualified_target_for_schema_qualified_table() {
	// Arrange
	let mut statement = Query::select();
	statement
		.column("id")
		.from(TableRef::schema_table("audit", "events"))
		.lock(LockType::Update)
		.lock_tables([TableRef::table("events")]);

	// Act
	let (sql, values) = PostgresQueryBuilder::new()
		.build_select_checked(&statement)
		.expect("the unqualified relation name is valid for an unaliased schema-qualified table");

	// Assert
	assert_eq!(
		sql,
		r#"SELECT "id" FROM "audit"."events" FOR UPDATE OF "events""#
	);
	assert_eq!(values.len(), 0);
}

#[rstest]
fn postgres_checked_builder_rejects_outer_locks_on_cte_backed_queries() {
	// Arrange
	let mut cte = Query::select();
	cte.column("id").from("jobs");
	let mut statement = Query::select();
	statement
		.with_cte("locked_jobs", cte)
		.column("id")
		.from("locked_jobs")
		.lock(LockType::Update);

	// Act
	let result = PostgresQueryBuilder::new().build_select_checked(&statement);

	// Assert
	assert_eq!(
		result,
		Err(QueryBuildError::UnsupportedBackendFeature {
			feature: "row locking on CTE-backed queries",
			backend: "PostgreSQL",
		})
	);
}

#[rstest]
fn postgres_checked_builder_rejects_locks_in_later_ctes_that_read_earlier_ctes() {
	// Arrange
	let mut earlier_cte = Query::select();
	earlier_cte.column("id").from("jobs");
	let mut later_cte = Query::select();
	later_cte
		.column("id")
		.from("earlier_jobs")
		.lock(LockType::Update);
	let mut statement = Query::select();
	statement
		.with_cte("earlier_jobs", earlier_cte)
		.with_cte("later_jobs", later_cte)
		.column("id")
		.from("later_jobs");

	// Act
	let result = PostgresQueryBuilder::new().build_select_checked(&statement);

	// Assert
	assert_eq!(
		result,
		Err(QueryBuildError::UnsupportedBackendFeature {
			feature: "row locking on CTE-backed queries",
			backend: "PostgreSQL",
		})
	);
}

#[rstest]
fn postgres_checked_builder_propagates_targetless_locks_to_derived_aggregate_queries() {
	// Arrange
	let mut derived = Query::select();
	derived
		.expr(Func::count(Expr::col("id").into_simple_expr()))
		.from("jobs");

	// Act
	let result = validate_postgres_outer_lock_on_derived(derived);

	// Assert
	assert_eq!(
		result,
		Err(QueryBuildError::UnsupportedBackendFeature {
			feature: "row locking with aggregate queries",
			backend: "PostgreSQL",
		})
	);
}

#[rstest]
fn postgres_checked_builder_propagates_targetless_locks_to_derived_grouped_queries() {
	// Arrange
	let mut derived = Query::select();
	derived.column("state").from("jobs").group_by("state");

	// Act
	let result = validate_postgres_outer_lock_on_derived(derived);

	// Assert
	assert_eq!(
		result,
		Err(QueryBuildError::UnsupportedBackendFeature {
			feature: "row locking with GROUP BY or HAVING queries",
			backend: "PostgreSQL",
		})
	);
}

#[rstest]
fn postgres_checked_builder_propagates_targetless_locks_to_derived_distinct_queries() {
	// Arrange
	let mut derived = Query::select();
	derived.column("id").from("jobs").distinct();

	// Act
	let result = validate_postgres_outer_lock_on_derived(derived);

	// Assert
	assert_eq!(
		result,
		Err(QueryBuildError::UnsupportedBackendFeature {
			feature: "row locking with DISTINCT queries",
			backend: "PostgreSQL",
		})
	);
}

#[rstest]
fn postgres_checked_builder_propagates_targetless_locks_to_derived_window_queries() {
	// Arrange
	let mut derived = Query::select();
	derived
		.expr(Expr::row_number().over(reinhardt_query::types::WindowStatement::default()))
		.from("jobs");

	// Act
	let result = validate_postgres_outer_lock_on_derived(derived);

	// Assert
	assert_eq!(
		result,
		Err(QueryBuildError::UnsupportedBackendFeature {
			feature: "row locking with window-function queries",
			backend: "PostgreSQL",
		})
	);
}

#[rstest]
fn postgres_checked_builder_propagates_targetless_locks_to_derived_union_queries() {
	// Arrange
	let mut derived = Query::select();
	derived.column("id").from("jobs");
	let mut union = Query::select();
	union.column("id").from("archived_jobs");
	derived.union(union);

	// Act
	let result = validate_postgres_outer_lock_on_derived(derived);

	// Assert
	assert_eq!(
		result,
		Err(QueryBuildError::UnsupportedBackendFeature {
			feature: "row locking on UNION queries",
			backend: "PostgreSQL",
		})
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
fn mysql_checked_builder_rejects_locks_on_union_queries() {
	// Arrange
	let mut statement = locked_select(LockType::Update);
	let mut union = Query::select();
	union.column("id").from("archived_users");
	statement.union(union);

	// Act
	let result = MySqlQueryBuilder::new().build_select_checked(&statement);

	// Assert
	assert_eq!(
		result,
		Err(QueryBuildError::UnsupportedBackendFeature {
			feature: "row locking on UNION queries",
			backend: "MySQL",
		})
	);
}

#[rstest]
fn mysql_checked_builder_rejects_lock_on_union_arm() {
	// Arrange
	let mut statement = Query::select();
	statement.column("id").from("users");
	let mut union = Query::select();
	union
		.column("id")
		.from("archived_users")
		.lock(LockType::Update);
	statement.union(union);

	// Act
	let result = MySqlQueryBuilder::new().build_select_checked(&statement);

	// Assert
	assert_eq!(
		result,
		Err(QueryBuildError::UnsupportedBackendFeature {
			feature: "row locking on UNION queries",
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

	let expected_lock = match lock_type {
		LockType::Update => "FOR UPDATE",
		LockType::NoKeyUpdate => "FOR NO KEY UPDATE",
		LockType::Share => "FOR SHARE",
		LockType::KeyShare => "FOR KEY SHARE",
		_ => unreachable!("the test cases cover every supported lock type"),
	};
	assert_eq!(
		sql,
		format!(r#"SELECT "id" FROM "users" {expected_lock} NOWAIT"#)
	);
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

#[rstest]
#[case::without_table_targets(false)]
#[case::with_table_targets(true)]
fn cockroach_checked_builder_rejects_skip_locked(#[case] with_table_targets: bool) {
	// Arrange
	let mut statement = locked_select(LockType::Update);
	statement.lock_behavior(LockBehavior::SkipLocked);
	if with_table_targets {
		statement.lock_tables([TableRef::table("users")]);
	}

	// Act
	let result = CockroachDBQueryBuilder::new().build_select_checked(&statement);

	// Assert
	assert_eq!(
		result,
		Err(QueryBuildError::UnsupportedBackendFeature {
			feature: "SKIP LOCKED row locking",
			backend: "CockroachDB",
		})
	);
}

#[cfg(feature = "pgvector")]
#[rstest]
fn checked_insert_builders_reject_pgvector_returning_values() {
	// Arrange
	let returning_value = SimpleExpr::Value(Value::Vector(Some(Box::new(vec![1.0, 2.0, 3.0]))));
	let mut statement = Query::insert();
	statement
		.into_table("documents")
		.column("name")
		.values_panic(["document"])
		.returning_exprs([returning_value]);

	// Act
	let mysql_result = MySqlQueryBuilder::new().build_insert_checked(&statement);
	let sqlite_result = SqliteQueryBuilder::new().build_insert_checked(&statement);
	let cockroach_result = CockroachDBQueryBuilder::new().build_insert_checked(&statement);
	let detected_feature = insert_pgvector_feature(&statement);

	// Assert
	assert_eq!(
		mysql_result,
		Err(QueryBuildError::UnsupportedBackendFeature {
			feature: "pgvector values",
			backend: "MySQL",
		})
	);
	assert_eq!(
		sqlite_result,
		Err(QueryBuildError::UnsupportedBackendFeature {
			feature: "pgvector values",
			backend: "SQLite",
		})
	);
	assert_eq!(
		cockroach_result,
		Err(QueryBuildError::UnsupportedBackendFeature {
			feature: "pgvector values",
			backend: "CockroachDB",
		})
	);
	assert_eq!(detected_feature, Some(PgvectorFeature::VectorValue));
}
