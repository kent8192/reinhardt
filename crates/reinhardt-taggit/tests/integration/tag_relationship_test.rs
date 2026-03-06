//! Integration tests for Tag-TaggedItem relationship
//!
//! Tests the relationship between Tag and TaggedItem models.

use reinhardt_db::backends::DatabaseConnection;
use reinhardt_taggit_tests::fixtures::{insert_tag_to_db, insert_tagged_item_to_db, taggit_db};
use rstest::rstest;
use sea_query::{Alias, Asterisk, Expr, ExprTrait, Func, JoinType, PostgresQueryBuilder, Query};
use sqlx::{PgPool, Row};
use std::sync::Arc;
use testcontainers::{ContainerAsync, GenericImage};

/// Test Tag-TaggedItem relationship
///
/// This test verifies that TaggedItems correctly reference Tags.
#[rstest]
#[tokio::test]
async fn test_tag_tagged_item_relationship(
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

	// Act - join query to verify relationship
	let select_sql = Query::select()
		.expr_as(
			Expr::col((Alias::new("t"), Alias::new("name"))),
			Alias::new("tag_name"),
		)
		.expr_as(
			Expr::col((Alias::new("ti"), Alias::new("content_type"))),
			Alias::new("content_type"),
		)
		.expr_as(
			Expr::col((Alias::new("ti"), Alias::new("object_id"))),
			Alias::new("object_id"),
		)
		.from_as(Alias::new("tagged_items"), Alias::new("ti"))
		.join_as(
			JoinType::InnerJoin,
			Alias::new("tags"),
			Alias::new("t"),
			Expr::col((Alias::new("ti"), Alias::new("tag_id")))
				.equals((Alias::new("t"), Alias::new("id"))),
		)
		.and_where(Expr::col((Alias::new("ti"), Alias::new("id"))).eq(item_id))
		.to_string(PostgresQueryBuilder);

	let row = sqlx::query(&select_sql)
		.fetch_one(pool.as_ref())
		.await
		.expect("Failed to join tags and tagged_items");

	// Assert
	let tag_name: String = row.get("tag_name");
	let content_type: String = row.get("content_type");
	let object_id: i64 = row.get("object_id");
	assert_eq!(tag_name, "rust");
	assert_eq!(content_type, "Food");
	assert_eq!(object_id, 42);
}

/// Test multi-object tagging with same tag
///
/// This test verifies that multiple objects can share the same tag.
#[rstest]
#[tokio::test]
async fn test_multi_object_tagging(
	#[future] taggit_db: (
		ContainerAsync<GenericImage>,
		Arc<PgPool>,
		DatabaseConnection,
	),
) {
	// Arrange
	let (_container, pool, _db) = taggit_db.await;
	let tag_id = insert_tag_to_db(&pool, "healthy", "healthy").await;

	// Act - tag multiple objects with the same tag
	insert_tagged_item_to_db(&pool, tag_id, "Food", 1).await;
	insert_tagged_item_to_db(&pool, tag_id, "Food", 2).await;
	insert_tagged_item_to_db(&pool, tag_id, "Food", 3).await;

	// Assert
	let select_sql = Query::select()
		.column(Asterisk)
		.from(Alias::new("tagged_items"))
		.and_where(Expr::col(Alias::new("tag_id")).eq(tag_id))
		.to_string(PostgresQueryBuilder);

	let rows = sqlx::query(&select_sql)
		.fetch_all(pool.as_ref())
		.await
		.expect("Failed to fetch tagged items");

	assert_eq!(rows.len(), 3);
}

/// Test multi-tag per object
///
/// This test verifies that a single object can have multiple tags.
#[rstest]
#[tokio::test]
async fn test_multi_tag_per_object(
	#[future] taggit_db: (
		ContainerAsync<GenericImage>,
		Arc<PgPool>,
		DatabaseConnection,
	),
) {
	// Arrange
	let (_container, pool, _db) = taggit_db.await;
	let tag1_id = insert_tag_to_db(&pool, "healthy", "healthy").await;
	let tag2_id = insert_tag_to_db(&pool, "organic", "organic").await;
	let tag3_id = insert_tag_to_db(&pool, "vegan", "vegan").await;

	// Act - tag the same object with multiple tags
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
		.expect("Failed to fetch tags for object");

	assert_eq!(rows.len(), 3);
}

/// Test polymorphic content type tagging
///
/// This test verifies that different content types can use the same tag.
#[rstest]
#[tokio::test]
async fn test_polymorphic_content_type_tagging(
	#[future] taggit_db: (
		ContainerAsync<GenericImage>,
		Arc<PgPool>,
		DatabaseConnection,
	),
) {
	// Arrange
	let (_container, pool, _db) = taggit_db.await;
	let tag_id = insert_tag_to_db(&pool, "featured", "featured").await;

	// Act - tag objects of different content types
	insert_tagged_item_to_db(&pool, tag_id, "Food", 1).await;
	insert_tagged_item_to_db(&pool, tag_id, "Recipe", 1).await;
	insert_tagged_item_to_db(&pool, tag_id, "Article", 1).await;

	// Assert - query by tag_id returns all content types
	let select_sql = Query::select()
		.column(Alias::new("content_type"))
		.from(Alias::new("tagged_items"))
		.and_where(Expr::col(Alias::new("tag_id")).eq(tag_id))
		.order_by(Alias::new("content_type"), sea_query::Order::Asc)
		.to_string(PostgresQueryBuilder);

	let rows = sqlx::query(&select_sql)
		.fetch_all(pool.as_ref())
		.await
		.expect("Failed to fetch tagged items");

	assert_eq!(rows.len(), 3);
	let ct0: String = rows[0].get("content_type");
	let ct1: String = rows[1].get("content_type");
	let ct2: String = rows[2].get("content_type");
	assert_eq!(ct0, "Article");
	assert_eq!(ct1, "Food");
	assert_eq!(ct2, "Recipe");
}

/// Test query tagged items by tag
///
/// This test verifies querying TaggedItems by their associated Tag.
#[rstest]
#[tokio::test]
async fn test_query_tagged_items_by_tag(
	#[future] taggit_db: (
		ContainerAsync<GenericImage>,
		Arc<PgPool>,
		DatabaseConnection,
	),
) {
	// Arrange
	let (_container, pool, _db) = taggit_db.await;
	let rust_id = insert_tag_to_db(&pool, "rust", "rust").await;
	let python_id = insert_tag_to_db(&pool, "python", "python").await;
	insert_tagged_item_to_db(&pool, rust_id, "Food", 1).await;
	insert_tagged_item_to_db(&pool, rust_id, "Food", 2).await;
	insert_tagged_item_to_db(&pool, python_id, "Food", 3).await;

	// Act - query by tag name via JOIN
	let select_sql = Query::select()
		.expr_as(
			Expr::col((Alias::new("ti"), Alias::new("object_id"))),
			Alias::new("object_id"),
		)
		.from_as(Alias::new("tagged_items"), Alias::new("ti"))
		.join_as(
			JoinType::InnerJoin,
			Alias::new("tags"),
			Alias::new("t"),
			Expr::col((Alias::new("ti"), Alias::new("tag_id")))
				.equals((Alias::new("t"), Alias::new("id"))),
		)
		.and_where(Expr::col((Alias::new("t"), Alias::new("name"))).eq("rust"))
		.to_string(PostgresQueryBuilder);

	let rows = sqlx::query(&select_sql)
		.fetch_all(pool.as_ref())
		.await
		.expect("Failed to query tagged items by tag");

	// Assert
	assert_eq!(rows.len(), 2);
}

/// Test query tags by content type
///
/// This test verifies querying Tags used by a specific content type.
#[rstest]
#[tokio::test]
async fn test_query_tags_by_content_type(
	#[future] taggit_db: (
		ContainerAsync<GenericImage>,
		Arc<PgPool>,
		DatabaseConnection,
	),
) {
	// Arrange
	let (_container, pool, _db) = taggit_db.await;
	let tag1_id = insert_tag_to_db(&pool, "healthy", "healthy").await;
	let tag2_id = insert_tag_to_db(&pool, "organic", "organic").await;
	let tag3_id = insert_tag_to_db(&pool, "spicy", "spicy").await;
	insert_tagged_item_to_db(&pool, tag1_id, "Food", 1).await;
	insert_tagged_item_to_db(&pool, tag2_id, "Food", 2).await;
	insert_tagged_item_to_db(&pool, tag3_id, "Recipe", 1).await;

	// Act - query distinct tags used by "Food" content type
	let select_sql = Query::select()
		.distinct()
		.expr_as(
			Expr::col((Alias::new("t"), Alias::new("name"))),
			Alias::new("tag_name"),
		)
		.from_as(Alias::new("tagged_items"), Alias::new("ti"))
		.join_as(
			JoinType::InnerJoin,
			Alias::new("tags"),
			Alias::new("t"),
			Expr::col((Alias::new("ti"), Alias::new("tag_id")))
				.equals((Alias::new("t"), Alias::new("id"))),
		)
		.and_where(Expr::col((Alias::new("ti"), Alias::new("content_type"))).eq("Food"))
		.to_string(PostgresQueryBuilder);

	let rows = sqlx::query(&select_sql)
		.fetch_all(pool.as_ref())
		.await
		.expect("Failed to query tags by content type");

	// Assert - only 2 tags are used by "Food"
	assert_eq!(rows.len(), 2);
}

/// Test query tags by object
///
/// This test verifies querying Tags associated with a specific object.
#[rstest]
#[tokio::test]
async fn test_query_tags_by_object(
	#[future] taggit_db: (
		ContainerAsync<GenericImage>,
		Arc<PgPool>,
		DatabaseConnection,
	),
) {
	// Arrange
	let (_container, pool, _db) = taggit_db.await;
	let tag1_id = insert_tag_to_db(&pool, "healthy", "healthy").await;
	let tag2_id = insert_tag_to_db(&pool, "organic", "organic").await;
	let tag3_id = insert_tag_to_db(&pool, "spicy", "spicy").await;
	insert_tagged_item_to_db(&pool, tag1_id, "Food", 42).await;
	insert_tagged_item_to_db(&pool, tag2_id, "Food", 42).await;
	insert_tagged_item_to_db(&pool, tag3_id, "Food", 99).await;

	// Act - query tags for a specific object (Food:42)
	let select_sql = Query::select()
		.expr_as(
			Expr::col((Alias::new("t"), Alias::new("name"))),
			Alias::new("tag_name"),
		)
		.from_as(Alias::new("tagged_items"), Alias::new("ti"))
		.join_as(
			JoinType::InnerJoin,
			Alias::new("tags"),
			Alias::new("t"),
			Expr::col((Alias::new("ti"), Alias::new("tag_id")))
				.equals((Alias::new("t"), Alias::new("id"))),
		)
		.and_where(Expr::col((Alias::new("ti"), Alias::new("content_type"))).eq("Food"))
		.and_where(Expr::col((Alias::new("ti"), Alias::new("object_id"))).eq(42i64))
		.to_string(PostgresQueryBuilder);

	let rows = sqlx::query(&select_sql)
		.fetch_all(pool.as_ref())
		.await
		.expect("Failed to query tags by object");

	// Assert - Food:42 has 2 tags
	assert_eq!(rows.len(), 2);
}

/// Test complex tag filtering
///
/// This test verifies complex filtering scenarios with multiple tags.
#[rstest]
#[tokio::test]
async fn test_complex_tag_filtering(
	#[future] taggit_db: (
		ContainerAsync<GenericImage>,
		Arc<PgPool>,
		DatabaseConnection,
	),
) {
	// Arrange
	let (_container, pool, _db) = taggit_db.await;
	let rust_id = insert_tag_to_db(&pool, "rust", "rust").await;
	let python_id = insert_tag_to_db(&pool, "python", "python").await;
	let web_id = insert_tag_to_db(&pool, "web", "web").await;

	// Food items
	insert_tagged_item_to_db(&pool, rust_id, "Food", 1).await;
	insert_tagged_item_to_db(&pool, python_id, "Food", 2).await;
	insert_tagged_item_to_db(&pool, web_id, "Food", 3).await;
	// Recipe items
	insert_tagged_item_to_db(&pool, rust_id, "Recipe", 10).await;
	insert_tagged_item_to_db(&pool, web_id, "Recipe", 11).await;

	// Act - find all objects tagged with "rust" OR "web" in "Food" content type
	let select_sql = Query::select()
		.column(Alias::new("object_id"))
		.from(Alias::new("tagged_items"))
		.and_where(Expr::col(Alias::new("content_type")).eq("Food"))
		.and_where(Expr::col(Alias::new("tag_id")).is_in([rust_id, web_id]))
		.to_string(PostgresQueryBuilder);

	let rows = sqlx::query(&select_sql)
		.fetch_all(pool.as_ref())
		.await
		.expect("Failed to execute complex filter");

	// Assert - Food items tagged with "rust" or "web"
	assert_eq!(rows.len(), 2);
}

/// Test tag count per object
///
/// This test verifies counting tags per object.
#[rstest]
#[tokio::test]
async fn test_tag_count_per_object(
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
	let tag3_id = insert_tag_to_db(&pool, "web", "web").await;

	// Object 1 has 3 tags, Object 2 has 1 tag
	insert_tagged_item_to_db(&pool, tag1_id, "Food", 1).await;
	insert_tagged_item_to_db(&pool, tag2_id, "Food", 1).await;
	insert_tagged_item_to_db(&pool, tag3_id, "Food", 1).await;
	insert_tagged_item_to_db(&pool, tag1_id, "Food", 2).await;

	// Act - count tags per object
	let select_sql = Query::select()
		.column(Alias::new("object_id"))
		.expr_as(
			Func::count(Expr::col(Alias::new("id"))),
			Alias::new("tag_count"),
		)
		.from(Alias::new("tagged_items"))
		.and_where(Expr::col(Alias::new("content_type")).eq("Food"))
		.group_by_col(Alias::new("object_id"))
		.order_by(Alias::new("object_id"), sea_query::Order::Asc)
		.to_string(PostgresQueryBuilder);

	let rows = sqlx::query(&select_sql)
		.fetch_all(pool.as_ref())
		.await
		.expect("Failed to count tags per object");

	// Assert
	assert_eq!(rows.len(), 2);
	let obj1_count: i64 = rows[0].get("tag_count");
	let obj2_count: i64 = rows[1].get("tag_count");
	assert_eq!(obj1_count, 3);
	assert_eq!(obj2_count, 1);
}

/// Test popular tags query
///
/// This test verifies querying most used tags across all objects.
#[rstest]
#[tokio::test]
async fn test_popular_tags_query(
	#[future] taggit_db: (
		ContainerAsync<GenericImage>,
		Arc<PgPool>,
		DatabaseConnection,
	),
) {
	// Arrange
	let (_container, pool, _db) = taggit_db.await;
	let rust_id = insert_tag_to_db(&pool, "rust", "rust").await;
	let python_id = insert_tag_to_db(&pool, "python", "python").await;
	let web_id = insert_tag_to_db(&pool, "web", "web").await;

	// "rust" used 3 times, "python" used 2 times, "web" used 1 time
	insert_tagged_item_to_db(&pool, rust_id, "Food", 1).await;
	insert_tagged_item_to_db(&pool, rust_id, "Food", 2).await;
	insert_tagged_item_to_db(&pool, rust_id, "Recipe", 3).await;
	insert_tagged_item_to_db(&pool, python_id, "Food", 4).await;
	insert_tagged_item_to_db(&pool, python_id, "Recipe", 5).await;
	insert_tagged_item_to_db(&pool, web_id, "Food", 6).await;

	// Act - query most popular tags (ordered by usage count desc)
	let select_sql = Query::select()
		.expr_as(
			Expr::col((Alias::new("t"), Alias::new("name"))),
			Alias::new("tag_name"),
		)
		.expr_as(
			Func::count(Expr::col((Alias::new("ti"), Alias::new("id")))),
			Alias::new("usage_count"),
		)
		.from_as(Alias::new("tagged_items"), Alias::new("ti"))
		.join_as(
			JoinType::InnerJoin,
			Alias::new("tags"),
			Alias::new("t"),
			Expr::col((Alias::new("ti"), Alias::new("tag_id")))
				.equals((Alias::new("t"), Alias::new("id"))),
		)
		.group_by_col((Alias::new("t"), Alias::new("name")))
		.order_by(Alias::new("usage_count"), sea_query::Order::Desc)
		.to_string(PostgresQueryBuilder);

	let rows = sqlx::query(&select_sql)
		.fetch_all(pool.as_ref())
		.await
		.expect("Failed to query popular tags");

	// Assert - ordered by popularity
	assert_eq!(rows.len(), 3);

	let tag1_name: String = rows[0].get("tag_name");
	let tag1_count: i64 = rows[0].get("usage_count");
	let tag2_name: String = rows[1].get("tag_name");
	let tag2_count: i64 = rows[1].get("usage_count");
	let tag3_name: String = rows[2].get("tag_name");
	let tag3_count: i64 = rows[2].get("usage_count");

	assert_eq!(tag1_name, "rust");
	assert_eq!(tag1_count, 3);
	assert_eq!(tag2_name, "python");
	assert_eq!(tag2_count, 2);
	assert_eq!(tag3_name, "web");
	assert_eq!(tag3_count, 1);
}
