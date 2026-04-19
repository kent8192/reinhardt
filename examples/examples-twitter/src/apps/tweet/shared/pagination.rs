//! Pagination types for tweet application
//!
//! Provides standardized pagination response format following
//! the cookbook patterns (docs/cookbook/pagination.en.md).

use serde::{Deserialize, Serialize};

/// Paginated response wrapper
///
/// Standard format for paginated API responses with metadata
/// for navigation (next/previous links) and total count.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaginatedResponse<T> {
	/// Total number of items across all pages
	pub count: usize,
	/// URL for the next page, if available
	pub next: Option<String>,
	/// URL for the previous page, if available
	pub previous: Option<String>,
	/// Items on the current page
	pub results: Vec<T>,
}

impl<T> PaginatedResponse<T> {
	/// Create a new paginated response
	///
	/// # Arguments
	///
	/// * `results` - Items for the current page
	/// * `total_count` - Total number of items across all pages
	/// * `page` - Current page number (1-indexed)
	/// * `page_size` - Number of items per page
	/// * `base_url` - Base URL for generating next/previous links
	pub fn new(
		results: Vec<T>,
		total_count: usize,
		page: usize,
		page_size: usize,
		base_url: &str,
	) -> Self {
		let num_pages = total_count.div_ceil(page_size);

		Self {
			count: total_count,
			next: if page < num_pages {
				Some(format!("{}?page={}", base_url, page + 1))
			} else {
				None
			},
			previous: if page > 1 {
				Some(format!("{}?page={}", base_url, page - 1))
			} else {
				None
			},
			results,
		}
	}
}

/// Query parameters for pagination
///
/// Deserializes pagination parameters from query string.
/// Provides sensible defaults and validation.
#[derive(Debug, Clone, Deserialize)]
pub struct PageQuery {
	/// Page number (1-indexed), defaults to 1
	pub page: Option<usize>,
	/// Number of items per page, defaults to DEFAULT_PAGE_SIZE
	pub page_size: Option<usize>,
}

impl PageQuery {
	/// Default number of items per page
	pub const DEFAULT_PAGE_SIZE: usize = 20;
	/// Maximum allowed page size to prevent excessive queries
	pub const MAX_PAGE_SIZE: usize = 100;

	/// Get the page number, defaulting to 1
	pub fn page(&self) -> usize {
		self.page.unwrap_or(1)
	}

	/// Get the page size with bounds checking
	///
	/// Returns the requested page size clamped to MAX_PAGE_SIZE,
	/// or DEFAULT_PAGE_SIZE if not specified.
	pub fn page_size(&self) -> usize {
		self.page_size
			.unwrap_or(Self::DEFAULT_PAGE_SIZE)
			.min(Self::MAX_PAGE_SIZE)
	}

	/// Calculate the offset for database queries
	///
	/// Returns the number of items to skip based on
	/// the current page and page size.
	pub fn offset(&self) -> usize {
		(self.page() - 1) * self.page_size()
	}
}
