//! Integration tests for state transitions and data lifecycle
//!
//! Tests that verify state transitions, full CRUD lifecycle, import/export round-trips,
//! and edge cases for non-existent records.

use super::server_fn_helpers::{
	AllPermissionsModelAdmin, TEST_CSRF_TOKEN, make_auth_user, make_staff_request,
	server_fn_context,
};
use reinhardt_admin::adapters::{BulkDeleteRequest, ListQueryParams, MutationRequest};
use reinhardt_admin::core::{AdminDatabase, AdminRecord, AdminSite, ExportFormat, ImportFormat};
use reinhardt_admin::server::{
	bulk_delete_records, create_record, delete_record, export_data, get_detail, get_list,
	import_data, update_record,
};
use reinhardt_di::Depends;
use rstest::*;
use serde_json::json;
use std::collections::HashMap;

// ==================== Full lifecycle tests ====================

/// Create → list → update → verify → delete → verify gone
#[rstest]
#[tokio::test]
async fn test_create_then_update_then_delete_lifecycle(
	#[future] server_fn_context: (Depends<AdminSite>, Depends<AdminDatabase>),
) {
	// Arrange
	let (site, db) = server_fn_context.await;

	// Step 1: Create a record
	let mut data = HashMap::new();
	data.insert("name".to_string(), json!("Lifecycle Item"));
	data.insert("status".to_string(), json!("draft"));

	let create_request = MutationRequest {
		csrf_token: TEST_CSRF_TOKEN.to_string(),
		data,
	};

	let create_result = create_record(
		"TestModel".to_string(),
		create_request,
		site.clone(),
		db.clone(),
		make_staff_request(),
		make_auth_user(),
	)
	.await;
	let create_response = create_result.expect("Create should succeed");
	assert!(create_response.success);
	let created_id = create_response
		.affected
		.expect("Should return affected count")
		.to_string();

	// Step 2: Verify the record appears in the list
	let list_result = get_list(
		"TestModel".to_string(),
		ListQueryParams::default(),
		site.clone(),
		db.clone(),
		make_auth_user(),
	)
	.await;
	let list_response = list_result.expect("List should succeed");
	assert!(
		list_response.count >= 1,
		"List should contain at least 1 record"
	);

	// Step 3: Update the record
	let mut update_data = HashMap::new();
	update_data.insert("name".to_string(), json!("Lifecycle Item Updated"));
	update_data.insert("status".to_string(), json!("active"));

	let update_request = MutationRequest {
		csrf_token: TEST_CSRF_TOKEN.to_string(),
		data: update_data,
	};

	let update_result = update_record(
		"TestModel".to_string(),
		created_id.clone(),
		update_request,
		site.clone(),
		db.clone(),
		make_staff_request(),
		make_auth_user(),
	)
	.await;
	assert!(
		update_result.is_ok(),
		"Update should succeed: {:?}",
		update_result
	);

	// Step 4: Verify the update via get_detail
	let detail_result = get_detail(
		"TestModel".to_string(),
		created_id.clone(),
		site.clone(),
		db.clone(),
		make_staff_request(),
		make_auth_user(),
	)
	.await;
	let detail_response = detail_result.expect("Detail should succeed after update");
	assert_eq!(
		detail_response.data.get("name"),
		Some(&json!("Lifecycle Item Updated")),
		"Name should reflect update"
	);
	assert_eq!(
		detail_response.data.get("status"),
		Some(&json!("active")),
		"Status should reflect update"
	);

	// Step 5: Delete the record
	let delete_result = delete_record(
		"TestModel".to_string(),
		created_id.clone(),
		TEST_CSRF_TOKEN.to_string(),
		site.clone(),
		db.clone(),
		make_staff_request(),
		make_auth_user(),
	)
	.await;
	assert!(
		delete_result.is_ok(),
		"Delete should succeed: {:?}",
		delete_result
	);

	// Step 6: Verify the record is gone
	let detail_after_delete = get_detail(
		"TestModel".to_string(),
		created_id,
		site,
		db,
		make_staff_request(),
		make_auth_user(),
	)
	.await;
	assert!(
		detail_after_delete.is_err(),
		"Detail should fail after deletion"
	);
}

// ==================== Registration lifecycle ====================

/// Register model → verify → unregister → verify gone → re-register → verify restored
#[rstest]
fn test_register_unregister_reregister() {
	// Arrange
	let site = AdminSite::new("Registration Test Site");

	// Step 1: Register model
	let admin = AllPermissionsModelAdmin::test_model("test_table");
	let register_result = site.register("RegistrationTest", admin);

	// Assert: registration succeeds
	assert!(
		register_result.is_ok(),
		"Initial registration should succeed"
	);
	assert!(
		site.get_model_admin("RegistrationTest").is_ok(),
		"Model should be found after registration"
	);

	// Step 2: Unregister model
	let unregister_result = site.unregister("RegistrationTest");
	assert!(unregister_result.is_ok(), "Unregistration should succeed");

	// Step 3: Verify model is gone
	assert!(
		site.get_model_admin("RegistrationTest").is_err(),
		"Model should not be found after unregistration"
	);

	// Step 4: Re-register model
	let admin2 = AllPermissionsModelAdmin::test_model("test_table");
	let reregister_result = site.register("RegistrationTest", admin2);

	// Assert: re-registration succeeds
	assert!(reregister_result.is_ok(), "Re-registration should succeed");
	assert!(
		site.get_model_admin("RegistrationTest").is_ok(),
		"Model should be found after re-registration"
	);
}

// ==================== Import/Export round-trip ====================

/// Import JSON → export as JSON → verify data matches
#[rstest]
#[tokio::test]
async fn test_import_then_export_round_trip_json(
	#[future] server_fn_context: (Depends<AdminSite>, Depends<AdminDatabase>),
) {
	// Arrange
	let (site, db) = server_fn_context.await;

	let import_records = serde_json::json!([
		{"name": "Round Trip 1", "status": "active"},
		{"name": "Round Trip 2", "status": "draft"},
		{"name": "Round Trip 3", "status": "active"}
	]);
	let json_data = serde_json::to_vec(&import_records).expect("Serialization should succeed");

	// Act: Import
	let import_result = import_data(
		"TestModel".to_string(),
		ImportFormat::JSON,
		json_data,
		site.clone(),
		db.clone(),
		make_staff_request(),
		make_auth_user(),
	)
	.await;
	let import_response = import_result.expect("Import should succeed");
	assert_eq!(import_response.imported, 3, "Should import 3 records");

	// Act: Export
	let export_result = export_data(
		"TestModel".to_string(),
		ExportFormat::JSON,
		site,
		db,
		make_staff_request(),
		make_auth_user(),
	)
	.await;
	let export_response = export_result.expect("Export should succeed");

	// Assert: exported data contains imported records
	let exported: Vec<HashMap<String, serde_json::Value>> =
		serde_json::from_slice(&export_response.data).expect("Export should be valid JSON");
	assert!(
		exported.len() >= 3,
		"Should have at least 3 exported records, got {}",
		exported.len()
	);

	// Verify imported names appear in export
	let exported_names: Vec<&str> = exported
		.iter()
		.filter_map(|r| r.get("name").and_then(|v| v.as_str()))
		.collect();
	assert!(
		exported_names.contains(&"Round Trip 1"),
		"Export should contain 'Round Trip 1'"
	);
	assert!(
		exported_names.contains(&"Round Trip 2"),
		"Export should contain 'Round Trip 2'"
	);
	assert!(
		exported_names.contains(&"Round Trip 3"),
		"Export should contain 'Round Trip 3'"
	);
}

// ==================== Site configuration ====================

/// Register model → change site config → verify config applied
#[rstest]
fn test_configure_site_after_register() {
	// Arrange
	let site = AdminSite::new("Config Test Site");
	let admin = AllPermissionsModelAdmin::test_model("test_table");
	site.register("ConfigModel", admin)
		.expect("Registration should succeed");

	// Act: change site configuration
	site.configure(|config| {
		config.site_title = "New Title".to_string();
		config.site_header = "New Header".to_string();
		config.list_per_page = 50;
	});

	// Assert: config is applied and model is still registered
	let config = site.config();
	assert_eq!(config.site_title, "New Title");
	assert_eq!(config.site_header, "New Header");
	assert_eq!(config.list_per_page, 50);
	assert!(
		site.get_model_admin("ConfigModel").is_ok(),
		"Model should still be registered after config change"
	);
}

// ==================== Edge case tests ====================

/// get_detail with non-existent ID returns error
#[rstest]
#[tokio::test]
async fn test_detail_nonexistent_id(
	#[future] server_fn_context: (Depends<AdminSite>, Depends<AdminDatabase>),
) {
	// Arrange
	let (site, db) = server_fn_context.await;
	let http_request = make_staff_request();
	let auth_user = make_auth_user();

	// Act
	let result = get_detail(
		"TestModel".to_string(),
		"99999".to_string(),
		site,
		db,
		http_request,
		auth_user,
	)
	.await;

	// Assert
	assert!(
		result.is_err(),
		"get_detail with non-existent ID should return error"
	);
}

/// update_record with non-existent ID returns error
#[rstest]
#[tokio::test]
async fn test_update_nonexistent_id(
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
		"99999".to_string(),
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
		"update_record with non-existent ID should return error"
	);
}

/// delete_record with non-existent ID returns error
#[rstest]
#[tokio::test]
async fn test_delete_nonexistent_id(
	#[future] server_fn_context: (Depends<AdminSite>, Depends<AdminDatabase>),
) {
	// Arrange
	let (site, db) = server_fn_context.await;
	let http_request = make_staff_request();
	let auth_user = make_auth_user();

	// Act
	let result = delete_record(
		"TestModel".to_string(),
		"99999".to_string(),
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
		"delete_record with non-existent ID should return error"
	);
}

/// get_list with page exceeding total returns empty results but valid metadata
#[rstest]
#[tokio::test]
async fn test_list_page_exceeds_total(
	#[future] server_fn_context: (Depends<AdminSite>, Depends<AdminDatabase>),
) {
	// Arrange
	let (site, db) = server_fn_context.await;
	let auth_user = make_auth_user();

	// Insert one record to ensure there is at least some data
	let mut data = HashMap::new();
	data.insert("name".to_string(), json!("Page Exceed Test"));
	data.insert("status".to_string(), json!("active"));
	db.create::<AdminRecord>("test_models", None, data)
		.await
		.expect("Failed to create test record");

	let params = ListQueryParams {
		page: Some(9999),
		..Default::default()
	};

	// Act
	let result = get_list("TestModel".to_string(), params, site, db, auth_user).await;

	// Assert
	assert!(
		result.is_ok(),
		"get_list with high page number should not error: {:?}",
		result
	);
	let response = result.unwrap();
	assert_eq!(response.page, 9999, "Page should reflect requested page");
	assert!(
		response.results.is_empty(),
		"Results should be empty for page beyond total: got {} results",
		response.results.len()
	);
	assert!(
		response.total_pages >= 1,
		"Total pages should be at least 1"
	);
}

/// bulk_delete with empty IDs list
#[rstest]
#[tokio::test]
async fn test_bulk_delete_empty_ids_list(
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
}
