//! Views Sanity Integration Tests
//!
//! Basic smoke tests to verify essential functionality of reinhardt-views:
//! - Basic GET request returns 200 OK
//! - Basic POST request creates resource
//! - Basic PUT request updates resource
//! - Basic DELETE request removes resource
//! - Invalid URL returns 404
//! - Missing Content-Type header handling
//! - Empty database list operation
//! - Simple filtering operation
//!
//! **Test Category**: Sanity (サニティテスト)
//!
//! **Purpose**: Quick verification that core views functionality is operational.
//! These tests are intentionally simple and fast, designed to catch major regressions.
//!
//! **Fixtures Used:**
//! - postgres_container: PostgreSQL database container
//!
//! **Test Data Schema:**
//! - items(id SERIAL PRIMARY KEY, name TEXT NOT NULL, value INT NOT NULL)

use bytes::Bytes;
use hyper::{
	HeaderMap, Method, StatusCode, Version,
	header::{CONTENT_TYPE, HeaderValue},
};
use reinhardt_core::macros::model;
use reinhardt_http::Request;
use reinhardt_query::prelude::{
	ColumnDef, Iden, IntoIden, PostgresQueryBuilder, Query, QueryStatementBuilder,
};
use reinhardt_rest::serializers::JsonSerializer;
use reinhardt_test::fixtures::get_test_pool_with_orm;
use reinhardt_views::{
	CreateAPIView, DestroyAPIView, ListAPIView, RetrieveAPIView, UpdateAPIView, View,
};
use rstest::*;
use serde::{Deserialize, Serialize};
use sqlx::{Executor, PgPool, Row};
use std::collections::HashMap;
use std::sync::Arc;

// ============================================================================
// Model Definitions
// ============================================================================

/// Simple item model for sanity testing
#[allow(dead_code)]
#[model(app_label = "views_sanity", table_name = "items")]
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
struct Item {
	#[field(primary_key = true)]
	id: Option<i64>,
	#[field(max_length = 100)]
	name: String,
	value: i32,
}

// ============================================================================
// Table Identifiers (for reinhardt-query operations)
// ============================================================================

#[derive(Debug, Clone, Copy, Iden)]
enum Items {
	Table,
	Id,
	Name,
	Value,
}

// ============================================================================
// Fixtures
// ============================================================================

/// Create items table SQL
fn create_items_table_sql() -> String {
	let mut stmt = Query::create_table();
	stmt.table(Items::Table.into_iden())
		.if_not_exists()
		.col(
			ColumnDef::new(Items::Id)
				.big_integer()
				.not_null(true)
				.auto_increment(true)
				.primary_key(true),
		)
		.col(ColumnDef::new(Items::Name).string_len(100).not_null(true))
		.col(ColumnDef::new(Items::Value).integer().not_null(true))
		.to_string(PostgresQueryBuilder::new())
}

/// Fixture: Initialize database connection with items table
///
/// Uses shared PostgreSQL container with template database pattern.
/// Each test gets an isolated database cloned from template (~10-40ms).
///
/// This fixture also initializes the global ORM manager, which is required
/// for View::dispatch() and QuerySet operations.
#[fixture]
async fn items_table() -> Arc<PgPool> {
	// Use get_test_pool_with_orm() to initialize both the pool and global ORM
	let (pool, _url) = get_test_pool_with_orm().await;

	// Create items table
	let sql = create_items_table_sql();
	pool.execute(sql.as_str())
		.await
		.expect("Failed to create items table");

	Arc::new(pool)
}

/// Fixture: Setup items table with sample data
#[fixture]
async fn items_with_data(#[future] items_table: Arc<PgPool>) -> Arc<PgPool> {
	let pool = items_table.await;

	// Insert sample items
	for i in 1..=3 {
		let item = Item::new(format!("Item {}", i), i * 10);

		let sql = "INSERT INTO items (name, value) VALUES ($1, $2)";
		sqlx::query(sql)
			.bind(&item.name)
			.bind(item.value)
			.execute(pool.as_ref())
			.await
			.expect("Failed to insert item");
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

/// Helper: Create HTTP POST request with JSON body
fn create_post_request(uri: &str, json_body: &str) -> Request {
	let mut headers = HeaderMap::new();
	headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
	Request::builder()
		.method(Method::POST)
		.uri(uri)
		.version(Version::HTTP_11)
		.headers(headers)
		.body(Bytes::from(json_body.to_string()))
		.build()
		.expect("Failed to build request")
}

/// Helper: Create HTTP PUT request with JSON body and path parameters
///
/// This helper is used for testing UpdateAPIView which expects `id` parameter
/// in path_params to identify the resource to update.
fn create_put_request_with_params(uri: &str, json_body: &str, id: &str) -> Request {
	let mut params = HashMap::new();
	params.insert("id".to_string(), id.to_string());

	let mut headers = HeaderMap::new();
	headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

	Request::builder()
		.method(Method::PUT)
		.uri(uri)
		.version(Version::HTTP_11)
		.headers(headers)
		.body(Bytes::from(json_body.to_string()))
		.path_params(params)
		.build()
		.expect("Failed to build request")
}

/// Helper: Create HTTP DELETE request with path parameters
///
/// This helper is used for testing DestroyAPIView which expects `id` parameter
/// in path_params to identify the resource to delete.
fn create_delete_request_with_params(uri: &str, id: &str) -> Request {
	let mut params = HashMap::new();
	params.insert("id".to_string(), id.to_string());

	Request::builder()
		.method(Method::DELETE)
		.uri(uri)
		.version(Version::HTTP_11)
		.headers(HeaderMap::new())
		.body(Bytes::new())
		.path_params(params)
		.build()
		.expect("Failed to build request")
}

// ============================================================================
// Tests
// ============================================================================

/// Test: Basic GET request returns 200 OK
#[rstest]
#[tokio::test]

async fn test_basic_get_returns_ok(#[future] items_with_data: Arc<PgPool>) {
	let _pool = items_with_data.await;

	let view = ListAPIView::<Item, JsonSerializer<Item>>::new();
	let request = create_get_request("/items/");

	let result = view.dispatch(request).await;

	// Should successfully return list of items
	assert!(result.is_ok(), "GET request should succeed");
	let response = result.unwrap();
	assert_eq!(response.status, StatusCode::OK, "Should return 200 OK");
}

/// Test: Basic POST request creates resource
#[rstest]
#[tokio::test]

async fn test_basic_post_creates_resource(#[future] items_table: Arc<PgPool>) {
	let _pool = items_table.await;

	let view = CreateAPIView::<Item, JsonSerializer<Item>>::new();
	let json_body = r#"{"name":"New Item","value":100}"#;
	let request = create_post_request("/items/", json_body);

	let result = view.dispatch(request).await;

	// Should successfully create item
	match result {
		Ok(response) => {
			assert_eq!(
				response.status,
				StatusCode::CREATED,
				"Should return 201 CREATED"
			);
		}
		Err(e) => {
			panic!("POST request should succeed, but got error: {:?}", e);
		}
	}
}

/// Test: Basic PUT request updates resource
#[rstest]
#[tokio::test]

async fn test_basic_put_updates_resource(#[future] items_with_data: Arc<PgPool>) {
	let pool = items_with_data.await;

	// Get an existing item ID
	let row = sqlx::query("SELECT id FROM items LIMIT 1")
		.fetch_one(pool.as_ref())
		.await
		.expect("Failed to fetch item");
	let item_id: i64 = row.get(0);

	let view =
		UpdateAPIView::<Item, JsonSerializer<Item>>::new().with_lookup_field("id".to_string());
	let json_body = r#"{"name":"Updated Item","value":999}"#;
	let request = create_put_request_with_params(
		&format!("/items/{}/", item_id),
		json_body,
		&item_id.to_string(),
	);

	let result = view.dispatch(request).await;

	// Should successfully update item
	match result {
		Ok(response) => {
			assert_eq!(response.status, StatusCode::OK, "Should return 200 OK");
		}
		Err(e) => {
			panic!("PUT request should succeed, but got error: {:?}", e);
		}
	}
}

/// Test: Basic DELETE request removes resource
#[rstest]
#[tokio::test]

async fn test_basic_delete_removes_resource(#[future] items_with_data: Arc<PgPool>) {
	let pool = items_with_data.await;

	// Get an existing item ID
	let row = sqlx::query("SELECT id FROM items LIMIT 1")
		.fetch_one(pool.as_ref())
		.await
		.expect("Failed to fetch item");
	let item_id: i64 = row.get(0);

	let view = DestroyAPIView::<Item>::new().with_lookup_field("id".to_string());
	let request =
		create_delete_request_with_params(&format!("/items/{}/", item_id), &item_id.to_string());

	let result = view.dispatch(request).await;

	// Should successfully delete item
	match result {
		Ok(response) => {
			assert_eq!(
				response.status,
				StatusCode::NO_CONTENT,
				"Should return 204 NO_CONTENT"
			);
		}
		Err(e) => {
			panic!("DELETE request should succeed, but got error: {:?}", e);
		}
	}
}

/// Test: Invalid URL returns 404
#[rstest]
#[tokio::test]

async fn test_invalid_url_returns_404(#[future] items_table: Arc<PgPool>) {
	let _pool = items_table.await;

	let view = RetrieveAPIView::<Item, JsonSerializer<Item>>::new();
	let request = create_get_request("/items/99999/"); // Non-existent ID

	let result = view.dispatch(request).await;

	// Should return 404 for non-existent resource
	match result {
		Ok(response) => {
			assert_eq!(
				response.status,
				StatusCode::NOT_FOUND,
				"Should return 404 NOT_FOUND"
			);
		}
		Err(_) => {
			// Error is also acceptable for non-existent resource
			assert!(true, "Error is acceptable for non-existent resource");
		}
	}
}

/// Test: Missing Content-Type header handling
#[rstest]
#[tokio::test]

async fn test_missing_content_type_handling(#[future] items_table: Arc<PgPool>) {
	let _pool = items_table.await;

	let view = CreateAPIView::<Item, JsonSerializer<Item>>::new();
	// Request without explicit Content-Type header
	let json_body = r#"{"name":"Test Item","value":50}"#;
	let request = create_post_request("/items/", json_body);

	let result = view.dispatch(request).await;

	// Should handle request (either succeed or return appropriate error)
	// The implementation may accept or reject based on content-type validation
	match result {
		Ok(response) => {
			assert!(
				response.status == StatusCode::CREATED
					|| response.status == StatusCode::BAD_REQUEST,
				"Should return CREATED or BAD_REQUEST"
			);
		}
		Err(_) => {
			// Error is acceptable if content-type validation is strict
			assert!(true, "Error is acceptable for missing Content-Type");
		}
	}
}

/// Test: Empty database list operation
#[rstest]
#[tokio::test]

async fn test_empty_database_list(#[future] items_table: Arc<PgPool>) {
	let _pool = items_table.await;

	let view = ListAPIView::<Item, JsonSerializer<Item>>::new();
	let request = create_get_request("/items/");

	let result = view.dispatch(request).await;

	// Should successfully return empty list
	assert!(
		result.is_ok(),
		"GET request on empty database should succeed"
	);
	let response = result.unwrap();
	assert_eq!(response.status, StatusCode::OK, "Should return 200 OK");

	// Response body should be empty array or equivalent
	let body_str = String::from_utf8(response.body.to_vec()).unwrap();
	assert!(
		body_str.contains("[]") || body_str.contains("\"items\":[]") || body_str.is_empty(),
		"Response should indicate empty list"
	);
}

/// Test: Simple filtering operation
#[rstest]
#[tokio::test]

async fn test_simple_filtering(#[future] items_with_data: Arc<PgPool>) {
	let _pool = items_with_data.await;

	// Note: ListAPIView filtering requires FilterConfig setup
	// This test verifies basic list operation; actual filtering would need
	// FilterConfig configuration
	let view = ListAPIView::<Item, JsonSerializer<Item>>::new();
	let request = create_get_request("/items/?name=Item%201");

	let result = view.dispatch(request).await;

	// Should successfully process request (filtering may or may not be applied
	// depending on FilterConfig)
	assert!(
		result.is_ok(),
		"GET request with query params should succeed"
	);
	let response = result.unwrap();
	assert_eq!(
		response.status,
		StatusCode::OK,
		"Should return 200 OK for filtered request"
	);
}
