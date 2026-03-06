//! Table trait and implementation

use crate::column::Column;
use crate::error::Result;
use std::collections::HashMap;

/// Represents the sort order for a column
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortOrder {
	/// Sort in ascending order
	Ascending,
	/// Sort in descending order
	Descending,
}

/// Configuration for table sorting
#[derive(Debug, Clone)]
pub struct SortConfig {
	/// Field name to sort by
	pub field: String,
	/// Sort order
	pub order: SortOrder,
}

/// Configuration for table pagination
#[derive(Debug, Clone)]
pub struct PaginationConfig {
	/// Current page number (1-indexed)
	pub page: usize,
	/// Number of items per page
	pub per_page: usize,
}

/// Trait for table implementations
///
/// This trait defines the core interface for data tables in Reinhardt.
/// Implementations should provide methods for accessing rows, columns,
/// and applying sorting, filtering, and pagination.
pub trait Table {
	/// The type of rows in this table
	type Row;

	/// Returns the columns definition for this table
	fn columns(&self) -> &[Box<dyn Column<Row = Self::Row>>];

	/// Returns all rows in the table (before filtering/pagination)
	fn rows(&self) -> &[Self::Row];

	/// Returns the current sort configuration
	fn sort_config(&self) -> Option<&SortConfig>;

	/// Returns the current pagination configuration
	fn pagination_config(&self) -> Option<&PaginationConfig>;

	/// Returns the current filter configuration
	fn filters(&self) -> &HashMap<String, String>;

	/// Applies sorting to the table
	fn sort_by(&mut self, field: &str, order: SortOrder) -> Result<()>;

	/// Applies pagination to the table
	fn paginate(&mut self, page: usize, per_page: usize) -> Result<()>;

	/// Applies filters to the table
	fn filter(&mut self, filters: HashMap<String, String>) -> Result<()>;

	/// Returns the filtered and paginated rows
	fn visible_rows(&self) -> Vec<&Self::Row>;

	/// Returns the total number of rows (before filtering)
	fn total_rows(&self) -> usize {
		self.rows().len()
	}

	/// Returns the number of visible rows (after filtering but before pagination)
	fn filtered_rows_count(&self) -> usize;

	/// Returns the total number of pages
	fn total_pages(&self) -> usize {
		if let Some(pagination) = self.pagination_config() {
			let filtered_count = self.filtered_rows_count();
			filtered_count.div_ceil(pagination.per_page)
		} else {
			1
		}
	}
}

/// A basic table implementation
///
/// This is a simple implementation of the `Table` trait that stores rows
/// and applies sorting, filtering, and pagination.
#[derive(Debug)]
pub struct SimpleTable<R> {
	columns: Vec<Box<dyn Column<Row = R>>>,
	rows: Vec<R>,
	sort_config: Option<SortConfig>,
	pagination_config: Option<PaginationConfig>,
	filters: HashMap<String, String>,
}

impl<R> SimpleTable<R> {
	/// Creates a new empty table
	pub fn new() -> Self {
		Self {
			columns: Vec::new(),
			rows: Vec::new(),
			sort_config: None,
			pagination_config: None,
			filters: HashMap::new(),
		}
	}

	/// Creates a new table with the given rows
	pub fn with_rows(rows: Vec<R>) -> Self {
		Self {
			columns: Vec::new(),
			rows,
			sort_config: None,
			pagination_config: None,
			filters: HashMap::new(),
		}
	}

	/// Adds a column to the table
	pub fn add_column(&mut self, column: Box<dyn Column<Row = R>>) {
		self.columns.push(column);
	}
}

impl<R> Default for SimpleTable<R> {
	fn default() -> Self {
		Self::new()
	}
}

impl<R> Table for SimpleTable<R> {
	type Row = R;

	fn columns(&self) -> &[Box<dyn Column<Row = Self::Row>>] {
		&self.columns
	}

	fn rows(&self) -> &[Self::Row] {
		&self.rows
	}

	fn sort_config(&self) -> Option<&SortConfig> {
		self.sort_config.as_ref()
	}

	fn pagination_config(&self) -> Option<&PaginationConfig> {
		self.pagination_config.as_ref()
	}

	fn filters(&self) -> &HashMap<String, String> {
		&self.filters
	}

	fn sort_by(&mut self, field: &str, order: SortOrder) -> Result<()> {
		// Validate column exists and is sortable
		let column = self
			.columns
			.iter()
			.find(|col| col.name() == field)
			.ok_or_else(|| crate::error::TableError::ColumnNotFound(field.to_string()))?;

		if !column.sortable() {
			return Err(crate::error::TableError::InvalidSortOrder(format!(
				"Column '{}' is not sortable",
				field
			)));
		}

		self.sort_config = Some(SortConfig {
			field: field.to_string(),
			order,
		});
		Ok(())
	}

	fn paginate(&mut self, page: usize, per_page: usize) -> Result<()> {
		if page == 0 {
			return Err(crate::error::TableError::InvalidPageNumber(page));
		}
		if per_page == 0 {
			return Err(crate::error::TableError::InvalidPerPage(per_page));
		}
		self.pagination_config = Some(PaginationConfig { page, per_page });
		Ok(())
	}

	fn filter(&mut self, filters: HashMap<String, String>) -> Result<()> {
		// Validate all filter columns exist and are filterable
		for field in filters.keys() {
			let column = self
				.columns
				.iter()
				.find(|col| col.name() == field)
				.ok_or_else(|| crate::error::TableError::ColumnNotFound(field.to_string()))?;

			if !column.filterable() {
				return Err(crate::error::TableError::ColumnNotFilterable(
					field.to_string(),
				));
			}
		}

		self.filters = filters;
		Ok(())
	}

	fn visible_rows(&self) -> Vec<&Self::Row> {
		let mut rows: Vec<&Self::Row> = self.rows.iter().collect();

		// Apply filtering
		if !self.filters.is_empty() {
			rows.retain(|row| {
				self.filters.iter().all(|(field, filter_value)| {
					if let Some(column) = self.columns.iter().find(|col| col.name() == field) {
						let rendered = column.render(row);
						rendered.contains(filter_value.as_str())
					} else {
						false
					}
				})
			});
		}

		// Apply sorting
		if let Some(sort_config) = &self.sort_config
			&& let Some(column) = self
				.columns
				.iter()
				.find(|col| col.name() == sort_config.field)
		{
			rows.sort_by(|a, b| {
				let a_value = column.render(a);
				let b_value = column.render(b);
				match sort_config.order {
					SortOrder::Ascending => a_value.cmp(&b_value),
					SortOrder::Descending => b_value.cmp(&a_value),
				}
			});
		}

		// Apply pagination
		if let Some(pagination) = &self.pagination_config {
			let start = (pagination.page - 1) * pagination.per_page;
			rows.into_iter()
				.skip(start)
				.take(pagination.per_page)
				.collect()
		} else {
			rows
		}
	}

	fn filtered_rows_count(&self) -> usize {
		if self.filters.is_empty() {
			return self.rows.len();
		}

		self.rows
			.iter()
			.filter(|row| {
				self.filters.iter().all(|(field, filter_value)| {
					if let Some(column) = self.columns.iter().find(|col| col.name() == field) {
						let rendered = column.render(row);
						rendered.contains(filter_value.as_str())
					} else {
						false
					}
				})
			})
			.count()
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_sort_order_equality() {
		assert_eq!(SortOrder::Ascending, SortOrder::Ascending);
		assert_eq!(SortOrder::Descending, SortOrder::Descending);
		assert_ne!(SortOrder::Ascending, SortOrder::Descending);
	}
}
