//! Pagination functionality for tables

/// Pagination configuration
#[derive(Debug, Clone)]
pub struct Pagination {
	/// Number of items per page
	pub per_page: usize,
	/// Current page number (1-indexed)
	pub current_page: usize,
	/// Total number of items
	pub total_items: usize,
}

impl Pagination {
	/// Creates a new pagination configuration
	///
	/// # Arguments
	///
	/// * `per_page` - Number of items per page
	pub fn new(per_page: usize) -> Self {
		Self {
			per_page,
			current_page: 1,
			total_items: 0,
		}
	}

	/// Returns the total number of pages
	pub fn total_pages(&self) -> usize {
		if self.total_items == 0 {
			0
		} else {
			self.total_items.div_ceil(self.per_page)
		}
	}

	/// Returns the start index for the current page (0-indexed)
	pub fn start_index(&self) -> usize {
		(self.current_page.saturating_sub(1)) * self.per_page
	}

	/// Returns the end index for the current page (exclusive, 0-indexed)
	pub fn end_index(&self) -> usize {
		(self.start_index() + self.per_page).min(self.total_items)
	}

	/// Moves to the next page if available
	pub fn next_page(&mut self) -> bool {
		if self.current_page < self.total_pages() {
			self.current_page += 1;
			true
		} else {
			false
		}
	}

	/// Moves to the previous page if available
	pub fn prev_page(&mut self) -> bool {
		if self.current_page > 1 {
			self.current_page -= 1;
			true
		} else {
			false
		}
	}

	/// Sets the current page
	pub fn set_page(&mut self, page: usize) {
		self.current_page = page.max(1).min(self.total_pages().max(1));
	}
}
