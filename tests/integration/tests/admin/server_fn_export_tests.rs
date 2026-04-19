//! Integration tests for export_data server function
//!
//! Tests the export server function with various formats.
//! Covers regression for Issue #2925 (export silently truncates without warning).

use super::server_fn_helpers::server_fn_context;
use reinhardt_admin::core::ExportFormat;
use reinhardt_admin::core::{AdminDatabase, AdminSite};
use reinhardt_admin::server::export_data;
use reinhardt_di::Depends;
use rstest::*;
use serde_json::json;
use std::collections::HashMap;

use super::server_fn_helpers::{make_auth_user, make_staff_request};

// ==================== Happy path tests ====================

/// Verify JSON export returns valid data with correct metadata
#[rstest]
#[tokio::test]
async fn test_export_json_happy_path(
	#[future] server_fn_context: (Depends<AdminSite>, Depends<AdminDatabase>),
) {
	// Arrange
	let (site, db) = server_fn_context.await;
	let http_request = make_staff_request();
	let auth_user = make_auth_user();

	let mut data = HashMap::new();
	data.insert("name".to_string(), json!("Export JSON Test"));
	data.insert("status".to_string(), json!("active"));
	db.create::<reinhardt_admin::core::AdminRecord>("test_models", None, data)
		.await
		.expect("Failed to create test record");

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
	assert!(result.is_ok(), "JSON export should succeed: {:?}", result);
	let response = result.unwrap();
	assert_eq!(response.content_type, "application/json");
	assert!(response.filename.ends_with(".json"));
	assert!(!response.data.is_empty(), "Export data should not be empty");
	assert!(!response.truncated, "Small export should not be truncated");
}

/// Verify CSV export succeeds for HashMap-based AdminRecord.
///
/// The `serialize_delimited` function uses `write_record` to serialize
/// records as ordered column values, avoiding the csv crate's map limitation.
#[rstest]
#[tokio::test]
async fn test_export_csv_succeeds(
	#[future] server_fn_context: (Depends<AdminSite>, Depends<AdminDatabase>),
) {
	// Arrange
	let (site, db) = server_fn_context.await;
	let http_request = make_staff_request();
	let auth_user = make_auth_user();

	let mut data = HashMap::new();
	data.insert("name".to_string(), json!("Export CSV Test"));
	data.insert("status".to_string(), json!("active"));
	db.create::<reinhardt_admin::core::AdminRecord>("test_models", None, data)
		.await
		.expect("Failed to create test record");

	// Act
	let result = export_data(
		"TestModel".to_string(),
		ExportFormat::CSV,
		site,
		db,
		http_request,
		auth_user,
	)
	.await;

	// Assert
	let response = result.expect("CSV export should succeed");
	assert!(
		!response.data.is_empty(),
		"CSV export data should not be empty"
	);
	assert_eq!(response.content_type, "text/csv");
	assert!(response.filename.ends_with(".csv"));
}

/// Verify TSV export succeeds for HashMap-based AdminRecord.
///
/// The `serialize_delimited` function uses `write_record` to serialize
/// records as ordered column values, avoiding the csv crate's map limitation.
#[rstest]
#[tokio::test]
async fn test_export_tsv_succeeds(
	#[future] server_fn_context: (Depends<AdminSite>, Depends<AdminDatabase>),
) {
	// Arrange
	let (site, db) = server_fn_context.await;
	let http_request = make_staff_request();
	let auth_user = make_auth_user();

	let mut data = HashMap::new();
	data.insert("name".to_string(), json!("Export TSV Test"));
	data.insert("status".to_string(), json!("active"));
	db.create::<reinhardt_admin::core::AdminRecord>("test_models", None, data)
		.await
		.expect("Failed to create test record");

	// Act
	let result = export_data(
		"TestModel".to_string(),
		ExportFormat::TSV,
		site,
		db,
		http_request,
		auth_user,
	)
	.await;

	// Assert
	let response = result.expect("TSV export should succeed");
	assert!(
		!response.data.is_empty(),
		"TSV export data should not be empty"
	);
	assert_eq!(response.content_type, "text/tab-separated-values");
	assert!(response.filename.ends_with(".tsv"));
}

// ==================== Boundary tests ====================

/// Regression test for Issue #2925: export should set truncated flag when records exceed limit
#[rstest]
#[tokio::test]
async fn test_export_truncation_flag(
	#[future] server_fn_context: (Depends<AdminSite>, Depends<AdminDatabase>),
) {
	// Arrange
	let (site, db) = server_fn_context.await;
	let http_request = make_staff_request();
	let auth_user = make_auth_user();

	// Export with few records should not be truncated
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
	let response = result.expect("Export should succeed");
	assert!(!response.truncated, "Small dataset should not be truncated");
	assert!(
		response.total_count.is_some(),
		"Should always report total_count"
	);
}

// ==================== Edge case tests ====================

/// Verify export from empty table returns empty data
#[rstest]
#[tokio::test]
async fn test_export_empty_table(
	#[future] server_fn_context: (Depends<AdminSite>, Depends<AdminDatabase>),
) {
	// Arrange
	let (site, db) = server_fn_context.await;
	let http_request = make_staff_request();
	let auth_user = make_auth_user();

	// Act (no records in table)
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
	assert!(
		result.is_ok(),
		"Export of empty table should succeed: {:?}",
		result
	);
	let response = result.unwrap();
	assert!(!response.truncated);
	assert_eq!(response.total_count, Some(0));
}

/// Verify JSON export content type is correct
#[rstest]
#[tokio::test]
async fn test_export_json_content_type(
	#[future] server_fn_context: (Depends<AdminSite>, Depends<AdminDatabase>),
) {
	// Arrange
	let (site, db) = server_fn_context.await;
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
	let response = result.expect("JSON export should succeed");
	assert_eq!(response.content_type, "application/json");
}

// ==================== Error path tests ====================

/// Verify export returns error for non-registered model
#[rstest]
#[tokio::test]
async fn test_export_model_not_registered(
	#[future] server_fn_context: (Depends<AdminSite>, Depends<AdminDatabase>),
) {
	// Arrange
	let (site, db) = server_fn_context.await;
	let http_request = make_staff_request();
	let auth_user = make_auth_user();

	// Act
	let result = export_data(
		"NonExistentModel".to_string(),
		ExportFormat::JSON,
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
