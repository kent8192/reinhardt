//! CheckBox column type implementation

use crate::builder::html::{input, td};
use crate::dom::Element;
use crate::tables::column::Column as ColumnTrait;
use std::any::Any;

/// Column with checkbox
///
/// This column type renders checkboxes for row selection.
pub struct CheckBoxColumn {
	name: String,
	label: String,
	orderable: bool,
	visible: bool,
}

impl CheckBoxColumn {
	/// Creates a new checkbox column
	pub fn new(name: impl Into<String>, label: impl Into<String>) -> Self {
		Self {
			name: name.into(),
			label: label.into(),
			orderable: false,
			visible: true,
		}
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

impl ColumnTrait for CheckBoxColumn {
	fn name(&self) -> &str {
		&self.name
	}

	fn label(&self) -> &str {
		&self.label
	}

	fn render(&self, value: &dyn Any) -> Element {
		let checkbox = input().attr("type", "checkbox");
		let checkbox = if let Some(&val) = value.downcast_ref::<bool>() {
			if val {
				checkbox.attr("checked", "checked")
			} else {
				checkbox
			}
		} else {
			checkbox
		};
		td().child(checkbox.build()).build()
	}

	fn is_orderable(&self) -> bool {
		self.orderable
	}

	fn is_visible(&self) -> bool {
		self.visible
	}
}
