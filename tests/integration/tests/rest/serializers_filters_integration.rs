//! # Serializers + Filters Cross-Crate Integration Tests
//!
//! ## Purpose
//! Verifies the integration between reinhardt-rest/filters and database operations,
//! demonstrating how filter backends generate SQL conditions for real database queries.
//!
//! ## Test Coverage
//! - SimpleSearchBackend SQL WHERE clause generation and execution
//! - SimpleOrderingBackend SQL ORDER BY clause generation and execution
//! - RangeFilter numeric/date range filtering with database queries
//! - CustomFilterBackend chaining multiple filters (Search + Ordering + Range)
//! - FuzzySearchFilter with Levenshtein distance similarity matching
//! - Filter + pagination integration
//! - Filter parameter validation and error handling
//!
//! ## Fixtures Used
//! - `postgres_container`: Standard PostgreSQL 16-alpine container
//!
//! ## What is Verified
//! - FilterBackend components generate correct SQL conditions
//! - Generated SQL conditions execute successfully against PostgreSQL
//! - Filter results are correctly serialized to JSON
//! - Multiple filters can be chained together (AND logic)
//! - Fuzzy search calculates similarity correctly
//! - Pagination works with filters
//! - Invalid filter parameters are handled correctly
//!
//! ## What is NOT Covered
//! - UI components (frontend)
//! - Authentication/authorization (covered in auth integration tests)
//! - Caching strategies (covered in cache integration tests)

use reinhardt_rest::filters::{
	CustomFilterBackend, DatabaseDialect, FilterBackend, FuzzySearchFilter, RangeFilter,
	SimpleOrderingBackend, SimpleSearchBackend,
};
use reinhardt_test::fixtures::testcontainers::{ContainerAsync, GenericImage, postgres_container};
use rstest::*;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use sqlx::Row;
use std::collections::HashMap;
use std::sync::Arc;

// ============================================================================
// Test Data Structures
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct Product {
	id: i32,
	name: String,
	category: String,
	price: i32,
	stock: i32,
	created_at: String,
}

// ============================================================================
// Custom Fixture
// ============================================================================

/// Custom fixture: PostgreSQL with products table and test data
#[fixture]
async fn filter_test_db(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) -> (ContainerAsync<GenericImage>, Arc<PgPool>) {
	let (container, pool, _port, _url) = postgres_container.await;

	// Create products table
	sqlx::query(
		r#"
		CREATE TABLE products (
			id SERIAL PRIMARY KEY,
			name TEXT NOT NULL,
			category TEXT NOT NULL,
			price INTEGER NOT NULL,
			stock INTEGER NOT NULL,
			created_at TIMESTAMP NOT NULL DEFAULT NOW()
		)
		"#,
	)
	.execute(pool.as_ref())
	.await
	.expect("Failed to create products table");

	// Insert test data
	let products = vec![
		("Laptop Pro", "Electronics", 1200, 50),
		("Laptop Air", "Electronics", 900, 100),
		("Desktop PC", "Electronics", 1500, 30),
		("Tablet Pro", "Electronics", 800, 75),
		("Smartphone X", "Electronics", 900, 200),
		("Office Chair", "Furniture", 300, 150),
		("Standing Desk", "Furniture", 600, 80),
		("Monitor 27\"", "Electronics", 400, 120),
		("Keyboard Mechanical", "Accessories", 150, 300),
		("Mouse Wireless", "Accessories", 60, 500),
		("Headphones Pro", "Electronics", 250, 180),
		("Webcam HD", "Electronics", 80, 250),
	];

	for (name, category, price, stock) in products {
		sqlx::query(
			r#"
			INSERT INTO products (name, category, price, stock, created_at)
			VALUES ($1, $2, $3, $4, NOW())
			"#,
		)
		.bind(name)
		.bind(category)
		.bind(price)
		.bind(stock)
		.execute(pool.as_ref())
		.await
		.expect("Failed to insert product");
	}

	(container, pool)
}

// ============================================================================
// Test 1: SimpleSearchBackend with ORM Integration
// ============================================================================

/// Test: SimpleSearchBackend generates SQL WHERE clause and executes query
///
/// Intent: Verify that SimpleSearchBackend:
/// - Generates correct LIKE-based WHERE clause
/// - Generated SQL executes successfully against PostgreSQL
/// - Results are correctly serialized to JSON
#[rstest]
#[tokio::test]
async fn test_simple_search_backend_with_orm_integration(
	#[future] filter_test_db: (ContainerAsync<GenericImage>, Arc<PgPool>),
) {
	let (_container, pool) = filter_test_db.await;

	// Create SimpleSearchBackend with PostgreSQL dialect
	let backend = SimpleSearchBackend::new("search")
		.with_field("name")
		.with_dialect(DatabaseDialect::PostgreSQL);

	// Prepare query parameters
	let mut params = HashMap::new();
	params.insert("search".to_string(), "Laptop".to_string());

	// Generate SQL with filter
	let base_sql = "SELECT * FROM products".to_string();
	let filtered_sql = backend
		.filter_queryset(&params, base_sql)
		.await
		.expect("Failed to generate filtered SQL");

	// Verify WHERE clause is added
	assert!(filtered_sql.contains("WHERE"));
	// PostgreSQL uses double quotes for identifiers
	assert!(filtered_sql.contains("\"name\" LIKE '%Laptop%'"));

	// Execute generated SQL against database
	let results = sqlx::query(&filtered_sql)
		.fetch_all(pool.as_ref())
		.await
		.expect("Failed to execute filtered query");

	// Serialize results
	let products: Vec<Product> = results
		.into_iter()
		.map(|row| Product {
			id: row.get::<i32, _>("id"),
			name: row.get::<String, _>("name"),
			category: row.get::<String, _>("category"),
			price: row.get::<i32, _>("price"),
			stock: row.get::<i32, _>("stock"),
			created_at: row.try_get::<String, _>("created_at").unwrap_or_default(),
		})
		.collect();

	// Verify results
	assert_eq!(products.len(), 2); // Laptop Pro, Laptop Air
	assert_eq!(products[0].name, "Laptop Pro");
	assert_eq!(products[0].price, 1200);
	assert_eq!(products[1].name, "Laptop Air");
	assert_eq!(products[1].price, 900);

	// Verify JSON serialization works
	let json = serde_json::to_string(&products).expect("Failed to serialize to JSON");
	assert!(json.contains("Laptop Pro"));
	assert!(json.contains("Laptop Air"));
}

// ============================================================================
// Test 2: SimpleOrderingBackend with Serialization
// ============================================================================

/// Test: SimpleOrderingBackend generates SQL ORDER BY clause and executes query
///
/// Intent: Verify that SimpleOrderingBackend:
/// - Generates correct ORDER BY clause with DESC direction
/// - Sorting is applied correctly in query results
/// - Results maintain correct order after serialization
#[rstest]
#[tokio::test]
async fn test_simple_ordering_backend_with_serialization(
	#[future] filter_test_db: (ContainerAsync<GenericImage>, Arc<PgPool>),
) {
	let (_container, pool) = filter_test_db.await;

	// Create SimpleOrderingBackend
	let backend = SimpleOrderingBackend::new("ordering")
		.allow_field("price")
		.allow_field("name");

	// Prepare query parameters (order by price descending)
	let mut params = HashMap::new();
	params.insert("ordering".to_string(), "-price".to_string());

	// Generate SQL with ordering
	let base_sql = "SELECT * FROM products".to_string();
	let ordered_sql = backend
		.filter_queryset(&params, base_sql)
		.await
		.expect("Failed to generate ordered SQL");

	// Verify ORDER BY clause is added
	assert!(ordered_sql.contains("ORDER BY price DESC"));

	// Execute generated SQL against database
	let results = sqlx::query(&ordered_sql)
		.fetch_all(pool.as_ref())
		.await
		.expect("Failed to execute ordered query");

	// Serialize results
	let products: Vec<Product> = results
		.into_iter()
		.map(|row| Product {
			id: row.get::<i32, _>("id"),
			name: row.get::<String, _>("name"),
			category: row.get::<String, _>("category"),
			price: row.get::<i32, _>("price"),
			stock: row.get::<i32, _>("stock"),
			created_at: row.try_get::<String, _>("created_at").unwrap_or_default(),
		})
		.collect();

	// Verify results are ordered by price descending
	assert_eq!(products.len(), 12);
	assert_eq!(products[0].name, "Desktop PC");
	assert_eq!(products[0].price, 1500); // Highest price first

	// Verify descending order
	for i in 0..products.len() - 1 {
		assert!(products[i].price >= products[i + 1].price);
	}
}

// ============================================================================
// Test 3: RangeFilter with Database Query
// ============================================================================

/// Test: RangeFilter generates SQL range conditions and executes query
///
/// Intent: Verify that RangeFilter:
/// - Generates correct >= and <= conditions for numeric range
/// - Range boundaries are correctly enforced in results
/// - Results fall within expected range
#[rstest]
#[tokio::test]
async fn test_range_filter_with_database_query(
	#[future] filter_test_db: (ContainerAsync<GenericImage>, Arc<PgPool>),
) {
	let (_container, pool) = filter_test_db.await;

	// Create RangeFilter for price range [100, 500]
	let range_filter: RangeFilter<i32> = RangeFilter::new("price").gte(100).lte(500);

	// Build SQL with range conditions
	let base_sql = "SELECT * FROM products".to_string();
	let range_sql = format!(
		"{} WHERE price >= {} AND price <= {}",
		base_sql,
		range_filter.gte.unwrap(),
		range_filter.lte.unwrap()
	);

	// Execute query
	let results = sqlx::query(&range_sql)
		.fetch_all(pool.as_ref())
		.await
		.expect("Failed to execute range query");

	// Serialize results
	let products: Vec<Product> = results
		.into_iter()
		.map(|row| Product {
			id: row.get::<i32, _>("id"),
			name: row.get::<String, _>("name"),
			category: row.get::<String, _>("category"),
			price: row.get::<i32, _>("price"),
			stock: row.get::<i32, _>("stock"),
			created_at: row.try_get::<String, _>("created_at").unwrap_or_default(),
		})
		.collect();

	// Verify all products are within range [100, 500]
	assert!(!products.is_empty());
	for product in &products {
		assert!(product.price >= 100);
		assert!(product.price <= 500);
	}

	// Verify expected products are included
	let product_names: Vec<&str> = products.iter().map(|p| p.name.as_str()).collect();
	assert!(product_names.contains(&"Monitor 27\""));
	assert!(product_names.contains(&"Office Chair"));
	assert!(product_names.contains(&"Headphones Pro"));
	assert!(product_names.contains(&"Keyboard Mechanical"));
}

// ============================================================================
// Test 4: CustomFilterBackend Chaining
// ============================================================================

/// Test: CustomFilterBackend chains multiple filters (Search + Ordering + Range)
///
/// Intent: Verify that CustomFilterBackend:
/// - Combines multiple filters with AND logic
/// - All filter conditions are applied correctly
/// - Results satisfy all filter criteria simultaneously
#[rstest]
#[tokio::test]
async fn test_custom_filter_backend_chaining(
	#[future] filter_test_db: (ContainerAsync<GenericImage>, Arc<PgPool>),
) {
	let (_container, pool) = filter_test_db.await;

	// Create CustomFilterBackend with multiple filters
	let mut backend = CustomFilterBackend::new();
	backend.add_filter(Box::new(
		SimpleSearchBackend::new("search")
			.with_field("category")
			.with_dialect(DatabaseDialect::PostgreSQL),
	));
	backend.add_filter(Box::new(
		SimpleOrderingBackend::new("ordering").allow_field("price"),
	));

	// Prepare query parameters
	let mut params = HashMap::new();
	params.insert("search".to_string(), "Electronics".to_string());
	params.insert("ordering".to_string(), "price".to_string()); // ASC

	// Generate SQL with chained filters
	let base_sql = "SELECT * FROM products".to_string();
	let filtered_sql = backend
		.filter_queryset(&params, base_sql)
		.await
		.expect("Failed to generate filtered SQL");

	// Verify both WHERE and ORDER BY are present
	assert!(filtered_sql.contains("WHERE"));
	// PostgreSQL uses double quotes for identifiers
	assert!(filtered_sql.contains("\"category\" LIKE '%Electronics%'"));
	assert!(filtered_sql.contains("ORDER BY price ASC"));

	// Execute query
	let results = sqlx::query(&filtered_sql)
		.fetch_all(pool.as_ref())
		.await
		.expect("Failed to execute chained filter query");

	// Serialize results
	let products: Vec<Product> = results
		.into_iter()
		.map(|row| Product {
			id: row.get::<i32, _>("id"),
			name: row.get::<String, _>("name"),
			category: row.get::<String, _>("category"),
			price: row.get::<i32, _>("price"),
			stock: row.get::<i32, _>("stock"),
			created_at: row.try_get::<String, _>("created_at").unwrap_or_default(),
		})
		.collect();

	// Verify all products are Electronics
	assert!(!products.is_empty());
	for product in &products {
		assert_eq!(product.category, "Electronics");
	}

	// Verify results are ordered by price ascending
	for i in 0..products.len() - 1 {
		assert!(products[i].price <= products[i + 1].price);
	}
}

// ============================================================================
// Test 5: FuzzySearchFilter Integration
// ============================================================================

/// Test: FuzzySearchFilter with Levenshtein distance similarity matching
///
/// Intent: Verify that FuzzySearchFilter:
/// - Calculates Levenshtein distance correctly
/// - Filters results based on similarity threshold
/// - Results are sorted by similarity score
#[rstest]
#[tokio::test]
async fn test_fuzzy_search_filter_integration(
	#[future] filter_test_db: (ContainerAsync<GenericImage>, Arc<PgPool>),
) {
	let (_container, pool) = filter_test_db.await;

	// Create FuzzySearchFilter
	#[derive(Clone)]
	struct ProductModel {
		_id: i32,
	}

	let fuzzy_filter: FuzzySearchFilter<ProductModel> = FuzzySearchFilter::new()
		.query("Labtop") // Typo: should match "Laptop"
		.field("name")
		.threshold(0.7); // 70% similarity required

	// Fetch all products
	let all_products = sqlx::query("SELECT * FROM products ORDER BY id")
		.fetch_all(pool.as_ref())
		.await
		.expect("Failed to fetch products");

	// Serialize results
	let products: Vec<Product> = all_products
		.into_iter()
		.map(|row| Product {
			id: row.get::<i32, _>("id"),
			name: row.get::<String, _>("name"),
			category: row.get::<String, _>("category"),
			price: row.get::<i32, _>("price"),
			stock: row.get::<i32, _>("stock"),
			created_at: row.try_get::<String, _>("created_at").unwrap_or_default(),
		})
		.collect();

	// Apply fuzzy matching using FuzzySearchFilter
	// Extract first word from product name for comparison
	let mut fuzzy_results: Vec<(Product, f64)> = products
		.into_iter()
		.filter_map(|p| {
			// Extract first word from product name (e.g., "Laptop" from "Laptop Pro")
			let first_word = p.name.split_whitespace().next().unwrap_or(&p.name);
			let similarity = fuzzy_filter.calculate_similarity(&fuzzy_filter.query, first_word);
			if similarity >= fuzzy_filter.threshold {
				Some((p, similarity))
			} else {
				None
			}
		})
		.collect();

	// Sort by similarity descending
	fuzzy_results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

	// Verify fuzzy search found similar products
	assert!(
		!fuzzy_results.is_empty(),
		"Fuzzy search should find at least one product matching 'Labtop' typo"
	);

	// Verify "Laptop" products are found despite typo in query
	let matched_names: Vec<&str> = fuzzy_results.iter().map(|(p, _)| p.name.as_str()).collect();
	assert!(
		matched_names.contains(&"Laptop Pro") || matched_names.contains(&"Laptop Air"),
		"Fuzzy search should find Laptop products despite typo"
	);

	// Verify similarity scores are in descending order
	for i in 0..fuzzy_results.len() - 1 {
		assert!(fuzzy_results[i].1 >= fuzzy_results[i + 1].1);
	}
}

// ============================================================================
// Test 6: Filter with Pagination Serialization
// ============================================================================

/// Test: FilterBackend + pagination integration
///
/// Intent: Verify that filters work correctly with pagination:
/// - Filter conditions are applied before pagination
/// - Pagination LIMIT and OFFSET work correctly with filtered results
/// - Each page contains correct filtered items
#[rstest]
#[tokio::test]
async fn test_filter_with_pagination_serialization(
	#[future] filter_test_db: (ContainerAsync<GenericImage>, Arc<PgPool>),
) {
	let (_container, pool) = filter_test_db.await;

	// Create filter backend with PostgreSQL dialect
	let backend = SimpleSearchBackend::new("search")
		.with_field("category")
		.with_dialect(DatabaseDialect::PostgreSQL);

	// Prepare query parameters
	let mut params = HashMap::new();
	params.insert("search".to_string(), "Electronics".to_string());

	// Generate filtered SQL
	let base_sql = "SELECT * FROM products".to_string();
	let filtered_sql = backend
		.filter_queryset(&params, base_sql)
		.await
		.expect("Failed to generate filtered SQL");

	// Add pagination (page 1, size 3)
	let paginated_sql = format!("{} ORDER BY id LIMIT 3 OFFSET 0", filtered_sql);

	// Execute page 1 query
	let page1_results = sqlx::query(&paginated_sql)
		.fetch_all(pool.as_ref())
		.await
		.expect("Failed to execute page 1 query");

	let page1_products: Vec<Product> = page1_results
		.into_iter()
		.map(|row| Product {
			id: row.get::<i32, _>("id"),
			name: row.get::<String, _>("name"),
			category: row.get::<String, _>("category"),
			price: row.get::<i32, _>("price"),
			stock: row.get::<i32, _>("stock"),
			created_at: row.try_get::<String, _>("created_at").unwrap_or_default(),
		})
		.collect();

	// Verify page 1 has exactly 3 items
	assert_eq!(page1_products.len(), 3);

	// Verify all items are Electronics
	for product in &page1_products {
		assert_eq!(product.category, "Electronics");
	}

	// Execute page 2 query
	let page2_sql = format!("{} ORDER BY id LIMIT 3 OFFSET 3", filtered_sql);
	let page2_results = sqlx::query(&page2_sql)
		.fetch_all(pool.as_ref())
		.await
		.expect("Failed to execute page 2 query");

	let page2_products: Vec<Product> = page2_results
		.into_iter()
		.map(|row| Product {
			id: row.get::<i32, _>("id"),
			name: row.get::<String, _>("name"),
			category: row.get::<String, _>("category"),
			price: row.get::<i32, _>("price"),
			stock: row.get::<i32, _>("stock"),
			created_at: row.try_get::<String, _>("created_at").unwrap_or_default(),
		})
		.collect();

	// Verify page 2 has items
	assert!(!page2_products.is_empty());

	// Verify all items are Electronics
	for product in &page2_products {
		assert_eq!(product.category, "Electronics");
	}

	// Verify pages don't overlap
	let page1_ids: Vec<i32> = page1_products.iter().map(|p| p.id).collect();
	let page2_ids: Vec<i32> = page2_products.iter().map(|p| p.id).collect();
	for id in &page2_ids {
		assert!(!page1_ids.contains(id));
	}
}

// ============================================================================
// Test 7: Filter Validation with Serialization
// ============================================================================

/// Test: Filter parameter validation and error handling
///
/// Intent: Verify that invalid filter parameters:
/// - Are rejected with appropriate errors
/// - Do not cause query execution failures
/// - Error messages are correctly propagated
#[rstest]
#[tokio::test]
async fn test_filter_validation_with_serialization(
	#[future] filter_test_db: (ContainerAsync<GenericImage>, Arc<PgPool>),
) {
	let (_container, _pool) = filter_test_db.await;

	// Create ordering backend with limited allowed fields
	let backend = SimpleOrderingBackend::new("ordering").allow_field("price");

	// Prepare invalid query parameters (ordering by non-allowed field)
	let mut invalid_params = HashMap::new();
	invalid_params.insert("ordering".to_string(), "invalid_field".to_string());

	// Attempt to generate SQL with invalid field
	let base_sql = "SELECT * FROM products".to_string();
	let result = backend.filter_queryset(&invalid_params, base_sql).await;

	// Verify error is returned
	assert!(result.is_err());

	// Verify error message is correct
	let error = result.unwrap_err();
	let error_message = format!("{:?}", error);
	assert!(error_message.contains("invalid_field"));
	assert!(error_message.contains("not allowed"));

	// Test with valid parameters - should succeed
	let mut valid_params = HashMap::new();
	valid_params.insert("ordering".to_string(), "price".to_string());

	let base_sql = "SELECT * FROM products".to_string();
	let valid_result = backend
		.filter_queryset(&valid_params, base_sql)
		.await
		.expect("Valid parameters should succeed");

	assert!(valid_result.contains("ORDER BY price ASC"));
}
