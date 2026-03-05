//! Advanced ORM Integration Tests for Pagination
//!
//! Tests advanced pagination scenarios including:
//! - Out-of-bounds page handling
//! - Dynamic page size changes
//! - Page metadata accuracy across all boundary conditions
//!
//! **Fixtures Used:**
//! - postgres_container: PostgreSQL database container (reinhardt-test)
//! - paginate_test_db: Custom fixture providing database connection with test schema

use reinhardt_core::pagination::PageNumberPagination;
use reinhardt_test::fixtures::testcontainers::{ContainerAsync, GenericImage, postgres_container};
use rstest::*;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

// ============================================================================
// Test Models
// ============================================================================

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, sqlx::FromRow)]
struct Article {
	id: Option<i32>,
	title: String,
	content: String,
	author: String,
	published: bool,
}

reinhardt_test::impl_test_model!(Article, i32, "articles");

// ============================================================================
// Custom Fixtures
// ============================================================================

/// Custom fixture providing PostgreSQL database with pagination test schema
///
/// **Schema:**
/// - articles: id, title, content, author, published
///
/// **Integration Point**: postgres_container → paginate_test_db (fixture chaining)
#[fixture]
async fn paginate_test_db(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<sqlx::PgPool>, u16, String),
) -> (ContainerAsync<GenericImage>, Arc<sqlx::PgPool>) {
	let (container, pool, _port, _url) = postgres_container.await;

	// Create articles table
	sqlx::query(
		r#"
		CREATE TABLE articles (
			id SERIAL PRIMARY KEY,
			title TEXT NOT NULL,
			content TEXT NOT NULL,
			author TEXT NOT NULL,
			published BOOLEAN NOT NULL DEFAULT false
		)
		"#,
	)
	.execute(pool.as_ref())
	.await
	.expect("Failed to create articles table");

	(container, pool)
}

/// Helper function to seed test data
async fn seed_articles(pool: &Arc<sqlx::PgPool>, count: usize) {
	for i in 1..=count {
		sqlx::query(
			"INSERT INTO articles (title, content, author, published) VALUES ($1, $2, $3, $4)",
		)
		.bind(format!("Article {}", i))
		.bind(format!("Content for article {}", i))
		.bind("Test Author")
		.bind(true)
		.execute(pool.as_ref())
		.await
		.expect("Failed to insert test article");
	}
}

/// Helper function to fetch all articles ordered by ID
async fn fetch_all_articles(pool: &Arc<sqlx::PgPool>) -> Vec<Article> {
	sqlx::query_as::<_, Article>(
		"SELECT id, title, content, author, published FROM articles ORDER BY id",
	)
	.fetch_all(pool.as_ref())
	.await
	.expect("Failed to fetch articles")
}

// ============================================================================
// Advanced Pagination Tests
// ============================================================================

/// Test 1: Page out of bounds - Zero page number
///
/// Verifies that requesting page 0 returns the first page (lenient behavior)
#[rstest]
#[tokio::test]
async fn test_page_out_of_bounds_zero(
	#[future] paginate_test_db: (ContainerAsync<GenericImage>, Arc<sqlx::PgPool>),
) {
	let (_container, pool) = paginate_test_db.await;
	seed_articles(&pool, 25).await;

	let articles = fetch_all_articles(&pool).await;
	let paginator = PageNumberPagination::new().page_size(10);

	// Request page 0 - should return first page with lenient behavior
	let page = paginator.get_page(&articles, Some("0"));

	// Verify it returns first page
	assert_eq!(page.number, 1);
	assert_eq!(page.len(), 10);
	assert_eq!(page.get(0).unwrap().title, "Article 1");
	assert_eq!(page.start_index(), 1);
	assert_eq!(page.end_index(), 10);
	assert!(!page.has_previous());
	assert!(page.has_next());
}

/// Test 2: Page out of bounds - Negative page number (parsed as invalid)
///
/// Verifies that requesting a negative page returns the first page (lenient behavior)
#[rstest]
#[tokio::test]
async fn test_page_out_of_bounds_negative(
	#[future] paginate_test_db: (ContainerAsync<GenericImage>, Arc<sqlx::PgPool>),
) {
	let (_container, pool) = paginate_test_db.await;
	seed_articles(&pool, 25).await;

	let articles = fetch_all_articles(&pool).await;
	let paginator = PageNumberPagination::new().page_size(10);

	// Request negative page - should return first page
	let page = paginator.get_page(&articles, Some("-1"));

	// Verify it returns first page (lenient fallback)
	assert_eq!(page.number, 1);
	assert_eq!(page.len(), 10);
	assert_eq!(page.get(0).unwrap().title, "Article 1");
	assert!(!page.has_previous());
	assert!(page.has_next());
}

/// Test 3: Page out of bounds - Beyond max page
///
/// Verifies that requesting a page beyond the maximum returns the last page
#[rstest]
#[tokio::test]
async fn test_page_out_of_bounds_beyond_max(
	#[future] paginate_test_db: (ContainerAsync<GenericImage>, Arc<sqlx::PgPool>),
) {
	let (_container, pool) = paginate_test_db.await;
	seed_articles(&pool, 25).await;

	let articles = fetch_all_articles(&pool).await;
	let paginator = PageNumberPagination::new().page_size(10);

	// Request page 100 (way beyond last page 3) - should return last page
	let page = paginator.get_page(&articles, Some("100"));

	// Verify it returns last page
	assert_eq!(page.number, 3);
	assert_eq!(page.len(), 5); // Last page has 5 items
	assert_eq!(page.get(0).unwrap().title, "Article 21");
	assert_eq!(page.get(4).unwrap().title, "Article 25");
	assert_eq!(page.start_index(), 21);
	assert_eq!(page.end_index(), 25);
	assert!(page.has_previous());
	assert!(!page.has_next());
}

/// Test 4: Dynamic page size change - Same dataset, different page sizes
///
/// Verifies that changing page size correctly recalculates pagination
#[rstest]
#[tokio::test]
async fn test_dynamic_page_size_change(
	#[future] paginate_test_db: (ContainerAsync<GenericImage>, Arc<sqlx::PgPool>),
) {
	let (_container, pool) = paginate_test_db.await;
	seed_articles(&pool, 50).await;

	let articles = fetch_all_articles(&pool).await;

	// Scenario 1: Page size 10
	let paginator_10 = PageNumberPagination::new().page_size(10);
	let page_10 = paginator_10.get_page(&articles, Some("2"));

	assert_eq!(page_10.number, 2);
	assert_eq!(page_10.num_pages, 5); // 50 / 10 = 5 pages
	assert_eq!(page_10.len(), 10);
	assert_eq!(page_10.start_index(), 11);
	assert_eq!(page_10.end_index(), 20);
	assert_eq!(page_10.get(0).unwrap().title, "Article 11");

	// Scenario 2: Page size 20 - Same dataset
	let paginator_20 = PageNumberPagination::new().page_size(20);
	let page_20 = paginator_20.get_page(&articles, Some("2"));

	assert_eq!(page_20.number, 2);
	assert_eq!(page_20.num_pages, 3); // 50 / 20 = 2.5 → 3 pages
	assert_eq!(page_20.len(), 20);
	assert_eq!(page_20.start_index(), 21);
	assert_eq!(page_20.end_index(), 40);
	assert_eq!(page_20.get(0).unwrap().title, "Article 21");

	// Scenario 3: Page size 7 (uneven division)
	let paginator_7 = PageNumberPagination::new().page_size(7);
	let page_7 = paginator_7.get_page(&articles, Some("3"));

	assert_eq!(page_7.number, 3);
	assert_eq!(page_7.num_pages, 8); // 50 / 7 = 7.14 → 8 pages
	assert_eq!(page_7.len(), 7);
	assert_eq!(page_7.start_index(), 15);
	assert_eq!(page_7.end_index(), 21);
	assert_eq!(page_7.get(0).unwrap().title, "Article 15");

	// Scenario 4: Verify last page with page size 7
	let last_page = paginator_7.get_page(&articles, Some("8"));
	assert_eq!(last_page.len(), 1); // 50 % 7 = 1 item on last page
	assert_eq!(last_page.get(0).unwrap().title, "Article 50");
}

/// Test 5: Page metadata accuracy - First page boundary
///
/// Verifies all metadata fields are correct for the first page
#[rstest]
#[tokio::test]
async fn test_page_metadata_first_page(
	#[future] paginate_test_db: (ContainerAsync<GenericImage>, Arc<sqlx::PgPool>),
) {
	let (_container, pool) = paginate_test_db.await;
	seed_articles(&pool, 30).await;

	let articles = fetch_all_articles(&pool).await;
	let paginator = PageNumberPagination::new().page_size(10);

	// First page
	let page = paginator.get_page(&articles, Some("1"));

	// Verify metadata
	assert_eq!(page.number, 1);
	assert_eq!(page.num_pages, 3); // 30 / 10 = 3 pages
	assert_eq!(page.count, 30);
	assert_eq!(page.page_size, 10);
	assert_eq!(page.len(), 10);
	assert_eq!(page.start_index(), 1);
	assert_eq!(page.end_index(), 10);
	assert!(!page.has_previous());
	assert!(page.has_next());
	assert!(page.has_other_pages());

	// Verify next_page_number() succeeds
	assert_eq!(page.next_page_number().unwrap(), 2);

	// Verify previous_page_number() fails
	assert!(page.previous_page_number().is_err());
}

/// Test 6: Page metadata accuracy - Middle page boundary
///
/// Verifies all metadata fields are correct for a middle page
#[rstest]
#[tokio::test]
async fn test_page_metadata_middle_page(
	#[future] paginate_test_db: (ContainerAsync<GenericImage>, Arc<sqlx::PgPool>),
) {
	let (_container, pool) = paginate_test_db.await;
	seed_articles(&pool, 30).await;

	let articles = fetch_all_articles(&pool).await;
	let paginator = PageNumberPagination::new().page_size(10);

	// Middle page
	let page = paginator.get_page(&articles, Some("2"));

	// Verify metadata
	assert_eq!(page.number, 2);
	assert_eq!(page.num_pages, 3); // 30 / 10 = 3 pages
	assert_eq!(page.count, 30);
	assert_eq!(page.page_size, 10);
	assert_eq!(page.len(), 10);
	assert_eq!(page.start_index(), 11);
	assert_eq!(page.end_index(), 20);
	assert!(page.has_previous());
	assert!(page.has_next());
	assert!(page.has_other_pages());

	// Verify both next and previous page numbers
	assert_eq!(page.next_page_number().unwrap(), 3);
	assert_eq!(page.previous_page_number().unwrap(), 1);
}

/// Test 7: Page metadata accuracy - Last page boundary
///
/// Verifies all metadata fields are correct for the last page
#[rstest]
#[tokio::test]
async fn test_page_metadata_last_page(
	#[future] paginate_test_db: (ContainerAsync<GenericImage>, Arc<sqlx::PgPool>),
) {
	let (_container, pool) = paginate_test_db.await;
	seed_articles(&pool, 30).await;

	let articles = fetch_all_articles(&pool).await;
	let paginator = PageNumberPagination::new().page_size(10);

	// Last page
	let page = paginator.get_page(&articles, Some("3"));

	// Verify metadata
	assert_eq!(page.number, 3);
	assert_eq!(page.num_pages, 3); // 30 / 10 = 3 pages
	assert_eq!(page.count, 30);
	assert_eq!(page.page_size, 10);
	assert_eq!(page.len(), 10);
	assert_eq!(page.start_index(), 21);
	assert_eq!(page.end_index(), 30);
	assert!(page.has_previous());
	assert!(!page.has_next());
	assert!(page.has_other_pages());

	// Verify previous_page_number() succeeds
	assert_eq!(page.previous_page_number().unwrap(), 2);

	// Verify next_page_number() fails
	assert!(page.next_page_number().is_err());
}

/// Test 8: Page metadata accuracy - Last page with partial items
///
/// Verifies metadata is correct when the last page has fewer items than page_size
#[rstest]
#[tokio::test]
async fn test_page_metadata_last_page_partial(
	#[future] paginate_test_db: (ContainerAsync<GenericImage>, Arc<sqlx::PgPool>),
) {
	let (_container, pool) = paginate_test_db.await;
	seed_articles(&pool, 25).await; // 25 items with page_size 10 → last page has 5 items

	let articles = fetch_all_articles(&pool).await;
	let paginator = PageNumberPagination::new().page_size(10);

	// Last page (partial)
	let page = paginator.get_page(&articles, Some("3"));

	// Verify metadata
	assert_eq!(page.number, 3);
	assert_eq!(page.num_pages, 3); // 25 / 10 = 2.5 → 3 pages
	assert_eq!(page.count, 25);
	assert_eq!(page.page_size, 10);
	assert_eq!(page.len(), 5); // Only 5 items on last page
	assert_eq!(page.start_index(), 21);
	assert_eq!(page.end_index(), 25);
	assert!(page.has_previous());
	assert!(!page.has_next());
	assert!(page.has_other_pages());

	// Verify item content
	assert_eq!(page.get(0).unwrap().title, "Article 21");
	assert_eq!(page.get(4).unwrap().title, "Article 25");
}

/// Test 9: Page metadata accuracy - Single page dataset
///
/// Verifies metadata when all data fits in a single page
#[rstest]
#[tokio::test]
async fn test_page_metadata_single_page(
	#[future] paginate_test_db: (ContainerAsync<GenericImage>, Arc<sqlx::PgPool>),
) {
	let (_container, pool) = paginate_test_db.await;
	seed_articles(&pool, 5).await; // Only 5 items with page_size 10

	let articles = fetch_all_articles(&pool).await;
	let paginator = PageNumberPagination::new().page_size(10);

	// Single page
	let page = paginator.get_page(&articles, Some("1"));

	// Verify metadata
	assert_eq!(page.number, 1);
	assert_eq!(page.num_pages, 1); // All data fits in 1 page
	assert_eq!(page.count, 5);
	assert_eq!(page.page_size, 10);
	assert_eq!(page.len(), 5);
	assert_eq!(page.start_index(), 1);
	assert_eq!(page.end_index(), 5);
	assert!(!page.has_previous());
	assert!(!page.has_next());
	assert!(!page.has_other_pages());

	// Both methods should fail
	assert!(page.previous_page_number().is_err());
	assert!(page.next_page_number().is_err());
}

/// Test 10: Page metadata accuracy - Empty dataset
///
/// Verifies metadata when the dataset is empty
#[rstest]
#[tokio::test]
async fn test_page_metadata_empty_dataset(
	#[future] paginate_test_db: (ContainerAsync<GenericImage>, Arc<sqlx::PgPool>),
) {
	let (_container, pool) = paginate_test_db.await;
	// Don't seed any data

	let articles = fetch_all_articles(&pool).await;
	assert_eq!(articles.len(), 0);

	let paginator = PageNumberPagination::new().page_size(10);

	// Request first page from empty dataset
	let page = paginator.get_page(&articles, Some("1"));

	// Verify metadata for empty page
	assert_eq!(page.number, 1);
	assert_eq!(page.num_pages, 1); // Still 1 page even if empty (allow_empty_first_page=true)
	assert_eq!(page.count, 0);
	assert_eq!(page.page_size, 10);
	assert_eq!(page.len(), 0);
	assert_eq!(page.start_index(), 0); // Empty page has start_index 0
	assert_eq!(page.end_index(), 0); // Empty page has end_index 0
	assert!(!page.has_previous());
	assert!(!page.has_next());
	assert!(!page.has_other_pages());

	// Both methods should fail
	assert!(page.previous_page_number().is_err());
	assert!(page.next_page_number().is_err());
}

/// Test 11: Page metadata accuracy - Exactly page_size items
///
/// Verifies metadata when total items exactly equals page_size
#[rstest]
#[tokio::test]
async fn test_page_metadata_exact_page_size(
	#[future] paginate_test_db: (ContainerAsync<GenericImage>, Arc<sqlx::PgPool>),
) {
	let (_container, pool) = paginate_test_db.await;
	seed_articles(&pool, 10).await; // Exactly page_size items

	let articles = fetch_all_articles(&pool).await;
	let paginator = PageNumberPagination::new().page_size(10);

	// Single page with exactly page_size items
	let page = paginator.get_page(&articles, Some("1"));

	// Verify metadata
	assert_eq!(page.number, 1);
	assert_eq!(page.num_pages, 1); // All data fits exactly in 1 page
	assert_eq!(page.count, 10);
	assert_eq!(page.page_size, 10);
	assert_eq!(page.len(), 10);
	assert_eq!(page.start_index(), 1);
	assert_eq!(page.end_index(), 10);
	assert!(!page.has_previous());
	assert!(!page.has_next());
	assert!(!page.has_other_pages());
}
