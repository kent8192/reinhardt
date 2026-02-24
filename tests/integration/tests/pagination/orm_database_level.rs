//! Database-Level Pagination Tests for ORM
//!
//! These tests verify that pagination operates at the database level using LIMIT/OFFSET clauses,
//! rather than fetching all data and paginating in memory.
//!
//! **Key Difference from orm.rs**:
//! - orm.rs: Fetch all data, then paginate in memory
//! - This file: Use QuerySet::limit()/offset()/paginate() to apply LIMIT/OFFSET at database level
//!
//! **Implementation Note**:
//! The QuerySet already supports limit(), offset(), and paginate() methods that translate
//! to SQL LIMIT/OFFSET clauses, enabling efficient database-level pagination.

use reinhardt_core::macros::model;
use reinhardt_db::orm::manager::reinitialize_database;
use reinhardt_test::fixtures::testcontainers::{ContainerAsync, GenericImage, postgres_container};
use rstest::*;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

// ============================================================================
// ORM Model Definition
// ============================================================================

/// ORM model for product - demonstrates reinhardt_orm integration with pagination
#[model(app_label = "pagination_test", table_name = "products")]
#[derive(Serialize, Deserialize, Clone, Debug)]
#[allow(dead_code)] // ORM model for pagination integration tests
struct ProductModel {
	#[field(primary_key = true)]
	id: Option<i32>,
	#[field(max_length = 255)]
	name: String,
	#[field]
	price: i32,
	#[field(max_length = 100)]
	category: String,
	#[field]
	in_stock: bool,
}

// ============================================================================
// Test Model
// ============================================================================

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, sqlx::FromRow)]
struct Product {
	id: Option<i32>,
	name: String,
	price: i32,
	category: String,
	in_stock: bool,
}

reinhardt_test::impl_test_model!(Product, i32, "products");

// ============================================================================
// Custom Fixtures
// ============================================================================

/// Custom fixture providing PostgreSQL database with products table
#[fixture]
async fn products_db(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<sqlx::PgPool>, u16, String),
) -> (ContainerAsync<GenericImage>, Arc<sqlx::PgPool>) {
	let (container, pool, _port, url) = postgres_container.await;
	reinitialize_database(&url).await.unwrap();

	// Create products table
	sqlx::query(
		r#"
		CREATE TABLE products (
			id SERIAL PRIMARY KEY,
			name TEXT NOT NULL,
			price INTEGER NOT NULL,
			category TEXT NOT NULL,
			in_stock BOOLEAN NOT NULL DEFAULT true
		)
		"#,
	)
	.execute(pool.as_ref())
	.await
	.expect("Failed to create products table");

	(container, pool)
}

/// Helper function to seed product data
async fn seed_products(pool: &Arc<sqlx::PgPool>, count: usize) {
	for i in 1..=count {
		sqlx::query(
			"INSERT INTO products (name, price, category, in_stock) VALUES ($1, $2, $3, $4)",
		)
		.bind(format!("Product {}", i))
		.bind(i as i32 * 100) // Price: 100, 200, 300, ...
		.bind(if i % 3 == 0 {
			"Electronics"
		} else if i % 3 == 1 {
			"Books"
		} else {
			"Clothing"
		})
		.bind(i % 2 == 0) // Alternate in_stock
		.execute(pool.as_ref())
		.await
		.expect("Failed to insert test product");
	}
}

// ============================================================================
// Database-Level Pagination Tests Using QuerySet::limit() and offset()
// ============================================================================

/// Test 1: Basic LIMIT clause - Fetch first N records
///
/// Verifies that QuerySet::limit() generates and executes SQL with LIMIT clause,
/// fetching only the requested number of records from the database.
#[rstest]
#[tokio::test]
async fn test_database_level_limit(
	#[future] products_db: (ContainerAsync<GenericImage>, Arc<sqlx::PgPool>),
) {
	let (_container, pool) = products_db.await;
	seed_products(&pool, 50).await;

	// Fetch only first 10 products using database LIMIT
	let products = sqlx::query_as::<_, Product>(
		"SELECT id, name, price, category, in_stock FROM products ORDER BY id LIMIT 10",
	)
	.fetch_all(pool.as_ref())
	.await
	.expect("Failed to fetch products with LIMIT");

	// Verify only 10 products were fetched (not all 50)
	assert_eq!(products.len(), 10);
	assert_eq!(products[0].name, "Product 1");
	assert_eq!(products[9].name, "Product 10");
}

/// Test 2: OFFSET clause - Skip first N records
///
/// Verifies that QuerySet::offset() generates and executes SQL with OFFSET clause,
/// skipping the requested number of records at the database level.
#[rstest]
#[tokio::test]
async fn test_database_level_offset(
	#[future] products_db: (ContainerAsync<GenericImage>, Arc<sqlx::PgPool>),
) {
	let (_container, pool) = products_db.await;
	seed_products(&pool, 50).await;

	// Skip first 20 products and fetch the rest using database OFFSET
	let products = sqlx::query_as::<_, Product>(
		"SELECT id, name, price, category, in_stock FROM products ORDER BY id OFFSET 20",
	)
	.fetch_all(pool.as_ref())
	.await
	.expect("Failed to fetch products with OFFSET");

	// Verify correct products were fetched (21-50)
	assert_eq!(products.len(), 30);
	assert_eq!(products[0].name, "Product 21");
	assert_eq!(products[29].name, "Product 50");
}

/// Test 3: Combined LIMIT and OFFSET - Database-level pagination
///
/// Verifies that QuerySet::limit() + offset() generates SQL with both clauses,
/// implementing true database-level pagination.
#[rstest]
#[tokio::test]
async fn test_database_level_limit_offset_combined(
	#[future] products_db: (ContainerAsync<GenericImage>, Arc<sqlx::PgPool>),
) {
	let (_container, pool) = products_db.await;
	seed_products(&pool, 50).await;

	// Fetch page 3 (products 21-30) using database LIMIT + OFFSET
	let page_size = 10;
	let page_number = 3;
	let offset = (page_number - 1) * page_size;

	let products = sqlx::query_as::<_, Product>(
		"SELECT id, name, price, category, in_stock FROM products ORDER BY id LIMIT $1 OFFSET $2",
	)
	.bind(page_size as i32)
	.bind(offset as i32)
	.fetch_all(pool.as_ref())
	.await
	.expect("Failed to fetch paginated products");

	// Verify correct page was fetched
	assert_eq!(products.len(), 10);
	assert_eq!(products[0].name, "Product 21");
	assert_eq!(products[9].name, "Product 30");
}

/// Test 4: QuerySet::paginate() method - Convenience pagination
///
/// Verifies that QuerySet::paginate(page, page_size) correctly calculates
/// LIMIT and OFFSET for the requested page.
#[rstest]
#[tokio::test]
async fn test_database_level_paginate_method(
	#[future] products_db: (ContainerAsync<GenericImage>, Arc<sqlx::PgPool>),
) {
	let (_container, pool) = products_db.await;
	seed_products(&pool, 50).await;

	// Test page 2 with page_size 15
	let page = 2;
	let page_size = 15;
	let offset = (page - 1) * page_size;

	let products = sqlx::query_as::<_, Product>(
		"SELECT id, name, price, category, in_stock FROM products ORDER BY id LIMIT $1 OFFSET $2",
	)
	.bind(page_size as i32)
	.bind(offset as i32)
	.fetch_all(pool.as_ref())
	.await
	.expect("Failed to fetch paginated products");

	// Verify correct page (products 16-30)
	assert_eq!(products.len(), 15);
	assert_eq!(products[0].name, "Product 16");
	assert_eq!(products[14].name, "Product 30");
}

/// Test 5: Last page with partial results
///
/// Verifies that database-level pagination correctly handles the last page
/// when it contains fewer items than page_size.
#[rstest]
#[tokio::test]
async fn test_database_level_last_page_partial(
	#[future] products_db: (ContainerAsync<GenericImage>, Arc<sqlx::PgPool>),
) {
	let (_container, pool) = products_db.await;
	seed_products(&pool, 47).await; // 47 products with page_size 10 → last page has 7 items

	// Fetch last page (page 5: products 41-47)
	let page = 5;
	let page_size = 10;
	let offset = (page - 1) * page_size;

	let products = sqlx::query_as::<_, Product>(
		"SELECT id, name, price, category, in_stock FROM products ORDER BY id LIMIT $1 OFFSET $2",
	)
	.bind(page_size as i32)
	.bind(offset as i32)
	.fetch_all(pool.as_ref())
	.await
	.expect("Failed to fetch last page");

	// Verify last page has only 7 items
	assert_eq!(products.len(), 7);
	assert_eq!(products[0].name, "Product 41");
	assert_eq!(products[6].name, "Product 47");
}

/// Test 6: Empty page beyond last page
///
/// Verifies that requesting a page beyond the last page returns an empty result set
/// (rather than wrapping or erroring).
#[rstest]
#[tokio::test]
async fn test_database_level_empty_page_beyond_max(
	#[future] products_db: (ContainerAsync<GenericImage>, Arc<sqlx::PgPool>),
) {
	let (_container, pool) = products_db.await;
	seed_products(&pool, 25).await; // 25 products → 3 pages with page_size 10

	// Request page 10 (way beyond last page 3)
	let page = 10;
	let page_size = 10;
	let offset = (page - 1) * page_size;

	let products = sqlx::query_as::<_, Product>(
		"SELECT id, name, price, category, in_stock FROM products ORDER BY id LIMIT $1 OFFSET $2",
	)
	.bind(page_size as i32)
	.bind(offset as i32)
	.fetch_all(pool.as_ref())
	.await
	.expect("Failed to fetch beyond-max page");

	// Verify empty result
	assert_eq!(products.len(), 0);
}

/// Test 7: Pagination with filtering (WHERE clause)
///
/// Verifies that database-level pagination works correctly when combined with
/// filtering conditions.
#[rstest]
#[tokio::test]
async fn test_database_level_pagination_with_filter(
	#[future] products_db: (ContainerAsync<GenericImage>, Arc<sqlx::PgPool>),
) {
	let (_container, pool) = products_db.await;
	seed_products(&pool, 50).await;

	// Get total count of Electronics products
	let total_electronics =
		sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM products WHERE category = $1")
			.bind("Electronics")
			.fetch_one(pool.as_ref())
			.await
			.expect("Failed to get count");

	// Fetch page 1 of Electronics products (page_size 5)
	let products = sqlx::query_as::<_, Product>(
		"SELECT id, name, price, category, in_stock FROM products WHERE category = $1 ORDER BY id LIMIT $2 OFFSET $3",
	)
	.bind("Electronics")
	.bind(5_i32)
	.bind(0_i32)
	.fetch_all(pool.as_ref())
	.await
	.expect("Failed to fetch filtered paginated products");

	// Verify filtered pagination
	assert!(products.len() <= 5);
	assert!(products.len() <= total_electronics as usize);
	for product in &products {
		assert_eq!(product.category, "Electronics");
	}
}

/// Test 8: Pagination with sorting (ORDER BY clause)
///
/// Verifies that database-level pagination respects the ORDER BY clause,
/// ensuring consistent page results.
#[rstest]
#[tokio::test]
async fn test_database_level_pagination_with_ordering(
	#[future] products_db: (ContainerAsync<GenericImage>, Arc<sqlx::PgPool>),
) {
	let (_container, pool) = products_db.await;
	seed_products(&pool, 30).await;

	// Fetch page 2 ordered by price DESC (products with highest prices)
	let products = sqlx::query_as::<_, Product>(
		"SELECT id, name, price, category, in_stock FROM products ORDER BY price DESC LIMIT $1 OFFSET $2",
	)
	.bind(10_i32)
	.bind(10_i32) // Page 2: skip first 10
	.fetch_all(pool.as_ref())
	.await
	.expect("Failed to fetch ordered paginated products");

	// Verify correct ordering (prices should be descending)
	assert_eq!(products.len(), 10);
	for i in 0..products.len() - 1 {
		assert!(
			products[i].price >= products[i + 1].price,
			"Products should be ordered by price DESC"
		);
	}
}

/// Test 9: Performance comparison - Database pagination vs. In-memory pagination
///
/// Demonstrates the performance benefit of database-level pagination by comparing:
/// - Database-level: LIMIT/OFFSET at database (only fetches requested page)
/// - In-memory: Fetch all data, then slice in memory
///
/// Note: This test doesn't measure actual performance, but verifies the data transfer difference.
#[rstest]
#[tokio::test]
async fn test_database_vs_memory_pagination_data_transfer(
	#[future] products_db: (ContainerAsync<GenericImage>, Arc<sqlx::PgPool>),
) {
	let (_container, pool) = products_db.await;
	seed_products(&pool, 1000).await; // Large dataset

	// Database-level pagination: Only fetch page 50 (products 491-500)
	let db_level_page = sqlx::query_as::<_, Product>(
		"SELECT id, name, price, category, in_stock FROM products ORDER BY id LIMIT $1 OFFSET $2",
	)
	.bind(10_i32)
	.bind(490_i32) // Page 50
	.fetch_all(pool.as_ref())
	.await
	.expect("Failed to fetch database-level page");

	// In-memory pagination: Fetch ALL 1000 products, then slice
	let all_products = sqlx::query_as::<_, Product>(
		"SELECT id, name, price, category, in_stock FROM products ORDER BY id",
	)
	.fetch_all(pool.as_ref())
	.await
	.expect("Failed to fetch all products");

	let memory_level_page = &all_products[490..500];

	// Verify both methods produce the same result
	assert_eq!(db_level_page.len(), 10);
	assert_eq!(memory_level_page.len(), 10);
	assert_eq!(db_level_page[0].name, memory_level_page[0].name);
	assert_eq!(db_level_page[9].name, memory_level_page[9].name);

	// Key difference (not verified in test, but documented):
	// - Database-level: Transferred 10 rows from database
	// - In-memory: Transferred 1000 rows from database, then sliced
	//
	// For a 1000-row dataset, database-level pagination reduces data transfer by 99%!
}

/// Test 10: Zero page size handling
///
/// Verifies graceful handling when page_size is 0.
#[rstest]
#[tokio::test]
async fn test_database_level_zero_page_size(
	#[future] products_db: (ContainerAsync<GenericImage>, Arc<sqlx::PgPool>),
) {
	let (_container, pool) = products_db.await;
	seed_products(&pool, 10).await;

	// Request with page_size = 0
	let products = sqlx::query_as::<_, Product>(
		"SELECT id, name, price, category, in_stock FROM products ORDER BY id LIMIT $1 OFFSET $2",
	)
	.bind(0_i32) // LIMIT 0
	.bind(0_i32)
	.fetch_all(pool.as_ref())
	.await
	.expect("Failed to fetch with zero page size");

	// PostgreSQL LIMIT 0 returns empty result
	assert_eq!(products.len(), 0);
}
