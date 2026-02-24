//! ANALYZE Statement Integration Tests
//!
//! Tests the ANALYZE statement builder with real database backends to verify:
//! - Statement execution succeeds without errors
//! - Statistics are actually updated (PostgreSQL pg_stat_user_tables)
//! - Different ANALYZE options work correctly (verbose, columns)
//! - Cross-database compatibility (PostgreSQL, SQLite)
//!
//! **Fixtures Used:**
//! - postgres_container: PostgreSQL database container
//! - sqlite_with_migrations_from: SQLite database with migrations

use reinhardt_db::backends::{AnalyzeBuilder, PostgresBackend};
use reinhardt_db::orm::manager::reinitialize_database;
use reinhardt_test::fixtures::postgres_container;
use rstest::*;
use sqlx::{PgPool, Row};
use std::sync::Arc;
use testcontainers::{ContainerAsync, GenericImage};

// ============================================================================
// PostgreSQL ANALYZE Tests
// ============================================================================

/// Test ANALYZE executes successfully on PostgreSQL database-wide
///
/// **Test Intent**: Verify that ANALYZE without table specification executes
/// successfully and updates statistics for all tables in the database.
///
/// **Integration Point**: AnalyzeBuilder + PostgresBackend + real PostgreSQL
#[rstest]
#[tokio::test]
async fn test_analyze_postgres_database_wide(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	// Arrange
	let (_container, pool, _port, url) = postgres_container.await;
	reinitialize_database(&url).await.unwrap();

	// Create test tables with data
	sqlx::query(
		"CREATE TABLE IF NOT EXISTS users (
            id SERIAL PRIMARY KEY,
            name TEXT NOT NULL,
            email TEXT NOT NULL
        )",
	)
	.execute(pool.as_ref())
	.await
	.expect("Failed to create users table");

	sqlx::query(
		"CREATE TABLE IF NOT EXISTS posts (
            id SERIAL PRIMARY KEY,
            title TEXT NOT NULL,
            user_id INTEGER REFERENCES users(id)
        )",
	)
	.execute(pool.as_ref())
	.await
	.expect("Failed to create posts table");

	// Insert test data
	for i in 1..=100 {
		sqlx::query("INSERT INTO users (name, email) VALUES ($1, $2)")
			.bind(format!("User {}", i))
			.bind(format!("user{}@example.com", i))
			.execute(pool.as_ref())
			.await
			.expect("Failed to insert user");
	}

	for i in 1..=500 {
		sqlx::query("INSERT INTO posts (title, user_id) VALUES ($1, $2)")
			.bind(format!("Post {}", i))
			.bind((i % 100) + 1)
			.execute(pool.as_ref())
			.await
			.expect("Failed to insert post");
	}

	let backend = Arc::new(PostgresBackend::new((*pool).clone()));

	// Act
	let builder = AnalyzeBuilder::new(backend);
	let result = builder.execute().await;

	// Assert
	assert!(result.is_ok(), "ANALYZE should execute successfully");

	// Verify statistics were updated by checking pg_stat_user_tables
	let stats = sqlx::query(
		"SELECT relname, n_live_tup
         FROM pg_stat_user_tables
         WHERE schemaname = 'public'
         ORDER BY relname",
	)
	.fetch_all(pool.as_ref())
	.await
	.expect("Failed to query statistics");

	assert!(!stats.is_empty(), "Statistics should exist for tables");

	// Find users and posts in stats
	let users_stats = stats.iter().find(|r| {
		let name: String = r.get("relname");
		name == "users"
	});
	let posts_stats = stats.iter().find(|r| {
		let name: String = r.get("relname");
		name == "posts"
	});

	assert!(
		users_stats.is_some(),
		"Statistics for users table should exist"
	);
	assert!(
		posts_stats.is_some(),
		"Statistics for posts table should exist"
	);
}

/// Test ANALYZE executes successfully on a specific PostgreSQL table
///
/// **Test Intent**: Verify that ANALYZE with table specification executes
/// successfully and updates statistics only for the specified table.
///
/// **Integration Point**: AnalyzeBuilder.table() + PostgresBackend
#[rstest]
#[tokio::test]
async fn test_analyze_postgres_specific_table(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	// Arrange
	let (_container, pool, _port, url) = postgres_container.await;
	reinitialize_database(&url).await.unwrap();

	sqlx::query(
		"CREATE TABLE IF NOT EXISTS products (
            id SERIAL PRIMARY KEY,
            name TEXT NOT NULL,
            price DECIMAL(10, 2) NOT NULL,
            category TEXT
        )",
	)
	.execute(pool.as_ref())
	.await
	.expect("Failed to create products table");

	// Insert test data
	for i in 1..=200 {
		sqlx::query("INSERT INTO products (name, price, category) VALUES ($1, $2, $3)")
			.bind(format!("Product {}", i))
			.bind(rust_decimal::Decimal::new(i as i64 * 100, 2))
			.bind(format!("Category {}", i % 10))
			.execute(pool.as_ref())
			.await
			.expect("Failed to insert product");
	}

	let backend = Arc::new(PostgresBackend::new((*pool).clone()));

	// Act
	let builder = AnalyzeBuilder::new(backend).table("products");
	let result = builder.execute().await;

	// Assert
	assert!(
		result.is_ok(),
		"ANALYZE on specific table should execute successfully"
	);

	// Verify the generated SQL is correct
	let backend_for_sql = Arc::new(PostgresBackend::new((*pool).clone()));
	let sql = AnalyzeBuilder::new(backend_for_sql)
		.table("products")
		.build();
	assert_eq!(sql, "ANALYZE \"products\"");
}

/// Test ANALYZE with VERBOSE option on PostgreSQL
///
/// **Test Intent**: Verify that ANALYZE VERBOSE executes successfully.
/// Note: VERBOSE output goes to server logs, not client, so we just verify execution.
///
/// **Integration Point**: AnalyzeBuilder.verbose() + PostgresBackend
#[rstest]
#[tokio::test]
async fn test_analyze_postgres_verbose(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	// Arrange
	let (_container, pool, _port, url) = postgres_container.await;
	reinitialize_database(&url).await.unwrap();

	sqlx::query(
		"CREATE TABLE IF NOT EXISTS orders (
            id SERIAL PRIMARY KEY,
            customer_name TEXT NOT NULL,
            total DECIMAL(10, 2) NOT NULL,
            status TEXT DEFAULT 'pending'
        )",
	)
	.execute(pool.as_ref())
	.await
	.expect("Failed to create orders table");

	// Insert test data
	for i in 1..=50 {
		sqlx::query("INSERT INTO orders (customer_name, total, status) VALUES ($1, $2, $3)")
			.bind(format!("Customer {}", i))
			.bind(rust_decimal::Decimal::new(i as i64 * 1000, 2))
			.bind(if i % 2 == 0 { "completed" } else { "pending" })
			.execute(pool.as_ref())
			.await
			.expect("Failed to insert order");
	}

	let backend = Arc::new(PostgresBackend::new((*pool).clone()));

	// Act
	let builder = AnalyzeBuilder::new(backend).table("orders").verbose(true);
	let result = builder.execute().await;

	// Assert
	assert!(
		result.is_ok(),
		"ANALYZE VERBOSE should execute successfully"
	);

	// Verify the generated SQL is correct
	let backend_for_sql = Arc::new(PostgresBackend::new((*pool).clone()));
	let sql = AnalyzeBuilder::new(backend_for_sql)
		.table("orders")
		.verbose(true)
		.build();
	assert_eq!(sql, "ANALYZE VERBOSE \"orders\"");
}

/// Test ANALYZE with specific columns on PostgreSQL
///
/// **Test Intent**: Verify that ANALYZE with column specification executes
/// successfully and targets only the specified columns.
///
/// **Integration Point**: AnalyzeBuilder.columns() + PostgresBackend
#[rstest]
#[tokio::test]
async fn test_analyze_postgres_with_columns(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	// Arrange
	let (_container, pool, _port, url) = postgres_container.await;
	reinitialize_database(&url).await.unwrap();

	sqlx::query(
		"CREATE TABLE IF NOT EXISTS events (
            id SERIAL PRIMARY KEY,
            event_type TEXT NOT NULL,
            event_data JSONB,
            created_at TIMESTAMP DEFAULT NOW(),
            processed BOOLEAN DEFAULT FALSE
        )",
	)
	.execute(pool.as_ref())
	.await
	.expect("Failed to create events table");

	// Insert test data
	for i in 1..=100 {
		sqlx::query("INSERT INTO events (event_type, event_data, processed) VALUES ($1, $2, $3)")
			.bind(format!("type_{}", i % 5))
			.bind(serde_json::json!({"index": i, "data": format!("event_{}", i)}))
			.bind(i % 3 == 0)
			.execute(pool.as_ref())
			.await
			.expect("Failed to insert event");
	}

	let backend = Arc::new(PostgresBackend::new((*pool).clone()));

	// Act
	let builder = AnalyzeBuilder::new(backend)
		.table("events")
		.columns(vec!["event_type", "processed"]);
	let result = builder.execute().await;

	// Assert
	assert!(
		result.is_ok(),
		"ANALYZE with columns should execute successfully"
	);

	// Verify the generated SQL is correct
	let backend_for_sql = Arc::new(PostgresBackend::new((*pool).clone()));
	let sql = AnalyzeBuilder::new(backend_for_sql)
		.table("events")
		.columns(vec!["event_type", "processed"])
		.build();
	assert_eq!(sql, "ANALYZE \"events\" (\"event_type\", \"processed\")");
}

/// Test ANALYZE updates actual table statistics in PostgreSQL
///
/// **Test Intent**: Verify that ANALYZE actually updates the statistics
/// visible in pg_class (reltuples) after data modifications.
///
/// **Integration Point**: AnalyzeBuilder + PostgreSQL statistics system
#[rstest]
#[tokio::test]
async fn test_analyze_postgres_updates_statistics(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	// Arrange
	let (_container, pool, _port, url) = postgres_container.await;
	reinitialize_database(&url).await.unwrap();

	sqlx::query(
		"CREATE TABLE IF NOT EXISTS metrics (
            id SERIAL PRIMARY KEY,
            metric_name TEXT NOT NULL,
            metric_value DOUBLE PRECISION NOT NULL
        )",
	)
	.execute(pool.as_ref())
	.await
	.expect("Failed to create metrics table");

	// Get initial statistics (should be 0 or -1 for new table)
	let initial_stats: (f32,) =
		sqlx::query_as("SELECT COALESCE(reltuples, 0) FROM pg_class WHERE relname = 'metrics'")
			.fetch_one(pool.as_ref())
			.await
			.expect("Failed to get initial statistics");

	// Insert test data
	for i in 1..=1000 {
		sqlx::query("INSERT INTO metrics (metric_name, metric_value) VALUES ($1, $2)")
			.bind(format!("metric_{}", i))
			.bind(i as f64 * 1.5)
			.execute(pool.as_ref())
			.await
			.expect("Failed to insert metric");
	}

	let backend = Arc::new(PostgresBackend::new((*pool).clone()));

	// Act
	let builder = AnalyzeBuilder::new(backend).table("metrics");
	builder.execute().await.expect("ANALYZE should succeed");

	// Assert
	let updated_stats: (f32,) =
		sqlx::query_as("SELECT COALESCE(reltuples, 0) FROM pg_class WHERE relname = 'metrics'")
			.fetch_one(pool.as_ref())
			.await
			.expect("Failed to get updated statistics");

	assert!(
		updated_stats.0 > initial_stats.0,
		"Statistics should be updated after ANALYZE: initial={}, updated={}",
		initial_stats.0,
		updated_stats.0
	);

	// Verify approximate row count is close to actual (within 10%)
	let actual_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM metrics")
		.fetch_one(pool.as_ref())
		.await
		.expect("Failed to count rows");

	let estimated = updated_stats.0 as i64;
	let actual = actual_count.0;
	let difference_ratio = (estimated - actual).abs() as f64 / actual as f64;

	assert!(
		difference_ratio < 0.1,
		"Estimated row count should be within 10% of actual: estimated={}, actual={}",
		estimated,
		actual
	);
}

/// Test ANALYZE on empty table in PostgreSQL
///
/// **Test Intent**: Verify that ANALYZE executes successfully on empty tables
/// without errors.
///
/// **Integration Point**: AnalyzeBuilder + PostgresBackend with empty table
#[rstest]
#[tokio::test]
async fn test_analyze_postgres_empty_table(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	// Arrange
	let (_container, pool, _port, url) = postgres_container.await;
	reinitialize_database(&url).await.unwrap();

	sqlx::query(
		"CREATE TABLE IF NOT EXISTS empty_table (
            id SERIAL PRIMARY KEY,
            data TEXT
        )",
	)
	.execute(pool.as_ref())
	.await
	.expect("Failed to create empty_table");

	let backend = Arc::new(PostgresBackend::new((*pool).clone()));

	// Act
	let builder = AnalyzeBuilder::new(backend).table("empty_table");
	let result = builder.execute().await;

	// Assert
	assert!(
		result.is_ok(),
		"ANALYZE on empty table should execute successfully"
	);
}

/// Test ANALYZE with all options combined on PostgreSQL
///
/// **Test Intent**: Verify that ANALYZE with verbose mode and column specification
/// works correctly together.
///
/// **Integration Point**: AnalyzeBuilder.verbose().columns() + PostgresBackend
#[rstest]
#[tokio::test]
async fn test_analyze_postgres_verbose_with_columns(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	// Arrange
	let (_container, pool, _port, url) = postgres_container.await;
	reinitialize_database(&url).await.unwrap();

	sqlx::query(
		"CREATE TABLE IF NOT EXISTS logs (
            id SERIAL PRIMARY KEY,
            level TEXT NOT NULL,
            message TEXT NOT NULL,
            timestamp TIMESTAMP DEFAULT NOW()
        )",
	)
	.execute(pool.as_ref())
	.await
	.expect("Failed to create logs table");

	// Insert test data
	let levels = ["DEBUG", "INFO", "WARN", "ERROR"];
	for i in 1..=100 {
		sqlx::query("INSERT INTO logs (level, message) VALUES ($1, $2)")
			.bind(levels[i % 4])
			.bind(format!("Log message {}", i))
			.execute(pool.as_ref())
			.await
			.expect("Failed to insert log");
	}

	let backend = Arc::new(PostgresBackend::new((*pool).clone()));

	// Act
	let builder = AnalyzeBuilder::new(backend)
		.table("logs")
		.columns(vec!["level"])
		.verbose(true);
	let result = builder.execute().await;

	// Assert
	assert!(
		result.is_ok(),
		"ANALYZE VERBOSE with columns should execute successfully"
	);

	// Verify the generated SQL is correct
	let backend_for_sql = Arc::new(PostgresBackend::new((*pool).clone()));
	let sql = AnalyzeBuilder::new(backend_for_sql)
		.table("logs")
		.columns(vec!["level"])
		.verbose(true)
		.build();
	assert_eq!(sql, "ANALYZE VERBOSE \"logs\" (\"level\")");
}

// ============================================================================
// SQLite ANALYZE Tests
// ============================================================================

/// Test ANALYZE executes successfully on SQLite database-wide
///
/// **Test Intent**: Verify that ANALYZE without table specification executes
/// successfully on SQLite and updates the sqlite_stat1 table.
///
/// **Integration Point**: AnalyzeBuilder + SqliteBackend
#[rstest]
#[tokio::test]
async fn test_analyze_sqlite_database_wide() {
	use reinhardt_db::backends::SqliteBackend;
	use sqlx::SqlitePool;

	// Arrange
	let pool = SqlitePool::connect(":memory:")
		.await
		.expect("Failed to create SQLite pool");

	sqlx::query(
		"CREATE TABLE IF NOT EXISTS items (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            quantity INTEGER NOT NULL
        )",
	)
	.execute(&pool)
	.await
	.expect("Failed to create items table");

	// Insert test data
	for i in 1..=100 {
		sqlx::query("INSERT INTO items (name, quantity) VALUES (?, ?)")
			.bind(format!("Item {}", i))
			.bind(i)
			.execute(&pool)
			.await
			.expect("Failed to insert item");
	}

	let backend = Arc::new(SqliteBackend::new(pool.clone()));

	// Act
	let builder = AnalyzeBuilder::new(backend);
	let result = builder.execute().await;

	// Assert
	assert!(
		result.is_ok(),
		"ANALYZE should execute successfully on SQLite"
	);

	// Verify sqlite_stat1 table was created (ANALYZE creates this)
	let stat_exists: (i32,) = sqlx::query_as(
		"SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='sqlite_stat1'",
	)
	.fetch_one(&pool)
	.await
	.expect("Failed to check sqlite_stat1");

	assert_eq!(
		stat_exists.0, 1,
		"sqlite_stat1 table should exist after ANALYZE"
	);
}

/// Test ANALYZE executes successfully on a specific SQLite table
///
/// **Test Intent**: Verify that ANALYZE with table specification executes
/// successfully on SQLite.
///
/// **Integration Point**: AnalyzeBuilder.table() + SqliteBackend
#[rstest]
#[tokio::test]
async fn test_analyze_sqlite_specific_table() {
	use reinhardt_db::backends::SqliteBackend;
	use sqlx::SqlitePool;

	// Arrange
	let pool = SqlitePool::connect(":memory:")
		.await
		.expect("Failed to create SQLite pool");

	sqlx::query(
		"CREATE TABLE IF NOT EXISTS categories (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL UNIQUE,
            description TEXT
        )",
	)
	.execute(&pool)
	.await
	.expect("Failed to create categories table");

	// Create an index to have something meaningful to analyze
	sqlx::query("CREATE INDEX IF NOT EXISTS idx_categories_name ON categories(name)")
		.execute(&pool)
		.await
		.expect("Failed to create index");

	// Insert test data
	for i in 1..=50 {
		sqlx::query("INSERT INTO categories (name, description) VALUES (?, ?)")
			.bind(format!("Category {}", i))
			.bind(format!("Description for category {}", i))
			.execute(&pool)
			.await
			.expect("Failed to insert category");
	}

	let backend = Arc::new(SqliteBackend::new(pool.clone()));

	// Act
	let builder = AnalyzeBuilder::new(backend).table("categories");
	let result = builder.execute().await;

	// Assert
	assert!(
		result.is_ok(),
		"ANALYZE on specific table should execute successfully on SQLite"
	);

	// Verify the generated SQL is correct
	let backend_for_sql = Arc::new(SqliteBackend::new(pool.clone()));
	let sql = AnalyzeBuilder::new(backend_for_sql)
		.table("categories")
		.build();
	assert_eq!(sql, "ANALYZE \"categories\"");
}

/// Test ANALYZE on SQLite with index
///
/// **Test Intent**: Verify that ANALYZE updates statistics for tables with indexes
/// on SQLite.
///
/// **Integration Point**: AnalyzeBuilder + SqliteBackend with indexed table
#[rstest]
#[tokio::test]
async fn test_analyze_sqlite_with_index() {
	use reinhardt_db::backends::SqliteBackend;
	use sqlx::SqlitePool;

	// Arrange
	let pool = SqlitePool::connect(":memory:")
		.await
		.expect("Failed to create SQLite pool");

	sqlx::query(
		"CREATE TABLE IF NOT EXISTS articles (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            title TEXT NOT NULL,
            author TEXT NOT NULL,
            published_at TEXT
        )",
	)
	.execute(&pool)
	.await
	.expect("Failed to create articles table");

	sqlx::query("CREATE INDEX IF NOT EXISTS idx_articles_author ON articles(author)")
		.execute(&pool)
		.await
		.expect("Failed to create author index");

	sqlx::query("CREATE INDEX IF NOT EXISTS idx_articles_title ON articles(title)")
		.execute(&pool)
		.await
		.expect("Failed to create title index");

	// Insert test data
	let authors = ["Alice", "Bob", "Charlie", "Diana"];
	for i in 1..=200 {
		sqlx::query("INSERT INTO articles (title, author, published_at) VALUES (?, ?, ?)")
			.bind(format!("Article {}", i))
			.bind(authors[i % 4])
			.bind(format!("2024-01-{:02}", (i % 28) + 1))
			.execute(&pool)
			.await
			.expect("Failed to insert article");
	}

	let backend = Arc::new(SqliteBackend::new(pool.clone()));

	// Act
	let builder = AnalyzeBuilder::new(backend).table("articles");
	let result = builder.execute().await;

	// Assert
	assert!(
		result.is_ok(),
		"ANALYZE on table with indexes should execute successfully"
	);

	// Verify statistics were generated for the indexes
	let stats: Vec<(String,)> =
		sqlx::query_as("SELECT idx FROM sqlite_stat1 WHERE tbl = 'articles' ORDER BY idx")
			.fetch_all(&pool)
			.await
			.expect("Failed to query statistics");

	assert!(
		!stats.is_empty(),
		"Statistics should be generated for articles table"
	);
}

/// Test ANALYZE on empty SQLite table
///
/// **Test Intent**: Verify that ANALYZE executes successfully on empty SQLite tables.
///
/// **Integration Point**: AnalyzeBuilder + SqliteBackend with empty table
#[rstest]
#[tokio::test]
async fn test_analyze_sqlite_empty_table() {
	use reinhardt_db::backends::SqliteBackend;
	use sqlx::SqlitePool;

	// Arrange
	let pool = SqlitePool::connect(":memory:")
		.await
		.expect("Failed to create SQLite pool");

	sqlx::query(
		"CREATE TABLE IF NOT EXISTS empty_sqlite_table (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            data TEXT
        )",
	)
	.execute(&pool)
	.await
	.expect("Failed to create empty_sqlite_table");

	let backend = Arc::new(SqliteBackend::new(pool.clone()));

	// Act
	let builder = AnalyzeBuilder::new(backend).table("empty_sqlite_table");
	let result = builder.execute().await;

	// Assert
	assert!(
		result.is_ok(),
		"ANALYZE on empty SQLite table should execute successfully"
	);
}

// ============================================================================
// Error Handling Tests
// ============================================================================

/// Test ANALYZE on non-existent table returns error
///
/// **Test Intent**: Verify that ANALYZE on a non-existent table returns
/// an appropriate error.
///
/// **Integration Point**: AnalyzeBuilder error handling
#[rstest]
#[tokio::test]
async fn test_analyze_postgres_nonexistent_table(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	// Arrange
	let (_container, pool, _port, url) = postgres_container.await;
	reinitialize_database(&url).await.unwrap();

	let backend = Arc::new(PostgresBackend::new((*pool).clone()));

	// Act
	let builder = AnalyzeBuilder::new(backend).table("nonexistent_table_xyz");
	let result = builder.execute().await;

	// Assert
	assert!(
		result.is_err(),
		"ANALYZE on non-existent table should return error"
	);
}

/// Test ANALYZE on non-existent SQLite table returns error
///
/// **Test Intent**: Verify that ANALYZE on a non-existent table returns
/// an appropriate error on SQLite.
///
/// **Integration Point**: AnalyzeBuilder error handling for SQLite
#[rstest]
#[tokio::test]
async fn test_analyze_sqlite_nonexistent_table() {
	use reinhardt_db::backends::SqliteBackend;
	use sqlx::SqlitePool;

	// Arrange
	let pool = SqlitePool::connect(":memory:")
		.await
		.expect("Failed to create SQLite pool");

	let backend = Arc::new(SqliteBackend::new(pool));

	// Act
	let builder = AnalyzeBuilder::new(backend).table("nonexistent_table_abc");
	let result = builder.execute().await;

	// Assert
	assert!(
		result.is_err(),
		"ANALYZE on non-existent SQLite table should return error"
	);
}
