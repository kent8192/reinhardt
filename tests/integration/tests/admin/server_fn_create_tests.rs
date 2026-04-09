//! Integration tests for create_record server function
//!
//! Tests the create operation server function.
//! Covers regression for Issue #2946 (create() hardcodes "id" in RETURNING clause).

use super::server_fn_helpers::server_fn_context;
use reinhardt_admin::adapters::MutationRequest;
use reinhardt_admin::core::AdminRecord;
use reinhardt_admin::core::{AdminDatabase, AdminSite};
use reinhardt_admin::server::create_record;
use reinhardt_di::Depends;
use rstest::*;
use serde_json::json;
use std::collections::HashMap;

use super::server_fn_helpers::{TEST_CSRF_TOKEN, make_auth_user, make_staff_request};

// ==================== Happy path tests ====================

/// Verify that create_record succeeds with valid data
#[rstest]
#[tokio::test]
async fn test_create_record_happy_path(
	#[future] server_fn_context: (Depends<AdminSite>, Depends<AdminDatabase>),
) {
	// Arrange
	let (site, db) = server_fn_context.await;
	let http_request = make_staff_request();
	let auth_user = make_auth_user();

	let mut data = HashMap::new();
	data.insert("name".to_string(), json!("Created Item"));
	data.insert("status".to_string(), json!("active"));

	let request = MutationRequest {
		csrf_token: TEST_CSRF_TOKEN.to_string(),
		data,
	};

	// Act
	let result = create_record(
		"TestModel".to_string(),
		request,
		site,
		db,
		http_request,
		auth_user,
	)
	.await;

	// Assert
	assert!(result.is_ok(), "create_record should succeed: {:?}", result);
	let response = result.unwrap();
	assert!(response.success);
	assert!(response.affected.is_some());
}

/// Verify create_record returns valid response metadata
#[rstest]
#[tokio::test]
async fn test_create_record_returns_valid_response(
	#[future] server_fn_context: (Depends<AdminSite>, Depends<AdminDatabase>),
) {
	// Arrange
	let (site, db) = server_fn_context.await;
	let http_request = make_staff_request();
	let auth_user = make_auth_user();

	let mut data = HashMap::new();
	data.insert("name".to_string(), json!("Response Metadata Test"));
	data.insert("status".to_string(), json!("active"));

	let request = MutationRequest {
		csrf_token: TEST_CSRF_TOKEN.to_string(),
		data,
	};

	// Act
	let result = create_record(
		"TestModel".to_string(),
		request,
		site,
		db,
		http_request,
		auth_user,
	)
	.await;

	// Assert
	let response = result.expect("create_record should succeed");
	assert!(response.success);
	assert!(
		response.message.contains("TestModel"),
		"Message should contain model name: {}",
		response.message
	);
	assert!(
		response.message.contains("created"),
		"Message should indicate creation: {}",
		response.message
	);
}

/// Verify created record persists to database and can be retrieved
#[rstest]
#[tokio::test]
async fn test_create_record_persists_to_database(
	#[future] server_fn_context: (Depends<AdminSite>, Depends<AdminDatabase>),
) {
	// Arrange
	let (site, db) = server_fn_context.await;
	let http_request = make_staff_request();
	let auth_user = make_auth_user();

	let mut data = HashMap::new();
	data.insert("name".to_string(), json!("Persistent Record"));
	data.insert("status".to_string(), json!("active"));

	let request = MutationRequest {
		csrf_token: TEST_CSRF_TOKEN.to_string(),
		data,
	};

	// Act
	let create_result = create_record(
		"TestModel".to_string(),
		request,
		site.clone(),
		db.clone(),
		http_request,
		auth_user,
	)
	.await;
	let create_response = create_result.expect("Create should succeed");

	// Verify by reading directly from DB
	let created_id = create_response
		.affected
		.expect("Should return affected count");
	let record = db
		.get::<AdminRecord>("test_models", "id", &created_id.to_string())
		.await;

	// Assert
	assert!(
		record.is_ok(),
		"Should be able to read created record from DB"
	);
}

/// Verify create_record works with multiple fields
#[rstest]
#[tokio::test]
async fn test_create_record_multiple_fields(
	#[future] server_fn_context: (Depends<AdminSite>, Depends<AdminDatabase>),
) {
	// Arrange
	let (site, db) = server_fn_context.await;
	let http_request = make_staff_request();
	let auth_user = make_auth_user();

	let mut data = HashMap::new();
	data.insert("name".to_string(), json!("Multi-Field Record"));
	data.insert("status".to_string(), json!("draft"));

	let request = MutationRequest {
		csrf_token: TEST_CSRF_TOKEN.to_string(),
		data,
	};

	// Act
	let result = create_record(
		"TestModel".to_string(),
		request,
		site,
		db,
		http_request,
		auth_user,
	)
	.await;

	// Assert
	assert!(
		result.is_ok(),
		"Should handle multiple fields: {:?}",
		result
	);
}

/// Verify create_record handles special characters and Unicode
#[rstest]
#[tokio::test]
async fn test_create_record_special_characters(
	#[future] server_fn_context: (Depends<AdminSite>, Depends<AdminDatabase>),
) {
	// Arrange
	let (site, db) = server_fn_context.await;
	let http_request = make_staff_request();
	let auth_user = make_auth_user();

	let mut data = HashMap::new();
	data.insert(
		"name".to_string(),
		json!("Special: <>&\"' \u{00e9}\u{00f1}\u{00fc} \u{65e5}\u{672c}\u{8a9e}"),
	);
	data.insert("status".to_string(), json!("active"));

	let request = MutationRequest {
		csrf_token: TEST_CSRF_TOKEN.to_string(),
		data,
	};

	// Act
	let result = create_record(
		"TestModel".to_string(),
		request,
		site,
		db,
		http_request,
		auth_user,
	)
	.await;

	// Assert
	assert!(
		result.is_ok(),
		"Should handle special characters: {:?}",
		result
	);
}

// ==================== Error path tests ====================

/// Verify that create_record returns error for non-registered model
#[rstest]
#[tokio::test]
async fn test_create_record_model_not_registered(
	#[future] server_fn_context: (Depends<AdminSite>, Depends<AdminDatabase>),
) {
	// Arrange
	let (site, db) = server_fn_context.await;
	let http_request = make_staff_request();
	let auth_user = make_auth_user();

	let request = MutationRequest {
		csrf_token: TEST_CSRF_TOKEN.to_string(),
		data: HashMap::new(),
	};

	// Act
	let result = create_record(
		"NonExistentModel".to_string(),
		request,
		site,
		db,
		http_request,
		auth_user,
	)
	.await;

	// Assert
	assert!(
		result.is_err(),
		"Should return error for unregistered model"
	);
}
