//! Integration tests for Tag CRUD operations
//!
//! Tests create, read, update, delete operations for Tag model with database.

use reinhardt_db::backends::DatabaseConnection;
use reinhardt_taggit_tests::fixtures::{insert_tag_to_db, taggit_db};
use rstest::rstest;
use sea_query::{Alias, Asterisk, Expr, ExprTrait, PostgresQueryBuilder, Query};
use sqlx::{PgPool, Row};
use std::sync::Arc;
use testcontainers::{ContainerAsync, GenericImage};

/// Test Tag creation
///
/// This test creates a tag in the database and verifies:
/// - The tag is saved successfully
/// - An ID is generated
/// - Fields match the input values
#[rstest]
#[tokio::test]
async fn test_tag_create(
	#[future] taggit_db: (
		ContainerAsync<GenericImage>,
		Arc<PgPool>,
		DatabaseConnection,
	),
) {
	// Arrange
	let (_container, pool, _db) = taggit_db.await;

	// Act
	let id = insert_tag_to_db(&pool, "rust", "rust").await;

	// Assert
	assert!(id > 0);

	let select_sql = Query::select()
		.column(Asterisk)
		.from(Alias::new("tags"))
		.and_where(Expr::col(Alias::new("id")).eq(id))
		.to_string(PostgresQueryBuilder);

	let row = sqlx::query(&select_sql)
		.fetch_one(pool.as_ref())
		.await
		.expect("Failed to fetch inserted tag");

	let name: String = row.get("name");
	let slug: String = row.get("slug");
	assert_eq!(name, "rust");
	assert_eq!(slug, "rust");
}

/// Test Tag reading by ID
///
/// This test creates a tag and then retrieves it by ID.
#[rstest]
#[tokio::test]
async fn test_tag_read(
	#[future] taggit_db: (
		ContainerAsync<GenericImage>,
		Arc<PgPool>,
		DatabaseConnection,
	),
) {
	// Arrange
	let (_container, pool, _db) = taggit_db.await;
	let id = insert_tag_to_db(&pool, "python", "python").await;

	// Act
	let select_sql = Query::select()
		.column(Asterisk)
		.from(Alias::new("tags"))
		.and_where(Expr::col(Alias::new("id")).eq(id))
		.to_string(PostgresQueryBuilder);

	let row = sqlx::query(&select_sql)
		.fetch_one(pool.as_ref())
		.await
		.expect("Failed to fetch tag by id");

	// Assert
	let fetched_id: i64 = row.get("id");
	let name: String = row.get("name");
	let slug: String = row.get("slug");
	assert_eq!(fetched_id, id);
	assert_eq!(name, "python");
	assert_eq!(slug, "python");
}

/// Test Tag update
///
/// This test creates a tag, updates its fields, and verifies the update.
#[rstest]
#[tokio::test]
async fn test_tag_update(
	#[future] taggit_db: (
		ContainerAsync<GenericImage>,
		Arc<PgPool>,
		DatabaseConnection,
	),
) {
	// Arrange
	let (_container, pool, _db) = taggit_db.await;
	let id = insert_tag_to_db(&pool, "rust", "rust").await;

	// Act
	let update_sql = Query::update()
		.table(Alias::new("tags"))
		.value(Alias::new("name"), Expr::value("rust-lang"))
		.value(Alias::new("slug"), Expr::value("rust-lang"))
		.and_where(Expr::col(Alias::new("id")).eq(id))
		.to_string(PostgresQueryBuilder);

	sqlx::query(&update_sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to update tag");

	// Assert
	let select_sql = Query::select()
		.column(Asterisk)
		.from(Alias::new("tags"))
		.and_where(Expr::col(Alias::new("id")).eq(id))
		.to_string(PostgresQueryBuilder);

	let row = sqlx::query(&select_sql)
		.fetch_one(pool.as_ref())
		.await
		.expect("Failed to fetch updated tag");

	let name: String = row.get("name");
	let slug: String = row.get("slug");
	assert_eq!(name, "rust-lang");
	assert_eq!(slug, "rust-lang");
}

/// Test Tag deletion
///
/// This test creates a tag, deletes it, and verifies it's removed.
#[rstest]
#[tokio::test]
async fn test_tag_delete(
	#[future] taggit_db: (
		ContainerAsync<GenericImage>,
		Arc<PgPool>,
		DatabaseConnection,
	),
) {
	// Arrange
	let (_container, pool, _db) = taggit_db.await;
	let id = insert_tag_to_db(&pool, "rust", "rust").await;

	// Act
	let delete_sql = Query::delete()
		.from_table(Alias::new("tags"))
		.and_where(Expr::col(Alias::new("id")).eq(id))
		.to_string(PostgresQueryBuilder);

	sqlx::query(&delete_sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to delete tag");

	// Assert
	let select_sql = Query::select()
		.column(Asterisk)
		.from(Alias::new("tags"))
		.and_where(Expr::col(Alias::new("id")).eq(id))
		.to_string(PostgresQueryBuilder);

	let row = sqlx::query(&select_sql)
		.fetch_optional(pool.as_ref())
		.await
		.expect("Failed to query after delete");

	assert!(row.is_none());
}

/// Test Tag name unique constraint
///
/// This test verifies that creating two tags with the same name fails.
#[rstest]
#[tokio::test]
async fn test_tag_name_unique_constraint(
	#[future] taggit_db: (
		ContainerAsync<GenericImage>,
		Arc<PgPool>,
		DatabaseConnection,
	),
) {
	// Arrange
	let (_container, pool, _db) = taggit_db.await;
	insert_tag_to_db(&pool, "rust", "rust").await;

	// Act - insert a duplicate name with different slug
	let dup_sql = Query::insert()
		.into_table(Alias::new("tags"))
		.columns([Alias::new("name"), Alias::new("slug")])
		.values_panic(["rust".into(), "rust-duplicate".into()])
		.to_string(PostgresQueryBuilder);

	let result = sqlx::query(&dup_sql).execute(pool.as_ref()).await;

	// Assert
	assert!(result.is_err());
}

/// Test Tag slug unique constraint
///
/// This test verifies that creating two tags with the same slug fails.
#[rstest]
#[tokio::test]
async fn test_tag_slug_unique_constraint(
	#[future] taggit_db: (
		ContainerAsync<GenericImage>,
		Arc<PgPool>,
		DatabaseConnection,
	),
) {
	// Arrange
	let (_container, pool, _db) = taggit_db.await;
	insert_tag_to_db(&pool, "rust", "rust").await;

	// Act - insert a different name with duplicate slug
	let dup_sql = Query::insert()
		.into_table(Alias::new("tags"))
		.columns([Alias::new("name"), Alias::new("slug")])
		.values_panic(["rust-lang".into(), "rust".into()])
		.to_string(PostgresQueryBuilder);

	let result = sqlx::query(&dup_sql).execute(pool.as_ref()).await;

	// Assert
	assert!(result.is_err());
}

/// Test Tag max length constraint (255 characters)
///
/// This test verifies that tag names and slugs up to 255 characters succeed,
/// but 256 characters fail.
#[rstest]
#[tokio::test]
async fn test_tag_max_length_constraint(
	#[future] taggit_db: (
		ContainerAsync<GenericImage>,
		Arc<PgPool>,
		DatabaseConnection,
	),
) {
	// Arrange
	let (_container, pool, _db) = taggit_db.await;
	let name_255 = "a".repeat(255);
	let slug_255 = "b".repeat(255);

	// Act - 255 chars should succeed
	let id = insert_tag_to_db(&pool, &name_255, &slug_255).await;

	// Assert - 255 chars stored correctly
	assert!(id > 0);

	// Act - 256 chars should fail
	let name_256 = "c".repeat(256);
	let slug_256 = "d".repeat(256);

	let over_sql = Query::insert()
		.into_table(Alias::new("tags"))
		.columns([Alias::new("name"), Alias::new("slug")])
		.values_panic([name_256.into(), slug_256.into()])
		.to_string(PostgresQueryBuilder);

	let result = sqlx::query(&over_sql).execute(pool.as_ref()).await;

	// Assert - 256 chars should be rejected by VARCHAR(255)
	assert!(result.is_err());
}

/// Test Tag created_at auto-generation
///
/// This test verifies that created_at is automatically set when saving.
#[rstest]
#[tokio::test]
async fn test_tag_created_at_auto_generation(
	#[future] taggit_db: (
		ContainerAsync<GenericImage>,
		Arc<PgPool>,
		DatabaseConnection,
	),
) {
	// Arrange
	let (_container, pool, _db) = taggit_db.await;
	let before = chrono::Utc::now();

	// Act
	let id = insert_tag_to_db(&pool, "rust", "rust").await;

	// Assert
	let select_sql = Query::select()
		.column(Asterisk)
		.from(Alias::new("tags"))
		.and_where(Expr::col(Alias::new("id")).eq(id))
		.to_string(PostgresQueryBuilder);

	let row = sqlx::query(&select_sql)
		.fetch_one(pool.as_ref())
		.await
		.expect("Failed to fetch tag");

	let created_at: chrono::DateTime<chrono::Utc> = row.get("created_at");
	let after = chrono::Utc::now();

	assert!(created_at >= before);
	assert!(created_at <= after);
}

/// Test Tag list all
///
/// This test creates multiple tags and retrieves all of them.
#[rstest]
#[tokio::test]
async fn test_tag_list_all(
	#[future] taggit_db: (
		ContainerAsync<GenericImage>,
		Arc<PgPool>,
		DatabaseConnection,
	),
) {
	// Arrange
	let (_container, pool, _db) = taggit_db.await;
	insert_tag_to_db(&pool, "rust", "rust").await;
	insert_tag_to_db(&pool, "python", "python").await;
	insert_tag_to_db(&pool, "javascript", "javascript").await;

	// Act
	let select_sql = Query::select()
		.column(Asterisk)
		.from(Alias::new("tags"))
		.to_string(PostgresQueryBuilder);

	let rows = sqlx::query(&select_sql)
		.fetch_all(pool.as_ref())
		.await
		.expect("Failed to fetch all tags");

	// Assert
	assert_eq!(rows.len(), 3);
}

/// Test Tag filter by name
///
/// This test creates multiple tags and filters them by name.
#[rstest]
#[tokio::test]
async fn test_tag_filter_by_name(
	#[future] taggit_db: (
		ContainerAsync<GenericImage>,
		Arc<PgPool>,
		DatabaseConnection,
	),
) {
	// Arrange
	let (_container, pool, _db) = taggit_db.await;
	insert_tag_to_db(&pool, "rust", "rust").await;
	insert_tag_to_db(&pool, "python", "python").await;
	insert_tag_to_db(&pool, "javascript", "javascript").await;

	// Act
	let select_sql = Query::select()
		.column(Asterisk)
		.from(Alias::new("tags"))
		.and_where(Expr::col(Alias::new("name")).eq("python"))
		.to_string(PostgresQueryBuilder);

	let rows = sqlx::query(&select_sql)
		.fetch_all(pool.as_ref())
		.await
		.expect("Failed to filter tags by name");

	// Assert
	assert_eq!(rows.len(), 1);
	let name: String = rows[0].get("name");
	assert_eq!(name, "python");
}
