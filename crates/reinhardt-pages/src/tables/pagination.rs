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

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::*;

	#[rstest]
	fn pagination_clamps_and_moves_at_boundaries() {
		// Arrange
		let mut empty = Pagination::new(5);
		empty.set_page(99);
		let mut partial_last = Pagination::new(5);
		partial_last.total_items = 11;
		partial_last.set_page(99);
		let mut page_zero = Pagination::new(5);
		page_zero.total_items = 11;
		page_zero.set_page(0);
		let mut first = Pagination::new(5);
		first.total_items = 11;
		let mut last = Pagination::new(5);
		last.total_items = 11;
		last.set_page(3);

		// Act
		let empty_state = (
			empty.total_pages(),
			empty.start_index(),
			empty.end_index(),
			empty.current_page,
			empty.next_page(),
		);
		let partial_last_state = (
			partial_last.total_pages(),
			partial_last.start_index(),
			partial_last.end_index(),
			partial_last.current_page,
			partial_last.next_page(),
		);
		let page_zero_state = (
			page_zero.total_pages(),
			page_zero.start_index(),
			page_zero.end_index(),
			page_zero.current_page,
			page_zero.prev_page(),
		);
		let first_state = (
			first.total_pages(),
			first.start_index(),
			first.end_index(),
			first.current_page,
			first.next_page(),
		);
		let last_state = (
			last.total_pages(),
			last.start_index(),
			last.end_index(),
			last.current_page,
			last.prev_page(),
		);

		// Assert
		assert_eq!(empty_state, (0, 0, 0, 1, false));
		assert_eq!(partial_last_state, (3, 10, 11, 3, false));
		assert_eq!(page_zero_state, (3, 0, 5, 1, false));
		assert_eq!(first_state, (3, 0, 5, 1, true));
		assert_eq!(last_state, (3, 10, 11, 3, true));
		assert_eq!(first.current_page, 2);
		assert_eq!(last.current_page, 2);
	}
}
