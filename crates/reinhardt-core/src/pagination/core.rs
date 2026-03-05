//! Core pagination types and traits

use crate::exception::{Error, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Represents pagination metadata
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PaginationMetadata {
	pub count: usize,
	pub next: Option<String>,
	pub previous: Option<String>,
}

/// Paginated response wrapper
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PaginatedResponse<T> {
	pub count: usize,
	pub next: Option<String>,
	pub previous: Option<String>,
	pub results: Vec<T>,
}

impl<T> PaginatedResponse<T> {
	/// Creates a new paginated response with results and metadata.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::pagination::{PaginatedResponse, PaginationMetadata};
	///
	/// let metadata = PaginationMetadata {
	///     count: 100,
	///     next: Some("/api/items?page=2".to_string()),
	///     previous: None,
	/// };
	/// let items = vec![1, 2, 3, 4, 5];
	/// let response = PaginatedResponse::new(items, metadata);
	/// assert_eq!(response.count, 100);
	/// assert_eq!(response.results.len(), 5);
	/// ```
	pub fn new(results: Vec<T>, metadata: PaginationMetadata) -> Self {
		Self {
			count: metadata.count,
			next: metadata.next,
			previous: metadata.previous,
			results,
		}
	}
}

/// Represents a single page of results
#[derive(Debug, Clone)]
pub struct Page<T> {
	/// Items in this page
	pub object_list: Vec<T>,
	/// Current page number (1-indexed)
	pub number: usize,
	/// Total number of pages
	pub num_pages: usize,
	/// Total number of items across all pages
	pub count: usize,
	/// Items per page
	pub page_size: usize,
}

impl<T: Clone> Page<T> {
	/// Creates a new page with the given parameters.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::pagination::Page;
	///
	/// let items = vec!["item1", "item2", "item3"];
	/// let page = Page::new(items, 1, 10, 30, 3);
	/// assert_eq!(page.number, 1);
	/// assert_eq!(page.num_pages, 10);
	/// assert_eq!(page.count, 30);
	/// assert_eq!(page.object_list.len(), 3);
	/// ```
	pub fn new(
		object_list: Vec<T>,
		number: usize,
		num_pages: usize,
		count: usize,
		page_size: usize,
	) -> Self {
		Self {
			object_list,
			number,
			num_pages,
			count,
			page_size,
		}
	}
	/// Returns the 1-based index of the first item on this page
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::pagination::Page;
	///
	/// let items = vec!["a", "b", "c"];
	/// let page = Page::new(items, 2, 5, 15, 3);
	/// assert_eq!(page.start_index(), 4); // (2-1) * 3 + 1 = 4
	/// ```
	pub fn start_index(&self) -> usize {
		if self.object_list.is_empty() {
			0
		} else {
			(self.number - 1) * self.page_size + 1
		}
	}
	/// Returns the 1-based index of the last item on this page
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::pagination::Page;
	///
	/// let items = vec!["a", "b", "c"];
	/// let page = Page::new(items, 2, 5, 15, 3);
	/// assert_eq!(page.end_index(), 6); // start_index (4) + len (3) - 1 = 6
	/// ```
	pub fn end_index(&self) -> usize {
		if self.object_list.is_empty() {
			0
		} else {
			self.start_index() + self.object_list.len() - 1
		}
	}
	/// Returns true if there is a next page
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::pagination::Page;
	///
	/// let items = vec![1, 2, 3];
	/// let page = Page::new(items, 2, 5, 50, 10);
	/// assert!(page.has_next());
	///
	/// let last_page = Page::new(vec![1], 5, 5, 50, 10);
	/// assert!(!last_page.has_next());
	/// ```
	pub fn has_next(&self) -> bool {
		self.number < self.num_pages
	}
	/// Returns true if there is a previous page
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::pagination::Page;
	///
	/// let items = vec![1, 2, 3];
	/// let page = Page::new(items, 2, 5, 50, 10);
	/// assert!(page.has_previous());
	///
	/// let first_page = Page::new(vec![1], 1, 5, 50, 10);
	/// assert!(!first_page.has_previous());
	/// ```
	pub fn has_previous(&self) -> bool {
		self.number > 1
	}
	/// Returns true if there are other pages (previous or next)
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::pagination::Page;
	///
	/// let middle_page = Page::new(vec![1, 2, 3], 2, 5, 50, 10);
	/// assert!(middle_page.has_other_pages());
	///
	/// let only_page = Page::new(vec![1], 1, 1, 1, 10);
	/// assert!(!only_page.has_other_pages());
	/// ```
	pub fn has_other_pages(&self) -> bool {
		self.has_previous() || self.has_next()
	}
	/// Returns the next page number
	///
	/// # Errors
	/// Returns `InvalidPage` if there is no next page
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::pagination::Page;
	///
	/// let page = Page::new(vec![1, 2, 3], 2, 5, 50, 10);
	/// assert_eq!(page.next_page_number().unwrap(), 3);
	///
	/// let last_page = Page::new(vec![1], 5, 5, 50, 10);
	/// assert!(last_page.next_page_number().is_err());
	/// ```
	pub fn next_page_number(&self) -> Result<usize> {
		if self.has_next() {
			Ok(self.number + 1)
		} else {
			Err(Error::Validation(
				"That page contains no results".to_string(),
			))
		}
	}
	/// Returns the previous page number
	///
	/// # Errors
	/// Returns `InvalidPage` if there is no previous page
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::pagination::Page;
	///
	/// let page = Page::new(vec![1, 2, 3], 3, 5, 50, 10);
	/// assert_eq!(page.previous_page_number().unwrap(), 2);
	///
	/// let first_page = Page::new(vec![1], 1, 5, 50, 10);
	/// assert!(first_page.previous_page_number().is_err());
	/// ```
	pub fn previous_page_number(&self) -> Result<usize> {
		if self.has_previous() {
			Ok(self.number - 1)
		} else {
			Err(Error::InvalidPage(
				"That page number is less than 1".to_string(),
			))
		}
	}
	/// Returns the length of items in this page
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::pagination::Page;
	///
	/// let items = vec!["a", "b", "c"];
	/// let page = Page::new(items, 1, 3, 10, 5);
	/// assert_eq!(page.len(), 3);
	/// ```
	pub fn len(&self) -> usize {
		self.object_list.len()
	}
	/// Returns true if this page contains no items
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::pagination::Page;
	///
	/// let empty_page: Page<i32> = Page::new(vec![], 1, 1, 0, 10);
	/// assert!(empty_page.is_empty());
	///
	/// let page = Page::new(vec![1], 1, 1, 1, 10);
	/// assert!(!page.is_empty());
	/// ```
	pub fn is_empty(&self) -> bool {
		self.object_list.is_empty()
	}
	/// Returns an iterator over all page numbers (1-indexed)
	///
	/// This is equivalent to Django's `paginator.page_range`
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::pagination::Page;
	///
	/// let page = Page::new(vec![1], 2, 5, 50, 10);
	/// let pages: Vec<usize> = page.page_range().collect();
	/// assert_eq!(pages, vec![1, 2, 3, 4, 5]);
	/// ```
	pub fn page_range(&self) -> std::ops::RangeInclusive<usize> {
		1..=self.num_pages
	}
	/// Returns an elided page range with ellipsis for long ranges
	///
	/// This is equivalent to Django's `paginator.get_elided_page_range()`
	///
	/// # Arguments
	/// * `on_each_side` - Number of pages on each side of current page (default: 3)
	/// * `on_ends` - Number of pages on start and end (default: 2)
	///
	/// # Returns
	/// Vector of page numbers or `None` (representing ellipsis)
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::pagination::Page;
	///
	/// // For a large page range, ellipsis (None) are added
	/// let page = Page::new(vec![1], 10, 20, 200, 10);
	/// let elided = page.get_elided_page_range(2, 2);
	/// // Result like: [Some(1), Some(2), None, Some(8), Some(9), Some(10), Some(11), Some(12), None, Some(19), Some(20)]
	/// assert!(elided.contains(&None)); // Contains ellipsis
	/// assert!(elided.contains(&Some(10))); // Contains current page
	/// ```
	pub fn get_elided_page_range(&self, on_each_side: usize, on_ends: usize) -> Vec<Option<usize>> {
		let mut result = Vec::new();

		// If we have few enough pages, don't elide
		let needed_pages = on_each_side * 2 + 1 + on_ends * 2;
		if self.num_pages <= needed_pages {
			return (1..=self.num_pages).map(Some).collect();
		}

		// Start pages
		for i in 1..=on_ends {
			if i <= self.num_pages {
				result.push(Some(i));
			}
		}

		// Left ellipsis
		let left_start = self.number.saturating_sub(on_each_side);
		if left_start > on_ends + 1 {
			result.push(None); // Ellipsis
		}

		// Middle pages
		let middle_start = std::cmp::max(on_ends + 1, left_start);
		let middle_end = std::cmp::min(self.num_pages - on_ends, self.number + on_each_side);

		for i in middle_start..=middle_end {
			if i > on_ends && i <= self.num_pages - on_ends {
				result.push(Some(i));
			}
		}

		// Right ellipsis
		if middle_end < self.num_pages - on_ends {
			result.push(None); // Ellipsis
		}

		// End pages
		for i in (self.num_pages - on_ends + 1)..=self.num_pages {
			if i > middle_end {
				result.push(Some(i));
			}
		}

		result
	}
	/// Get an item by index
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::pagination::Page;
	///
	/// let items = vec!["a", "b", "c"];
	/// let page = Page::new(items, 1, 1, 3, 10);
	/// assert_eq!(page.get(0), Some(&"a"));
	/// assert_eq!(page.get(2), Some(&"c"));
	/// assert_eq!(page.get(3), None);
	/// ```
	pub fn get(&self, index: usize) -> Option<&T> {
		self.object_list.get(index)
	}
	/// Get a slice of items
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::pagination::Page;
	///
	/// let items = vec!["a", "b", "c", "d", "e"];
	/// let page = Page::new(items, 1, 1, 5, 10);
	/// assert_eq!(page.get_slice(1..4), Some(&["b", "c", "d"][..]));
	/// assert_eq!(page.get_slice(0..2), Some(&["a", "b"][..]));
	/// ```
	pub fn get_slice(&self, range: std::ops::Range<usize>) -> Option<&[T]> {
		self.object_list.get(range)
	}
}

// Implement Index trait for direct indexing
impl<T: Clone> std::ops::Index<usize> for Page<T> {
	type Output = T;

	fn index(&self, index: usize) -> &Self::Output {
		&self.object_list[index]
	}
}

// Implement IntoIterator for Page
impl<T: Clone> IntoIterator for Page<T> {
	type Item = T;
	type IntoIter = std::vec::IntoIter<T>;

	fn into_iter(self) -> Self::IntoIter {
		self.object_list.into_iter()
	}
}

// Implement IntoIterator for &Page
impl<'a, T: Clone> IntoIterator for &'a Page<T> {
	type Item = &'a T;
	type IntoIter = std::slice::Iter<'a, T>;

	fn into_iter(self) -> Self::IntoIter {
		self.object_list.iter()
	}
}

/// Schema parameter for OpenAPI/documentation
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SchemaParameter {
	pub name: String,
	pub required: bool,
	pub location: String,
	pub description: String,
	pub schema_type: String,
}

/// Trait for pagination implementations
#[async_trait]
pub trait Paginator: Send + Sync {
	/// Paginate the given items based on request parameters
	fn paginate<T: Clone + Send + Sync>(
		&self,
		items: &[T],
		page_param: Option<&str>,
		base_url: &str,
	) -> Result<PaginatedResponse<T>>;

	/// Get the pagination metadata for schema generation
	fn get_schema_parameters(&self) -> Vec<SchemaParameter> {
		Vec::new()
	}
}

/// Async version of Paginator trait
///
/// This trait provides async pagination support, equivalent to Django's AsyncPaginator.
/// For in-memory operations, this simply wraps the sync implementation.
/// This trait becomes useful when integrating with async data sources (databases, APIs).
#[async_trait]
pub trait AsyncPaginator: Send + Sync {
	/// Async version of paginate
	///
	/// Paginate the given items based on request parameters asynchronously.
	async fn apaginate<T: Clone + Send + Sync>(
		&self,
		items: &[T],
		page_param: Option<&str>,
		base_url: &str,
	) -> Result<PaginatedResponse<T>>;

	/// Get the pagination metadata for schema generation
	/// (Same as sync version - no I/O involved)
	fn get_schema_parameters(&self) -> Vec<SchemaParameter> {
		Vec::new()
	}
}
