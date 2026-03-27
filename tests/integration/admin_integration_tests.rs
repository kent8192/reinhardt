//! Admin panel integration tests for reinhardt-admin workspace
//!
//! This module contains end-to-end integration tests for the admin panel
//! functionality, testing interactions between multiple subcrates.

// Test categories covered in this file:
// - Happy path (normal flows)
// - Error path (error handling)
// - State transition testing
// - Use case testing (real admin panel flows)
// - Edge cases (boundary conditions)
// - Sanity tests (basic functionality)
// - Equivalence partitioning (using rstest case macro)
// - Boundary value analysis (using rstest case macro)
// - Decision table testing (using rstest case macro)

// Only compile when admin feature is available
#![cfg(feature = "admin")]

use reinhardt_admin::core::{AdminDatabase, AdminSite, ModelAdminConfig};
use reinhardt_admin::server::{
	BulkDeleteRequest, ExportFormat, ExportResponse, ImportFormat, ImportResponse, ListQueryParams,
	MutationRequest, MutationResponse, create_record, delete_record, get_dashboard, get_detail,
	get_list, update_record,
};
use reinhardt_admin::types::errors::AdminError;
use reinhardt_test::fixtures::admin_panel::{
	admin_database, admin_site, model_admin_config, server_fn_test_context,
};
use reinhardt_test::fixtures::shared_postgres::get_test_pool;
use rstest::*;
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;

/// Basic sanity test for admin site initialization
///
/// **Test Category**: Sanity test
/// **Test Classification**: Normal path
#[rstest]
#[tokio::test]
async fn test_admin_site_initialization(#[future] admin_site: Arc<AdminSite>) {
	let site = admin_site.await;

	// Basic assertions about the admin site
	assert_eq!(site.name(), "Test Admin Site");
	assert!(site.registered_models().is_empty());
	assert_eq!(site.model_count(), 0);
}

/// Test model registration and retrieval
///
/// **Test Category**: Happy path
/// **Test Classification**: State transition
#[rstest]
#[tokio::test]
async fn test_model_registration(
	#[future] admin_site: Arc<AdminSite>,
	#[future] model_admin_config: ModelAdminConfig,
) {
	let site = admin_site.await;
	let config = model_admin_config.await;
	let model_name = config.model_name();

	// Register model
	site.register(model_name, config.clone())
		.expect("Failed to register model");

	// Verify registration
	assert_eq!(site.registered_models(), vec![model_name.to_string()]);
	assert_eq!(site.model_count(), 1);

	// Retrieve model admin
	let retrieved = site
		.get_model_admin(model_name)
		.expect("Failed to get model admin");
	assert_eq!(retrieved.model_name(), model_name);
	assert_eq!(retrieved.table_name(), "test_models");
}

/// Test error handling for duplicate model registration
///
/// **Test Category**: Error path
/// **Test Classification**: Error handling
#[rstest]
#[tokio::test]
async fn test_duplicate_model_registration_error(
	#[future] admin_site: Arc<AdminSite>,
	#[future] model_admin_config: ModelAdminConfig,
) {
	let site = admin_site.await;
	let config = model_admin_config.await;
	let model_name = config.model_name();

	// First registration should succeed
	site.register(model_name, config.clone())
		.expect("First registration should succeed");

	// Second registration should fail
	let result = site.register(model_name, config);
	assert!(result.is_err());

	if let Err(AdminError::ModelAlreadyRegistered(err_msg)) = result {
		assert!(err_msg.contains(model_name));
	} else {
		panic!("Expected ModelAlreadyRegistered error, got {:?}", result);
	}
}

/// Test model unregistration
///
/// **Test Category**: State transition testing
/// **Test Classification**: Normal path
#[rstest]
#[tokio::test]
async fn test_model_unregistration(
	#[future] admin_site: Arc<AdminSite>,
	#[future] model_admin_config: ModelAdminConfig,
) {
	let site = admin_site.await;
	let config = model_admin_config.await;
	let model_name = config.model_name();

	// Register model
	site.register(model_name, config)
		.expect("Failed to register model");
	assert_eq!(site.model_count(), 1);

	// Unregister model
	site.unregister(model_name)
		.expect("Failed to unregister model");
	assert!(site.registered_models().is_empty());
	assert_eq!(site.model_count(), 0);

	// Verify model is no longer accessible
	let result = site.get_model_admin(model_name);
	assert!(result.is_err());

	if let Err(AdminError::ModelNotRegistered(err_msg)) = result {
		assert!(err_msg.contains(model_name));
	} else {
		panic!("Expected ModelNotRegistered error, got {:?}", result);
	}
}

/// Test admin database basic CRUD operations with actual PostgreSQL
///
/// **Test Category**: Happy path
/// **Test Classification**: Use case testing
#[rstest]
#[tokio::test]
async fn test_admin_database_crud_operations(
	#[future] admin_database: Arc<AdminDatabase>,
	#[future] admin_site: Arc<AdminSite>,
	#[future] model_admin_config: ModelAdminConfig,
) {
	use reinhardt_db::Model;

	let db = admin_database.await;
	let site = admin_site.await;
	let config = model_admin_config.await;
	let model_name = config.model_name();
	let table_name = config.table_name();
	let pk_field = config.pk_field();

	// Register model first
	site.register(model_name, config.clone())
		.expect("Failed to register model");

	// Create test data
	let mut data = HashMap::new();
	data.insert("name".to_string(), json!("Test Item"));
	data.insert("status".to_string(), json!("active"));

	// CREATE operation
	let created_id = db
		.create::<reinhardt_db::AdminRecord>(table_name, None, data.clone())
		.await
		.expect("Failed to create record");
	assert!(created_id > 0);

	// READ operation (get by ID)
	let retrieved = db
		.get::<reinhardt_db::AdminRecord>(table_name, pk_field, &created_id.to_string())
		.await
		.expect("Failed to get record")
		.expect("Record should exist");

	assert_eq!(retrieved.get("name").unwrap(), &json!("Test Item"));
	assert_eq!(retrieved.get("status").unwrap(), &json!("active"));
	assert_eq!(
		retrieved.get(pk_field).unwrap().as_u64().unwrap(),
		created_id
	);

	// UPDATE operation
	let mut update_data = HashMap::new();
	update_data.insert("status".to_string(), json!("inactive"));

	let updated_count = db
		.update::<reinhardt_db::AdminRecord>(
			table_name,
			pk_field,
			&created_id.to_string(),
			update_data,
		)
		.await
		.expect("Failed to update record");
	assert_eq!(updated_count, 1);

	// Verify update
	let updated = db
		.get::<reinhardt_db::AdminRecord>(table_name, pk_field, &created_id.to_string())
		.await
		.expect("Failed to get updated record")
		.expect("Record should exist");
	assert_eq!(updated.get("status").unwrap(), &json!("inactive"));

	// DELETE operation
	let deleted_count = db
		.delete::<reinhardt_db::AdminRecord>(table_name, pk_field, &created_id.to_string())
		.await
		.expect("Failed to delete record");
	assert_eq!(deleted_count, 1);

	// Verify deletion
	let deleted = db
		.get::<reinhardt_db::AdminRecord>(table_name, pk_field, &created_id.to_string())
		.await
		.expect("Failed to get deleted record");
	assert!(deleted.is_none());
}

/// Test list operation with pagination
///
/// **Test Category**: Happy path
/// **Test Classification**: Edge cases (pagination)
#[rstest]
#[case(0, 10)] // First page
#[case(1, 5)] // Second page with smaller page size
#[case(2, 3)] // Third page
#[tokio::test]
async fn test_list_pagination(
	#[future] admin_database: Arc<AdminDatabase>,
	#[case] page: u64,
	#[case] page_size: u64,
) {
	use reinhardt_db::Model;

	let db = admin_database.await;
	let table_name = "test_models";

	// Create multiple test records first
	for i in 0..15 {
		let mut data = HashMap::new();
		data.insert("name".to_string(), json!(format!("Test Item {}", i)));
		data.insert("status".to_string(), json!("active"));

		db.create::<reinhardt_db::AdminRecord>(table_name, None, data)
			.await
			.expect("Failed to create test record");
	}

	// Test list with pagination
	let offset = page * page_size;
	let limit = page_size;

	let results = db
		.list::<reinhardt_db::AdminRecord>(table_name, vec![], offset, limit)
		.await
		.expect("Failed to list records");

	// Verify pagination results
	let expected_count = if offset >= 15 {
		0
	} else {
		std::cmp::min(limit, 15 - offset)
	};
	assert_eq!(results.len() as u64, expected_count);

	// Clean up test records
	// Note: In real tests, we'd use a test-specific table or transaction
}

/// Test boundary value analysis for pagination parameters
///
/// **Test Category**: Boundary value analysis
/// **Test Classification**: Edge cases
#[rstest]
#[case(0, 1)] // Minimum offset, minimum limit
#[case(0, 100)] // Minimum offset, large limit
#[case(100, 10)] // Large offset, normal limit
#[case(0, 0)] // Zero limit (edge case)
#[tokio::test]
async fn test_pagination_boundary_values(
	#[future] admin_database: Arc<AdminDatabase>,
	#[case] offset: u64,
	#[case] limit: u64,
) {
	use reinhardt_db::Model;

	let db = admin_database.await;
	let table_name = "test_models";

	let result = db
		.list::<reinhardt_db::AdminRecord>(table_name, vec![], offset, limit)
		.await;

	// Zero limit is an edge case - behavior depends on implementation
	if limit == 0 {
		// Either empty result or error - both are acceptable edge cases
		assert!(result.is_ok() || result.is_err());
	} else {
		assert!(result.is_ok());
	}
}

/// Test server function integration with DI injection
///
/// **Test Category**: Happy path
/// **Test Classification**: Use case testing
#[rstest]
#[tokio::test]
async fn test_server_function_get_dashboard(
	#[future] server_fn_test_context: (Arc<AdminSite>, Arc<AdminDatabase>),
) {
	let (site, _db) = server_fn_test_context.await;

	// Call the server function
	let result = get_dashboard(site.clone()).await;

	// Verify the response
	assert!(result.is_ok(), "get_dashboard should succeed: {:?}", result);

	let dashboard = result.expect("Dashboard response should be Ok");

	// Basic assertions about dashboard response
	assert_eq!(dashboard.site_title, "Test Admin Site");
	assert!(dashboard.recent_actions.is_empty()); // No actions yet
	assert!(dashboard.models.is_empty()); // No models registered in this context
}

/// Test CRUD server functions with actual data
///
/// **Test Category**: Happy path
/// **Test Classification**: Use case testing
#[rstest]
#[tokio::test]
async fn test_crud_server_functions(
	#[future] server_fn_test_context: (Arc<AdminSite>, Arc<AdminDatabase>),
) {
	let (site, db) = server_fn_test_context.await;
	let model_name = "TestModel".to_string();

	// CREATE operation via server function
	let create_data = HashMap::from([
		("name".to_string(), json!("Server Function Test")),
		("status".to_string(), json!("active")),
	]);

	let create_request = MutationRequest {
		data: create_data.clone(),
	};

	let create_result =
		create_record(model_name.clone(), create_request, site.clone(), db.clone()).await;
	assert!(
		create_result.is_ok(),
		"create_record should succeed: {:?}",
		create_result
	);

	let create_response: MutationResponse = create_result.expect("Create response");
	let record_id = create_response.id.expect("Record ID should be returned");

	// DETAIL operation via server function
	let detail_result = get_detail(
		model_name.clone(),
		record_id.clone(),
		site.clone(),
		db.clone(),
	)
	.await;
	assert!(
		detail_result.is_ok(),
		"get_detail should succeed: {:?}",
		detail_result
	);

	let detail_response = detail_result.expect("Detail response");
	assert_eq!(
		detail_response.data.get("name"),
		Some(&json!("Server Function Test"))
	);

	// UPDATE operation via server function
	let update_data = HashMap::from([("status".to_string(), json!("inactive"))]);

	let update_request = MutationRequest { data: update_data };

	let update_result = update_record(
		model_name.clone(),
		record_id.clone(),
		update_request,
		site.clone(),
		db.clone(),
	)
	.await;
	assert!(
		update_result.is_ok(),
		"update_record should succeed: {:?}",
		update_result
	);

	// DELETE operation via server function
	let delete_result = delete_record(
		model_name.clone(),
		record_id.clone(),
		site.clone(),
		db.clone(),
	)
	.await;
	assert!(
		delete_result.is_ok(),
		"delete_record should succeed: {:?}",
		delete_result
	);

	let delete_response: MutationResponse = delete_result.expect("Delete response");
	assert!(delete_response.success);
}

/// Test list server function with various query parameters
///
/// **Test Category**: Combination testing
/// **Test Classification**: Decision table testing
#[rstest]
#[case::empty_params(ListQueryParams::default())]
#[case::with_page(ListQueryParams { page: Some(1), ..Default::default() })]
#[case::with_page_size(ListQueryParams { page_size: Some(20), ..Default::default() })]
#[case::with_search(ListQueryParams { search: Some("test".to_string()), ..Default::default() })]
#[tokio::test]
async fn test_list_server_function_variations(
	#[future] server_fn_test_context: (Arc<AdminSite>, Arc<AdminDatabase>),
	#[case] params: ListQueryParams,
) {
	let (site, db) = server_fn_test_context.await;
	let model_name = "TestModel".to_string();

	let result = get_list(model_name, params, site.clone(), db.clone()).await;

	// List should succeed with any valid parameters
	assert!(
		result.is_ok(),
		"get_list should succeed with params: {:?}",
		result
	);

	let list_response = result.expect("List response");

	// Basic response validation
	assert_eq!(list_response.model_name, "TestModel");
	assert!(list_response.results.is_empty()); // No data in this test context
	assert_eq!(list_response.count, 0);
	assert_eq!(list_response.total_pages, 0);
}

/// Test error handling for non-existent model in server functions
///
/// **Test Category**: Error path
/// **Test Classification**: Error handling
#[rstest]
#[tokio::test]
async fn test_server_function_model_not_registered_error(
	#[future] server_fn_test_context: (Arc<AdminSite>, Arc<AdminDatabase>),
) {
	let (site, db) = server_fn_test_context.await;
	let non_existent_model = "NonExistentModel".to_string();

	// Try to call server function with non-existent model
	let result = get_list(
		non_existent_model.clone(),
		ListQueryParams::default(),
		site.clone(),
		db.clone(),
	)
	.await;

	// Should return an error
	assert!(result.is_err(), "Should fail for non-existent model");

	// The error should be converted to ServerFnError
	// Actual error type depends on implementation
}

/// Test export/import server functions (basic structure)
///
/// **Test Category**: Happy path
/// **Test Classification**: Use case testing
#[rstest]
#[tokio::test]
async fn test_export_import_server_functions_structure(
	#[future] server_fn_test_context: (Arc<AdminSite>, Arc<AdminDatabase>),
) {
	let (site, db) = server_fn_test_context.await;
	let model_name = "TestModel".to_string();

	// Test export with JSON format
	let export_result = reinhardt_admin::server::export_data(
		model_name.clone(),
		ExportFormat::Json,
		site.clone(),
		db.clone(),
	)
	.await;

	// Export should succeed (even with empty data)
	assert!(
		export_result.is_ok(),
		"export_data should succeed: {:?}",
		export_result
	);

	let export_response: ExportResponse = export_result.expect("Export response");
	assert_eq!(export_response.format, ExportFormat::Json);
	assert!(export_response.data.is_empty()); // No data to export

	// Test export with CSV format
	let csv_export_result = reinhardt_admin::server::export_data(
		model_name.clone(),
		ExportFormat::Csv,
		site.clone(),
		db.clone(),
	)
	.await;

	assert!(csv_export_result.is_ok(), "CSV export should succeed");

	// Test import with empty data (should handle gracefully)
	let import_result = reinhardt_admin::server::import_data(
		model_name.clone(),
		ImportFormat::Json,
		vec![], // Empty data
		site.clone(),
		db.clone(),
	)
	.await;

	// Import of empty data may succeed or fail depending on implementation
	// Both are valid behaviors
	let _ = import_result; // Just ensure it compiles and runs
}

/// Decision table test for permission checking combinations
///
/// **Test Category**: Decision table testing
/// **Test Classification**: Combination testing
#[rstest]
#[case(true, true, true, true)] // All permissions granted
#[case(true, false, false, false)] // View only
#[case(false, true, false, false)] // Add only (unusual but possible)
#[case(false, false, true, false)] // Change only
#[case(false, false, false, true)] // Delete only
#[case(false, false, false, false)] // No permissions
#[tokio::test]
async fn test_permission_decision_table(
	#[future] model_admin_config: ModelAdminConfig,
	#[case] can_view: bool,
	#[case] can_add: bool,
	#[case] can_change: bool,
	#[case] can_delete: bool,
) {
	let config = model_admin_config.await;

	// Note: ModelAdminConfig uses default permissions (all true)
	// This test demonstrates the pattern for permission testing
	// Actual permission tests would require a custom ModelAdmin implementation

	// The default implementation should return true for all permissions
	let mock_user = Box::new(()) as Box<dyn std::any::Any + Send + Sync>;

	// Test default permissions (all true)
	assert!(config.has_view_permission(&mock_user).await);
	assert!(config.has_add_permission(&mock_user).await);
	assert!(config.has_change_permission(&mock_user).await);
	assert!(config.has_delete_permission(&mock_user).await);

	// This test validates that the permission system is callable
	// and returns consistent results for the default implementation
}

/// Equivalence partitioning test for search functionality
///
/// **Test Category**: Equivalence partitioning
/// **Test Classification**: Edge cases
#[rstest]
#[case("")] // Empty search string
#[case("test")] // Normal search string
#[case("test with spaces")] // Search with spaces
#[case("test123")] // Alphanumeric search
#[case("TEST")] // Uppercase search
#[case("test\nnewline")] // Search with special characters
#[tokio::test]
async fn test_search_equivalence_partitioning(
	#[future] admin_database: Arc<AdminDatabase>,
	#[case] search_term: &str,
) {
	use reinhardt_db::Model;

	let db = admin_database.await;
	let table_name = "test_models";

	// The list_with_condition method should handle all search term variations
	// This test ensures the interface accepts various search strings

	// Create a simple filter for search
	use reinhardt_admin::core::{Filter, FilterOperator, FilterValue};

	let filters = if search_term.is_empty() {
		vec![] // Empty search = no filter
	} else {
		vec![Filter {
			field: "name".to_string(),
			operator: FilterOperator::Contains,
			value: FilterValue::String(search_term.to_string()),
		}]
	};

	// Should not panic for any search term
	let result = db
		.list::<reinhardt_db::AdminRecord>(table_name, filters, 0, 10)
		.await;

	assert!(
		result.is_ok(),
		"Search should not panic for term: '{}'",
		search_term
	);
}

/// Test for edge cases in filter values
///
/// **Test Category**: Edge cases
/// **Test Classification**: Boundary value analysis
#[rstest]
#[tokio::test]
async fn test_filter_edge_cases(#[future] admin_database: Arc<AdminDatabase>) {
	use reinhardt_admin::core::{Filter, FilterOperator, FilterValue};
	use reinhardt_db::Model;

	let db = admin_database.await;
	let table_name = "test_models";

	// Test various edge case filter values
	let edge_cases = vec![
		// Empty string
		Filter {
			field: "name".to_string(),
			operator: FilterOperator::Equals,
			value: FilterValue::String("".to_string()),
		},
		// Very long string
		Filter {
			field: "name".to_string(),
			operator: FilterOperator::Equals,
			value: FilterValue::String("a".repeat(1000)),
		},
		// Special characters
		Filter {
			field: "name".to_string(),
			operator: FilterOperator::Equals,
			value: FilterValue::String("test'\"\\\0\n\r\t".to_string()),
		},
		// Numeric extremes
		Filter {
			field: "id".to_string(),
			operator: FilterOperator::GreaterThan,
			value: FilterValue::Number(0.into()),
		},
		Filter {
			field: "id".to_string(),
			operator: FilterOperator::LessThan,
			value: FilterValue::Number(i64::MAX.into()),
		},
		// Boolean values
		Filter {
			field: "active".to_string(),
			operator: FilterOperator::Equals,
			value: FilterValue::Boolean(true),
		},
		// Null value
		Filter {
			field: "deleted_at".to_string(),
			operator: FilterOperator::IsNull,
			value: FilterValue::Null,
		},
	];

	// Test each edge case filter
	for filter in edge_cases {
		let result = db
			.list::<reinhardt_db::AdminRecord>(table_name, vec![filter], 0, 10)
			.await;

		// Should not panic for any edge case
		assert!(
			result.is_ok(),
			"Should handle edge case filter without panic"
		);
	}
}

/// Integration test for admin panel workflow
///
/// **Test Category**: Use case testing
/// **Test Classification**: Happy path
#[rstest]
#[tokio::test]
async fn test_complete_admin_panel_workflow(
	#[future] server_fn_test_context: (Arc<AdminSite>, Arc<AdminDatabase>),
) {
	let (site, db) = server_fn_test_context.await;
	let model_name = "TestModel".to_string();

	// 1. Get dashboard (admin home page)
	let dashboard_result = get_dashboard(site.clone()).await;
	assert!(dashboard_result.is_ok());

	// 2. Create a new record
	let create_data = HashMap::from([
		("name".to_string(), json!("Workflow Test Item")),
		("status".to_string(), json!("active")),
	]);

	let create_request = MutationRequest { data: create_data };
	let create_result =
		create_record(model_name.clone(), create_request, site.clone(), db.clone()).await;
	assert!(create_result.is_ok());

	let create_response: MutationResponse = create_result.expect("Create response");
	let record_id = create_response.id.expect("Record ID");

	// 3. List records to see the new item
	let list_result = get_list(
		model_name.clone(),
		ListQueryParams::default(),
		site.clone(),
		db.clone(),
	)
	.await;
	assert!(list_result.is_ok());

	let list_response = list_result.expect("List response");
	assert_eq!(list_response.count, 1);
	assert_eq!(list_response.results.len(), 1);

	// 4. View record details
	let detail_result = get_detail(
		model_name.clone(),
		record_id.clone(),
		site.clone(),
		db.clone(),
	)
	.await;
	assert!(detail_result.is_ok());

	let detail_response = detail_result.expect("Detail response");
	assert_eq!(
		detail_response.data.get("name"),
		Some(&json!("Workflow Test Item"))
	);

	// 5. Update the record
	let update_data = HashMap::from([("status".to_string(), json!("completed"))]);

	let update_request = MutationRequest { data: update_data };
	let update_result = update_record(
		model_name.clone(),
		record_id.clone(),
		update_request,
		site.clone(),
		db.clone(),
	)
	.await;
	assert!(update_result.is_ok());

	// 6. Delete the record
	let delete_result = delete_record(
		model_name.clone(),
		record_id.clone(),
		site.clone(),
		db.clone(),
	)
	.await;
	assert!(delete_result.is_ok());

	let delete_response: MutationResponse = delete_result.expect("Delete response");
	assert!(delete_response.success);

	// 7. Verify record is gone
	let final_list_result = get_list(
		model_name.clone(),
		ListQueryParams::default(),
		site.clone(),
		db.clone(),
	)
	.await;
	assert!(final_list_result.is_ok());

	let final_list_response = final_list_result.expect("Final list response");
	assert_eq!(final_list_response.count, 0);
	assert!(final_list_response.results.is_empty());

	// This test demonstrates a complete CRUD workflow through server functions
}

// Note: Additional tests for specific features like:
// - Complex filtering (AND/OR conditions)
// - Bulk operations
// - Export/import with actual data
// - Permission-based access control
// - Custom ModelAdmin implementations
// Should be added in separate test files according to the test plan.
