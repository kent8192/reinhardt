// Combination tests for INSERT statement
//
// These tests verify INSERT combined with advanced SQL features:
// ON CONFLICT DO UPDATE (upsert), ON CONFLICT DO NOTHING, and multi-column
// ON CONFLICT targeting.

use reinhardt_query::prelude::*;
use rstest::*;

/// Test INSERT ... ON CONFLICT DO UPDATE (UPSERT)
///
/// Verifies that `on_conflict` with `update_columns` generates correct upsert SQL.
#[rstest]
fn test_insert_on_conflict_do_update() {
	// Arrange
	let stmt = Query::insert()
		.into_table("users")
		.columns(["name", "email", "age"])
		.values_panic([
			Value::String(Some(Box::new("Alice Updated".to_string()))),
			Value::String(Some(Box::new("alice@example.com".to_string()))),
			Value::Int(Some(35)),
		])
		.on_conflict(OnConflict::column("email").update_columns(["name", "age"]))
		.to_owned();

	// Act
	let builder = PostgresQueryBuilder;
	let (sql, values) = builder.build_insert(&stmt);

	// Assert
	assert_eq!(
		sql,
		r#"INSERT INTO "users" ("name", "email", "age") VALUES ($1, $2, $3) ON CONFLICT ("email") DO UPDATE SET "name" = EXCLUDED."name", "age" = EXCLUDED."age""#
	);
	assert_eq!(values.len(), 3);
}

/// Test INSERT ... ON CONFLICT DO NOTHING
///
/// Verifies that `on_conflict` with `do_nothing` generates correct SQL
/// to skip conflicting rows.
#[rstest]
fn test_insert_on_conflict_do_nothing() {
	// Arrange
	let stmt = Query::insert()
		.into_table("users")
		.columns(["name", "email", "age"])
		.values_panic([
			Value::String(Some(Box::new("Bob".to_string()))),
			Value::String(Some(Box::new("bob@example.com".to_string()))),
			Value::Int(Some(25)),
		])
		.on_conflict(OnConflict::column("email").do_nothing())
		.to_owned();

	// Act
	let builder = PostgresQueryBuilder;
	let (sql, values) = builder.build_insert(&stmt);

	// Assert
	assert_eq!(
		sql,
		r#"INSERT INTO "users" ("name", "email", "age") VALUES ($1, $2, $3) ON CONFLICT ("email") DO NOTHING"#
	);
	assert_eq!(values.len(), 3);
}

/// Test INSERT with multi-column ON CONFLICT
///
/// Verifies that `on_conflict` can target multiple columns for composite
/// unique constraints.
#[rstest]
fn test_insert_on_conflict_multi_column() {
	// Arrange
	let stmt = Query::insert()
		.into_table("user_roles")
		.columns(["user_id", "role_id", "granted_at"])
		.values_panic([
			Value::Int(Some(1)),
			Value::Int(Some(10)),
			Value::String(Some(Box::new("2024-01-01".to_string()))),
		])
		.on_conflict(OnConflict::columns(["user_id", "role_id"]).update_columns(["granted_at"]))
		.to_owned();

	// Act
	let builder = PostgresQueryBuilder;
	let (sql, values) = builder.build_insert(&stmt);

	// Assert
	assert_eq!(
		sql,
		r#"INSERT INTO "user_roles" ("user_id", "role_id", "granted_at") VALUES ($1, $2, $3) ON CONFLICT ("user_id", "role_id") DO UPDATE SET "granted_at" = EXCLUDED."granted_at""#
	);
	assert_eq!(values.len(), 3);
}
