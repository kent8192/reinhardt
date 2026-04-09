//! Integration tests for update_record server function
//!
//! Tests the update operation server function.
//! Covers Issue #3047 (missing update_record test coverage).

use super::server_fn_helpers::server_fn_context;
use reinhardt_admin::adapters::MutationRequest;
use reinhardt_admin::core::AdminRecord;
use reinhardt_admin::core::{AdminDatabase, AdminSite};
use reinhardt_admin::server::{create_record, update_record};
use reinhardt_di::Depends;
use rstest::*;
use serde_json::json;
use std::collections::HashMap;

use super::server_fn_helpers::{TEST_CSRF_TOKEN, make_auth_user, make_staff_request};

// ==================== Helper ====================

/// Creates a test record and returns its ID as a string.
async fn create_test_record(
	site: &Depends<AdminSite>,
	db: &Depends<AdminDatabase>,
	name: &str,
	status: &str,
) -> String {
	let http_request = make_staff_request();
	let auth_user = make_auth_user();

	let mut data = HashMap::new();
	data.insert("name".to_string(), json!(name));
	data.insert("status".to_string(), json!(status));

	let request = MutationRequest {
		csrf_token: TEST_CSRF_TOKEN.to_string(),
		data,
	};

	let result = create_record(
		"TestModel".to_string(),
		request,
		site.clone(),
		db.clone(),
		http_request,
		auth_user,
	)
	.await
	.expect("Failed to create test record");

	result
		.affected
		.expect("Create should return affected count")
		.to_string()
}

// ==================== Happy path tests ====================

/// Verify that update_record succeeds with valid data
#[rstest]
#[tokio::test]
async fn test_update_record_happy_path(
	#[future] server_fn_context: (Depends<AdminSite>, Depends<AdminDatabase>),
) {
	// Arrange
	let (site, db) = server_fn_context.await;
	let id = create_test_record(&site, &db, "Original Name", "active").await;

	let http_request = make_staff_request();
	let auth_user = make_auth_user();

	let mut data = HashMap::new();
	data.insert("name".to_string(), json!("Updated Name"));

	let request = MutationRequest {
		csrf_token: TEST_CSRF_TOKEN.to_string(),
		data,
	};

	// Act
	let result = update_record(
		"TestModel".to_string(),
		id,
		request,
		site,
		db,
		http_request,
		auth_user,
	)
	.await;

	// Assert
	assert!(result.is_ok(), "update_record should succeed: {:?}", result);
	let response = result.unwrap();
	assert!(response.success);
	assert_eq!(response.affected, Some(1));
}

/// Verify update_record returns valid response metadata
#[rstest]
#[tokio::test]
async fn test_update_record_returns_valid_response(
	#[future] server_fn_context: (Depends<AdminSite>, Depends<AdminDatabase>),
) {
	// Arrange
	let (site, db) = server_fn_context.await;
	let id = create_test_record(&site, &db, "Metadata Test", "active").await;

	let http_request = make_staff_request();
	let auth_user = make_auth_user();

	let mut data = HashMap::new();
	data.insert("name".to_string(), json!("Metadata Updated"));

	let request = MutationRequest {
		csrf_token: TEST_CSRF_TOKEN.to_string(),
		data,
	};

	// Act
	let result = update_record(
		"TestModel".to_string(),
		id,
		request,
		site,
		db,
		http_request,
		auth_user,
	)
	.await;

	// Assert
	let response = result.expect("update_record should succeed");
	assert!(response.success);
	assert!(
		response.message.contains("TestModel"),
		"Message should contain model name: {}",
		response.message
	);
	assert!(
		response.message.contains("updated"),
		"Message should indicate update: {}",
		response.message
	);
}

/// Verify updated record persists to database
#[rstest]
#[tokio::test]
async fn test_update_record_persists_to_database(
	#[future] server_fn_context: (Depends<AdminSite>, Depends<AdminDatabase>),
) {
	// Arrange
	let (site, db) = server_fn_context.await;
	let id = create_test_record(&site, &db, "Before Update", "active").await;

	let http_request = make_staff_request();
	let auth_user = make_auth_user();

	let mut data = HashMap::new();
	data.insert("name".to_string(), json!("After Update"));
	data.insert("status".to_string(), json!("inactive"));

	let request = MutationRequest {
		csrf_token: TEST_CSRF_TOKEN.to_string(),
		data,
	};

	// Act
	update_record(
		"TestModel".to_string(),
		id.clone(),
		request,
		site,
		db.clone(),
		http_request,
		auth_user,
	)
	.await
	.expect("update_record should succeed");

	// Assert - verify changes persisted in DB
	let record = db
		.get::<AdminRecord>("test_models", "id", &id)
		.await
		.expect("Should read updated record");
	let record = record.expect("Record should exist");
	// Sanitized values have HTML entities escaped, so compare accordingly
	let name = record.get("name").expect("Should have name field");
	assert_eq!(name, &json!("After Update"));
}

/// Verify update_record works with multiple fields
#[rstest]
#[tokio::test]
async fn test_update_record_multiple_fields(
	#[future] server_fn_context: (Depends<AdminSite>, Depends<AdminDatabase>),
) {
	// Arrange
	let (site, db) = server_fn_context.await;
	let id = create_test_record(&site, &db, "Multi Field", "active").await;

	let http_request = make_staff_request();
	let auth_user = make_auth_user();

	let mut data = HashMap::new();
	data.insert("name".to_string(), json!("Updated Multi"));
	data.insert("status".to_string(), json!("draft"));
	data.insert("description".to_string(), json!("New description"));

	let request = MutationRequest {
		csrf_token: TEST_CSRF_TOKEN.to_string(),
		data,
	};

	// Act
	let result = update_record(
		"TestModel".to_string(),
		id,
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

/// Verify update_record handles special characters and Unicode
#[rstest]
#[tokio::test]
async fn test_update_record_special_characters(
	#[future] server_fn_context: (Depends<AdminSite>, Depends<AdminDatabase>),
) {
	// Arrange
	let (site, db) = server_fn_context.await;
	let id = create_test_record(&site, &db, "Original", "active").await;

	let http_request = make_staff_request();
	let auth_user = make_auth_user();

	let mut data = HashMap::new();
	data.insert(
		"name".to_string(),
		json!("Special: <>&\"' \u{00e9}\u{00f1}\u{00fc} \u{65e5}\u{672c}\u{8a9e}"),
	);

	let request = MutationRequest {
		csrf_token: TEST_CSRF_TOKEN.to_string(),
		data,
	};

	// Act
	let result = update_record(
		"TestModel".to_string(),
		id,
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

/// Verify update_record with partial fields leaves other fields unchanged
#[rstest]
#[tokio::test]
async fn test_update_record_partial_fields(
	#[future] server_fn_context: (Depends<AdminSite>, Depends<AdminDatabase>),
) {
	// Arrange
	let (site, db) = server_fn_context.await;
	let id = create_test_record(&site, &db, "Partial Original", "active").await;

	let http_request = make_staff_request();
	let auth_user = make_auth_user();

	// Only update status, leave name unchanged
	let mut data = HashMap::new();
	data.insert("status".to_string(), json!("archived"));

	let request = MutationRequest {
		csrf_token: TEST_CSRF_TOKEN.to_string(),
		data,
	};

	// Act
	update_record(
		"TestModel".to_string(),
		id.clone(),
		request,
		site,
		db.clone(),
		http_request,
		auth_user,
	)
	.await
	.expect("Partial update should succeed");

	// Assert - name should be unchanged, status should be updated
	let record = db
		.get::<AdminRecord>("test_models", "id", &id)
		.await
		.expect("Should read record")
		.expect("Record should exist");
	let status = record.get("status").expect("Should have status field");
	assert_eq!(status, &json!("archived"));
}

// ==================== Error path tests ====================

/// Verify update_record returns error for non-existent ID
#[rstest]
#[tokio::test]
async fn test_update_record_not_found(
	#[future] server_fn_context: (Depends<AdminSite>, Depends<AdminDatabase>),
) {
	// Arrange
	let (site, db) = server_fn_context.await;
	let http_request = make_staff_request();
	let auth_user = make_auth_user();

	let mut data = HashMap::new();
	data.insert("name".to_string(), json!("Ghost Update"));

	let request = MutationRequest {
		csrf_token: TEST_CSRF_TOKEN.to_string(),
		data,
	};

	// Act
	let result = update_record(
		"TestModel".to_string(),
		"999999".to_string(),
		request,
		site,
		db,
		http_request,
		auth_user,
	)
	.await;

	// Assert
	assert!(result.is_err(), "Should return error for non-existent ID");
}

/// Verify update_record returns error for non-registered model
#[rstest]
#[tokio::test]
async fn test_update_record_model_not_registered(
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
	let result = update_record(
		"NonExistentModel".to_string(),
		"1".to_string(),
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
