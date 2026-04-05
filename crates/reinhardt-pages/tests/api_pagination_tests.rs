#![cfg(not(target_arch = "wasm32"))]
//! API Pagination Integration Tests
//!
//! Tests for various pagination strategies supported by the API QuerySet system.
//!
//! Success Criteria:
//! 1. Offset-based pagination generates correct query parameters
//! 2. Page-based pagination converts to offset correctly
//! 3. Edge cases (first page, boundary values) are handled properly
//! 4. Pagination calculations are accurate
//!
//! Test Categories:
//! - Category 1: Offset-Based Pagination (10 tests)
//! - Category 2: Page-Based Pagination (7 tests)
//! - Category 3: Cursor-Based Pagination (Documentation only - future implementation)
//!
//! Total: 17 tests
//!
//! Note: Error handling tests (network errors, HTTP errors) require WASM
//! environment with mock server.

use reinhardt_pages::api::ApiQuerySet;

// ============================================================================
// Category 1: Offset-Based Pagination (10 tests)
// ============================================================================

/// Tests limit parameter in URL
#[test]
fn test_offset_pagination_limit() {
	let qs: ApiQuerySet<serde_json::Value> = ApiQuerySet::new("/api/users/").limit(20);

	let url = qs.build_url();
	assert!(url.contains("limit=20"));
}

/// Tests offset parameter in URL
#[test]
fn test_offset_pagination_offset() {
	let qs: ApiQuerySet<serde_json::Value> = ApiQuerySet::new("/api/users/").offset(40);

	let url = qs.build_url();
	assert!(url.contains("offset=40"));
}

/// Tests combined limit and offset
#[test]
fn test_offset_pagination_combined() {
	let qs: ApiQuerySet<serde_json::Value> = ApiQuerySet::new("/api/users/").limit(10).offset(30);

	let url = qs.build_url();
	assert!(url.contains("limit=10"));
	assert!(url.contains("offset=30"));
}

/// Tests first page (offset = 0)
#[test]
fn test_offset_pagination_first_page() {
	let qs: ApiQuerySet<serde_json::Value> = ApiQuerySet::new("/api/users/").limit(10).offset(0);

	let url = qs.build_url();
	assert!(url.contains("limit=10"));
	assert!(url.contains("offset=0"));
}

/// Tests large limit value
#[test]
fn test_offset_pagination_large_limit() {
	let qs: ApiQuerySet<serde_json::Value> = ApiQuerySet::new("/api/users/").limit(1000);

	let url = qs.build_url();
	assert!(url.contains("limit=1000"));
}

/// Tests large offset value
#[test]
fn test_offset_pagination_large_offset() {
	let qs: ApiQuerySet<serde_json::Value> = ApiQuerySet::new("/api/users/").offset(999999);

	let url = qs.build_url();
	assert!(url.contains("offset=999999"));
}

/// Tests pagination with filters
#[test]
fn test_offset_pagination_with_filters() {
	let qs: ApiQuerySet<serde_json::Value> = ApiQuerySet::new("/api/users/")
		.filter("is_active", true)
		.limit(10)
		.offset(20);

	let url = qs.build_url();
	assert!(url.contains("is_active=true"));
	assert!(url.contains("limit=10"));
	assert!(url.contains("offset=20"));
}

/// Tests pagination with ordering
#[test]
fn test_offset_pagination_with_ordering() {
	let qs: ApiQuerySet<serde_json::Value> = ApiQuerySet::new("/api/users/")
		.order_by(&["-created_at"])
		.limit(10)
		.offset(0);

	let url = qs.build_url();
	assert!(url.contains("ordering=-created_at"));
	assert!(url.contains("limit=10"));
}

/// Tests updating limit on existing QuerySet
#[test]
fn test_offset_pagination_update_limit() {
	let qs1: ApiQuerySet<serde_json::Value> = ApiQuerySet::new("/api/users/").limit(10);

	let qs2 = qs1.limit(20);

	let url = qs2.build_url();
	assert!(url.contains("limit=20"));
}

/// Tests updating offset on existing QuerySet
#[test]
fn test_offset_pagination_update_offset() {
	let qs1: ApiQuerySet<serde_json::Value> = ApiQuerySet::new("/api/users/").offset(0);

	let qs2 = qs1.offset(10);

	let url = qs2.build_url();
	assert!(url.contains("offset=10"));
}

// ============================================================================
// Category 2: Page-Based Pagination (7 tests)
// ============================================================================

/// Tests calculating offset from page number (page 1, page_size 10)
#[test]
fn test_page_based_first_page() {
	let page = 1;
	let page_size = 10;
	let offset = (page - 1) * page_size;

	let qs: ApiQuerySet<serde_json::Value> = ApiQuerySet::new("/api/users/")
		.limit(page_size)
		.offset(offset);

	let url = qs.build_url();
	assert!(url.contains("limit=10"));
	assert!(url.contains("offset=0"));
}

/// Tests calculating offset from page number (page 2, page_size 10)
#[test]
fn test_page_based_second_page() {
	let page = 2;
	let page_size = 10;
	let offset = (page - 1) * page_size;

	let qs: ApiQuerySet<serde_json::Value> = ApiQuerySet::new("/api/users/")
		.limit(page_size)
		.offset(offset);

	let url = qs.build_url();
	assert!(url.contains("limit=10"));
	assert!(url.contains("offset=10"));
}

/// Tests calculating offset from page number (page 5, page_size 20)
#[test]
fn test_page_based_fifth_page() {
	let page = 5;
	let page_size = 20;
	let offset = (page - 1) * page_size;

	let qs: ApiQuerySet<serde_json::Value> = ApiQuerySet::new("/api/users/")
		.limit(page_size)
		.offset(offset);

	let url = qs.build_url();
	assert!(url.contains("limit=20"));
	assert!(url.contains("offset=80"));
}

/// Tests page-based pagination with custom page size
#[test]
fn test_page_based_custom_page_size() {
	let page = 3;
	let page_size = 25;
	let offset = (page - 1) * page_size;

	let qs: ApiQuerySet<serde_json::Value> = ApiQuerySet::new("/api/users/")
		.limit(page_size)
		.offset(offset);

	let url = qs.build_url();
	assert!(url.contains("limit=25"));
	assert!(url.contains("offset=50"));
}

/// Tests page-based pagination with filters
#[test]
fn test_page_based_with_filters() {
	let page = 2;
	let page_size = 10;
	let offset = (page - 1) * page_size;

	let qs: ApiQuerySet<serde_json::Value> = ApiQuerySet::new("/api/users/")
		.filter("is_active", true)
		.limit(page_size)
		.offset(offset);

	let url = qs.build_url();
	assert!(url.contains("is_active=true"));
	assert!(url.contains("limit=10"));
	assert!(url.contains("offset=10"));
}

/// Tests calculating total pages from count and page size
#[test]
fn test_page_calculation_total_pages() {
	let total_count = 95;
	let page_size = 10;
	let total_pages = (total_count + page_size - 1) / page_size; // Ceiling division

	assert_eq!(total_pages, 10); // 95 items / 10 per page = 10 pages
}

/// Tests page calculation for exact multiple
#[test]
fn test_page_calculation_exact_multiple() {
	let total_count = 100;
	let page_size = 10;
	let total_pages = (total_count + page_size - 1) / page_size;

	assert_eq!(total_pages, 10); // 100 items / 10 per page = 10 pages exactly
}

// ============================================================================
// Category 3: Cursor-Based Pagination (Documentation)
// ============================================================================

/// Documentation test for cursor-based pagination (future implementation)
///
/// Cursor-based pagination is planned for future implementation and will provide:
///
/// 1. **Stable Pagination**: Cursors remain valid even when data changes
/// 2. **Performance**: More efficient for large datasets
/// 3. **API Design**:
///    ```ignore
///    let qs = ApiQuerySet::new("/api/users/")
///        .cursor_after("eyJpZCI6MTAwfQ==")
///        .limit(10);
///    ```
/// 4. **Response Format**:
///    ```json
///    {
///      "results": [...],
///      "next": "eyJpZCI6MTEwfQ==",
///      "previous": "eyJpZCI6OTB9",
///      "has_next": true,
///      "has_previous": true
///    }
///    ```
/// 5. **Implementation Requirements**:
///    - Add `cursor_after` and `cursor_before` methods to ApiQuerySet
///    - Server must support cursor encoding/decoding
///    - Cursors should be opaque (base64-encoded JSON)
///    - Support bidirectional pagination
///
/// See: <https://jsonapi.org/profiles/ethanresnick/cursor-pagination/>
#[test]
fn test_cursor_pagination_documentation() {
	// This test exists solely for documentation purposes
	// Actual implementation will be added in a future phase

	// Example of how cursor pagination would work:
	// let qs: ApiQuerySet<serde_json::Value> = ApiQuerySet::new("/api/users/")
	//     .cursor_after("eyJpZCI6MTAwfQ==")
	//     .limit(10);
	//
	// let url = qs.build_url();
	// assert!(url.contains("cursor=eyJpZCI6MTAwfQ=="));
	// assert!(url.contains("limit=10"));
}
