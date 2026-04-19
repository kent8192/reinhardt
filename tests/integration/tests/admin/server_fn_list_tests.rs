//! Integration tests for get_list server function
//!
//! Tests the list view server function with search, filters, sorting, and pagination.
//! Covers regression for Issue #2922 (sort_by not validated against allowed fields).

use super::server_fn_helpers::server_fn_context;
use reinhardt_admin::adapters::ListQueryParams;
use reinhardt_admin::core::AdminRecord;
use reinhardt_admin::core::{AdminDatabase, AdminSite};
use reinhardt_admin::server::get_list;
use reinhardt_di::Depends;
use rstest::*;
use serde_json::json;
use std::collections::HashMap;

use super::server_fn_helpers::make_auth_user;

// ==================== Happy path tests ====================

/// Verify that get_list returns records with correct pagination metadata
#[rstest]
#[tokio::test]
async fn test_get_list_happy_path(
	#[future] server_fn_context: (Depends<AdminSite>, Depends<AdminDatabase>),
) {
	// Arrange
	let (site, db) = server_fn_context.await;
	let auth_user = make_auth_user();

	for i in 0..3 {
		let mut data = HashMap::new();
		data.insert("name".to_string(), json!(format!("Item {}", i)));
		data.insert("status".to_string(), json!("active"));
		db.create::<AdminRecord>("test_models", None, data)
			.await
			.expect("Failed to create test record");
	}

	let params = ListQueryParams::default();

	// Act
	let result = get_list("TestModel".to_string(), params, site, db, auth_user).await;

	// Assert
	assert!(result.is_ok(), "get_list should succeed: {:?}", result);
	let response = result.unwrap();
	assert_eq!(response.model_name, "TestModel");
	assert!(response.count >= 3, "Should have at least 3 records");
	assert_eq!(response.page, 1);
	assert!(response.page_size > 0);
	assert!(response.total_pages >= 1);
}

/// Verify that search filters records by search fields (OR logic)
#[rstest]
#[tokio::test]
async fn test_get_list_with_search(
	#[future] server_fn_context: (Depends<AdminSite>, Depends<AdminDatabase>),
) {
	// Arrange
	let (site, db) = server_fn_context.await;
	let auth_user = make_auth_user();

	let mut data = HashMap::new();
	data.insert("name".to_string(), json!("UniqueSearchTarget"));
	data.insert("status".to_string(), json!("active"));
	db.create::<AdminRecord>("test_models", None, data)
		.await
		.expect("Failed to create test record");

	let params = ListQueryParams {
		search: Some("UniqueSearchTarget".to_string()),
		..Default::default()
	};

	// Act
	let result = get_list("TestModel".to_string(), params, site, db, auth_user).await;

	// Assert
	let response = result.expect("get_list should succeed");
	assert!(
		response.count >= 1,
		"Should find at least 1 matching record"
	);
}

/// Verify that filter by allowed field works
#[rstest]
#[tokio::test]
async fn test_get_list_with_filter(
	#[future] server_fn_context: (Depends<AdminSite>, Depends<AdminDatabase>),
) {
	// Arrange
	let (site, db) = server_fn_context.await;
	let auth_user = make_auth_user();

	let mut data = HashMap::new();
	data.insert("name".to_string(), json!("Filter Test"));
	data.insert("status".to_string(), json!("filterable_status"));
	db.create::<AdminRecord>("test_models", None, data)
		.await
		.expect("Failed to create test record");

	let mut filters = HashMap::new();
	filters.insert("status".to_string(), "filterable_status".to_string());

	let params = ListQueryParams {
		filters,
		..Default::default()
	};

	// Act
	let result = get_list("TestModel".to_string(), params, site, db, auth_user).await;

	// Assert
	let response = result.expect("get_list should succeed with valid filter");
	assert!(
		response.count >= 1,
		"Should find records matching the filter"
	);
}

/// Verify that descending sort with "-" prefix works
#[rstest]
#[tokio::test]
async fn test_get_list_sort_descending(
	#[future] server_fn_context: (Depends<AdminSite>, Depends<AdminDatabase>),
) {
	// Arrange
	let (site, db) = server_fn_context.await;
	let auth_user = make_auth_user();

	let params = ListQueryParams {
		sort_by: Some("-name".to_string()),
		..Default::default()
	};

	// Act
	let result = get_list("TestModel".to_string(), params, site, db, auth_user).await;

	// Assert
	assert!(
		result.is_ok(),
		"get_list should succeed with descending sort: {:?}",
		result
	);
}

// ==================== Validation tests ====================

/// Regression test for Issue #2922: sort_by parameter not validated against allowed fields
#[rstest]
#[tokio::test]
async fn test_get_list_sort_by_invalid_field(
	#[future] server_fn_context: (Depends<AdminSite>, Depends<AdminDatabase>),
) {
	// Arrange
	let (site, db) = server_fn_context.await;
	let auth_user = make_auth_user();

	let params = ListQueryParams {
		sort_by: Some("nonexistent_column".to_string()),
		..Default::default()
	};

	// Act
	let result = get_list("TestModel".to_string(), params, site, db, auth_user).await;

	// Assert
	assert!(
		result.is_err(),
		"Should reject sort_by with invalid field name"
	);
	let err = format!("{}", result.unwrap_err());
	assert!(
		err.contains("sort field") || err.contains("400") || err.contains("Unknown"),
		"Error should indicate invalid sort field: {}",
		err
	);
}

/// Verify that unknown filter field returns 400 error
#[rstest]
#[tokio::test]
async fn test_get_list_unknown_filter_field(
	#[future] server_fn_context: (Depends<AdminSite>, Depends<AdminDatabase>),
) {
	// Arrange
	let (site, db) = server_fn_context.await;
	let auth_user = make_auth_user();

	let mut filters = HashMap::new();
	filters.insert("nonexistent_field".to_string(), "some_value".to_string());

	let params = ListQueryParams {
		filters,
		..Default::default()
	};

	// Act
	let result = get_list("TestModel".to_string(), params, site, db, auth_user).await;

	// Assert
	assert!(
		result.is_err(),
		"Should reject filter with unknown field name"
	);
	let err = format!("{}", result.unwrap_err());
	assert!(
		err.contains("filter field") || err.contains("400") || err.contains("Unknown"),
		"Error should indicate unknown filter field: {}",
		err
	);
}

// ==================== Pagination tests ====================

/// Verify default pagination: page=1, page_size=25
#[rstest]
#[tokio::test]
async fn test_get_list_pagination_defaults(
	#[future] server_fn_context: (Depends<AdminSite>, Depends<AdminDatabase>),
) {
	// Arrange
	let (site, db) = server_fn_context.await;
	let auth_user = make_auth_user();

	// Act
	let result = get_list(
		"TestModel".to_string(),
		ListQueryParams::default(),
		site,
		db,
		auth_user,
	)
	.await;

	// Assert
	let response = result.expect("get_list should succeed");
	assert_eq!(response.page, 1, "Default page should be 1");
	assert!(
		response.page_size <= 500,
		"Default page_size should not exceed MAX_PAGE_SIZE"
	);
}

/// Verify that page_size is capped at MAX_PAGE_SIZE (500)
#[rstest]
#[tokio::test]
async fn test_get_list_page_size_capped(
	#[future] server_fn_context: (Depends<AdminSite>, Depends<AdminDatabase>),
) {
	// Arrange
	let (site, db) = server_fn_context.await;
	let auth_user = make_auth_user();

	let params = ListQueryParams {
		page_size: Some(10000),
		..Default::default()
	};

	// Act
	let result = get_list("TestModel".to_string(), params, site, db, auth_user).await;

	// Assert
	let response = result.expect("get_list should succeed with large page_size");
	assert!(
		response.page_size <= 500,
		"Page size should be capped at MAX_PAGE_SIZE(500), got {}",
		response.page_size
	);
}

/// Verify that page=0 is treated as page=1
#[rstest]
#[tokio::test]
async fn test_get_list_page_zero_treated_as_one(
	#[future] server_fn_context: (Depends<AdminSite>, Depends<AdminDatabase>),
) {
	// Arrange
	let (site, db) = server_fn_context.await;
	let auth_user = make_auth_user();

	let params = ListQueryParams {
		page: Some(0),
		..Default::default()
	};

	// Act
	let result = get_list("TestModel".to_string(), params, site, db, auth_user).await;

	// Assert
	let response = result.expect("get_list should succeed with page=0");
	assert_eq!(response.page, 1, "Page 0 should be treated as page 1");
}

// ==================== Edge case tests ====================

/// Verify that get_list with empty table returns count=0, total_pages=1
#[rstest]
#[tokio::test]
async fn test_get_list_empty_table(
	#[future] server_fn_context: (Depends<AdminSite>, Depends<AdminDatabase>),
) {
	// Arrange
	let (site, db) = server_fn_context.await;
	let auth_user = make_auth_user();

	// Act (no records inserted)
	let result = get_list(
		"TestModel".to_string(),
		ListQueryParams::default(),
		site,
		db,
		auth_user,
	)
	.await;

	// Assert
	let response = result.expect("get_list should succeed on empty table");
	assert_eq!(response.total_pages, 1, "Empty table should have 1 page");
}

// ==================== Contract tests ====================

/// Verify that response columns match model_admin.list_display()
#[rstest]
#[tokio::test]
async fn test_get_list_columns_match_list_display(
	#[future] server_fn_context: (Depends<AdminSite>, Depends<AdminDatabase>),
) {
	// Arrange
	let (site, db) = server_fn_context.await;
	let auth_user = make_auth_user();

	// Act
	let result = get_list(
		"TestModel".to_string(),
		ListQueryParams::default(),
		site,
		db,
		auth_user,
	)
	.await;

	// Assert
	let response = result.expect("get_list should succeed");
	let columns = response.columns.expect("Should have columns");
	let column_names: Vec<&str> = columns.iter().map(|c| c.field.as_str()).collect();
	// model_admin_config fixture has list_display: ["id", "name", "created_at"]
	assert!(column_names.contains(&"id"), "Columns should contain 'id'");
	assert!(
		column_names.contains(&"name"),
		"Columns should contain 'name'"
	);
	assert!(
		column_names.contains(&"created_at"),
		"Columns should contain 'created_at'"
	);
}

/// Verify that response available_filters match model_admin.list_filter()
#[rstest]
#[tokio::test]
async fn test_get_list_filters_match_list_filter(
	#[future] server_fn_context: (Depends<AdminSite>, Depends<AdminDatabase>),
) {
	// Arrange
	let (site, db) = server_fn_context.await;
	let auth_user = make_auth_user();

	// Act
	let result = get_list(
		"TestModel".to_string(),
		ListQueryParams::default(),
		site,
		db,
		auth_user,
	)
	.await;

	// Assert
	let response = result.expect("get_list should succeed");
	let filters = response
		.available_filters
		.expect("Should have available_filters");
	let filter_fields: Vec<&str> = filters.iter().map(|f| f.field.as_str()).collect();
	// model_admin_config fixture has list_filter: ["status"]
	assert!(
		filter_fields.contains(&"status"),
		"Filters should contain 'status'"
	);
}

// ==================== Error path tests ====================

/// Verify that get_list returns error for non-registered model
#[rstest]
#[tokio::test]
async fn test_get_list_model_not_registered(
	#[future] server_fn_context: (Depends<AdminSite>, Depends<AdminDatabase>),
) {
	// Arrange
	let (site, db) = server_fn_context.await;
	let auth_user = make_auth_user();

	// Act
	let result = get_list(
		"NonExistentModel".to_string(),
		ListQueryParams::default(),
		site,
		db,
		auth_user,
	)
	.await;

	// Assert
	assert!(
		result.is_err(),
		"Should return error for unregistered model"
	);
}
