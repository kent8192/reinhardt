//! Integration tests for TaggedItem CRUD operations
//!
//! Tests create, read, update, delete operations for TaggedItem model with database.

use reinhardt_db::backends::DatabaseConnection;
use reinhardt_taggit_tests::fixtures::{insert_tag_to_db, insert_tagged_item_to_db, taggit_db};
use rstest::rstest;
use sea_query::{Alias, Asterisk, Expr, ExprTrait, PostgresQueryBuilder, Query};
use sqlx::{PgPool, Row};
use std::sync::Arc;
use testcontainers::{ContainerAsync, GenericImage};

/// Test TaggedItem creation
///
/// This test creates a tagged_item in the database and verifies:
/// - The tagged_item is saved successfully
/// - An ID is generated
/// - Fields match the input values
#[rstest]
#[tokio::test]
async fn test_tagged_item_create(
	#[future] taggit_db: (
		ContainerAsync<GenericImage>,
		Arc<PgPool>,
		DatabaseConnection,
	),
) {
	// Arrange
	let (_container, pool, _db) = taggit_db.await;
	let tag_id = insert_tag_to_db(&pool, "rust", "rust").await;

	// Act
	let item_id = insert_tagged_item_to_db(&pool, tag_id, "Food", 42).await;

	// Assert
	assert!(item_id > 0);

	let select_sql = Query::select()
		.column(Asterisk)
		.from(Alias::new("tagged_items"))
		.and_where(Expr::col(Alias::new("id")).eq(item_id))
		.to_string(PostgresQueryBuilder);

	let row = sqlx::query(&select_sql)
		.fetch_one(pool.as_ref())
		.await
		.expect("Failed to fetch inserted tagged item");

	let fetched_tag_id: i64 = row.get("tag_id");
	let content_type: String = row.get("content_type");
	let object_id: i64 = row.get("object_id");
	assert_eq!(fetched_tag_id, tag_id);
	assert_eq!(content_type, "Food");
	assert_eq!(object_id, 42);
}

/// Test TaggedItem reading by ID
///
/// This test creates a tagged_item and then retrieves it by ID.
#[rstest]
#[tokio::test]
async fn test_tagged_item_read(
	#[future] taggit_db: (
		ContainerAsync<GenericImage>,
		Arc<PgPool>,
		DatabaseConnection,
	),
) {
	// Arrange
	let (_container, pool, _db) = taggit_db.await;
	let tag_id = insert_tag_to_db(&pool, "python", "python").await;
	let item_id = insert_tagged_item_to_db(&pool, tag_id, "Recipe", 100).await;

	// Act
	let select_sql = Query::select()
		.column(Asterisk)
		.from(Alias::new("tagged_items"))
		.and_where(Expr::col(Alias::new("id")).eq(item_id))
		.to_string(PostgresQueryBuilder);

	let row = sqlx::query(&select_sql)
		.fetch_one(pool.as_ref())
		.await
		.expect("Failed to fetch tagged item by id");

	// Assert
	let fetched_id: i64 = row.get("id");
	let content_type: String = row.get("content_type");
	let object_id: i64 = row.get("object_id");
	assert_eq!(fetched_id, item_id);
	assert_eq!(content_type, "Recipe");
	assert_eq!(object_id, 100);
}

/// Test TaggedItem update
///
/// This test creates a tagged_item, updates its fields, and verifies the update.
#[rstest]
#[tokio::test]
async fn test_tagged_item_update(
	#[future] taggit_db: (
		ContainerAsync<GenericImage>,
		Arc<PgPool>,
		DatabaseConnection,
	),
) {
	// Arrange
	let (_container, pool, _db) = taggit_db.await;
	let tag_id = insert_tag_to_db(&pool, "rust", "rust").await;
	let item_id = insert_tagged_item_to_db(&pool, tag_id, "Food", 42).await;

	// Act
	let update_sql = Query::update()
		.table(Alias::new("tagged_items"))
		.value(Alias::new("content_type"), Expr::value("Recipe"))
		.value(Alias::new("object_id"), Expr::value(100i64))
		.and_where(Expr::col(Alias::new("id")).eq(item_id))
		.to_string(PostgresQueryBuilder);

	sqlx::query(&update_sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to update tagged item");

	// Assert
	let select_sql = Query::select()
		.column(Asterisk)
		.from(Alias::new("tagged_items"))
		.and_where(Expr::col(Alias::new("id")).eq(item_id))
		.to_string(PostgresQueryBuilder);

	let row = sqlx::query(&select_sql)
		.fetch_one(pool.as_ref())
		.await
		.expect("Failed to fetch updated tagged item");

	let content_type: String = row.get("content_type");
	let object_id: i64 = row.get("object_id");
	assert_eq!(content_type, "Recipe");
	assert_eq!(object_id, 100);
}

/// Test TaggedItem deletion
///
/// This test creates a tagged_item, deletes it, and verifies it's removed.
#[rstest]
#[tokio::test]
async fn test_tagged_item_delete(
	#[future] taggit_db: (
		ContainerAsync<GenericImage>,
		Arc<PgPool>,
		DatabaseConnection,
	),
) {
	// Arrange
	let (_container, pool, _db) = taggit_db.await;
	let tag_id = insert_tag_to_db(&pool, "rust", "rust").await;
	let item_id = insert_tagged_item_to_db(&pool, tag_id, "Food", 42).await;

	// Act
	let delete_sql = Query::delete()
		.from_table(Alias::new("tagged_items"))
		.and_where(Expr::col(Alias::new("id")).eq(item_id))
		.to_string(PostgresQueryBuilder);

	sqlx::query(&delete_sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to delete tagged item");

	// Assert
	let select_sql = Query::select()
		.column(Asterisk)
		.from(Alias::new("tagged_items"))
		.and_where(Expr::col(Alias::new("id")).eq(item_id))
		.to_string(PostgresQueryBuilder);

	let row = sqlx::query(&select_sql)
		.fetch_optional(pool.as_ref())
		.await
		.expect("Failed to query after delete");

	assert!(row.is_none());
}

/// Test TaggedItem unique composite constraint
///
/// The composite unique constraint (tag_id, content_type, object_id)
/// is not yet enforced at the database level. This test verifies
/// that duplicate combinations can be inserted (current behavior).
// TODO: Add UNIQUE(tag_id, content_type, object_id) constraint to TaggedItem model
#[rstest]
#[tokio::test]
async fn test_tagged_item_unique_composite_constraint(
	#[future] taggit_db: (
		ContainerAsync<GenericImage>,
		Arc<PgPool>,
		DatabaseConnection,
	),
) {
	// Arrange
	let (_container, pool, _db) = taggit_db.await;
	let tag_id = insert_tag_to_db(&pool, "rust", "rust").await;
	insert_tagged_item_to_db(&pool, tag_id, "Food", 42).await;

	// Act - insert duplicate combination (currently allowed)
	let dup_id = insert_tagged_item_to_db(&pool, tag_id, "Food", 42).await;

	// Assert - duplicate is currently accepted (no composite unique constraint)
	assert!(dup_id > 0);
}

/// Test TaggedItem foreign key constraint
///
/// This test verifies that creating a TaggedItem with a non-existent
/// tag_id fails due to foreign key constraint.
#[rstest]
#[tokio::test]
async fn test_tagged_item_foreign_key_constraint(
	#[future] taggit_db: (
		ContainerAsync<GenericImage>,
		Arc<PgPool>,
		DatabaseConnection,
	),
) {
	// Arrange
	let (_container, pool, _db) = taggit_db.await;
	let non_existent_tag_id = 99999i64;

	// Act
	let insert_sql = Query::insert()
		.into_table(Alias::new("tagged_items"))
		.columns([
			Alias::new("tag_id"),
			Alias::new("content_type"),
			Alias::new("object_id"),
		])
		.values_panic([non_existent_tag_id.into(), "Food".into(), 42i64.into()])
		.to_string(PostgresQueryBuilder);

	let result = sqlx::query(&insert_sql).execute(pool.as_ref()).await;

	// Assert
	assert!(result.is_err());
}

/// Test TaggedItem polymorphic content types
///
/// This test verifies that different content types can use the same tag.
#[rstest]
#[tokio::test]
async fn test_tagged_item_polymorphic_content_types(
	#[future] taggit_db: (
		ContainerAsync<GenericImage>,
		Arc<PgPool>,
		DatabaseConnection,
	),
) {
	// Arrange
	let (_container, pool, _db) = taggit_db.await;
	let tag_id = insert_tag_to_db(&pool, "healthy", "healthy").await;

	// Act
	let food_id = insert_tagged_item_to_db(&pool, tag_id, "Food", 1).await;
	let recipe_id = insert_tagged_item_to_db(&pool, tag_id, "Recipe", 1).await;
	let article_id = insert_tagged_item_to_db(&pool, tag_id, "Article", 1).await;

	// Assert
	assert!(food_id > 0);
	assert!(recipe_id > 0);
	assert!(article_id > 0);

	let select_sql = Query::select()
		.column(Asterisk)
		.from(Alias::new("tagged_items"))
		.and_where(Expr::col(Alias::new("tag_id")).eq(tag_id))
		.to_string(PostgresQueryBuilder);

	let rows = sqlx::query(&select_sql)
		.fetch_all(pool.as_ref())
		.await
		.expect("Failed to fetch tagged items by tag_id");

	assert_eq!(rows.len(), 3);
}

/// Test TaggedItem same object different tags
///
/// This test verifies that the same object can have multiple tags.
#[rstest]
#[tokio::test]
async fn test_tagged_item_same_object_different_tags(
	#[future] taggit_db: (
		ContainerAsync<GenericImage>,
		Arc<PgPool>,
		DatabaseConnection,
	),
) {
	// Arrange
	let (_container, pool, _db) = taggit_db.await;
	let tag1_id = insert_tag_to_db(&pool, "rust", "rust").await;
	let tag2_id = insert_tag_to_db(&pool, "programming", "programming").await;
	let tag3_id = insert_tag_to_db(&pool, "systems", "systems").await;

	// Act - tag the same object (Food:42) with three different tags
	insert_tagged_item_to_db(&pool, tag1_id, "Food", 42).await;
	insert_tagged_item_to_db(&pool, tag2_id, "Food", 42).await;
	insert_tagged_item_to_db(&pool, tag3_id, "Food", 42).await;

	// Assert
	let select_sql = Query::select()
		.column(Asterisk)
		.from(Alias::new("tagged_items"))
		.and_where(Expr::col(Alias::new("content_type")).eq("Food"))
		.and_where(Expr::col(Alias::new("object_id")).eq(42i64))
		.to_string(PostgresQueryBuilder);

	let rows = sqlx::query(&select_sql)
		.fetch_all(pool.as_ref())
		.await
		.expect("Failed to fetch tagged items for object");

	assert_eq!(rows.len(), 3);
}

/// Test TaggedItem created_at auto-generation
///
/// This test verifies that created_at is automatically set when saving.
#[rstest]
#[tokio::test]
async fn test_tagged_item_created_at_auto_generation(
	#[future] taggit_db: (
		ContainerAsync<GenericImage>,
		Arc<PgPool>,
		DatabaseConnection,
	),
) {
	// Arrange
	let (_container, pool, _db) = taggit_db.await;
	let tag_id = insert_tag_to_db(&pool, "rust", "rust").await;
	let before = chrono::Utc::now();

	// Act
	let item_id = insert_tagged_item_to_db(&pool, tag_id, "Food", 42).await;

	// Assert
	let select_sql = Query::select()
		.column(Asterisk)
		.from(Alias::new("tagged_items"))
		.and_where(Expr::col(Alias::new("id")).eq(item_id))
		.to_string(PostgresQueryBuilder);

	let row = sqlx::query(&select_sql)
		.fetch_one(pool.as_ref())
		.await
		.expect("Failed to fetch tagged item");

	let created_at: chrono::DateTime<chrono::Utc> = row.get("created_at");
	let after = chrono::Utc::now();

	assert!(created_at >= before);
	assert!(created_at <= after);
}

/// Test TaggedItem list all
///
/// This test creates multiple tagged_items and retrieves all of them.
#[rstest]
#[tokio::test]
async fn test_tagged_item_list_all(
	#[future] taggit_db: (
		ContainerAsync<GenericImage>,
		Arc<PgPool>,
		DatabaseConnection,
	),
) {
	// Arrange
	let (_container, pool, _db) = taggit_db.await;
	let tag_id = insert_tag_to_db(&pool, "rust", "rust").await;
	insert_tagged_item_to_db(&pool, tag_id, "Food", 1).await;
	insert_tagged_item_to_db(&pool, tag_id, "Recipe", 2).await;
	insert_tagged_item_to_db(&pool, tag_id, "Article", 3).await;

	// Act
	let select_sql = Query::select()
		.column(Asterisk)
		.from(Alias::new("tagged_items"))
		.to_string(PostgresQueryBuilder);

	let rows = sqlx::query(&select_sql)
		.fetch_all(pool.as_ref())
		.await
		.expect("Failed to fetch all tagged items");

	// Assert
	assert_eq!(rows.len(), 3);
}

/// Test TaggedItem filter by tag_id
///
/// This test creates multiple tagged_items and filters them by tag_id.
#[rstest]
#[tokio::test]
async fn test_tagged_item_filter_by_tag_id(
	#[future] taggit_db: (
		ContainerAsync<GenericImage>,
		Arc<PgPool>,
		DatabaseConnection,
	),
) {
	// Arrange
	let (_container, pool, _db) = taggit_db.await;
	let tag1_id = insert_tag_to_db(&pool, "rust", "rust").await;
	let tag2_id = insert_tag_to_db(&pool, "python", "python").await;
	insert_tagged_item_to_db(&pool, tag1_id, "Food", 1).await;
	insert_tagged_item_to_db(&pool, tag1_id, "Food", 2).await;
	insert_tagged_item_to_db(&pool, tag2_id, "Food", 3).await;

	// Act
	let select_sql = Query::select()
		.column(Asterisk)
		.from(Alias::new("tagged_items"))
		.and_where(Expr::col(Alias::new("tag_id")).eq(tag1_id))
		.to_string(PostgresQueryBuilder);

	let rows = sqlx::query(&select_sql)
		.fetch_all(pool.as_ref())
		.await
		.expect("Failed to filter by tag_id");

	// Assert
	assert_eq!(rows.len(), 2);
}

/// Test TaggedItem filter by content type
///
/// This test creates multiple tagged_items and filters them by content_type.
#[rstest]
#[tokio::test]
async fn test_tagged_item_filter_by_content_type(
	#[future] taggit_db: (
		ContainerAsync<GenericImage>,
		Arc<PgPool>,
		DatabaseConnection,
	),
) {
	// Arrange
	let (_container, pool, _db) = taggit_db.await;
	let tag_id = insert_tag_to_db(&pool, "rust", "rust").await;
	insert_tagged_item_to_db(&pool, tag_id, "Food", 1).await;
	insert_tagged_item_to_db(&pool, tag_id, "Recipe", 2).await;
	insert_tagged_item_to_db(&pool, tag_id, "Food", 3).await;

	// Act
	let select_sql = Query::select()
		.column(Asterisk)
		.from(Alias::new("tagged_items"))
		.and_where(Expr::col(Alias::new("content_type")).eq("Food"))
		.to_string(PostgresQueryBuilder);

	let rows = sqlx::query(&select_sql)
		.fetch_all(pool.as_ref())
		.await
		.expect("Failed to filter by content_type");

	// Assert
	assert_eq!(rows.len(), 2);
}

/// Test TaggedItem filter by object_id
///
/// This test creates multiple tagged_items and filters them by object_id.
#[rstest]
#[tokio::test]
async fn test_tagged_item_filter_by_object_id(
	#[future] taggit_db: (
		ContainerAsync<GenericImage>,
		Arc<PgPool>,
		DatabaseConnection,
	),
) {
	// Arrange
	let (_container, pool, _db) = taggit_db.await;
	let tag1_id = insert_tag_to_db(&pool, "rust", "rust").await;
	let tag2_id = insert_tag_to_db(&pool, "python", "python").await;
	insert_tagged_item_to_db(&pool, tag1_id, "Food", 42).await;
	insert_tagged_item_to_db(&pool, tag2_id, "Food", 42).await;
	insert_tagged_item_to_db(&pool, tag1_id, "Food", 99).await;

	// Act
	let select_sql = Query::select()
		.column(Asterisk)
		.from(Alias::new("tagged_items"))
		.and_where(Expr::col(Alias::new("object_id")).eq(42i64))
		.to_string(PostgresQueryBuilder);

	let rows = sqlx::query(&select_sql)
		.fetch_all(pool.as_ref())
		.await
		.expect("Failed to filter by object_id");

	// Assert
	assert_eq!(rows.len(), 2);
}
