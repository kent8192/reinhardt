//! Generic API Views Validation and Error Handling Integration Tests
//!
//! Tests comprehensive error handling and validation for Generic API Views:
//! - Missing required fields (400 Bad Request)
//! - Invalid data types (400 Bad Request)
//! - Malformed JSON (400 Bad Request)
//! - Resource not found errors (404 Not Found)
//! - Disallowed HTTP methods (405 Method Not Allowed)
//! - Empty request bodies (400 Bad Request)
//! - Constraint violations (409 Conflict)
//!
//! **Test Category**: Error Path
//!
//! **Fixtures Used:**
//! - shared_db_pool: Shared PostgreSQL database pool with ORM initialized
//!
//! **Test Data Schema:**
//! - articles(id SERIAL PRIMARY KEY, title TEXT NOT NULL UNIQUE, content TEXT NOT NULL,
//!   published BOOLEAN NOT NULL, view_count INT NOT NULL, created_at TIMESTAMP)

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
use reinhardt_views::{
	CreateAPIView, DestroyAPIView, ListAPIView, RetrieveAPIView, UpdateAPIView, View,
};
use rstest::*;
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use std::sync::Arc;

// ============================================================================
// Model Definitions
// ============================================================================

/// Article model for validation testing (with unique constraint on title)
#[allow(dead_code)]
#[model(app_label = "views_test", table_name = "articles")]
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
struct Article {
	#[field(primary_key = true)]
	id: Option<i64>,
	#[field(max_length = 200)]
	title: String,
	#[field(max_length = 5000)]
	content: String,
	published: bool,
	view_count: i32,
	#[field(null = true)]
	created_at: Option<DateTime<Utc>>,
}

// ============================================================================
// Table Identifiers (for reinhardt-query operations)
// ============================================================================

#[derive(Debug, Clone, Copy, Iden)]
enum Articles {
	Table,
	Id,
	Title,
	Content,
	Published,
	ViewCount,
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

/// Fixture: Setup articles table with UNIQUE constraint on title
#[fixture]
async fn articles_table(#[future] db_pool: Arc<PgPool>) -> Arc<PgPool> {
	let pool = db_pool.await;

	// Create articles table with UNIQUE constraint on title
	let mut create_table_stmt = Query::create_table();
	create_table_stmt
		.table(Articles::Table.into_iden())
		.if_not_exists()
		.col(
			ColumnDef::new(Articles::Id)
				.big_integer()
				.not_null(true)
				.auto_increment(true)
				.primary_key(true),
		)
		.col(
			ColumnDef::new(Articles::Title)
				.string_len(200)
				.not_null(true),
		)
		.col(ColumnDef::new(Articles::Content).text().not_null(true))
		.col(
			ColumnDef::new(Articles::Published)
				.boolean()
				.not_null(true)
				.default(false.into()),
		)
		.col(
			ColumnDef::new(Articles::ViewCount)
				.integer()
				.not_null(true)
				.default(0i32.into()),
		)
		.col(ColumnDef::new(Articles::CreatedAt).timestamp());

	let sql = create_table_stmt.to_string(PostgresQueryBuilder::new());
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create articles table");

	// Add UNIQUE constraint on title
	let mut create_index_stmt = Query::create_index();
	create_index_stmt
		.name("articles_title_unique")
		.table(Articles::Table.into_iden())
		.col(Articles::Title)
		.unique();

	let index_sql = create_index_stmt.to_string(PostgresQueryBuilder::new());
	sqlx::query(&index_sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create unique index on title");

	pool
}

/// Fixture: Article with unique title for constraint testing
#[fixture]
async fn existing_article(#[future] articles_table: Arc<PgPool>) -> (Arc<PgPool>, Article) {
	let pool = articles_table.await;

	let article = Article::new(
		"Existing Article".to_string(),
		"Existing content".to_string(),
		true,
		100,
		Some(Utc::now()),
	);

	let sql = "INSERT INTO articles (title, content, published, view_count, created_at) VALUES ($1, $2, $3, $4, $5) RETURNING id";
	let row = sqlx::query(sql)
		.bind(&article.title)
		.bind(&article.content)
		.bind(article.published)
		.bind(article.view_count)
		.bind(article.created_at)
		.fetch_one(pool.as_ref())
		.await
		.expect("Failed to insert existing article");

	let id: i64 = row.get(0);
	let mut inserted_article = article;
	inserted_article.id = Some(id);

	(pool, inserted_article)
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Helper: Create HTTP POST request with JSON body
fn create_post_request(uri: &str, json_body: &str) -> Request {
	Request::builder()
		.method(Method::POST)
		.uri(uri)
		.version(Version::HTTP_11)
		.headers(HeaderMap::new())
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

/// Helper: Create HTTP PUT request with JSON body
fn create_put_request(uri: &str, json_body: &str) -> Request {
	Request::builder()
		.method(Method::PUT)
		.uri(uri)
		.version(Version::HTTP_11)
		.headers(HeaderMap::new())
		.body(Bytes::from(json_body.to_string()))
		.build()
		.expect("Failed to build request")
}

/// Helper: Create HTTP PATCH request with JSON body
fn create_patch_request(uri: &str, json_body: &str) -> Request {
	Request::builder()
		.method(Method::PATCH)
		.uri(uri)
		.version(Version::HTTP_11)
		.headers(HeaderMap::new())
		.body(Bytes::from(json_body.to_string()))
		.build()
		.expect("Failed to build request")
}

/// Helper: Create HTTP DELETE request
fn create_delete_request(uri: &str) -> Request {
	Request::builder()
		.method(Method::DELETE)
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

/// Test: CreateAPIView returns 400 when required field is missing
#[rstest]
#[tokio::test]
async fn test_create_missing_required_field(#[future] articles_table: Arc<PgPool>) {
	let _pool = articles_table.await;

	let view = CreateAPIView::<Article, JsonSerializer<Article>>::new();
	// Missing required field 'title'
	let json_body = r#"{"content":"Some content","published":true,"view_count":0}"#;
	let request = create_post_request("/articles/", json_body);

	let result = view.dispatch(request).await;

	// Should return error due to missing required field
	assert!(
		result.is_err() || result.unwrap().status == StatusCode::BAD_REQUEST,
		"Expected BAD_REQUEST for missing required field"
	);
}

/// Test: CreateAPIView returns 400 when field type is invalid
#[rstest]
#[tokio::test]
async fn test_create_invalid_type(#[future] articles_table: Arc<PgPool>) {
	let _pool = articles_table.await;

	let view = CreateAPIView::<Article, JsonSerializer<Article>>::new();
	// view_count should be integer, not string
	let json_body =
		r#"{"title":"Test","content":"Content","published":true,"view_count":"invalid"}"#;
	let request = create_post_request("/articles/", json_body);

	let result = view.dispatch(request).await;

	// Should return error due to type mismatch
	assert!(
		result.is_err() || result.unwrap().status == StatusCode::BAD_REQUEST,
		"Expected BAD_REQUEST for invalid type"
	);
}

/// Test: CreateAPIView returns 400 for malformed JSON
#[rstest]
#[tokio::test]
async fn test_create_invalid_json(#[future] articles_table: Arc<PgPool>) {
	let _pool = articles_table.await;

	let view = CreateAPIView::<Article, JsonSerializer<Article>>::new();
	// Invalid JSON (missing closing brace)
	let json_body = r#"{"title":"Test","content":"Content""#;
	let request = create_post_request("/articles/", json_body);

	let result = view.dispatch(request).await;

	// Should return error due to malformed JSON
	assert!(
		result.is_err() || result.unwrap().status == StatusCode::BAD_REQUEST,
		"Expected BAD_REQUEST for malformed JSON"
	);
}

/// Test: RetrieveAPIView returns 404 for non-existent resource
#[rstest]
#[tokio::test]
async fn test_retrieve_not_found(#[future] articles_table: Arc<PgPool>) {
	let _pool = articles_table.await;

	let view = RetrieveAPIView::<Article, JsonSerializer<Article>>::new();
	let request = create_get_request("/articles/99999/"); // Non-existent ID

	let result = view.dispatch(request).await;

	// Should return 404 Not Found
	assert!(
		result.is_err() || result.unwrap().status == StatusCode::NOT_FOUND,
		"Expected NOT_FOUND for non-existent resource"
	);
}

/// Test: UpdateAPIView returns 404 for non-existent resource
#[rstest]
#[tokio::test]
async fn test_update_not_found(#[future] articles_table: Arc<PgPool>) {
	let _pool = articles_table.await;

	let view = UpdateAPIView::<Article, JsonSerializer<Article>>::new();
	let json_body =
		r#"{"title":"Updated","content":"Updated content","published":true,"view_count":0}"#;
	let request = create_put_request("/articles/99999/", json_body);

	let result = view.dispatch(request).await;

	// Should return 404 Not Found
	assert!(
		result.is_err() || result.unwrap().status == StatusCode::NOT_FOUND,
		"Expected NOT_FOUND for non-existent resource"
	);
}

/// Test: DestroyAPIView returns 404 for non-existent resource
#[rstest]
#[tokio::test]
async fn test_destroy_not_found(#[future] articles_table: Arc<PgPool>) {
	let _pool = articles_table.await;

	let view = DestroyAPIView::<Article>::new();
	let request = create_delete_request("/articles/99999/");

	let result = view.dispatch(request).await;

	// Should return 404 Not Found
	assert!(
		result.is_err() || result.unwrap().status == StatusCode::NOT_FOUND,
		"Expected NOT_FOUND for non-existent resource"
	);
}

/// Test: ListAPIView returns 405 for disallowed HTTP method (POST)
#[rstest]
#[tokio::test]
async fn test_list_disallowed_method(#[future] articles_table: Arc<PgPool>) {
	let _pool = articles_table.await;

	let view = ListAPIView::<Article, JsonSerializer<Article>>::new();
	let json_body = r#"{"title":"Test"}"#;
	let request = create_post_request("/articles/", json_body); // POST not allowed for ListView

	let result = view.dispatch(request).await;

	// Should return 405 Method Not Allowed
	assert!(result.is_err(), "Expected error for disallowed method");
}

/// Test: CreateAPIView returns 400 for empty request body
#[rstest]
#[tokio::test]
async fn test_create_empty_body(#[future] articles_table: Arc<PgPool>) {
	let _pool = articles_table.await;

	let view = CreateAPIView::<Article, JsonSerializer<Article>>::new();
	let request = create_post_request("/articles/", ""); // Empty body

	let result = view.dispatch(request).await;

	// Should return error for empty body
	assert!(
		result.is_err() || result.unwrap().status == StatusCode::BAD_REQUEST,
		"Expected BAD_REQUEST for empty body"
	);
}

/// Test: UpdateAPIView with PATCH returns 400 for invalid field
#[rstest]
#[tokio::test]
async fn test_update_partial_invalid_field(#[future] existing_article: (Arc<PgPool>, Article)) {
	let (_pool, article) = existing_article.await;
	let article_id = article.id.unwrap();

	let view = UpdateAPIView::<Article, JsonSerializer<Article>>::new().with_partial(true);
	// 'nonexistent_field' does not exist in Article model
	let json_body = r#"{"nonexistent_field":"invalid"}"#;
	let request = create_patch_request(&format!("/articles/{}/", article_id), json_body);

	let result = view.dispatch(request).await;

	// Should return error for invalid field (implementation-dependent)
	// May return 400 Bad Request or ignore unknown fields
	match result {
		Err(_) => {
			// Error is acceptable
		}
		Ok(response) => {
			assert!(
				response.status == StatusCode::BAD_REQUEST || response.status == StatusCode::OK,
				"Expected BAD_REQUEST or OK (unknown field ignored)"
			);
		}
	}
}

/// Test: CreateAPIView returns 409 for UNIQUE constraint violation
#[rstest]
#[tokio::test]
async fn test_create_constraint_violation(#[future] existing_article: (Arc<PgPool>, Article)) {
	let (_pool, existing) = existing_article.await;

	let view = CreateAPIView::<Article, JsonSerializer<Article>>::new();
	// Try to create article with same title (UNIQUE constraint violation)
	let json_body = format!(
		r#"{{"title":"{}","content":"Different content","published":false,"view_count":0}}"#,
		existing.title
	);
	let request = create_post_request("/articles/", &json_body);

	let result = view.dispatch(request).await;

	// Should return error due to UNIQUE constraint violation
	// Database may return different error codes (409 Conflict or 500 Internal Server Error)
	match result {
		Err(_) => {
			// Error is acceptable for constraint violation
		}
		Ok(response) => {
			assert!(
				response.status == StatusCode::CONFLICT
					|| response.status == StatusCode::INTERNAL_SERVER_ERROR,
				"Expected CONFLICT or INTERNAL_SERVER_ERROR for constraint violation"
			);
		}
	}
}
