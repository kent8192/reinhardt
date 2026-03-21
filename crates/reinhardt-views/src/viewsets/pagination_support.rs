//! Pagination support for ViewSets
//!
//! Provides automatic pagination integration for list actions in ViewSets.

use async_trait::async_trait;
use reinhardt_core::pagination::{
	CursorPagination, LimitOffsetPagination, PageNumberPagination, PaginatedResponse, Paginator,
};
use reinhardt_http::{Request, Result};
use serde::Serialize;

/// Pagination configuration for ViewSets
#[derive(Debug, Clone)]
pub enum PaginationConfig {
	/// Page number based pagination (default: page_size=10, max_page_size=100)
	PageNumber {
		/// Number of items per page.
		page_size: usize,
		/// Maximum allowed page size.
		max_page_size: Option<usize>,
	},
	/// Limit/offset based pagination (default: default_limit=10, max_limit=100)
	LimitOffset {
		/// Default number of items to return.
		default_limit: usize,
		/// Maximum allowed limit.
		max_limit: Option<usize>,
	},
	/// Cursor based pagination for large datasets
	Cursor {
		/// Number of items per page.
		page_size: usize,
		/// Field name used for cursor ordering.
		ordering_field: String,
	},
	/// No pagination - return all results
	None,
}

impl Default for PaginationConfig {
	fn default() -> Self {
		Self::PageNumber {
			page_size: 10,
			max_page_size: Some(100),
		}
	}
}

impl PaginationConfig {
	/// Create page number pagination with custom settings
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_views::viewsets::PaginationConfig;
	///
	/// let config = PaginationConfig::page_number(20, Some(200));
	/// ```
	pub fn page_number(page_size: usize, max_page_size: Option<usize>) -> Self {
		Self::PageNumber {
			page_size,
			max_page_size,
		}
	}

	/// Create limit/offset pagination with custom settings
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_views::viewsets::PaginationConfig;
	///
	/// let config = PaginationConfig::limit_offset(25, Some(500));
	/// ```
	pub fn limit_offset(default_limit: usize, max_limit: Option<usize>) -> Self {
		Self::LimitOffset {
			default_limit,
			max_limit,
		}
	}

	/// Create cursor pagination with custom settings
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_views::viewsets::PaginationConfig;
	///
	/// let config = PaginationConfig::cursor(50, "created_at");
	/// ```
	pub fn cursor(page_size: usize, ordering_field: impl Into<String>) -> Self {
		Self::Cursor {
			page_size,
			ordering_field: ordering_field.into(),
		}
	}

	/// Disable pagination - return all results
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_views::viewsets::PaginationConfig;
	///
	/// let config = PaginationConfig::none();
	/// ```
	pub fn none() -> Self {
		Self::None
	}
}

/// Trait for ViewSets that support pagination
#[async_trait]
pub trait PaginatedViewSet: Send + Sync {
	/// Get pagination configuration for this ViewSet
	///
	/// Returns `None` to disable pagination, or a `PaginationConfig` to enable it.
	fn get_pagination_config(&self) -> Option<PaginationConfig> {
		Some(PaginationConfig::default())
	}

	/// Paginate a list of items based on the request and configuration
	///
	/// This method is automatically called by list actions when pagination is enabled.
	async fn paginate_queryset<T: Serialize + Clone + Send + Sync>(
		&self,
		items: Vec<T>,
		request: &Request,
	) -> Result<PaginatedResponse<T>> {
		let config = self.get_pagination_config().unwrap_or_default();

		// Extract page parameter and base URL from request
		let query_string = request.uri.query().unwrap_or("");
		let base_url = request
			.uri
			.path_and_query()
			.map(|pq| pq.path())
			.unwrap_or("/");

		match config {
			PaginationConfig::PageNumber {
				page_size,
				max_page_size,
			} => {
				let mut paginator = PageNumberPagination::new().page_size(page_size);
				if let Some(max) = max_page_size {
					paginator = paginator.max_page_size(max);
				}
				paginator.paginate(&items, Some(query_string), base_url)
			}
			PaginationConfig::LimitOffset {
				default_limit,
				max_limit,
			} => {
				let mut paginator = LimitOffsetPagination::new().default_limit(default_limit);
				if let Some(max) = max_limit {
					paginator = paginator.max_limit(max);
				}
				paginator.paginate(&items, Some(query_string), base_url)
			}
			PaginationConfig::Cursor {
				page_size,
				ordering_field: _,
			} => {
				let paginator = CursorPagination::new().page_size(page_size);
				paginator.paginate(&items, Some(query_string), base_url)
			}
			PaginationConfig::None => {
				// No pagination - return all items
				Ok(PaginatedResponse {
					count: items.len(),
					next: None,
					previous: None,
					results: items,
				})
			}
		}
	}
}
