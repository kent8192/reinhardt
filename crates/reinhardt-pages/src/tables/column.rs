//! Column trait and basic implementation

use crate::dom::Element;
use std::any::Any;

/// Represents a table column
///
/// This trait defines the core functionality for table columns including
/// rendering, ordering, and metadata access.
pub trait Column {
	/// Returns the column name (internal field name)
	fn name(&self) -> &str;

	/// Returns the column display label
	fn label(&self) -> &str;

	/// Renders the column value as an Element
	fn render(&self, value: &dyn Any) -> Element;

	/// Returns whether this column is orderable (sortable)
	fn is_orderable(&self) -> bool {
		true
	}

	/// Returns whether this column is visible
	fn is_visible(&self) -> bool {
		true
	}

	/// Returns custom HTML attributes for the column header
	fn header_attrs(&self) -> Vec<(&str, &str)> {
		vec![]
	}

	/// Returns custom HTML attributes for the column cell
	fn cell_attrs(&self) -> Vec<(&str, &str)> {
		vec![]
	}
}
