//! Table trait and basic implementation

use super::column::Column;
use super::sorting::SortDirection;
use crate::dom::Element;

/// Represents a table with rows and columns
///
/// This trait defines the core functionality for tables including
/// data access, rendering, sorting, and pagination.
pub trait Table {
	/// The type of data rows in this table
	type Row;

	/// Returns all rows in the table
	fn rows(&self) -> Vec<&Self::Row>;

	/// Returns all columns in the table
	fn columns(&self) -> Vec<&dyn Column>;

	/// Renders the table as an Element for HTML output
	fn render(&self) -> Element;

	/// Handles sorting by the specified field and direction
	fn handle_sort(&mut self, field: &str, direction: SortDirection);

	/// Handles pagination by setting the current page
	fn handle_pagination(&mut self, page: usize);

	/// Returns the total number of rows
	fn total_rows(&self) -> usize {
		self.rows().len()
	}

	/// Returns the current page number (1-indexed)
	fn current_page(&self) -> usize {
		1
	}

	/// Returns the number of rows per page
	fn per_page(&self) -> usize {
		10
	}

	/// Returns whether the table supports sorting
	fn is_sortable(&self) -> bool {
		true
	}

	/// Returns whether the table supports pagination
	fn is_paginated(&self) -> bool {
		false
	}
}
