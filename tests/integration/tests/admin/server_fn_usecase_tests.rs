//! Use case tests for admin server functions
//!
//! Tests real-world usage scenarios that combine multiple server functions
//! to accomplish typical admin workflows.

use super::server_fn_helpers::{
	TEST_CSRF_TOKEN, make_auth_user, make_staff_request, server_fn_context,
};
use reinhardt_admin::core::{AdminDatabase, AdminSite, ExportFormat, ImportFormat};
use reinhardt_admin::server::{
	bulk_delete_records, create_record, delete_record, export_data, get_dashboard, get_detail,
	get_fields, get_list, import_data,
};
use reinhardt_admin::types::{BulkDeleteRequest, ListQueryParams, MutationRequest};
use reinhardt_di::Depends;
use rstest::*;
use serde_json::json;
use std::collections::HashMap;

// ==================== Full CRUD Lifecycle ====================

#[rstest]
#[tokio::test]
async fn test_full_crud_lifecycle_usecase(
	#[future] server_fn_context: (Depends<AdminSite>, Depends<AdminDatabase>),
) {
	// Arrange
	let (site, db) = server_fn_context.await;
	let user = make_auth_user();
	let request = make_staff_request();

	// Act & Assert: Create 3 records
	for name in ["Alice", "Bob", "Charlie"] {
		let mut data = HashMap::new();
		data.insert("name".to_string(), json!(name));
		data.insert("status".to_string(), json!("active"));
		let mutation = MutationRequest {
			csrf_token: TEST_CSRF_TOKEN.to_string(),
			data,
		};
		let result = create_record(
			"TestModel".to_string(),
			mutation,
			site.clone(),
			db.clone(),
			request.clone(),
			user.clone(),
		)
		.await;
		assert!(
			result.is_ok(),
			"Failed to create record for {}: {:?}",
			name,
			result.err()
		);
	}

	// Verify count = 3 via list
	let list_params = ListQueryParams::default();
	let list_result = get_list(
		"TestModel".to_string(),
		list_params,
		site.clone(),
		db.clone(),
		user.clone(),
	)
	.await
	.expect("get_list should succeed");
	assert_eq!(list_result.count, 3, "Should have 3 records after creation");

	// Update first record (id=1)
	let mut update_data = HashMap::new();
	update_data.insert("name".to_string(), json!("Alice Updated"));
	let update_mutation = MutationRequest {
		csrf_token: TEST_CSRF_TOKEN.to_string(),
		data: update_data,
	};
	let update_result = reinhardt_admin::server::update_record(
		"TestModel".to_string(),
		"1".to_string(),
		update_mutation,
		site.clone(),
		db.clone(),
		request.clone(),
		user.clone(),
	)
	.await
	.expect("update_record should succeed");
	assert!(update_result.success);

	// Verify update via detail
	let detail_result = get_detail(
		"TestModel".to_string(),
		"1".to_string(),
		site.clone(),
		db.clone(),
		request.clone(),
		user.clone(),
	)
	.await
	.expect("get_detail should succeed");
	let name_value = detail_result
		.data
		.get("name")
		.expect("name field should exist");
	assert_eq!(name_value, &json!("Alice Updated"));

	// Delete one record (id=2)
	let delete_result = delete_record(
		"TestModel".to_string(),
		"2".to_string(),
		TEST_CSRF_TOKEN.to_string(),
		site.clone(),
		db.clone(),
		request.clone(),
		user.clone(),
	)
	.await
	.expect("delete_record should succeed");
	assert!(delete_result.success);

	// Verify count = 2
	let list_result2 = get_list(
		"TestModel".to_string(),
		ListQueryParams::default(),
		site.clone(),
		db.clone(),
		user.clone(),
	)
	.await
	.expect("get_list should succeed");
	assert_eq!(
		list_result2.count, 2,
		"Should have 2 records after deletion"
	);

	// Bulk delete remaining
	let remaining_ids: Vec<String> = list_result2
		.results
		.iter()
		.filter_map(|r| {
			r.get("id")
				.and_then(|v| v.as_i64())
				.map(|id| id.to_string())
		})
		.collect();
	assert_eq!(remaining_ids.len(), 2, "Should have 2 remaining IDs");

	let bulk_req = BulkDeleteRequest {
		csrf_token: TEST_CSRF_TOKEN.to_string(),
		ids: remaining_ids,
	};
	let bulk_result = bulk_delete_records(
		"TestModel".to_string(),
		bulk_req,
		site.clone(),
		db.clone(),
		request.clone(),
		user.clone(),
	)
	.await
	.expect("bulk_delete_records should succeed");
	assert_eq!(bulk_result.deleted, 2);

	// Verify count = 0
	let list_result3 = get_list(
		"TestModel".to_string(),
		ListQueryParams::default(),
		site.clone(),
		db.clone(),
		user.clone(),
	)
	.await
	.expect("get_list should succeed");
	assert_eq!(
		list_result3.count, 0,
		"Should have 0 records after bulk delete"
	);
}

// ==================== Dashboard Shows Registered Models ====================

#[rstest]
#[tokio::test]
async fn test_admin_dashboard_shows_registered_models(
	#[future] server_fn_context: (Depends<AdminSite>, Depends<AdminDatabase>),
) {
	// Arrange
	let (site, _db) = server_fn_context.await;
	let request = make_staff_request();

	// Act
	let auth_user = make_auth_user();
	let result = get_dashboard(site.clone(), request.clone(), auth_user).await;

	// Assert
	let dashboard = result.expect("get_dashboard should succeed");
	let model_names: Vec<&str> = dashboard.models.iter().map(|m| m.name.as_str()).collect();
	assert!(
		model_names.contains(&"TestModel"),
		"Dashboard should contain TestModel, got: {:?}",
		model_names
	);
	assert!(
		!dashboard.site_name.is_empty(),
		"Site name should not be empty"
	);
}

// ==================== Search Then Export ====================

#[rstest]
#[tokio::test]
async fn test_search_then_export_matching_records(
	#[future] server_fn_context: (Depends<AdminSite>, Depends<AdminDatabase>),
) {
	// Arrange: Create records with distinct names
	let (site, db) = server_fn_context.await;
	let user = make_auth_user();
	let request = make_staff_request();

	let names = ["UniqueAlpha", "BetaRecord", "UniqueGamma"];
	for name in &names {
		let mut data = HashMap::new();
		data.insert("name".to_string(), json!(name));
		data.insert("status".to_string(), json!("active"));
		let mutation = MutationRequest {
			csrf_token: TEST_CSRF_TOKEN.to_string(),
			data,
		};
		create_record(
			"TestModel".to_string(),
			mutation,
			site.clone(),
			db.clone(),
			request.clone(),
			user.clone(),
		)
		.await
		.expect("create should succeed");
	}

	// Act: Search for "Unique" prefix
	let search_params = ListQueryParams {
		search: Some("Unique".to_string()),
		..Default::default()
	};
	let search_result = get_list(
		"TestModel".to_string(),
		search_params,
		site.clone(),
		db.clone(),
		user.clone(),
	)
	.await
	.expect("search should succeed");

	// Assert: Only matching records returned
	assert_eq!(
		search_result.count, 2,
		"Search for 'Unique' should match 2 records"
	);

	// Act: Export all records
	let export_result = export_data(
		"TestModel".to_string(),
		ExportFormat::JSON,
		site.clone(),
		db.clone(),
		request.clone(),
		user.clone(),
	)
	.await
	.expect("export should succeed");

	// Assert: Exported data contains all 3 records
	let exported: Vec<serde_json::Value> =
		serde_json::from_slice(&export_result.data).expect("export should be valid JSON");
	assert_eq!(exported.len(), 3, "Export should contain all 3 records");
}

// ==================== Import CSV Then Verify List ====================

#[rstest]
#[tokio::test]
async fn test_import_csv_then_verify_list(
	#[future] server_fn_context: (Depends<AdminSite>, Depends<AdminDatabase>),
) {
	// Arrange
	let (site, db) = server_fn_context.await;
	let user = make_auth_user();
	let request = make_staff_request();

	let csv_data = b"name,status\nAlice,active\nBob,inactive\nCharlie,active".to_vec();

	// Act: Import CSV data
	let import_result = import_data(
		"TestModel".to_string(),
		ImportFormat::CSV,
		csv_data,
		site.clone(),
		db.clone(),
		request.clone(),
		user.clone(),
	)
	.await
	.expect("import should succeed");

	// Assert: All 3 records imported
	assert_eq!(
		import_result.imported, 3,
		"Should import 3 records from CSV"
	);
	assert_eq!(import_result.failed, 0, "No records should fail");

	// Verify via list
	let list_result = get_list(
		"TestModel".to_string(),
		ListQueryParams::default(),
		site.clone(),
		db.clone(),
		user.clone(),
	)
	.await
	.expect("get_list should succeed");
	assert_eq!(
		list_result.count, 3,
		"List should show all 3 imported records"
	);
}

// ==================== Bulk Delete Then Verify Count ====================

#[rstest]
#[tokio::test]
async fn test_bulk_delete_then_verify_count(
	#[future] server_fn_context: (Depends<AdminSite>, Depends<AdminDatabase>),
) {
	// Arrange: Create 5 records
	let (site, db) = server_fn_context.await;
	let user = make_auth_user();
	let request = make_staff_request();

	for i in 1..=5 {
		let mut data = HashMap::new();
		data.insert("name".to_string(), json!(format!("Record_{}", i)));
		data.insert("status".to_string(), json!("active"));
		let mutation = MutationRequest {
			csrf_token: TEST_CSRF_TOKEN.to_string(),
			data,
		};
		create_record(
			"TestModel".to_string(),
			mutation,
			site.clone(),
			db.clone(),
			request.clone(),
			user.clone(),
		)
		.await
		.expect("create should succeed");
	}

	// Act: Bulk delete first 3 records (ids 1, 2, 3)
	let bulk_req = BulkDeleteRequest {
		csrf_token: TEST_CSRF_TOKEN.to_string(),
		ids: vec!["1".to_string(), "2".to_string(), "3".to_string()],
	};
	let bulk_result = bulk_delete_records(
		"TestModel".to_string(),
		bulk_req,
		site.clone(),
		db.clone(),
		request.clone(),
		user.clone(),
	)
	.await
	.expect("bulk_delete should succeed");

	// Assert
	assert_eq!(bulk_result.deleted, 3, "Should delete 3 records");

	// Verify remaining count
	let list_result = get_list(
		"TestModel".to_string(),
		ListQueryParams::default(),
		site.clone(),
		db.clone(),
		user.clone(),
	)
	.await
	.expect("get_list should succeed");
	assert_eq!(list_result.count, 2, "Should have 2 records remaining");
}

// ==================== Get Fields Then Create With Valid Data ====================

#[rstest]
#[tokio::test]
async fn test_get_fields_then_create_with_valid_data(
	#[future] server_fn_context: (Depends<AdminSite>, Depends<AdminDatabase>),
) {
	// Arrange
	let (site, db) = server_fn_context.await;
	let user = make_auth_user();
	let request = make_staff_request();

	// Act: Get field definitions
	let fields_result = get_fields(
		"TestModel".to_string(),
		None,
		site.clone(),
		db.clone(),
		request.clone(),
		user.clone(),
	)
	.await
	.expect("get_fields should succeed");

	// Assert: Fields contain expected field names
	let field_names: Vec<&str> = fields_result
		.fields
		.iter()
		.map(|f| f.name.as_str())
		.collect();
	assert!(
		field_names.contains(&"name"),
		"Fields should include 'name'"
	);
	assert!(
		field_names.contains(&"status"),
		"Fields should include 'status'"
	);

	// Act: Create a record using discovered fields
	let mut data = HashMap::new();
	data.insert("name".to_string(), json!("FieldDiscoveredRecord"));
	data.insert("status".to_string(), json!("active"));
	let mutation = MutationRequest {
		csrf_token: TEST_CSRF_TOKEN.to_string(),
		data,
	};
	let create_result = create_record(
		"TestModel".to_string(),
		mutation,
		site.clone(),
		db.clone(),
		request.clone(),
		user.clone(),
	)
	.await;

	// Assert
	assert!(
		create_result.is_ok(),
		"Create with field-discovered data should succeed"
	);
	let response = create_result.unwrap();
	assert!(response.success);
}
