//! Combination tests for admin server functions
//!
//! Tests multiple parameters and conditions combined together to verify
//! correct behavior under complex input combinations.

use super::server_fn_helpers::{
	TEST_CSRF_TOKEN, make_auth_user, make_staff_request, server_fn_context,
};
use reinhardt_admin::core::{AdminDatabase, AdminSite, ExportFormat};
use reinhardt_admin::server::{create_record, export_data, get_list};
use reinhardt_admin::types::{ListQueryParams, MutationRequest};
use reinhardt_di::Depends;
use rstest::*;
use serde_json::json;
use std::collections::HashMap;

/// Helper to create a test record with given name and status.
async fn create_test_record(
	site: &Depends<AdminSite>,
	db: &Depends<AdminDatabase>,
	name: &str,
	status: &str,
) {
	let user = make_auth_user();
	let request = make_staff_request();
	let mut data = HashMap::new();
	data.insert("name".to_string(), json!(name));
	data.insert("status".to_string(), json!(status));
	let mutation = MutationRequest {
		csrf_token: TEST_CSRF_TOKEN.to_string(),
		data,
	};
	create_record(
		"TestModel".to_string(),
		mutation,
		site.clone(),
		db.clone(),
		request,
		user,
	)
	.await
	.expect("create_test_record helper should succeed");
}

// ==================== Search + Filter Combined ====================

#[rstest]
#[tokio::test]
async fn test_list_with_search_and_filter_combined(
	#[future] server_fn_context: (Depends<AdminSite>, Depends<AdminDatabase>),
) {
	// Arrange: Create records with various names and statuses
	let (site, db) = server_fn_context.await;

	create_test_record(&site, &db, "AlphaActive", "active").await;
	create_test_record(&site, &db, "AlphaInactive", "inactive").await;
	create_test_record(&site, &db, "BetaActive", "active").await;
	create_test_record(&site, &db, "BetaInactive", "inactive").await;

	let user = make_auth_user();

	// Act: Search for "Alpha" AND filter by status=active
	let mut filters = HashMap::new();
	filters.insert("status".to_string(), "active".to_string());
	let params = ListQueryParams {
		search: Some("Alpha".to_string()),
		filters,
		..Default::default()
	};
	let result = get_list(
		"TestModel".to_string(),
		params,
		site.clone(),
		db.clone(),
		user.clone(),
	)
	.await
	.expect("get_list with search+filter should succeed");

	// Assert: Only AlphaActive should match both conditions
	assert_eq!(
		result.count, 1,
		"Only one record should match search='Alpha' AND status='active'"
	);
	let first = &result.results[0];
	let name = first.get("name").and_then(|v| v.as_str()).unwrap_or("");
	assert_eq!(name, "AlphaActive");
}

// ==================== Search + Pagination ====================

#[rstest]
#[tokio::test]
async fn test_list_with_search_and_pagination(
	#[future] server_fn_context: (Depends<AdminSite>, Depends<AdminDatabase>),
) {
	// Arrange: Create 10+ records with searchable names
	let (site, db) = server_fn_context.await;

	for i in 1..=12 {
		create_test_record(&site, &db, &format!("SearchItem_{:02}", i), "active").await;
	}

	let user = make_auth_user();

	// Act: Search with page_size=3, page=2
	let params = ListQueryParams {
		search: Some("SearchItem".to_string()),
		page: Some(2),
		page_size: Some(3),
		..Default::default()
	};
	let result = get_list(
		"TestModel".to_string(),
		params,
		site.clone(),
		db.clone(),
		user.clone(),
	)
	.await
	.expect("get_list with search+pagination should succeed");

	// Assert
	assert_eq!(
		result.count, 12,
		"Total count should be 12 matching records"
	);
	assert_eq!(
		result.results.len(),
		3,
		"Page 2 with page_size=3 should return 3 records"
	);
	assert_eq!(result.page, 2, "Current page should be 2");
	assert_eq!(result.page_size, 3, "Page size should be 3");
	assert_eq!(result.total_pages, 4, "12 records / 3 per page = 4 pages");
}

// ==================== Sort Ascending vs Descending ====================

#[rstest]
#[tokio::test]
async fn test_list_with_sort_ascending_and_descending(
	#[future] server_fn_context: (Depends<AdminSite>, Depends<AdminDatabase>),
) {
	// Arrange
	let (site, db) = server_fn_context.await;

	create_test_record(&site, &db, "Charlie", "active").await;
	create_test_record(&site, &db, "Alice", "active").await;
	create_test_record(&site, &db, "Bob", "active").await;

	let user = make_auth_user();

	// Act: Sort ascending by name
	let asc_params = ListQueryParams {
		sort_by: Some("name".to_string()),
		..Default::default()
	};
	let asc_result = get_list(
		"TestModel".to_string(),
		asc_params,
		site.clone(),
		db.clone(),
		user.clone(),
	)
	.await
	.expect("ascending sort should succeed");

	// Act: Sort descending by name
	let desc_params = ListQueryParams {
		sort_by: Some("-name".to_string()),
		..Default::default()
	};
	let desc_result = get_list(
		"TestModel".to_string(),
		desc_params,
		site.clone(),
		db.clone(),
		user.clone(),
	)
	.await
	.expect("descending sort should succeed");

	// Assert: Extract names from both results
	let asc_names: Vec<String> = asc_result
		.results
		.iter()
		.filter_map(|r| r.get("name").and_then(|v| v.as_str()).map(String::from))
		.collect();
	let desc_names: Vec<String> = desc_result
		.results
		.iter()
		.filter_map(|r| r.get("name").and_then(|v| v.as_str()).map(String::from))
		.collect();

	assert_eq!(
		asc_names,
		vec!["Alice", "Bob", "Charlie"],
		"Ascending sort should be alphabetical"
	);
	assert_eq!(
		desc_names,
		vec!["Charlie", "Bob", "Alice"],
		"Descending sort should be reverse alphabetical"
	);
}

// ==================== Create Record With All Field Types ====================

#[rstest]
#[tokio::test]
async fn test_create_record_with_all_field_types(
	#[future] server_fn_context: (Depends<AdminSite>, Depends<AdminDatabase>),
) {
	// Arrange
	let (site, db) = server_fn_context.await;
	let user = make_auth_user();
	let request = make_staff_request();

	let mut data = HashMap::new();
	data.insert("name".to_string(), json!("FullFieldRecord"));
	data.insert("status".to_string(), json!("active"));
	data.insert(
		"description".to_string(),
		json!("A detailed description with multiple words"),
	);
	let mutation = MutationRequest {
		csrf_token: TEST_CSRF_TOKEN.to_string(),
		data,
	};

	// Act: Create record
	let create_result = create_record(
		"TestModel".to_string(),
		mutation,
		site.clone(),
		db.clone(),
		request.clone(),
		user.clone(),
	)
	.await
	.expect("create with all fields should succeed");
	assert!(create_result.success);

	// Verify via detail
	let detail = reinhardt_admin::server::get_detail(
		"TestModel".to_string(),
		"1".to_string(),
		site.clone(),
		db.clone(),
		request.clone(),
		user.clone(),
	)
	.await
	.expect("get_detail should succeed");

	// Assert: All fields stored correctly
	assert_eq!(
		detail.data.get("name").and_then(|v| v.as_str()),
		Some("FullFieldRecord")
	);
	assert_eq!(
		detail.data.get("status").and_then(|v| v.as_str()),
		Some("active")
	);
	assert_eq!(
		detail.data.get("description").and_then(|v| v.as_str()),
		Some("A detailed description with multiple words")
	);
	// created_at should be populated by database default
	assert!(
		detail.data.contains_key("created_at"),
		"created_at should exist"
	);
}

// ==================== Export All Formats ====================

#[rstest]
#[tokio::test]
async fn test_export_all_formats_produce_valid_output(
	#[future] server_fn_context: (Depends<AdminSite>, Depends<AdminDatabase>),
) {
	// Arrange: Create a record so export has data
	let (site, db) = server_fn_context.await;
	let user = make_auth_user();
	let request = make_staff_request();

	create_test_record(&site, &db, "ExportTest", "active").await;

	// Act & Assert: Export as JSON
	let json_export = export_data(
		"TestModel".to_string(),
		ExportFormat::JSON,
		site.clone(),
		db.clone(),
		request.clone(),
		user.clone(),
	)
	.await
	.expect("JSON export should succeed");
	assert!(
		!json_export.data.is_empty(),
		"JSON export data should not be empty"
	);
	assert_eq!(json_export.content_type, "application/json");
	assert!(json_export.filename.ends_with(".json"));

	// Act & Assert: Export as CSV
	let csv_export = export_data(
		"TestModel".to_string(),
		ExportFormat::CSV,
		site.clone(),
		db.clone(),
		request.clone(),
		user.clone(),
	)
	.await
	.expect("CSV export should succeed");
	assert!(
		!csv_export.data.is_empty(),
		"CSV export data should not be empty"
	);
	assert_eq!(csv_export.content_type, "text/csv");
	assert!(csv_export.filename.ends_with(".csv"));

	// Act & Assert: Export as TSV
	let tsv_export = export_data(
		"TestModel".to_string(),
		ExportFormat::TSV,
		site.clone(),
		db.clone(),
		request.clone(),
		user.clone(),
	)
	.await
	.expect("TSV export should succeed");
	assert!(
		!tsv_export.data.is_empty(),
		"TSV export data should not be empty"
	);
	assert_eq!(tsv_export.content_type, "text/tab-separated-values");
	assert!(tsv_export.filename.ends_with(".tsv"));
}

// ==================== Filter + Sort Combined ====================

#[rstest]
#[tokio::test]
async fn test_list_with_filter_and_sort_combined(
	#[future] server_fn_context: (Depends<AdminSite>, Depends<AdminDatabase>),
) {
	// Arrange
	let (site, db) = server_fn_context.await;

	create_test_record(&site, &db, "Charlie", "active").await;
	create_test_record(&site, &db, "Alice", "inactive").await;
	create_test_record(&site, &db, "Bob", "active").await;
	create_test_record(&site, &db, "Dave", "active").await;

	let user = make_auth_user();

	// Act: Filter by active + sort by name ascending
	let mut filters = HashMap::new();
	filters.insert("status".to_string(), "active".to_string());
	let params = ListQueryParams {
		filters,
		sort_by: Some("name".to_string()),
		..Default::default()
	};
	let result = get_list(
		"TestModel".to_string(),
		params,
		site.clone(),
		db.clone(),
		user.clone(),
	)
	.await
	.expect("filter+sort should succeed");

	// Assert: Only active records, sorted by name
	assert_eq!(result.count, 3, "Should have 3 active records");
	let names: Vec<String> = result
		.results
		.iter()
		.filter_map(|r| r.get("name").and_then(|v| v.as_str()).map(String::from))
		.collect();
	assert_eq!(
		names,
		vec!["Bob", "Charlie", "Dave"],
		"Active records should be sorted by name"
	);
}

// ==================== Concurrent Record Creation ====================

#[rstest]
#[tokio::test]
async fn test_concurrent_record_creation(
	#[future] server_fn_context: (Depends<AdminSite>, Depends<AdminDatabase>),
) {
	// Arrange
	let (site, db) = server_fn_context.await;

	// Act: Create 10 records concurrently
	let mut handles = Vec::new();
	for i in 1..=10 {
		let site = site.clone();
		let db = db.clone();
		let handle = tokio::spawn(async move {
			let user = make_auth_user();
			let request = make_staff_request();
			let mut data = HashMap::new();
			data.insert("name".to_string(), json!(format!("Concurrent_{}", i)));
			data.insert("status".to_string(), json!("active"));
			let mutation = MutationRequest {
				csrf_token: TEST_CSRF_TOKEN.to_string(),
				data,
			};
			create_record("TestModel".to_string(), mutation, site, db, request, user).await
		});
		handles.push(handle);
	}

	// Wait for all to complete
	let results: Vec<_> = futures::future::join_all(handles).await;
	for (i, result) in results.iter().enumerate() {
		let inner = result.as_ref().expect("tokio task should not panic");
		assert!(
			inner.is_ok(),
			"Concurrent create {} should succeed: {:?}",
			i + 1,
			inner.as_ref().err()
		);
	}

	// Assert: All 10 exist
	let user = make_auth_user();
	let list_result = get_list(
		"TestModel".to_string(),
		ListQueryParams {
			page_size: Some(25),
			..Default::default()
		},
		site.clone(),
		db.clone(),
		user,
	)
	.await
	.expect("get_list should succeed");
	assert_eq!(
		list_result.count, 10,
		"All 10 concurrently created records should exist"
	);
}
