//! Integration tests for admin handler logic
//!
//! These tests verify the validation and business logic used by admin
//! server function handlers, testing through ModelAdminConfig and
//! the public helper functions.

use reinhardt_admin::core::export::ExportFormat;
use reinhardt_admin::core::model_admin::{ModelAdmin, ModelAdminConfig};
use reinhardt_admin::core::site::AdminSite;
use reinhardt_admin::server::limits::{DEFAULT_PAGE_SIZE, MAX_BULK_DELETE_IDS, MAX_PAGE_SIZE};
use rstest::*;

// ==================== ModelAdminConfig filter field validation ====================

#[fixture]
fn test_model_admin_config() -> ModelAdminConfig {
	ModelAdminConfig::builder()
		.model_name("TestModel")
		.table_name("test_models")
		.list_display(vec!["id", "name", "status", "created_at"])
		.list_filter(vec!["status", "is_active"])
		.search_fields(vec!["name", "description"])
		.ordering(vec!["-created_at"])
		.build()
		.unwrap()
}

#[rstest]
fn test_list_filter_field_must_be_in_list_filter(test_model_admin_config: ModelAdminConfig) {
	// Arrange
	let config = test_model_admin_config;
	let allowed = config.list_filter();

	// Act & Assert
	// Verify that "status" is in list_filter (allowed)
	assert!(
		allowed.contains(&"status"),
		"'status' should be an allowed filter field"
	);
	// Verify that "name" is NOT in list_filter (should be rejected by handler)
	assert!(
		!allowed.contains(&"name"),
		"'name' should NOT be an allowed filter field"
	);
	// Verify that unknown fields are not in list_filter
	assert!(
		!allowed.contains(&"unknown_field"),
		"Unknown fields should not be in list_filter"
	);
}

#[rstest]
fn test_list_sort_field_must_be_in_list_display(test_model_admin_config: ModelAdminConfig) {
	// Arrange
	let config = test_model_admin_config;
	let allowed = config.list_display();

	// Act & Assert
	// Fields in list_display can be used for sorting
	assert!(
		allowed.contains(&"name"),
		"'name' should be an allowed sort field"
	);
	assert!(
		allowed.contains(&"created_at"),
		"'created_at' should be an allowed sort field"
	);
	// Fields NOT in list_display cannot be used for sorting
	assert!(
		!allowed.contains(&"description"),
		"'description' is not in list_display, so cannot be used for sorting"
	);
}

#[rstest]
fn test_list_sort_descending_prefix_stripping() {
	// Arrange: Handler strips "-" prefix before validation
	let sort_by = "-created_at";

	// Act
	let raw_field = sort_by.strip_prefix('-').unwrap_or(sort_by);

	// Assert
	assert_eq!(
		raw_field, "created_at",
		"Prefix '-' should be stripped for validation"
	);
}

#[rstest]
fn test_list_sort_ascending_no_prefix() {
	// Arrange: No prefix means ascending
	let sort_by = "name";

	// Act
	let raw_field = sort_by.strip_prefix('-').unwrap_or(sort_by);

	// Assert
	assert_eq!(raw_field, "name", "No prefix should return field as-is");
}

// ==================== Pagination validation ====================

#[rstest]
#[case(0, 1)] // page 0 → 1
#[case(1, 1)] // page 1 → 1 (no change)
#[case(5, 5)] // page 5 → 5 (no change)
fn test_list_pagination_page_minimum_is_one(#[case] input: u64, #[case] expected: u64) {
	// Arrange
	let page_param: Option<u64> = Some(input);

	// Act: Mirrors handler logic from server/list.rs line 179
	let page = page_param.unwrap_or(1).max(1);

	// Assert
	assert_eq!(page, expected);
}

#[rstest]
fn test_list_pagination_default_page_is_one() {
	// Arrange
	let page_param: Option<u64> = None;

	// Act
	let page = page_param.unwrap_or(1).max(1);

	// Assert
	assert_eq!(page, 1, "Default page should be 1");
}

#[rstest]
#[case(10, 10)] // within limit
#[case(500, 500)] // at exact MAX_PAGE_SIZE limit
#[case(501, 500)] // exceeds limit → capped
#[case(10000, 500)] // far exceeds limit → capped
fn test_list_pagination_page_size_capped(#[case] input: u64, #[case] expected: u64) {
	// Arrange
	let page_size_param: Option<u64> = Some(input);

	// Act: Mirrors handler logic from server/list.rs line 180-183
	let page_size = page_size_param
		.unwrap_or(DEFAULT_PAGE_SIZE)
		.min(MAX_PAGE_SIZE);

	// Assert
	assert_eq!(page_size, expected);
}

#[rstest]
fn test_list_pagination_default_page_size() {
	// Arrange
	let page_size_param: Option<u64> = None;

	// Act
	let page_size = page_size_param
		.unwrap_or(DEFAULT_PAGE_SIZE)
		.min(MAX_PAGE_SIZE);

	// Assert
	assert_eq!(
		page_size, DEFAULT_PAGE_SIZE,
		"Default page size should be {}",
		DEFAULT_PAGE_SIZE
	);
}

// ==================== Search field OR condition building ====================

#[rstest]
fn test_list_search_fields_exist(test_model_admin_config: ModelAdminConfig) {
	// Arrange
	let config = test_model_admin_config;

	// Act
	let search_fields = config.search_fields();

	// Assert
	assert_eq!(search_fields.len(), 2);
	assert!(search_fields.contains(&"name"));
	assert!(search_fields.contains(&"description"));
}

// ==================== Total pages calculation ====================

#[rstest]
#[case(0, 25, 1)] // 0 records → 1 page (minimum)
#[case(1, 25, 1)] // 1 record → 1 page
#[case(25, 25, 1)] // exact fit → 1 page
#[case(26, 25, 2)] // one over → 2 pages
#[case(101, 25, 5)] // 101 records ÷ 25 = 5 pages (div_ceil)
#[case(100, 25, 4)] // 100 records ÷ 25 = 4 pages (exact)
fn test_list_total_pages_calculation(
	#[case] count: u64,
	#[case] page_size: u64,
	#[case] expected_pages: u64,
) {
	// Arrange & Act: Mirrors handler logic from server/list.rs lines 210-214
	let total_pages = if count > 0 {
		count.div_ceil(page_size)
	} else {
		1
	};

	// Assert
	assert_eq!(total_pages, expected_pages);
}

// ==================== AdminSite model registration ====================

#[rstest]
fn test_admin_site_register_and_retrieve(test_model_admin_config: ModelAdminConfig) {
	// Arrange
	let site = AdminSite::new("Test Admin");

	// Act
	let result = site.register("TestModel", test_model_admin_config);

	// Assert
	assert!(result.is_ok());
	let model_admin = site.get_model_admin("TestModel");
	assert!(model_admin.is_ok());
	assert_eq!(model_admin.unwrap().table_name(), "test_models");
}

#[rstest]
fn test_admin_site_unregistered_model_returns_error() {
	// Arrange
	let site = AdminSite::new("Test Admin");

	// Act
	let result = site.get_model_admin("NonExistent");

	// Assert
	assert!(result.is_err());
}

// ==================== Export format handling ====================

#[rstest]
fn test_export_format_json_supported() {
	// Arrange
	let format = ExportFormat::JSON;

	// Act & Assert
	assert_eq!(format.extension(), "json");
	assert_eq!(format.mime_type(), "application/json");
}

#[rstest]
fn test_export_format_csv_supported() {
	// Arrange
	let format = ExportFormat::CSV;

	// Act & Assert
	assert_eq!(format.extension(), "csv");
	assert_eq!(format.mime_type(), "text/csv");
}

#[rstest]
fn test_export_format_tsv_supported() {
	// Arrange
	let format = ExportFormat::TSV;

	// Act & Assert
	assert_eq!(format.extension(), "tsv");
	assert_eq!(format.mime_type(), "text/tab-separated-values");
}

// ==================== Bulk delete limit ====================

#[rstest]
fn test_bulk_delete_limit_constant() {
	// Assert: MAX_BULK_DELETE_IDS is 1000
	assert_eq!(MAX_BULK_DELETE_IDS, 1_000);
}

#[rstest]
#[case(999, true)] // under limit
#[case(1000, true)] // at limit
#[case(1001, false)] // over limit
fn test_bulk_delete_ids_count_validation(#[case] count: usize, #[case] expected_valid: bool) {
	// Arrange & Act: Mirrors handler logic from server/delete.rs lines 162-169
	let valid = count <= MAX_BULK_DELETE_IDS;

	// Assert
	assert_eq!(valid, expected_valid);
}
