//! WASM tests for tweet shared types
//!
//! Tests that tweet request/response types and pagination types serialize
//! correctly in the WASM environment, ensuring client-server communication
//! compatibility.
//!
//! **Run with**: `cargo make wasm-test`

#![cfg(wasm)]

use wasm_bindgen_test::*;

use examples_twitter::apps::tweet::shared::types::*;

wasm_bindgen_test_configure!(run_in_browser);

// ============================================================================
// TweetInfo Tests
// ============================================================================

/// Test TweetInfo::new constructor
#[wasm_bindgen_test]
fn test_tweet_info_new() {
	// Arrange
	let id = uuid::Uuid::nil();
	let user_id = uuid::Uuid::nil();

	// Act
	let tweet = TweetInfo::new(
		id,
		user_id,
		"testuser".to_string(),
		"Hello WASM!".to_string(),
		5,
		2,
		"2026-01-01T00:00:00Z".to_string(),
	);

	// Assert
	assert_eq!(tweet.id, id);
	assert_eq!(tweet.user_id, user_id);
	assert_eq!(tweet.username, "testuser");
	assert_eq!(tweet.content, "Hello WASM!");
	assert_eq!(tweet.like_count, 5);
	assert_eq!(tweet.retweet_count, 2);
	assert_eq!(tweet.created_at, "2026-01-01T00:00:00Z");
}

/// Test TweetInfo serialization roundtrip in WASM
#[wasm_bindgen_test]
fn test_tweet_info_serialization_roundtrip() {
	// Arrange
	let tweet = TweetInfo::new(
		uuid::Uuid::nil(),
		uuid::Uuid::nil(),
		"author".to_string(),
		"Test tweet content".to_string(),
		0,
		0,
		"2026-01-01T00:00:00Z".to_string(),
	);

	// Act
	let json = serde_json::to_string(&tweet).unwrap();
	let deserialized: TweetInfo = serde_json::from_str(&json).unwrap();

	// Assert
	assert_eq!(deserialized.content, "Test tweet content");
	assert_eq!(deserialized.username, "author");
	assert_eq!(deserialized.like_count, 0);
	assert_eq!(deserialized.retweet_count, 0);
}

/// Test TweetInfo deserialization from JSON string
#[wasm_bindgen_test]
fn test_tweet_info_deserialization() {
	// Arrange
	let json = r#"{
		"id": "00000000-0000-0000-0000-000000000000",
		"user_id": "00000000-0000-0000-0000-000000000000",
		"username": "someone",
		"content": "Hello from JSON",
		"like_count": 10,
		"retweet_count": 3,
		"created_at": "2026-06-15T12:00:00Z"
	}"#;

	// Act
	let tweet: TweetInfo = serde_json::from_str(json).unwrap();

	// Assert
	assert_eq!(tweet.username, "someone");
	assert_eq!(tweet.content, "Hello from JSON");
	assert_eq!(tweet.like_count, 10);
	assert_eq!(tweet.retweet_count, 3);
}

// ============================================================================
// CreateTweetRequest Tests
// ============================================================================

/// Test CreateTweetRequest serialization roundtrip in WASM
#[wasm_bindgen_test]
fn test_create_tweet_request_serialization_roundtrip() {
	// Arrange
	let request = CreateTweetRequest {
		content: "Hello from WASM!".to_string(),
	};

	// Act
	let json = serde_json::to_string(&request).unwrap();
	let deserialized: CreateTweetRequest = serde_json::from_str(&json).unwrap();

	// Assert
	assert_eq!(deserialized.content, "Hello from WASM!");
}

/// Test CreateTweetRequest with max length content (280 characters)
#[wasm_bindgen_test]
fn test_create_tweet_request_max_length_content() {
	// Arrange - 280 characters (Twitter limit)
	let content = "a".repeat(280);
	let request = CreateTweetRequest {
		content: content.clone(),
	};

	// Act
	let json = serde_json::to_string(&request).unwrap();
	let deserialized: CreateTweetRequest = serde_json::from_str(&json).unwrap();

	// Assert
	assert_eq!(deserialized.content.len(), 280);
	assert_eq!(deserialized.content, content);
}

// ============================================================================
// PaginatedResponse Tests
// ============================================================================

/// Test PaginatedResponse serialization roundtrip with TweetInfo
#[wasm_bindgen_test]
fn test_paginated_response_serialization_roundtrip() {
	use examples_twitter::apps::tweet::shared::pagination::PaginatedResponse;

	// Arrange
	let tweets = vec![
		TweetInfo::new(
			uuid::Uuid::nil(),
			uuid::Uuid::nil(),
			"user1".to_string(),
			"First tweet".to_string(),
			1,
			0,
			"2026-01-01T00:00:00Z".to_string(),
		),
		TweetInfo::new(
			uuid::Uuid::nil(),
			uuid::Uuid::nil(),
			"user2".to_string(),
			"Second tweet".to_string(),
			0,
			1,
			"2026-01-02T00:00:00Z".to_string(),
		),
	];
	let response = PaginatedResponse::new(tweets, 10, 1, 2, "/api/tweets");

	// Act
	let json = serde_json::to_string(&response).unwrap();
	let deserialized: PaginatedResponse<TweetInfo> = serde_json::from_str(&json).unwrap();

	// Assert
	assert_eq!(deserialized.count, 10);
	assert_eq!(deserialized.results.len(), 2);
	assert_eq!(deserialized.results[0].content, "First tweet");
	assert_eq!(deserialized.results[1].content, "Second tweet");
	assert!(deserialized.next.is_some());
	assert!(deserialized.previous.is_none());
}

/// Test PaginatedResponse pagination links
#[wasm_bindgen_test]
fn test_paginated_response_links() {
	use examples_twitter::apps::tweet::shared::pagination::PaginatedResponse;

	// Arrange - page 2 of 5 (20 items, 5 per page)
	let tweets: Vec<TweetInfo> = Vec::new();
	let response = PaginatedResponse::new(tweets, 20, 2, 5, "/api/tweets");

	// Assert
	assert_eq!(response.next, Some("/api/tweets?page=3".to_string()));
	assert_eq!(response.previous, Some("/api/tweets?page=1".to_string()));
}

/// Test PaginatedResponse last page has no next link
#[wasm_bindgen_test]
fn test_paginated_response_last_page() {
	use examples_twitter::apps::tweet::shared::pagination::PaginatedResponse;

	// Arrange - last page (page 4 of 4, 20 items, 5 per page)
	let tweets: Vec<TweetInfo> = Vec::new();
	let response = PaginatedResponse::new(tweets, 20, 4, 5, "/api/tweets");

	// Assert
	assert!(response.next.is_none());
	assert!(response.previous.is_some());
}

/// Test PageQuery defaults and bounds
#[wasm_bindgen_test]
fn test_page_query_defaults() {
	use examples_twitter::apps::tweet::shared::pagination::PageQuery;

	// Arrange
	let json = r#"{"page": null, "page_size": null}"#;
	let query: PageQuery = serde_json::from_str(json).unwrap();

	// Assert
	assert_eq!(query.page(), 1);
	assert_eq!(query.page_size(), PageQuery::DEFAULT_PAGE_SIZE);
	assert_eq!(query.offset(), 0);
}

/// Test PageQuery with explicit values
#[wasm_bindgen_test]
fn test_page_query_explicit_values() {
	use examples_twitter::apps::tweet::shared::pagination::PageQuery;

	// Arrange
	let json = r#"{"page": 3, "page_size": 10}"#;
	let query: PageQuery = serde_json::from_str(json).unwrap();

	// Assert
	assert_eq!(query.page(), 3);
	assert_eq!(query.page_size(), 10);
	assert_eq!(query.offset(), 20);
}

/// Test PageQuery clamps page_size to MAX_PAGE_SIZE
#[wasm_bindgen_test]
fn test_page_query_max_page_size_clamp() {
	use examples_twitter::apps::tweet::shared::pagination::PageQuery;

	// Arrange - request 500 per page, should be clamped to MAX_PAGE_SIZE
	let json = r#"{"page": 1, "page_size": 500}"#;
	let query: PageQuery = serde_json::from_str(json).unwrap();

	// Assert
	assert_eq!(query.page_size(), PageQuery::MAX_PAGE_SIZE);
}
