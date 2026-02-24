//! REST API Integration Tests
//!
//! Compilation and execution control:
//! - Cargo.toml: [[test]] name = "api_tests" required-features = ["with-reinhardt"]
//! - build.rs: Sets 'with-reinhardt' feature when reinhardt is available
//! - When feature is disabled, this entire test file is excluded from compilation
//!
//! Uses standard fixtures from reinhardt-test for automatic test server management.
//!
//! Note: Tests must be run serially due to shared in-memory storage.

use json::{Value, json};
use reinhardt::core::serde::json;
use reinhardt::test::client::APIClient;
use reinhardt::test::fixtures::test_server_guard;
use rstest::*;
use serial_test::serial;

// Import the application's routes function and storage
use examples_rest_api::apps::api::storage;
use examples_rest_api::config::urls::routes;

// ============================================================================
// Fixtures
// ============================================================================

#[fixture]
async fn server() -> reinhardt::test::fixtures::TestServerGuard {
	// Use the actual application router instead of an empty one
	let router = routes().into_server();
	test_server_guard(router).await
}

// ============================================================================
// Basic Endpoint Tests
// ============================================================================

/// Test root endpoint returns 200 OK
#[rstest]
#[tokio::test]
async fn test_root_endpoint(#[future] server: reinhardt::test::fixtures::TestServerGuard) {
	let server = server.await;
	let client = APIClient::with_base_url(&server.url);
	let response = client.get("/").await.expect("Failed to send request");

	assert_eq!(response.status_code(), 200);
	println!("✅ Root endpoint returns 200 OK");
}

/// Test health check endpoint returns JSON status
#[rstest]
#[tokio::test]
async fn test_health_check_endpoint(#[future] server: reinhardt::test::fixtures::TestServerGuard) {
	let server = server.await;

	let client = APIClient::with_base_url(&server.url);
	let response = client.get("/health").await.expect("Failed to send request");

	assert_eq!(response.status_code(), 200);

	let body: Value = response.json().expect("Failed to parse JSON response");

	assert_eq!(body["status"], "ok");
	println!("✅ Health check endpoint returns correct JSON");
}

// ============================================================================
// Article API Tests - List
// ============================================================================

/// Test listing articles returns empty array initially
#[rstest]
#[tokio::test]
#[serial(articles)]
async fn test_list_articles_empty(#[future] server: reinhardt::test::fixtures::TestServerGuard) {
	storage::clear_articles();
	let server = server.await;

	let client = APIClient::with_base_url(&server.url);
	let response = client
		.get("/api/articles")
		.await
		.expect("Failed to send request");

	assert_eq!(response.status_code(), reqwest::StatusCode::OK);

	let body: Value = response.json().expect("Failed to parse JSON response");

	assert_eq!(body["count"], 0);
	assert_eq!(body["results"].as_array().unwrap().len(), 0);
	println!("✅ List articles returns empty array");
}

// ============================================================================
// Article API Tests - Create
// ============================================================================

/// Test creating a new article
#[rstest]
#[tokio::test]
#[serial(articles)]
async fn test_create_article(#[future] server: reinhardt::test::fixtures::TestServerGuard) {
	storage::clear_articles();
	let server = server.await;

	let client = APIClient::with_base_url(&server.url);

	// Create article request
	let create_req = json!({
		"title": "Introduction to Reinhardt",
		"content": "Reinhardt is a batteries-included web framework for Rust...",
		"author": "John Doe",
		"published": true
	});

	let response = client
		.post("/api/articles", &create_req, "json")
		.await
		.expect("Failed to send request");

	assert_eq!(response.status_code(), 201);

	let body: Value = response.json().expect("Failed to parse JSON response");

	assert!(body["id"].as_i64().unwrap() > 0);
	assert_eq!(body["title"], "Introduction to Reinhardt");
	assert_eq!(body["author"], "John Doe");
	assert_eq!(body["published"], true);

	println!("✅ Article created successfully");
}

/// Test creating article with invalid data returns validation error
#[rstest]
#[tokio::test]
#[serial(articles)]
async fn test_create_article_validation_error(
	#[future] server: reinhardt::test::fixtures::TestServerGuard,
) {
	storage::clear_articles();
	let server = server.await;

	let client = APIClient::with_base_url(&server.url);

	// Missing required field 'title'
	let invalid_req = json!({
		"content": "Some content",
		"author": "John Doe",
		"published": true
	});

	let response = client
		.post("/api/articles", &invalid_req, "json")
		.await
		.expect("Failed to send request");

	assert_eq!(response.status_code(), 400);
	println!("✅ Create article with invalid data returns 400");
}

// ============================================================================
// Article API Tests - Get
// ============================================================================

/// Test getting a specific article by ID
#[rstest]
#[tokio::test]
#[serial(articles)]
async fn test_get_article_by_id(#[future] server: reinhardt::test::fixtures::TestServerGuard) {
	storage::clear_articles();
	let server = server.await;

	let client = APIClient::with_base_url(&server.url);

	// First create an article
	let create_req = json!({
		"title": "Test Article",
		"content": "Test content",
		"author": "Alice",
		"published": true
	});

	let create_response = client
		.post("/api/articles", &create_req, "json")
		.await
		.expect("Failed to create article");

	let created: Value = create_response
		.json()
		.expect("Failed to parse create response");
	let article_id = created["id"].as_i64().unwrap();

	// Now get the article
	let response = client
		.get(&format!("/api/articles/{}", article_id))
		.await
		.expect("Failed to send request");

	assert_eq!(response.status_code(), reqwest::StatusCode::OK);

	let body: Value = response.json().expect("Failed to parse JSON response");

	assert_eq!(body["id"], article_id);
	assert_eq!(body["title"], "Test Article");
	assert_eq!(body["author"], "Alice");

	println!("✅ Get article by ID successful");
}

/// Test getting non-existent article returns 404
#[rstest]
#[tokio::test]
#[serial(articles)]
async fn test_get_nonexistent_article(
	#[future] server: reinhardt::test::fixtures::TestServerGuard,
) {
	storage::clear_articles();
	let server = server.await;

	let client = APIClient::with_base_url(&server.url);
	let response = client
		.get("/api/articles/99999")
		.await
		.expect("Failed to send request");

	assert_eq!(response.status_code(), reqwest::StatusCode::NOT_FOUND);
	println!("✅ Get non-existent article returns 404");
}

// ============================================================================
// Article API Tests - Update
// ============================================================================

/// Test updating an article
#[rstest]
#[tokio::test]
#[serial(articles)]
async fn test_update_article(#[future] server: reinhardt::test::fixtures::TestServerGuard) {
	storage::clear_articles();
	let server = server.await;

	let client = APIClient::with_base_url(&server.url);

	// First create an article
	let create_req = json!({
		"title": "Original Title",
		"content": "Original content",
		"author": "Bob",
		"published": false
	});

	let create_response = client
		.post("/api/articles", &create_req, "json")
		.await
		.expect("Failed to create article");

	let created: Value = create_response
		.json()
		.expect("Failed to parse create response");
	let article_id = created["id"].as_i64().unwrap();

	// Update the article
	let update_req = json!({
		"title": "Updated Title",
		"published": true
	});

	let response = client
		.put(
			&format!("/api/articles/{}", article_id),
			&update_req,
			"json",
		)
		.await
		.expect("Failed to send request");

	assert_eq!(response.status_code(), 200);

	let body: Value = response.json().expect("Failed to parse JSON response");

	assert_eq!(body["id"], article_id);
	assert_eq!(body["title"], "Updated Title");
	assert_eq!(body["published"], true);
	assert_eq!(body["author"], "Bob"); // Unchanged field

	println!("✅ Article updated successfully");
}

/// Test updating non-existent article returns 404
#[rstest]
#[tokio::test]
#[serial(articles)]
async fn test_update_nonexistent_article(
	#[future] server: reinhardt::test::fixtures::TestServerGuard,
) {
	storage::clear_articles();
	let server = server.await;

	let client = APIClient::with_base_url(&server.url);

	let update_req = json!({
		"title": "Updated Title"
	});

	let response = client
		.put("/api/articles/99999", &update_req, "json")
		.await
		.expect("Failed to send request");

	assert_eq!(response.status_code(), 404);
	println!("✅ Update non-existent article returns 404");
}

// ============================================================================
// Article API Tests - Delete
// ============================================================================

/// Test deleting an article
#[rstest]
#[tokio::test]
#[serial(articles)]
async fn test_delete_article(#[future] server: reinhardt::test::fixtures::TestServerGuard) {
	storage::clear_articles();
	let server = server.await;

	let client = APIClient::with_base_url(&server.url);

	// First create an article
	let create_req = json!({
		"title": "To Be Deleted",
		"content": "This article will be deleted",
		"author": "Charlie",
		"published": true
	});

	let create_response = client
		.post("/api/articles", &create_req, "json")
		.await
		.expect("Failed to create article");

	let created: Value = create_response
		.json()
		.expect("Failed to parse create response");
	let article_id = created["id"].as_i64().unwrap();

	// Delete the article
	let response = client
		.delete(&format!("/api/articles/{}", article_id))
		.await
		.expect("Failed to send request");

	assert_eq!(response.status_code(), 204);

	// Verify article is deleted (should return 404)
	let get_response = client
		.get(&format!("/api/articles/{}", article_id))
		.await
		.expect("Failed to send request");

	assert_eq!(get_response.status_code(), reqwest::StatusCode::NOT_FOUND);
	println!("✅ Article deleted successfully");
}

/// Test deleting non-existent article returns 404
#[rstest]
#[tokio::test]
#[serial(articles)]
async fn test_delete_nonexistent_article(
	#[future] server: reinhardt::test::fixtures::TestServerGuard,
) {
	storage::clear_articles();
	let server = server.await;

	let client = APIClient::with_base_url(&server.url);
	let response = client
		.delete("/api/articles/99999")
		.await
		.expect("Failed to send request");

	assert_eq!(response.status_code(), 404);
	println!("✅ Delete non-existent article returns 404");
}

// ============================================================================
// Article API Tests - Comprehensive Flow
// ============================================================================

/// Test full CRUD workflow
#[rstest]
#[tokio::test]
#[serial(articles)]
async fn test_article_crud_workflow(#[future] server: reinhardt::test::fixtures::TestServerGuard) {
	storage::clear_articles();
	let server = server.await;

	let client = APIClient::with_base_url(&server.url);

	// 1. List (should be empty)
	let list_response = client
		.get("/api/articles")
		.await
		.expect("Failed to list articles");
	let list_body: Value = list_response.json().unwrap();
	assert_eq!(list_body["count"], 0);

	// 2. Create
	let create_req = json!({
		"title": "CRUD Test Article",
		"content": "Testing full CRUD workflow",
		"author": "Dave",
		"published": false
	});

	let create_response = client
		.post("/api/articles", &create_req, "json")
		.await
		.expect("Failed to create article");
	assert_eq!(create_response.status_code(), 201);

	let created: Value = create_response.json().unwrap();
	let article_id = created["id"].as_i64().unwrap();

	// 3. Read
	let get_response = client
		.get(&format!("/api/articles/{}", article_id))
		.await
		.expect("Failed to get article");
	assert_eq!(get_response.status_code(), 200);

	// 4. Update
	let update_req = json!({"published": true});
	let update_response = client
		.put(
			&format!("/api/articles/{}", article_id),
			&update_req,
			"json",
		)
		.await
		.expect("Failed to update article");
	assert_eq!(update_response.status_code(), 200);

	let updated: Value = update_response.json().unwrap();
	assert_eq!(updated["published"], true);

	// 5. Delete
	let delete_response = client
		.delete(&format!("/api/articles/{}", article_id))
		.await
		.expect("Failed to delete article");
	assert_eq!(delete_response.status_code(), 204);

	// 6. Verify deletion
	let verify_response = client
		.get(&format!("/api/articles/{}", article_id))
		.await
		.expect("Failed to verify deletion");
	assert_eq!(verify_response.status_code(), 404);

	println!("✅ Full CRUD workflow successful");
}

// ============================================================================
// Error Handling Tests
// ============================================================================

/// Test invalid path parameter returns 400
#[rstest]
#[tokio::test]
async fn test_invalid_path_parameter(#[future] server: reinhardt::test::fixtures::TestServerGuard) {
	let server = server.await;

	let client = APIClient::with_base_url(&server.url);
	let response = client
		.get("/api/articles/invalid-id")
		.await
		.expect("Failed to send request");

	assert!(response.status_code() == 400 || response.status_code() == 404);
	println!("✅ Invalid path parameter handled correctly");
}

/// Test unsupported method returns 405
#[rstest]
#[tokio::test]

async fn test_unsupported_method(#[future] server: reinhardt::test::fixtures::TestServerGuard) {
	let server = server.await;

	let client = APIClient::with_base_url(&server.url);
	let empty_data = json!({});
	let response = client
		.patch("/api/articles/1", &empty_data, "json")
		.await
		.expect("Failed to send request");

	assert_eq!(response.status_code(), 405);
	println!("✅ Unsupported method returns 405");
}

/// Test non-existent route returns 404
#[rstest]
#[tokio::test]
async fn test_nonexistent_route(#[future] server: reinhardt::test::fixtures::TestServerGuard) {
	let server = server.await;

	let client = APIClient::with_base_url(&server.url);
	let response = client
		.get("/api/nonexistent")
		.await
		.expect("Failed to send request");

	assert_eq!(response.status_code(), 404);
	println!("✅ Non-existent route returns 404");
}
