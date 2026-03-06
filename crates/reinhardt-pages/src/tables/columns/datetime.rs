//! DateTime column type implementation

use crate::builder::html::td;
use crate::dom::Element;
use crate::tables::column::Column as ColumnTrait;
use std::any::Any;

/// Column for date/time values
///
/// This column type renders date/time values with customizable formatting.
pub struct DateTimeColumn {
	name: String,
	label: String,
	format: String,
	orderable: bool,
	visible: bool,
}

impl DateTimeColumn {
	/// Creates a new datetime column
	pub fn new(name: impl Into<String>, label: impl Into<String>) -> Self {
		Self {
			name: name.into(),
			label: label.into(),
			format: "%Y-%m-%d %H:%M:%S".to_string(),
			orderable: true,
			visible: true,
		}
	}

	/// Sets the datetime format string
	pub fn format(mut self, format: impl Into<String>) -> Self {
		self.format = format.into();
		self
	}

	/// Sets whether this column is orderable
	pub fn orderable(mut self, orderable: bool) -> Self {
		self.orderable = orderable;
		self
	}

	/// Sets whether this column is visible
	pub fn visible(mut self, visible: bool) -> Self {
		self.visible = visible;
		self
	}
}

impl ColumnTrait for DateTimeColumn {
	fn name(&self) -> &str {
		&self.name
	}

	fn label(&self) -> &str {
		&self.label
	}

	fn render(&self, value: &dyn Any) -> Element {
		#[cfg(feature = "chrono")]
		{
			if let Some(dt) = value.downcast_ref::<chrono::NaiveDateTime>() {
				return td().text(&dt.format(&self.format).to_string()).build();
			}
			if let Some(dt) = value.downcast_ref::<chrono::DateTime<chrono::Utc>>() {
				return td().text(&dt.format(&self.format).to_string()).build();
			}
		}
		if let Some(s) = value.downcast_ref::<String>() {
			td().text(s).build()
		} else {
			td().text("-").build()
		}
	}

	fn is_orderable(&self) -> bool {
		self.orderable
	}

	fn is_visible(&self) -> bool {
		self.visible
	}
}
