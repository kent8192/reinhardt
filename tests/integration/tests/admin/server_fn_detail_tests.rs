//! Integration tests for get_detail server function
//!
//! Tests the detail view server function which retrieves a single record by ID.

use super::server_fn_helpers::{server_fn_context, uuid_pk_context};
use reinhardt_admin::core::AdminRecord;
use reinhardt_admin::core::{AdminDatabase, AdminSite};
use reinhardt_admin::server::get_detail;
use reinhardt_di::Depends;
use rstest::*;
use serde_json::json;
use std::collections::HashMap;

use super::server_fn_helpers::{make_auth_user, make_staff_request};

// ==================== Happy path tests ====================

/// Verify that get_detail returns the correct record when given an existing ID
#[rstest]
#[tokio::test]
async fn test_get_detail_happy_path(
	#[future] server_fn_context: (Depends<AdminSite>, Depends<AdminDatabase>),
) {
	// Arrange
	let (site, db) = server_fn_context.await;
	let http_request = make_staff_request();
	let auth_user = make_auth_user();

	let mut data = HashMap::new();
	data.insert("name".to_string(), json!("Detail Test Item"));
	data.insert("status".to_string(), json!("active"));

	let created_id = db
		.create::<AdminRecord>("test_models", None, data)
		.await
		.expect("Failed to create test record");

	// Act
	let result = get_detail(
		"TestModel".to_string(),
		created_id.to_string(),
		site,
		db,
		http_request,
		auth_user,
	)
	.await;

	// Assert
	assert!(result.is_ok(), "get_detail should succeed: {:?}", result);
	let response = result.unwrap();
	assert_eq!(response.model_name, "TestModel");
	assert_eq!(response.data.get("name"), Some(&json!("Detail Test Item")));
	assert_eq!(response.data.get("status"), Some(&json!("active")));
}

/// Verify that get_detail response contains all expected fields
#[rstest]
#[tokio::test]
async fn test_get_detail_returns_all_fields(
	#[future] server_fn_context: (Depends<AdminSite>, Depends<AdminDatabase>),
) {
	// Arrange
	let (site, db) = server_fn_context.await;
	let http_request = make_staff_request();
	let auth_user = make_auth_user();

	let mut data = HashMap::new();
	data.insert("name".to_string(), json!("All Fields Item"));
	data.insert("status".to_string(), json!("pending"));

	let created_id = db
		.create::<AdminRecord>("test_models", None, data)
		.await
		.expect("Failed to create test record");

	// Act
	let result = get_detail(
		"TestModel".to_string(),
		created_id.to_string(),
		site,
		db,
		http_request,
		auth_user,
	)
	.await;

	// Assert
	let response = result.expect("get_detail should succeed");
	assert!(response.data.contains_key("id"), "Should contain id field");
	assert!(
		response.data.contains_key("name"),
		"Should contain name field"
	);
	assert!(
		response.data.contains_key("status"),
		"Should contain status field"
	);
}

/// Verify get_detail works with Unicode and special characters
#[rstest]
#[tokio::test]
async fn test_get_detail_with_various_data_types(
	#[future] server_fn_context: (Depends<AdminSite>, Depends<AdminDatabase>),
) {
	// Arrange
	let (site, db) = server_fn_context.await;
	let http_request = make_staff_request();
	let auth_user = make_auth_user();

	let mut data = HashMap::new();
	data.insert(
		"name".to_string(),
		json!("Unicode \u{30c6}\u{30b9}\u{30c8}"),
	);
	data.insert("status".to_string(), json!("active"));

	let created_id = db
		.create::<AdminRecord>("test_models", None, data)
		.await
		.expect("Failed to create test record");

	// Act
	let result = get_detail(
		"TestModel".to_string(),
		created_id.to_string(),
		site,
		db,
		http_request,
		auth_user,
	)
	.await;

	// Assert
	let response = result.expect("get_detail should succeed");
	assert_eq!(
		response.data.get("name"),
		Some(&json!("Unicode \u{30c6}\u{30b9}\u{30c8}"))
	);
}

// ==================== Error path tests ====================

/// Verify that get_detail returns error for non-existent record ID
#[rstest]
#[tokio::test]
async fn test_get_detail_not_found(
	#[future] server_fn_context: (Depends<AdminSite>, Depends<AdminDatabase>),
) {
	// Arrange
	let (site, db) = server_fn_context.await;
	let http_request = make_staff_request();
	let auth_user = make_auth_user();

	// Act
	let result = get_detail(
		"TestModel".to_string(),
		"999999".to_string(),
		site,
		db,
		http_request,
		auth_user,
	)
	.await;

	// Assert
	assert!(result.is_err(), "Should return error for non-existent ID");
	let err = format!("{}", result.unwrap_err());
	assert!(
		err.contains("not found") || err.contains("404"),
		"Error should indicate not found: {}",
		err
	);
}

/// Verify that get_detail returns error for non-registered model
#[rstest]
#[tokio::test]
async fn test_get_detail_model_not_registered(
	#[future] server_fn_context: (Depends<AdminSite>, Depends<AdminDatabase>),
) {
	// Arrange
	let (site, db) = server_fn_context.await;
	let http_request = make_staff_request();
	let auth_user = make_auth_user();

	// Act
	let result = get_detail(
		"NonExistentModel".to_string(),
		"1".to_string(),
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

// ==================== UUID primary key tests ====================

/// Verify that get_detail works with UUID primary keys (issue #3099)
#[rstest]
#[tokio::test]
async fn test_get_detail_uuid_pk(
	#[future] uuid_pk_context: (Depends<AdminSite>, Depends<AdminDatabase>, sqlx::PgPool),
) {
	// Arrange
	let (site, db, pool) = uuid_pk_context.await;
	let http_request = make_staff_request();
	let auth_user = make_auth_user();

	// Insert a record with UUID PK via sqlx (AdminDatabase::create returns u64,
	// which cannot represent UUIDs)
	let uuid_id = uuid::Uuid::now_v7();
	sqlx::query("INSERT INTO uuid_test_models (id, name, status) VALUES ($1, $2, $3)")
		.bind(uuid_id)
		.bind("UUID Test Item")
		.bind("active")
		.execute(&pool)
		.await
		.expect("Failed to insert UUID test record");

	// Act
	let result = get_detail(
		"UuidModel".to_string(),
		uuid_id.to_string(),
		site,
		db,
		http_request,
		auth_user,
	)
	.await;

	// Assert
	assert!(
		result.is_ok(),
		"get_detail should find UUID PK record: {:?}",
		result
	);
	let response = result.unwrap();
	assert_eq!(response.model_name, "UuidModel");
	assert_eq!(response.data.get("name"), Some(&json!("UUID Test Item")),);
}
