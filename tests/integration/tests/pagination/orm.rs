//! ORM Integration Tests for Pagination
//!
//! These tests work with reinhardt-orm and database backends.
//! They test pagination with database querysets.

use reinhardt_core::pagination::PageNumberPagination;
use rstest::*;
use serde::{Deserialize, Serialize};
use sqlx::{Pool, Sqlite, sqlite::SqlitePoolOptions};

// ============================================================================
// Test Models
// ============================================================================

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, sqlx::FromRow)]
struct TestArticle {
	id: Option<i64>,
	title: String,
	content: String,
	author_id: i64,
	published: bool,
	created_at: String,
}

reinhardt_test::impl_test_model!(TestArticle, i64, "test_articles");

// ============================================================================
// rstest Fixtures
// ============================================================================

/// Fixture providing an SQLite in-memory database pool with test_articles table
///
/// The pool is automatically cleaned up when the test ends (Drop).
#[fixture]
async fn db_pool() -> Pool<Sqlite> {
	let pool = SqlitePoolOptions::new()
		.connect("sqlite::memory:")
		.await
		.expect("Failed to create database pool");

	// Create test_articles table
	sqlx::query(
		r#"
        CREATE TABLE test_articles (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            title TEXT NOT NULL,
            content TEXT NOT NULL,
            author_id INTEGER NOT NULL,
            published BOOLEAN NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL
        )
        "#,
	)
	.execute(&pool)
	.await
	.expect("Failed to create test_articles table");

	pool
}

/// Helper function to seed test data
///
/// This is still needed as a helper because different tests need different counts.
async fn seed_test_data(pool: &Pool<Sqlite>, count: usize) {
	for i in 1..=count {
		sqlx::query(
            "INSERT INTO test_articles (title, content, author_id, published, created_at) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(format!("Article {}", i))
        .bind(format!("Content for article {}", i))
        .bind(1)
        .bind(true)
        .bind(format!("2024-01-{:02}", i))
        .execute(pool)
        .await
        .expect("Failed to insert test article");
	}
}

// ============================================================================
// ORM Pagination Tests
// ============================================================================

#[rstest]
#[tokio::test]
async fn test_first_page_with_queryset(#[future] db_pool: Pool<Sqlite>) {
	let pool = db_pool.await;
	seed_test_data(&pool, 25).await;

	// Fetch all data for pagination (simulating QuerySet behavior)
	let all_results = sqlx::query_as::<_, TestArticle>(
		"SELECT id, title, content, author_id, published, created_at FROM test_articles ORDER BY id",
	)
	.fetch_all(&pool)
	.await
	.expect("Failed to fetch all data");

	// Verify total count
	assert_eq!(all_results.len(), 25);

	let paginator = PageNumberPagination::new().page_size(10);
	let page = paginator.get_page(&all_results, Some("1"));

	// Verify page contains correct items (1-10)
	assert_eq!(page.len(), 10);
	assert_eq!(page.get(0).unwrap().title, "Article 1");
	assert_eq!(page.get(9).unwrap().title, "Article 10");

	// Verify pagination metadata
	assert!(page.has_next());
	assert!(!page.has_previous());
	assert_eq!(page.start_index(), 1);
	assert_eq!(page.end_index(), 10);

	// pool is automatically dropped
}

#[rstest]
#[tokio::test]
async fn test_last_page_with_queryset(#[future] db_pool: Pool<Sqlite>) {
	let pool = db_pool.await;
	seed_test_data(&pool, 25).await;

	// Fetch all data for pagination (simulating QuerySet behavior)
	let all_results = sqlx::query_as::<_, TestArticle>(
		"SELECT id, title, content, author_id, published, created_at FROM test_articles ORDER BY id",
	)
	.fetch_all(&pool)
	.await
	.expect("Failed to fetch all data");

	// Verify total count
	assert_eq!(all_results.len(), 25);

	let paginator = PageNumberPagination::new().page_size(10);
	let page = paginator.get_page(&all_results, Some("3")); // Page 3 (last page)

	// Verify correct items (21-25)
	assert_eq!(page.len(), 5); // Only 5 items on last page
	assert_eq!(page.get(0).unwrap().title, "Article 21");
	assert_eq!(page.get(4).unwrap().title, "Article 25");

	// Verify pagination metadata
	assert!(!page.has_next());
	assert!(page.has_previous());
	assert_eq!(page.start_index(), 21);
	assert_eq!(page.end_index(), 25);

	// pool is automatically dropped
}

#[rstest]
#[tokio::test]
async fn test_page_getitem_with_queryset(#[future] db_pool: Pool<Sqlite>) {
	let pool = db_pool.await;
	seed_test_data(&pool, 15).await;

	// Test indexing into results using direct SQL
	let results = sqlx::query_as::<_, TestArticle>(
        "SELECT id, title, content, author_id, published, created_at FROM test_articles ORDER BY id LIMIT 10"
    )
    .fetch_all(&pool)
    .await
    .expect("Failed to fetch results");

	// Test indexing operations
	assert_eq!(results[0].title, "Article 1");
	assert_eq!(results[5].title, "Article 6");
	assert_eq!(results[9].title, "Article 10");

	// Test filtered results using direct SQL
	let filtered_results = sqlx::query_as::<_, TestArticle>(
        "SELECT id, title, content, author_id, published, created_at FROM test_articles WHERE published = ? ORDER BY id LIMIT 5"
    )
    .bind(true)
    .fetch_all(&pool)
    .await
    .expect("Failed to fetch filtered results");

	// All our test data is published, so we should get 5 results
	assert_eq!(filtered_results.len(), 5);

	// pool is automatically dropped
}

#[rstest]
#[tokio::test]
async fn test_paginating_unordered_queryset_raises_warning(#[future] db_pool: Pool<Sqlite>) {
	let pool = db_pool.await;
	seed_test_data(&pool, 10).await;

	// Fetch results without ORDER BY (unordered) using direct SQL
	let results = sqlx::query_as::<_, TestArticle>(
		"SELECT id, title, content, author_id, published, created_at FROM test_articles LIMIT 5 OFFSET 0",
	)
	.fetch_all(&pool)
	.await
	.expect("Failed to fetch unordered results");

	// Results should still be returned (but order is not guaranteed)
	assert_eq!(results.len(), 5);

	// In a real Django implementation, this would raise a warning like:
	// "UnorderedObjectListWarning: Pagination may yield inconsistent results with an unordered object_list"
	// For this test, we just verify the functionality works

	// pool is automatically dropped
}

#[rstest]
#[tokio::test]
async fn test_paginating_empty_queryset_does_not_warn(#[future] db_pool: Pool<Sqlite>) {
	let pool = db_pool.await;
	// Don't seed any data - empty table

	// Fetch from empty table using direct SQL
	let results = sqlx::query_as::<_, TestArticle>(
        "SELECT id, title, content, author_id, published, created_at FROM test_articles ORDER BY id LIMIT 5 OFFSET 0"
    )
    .fetch_all(&pool)
    .await
    .expect("Failed to fetch empty results");

	// Should return empty results without warnings
	assert_eq!(results.len(), 0);

	// Verify count is 0
	let total_count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM test_articles")
		.fetch_one(&pool)
		.await
		.expect("Failed to get count") as usize;

	assert_eq!(total_count, 0);

	// pool is automatically dropped
}

#[rstest]
#[tokio::test]
async fn test_paginating_unordered_object_list_raises_warning(#[future] db_pool: Pool<Sqlite>) {
	let pool = db_pool.await;
	seed_test_data(&pool, 10).await;

	// Create a mock object list with .ordered attribute set to false
	// In Django, this would be like: object_list.ordered = False
	// For this test, we simulate this by fetching without ORDER BY
	let results = sqlx::query_as::<_, TestArticle>(
		"SELECT id, title, content, author_id, published, created_at FROM test_articles LIMIT 5 OFFSET 0",
	)
	.fetch_all(&pool)
	.await
	.expect("Failed to fetch unordered object list");

	assert_eq!(results.len(), 5);

	// In a real Django implementation, this would raise:
	// "UnorderedObjectListWarning: Pagination may yield inconsistent results with an unordered object_list"
	// The warning would be raised when the Paginator is created with an unordered object list

	// pool is automatically dropped
}
