//! GraphQL Relay-style cursor pagination
//!
//! Implements the Relay Cursor Connections Specification:
//! <https://relay.dev/graphql/connections.htm>

use serde::{Deserialize, Serialize};

use super::CursorEncoder;
use crate::exception::Result;
use std::sync::Arc;

/// An edge in a Relay connection
///
/// Contains a node (the actual data item) and a cursor for pagination.
///
/// # Examples
///
/// ```
/// use reinhardt_core::pagination::cursor::Edge;
///
/// let edge = Edge {
///     node: "Item 1".to_string(),
///     cursor: "Y3Vyc29yMQ==".to_string(),
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Edge<T> {
	/// The actual data item
	pub node: T,
	/// Opaque cursor for this edge
	pub cursor: String,
}

/// Page information for Relay-style pagination
///
/// Provides metadata about the current page and available navigation.
///
/// # Examples
///
/// ```
/// use reinhardt_core::pagination::cursor::PageInfo;
///
/// let page_info = PageInfo {
///     has_next_page: true,
///     has_previous_page: false,
///     start_cursor: Some("start".to_string()),
///     end_cursor: Some("end".to_string()),
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PageInfo {
	/// Whether there are more items after this page
	pub has_next_page: bool,
	/// Whether there are more items before this page
	pub has_previous_page: bool,
	/// Cursor of the first edge (None if empty)
	pub start_cursor: Option<String>,
	/// Cursor of the last edge (None if empty)
	pub end_cursor: Option<String>,
}

/// A Relay-style connection
///
/// Contains edges (items with cursors) and page information.
///
/// # Examples
///
/// ```
/// use reinhardt_core::pagination::cursor::{Connection, Edge, PageInfo};
///
/// let connection = Connection {
///     edges: vec![
///         Edge { node: 1, cursor: "cursor1".to_string() },
///         Edge { node: 2, cursor: "cursor2".to_string() },
///     ],
///     page_info: PageInfo {
///         has_next_page: true,
///         has_previous_page: false,
///         start_cursor: Some("cursor1".to_string()),
///         end_cursor: Some("cursor2".to_string()),
///     },
///     total_count: Some(100),
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Connection<T> {
	/// The edges (items with cursors)
	pub edges: Vec<Edge<T>>,
	/// Information about the current page
	pub page_info: PageInfo,
	/// Total count of items (optional, may be expensive to compute)
	pub total_count: Option<usize>,
}

/// Relay-style cursor pagination
///
/// Implements the GraphQL Relay Cursor Connections Specification.
/// Provides `first`/`after` and `last`/`before` pagination parameters.
///
/// # Examples
///
/// ```
/// use reinhardt_core::pagination::cursor::RelayPagination;
///
/// let paginator = RelayPagination::new()
///     .default_page_size(10)
///     .max_page_size(100);
/// ```
#[derive(Clone)]
pub struct RelayPagination {
	/// Default page size (for `first` or `last`)
	pub default_page_size: usize,
	/// Maximum allowed page size
	pub max_page_size: Option<usize>,
	/// Include total count in response (may be expensive)
	pub include_total_count: bool,
	/// Cursor encoder
	encoder: Arc<dyn CursorEncoder>,
}

impl RelayPagination {
	/// Create a new RelayPagination with default settings
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::pagination::cursor::RelayPagination;
	///
	/// let paginator = RelayPagination::new();
	/// assert_eq!(paginator.default_page_size, 10);
	/// ```
	pub fn new() -> Self {
		Self {
			default_page_size: 10,
			max_page_size: Some(100),
			include_total_count: true,
			encoder: Arc::new(super::Base64CursorEncoder::new()),
		}
	}

	/// Set default page size
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::pagination::cursor::RelayPagination;
	///
	/// let paginator = RelayPagination::new().default_page_size(20);
	/// assert_eq!(paginator.default_page_size, 20);
	/// ```
	pub fn default_page_size(mut self, size: usize) -> Self {
		self.default_page_size = size;
		self
	}

	/// Set maximum page size
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::pagination::cursor::RelayPagination;
	///
	/// let paginator = RelayPagination::new().max_page_size(50);
	/// assert_eq!(paginator.max_page_size, Some(50));
	/// ```
	pub fn max_page_size(mut self, size: usize) -> Self {
		self.max_page_size = Some(size);
		self
	}

	/// Set whether to include total count
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::pagination::cursor::RelayPagination;
	///
	/// let paginator = RelayPagination::new().include_total_count(false);
	/// assert!(!paginator.include_total_count);
	/// ```
	pub fn include_total_count(mut self, include: bool) -> Self {
		self.include_total_count = include;
		self
	}

	/// Set custom cursor encoder
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::pagination::cursor::{RelayPagination, Base64CursorEncoder};
	///
	/// let encoder = Base64CursorEncoder::new().expiry_seconds(3600);
	/// let paginator = RelayPagination::new()
	///     .with_encoder(encoder);
	/// ```
	pub fn with_encoder<E: CursorEncoder + 'static>(mut self, encoder: E) -> Self {
		self.encoder = Arc::new(encoder);
		self
	}

	/// Paginate items into a Relay-style connection
	///
	/// # Arguments
	///
	/// * `items` - The items to paginate
	/// * `first` - Number of items to return from the start (forward pagination)
	/// * `after` - Cursor to start from (forward pagination)
	/// * `last` - Number of items to return from the end (backward pagination)
	/// * `before` - Cursor to end at (backward pagination)
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::pagination::cursor::RelayPagination;
	///
	/// let items: Vec<i32> = (1..=100).collect();
	/// let paginator = RelayPagination::new();
	///
	/// // Forward pagination: first 10 items
	/// let connection = paginator.paginate(&items, Some(10), None, None, None).unwrap();
	/// assert_eq!(connection.edges.len(), 10);
	/// assert!(connection.page_info.has_next_page);
	/// ```
	pub fn paginate<T: Clone + Send + Sync>(
		&self,
		items: &[T],
		first: Option<usize>,
		after: Option<&str>,
		last: Option<usize>,
		before: Option<&str>,
	) -> Result<Connection<T>> {
		let total_count = items.len();

		// Determine pagination direction and size
		let (page_size, is_forward) = if let Some(f) = first {
			let size = if let Some(max) = self.max_page_size {
				std::cmp::min(f, max)
			} else {
				f
			};
			(size, true)
		} else if let Some(l) = last {
			let size = if let Some(max) = self.max_page_size {
				std::cmp::min(l, max)
			} else {
				l
			};
			(size, false)
		} else {
			(self.default_page_size, true)
		};

		// Determine start position
		let start = if let Some(after_cursor) = after {
			self.encoder.decode(after_cursor)? + 1
		} else if let Some(before_cursor) = before {
			let before_pos = self.encoder.decode(before_cursor)?;
			before_pos.saturating_sub(page_size)
		} else if is_forward {
			0
		} else {
			// Backward pagination from end
			total_count.saturating_sub(page_size)
		};

		// Clamp start position to data length to prevent out-of-range panic
		let start = std::cmp::min(start, total_count);

		// Calculate slice bounds
		let end = std::cmp::min(start + page_size, total_count);

		// Get items
		let slice = &items[start..end];

		// Create edges with cursors
		let edges: Result<Vec<Edge<T>>> = slice
			.iter()
			.enumerate()
			.map(|(i, item)| {
				let position = start + i;
				let cursor = self.encoder.encode(position)?;
				Ok(Edge {
					node: item.clone(),
					cursor,
				})
			})
			.collect();
		let edges = edges?;

		// Determine page info
		let has_previous_page = start > 0;
		let has_next_page = end < total_count;
		let start_cursor = edges.first().map(|e| e.cursor.clone());
		let end_cursor = edges.last().map(|e| e.cursor.clone());

		let page_info = PageInfo {
			has_next_page,
			has_previous_page,
			start_cursor,
			end_cursor,
		};

		Ok(Connection {
			edges,
			page_info,
			total_count: if self.include_total_count {
				Some(total_count)
			} else {
				None
			},
		})
	}
}

impl Default for RelayPagination {
	fn default() -> Self {
		Self::new()
	}
}

impl std::fmt::Debug for RelayPagination {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("RelayPagination")
			.field("default_page_size", &self.default_page_size)
			.field("max_page_size", &self.max_page_size)
			.field("include_total_count", &self.include_total_count)
			.finish()
	}
}

#[cfg(test)]
mod tests {
	use rstest::rstest;

	use super::*;

	#[rstest]
	fn test_relay_pagination_forward() {
		let items: Vec<i32> = (1..=100).collect();
		let paginator = RelayPagination::new().default_page_size(10);

		let connection = paginator
			.paginate(&items, Some(10), None, None, None)
			.unwrap();

		assert_eq!(connection.edges.len(), 10);
		assert_eq!(connection.edges[0].node, 1);
		assert_eq!(connection.edges[9].node, 10);
		assert!(connection.page_info.has_next_page);
		assert!(!connection.page_info.has_previous_page);
		assert_eq!(connection.total_count, Some(100));
	}

	#[rstest]
	fn test_relay_pagination_forward_with_after() {
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

		assert_eq!(page2.edges.len(), 10);
		assert_eq!(page2.edges[0].node, 11);
		assert_eq!(page2.edges[9].node, 20);
		assert!(page2.page_info.has_previous_page);
		assert!(page2.page_info.has_next_page);
	}

	#[rstest]
	fn test_relay_pagination_backward() {
		let items: Vec<i32> = (1..=100).collect();
		let paginator = RelayPagination::new();

		let connection = paginator
			.paginate(&items, None, None, Some(10), None)
			.unwrap();

		assert_eq!(connection.edges.len(), 10);
		assert_eq!(connection.edges[0].node, 91);
		assert_eq!(connection.edges[9].node, 100);
		assert!(!connection.page_info.has_next_page);
		assert!(connection.page_info.has_previous_page);
	}

	#[rstest]
	fn test_relay_pagination_edge_structure() {
		let items = vec!["a", "b", "c"];
		let paginator = RelayPagination::new();

		let connection = paginator
			.paginate(&items, Some(2), None, None, None)
			.unwrap();

		assert_eq!(connection.edges.len(), 2);
		assert_eq!(connection.edges[0].node, "a");
		assert!(!connection.edges[0].cursor.is_empty());
		assert_eq!(connection.edges[1].node, "b");
		assert!(!connection.edges[1].cursor.is_empty());
	}

	#[rstest]
	fn test_relay_pagination_page_info() {
		let items: Vec<i32> = (1..=5).collect();
		let paginator = RelayPagination::new();

		let connection = paginator
			.paginate(&items, Some(3), None, None, None)
			.unwrap();

		assert!(connection.page_info.start_cursor.is_some());
		assert!(connection.page_info.end_cursor.is_some());
		assert!(connection.page_info.has_next_page);
		assert!(!connection.page_info.has_previous_page);
	}

	#[rstest]
	fn test_relay_pagination_max_page_size() {
		let items: Vec<i32> = (1..=100).collect();
		let paginator = RelayPagination::new().max_page_size(20);

		// Request 50, but limited to 20
		let connection = paginator
			.paginate(&items, Some(50), None, None, None)
			.unwrap();

		assert_eq!(connection.edges.len(), 20);
	}

	#[rstest]
	fn test_relay_pagination_without_total_count() {
		let items: Vec<i32> = (1..=100).collect();
		let paginator = RelayPagination::new().include_total_count(false);

		let connection = paginator
			.paginate(&items, Some(10), None, None, None)
			.unwrap();

		assert_eq!(connection.total_count, None);
	}

	#[rstest]
	fn test_relay_pagination_empty_list() {
		let items: Vec<i32> = vec![];
		let paginator = RelayPagination::new();

		let connection = paginator
			.paginate(&items, Some(10), None, None, None)
			.unwrap();

		assert_eq!(connection.edges.len(), 0);
		assert!(!connection.page_info.has_next_page);
		assert!(!connection.page_info.has_previous_page);
		assert!(connection.page_info.start_cursor.is_none());
		assert!(connection.page_info.end_cursor.is_none());
	}

	#[rstest]
	fn relay_pagination_does_not_panic_on_out_of_range_after_cursor() {
		// Arrange - use the same encoder instance for both cursor generation and paginator
		let encoder = crate::pagination::cursor::Base64CursorEncoder::with_secret_key(
			b"test-secret-key-for-unit-tests!!",
		);
		let items: Vec<i32> = (1..=10).collect();
		// Encode a cursor pointing beyond data length
		let out_of_range_cursor = encoder.encode(100).unwrap();
		let paginator = RelayPagination::new().with_encoder(encoder);

		// Act
		let result = paginator.paginate(&items, Some(5), Some(&out_of_range_cursor), None, None);

		// Assert - should return empty edges, not panic
		assert!(result.is_ok());
		let connection = result.unwrap();
		assert!(connection.edges.is_empty());
		assert!(!connection.page_info.has_next_page);
	}

	#[rstest]
	fn relay_pagination_does_not_panic_on_out_of_range_before_cursor() {
		// Arrange - use the same encoder instance for both cursor generation and paginator
		let encoder = crate::pagination::cursor::Base64CursorEncoder::with_secret_key(
			b"test-secret-key-for-unit-tests!!",
		);
		let items: Vec<i32> = (1..=10).collect();
		// Encode a cursor pointing beyond data length
		let out_of_range_cursor = encoder.encode(100).unwrap();
		let paginator = RelayPagination::new().with_encoder(encoder);

		// Act
		let result = paginator.paginate(&items, None, None, Some(5), Some(&out_of_range_cursor));

		// Assert - should not panic
		assert!(result.is_ok());
	}

	#[rstest]
	fn relay_pagination_empty_dataset_with_after_cursor() {
		// Arrange - use the same encoder instance for both cursor generation and paginator
		let encoder = crate::pagination::cursor::Base64CursorEncoder::with_secret_key(
			b"test-secret-key-for-unit-tests!!",
		);
		let items: Vec<i32> = vec![];
		let cursor = encoder.encode(0).unwrap();
		let paginator = RelayPagination::new().with_encoder(encoder);

		// Act
		let result = paginator.paginate(&items, Some(10), Some(&cursor), None, None);

		// Assert
		assert!(result.is_ok());
		let connection = result.unwrap();
		assert!(connection.edges.is_empty());
	}
}
