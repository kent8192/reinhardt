//! Cursor-based pagination implementation
//!
//! This module provides cursor-based pagination with support for:
//! - Custom cursor encoding strategies via [`encoder`]
//! - Bi-directional pagination
//! - Relay-style pagination via [`relay`]
//! - Custom ordering strategies via [`ordering`]
//! - Database-integrated cursor pagination via [`database`]

pub mod database;
pub mod encoder;
pub mod ordering;
pub mod relay;

use crate::exception::Result;
use async_trait::async_trait;

use super::core::{AsyncPaginator, PaginatedResponse, Paginator, SchemaParameter};
pub use database::{
	Cursor as DatabaseCursor, CursorPaginatedResponse, CursorPaginator, Direction, HasTimestamp,
	PaginationError,
};
pub use encoder::{Base64CursorEncoder, CursorEncoder};
pub use ordering::{CreatedAtOrdering, IdOrdering, OrderingStrategy};
pub use relay::{Connection, Edge, PageInfo, RelayPagination};

use std::sync::Arc;

/// Cursor-based pagination for large datasets
///
/// Provides consistent pagination even when items are added/removed.
/// Uses opaque cursor tokens instead of page numbers.
///
/// # Examples
///
/// ```
/// use reinhardt_core::pagination::CursorPagination;
///
/// let paginator = CursorPagination::new()
///     .page_size(20)
///     .max_page_size(100);
///
/// // Use with custom encoder
/// use reinhardt_core::pagination::cursor::Base64CursorEncoder;
/// let encoder = Base64CursorEncoder::new();
/// let paginator = CursorPagination::new()
///     .with_encoder(encoder);
/// ```
#[derive(Clone)]
pub struct CursorPagination {
	/// Default page size
	pub page_size: usize,
	/// Query parameter name for cursor
	pub cursor_query_param: String,
	/// Query parameter name for page size (optional)
	pub page_size_query_param: Option<String>,
	/// Ordering field(s) for cursor
	pub ordering: Vec<String>,
	/// Maximum allowed page size
	pub max_page_size: Option<usize>,
	/// Cursor encoder
	encoder: Arc<dyn CursorEncoder>,
	/// Enable bi-directional pagination
	bidirectional: bool,
}

impl Default for CursorPagination {
	fn default() -> Self {
		Self {
			page_size: 10,
			cursor_query_param: "cursor".to_string(),
			page_size_query_param: Some("page_size".to_string()),
			ordering: vec!["-created".to_string()],
			max_page_size: Some(100),
			encoder: Arc::new(Base64CursorEncoder::new()),
			bidirectional: false,
		}
	}
}

impl CursorPagination {
	/// Creates a new CursorPagination with default settings
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::pagination::CursorPagination;
	///
	/// let paginator = CursorPagination::new();
	/// assert_eq!(paginator.page_size, 10);
	/// assert_eq!(paginator.cursor_query_param, "cursor");
	/// assert_eq!(paginator.max_page_size, Some(100));
	/// ```
	pub fn new() -> Self {
		Self::default()
	}

	/// Sets the default page size for cursor pagination
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::pagination::CursorPagination;
	///
	/// let paginator = CursorPagination::new().page_size(20);
	/// assert_eq!(paginator.page_size, 20);
	/// ```
	pub fn page_size(mut self, size: usize) -> Self {
		self.page_size = size;
		self
	}

	/// Sets the maximum allowed page size
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::pagination::CursorPagination;
	///
	/// let paginator = CursorPagination::new()
	///     .page_size(10)
	///     .max_page_size(50);
	/// assert_eq!(paginator.max_page_size, Some(50));
	/// ```
	pub fn max_page_size(mut self, size: usize) -> Self {
		self.max_page_size = Some(size);
		self
	}

	/// Sets the ordering fields for cursor-based pagination
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::pagination::CursorPagination;
	///
	/// let paginator = CursorPagination::new()
	///     .ordering(vec!["-created_at".to_string(), "id".to_string()]);
	/// assert_eq!(paginator.ordering, vec!["-created_at", "id"]);
	/// ```
	pub fn ordering(mut self, fields: Vec<String>) -> Self {
		self.ordering = fields;
		self
	}

	/// Sets a custom cursor encoder
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::pagination::CursorPagination;
	/// use reinhardt_core::pagination::cursor::Base64CursorEncoder;
	///
	/// let encoder = Base64CursorEncoder::new().expiry_seconds(3600);
	/// let paginator = CursorPagination::new()
	///     .with_encoder(encoder);
	/// ```
	pub fn with_encoder<E: CursorEncoder + 'static>(mut self, encoder: E) -> Self {
		self.encoder = Arc::new(encoder);
		self
	}

	/// Enable bi-directional cursor pagination
	///
	/// When enabled, both previous and next cursors are provided for navigation.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::pagination::CursorPagination;
	///
	/// let paginator = CursorPagination::new()
	///     .with_bidirectional();
	/// ```
	pub fn with_bidirectional(mut self) -> Self {
		self.bidirectional = true;
		self
	}

	fn build_url(&self, base_url: &str, cursor: &str) -> String {
		let url = super::parse_base_url(base_url);

		let mut new_url = url.clone();
		new_url
			.query_pairs_mut()
			.clear()
			.append_pair(&self.cursor_query_param, cursor);

		// Copy other query parameters (including page_size)
		for (key, value) in url.query_pairs() {
			if key != self.cursor_query_param {
				new_url.query_pairs_mut().append_pair(&key, &value);
			}
		}

		new_url.to_string()
	}
}

impl std::fmt::Debug for CursorPagination {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("CursorPagination")
			.field("page_size", &self.page_size)
			.field("cursor_query_param", &self.cursor_query_param)
			.field("page_size_query_param", &self.page_size_query_param)
			.field("ordering", &self.ordering)
			.field("max_page_size", &self.max_page_size)
			.field("bidirectional", &self.bidirectional)
			.finish()
	}
}

#[async_trait]
impl Paginator for CursorPagination {
	fn paginate<T: Clone + Send + Sync>(
		&self,
		items: &[T],
		cursor_param: Option<&str>,
		base_url: &str,
	) -> Result<PaginatedResponse<T>> {
		let total_count = items.len();

		// Parse page_size from URL if page_size_query_param is set
		let page_size = if let Some(ref param_name) = self.page_size_query_param {
			if let Ok(url) = url::Url::parse(base_url) {
				url.query_pairs()
                    .find(|(key, _)| key == param_name)
                    .and_then(|(_, value)| value.parse::<usize>().ok())
                    .filter(|&size| size > 0) // Reject 0 or negative
                    .map(|size| {
                        // Clamp to max_page_size if set
                        if let Some(max) = self.max_page_size {
                            std::cmp::min(size, max)
                        } else {
                            size
                        }
                    })
                    .unwrap_or(self.page_size)
			} else {
				self.page_size
			}
		} else {
			self.page_size
		};

		// Get position from cursor
		let position = if let Some(cursor) = cursor_param {
			self.encoder.decode(cursor)?
		} else {
			0
		};

		// Clamp start position to data length to prevent out-of-range panic
		let start = std::cmp::min(position, total_count);
		let end = std::cmp::min(start + page_size, total_count);

		// Get results
		let results = items[start..end].to_vec();

		// Build next/previous cursors
		let next = if end < total_count {
			let next_cursor = self.encoder.encode(end)?;
			Some(self.build_url(base_url, &next_cursor))
		} else {
			None
		};

		let previous = if self.bidirectional && position > 0 {
			let prev_position = position.saturating_sub(page_size);
			let prev_cursor = self.encoder.encode(prev_position)?;
			Some(self.build_url(base_url, &prev_cursor))
		} else {
			None
		};

		Ok(PaginatedResponse {
			count: total_count,
			next,
			previous,
			results,
		})
	}

	fn get_schema_parameters(&self) -> Vec<SchemaParameter> {
		let mut params = vec![SchemaParameter {
			name: self.cursor_query_param.clone(),
			required: false,
			location: "query".to_string(),
			description: "The pagination cursor value.".to_string(),
			schema_type: "string".to_string(),
		}];

		if let Some(ref param_name) = self.page_size_query_param {
			params.push(SchemaParameter {
				name: param_name.clone(),
				required: false,
				location: "query".to_string(),
				description: "Number of results to return per page.".to_string(),
				schema_type: "integer".to_string(),
			});
		}

		params
	}
}

#[async_trait]
impl AsyncPaginator for CursorPagination {
	async fn apaginate<T: Clone + Send + Sync>(
		&self,
		items: &[T],
		cursor_param: Option<&str>,
		base_url: &str,
	) -> Result<PaginatedResponse<T>> {
		// For in-memory operations, just call the sync version
		self.paginate(items, cursor_param, base_url)
	}

	fn get_schema_parameters(&self) -> Vec<SchemaParameter> {
		Paginator::get_schema_parameters(self)
	}
}

#[cfg(test)]
mod tests {
	use rstest::rstest;

	use super::*;

	#[rstest]
	#[case("not a valid url at all \x00\x01")]
	#[case("://missing-scheme")]
	#[case("")]
	fn build_url_does_not_panic_with_malformed_base_url(#[case] malformed_url: &str) {
		// Arrange
		let paginator = CursorPagination::new();
		let items: Vec<i32> = (0..20).collect();

		// Act
		let result = paginator.paginate(&items, None, malformed_url);

		// Assert
		assert!(
			result.is_ok(),
			"paginate should not panic with malformed URL: {malformed_url:?}"
		);
	}

	#[rstest]
	fn paginate_returns_empty_results_for_out_of_range_cursor() {
		// Arrange
		let encoder =
			encoder::Base64CursorEncoder::with_secret_key(b"test-secret-key-for-unit-tests!!");
		let paginator = CursorPagination::new().with_encoder(
			encoder::Base64CursorEncoder::with_secret_key(b"test-secret-key-for-unit-tests!!"),
		);
		let items: Vec<i32> = (1..=10).collect();
		// Encode a cursor pointing beyond data length
		let out_of_range_cursor = encoder.encode(100).unwrap();

		// Act
		let result = paginator.paginate(
			&items,
			Some(&out_of_range_cursor),
			"http://example.com/items",
		);

		// Assert - should return empty results, not panic
		assert!(result.is_ok());
		let response = result.unwrap();
		assert!(response.results.is_empty());
	}

	#[rstest]
	fn paginate_returns_empty_results_for_cursor_on_empty_dataset() {
		// Arrange
		let encoder =
			encoder::Base64CursorEncoder::with_secret_key(b"test-secret-key-for-unit-tests!!");
		let paginator = CursorPagination::new().with_encoder(
			encoder::Base64CursorEncoder::with_secret_key(b"test-secret-key-for-unit-tests!!"),
		);
		let items: Vec<i32> = vec![];
		let cursor = encoder.encode(0).unwrap();

		// Act
		let result = paginator.paginate(&items, Some(&cursor), "http://example.com/items");

		// Assert
		assert!(result.is_ok());
		let response = result.unwrap();
		assert!(response.results.is_empty());
		assert_eq!(response.count, 0);
	}
}
