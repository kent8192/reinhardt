//! Filtering functionality for tables

/// Trait for filterable tables
pub trait Filterable {
	/// Applies a filter to the table
	fn filter_by(&mut self, field: &str, value: &str);

	/// Clears all filters
	fn clear_filters(&mut self);

	/// Returns the current filters
	fn current_filters(&self) -> Vec<(&str, &str)>;
}
