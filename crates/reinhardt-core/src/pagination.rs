//! # Reinhardt Pagination
//!
//! Pagination support for Reinhardt framework, inspired by Django REST Framework's pagination.
//!
//! ## Pagination Styles
//!
//! - **PageNumberPagination**: Simple page number based pagination
//! - **LimitOffsetPagination**: Limit/offset based pagination
//! - **CursorPagination**: Cursor-based pagination for large datasets with custom encoding
//! - **Database Cursor Pagination**: Optimized cursor-based pagination for database queries
//!
//! ## Features
//!
//! ### Cursor Pagination
//!
//! - **Custom cursor encoding strategies**: Base64, JWT (future), or custom implementations
//! - **Bi-directional pagination**: Navigate forward and backward through datasets
//! - **Relay-style pagination**: GraphQL Relay Cursor Connections Specification
//! - **Custom ordering strategies**: Define how items are ordered for stable pagination
//!
//! ### Database Cursor Pagination (NEW)
//!
//! - **O(k) Performance**: Uses indexed cursor fields (id, timestamp) instead of OFFSET/LIMIT
//! - **Deep Page Efficiency**: 1000x+ faster for accessing deep pages in large datasets
//! - **Stable Ordering**: Tie-breaking with timestamp ensures consistent results
//! - **QuerySet Integration**: Designed to work with database queries (future integration)
//!
//! ## Example
//!
//! ```rust,no_run
//! use reinhardt_core::pagination::PageNumberPagination;
//!
//! let paginator = PageNumberPagination::new()
//!     .page_size(10)
//!     .max_page_size(100);
//! ```
//!
//! ## Cursor Pagination Example
//!
//! ```rust,no_run
//! use reinhardt_core::pagination::CursorPagination;
//! use reinhardt_core::pagination::cursor::RelayPagination;
//!
//! // Standard cursor pagination
//! let paginator = CursorPagination::new()
//!     .page_size(20)
//!     .with_bidirectional();
//!
//! // Relay-style pagination
//! let relay = RelayPagination::new()
//!     .default_page_size(10)
//!     .max_page_size(100);
//! ```
//!
//! ## Database Cursor Pagination Example
//!
//! ```rust,no_run
//! use reinhardt_core::pagination::{CursorPaginator, HasTimestamp};
//!
//! // Define your model
//! #[derive(Clone)]
//! struct User {
//!     id: i64,
//!     created_at: i64,
//!     name: String,
//! }
//!
//! impl HasTimestamp for User {
//!     fn id(&self) -> i64 { self.id }
//!     fn timestamp(&self) -> i64 { self.created_at }
//! }
//!
//! // Use cursor paginator
//! let paginator = CursorPaginator::new(20);  // 20 items per page
//! let users: Vec<User> = vec![];
//! let page1 = paginator.paginate(&users, None).unwrap();
//!
//! // Navigate to next page
//! let page2 = paginator.paginate(&users, page1.next_cursor).unwrap();
//! ```
//!
//! ## Performance Comparison
//!
//! | Method | Complexity | Deep Page Performance |
//! |--------|------------|----------------------|
//! | OFFSET/LIMIT | O(n+k) | Gets progressively slower |
//! | Cursor-based | O(k) | Constant performance |
//!
//! For page 1000 with page size 20:
//! - OFFSET/LIMIT: Database scans ~20,000 rows
//! - Cursor-based: Database scans only 20 rows (with proper indexes)

mod core;
pub mod cursor;
mod limit_offset;
mod page_number;

/// Parses a base URL with fallback for relative paths and malformed URLs.
/// Returns a valid `Url` without panicking regardless of input.
pub(crate) fn parse_base_url(base_url: &str) -> url::Url {
	url::Url::parse(base_url)
		.or_else(|_| url::Url::parse(&format!("http://localhost{}", base_url)))
		// SAFETY: "http://localhost/" is a valid URL constant; parse cannot fail
		.unwrap_or_else(|_| url::Url::parse("http://localhost/").unwrap())
}

// Re-export core types and traits
pub use self::core::{
	AsyncPaginator, Page, PaginatedResponse, PaginationMetadata, Paginator, SchemaParameter,
};

// Re-export pagination implementations
pub use self::cursor::CursorPagination;
pub use self::limit_offset::LimitOffsetPagination;
pub use self::page_number::{ErrorMessages, PageNumberPagination};

// Re-export database cursor types
pub use self::cursor::{
	CursorPaginatedResponse as DatabaseCursorPaginatedResponse, CursorPaginator, DatabaseCursor,
	Direction, HasTimestamp, PaginationError as DatabasePaginationError,
};

use crate::exception::Result;
use async_trait::async_trait;

// ============================================================================
// Enum Wrapper for dyn Paginator Compatibility
// ============================================================================

/// Enum wrapper for Paginator implementations to enable dyn compatibility
///
/// This wrapper allows using different pagination strategies through a single
/// type, solving the issue that `Paginator` trait with generic methods cannot
/// be used as `dyn Paginator`.
#[derive(Debug, Clone)]
pub enum PaginatorImpl {
	/// Page number based pagination
	PageNumber(PageNumberPagination),
	/// Limit/offset based pagination
	LimitOffset(LimitOffsetPagination),
	/// Cursor based pagination
	Cursor(CursorPagination),
}

impl Paginator for PaginatorImpl {
	fn paginate<T: Clone + Send + Sync>(
		&self,
		items: &[T],
		page_param: Option<&str>,
		base_url: &str,
	) -> Result<PaginatedResponse<T>> {
		match self {
			Self::PageNumber(p) => p.paginate(items, page_param, base_url),
			Self::LimitOffset(p) => p.paginate(items, page_param, base_url),
			Self::Cursor(p) => p.paginate(items, page_param, base_url),
		}
	}

	fn get_schema_parameters(&self) -> Vec<SchemaParameter> {
		match self {
			Self::PageNumber(p) => Paginator::get_schema_parameters(p),
			Self::LimitOffset(p) => Paginator::get_schema_parameters(p),
			Self::Cursor(p) => Paginator::get_schema_parameters(p),
		}
	}
}

#[async_trait]
impl AsyncPaginator for PaginatorImpl {
	async fn apaginate<T: Clone + Send + Sync>(
		&self,
		items: &[T],
		page_param: Option<&str>,
		base_url: &str,
	) -> Result<PaginatedResponse<T>> {
		match self {
			Self::PageNumber(p) => p.apaginate(items, page_param, base_url).await,
			Self::LimitOffset(p) => p.apaginate(items, page_param, base_url).await,
			Self::Cursor(p) => p.apaginate(items, page_param, base_url).await,
		}
	}

	fn get_schema_parameters(&self) -> Vec<SchemaParameter> {
		match self {
			Self::PageNumber(p) => AsyncPaginator::get_schema_parameters(p),
			Self::LimitOffset(p) => AsyncPaginator::get_schema_parameters(p),
			Self::Cursor(p) => AsyncPaginator::get_schema_parameters(p),
		}
	}
}

impl PaginatorImpl {
	/// Create a page number pagination instance
	pub fn page_number(pagination: PageNumberPagination) -> Self {
		Self::PageNumber(pagination)
	}

	/// Create a limit/offset pagination instance
	pub fn limit_offset(pagination: LimitOffsetPagination) -> Self {
		Self::LimitOffset(pagination)
	}

	/// Create a cursor pagination instance
	pub fn cursor(pagination: CursorPagination) -> Self {
		Self::Cursor(pagination)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	// ========================================
	// PageNumberPagination Tests
	// ========================================

	#[test]
	fn test_page_number_pagination_first_page() {
		let items: Vec<i32> = (1..=25).collect();
		let paginator = PageNumberPagination::new().page_size(10);

		let page = paginator
			.paginate(&items, Some("1"), "http://api.example.com/items")
			.unwrap();
		assert_eq!(page.results.len(), 10);
		assert_eq!(page.results[0], 1);
		assert_eq!(page.count, 25);
		assert!(page.next.is_some());
		assert!(page.previous.is_none());
	}

	#[test]
	fn test_page_number_pagination_second_page() {
		let items: Vec<i32> = (1..=25).collect();
		let paginator = PageNumberPagination::new().page_size(10);

		let page = paginator
			.paginate(&items, Some("2"), "http://api.example.com/items")
			.unwrap();
		assert_eq!(page.results.len(), 10);
		assert_eq!(page.results[0], 11);
		assert!(page.next.is_some());
		assert!(page.previous.is_some());
	}

	#[test]
	fn test_page_number_pagination_last_page() {
		let items: Vec<i32> = (1..=25).collect();
		let paginator = PageNumberPagination::new().page_size(10);

		let page = paginator
			.paginate(&items, Some("3"), "http://api.example.com/items")
			.unwrap();
		assert_eq!(page.results.len(), 5);
		assert_eq!(page.results[0], 21);
		assert!(page.next.is_none());
		assert!(page.previous.is_some());
	}

	#[test]
	fn test_page_number_pagination_last_keyword() {
		let items: Vec<i32> = (1..=25).collect();
		let paginator = PageNumberPagination::new().page_size(10);

		let page = paginator
			.paginate(&items, Some("last"), "http://api.example.com/items")
			.unwrap();
		assert_eq!(page.results.len(), 5);
		assert_eq!(page.results[0], 21);
		assert!(page.next.is_none());
		assert!(page.previous.is_some());
	}

	#[test]
	fn test_page_number_pagination_no_page_param() {
		let items: Vec<i32> = (1..=25).collect();
		let paginator = PageNumberPagination::new().page_size(5);

		let page = paginator
			.paginate(&items, None, "http://api.example.com/items")
			.unwrap();
		assert_eq!(page.results, vec![1, 2, 3, 4, 5]);
		assert!(page.next.is_some());
		assert!(page.previous.is_none());
	}

	#[test]
	fn test_page_number_pagination_invalid_page() {
		let items: Vec<i32> = (1..=25).collect();
		let paginator = PageNumberPagination::new().page_size(10);

		let result = paginator.paginate(&items, Some("invalid"), "http://api.example.com/items");
		assert!(result.is_err());
		assert!(matches!(
			result.unwrap_err(),
			crate::exception::Error::InvalidPage(_)
		));
	}

	#[test]
	fn test_page_number_pagination_zero_page() {
		let items: Vec<i32> = (1..=25).collect();
		let paginator = PageNumberPagination::new().page_size(10);

		let result = paginator.paginate(&items, Some("0"), "http://api.example.com/items");
		assert!(result.is_err());
		assert!(matches!(
			result.unwrap_err(),
			crate::exception::Error::InvalidPage(_)
		));
	}

	#[test]
	fn test_page_number_pagination_out_of_range() {
		let items: Vec<i32> = (1..=25).collect();
		let paginator = PageNumberPagination::new().page_size(10);

		let result = paginator.paginate(&items, Some("10"), "http://api.example.com/items");
		assert!(result.is_err());
	}

	#[test]
	fn test_page_number_pagination_empty_list() {
		let items: Vec<i32> = vec![];
		let paginator = PageNumberPagination::new().page_size(10);

		let page = paginator
			.paginate(&items, Some("1"), "http://api.example.com/items")
			.unwrap();
		assert_eq!(page.results.len(), 0);
		assert_eq!(page.count, 0);
		assert!(page.next.is_none());
		assert!(page.previous.is_none());
	}

	#[test]
	fn test_page_number_pagination_single_item() {
		let items: Vec<i32> = vec![1];
		let paginator = PageNumberPagination::new().page_size(10);

		let page = paginator
			.paginate(&items, Some("1"), "http://api.example.com/items")
			.unwrap();
		assert_eq!(page.results, vec![1]);
		assert_eq!(page.count, 1);
		assert!(page.next.is_none());
		assert!(page.previous.is_none());
	}

	#[test]
	fn test_page_number_pagination_items_equal_to_page_size() {
		let items: Vec<i32> = (1..=10).collect();
		let paginator = PageNumberPagination::new().page_size(10);

		let page = paginator
			.paginate(&items, Some("1"), "http://api.example.com/items")
			.unwrap();
		assert_eq!(page.results.len(), 10);
		assert!(page.next.is_none());
		assert!(page.previous.is_none());
	}

	#[test]
	fn test_page_number_pagination_items_one_more_than_page_size() {
		let items: Vec<i32> = (1..=11).collect();
		let paginator = PageNumberPagination::new().page_size(10);

		let page1 = paginator
			.paginate(&items, Some("1"), "http://api.example.com/items")
			.unwrap();
		assert_eq!(page1.results.len(), 10);
		assert!(page1.next.is_some());

		let page2 = paginator
			.paginate(&items, Some("2"), "http://api.example.com/items")
			.unwrap();
		assert_eq!(page2.results.len(), 1);
		assert!(page2.next.is_none());
	}

	// ========================================
	// LimitOffsetPagination Tests
	// ========================================

	#[test]
	fn test_limit_offset_pagination_no_params() {
		let items: Vec<i32> = (1..=25).collect();
		let paginator = LimitOffsetPagination::new().default_limit(10);

		let page = paginator
			.paginate(&items, None, "http://api.example.com/items")
			.unwrap();
		assert_eq!(page.results.len(), 10);
		assert_eq!(page.results[0], 1);
		assert_eq!(page.count, 25);
		assert!(page.next.is_some());
		assert!(page.previous.is_none());
	}

	#[test]
	fn test_limit_offset_pagination_with_offset() {
		let items: Vec<i32> = (1..=25).collect();
		let paginator = LimitOffsetPagination::new().default_limit(10);

		let page = paginator
			.paginate(
				&items,
				Some("offset=10&limit=10"),
				"http://api.example.com/items",
			)
			.unwrap();
		assert_eq!(page.results.len(), 10);
		assert_eq!(page.results[0], 11);
		assert!(page.next.is_some());
		assert!(page.previous.is_some());
	}

	#[test]
	fn test_limit_offset_pagination_ending_offset() {
		let items: Vec<i32> = (1..=25).collect();
		let paginator = LimitOffsetPagination::new().default_limit(10);

		let page = paginator
			.paginate(
				&items,
				Some("offset=20&limit=10"),
				"http://api.example.com/items",
			)
			.unwrap();
		assert_eq!(page.results.len(), 5);
		assert_eq!(page.results[0], 21);
		assert!(page.next.is_none());
		assert!(page.previous.is_some());
	}

	#[test]
	fn test_limit_offset_pagination_offset_beyond_count() {
		let items: Vec<i32> = (1..=25).collect();
		let paginator = LimitOffsetPagination::new().default_limit(10);

		let page = paginator
			.paginate(
				&items,
				Some("offset=100&limit=10"),
				"http://api.example.com/items",
			)
			.unwrap();
		assert_eq!(page.results.len(), 0);
		assert!(page.next.is_none());
		assert!(page.previous.is_none());
	}

	#[test]
	fn test_limit_offset_pagination_invalid_limit() {
		let items: Vec<i32> = (1..=25).collect();
		let paginator = LimitOffsetPagination::new().default_limit(10);

		let result = paginator.paginate(
			&items,
			Some("limit=invalid"),
			"http://api.example.com/items",
		);
		assert!(result.is_err());
	}

	#[test]
	fn test_limit_offset_pagination_invalid_offset() {
		let items: Vec<i32> = (1..=25).collect();
		let paginator = LimitOffsetPagination::new().default_limit(10);

		let result = paginator.paginate(
			&items,
			Some("offset=invalid"),
			"http://api.example.com/items",
		);
		assert!(result.is_err());
	}

	#[test]
	fn test_limit_offset_pagination_max_limit() {
		let items: Vec<i32> = (1..=100).collect();
		let paginator = LimitOffsetPagination::new().default_limit(10).max_limit(20);

		let result = paginator.paginate(&items, Some("limit=50"), "http://api.example.com/items");
		assert!(result.is_err());
		assert!(matches!(
			result.unwrap_err(),
			crate::exception::Error::InvalidLimit(_)
		));
	}

	#[test]
	fn test_limit_offset_pagination_within_max_limit() {
		let items: Vec<i32> = (1..=100).collect();
		let paginator = LimitOffsetPagination::new().default_limit(10).max_limit(20);

		let page = paginator
			.paginate(&items, Some("limit=15"), "http://api.example.com/items")
			.unwrap();
		assert_eq!(page.results.len(), 15);
	}

	#[test]
	fn test_limit_offset_pagination_zero_limit_rejected() {
		let items: Vec<i32> = (1..=100).collect();
		let paginator = LimitOffsetPagination::new().default_limit(10);

		let result = paginator.paginate(&items, Some("limit=0"), "http://api.example.com/items");
		assert!(result.is_err());
		let err = result.unwrap_err();
		if let crate::exception::Error::InvalidLimit(msg) = &err {
			assert!(
				msg.contains("greater than zero"),
				"expected zero-limit error message, got: {}",
				msg
			);
		} else {
			panic!("expected InvalidLimit error for zero limit, got: {:?}", err);
		}
	}

	#[test]
	fn test_limit_offset_pagination_limit_one_works() {
		let items: Vec<i32> = (1..=10).collect();
		let paginator = LimitOffsetPagination::new().default_limit(10);

		let page = paginator
			.paginate(&items, Some("limit=1"), "http://api.example.com/items")
			.unwrap();
		assert_eq!(page.results.len(), 1);
		assert_eq!(page.results[0], 1);
		assert!(page.next.is_some());
	}

	// ========================================
	// CursorPagination Tests
	// ========================================

	#[test]
	fn test_cursor_pagination_first_page() {
		let items: Vec<i32> = (1..=25).collect();
		let paginator = CursorPagination::new().page_size(10);

		let page = paginator
			.paginate(&items, None, "http://api.example.com/items")
			.unwrap();
		assert_eq!(page.results.len(), 10);
		assert_eq!(page.results[0], 1);
		assert!(page.next.is_some());
		assert!(page.previous.is_none());
	}

	#[test]
	fn test_cursor_pagination_navigation() {
		let items: Vec<i32> = (1..=25).collect();
		let paginator = CursorPagination::new().page_size(10).with_bidirectional();

		// First page
		let page1 = paginator
			.paginate(&items, None, "http://api.example.com/items")
			.unwrap();
		assert!(page1.next.is_some());

		// Extract cursor from next URL
		let next_url = page1.next.unwrap();
		let url = url::Url::parse(&next_url).unwrap();
		let cursor = url
			.query_pairs()
			.find(|(key, _)| key == "cursor")
			.map(|(_, value)| value.to_string())
			.unwrap();

		// Second page using cursor
		let page2 = paginator
			.paginate(&items, Some(&cursor), "http://api.example.com/items")
			.unwrap();
		assert_eq!(page2.results[0], 11);
		assert!(page2.next.is_some());
		assert!(page2.previous.is_some());
	}

	#[test]
	fn test_cursor_pagination_invalid_cursor() {
		let items: Vec<i32> = (1..=25).collect();
		let paginator = CursorPagination::new().page_size(10);

		let result = paginator.paginate(
			&items,
			Some("invalid_cursor"),
			"http://api.example.com/items",
		);
		assert!(result.is_err());
		assert!(matches!(
			result.unwrap_err(),
			crate::exception::Error::InvalidPage(_)
		));
	}

	#[test]
	fn test_cursor_pagination_empty_list() {
		let items: Vec<i32> = vec![];
		let paginator = CursorPagination::new().page_size(10);

		let page = paginator
			.paginate(&items, None, "http://api.example.com/items")
			.unwrap();
		assert_eq!(page.results.len(), 0);
		assert!(page.next.is_none());
		assert!(page.previous.is_none());
	}

	// ========================================
	// Schema and Configuration Tests
	// ========================================

	#[test]
	fn test_page_number_pagination_schema_parameters() {
		let paginator = PageNumberPagination::new().page_size_query_param("page_size");

		let params = Paginator::get_schema_parameters(&paginator);
		assert_eq!(params.len(), 2);
		assert_eq!(params[0].name, "page");
		assert_eq!(params[1].name, "page_size");
	}

	#[test]
	fn test_limit_offset_pagination_schema_parameters() {
		let paginator = LimitOffsetPagination::new();

		let params = Paginator::get_schema_parameters(&paginator);
		assert_eq!(params.len(), 2);
		assert_eq!(params[0].name, "limit");
		assert_eq!(params[1].name, "offset");
	}

	#[test]
	fn test_cursor_pagination_schema_parameters() {
		let paginator = CursorPagination::new();

		let params = Paginator::get_schema_parameters(&paginator);
		assert_eq!(params.len(), 2);
		assert_eq!(params[0].name, "cursor");
		assert_eq!(params[1].name, "page_size");
	}

	// ========================================
	// Additional Edge Cases
	// ========================================

	#[test]
	fn test_paginated_response_new() {
		let metadata = PaginationMetadata {
			count: 100,
			next: Some("http://example.com/next".to_string()),
			previous: None,
		};
		let results = vec![1, 2, 3];

		let response = PaginatedResponse::new(results.clone(), metadata);
		assert_eq!(response.count, 100);
		assert_eq!(response.results, results);
		assert!(response.next.is_some());
		assert!(response.previous.is_none());
	}

	#[test]
	fn test_page_number_pagination_builder_pattern() {
		let paginator = PageNumberPagination::new()
			.page_size(20)
			.max_page_size(100)
			.page_size_query_param("size");

		assert_eq!(paginator.page_size, 20);
		assert_eq!(paginator.max_page_size, Some(100));
		assert_eq!(paginator.page_size_query_param, Some("size".to_string()));
	}

	#[test]
	fn test_limit_offset_pagination_builder_pattern() {
		let paginator = LimitOffsetPagination::new()
			.default_limit(30)
			.max_limit(200);

		assert_eq!(paginator.default_limit, 30);
		assert_eq!(paginator.max_limit, Some(200));
	}

	#[test]
	fn test_cursor_pagination_builder_pattern() {
		let paginator = CursorPagination::new()
			.page_size(15)
			.max_page_size(50)
			.ordering(vec!["created".to_string()]);

		assert_eq!(paginator.page_size, 15);
		assert_eq!(paginator.max_page_size, Some(50));
		assert_eq!(paginator.ordering, vec!["created".to_string()]);
	}

	#[test]
	fn test_cursor_pagination_with_page_size() {
		let items: Vec<i32> = (1..=30).collect();
		let paginator = CursorPagination::new().page_size(5);

		// Request page_size=20 in URL
		let page1 = paginator
			.paginate(&items, None, "http://api.example.com/items?page_size=20")
			.unwrap();
		assert_eq!(page1.results.len(), 20);
		assert_eq!(page1.results, (1..=20).collect::<Vec<i32>>());

		// Navigate to next page - should preserve page_size
		let next_url = page1.next.unwrap();
		assert!(next_url.contains("page_size=20"));
	}

	#[test]
	fn test_cursor_pagination_with_page_size_over_limit() {
		let items: Vec<i32> = (1..=30).collect();
		let paginator = CursorPagination::new().page_size(5).max_page_size(20);

		// Request page_size=30, should be clamped to max_page_size=20
		let page1 = paginator
			.paginate(&items, None, "http://api.example.com/items?page_size=30")
			.unwrap();
		assert_eq!(page1.results.len(), 20);
		assert_eq!(page1.results, (1..=20).collect::<Vec<i32>>());
	}

	#[test]
	fn test_cursor_pagination_with_page_size_zero() {
		let items: Vec<i32> = (1..=30).collect();
		let paginator = CursorPagination::new().page_size(5);

		// page_size=0 should fall back to default page_size=5
		let page1 = paginator
			.paginate(&items, None, "http://api.example.com/items?page_size=0")
			.unwrap();
		assert_eq!(page1.results.len(), 5);
		assert_eq!(page1.results, vec![1, 2, 3, 4, 5]);
	}

	#[test]
	fn test_cursor_pagination_with_page_size_invalid() {
		let items: Vec<i32> = (1..=30).collect();
		let paginator = CursorPagination::new().page_size(5);

		// Invalid page_size should fall back to default
		let page1 = paginator
			.paginate(&items, None, "http://api.example.com/items?page_size=abc")
			.unwrap();
		assert_eq!(page1.results.len(), 5);
		assert_eq!(page1.results, vec![1, 2, 3, 4, 5]);
	}

	#[test]
	fn test_cursor_pagination_page_size_in_schema() {
		let paginator = CursorPagination::new();
		let params = Paginator::get_schema_parameters(&paginator);

		assert_eq!(params.len(), 2);
		assert_eq!(params[0].name, "cursor");
		assert_eq!(params[1].name, "page_size");
		assert_eq!(params[1].schema_type, "integer");
	}

	// ========================================
	// Page Structure Tests
	// ========================================

	#[test]
	fn test_page_indexes() {
		// Test with full page
		let page = Page::new(vec![1, 2, 3, 4, 5], 1, 3, 15, 5);
		assert_eq!(page.start_index(), 1);
		assert_eq!(page.end_index(), 5);

		// Test with partial page
		let page2 = Page::new(vec![11, 12, 13, 14, 15], 3, 3, 15, 5);
		assert_eq!(page2.start_index(), 11);
		assert_eq!(page2.end_index(), 15);

		// Test with empty page
		let empty_page: Page<i32> = Page::new(vec![], 1, 1, 0, 5);
		assert_eq!(empty_page.start_index(), 0);
		assert_eq!(empty_page.end_index(), 0);
	}

	#[test]
	fn test_page_sequence() {
		let items: Vec<char> = "abcdefghijk".chars().collect();
		let page = Page::new(items[5..11].to_vec(), 2, 2, 11, 5);

		assert_eq!(page.len(), 6);
		assert!(page.get(0).is_some());
		assert_eq!(*page.get(0).unwrap(), 'f');

		// Test iteration
		let collected: Vec<char> = page.clone().into_iter().collect();
		assert_eq!(collected, vec!['f', 'g', 'h', 'i', 'j', 'k']);

		// Test reverse iteration
		let reversed: Vec<char> = collected.into_iter().rev().collect();
		assert_eq!(reversed, vec!['k', 'j', 'i', 'h', 'g', 'f']);
	}

	#[test]
	fn test_page_has_next_previous() {
		// First page
		let page1 = Page::new(vec![1, 2, 3], 1, 3, 9, 3);
		assert!(!page1.has_previous());
		assert!(page1.has_next());
		assert!(page1.has_other_pages());
		assert_eq!(page1.next_page_number().unwrap(), 2);
		assert!(page1.previous_page_number().is_err());

		// Middle page
		let page2 = Page::new(vec![4, 5, 6], 2, 3, 9, 3);
		assert!(page2.has_previous());
		assert!(page2.has_next());
		assert!(page2.has_other_pages());
		assert_eq!(page2.next_page_number().unwrap(), 3);
		assert_eq!(page2.previous_page_number().unwrap(), 1);

		// Last page
		let page3 = Page::new(vec![7, 8, 9], 3, 3, 9, 3);
		assert!(page3.has_previous());
		assert!(!page3.has_next());
		assert!(page3.has_other_pages());
		assert!(page3.next_page_number().is_err());
		assert_eq!(page3.previous_page_number().unwrap(), 2);

		// Single page
		let single_page = Page::new(vec![1, 2, 3], 1, 1, 3, 10);
		assert!(!single_page.has_previous());
		assert!(!single_page.has_next());
		assert!(!single_page.has_other_pages());
	}

	#[test]
	fn test_page_indexing() {
		let page = Page::new(vec![10, 20, 30, 40, 50], 1, 1, 5, 5);

		// Test Index trait
		assert_eq!(page[0], 10);
		assert_eq!(page[2], 30);
		assert_eq!(page[4], 50);

		// Test get method
		assert_eq!(page.get(1), Some(&20));
		assert_eq!(page.get(10), None);

		// Test get_slice method
		assert_eq!(page.get_slice(1..3), Some(&[20, 30][..]));
		assert_eq!(page.get_slice(10..20), None);
	}

	#[test]
	fn test_page_empty() {
		let empty_page: Page<i32> = Page::new(vec![], 1, 0, 0, 10);
		assert!(empty_page.is_empty());
		assert_eq!(empty_page.len(), 0);

		let non_empty_page = Page::new(vec![1], 1, 1, 1, 10);
		assert!(!non_empty_page.is_empty());
		assert_eq!(non_empty_page.len(), 1);
	}

	// ========================================
	// Orphans and allow_empty_first_page Tests
	// ========================================

	#[test]
	fn test_orphans_merge_last_page() {
		let items: Vec<i32> = (1..=11).collect(); // 11 items
		let paginator = PageNumberPagination::new().page_size(10).orphans(2); // Merge if last page has <= 2 items

		// With orphans=2, last page with 1 item should merge with page 1
		let page1 = paginator
			.paginate(&items, Some("1"), "http://api.example.com/items")
			.unwrap();
		assert_eq!(page1.results.len(), 11); // All items on one page
		assert_eq!(page1.count, 11);
		assert!(page1.next.is_none()); // No next page

		// Trying to access page 2 should fail
		let result = paginator.paginate(&items, Some("2"), "http://api.example.com/items");
		assert!(result.is_err());
	}

	#[test]
	fn test_orphans_no_merge() {
		let items: Vec<i32> = (1..=13).collect(); // 13 items
		let paginator = PageNumberPagination::new().page_size(10).orphans(2); // Merge if last page has <= 2 items

		// With orphans=2, last page with 3 items should NOT merge
		let page1 = paginator
			.paginate(&items, Some("1"), "http://api.example.com/items")
			.unwrap();
		assert_eq!(page1.results.len(), 10);
		assert!(page1.next.is_some());

		let page2 = paginator
			.paginate(&items, Some("2"), "http://api.example.com/items")
			.unwrap();
		assert_eq!(page2.results.len(), 3);
		assert!(page2.next.is_none());
	}

	#[test]
	fn test_allow_empty_first_page_true() {
		let items: Vec<i32> = vec![];
		let paginator = PageNumberPagination::new()
			.page_size(10)
			.allow_empty_first_page(true);

		let page = paginator
			.paginate(&items, Some("1"), "http://api.example.com/items")
			.unwrap();
		assert_eq!(page.results.len(), 0);
		assert_eq!(page.count, 0);
	}

	#[test]
	fn test_allow_empty_first_page_false() {
		let items: Vec<i32> = vec![];
		let paginator = PageNumberPagination::new()
			.page_size(10)
			.allow_empty_first_page(false);

		let result = paginator.paginate(&items, Some("1"), "http://api.example.com/items");
		assert!(result.is_err());
		assert!(matches!(
			result.unwrap_err(),
			crate::exception::Error::InvalidPage(_)
		));
	}

	#[test]
	fn test_orphans_with_various_counts() {
		// Test case from Django: 10 items, page_size=4, orphans=2
		let items: Vec<i32> = (1..=10).collect();

		// orphans=0: should have 3 pages (4+4+2)
		let paginator0 = PageNumberPagination::new().page_size(4).orphans(0);
		let page1 = paginator0
			.paginate(&items, Some("1"), "http://api.example.com/items")
			.unwrap();
		assert!(page1.next.is_some());

		// orphans=1: should have 3 pages (4+4+2, because 2 > 1)
		let paginator1 = PageNumberPagination::new().page_size(4).orphans(1);
		let page1 = paginator1
			.paginate(&items, Some("1"), "http://api.example.com/items")
			.unwrap();
		assert!(page1.next.is_some());

		// orphans=2: should have 2 pages (4+6, because 2 <= 2)
		let paginator2 = PageNumberPagination::new().page_size(4).orphans(2);
		let page1 = paginator2
			.paginate(&items, Some("1"), "http://api.example.com/items")
			.unwrap();
		assert_eq!(page1.results.len(), 4);
		let page2 = paginator2
			.paginate(&items, Some("2"), "http://api.example.com/items")
			.unwrap();
		assert_eq!(page2.results.len(), 6); // 4 + 2 orphans
		assert!(page2.next.is_none());
	}

	#[test]
	fn test_error_messages_custom() {
		let items: Vec<i32> = (1..=3).collect();
		let custom_messages = ErrorMessages {
			invalid_page: "Wrong page number".to_string(),
			min_page: "Too small".to_string(),
			no_results: "There is nothing here".to_string(),
		};
		let paginator = PageNumberPagination::new()
			.page_size(2)
			.error_messages(custom_messages);

		// Test invalid page (non-numeric)
		let result = paginator.paginate(&items, Some("abc"), "http://api.example.com/items");
		assert!(result.is_err());
		if let Err(crate::exception::Error::InvalidPage(msg)) = result {
			assert_eq!(msg, "Wrong page number");
		} else {
			panic!("Expected InvalidPage error");
		}

		// Test min page (page 0)
		let result = paginator.paginate(&items, Some("0"), "http://api.example.com/items");
		assert!(result.is_err());
		if let Err(crate::exception::Error::InvalidPage(msg)) = result {
			assert_eq!(msg, "Too small");
		} else {
			panic!("Expected InvalidPage error");
		}

		// Test no results (page beyond range)
		let result = paginator.paginate(&items, Some("10"), "http://api.example.com/items");
		assert!(result.is_err());
		if let Err(crate::exception::Error::InvalidPage(msg)) = result {
			assert_eq!(msg, "There is nothing here");
		} else {
			panic!("Expected InvalidPage error");
		}
	}

	#[test]
	fn test_error_messages_default() {
		let items: Vec<i32> = (1..=3).collect();
		let paginator = PageNumberPagination::new().page_size(2);

		// Test default error message for out of range
		let result = paginator.paginate(&items, Some("10"), "http://api.example.com/items");
		assert!(result.is_err());
		if let Err(crate::exception::Error::InvalidPage(msg)) = result {
			assert_eq!(msg, "That page contains no results");
		} else {
			panic!("Expected InvalidPage error");
		}

		// Test default error message for page 0
		let result = paginator.paginate(&items, Some("0"), "http://api.example.com/items");
		assert!(result.is_err());
		if let Err(crate::exception::Error::InvalidPage(msg)) = result {
			assert_eq!(msg, "That page number is less than 1");
		} else {
			panic!("Expected InvalidPage error");
		}
	}

	#[test]
	fn test_error_messages_partial_custom() {
		let items: Vec<i32> = (1..=3).collect();
		let custom_messages = ErrorMessages {
			min_page: "Too small".to_string(),
			..ErrorMessages::default()
		};
		let paginator = PageNumberPagination::new()
			.page_size(2)
			.error_messages(custom_messages);

		// Custom message for min_page
		let result = paginator.paginate(&items, Some("0"), "http://api.example.com/items");
		assert!(result.is_err());
		if let Err(crate::exception::Error::InvalidPage(msg)) = result {
			assert_eq!(msg, "Too small");
		} else {
			panic!("Expected InvalidPage error");
		}

		// Default message for no_results
		let result = paginator.paginate(&items, Some("10"), "http://api.example.com/items");
		assert!(result.is_err());
		if let Err(crate::exception::Error::InvalidPage(msg)) = result {
			assert_eq!(msg, "That page contains no results");
		} else {
			panic!("Expected InvalidPage error");
		}
	}

	#[test]
	fn test_get_page_valid() {
		let items: Vec<i32> = (1..=10).collect();
		let paginator = PageNumberPagination::new().page_size(3);

		let page1 = paginator.get_page(&items, Some("1"));
		assert_eq!(page1.number, 1);
		assert_eq!(page1.object_list, vec![1, 2, 3]);
		assert_eq!(page1.num_pages, 4);
		assert_eq!(page1.count, 10);

		let page2 = paginator.get_page(&items, Some("2"));
		assert_eq!(page2.number, 2);
		assert_eq!(page2.object_list, vec![4, 5, 6]);
	}

	#[test]
	fn test_get_page_out_of_range_returns_last_page() {
		let items: Vec<i32> = (1..=10).collect();
		let paginator = PageNumberPagination::new().page_size(3);

		// Out of range returns last page (page 4)
		let page = paginator.get_page(&items, Some("100"));
		assert_eq!(page.number, 4);
		assert_eq!(page.object_list, vec![10]);
	}

	#[test]
	fn test_get_page_invalid_returns_first_page() {
		let items: Vec<i32> = (1..=10).collect();
		let paginator = PageNumberPagination::new().page_size(3);

		// Invalid page number returns first page
		let page = paginator.get_page(&items, Some("abc"));
		assert_eq!(page.number, 1);
		assert_eq!(page.object_list, vec![1, 2, 3]);

		// None returns first page
		let page = paginator.get_page(&items, None);
		assert_eq!(page.number, 1);
		assert_eq!(page.object_list, vec![1, 2, 3]);
	}

	#[test]
	fn test_get_page_empty_list() {
		let items: Vec<i32> = vec![];
		let paginator = PageNumberPagination::new().page_size(3);

		let page = paginator.get_page(&items, Some("1"));
		assert_eq!(page.number, 1);
		assert_eq!(page.object_list, Vec::<i32>::new());
		assert_eq!(page.num_pages, 1);
		assert_eq!(page.count, 0);
	}

	#[test]
	fn test_get_page_with_orphans() {
		let items: Vec<i32> = (1..=11).collect();
		let paginator = PageNumberPagination::new().page_size(10).orphans(2);

		// With orphans=2, should have only 1 page
		let page = paginator.get_page(&items, Some("1"));
		assert_eq!(page.number, 1);
		assert_eq!(page.object_list.len(), 11);
		assert_eq!(page.num_pages, 1);

		// Requesting page 2 should return page 1 (last available page)
		let page = paginator.get_page(&items, Some("2"));
		assert_eq!(page.number, 1);
		assert_eq!(page.object_list.len(), 11);
	}

	#[test]
	fn test_float_integer_page() {
		let items: Vec<i32> = (1..=10).collect();
		let paginator = PageNumberPagination::new().page_size(3);

		// Float that represents an integer should work
		let page = paginator.paginate(&items, Some("1.0"), "http://api.example.com/items");
		let page = page.unwrap();
		assert_eq!(page.results, vec![1, 2, 3]);

		let page = paginator.paginate(&items, Some("2.0"), "http://api.example.com/items");
		let page = page.unwrap();
		assert_eq!(page.results, vec![4, 5, 6]);
	}

	#[test]
	fn test_float_non_integer_page_fails() {
		let items: Vec<i32> = (1..=10).collect();
		let paginator = PageNumberPagination::new().page_size(3);

		// Float with fractional part should fail
		let result = paginator.paginate(&items, Some("1.5"), "http://api.example.com/items");
		assert!(result.is_err());

		let result = paginator.paginate(&items, Some("2.3"), "http://api.example.com/items");
		assert!(result.is_err());
	}

	#[test]
	fn test_get_page_with_float() {
		let items: Vec<i32> = (1..=10).collect();
		let paginator = PageNumberPagination::new().page_size(3);

		// get_page accepts float integers but returns first page on non-integer floats
		let page = paginator.get_page(&items, Some("1.0"));
		assert_eq!(page.number, 1);
		assert_eq!(page.object_list, vec![1, 2, 3]);

		// Non-integer floats return first page (graceful fallback)
		let page = paginator.get_page(&items, Some("1.5"));
		assert_eq!(page.number, 1);
		assert_eq!(page.object_list, vec![1, 2, 3]);
	}

	#[test]
	fn test_page_range() {
		let items: Vec<i32> = (1..=10).collect();
		let paginator = PageNumberPagination::new().page_size(3);

		let page = paginator.get_page(&items, Some("1"));
		let range: Vec<usize> = page.page_range().collect();
		assert_eq!(range, vec![1, 2, 3, 4]);

		// Works with any page
		let page = paginator.get_page(&items, Some("2"));
		let range: Vec<usize> = page.page_range().collect();
		assert_eq!(range, vec![1, 2, 3, 4]);
	}

	#[test]
	fn test_page_range_single_page() {
		let items: Vec<i32> = (1..=5).collect();
		let paginator = PageNumberPagination::new().page_size(10);

		let page = paginator.get_page(&items, Some("1"));
		let range: Vec<usize> = page.page_range().collect();
		assert_eq!(range, vec![1]);
	}

	#[test]
	fn test_page_range_empty() {
		let items: Vec<i32> = vec![];
		let paginator = PageNumberPagination::new().page_size(10);

		let page = paginator.get_page(&items, Some("1"));
		let range: Vec<usize> = page.page_range().collect();
		assert_eq!(range, vec![1]);
	}

	#[test]
	fn test_elided_page_range_not_elided() {
		// Test when range is not elided (10 pages or less with default settings)
		let items: Vec<i32> = (1..=100).collect();
		let paginator = PageNumberPagination::new().page_size(10);

		let page = paginator.get_page(&items, Some("1"));
		let range = page.get_elided_page_range(3, 2);
		let expected: Vec<Option<usize>> = (1..=10).map(Some).collect();
		assert_eq!(range, expected);
	}

	#[test]
	fn test_elided_page_range_with_ellipsis() {
		// Test Django's example: 50 pages, on_each_side=3, on_ends=2
		let items: Vec<i32> = (1..=5000).collect();
		let paginator = PageNumberPagination::new().page_size(100);

		// Page 1: [1, 2, 3, 4, ..., 49, 50]
		let page1 = paginator.get_page(&items, Some("1"));
		let range1 = page1.get_elided_page_range(3, 2);
		assert_eq!(
			range1,
			vec![
				Some(1),
				Some(2),
				Some(3),
				Some(4),
				None, // Ellipsis
				Some(49),
				Some(50)
			]
		);

		// Page 8: [1, 2, ..., 5, 6, 7, 8, 9, 10, 11, ..., 49, 50]
		let page8 = paginator.get_page(&items, Some("8"));
		let range8 = page8.get_elided_page_range(3, 2);
		assert_eq!(
			range8,
			vec![
				Some(1),
				Some(2),
				None, // Ellipsis
				Some(5),
				Some(6),
				Some(7),
				Some(8),
				Some(9),
				Some(10),
				Some(11),
				None, // Ellipsis
				Some(49),
				Some(50)
			]
		);

		// Page 50: [1, 2, ..., 47, 48, 49, 50]
		let page50 = paginator.get_page(&items, Some("50"));
		let range50 = page50.get_elided_page_range(3, 2);
		assert_eq!(
			range50,
			vec![
				Some(1),
				Some(2),
				None, // Ellipsis
				Some(47),
				Some(48),
				Some(49),
				Some(50)
			]
		);
	}

	#[test]
	fn test_elided_page_range_custom_params() {
		// Test with custom on_each_side and on_ends
		let items: Vec<i32> = (1..=3000).collect();
		let paginator = PageNumberPagination::new().page_size(100);

		let page15 = paginator.get_page(&items, Some("15"));

		// on_each_side=1, on_ends=1: [1, ..., 14, 15, 16, ..., 30]
		let range = page15.get_elided_page_range(1, 1);
		assert_eq!(
			range,
			vec![
				Some(1),
				None, // Ellipsis
				Some(14),
				Some(15),
				Some(16),
				None, // Ellipsis
				Some(30)
			]
		);
	}

	// ========================================
	// Advanced Cursor Pagination Tests
	// ========================================

	#[test]
	fn test_cursor_pagination_with_custom_encoder() {
		use crate::pagination::cursor::Base64CursorEncoder;

		let items: Vec<i32> = (1..=30).collect();
		let encoder = Base64CursorEncoder::new().expiry_seconds(3600);
		let paginator = CursorPagination::new().page_size(10).with_encoder(encoder);

		let page = paginator
			.paginate(&items, None, "http://api.example.com/items")
			.unwrap();

		assert_eq!(page.results.len(), 10);
		assert!(page.next.is_some());
	}

	#[test]
	fn test_cursor_pagination_bidirectional() {
		let items: Vec<i32> = (1..=30).collect();
		let paginator = CursorPagination::new().page_size(10).with_bidirectional();

		// First page - no previous
		let page1 = paginator
			.paginate(&items, None, "http://api.example.com/items")
			.unwrap();
		assert!(page1.next.is_some());
		assert!(page1.previous.is_none());

		// Navigate to second page
		let next_url = page1.next.unwrap();
		let url = url::Url::parse(&next_url).unwrap();
		let cursor = url
			.query_pairs()
			.find(|(key, _)| key == "cursor")
			.map(|(_, value)| value.to_string())
			.unwrap();

		let page2 = paginator
			.paginate(&items, Some(&cursor), &next_url)
			.unwrap();

		// Second page should have both next and previous
		assert!(page2.next.is_some());
		assert!(page2.previous.is_some());
	}

	#[test]
	fn test_relay_pagination_basic() {
		use crate::pagination::cursor::RelayPagination;

		let items: Vec<i32> = (1..=100).collect();
		let paginator = RelayPagination::new().default_page_size(10);

		let connection = paginator
			.paginate(&items, Some(10), None, None, None)
			.unwrap();

		assert_eq!(connection.edges.len(), 10);
		assert!(connection.page_info.has_next_page);
		assert!(!connection.page_info.has_previous_page);
		assert_eq!(connection.total_count, Some(100));
	}

	#[test]
	fn test_relay_pagination_with_after() {
		use crate::pagination::cursor::RelayPagination;

		let items: Vec<i32> = (1..=100).collect();
		let paginator = RelayPagination::new();

		// First page
		let page1 = paginator
			.paginate(&items, Some(10), None, None, None)
			.unwrap();
		let after_cursor = page1.page_info.end_cursor.unwrap();

		// Second page
		let page2 = paginator
			.paginate(&items, Some(10), Some(&after_cursor), None, None)
			.unwrap();

		assert_eq!(page2.edges[0].node, 11);
		assert!(page2.page_info.has_previous_page);
	}

	#[test]
	fn test_ordering_strategy_created_at() {
		use crate::pagination::cursor::{CreatedAtOrdering, OrderingStrategy};

		let ordering = CreatedAtOrdering::new();
		assert_eq!(ordering.fields(), vec!["-created_at", "id"]);

		let custom = CreatedAtOrdering::new().with_fields("created", "pk");
		assert_eq!(custom.fields(), vec!["-created", "pk"]);
	}

	#[test]
	fn test_ordering_strategy_id() {
		use crate::pagination::cursor::{IdOrdering, OrderingStrategy};

		let asc = IdOrdering::new();
		assert_eq!(asc.fields(), vec!["id"]);

		let desc = IdOrdering::descending();
		assert_eq!(desc.fields(), vec!["-id"]);

		let custom = IdOrdering::new().with_field("pk");
		assert_eq!(custom.fields(), vec!["pk"]);
	}

	// Tests for async operations are in a separate module
	// since they require tokio runtime
}

// Async tests in a separate module
#[cfg(test)]
mod async_tests {
	use super::*;

	// ========================================
	// Async Tests - PageNumberPagination
	// ========================================

	#[tokio::test]
	async fn test_page_number_pagination_apaginate() {
		let items: Vec<i32> = (1..=100).collect();
		let paginator = PageNumberPagination::new().page_size(10);

		// Async pagination
		let page1 = paginator
			.apaginate(&items, Some("1"), "http://api.example.com/items")
			.await
			.unwrap();

		assert_eq!(page1.results.len(), 10);
		assert_eq!(page1.results, (1..=10).collect::<Vec<i32>>());
		assert!(page1.next.is_some());
		assert!(page1.previous.is_none());
	}

	#[tokio::test]
	async fn test_page_number_pagination_aget_page() {
		let items: Vec<i32> = (1..=100).collect();
		let paginator = PageNumberPagination::new().page_size(10);

		// Async get_page
		let page = paginator.aget_page(&items, Some("5")).await;

		assert_eq!(page.number, 5);
		assert_eq!(page.object_list.len(), 10);
		assert_eq!(page.object_list, (41..=50).collect::<Vec<i32>>());
	}

	#[tokio::test]
	async fn test_page_number_pagination_aget_page_empty() {
		let items: Vec<i32> = vec![];
		let paginator = PageNumberPagination::new().page_size(10);

		let page = paginator.aget_page(&items, Some("1")).await;

		assert_eq!(page.number, 1);
		assert_eq!(page.object_list.len(), 0);
		assert_eq!(page.count, 0);
	}

	#[tokio::test]
	async fn test_page_number_pagination_aget_page_out_of_range() {
		let items: Vec<i32> = (1..=30).collect();
		let paginator = PageNumberPagination::new().page_size(10);

		// Out of range returns last page
		let page = paginator.aget_page(&items, Some("100")).await;

		assert_eq!(page.number, 3); // Last page
		assert_eq!(page.object_list.len(), 10);
	}

	#[tokio::test]
	async fn test_page_number_pagination_aget_page_invalid() {
		let items: Vec<i32> = (1..=30).collect();
		let paginator = PageNumberPagination::new().page_size(10);

		// Invalid returns first page
		let page = paginator.aget_page(&items, Some("abc")).await;

		assert_eq!(page.number, 1);
		assert_eq!(page.object_list, (1..=10).collect::<Vec<i32>>());
	}

	#[tokio::test]
	async fn test_page_number_pagination_async_with_orphans() {
		let items: Vec<i32> = (1..=11).collect();
		let paginator = PageNumberPagination::new().page_size(10).orphans(2);

		let page = paginator.aget_page(&items, Some("1")).await;

		assert_eq!(page.object_list.len(), 11); // All items on one page
		assert_eq!(page.num_pages, 1);
	}

	#[tokio::test]
	async fn test_page_number_pagination_async_with_custom_errors() {
		let items: Vec<i32> = (1..=30).collect();
		let custom_messages = ErrorMessages {
			invalid_page: "Custom invalid".to_string(),
			min_page: "Custom min".to_string(),
			no_results: "Custom no results".to_string(),
		};
		let paginator = PageNumberPagination::new()
			.page_size(10)
			.error_messages(custom_messages);

		let result = paginator
			.apaginate(&items, Some("100"), "http://api.example.com/items")
			.await;

		assert!(result.is_err());
		if let Err(crate::exception::Error::InvalidPage(msg)) = result {
			assert_eq!(msg, "Custom no results");
		}
	}

	#[tokio::test]
	async fn test_page_number_pagination_async_iteration() {
		let items: Vec<i32> = (1..=30).collect();
		let paginator = PageNumberPagination::new().page_size(10);

		let page = paginator.aget_page(&items, Some("2")).await;

		// Test iteration
		let collected: Vec<i32> = page.into_iter().collect();
		assert_eq!(collected, (11..=20).collect::<Vec<i32>>());
	}

	#[tokio::test]
	async fn test_page_number_pagination_async_elided_range() {
		let items: Vec<i32> = (1..=5000).collect();
		let paginator = PageNumberPagination::new().page_size(100);

		let page = paginator.aget_page(&items, Some("8")).await;
		let range = page.get_elided_page_range(3, 2);

		// Should have ellipsis
		assert!(range.contains(&None));
		assert!(range.contains(&Some(8)));
	}

	#[tokio::test]
	async fn test_page_number_pagination_async_float_page() {
		let items: Vec<i32> = (1..=30).collect();
		let paginator = PageNumberPagination::new().page_size(10);

		let result = paginator
			.apaginate(&items, Some("2.0"), "http://api.example.com/items")
			.await;

		let page = result.unwrap();
		assert_eq!(page.results, (11..=20).collect::<Vec<i32>>());
	}

	// ========================================
	// Async Tests - LimitOffsetPagination
	// ========================================

	#[tokio::test]
	async fn test_limit_offset_pagination_apaginate() {
		let items: Vec<i32> = (1..=100).collect();
		let paginator = LimitOffsetPagination::new().default_limit(10);

		let page = paginator
			.apaginate(&items, None, "http://api.example.com/items")
			.await
			.unwrap();

		assert_eq!(page.results.len(), 10);
		assert_eq!(page.results, (1..=10).collect::<Vec<i32>>());
	}

	#[tokio::test]
	async fn test_limit_offset_pagination_async_with_limit() {
		let items: Vec<i32> = (1..=100).collect();
		let paginator = LimitOffsetPagination::new().default_limit(10);

		let page = paginator
			.apaginate(&items, Some("limit=20"), "http://api.example.com/items")
			.await
			.unwrap();

		assert_eq!(page.results.len(), 20);
		assert_eq!(page.results, (1..=20).collect::<Vec<i32>>());
	}

	#[tokio::test]
	async fn test_limit_offset_pagination_async_with_offset() {
		let items: Vec<i32> = (1..=100).collect();
		let paginator = LimitOffsetPagination::new().default_limit(10);

		let page = paginator
			.apaginate(&items, Some("offset=20"), "http://api.example.com/items")
			.await
			.unwrap();

		assert_eq!(page.results.len(), 10);
		assert_eq!(page.results, (21..=30).collect::<Vec<i32>>());
	}

	#[tokio::test]
	async fn test_limit_offset_pagination_async_max_limit() {
		let items: Vec<i32> = (1..=100).collect();
		let paginator = LimitOffsetPagination::new().default_limit(10).max_limit(50);

		let result = paginator
			.apaginate(&items, Some("limit=100"), "http://api.example.com/items")
			.await;

		assert!(result.is_err());
		assert!(matches!(
			result.unwrap_err(),
			crate::exception::Error::InvalidLimit(_)
		));
	}

	#[tokio::test]
	async fn test_limit_offset_pagination_async_invalid_params() {
		let items: Vec<i32> = (1..=100).collect();
		let paginator = LimitOffsetPagination::new().default_limit(10);

		let result = paginator
			.apaginate(&items, Some("limit=abc"), "http://api.example.com/items")
			.await;

		assert!(result.is_err()); // Invalid params cause error
	}

	#[tokio::test]
	async fn test_limit_offset_pagination_async_edge_cases() {
		let items: Vec<i32> = (1..=100).collect();
		let paginator = LimitOffsetPagination::new().default_limit(10);

		// Offset beyond count
		let page = paginator
			.apaginate(&items, Some("offset=200"), "http://api.example.com/items")
			.await
			.unwrap();

		assert_eq!(page.results.len(), 0);
		assert!(page.next.is_none());
	}

	// ========================================
	// Async Tests - CursorPagination
	// ========================================

	#[tokio::test]
	async fn test_cursor_pagination_apaginate() {
		let items: Vec<i32> = (1..=30).collect();
		let paginator = CursorPagination::new().page_size(10);

		let page = paginator
			.apaginate(&items, None, "http://api.example.com/items")
			.await
			.unwrap();

		assert_eq!(page.results.len(), 10);
		assert_eq!(page.results, (1..=10).collect::<Vec<i32>>());
		assert!(page.next.is_some());
	}

	#[tokio::test]
	async fn test_cursor_pagination_async_navigation() {
		let items: Vec<i32> = (1..=30).collect();
		let paginator = CursorPagination::new().page_size(10);

		let page1 = paginator
			.apaginate(&items, None, "http://api.example.com/items")
			.await
			.unwrap();

		let next_url = page1.next.unwrap();
		let url = url::Url::parse(&next_url).unwrap();
		let cursor = url
			.query_pairs()
			.find(|(key, _)| key == "cursor")
			.map(|(_, value)| value.to_string())
			.unwrap();

		let page2 = paginator
			.apaginate(&items, Some(&cursor), &next_url)
			.await
			.unwrap();

		assert_eq!(page2.results.len(), 10);
		assert_eq!(page2.results, (11..=20).collect::<Vec<i32>>());
	}

	#[tokio::test]
	async fn test_cursor_pagination_async_with_page_size() {
		let items: Vec<i32> = (1..=30).collect();
		let paginator = CursorPagination::new().page_size(5);

		let page = paginator
			.apaginate(&items, None, "http://api.example.com/items?page_size=20")
			.await
			.unwrap();

		assert_eq!(page.results.len(), 20);
		assert_eq!(page.results, (1..=20).collect::<Vec<i32>>());
	}

	#[tokio::test]
	async fn test_cursor_pagination_async_invalid_cursor() {
		let items: Vec<i32> = (1..=30).collect();
		let paginator = CursorPagination::new().page_size(10);

		let result = paginator
			.apaginate(
				&items,
				Some("invalid_cursor"),
				"http://api.example.com/items",
			)
			.await;

		assert!(result.is_err());
	}

	#[tokio::test]
	async fn test_cursor_pagination_async_empty_list() {
		let items: Vec<i32> = vec![];
		let paginator = CursorPagination::new().page_size(10);

		let page = paginator
			.apaginate(&items, None, "http://api.example.com/items")
			.await
			.unwrap();

		assert_eq!(page.results.len(), 0);
		assert!(page.next.is_none());
	}

	#[tokio::test]
	async fn test_cursor_pagination_async_edge_cases() {
		let items: Vec<i32> = (1..=10).collect();
		let paginator = CursorPagination::new().page_size(10);

		// Exact page size
		let page = paginator
			.apaginate(&items, None, "http://api.example.com/items")
			.await
			.unwrap();

		assert_eq!(page.results.len(), 10);
		assert!(page.next.is_none()); // No more items
	}
}
