//! Integration tests for CASCADE DELETE behavior
//!
//! Tests that TaggedItems are deleted when their associated Tag is deleted.

use reinhardt_db::backends::DatabaseConnection;
use reinhardt_taggit_tests::fixtures::{insert_tag_to_db, insert_tagged_item_to_db, taggit_db};
use rstest::rstest;
use sea_query::{Alias, Asterisk, Expr, ExprTrait, PostgresQueryBuilder, Query};
use sqlx::PgPool;
use std::sync::Arc;
use testcontainers::{ContainerAsync, GenericImage};

/// Test CASCADE DELETE on Tag deletion
///
/// This test verifies that when a Tag is deleted, all associated
/// TaggedItems are also deleted.
#[rstest]
#[tokio::test]
async fn test_cascade_delete_tag_deletes_tagged_items(
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

	// Act - delete the tag
	let delete_sql = Query::delete()
		.from_table(Alias::new("tags"))
		.and_where(Expr::col(Alias::new("id")).eq(tag_id))
		.to_string(PostgresQueryBuilder);

	sqlx::query(&delete_sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to delete tag");

	// Assert - tagged items should be cascade deleted
	let select_sql = Query::select()
		.column(Asterisk)
		.from(Alias::new("tagged_items"))
		.and_where(Expr::col(Alias::new("tag_id")).eq(tag_id))
		.to_string(PostgresQueryBuilder);

	let rows = sqlx::query(&select_sql)
		.fetch_all(pool.as_ref())
		.await
		.expect("Failed to query tagged items after cascade delete");

	assert_eq!(rows.len(), 0);
}

/// Test CASCADE DELETE with multiple TaggedItems
///
/// This test verifies that when a Tag with multiple TaggedItems
/// is deleted, all TaggedItems are deleted.
#[rstest]
#[tokio::test]
async fn test_cascade_delete_multiple_tagged_items(
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
	insert_tagged_item_to_db(&pool, tag_id, "Food", 2).await;
	insert_tagged_item_to_db(&pool, tag_id, "Food", 3).await;

	// Act
	let delete_sql = Query::delete()
		.from_table(Alias::new("tags"))
		.and_where(Expr::col(Alias::new("id")).eq(tag_id))
		.to_string(PostgresQueryBuilder);

	sqlx::query(&delete_sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to delete tag");

	// Assert
	let select_sql = Query::select()
		.column(Asterisk)
		.from(Alias::new("tagged_items"))
		.to_string(PostgresQueryBuilder);

	let rows = sqlx::query(&select_sql)
		.fetch_all(pool.as_ref())
		.await
		.expect("Failed to query tagged items");

	assert_eq!(rows.len(), 0);
}

/// Test CASCADE DELETE across different content types
///
/// This test verifies that CASCADE DELETE works correctly when
/// TaggedItems of different content types reference the same Tag.
#[rstest]
#[tokio::test]
async fn test_cascade_delete_across_content_types(
	#[future] taggit_db: (
		ContainerAsync<GenericImage>,
		Arc<PgPool>,
		DatabaseConnection,
	),
) {
	// Arrange
	let (_container, pool, _db) = taggit_db.await;
	let tag_id = insert_tag_to_db(&pool, "healthy", "healthy").await;
	insert_tagged_item_to_db(&pool, tag_id, "Food", 1).await;
	insert_tagged_item_to_db(&pool, tag_id, "Recipe", 2).await;
	insert_tagged_item_to_db(&pool, tag_id, "Article", 3).await;

	// Act
	let delete_sql = Query::delete()
		.from_table(Alias::new("tags"))
		.and_where(Expr::col(Alias::new("id")).eq(tag_id))
		.to_string(PostgresQueryBuilder);

	sqlx::query(&delete_sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to delete tag");

	// Assert - all tagged items across content types should be gone
	let select_sql = Query::select()
		.column(Asterisk)
		.from(Alias::new("tagged_items"))
		.to_string(PostgresQueryBuilder);

	let rows = sqlx::query(&select_sql)
		.fetch_all(pool.as_ref())
		.await
		.expect("Failed to query tagged items");

	assert_eq!(rows.len(), 0);
}

/// Test CASCADE DELETE does not affect other Tags
///
/// This test verifies that deleting one Tag does not affect TaggedItems
/// associated with other Tags.
#[rstest]
#[tokio::test]
async fn test_cascade_delete_does_not_affect_other_tags(
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
	insert_tagged_item_to_db(&pool, tag2_id, "Food", 2).await;
	insert_tagged_item_to_db(&pool, tag2_id, "Recipe", 3).await;

	// Act - delete only tag1
	let delete_sql = Query::delete()
		.from_table(Alias::new("tags"))
		.and_where(Expr::col(Alias::new("id")).eq(tag1_id))
		.to_string(PostgresQueryBuilder);

	sqlx::query(&delete_sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to delete tag");

	// Assert - tag2's items should still exist
	let select_sql = Query::select()
		.column(Asterisk)
		.from(Alias::new("tagged_items"))
		.and_where(Expr::col(Alias::new("tag_id")).eq(tag2_id))
		.to_string(PostgresQueryBuilder);

	let rows = sqlx::query(&select_sql)
		.fetch_all(pool.as_ref())
		.await
		.expect("Failed to query remaining tagged items");

	assert_eq!(rows.len(), 2);

	// tag1's items should be gone
	let select_tag1_sql = Query::select()
		.column(Asterisk)
		.from(Alias::new("tagged_items"))
		.and_where(Expr::col(Alias::new("tag_id")).eq(tag1_id))
		.to_string(PostgresQueryBuilder);

	let tag1_rows = sqlx::query(&select_tag1_sql)
		.fetch_all(pool.as_ref())
		.await
		.expect("Failed to query deleted tag's items");

	assert_eq!(tag1_rows.len(), 0);
}

/// Test CASCADE DELETE chain behavior
///
/// This test verifies the behavior when multiple Tags are deleted
/// sequentially.
#[rstest]
#[tokio::test]
async fn test_cascade_delete_chain_behavior(
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
	let tag3_id = insert_tag_to_db(&pool, "javascript", "javascript").await;

	insert_tagged_item_to_db(&pool, tag1_id, "Food", 1).await;
	insert_tagged_item_to_db(&pool, tag2_id, "Food", 2).await;
	insert_tagged_item_to_db(&pool, tag3_id, "Food", 3).await;

	// Act - delete tags one by one
	for tag_id in [tag1_id, tag2_id, tag3_id] {
		let delete_sql = Query::delete()
			.from_table(Alias::new("tags"))
			.and_where(Expr::col(Alias::new("id")).eq(tag_id))
			.to_string(PostgresQueryBuilder);

		sqlx::query(&delete_sql)
			.execute(pool.as_ref())
			.await
			.expect("Failed to delete tag");
	}

	// Assert - all tagged items should be gone
	let select_sql = Query::select()
		.column(Asterisk)
		.from(Alias::new("tagged_items"))
		.to_string(PostgresQueryBuilder);

	let rows = sqlx::query(&select_sql)
		.fetch_all(pool.as_ref())
		.await
		.expect("Failed to query tagged items");

	assert_eq!(rows.len(), 0);

	// All tags should also be gone
	let select_tags_sql = Query::select()
		.column(Asterisk)
		.from(Alias::new("tags"))
		.to_string(PostgresQueryBuilder);

	let tag_rows = sqlx::query(&select_tags_sql)
		.fetch_all(pool.as_ref())
		.await
		.expect("Failed to query tags");

	assert_eq!(tag_rows.len(), 0);
}

/// Test CASCADE DELETE verification after deletion
///
/// This test verifies that TaggedItems cannot be retrieved after
/// their associated Tag is deleted.
#[rstest]
#[tokio::test]
async fn test_cascade_delete_verification_after_deletion(
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

	// Verify item exists before deletion
	let pre_sql = Query::select()
		.column(Asterisk)
		.from(Alias::new("tagged_items"))
		.and_where(Expr::col(Alias::new("id")).eq(item_id))
		.to_string(PostgresQueryBuilder);

	let pre_row = sqlx::query(&pre_sql)
		.fetch_optional(pool.as_ref())
		.await
		.expect("Failed to query before delete");

	assert!(pre_row.is_some());

	// Act - delete the tag
	let delete_sql = Query::delete()
		.from_table(Alias::new("tags"))
		.and_where(Expr::col(Alias::new("id")).eq(tag_id))
		.to_string(PostgresQueryBuilder);

	sqlx::query(&delete_sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to delete tag");

	// Assert - item should no longer be retrievable by its id
	let post_sql = Query::select()
		.column(Asterisk)
		.from(Alias::new("tagged_items"))
		.and_where(Expr::col(Alias::new("id")).eq(item_id))
		.to_string(PostgresQueryBuilder);

	let post_row = sqlx::query(&post_sql)
		.fetch_optional(pool.as_ref())
		.await
		.expect("Failed to query after delete");

	assert!(post_row.is_none());
}
