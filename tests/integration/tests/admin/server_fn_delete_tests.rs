//! Integration tests for delete_record and bulk_delete_records server functions
//!
//! Covers regression for:
//! - Issue #2934 (Mutation operations return success with 0 affected rows)
//! - Issue #2935 (bulk_delete has no ID count limit)

use super::server_fn_helpers::server_fn_context;
use reinhardt_admin::adapters::BulkDeleteRequest;
use reinhardt_admin::core::AdminRecord;
use reinhardt_admin::core::{AdminDatabase, AdminSite};
use reinhardt_admin::server::{bulk_delete_records, delete_record};
use reinhardt_di::Depends;
use rstest::*;
use serde_json::json;
use std::collections::HashMap;

use super::server_fn_helpers::{TEST_CSRF_TOKEN, make_auth_user, make_staff_request};

// ==================== Single delete: Happy path ====================

/// Verify that delete_record succeeds for an existing record
#[rstest]
#[tokio::test]
async fn test_delete_record_happy_path(
	#[future] server_fn_context: (Depends<AdminSite>, Depends<AdminDatabase>),
) {
	// Arrange
	let (site, db) = server_fn_context.await;
	let http_request = make_staff_request();
	let auth_user = make_auth_user();

	let mut data = HashMap::new();
	data.insert("name".to_string(), json!("To Delete"));
	data.insert("status".to_string(), json!("active"));
	let created_id = db
		.create::<AdminRecord>("test_models", None, data)
		.await
		.expect("Failed to create test record");

	// Act
	let result = delete_record(
		"TestModel".to_string(),
		created_id.to_string(),
		TEST_CSRF_TOKEN.to_string(),
		site,
		db,
		http_request,
		auth_user,
	)
	.await;

	// Assert
	assert!(result.is_ok(), "delete_record should succeed: {:?}", result);
	let response = result.unwrap();
	assert!(response.success);
	assert_eq!(response.affected, Some(1));
}

/// Verify that deleted record is actually removed from database
#[rstest]
#[tokio::test]
async fn test_delete_record_actually_removes(
	#[future] server_fn_context: (Depends<AdminSite>, Depends<AdminDatabase>),
) {
	// Arrange
	let (site, db) = server_fn_context.await;
	let http_request = make_staff_request();
	let auth_user = make_auth_user();

	let mut data = HashMap::new();
	data.insert("name".to_string(), json!("Will Be Deleted"));
	data.insert("status".to_string(), json!("active"));
	let created_id = db
		.create::<AdminRecord>("test_models", None, data)
		.await
		.expect("Failed to create test record");

	// Act
	delete_record(
		"TestModel".to_string(),
		created_id.to_string(),
		TEST_CSRF_TOKEN.to_string(),
		site,
		db.clone(),
		http_request,
		auth_user,
	)
	.await
	.expect("delete should succeed");

	// Assert - record should be gone
	let fetched = db
		.get::<AdminRecord>("test_models", "id", &created_id.to_string())
		.await
		.expect("DB query should succeed");
	assert!(
		fetched.is_none(),
		"Record should no longer exist after deletion"
	);
}

// ==================== Single delete: Error path ====================

/// Regression test for Issue #2934: delete non-existent ID should return 404, not success
#[rstest]
#[tokio::test]
async fn test_delete_record_not_found(
	#[future] server_fn_context: (Depends<AdminSite>, Depends<AdminDatabase>),
) {
	// Arrange
	let (site, db) = server_fn_context.await;
	let http_request = make_staff_request();
	let auth_user = make_auth_user();

	// Act
	let result = delete_record(
		"TestModel".to_string(),
		"999999".to_string(),
		TEST_CSRF_TOKEN.to_string(),
		site,
		db,
		http_request,
		auth_user,
	)
	.await;

	// Assert
	assert!(
		result.is_err(),
		"Should return error (404) for non-existent ID, not success"
	);
	let err = format!("{}", result.unwrap_err());
	assert!(
		err.contains("not found") || err.contains("404"),
		"Error should indicate not found: {}",
		err
	);
}

/// Verify that delete_record returns error for non-registered model
#[rstest]
#[tokio::test]
async fn test_delete_record_model_not_registered(
	#[future] server_fn_context: (Depends<AdminSite>, Depends<AdminDatabase>),
) {
	// Arrange
	let (site, db) = server_fn_context.await;
	let http_request = make_staff_request();
	let auth_user = make_auth_user();

	// Act
	let result = delete_record(
		"NonExistentModel".to_string(),
		"1".to_string(),
		TEST_CSRF_TOKEN.to_string(),
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

// ==================== Bulk delete: Happy path ====================

/// Verify that bulk_delete_records deletes multiple records correctly
#[rstest]
#[tokio::test]
async fn test_bulk_delete_happy_path(
	#[future] server_fn_context: (Depends<AdminSite>, Depends<AdminDatabase>),
) {
	// Arrange
	let (site, db) = server_fn_context.await;
	let http_request = make_staff_request();
	let auth_user = make_auth_user();

	let mut ids = Vec::new();
	for i in 0..3 {
		let mut data = HashMap::new();
		data.insert("name".to_string(), json!(format!("Bulk Delete {}", i)));
		data.insert("status".to_string(), json!("active"));
		let id = db
			.create::<AdminRecord>("test_models", None, data)
			.await
			.expect("Failed to create test record");
		ids.push(id.to_string());
	}

	let request = BulkDeleteRequest {
		csrf_token: TEST_CSRF_TOKEN.to_string(),
		ids,
	};

	// Act
	let result = bulk_delete_records(
		"TestModel".to_string(),
		request,
		site,
		db,
		http_request,
		auth_user,
	)
	.await;

	// Assert
	assert!(result.is_ok(), "bulk_delete should succeed: {:?}", result);
	let response = result.unwrap();
	assert_eq!(response.deleted, 3, "Should delete all 3 records");
	assert!(response.success);
}

/// Verify bulk delete with single ID
#[rstest]
#[tokio::test]
async fn test_bulk_delete_single_id(
	#[future] server_fn_context: (Depends<AdminSite>, Depends<AdminDatabase>),
) {
	// Arrange
	let (site, db) = server_fn_context.await;
	let http_request = make_staff_request();
	let auth_user = make_auth_user();

	let mut data = HashMap::new();
	data.insert("name".to_string(), json!("Single Bulk Delete"));
	data.insert("status".to_string(), json!("active"));
	let id = db
		.create::<AdminRecord>("test_models", None, data)
		.await
		.expect("Failed to create test record");

	let request = BulkDeleteRequest {
		csrf_token: TEST_CSRF_TOKEN.to_string(),
		ids: vec![id.to_string()],
	};

	// Act
	let result = bulk_delete_records(
		"TestModel".to_string(),
		request,
		site,
		db,
		http_request,
		auth_user,
	)
	.await;

	// Assert
	assert!(result.is_ok(), "Single-ID bulk delete should succeed");
	assert_eq!(result.unwrap().deleted, 1);
}

// ==================== Bulk delete: Boundary tests ====================

/// Regression test for Issue #2935: bulk_delete should enforce MAX_BULK_DELETE_IDS limit
#[rstest]
#[tokio::test]
async fn test_bulk_delete_exceeds_limit(
	#[future] server_fn_context: (Depends<AdminSite>, Depends<AdminDatabase>),
) {
	// Arrange
	let (site, db) = server_fn_context.await;
	let http_request = make_staff_request();
	let auth_user = make_auth_user();

	// MAX_BULK_DELETE_IDS = 1000, create 1001 IDs
	let ids: Vec<String> = (0..1001).map(|i| i.to_string()).collect();

	let request = BulkDeleteRequest {
		csrf_token: TEST_CSRF_TOKEN.to_string(),
		ids,
	};

	// Act
	let result = bulk_delete_records(
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
		result.is_err(),
		"Should reject bulk delete exceeding MAX_BULK_DELETE_IDS"
	);
	let err = format!("{}", result.unwrap_err());
	assert!(
		err.contains("Too many") || err.contains("1000") || err.contains("exceeds"),
		"Error should mention the limit: {}",
		err
	);
}

// ==================== Bulk delete: Edge cases ====================

/// Verify bulk delete with empty IDs list
#[rstest]
#[tokio::test]
async fn test_bulk_delete_empty_ids(
	#[future] server_fn_context: (Depends<AdminSite>, Depends<AdminDatabase>),
) {
	// Arrange
	let (site, db) = server_fn_context.await;
	let http_request = make_staff_request();
	let auth_user = make_auth_user();

	let request = BulkDeleteRequest {
		csrf_token: TEST_CSRF_TOKEN.to_string(),
		ids: vec![],
	};

	// Act
	let result = bulk_delete_records(
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
		"Empty bulk delete should not error: {:?}",
		result
	);
	let response = result.unwrap();
	assert_eq!(response.deleted, 0, "Should delete 0 records");
	assert!(
		!response.success,
		"Empty delete should report success=false"
	);
}

/// Verify bulk delete with partially matching IDs
#[rstest]
#[tokio::test]
async fn test_bulk_delete_partial_match(
	#[future] server_fn_context: (Depends<AdminSite>, Depends<AdminDatabase>),
) {
	// Arrange
	let (site, db) = server_fn_context.await;
	let http_request = make_staff_request();
	let auth_user = make_auth_user();

	let mut data = HashMap::new();
	data.insert("name".to_string(), json!("Partial Match"));
	data.insert("status".to_string(), json!("active"));
	let existing_id = db
		.create::<AdminRecord>("test_models", None, data)
		.await
		.expect("Failed to create test record");

	// Mix of existing and non-existing IDs
	let request = BulkDeleteRequest {
		csrf_token: TEST_CSRF_TOKEN.to_string(),
		ids: vec![existing_id.to_string(), "999999".to_string()],
	};

	// Act
	let result = bulk_delete_records(
		"TestModel".to_string(),
		request,
		site,
		db,
		http_request,
		auth_user,
	)
	.await;

	// Assert
	assert!(result.is_ok(), "Partial match should not error");
	let response = result.unwrap();
	assert!(
		response.deleted >= 1,
		"Should have deleted at least the existing record"
	);
}

/// Verify bulk_delete returns error for non-registered model
#[rstest]
#[tokio::test]
async fn test_bulk_delete_model_not_registered(
	#[future] server_fn_context: (Depends<AdminSite>, Depends<AdminDatabase>),
) {
	// Arrange
	let (site, db) = server_fn_context.await;
	let http_request = make_staff_request();
	let auth_user = make_auth_user();

	let request = BulkDeleteRequest {
		csrf_token: TEST_CSRF_TOKEN.to_string(),
		ids: vec!["1".to_string()],
	};

	// Act
	let result = bulk_delete_records(
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
