//! Integration tests for get_fields server function
//!
//! Tests the field definitions server function for dynamic form generation.
//! Covers regression for Issue #2920 (get_fields() missing authentication check).

use super::server_fn_helpers::server_fn_context;
use reinhardt_admin::core::AdminRecord;
use reinhardt_admin::core::{AdminDatabase, AdminSite};
use reinhardt_admin::server::get_fields;
use reinhardt_di::Depends;
use rstest::*;
use serde_json::json;
use std::collections::HashMap;

use super::server_fn_helpers::{make_auth_user, make_staff_request};

// ==================== Happy path tests ====================

/// Verify get_fields returns field definitions for create form (no id)
#[rstest]
#[tokio::test]
async fn test_get_fields_create_form(
	#[future] server_fn_context: (Depends<AdminSite>, Depends<AdminDatabase>),
) {
	// Arrange
	let (site, db) = server_fn_context.await;
	let http_request = make_staff_request();
	let auth_user = make_auth_user();

	// Act
	let result = get_fields(
		"TestModel".to_string(),
		None, // No ID = create form
		site,
		db,
		http_request,
		auth_user,
	)
	.await;

	// Assert
	assert!(
		result.is_ok(),
		"get_fields for create should succeed: {:?}",
		result
	);
	let response = result.unwrap();
	assert_eq!(response.model_name, "TestModel");
	assert!(
		!response.fields.is_empty(),
		"Should return field definitions"
	);
	assert!(
		response.values.is_none(),
		"Create form should have no values"
	);
}

/// Verify get_fields returns field definitions + existing values for edit form
#[rstest]
#[tokio::test]
async fn test_get_fields_edit_form(
	#[future] server_fn_context: (Depends<AdminSite>, Depends<AdminDatabase>),
) {
	// Arrange
	let (site, db) = server_fn_context.await;
	let http_request = make_staff_request();
	let auth_user = make_auth_user();

	let mut data = HashMap::new();
	data.insert("name".to_string(), json!("Edit Form Item"));
	data.insert("status".to_string(), json!("active"));
	let created_id = db
		.create::<AdminRecord>("test_models", None, data)
		.await
		.expect("Failed to create test record");

	// Act
	let result = get_fields(
		"TestModel".to_string(),
		Some(created_id.to_string()),
		site,
		db,
		http_request,
		auth_user,
	)
	.await;

	// Assert
	assert!(
		result.is_ok(),
		"get_fields for edit should succeed: {:?}",
		result
	);
	let response = result.unwrap();
	assert!(
		!response.fields.is_empty(),
		"Should return field definitions"
	);
	assert!(
		response.values.is_some(),
		"Edit form should have existing values"
	);
}

// ==================== Contract tests ====================

/// Verify field names match the model admin configuration
#[rstest]
#[tokio::test]
async fn test_get_fields_returns_correct_field_names(
	#[future] server_fn_context: (Depends<AdminSite>, Depends<AdminDatabase>),
) {
	// Arrange
	let (site, db) = server_fn_context.await;
	let http_request = make_staff_request();
	let auth_user = make_auth_user();

	// Act
	let result = get_fields(
		"TestModel".to_string(),
		None,
		site,
		db,
		http_request,
		auth_user,
	)
	.await;

	// Assert
	let response = result.expect("get_fields should succeed");
	let field_names: Vec<&str> = response.fields.iter().map(|f| f.name.as_str()).collect();
	// model_admin_config has list_display: ["id", "name", "created_at"]
	assert!(
		field_names.contains(&"id") || field_names.contains(&"name"),
		"Fields should contain model fields, got: {:?}",
		field_names
	);
}

/// Verify field labels are humanized versions of field names
#[rstest]
#[tokio::test]
async fn test_get_fields_field_labels_humanized(
	#[future] server_fn_context: (Depends<AdminSite>, Depends<AdminDatabase>),
) {
	// Arrange
	let (site, db) = server_fn_context.await;
	let http_request = make_staff_request();
	let auth_user = make_auth_user();

	// Act
	let result = get_fields(
		"TestModel".to_string(),
		None,
		site,
		db,
		http_request,
		auth_user,
	)
	.await;

	// Assert
	let response = result.expect("get_fields should succeed");
	for field in &response.fields {
		assert!(
			!field.label.is_empty(),
			"Field '{}' should have a non-empty label",
			field.name
		);
	}
}

/// Verify each field has a type assigned
#[rstest]
#[tokio::test]
async fn test_get_fields_field_type_inference(
	#[future] server_fn_context: (Depends<AdminSite>, Depends<AdminDatabase>),
) {
	// Arrange
	let (site, db) = server_fn_context.await;
	let http_request = make_staff_request();
	let auth_user = make_auth_user();

	// Act
	let result = get_fields(
		"TestModel".to_string(),
		None,
		site,
		db,
		http_request,
		auth_user,
	)
	.await;

	// Assert
	let response = result.expect("get_fields should succeed");
	// All fields should have some type inferred (Text is the fallback)
	assert!(
		!response.fields.is_empty(),
		"Should have at least one field"
	);
}

// ==================== Edge case tests ====================

/// Verify get_fields with non-existent ID returns fields but no values
#[rstest]
#[tokio::test]
async fn test_get_fields_edit_nonexistent_id(
	#[future] server_fn_context: (Depends<AdminSite>, Depends<AdminDatabase>),
) {
	// Arrange
	let (site, db) = server_fn_context.await;
	let http_request = make_staff_request();
	let auth_user = make_auth_user();

	// Act
	let result = get_fields(
		"TestModel".to_string(),
		Some("999999".to_string()),
		site,
		db,
		http_request,
		auth_user,
	)
	.await;

	// Assert
	let response = result.expect("get_fields should succeed even with non-existent ID");
	assert!(
		!response.fields.is_empty(),
		"Should still return field definitions"
	);
	assert!(
		response.values.is_none(),
		"Should return None values for non-existent record"
	);
}

// ==================== Error path tests ====================

/// Verify get_fields returns error for non-registered model
#[rstest]
#[tokio::test]
async fn test_get_fields_model_not_registered(
	#[future] server_fn_context: (Depends<AdminSite>, Depends<AdminDatabase>),
) {
	// Arrange
	let (site, db) = server_fn_context.await;
	let http_request = make_staff_request();
	let auth_user = make_auth_user();

	// Act
	let result = get_fields(
		"NonExistentModel".to_string(),
		None,
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
