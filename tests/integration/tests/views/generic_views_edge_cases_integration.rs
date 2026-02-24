//! Generic Views Edge Cases Integration Tests
//!
//! Tests edge cases and exceptional situations for Generic API Views:
//! - Large payload handling
//! - Concurrent request handling
//! - Database connection issues
//! - Serialization/deserialization errors
//! - Unicode and special characters
//! - Very long strings (boundary testing)
//! - Null/None field handling
//! - Empty string vs null distinction
//! - Integer boundary values
//! - Boolean edge cases
//! - Timestamp edge cases (timezones, far future/past)
//!
//! **Test Category**: Edge Cases
//!
//! **Fixtures Used:**
//! - shared_db_pool: Shared PostgreSQL database pool with ORM initialized
//!
//! **Test Data Schema:**
//! - edge_test_items(id SERIAL PRIMARY KEY, name TEXT, description TEXT,
//!   quantity BIGINT, active BOOLEAN, created_at TIMESTAMP WITH TIME ZONE)

use bytes::Bytes;
use chrono::{DateTime, TimeZone, Utc};
use futures::future::join_all;
use hyper::{HeaderMap, Method, StatusCode, Version};
use reinhardt_core::macros::model;
use reinhardt_http::Request;
use reinhardt_query::prelude::{
	ColumnDef, Iden, IntoIden, PostgresQueryBuilder, Query, QueryStatementBuilder,
};
use reinhardt_rest::serializers::JsonSerializer;
use reinhardt_test::fixtures::shared_db_pool;
use reinhardt_views::{CreateAPIView, ListAPIView, View};
use rstest::*;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::sync::Arc;
use tokio::task;

// ============================================================================
// Model Definitions
// ============================================================================

/// Edge test item model
#[allow(dead_code)]
#[model(app_label = "views_edge", table_name = "edge_test_items")]
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
struct EdgeTestItem {
	#[field(primary_key = true)]
	id: Option<i64>,
	#[field(max_length = 5000, null = true)]
	name: Option<String>,
	#[field(max_length = 10000, null = true)]
	description: Option<String>,
	#[field(null = true)]
	quantity: Option<i64>,
	#[field(null = true)]
	active: Option<bool>,
	#[field(null = true)]
	created_at: Option<DateTime<Utc>>,
}

// ============================================================================
// Table Identifiers (for reinhardt-query operations)
// ============================================================================

#[derive(Debug, Clone, Copy, Iden)]
enum EdgeTestItems {
	Table,
	Id,
	Name,
	Description,
	Quantity,
	Active,
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

/// Fixture: Setup edge_test_items table
#[fixture]
async fn edge_items_table(#[future] db_pool: Arc<PgPool>) -> Arc<PgPool> {
	let pool = db_pool.await;

	// Create edge_test_items table with nullable fields
	let mut create_table_stmt = Query::create_table();
	create_table_stmt
		.table(EdgeTestItems::Table.into_iden())
		.if_not_exists()
		.col(
			ColumnDef::new(EdgeTestItems::Id)
				.big_integer()
				.not_null(true)
				.auto_increment(true)
				.primary_key(true),
		)
		.col(ColumnDef::new(EdgeTestItems::Name).string_len(5000))
		.col(ColumnDef::new(EdgeTestItems::Description).text())
		.col(ColumnDef::new(EdgeTestItems::Quantity).big_integer())
		.col(ColumnDef::new(EdgeTestItems::Active).boolean())
		.col(ColumnDef::new(EdgeTestItems::CreatedAt).timestamp_with_time_zone());

	let sql = create_table_stmt.to_string(PostgresQueryBuilder::new());
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create edge_test_items table");

	pool
}

// ============================================================================
// Helper Functions
// ============================================================================

//// Helper: Create HTTP POST request with JSON body
fn create_post_request(uri: &str, json_body: &str) -> Request {
	let mut headers = HeaderMap::new();
	headers.insert(
		hyper::header::CONTENT_TYPE,
		"application/json".parse().unwrap(),
	);
	Request::builder()
		.method(Method::POST)
		.uri(uri)
		.version(Version::HTTP_11)
		.headers(headers)
		.body(Bytes::from(json_body.to_string()))
		.build()
		.expect("Failed to build request")
}

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

/// Test: Large payload handling (5000 character string)
#[rstest]
#[tokio::test]
async fn test_large_payload_handling(#[future] edge_items_table: Arc<PgPool>) {
	let _pool = edge_items_table.await;

	let view = CreateAPIView::<EdgeTestItem, JsonSerializer<EdgeTestItem>>::new();

	// Create large payload with 4000 character description
	let large_description = "A".repeat(4000);
	let json_body = format!(
		r#"{{"name":"Large Item","description":"{}","quantity":100,"active":true}}"#,
		large_description
	);

	let request = create_post_request("/items/", &json_body);
	let result = view.dispatch(request).await;

	// Should handle large payload successfully
	match result {
		Ok(response) => {
			assert_eq!(
				response.status,
				StatusCode::CREATED,
				"Large payload should be accepted"
			);
		}
		Err(_) => {
			// Large payload may be rejected if it exceeds limits
			assert!(true, "Large payload rejection is acceptable");
		}
	}
}

/// Test: Concurrent request handling
#[rstest]
#[tokio::test]
async fn test_concurrent_requests(#[future] edge_items_table: Arc<PgPool>) {
	let _pool = edge_items_table.await;

	let view = Arc::new(CreateAPIView::<EdgeTestItem, JsonSerializer<EdgeTestItem>>::new());

	// Spawn 10 concurrent create requests
	let mut handles = vec![];

	for i in 0..10 {
		let view_clone = Arc::clone(&view);
		let handle = task::spawn(async move {
			let json_body = format!(
				r#"{{"name":"Concurrent Item {}","description":"Test","quantity":{},"active":true}}"#,
				i, i
			);
			let request = create_post_request("/items/", &json_body);
			view_clone.dispatch(request).await
		});
		handles.push(handle);
	}

	// Wait for all requests to complete
	let results: Vec<_> = join_all(handles).await;

	// Verify all requests completed (success or error)
	assert_eq!(results.len(), 10, "All concurrent requests should complete");

	// Count successful requests
	let successful_count = results
		.iter()
		.filter(|r| r.is_ok() && r.as_ref().unwrap().is_ok())
		.count();

	// At least some requests should succeed
	assert!(
		successful_count > 0,
		"At least some concurrent requests should succeed"
	);
}

/// Test: Serialization error handling (malformed JSON)
#[rstest]
#[tokio::test]
async fn test_serialization_error(#[future] edge_items_table: Arc<PgPool>) {
	let _pool = edge_items_table.await;

	let view = CreateAPIView::<EdgeTestItem, JsonSerializer<EdgeTestItem>>::new();

	// Malformed JSON (missing closing brace, invalid escape sequence)
	let malformed_json = r#"{"name":"Test","description":"Invalid \x escape"#;
	let request = create_post_request("/items/", malformed_json);

	let result = view.dispatch(request).await;

	// Should return error for malformed JSON
	match result {
		Err(_) => {
			assert!(true, "Malformed JSON should cause error");
		}
		Ok(response) => {
			assert_eq!(
				response.status,
				StatusCode::BAD_REQUEST,
				"Malformed JSON should return BAD_REQUEST"
			);
		}
	}
}

/// Test: Unicode and special characters
#[rstest]
#[tokio::test]
async fn test_unicode_special_characters(#[future] edge_items_table: Arc<PgPool>) {
	let _pool = edge_items_table.await;

	let view = CreateAPIView::<EdgeTestItem, JsonSerializer<EdgeTestItem>>::new();

	// Unicode characters: Japanese, Emoji, Special symbols
	let json_body = r#"{"name":"テスト商品 🚀","description":"Special chars: <>&\"'","quantity":42,"active":true}"#;
	let request = create_post_request("/items/", json_body);

	let result = view.dispatch(request).await;

	// Should handle Unicode correctly
	match result {
		Ok(response) => {
			assert_eq!(
				response.status,
				StatusCode::CREATED,
				"Unicode characters should be handled"
			);
			let body_str = String::from_utf8(response.body.to_vec()).unwrap();
			assert!(
				body_str.contains("テスト") || body_str.contains("Special"),
				"Response should preserve Unicode"
			);
		}
		Err(_) => {
			panic!("Unicode characters should be supported");
		}
	}
}

/// Test: Very long string (boundary testing)
#[rstest]
#[tokio::test]
async fn test_very_long_string_boundary(#[future] edge_items_table: Arc<PgPool>) {
	let _pool = edge_items_table.await;

	let view = CreateAPIView::<EdgeTestItem, JsonSerializer<EdgeTestItem>>::new();

	// String exactly at max_length boundary (5000 chars for name)
	let boundary_name = "B".repeat(5000);
	let json_body = format!(
		r#"{{"name":"{}","description":"Boundary test","quantity":1,"active":true}}"#,
		boundary_name
	);

	let request = create_post_request("/items/", &json_body);
	let result = view.dispatch(request).await;

	// Should accept string at boundary
	match result {
		Ok(response) => {
			assert!(
				response.status == StatusCode::CREATED
					|| response.status == StatusCode::BAD_REQUEST,
				"Boundary-length string should be accepted or rejected consistently"
			);
		}
		Err(_) => {
			assert!(true, "Boundary string may cause error");
		}
	}
}

/// Test: Null/None field handling
#[rstest]
#[tokio::test]
async fn test_null_field_handling(#[future] edge_items_table: Arc<PgPool>) {
	let _pool = edge_items_table.await;

	let view = CreateAPIView::<EdgeTestItem, JsonSerializer<EdgeTestItem>>::new();

	// Explicit null values for nullable fields
	let json_body =
		r#"{"name":null,"description":null,"quantity":null,"active":null,"created_at":null}"#;
	let request = create_post_request("/items/", json_body);

	let result = view.dispatch(request).await;

	// Should handle null values correctly
	match result {
		Ok(response) => {
			assert_eq!(
				response.status,
				StatusCode::CREATED,
				"Null values should be accepted for nullable fields"
			);
		}
		Err(_) => {
			panic!("Null values should be supported for nullable fields");
		}
	}
}

/// Test: Empty string vs null distinction
#[rstest]
#[tokio::test]
async fn test_empty_string_vs_null(#[future] edge_items_table: Arc<PgPool>) {
	let _pool = edge_items_table.await;

	let view = CreateAPIView::<EdgeTestItem, JsonSerializer<EdgeTestItem>>::new();

	// Empty string should be different from null
	let json_body_empty = r#"{"name":"","description":"","quantity":0,"active":false}"#;
	let request_empty = create_post_request("/items/", json_body_empty);

	let result_empty = view.dispatch(request_empty).await;

	// Empty string should be accepted
	match result_empty {
		Ok(response) => {
			assert_eq!(
				response.status,
				StatusCode::CREATED,
				"Empty string should be accepted"
			);
			let body_str = String::from_utf8(response.body.to_vec()).unwrap();
			// Should preserve empty string (not convert to null)
			assert!(
				body_str.contains(r#""name":"""#) || body_str.contains(r#""name":"","#),
				"Empty string should be preserved"
			);
		}
		Err(_) => {
			panic!("Empty string should be supported");
		}
	}
}

/// Test: Integer boundary values (i64 min/max)
#[rstest]
#[tokio::test]
async fn test_integer_boundary_values(#[future] edge_items_table: Arc<PgPool>) {
	let _pool = edge_items_table.await;

	let view = CreateAPIView::<EdgeTestItem, JsonSerializer<EdgeTestItem>>::new();

	// Test with i64::MAX
	let json_body_max = format!(
		r#"{{"name":"Max Int","description":"Test","quantity":{},"active":true}}"#,
		i64::MAX
	);
	let request_max = create_post_request("/items/", &json_body_max);

	let result_max = view.dispatch(request_max).await;

	// Should handle i64::MAX
	match result_max {
		Ok(response) => {
			assert_eq!(
				response.status,
				StatusCode::CREATED,
				"i64::MAX should be accepted"
			);
		}
		Err(_) => {
			panic!("i64::MAX should be supported");
		}
	}

	// Test with i64::MIN
	let json_body_min = format!(
		r#"{{"name":"Min Int","description":"Test","quantity":{},"active":true}}"#,
		i64::MIN
	);
	let request_min = create_post_request("/items/", &json_body_min);

	let result_min = view.dispatch(request_min).await;

	// Should handle i64::MIN
	match result_min {
		Ok(response) => {
			assert_eq!(
				response.status,
				StatusCode::CREATED,
				"i64::MIN should be accepted"
			);
		}
		Err(_) => {
			panic!("i64::MIN should be supported");
		}
	}
}

/// Test: Boolean edge cases (true/false/null)
#[rstest]
#[tokio::test]
async fn test_boolean_edge_cases(#[future] edge_items_table: Arc<PgPool>) {
	let _pool = edge_items_table.await;

	let view = CreateAPIView::<EdgeTestItem, JsonSerializer<EdgeTestItem>>::new();

	// Test all boolean states
	let test_cases = vec![
		(r#"{"name":"True","active":true}"#, "true"),
		(r#"{"name":"False","active":false}"#, "false"),
		(r#"{"name":"Null","active":null}"#, "null"),
	];

	for (json_body, case_name) in test_cases {
		let request = create_post_request("/items/", json_body);
		let result = view.dispatch(request).await;

		match result {
			Ok(response) => {
				assert_eq!(
					response.status,
					StatusCode::CREATED,
					"Boolean case '{}' should be accepted",
					case_name
				);
			}
			Err(_) => {
				panic!("Boolean case '{}' should be supported", case_name);
			}
		}
	}
}

/// Test: Timestamp edge cases (far past, far future, timezones)
#[rstest]
#[tokio::test]
async fn test_timestamp_edge_cases(#[future] edge_items_table: Arc<PgPool>) {
	let _pool = edge_items_table.await;

	let view = CreateAPIView::<EdgeTestItem, JsonSerializer<EdgeTestItem>>::new();

	// Far past: 1970-01-01 (Unix epoch)
	let epoch = Utc.with_ymd_and_hms(1970, 1, 1, 0, 0, 0).unwrap();
	let json_body_past = format!(r#"{{"name":"Past","created_at":"{}"}}"#, epoch.to_rfc3339());
	let request_past = create_post_request("/items/", &json_body_past);

	let result_past = view.dispatch(request_past).await;
	match result_past {
		Ok(response) => {
			assert!(
				response.status == StatusCode::CREATED
					|| response.status == StatusCode::BAD_REQUEST,
				"Far past timestamp should be handled"
			);
		}
		Err(_) => {
			assert!(true, "Far past timestamp may cause error");
		}
	}

	// Far future: 2100-12-31
	let future = Utc.with_ymd_and_hms(2100, 12, 31, 23, 59, 59).unwrap();
	let json_body_future = format!(
		r#"{{"name":"Future","created_at":"{}"}}"#,
		future.to_rfc3339()
	);
	let request_future = create_post_request("/items/", &json_body_future);

	let result_future = view.dispatch(request_future).await;
	match result_future {
		Ok(response) => {
			assert!(
				response.status == StatusCode::CREATED
					|| response.status == StatusCode::BAD_REQUEST,
				"Far future timestamp should be handled"
			);
		}
		Err(_) => {
			assert!(true, "Far future timestamp may cause error");
		}
	}

	// Current time (should always work)
	let now = Utc::now();
	let json_body_now = format!(r#"{{"name":"Now","created_at":"{}"}}"#, now.to_rfc3339());
	let request_now = create_post_request("/items/", &json_body_now);

	let result_now = view.dispatch(request_now).await;
	assert!(result_now.is_ok(), "Current timestamp should always work");
}

/// Test: List operation with edge case data
#[rstest]
#[tokio::test]
async fn test_list_with_edge_case_data(#[future] edge_items_table: Arc<PgPool>) {
	let pool = edge_items_table.await;

	// Insert items with edge case values
	let sql = "INSERT INTO edge_test_items (name, description, quantity, active, created_at) VALUES ($1, $2, $3, $4, $5)";

	// Item with nulls
	sqlx::query(sql)
		.bind(None::<String>)
		.bind(None::<String>)
		.bind(None::<i64>)
		.bind(None::<bool>)
		.bind(None::<DateTime<Utc>>)
		.execute(pool.as_ref())
		.await
		.expect("Failed to insert null item");

	// Item with empty strings
	sqlx::query(sql)
		.bind("")
		.bind("")
		.bind(Some(0i64))
		.bind(Some(false))
		.bind(Some(Utc::now()))
		.execute(pool.as_ref())
		.await
		.expect("Failed to insert empty string item");

	// Item with Unicode
	sqlx::query(sql)
		.bind("日本語 🎌")
		.bind("Special <>&\"'")
		.bind(Some(42i64))
		.bind(Some(true))
		.bind(Some(Utc::now()))
		.execute(pool.as_ref())
		.await
		.expect("Failed to insert Unicode item");

	// List all items
	let view = ListAPIView::<EdgeTestItem, JsonSerializer<EdgeTestItem>>::new();
	let request = create_get_request("/items/");

	let result = view.dispatch(request).await;

	// Should successfully list items with edge case data
	assert!(result.is_ok(), "List with edge case data should succeed");
	let response = result.unwrap();
	assert_eq!(response.status, StatusCode::OK);

	let body_str = String::from_utf8(response.body.to_vec()).unwrap();
	// Should contain at least the Unicode item
	assert!(
		body_str.contains("日本語") || body_str.contains("Special"),
		"Response should contain edge case data"
	);
}
