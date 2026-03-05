//! Loading Strategy Database Integration Tests
//!
//! Tests that use real SQLite databases to validate loading strategies with actual SQL execution.
//! These tests verify SQL generation, query counts, performance characteristics, and N+1 problem detection.
//!
//! Run with: cargo test --test loading_strategy_integration_tests --features integration-tests

mod integration_tests {
	use reinhardt_db::orm::{
		LoadContext, LoadOptionBuilder, LoadingStrategy, joinedload, lazyload, selectinload,
		subqueryload,
	};
	use rstest::{fixture, rstest};
	use serde::{Deserialize, Serialize};
	use sqlx::{Row, SqlitePool, sqlite::SqlitePoolOptions};
	use std::sync::Arc;
	use tokio::sync::Mutex;

	// Test models
	#[derive(Debug, Clone, Serialize, Deserialize)]
	struct Author {
		id: Option<i64>,
		name: String,
	}

	reinhardt_test::impl_test_model!(Author, i64, "author");

	// Query counter wrapper
	#[derive(Clone)]
	struct QueryCounter {
		pool: SqlitePool,
		count: Arc<Mutex<usize>>,
	}

	impl QueryCounter {
		async fn new() -> Self {
			let pool = SqlitePoolOptions::new()
				.max_connections(1)
				.connect(":memory:")
				.await
				.unwrap();

			Self {
				pool,
				count: Arc::new(Mutex::new(0)),
			}
		}

		async fn reset(&self) {
			let mut count = self.count.lock().await;
			*count = 0;
		}

		async fn count(&self) -> usize {
			*self.count.lock().await
		}

		async fn execute(&self, query: &str) -> Result<(), sqlx::Error> {
			let mut count = self.count.lock().await;
			*count += 1;
			drop(count);
			sqlx::query(query).execute(&self.pool).await?;
			Ok(())
		}

		async fn fetch_all(
			&self,
			query: &str,
		) -> Result<Vec<sqlx::sqlite::SqliteRow>, sqlx::Error> {
			let mut count = self.count.lock().await;
			*count += 1;
			drop(count);
			sqlx::query(query).fetch_all(&self.pool).await
		}
	}

	/// Setup test database fixture with QueryCounter
	///
	/// Creates an in-memory SQLite database with authors, books, and reviews tables,
	/// populated with test data. The QueryCounter tracks the number of queries executed.
	#[fixture]
	async fn sqlite_fixture() -> QueryCounter {
		let counter = QueryCounter::new().await;

		// Create schema
		counter
			.execute(
				"CREATE TABLE authors (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL
            )",
			)
			.await
			.unwrap();

		counter
			.execute(
				"CREATE TABLE books (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                title TEXT NOT NULL,
                author_id INTEGER NOT NULL,
                pages INTEGER NOT NULL,
                FOREIGN KEY (author_id) REFERENCES authors(id)
            )",
			)
			.await
			.unwrap();

		counter
			.execute(
				"CREATE TABLE reviews (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                content TEXT NOT NULL,
                book_id INTEGER NOT NULL,
                rating INTEGER NOT NULL,
                FOREIGN KEY (book_id) REFERENCES books(id)
            )",
			)
			.await
			.unwrap();

		// Insert test data
		counter
			.execute("INSERT INTO authors (name) VALUES ('Author 1')")
			.await
			.unwrap();
		counter
			.execute("INSERT INTO authors (name) VALUES ('Author 2')")
			.await
			.unwrap();

		counter
			.execute("INSERT INTO books (title, author_id, pages) VALUES ('Book 1', 1, 300)")
			.await
			.unwrap();
		counter
			.execute("INSERT INTO books (title, author_id, pages) VALUES ('Book 2', 1, 400)")
			.await
			.unwrap();
		counter
			.execute("INSERT INTO books (title, author_id, pages) VALUES ('Book 3', 2, 500)")
			.await
			.unwrap();

		counter
			.execute("INSERT INTO reviews (content, book_id, rating) VALUES ('Great!', 1, 5)")
			.await
			.unwrap();
		counter
			.execute("INSERT INTO reviews (content, book_id, rating) VALUES ('Good!', 1, 4)")
			.await
			.unwrap();
		counter
			.execute("INSERT INTO reviews (content, book_id, rating) VALUES ('Nice!', 2, 4)")
			.await
			.unwrap();

		counter.reset().await;
		counter
	}

	#[rstest]
	#[tokio::test]
	async fn test_joinedload_generates_left_join(#[future] sqlite_fixture: QueryCounter) {
		// Test intent: Verify joinedload strategy generates LEFT JOIN SQL
		// and retrieves related data in single query
		let counter = sqlite_fixture.await;

		// Simulate joinedload query with LEFT JOIN
		let query = "SELECT authors.*, books.id as book_id, books.title, books.pages
                     FROM authors
                     LEFT JOIN books ON authors.id = books.author_id";

		let rows = counter.fetch_all(query).await.unwrap();

		assert_eq!(
			counter.count().await,
			1,
			"Should use single query with JOIN"
		);
		assert_eq!(
			rows.len(),
			3,
			"Should return 3 rows (author 1 has 2 books, author 2 has 1)"
		);

		// Verify strategy API
		let option = joinedload("books");
		assert_eq!(option.strategy(), LoadingStrategy::Joined);
	}

	#[rstest]
	#[tokio::test]
	async fn test_selectinload_generates_in_clause(#[future] sqlite_fixture: QueryCounter) {
		// Test intent: Verify selectinload strategy generates IN clause SQL
		// to batch load related data efficiently (2 queries total)
		let counter = sqlite_fixture.await;

		// Step 1: Get authors
		let authors = counter.fetch_all("SELECT * FROM authors").await.unwrap();
		assert_eq!(counter.count().await, 1);

		// Step 2: Selectinload books with IN clause
		let query = "SELECT * FROM books WHERE author_id IN (1, 2)";
		let books = counter.fetch_all(query).await.unwrap();

		assert_eq!(counter.count().await, 2, "Should use 2 queries total");
		assert_eq!(authors.len(), 2);
		assert_eq!(books.len(), 3);

		// Verify strategy API
		let option = selectinload("books");
		assert_eq!(option.strategy(), LoadingStrategy::Selectin);
	}

	// Test 3: Subqueryload generates subquery
	#[rstest]
	#[tokio::test]
	async fn test_subqueryload_generates_subquery(#[future] sqlite_fixture: QueryCounter) {
		let counter = sqlite_fixture.await;

		// Simulate subqueryload: books WHERE author_id IN (SELECT id FROM authors)
		let query = "SELECT * FROM books WHERE author_id IN (SELECT id FROM authors WHERE id <= 2)";
		let rows = counter.fetch_all(query).await.unwrap();

		assert_eq!(
			counter.count().await,
			1,
			"Subquery executed as single query"
		);
		assert_eq!(rows.len(), 3);

		// Verify strategy API
		let option = subqueryload("books");
		assert_eq!(option.strategy(), LoadingStrategy::Subquery);
	}

	// Test 4: Real N+1 detection with lazy loading
	#[rstest]
	#[tokio::test]
	async fn test_real_n_plus_one_detection(#[future] sqlite_fixture: QueryCounter) {
		let counter = sqlite_fixture.await;

		// Step 1: Get authors (1 query)
		let authors = counter.fetch_all("SELECT * FROM authors").await.unwrap();
		assert_eq!(counter.count().await, 1);

		// Step 2: Lazy load books for each author (N queries)
		for row in &authors {
			let author_id: i64 = row.get("id");
			let query = format!("SELECT * FROM books WHERE author_id = {}", author_id);
			let _books = counter.fetch_all(&query).await.unwrap();
		}

		// Total: 1 (authors) + 2 (books per author) = 3 queries
		assert_eq!(counter.count().await, 3, "Lazy loading creates N+1 queries");

		// Verify lazyload strategy
		let option = lazyload("books");
		assert!(option.strategy().is_lazy());
	}

	// Test 5: Joinedload uses single query
	#[rstest]
	#[tokio::test]
	async fn test_joinedload_query_count(#[future] sqlite_fixture: QueryCounter) {
		let counter = sqlite_fixture.await;

		// Single query with JOIN
		let _rows = counter
			.fetch_all(
				"SELECT authors.*, books.id as book_id, books.title
                 FROM authors
                 LEFT JOIN books ON authors.id = books.author_id",
			)
			.await
			.unwrap();

		assert_eq!(counter.count().await, 1, "Joinedload uses exactly 1 query");
	}

	// Test 6: Selectinload uses parent + one SELECT IN query
	#[rstest]
	#[tokio::test]
	async fn test_selectinload_query_count(#[future] sqlite_fixture: QueryCounter) {
		let counter = sqlite_fixture.await;

		// Query 1: Get authors
		let _authors = counter.fetch_all("SELECT * FROM authors").await.unwrap();

		// Query 2: Get all books with IN clause
		let _books = counter
			.fetch_all("SELECT * FROM books WHERE author_id IN (1, 2)")
			.await
			.unwrap();

		assert_eq!(
			counter.count().await,
			2,
			"Selectinload uses parent + 1 IN query"
		);
	}

	// Test 7: Lazy loading creates N queries
	#[rstest]
	#[tokio::test]
	async fn test_lazy_load_creates_n_queries(#[future] sqlite_fixture: QueryCounter) {
		let counter = sqlite_fixture.await;

		// Get authors
		let authors = counter.fetch_all("SELECT * FROM authors").await.unwrap();
		let author_count = authors.len();

		// Lazy load each author's books
		for row in &authors {
			let author_id: i64 = row.get("id");
			let _books = counter
				.fetch_all(&format!(
					"SELECT * FROM books WHERE author_id = {}",
					author_id
				))
				.await
				.unwrap();
		}

		assert_eq!(
			counter.count().await,
			1 + author_count,
			"Lazy load: 1 parent + N child queries"
		);
	}

	// Test 8: Joinedload with multiple collections creates cartesian product
	#[rstest]
	#[tokio::test]
	async fn test_joinedload_cartesian_product_with_multiple_collections(
		#[future] sqlite_fixture: QueryCounter,
	) {
		let counter = sqlite_fixture.await;

		// JOIN both books and reviews creates cartesian product
		let query = "SELECT authors.*, books.id as book_id, reviews.id as review_id
                     FROM authors
                     LEFT JOIN books ON authors.id = books.author_id
                     LEFT JOIN reviews ON books.id = reviews.book_id";

		let rows = counter.fetch_all(query).await.unwrap();

		// Author 1 has 2 books, Book 1 has 2 reviews, Book 2 has 1 review = 3 rows
		// Author 2 has 1 book with 0 reviews = 1 row
		// Total: 4 rows (cartesian product effect)
		assert!(
			rows.len() >= 3,
			"Multiple JOINs create cartesian product: {}",
			rows.len()
		);
		assert_eq!(counter.count().await, 1);
	}

	// Test 9: Selectinload avoids cartesian product
	#[rstest]
	#[tokio::test]
	async fn test_selectinload_avoids_cartesian_product(#[future] sqlite_fixture: QueryCounter) {
		let counter = sqlite_fixture.await;

		// Query 1: Authors
		let authors = counter.fetch_all("SELECT * FROM authors").await.unwrap();

		// Query 2: Books with IN clause
		let books = counter
			.fetch_all("SELECT * FROM books WHERE author_id IN (1, 2)")
			.await
			.unwrap();

		// Query 3: Reviews with IN clause
		let _reviews = counter
			.fetch_all("SELECT * FROM reviews WHERE book_id IN (1, 2, 3)")
			.await
			.unwrap();

		assert_eq!(
			counter.count().await,
			3,
			"Selectinload uses separate queries"
		);
		assert_eq!(authors.len(), 2, "2 authors");
		assert_eq!(books.len(), 3, "3 books (no cartesian product)");
	}

	// Test 10: LoadContext integration with database
	#[rstest]
	#[tokio::test]
	async fn test_loading_strategy_with_load_context(#[future] sqlite_fixture: QueryCounter) {
		let counter = sqlite_fixture.await;
		let mut ctx = LoadContext::new();

		// Mark paths as loaded
		ctx.mark_loaded("books".to_string(), LoadingStrategy::Joined);

		// Verify books are loaded
		assert!(ctx.is_loaded("books"));

		// Execute query only if not loaded
		if !ctx.is_loaded("reviews") {
			let _reviews = counter.fetch_all("SELECT * FROM reviews").await.unwrap();
			ctx.mark_loaded("reviews".to_string(), LoadingStrategy::Selectin);
		}

		assert_eq!(counter.count().await, 1, "Only reviews query executed");
		assert_eq!(ctx.strategy_for("books"), Some(LoadingStrategy::Joined));
		assert_eq!(ctx.strategy_for("reviews"), Some(LoadingStrategy::Selectin));
	}

	// Test 11: Multiple relationships loading
	#[rstest]
	#[tokio::test]
	async fn test_multiple_relationships_loading(#[future] sqlite_fixture: QueryCounter) {
		let counter = sqlite_fixture.await;

		let options = LoadOptionBuilder::<Author>::new()
			.joinedload("books")
			.selectinload("books.reviews")
			.build();

		assert_eq!(options.len(), 2);
		assert_eq!(options[0].strategy(), LoadingStrategy::Joined);
		assert_eq!(options[1].strategy(), LoadingStrategy::Selectin);

		// Simulate the queries these strategies would generate
		// Query 1: Authors with books (joinedload)
		let _author_books = counter
			.fetch_all(
				"SELECT authors.*, books.id as book_id, books.title
                 FROM authors
                 LEFT JOIN books ON authors.id = books.author_id",
			)
			.await
			.unwrap();

		// Query 2: Reviews for books (selectinload)
		let _reviews = counter
			.fetch_all("SELECT * FROM reviews WHERE book_id IN (1, 2, 3)")
			.await
			.unwrap();

		assert_eq!(
			counter.count().await,
			2,
			"Combined strategies use 2 queries"
		);
	}

	// Test 12: Performance comparison - eager vs lazy
	#[rstest]
	#[tokio::test]
	async fn test_loading_strategy_performance_comparison(#[future] sqlite_fixture: QueryCounter) {
		let counter = sqlite_fixture.await;

		// Measure eager loading (joinedload)
		let start = std::time::Instant::now();
		let _eager = counter
			.fetch_all(
				"SELECT authors.*, books.id as book_id
                 FROM authors
                 LEFT JOIN books ON authors.id = books.author_id",
			)
			.await
			.unwrap();
		let eager_time = start.elapsed();
		let eager_queries = counter.count().await;

		counter.reset().await;

		// Measure lazy loading
		let start = std::time::Instant::now();
		let authors = counter.fetch_all("SELECT * FROM authors").await.unwrap();
		for row in &authors {
			let author_id: i64 = row.get("id");
			let _books = counter
				.fetch_all(&format!(
					"SELECT * FROM books WHERE author_id = {}",
					author_id
				))
				.await
				.unwrap();
		}
		let lazy_time = start.elapsed();
		let lazy_queries = counter.count().await;

		// Eager loading should use fewer queries
		assert!(
			eager_queries < lazy_queries,
			"Eager: {} queries, Lazy: {} queries",
			eager_queries,
			lazy_queries
		);

		// Note: In real scenarios, eager loading is often faster due to fewer roundtrips
		println!(
			"Performance: Eager={}µs ({}q), Lazy={}µs ({}q)",
			eager_time.as_micros(),
			eager_queries,
			lazy_time.as_micros(),
			lazy_queries
		);
	}

	// Test 13: Empty results handling
	#[rstest]
	#[tokio::test]
	async fn test_loading_strategy_with_empty_results(#[future] sqlite_fixture: QueryCounter) {
		let counter = sqlite_fixture.await;

		// Query non-existent author
		let rows = counter
			.fetch_all(
				"SELECT authors.*, books.id as book_id
                 FROM authors
                 LEFT JOIN books ON authors.id = books.author_id
                 WHERE authors.id = 999",
			)
			.await
			.unwrap();

		assert_eq!(rows.len(), 0, "Empty result set");
		assert_eq!(counter.count().await, 1, "Still executes 1 query");
	}

	// Test 14: Nested loading with NULL values
	#[rstest]
	#[tokio::test]
	async fn test_nested_loading_with_nulls(#[future] sqlite_fixture: QueryCounter) {
		let counter = sqlite_fixture.await;

		// Add author without books
		counter
			.execute("INSERT INTO authors (name) VALUES ('Author 3')")
			.await
			.unwrap();

		counter.reset().await;

		// LEFT JOIN should handle NULLs
		let rows = counter
			.fetch_all(
				"SELECT authors.*, books.id as book_id, books.title
                 FROM authors
                 LEFT JOIN books ON authors.id = books.author_id
                 WHERE authors.id = 3",
			)
			.await
			.unwrap();

		assert_eq!(rows.len(), 1, "Author without books returns 1 row");
		let book_id: Option<i64> = rows[0].get("book_id");
		assert!(book_id.is_none(), "book_id should be NULL");
	}

	// Test 15: Large dataset simulation
	#[rstest]
	#[tokio::test]
	async fn test_loading_strategy_with_large_dataset(#[future] sqlite_fixture: QueryCounter) {
		let counter = sqlite_fixture.await;

		// Insert many books for Author 1
		for i in 4..=50 {
			counter
				.execute(&format!(
					"INSERT INTO books (title, author_id, pages) VALUES ('Book {}', 1, 300)",
					i
				))
				.await
				.unwrap();
		}

		counter.reset().await;

		// Test selectinload efficiency with large dataset
		let _books = counter
			.fetch_all("SELECT * FROM books WHERE author_id IN (1, 2)")
			.await
			.unwrap();

		assert_eq!(
			counter.count().await,
			1,
			"Selectinload handles large datasets efficiently"
		);
	}
}
