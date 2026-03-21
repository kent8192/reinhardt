//! Database-integrated cursor pagination for O(k) performance
//!
//! This module provides cursor-based pagination that integrates directly with
//! database queries, avoiding the O(n) cost of OFFSET/LIMIT by using indexed
//! cursor fields (id, timestamp) for efficient seek operations.

use base64::{Engine, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};

/// Cursor structure for database pagination
///
/// Contains the primary key and a tie-breaker timestamp to ensure stable
/// ordering even when multiple records share the same primary key.
///
/// # Examples
///
/// ```
/// use reinhardt_core::pagination::cursor::database::Cursor;
///
/// let cursor = Cursor::new(42, 1234567890);
/// let encoded = cursor.encode();
/// let decoded = Cursor::decode(&encoded).unwrap();
/// assert_eq!(decoded.id, 42);
/// assert_eq!(decoded.timestamp, 1234567890);
/// ```
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Cursor {
	/// Primary key (id) of the record
	pub id: i64,
	/// Timestamp for tie-breaking when IDs are equal
	pub timestamp: i64,
}

impl Cursor {
	/// Create a new cursor with the given id and timestamp
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::pagination::cursor::database::Cursor;
	///
	/// let cursor = Cursor::new(100, 1609459200);
	/// assert_eq!(cursor.id, 100);
	/// assert_eq!(cursor.timestamp, 1609459200);
	/// ```
	pub fn new(id: i64, timestamp: i64) -> Self {
		Self { id, timestamp }
	}

	/// Encode the cursor to a base64 string
	///
	/// The cursor is serialized to JSON and then base64-encoded to create
	/// an opaque token that users cannot easily manipulate.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::pagination::cursor::database::Cursor;
	///
	/// let cursor = Cursor::new(42, 1234567890);
	/// let encoded = cursor.encode();
	/// assert!(!encoded.is_empty());
	/// ```
	pub fn encode(&self) -> String {
		let json = serde_json::to_string(self).expect("Failed to serialize cursor");
		STANDARD.encode(json.as_bytes())
	}

	/// Decode a base64-encoded cursor string
	///
	/// # Errors
	///
	/// Returns a `PaginationError::InvalidCursor` if:
	/// - The string is not valid base64
	/// - The decoded data is not valid JSON
	/// - The JSON does not match the Cursor structure
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::pagination::cursor::database::Cursor;
	///
	/// let cursor = Cursor::new(42, 1234567890);
	/// let encoded = cursor.encode();
	/// let decoded = Cursor::decode(&encoded).unwrap();
	/// assert_eq!(decoded, cursor);
	/// ```
	pub fn decode(cursor: &str) -> Result<Self, PaginationError> {
		let bytes = STANDARD
			.decode(cursor)
			.map_err(|e| PaginationError::InvalidCursor(format!("Base64 decode error: {}", e)))?;

		serde_json::from_slice(&bytes)
			.map_err(|e| PaginationError::InvalidCursor(format!("JSON parse error: {}", e)))
	}
}

/// Direction for cursor-based pagination
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
	/// Move forward through the dataset (next page)
	Forward,
	/// Move backward through the dataset (previous page)
	Backward,
}

/// Pagination errors specific to cursor operations
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PaginationError {
	/// Invalid cursor format or decoding error
	InvalidCursor(String),
	/// Database query error
	DatabaseError(String),
}

impl std::fmt::Display for PaginationError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::InvalidCursor(msg) => write!(f, "Invalid cursor: {}", msg),
			Self::DatabaseError(msg) => write!(f, "Database error: {}", msg),
		}
	}
}

impl std::error::Error for PaginationError {}

/// Trait for models that have id and timestamp fields
///
/// This trait is required for cursor-based pagination to work efficiently.
/// It provides access to the primary key (id) and a timestamp field that
/// can be used as a tie-breaker for stable ordering.
///
/// # Examples
///
/// ```
/// use reinhardt_core::pagination::cursor::database::HasTimestamp;
///
/// #[derive(Clone)]
/// struct User {
///     id: i64,
///     created_at: i64,
///     name: String,
/// }
///
/// impl HasTimestamp for User {
///     fn id(&self) -> i64 {
///         self.id
///     }
///
///     fn timestamp(&self) -> i64 {
///         self.created_at
///     }
/// }
/// ```
pub trait HasTimestamp {
	/// Returns the primary key (id) of the record
	fn id(&self) -> i64;

	/// Returns the timestamp field value
	///
	/// This is used as a tie-breaker when multiple records have the same id,
	/// ensuring stable pagination order.
	fn timestamp(&self) -> i64;
}

/// Cursor-based paginator for efficient database pagination
///
/// This paginator uses id/timestamp-based cursors to achieve O(k) performance
/// instead of the O(n) cost of OFFSET/LIMIT pagination.
///
/// # Examples
///
/// ```
/// use reinhardt_core::pagination::cursor::database::{CursorPaginator, HasTimestamp};
///
/// #[derive(Clone)]
/// struct User {
///     id: i64,
///     created_at: i64,
///     name: String,
/// }
///
/// impl HasTimestamp for User {
///     fn id(&self) -> i64 { self.id }
///     fn timestamp(&self) -> i64 { self.created_at }
/// }
///
/// let users = vec![
///     User { id: 1, created_at: 1000, name: "Alice".to_string() },
///     User { id: 2, created_at: 2000, name: "Bob".to_string() },
/// ];
///
/// let paginator = CursorPaginator::new(10);
/// // let page = paginator.paginate(users, None);
/// ```
pub struct CursorPaginator {
	page_size: usize,
}

impl CursorPaginator {
	/// Create a new cursor paginator with the given page size
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::pagination::cursor::database::CursorPaginator;
	///
	/// let paginator = CursorPaginator::new(20);
	/// ```
	pub fn new(page_size: usize) -> Self {
		Self { page_size }
	}

	/// Paginate a collection of items using cursor-based pagination
	///
	/// # Arguments
	///
	/// * `items` - The collection to paginate (must implement HasTimestamp)
	/// * `cursor` - Optional cursor string from previous page
	///
	/// # Returns
	///
	/// A `CursorPaginatedResponse` containing:
	/// - `results`: The items for this page
	/// - `next_cursor`: Cursor for next page (if available)
	/// - `prev_cursor`: Cursor for previous page (if available)
	/// - `has_next`: Whether there is a next page
	/// - `has_prev`: Whether there is a previous page
	///
	/// # Performance
	///
	/// For in-memory slices, this is O(n) for finding the cursor position.
	/// For database queries with proper indexes, this becomes O(k) where k is page_size.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::pagination::cursor::database::{CursorPaginator, HasTimestamp};
	///
	/// #[derive(Clone)]
	/// struct Item {
	///     id: i64,
	///     timestamp: i64,
	/// }
	///
	/// impl HasTimestamp for Item {
	///     fn id(&self) -> i64 { self.id }
	///     fn timestamp(&self) -> i64 { self.timestamp }
	/// }
	///
	/// let items = vec![
	///     Item { id: 1, timestamp: 100 },
	///     Item { id: 2, timestamp: 200 },
	///     Item { id: 3, timestamp: 300 },
	/// ];
	///
	/// let paginator = CursorPaginator::new(2);
	/// let page1 = paginator.paginate(&items, None).unwrap();
	///
	/// assert_eq!(page1.results.len(), 2);
	/// assert!(page1.has_next);
	/// assert!(!page1.has_prev);
	/// ```
	pub fn paginate<T>(
		&self,
		items: &[T],
		cursor: Option<String>,
	) -> Result<CursorPaginatedResponse<T>, PaginationError>
	where
		T: HasTimestamp + Clone,
	{
		// Decode cursor if provided
		let start_pos = if let Some(cursor_str) = cursor {
			let cursor = Cursor::decode(&cursor_str)?;
			// Find the position after the cursor
			items
				.iter()
				.position(|item| {
					item.id() > cursor.id
						|| (item.id() == cursor.id && item.timestamp() > cursor.timestamp)
				})
				.unwrap_or(items.len())
		} else {
			0
		};

		// Get page_size + 1 items to check if there's a next page
		let end_pos = std::cmp::min(start_pos + self.page_size + 1, items.len());
		let page_items = &items[start_pos..end_pos];

		let has_next = page_items.len() > self.page_size;
		let results: Vec<T> = page_items.iter().take(self.page_size).cloned().collect();

		// Generate next cursor
		let next_cursor = if has_next && !results.is_empty() {
			let last = results.last().unwrap();
			Some(Cursor::new(last.id(), last.timestamp()).encode())
		} else {
			None
		};

		// Generate previous cursor
		let prev_cursor = if start_pos > 0 && !results.is_empty() {
			let first = results.first().unwrap();
			Some(Cursor::new(first.id(), first.timestamp()).encode())
		} else {
			None
		};

		Ok(CursorPaginatedResponse {
			results,
			next_cursor,
			prev_cursor,
			has_next,
			has_prev: start_pos > 0,
		})
	}
}

/// Paginated response with cursor navigation
///
/// Contains the results and metadata for navigating through pages using cursors.
///
/// # Examples
///
/// ```
/// use reinhardt_core::pagination::cursor::database::CursorPaginatedResponse;
///
/// let response: CursorPaginatedResponse<i32> = CursorPaginatedResponse {
///     results: vec![1, 2, 3],
///     next_cursor: Some("encoded_cursor".to_string()),
///     prev_cursor: None,
///     has_next: true,
///     has_prev: false,
/// };
///
/// assert_eq!(response.results.len(), 3);
/// assert!(response.has_next);
/// assert!(!response.has_prev);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CursorPaginatedResponse<T> {
	/// The items on this page
	pub results: Vec<T>,
	/// Cursor for the next page (if available)
	pub next_cursor: Option<String>,
	/// Cursor for the previous page (if available)
	pub prev_cursor: Option<String>,
	/// Whether there is a next page
	pub has_next: bool,
	/// Whether there is a previous page
	pub has_prev: bool,
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_cursor_new() {
		let cursor = Cursor::new(42, 1234567890);
		assert_eq!(cursor.id, 42);
		assert_eq!(cursor.timestamp, 1234567890);
	}

	#[test]
	fn test_cursor_encode_decode() {
		let cursor = Cursor::new(100, 9876543210);
		let encoded = cursor.encode();

		// Encoded cursor should be non-empty and base64
		assert!(!encoded.is_empty());

		// Should decode back to original
		let decoded = Cursor::decode(&encoded).unwrap();
		assert_eq!(decoded, cursor);
	}

	#[test]
	fn test_cursor_decode_invalid_base64() {
		let result = Cursor::decode("not-valid-base64!!!");
		assert!(result.is_err());
		match result {
			Err(PaginationError::InvalidCursor(msg)) => {
				assert!(
					msg.starts_with("Base64 decode error:"),
					"Cursor decode should return base64 error message. Got: {}",
					msg
				);
			}
			_ => panic!("Expected InvalidCursor error"),
		}
	}

	#[test]
	fn test_cursor_decode_invalid_json() {
		// Valid base64 but invalid JSON
		let invalid = STANDARD.encode(b"not json");
		let result = Cursor::decode(&invalid);
		assert!(result.is_err());
		match result {
			Err(PaginationError::InvalidCursor(msg)) => {
				assert!(
					msg.starts_with("JSON parse error:"),
					"Cursor decode should return JSON parse error message. Got: {}",
					msg
				);
			}
			_ => panic!("Expected InvalidCursor error"),
		}
	}

	#[test]
	fn test_cursor_decode_malformed_json() {
		// Valid JSON but wrong structure
		let invalid_json = serde_json::json!({"wrong": "structure"});
		let invalid = STANDARD.encode(invalid_json.to_string().as_bytes());
		let result = Cursor::decode(&invalid);
		assert!(result.is_err());
	}

	#[test]
	fn test_cursor_roundtrip_edge_cases() {
		// Test with edge case values
		let test_cases = vec![
			Cursor::new(0, 0),
			Cursor::new(i64::MAX, i64::MAX),
			Cursor::new(i64::MIN, i64::MIN),
			Cursor::new(-1, -1),
		];

		for cursor in test_cases {
			let encoded = cursor.encode();
			let decoded = Cursor::decode(&encoded).unwrap();
			assert_eq!(decoded, cursor);
		}
	}

	#[test]
	fn test_direction() {
		assert_eq!(Direction::Forward, Direction::Forward);
		assert_eq!(Direction::Backward, Direction::Backward);
		assert_ne!(Direction::Forward, Direction::Backward);
	}

	#[test]
	fn test_pagination_error_display() {
		let err1 = PaginationError::InvalidCursor("bad cursor".to_string());
		assert_eq!(format!("{}", err1), "Invalid cursor: bad cursor");

		let err2 = PaginationError::DatabaseError("connection failed".to_string());
		assert_eq!(format!("{}", err2), "Database error: connection failed");
	}

	#[test]
	fn test_cursor_opaque() {
		// Ensure cursor is not easily human-readable
		let cursor = Cursor::new(42, 1234567890);
		let encoded = cursor.encode();

		// Should not contain raw values
		assert!(!encoded.contains("42"));
		assert!(!encoded.contains("1234567890"));
	}

	#[test]
	fn test_cursor_paginated_response() {
		let response: CursorPaginatedResponse<i32> = CursorPaginatedResponse {
			results: vec![1, 2, 3, 4, 5],
			next_cursor: Some("next".to_string()),
			prev_cursor: Some("prev".to_string()),
			has_next: true,
			has_prev: true,
		};

		assert_eq!(response.results.len(), 5);
		assert!(response.next_cursor.is_some());
		assert!(response.prev_cursor.is_some());
		assert!(response.has_next);
		assert!(response.has_prev);
	}

	// ========================================
	// HasTimestamp and CursorPaginator Tests
	// ========================================

	#[derive(Debug, Clone, PartialEq)]
	struct TestItem {
		id: i64,
		timestamp: i64,
		name: String,
	}

	impl HasTimestamp for TestItem {
		fn id(&self) -> i64 {
			self.id
		}

		fn timestamp(&self) -> i64 {
			self.timestamp
		}
	}

	fn create_test_items(count: usize) -> Vec<TestItem> {
		(1..=count)
			.map(|i| TestItem {
				id: i as i64,
				timestamp: (i as i64) * 1000,
				name: format!("Item {}", i),
			})
			.collect()
	}

	#[test]
	fn test_cursor_paginator_first_page() {
		let items = create_test_items(25);
		let paginator = CursorPaginator::new(10);

		let page = paginator.paginate(&items, None).unwrap();

		assert_eq!(page.results.len(), 10);
		assert_eq!(page.results[0].id, 1);
		assert_eq!(page.results[9].id, 10);
		assert!(page.has_next);
		assert!(!page.has_prev);
		assert!(page.next_cursor.is_some());
		assert!(page.prev_cursor.is_none());
	}

	#[test]
	fn test_cursor_paginator_navigation() {
		let items = create_test_items(25);
		let paginator = CursorPaginator::new(10);

		// First page
		let page1 = paginator.paginate(&items, None).unwrap();
		assert_eq!(page1.results.len(), 10);
		assert!(page1.has_next);
		assert!(!page1.has_prev);

		// Second page using cursor from page1
		let cursor = page1.next_cursor.unwrap();
		let page2 = paginator.paginate(&items, Some(cursor)).unwrap();
		assert_eq!(page2.results.len(), 10);
		assert_eq!(page2.results[0].id, 11);
		assert!(page2.has_next);
		assert!(page2.has_prev);

		// Third page
		let cursor = page2.next_cursor.unwrap();
		let page3 = paginator.paginate(&items, Some(cursor)).unwrap();
		assert_eq!(page3.results.len(), 5);
		assert_eq!(page3.results[0].id, 21);
		assert!(!page3.has_next);
		assert!(page3.has_prev);
	}

	#[test]
	fn test_cursor_paginator_empty_list() {
		let items: Vec<TestItem> = vec![];
		let paginator = CursorPaginator::new(10);

		let page = paginator.paginate(&items, None).unwrap();

		assert_eq!(page.results.len(), 0);
		assert!(!page.has_next);
		assert!(!page.has_prev);
		assert!(page.next_cursor.is_none());
		assert!(page.prev_cursor.is_none());
	}

	#[test]
	fn test_cursor_paginator_single_page() {
		let items = create_test_items(5);
		let paginator = CursorPaginator::new(10);

		let page = paginator.paginate(&items, None).unwrap();

		assert_eq!(page.results.len(), 5);
		assert!(!page.has_next);
		assert!(!page.has_prev);
		assert!(page.next_cursor.is_none());
	}

	#[test]
	fn test_cursor_paginator_exact_page_size() {
		let items = create_test_items(10);
		let paginator = CursorPaginator::new(10);

		let page = paginator.paginate(&items, None).unwrap();

		assert_eq!(page.results.len(), 10);
		assert!(!page.has_next);
		assert!(!page.has_prev);
	}

	#[test]
	fn test_cursor_paginator_one_more_than_page_size() {
		let items = create_test_items(11);
		let paginator = CursorPaginator::new(10);

		let page1 = paginator.paginate(&items, None).unwrap();
		assert_eq!(page1.results.len(), 10);
		assert!(page1.has_next);

		let cursor = page1.next_cursor.unwrap();
		let page2 = paginator.paginate(&items, Some(cursor)).unwrap();
		assert_eq!(page2.results.len(), 1);
		assert!(!page2.has_next);
	}

	#[test]
	fn test_cursor_paginator_invalid_cursor() {
		let items = create_test_items(25);
		let paginator = CursorPaginator::new(10);

		let result = paginator.paginate(&items, Some("invalid_cursor".to_string()));
		assert!(result.is_err());
	}

	#[test]
	fn test_cursor_paginator_tie_breaker() {
		// Test items with same id but different timestamps
		let items = vec![
			TestItem {
				id: 1,
				timestamp: 1000,
				name: "Item 1a".to_string(),
			},
			TestItem {
				id: 1,
				timestamp: 2000,
				name: "Item 1b".to_string(),
			},
			TestItem {
				id: 2,
				timestamp: 3000,
				name: "Item 2".to_string(),
			},
		];

		let paginator = CursorPaginator::new(1);

		// First page
		let page1 = paginator.paginate(&items, None).unwrap();
		assert_eq!(page1.results.len(), 1);
		assert_eq!(page1.results[0].timestamp, 1000);

		// Second page - should get item with same id but higher timestamp
		let cursor = page1.next_cursor.unwrap();
		let page2 = paginator.paginate(&items, Some(cursor)).unwrap();
		assert_eq!(page2.results.len(), 1);
		assert_eq!(page2.results[0].timestamp, 2000);
	}

	#[test]
	fn test_cursor_stability() {
		// Verify cursor is stable and reproducible
		let items = create_test_items(10);
		let paginator = CursorPaginator::new(5);

		let page1_a = paginator.paginate(&items, None).unwrap();
		let page1_b = paginator.paginate(&items, None).unwrap();

		// Same cursor should be generated for the same position
		assert_eq!(page1_a.next_cursor, page1_b.next_cursor);

		// Navigate with cursor
		let cursor = page1_a.next_cursor.unwrap();
		let page2_a = paginator.paginate(&items, Some(cursor.clone())).unwrap();
		let page2_b = paginator.paginate(&items, Some(cursor)).unwrap();

		assert_eq!(page2_a.results, page2_b.results);
	}

	#[test]
	fn test_cursor_paginator_performance_vs_offset() {
		// This test demonstrates the theoretical performance difference
		// In practice, database indexes make the difference more pronounced

		let items = create_test_items(10000);
		let paginator = CursorPaginator::new(100);

		// Navigate to "deep" page (page 50)
		let mut cursor: Option<String> = None;
		for _ in 0..49 {
			let page = paginator.paginate(&items, cursor).unwrap();
			cursor = page.next_cursor;
		}

		let page50 = paginator.paginate(&items, cursor).unwrap();
		assert_eq!(page50.results[0].id, 4901);

		// With cursor pagination, each page fetch is independent
		// With OFFSET/LIMIT, each page gets progressively slower
	}
}
