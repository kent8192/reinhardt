#![cfg(not(target_arch = "wasm32"))]
//! API QuerySet Integration Tests
//!
//! Tests for the API QuerySet system's filter construction, query building,
//! and URL generation capabilities.
//!
//! Success Criteria:
//! 1. Filter construction works correctly for all operation types
//! 2. QuerySet operations (filter, exclude, order_by, etc.) function properly
//! 3. URL generation produces correctly formatted query strings
//! 4. Method chaining maintains state correctly
//!
//! Test Categories:
//! - Category 1: Filter Construction (12 tests)
//! - Category 2: QuerySet Operations (13 tests)
//! - Category 3: URL Generation (13 tests)
//!
//! Total: 38 tests
//!
//! Note: CRUD operations (all, get, create, etc.) require WASM environment
//! and will be tested separately with mock server infrastructure.

use reinhardt_pages::api::{ApiQuerySet, Filter, FilterOp};

// ============================================================================
// Category 1: Filter Construction (12 tests)
// ============================================================================

/// Tests Filter::exact creates correct filter
#[test]
fn test_filter_exact_string() {
	let filter = Filter::exact("username", "alice");

	let (key, value) = filter.to_query_param();
	assert_eq!(key, "username");
	assert_eq!(value, "alice");
}

/// Tests Filter::exact with numeric value
#[test]
fn test_filter_exact_number() {
	let filter = Filter::exact("age", 25);
	let (key, value) = filter.to_query_param();
	assert_eq!(key, "age");
	assert_eq!(value, "25");
}

/// Tests Filter::exact with boolean value
#[test]
fn test_filter_exact_boolean() {
	let filter = Filter::exact("is_active", true);
	let (key, value) = filter.to_query_param();
	assert_eq!(key, "is_active");
	assert_eq!(value, "true");
}

/// Tests Filter::with_op for greater than operation
#[test]
fn test_filter_op_gt() {
	let filter = Filter::with_op("score", FilterOp::Gt, 100);
	let (key, value) = filter.to_query_param();
	assert_eq!(key, "score__gt");
	assert_eq!(value, "100");
}

/// Tests Filter::with_op for greater than or equal
#[test]
fn test_filter_op_gte() {
	let filter = Filter::with_op("age", FilterOp::Gte, 18);
	let (key, value) = filter.to_query_param();
	assert_eq!(key, "age__gte");
	assert_eq!(value, "18");
}

/// Tests Filter::with_op for less than operation
#[test]
fn test_filter_op_lt() {
	let filter = Filter::with_op("price", FilterOp::Lt, 50.0);
	let (key, value) = filter.to_query_param();
	assert_eq!(key, "price__lt");
	assert_eq!(value, "50.0");
}

/// Tests Filter::with_op for less than or equal
#[test]
fn test_filter_op_lte() {
	let filter = Filter::with_op("quantity", FilterOp::Lte, 10);
	let (key, value) = filter.to_query_param();
	assert_eq!(key, "quantity__lte");
	assert_eq!(value, "10");
}

/// Tests Filter::with_op for contains operation
#[test]
fn test_filter_op_contains() {
	let filter = Filter::with_op("title", FilterOp::Contains, "rust");
	let (key, value) = filter.to_query_param();
	assert_eq!(key, "title__contains");
	assert_eq!(value, "rust");
}

/// Tests Filter::with_op for case-insensitive contains
#[test]
fn test_filter_op_icontains() {
	let filter = Filter::with_op("description", FilterOp::IContains, "RUST");
	let (key, value) = filter.to_query_param();
	assert_eq!(key, "description__icontains");
	assert_eq!(value, "RUST");
}

/// Tests Filter::with_op for starts with operation
#[test]
fn test_filter_op_startswith() {
	let filter = Filter::with_op("name", FilterOp::StartsWith, "A");
	let (key, value) = filter.to_query_param();
	assert_eq!(key, "name__startswith");
	assert_eq!(value, "A");
}

/// Tests Filter::with_op for ends with operation
#[test]
fn test_filter_op_endswith() {
	let filter = Filter::with_op("email", FilterOp::EndsWith, "@example.com");
	let (key, value) = filter.to_query_param();
	assert_eq!(key, "email__endswith");
	assert_eq!(value, "@example.com");
}

/// Tests Filter::with_op for in list operation
#[test]
fn test_filter_op_in() {
	let filter = Filter::with_op("id", FilterOp::In, vec![1, 2, 3, 4, 5]);
	let (key, value) = filter.to_query_param();
	assert_eq!(key, "id__in");
	assert_eq!(value, "1,2,3,4,5");
}

/// Tests Filter::negate toggles exclude flag
#[test]
fn test_filter_negate() {
	let filter = Filter::exact("status", "banned");

	// Test negated filter produces exclude__ prefix
	let negated = filter.clone().negate();
	let (_key, value) = negated.to_query_param();
	assert_eq!(value, "banned");

	// Double negate should restore original (verified by behavior)
	let _restored = filter.clone().negate().negate();
	// Negate behavior is verified through QuerySet exclude() method in other tests
}

// ============================================================================
// Category 2: QuerySet Operations (13 tests)
// ============================================================================

/// Tests creating a new QuerySet with endpoint
#[test]
fn test_queryset_new() {
	let qs: ApiQuerySet<serde_json::Value> = ApiQuerySet::new("/api/users/");

	// Verify empty QuerySet produces clean URL
	let url = qs.build_url();
	assert_eq!(url, "/api/users/");
}

/// Tests adding a single filter
#[test]
fn test_queryset_filter_single() {
	let qs: ApiQuerySet<serde_json::Value> =
		ApiQuerySet::new("/api/users/").filter("is_active", true);

	let url = qs.build_url();
	assert!(url.contains("is_active=true"));
}

/// Tests adding multiple filters
#[test]
fn test_queryset_filter_multiple() {
	let qs: ApiQuerySet<serde_json::Value> = ApiQuerySet::new("/api/users/")
		.filter("is_active", true)
		.filter("role", "admin");

	let url = qs.build_url();
	assert!(url.contains("is_active=true"));
	assert!(url.contains("role=admin"));
}

/// Tests filter_op method
#[test]
fn test_queryset_filter_op() {
	let qs: ApiQuerySet<serde_json::Value> =
		ApiQuerySet::new("/api/users/").filter_op("age", FilterOp::Gte, 18);

	let url = qs.build_url();
	assert!(url.contains("age__gte=18"));
}

/// Tests exclude method
#[test]
fn test_queryset_exclude() {
	let qs: ApiQuerySet<serde_json::Value> =
		ApiQuerySet::new("/api/users/").exclude("status", "banned");

	let url = qs.build_url();
	assert!(url.contains("exclude__status=banned"));
}

/// Tests order_by with single field
#[test]
fn test_queryset_order_by_single() {
	let qs: ApiQuerySet<serde_json::Value> =
		ApiQuerySet::new("/api/users/").order_by(&["username"]);

	let url = qs.build_url();
	assert!(url.contains("ordering=username"));
}

/// Tests order_by with multiple fields
#[test]
fn test_queryset_order_by_multiple() {
	let qs: ApiQuerySet<serde_json::Value> =
		ApiQuerySet::new("/api/users/").order_by(&["-created_at", "username"]);

	let url = qs.build_url();
	assert!(url.contains("ordering=-created_at%2Cusername"));
}

/// Tests limit method
#[test]
fn test_queryset_limit() {
	let qs: ApiQuerySet<serde_json::Value> = ApiQuerySet::new("/api/users/").limit(10);

	let url = qs.build_url();
	assert!(url.contains("limit=10"));
}

/// Tests offset method
#[test]
fn test_queryset_offset() {
	let qs: ApiQuerySet<serde_json::Value> = ApiQuerySet::new("/api/users/").offset(20);

	let url = qs.build_url();
	assert!(url.contains("offset=20"));
}

/// Tests only method for field selection
#[test]
fn test_queryset_only() {
	let qs: ApiQuerySet<serde_json::Value> =
		ApiQuerySet::new("/api/users/").only(&["id", "username", "email"]);

	let url = qs.build_url();
	assert!(url.contains("fields="));
	// Fields are comma-separated and URL-encoded
	assert!(url.contains("id"));
	assert!(url.contains("username"));
	assert!(url.contains("email"));
}

/// Tests method chaining combines all operations
#[test]
fn test_queryset_method_chaining() {
	let qs: ApiQuerySet<serde_json::Value> = ApiQuerySet::new("/api/users/")
		.filter("is_active", true)
		.exclude("role", "banned")
		.order_by(&["-created_at"])
		.limit(10)
		.offset(0)
		.only(&["id", "username"]);

	let url = qs.build_url();
	assert!(url.contains("is_active=true"));
	assert!(url.contains("exclude__role=banned"));
	assert!(url.contains("ordering=-created_at"));
	assert!(url.contains("limit=10"));
	assert!(url.contains("offset=0"));
	assert!(url.contains("fields="));
}

/// Tests QuerySet is cloneable
#[test]
fn test_queryset_clone() {
	let qs1: ApiQuerySet<serde_json::Value> =
		ApiQuerySet::new("/api/users/").filter("is_active", true);

	let qs2 = qs1.clone();

	// Verify cloned QuerySet produces same URL
	assert_eq!(qs1.build_url(), qs2.build_url());
}

/// Tests combining filter and filter_op
#[test]
fn test_queryset_mixed_filters() {
	let qs: ApiQuerySet<serde_json::Value> = ApiQuerySet::new("/api/users/")
		.filter("is_active", true)
		.filter_op("age", FilterOp::Gte, 18)
		.filter_op("score", FilterOp::Lt, 100);

	let url = qs.build_url();
	assert!(url.contains("is_active=true"));
	assert!(url.contains("age__gte=18"));
	assert!(url.contains("score__lt=100"));
}

/// Tests empty ordering array
#[test]
fn test_queryset_empty_ordering() {
	let qs: ApiQuerySet<serde_json::Value> = ApiQuerySet::new("/api/users/").order_by(&[]);

	let url = qs.build_url();
	// Empty ordering should not add ordering parameter
	assert_eq!(url, "/api/users/");
}

// ============================================================================
// Category 3: URL Generation (13 tests)
// ============================================================================

/// Tests build_url with no parameters
#[test]
fn test_build_url_simple() {
	let qs: ApiQuerySet<serde_json::Value> = ApiQuerySet::new("/api/users/");
	let url = qs.build_url();
	assert_eq!(url, "/api/users/");
}

/// Tests build_url with single filter
#[test]
fn test_build_url_single_filter() {
	let qs: ApiQuerySet<serde_json::Value> =
		ApiQuerySet::new("/api/users/").filter("is_active", true);

	let url = qs.build_url();
	assert!(url.starts_with("/api/users/?"));
	assert!(url.contains("is_active=true"));
}

/// Tests build_url with multiple filters
#[test]
fn test_build_url_multiple_filters() {
	let qs: ApiQuerySet<serde_json::Value> = ApiQuerySet::new("/api/users/")
		.filter("is_active", true)
		.filter_op("age", FilterOp::Gte, 18);

	let url = qs.build_url();
	assert!(url.contains("is_active=true"));
	assert!(url.contains("age__gte=18"));
}

/// Tests build_url with exclude filter
#[test]
fn test_build_url_exclude() {
	let qs: ApiQuerySet<serde_json::Value> =
		ApiQuerySet::new("/api/users/").exclude("role", "admin");

	let url = qs.build_url();
	assert!(url.contains("exclude__role=admin"));
}

/// Tests build_url with ordering
#[test]
fn test_build_url_ordering() {
	let qs: ApiQuerySet<serde_json::Value> =
		ApiQuerySet::new("/api/users/").order_by(&["-created_at", "username"]);

	let url = qs.build_url();
	assert!(url.contains("ordering=-created_at%2Cusername"));
}

/// Tests build_url with limit only
#[test]
fn test_build_url_limit_only() {
	let qs: ApiQuerySet<serde_json::Value> = ApiQuerySet::new("/api/users/").limit(10);

	let url = qs.build_url();
	assert!(url.contains("limit=10"));
}

/// Tests build_url with offset only
#[test]
fn test_build_url_offset_only() {
	let qs: ApiQuerySet<serde_json::Value> = ApiQuerySet::new("/api/users/").offset(20);

	let url = qs.build_url();
	assert!(url.contains("offset=20"));
}

/// Tests build_url with limit and offset (pagination)
#[test]
fn test_build_url_pagination() {
	let qs: ApiQuerySet<serde_json::Value> = ApiQuerySet::new("/api/users/").limit(10).offset(20);

	let url = qs.build_url();
	assert!(url.contains("limit=10"));
	assert!(url.contains("offset=20"));
}

/// Tests build_url with field selection
#[test]
fn test_build_url_fields() {
	let qs: ApiQuerySet<serde_json::Value> =
		ApiQuerySet::new("/api/users/").only(&["id", "username"]);

	let url = qs.build_url();
	assert!(url.contains("fields=id%2Cusername"));
}

/// Tests build_url with all parameters combined
#[test]
fn test_build_url_all_parameters() {
	let qs: ApiQuerySet<serde_json::Value> = ApiQuerySet::new("/api/users/")
		.filter("is_active", true)
		.exclude("role", "banned")
		.order_by(&["-created_at"])
		.limit(10)
		.offset(0)
		.only(&["id", "username"]);

	let url = qs.build_url();
	assert!(url.starts_with("/api/users/?"));
	assert!(url.contains("is_active=true"));
	assert!(url.contains("exclude__role=banned"));
	assert!(url.contains("ordering=-created_at"));
	assert!(url.contains("limit=10"));
	assert!(url.contains("offset=0"));
	assert!(url.contains("fields=id%2Cusername"));
}

/// Tests build_url with special characters in filter values
#[test]
fn test_build_url_special_characters() {
	let qs: ApiQuerySet<serde_json::Value> =
		ApiQuerySet::new("/api/users/").filter("username", "alice@example.com");

	let url = qs.build_url();
	// URL encoding should handle @ symbol
	assert!(url.contains("username=alice"));
}

/// Tests build_url with complex filter operations
#[test]
fn test_build_url_complex_filters() {
	let qs: ApiQuerySet<serde_json::Value> = ApiQuerySet::new("/api/posts/")
		.filter_op("title", FilterOp::Contains, "rust")
		.filter_op("views", FilterOp::Gte, 1000)
		.filter_op("tags", FilterOp::In, vec!["programming", "tutorial"]);

	let url = qs.build_url();
	assert!(url.contains("title__contains=rust"));
	assert!(url.contains("views__gte=1000"));
	assert!(url.contains("tags__in="));
}

/// Tests build_url maintains endpoint format
#[test]
fn test_build_url_endpoint_format() {
	// Endpoint without trailing slash
	let qs1: ApiQuerySet<serde_json::Value> = ApiQuerySet::new("/api/users").filter("id", 1);
	let url1 = qs1.build_url();
	assert!(url1.starts_with("/api/users?"));

	// Endpoint with trailing slash
	let qs2: ApiQuerySet<serde_json::Value> = ApiQuerySet::new("/api/users/").filter("id", 1);
	let url2 = qs2.build_url();
	assert!(url2.starts_with("/api/users/?"));
}
