//! Integration tests for reinhardt-core pagination module
//!
//! Tests cover PageNumberPagination, LimitOffsetPagination, CursorPagination,
//! CursorPaginator (database-optimized), PaginatorImpl enum dispatch,
//! boundary conditions, metadata verification, and error handling.

use reinhardt_core::pagination::{
	CursorPagination, CursorPaginator, DatabaseCursor, ErrorMessages, HasTimestamp,
	LimitOffsetPagination, Page, PageNumberPagination, PaginatedResponse, PaginationMetadata,
	Paginator, PaginatorImpl,
};
use rstest::rstest;

// ============================================================================
// Test helpers
// ============================================================================

/// Test struct implementing HasTimestamp for CursorPaginator tests
#[derive(Debug, Clone, PartialEq)]
struct TestRecord {
	id: i64,
	created_at: i64,
	label: String,
}

impl HasTimestamp for TestRecord {
	fn id(&self) -> i64 {
		self.id
	}

	fn timestamp(&self) -> i64 {
		self.created_at
	}
}

fn make_records(count: usize) -> Vec<TestRecord> {
	(1..=count)
		.map(|i| TestRecord {
			id: i as i64,
			created_at: (i as i64) * 1000,
			label: format!("record-{}", i),
		})
		.collect()
}

fn make_items(count: usize) -> Vec<i32> {
	(1..=count as i32).collect()
}

const BASE_URL: &str = "http://api.example.com/items";

// ============================================================================
// PageNumberPagination tests
// ============================================================================

#[rstest]
fn page_number_basic_first_page() {
	// Arrange
	let items = make_items(25);
	let paginator = PageNumberPagination::new().page_size(10);

	// Act
	let response = paginator.paginate(&items, None, BASE_URL).unwrap();

	// Assert
	assert_eq!(response.results.len(), 10);
	assert_eq!(response.results[0], 1);
	assert_eq!(response.results[9], 10);
	assert_eq!(response.count, 25);
	assert!(response.next.is_some());
	assert!(response.previous.is_none());
}

#[rstest]
fn page_number_explicit_page_1() {
	// Arrange
	let items = make_items(25);
	let paginator = PageNumberPagination::new().page_size(10);

	// Act
	let response = paginator.paginate(&items, Some("1"), BASE_URL).unwrap();

	// Assert
	assert_eq!(response.results.len(), 10);
	assert_eq!(response.results[0], 1);
	assert_eq!(response.count, 25);
	assert!(response.next.is_some());
	assert!(response.previous.is_none());
}

#[rstest]
fn page_number_page_2() {
	// Arrange
	let items = make_items(25);
	let paginator = PageNumberPagination::new().page_size(10);

	// Act
	let response = paginator.paginate(&items, Some("2"), BASE_URL).unwrap();

	// Assert
	assert_eq!(response.results.len(), 10);
	assert_eq!(response.results[0], 11);
	assert_eq!(response.results[9], 20);
	assert_eq!(response.count, 25);
	assert!(response.next.is_some());
	assert!(response.previous.is_some());
}

#[rstest]
fn page_number_last_page() {
	// Arrange
	let items = make_items(25);
	let paginator = PageNumberPagination::new().page_size(10);

	// Act
	let response = paginator.paginate(&items, Some("3"), BASE_URL).unwrap();

	// Assert
	assert_eq!(response.results.len(), 5);
	assert_eq!(response.results[0], 21);
	assert_eq!(response.results[4], 25);
	assert_eq!(response.count, 25);
	assert!(response.next.is_none());
	assert!(response.previous.is_some());
}

#[rstest]
fn page_number_last_page_string() {
	// Arrange
	let items = make_items(25);
	let paginator = PageNumberPagination::new().page_size(10);

	// Act
	let response = paginator.paginate(&items, Some("last"), BASE_URL).unwrap();

	// Assert
	assert_eq!(response.results.len(), 5);
	assert_eq!(response.results[0], 21);
	assert!(response.next.is_none());
}

#[rstest]
fn page_number_invalid_page_returns_error() {
	// Arrange
	let items = make_items(25);
	let paginator = PageNumberPagination::new().page_size(10);

	// Act
	let result = paginator.paginate(&items, Some("abc"), BASE_URL);

	// Assert
	assert!(result.is_err());
}

#[rstest]
fn page_number_page_zero_returns_error() {
	// Arrange
	let items = make_items(25);
	let paginator = PageNumberPagination::new().page_size(10);

	// Act
	let result = paginator.paginate(&items, Some("0"), BASE_URL);

	// Assert
	assert!(result.is_err());
}

#[rstest]
fn page_number_page_beyond_range_returns_error() {
	// Arrange
	let items = make_items(25);
	let paginator = PageNumberPagination::new().page_size(10);

	// Act
	let result = paginator.paginate(&items, Some("100"), BASE_URL);

	// Assert
	assert!(result.is_err());
}

#[rstest]
fn page_number_empty_items_allowed_first_page() {
	// Arrange
	let items: Vec<i32> = vec![];
	let paginator = PageNumberPagination::new()
		.page_size(10)
		.allow_empty_first_page(true);

	// Act
	let response = paginator.paginate(&items, None, BASE_URL).unwrap();

	// Assert
	assert_eq!(response.results.len(), 0);
	assert_eq!(response.count, 0);
	assert!(response.next.is_none());
	assert!(response.previous.is_none());
}

#[rstest]
fn page_number_empty_items_disallowed_first_page() {
	// Arrange
	let items: Vec<i32> = vec![];
	let paginator = PageNumberPagination::new()
		.page_size(10)
		.allow_empty_first_page(false);

	// Act
	let result = paginator.paginate(&items, None, BASE_URL);

	// Assert
	assert!(result.is_err());
}

#[rstest]
fn page_number_single_item() {
	// Arrange
	let items = vec![42];
	let paginator = PageNumberPagination::new().page_size(10);

	// Act
	let response = paginator.paginate(&items, None, BASE_URL).unwrap();

	// Assert
	assert_eq!(response.results.len(), 1);
	assert_eq!(response.results[0], 42);
	assert_eq!(response.count, 1);
	assert!(response.next.is_none());
	assert!(response.previous.is_none());
}

#[rstest]
fn page_number_exact_page_size_boundary() {
	// Arrange
	let items = make_items(10);
	let paginator = PageNumberPagination::new().page_size(10);

	// Act
	let response = paginator.paginate(&items, None, BASE_URL).unwrap();

	// Assert
	assert_eq!(response.results.len(), 10);
	assert_eq!(response.count, 10);
	assert!(response.next.is_none());
	assert!(response.previous.is_none());
}

#[rstest]
fn page_number_exact_page_size_plus_one() {
	// Arrange
	let items = make_items(11);
	let paginator = PageNumberPagination::new().page_size(10);

	// Act
	let page1 = paginator.paginate(&items, Some("1"), BASE_URL).unwrap();
	let page2 = paginator.paginate(&items, Some("2"), BASE_URL).unwrap();

	// Assert
	assert_eq!(page1.results.len(), 10);
	assert!(page1.next.is_some());
	assert_eq!(page2.results.len(), 1);
	assert!(page2.next.is_none());
}

#[rstest]
fn page_number_orphans_merge_last_page() {
	// Arrange
	// 13 items, page_size=5, orphans=3 -> pages: 5, 5, 3
	// Since remainder (3) <= orphans (3), last page merges:
	// page 1: 5 items, page 2: 8 items (5+3)
	let items = make_items(13);
	let paginator = PageNumberPagination::new().page_size(5).orphans(3);

	// Act
	let page1 = paginator.paginate(&items, Some("1"), BASE_URL).unwrap();
	let page2 = paginator.paginate(&items, Some("2"), BASE_URL).unwrap();

	// Assert
	assert_eq!(page1.results.len(), 5);
	assert!(page1.next.is_some());
	assert_eq!(page2.results.len(), 8);
	assert!(page2.next.is_none());
}

#[rstest]
fn page_number_custom_error_messages() {
	// Arrange
	let items = make_items(10);
	let messages = ErrorMessages {
		invalid_page: "Custom invalid page".into(),
		min_page: "Custom min page".into(),
		no_results: "Custom no results".into(),
	};
	let paginator = PageNumberPagination::new()
		.page_size(10)
		.error_messages(messages);

	// Act
	let result = paginator.paginate(&items, Some("abc"), BASE_URL);

	// Assert
	assert!(result.is_err());
	let err_msg = format!("{}", result.unwrap_err());
	assert!(
		err_msg.contains("Custom invalid page"),
		"Error message should contain custom text, got: {}",
		err_msg
	);
}

#[rstest]
fn page_number_metadata_next_previous_urls() {
	// Arrange
	let items = make_items(30);
	let paginator = PageNumberPagination::new().page_size(10);

	// Act
	let response = paginator.paginate(&items, Some("2"), BASE_URL).unwrap();

	// Assert
	let next_url = response.next.as_ref().unwrap();
	let prev_url = response.previous.as_ref().unwrap();
	assert!(
		next_url.contains("page=3"),
		"Next URL should contain page=3, got: {}",
		next_url
	);
	assert!(
		prev_url.contains("page=1"),
		"Previous URL should contain page=1, got: {}",
		prev_url
	);
}

// ============================================================================
// Page type tests
// ============================================================================

#[rstest]
fn page_has_next_and_previous() {
	// Arrange
	let items = vec![1, 2, 3];
	let page = Page::new(items, 2, 5, 50, 10);

	// Act
	let has_next = page.has_next();
	let has_previous = page.has_previous();
	let has_other = page.has_other_pages();
	let next_num = page.next_page_number().unwrap();
	let prev_num = page.previous_page_number().unwrap();

	// Assert
	assert!(has_next);
	assert!(has_previous);
	assert!(has_other);
	assert_eq!(next_num, 3);
	assert_eq!(prev_num, 1);
}

#[rstest]
fn page_first_page_no_previous() {
	// Arrange
	let items = vec![1, 2, 3];
	let page = Page::new(items, 1, 5, 50, 10);

	// Act
	let has_next = page.has_next();
	let has_previous = page.has_previous();
	let prev_result = page.previous_page_number();

	// Assert
	assert!(has_next);
	assert!(!has_previous);
	assert!(prev_result.is_err());
}

#[rstest]
fn page_last_page_no_next() {
	// Arrange
	let items = vec![1];
	let page = Page::new(items, 5, 5, 50, 10);

	// Act
	let has_next = page.has_next();
	let has_previous = page.has_previous();
	let next_result = page.next_page_number();

	// Assert
	assert!(!has_next);
	assert!(has_previous);
	assert!(next_result.is_err());
}

#[rstest]
fn page_single_page_no_other_pages() {
	// Arrange
	let items = vec![1];
	let page = Page::new(items, 1, 1, 1, 10);

	// Act
	let has_next = page.has_next();
	let has_previous = page.has_previous();
	let has_other = page.has_other_pages();

	// Assert
	assert!(!has_next);
	assert!(!has_previous);
	assert!(!has_other);
}

#[rstest]
fn page_start_and_end_index() {
	// Arrange
	let items = vec!["a", "b", "c"];
	let page = Page::new(items, 2, 5, 15, 3);

	// Act
	let start = page.start_index();
	let end = page.end_index();

	// Assert
	assert_eq!(start, 4); // (2-1)*3 + 1 = 4
	assert_eq!(end, 6); // 4 + 3 - 1 = 6
}

#[rstest]
fn page_empty_page_indices() {
	// Arrange
	let items: Vec<i32> = vec![];
	let page = Page::new(items, 1, 1, 0, 10);

	// Act
	let start = page.start_index();
	let end = page.end_index();
	let is_empty = page.is_empty();
	let len = page.len();

	// Assert
	assert_eq!(start, 0);
	assert_eq!(end, 0);
	assert!(is_empty);
	assert_eq!(len, 0);
}

#[rstest]
fn page_range_and_elided() {
	// Arrange
	let items = vec![1];
	let page = Page::new(items, 1, 5, 50, 10);

	// Act
	let range: Vec<usize> = page.page_range().collect();

	// Assert
	assert_eq!(range, vec![1, 2, 3, 4, 5]);
}

#[rstest]
fn page_get_page_lenient_invalid_input() {
	// Arrange
	let items = make_items(20);
	let paginator = PageNumberPagination::new().page_size(5);

	// Act - invalid page defaults to page 1
	let page = paginator.get_page(&items, Some("invalid"));

	// Assert
	assert_eq!(page.number, 1);
	assert_eq!(page.len(), 5);
}

#[rstest]
fn page_get_page_lenient_out_of_range() {
	// Arrange
	let items = make_items(20);
	let paginator = PageNumberPagination::new().page_size(5);

	// Act - out of range returns last page
	let page = paginator.get_page(&items, Some("100"));

	// Assert
	assert_eq!(page.number, 4);
	assert_eq!(page.len(), 5);
}

// ============================================================================
// LimitOffsetPagination tests
// ============================================================================

#[rstest]
fn limit_offset_basic_default_params() {
	// Arrange
	let items = make_items(25);
	let paginator = LimitOffsetPagination::new().default_limit(10);

	// Act
	let response = paginator.paginate(&items, None, BASE_URL).unwrap();

	// Assert
	assert_eq!(response.results.len(), 10);
	assert_eq!(response.results[0], 1);
	assert_eq!(response.count, 25);
	assert!(response.next.is_some());
	assert!(response.previous.is_none());
}

#[rstest]
fn limit_offset_with_offset_and_limit() {
	// Arrange
	let items = make_items(25);
	let paginator = LimitOffsetPagination::new().default_limit(10);

	// Act
	let response = paginator
		.paginate(&items, Some("offset=10&limit=5"), BASE_URL)
		.unwrap();

	// Assert
	assert_eq!(response.results.len(), 5);
	assert_eq!(response.results[0], 11);
	assert_eq!(response.results[4], 15);
	assert_eq!(response.count, 25);
	assert!(response.next.is_some());
	assert!(response.previous.is_some());
}

#[rstest]
fn limit_offset_last_slice() {
	// Arrange
	let items = make_items(25);
	let paginator = LimitOffsetPagination::new().default_limit(10);

	// Act
	let response = paginator
		.paginate(&items, Some("offset=20&limit=10"), BASE_URL)
		.unwrap();

	// Assert
	assert_eq!(response.results.len(), 5);
	assert_eq!(response.results[0], 21);
	assert_eq!(response.count, 25);
	assert!(response.next.is_none());
	assert!(response.previous.is_some());
}

#[rstest]
fn limit_offset_offset_beyond_total() {
	// Arrange
	let items = make_items(10);
	let paginator = LimitOffsetPagination::new().default_limit(5);

	// Act
	let response = paginator
		.paginate(&items, Some("offset=100&limit=5"), BASE_URL)
		.unwrap();

	// Assert
	assert_eq!(response.results.len(), 0);
	assert_eq!(response.count, 10);
	assert!(response.next.is_none());
	assert!(response.previous.is_none());
}

#[rstest]
fn limit_offset_empty_items() {
	// Arrange
	let items: Vec<i32> = vec![];
	let paginator = LimitOffsetPagination::new().default_limit(10);

	// Act
	let response = paginator.paginate(&items, None, BASE_URL).unwrap();

	// Assert
	assert_eq!(response.results.len(), 0);
	assert_eq!(response.count, 0);
	assert!(response.next.is_none());
	assert!(response.previous.is_none());
}

#[rstest]
fn limit_offset_single_item() {
	// Arrange
	let items = vec![42];
	let paginator = LimitOffsetPagination::new().default_limit(10);

	// Act
	let response = paginator.paginate(&items, None, BASE_URL).unwrap();

	// Assert
	assert_eq!(response.results.len(), 1);
	assert_eq!(response.results[0], 42);
	assert_eq!(response.count, 1);
	assert!(response.next.is_none());
}

#[rstest]
fn limit_offset_max_limit_exceeded() {
	// Arrange
	let items = make_items(100);
	let paginator = LimitOffsetPagination::new().default_limit(10).max_limit(20);

	// Act
	let result = paginator.paginate(&items, Some("limit=50"), BASE_URL);

	// Assert
	assert!(result.is_err());
}

#[rstest]
fn limit_offset_metadata_urls() {
	// Arrange
	let items = make_items(30);
	let paginator = LimitOffsetPagination::new().default_limit(10);

	// Act
	let response = paginator
		.paginate(&items, Some("offset=10&limit=10"), BASE_URL)
		.unwrap();

	// Assert
	let next_url = response.next.as_ref().unwrap();
	let prev_url = response.previous.as_ref().unwrap();
	assert!(
		next_url.contains("offset=20"),
		"Next URL should contain offset=20, got: {}",
		next_url
	);
	assert!(
		next_url.contains("limit=10"),
		"Next URL should contain limit=10, got: {}",
		next_url
	);
	assert!(
		prev_url.contains("offset=0"),
		"Previous URL should contain offset=0, got: {}",
		prev_url
	);
}

// ============================================================================
// CursorPagination (in-memory, offset-based) tests
// ============================================================================

#[rstest]
fn cursor_pagination_first_page() {
	// Arrange
	let items = make_items(25);
	let paginator = CursorPagination::new().page_size(10);

	// Act
	let response = paginator.paginate(&items, None, BASE_URL).unwrap();

	// Assert
	assert_eq!(response.results.len(), 10);
	assert_eq!(response.results[0], 1);
	assert_eq!(response.count, 25);
	assert!(response.next.is_some());
}

#[rstest]
fn cursor_pagination_bidirectional() {
	// Arrange
	let items = make_items(30);
	let paginator = CursorPagination::new().page_size(10).with_bidirectional();

	// Act - get first page
	let page1 = paginator.paginate(&items, None, BASE_URL).unwrap();

	// Assert - first page has no previous even in bidirectional mode
	assert_eq!(page1.results.len(), 10);
	assert!(page1.next.is_some());
	assert!(page1.previous.is_none()); // No previous on first page
}

#[rstest]
fn cursor_pagination_empty_items() {
	// Arrange
	let items: Vec<i32> = vec![];
	let paginator = CursorPagination::new().page_size(10);

	// Act
	let response = paginator.paginate(&items, None, BASE_URL).unwrap();

	// Assert
	assert_eq!(response.results.len(), 0);
	assert_eq!(response.count, 0);
	assert!(response.next.is_none());
}

#[rstest]
fn cursor_pagination_exact_page_size() {
	// Arrange
	let items = make_items(10);
	let paginator = CursorPagination::new().page_size(10);

	// Act
	let response = paginator.paginate(&items, None, BASE_URL).unwrap();

	// Assert
	assert_eq!(response.results.len(), 10);
	assert!(response.next.is_none());
}

// ============================================================================
// CursorPaginator (database-optimized) tests
// ============================================================================

#[rstest]
fn cursor_paginator_first_page_forward() {
	// Arrange
	let records = make_records(25);
	let paginator = CursorPaginator::new(10);

	// Act
	let page = paginator.paginate(&records, None).unwrap();

	// Assert
	assert_eq!(page.results.len(), 10);
	assert_eq!(page.results[0].id, 1);
	assert_eq!(page.results[9].id, 10);
	assert!(page.has_next);
	assert!(!page.has_prev);
	assert!(page.next_cursor.is_some());
	assert!(page.prev_cursor.is_none());
}

#[rstest]
fn cursor_paginator_navigate_pages() {
	// Arrange
	let records = make_records(25);
	let paginator = CursorPaginator::new(10);

	// Act - page 1
	let page1 = paginator.paginate(&records, None).unwrap();
	let cursor1 = page1.next_cursor.clone().unwrap();

	// Act - page 2
	let page2 = paginator.paginate(&records, Some(cursor1)).unwrap();
	let cursor2 = page2.next_cursor.clone().unwrap();

	// Act - page 3 (last)
	let page3 = paginator.paginate(&records, Some(cursor2)).unwrap();

	// Assert
	assert_eq!(page1.results.len(), 10);
	assert_eq!(page2.results.len(), 10);
	assert_eq!(page2.results[0].id, 11);
	assert!(page2.has_next);
	assert!(page2.has_prev);
	assert_eq!(page3.results.len(), 5);
	assert_eq!(page3.results[0].id, 21);
	assert!(!page3.has_next);
	assert!(page3.has_prev);
}

#[rstest]
fn cursor_paginator_empty_collection() {
	// Arrange
	let records: Vec<TestRecord> = vec![];
	let paginator = CursorPaginator::new(10);

	// Act
	let page = paginator.paginate(&records, None).unwrap();

	// Assert
	assert_eq!(page.results.len(), 0);
	assert!(!page.has_next);
	assert!(!page.has_prev);
	assert!(page.next_cursor.is_none());
	assert!(page.prev_cursor.is_none());
}

#[rstest]
fn cursor_paginator_single_record() {
	// Arrange
	let records = make_records(1);
	let paginator = CursorPaginator::new(10);

	// Act
	let page = paginator.paginate(&records, None).unwrap();

	// Assert
	assert_eq!(page.results.len(), 1);
	assert_eq!(page.results[0].id, 1);
	assert!(!page.has_next);
	assert!(!page.has_prev);
}

#[rstest]
fn cursor_paginator_exact_page_size() {
	// Arrange
	let records = make_records(10);
	let paginator = CursorPaginator::new(10);

	// Act
	let page = paginator.paginate(&records, None).unwrap();

	// Assert
	assert_eq!(page.results.len(), 10);
	assert!(!page.has_next);
	assert!(!page.has_prev);
}

#[rstest]
fn cursor_paginator_one_over_page_size() {
	// Arrange
	let records = make_records(11);
	let paginator = CursorPaginator::new(10);

	// Act
	let page1 = paginator.paginate(&records, None).unwrap();

	// Assert
	assert_eq!(page1.results.len(), 10);
	assert!(page1.has_next);

	// Act - page 2
	let cursor = page1.next_cursor.unwrap();
	let page2 = paginator.paginate(&records, Some(cursor)).unwrap();

	// Assert
	assert_eq!(page2.results.len(), 1);
	assert!(!page2.has_next);
}

#[rstest]
fn cursor_paginator_invalid_cursor_returns_error() {
	// Arrange
	let records = make_records(10);
	let paginator = CursorPaginator::new(5);

	// Act
	let result = paginator.paginate(&records, Some("not_valid_cursor".into()));

	// Assert
	assert!(result.is_err());
}

#[rstest]
fn cursor_paginator_tie_breaking_same_id() {
	// Arrange - items with same id but different timestamps
	let records = vec![
		TestRecord {
			id: 1,
			created_at: 1000,
			label: "first".into(),
		},
		TestRecord {
			id: 1,
			created_at: 2000,
			label: "second".into(),
		},
		TestRecord {
			id: 2,
			created_at: 3000,
			label: "third".into(),
		},
	];
	let paginator = CursorPaginator::new(1);

	// Act
	let page1 = paginator.paginate(&records, None).unwrap();

	// Assert
	assert_eq!(page1.results.len(), 1);
	assert_eq!(page1.results[0].created_at, 1000);
	assert!(page1.has_next);

	// Act - page 2 via cursor
	let cursor = page1.next_cursor.unwrap();
	let page2 = paginator.paginate(&records, Some(cursor)).unwrap();

	// Assert - tie-breaker ensures we get the next item
	assert_eq!(page2.results.len(), 1);
	assert_eq!(page2.results[0].created_at, 2000);
}

#[rstest]
fn cursor_paginator_cursor_stability() {
	// Arrange
	let records = make_records(20);
	let paginator = CursorPaginator::new(5);

	// Act - paginate twice from the same starting point
	let page_a = paginator.paginate(&records, None).unwrap();
	let page_b = paginator.paginate(&records, None).unwrap();

	// Assert - cursors should be identical for same position
	assert_eq!(page_a.next_cursor, page_b.next_cursor);
	assert_eq!(page_a.results, page_b.results);
}

#[rstest]
fn database_cursor_encode_decode_roundtrip() {
	// Arrange
	let cursor = DatabaseCursor::new(42, 1234567890);

	// Act
	let encoded = cursor.encode();
	let decoded = DatabaseCursor::decode(&encoded).unwrap();

	// Assert
	assert_eq!(decoded.id, 42);
	assert_eq!(decoded.timestamp, 1234567890);
}

#[rstest]
fn database_cursor_invalid_decode() {
	// Act
	let result = DatabaseCursor::decode("totally-invalid");

	// Assert
	assert!(result.is_err());
}

// ============================================================================
// PaginatorImpl enum dispatch tests
// ============================================================================

#[rstest]
fn paginator_impl_page_number_dispatch() {
	// Arrange
	let items = make_items(20);
	let paginator = PaginatorImpl::page_number(PageNumberPagination::new().page_size(5));

	// Act
	let response = paginator.paginate(&items, Some("1"), BASE_URL).unwrap();

	// Assert
	assert_eq!(response.results.len(), 5);
	assert_eq!(response.results[0], 1);
	assert_eq!(response.count, 20);
}

#[rstest]
fn paginator_impl_limit_offset_dispatch() {
	// Arrange
	let items = make_items(20);
	let paginator = PaginatorImpl::limit_offset(LimitOffsetPagination::new().default_limit(5));

	// Act
	let response = paginator
		.paginate(&items, Some("offset=5&limit=5"), BASE_URL)
		.unwrap();

	// Assert
	assert_eq!(response.results.len(), 5);
	assert_eq!(response.results[0], 6);
	assert_eq!(response.count, 20);
}

#[rstest]
fn paginator_impl_cursor_dispatch() {
	// Arrange
	let items = make_items(20);
	let paginator = PaginatorImpl::cursor(CursorPagination::new().page_size(5));

	// Act
	let response = paginator.paginate(&items, None, BASE_URL).unwrap();

	// Assert
	assert_eq!(response.results.len(), 5);
	assert_eq!(response.results[0], 1);
	assert_eq!(response.count, 20);
}

#[rstest]
fn paginator_impl_schema_parameters() {
	// Arrange
	let page_num = PaginatorImpl::page_number(PageNumberPagination::new());
	let limit_off = PaginatorImpl::limit_offset(LimitOffsetPagination::new());
	let cursor = PaginatorImpl::cursor(CursorPagination::new());

	// Act
	let page_num_params = page_num.get_schema_parameters();
	let limit_off_params = limit_off.get_schema_parameters();
	let cursor_params = cursor.get_schema_parameters();

	// Assert
	assert!(
		!page_num_params.is_empty(),
		"PageNumber should have schema parameters"
	);
	assert!(
		!limit_off_params.is_empty(),
		"LimitOffset should have schema parameters"
	);
	assert!(
		!cursor_params.is_empty(),
		"Cursor should have schema parameters"
	);
}

// ============================================================================
// PaginatedResponse and PaginationMetadata tests
// ============================================================================

#[rstest]
fn paginated_response_new_with_metadata() {
	// Arrange
	let metadata = PaginationMetadata {
		count: 100,
		next: Some("/api/items?page=2".into()),
		previous: None,
	};
	let results = vec![1, 2, 3, 4, 5];

	// Act
	let response = PaginatedResponse::new(results, metadata);

	// Assert
	assert_eq!(response.count, 100);
	assert_eq!(response.results.len(), 5);
	assert!(response.next.is_some());
	assert!(response.previous.is_none());
}

#[rstest]
fn paginated_response_metadata_consistency() {
	// Arrange
	let items = make_items(50);
	let paginator = PageNumberPagination::new().page_size(10);

	// Act
	let page1 = paginator.paginate(&items, Some("1"), BASE_URL).unwrap();
	let page3 = paginator.paginate(&items, Some("3"), BASE_URL).unwrap();
	let page5 = paginator.paginate(&items, Some("5"), BASE_URL).unwrap();

	// Assert - count should be consistent across all pages
	assert_eq!(page1.count, 50);
	assert_eq!(page3.count, 50);
	assert_eq!(page5.count, 50);

	// Assert - first page: no previous, last page: no next
	assert!(page1.previous.is_none());
	assert!(page1.next.is_some());
	assert!(page3.previous.is_some());
	assert!(page3.next.is_some());
	assert!(page5.previous.is_some());
	assert!(page5.next.is_none());
}

// ============================================================================
// Schema parameters tests
// ============================================================================

#[rstest]
fn page_number_schema_parameters() {
	// Arrange
	let paginator = PageNumberPagination::new().page_size_query_param("page_size");

	// Act
	let params = Paginator::get_schema_parameters(&paginator);

	// Assert
	assert_eq!(params.len(), 2);
	assert_eq!(params[0].name, "page");
	assert_eq!(params[1].name, "page_size");
	assert_eq!(params[0].location, "query");
	assert_eq!(params[0].schema_type, "integer");
}

#[rstest]
fn limit_offset_schema_parameters() {
	// Arrange
	let paginator = LimitOffsetPagination::new();

	// Act
	let params = Paginator::get_schema_parameters(&paginator);

	// Assert
	assert_eq!(params.len(), 2);
	let names: Vec<&str> = params.iter().map(|p| p.name.as_str()).collect();
	assert!(names.contains(&"limit"));
	assert!(names.contains(&"offset"));
}

#[rstest]
fn cursor_schema_parameters() {
	// Arrange
	let paginator = CursorPagination::new();

	// Act
	let params = Paginator::get_schema_parameters(&paginator);

	// Assert
	assert!(params.iter().any(|p| p.name == "cursor"));
	assert!(params.iter().any(|p| p.schema_type == "string"));
}
