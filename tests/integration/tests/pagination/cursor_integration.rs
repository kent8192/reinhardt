//! Cursor-based Pagination Integration Tests
//!
//! These tests verify cursor-based pagination with PostgreSQL TestContainers,
//! testing cursor encoding/decoding, forward/backward navigation, and pagination metadata.

use reinhardt_core::pagination::{AsyncPaginator, CursorPagination};
use reinhardt_db::orm::DatabaseConnection;
use reinhardt_test::fixtures::testcontainers::postgres_container;
use rstest::*;
use serde::{Deserialize, Serialize};
use serial_test::serial;
use std::sync::Arc;
use testcontainers::GenericImage;
use url::Url;

// ============================================================================
// Test Models
// ============================================================================

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, sqlx::FromRow)]
struct TestArticle {
	id: i32,
	title: String,
	content: String,
	author_id: i32,
	published: bool,
	created_at: String,
}

// ============================================================================
// rstest Fixtures
// ============================================================================

/// Fixture providing PostgreSQL container with test_articles table and seeded data
///
/// Test intent: Provide a ready-to-use PostgreSQL database with test data
/// for cursor-based pagination integration testing.
#[fixture]
async fn setup_test_db(
	#[future] postgres_container: (
		testcontainers::ContainerAsync<GenericImage>,
		Arc<sqlx::PgPool>,
		u16,
		String,
	),
) -> (
	testcontainers::ContainerAsync<GenericImage>,
	Arc<sqlx::PgPool>,
	Vec<TestArticle>,
) {
	let (container, pool, _port, database_url) = postgres_container.await;

	// Create connection using DatabaseConnection
	let conn = DatabaseConnection::connect(&database_url)
		.await
		.expect("Failed to connect to database");

	// Create test_articles table
	conn.execute(
		"CREATE TABLE IF NOT EXISTS test_articles (
            id SERIAL PRIMARY KEY,
            title TEXT NOT NULL,
            content TEXT NOT NULL,
            author_id INTEGER NOT NULL,
            published BOOLEAN NOT NULL DEFAULT true,
            created_at TEXT NOT NULL
        )",
		vec![],
	)
	.await
	.expect("Failed to create test_articles table");

	// Seed test data (25 articles)
	let mut articles = Vec::new();
	for i in 1..=25 {
		let title = format!("Article {}", i);
		let content = format!("Content for article {}", i);
		let author_id = 1;
		let published = true;
		let created_at = format!("2024-01-{:02}", i);

		sqlx::query(
			"INSERT INTO test_articles (title, content, author_id, published, created_at)
             VALUES ($1, $2, $3, $4, $5)",
		)
		.bind(&title)
		.bind(&content)
		.bind(author_id)
		.bind(published)
		.bind(&created_at)
		.execute(&*pool)
		.await
		.expect("Failed to insert test article");

		articles.push(TestArticle {
			id: i,
			title,
			content,
			author_id,
			published,
			created_at,
		});
	}

	(container, pool, articles)
}

// ============================================================================
// Cursor Pagination Integration Tests
// ============================================================================

/// Test cursor-based pagination basic operation
///
/// Test intent: Verify that CursorPagination correctly paginates a dataset
/// with proper cursor encoding/decoding, next/previous links, and metadata.
#[rstest]
#[tokio::test]
#[serial(pagination_cursor)]
async fn test_cursor_based_pagination_integration(
	#[future] setup_test_db: (
		testcontainers::ContainerAsync<GenericImage>,
		Arc<sqlx::PgPool>,
		Vec<TestArticle>,
	),
) {
	let (_container, pool, expected_articles) = setup_test_db.await;

	// Fetch all articles from database
	let articles = sqlx::query_as::<_, TestArticle>(
		"SELECT id, title, content, author_id, published, created_at
         FROM test_articles
         ORDER BY id",
	)
	.fetch_all(&*pool)
	.await
	.expect("Failed to fetch articles");

	// Verify total count
	assert_eq!(articles.len(), 25);

	// Create cursor paginator with page_size=10
	let paginator = CursorPagination::new().page_size(10);

	// Get first page (cursor=None)
	let base_url = "http://localhost/api/articles";
	let first_page = paginator
		.apaginate(&articles, None, base_url)
		.await
		.expect("Failed to paginate first page");

	// Verify first page results (items 0-9)
	assert_eq!(first_page.results.len(), 10);
	assert_eq!(first_page.results[0].id, expected_articles[0].id);
	assert_eq!(first_page.results[0].title, "Article 1");
	assert_eq!(first_page.results[9].id, expected_articles[9].id);
	assert_eq!(first_page.results[9].title, "Article 10");

	// Verify first page metadata
	assert_eq!(first_page.count, 25); // Total count
	assert!(first_page.next.is_some()); // Has next page
	assert!(first_page.previous.is_none()); // No previous page (first page)

	// Extract next cursor from URL
	let next_url = first_page.next.as_ref().unwrap();
	let parsed_url = Url::parse(next_url).expect("Failed to parse next URL");
	let next_cursor = parsed_url
		.query_pairs()
		.find(|(key, _)| key == "cursor")
		.map(|(_, value)| value.to_string())
		.expect("Next cursor not found");

	// Get second page using next cursor
	let second_page = paginator
		.apaginate(&articles, Some(&next_cursor), base_url)
		.await
		.expect("Failed to paginate second page");

	// Verify second page results (items 10-19)
	assert_eq!(second_page.results.len(), 10);
	assert_eq!(second_page.results[0].id, expected_articles[10].id);
	assert_eq!(second_page.results[0].title, "Article 11");
	assert_eq!(second_page.results[9].id, expected_articles[19].id);
	assert_eq!(second_page.results[9].title, "Article 20");

	// Verify second page metadata
	assert_eq!(second_page.count, 25);
	assert!(second_page.next.is_some()); // Has next page (page 3)
	assert!(second_page.previous.is_none()); // No previous (bidirectional not enabled)

	// Get third page (last page, items 20-24)
	let third_cursor = Url::parse(second_page.next.as_ref().unwrap())
		.unwrap()
		.query_pairs()
		.find(|(key, _)| key == "cursor")
		.map(|(_, value)| value.to_string())
		.unwrap();

	let third_page = paginator
		.apaginate(&articles, Some(&third_cursor), base_url)
		.await
		.expect("Failed to paginate third page");

	// Verify third page results (items 20-24, only 5 items)
	assert_eq!(third_page.results.len(), 5);
	assert_eq!(third_page.results[0].id, expected_articles[20].id);
	assert_eq!(third_page.results[0].title, "Article 21");
	assert_eq!(third_page.results[4].id, expected_articles[24].id);
	assert_eq!(third_page.results[4].title, "Article 25");

	// Verify third page metadata (last page)
	assert_eq!(third_page.count, 25);
	assert!(third_page.next.is_none()); // No next page (last page)
	assert!(third_page.previous.is_none()); // No previous (bidirectional not enabled)

	// Container is automatically cleaned up when dropped
}

/// Test cursor-based pagination with forward and backward navigation
///
/// Test intent: Verify that CursorPagination with bidirectional mode
/// provides both next_cursor and previous_cursor for navigation.
#[rstest]
#[tokio::test]
#[serial(pagination_cursor)]
async fn test_cursor_forward_backward_navigation(
	#[future] setup_test_db: (
		testcontainers::ContainerAsync<GenericImage>,
		Arc<sqlx::PgPool>,
		Vec<TestArticle>,
	),
) {
	let (_container, pool, expected_articles) = setup_test_db.await;

	// Fetch all articles from database
	let articles = sqlx::query_as::<_, TestArticle>(
		"SELECT id, title, content, author_id, published, created_at
         FROM test_articles
         ORDER BY id",
	)
	.fetch_all(&*pool)
	.await
	.expect("Failed to fetch articles");

	// Create cursor paginator with bidirectional mode and page_size=10
	let paginator = CursorPagination::new().page_size(10).with_bidirectional();

	let base_url = "http://localhost/api/articles";

	// Get first page
	let first_page = paginator
		.apaginate(&articles, None, base_url)
		.await
		.expect("Failed to paginate first page");

	// Verify first page results
	assert_eq!(first_page.results.len(), 10);
	assert_eq!(first_page.results[0].title, "Article 1");
	assert_eq!(first_page.results[9].title, "Article 10");

	// Verify first page navigation
	assert!(first_page.next.is_some()); // Has next cursor
	assert!(first_page.previous.is_none()); // No previous cursor (first page)

	// Extract next cursor
	let next_cursor = Url::parse(first_page.next.as_ref().unwrap())
		.unwrap()
		.query_pairs()
		.find(|(key, _)| key == "cursor")
		.map(|(_, value)| value.to_string())
		.unwrap();

	// Navigate to second page
	let second_page = paginator
		.apaginate(&articles, Some(&next_cursor), base_url)
		.await
		.expect("Failed to paginate second page");

	// Verify second page results
	assert_eq!(second_page.results.len(), 10);
	assert_eq!(second_page.results[0].id, expected_articles[10].id);
	assert_eq!(second_page.results[0].title, "Article 11");
	assert_eq!(second_page.results[9].id, expected_articles[19].id);
	assert_eq!(second_page.results[9].title, "Article 20");

	// Verify second page navigation (bidirectional)
	assert!(second_page.next.is_some()); // Has next cursor
	assert!(second_page.previous.is_some()); // Has previous cursor (bidirectional enabled)

	// Extract previous cursor
	let prev_cursor = Url::parse(second_page.previous.as_ref().unwrap())
		.unwrap()
		.query_pairs()
		.find(|(key, _)| key == "cursor")
		.map(|(_, value)| value.to_string())
		.unwrap();

	// Navigate back to first page using previous cursor
	let back_to_first = paginator
		.apaginate(&articles, Some(&prev_cursor), base_url)
		.await
		.expect("Failed to navigate back to first page");

	// Verify we're back at the first page
	assert_eq!(back_to_first.results.len(), 10);
	assert_eq!(back_to_first.results[0].title, "Article 1");
	assert_eq!(back_to_first.results[9].title, "Article 10");

	// Verify navigation links
	assert!(back_to_first.next.is_some());
	assert!(back_to_first.previous.is_none()); // No previous (back at first page)

	// Navigate to third page (last page)
	let third_cursor = Url::parse(second_page.next.as_ref().unwrap())
		.unwrap()
		.query_pairs()
		.find(|(key, _)| key == "cursor")
		.map(|(_, value)| value.to_string())
		.unwrap();

	let third_page = paginator
		.apaginate(&articles, Some(&third_cursor), base_url)
		.await
		.expect("Failed to paginate third page");

	// Verify third page (last page)
	assert_eq!(third_page.results.len(), 5); // Only 5 items
	assert_eq!(third_page.results[0].title, "Article 21");
	assert_eq!(third_page.results[4].title, "Article 25");

	// Verify last page navigation
	assert!(third_page.next.is_none()); // No next (last page)
	assert!(third_page.previous.is_some()); // Has previous cursor

	// Navigate back to second page using previous cursor
	let back_to_second_cursor = Url::parse(third_page.previous.as_ref().unwrap())
		.unwrap()
		.query_pairs()
		.find(|(key, _)| key == "cursor")
		.map(|(_, value)| value.to_string())
		.unwrap();

	let back_to_second = paginator
		.apaginate(&articles, Some(&back_to_second_cursor), base_url)
		.await
		.expect("Failed to navigate back to second page");

	// Verify we're back at the second page
	assert_eq!(back_to_second.results.len(), 10);
	assert_eq!(back_to_second.results[0].title, "Article 11");
	assert_eq!(back_to_second.results[9].title, "Article 20");

	// Container is automatically cleaned up when dropped
}
