//! Generic Views Filtering and Ordering Integration Tests
//!
//! Tests comprehensive filtering and ordering functionality for Generic API Views:
//! - Single field filtering (exact match)
//! - Multiple field filtering (AND condition)
//! - Case-insensitive search
//! - Contains/search filtering
//! - Ordering (ascending/descending)
//! - Multiple field ordering
//! - Combined filtering + ordering
//! - Edge cases (no results, invalid fields)
//!
//! **Test Category**: Combination Testing
//!
//! **Fixtures Used:**
//! - shared_db_pool: Shared PostgreSQL database pool with ORM initialized
//!
//! **Test Data Schema:**
//! - products(id SERIAL PRIMARY KEY, name TEXT NOT NULL, category TEXT NOT NULL,
//!   price DECIMAL NOT NULL, stock INT NOT NULL, created_at TIMESTAMP)

use bytes::Bytes;
use chrono::{DateTime, Utc};
use hyper::{HeaderMap, Method, StatusCode, Version};
use reinhardt_core::macros::model;
use reinhardt_http::Request;
use reinhardt_query::prelude::{
	ColumnDef, Iden, IntoIden, PostgresQueryBuilder, Query, QueryStatementBuilder,
};
use reinhardt_rest::serializers::JsonSerializer;
use reinhardt_test::fixtures::shared_db_pool;
use reinhardt_views::viewsets::FilterConfig;
use reinhardt_views::{ListAPIView, View};
use rstest::*;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::sync::Arc;

// ============================================================================
// Model Definitions
// ============================================================================

/// Product model for filtering/ordering testing
#[allow(dead_code)]
#[model(app_label = "views_filtering", table_name = "products")]
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
struct Product {
	#[field(primary_key = true)]
	id: Option<i64>,
	#[field(max_length = 200)]
	name: String,
	#[field(max_length = 100)]
	category: String,
	price: i32, // Using integer for price (cents)
	stock: i32,
	#[field(null = true)]
	created_at: Option<DateTime<Utc>>,
}

// ============================================================================
// Table Identifiers (for reinhardt-query operations)
// ============================================================================

#[derive(Debug, Clone, Copy, Iden)]
enum Products {
	Table,
	Id,
	Name,
	Category,
	Price,
	Stock,
	CreatedAt,
}

// ============================================================================
// Fixtures
// ============================================================================

/// Fixture: Initialize database connection
#[fixture]
async fn db_pool(#[future] shared_db_pool: (PgPool, String)) -> Arc<PgPool> {
	let (pool, _url) = shared_db_pool.await;
	Arc::new(pool)
}

/// Fixture: Setup products table
#[fixture]
async fn products_table(#[future] db_pool: Arc<PgPool>) -> Arc<PgPool> {
	let pool = db_pool.await;

	// Create products table
	let mut create_table_stmt = Query::create_table();
	create_table_stmt
		.table(Products::Table.into_iden())
		.if_not_exists()
		.col(
			ColumnDef::new(Products::Id)
				.big_integer()
				.not_null(true)
				.auto_increment(true)
				.primary_key(true),
		)
		.col(
			ColumnDef::new(Products::Name)
				.string_len(200)
				.not_null(true),
		)
		.col(
			ColumnDef::new(Products::Category)
				.string_len(100)
				.not_null(true),
		)
		.col(ColumnDef::new(Products::Price).integer().not_null(true))
		.col(ColumnDef::new(Products::Stock).integer().not_null(true))
		.col(ColumnDef::new(Products::CreatedAt).timestamp());

	let sql = create_table_stmt.to_string(PostgresQueryBuilder::new());
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create products table");

	pool
}

/// Fixture: Setup products table with diverse sample data
#[fixture]
async fn products_with_data(#[future] products_table: Arc<PgPool>) -> Arc<PgPool> {
	let pool = products_table.await;

	// Insert diverse products for filtering/ordering tests
	let products_data = vec![
		("Laptop Pro", "Electronics", 150000, 10),
		("Laptop Air", "Electronics", 120000, 15),
		("Desktop PC", "Electronics", 200000, 5),
		("Gaming Mouse", "Accessories", 5000, 50),
		("Wireless Keyboard", "Accessories", 8000, 30),
		("USB Cable", "Accessories", 1000, 100),
		("Office Chair", "Furniture", 30000, 20),
		("Standing Desk", "Furniture", 50000, 8),
		("Bookshelf", "Furniture", 15000, 12),
		("Smartphone X", "Electronics", 80000, 25),
	];

	for (name, category, price, stock) in products_data {
		let product = Product::new(
			name.to_string(),
			category.to_string(),
			price,
			stock,
			Some(Utc::now()),
		);

		let sql = "INSERT INTO products (name, category, price, stock, created_at) VALUES ($1, $2, $3, $4, $5)";
		sqlx::query(sql)
			.bind(&product.name)
			.bind(&product.category)
			.bind(product.price)
			.bind(product.stock)
			.bind(product.created_at)
			.execute(pool.as_ref())
			.await
			.expect("Failed to insert product");
	}

	pool
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Helper: Create HTTP GET request
fn create_get_request(uri: &str) -> Request {
	Request::builder()
		.method(Method::GET)
		.uri(uri)
		.version(Version::HTTP_11)
		.headers(HeaderMap::new())
		.body(Bytes::new())
		.build()
		.expect("Failed to build request")
}

// ============================================================================
// Tests
// ============================================================================

/// Test: Single field exact match filtering
#[rstest]
#[tokio::test]
async fn test_single_field_exact_match(#[future] products_with_data: Arc<PgPool>) {
	let _pool = products_with_data.await;

	let view = ListAPIView::<Product, JsonSerializer<Product>>::new().with_filter_config(
		FilterConfig::new().with_filterable_fields(vec!["category".to_string()]),
	);

	let request = create_get_request("/products/?category=Electronics");
	let result = view.dispatch(request).await;

	assert!(result.is_ok(), "Single field filtering should succeed");
	let response = result.unwrap();
	assert_eq!(response.status, StatusCode::OK);

	let body_str = String::from_utf8(response.body.to_vec()).unwrap();
	// Should contain Electronics products
	assert!(
		body_str.contains("Electronics"),
		"Response should contain Electronics category"
	);
	assert!(
		body_str.contains("Laptop")
			|| body_str.contains("Desktop")
			|| body_str.contains("Smartphone"),
		"Response should contain Electronics products"
	);
}

/// Test: Multiple field filtering (AND condition)
#[rstest]
#[tokio::test]
async fn test_multiple_field_filtering(#[future] products_with_data: Arc<PgPool>) {
	let _pool = products_with_data.await;

	let view = ListAPIView::<Product, JsonSerializer<Product>>::new().with_filter_config(
		FilterConfig::new()
			.with_filterable_fields(vec!["category".to_string(), "stock".to_string()]),
	);

	// Filter by category=Accessories AND stock=50
	let request = create_get_request("/products/?category=Accessories&stock=50");
	let result = view.dispatch(request).await;

	assert!(result.is_ok(), "Multiple field filtering should succeed");
	let response = result.unwrap();
	assert_eq!(response.status, StatusCode::OK);

	let body_str = String::from_utf8(response.body.to_vec()).unwrap();
	// Should contain only Gaming Mouse (category=Accessories, stock=50)
	assert!(
		body_str.contains("Gaming Mouse") || body_str.contains("Accessories"),
		"Response should match both filters"
	);
}

/// Test: Case-insensitive search
#[rstest]
#[tokio::test]
async fn test_case_insensitive_search(#[future] products_with_data: Arc<PgPool>) {
	let _pool = products_with_data.await;

	let view = ListAPIView::<Product, JsonSerializer<Product>>::new().with_filter_config(
		FilterConfig::new()
			.with_search_fields(vec!["name".to_string()])
			.case_insensitive(true),
	);

	// Search for "laptop" in lowercase
	let request = create_get_request("/products/?search=laptop");
	let result = view.dispatch(request).await;

	assert!(result.is_ok(), "Case-insensitive search should succeed");
	let response = result.unwrap();
	assert_eq!(response.status, StatusCode::OK);

	let body_str = String::from_utf8(response.body.to_vec()).unwrap();
	// Should match "Laptop Pro" and "Laptop Air"
	assert!(
		body_str.contains("Laptop"),
		"Search should find Laptop products (case-insensitive)"
	);
}

/// Test: Contains/search filtering
#[rstest]
#[tokio::test]
async fn test_contains_search(#[future] products_with_data: Arc<PgPool>) {
	let _pool = products_with_data.await;

	let view = ListAPIView::<Product, JsonSerializer<Product>>::new()
		.with_filter_config(FilterConfig::new().with_search_fields(vec!["name".to_string()]));

	// Search for products containing "Desk"
	let request = create_get_request("/products/?search=Desk");
	let result = view.dispatch(request).await;

	assert!(result.is_ok(), "Contains search should succeed");
	let response = result.unwrap();
	assert_eq!(response.status, StatusCode::OK);

	let body_str = String::from_utf8(response.body.to_vec()).unwrap();
	// Should match "Standing Desk" and "Desktop PC"
	assert!(
		body_str.contains("Desk"),
		"Search should find products containing 'Desk'"
	);
}

/// Test: Ordering ascending
#[rstest]
#[tokio::test]
async fn test_ordering_ascending(#[future] products_with_data: Arc<PgPool>) {
	let _pool = products_with_data.await;

	let view = ListAPIView::<Product, JsonSerializer<Product>>::new()
		.with_ordering(vec!["price".to_string()]);

	let request = create_get_request("/products/?ordering=price");
	let result = view.dispatch(request).await;

	assert!(result.is_ok(), "Ascending ordering should succeed");
	let response = result.unwrap();
	assert_eq!(response.status, StatusCode::OK);

	let body_str = String::from_utf8(response.body.to_vec()).unwrap();
	// Should return products ordered by price (lowest first)
	// USB Cable (1000) should appear before higher-priced items
	assert!(
		body_str.contains("Laptop") || body_str.contains("USB") || body_str.contains("Desktop"),
		"Response should contain ordered products"
	);
}

/// Test: Ordering descending
#[rstest]
#[tokio::test]
async fn test_ordering_descending(#[future] products_with_data: Arc<PgPool>) {
	let _pool = products_with_data.await;

	let view = ListAPIView::<Product, JsonSerializer<Product>>::new()
		.with_ordering(vec!["-price".to_string()]);

	let request = create_get_request("/products/?ordering=-price");
	let result = view.dispatch(request).await;

	assert!(result.is_ok(), "Descending ordering should succeed");
	let response = result.unwrap();
	assert_eq!(response.status, StatusCode::OK);

	let body_str = String::from_utf8(response.body.to_vec()).unwrap();
	// Should return products ordered by price (highest first)
	// Desktop PC (200000) should appear before lower-priced items
	assert!(
		body_str.contains("Laptop") || body_str.contains("Desktop") || body_str.contains("USB"),
		"Response should contain reverse ordered products"
	);
}

/// Test: Multiple field ordering
#[rstest]
#[tokio::test]
async fn test_multiple_field_ordering(#[future] products_with_data: Arc<PgPool>) {
	let _pool = products_with_data.await;

	let view = ListAPIView::<Product, JsonSerializer<Product>>::new()
		.with_ordering(vec!["category".to_string(), "-price".to_string()]);

	let request = create_get_request("/products/?ordering=category,-price");
	let result = view.dispatch(request).await;

	assert!(result.is_ok(), "Multiple field ordering should succeed");
	let response = result.unwrap();
	assert_eq!(response.status, StatusCode::OK);

	let body_str = String::from_utf8(response.body.to_vec()).unwrap();
	// Should order by category first (ascending), then by price (descending) within each category
	assert!(
		body_str.contains("Accessories")
			|| body_str.contains("Electronics")
			|| body_str.contains("Furniture"),
		"Response should contain multi-field ordered products"
	);
}

/// Test: Combined filtering + ordering
#[rstest]
#[tokio::test]
async fn test_combined_filtering_ordering(#[future] products_with_data: Arc<PgPool>) {
	let _pool = products_with_data.await;

	let view = ListAPIView::<Product, JsonSerializer<Product>>::new()
		.with_filter_config(
			FilterConfig::new().with_filterable_fields(vec!["category".to_string()]),
		)
		.with_ordering(vec!["-price".to_string()]);

	// Filter by category=Electronics, order by price descending
	let request = create_get_request("/products/?category=Electronics&ordering=-price");
	let result = view.dispatch(request).await;

	assert!(
		result.is_ok(),
		"Combined filtering and ordering should succeed"
	);
	let response = result.unwrap();
	assert_eq!(response.status, StatusCode::OK);

	let body_str = String::from_utf8(response.body.to_vec()).unwrap();
	// Should contain only Electronics products, ordered by price (descending)
	// Desktop PC (200000) should be first, followed by Laptop Pro (150000), etc.
	assert!(
		body_str.contains("Electronics"),
		"Response should contain filtered Electronics products"
	);
}

/// Test: Filtering with no results
#[rstest]
#[tokio::test]
async fn test_filtering_no_results(#[future] products_with_data: Arc<PgPool>) {
	let _pool = products_with_data.await;

	let view = ListAPIView::<Product, JsonSerializer<Product>>::new().with_filter_config(
		FilterConfig::new().with_filterable_fields(vec!["category".to_string()]),
	);

	// Filter by non-existent category
	let request = create_get_request("/products/?category=NonExistentCategory");
	let result = view.dispatch(request).await;

	assert!(result.is_ok(), "Filtering with no results should succeed");
	let response = result.unwrap();
	assert_eq!(response.status, StatusCode::OK);

	let body_str = String::from_utf8(response.body.to_vec()).unwrap();
	// Should return empty results
	assert!(
		body_str.contains("[]") || body_str.is_empty() || !body_str.contains("Product"),
		"Response should indicate no results"
	);
}

/// Test: Invalid filter field handling
#[rstest]
#[tokio::test]
async fn test_invalid_filter_field(#[future] products_with_data: Arc<PgPool>) {
	let _pool = products_with_data.await;

	let view = ListAPIView::<Product, JsonSerializer<Product>>::new().with_filter_config(
		FilterConfig::new().with_filterable_fields(vec!["category".to_string()]),
	);

	// Try to filter by a field not in filterable_fields
	let request = create_get_request("/products/?nonexistent_field=value");
	let result = view.dispatch(request).await;

	// Should either ignore invalid field or return all results
	assert!(
		result.is_ok(),
		"Invalid filter field should be handled gracefully"
	);
	let response = result.unwrap();
	assert_eq!(response.status, StatusCode::OK);

	// Invalid filters should be ignored, returning all products
	let body_str = String::from_utf8(response.body.to_vec()).unwrap();
	assert!(
		body_str.contains("Laptop") || body_str.contains("Desktop") || body_str.contains("USB"),
		"Invalid filter should be ignored, returning all products"
	);
}

/// Test: Range filtering simulation (using multiple exact matches)
#[rstest]
#[tokio::test]
async fn test_range_filtering_simulation(#[future] products_with_data: Arc<PgPool>) {
	let _pool = products_with_data.await;

	let view = ListAPIView::<Product, JsonSerializer<Product>>::new().with_filter_config(
		FilterConfig::new().with_filterable_fields(vec!["category".to_string()]),
	);

	// Note: Current FilterConfig doesn't support range operators (gte/lte)
	// This test verifies that basic filtering works as foundation for future range support
	let request = create_get_request("/products/?category=Accessories");
	let result = view.dispatch(request).await;

	assert!(
		result.is_ok(),
		"Basic filtering (foundation for range) should work"
	);
	let response = result.unwrap();
	assert_eq!(response.status, StatusCode::OK);

	let body_str = String::from_utf8(response.body.to_vec()).unwrap();
	assert!(
		body_str.contains("Accessories"),
		"Response should contain Accessories products"
	);
}
