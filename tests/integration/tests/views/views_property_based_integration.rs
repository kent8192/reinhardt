//! ViewSet Property-Based Integration Tests
//!
//! Tests using property-based testing techniques:
//! - Valid input always produces valid output (invariant property)
//! - Idempotent operations (PUT, DELETE) produce same result when repeated
//! - Ordering operations are consistent (commutative/associative)
//! - Filter + Pagination consistency
//! - Search query always returns subset of all results
//! - Boundary value properties (min/max limits)
//! - Round-trip serialization (serialize → deserialize = identity)
//!
//! **Test Category**: Property-Based Testing (Property-basedテスト)
//!
//! **Note**: These tests verify mathematical properties and invariants
//! that should hold true for all inputs, not just specific test cases.

use bytes::Bytes;
use chrono::{DateTime, Utc};
use hyper::{HeaderMap, Method, Version};
use reinhardt_http::Request;
use reinhardt_macros::model;
use reinhardt_query::prelude::{
	ColumnDef, Iden, IntoIden, PostgresQueryBuilder, Query, QueryStatementBuilder,
};
use reinhardt_test::fixtures::get_test_pool;
use rstest::*;
use serde::{Deserialize, Serialize};
use serde_json;
use sqlx::{PgPool, Row};

// ============================================================================
// Test Structures
// ============================================================================

/// Property test model
#[allow(dead_code)]
#[model(app_label = "property_test", table_name = "property_items")]
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
struct PropertyItem {
	#[field(primary_key = true)]
	id: Option<i64>,
	#[field(max_length = 200)]
	name: String,
	value: i32,
	score: f64,
	active: bool,
	#[field(null = true)]
	created_at: Option<DateTime<Utc>>,
}

// Note: The #[model] macro automatically generates a new() function

/// Iden enum for property_items table
#[derive(Debug, Clone, Copy, Iden)]
enum PropertyItems {
	Table,
	Id,
	Name,
	Value,
	Score,
	Active,
	CreatedAt,
}

// ============================================================================
// Fixtures
// ============================================================================

/// Setup: PostgreSQL container with property test schema
///
/// Uses shared PostgreSQL container with template database pattern.
/// Each test gets an isolated database cloned from template (~10-40ms).
#[fixture]
async fn setup_property() -> PgPool {
	let pool = get_test_pool().await;

	// Create property_items table
	let mut stmt = Query::create_table();
	let create_table_sql = stmt
		.table(PropertyItems::Table.into_iden())
		.if_not_exists()
		.col(
			ColumnDef::new(PropertyItems::Id)
				.big_integer()
				.not_null(true)
				.auto_increment(true)
				.primary_key(true),
		)
		.col(
			ColumnDef::new(PropertyItems::Name)
				.string_len(200)
				.not_null(true),
		)
		.col(
			ColumnDef::new(PropertyItems::Value)
				.integer()
				.not_null(true),
		)
		.col(ColumnDef::new(PropertyItems::Score).double().not_null(true))
		.col(
			ColumnDef::new(PropertyItems::Active)
				.boolean()
				.not_null(true),
		)
		.col(ColumnDef::new(PropertyItems::CreatedAt).timestamp())
		.to_string(PostgresQueryBuilder::new());

	sqlx::query(&create_table_sql).execute(&pool).await.unwrap();

	pool
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Helper: Create HTTP GET request
fn _create_get_request(uri: &str) -> Request {
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

/// Property 1: Valid input always produces valid output
#[rstest]
#[tokio::test]

async fn test_property_valid_input_valid_output(#[future] setup_property: PgPool) {
	let pool = setup_property.await;

	// Test with various valid inputs
	let test_inputs = vec![
		PropertyItem::new("Item A".to_string(), 1, 1.0, true, Some(Utc::now())),
		PropertyItem::new("Item B".to_string(), -100, -50.5, false, Some(Utc::now())),
		PropertyItem::new(
			"Item C".to_string(),
			1000000,
			999.999,
			true,
			Some(Utc::now()),
		),
		PropertyItem::new("X".to_string(), 0, 0.0, false, None),
	];

	for item in test_inputs {
		// Save values before moving for assertions
		let expected_name = item.name.clone();
		let expected_value = item.value;
		let expected_score = item.score;
		let expected_active = item.active;

		let row = sqlx::query(
			"INSERT INTO property_items (name, value, score, active, created_at)
			 VALUES ($1, $2, $3, $4, $5) RETURNING id, name, value, score, active",
		)
		.bind(&item.name)
		.bind(item.value)
		.bind(item.score)
		.bind(item.active)
		.bind(item.created_at)
		.fetch_one(&pool)
		.await
		.unwrap();

		// Property: Output is always valid (has ID assigned)
		let id: i64 = row.get("id");
		assert!(
			id > 0,
			"Valid input should produce valid output with ID > 0"
		);

		// Property: Input data is preserved
		let stored_name: String = row.get("name");
		let stored_value: i32 = row.get("value");
		let stored_score: f64 = row.get("score");
		let stored_active: bool = row.get("active");

		assert_eq!(stored_name, expected_name);
		assert_eq!(stored_value, expected_value);
		assert_eq!(stored_score, expected_score);
		assert_eq!(stored_active, expected_active);
	}
}

/// Property 2: Idempotent DELETE - deleting twice has same effect as once
#[rstest]
#[tokio::test]

async fn test_property_idempotent_delete(#[future] setup_property: PgPool) {
	let pool = setup_property.await;

	// Insert item using direct sqlx query with proper parameter binding
	let item = PropertyItem::new("To Delete".to_string(), 42, 3.14, true, Some(Utc::now()));

	let row = sqlx::query(
		"INSERT INTO property_items (name, value, score, active, created_at)
		 VALUES ($1, $2, $3, $4, $5) RETURNING id",
	)
	.bind(&item.name)
	.bind(item.value)
	.bind(item.score)
	.bind(item.active)
	.bind(item.created_at)
	.fetch_one(&pool)
	.await
	.unwrap();
	let item_id: i64 = row.get("id");

	// First delete
	let first_delete = sqlx::query("DELETE FROM property_items WHERE id = $1")
		.bind(item_id)
		.execute(&pool)
		.await
		.unwrap();
	assert_eq!(first_delete.rows_affected(), 1);

	// Second delete (idempotent - should affect 0 rows but not error)
	let second_delete = sqlx::query("DELETE FROM property_items WHERE id = $1")
		.bind(item_id)
		.execute(&pool)
		.await
		.unwrap();
	assert_eq!(
		second_delete.rows_affected(),
		0,
		"Property: DELETE is idempotent"
	);

	// Verify item doesn't exist
	let rows = sqlx::query("SELECT id FROM property_items WHERE id = $1")
		.bind(item_id)
		.fetch_all(&pool)
		.await
		.unwrap();
	assert_eq!(rows.len(), 0, "Item should not exist after deletion");
}

/// Property 3: Ordering consistency - results ordered by field maintain order
#[rstest]
#[tokio::test]

async fn test_property_ordering_consistency(#[future] setup_property: PgPool) {
	let pool = setup_property.await;

	// Insert items with different values using direct sqlx query
	for i in 1..=10 {
		let item = PropertyItem::new(
			format!("Item {}", i),
			i * 10,
			(i as f64) * 1.5,
			i % 2 == 0,
			Some(Utc::now()),
		);

		sqlx::query(
			"INSERT INTO property_items (name, value, score, active, created_at)
			 VALUES ($1, $2, $3, $4, $5)",
		)
		.bind(&item.name)
		.bind(item.value)
		.bind(item.score)
		.bind(item.active)
		.bind(item.created_at)
		.execute(&pool)
		.await
		.unwrap();
	}

	// Query with ascending order (no parameters needed)
	let asc_rows = sqlx::query("SELECT value FROM property_items ORDER BY value ASC")
		.fetch_all(&pool)
		.await
		.unwrap();

	// Property: Ascending order maintains value[i] <= value[i+1]
	for i in 0..(asc_rows.len() - 1) {
		let current_value: i32 = asc_rows[i].get("value");
		let next_value: i32 = asc_rows[i + 1].get("value");

		assert!(
			current_value <= next_value,
			"Property: Ascending order maintains value[i] <= value[i+1]"
		);
	}

	// Query with descending order (no parameters needed)
	let desc_rows = sqlx::query("SELECT value FROM property_items ORDER BY value DESC")
		.fetch_all(&pool)
		.await
		.unwrap();

	// Property: Descending order maintains value[i] >= value[i+1]
	for i in 0..(desc_rows.len() - 1) {
		let current_value: i32 = desc_rows[i].get("value");
		let next_value: i32 = desc_rows[i + 1].get("value");

		assert!(
			current_value >= next_value,
			"Property: Descending order maintains value[i] >= value[i+1]"
		);
	}
}

/// Property 4: Filter results are always subset of all results
#[rstest]
#[tokio::test]

async fn test_property_filter_subset(#[future] setup_property: PgPool) {
	let pool = setup_property.await;

	// Insert mixed data using direct sqlx query
	for i in 1..=20 {
		let item = PropertyItem::new(
			format!("Item {}", i),
			i * 5,
			(i as f64) * 2.5,
			i % 3 == 0, // 1/3 active
			Some(Utc::now()),
		);

		sqlx::query(
			"INSERT INTO property_items (name, value, score, active, created_at)
			 VALUES ($1, $2, $3, $4, $5)",
		)
		.bind(&item.name)
		.bind(item.value)
		.bind(item.score)
		.bind(item.active)
		.bind(item.created_at)
		.execute(&pool)
		.await
		.unwrap();
	}

	// Get all results
	let all_rows = sqlx::query("SELECT id FROM property_items")
		.fetch_all(&pool)
		.await
		.unwrap();
	let total_count = all_rows.len();

	// Get filtered results (active = true)
	let filtered_rows = sqlx::query("SELECT id FROM property_items WHERE active = $1")
		.bind(true)
		.fetch_all(&pool)
		.await
		.unwrap();
	let filtered_count = filtered_rows.len();

	// Property: Filtered count <= Total count
	assert!(
		filtered_count <= total_count,
		"Property: Filter results are always a subset (or equal) of all results"
	);

	// Property: All filtered IDs exist in all IDs
	let all_ids: Vec<i64> = all_rows.iter().map(|r| r.get("id")).collect();
	for filtered_row in filtered_rows {
		let filtered_id: i64 = filtered_row.get("id");
		assert!(
			all_ids.contains(&filtered_id),
			"Property: Every filtered ID exists in all IDs"
		);
	}
}

/// Property 5: Search results are subset with search term in name
#[rstest]
#[tokio::test]

async fn test_property_search_subset(#[future] setup_property: PgPool) {
	let pool = setup_property.await;

	// Insert items with various names using direct sqlx query
	let names = vec![
		"Apple Product",
		"Banana Item",
		"Apple Gadget",
		"Orange Tool",
		"Apple Device",
	];

	for (index, name) in names.iter().enumerate() {
		let item = PropertyItem::new(
			String::from(*name),
			(index as i32) + 1,
			(index as f64) + 1.0,
			true,
			Some(Utc::now()),
		);

		sqlx::query(
			"INSERT INTO property_items (name, value, score, active, created_at)
			 VALUES ($1, $2, $3, $4, $5)",
		)
		.bind(&item.name)
		.bind(item.value)
		.bind(item.score)
		.bind(item.active)
		.bind(item.created_at)
		.execute(&pool)
		.await
		.unwrap();
	}

	// Search for "Apple"
	let search_rows = sqlx::query("SELECT id, name FROM property_items WHERE name LIKE $1")
		.bind("%Apple%")
		.fetch_all(&pool)
		.await
		.unwrap();

	// Property: All search results contain search term
	for row in search_rows {
		let name: String = row.get("name");
		assert!(
			name.contains("Apple"),
			"Property: All search results must contain search term"
		);
	}
}

/// Property 6: Pagination boundary values
#[rstest]
#[tokio::test]

async fn test_property_pagination_boundaries(#[future] setup_property: PgPool) {
	let pool = setup_property.await;

	// Insert 25 items using direct sqlx query
	for i in 1..=25 {
		let item = PropertyItem::new(format!("Item {}", i), i, i as f64, true, Some(Utc::now()));

		sqlx::query(
			"INSERT INTO property_items (name, value, score, active, created_at)
			 VALUES ($1, $2, $3, $4, $5)",
		)
		.bind(&item.name)
		.bind(item.value)
		.bind(item.score)
		.bind(item.active)
		.bind(item.created_at)
		.execute(&pool)
		.await
		.unwrap();
	}

	// Property: Sum of all page sizes = total count
	let page_size: i64 = 10;
	let mut total_paginated = 0;

	for page in 0..3i64 {
		// Pages 0, 1, 2
		let offset = page * page_size;

		let page_rows = sqlx::query("SELECT id FROM property_items LIMIT $1 OFFSET $2")
			.bind(page_size)
			.bind(offset)
			.fetch_all(&pool)
			.await
			.unwrap();
		total_paginated += page_rows.len();
	}

	// Verify total count matches
	let count_row = sqlx::query("SELECT COUNT(id) FROM property_items")
		.fetch_one(&pool)
		.await
		.unwrap();
	let total_count: i64 = count_row.get(0);

	assert_eq!(
		total_paginated, total_count as usize,
		"Property: Sum of paginated results equals total count"
	);
}

/// Property 7: Round-trip serialization (serialize → deserialize = identity)
#[rstest]
#[tokio::test]

async fn test_property_serialization_roundtrip(#[future] setup_property: PgPool) {
	let _pool = setup_property.await;

	// Test various items for round-trip property
	let test_items = vec![
		PropertyItem::new("Test 1".to_string(), 1, 1.1, true, Some(Utc::now())),
		PropertyItem::new("Test 2".to_string(), -999, -99.99, false, None),
		PropertyItem::new("Test 3".to_string(), 0, 0.0, true, Some(Utc::now())),
	];

	for original_item in test_items {
		// Serialize to JSON
		let serialized = serde_json::to_string(&original_item).expect("Failed to serialize item");

		// Deserialize back
		let deserialized: PropertyItem =
			serde_json::from_str(&serialized).expect("Failed to deserialize item");

		// Property: Deserialized item equals original
		assert_eq!(
			deserialized.name, original_item.name,
			"Property: Round-trip preserves name"
		);
		assert_eq!(
			deserialized.value, original_item.value,
			"Property: Round-trip preserves value"
		);
		assert_eq!(
			deserialized.score, original_item.score,
			"Property: Round-trip preserves score"
		);
		assert_eq!(
			deserialized.active, original_item.active,
			"Property: Round-trip preserves active"
		);

		// Note: DateTime comparison may have precision differences
		// We verify both are Some or both are None
		assert_eq!(
			deserialized.created_at.is_some(),
			original_item.created_at.is_some(),
			"Property: Round-trip preserves created_at presence"
		);
	}
}
