//! Page number based pagination implementation

use crate::exception::{Error, Result};
use async_trait::async_trait;

use super::core::{AsyncPaginator, Page, PaginatedResponse, Paginator, SchemaParameter};

/// Custom error messages for pagination
#[derive(Debug, Clone)]
pub struct ErrorMessages {
	/// Error message for invalid page number (not parseable as integer)
	pub invalid_page: String,
	/// Error message for page number less than 1
	pub min_page: String,
	/// Error message for page number beyond available pages
	pub no_results: String,
}

impl Default for ErrorMessages {
	fn default() -> Self {
		Self {
			invalid_page: "Invalid page number".to_string(),
			min_page: "That page number is less than 1".to_string(),
			no_results: "That page contains no results".to_string(),
		}
	}
}

/// Page number based pagination
///
/// Example URLs:
/// - `http://api.example.org/accounts/?page=4`
/// - `http://api.example.org/accounts/?page=4&page_size=100`
#[derive(Debug, Clone)]
pub struct PageNumberPagination {
	/// Default page size
	pub page_size: usize,
	/// Query parameter name for page number
	pub page_query_param: String,
	/// Query parameter name for page size (optional)
	pub page_size_query_param: Option<String>,
	/// Maximum allowed page size
	pub max_page_size: Option<usize>,
	/// Strings that represent the last page
	pub last_page_strings: Vec<String>,
	/// Minimum number of items allowed on the last page
	/// If the last page has fewer items than this, they are merged with the previous page
	pub orphans: usize,
	/// Whether to allow an empty first page
	pub allow_empty_first_page: bool,
	/// Custom error messages
	pub error_messages: ErrorMessages,
}

impl Default for PageNumberPagination {
	fn default() -> Self {
		Self {
			page_size: 10,
			page_query_param: "page".to_string(),
			page_size_query_param: None,
			max_page_size: None,
			last_page_strings: vec!["last".to_string()],
			orphans: 0,
			allow_empty_first_page: true,
			error_messages: ErrorMessages::default(),
		}
	}
}

impl PageNumberPagination {
	/// Creates a new PageNumberPagination with default settings
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::pagination::PageNumberPagination;
	///
	/// let paginator = PageNumberPagination::new();
	/// assert_eq!(paginator.page_size, 10);
	/// assert_eq!(paginator.page_query_param, "page");
	/// ```
	pub fn new() -> Self {
		Self::default()
	}
	/// Sets the default page size for pagination
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::pagination::PageNumberPagination;
	///
	/// let paginator = PageNumberPagination::new().page_size(20);
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
	/// use reinhardt_core::pagination::PageNumberPagination;
	///
	/// let paginator = PageNumberPagination::new()
	///     .page_size(10)
	///     .max_page_size(100);
	/// assert_eq!(paginator.max_page_size, Some(100));
	/// ```
	pub fn max_page_size(mut self, size: usize) -> Self {
		self.max_page_size = Some(size);
		self
	}
	/// Sets the query parameter name for custom page size
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::pagination::PageNumberPagination;
	///
	/// let paginator = PageNumberPagination::new()
	///     .page_size_query_param("limit");
	/// assert_eq!(paginator.page_size_query_param, Some("limit".to_string()));
	/// ```
	pub fn page_size_query_param(mut self, param: impl Into<String>) -> Self {
		self.page_size_query_param = Some(param.into());
		self
	}
	/// Sets the minimum number of items allowed on the last page
	///
	/// If the last page has fewer items than this, they are merged with the previous page
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::pagination::PageNumberPagination;
	///
	/// let paginator = PageNumberPagination::new()
	///     .page_size(10)
	///     .orphans(3);
	/// assert_eq!(paginator.orphans, 3);
	/// ```
	pub fn orphans(mut self, orphans: usize) -> Self {
		self.orphans = orphans;
		self
	}
	/// Sets whether to allow an empty first page
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::pagination::PageNumberPagination;
	///
	/// let paginator = PageNumberPagination::new()
	///     .allow_empty_first_page(false);
	/// assert!(!paginator.allow_empty_first_page);
	/// ```
	pub fn allow_empty_first_page(mut self, allow: bool) -> Self {
		self.allow_empty_first_page = allow;
		self
	}
	/// Sets custom error messages for pagination errors
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::pagination::{PageNumberPagination, ErrorMessages};
	///
	/// let messages = ErrorMessages {
	///     invalid_page: "Invalid page!".to_string(),
	///     min_page: "Page too low!".to_string(),
	///     no_results: "No results!".to_string(),
	/// };
	/// let paginator = PageNumberPagination::new()
	///     .error_messages(messages);
	/// assert_eq!(paginator.error_messages.invalid_page, "Invalid page!");
	/// ```
	pub fn error_messages(mut self, messages: ErrorMessages) -> Self {
		self.error_messages = messages;
		self
	}
	/// Get a page, returning a valid page even with invalid arguments
	///
	/// This is a lenient version that:
	/// - Returns the first page if the page number is invalid (not parseable)
	/// - Returns the last page if the page number is out of range
	/// - Never returns an error
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::pagination::{PageNumberPagination, Page};
	///
	/// let paginator = PageNumberPagination::new().page_size(5);
	/// let items: Vec<i32> = (1..=20).collect();
	///
	/// // Get page 2
	/// let page = paginator.get_page(&items, Some("2"));
	/// assert_eq!(page.number, 2);
	/// assert_eq!(page.len(), 5);
	///
	/// // Invalid page number defaults to page 1
	/// let page = paginator.get_page(&items, Some("invalid"));
	/// assert_eq!(page.number, 1);
	///
	/// // Out of range page number returns last page
	/// let page = paginator.get_page(&items, Some("100"));
	/// assert_eq!(page.number, 4);
	/// ```
	pub fn get_page<T: Clone>(&self, items: &[T], page_param: Option<&str>) -> Page<T> {
		let total_count = items.len();

		// Calculate total pages (same logic as paginate)
		let total_pages = if total_count <= self.page_size {
			1
		} else {
			let pages = total_count / self.page_size;
			let remainder = total_count % self.page_size;
			if remainder > 0 && remainder <= self.orphans {
				pages
			} else if remainder > 0 {
				pages + 1
			} else {
				pages
			}
		};

		// Parse page number with fallback
		let page_number = page_param
			.and_then(|p| self.parse_page_number(p, total_pages).ok())
			.unwrap_or(1); // Default to 1 if parsing fails

		// Clamp page number to valid range
		let page_number = if page_number > total_pages && total_count > 0 {
			total_pages // Return last page if out of range
		} else if page_number == 0 {
			1
		} else {
			page_number
		};

		// Calculate offsets
		let (start, end) = if total_count == 0 {
			(0, 0)
		} else if page_number == total_pages {
			let start = (page_number - 1) * self.page_size;
			(start, total_count)
		} else {
			let start = (page_number - 1) * self.page_size;
			let end = std::cmp::min(start + self.page_size, total_count);
			(start, end)
		};

		let object_list = items[start..end].to_vec();

		Page {
			object_list,
			number: page_number,
			num_pages: total_pages,
			count: total_count,
			page_size: self.page_size,
		}
	}
	/// Async version of get_page
	///
	/// Returns a valid page even with invalid arguments, never errors.
	/// This is the async equivalent of Django's `aget_page()`.
	///
	/// # Note
	/// For in-memory operations (current implementation), this simply calls
	/// the sync version since no I/O is involved.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::pagination::PageNumberPagination;
	///
	/// async fn example() {
	///     let paginator = PageNumberPagination::new().page_size(5);
	///     let items: Vec<i32> = (1..=20).collect();
	///
	///     let page = paginator.aget_page(&items, Some("2")).await;
	///     assert_eq!(page.number, 2);
	///     assert_eq!(page.len(), 5);
	/// }
	/// ```
	pub async fn aget_page<T: Clone + Send + Sync>(
		&self,
		items: &[T],
		page_param: Option<&str>,
	) -> Page<T> {
		self.get_page(items, page_param)
	}

	fn parse_page_number(&self, page_str: &str, total_pages: usize) -> Result<usize> {
		// Check if it's a "last" page string
		if self.last_page_strings.iter().any(|s| s == page_str) {
			return Ok(total_pages);
		}

		// Try to parse as integer first
		if let Ok(n) = page_str.parse::<usize>() {
			if n == 0 {
				return Err(Error::InvalidPage(self.error_messages.min_page.clone()));
			}
			return Ok(n);
		}

		// Try to parse as float and convert to integer
		if let Ok(f) = page_str.parse::<f64>() {
			// Check if it's a valid integer float (e.g., 1.0, 2.0)
			if f.fract() == 0.0 && f > 0.0 {
				let n = f as usize;
				if n == 0 {
					return Err(Error::InvalidPage(self.error_messages.min_page.clone()));
				}
				return Ok(n);
			}
		}

		// If all parsing failed, return error
		Err(Error::InvalidPage(self.error_messages.invalid_page.clone()))
	}

	fn build_url(&self, base_url: &str, page: usize) -> String {
		let url = url::Url::parse(base_url)
			.unwrap_or_else(|_| url::Url::parse(&format!("http://localhost{}", base_url)).unwrap());

		let mut new_url = url.clone();
		new_url
			.query_pairs_mut()
			.clear()
			.append_pair(&self.page_query_param, &page.to_string());

		// Copy other query parameters
		for (key, value) in url.query_pairs() {
			if key != self.page_query_param {
				new_url.query_pairs_mut().append_pair(&key, &value);
			}
		}

		new_url.to_string()
	}
}

#[async_trait]
impl Paginator for PageNumberPagination {
	fn paginate<T: Clone + Send + Sync>(
		&self,
		items: &[T],
		page_param: Option<&str>,
		base_url: &str,
	) -> Result<PaginatedResponse<T>> {
		let total_count = items.len();

		// Handle empty list with allow_empty_first_page=false
		if total_count == 0 && !self.allow_empty_first_page {
			return Err(Error::InvalidPage(self.error_messages.no_results.clone()));
		}

		// Calculate total pages considering orphans
		let total_pages = if total_count == 0 {
			if self.allow_empty_first_page { 1 } else { 0 }
		} else {
			// Calculate pages with orphans consideration
			if total_count <= self.page_size {
				1
			} else {
				let pages = total_count / self.page_size;
				let remainder = total_count % self.page_size;

				// If remainder is small enough (orphans), merge with previous page
				if remainder > 0 && remainder <= self.orphans {
					// Merge with previous page
					pages
				} else if remainder > 0 {
					// Create new page
					pages + 1
				} else {
					pages
				}
			}
		};

		// Get page number
		let page_number = if let Some(param) = page_param {
			self.parse_page_number(param, total_pages)?
		} else {
			1
		};

		// Validate page number
		if page_number > total_pages && total_count > 0 {
			return Err(Error::InvalidPage(self.error_messages.no_results.clone()));
		}

		// Calculate offset considering orphans
		let (start, end) = if total_count == 0 {
			(0, 0)
		} else if page_number == total_pages {
			// Last page: might include orphans from calculation
			let start = (page_number - 1) * self.page_size;
			(start, total_count)
		} else {
			let start = (page_number - 1) * self.page_size;
			let end = std::cmp::min(start + self.page_size, total_count);
			(start, end)
		};

		// Get page results
		let results = items[start..end].to_vec();

		// Build next/previous links
		let next = if page_number < total_pages {
			Some(self.build_url(base_url, page_number + 1))
		} else {
			None
		};

		let previous = if page_number > 1 {
			Some(self.build_url(base_url, page_number - 1))
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
			name: self.page_query_param.clone(),
			required: false,
			location: "query".to_string(),
			description: "A page number within the paginated result set.".to_string(),
			schema_type: "integer".to_string(),
		}];

		if let Some(ref page_size_param) = self.page_size_query_param {
			params.push(SchemaParameter {
				name: page_size_param.clone(),
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
impl AsyncPaginator for PageNumberPagination {
	async fn apaginate<T: Clone + Send + Sync>(
		&self,
		items: &[T],
		page_param: Option<&str>,
		base_url: &str,
	) -> Result<PaginatedResponse<T>> {
		// For in-memory operations, just call the sync version
		self.paginate(items, page_param, base_url)
	}

	fn get_schema_parameters(&self) -> Vec<SchemaParameter> {
		Paginator::get_schema_parameters(self)
	}
}
