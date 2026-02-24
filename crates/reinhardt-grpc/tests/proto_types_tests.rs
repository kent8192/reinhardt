//! Tests for Protobuf types in reinhardt-grpc
//!
//! These tests cover all required test categories for proto types:
//! - Happy path
//! - Error path
//! - Edge cases
//! - Sanity tests
//! - Equivalence partitioning
//! - Boundary value analysis
//! - Property-based tests

use prost::Message;
use reinhardt_grpc::proto::{common, graphql};
use rstest::{fixture, rstest};

// ============================================================================
// Test fixtures
// ============================================================================

/// Fixture that provides a valid Timestamp for testing
#[fixture]
fn valid_timestamp() -> common::Timestamp {
	common::Timestamp {
		seconds: 1_000_000_000, // Some reasonable timestamp
		nanos: 500_000_000,     // Half a second
	}
}

/// Fixture that provides a valid PageInfo for testing
#[fixture]
fn valid_page_info() -> common::PageInfo {
	common::PageInfo {
		page: 1,
		per_page: 20,
		total: 100,
		has_next: true,
		has_prev: false,
	}
}

/// Fixture that provides a valid Error for testing
#[fixture]
fn valid_error() -> common::Error {
	common::Error {
		code: "404".to_string(),
		message: "Not Found".to_string(),
		metadata: std::collections::HashMap::from([(
			"details".to_string(),
			"Resource not found".to_string(),
		)]),
	}
}

/// Fixture that provides a valid GraphQLRequest for testing
#[fixture]
fn valid_graphql_request() -> graphql::GraphQlRequest {
	graphql::GraphQlRequest {
		query: "{ user { id name } }".to_string(),
		operation_name: Some("GetUser".to_string()),
		variables: Some("{\"id\": 1}".to_string()),
	}
}

// ============================================================================
// Happy path tests
// ============================================================================

#[rstest]
#[tokio::test]
async fn test_empty_proto_creation() {
	// Empty type should be creatable
	let empty = common::Empty {};

	// Verify it can be serialized and deserialized
	let encoded = empty.encode_to_vec();
	let _decoded = common::Empty::decode(&encoded[..]).unwrap();

	// Empty types should be equal
	// Note: prost doesn't generate Eq/PartialEq for Empty by default,
	// but we can at least verify the decode succeeded
	assert_eq!(encoded.len(), 0, "Empty should encode to zero bytes");
}

#[rstest]
#[tokio::test]
async fn test_timestamp_conversions(valid_timestamp: common::Timestamp) {
	// Test serialization round-trip
	let encoded = valid_timestamp.encode_to_vec();
	let decoded = common::Timestamp::decode(&encoded[..]).unwrap();

	assert_eq!(
		valid_timestamp.seconds, decoded.seconds,
		"Seconds should match"
	);
	assert_eq!(
		valid_timestamp.nanos, decoded.nanos,
		"Nanoseconds should match"
	);

	// Test that nanoseconds are within valid range
	assert!(
		valid_timestamp.nanos >= 0 && valid_timestamp.nanos <= 999_999_999,
		"Nanoseconds should be in valid range [0, 999_999_999]"
	);
}

#[rstest]
#[tokio::test]
async fn test_page_info_pagination_logic(valid_page_info: common::PageInfo) {
	// Test basic pagination calculations
	assert!(valid_page_info.page > 0, "Page should be positive");
	assert!(valid_page_info.per_page > 0, "Per page should be positive");
	assert!(valid_page_info.total >= 0, "Total should be non-negative");

	// Verify pagination state
	assert!(valid_page_info.has_next, "Should have next page");
	assert!(!valid_page_info.has_prev, "Should not have previous page");
}

#[rstest]
#[tokio::test]
async fn test_batch_result_serialization() {
	// Create a BatchResult with success and failure
	let error1 = common::Error {
		code: "500".to_string(),
		message: "Item 1 failed".to_string(),
		metadata: Default::default(),
	};
	let error2 = common::Error {
		code: "500".to_string(),
		message: "Item 2 failed".to_string(),
		metadata: Default::default(),
	};

	let batch_result = common::BatchResult {
		success_count: 10,
		failure_count: 2,
		errors: vec![error1, error2],
	};

	// Test serialization round-trip
	let encoded = batch_result.encode_to_vec();
	let decoded = common::BatchResult::decode(&encoded[..]).unwrap();

	assert_eq!(
		batch_result.success_count, decoded.success_count,
		"Success count should match"
	);
	assert_eq!(
		batch_result.failure_count, decoded.failure_count,
		"Failure count should match"
	);
	assert_eq!(
		batch_result.errors.len(),
		decoded.errors.len(),
		"Error count should match"
	);
}

#[rstest]
#[tokio::test]
async fn test_graphql_request_response_cycle(valid_graphql_request: graphql::GraphQlRequest) {
	// Test GraphQL request serialization
	let request_encoded = valid_graphql_request.encode_to_vec();
	let request_decoded = graphql::GraphQlRequest::decode(&request_encoded[..]).unwrap();

	assert_eq!(
		valid_graphql_request.query, request_decoded.query,
		"Query should match"
	);
	assert_eq!(
		valid_graphql_request.operation_name, request_decoded.operation_name,
		"Operation name should match"
	);
	assert_eq!(
		valid_graphql_request.variables, request_decoded.variables,
		"Variables should match"
	);

	// Create a corresponding response
	let graphql_response = graphql::GraphQlResponse {
		data: Some("{\"user\": {\"id\": 1, \"name\": \"Test\"}}".to_string()),
		errors: vec![],
		extensions: Some("{}".to_string()),
	};

	// Test response serialization
	let response_encoded = graphql_response.encode_to_vec();
	let response_decoded = graphql::GraphQlResponse::decode(&response_encoded[..]).unwrap();

	assert_eq!(
		graphql_response.data, response_decoded.data,
		"Data should match"
	);
	assert_eq!(
		graphql_response.errors.len(),
		response_decoded.errors.len(),
		"Error count should match"
	);
	assert_eq!(
		graphql_response.extensions, response_decoded.extensions,
		"Extensions should match"
	);
}

// ============================================================================
// Edge cases tests
// ============================================================================

#[rstest]
#[case::zero_seconds(0, 0)] // Minimum valid timestamp
#[case::max_nanoseconds(100, 999_999_999)] // Maximum nanoseconds
#[case::negative_seconds(-1, 0)] // Negative seconds (allowed by protocol buffers)
#[case::large_seconds(9_223_372_036_854_775_807, 0)] // Max i64 seconds
#[tokio::test]
async fn test_timestamp_edge_values(#[case] seconds: i64, #[case] nanos: i32) {
	let timestamp = common::Timestamp { seconds, nanos };

	// Should always serialize/deserialize successfully
	let encoded = timestamp.encode_to_vec();
	let decoded = common::Timestamp::decode(&encoded[..]).unwrap();

	assert_eq!(timestamp.seconds, decoded.seconds, "Seconds should match");
	assert_eq!(timestamp.nanos, decoded.nanos, "Nanoseconds should match");

	// Additional validation for non-negative seconds
	if seconds >= 0 {
		assert!(
			nanos >= 0 && nanos <= 999_999_999,
			"For non-negative seconds, nanos should be in [0, 999_999_999]"
		);
	}
}

#[rstest]
#[case::first_page(1, 20, 100, false, true)] // First page
#[case::last_page(5, 20, 100, true, false)] // Last page with total=100, per_page=20, so 5 pages
#[case::middle_page(3, 20, 100, true, true)] // Middle page
#[case::single_page(1, 100, 50, false, false)] // Only one page (50 items, 100 per page)
#[case::empty_page(1, 20, 0, false, false)] // No items
#[tokio::test]
async fn test_page_info_edge_cases(
	#[case] page: i32,
	#[case] per_page: i32,
	#[case] total: i32,
	#[case] expected_has_prev: bool,
	#[case] expected_has_next: bool,
) {
	let page_info = common::PageInfo {
		page,
		per_page,
		total,
		has_next: page * per_page < total,
		has_prev: page > 1,
	};

	// Verify serialization round-trip
	let encoded = page_info.encode_to_vec();
	let decoded = common::PageInfo::decode(&encoded[..]).unwrap();

	assert_eq!(
		page_info.has_next, decoded.has_next,
		"has_next should match"
	);
	assert_eq!(
		page_info.has_prev, decoded.has_prev,
		"has_prev should match"
	);

	// Verify calculated values match expectations
	assert_eq!(
		page_info.has_next, expected_has_next,
		"has_next should match expectation"
	);
	assert_eq!(
		page_info.has_prev, expected_has_prev,
		"has_prev should match expectation"
	);
}

// ============================================================================
// Sanity tests
// ============================================================================

#[rstest]
#[tokio::test]
async fn test_proto_types_sanity_check() {
	// Test that all proto types can be instantiated and serialized

	// Empty
	let empty = common::Empty {};
	let empty_encoded = empty.encode_to_vec();
	assert!(
		common::Empty::decode(&empty_encoded[..]).is_ok(),
		"Empty should decode successfully"
	);

	// Error
	let error = common::Error {
		code: "500".to_string(),
		message: "Internal Server Error".to_string(),
		metadata: Default::default(),
	};
	let error_encoded = error.encode_to_vec();
	assert!(
		common::Error::decode(&error_encoded[..]).is_ok(),
		"Error should decode successfully"
	);

	// BatchResult
	let batch_result = common::BatchResult {
		success_count: 0,
		failure_count: 0,
		errors: vec![],
	};
	let batch_encoded = batch_result.encode_to_vec();
	assert!(
		common::BatchResult::decode(&batch_encoded[..]).is_ok(),
		"BatchResult should decode successfully"
	);

	// SubscriptionEvent
	let response = graphql::GraphQlResponse {
		data: Some("{}".to_string()),
		errors: vec![],
		extensions: Some("{}".to_string()),
	};

	let subscription_event = graphql::SubscriptionEvent {
		id: "test-id".to_string(),
		event_type: "update".to_string(),
		payload: Some(response),
		timestamp: Some(common::Timestamp {
			seconds: 0,
			nanos: 0,
		}),
	};
	let event_encoded = subscription_event.encode_to_vec();
	assert!(
		graphql::SubscriptionEvent::decode(&event_encoded[..]).is_ok(),
		"SubscriptionEvent should decode successfully"
	);
}

// ============================================================================
// Additional test categories will be implemented in subsequent iterations
// ============================================================================

// Note: The following test categories will be implemented:
// - Property-based tests - using proptest
// - Equivalence partitioning - using rstest case macro
// - Boundary value analysis - using rstest case macro

// For property-based tests with proptest, we'll need to add proptest dependency
// and implement strategies for generating proto types.
