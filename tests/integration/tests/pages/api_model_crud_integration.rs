//! API Model CRUD Integration Tests
//!
//! This module contains comprehensive integration tests for the reinhardt-pages
//! API QuerySet CRUD operations, covering filter, exclude, order_by, pagination,
//! and HTTP method interactions.
//!
//! Success Criteria:
//! 1. QuerySet correctly builds URLs with filters, ordering, and pagination
//! 2. Filter operations (exact, contains, gte, etc.) generate correct query parameters
//! 3. Pagination works correctly with limit and offset
//! 4. Multiple filters can be combined
//! 5. Exclude filters work correctly
//! 6. Edge cases are handled properly (empty results, large datasets, boundary values)
//!
//! Test Categories:
//! - Happy Path: 5 tests
//! - Error Path: 4 tests
//! - Edge Cases: 3 tests
//! - State Transitions: 1 test
//! - Use Cases: 3 tests
//! - Fuzz: 1 test
//! - Property-based: 1 test
//! - Combination: 2 tests
//! - Sanity: 1 test
//! - Equivalence Partitioning: 5 tests
//! - Boundary Analysis: 6 tests
//! - Decision Table: 8 tests
//!
//! Total: 40 test cases

use super::fixtures::*;
use reinhardt_pages::api::{ApiQuerySet, Filter, FilterOp};
use rstest::*;

// ============================================================================
// Happy Path Tests (5 tests)
// ============================================================================

/// Tests basic filter operation with exact match
#[rstest]
fn test_api_queryset_filter_exact() {
	let queryset: ApiQuerySet<TestModel> =
		ApiQuerySet::new("/api/posts/").filter("published", true);

	let url = queryset.build_url();
	assert!(url.contains("published=true"));
	assert!(url.starts_with("/api/posts/?"));
}

/// Tests contains filter operation
#[rstest]
fn test_api_queryset_filter_contains() {
	let queryset: ApiQuerySet<TestModel> =
		ApiQuerySet::new("/api/posts/").filter_op("title", FilterOp::Contains, "test");

	let url = queryset.build_url();
	assert!(url.contains("title__contains=test"));
}

/// Tests pagination with limit and offset
#[rstest]
fn test_api_queryset_pagination() {
	let queryset: ApiQuerySet<TestModel> = ApiQuerySet::new("/api/posts/").limit(10).offset(20);

	let url = queryset.build_url();
	assert!(url.contains("limit=10"));
	assert!(url.contains("offset=20"));
}

/// Tests ordering with ascending and descending fields
#[rstest]
fn test_api_queryset_ordering() {
	let queryset: ApiQuerySet<TestModel> =
		ApiQuerySet::new("/api/posts/").order_by(&["-created_at", "title"]);

	let url = queryset.build_url();
	assert!(url.contains("ordering=-created_at%2Ctitle"));
}

/// Tests field selection for partial responses
#[rstest]
fn test_api_queryset_field_selection() {
	let queryset: ApiQuerySet<TestModel> = ApiQuerySet::new("/api/posts/").only(&["id", "title"]);

	let url = queryset.build_url();
	assert!(url.contains("fields=id%2Ctitle"));
}

// ============================================================================
// Error Path Tests (4 tests)
// ============================================================================

/// Tests that non-WASM environment returns proper error for all() method
#[rstest]
#[tokio::test]
#[cfg(not(target_arch = "wasm32"))]
async fn test_api_queryset_all_non_wasm_error() {
	let queryset: ApiQuerySet<TestModel> = ApiQuerySet::new("/api/posts/");

	let result = queryset.all().await;
	assert!(result.is_err());
	assert!(matches!(
		result.unwrap_err(),
		reinhardt_pages::server_fn::ServerFnError::Network(_)
	));
}

/// Tests that non-WASM environment returns proper error for create() method
#[rstest]
#[tokio::test]
#[cfg(not(target_arch = "wasm32"))]
async fn test_api_queryset_create_non_wasm_error(test_model: TestModel) {
	let queryset: ApiQuerySet<TestModel> = ApiQuerySet::new("/api/posts/");

	let result = queryset.create(&test_model).await;
	assert!(result.is_err());
	assert!(matches!(
		result.unwrap_err(),
		reinhardt_pages::server_fn::ServerFnError::Network(_)
	));
}

/// Tests that non-WASM environment returns proper error for get() method
#[rstest]
#[tokio::test]
#[cfg(not(target_arch = "wasm32"))]
async fn test_api_queryset_get_non_wasm_error() {
	let queryset: ApiQuerySet<TestModel> = ApiQuerySet::new("/api/posts/");

	let result = queryset.get(1).await;
	assert!(result.is_err());
	assert!(matches!(
		result.unwrap_err(),
		reinhardt_pages::server_fn::ServerFnError::Network(_)
	));
}

/// Tests that non-WASM environment returns proper error for delete() method
#[rstest]
#[tokio::test]
#[cfg(not(target_arch = "wasm32"))]
async fn test_api_queryset_delete_non_wasm_error() {
	let queryset: ApiQuerySet<TestModel> = ApiQuerySet::new("/api/posts/");

	let result = queryset.delete(1).await;
	assert!(result.is_err());
	assert!(matches!(
		result.unwrap_err(),
		reinhardt_pages::server_fn::ServerFnError::Network(_)
	));
}

// ============================================================================
// Edge Case Tests (3 tests)
// ============================================================================

/// Tests QuerySet with no filters returns base URL
#[rstest]
fn test_api_queryset_empty_filters() {
	let queryset: ApiQuerySet<TestModel> = ApiQuerySet::new("/api/posts/");

	let url = queryset.build_url();
	assert_eq!(url, "/api/posts/");
}

/// Tests QuerySet with extremely large pagination values
#[rstest]
fn test_api_queryset_large_pagination() {
	let queryset: ApiQuerySet<TestModel> = ApiQuerySet::new("/api/posts/")
		.limit(1_000_000)
		.offset(10_000_000);

	let url = queryset.build_url();
	assert!(url.contains("limit=1000000"));
	assert!(url.contains("offset=10000000"));
}

/// Tests QuerySet with special characters in filter values
#[rstest]
fn test_api_queryset_special_chars_in_filter(special_chars_string: String) {
	let queryset: ApiQuerySet<TestModel> =
		ApiQuerySet::new("/api/posts/").filter("title", special_chars_string.clone());

	let url = queryset.build_url();
	// URL should contain encoded special characters
	assert!(url.contains("title="));
	assert!(url.contains("%3C") || url.contains("%3E")); // '<' or '>' encoded
}

// ============================================================================
// State Transition Tests (1 test)
// ============================================================================

/// Tests QuerySet state transitions through method chaining
#[rstest]
fn test_api_queryset_state_transitions() {
	// Initial state: empty QuerySet
	let queryset: ApiQuerySet<TestModel> = ApiQuerySet::new("/api/posts/");
	assert_eq!(queryset.build_url(), "/api/posts/");

	// Transition 1: Add filter
	let queryset = queryset.filter("published", true);
	assert!(queryset.build_url().contains("published=true"));

	// Transition 2: Add ordering
	let queryset = queryset.order_by(&["-created_at"]);
	let url = queryset.build_url();
	assert!(url.contains("published=true"));
	assert!(url.contains("ordering=-created_at"));

	// Transition 3: Add pagination
	let queryset = queryset.limit(10).offset(0);
	let url = queryset.build_url();
	assert!(url.contains("published=true"));
	assert!(url.contains("ordering=-created_at"));
	assert!(url.contains("limit=10"));
	assert!(url.contains("offset=0"));
}

// ============================================================================
// Use Case Tests (3 tests)
// ============================================================================

/// Tests realistic search use case: published posts sorted by view count
#[rstest]
fn test_api_queryset_use_case_search() {
	let queryset: ApiQuerySet<TestModel> = ApiQuerySet::new("/api/posts/")
		.filter("published", true)
		.order_by(&["-view_count"])
		.limit(20);

	let url = queryset.build_url();
	assert!(url.contains("published=true"));
	assert!(url.contains("ordering=-view_count"));
	assert!(url.contains("limit=20"));
}

/// Tests realistic sorting use case: posts by title ascending
#[rstest]
fn test_api_queryset_use_case_sort() {
	let queryset: ApiQuerySet<TestModel> =
		ApiQuerySet::new("/api/posts/").order_by(&["title", "id"]);

	let url = queryset.build_url();
	assert!(url.contains("ordering=title%2Cid"));
}

/// Tests complex filter use case: published posts with high view count
#[rstest]
fn test_api_queryset_use_case_complex_filter() {
	let queryset: ApiQuerySet<TestModel> = ApiQuerySet::new("/api/posts/")
		.filter("published", true)
		.filter_op("view_count", FilterOp::Gte, 100)
		.filter_op("title", FilterOp::Contains, "important")
		.exclude("id", 999);

	let url = queryset.build_url();
	assert!(url.contains("published=true"));
	assert!(url.contains("view_count__gte=100"));
	assert!(url.contains("title__contains=important"));
	assert!(url.contains("exclude__id=999"));
}

// ============================================================================
// Fuzz Tests (1 test)
// ============================================================================

/// Tests QuerySet with random filter values using proptest
#[cfg(feature = "proptest")]
#[rstest]
fn test_api_queryset_fuzz_random_filters() {
	use proptest::prelude::*;

	proptest!(|(value in any::<i32>())| {
		let queryset: ApiQuerySet<TestModel> = ApiQuerySet::new("/api/posts/")
			.filter("view_count", value);

		let url = queryset.build_url();
		// Should contain the filter parameter
		assert!(url.contains("view_count="));
	});
}

// ============================================================================
// Property-based Tests (1 test)
// ============================================================================

/// Tests URL validity property: build_url always returns valid URL structure
#[rstest]
fn test_api_queryset_property_url_validity() {
	let test_cases = vec![
		ApiQuerySet::<TestModel>::new("/api/posts/"),
		ApiQuerySet::new("/api/posts/").filter("published", true),
		ApiQuerySet::new("/api/posts/").limit(10).offset(20),
		ApiQuerySet::new("/api/posts/")
			.filter("published", true)
			.order_by(&["-created_at"])
			.limit(10),
	];

	for queryset in test_cases {
		let url = queryset.build_url();

		// Property 1: URL should start with endpoint
		assert!(url.starts_with("/api/posts/"));

		// Property 2: If URL has query params, should contain '?'
		if url.len() > "/api/posts/".len() {
			assert!(url.contains('?'));
		}

		// Property 3: Query params should be URL-encoded (no raw spaces)
		if url.contains('?') {
			let query_part = url.split('?').nth(1).unwrap();
			assert!(!query_part.contains(' '));
		}
	}
}

// ============================================================================
// Combination Tests (2 tests)
// ============================================================================

/// Tests QuerySet combined with CSRF token handling
#[rstest]
fn test_api_queryset_with_csrf_token(csrf_token: String) {
	// This test verifies that QuerySet can be used in contexts where CSRF is present
	// In WASM, the actual HTTP request would include CSRF headers automatically

	let queryset: ApiQuerySet<TestModel> = ApiQuerySet::new("/api/posts/")
		.filter("published", true)
		.limit(10);

	let url = queryset.build_url();

	// The URL itself doesn't contain CSRF token (it's in HTTP headers)
	assert!(!url.contains(&csrf_token));
	assert!(url.contains("published=true"));
	assert!(url.contains("limit=10"));
}

/// Tests QuerySet URL can be used with Server Function pattern
#[rstest]
fn test_api_queryset_with_server_function_pattern() {
	// Server functions might use QuerySet to build API URLs
	let queryset: ApiQuerySet<TestModel> = ApiQuerySet::new("/api/posts/")
		.filter("published", true)
		.order_by(&["-created_at"])
		.limit(5);

	let url = queryset.build_url();

	// Verify the URL is well-formed for use in server functions
	assert!(url.starts_with("/api/posts/?"));
	assert!(url.contains("published=true"));
	assert!(url.contains("ordering=-created_at"));
	assert!(url.contains("limit=5"));

	// URL should not end with '&' (no trailing separators)
	assert!(!url.ends_with('&'));
}

// ============================================================================
// Sanity Tests (1 test)
// ============================================================================

/// Tests basic QuerySet creation and cloning
#[rstest]
fn test_api_queryset_sanity() {
	let queryset: ApiQuerySet<TestModel> = ApiQuerySet::new("/api/posts/");
	assert_eq!(queryset.build_url(), "/api/posts/");

	// Test all_clone creates new instance with same endpoint
	let cloned = queryset.all_clone();
	assert_eq!(cloned.build_url(), "/api/posts/");
}

// ============================================================================
// Equivalence Partitioning Tests (5 tests)
// ============================================================================

/// Tests FilterOp::Exact partition
#[rstest]
#[case::string_exact("title", FilterOp::Exact, "test", "title=test")]
#[case::int_exact("view_count", FilterOp::Exact, 100, "view_count=100")]
#[case::bool_exact("published", FilterOp::Exact, true, "published=true")]
fn test_api_queryset_filter_op_exact_partition(
	#[case] field: &str,
	#[case] op: FilterOp,
	#[case] value: impl serde::Serialize,
	#[case] expected_param: &str,
) {
	let filter = Filter::with_op(field, op, value);
	let (key, val) = filter.to_query_param();
	let param = format!("{}={}", key, val);
	assert_eq!(param, expected_param);
}

/// Tests FilterOp::Contains partition
#[rstest]
#[case::contains("title", FilterOp::Contains, "test", "title__contains=test")]
#[case::icontains("title", FilterOp::IContains, "TEST", "title__icontains=TEST")]
fn test_api_queryset_filter_op_contains_partition(
	#[case] field: &str,
	#[case] op: FilterOp,
	#[case] value: &str,
	#[case] expected_param: &str,
) {
	let filter = Filter::with_op(field, op, value);
	let (key, val) = filter.to_query_param();
	let param = format!("{}={}", key, val);
	assert_eq!(param, expected_param);
}

/// Tests FilterOp comparison operators partition (Gt, Gte, Lt, Lte)
#[rstest]
#[case::gt("view_count", FilterOp::Gt, 100, "view_count__gt=100")]
#[case::gte("view_count", FilterOp::Gte, 50, "view_count__gte=50")]
#[case::lt("view_count", FilterOp::Lt, 200, "view_count__lt=200")]
#[case::lte("view_count", FilterOp::Lte, 150, "view_count__lte=150")]
fn test_api_queryset_filter_op_comparison_partition(
	#[case] field: &str,
	#[case] op: FilterOp,
	#[case] value: i32,
	#[case] expected_param: &str,
) {
	let filter = Filter::with_op(field, op, value);
	let (key, val) = filter.to_query_param();
	let param = format!("{}={}", key, val);
	assert_eq!(param, expected_param);
}

/// Tests FilterOp string matching partition (StartsWith, EndsWith)
#[rstest]
#[case::startswith("title", FilterOp::StartsWith, "Test", "title__startswith=Test")]
#[case::istartswith("title", FilterOp::IStartsWith, "test", "title__istartswith=test")]
#[case::endswith("title", FilterOp::EndsWith, "Post", "title__endswith=Post")]
#[case::iendswith("title", FilterOp::IEndsWith, "post", "title__iendswith=post")]
fn test_api_queryset_filter_op_string_match_partition(
	#[case] field: &str,
	#[case] op: FilterOp,
	#[case] value: &str,
	#[case] expected_param: &str,
) {
	let filter = Filter::with_op(field, op, value);
	let (key, val) = filter.to_query_param();
	let param = format!("{}={}", key, val);
	assert_eq!(param, expected_param);
}

/// Tests FilterOp special operators partition (In, IsNull)
#[rstest]
#[case::in_list("id", FilterOp::In, vec![1, 2, 3], "id__in=1,2,3")]
#[case::is_null("deleted_at", FilterOp::IsNull, true, "deleted_at__isnull=true")]
fn test_api_queryset_filter_op_special_partition(
	#[case] field: &str,
	#[case] op: FilterOp,
	#[case] value: impl serde::Serialize,
	#[case] expected_param: &str,
) {
	let filter = Filter::with_op(field, op, value);
	let (key, val) = filter.to_query_param();
	let param = format!("{}={}", key, val);
	assert_eq!(param, expected_param);
}

// ============================================================================
// Boundary Analysis Tests (6 tests)
// ============================================================================

/// Tests boundary values for page size (limit)
#[rstest]
#[case::zero_limit(0)]
#[case::one_limit(1)]
#[case::typical_limit(20)]
#[case::large_limit(1000)]
#[case::max_limit(usize::MAX)]
fn test_api_queryset_limit_boundaries(#[case] limit: usize) {
	let queryset: ApiQuerySet<TestModel> = ApiQuerySet::new("/api/posts/").limit(limit);

	let url = queryset.build_url();
	assert!(url.contains(&format!("limit={}", limit)));
}

/// Tests boundary values for offset
#[rstest]
#[case::zero_offset(0)]
#[case::one_offset(1)]
#[case::typical_offset(100)]
#[case::large_offset(10000)]
#[case::max_offset(usize::MAX)]
fn test_api_queryset_offset_boundaries(#[case] offset: usize) {
	let queryset: ApiQuerySet<TestModel> = ApiQuerySet::new("/api/posts/").offset(offset);

	let url = queryset.build_url();
	assert!(url.contains(&format!("offset={}", offset)));
}

/// Tests boundary for number of filters
#[rstest]
fn test_api_queryset_multiple_filters_boundary() {
	// Test with 0, 1, 3, 10 filters
	let qs0: ApiQuerySet<TestModel> = ApiQuerySet::new("/api/posts/");
	assert_eq!(qs0.build_url(), "/api/posts/");

	let qs1: ApiQuerySet<TestModel> = ApiQuerySet::new("/api/posts/").filter("published", true);
	assert!(qs1.build_url().contains("published=true"));

	let qs3: ApiQuerySet<TestModel> = ApiQuerySet::new("/api/posts/")
		.filter("published", true)
		.filter("view_count", 100)
		.filter("title", "test");
	let url3 = qs3.build_url();
	assert!(url3.contains("published=true"));
	assert!(url3.contains("view_count=100"));
	assert!(url3.contains("title=test"));

	// 10 filters
	let mut qs10: ApiQuerySet<TestModel> = ApiQuerySet::new("/api/posts/");
	for i in 0..10 {
		qs10 = qs10.filter(&format!("field{}", i), i);
	}
	let url10 = qs10.build_url();
	assert!(url10.contains("field0=0"));
	assert!(url10.contains("field9=9"));
}

/// Tests boundary for ordering field count
#[rstest]
fn test_api_queryset_ordering_fields_boundary() {
	// Test with 0, 1, 3 ordering fields
	let qs0: ApiQuerySet<TestModel> = ApiQuerySet::new("/api/posts/");
	assert!(!qs0.build_url().contains("ordering="));

	let qs1: ApiQuerySet<TestModel> = ApiQuerySet::new("/api/posts/").order_by(&["title"]);
	assert!(qs1.build_url().contains("ordering=title"));

	let qs3: ApiQuerySet<TestModel> =
		ApiQuerySet::new("/api/posts/").order_by(&["-view_count", "title", "id"]);
	assert!(
		qs3.build_url()
			.contains("ordering=-view_count%2Ctitle%2Cid")
	);
}

/// Tests boundary for field selection count
#[rstest]
fn test_api_queryset_field_selection_boundary() {
	// Test with 0, 1, 5 fields
	let qs0: ApiQuerySet<TestModel> = ApiQuerySet::new("/api/posts/");
	assert!(!qs0.build_url().contains("fields="));

	let qs1: ApiQuerySet<TestModel> = ApiQuerySet::new("/api/posts/").only(&["id"]);
	assert!(qs1.build_url().contains("fields=id"));

	let qs5: ApiQuerySet<TestModel> = ApiQuerySet::new("/api/posts/").only(&[
		"id",
		"title",
		"content",
		"published",
		"view_count",
	]);
	let url = qs5.build_url();
	assert!(url.contains("fields="));
	assert!(url.contains("id"));
	assert!(url.contains("title"));
	assert!(url.contains("view_count"));
}

/// Tests boundary for filter value sizes
#[rstest]
fn test_api_queryset_filter_value_size_boundary(empty_string: String, long_string: String) {
	// Empty string
	let qs_empty = ApiQuerySet::<TestModel>::new("/api/posts/").filter("title", &empty_string);
	assert!(qs_empty.build_url().contains("title="));

	// Very long string
	let qs_long = ApiQuerySet::<TestModel>::new("/api/posts/").filter("title", &long_string);
	let url = qs_long.build_url();
	assert!(url.contains("title="));
	// URL should contain encoded long string
	assert!(url.len() > 100);
}

// ============================================================================
// Decision Table Tests (8 tests)
// ============================================================================

/// Decision table: Filter presence × Filter type × Pagination
#[rstest]
#[case::no_filter_no_pagination("/api/posts/", false, FilterOp::Exact, 0, false, vec![])]
#[case::filter_no_pagination("/api/posts/", true, FilterOp::Exact, 0, false, vec!["published=true"])]
#[case::no_filter_with_pagination("/api/posts/", false, FilterOp::Exact, 10, true, vec!["limit=10"])]
#[case::filter_with_pagination("/api/posts/", true, FilterOp::Exact, 10, true, vec!["published=true", "limit=10"])]
#[case::filter_gte_with_pagination("/api/posts/", true, FilterOp::Gte, 20, true, vec!["view_count__gte=100", "limit=20"])]
#[case::filter_contains_no_pagination("/api/posts/", true, FilterOp::Contains, 0, false, vec!["title__contains=test"])]
#[case::multiple_filters_with_pagination("/api/posts/", true, FilterOp::Exact, 5, true, vec!["published=true", "limit=5"])]
#[case::exclude_filter_with_pagination("/api/posts/", true, FilterOp::Exact, 15, true, vec!["exclude__id=999", "limit=15"])]
fn test_api_queryset_decision_table(
	#[case] endpoint: &str,
	#[case] has_filter: bool,
	#[case] filter_op: FilterOp,
	#[case] limit_value: usize,
	#[case] has_pagination: bool,
	#[case] expected_params: Vec<&str>,
) {
	let mut queryset: ApiQuerySet<TestModel> = ApiQuerySet::new(endpoint);

	if has_filter {
		// Check if this is an exclude test case first
		if expected_params.iter().any(|p| p.contains("exclude")) {
			queryset = queryset.exclude("id", 999);
		} else {
			match filter_op {
				FilterOp::Exact => queryset = queryset.filter("published", true),
				FilterOp::Gte => queryset = queryset.filter_op("view_count", FilterOp::Gte, 100),
				FilterOp::Contains => {
					queryset = queryset.filter_op("title", FilterOp::Contains, "test")
				}
				_ => queryset = queryset.filter("published", true),
			}
		}
	}

	if has_pagination {
		queryset = queryset.limit(limit_value);
	}

	let url = queryset.build_url();

	for param in expected_params {
		assert!(url.contains(param), "URL should contain: {}", param);
	}

	if !has_filter && !has_pagination {
		assert_eq!(url, endpoint);
	}
}
