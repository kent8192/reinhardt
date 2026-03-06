//! Base column trait and implementation

use std::fmt::Debug;

/// Trait for table column definitions
///
/// This trait defines how a column extracts and renders data from a row.
/// Each column is responsible for:
/// - Providing a name and header text
/// - Extracting data from a row
/// - Rendering the data as a string (for HTML or export)
/// - Specifying if the column is sortable or filterable
pub trait Column: Debug {
	/// The type of rows this column operates on
	type Row;

	/// Returns the name of this column
	///
	/// This is used as the identifier for sorting and filtering
	fn name(&self) -> &str;

	/// Returns the header text for this column
	///
	/// This is displayed in the table header
	fn header(&self) -> &str;

	/// Renders the column value for the given row
	///
	/// This is called for each cell in the table
	fn render(&self, row: &Self::Row) -> String;

	/// Returns whether this column can be sorted
	///
	/// Default: true
	fn sortable(&self) -> bool {
		true
	}

	/// Returns whether this column can be filtered
	///
	/// Default: true
	fn filterable(&self) -> bool {
		true
	}

	/// Returns CSS classes to apply to cells in this column
	///
	/// Default: empty string
	fn css_classes(&self) -> &str {
		""
	}
}

/// A basic column implementation using a function to extract values
///
/// # Example
///
/// ```rust
/// use reinhardt_tables::column::BaseColumn;
///
/// struct User {
///     id: i32,
///     name: String,
/// }
///
/// let name_column = BaseColumn::new(
///     "name",
///     "User Name",
///     |user: &User| user.name.clone(),
/// );
/// ```
pub struct BaseColumn<R, F>
where
	F: Fn(&R) -> String,
{
	name: String,
	header: String,
	extractor: F,
	sortable: bool,
	filterable: bool,
	css_classes: String,
	_phantom: std::marker::PhantomData<R>,
}

impl<R, F> BaseColumn<R, F>
where
	F: Fn(&R) -> String,
{
	/// Creates a new base column
	pub fn new(name: impl Into<String>, header: impl Into<String>, extractor: F) -> Self {
		Self {
			name: name.into(),
			header: header.into(),
			extractor,
			sortable: true,
			filterable: true,
			css_classes: String::new(),
			_phantom: std::marker::PhantomData,
		}
	}

	/// Sets whether this column is sortable
	pub fn sortable(mut self, sortable: bool) -> Self {
		self.sortable = sortable;
		self
	}

	/// Sets whether this column is filterable
	pub fn filterable(mut self, filterable: bool) -> Self {
		self.filterable = filterable;
		self
	}

	/// Sets CSS classes for this column
	pub fn css_classes(mut self, css_classes: impl Into<String>) -> Self {
		self.css_classes = css_classes.into();
		self
	}
}

impl<R, F> Debug for BaseColumn<R, F>
where
	F: Fn(&R) -> String,
{
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("BaseColumn")
			.field("name", &self.name)
			.field("header", &self.header)
			.field("sortable", &self.sortable)
			.field("filterable", &self.filterable)
			.field("css_classes", &self.css_classes)
			.finish_non_exhaustive()
	}
}

impl<R, F> Column for BaseColumn<R, F>
where
	R: Debug,
	F: Fn(&R) -> String,
{
	type Row = R;

	fn name(&self) -> &str {
		&self.name
	}

	fn header(&self) -> &str {
		&self.header
	}

	fn render(&self, row: &Self::Row) -> String {
		(self.extractor)(row)
	}

	fn sortable(&self) -> bool {
		self.sortable
	}

	fn filterable(&self) -> bool {
		self.filterable
	}

	fn css_classes(&self) -> &str {
		&self.css_classes
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[derive(Debug)]
	struct TestRow {
		value: String,
	}

	#[test]
	fn test_base_column_creation() {
		let column = BaseColumn::new("test", "Test Column", |row: &TestRow| row.value.clone());
		assert_eq!(column.name(), "test");
		assert_eq!(column.header(), "Test Column");
		assert_eq!(Column::sortable(&column), true);
		assert_eq!(Column::filterable(&column), true);
	}

	#[test]
	fn test_base_column_render() {
		let column = BaseColumn::new("test", "Test", |row: &TestRow| row.value.clone());
		let row = TestRow {
			value: "Hello".to_string(),
		};
		assert_eq!(column.render(&row), "Hello");
	}

	#[test]
	fn test_base_column_builder() {
		let column = BaseColumn::new("test", "Test", |row: &TestRow| row.value.clone())
			.sortable(false)
			.filterable(false)
			.css_classes("custom-class");

		// Use Column trait methods (not builder methods)
		assert_eq!(Column::sortable(&column), false);
		assert_eq!(Column::filterable(&column), false);
		assert_eq!(Column::css_classes(&column), "custom-class");
	}
}
