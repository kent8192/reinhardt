//! Integration tests for permission-denial code paths in admin server functions.
//!
//! These tests verify that server functions correctly reject requests when
//! the ModelAdmin denies the required permission. Covers Issue #3118.

use super::server_fn_helpers::{
	deny_all_context, make_auth_user, make_staff_request, view_only_context,
};
use reinhardt_admin::adapters::ListQueryParams;
use reinhardt_admin::adapters::{BulkDeleteRequest, ImportFormat, MutationRequest};
use reinhardt_admin::core::ExportFormat;
use reinhardt_admin::core::{AdminDatabase, AdminSite};
use reinhardt_admin::server::{
	bulk_delete_records, create_record, delete_record, export_data, get_detail, get_fields,
	get_list, import_data, update_record,
};
use rstest::*;
use serde_json::json;
use std::sync::Arc;

use super::server_fn_helpers::TEST_CSRF_TOKEN;

// ==================== DenyAll permission tests ====================

/// Verify get_list is denied when view permission is false
#[rstest]
#[tokio::test]
async fn test_deny_all_get_list(#[future] deny_all_context: (Arc<AdminSite>, Arc<AdminDatabase>)) {
	// Arrange
	let (site, db) = deny_all_context.await;
	let auth_user = make_auth_user();
	let params = ListQueryParams::default();

	// Act
	let result = get_list("TestModel".to_string(), params, site, db, auth_user).await;

	// Assert
	assert!(result.is_err(), "get_list should be denied");
	let err = format!("{}", result.unwrap_err());
	assert!(
		err.contains("Permission denied") || err.contains("403"),
		"Error should indicate permission denied: {}",
		err
	);
}

/// Verify get_detail is denied when view permission is false
#[rstest]
#[tokio::test]
async fn test_deny_all_get_detail(
	#[future] deny_all_context: (Arc<AdminSite>, Arc<AdminDatabase>),
) {
	// Arrange
	let (site, db) = deny_all_context.await;
	let http_request = make_staff_request();
	let auth_user = make_auth_user();

	// Act
	let result = get_detail(
		"TestModel".to_string(),
		"1".to_string(),
		site,
		db,
		http_request,
		auth_user,
	)
	.await;

	// Assert
	assert!(result.is_err(), "get_detail should be denied");
	let err = format!("{}", result.unwrap_err());
	assert!(
		err.contains("Permission denied") || err.contains("403"),
		"Error should indicate permission denied: {}",
		err
	);
}

/// Verify get_fields is denied when view permission is false
#[rstest]
#[tokio::test]
async fn test_deny_all_get_fields(
	#[future] deny_all_context: (Arc<AdminSite>, Arc<AdminDatabase>),
) {
	// Arrange
	let (site, db) = deny_all_context.await;
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
	assert!(result.is_err(), "get_fields should be denied");
	let err = format!("{}", result.unwrap_err());
	assert!(
		err.contains("Permission denied") || err.contains("403"),
		"Error should indicate permission denied: {}",
		err
	);
}

/// Verify export_data is denied when view permission is false
#[rstest]
#[tokio::test]
async fn test_deny_all_export(#[future] deny_all_context: (Arc<AdminSite>, Arc<AdminDatabase>)) {
	// Arrange
	let (site, db) = deny_all_context.await;
	let http_request = make_staff_request();
	let auth_user = make_auth_user();

	// Act
	let result = export_data(
		"TestModel".to_string(),
		ExportFormat::JSON,
		site,
		db,
		http_request,
		auth_user,
	)
	.await;

	// Assert
	assert!(result.is_err(), "export_data should be denied");
	let err = format!("{}", result.unwrap_err());
	assert!(
		err.contains("Permission denied") || err.contains("403"),
		"Error should indicate permission denied: {}",
		err
	);
}

/// Verify create_record is denied when add permission is false
#[rstest]
#[tokio::test]
async fn test_deny_all_create(#[future] deny_all_context: (Arc<AdminSite>, Arc<AdminDatabase>)) {
	// Arrange
	let (site, db) = deny_all_context.await;
	let http_request = make_staff_request();
	let auth_user = make_auth_user();

	let request = MutationRequest {
		csrf_token: TEST_CSRF_TOKEN.to_string(),
		data: serde_json::Map::from_iter([
			("name".to_string(), json!("test")),
			("status".to_string(), json!("active")),
		]),
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
	assert!(result.is_err(), "create_record should be denied");
	let err = format!("{}", result.unwrap_err());
	assert!(
		err.contains("Permission denied") || err.contains("403"),
		"Error should indicate permission denied: {}",
		err
	);
}

/// Verify update_record is denied when change permission is false
#[rstest]
#[tokio::test]
async fn test_deny_all_update(#[future] deny_all_context: (Arc<AdminSite>, Arc<AdminDatabase>)) {
	// Arrange
	let (site, db) = deny_all_context.await;
	let http_request = make_staff_request();
	let auth_user = make_auth_user();

	let request = MutationRequest {
		csrf_token: TEST_CSRF_TOKEN.to_string(),
		data: serde_json::Map::from_iter([("name".to_string(), json!("updated"))]),
	};

	// Act
	let result = update_record(
		"TestModel".to_string(),
		"1".to_string(),
		request,
		site,
		db,
		http_request,
		auth_user,
	)
	.await;

	// Assert
	assert!(result.is_err(), "update_record should be denied");
	let err = format!("{}", result.unwrap_err());
	assert!(
		err.contains("Permission denied") || err.contains("403"),
		"Error should indicate permission denied: {}",
		err
	);
}

/// Verify delete_record is denied when delete permission is false
#[rstest]
#[tokio::test]
async fn test_deny_all_delete(#[future] deny_all_context: (Arc<AdminSite>, Arc<AdminDatabase>)) {
	// Arrange
	let (site, db) = deny_all_context.await;
	let http_request = make_staff_request();
	let auth_user = make_auth_user();

	// Act
	let result = delete_record(
		"TestModel".to_string(),
		"1".to_string(),
		TEST_CSRF_TOKEN.to_string(),
		site,
		db,
		http_request,
		auth_user,
	)
	.await;

	// Assert
	assert!(result.is_err(), "delete_record should be denied");
	let err = format!("{}", result.unwrap_err());
	assert!(
		err.contains("Permission denied") || err.contains("403"),
		"Error should indicate permission denied: {}",
		err
	);
}

/// Verify bulk_delete_records is denied when delete permission is false
#[rstest]
#[tokio::test]
async fn test_deny_all_bulk_delete(
	#[future] deny_all_context: (Arc<AdminSite>, Arc<AdminDatabase>),
) {
	// Arrange
	let (site, db) = deny_all_context.await;
	let http_request = make_staff_request();
	let auth_user = make_auth_user();

	let request = BulkDeleteRequest {
		csrf_token: TEST_CSRF_TOKEN.to_string(),
		ids: vec!["1".to_string()],
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
	assert!(result.is_err(), "bulk_delete_records should be denied");
	let err = format!("{}", result.unwrap_err());
	assert!(
		err.contains("Permission denied") || err.contains("403"),
		"Error should indicate permission denied: {}",
		err
	);
}

/// Verify import_data is denied when add permission is false
#[rstest]
#[tokio::test]
async fn test_deny_all_import(#[future] deny_all_context: (Arc<AdminSite>, Arc<AdminDatabase>)) {
	// Arrange
	let (site, db) = deny_all_context.await;
	let http_request = make_staff_request();
	let auth_user = make_auth_user();

	let csv_data = b"name,status\ntest,active".to_vec();

	// Act
	let result = import_data(
		"TestModel".to_string(),
		ImportFormat::CSV,
		csv_data,
		site,
		db,
		http_request,
		auth_user,
	)
	.await;

	// Assert
	assert!(result.is_err(), "import_data should be denied");
	let err = format!("{}", result.unwrap_err());
	assert!(
		err.contains("Permission denied") || err.contains("403"),
		"Error should indicate permission denied: {}",
		err
	);
}

// ==================== ViewOnly permission tests ====================

/// Verify get_list succeeds with view-only permission
#[rstest]
#[tokio::test]
async fn test_view_only_get_list_allowed(
	#[future] view_only_context: (Arc<AdminSite>, Arc<AdminDatabase>),
) {
	// Arrange
	let (site, db) = view_only_context.await;
	let auth_user = make_auth_user();
	let params = ListQueryParams::default();

	// Act
	let result = get_list("TestModel".to_string(), params, site, db, auth_user).await;

	// Assert
	assert!(
		result.is_ok(),
		"get_list should succeed with view permission: {:?}",
		result
	);
	let response = result.unwrap();
	assert_eq!(response.model_name, "TestModel");
}

/// Verify get_detail succeeds with view-only permission
#[rstest]
#[tokio::test]
async fn test_view_only_get_detail_allowed(
	#[future] view_only_context: (Arc<AdminSite>, Arc<AdminDatabase>),
) {
	// Arrange
	let (site, db) = view_only_context.await;
	let http_request = make_staff_request();
	let auth_user = make_auth_user();

	// Act
	let result = get_detail(
		"TestModel".to_string(),
		"1".to_string(),
		site,
		db,
		http_request,
		auth_user,
	)
	.await;

	// Assert
	assert!(
		result.is_ok(),
		"get_detail should succeed with view permission: {:?}",
		result
	);
}

/// Verify create_record is denied with view-only permission
#[rstest]
#[tokio::test]
async fn test_view_only_create_denied(
	#[future] view_only_context: (Arc<AdminSite>, Arc<AdminDatabase>),
) {
	// Arrange
	let (site, db) = view_only_context.await;
	let http_request = make_staff_request();
	let auth_user = make_auth_user();

	let request = MutationRequest {
		csrf_token: TEST_CSRF_TOKEN.to_string(),
		data: serde_json::Map::from_iter([
			("name".to_string(), json!("should fail")),
			("status".to_string(), json!("active")),
		]),
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
		result.is_err(),
		"create_record should be denied with view-only"
	);
	let err = format!("{}", result.unwrap_err());
	assert!(
		err.contains("Permission denied") || err.contains("403"),
		"Error should indicate permission denied: {}",
		err
	);
}

/// Verify update_record is denied with view-only permission
#[rstest]
#[tokio::test]
async fn test_view_only_update_denied(
	#[future] view_only_context: (Arc<AdminSite>, Arc<AdminDatabase>),
) {
	// Arrange
	let (site, db) = view_only_context.await;
	let http_request = make_staff_request();
	let auth_user = make_auth_user();

	let request = MutationRequest {
		csrf_token: TEST_CSRF_TOKEN.to_string(),
		data: serde_json::Map::from_iter([("name".to_string(), json!("updated"))]),
	};

	// Act
	let result = update_record(
		"TestModel".to_string(),
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
		"update_record should be denied with view-only"
	);
	let err = format!("{}", result.unwrap_err());
	assert!(
		err.contains("Permission denied") || err.contains("403"),
		"Error should indicate permission denied: {}",
		err
	);
}

/// Verify delete_record is denied with view-only permission
#[rstest]
#[tokio::test]
async fn test_view_only_delete_denied(
	#[future] view_only_context: (Arc<AdminSite>, Arc<AdminDatabase>),
) {
	// Arrange
	let (site, db) = view_only_context.await;
	let http_request = make_staff_request();
	let auth_user = make_auth_user();

	// Act
	let result = delete_record(
		"TestModel".to_string(),
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
		"delete_record should be denied with view-only"
	);
	let err = format!("{}", result.unwrap_err());
	assert!(
		err.contains("Permission denied") || err.contains("403"),
		"Error should indicate permission denied: {}",
		err
	);
}
